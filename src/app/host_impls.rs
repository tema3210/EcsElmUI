pub mod default {
    use crate::traits::{Hosts, System};
    use std::any::{Any, TypeId};
    use std::collections::BTreeMap;
    use crate::traits::{Context, GlobalState};
    use std::future::Future;
    use crate::errors::traits::{AllocError,ReduceError};
    use futures::task::SpawnExt;
    use futures::{FutureExt, TryFutureExt, StreamExt};
    use std::marker::PhantomData;
    use typemap::Entry;
    use std::convert::TryFrom;
    use bitmaps::Bitmap;
    use std::collections::btree_map::Entry as BEntry;

    pub struct Host {
        /// free ids
        ids: BTreeMap<u32,bitmaps::Bitmap<1024>>,
        /// global states of systems
        states: typemap::TypeMap,
        /// a map from entities to their components
        data: BTreeMap<usize,typemap::TypeMap>,
        ///a futures runtime.
        runtime: futures::executor::ThreadPool,
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
    }

    pub struct SystemData<S: System<Host>>{
        state: S::State,
        //TODO: get rid of boxing somehow
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
            self.data.get_mut(&which).map(|m| match m.entry::<EntityHolder<S>>() {
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
                (Some(data),state) => {
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

    // TODO: fix the implementation
    impl crate::traits::Host for Host {
        type Index = usize;
        type Event = ();

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
            unimplemented!()
        }

        fn update_round(&mut self) {
            unimplemented!()
        }
    }

    impl<S: crate::traits::System<Self>> Hosts<S> for Host
    {
        fn get_state(&mut self, which: Self::Index) -> Option<&mut S> {
            self.data.get_mut(&which).map(|tm|{
                tm.get_mut::<EntityHolder<S>>().map(|data| &mut data.data)
            }).flatten()
        }

        fn subscribe(&mut self, who: Self::Index, with: <S as System<Self>>::Props) {
            let component = EntityData {data: S::changed(None,&with), messages: vec![]};
            self.data.get_mut(&who).map(|map| map.insert::<EntityHolder<S>>(component));
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
    }

    impl<'h> crate::traits::Context<'h,Host> for HostCtx<'h> {
        fn get_host(&mut self) -> &mut Host {
            self.host
        }

        fn send<S: System<Host>>(&mut self, msg: <S as System<Host>>::Message, whom: usize) where Host: Hosts<S> {
            self.host.with_entity_data::<S,(),_>(whom,|x| { x.messages.push(msg);});
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
