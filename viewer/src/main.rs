//! Standalone Honda Sim engine viewer: follows the sim over ws://127.0.0.1:8765 and runs a demo until it connects.
//! `--mode airflow` (or internals, block, stress, materials) opens on that view.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use honda_sim_viewer::{run, Link, Mode, Options};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.windows(2).find(|w| w[0] == "--mode")
        .and_then(|w| Mode::ALL.into_iter().find(|m| m.label().to_lowercase().starts_with(&w[1].to_lowercase())))
        .unwrap_or(Mode::Full);
    // `--build na`, `--build turbo` (no intercooler) or `--build ic` sets the demo's build.
    let demo_build = match args.windows(2).find(|w| w[0] == "--build").map(|w| w[1].as_str()) {
        Some("na") => (false, true),
        Some("turbo") => (true, false),
        _ => (true, true),
    };
    // `--block k24` starts the demo on a K24A block.
    // `--view intake` (or exhaust, front, side, top, valvetrain) sets the starting camera.
    let view = args.windows(2).find(|w| w[0] == "--view")
        .and_then(|w| ["exhaust", "intake", "front", "side", "top", "valvetrain"].iter().position(|v| *v == w[1]))
        .unwrap_or(0);
    let demo_k24 = args.windows(2).any(|w| w[0] == "--block" && w[1].to_lowercase().starts_with("k24"));
    let check_clearance = args.iter().any(|a| a == "--check-clearance");
    run(Options { link: Link::listen(), mode, check_clearance, demo_build, demo_k24, view, ..Default::default() });
}
