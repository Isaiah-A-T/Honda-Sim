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
//! The standalone `honda-sim-viewer` program (src/main.rs) is the same window with `Link::listen()`.

pub mod app;
pub mod engine;
pub mod fake_sim;
pub mod kinematics;
pub mod link;
pub mod materials;
pub mod meshes;
pub mod state;

pub use app::{run, Options};
pub use engine::Mode;
pub use link::{Link, PORT};
pub use state::{PartMaterial, SimState, StressValue};
