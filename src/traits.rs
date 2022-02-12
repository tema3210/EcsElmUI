use std::future::Future;

pub mod render;

// pub struct NoSuchIndice;
pub struct InternalError;

pub trait Host {
    type Indice;

    fn allocate_entity(&mut self) -> Result<Self::Indice,crate::errors::traits::AllocError>;
    fn drop_entity(&mut self,which: Self::Indice);

    // fn subscribers<'h,S>(&'h self) -> Box<dyn Iterator<Item=Self::Indice>> where Self: Hosts<'h,S>;
}

pub trait Hosts<'h,S: System<'h,Self> + ?Sized + 'static>: Host + 'h {
    fn reduce<'s,'d>(&'h mut self, which: Self::Indice,with: &'d mut impl Iterator<Item=S::Message>,ctx: &'s mut impl Context<'h,Self>) -> Result<(),NoSuchIndice> where 's: 'd,'h: 's;

    fn get_state(&mut self,which: Self::Indice) -> Option<&mut S>;

    fn subscribe(&mut self, who: Self::Indice,with: S::Props);
    fn unsubscribe(&mut self, who: Self::Indice);
}

pub trait GlobalState: Sized {
    fn init() -> Self;
    fn update(&mut self, f: impl FnOnce(Self)->Self);
}

pub trait System<'h,H: Host + Hosts<'h,Self> + ?Sized + 'h>: 'static {
    /// inner message
    type Message: 'static;

    /// Some global state of a system
    type State: GlobalState + 'static;

    /// Properties for component initialization
    type Props;

    fn changed<'s>(this: Option<&'s mut Self>,props: &Self::Props)->Self;
    fn update<'s>(&'s mut self,state: &mut Self::State,msg: Self::Message, ctx: &mut impl Context<'h,H>) where 'h: 's;
    fn view<'v>(&'v self,renderer: &'v mut dyn render::Renderer<H>,vp: render::Viewport) -> fn();
}


pub trait Context<'s,H: Host + ?Sized + 's> {
    fn get_host(&mut self) -> &mut H;
    fn send<S: System<'s,H>>(&mut self,msg: S::Message,whom: H::Indice) where H: Hosts<'s,S>;

    fn spawn<T: 'static,F,Fut,S: System<'s,H>>(&mut self,fut: Fut, f: F)
        where Fut: Future<Output = T> + 'static, F: Fn(T) -> S::Message + 'static, H: Hosts<'s,S>;
    fn state<S: System<'s,H>>(&'s mut self) -> &'s mut S::State where H: Hosts<'s,S>;
}
