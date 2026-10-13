# Native Honda Sim

The sim as a native Rust program, so it and the 3D engine viewer build into one plain Windows .exe.

| Crate | What it is |
|---|---|
| `sim/` (`honda-sim-core`) | The engine, gas exchange, VTEC, turbo, drivetrain, internals, materials and parts model, ported line for line from `src/*.js` |

`sim/tests/parity.rs` runs fixed scenarios (single gas exchange cycles, full-throttle engine runs on stock, turbo and K24 builds, the bottom-end
calculator, and a launch through first and second gear) and checks the native results match the JavaScript model to within 1e-6.
When the JavaScript model changes, run `node tools/export-native-data.mjs` to refresh the data and fixtures, port the change, and
run the tests again.

```
cd native
cargo test --release                                  # parity tests
cargo run --release --example speed -p honda-sim-core # about 47× real time on one core
cargo build --release --target x86_64-pc-windows-gnu  # Windows build (needs mingw-w64)
```

The state other modules read is `simstate::render_state(&sim)`, the same JSON as the web app's `renderState` (docs/MODULES.md).
