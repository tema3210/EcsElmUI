use std::future::Future;

pub mod render;

pub struct InternalError;

pub trait Host {
    type Indice;

    fn allocate_entity(&mut self) -> Result<Self::Indice,crate::errors::traits::AllocError>;
    fn drop_entity(&mut self,which: Self::Indice);
}

pub trait Hosts<S: System<Self> + ?Sized + 'static>: Host {

    // fn reduce(&mut self, which: Self::Indice) -> Result<(),crate::errors::traits::ReduceError>;

    fn get_state(&mut self,which: Self::Indice) -> Option<&mut S>;

    fn subscribe(&mut self, who: Self::Indice,with: S::Props);
    fn unsubscribe(&mut self, who: Self::Indice);
}

pub trait GlobalState<H: Host + ?Sized>: Sized + 'static {
    /// ctor
    fn init()-> Self;
    /// registration routine
    fn register(&mut self, host: &mut H);
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
    fn view<'v>(&'v self,renderer: &'v mut dyn render::Renderer<H>,vp: render::Viewport);// -> fn();
}


pub trait Context<'s,H: Host + ?Sized> {
    fn get_host(&mut self) -> &mut H;
    fn send<S: System<H>>(&mut self,msg: S::Message,whom: H::Indice) where H: Hosts<S>;

    fn spawn<T: 'static + Send,F,Fut,S: System<H>>(&mut self,fut: Fut, f: F,whom: H::Indice) -> bool
        where Fut: Future<Output = T> + Send + 'static , F: Fn(T) -> S::Message + 'static, H: Hosts<S>;
    fn with_state<S: System<H>,T,F: FnOnce(&mut S::State) -> T>(&mut self,f: F) -> Option<T> where H: Hosts<S>;
}
