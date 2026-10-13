# Plugging modules into the sim

Display and logging modules (the 3D engine, extra gauges, a datalogger) read the sim through one interface,
`src/simstate.js`, so the engine model can change underneath them without breaking them.

```js
import { renderState, valveLift, FIRING_OFFSET_DEG } from './simstate.js';

const view = createMyModule(container);          // the module's own factory
// every animation frame, after the sim has stepped:
view.update(renderState(sim), dtSeconds);
```

`renderState(sim)` returns plain data (JSON-safe), built fresh each call. Modules never write back to the sim.

| Field | What it is |
|---|---|
| `running`, `firing`, `rpm`, `crankDeg` | Engine running, burning fuel this cycle (false on rev cut, decel cut, overboost cut), speed, crank angle 0–720 with cylinder 1 firing TDC at 0 |
| `gear`, `speedMs`, `throttlePct`, `mapKpa` | Drivetrain and load |
| `cams` | `highCam` (VTEC engaged), `vtcDeg` (intake cam advance), `vtecRpm` |
| `combustion` | Spark, best-torque spark, knock limit, crank angle where half the charge has burned (`ca50Deg`), burn duration, `knocking`, knock index (1 = edge), λ |
| `flow` | VE, airflow g/s, residual exhaust share, pumping loss, exhaust manifold pressure, EGT, peak cylinder pressure |
| `turbo` | `null` without a turbo, else boost, shaft rpm, wastegate opening 0–1, turbine and compressor power |
| `temps` | Piston crown, intake air, coolant |
| `parts` | Keyed by part (`piston`, `rod`, `rodBolt`, `bearing`, `crank`, `gasket`, `headBolt`, `sleeve`, `block`, `valvetrain`): `load` (0.8 = endurance limit, 1.5 = breaks), `damage` (life used 0–1), `failed`, and the selected `material` |
| `failed` | Name of the failed part, or `null` |
| `forces` | Rod force down on the firing stroke and up at TDC (N), sleeve hoop stress (MPa) |
| `geometry` | Bore, stroke, rod length, valve sizes (m) |

Helpers:
- `valveLift(state, cyl, intake, crankDeg?)` gives valve lift in metres for cylinder 1–4 from the same cam lobes and VTC
  phasing the flow model uses (high or low cam as engaged).
- `FIRING_OFFSET_DEG` is each cylinder's firing TDC after cylinder 1's (firing order 1-3-4-2).

A module that animates in slow motion can keep its own crank angle and advance it by `rpm` × its time scale, as the
3D renderer does; `crankDeg` is the sim's real-time angle.
