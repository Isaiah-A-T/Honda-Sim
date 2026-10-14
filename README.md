# Honda Sim

A drive-it-yourself simulator for testing Honda K-series tunes, starting with the 2006–2011 Civic Si (K20Z3, 6-speed).
Load your tune's tables, drive with a wheel and pedals, a gamepad, the keyboard or touch pedals, and watch what the
engine does: VTEC, knock, mixture, fuel flow, boost, temperatures and part loads, with a 3D view
of the engine running in the middle of the screen.

Current version: **0.11.1** (Windows, Android and iPhone, all from the same code).

![A 2nd gear pull on the stock K20Z3, just past the 5800 rpm VTEC point](docs/screenshots/drive-vtec.png)

## Get it

| Platform | Where |
|---|---|
| Windows | `HondaSim.exe` from the newest release on the [Releases](../../releases) page. One file, no installer. Windows may warn that it isn't signed: choose **More info**, then **Run anyway**. |
| Android | `HondaSim.apk` from the `android-latest` pre-release. Android 8 or newer, 64-bit. Open it on the phone and allow your browser to install apps when asked. |
| iPhone and iPad | `HondaSim-unsigned.ipa` from the `ios-latest` pre-release, signed with your Apple ID (Sideloadly or AltStore), or built from Xcode. See [shared/phone/README.md](shared/phone/README.md). |

Settings, tunes and logs live in `%APPDATA%\HondaSim` on Windows; the phone apps keep the same files (see the phone README).

## What you can do in it

**Drive it.** Start the engine, pick a gear and floor it. The dash along the top shows rpm, gear, speed, MAP, boost,
AFR, timing, knock retard, VTC, torque, power and temperatures, with lights for VTEC, knock, rev limit, fans and
cold, hot or heat-soaked conditions. The Drive tab times 0–60 and the quarter mile and shows what the ECU is doing
right now: the ignition cell against best-torque and knock-limit timing, lambda, injector pulse and duty, fuel pump
use, volumetric efficiency and more.

Keyboard: **R** start, **W**/**S** throttle and brake (hold Shift for faster), **C** or Space clutch, **E**/**Q**
shift, **1**–**6** gears, **0** reverse, **N** neutral. Map a wheel, pedals or gamepad on the Wheel tab.

**Edit your tune.** The Tables tab holds the eight tables a FlashPro tune needs: fuel, ignition and cam angle for low
and high cam, boost target and wastegate duty, plus the VTEC point, rev limit, injector size and dead time. Click a
cell to edit it; the cell the engine is on right now is underlined. Paste whole tables straight from FlashPro
Manager (below), and save named tunes.

![Tables tab with the low cam fuel table](docs/screenshots/tables.png)

**Pick the fuel and pump.** New in 0.11: a fuel dropdown (87, 91 and 93 pump gas, E85, Sunoco 260 GT, VP C16), each
with its own octane, stoichiometric ratio, energy and charge cooling, and a fuel pump picker from 20 to 1000 l/h.
The rail is a fixed 50 psi returnless rail; push past the pump's flow and the rail sags and the mixture goes lean.
See [docs/FUEL.md](docs/FUEL.md).

![Settings tab with the fuel dropdown open](docs/screenshots/fuel.png)

**Add boost.** Bolt on a GT35-class turbo kit, a Precision 6262 or a supercharger, with intercooler, wastegate,
overboost cut and turbo oil and coolant lines. Turbo setups now get fuel and ignition columns up to 250 kPa, so
boost runs its own cells instead of the last NA column.

![GT35 turbo setup on E85](docs/screenshots/turbo-setup.png)

**See inside it.** The 3D view has modes for the full engine, internals, block, airflow (charge air in blue,
exhaust in orange, cylinder by cylinder in firing order), cooling and oil, and stress. Hover or tap a part to see
its name and load.

![Airflow view on a 7 psi pull in 3rd](docs/screenshots/boost-airflow.png)

**Watch the parts.** Parts lets you fit aftermarket cranks, stroker kits, rods and pistons with real fitment rules
and materials. Internals shows each part's load and fatigue live, and parts fail when they're pushed past their
limit. Cooling models the coolant circuit, thermostat, radiator, fans, oil temperature, viscosity and pressure,
bearing oil feed and piston squirters; try **Cold start**.

![Internals tab with the cooling and oil view](docs/screenshots/internals-cooling.png)

## On a phone

The phone apps are the same sim and the same tabs, laid out for a phone: a compact dash on top, the engine filling
the screen and touch pedals over it (press higher on a pedal for more). **Menu** opens each desktop tab as a page;
in landscape the page opens on the left with the engine still running on the right. Bluetooth gamepads work too.

| Driving | Menu |
|---|---|
| ![Phone app mid-pull with the gas pedal held](docs/screenshots/phone-drive.png) | ![Phone menu open beside the running engine](docs/screenshots/phone-menu.png) |

## Getting your tune in

FlashPro `.fpcal` files are encrypted, so the sim can't read them directly. Copy the tables out of FlashPro Manager instead:

1. Open the calibration in FlashPro Manager (no car or FlashPro device needed).
2. Click inside a table, press **Ctrl+A** then **Ctrl+C**.
3. In Honda Sim, open **Tables**, then **Paste a table**, paste, pick which table it is, and load it.
4. Do this for the low and high cam ignition and fuel tables, then set the VTEC point and rev limit.
5. Save the tune so you don't have to paste again.

The fuel table can be lambda, AFR or injector pulse width (ms). The sim guesses from the numbers; check the
**Fuel table holds** setting on the Tables tab. On a phone, copy the table on the phone and use **Paste**.

## How close is it?

The engine and car are calibrated to public data: Honda's 197 hp / 139 lb-ft rating and gear ratios, Hondata's
VTEC crossover data, road tests, and published dynos of boosted builds (within about 8%). See
[docs/CALIBRATION.md](docs/CALIBRATION.md), [docs/VALIDATION.md](docs/VALIDATION.md) and [docs/FUEL.md](docs/FUEL.md).
It is still a model, not your car. Use it to catch mistakes in a tune (timing past the knock limit, a VTEC point that
leaves a hole, lean cells under load, a pump that can't keep up), not as a dyno sheet. The placeholder tune that
loads first is generic and is not Honda's or Hondata's calibration.

## Folders

| Folder | What it is |
|---|---|
| [`shared/core`](shared/core) | The sim itself (`honda-sim-core`, Rust): engine, gas exchange, fuel, turbo, cooling, drivetrain, internals, parts and engines. Every app uses it. |
| [`shared/viewer`](shared/viewer) | The 3D engine view (Rust), used by every app. |
| [`shared/phone`](shared/phone) | The phone app shared by Android and iOS: touch pedals, the phone menu and layout. |
| [`windows`](windows) | `HondaSim.exe`: the Windows program and the tune, parts, cooling and datalog panels (the phone app reuses them). |
| [`android`](android) | The Android entry point and APK settings. |
| [`ios`](ios) | The iOS entry point and the Xcode project. |
| [`web`](web) | The original browser and Electron version. Its JavaScript model (`web/src`) is the reference the Rust core is tested against, and `web/tools` holds the calibration and data scripts. |
| [`docs`](docs) | How each model works, its sources and what's estimated, plus the parts, engines, fuels and materials data. [docs/MODULES.md](docs/MODULES.md) covers plugging other programs into the sim. |

The Rust crates are one Cargo workspace (this folder's `Cargo.toml`).

## Building

```
cargo test --release -p honda-sim-core                                   # the sim against the JavaScript model
cargo build --release --target x86_64-pc-windows-gnu -p honda-sim-app    # HondaSim.exe (needs mingw-w64)
cargo run --release -p honda-sim-app                                     # the desktop app on Linux (needs libudev-dev)
cargo run --release -p honda-sim-mobile --bin honda-sim-phone-preview    # the phone layout on a computer
cd web && npm test                                                       # the JavaScript model's tests
```

GitHub Actions builds the releases: **Release** for Windows (push a `v*` tag or run it from the Actions tab),
**Android APK** and **iOS app** (.github/workflows). Release notes are in `.github/release-notes`. Android and iOS
build steps are in [shared/phone/README.md](shared/phone/README.md).

## License

MIT, see [LICENSE](LICENSE).
