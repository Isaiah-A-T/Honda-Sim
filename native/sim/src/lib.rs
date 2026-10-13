//! Honda Sim core: the K20Z3 engine, drivetrain and internals model, ported from the web app's src/*.js.
//! Parity tests (tests/parity.rs) compare it with the JavaScript model's results.

pub mod engine;
pub mod engines;
pub mod gasflow;
pub mod internals;
pub mod k20z3;
pub mod materials;
pub mod parts;
pub mod sim;
pub mod simstate;
pub mod tables;
pub mod thermal;
pub mod tune;
pub mod vehicle;

pub use sim::{Input, Sim};
pub use tune::Tune;
