use std::future::Future;

pub mod render;
pub mod event;

pub struct InternalError;

pub trait Host {
    /// A type used to identify entities
    type Index;
    /// A type of event received
    type Event: event::Event;
    // /// A type for DOM model node, for use in renderer,
    // type DOM;

    /// Allocate a unique, un occupied index
    fn allocate_entity(&mut self) -> Result<Self::Index,crate::errors::traits::AllocError>;
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
    fn register(&mut self, _: &mut H) {}
    /// reduce the global state of the system.
    fn update(&mut self, f: impl FnOnce(Self)->Self);
}

pub trait System<H: Host + Hosts<Self> + ?Sized>: 'static {
    /// inner message
    type Message: 'static + Send;

    /// Some global state of a system
    type State: GlobalState<H> + 'static;

    /// Properties for component initialization
    type Props;

    fn changed(this: Option<&mut Self>,props: &Self::Props)->Self;
    fn update<'s,'h: 's>(&'s mut self,state: &mut Self::State,msg: Self::Message, ctx: &mut impl Context<'h,H>) where 'h: 's;
    fn view<'v>(&'v self,renderer: &'v mut dyn render::Renderer<H>,vp: render::Viewport);
}


pub trait Context<'s,H: Host + ?Sized> {
    fn get_host(&mut self) -> &mut H;
    fn send<S: System<H>>(&mut self,msg: S::Message,whom: H::Indice) where H: Hosts<S>;

    fn spawn<T: 'static + Send,F,Fut,S: System<H>>(&mut self,fut: Fut, f: F,whom: H::Indice) -> bool
        where Fut: Future<Output = T> + Send + 'static , F: Fn(T) -> S::Message + 'static, H: Hosts<S>;
    fn with_state<S: System<H>,T,F: FnOnce(&mut S::State) -> T>(&mut self,f: F) -> Option<T> where H: Hosts<S>;
}
