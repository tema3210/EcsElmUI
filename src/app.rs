use std::process::exit;

pub use crate::traits::render::Viewport;
use luminance_glutin::GlutinSurface;
use glutin::event_loop::{EventLoop, ControlFlow};
use glutin::event::Event;

pub mod host_impls;

pub struct Application {
    vp: Viewport,
}

impl Application {
    pub fn new(vp: Viewport) -> Self {
        Self{ vp }
    }
    pub fn run(&mut self) {
        let (surface,ev_loop) = GlutinSurface::new_gl33(
            glutin::window::WindowBuilder::new()
                .with_inner_size(glutin::dpi::LogicalSize::new(self.vp.width,self.vp.height))
                .with_title("Placeholder")
                .with_resizable(false),
            4
        ).expect("Glutin creation");

        self.main_loop(surface,ev_loop)
    }
    fn main_loop(&mut self,mut surface: GlutinSurface,ctx: EventLoop<()> ) -> ! {
        ctx.run(move |ev,ev2,flag|{
            match ev {
                Event::NewEvents(resume_reason) => {

                }
                Event::WindowEvent { window_id: _,event } => {

                }
                Event::DeviceEvent { device_id,event } => {

                }

                Event::Suspended => {}
                Event::Resumed => {}

                Event::MainEventsCleared => {}
                Event::RedrawRequested(_) => {

                }
                Event::RedrawEventsCleared => {}

                Event::LoopDestroyed => { *flag = ControlFlow::Exit; }
                Event::UserEvent(()) => {}
            }
        });
    }
}
