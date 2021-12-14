use render::Renderer;
use std::future::Future;
use std::any::Any;

pub mod render;

pub struct AllocError;

pub trait Host {
    type Indice;

    fn allocate_entity(&mut self) -> Result<Self::Indice,AllocError>;
    fn drop_entity(&mut self,which: Self::Indice);
}

pub trait Hosts<'h,S: System<'h,Self> + ?Sized + 'static>: Host + 'h {
    fn get_state(&mut self,which: Self::Indice) -> Option<&mut S>;

    fn subscribe(&mut self, who: Self::Indice,with: S::Props);
    fn unsubscribe(&mut self, who: Self::Indice);
}

pub trait GlobalState: Sized {
    fn init() -> Self;
    fn update(&mut self, f: impl FnOnce(Self)->Self);
}

pub trait System<'h,H: Host + Hosts<'h,Self> + ?Sized + 'h>: 'static {
    type Message;
    type State: GlobalState;
    type Props;

    fn changed<'s>(this: Option<&'s mut Self>,props: &Self::Props)->Self;
    fn update<'s>(&'s mut self,gs: &mut Self::State,msg: Self::Message, ctx: impl Context<'s,H>) where 'h: 's;
    fn view<'v>(&'v self) -> Box<dyn render::Renderer<H> + 'v>;
}


pub trait Context<'s,H: Host + ?Sized + 's> {
    fn get_host(&mut self) -> &'s mut H;
    fn send<S: System<'s,H>>(&mut self,msg: S::Message,whom: H::Indice) where H: Hosts<'s,S>;

    fn spawn<T: 'static,F,Fut,S: System<'s,H>>(&mut self,fut: Fut, f: F)
        where Fut: Future<Output = T>, F: Fn(T) -> S::Message + 'static, H: Hosts<'s,S>;
    fn state<S: System<'s,H>>(&self) -> &'s mut S::State where H: Hosts<'s,S>;
}
