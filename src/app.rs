pub use crate::traits::render::Viewport;
use luminance_glutin::GlutinSurface;
use glutin::event_loop::{EventLoop, ControlFlow};
use glutin::event::{Event, WindowEvent, StartCause};
use std::collections::{BTreeMap, HashMap};
use std::ops::Add;

pub mod host_impls;
use app::host_impls::default::Host;

pub struct Application {
    vp: Viewport,
    host: host_impls::default::Host,
}

impl Application {
    pub fn new(vp: Viewport) -> Self {
        Self{ vp, host: Host::new() }
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

        mod ev {
            use glutin::event::Event::WindowEvent;
            use glutin::event;
            use glutin::event::Event;
            use std::hash::{Hash, Hasher};
            use std::ops::Add;
            use std::cmp::Ordering;

            #[derive(PartialEq,Debug)]
            pub enum Ev {
                ScaleFactorChanged{scale_factor: f64,physical_size: winit::dpi::PhysicalSize<u32>},
                Other(Event<'static,()>),
            }

            impl Eq for Ev {}

            impl<'a> From<&'a Event<'a,()>> for Ev {
                fn from(r: &'a Event<'a, ()>) -> Self {
                    match r {
                        x @ Event::WindowEvent{event: _ev @  event::WindowEvent::ScaleFactorChanged{scale_factor, new_inner_size} ,window_id: _} => {
                            Self::ScaleFactorChanged {scale_factor: *scale_factor,physical_size: **new_inner_size}
                        },
                        r => unsafe {
                            Self::Other(std::mem::transmute_copy( r ))
                        },
                    }
                }
            }

            impl Hash for Ev {
                fn hash<H: Hasher>(&self, state: &mut H) {
                    match self {
                        Self::ScaleFactorChanged {scale_factor,physical_size} => {
                            state.write_i64(*scale_factor as i64);
                            state.write_u32(physical_size.width);
                            state.write_u32(physical_size.height);
                        },
                        Self::Other(ev) => {
                            const SZ: usize = {
                                std::mem::size_of::<Event<()>>()
                            };
                            let ev_buf : [u8; SZ] = unsafe {std::mem::transmute_copy(ev)};
                            state.write(&ev_buf[..]);
                        }
                    }

                }
            }

        }


        let mut timer = std::time::Instant::now();
        let mut ev_stats: HashMap<ev::Ev, usize> = HashMap::new();


        let mut is_focused = true;
        ctx.run(move |ev,ev2,flag| {
            {
                let now = std::time::Instant::now();
                if now.duration_since(timer) > std::time::Duration::from_secs(3) {
                    timer = now;
                    print!("\t--- After 3 secs ---\n\tevents of interest:\n");
                    for (k, v) in ev_stats.iter() {
                        use ev::Ev;
                        match k {
                            Ev::Other(x) if matches!(x,Event::WindowEvent{window_id,event: _e @ glutin::event::WindowEvent::CloseRequested}) => {
                                println!("{:?} occured {} number of times", k, v);
                            }
                            _ => {}
                        }
                        println!("{:?} occured {} number of times",k,v);
                    };
                    ev_stats.clear();
                };
                {
                    let ref ev = ev;
                    let key = ev::Ev::from(ev);
                    let num = ev_stats.remove(&key).unwrap_or(0usize) + 1;
                    ev_stats.insert(key, num);
                };
            }

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
