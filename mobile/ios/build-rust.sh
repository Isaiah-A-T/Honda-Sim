#!/bin/sh
# Builds the Rust side of the iOS app (this crate plus mobile/shared, the sim and the viewer) as a static
# library for the device or the simulator, whichever Xcode is building for. Xcode runs this before compiling.
set -e
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
case "$PLATFORM_NAME" in
  iphonesimulator) target=aarch64-apple-ios-sim ;;
  *) target=aarch64-apple-ios ;;
esac
# Xcode points SDKROOT at the iOS SDK, which would break the Rust build scripts that run on the Mac itself.
env -u SDKROOT cargo build --release -p honda-sim-ios --target "$target"
out="target/ios/${PLATFORM_NAME:-iphoneos}"
mkdir -p "$out"
cp "target/$target/release/libhonda_sim_ios.a" "$out/"
