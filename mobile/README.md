# Honda Sim on phones

The same sim, tune tables, parts, cooling, internals, datalog and 3D engine view as the Windows app, laid
out for a phone: a compact dash along the top, the engine filling the screen with touch pedals over it,
and a **Menu** that opens each of the desktop's tabs as a page.

| Folder | What it is |
|---|---|
| `shared/` | The phone app itself. The sim (`native/sim`) and the 3D engine model (`viewer/`) are used unchanged, and every page is the desktop app's own tab (`native/app/src/gui.rs`, compiled in by `shared/build.rs`), so a feature added to the desktop shows up on phones too. `src/gui/phone.rs` is the phone menu and dash, `src/app.rs` the screen layout, `src/input.rs` multi-touch, `src/keyboard.rs` the on-screen keyboard, `src/scene.rs` the 3D view. |
| `android/` | The Android entry point and APK settings. |
| `ios/` | The iOS entry point (an OpenGL ES 3 surface), the Xcode project spec and the script Xcode runs to build the Rust code. |

It's its own Cargo workspace, so it builds without touching `native/` or `viewer/`.

## Using it

- **Pedals.** Right pedal gas, left pedal brake: press higher up the pedal for more. The narrow pedal on the
  far left is the clutch (it's automatic unless you hold it). The two buttons beside the gas shift up and
  down; **START**, **N** and **R** are along the top right. Use several fingers at once, like real pedals.
- **Engine view.** Drag to turn it, pinch to zoom, tap a part to see its name and load. Menu › 3D view picks
  the view mode (internals, airflow, cooling, stress…) and camera.
- **Menu.** Drive, Tables, Settings, Parts, Internals, Cooling, Controller, Log and 3D view. In landscape the
  page opens on the left with the engine still running on the right, so you can change a table and rev it.
- **Typing.** Tap a number or text box and a keyboard appears over the engine side of the screen. You can
  also drag a number box left and right. **Paste** pastes the phone's clipboard, so a table copied from
  FlashPro (or a spreadsheet) on the phone can be pasted into Tables › Paste a table.
- **Gamepad.** Pair a Bluetooth controller in the phone's settings. Right trigger is gas, left trigger brake,
  bumpers shift, Start starts the engine; change any of them in Menu › Controller. Triggers are on/off.
- **Files.** Tunes, logs and the controller mapping are the same files as on Windows. Android keeps them in
  `Android/data/com.hondasim.app/files/HondaSim`; on iPhone they're in the Files app under On My iPhone ›
  Honda Sim.

## Android

The **Android APK** workflow builds `HondaSim.apk` whenever this code changes and publishes it as the
`android-latest` pre-release. On the phone, open that release, download `HondaSim.apk`, open it, and allow
your browser to install apps when Android asks. Needs Android 8 or newer on a 64-bit phone.

To build it yourself: the Android SDK and NDK, then

```
rustup target add aarch64-linux-android
cargo install cargo-apk
keytool -genkeypair -keystore target/honda-sim-release.keystore -alias androiddebugkey -keyalg RSA -validity 10000 -storepass android -keypass android -dname "CN=Honda Sim"
cargo apk build --release -p honda-sim-android        # target/release/apk/HondaSim.apk
cargo apk run --release -p honda-sim-android          # build, install and start on a phone plugged in by USB
```

Each CI build is signed with a new key unless the repository has the secrets `ANDROID_KEYSTORE_BASE64` and
`ANDROID_KEYSTORE_PASSWORD`, so without them, uninstall the old version before installing a new one.

## iPhone and iPad

Apple only lets apps onto an iPhone signed by an Apple ID, so this part needs you:

1. **On a Mac with Xcode** (free Apple ID is enough for your own phone; apps signed with a free ID expire
   after 7 days and need reinstalling, a paid developer account lasts a year):
   ```
   rustup target add aarch64-apple-ios aarch64-apple-ios-sim
   brew install xcodegen
   cd mobile/ios/xcode && xcodegen && open HondaSim.xcodeproj
   ```
   In Xcode: select the HondaSim target › Signing & Capabilities › pick your team, plug in the phone, choose
   it as the run destination and press Run. The first time, the iPhone asks you to trust the developer
   (Settings › General › VPN & Device Management) and to turn on Developer Mode.
2. **Without a Mac:** the **iOS app** workflow builds `HondaSim-unsigned.ipa` on GitHub's Macs and publishes
   it as the `ios-latest` pre-release. A sideloading tool on Windows (such as Sideloadly or AltStore) can sign
   it with your Apple ID and install it over USB, with the same 7-day limit for a free Apple ID.

The 3D view uses OpenGL ES, which Apple has deprecated but still ships on iPhone and iPad.

## Trying the phone layout on a computer

```
cargo run --release -p honda-sim-mobile --bin honda-sim-phone-preview            # landscape phone
cargo run --release -p honda-sim-mobile --bin honda-sim-phone-preview portrait
```

The mouse acts as one finger and the scroll wheel zooms.

## Not the same as the desktop yet

- Gamepad triggers are on/off rather than analog, and steering wheels aren't supported on phones.
- "Open tunes folder" and "Open logs folder" do nothing on phones; use the file locations above.
