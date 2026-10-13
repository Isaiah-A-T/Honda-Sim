//! Standalone Honda Sim engine viewer: follows the sim over ws://127.0.0.1:8765 and runs a demo until it connects.
//! `--mode airflow` (or internals, block, stress, materials) opens on that view.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use honda_sim_viewer::{run, Link, Mode, Options};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.windows(2).find(|w| w[0] == "--mode")
        .and_then(|w| Mode::ALL.into_iter().find(|m| m.label().to_lowercase().starts_with(&w[1].to_lowercase())))
        .unwrap_or(Mode::Full);
    run(Options { link: Link::listen(), mode, ..Default::default() });
}
