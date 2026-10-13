//! The window's event loop, shared by Android, iOS and the desktop preview. Phones hand the app its screen
//! only while it's in front (winit's Resumed/Suspended), so the graphics surface is made on Resumed and dropped
//! on Suspended; the sim keeps its state the whole time.

use crate::app::App;
use three_d::{Context, RenderTarget};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

/// A window's OpenGL ES surface. Android and the desktop preview use three-d's (`WindowedSurface`); iOS brings
/// its own (mobile/ios).
pub trait GlSurface {
    fn context(&self) -> Context;
    fn resize(&self, width: u32, height: u32);
    /// Shows the frame just drawn.
    fn present(&self);
    /// The framebuffer to draw into, when it isn't the default one (iOS draws into its own).
    fn framebuffer(&self) -> Option<three_d::context::Framebuffer> {
        None
    }
}

#[cfg(not(target_os = "ios"))]
pub struct WindowedSurface(pub three_d::WindowedContext);

#[cfg(not(target_os = "ios"))]
impl WindowedSurface {
    pub fn new(window: &Window) -> WindowedSurface {
        let settings = three_d::SurfaceSettings { vsync: true, depth_buffer: 24, multisamples: 4, ..Default::default() };
        // Some phones don't offer multisampling; fall back to none.
        let ctx = three_d::WindowedContext::from_winit_window(window, settings)
            .or_else(|_| three_d::WindowedContext::from_winit_window(window, three_d::SurfaceSettings { multisamples: 0, ..settings }))
            .expect("OpenGL ES 3 surface");
        WindowedSurface(ctx)
    }
}

#[cfg(not(target_os = "ios"))]
impl GlSurface for WindowedSurface {
    fn context(&self) -> Context {
        (*self.0).clone()
    }
    fn resize(&self, width: u32, height: u32) {
        self.0.resize(winit::dpi::PhysicalSize::new(width, height));
    }
    fn present(&self) {
        let _ = self.0.swap_buffers();
    }
}

/// Runs the app until the window closes (never returns on phones). `window` builds the window and
/// `surface` makes its graphics surface each time the app comes to the front.
pub fn run(
    event_loop: EventLoop<()>,
    window: WindowBuilder,
    mut surface: impl FnMut(&Window) -> Box<dyn GlSurface> + 'static,
) -> ! {
    let mut app = App::new();
    let mut win: Option<Window> = None;
    let mut gl: Option<Box<dyn GlSurface>> = None;
    let mut cursor = (0.0f32, 0.0f32);
    let mut mouse_down = false;
    event_loop.run(move |event, target, flow| {
        *flow = ControlFlow::Poll;
        match event {
            Event::Resumed => {
                if win.is_none() {
                    win = Some(window.clone().build(target).expect("window"));
                }
                let w = win.as_ref().unwrap();
                let s = surface(w);
                app.resume(&s.context());
                gl = Some(s);
            }
            Event::Suspended => {
                app.suspend();
                gl = None;
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => {
                    if let Some(s) = &gl {
                        s.resize(size.width, size.height);
                    }
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    if let Some(s) = &gl {
                        s.resize(new_inner_size.width, new_inner_size.height);
                    }
                }
                ev => app.input.window_event(&ev, &mut cursor, &mut mouse_down),
            },
            Event::MainEventsCleared => {
                if let Some(w) = &win {
                    w.request_redraw();
                }
            }
            Event::RedrawRequested(_) => {
                if let (Some(w), Some(s)) = (&win, &gl) {
                    let size = w.inner_size();
                    let (width, height) = (size.width.max(1), size.height.max(1));
                    let ctx = s.context();
                    let target = match s.framebuffer() {
                        Some(fb) => RenderTarget::from_framebuffer(&ctx, width, height, fb),
                        None => RenderTarget::screen(&ctx, width, height),
                    };
                    app.frame(&target, width, height, w.scale_factor() as f32);
                    // A target made from someone else's framebuffer would delete it when dropped.
                    let _ = target.into_framebuffer();
                    s.present();
                }
            }
            _ => {}
        }
    })
}
