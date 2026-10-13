# Honda Sim engine view (3D)

A Three.js view of the K20Z3 for Honda Sim. It animates the crank, pistons, rods, both cams, all 16 valves, the
throttle plate and the turbo, fires the plugs in order (1-3-4-2) at the tune's spark advance, shows combustion in
each cylinder, lights the VTEC lobes when the high cam is on, and tints parts yellow, orange, then red as they near
or pass the limits the sim already uses. Hover any part to see its name, how close it is to its limit, and which
stroke its cylinder is on.

A GUI over the view switches between six modes, and each mode has its own panel:

| Mode | What you see |
|---|---|
| Full engine | Everything, with the block and head see-through, solid or hidden |
| Internals | Crank, rods, pistons, cams and valves on their own; VTEC lamp, VTC advance and per-cylinder valve lift |
| Block | Block, head, cam cover and oil pan as solid castings, with specs, block/head material and load |
| Airflow | Intake charge (blue) and exhaust (orange) particles through the intake, runners, ports, collector and turbo; each runner only flows while its valve is open |
| Stress | Parts tinted by load, with a bar per part |
| Materials | Parts coloured by material family; pick a material per part from the panel or by clicking the part |

The models are simple shapes built from real K20 dimensions (86 × 86 mm bore and stroke, 139 mm rods, 96 mm bore
spacing). Cam timing and lift are rough numbers picked to look right, not read from a cam card.

## Demo

With the sim running (`npm start`), open http://localhost:8080/renderer/demo.html for the view on its own with fake
data (a scripted pull, or sliders). Tests run with the sim's: `npm test`.

## Files

| File | What it does |
|---|---|
| `src/engine-renderer.js` | `createEngineRenderer(container, options)`: scene, camera, orbit controls, animation, view modes, airflow particles, tinting, hover labels, click to pick |
| `src/gui.js` | `createEngineGui(view, options)`: mode tabs and camera buttons over the view, and the panel for each mode |
| `src/materials.js` | Material choices per part, their colour family and a rough relative strength |
| `src/fake-sim.js` | `createFakeSim()`: stand-in for the sim's state (scripted pull or slider input) for building views before the real models exist |
| `src/parts.js` | The models: block, head, cam cover, crank with counterweights, pistons, rods, cams and lobes (low and VTEC), valves and springs, plugs and coils, timing chain, intake, exhaust, GT35-style turbo, flywheel and clutch |
| `src/kinematics.js` | K20 dimensions, piston and rod motion, firing order, valve lift, stress colours (no Three.js, unit tested) |
| `src/sim-adapter.js` | `stateFromSim(sim)`: turns Honda Sim's state into the renderer's state, including per-part stress |
| `demo.html`, `demo.js` | The view on fake data, for working on it without driving the sim |

Three.js r169 and the two add-ons it uses (OrbitControls, RoomEnvironment) are in the sim's `vendor/` folder and
mapped in the import maps in `index.html` and `demo.html`.

## How it plugs into the sim

`src/ui.js` loads three files from this folder on demand (so the sim still works without WebGL) and does three things:

```js
const view = createEngineRenderer(document.getElementById('r3d'), { slowMo });
createEngineGui(view, { partMaterials });        // the sim's internals materials, shown read-only
view.setState(stateFromSim(sim));                // every frame, while the view is on screen
```

The renderer never imports sim code. Everything it knows about the sim goes through `src/sim-adapter.js`, so a new
sim model only means a change there.

## API

```js
const view = createEngineRenderer(container, { slowMo: 0.02, xray: true, autoRotate: false, view: 'iso' });
view.setState({ rpm, throttle, mapKpa, boostKpa, vtec, ignDeg, knock, fuelCut, lambda, running, turbo, stress });
view.setOptions({ slowMo: 0.05, xray: 'solid' });   // xray: true (see-through), 'solid' or 'hidden'
view.setView('valvetrain');                         // iso (exhaust side), intake, front, side, top, valvetrain
view.setMode('airflow');                            // full, internals, block, airflow, stress, materials
view.select('rods');                                // highlight a part group (materials mode)
view.onFrame(v => {});                              // after each frame; returns an unsubscribe function

const gui = createEngineGui(view, { panel, materials, onMaterialsChange });
gui.materials;                                      // { pistons: 'stock-cast', rods: '4340-h', ... }
view.crankDeg;                                      // current crank angle, 0–720
view.dispose();
```

| State field | Meaning |
|---|---|
| `rpm` | Engine speed. The crank animates at `rpm × slowMo` (default 2 % of real speed: at real speed the parts strobe). |
| `crankDeg` | Optional. Set it to drive the crank angle yourself; leave it `null` to follow rpm. 0 = cylinder 1 TDC firing. |
| `throttle` | 0–1 throttle plate opening. |
| `mapKpa`, `boostKpa` | Manifold pressure (combustion brightness) and gauge boost (turbo speed). |
| `airGs` | Airflow in g/s: how fast the airflow particles move. The sim already computes `eng.airGs`. |
| `vtcDeg` | Intake cam advance from VTC, 0–50 crank degrees. The sim has no VTC model yet, so the adapter passes `eng.vtcDeg` and 0 until it exists. |
| `vtec` | High cam on: valves follow the high-cam profile and the middle lobes glow green. |
| `ignDeg` | Spark advance BTDC: when each plug flashes. |
| `knock` | Red combustion and a slight shake. |
| `fuelCut` | Rev limiter, decel or overboost cut: no combustion. |
| `lambda` | Rich burns orange, lean burns pale blue-white. |
| `turbo` | Show the turbo and charge pipe instead of the NA intake. |
| `materials` | `{ part: materialId }` from `materials.js`, for the materials colours. |
| `stress` | `{ rods, pistons, crank, valves, cams, head, block, exhaust, intake, turbo, clutch, flywheel }`, 0–1 with 1 at the limit. `rods`, `pistons` and `valves` also take a 4-number array, one per cylinder. |

## What the sim needs to provide

Everything the view uses comes through `stateFromSim`, so when the sim gains new models only the adapter changes:

- **VTC**: set `eng.vtcDeg` (intake cam advance) and the cams, valve timing and Internals panel follow it.
- **Materials**: today a material only divides that part's stress by a rough strength from `materials.js`. When the
  sim models materials, read `gui.materials` (or `onMaterialsChange`) in the sim and pass its own stress numbers.
- **Per-cylinder data**: `rods`, `pistons` and `valves` stress accept a 4-number array if the sim tracks cylinders separately.

## How stress is worked out

When the sim's internals model is running (`sim.internals`), its loads are used for pistons, rods (the worst of
rod, rod bolts and bearings), crank, head (gasket) and valvetrain, and a part that has failed shows red. The rest
are estimates from the limits the sim already has: These are indicators to spot a risky cell, not a failure model.

- **Rods, pistons, crank, block**: torque against the tune's torque warning (350 Nm for stock rods by default); rods
  and crank also rise with rpm². Pistons add a jump while knocking and with knock retard.
- **Valves, cams, flywheel**: (rpm / (rev limit + 600))², for valve float and inertia.
- **Head**: knock, intake air temperature and manifold pressure.
- **Exhaust heat**: rpm × load, plus late spark and lean mixture under load.
- **Turbo**: boost against the turbo's max boost. **Intake**: MAP against the overboost cut.
- **Clutch**: torque against the clutch capacity setting.
- **Materials**: each part's stress is divided by its material's rough strength (stock = 1).
