# Honda Sim

A drive-it-yourself simulator for testing Honda tunes, starting with the 2006–2011 Civic Si (K20Z3, 6-speed).
Load your tune's tables, drive with a wheel and pedals (or the keyboard), and watch what the engine does:
VTEC switch, knock, mixture, power, and a live dyno chart.

![Honda Sim running a pull in 2nd gear](docs/screenshot.png)

## Run it

```
npm start
```

Then open http://localhost:8080 in Chrome or Edge. No install step: there are no dependencies.

## Getting your tune in

FlashPro `.fpcal` files are encrypted, so the sim can't read them directly. Copy the tables out of FlashPro Manager instead:

1. Open the calibration in FlashPro Manager (no car or FlashPro device needed).
2. Click inside a table, press **Ctrl+A** then **Ctrl+C**.
3. In Honda Sim, open **Paste a table from FlashPro Manager**, paste, pick which table it is, and press **Load into tune**.
4. Do this for the low and high cam ignition and fuel tables, then set the VTEC point and rev limit.
5. **Save tune (.json)** keeps the result so you don't have to paste again.

Have the tables as CSV files instead? **Load CSV tables** takes several at once and picks the table from the file name
(`afr`, `fuel`, `ignition`, `boost`, `wg_duty`). A single ignition or fuel table is used for both cams.

The fuel table can be lambda, AFR or injector pulse width (ms). The sim guesses from the numbers; check the **Fuel table units** setting.

## Wheel and pedals

Wheelbases and pedals show up in the browser as game controllers. Press any pedal so the browser notices them, then
use **Learn** next to each control: press the pedal all the way and let go, or press the paddle or button.
Mappings are saved in the browser. Without a clutch pedal mapped, the clutch works automatically for launches and shifts.

Keyboard: W/↑ throttle, S/↓ brake, C or Space clutch, E/Q shift up/down, 1–6 gears, 0 reverse, N neutral, R start.

## Turbo builds

**Turbo build preset** switches on a Garrett GT3582R, 1000 cc injectors, a 200 kPa overboost cut, an upgraded clutch,
and a torque warning (RODS lamp) at 350 Nm for stock connecting rods. Your tables stay as they are.
Boost comes from the wastegate spring plus the wastegate duty table. How much boost a given duty makes depends on your
solenoid and wastegate, so set **WG duty gain** until the sim's boost matches a real datalog.

## How it works

| File | What it does |
|---|---|
| `src/tables.js` | Table lookups and the parser for tables pasted from FlashPro Manager |
| `src/k20z3.js` | Car data (gear ratios, weight, tires) and the engine's "truth": airflow per cam, best timing, knock limit, friction |
| `src/engine.js` | Runs the tune like the ECU (fuel, low/high cam ignition, VTEC, rev limit, decel cut, idle) and works out torque |
| `src/vehicle.js` | Clutch, gearbox and road load |
| `src/sim.js` | Ties it together: shifting, auto-clutch, stalling, datalog |
| `src/input.js` | Gamepad API wheel/pedal mapping and keyboard |
| `src/ui.js` | Dash, table view with the active cell highlighted, dyno chart |
| `tools/calibrate.mjs` | Re-solves the volumetric efficiency tables from the torque targets |

The engine and car are calibrated to public data: Honda's 197 hp / 139 lb-ft rating and gear ratios, Hondata's
VTEC crossover data, and road tests (0–60 in 6.8 s and a 15.2 s quarter mile at 94 mph, against 6.5–7.2 s and
14.9–15.6 s for the real car). See [docs/CALIBRATION.md](docs/CALIBRATION.md) for sources and what isn't calibrated yet.
It is still a model, not your car.
Use it to catch mistakes in a tune (timing past the knock limit, a VTEC point that leaves a hole, lean cells under load), not as a dyno sheet.
The placeholder tune that loads first is generic and is not Honda's or Hondata's calibration.

## Tests

```
npm test
```
