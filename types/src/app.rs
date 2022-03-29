pub use crate::traits::render::Viewport;
use std::collections::{BTreeMap, HashMap};
use std::ops::Add;

use crate::traits::Host;

use winit::{
    event::{Event,WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

pub mod host_impls;

//todo: fill this with wgpu states
pub struct Application<H: Host<Event = WindowEvent<'static>>>
{
    vp: Viewport,
    host: H,
}

//TODO: finish this: make wgpu and host to work
impl<H: Host<Event = WindowEvent<'static>>> Application<H> {

    pub fn new(vp: Viewport) -> Self {
        unimplemented!()
    }

    pub fn run(&mut self) {
        //ScaleFactorChanged event SHOULD NOT go further through event loop.
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_resizable(true)
            // .with_inner_size((self.vp.height,self.vp.width)) //todo: fix this
            .build(&event_loop).unwrap();

    }
}
