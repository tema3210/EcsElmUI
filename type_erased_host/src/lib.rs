extern crate typemap;
extern crate types;
extern crate futures;

use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use types::traits::{System, Hosts, Host as HostTrait, Context, GlobalState, View};
use types::errors::traits::{AllocError, ReduceError};
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
use types::render::{Anchor, Viewport, StyleTable, self, Visitor, Primitive, ZIndex, Layout};

/// A map from entities to their components data
type EntityStorage = BTreeMap<usize, (typemap::TypeMap, ProcessingFunctionsEntity)>;
/// A map from entities to their view's data
/// index -> set of portal's view data.
type EntityViews = BTreeMap<usize, Vec<(usize, ViewData<Host>)>>;

pub struct Host {
    /// free ids
    ids: BTreeMap<u32, bitmaps::Bitmap<1024>>,
    /// root id
    root: Option<usize>,
    /// global states of systems
    states: typemap::TypeMap,
    /// a map from entities to their components, event filters
    data: EntityStorage,
    /// a map from entities to their views
    data_view: EntityViews,
    /// collection of reducer functions, one for each system
    msg_reducers: HashMap<TypeId, std::sync::Arc<dyn Fn(&mut Self)>>,
    /// collection of Future resolvers
    future_delivery: HashMap<TypeId, std::sync::Arc<dyn Fn(&mut TypeMap, &mut EntityStorage)>>,
    /// a code for producing a views
    views: BTreeMap<usize,std::sync::Arc<dyn Fn(&mut EntityViews,&mut EntityStorage)>>,
    /// a futures runtime.
    runtime: futures::executor::ThreadPool,
}

pub struct ViewData<H: types::traits::Host> {
    /// a list of free anchors
    anchors: Vec<render::Anchor>,
    /// a map from anchor, to its layout, it the last is set
    layouts: HashMap<render::Anchor, (render::Layout<H>, render::ZIndex)>,
    /// an original size of view
    vp: render::Viewport,
    /// a table of styles
    styles: Box<dyn StyleTable<H>>,
    /// a cache for rendered versions of self
    view_cache: lfu::LFUCache<render::Viewport,Self::Primitive>,
}


impl<'a> types::render::Visitor<Host::Primitive> for ViewData<Host>
    where Host: 'a
{
    type Ctx = (&'a mut EntityViews,render::Viewport);

    //todo: implement render logic
    fn visit(&self, ctx: Self::Ctx) -> Host::Primitive {
        unimplemented!()
    }
}

impl<H: types::traits::Host + 'static> types::traits::View<H> for ViewData<H> {
    fn anchors(&self) -> &[Anchor] {
        &self.anchors[..]
    }

    fn set_layout(&mut self, anc: Anchor, filling: Option<render::Layout<H>>, z_index: render::ZIndex) {
        if self.anchors.iter().find(|&i| i.0 == anc.0).is_some() {
            if let Some(filling) = filling {
                self.layouts.insert(anc, (filling, z_index));
            } else {
                self.layouts.remove(&anc);
            }
        }
        //Do nothing if smth. tries to fill non existent anchor
    }

    fn viewport(&self) -> Viewport {
        self.vp
    }

    fn get_style_table(&self) -> &dyn StyleTable<H> {
        &self.styles as &dyn StyleTable<H>
    }

    fn get_style_table_mut(&mut self) -> &mut dyn StyleTable<H> {
        &mut self.styles as &mut dyn StyleTable<H>
    }
}

/// the functions to interact with systems in type erased setting
struct ProcessingFunctionsEntity {
    event_dispatch: Box<dyn for<'s> Fn(&'s <Host as types::traits::Host>::Event, &'s mut typemap::TypeMap)>,
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
    fn push(&mut self, msg: S::Message) {
        self.messages.push(msg);
    }

    fn reduce<'a>(&'a mut self, ctx: &mut impl types::traits::Context<'a, Host>) {
        let mut vec = Vec::new();
        std::mem::swap(&mut vec, &mut self.messages);
        for i in vec {
            S::update(&mut self.data, i, ctx)
        }
    }
}

pub struct SystemData<S: System<Host>> {
    state: S::State,
    future_handles: Vec<(usize, Pin<Box<dyn Future<Output=S::Message>>>)>,
}

/// This does nothing
static R_W_VTable: RawWakerVTable = RawWakerVTable::new(
    |waker_ptr| unsafe { core::ptr::read(waker_ptr as *const RawWaker) },
    |_p| {},
    |_p| {},
    |_p| {},
);

/// This is wrapper of the above
impl<S: System<Host>> SystemData<S> {
    fn poll(&mut self, host: &mut EntityStorage) {
        //init
        let mut rw: RawWaker = RawWaker::new(core::ptr::null(), &R_W_VTable);
        //patch in correct reference
        rw = RawWaker::new(&rw as *const _ as *const (), &R_W_VTable);

        // SAFETY the only usage of this waker is `wake`, which does nothing.
        // todo: revisit it later (20.03.22)
        let waker = unsafe { Waker::from_raw(rw) };

        for (to, fut) in self.future_handles.iter_mut() {
            match fut.as_mut().poll(&mut core::task::Context::from_waker(&waker)) {
                Poll::Ready(msg) => {
                    match host.entry(*to) {
                        BEntry::Occupied(mut e) => {
                            match e.get_mut().0.entry::<EntityHolder<S>>() {
                                Entry::Occupied(mut e) => e.get_mut().push(msg),
                                Entry::Vacant(_) => continue,
                            }
                        }
                        BEntry::Vacant(_) => continue,
                    }
                }
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
            root: None,
            states: typemap::TypeMap::new(),
            data: BTreeMap::new(),
            data_view: Default::default(),
            msg_reducers: Default::default(),
            future_delivery: HashMap::new(),
            views: Default::default(),
            runtime,
        }
    }
    pub(crate) fn spawn_fut<T: 'static + Send, F: FnOnce(T) -> S::Message + 'static, Fut: Future<Output=T> + Send + 'static, S: System<Self>>(&mut self, fut: Fut, f: F, whom: usize) -> bool
        where Self: Hosts<S>,
    {
        let clo = |states: &mut TypeMap, store: &mut EntityStorage| {
            match states.entry::<SystemHolder<S>>() {
                Entry::Occupied(mut e) => {
                    e.get_mut().poll(store);
                }
                Entry::Vacant(_) => {}
            }
        };
        match self.future_delivery.entry(TypeId::of::<S>()) {
            HEntry::Occupied(_) => {}
            HEntry::Vacant(mut e) => {
                //insert the processing function
                e.insert(Arc::new(clo));
            }
        };
        match self.states.entry::<SystemHolder<S>>() {
            Entry::Occupied(mut e) => {
                let s = e.get_mut();
                let handle = self.runtime.spawn_with_handle(fut).unwrap().map(f);
                s.future_handles.push((whom, Box::pin(handle)));
                true
            }
            Entry::Vacant(_) => false,
        }
    }

    pub(crate) fn with_entity_data<S: System<Self>, T, F: FnOnce(&mut EntityData<S>) -> T>(&mut self, which: usize, f: F) -> Option<T> where Self: Hosts<S>
    {
        self.data.get_mut(&which).map(|(m, _)| match m.entry::<EntityHolder<S>>() {
            Entry::Occupied(mut e) => {
                Some(f(e.get_mut()))
            }
            _ => {
                None
            }
        }).flatten()
    }
    pub(crate) fn with_system_data<S: System<Self>, T, F: FnOnce(&mut SystemData<S>) -> T>(&mut self, f: F) -> Option<T> where Self: Hosts<S> {
        match self.states.entry::<SystemHolder<S>>() {
            Entry::Occupied(mut e) => {
                Some(f(e.get_mut()))
            }
            Entry::Vacant(_) => None,
        }
    }
    pub(crate) fn with_system_and_entity_data<S: System<Host>, T, F: FnOnce(&mut SystemData<S>, &mut EntityData<S>) -> T>(&mut self, which: usize, f: F) -> Option<T> where Self: Hosts<S> {
        let (data, states) = (&mut self.data, &mut self.states);
        match (data.get_mut(&which), states) {
            (Some((data, _)), state) => {
                let (data, state) = (data.entry::<EntityHolder<S>>(), state.entry::<SystemHolder<S>>());
                match (data, state) {
                    (Entry::Occupied(mut data), Entry::Occupied(mut state)) => {
                        let res = f(state.get_mut(), data.get_mut());
                        Some(res)
                    }
                    (_, _) => None
                }
            }
            _ => None,
        }
    }
}

//todo: remove
mod stub {
    use types::render::Rect;

    #[derive(Clone, Copy)]
    pub struct Color;

    pub struct Primitive;

    impl super::render::Primitive for Primitive {
        type Color = Color;

        fn copy_from(&mut self, place: Rect<f32>, src: Self) {
            unimplemented!()
        }

        fn cut(&self, part: Rect) -> Self {
            unimplemented!()
        }

        fn resize(&self, scale: (f32, f32)) -> Self {
            unimplemented!()
        }

        fn blank() -> Self {
            unimplemented!()
        }
    }
}

impl types::traits::Host for Host {
    type Index = usize;

    type Event = winit::event::WindowEvent<'static>;

    type EntityData = ViewData<Host>;
    //todo: implement
    type Primitive = stub::Primitive;

    fn allocate_entity(&mut self) -> Result<Self::Index, types::errors::traits::AllocError> {
        const HALFWORD: u8 = (usize::BITS / 2) as u8;
        const MASK: usize = usize::MAX >> HALFWORD;

        let mut res = None;
        for (k, v) in self.ids.iter_mut() {
            if *v.as_value() == [0u128; 8] {
                continue;
            } else {
                match v.first_index() {
                    None => unreachable!(),
                    Some(idx) => {
                        let bit = v.set(idx, false);
                        assert_eq!(bit, true);
                        //idx here is the position in bitmap.
                        //res = half a word bits of `k` left and half a word bits of idx right
                        res = Some((((*k as usize & !MASK) << HALFWORD) & !MASK) | (idx & MASK));
                        break;
                    }
                }
            }
        };
        res.ok_or(AllocError)
    }

    fn set_entity_data(&mut self, which: Self::Index, data: ViewData<Self>, portal: usize) {
        match self.data_view.entry(which) {
            BEntry::Occupied(mut e) => {
                let e = e.get_mut();
                if let Some(pos) = e.iter_mut().find(|(ind, _)| *ind == portal) {
                    pos.1 = data;
                } else {
                    e.push((portal, data));
                }
            }
            BEntry::Vacant(mut e) => {
                e.insert(vec![(portal, data)]);
            }
        }
    }

    fn set_root_entity(&mut self, index: Self::Index) {
        self.root = Some(index);
    }

    // todo: clean the data stored in accounting structures
    fn drop_entity(&mut self, which: Self::Index) {
        const HALFWORD: u8 = (usize::BITS / 2) as u8;
        const MASK: usize = usize::MAX >> HALFWORD;

        let (left, right) = (u32::try_from((which >> HALFWORD) & MASK).unwrap(), which & MASK);
        match self.ids.entry(left) {
            BEntry::Vacant(_) => {}
            BEntry::Occupied(mut e) => {
                let bm = e.get_mut();
                bm.set(right, false);
                /// clean up
                self.data.remove(&which);
                self.data_view.remove(&which);
            }
        }
    }

    fn get_root_portal_count(&self) -> usize {
        let root = self.root.unwrap();
        self.data_view[&root].len()
    }

    fn render(&mut self, screen_idx: usize,vp: render::Viewport, by: impl FnOnce(Self::Primitive)) {
        let view: &[(usize,ViewData<_>)] = &self.data_view[self.root.expect("No root entity set before render")];
        let view = &view.iter().find(|(idx,_)| idx == screen_idx).expect("No such portal of root entity").1;

        let mut primitive = Self::Primitive::blank();
        let mut ctx = (&mut self.data_view,vp);
        let sc = Visitor::visit(view,ctx);
        let copy_rect = render::Rect::zero()
            .upper_left_relative(render::Point::relative(0.0,0.0))
            .down_right_relative(render::Point::relative(100.0,100.0));

        pritimive.copy_from(copy_rect,&sc);
        by(primitive);
    }

    //TODO: think of dispatch between currently rendered components
    fn receive_events(&mut self, events: impl Iterator<Item = Self::Event>) {
        for (_, (tm, f)) in self.data.iter_mut() {
            let mut f = |ev| (f.event_dispatch)(ev, tm);
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
            val(&mut self.states, &mut self.data)
        };
        let views: Vec<_> = self.views.values().cloned().collect();
        for viewer in views {
            viewer(&mut self.data_view,&mut self.data)
        }
    }
}

impl<S: types::traits::System<Self>> Hosts<S> for Host
{
    fn get_state(&mut self, which: Self::Index) -> Option<&mut S> {
        self.data.get_mut(&which).map(|(tm, _)| {
            tm.get_mut::<EntityHolder<S>>().map(|data| &mut data.data)
        }).flatten()
    }
    fn subscribe(&mut self, who: Self::Index, with: <S as System<Self>>::Props) {
        let view_function = move |view: &mut EntityViews,storage: &EntityStorage|{
            let view = view.get_mut(&who).expect("ill-fromed entity data");

            struct Renderer<'v>(&'v mut ViewData<Host>);

            impl<'v> render::Renderer<Host> for Renderer<'v> {
                fn anchors(&mut self) -> &[Anchor] {
                    self.0.anchors()
                }

                fn layout(&mut self, layout: Option<Layout<Host>>, label: Anchor, z_index: ZIndex) {
                    //trasition logic for anchors
                    // the point is, if anchors of a entity are already attached, we simply don't show them as available to the rest of components, and vise versa
                    match layout {
                        Some(layout) => {
                            if let Some(a) = self.0.anchors.iter().enumerate().find(|(a_,a)| a.0 == label.0).map(|(i,_)| i) {
                                let anch = self.0.anchors.swap_remove(a); //should not panic
                                let it = self.0.layouts.insert(anch,(layout,z_index));
                                assert!(it.is_none(),"calling setting of an already set anchors")
                            }
                        },
                        None => {
                            match self.0.anchors.iter().find(|i| i.0 == label.0) {
                                None => {
                                    let i = self.0.layouts.remove(&label);
                                    assert!(i.is_some(),"calling cleaning of un existent anchor")
                                }
                                Some(_) => unreachable!(),
                            }
                        }
                    }
                }

                fn styles(&self) -> &dyn StyleTable<Host> {
                    self.0.get_style_table()
                }

                fn patch_style_scope(&mut self, patch: &mut dyn FnMut(&mut dyn StyleTable<Host>)) {
                    let st_table = self.0.styles.scope();
                    patch(&mut self.0.styles);
                    self.0.styles = st_table;
                }
            }

            for (idx,vd) in view {
                let mut renderer = Renderer(vd);
                match storage.get(&who) {
                    None => {}
                    Some((tm,_)) => {
                        match tm.get::<EntityHolder<S>>() {
                            None => {}
                            Some(sd) => {
                                S::view(&sd.data,&mut renderer,vd.vp,*idx);
                            }
                        }
                    }
                }
            }
        };

        match self.views.entry(who) {
            BEntry::Vacant(mut e) => {
                let vf = Arc::new(view_function);
                e.insert(vf);
            }
            BEntry::Occupied(_) => {}
        }

        let reducer: fn(&mut Host) = move |hst: &mut Host| {
            let keys = hst.data.keys().cloned().collect::<Vec<_>>();
            for wch in keys {
                if let Some(mut e_data) = {
                    match hst.data.entry(wch) {
                        BEntry::Vacant(_) => None,
                        BEntry::Occupied(mut e) => {
                            let (e, _) = e.get_mut();
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
                    let mut ctx = HostCtx {
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
                        BEntry::Vacant(_) => {}
                        BEntry::Occupied(mut e) => {
                            let (e, _) = e.get_mut();
                            match e.entry::<EntityHolder<S>>() {
                                Entry::Occupied(_) => {
                                    unreachable!();
                                }
                                // and place our entry back
                                Entry::Vacant(mut e) => {
                                    std::mem::swap(&mut e_data.messages, msgs.downcast_mut::<Vec<S::Message>>().unwrap());
                                    e.insert(e_data);
                                }
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

        match self.data.entry(who) {
            BEntry::Vacant(mut e) => {
                let component = EntityData { data: S::changed(None, &with).unwrap(), messages: vec![] };
                let mut tm = typemap::TypeMap::new();
                tm.insert::<EntityHolder<S>>(component);
                e.insert((tm,ProcessingFunctionsEntity{ event_dispatch: Box::new(|_,_|{}) }));
            }
            BEntry::Occupied(mut e) => {
                let (mut tm,_) = e.get_mut();
                match tm.remove::<EntityHolder<S>>() {
                    Some(mut ed) => {
                        S::changed(Some(&mut ed.data),&with).unwrap();
                        tm.insert::<EntityHolder<S>>(ed);
                    },
                    None => {
                        let component = EntityData { data: S::changed(None, &with).unwrap(), messages: vec![] };
                        tm.insert::<EntityHolder<S>>(component);
                    }
                }
            }
        };
        // let component = EntityData { data: S::changed(None, &with).unwrap(), messages: vec![] };
        // self.data.get_mut(&who).map(|(map, _)| map.insert::<EntityHolder<S>>(component));
    }

    fn unsubscribe(&mut self, who: Self::Index) {
        use std::collections::btree_map::Entry::*;
        match self.data.entry(who) {
            Occupied(c) => {
                c.remove();
            }
            _ => {}
        };
    }
}

pub struct HostCtx<'h> {
    host: &'h mut Host,
    cur_index: usize,
    cur_type_id: TypeId,
    msgs: Box<dyn Any>,
}

impl<'h> types::traits::Context<'h, Host> for HostCtx<'h> {
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
            self.host.with_entity_data::<S, (), _>(whom, |x| { x.messages.push(msg); });
        }
    }

    fn subscribe<S: System<Host>>(&mut self, filter: fn(&<Host as types::traits::Host>::Event) -> Option<<S as System<Host>>::Message>) where Host: Hosts<S> {
        let index = self.cur_index;
        let reducer = move |ev: &winit::event::WindowEvent<'static>, e_data: &mut typemap::TypeMap| -> () {
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

    fn spawn<T: 'static + Send, F, Fut, S: System<Host>>(&mut self, fut: Fut, f: F, whom: usize) -> bool
        where Fut: Future<Output=T> + Send + 'static, F: FnOnce(T) -> S::Message + 'static, Host: Hosts<S>
    {
        self.host.spawn_fut(fut, f, whom)
    }

    fn with_state<S: System<Host>, T, F: FnOnce(&mut S::State) -> T>(&mut self, f: F) -> Option<T> where Host: Hosts<S> {
        self.host.with_system_data::<S, T, _>(|s| {
            f(&mut s.state)
        })
    }
}

