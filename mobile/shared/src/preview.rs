//! The phone layout in a phone-shaped window on a desktop, for trying it without a phone. The mouse is one
//! finger (drag to turn the engine, click pedals and buttons); the scroll wheel zooms.
//!
//!     cargo run --release -p honda-sim-mobile --bin honda-sim-phone-preview            # landscape
//!     cargo run --release -p honda-sim-mobile --bin honda-sim-phone-preview portrait

fn main() {
    use honda_sim_mobile::winit::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder};
    let portrait = std::env::args().any(|a| a == "portrait");
    let size = if portrait { LogicalSize::new(412.0, 915.0) } else { LogicalSize::new(915.0, 412.0) };
    let window = WindowBuilder::new().with_title("Honda Sim · phone preview").with_inner_size(size);
    #[cfg(not(target_os = "ios"))]
    honda_sim_mobile::run(EventLoop::new(), window, |w| Box::new(honda_sim_mobile::WindowedSurface::new(w)));
}
