use std::future::Future;

pub mod render;

//TODO: add a way to draw an app.
//todo: support styling?
pub trait Host {
    /// A type used to identify entities
    type Index;
    /// A type of events used by this host
    type Event;

    /// Allocate a unique, un occupied index
    fn allocate_entity(&mut self) -> Result<Self::Index,crate::errors::traits::AllocError>;
    /// Setups required data for entity
    fn set_entity_data(&mut self,which: Self::Index, anchors: Vec<render::Anchor>, vp: render::Viewport);
    /// Deallocate given index
    fn drop_entity(&mut self,which: Self::Index);

    /// Dispatch a batch of events
    fn receive_events(&mut self,events: &[Self::Event]);
    /// Run one update round
    fn update_round(&mut self);
}

pub trait Hosts<S: System<Self> + ?Sized + 'static>: Host {

    fn get_state(&mut self, which: Self::Index) -> Option<&mut S>;

    fn subscribe(&mut self, who: Self::Index, with: S::Props);

    fn unsubscribe(&mut self, who: Self::Index);
}

pub trait GlobalState<H: Host + ?Sized>: Sized + 'static {
    /// ctor
    fn init()-> Self;
    /// registration routine
    fn register(&mut self, place: &mut H);
    /// reduce the global state of the system.
    fn update(&mut self, f: impl FnOnce(Self)->Self);
}

pub trait System<H: Host + Hosts<Self> + ?Sized>: 'static + Unpin + Sized {
    /// inner message
    type Message: 'static + Send + Unpin;

    /// Some global state of a system
    type State: GlobalState<H> + 'static;

    /// Properties for component initialization
    type Props;
    /// if props has changed...
    fn changed(this: Option<&mut Self>,props: &Self::Props) -> Option<Self>;
    /// Note: Global state of the system can be accessed via a ctx
    fn update<'s,'h: 's>(&'s mut self,msg: Self::Message, ctx: &mut impl Context<'h,H>) where 'h: 's;
    /// This should return the number of different views of the component.
    fn count_views(&self) -> usize;
    /// Draw a component; viewport describes boundaries of a component, view_index is the number of view we are going to draw
    fn view<'v>(&'v self,renderer: &'v mut dyn render::Renderer<H>,viewport: render::Viewport,view_index: usize);
}


pub trait Context<'s,H: Host + ?Sized> {
    /// Get reference to a host
    fn get_host(&mut self) -> &mut dyn Host<Event = H::Event,Index = H::Index>;
    /// Get an index of current entity
    fn get_current_index(&mut self) -> H::Index;

    /// Send a strongly typed message to a component, if the component isn't registered for index, nothing will happen
    fn send<S: System<H>>(&mut self,msg: S::Message,whom: H::Index) where H: Hosts<S>;
    /// Set current event -> message transform for current (entity, system) pair
    fn subscribe<S: System<H>>(&mut self,filter: fn(&H::Event) -> Option<S::Message>) where H: Hosts<S>;

    /// spawn a future with a result -> message transform.
    fn spawn<T: 'static + Send,F,Fut,S: System<H>>(&mut self,fut: Fut, f: F,whom: H::Index) -> bool
        where Fut: Future<Output = T> + Send + 'static , F: Fn(T) -> S::Message + 'static, H: Hosts<S>;
    fn with_state<S: System<H>,T,F: FnOnce(&mut S::State) -> T>(&mut self,f: F) -> Option<T> where H: Hosts<S>;
}
