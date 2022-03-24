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
    use std::task::{Poll, Waker, RawWaker, RawWakerVTable};
    use std::pin::Pin;
    use winit::event::VirtualKeyCode::Wake;
    use std::sync::Arc;
    use std::path::Path;
    use crate::traits::render::{StyleShadow, StyleChange, Style, StyleTable,Anchor,Viewport,Filling,self};

    type EntityStorage = BTreeMap<usize,(typemap::TypeMap, ProcessingFunctionsEntity)>;

    pub struct Host {
        /// free ids
        ids: BTreeMap<u32,bitmaps::Bitmap<1024>>,
        /// global states of systems
        states: typemap::TypeMap,
        /// a map from entities to their components, event filters
        data: EntityStorage,
        /// a map from entities to their views
        data_view: BTreeMap<usize,ViewData<Host>>,
        /// collection of reducer functions, one for each system
        msg_reducers: HashMap<TypeId,std::sync::Arc<dyn Fn(&mut Self)>>,
        /// collection of Future resolvers
        future_delivery: HashMap<TypeId,std::sync::Arc<dyn Fn(&mut TypeMap,&mut EntityStorage)>>,
        /// a futures runtime.
        runtime: futures::executor::ThreadPool,
    }

    mod default_style_table;

    pub struct ViewData<H: crate::traits::Host> {
        // a list of anchors
        anchors: Vec<render::Anchor>,
        // a map from anchor, to its layout, it the last is set
        layouts: HashMap<render::Anchor,render::Layout<H>>,
        // an original size of view
        vp: render::Viewport,
        // a table of styles
        styles: default_style_table::DefaultStyleTable,
    }


    impl<H: crate::traits::Host> crate::traits::View<H> for ViewData<H> {
        fn anchors(&self) -> &[Anchor] {
            &self.anchors[..]
        }

        fn set_layout(&mut self, anc: Anchor, filling: Option<render::Layout<H>>) {
            if self.anchors.iter().find(|&i| i.0  == anc.0).is_some() {
                if let Some(filling) = filling {
                    self.layouts.insert(anc, filling);
                } else {
                    self.layouts.remove(&anc);
                }
            }
            //Do nothing if smth. tries to fill non existent anchor
        }

        fn viewport(&self) -> Viewport {
            self.vp
        }

        fn get_style_table(&self) -> &dyn StyleTable {
            &self.styles as &dyn StyleTable
        }

        fn get_style_table_mut(&mut self) -> &mut dyn StyleTable {
            &mut self.styles as &mut dyn StyleTable
        }
    }

    /// the functions to interact with systems in type erased setting
    struct ProcessingFunctionsEntity {
        event_dispatch: Box<dyn for<'s> Fn(&'s <Host as crate::traits::Host>::Event,&'s mut typemap::TypeMap)>,
        // poll_fn: Box<dyn for<'s> Fn(&'s mut typemap::TypeMap)>,
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

        fn reduce<'a>(&'a mut self,ctx: &mut impl crate::traits::Context<'a,Host>) {
            let mut vec = Vec::new();
            std::mem::swap(&mut vec, &mut self.messages);
            for i in vec {
                S::update(&mut self.data,i,ctx)
            }
        }
    }

    pub struct SystemData<S: System<Host>>{
        state: S::State,
        future_handles: Vec<(usize,Pin<Box<dyn Future<Output = S::Message>>>)>,
    }

    /// This does nothing
    static R_W_VTable: RawWakerVTable = RawWakerVTable::new(
        |waker_ptr| unsafe { core::ptr::read(waker_ptr as *const RawWaker) },
        |_p|{},
        |_p|{},
        |_p|{},
    );

    /// This is wrapper of the above
    impl<S: System<Host>> SystemData<S> {
        fn poll(&mut self,host: &mut EntityStorage) {
            //init
            let mut rw: RawWaker = RawWaker::new(core::ptr::null(),&R_W_VTable);
            //patch in correct reference
            rw = RawWaker::new(&rw as *const _ as *const (),&R_W_VTable);

            // SAFETY the only usage of this waker is `wake`, which does nothing.
            // todo: revisit it later (20.03.22)
            let waker = unsafe { Waker::from_raw(rw) };

            for (to,fut) in self.future_handles.iter_mut() {
                match fut.as_mut().poll(&mut core::task::Context::from_waker(&waker)) {
                    Poll::Ready(msg) => {
                        match host.entry(*to) {
                            BEntry::Occupied(mut e) => {
                                match e.get_mut().0.entry::<EntityHolder<S>>() {
                                    Entry::Occupied(mut e) => e.get_mut().push(msg),
                                    Entry::Vacant(_) => continue,
                                }
                            },
                            BEntry::Vacant(_) => continue,
                        }
                    },
                    Poll::Pending => continue,
                }
            }
        }
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
                data_view: Default::default(),
                msg_reducers: Default::default(),
                future_delivery: HashMap::new(),
                runtime,
            }
        }
        pub(crate) fn spawn_fut<T: 'static + Send,F: FnOnce(T) -> S::Message + 'static, Fut: Future<Output=T> + Send + 'static,S: System<Self>>(&mut self,fut: Fut,f: F, whom: usize) -> bool
            where Self: Hosts<S>,
        {
            let clo = |states: &mut TypeMap,store: &mut EntityStorage| {
                match states.entry::<SystemHolder<S>>() {
                    Entry::Occupied(mut e) => {
                        e.get_mut().poll(store);
                    },
                    Entry::Vacant(_) => {},
                }
            };
            match self.future_delivery.entry(TypeId::of::<S>()) {
                HEntry::Occupied(_) => {},
                HEntry::Vacant(mut e) => {
                    //insert the processing function
                    e.insert(Arc::new(clo));
                }
            };
            match self.states.entry::<SystemHolder<S>>() {
                Entry::Occupied(mut e) => {
                    let s = e.get_mut();
                    let handle = self.runtime.spawn_with_handle(fut).unwrap().map(f);
                    s.future_handles.push((whom,Box::pin(handle)));
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
        type EntityView = ViewData<Host>;

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

        fn set_entity_data(&mut self, which: Self::Index, data: ViewData<Self>) {
            self.data_view.insert(which,data);
        }

        // todo: clean the data stored in accounting structures
        fn drop_entity(&mut self, which: Self::Index) {
            const HALFWORD: u8 = (usize::BITS / 2) as u8;
            const MASK: usize = usize::MAX >> HALFWORD;

            let (left,right) = (u32::try_from((which >> HALFWORD) & MASK).unwrap(),which & MASK);
            match self.ids.entry(left) {
                BEntry::Vacant(_) => {},
                BEntry::Occupied(mut e) => {
                    let bm = e.get_mut();
                    bm.set(right,false);
                    /// clean up
                    self.data.remove(&which);
                    self.data_view.remove(&which);
                }
            }
        }
        //TODO: think of dispatch between currently rendered components
        fn receive_events(&mut self, events: &[Self::Event]) {
            for (_,(tm,f)) in self.data.iter_mut() {
                let mut f = |ev| (f.event_dispatch)(ev,tm);
                for ev in events.iter() {
                    // here must go filter for mouse events
                    f(ev);
                }
            }
        }
        fn update_round(&mut self) {
            let reducers: Vec<_> = self.msg_reducers.values().cloned().collect();
            for red in reducers {
                red(self)
            };
            let delivery: Vec<_> = self.future_delivery.values().cloned().collect();
            for val in delivery {
                val(&mut self.states,&mut self.data)
            };
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
            let reducer: fn(&mut Host) = move |hst: &mut Host| {
                let keys = hst.data.keys().cloned().collect::<Vec<_>>();
                for wch in keys {
                    if let Some(mut e_data) = {
                        match hst.data.entry(wch) {
                            BEntry::Vacant(_) => None,
                            BEntry::Occupied(mut e) => {
                                let (e,_) = e.get_mut();
                                match e.entry::<EntityHolder<S>>() {
                                    Entry::Occupied(mut ent) => {
                                        // take out of host our reduced data
                                        Some(ent.remove())
                                    }
                                    Entry::Vacant(_) => None
                                }
                            }
                        }
                    } {
                        // create reducing context
                        let mut ctx = HostCtx{
                            cur_index: wch,
                            host: hst,
                            cur_type_id: TypeId::of::<S>(),
                            msgs: Box::new(Vec::<S::Message>::new()),
                        };
                        // reduce our data
                        e_data.reduce(&mut ctx);

                        let mut msgs = ctx.msgs;
                        ctx.msgs = Box::new(());
                        drop(ctx);
                        //now, msgs contains new messages for current component
                        match hst.data.entry(wch) {
                            BEntry::Vacant(_) => {},
                            BEntry::Occupied(mut e) => {
                                let (e,_) = e.get_mut();
                                match e.entry::<EntityHolder<S>>() {
                                    Entry::Occupied(_) => {
                                        unreachable!();
                                    }
                                    // and place our entry back
                                    Entry::Vacant(mut e) => {
                                        std::mem::swap(&mut e_data.messages, msgs.downcast_mut::<Vec<S::Message>>().unwrap());
                                        e.insert(e_data);
                                    },
                                }
                            }
                        }
                    } else {
                        continue;
                    }
                }
            };
            //here we accumulated a reducer for subscribers system.
            match self.msg_reducers.entry(TypeId::of::<S>()) {
                HEntry::Occupied(_) => {}
                HEntry::Vacant(mut e) => {
                    e.insert(std::sync::Arc::new(reducer));
                }
            }

            let component = EntityData {data: S::changed(None,&with).unwrap(), messages: vec![]};
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
        cur_type_id: TypeId,
        msgs: Box<dyn Any>,
    }

    impl<'h> crate::traits::Context<'h,Host> for HostCtx<'h> {
        fn get_host(&mut self) -> &mut Host {
            self.host
        }

        fn get_current_index(&mut self) -> usize {
            self.cur_index
        }

        /// Note, in crate provided `Host` impl it's not possible to send a message
        fn send<S: System<Host>>(&mut self, msg: <S as System<Host>>::Message, whom: usize) where Host: Hosts<S> {
            if whom == self.cur_index && TypeId::of::<S>() == self.cur_type_id {
                let msgs = self.msgs.downcast_mut::<Vec<S::Message>>().unwrap();
                msgs.push(msg);
            } else {
                self.host.with_entity_data::<S,(),_>(whom,|x| { x.messages.push(msg);});
            }
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
