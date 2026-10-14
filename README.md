# Honda Sim

A drive-it-yourself simulator for testing Honda K-series tunes, starting with the 2006–2011 Civic Si (K20Z3, 6-speed).
Load your tune's tables, drive with a wheel and pedals, a gamepad, the keyboard or touch pedals, and watch what the
engine does: VTEC, knock, mixture, boost, part loads and a live dyno chart, with a 3D view of the engine.

![Honda Sim running a pull in 2nd gear](docs/screenshot.png)

## Get it

| Platform | Where |
|---|---|
| Windows | `HondaSim.exe` from the newest release on the [Releases](../../releases) page. One file, no installer. Windows may warn that it isn't signed: choose **More info**, then **Run anyway**. |
| Android | `HondaSim.apk` from the `android-latest` pre-release. |
| iPhone and iPad | `HondaSim-unsigned.ipa` from the `ios-latest` pre-release, signed with your Apple ID, or built from Xcode. See [shared/phone/README.md](shared/phone/README.md). |

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
| [`docs`](docs) | How each model works, its sources and what's estimated, plus the parts, engines and materials data. |

The Rust crates are one Cargo workspace (this folder's `Cargo.toml`).

## Building

```
cargo test --release -p honda-sim-core                                   # the sim against the JavaScript model
cargo build --release --target x86_64-pc-windows-gnu -p honda-sim-app    # HondaSim.exe (needs mingw-w64)
cargo run --release -p honda-sim-mobile --bin honda-sim-phone-preview    # the phone layout on a computer
cd web && npm test                                                       # the JavaScript model's tests
```

GitHub Actions builds the releases: **Release** for Windows, **Android APK** and **iOS app**
(.github/workflows). Android and iOS build steps are in [shared/phone/README.md](shared/phone/README.md).

## Getting your tune in

FlashPro `.fpcal` files are encrypted, so the sim can't read them directly. Copy the tables out of FlashPro Manager instead:

1. Open the calibration in FlashPro Manager (no car or FlashPro device needed).
2. Click inside a table, press **Ctrl+A** then **Ctrl+C**.
3. In Honda Sim, open **Tables**, then **Paste a table**, paste, pick which table it is, and load it.
4. Do this for the low and high cam ignition and fuel tables, then set the VTEC point and rev limit.
5. Save the tune so you don't have to paste again.

The fuel table can be lambda, AFR or injector pulse width (ms). The sim guesses from the numbers; check the fuel table units setting.

## How close is it?

The engine and car are calibrated to public data: Honda's 197 hp / 139 lb-ft rating and gear ratios, Hondata's
VTEC crossover data, road tests, and published dynos of boosted builds (within about 8%). See
[docs/CALIBRATION.md](docs/CALIBRATION.md), [docs/VALIDATION.md](docs/VALIDATION.md) and [docs/FUEL.md](docs/FUEL.md).
It is still a model, not your car. Use it to catch mistakes in a tune (timing past the knock limit, a VTEC point that
leaves a hole, lean cells under load), not as a dyno sheet. The placeholder tune that loads first is generic and is
not Honda's or Hondata's calibration.
