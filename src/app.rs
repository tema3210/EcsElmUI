pub use crate::traits::render::Viewport;
use luminance_glutin::GlutinSurface;
use glutin::event_loop::{EventLoop, ControlFlow};
use glutin::event::{Event, WindowEvent, StartCause};
use std::collections::{BTreeMap, HashMap};
use std::ops::Add;

pub mod host_impls;

pub struct Application {
    vp: Viewport,
    host: host_impls::default::Host,
}

impl Application {
    pub fn new(vp: Viewport) -> Self {
        Self{ vp, host: host_impls::default::Host::new() }
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
        let mut is_focused = true;
        ctx.run(move |ev,ev2,flag| {
            match ev {
                Event::NewEvents(resume_reason) => {
                    match resume_reason {
                        StartCause::ResumeTimeReached { .. } => {}
                        StartCause::WaitCancelled { .. } => {}
                        StartCause::Poll => {}
                        StartCause::Init => {}
                    }
                },
                Event::WindowEvent { window_id,event } => {
                    match event {
                        WindowEvent::Resized(_) => {}
                        WindowEvent::Moved(_) => {}
                        WindowEvent::CloseRequested => {
                            *flag = ControlFlow::Exit;
                        }
                        WindowEvent::Destroyed => {}
                        WindowEvent::DroppedFile(_) => {}
                        WindowEvent::HoveredFile(_) => {}
                        WindowEvent::HoveredFileCancelled => {}
                        WindowEvent::ReceivedCharacter(_) => {}
                        WindowEvent::Focused(is) => {
                            is_focused = is;
                        }
                        WindowEvent::KeyboardInput { .. } => {}
                        WindowEvent::ModifiersChanged(_) => {}
                        WindowEvent::CursorMoved { .. } => {}
                        WindowEvent::CursorEntered { .. } => {}
                        WindowEvent::CursorLeft { .. } => {}
                        WindowEvent::MouseWheel { .. } => {}
                        WindowEvent::MouseInput { .. } => {}
                        WindowEvent::TouchpadPressure { .. } => {}
                        WindowEvent::AxisMotion { .. } => {}
                        WindowEvent::Touch(_) => {}
                        WindowEvent::ScaleFactorChanged { .. } => {}
                        WindowEvent::ThemeChanged(_) => {}
                    }
                }
                Event::DeviceEvent { device_id,event } => {

                }

                Event::Suspended => {
                    *flag = ControlFlow::Wait
                }
                Event::Resumed => {
                    *flag = ControlFlow::WaitUntil(std::time::Instant::now().add(std::time::Duration::from_millis(15)))
                }

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
