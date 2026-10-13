// How many seconds of sim run per second of wall time (needs to stay well above 1 for real time).
use honda_sim_core::{k20z3::placeholder_tune, Input, Sim};
fn main() {
    let mut sim = Sim::new(placeholder_tune());
    sim.shift(3);
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        sim.step(Input { throttle: 1.0, ..Default::default() }, 0.01);
    }
    let wall = start.elapsed().as_secs_f64();
    println!("10 s of sim in {:.2} s wall ({:.0}x real time), rpm {:.0}", wall, 10.0 / wall, sim.eng.rpm);
}
