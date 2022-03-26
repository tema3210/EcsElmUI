pub use crate::traits::render::Viewport;
use std::collections::{BTreeMap, HashMap};
use std::ops::Add;

use winit::{
    event::{Event,WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

pub mod host_impls;

//todo: make this generic over the host + all consequences
pub struct Application {
    vp: Viewport,
    host: host_impls::default::Host,
}

//TODO: finish this: make wgpu and host to work
impl Application {
    pub fn new(vp: Viewport) -> Self {
        Self{ vp, host: host_impls::default::Host::new() }
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
