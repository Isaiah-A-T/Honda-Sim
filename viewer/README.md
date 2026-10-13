# Honda Sim 3D viewer

A native desktop app (Windows `.exe`, also builds on Linux and macOS) that draws the K20Z3 in 3D and follows the sim
live. It is separate from the sim: the sim sends its state over a local WebSocket, and the viewer runs on its own
demo data whenever nothing is connected.

Views, picked from the tabs along the top:

- **Full engine**: everything, with the block and head see-through.
- **Internals**: pistons, rods, crank and the valvetrain with cam lobes, VTEC switching to the high lobe and VTC phasing.
- **Block**: the castings on their own with the main dimensions.
- **Airflow**: intake and exhaust flow, through the turbo on a turbo build, gated by valve lift.
- **Stress**: each part tinted green, yellow, orange, red as it nears or passes its limit.
- **Materials**: each part colored by alloy family; hover for the exact material.

Drag to orbit, scroll to zoom. The camera buttons jump to set views.

## Running it

1. Start `honda-sim-viewer.exe`. Windows may ask about an unsigned app (More info, Run anyway).
2. Start the sim (`npm start`, open http://localhost:8080). The pill in the sim's 3D engine panel turns to
   "Viewer app connected", and the viewer's Data section says "Following the sim."

Either can start first; the sim retries every 2 seconds. The viewer only listens on 127.0.0.1, so it is not
reachable from other machines. `--mode stress` (or internals, block, airflow, materials) opens on that view.

## Building

Needs Rust (stable).

```sh
cd viewer
cargo run --release                                    # this machine
cargo build --release --target x86_64-pc-windows-gnu   # Windows .exe from Linux (needs mingw-w64)
cargo test
```

The Windows build lands in `target/x86_64-pc-windows-gnu/release/honda-sim-viewer.exe` and needs no DLLs beyond
Windows' own (OpenGL 3.3).

## Building it into the sim

The viewer is also a library crate (`honda_sim_viewer`), so a native sim can compile it into its own `.exe`
and ship one program. The sim hands states over directly, with no socket involved:

```rust
use honda_sim_viewer::{run, Link, Options, SimState};

let link = Link::in_process();
let feed = link.clone();
std::thread::spawn(move || loop {
    // step the sim, then:
    feed.send(SimState { rpm: sim.eng.rpm, vtec: sim.eng.high_cam, ..Default::default() });
});
run(Options { link, ..Default::default() });   // opens the window; blocks on the main thread until it closes
```

Add it with `honda-sim-viewer = { path = "viewer" }`. `run` must own the main thread (a windowing rule on
Windows and macOS), so the sim's loop goes on a worker thread. `cargo run --example embedded` shows this
working with the demo sim standing in. `SimState` has the same fields as the JSON below, in snake_case.

## Protocol

The viewer listens on `ws://127.0.0.1:8765`. Each text message is one JSON object with the latest state; send
it every frame or at a fixed rate (the sim sends 30 per second). Every field is optional. If no message arrives
for 1.5 seconds the viewer falls back to its demo.

| Field | Type | Meaning |
|---|---|---|
| `rpm` | number | Crank speed |
| `running` | bool | Engine running |
| `throttle` | 0–1 | Throttle opening |
| `mapKpa` | number | Manifold pressure, kPa absolute |
| `boostKpa` | number | Boost above atmosphere, kPa |
| `airGs` | number | Airflow, g/s (sets particle density in Airflow) |
| `vtec` | bool | High cam engaged |
| `vtcDeg` | number | Intake cam advance, 0–50° |
| `ignDeg` | number | Spark advance, ° BTDC |
| `knock` | bool | Knocking now |
| `fuelCut` | bool | Rev, decel or boost cut |
| `lambda` | number | Air-fuel ratio as lambda |
| `turbo` | bool | Turbo build (shows the turbo and its plumbing) |
| `stress` | object | Part key to load (1 = at the limit), or an array of 4 for per-cylinder parts |
| `partMaterials` | object | Part key to `{ part, label, family, detail }`. When present the Materials view is read-only |
| `readouts` | array | Extra `[label, value]` rows for the side panel |

Part keys: `pistons`, `rods`, `crank`, `valves`, `cams`, `block`, `head`, `exhaust`, `intake`, `turbo`,
`clutch`, `flywheel`. Material families: `aluminum`, `steel`, `chromoly`, `titanium`, `nickel`, `iron`,
`stainless`, `friction`.

The sim side is `src/viewer-link.js`. Anything else can drive the viewer the same way, for example from Python:

```python
import json, websocket
ws = websocket.create_connection("ws://127.0.0.1:8765")
ws.send(json.dumps({"rpm": 6500, "vtec": True, "stress": {"rods": 0.8}}))
```
