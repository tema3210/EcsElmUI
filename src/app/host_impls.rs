pub mod default {
    use crate::traits::{Hosts, System};
    use std::any::{Any, TypeId};
    use std::collections::{BTreeMap,HashMap};
    use crate::traits::{Context, GlobalState};
    use std::future::Future;
    use crate::errors::traits::{AllocError,ReduceError};
    use futures::task::SpawnExt;
    use futures::{FutureExt, TryFutureExt, StreamExt};
    use std::marker::PhantomData;
    use typemap::{Entry, TypeMap};
    use std::convert::TryFrom;
    use bitmaps::Bitmap;
    use std::collections::btree_map::Entry as BEntry;
    use std::collections::hash_map::Entry as HEntry;
    use winit::event::WindowEvent;

    pub struct Host {
        /// free ids
        ids: BTreeMap<u32,bitmaps::Bitmap<1024>>,
        /// global states of systems
        states: typemap::TypeMap,
        /// a map from entities to their components, event filters
        data: BTreeMap<usize,(typemap::TypeMap, ProcessingFunctionsEntity)>,
        /// collection of reducer functions, one for each system
        msg_reducers: HashMap<TypeId,Box<dyn FnMut(&mut Self)>>,
        /// a futures runtime.
        runtime: futures::executor::ThreadPool,
    }

    /// the functions to interact with systems in type erased setting
    struct ProcessingFunctionsEntity {
        event_dispatch: Box<dyn for<'s> Fn(&'s <Host as crate::traits::Host>::Event,&'s mut typemap::TypeMap)>,
    }

    pub struct EntityData<S: System<Host>> {
        messages: Vec<S::Message>,
        data: S,
    }

    impl<S: System<Host>> EntityData<S> {
        fn new(data: S) -> Self {
            Self {
                data,
                messages: vec![],
            }
        }
        fn push(&mut self,msg: S::Message) {
            self.messages.push(msg);
        }

        fn reduce(&mut self,ctx: &mut HostCtx<'_>) {
            let mut vec = Vec::new();
            std::mem::swap(&mut vec, &mut self.messages);
            for i in vec {
                S::update(&mut self.data,i,ctx)
            }
        }
    }

    pub struct SystemData<S: System<Host>>{
        state: S::State,
        future_handles: Vec<(usize,Box<dyn Future<Output = S::Message>>)>,
    }

    struct SystemHolder<S: System<Host> + Any>(PhantomData<S>);

    impl<S: System<Host> + Any> typemap::Key for SystemHolder<S>
    {
        type Value = SystemData<S>;
    }

    struct EntityHolder<S: System<Host> + Any>(PhantomData<S>);

    impl<S: System<Host> + Any> typemap::Key for EntityHolder<S>
    {
        type Value = EntityData<S>;
    }

    impl Host {
        pub fn new() -> Self {
            let runtime = futures::executor::ThreadPoolBuilder::new()
                .pool_size(4)
                .create().expect("failed to create a thread pool");
            Self {
                ids: BTreeMap::new(),
                states: typemap::TypeMap::new(),
                data: BTreeMap::new(),
                msg_reducers: Default::default(),
                runtime,
            }
        }

        pub(crate) fn spawn_fut<T: 'static + Send,F: FnOnce(T) -> S::Message + 'static, Fut: Future<Output=T> + Send + 'static,S: System<Self>>(&mut self,fut: Fut,f: F, whom: usize) -> bool
            where Self: Hosts<S>,
        {
            match self.states.entry::<SystemHolder<S>>() {
                Entry::Occupied(mut e) => {
                    let s = e.get_mut();
                    let handle = self.runtime.spawn_with_handle(fut).unwrap().map(f);
                    s.future_handles.push((whom,Box::new(handle)));
                    true
                },
                Entry::Vacant(_) => false,
            }
        }

        pub(crate) fn with_entity_data<S: System<Self>,T,F: FnOnce(&mut EntityData<S>) -> T >(&mut self, which: usize,f: F) -> Option<T> where Self: Hosts<S>
        {
            self.data.get_mut(&which).map(|(m,_)| match m.entry::<EntityHolder<S>>() {
                Entry::Occupied(mut e) => {
                    Some(f(e.get_mut()))
                },
                _ => {
                    None
                }
            }).flatten()
        }
        pub(crate) fn with_system_data<S: System<Self>,T,F: FnOnce(&mut SystemData<S>)-> T>(&mut self,f: F) -> Option<T> where Self: Hosts<S> {
            match self.states.entry::<SystemHolder<S>>() {
                Entry::Occupied(mut e) => {
                    Some(f(e.get_mut()))
                }
                Entry::Vacant(_) => None,
            }
        }
        pub(crate) fn with_system_and_entity_data<S: System<Host>,T,F: FnOnce(&mut SystemData<S>,&mut EntityData<S>) -> T>(&mut self,which: usize,f: F)-> Option<T> where Self: Hosts<S> {
            let (data, states) = (&mut self.data,&mut self.states);
            match (data.get_mut(&which),states) {
                (Some((data,_)),state) => {
                    let (data,state) = (data.entry::<EntityHolder<S>>(),state.entry::<SystemHolder<S>>());
                    match (data,state) {
                        (Entry::Occupied(mut data),Entry::Occupied(mut state)) => {
                            let res = f(state.get_mut(),data.get_mut());
                            Some(res)
                        },
                        (_, _) => None
                    }
                },
                _ => None,
            }
        }
    }

    impl crate::traits::Host for Host {
        type Index = usize;

        type Event = winit::event::WindowEvent<'static>;

        fn allocate_entity(&mut self) -> Result<Self::Index, crate::errors::traits::AllocError> {
            const HALFWORD: u8 = (usize::BITS / 2) as u8;
            const MASK: usize = usize::MAX >> HALFWORD;

            let mut res = None;
            for (k,v) in self.ids.iter_mut() {
                if *v.as_value() == [0u128;8] {
                    continue
                } else {
                    match v.first_index() {
                        None => unreachable!(),
                        Some(idx) => {
                            let bit = v.set(idx,false);
                            assert_eq!(bit,true);
                            //idx here is the position in bitmap.
                            //res = half a word bits of `k` left and half a word bits of idx right
                            res = Some( ( ((*k as usize & ! MASK) << HALFWORD) & !MASK ) | ( idx & MASK));
                            break;
                        }
                    }
                }
            };
            res.ok_or(AllocError)
        }

        fn drop_entity(&mut self, which: Self::Index) {
            const HALFWORD: u8 = (usize::BITS / 2) as u8;
            const MASK: usize = usize::MAX >> HALFWORD;

            let (left,right) = (u32::try_from((which >> HALFWORD) & MASK).unwrap(),which & MASK);
            match self.ids.entry(left) {
                BEntry::Vacant(_) => {},
                BEntry::Occupied(mut e) => {
                    let bm = e.get_mut();
                    bm.set(right,false);
                    self.data.remove(&which);
                }
            }
        }

        fn receive_events(&mut self, events: &[Self::Event]) {
            for (_,(tm,f)) in self.data.iter_mut() {
                let mut f = |ev| (f.event_dispatch)(ev,tm);
                for ev in events.iter() {
                    f(ev);
                }
            }
        }
        //TODO: make the borrow checker happy
        fn update_round(&mut self) {
            for (_,red) in self.msg_reducers.iter_mut() {
                red(self)
            }
        }
    }

    impl<S: crate::traits::System<Self>> Hosts<S> for Host
    {
        fn get_state(&mut self, which: Self::Index) -> Option<&mut S> {
            self.data.get_mut(&which).map(|(tm,_)|{
                tm.get_mut::<EntityHolder<S>>().map(|data| &mut data.data)
            }).flatten()
        }

        fn subscribe(&mut self, who: Self::Index, with: <S as System<Self>>::Props) {
            let reducer = <EntityData<S>>::reduce;
            //todo: make the borrow checker happy 2
            let reducer2 = move |hst: &mut Host| {
                for which in hst.data.keys().cloned() {
                    hst.with_entity_data(
                        which,
                        |h| reducer(h,&mut HostCtx{cur_index: which,host: hst})
                    );
                };
            };
            //here we accumulated a reducer for subscribers system.
            match self.msg_reducers.entry(TypeId::of::<S>()) {
                HEntry::Occupied(_) => {}
                HEntry::Vacant(mut e) => {
                    e.insert(Box::new(reducer2));
                }
            }

            let component = EntityData {data: S::changed(None,&with), messages: vec![]};
            self.data.get_mut(&who).map(|(map,_)| map.insert::<EntityHolder<S>>(component));
        }

        fn unsubscribe(&mut self, who: Self::Index) {
            use std::collections::btree_map::Entry::*;
            match self.data.entry(who) {
                Occupied(c) => {
                    c.remove();
                },
                _ => {},
            };
        }
    }

    pub struct HostCtx<'h> {
        host: &'h mut Host,
        cur_index: usize,
    }

    impl<'h> crate::traits::Context<'h,Host> for HostCtx<'h> {
        fn get_host(&mut self) -> &mut dyn crate::traits::Host<Event = winit::event::WindowEvent<'static>,Index = usize> {
            self.host
        }

        fn get_current_index(&mut self) -> usize {
            self.cur_index
        }

        fn send<S: System<Host>>(&mut self, msg: <S as System<Host>>::Message, whom: usize) where Host: Hosts<S> {
            self.host.with_entity_data::<S,(),_>(whom,|x| { x.messages.push(msg);});
        }

        fn subscribe<S: System<Host>>(&mut self, filter: fn(&<Host as crate::traits::Host>::Event) -> Option<<S as System<Host>>::Message>) where Host: Hosts<S> {
            let index = self.cur_index;
            let reducer = move |ev: &winit::event::WindowEvent<'static>,e_data: &mut typemap::TypeMap| -> () {
                if let Some(m) = filter(ev) {
                    match e_data.entry::<EntityHolder<S>>() {
                        Entry::Occupied(mut e) => {
                            let e = e.get_mut();
                            e.push(m);
                        }
                        Entry::Vacant(_) => {}
                    }
                };
            };
            match self.host.data.entry(index) {
                BEntry::Vacant(_) => {}
                BEntry::Occupied(mut e) => {
                    e.get_mut().1.event_dispatch = Box::new(reducer);
                }
            }
        }

        fn spawn<T: 'static + Send, F, Fut, S: System<Host>>(&mut self, fut: Fut, f: F,whom: usize) -> bool
            where Fut: Future<Output=T> + Send + 'static, F: FnOnce(T) -> S::Message + 'static, Host: Hosts<S>
        {
            self.host.spawn_fut(fut,f,whom)
        }

        fn with_state<S: System<Host>, T, F: FnOnce(&mut S::State) -> T>(&mut self,f: F) -> Option<T> where Host: Hosts<S> {
            self.host.with_system_data::<S,T,_>(|s| {
                f(&mut s.state)
            })
        }
    }
}
