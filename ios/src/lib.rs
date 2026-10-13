//! Honda Sim for iOS: the phone app (shared/phone) in a UIKit window, drawing with OpenGL ES 3 through an
//! EAGL context and a CAEAGLLayer. (three-d's own window code covers Android and desktops, not iOS.) Apple has
//! deprecated OpenGL ES but it still ships on every iPhone and iPad, and it's what the 3D engine view uses.
//!
//! Built as a static library; the Xcode project in ../xcode links it and its main() calls `honda_sim_main`.
#![cfg(target_os = "ios")]

use honda_sim_mobile::three_d::{self, context::HasContext};
use honda_sim_mobile::winit::event_loop::EventLoop;
use honda_sim_mobile::winit::platform::ios::WindowExtIOS;
use honda_sim_mobile::winit::window::{Window, WindowBuilder};
use honda_sim_mobile::{GlSurface, Platform};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send, msg_send_id};
use objc2_foundation::{CGRect, NSString};
use std::cell::Cell;
use std::sync::Arc;

const RENDERBUFFER: u32 = 0x8D41;
const EAGL_GLES3: usize = 3;

#[no_mangle]
pub extern "C" fn honda_sim_main() {
    // Tunes, logs and controls.json go in the app's Documents folder, which the Files app shows under
    // "On My iPhone › Honda Sim" (UIFileSharingEnabled in Info.plist).
    let docs = std::env::var("HOME").map(|h| format!("{h}/Documents")).unwrap_or_default();
    std::env::set_var("APPDATA", &docs);
    honda_sim_mobile::set_platform(Platform {
        clipboard: Some(Box::new(clipboard)),
        files_hint: "Files are in the Files app under On My iPhone › Honda Sim › HondaSim.".into(),
        poll_gamepads: Some(Box::new(poll_gamepads)),
    });
    honda_sim_mobile::run(EventLoop::new(), WindowBuilder::new().with_title("Honda Sim"), |w| Box::new(EaglSurface::new(w)));
}

/// The general pasteboard's text (for pasting tables copied from FlashPro or a spreadsheet).
fn clipboard() -> Option<String> {
    unsafe {
        let pb: *mut AnyObject = msg_send![class!(UIPasteboard), generalPasteboard];
        if pb.is_null() { return None; }
        let s: Option<Retained<NSString>> = msg_send_id![pb, string];
        s.map(|s| s.to_string())
    }
}

/// Bluetooth gamepads through Apple's GameController framework (iOS doesn't send their buttons as key
/// presses the way Android does). Reported with the same button codes as on Android, so a saved mapping
/// works on both.
fn poll_gamepads() {
    use objc2::runtime::AnyClass;
    let Some(cls) = AnyClass::get("GCController") else { return };
    unsafe {
        let all: *mut AnyObject = msg_send![cls, controllers];
        let n: usize = if all.is_null() { 0 } else { msg_send![all, count] };
        let mut down = [false; 9];
        for i in 0..n {
            let c: *mut AnyObject = msg_send![all, objectAtIndex: i];
            let pad: *mut AnyObject = msg_send![c, extendedGamepad];
            if pad.is_null() { continue; }
            let els: [*mut AnyObject; 9] = [
                msg_send![pad, buttonA], msg_send![pad, buttonB], msg_send![pad, buttonX], msg_send![pad, buttonY],
                msg_send![pad, leftShoulder], msg_send![pad, rightShoulder], msg_send![pad, leftTrigger], msg_send![pad, rightTrigger],
                msg_send![pad, buttonMenu],
            ];
            for (k, el) in els.iter().enumerate() {
                if el.is_null() { continue; }
                let value: f32 = msg_send![*el, value];
                down[k] |= value > 0.3;
            }
        }
        if n > 0 {
            for (k, code) in [304, 305, 307, 308, 310, 311, 312, 313, 315].into_iter().enumerate() {
                honda_sim_mobile::gamepad_button(code, down[k]);
            }
        }
    }
}

/// An OpenGL ES 3 context drawing into a CAEAGLLayer laid over winit's view.
struct EaglSurface {
    ctx: three_d::Context,
    eagl: Retained<AnyObject>,
    layer: Retained<AnyObject>,
    view: *mut AnyObject,
    fbo: three_d::context::Framebuffer,
    color: three_d::context::Renderbuffer,
    depth: three_d::context::Renderbuffer,
    size: Cell<(u32, u32)>,
}

impl EaglSurface {
    fn new(window: &Window) -> EaglSurface {
        unsafe {
            let eagl: Retained<AnyObject> = msg_send_id![msg_send_id![class!(EAGLContext), alloc], initWithAPI: EAGL_GLES3];
            let _: Bool = msg_send![class!(EAGLContext), setCurrentContext: &*eagl];
            let view = window.ui_view() as *mut AnyObject;
            let layer: Retained<AnyObject> = msg_send_id![class!(CAEAGLLayer), new];
            let _: () = msg_send![&*layer, setOpaque: Bool::YES];
            let _: () = msg_send![&*layer, setContentsScale: window.scale_factor()];
            let bounds: CGRect = msg_send![view, bounds];
            let _: () = msg_send![&*layer, setFrame: bounds];
            let view_layer: *mut AnyObject = msg_send![view, layer];
            let _: () = msg_send![view_layer, addSublayer: &*layer];

            let gl = three_d::context::Context::from_loader_function(|name| {
                let name = std::ffi::CString::new(name).unwrap();
                libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) as *const _
            });
            let fbo = gl.create_framebuffer().unwrap();
            let color = gl.create_renderbuffer().unwrap();
            let depth = gl.create_renderbuffer().unwrap();
            let ctx = three_d::Context::from_gl_context(Arc::new(gl)).expect("OpenGL ES 3");
            let s = EaglSurface { ctx, eagl, layer, view, fbo, color, depth, size: Cell::new((0, 0)) };
            let size = window.inner_size();
            s.resize(size.width, size.height);
            s
        }
    }
}

impl GlSurface for EaglSurface {
    fn context(&self) -> three_d::Context {
        self.ctx.clone()
    }

    fn resize(&self, width: u32, height: u32) {
        if self.size.get() == (width, height) {
            return;
        }
        self.size.set((width, height));
        unsafe {
            let bounds: CGRect = msg_send![self.view, bounds];
            let _: () = msg_send![&*self.layer, setFrame: bounds];
            let gl = &self.ctx;
            gl.bind_framebuffer(three_d::context::FRAMEBUFFER, Some(self.fbo));
            gl.bind_renderbuffer(RENDERBUFFER, Some(self.color));
            // The color buffer's storage is the layer itself, sized to it.
            let _: Bool = msg_send![&*self.eagl, renderbufferStorage: RENDERBUFFER as usize, fromDrawable: &*self.layer];
            gl.framebuffer_renderbuffer(three_d::context::FRAMEBUFFER, three_d::context::COLOR_ATTACHMENT0, RENDERBUFFER, Some(self.color));
            gl.bind_renderbuffer(RENDERBUFFER, Some(self.depth));
            gl.renderbuffer_storage(RENDERBUFFER, three_d::context::DEPTH_COMPONENT24, width as i32, height as i32);
            gl.framebuffer_renderbuffer(three_d::context::FRAMEBUFFER, three_d::context::DEPTH_ATTACHMENT, RENDERBUFFER, Some(self.depth));
        }
    }

    fn present(&self) {
        unsafe {
            self.ctx.bind_renderbuffer(RENDERBUFFER, Some(self.color));
            let _: Bool = msg_send![&*self.eagl, presentRenderbuffer: RENDERBUFFER as usize];
        }
    }

    fn framebuffer(&self) -> Option<three_d::context::Framebuffer> {
        Some(self.fbo)
    }
}
