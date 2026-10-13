//! Honda Sim engine viewer as a library, so the sim can compile it into its own program:
//!
//! ```no_run
//! use honda_sim_viewer::{run, Link, Options, SimState};
//! let link = Link::in_process();
//! let feed = link.clone();
//! std::thread::spawn(move || loop {
//!     // step the sim, then:
//!     feed.send(SimState { rpm: 6500.0, vtec: true, ..Default::default() });
//!     std::thread::sleep(std::time::Duration::from_millis(16));
//! });
//! run(Options { link, ..Default::default() });   // blocks on the main thread until the window closes
//! ```
//!
//! A host can draw its own panels around the 3D view (it stays in the middle) with `Options::host_panels`:
//!
//! ```no_run
//! use honda_sim_viewer::{egui, run, Link, Options};
//! run(Options {
//!     link: Link::in_process(),
//!     host_panels: Some(Box::new(|ui, state| {
//!         egui::Panel::left("sim").default_size(260.0).show_inside(ui, |ui| {
//!             ui.heading("Tune");
//!             ui.label(format!("{:.0} rpm", state.rpm));
//!         });
//!     })),
//!     ..Default::default()
//! });
//! ```
//!
//! The standalone `honda-sim-viewer` program (src/main.rs) is the same window with `Link::listen()`.

#[cfg(not(target_os = "ios"))]
pub mod app;
pub mod engine;
pub mod fake_sim;
pub mod kinematics;
pub mod link;
pub mod materials;
pub mod meshes;
pub mod state;

#[cfg(not(target_os = "ios"))]
pub use app::{run, Options};
/// The egui version the viewer draws with, for hosts adding their own panels through `Options::host_panels`.
pub use three_d::egui;
pub use engine::Mode;
pub use link::{Link, PORT};
pub use state::{PartMaterial, SimState, StressValue};
