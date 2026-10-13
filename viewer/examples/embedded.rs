//! The viewer built into another program: a stand-in sim runs on its own thread and hands each state to the
//! viewer in-process, the way the native sim will. `cargo run --example embedded`

use honda_sim_viewer::{egui, fake_sim::FakeSim, run, Link, Options};
use std::time::Duration;

fn main() {
    let link = Link::in_process();
    let feed = link.clone();
    std::thread::spawn(move || {
        let mut sim = FakeSim::new();
        loop {
            let mut state = sim.step(1.0 / 60.0, None, &|_| 1.0);
            state.readouts.push(("Source".into(), "in-process example".into()));
            feed.send(state);
            std::thread::sleep(Duration::from_millis(16));
        }
    });
    run(Options {
        link,
        title: "Viewer embedded in a sim".into(),
        // The host's own panels; the 3D view stays in the middle.
        host_panels: Some(Box::new(|ui, state| {
            egui::Panel::left("host").default_size(220.0).show_inside(ui, |ui| {
                ui.heading("Host panel");
                ui.label(format!("{:.0} rpm", state.rpm));
            });
        })),
        ..Default::default()
    });
}
