#!/bin/bash
# Build the xpr-mobile static library for the device and the simulator and
# wrap both in ios/build/XPRCore.xcframework (with the C header), which the
# Xcode project links. Run from anywhere; Xcode runs it as a pre-build phase.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
IOS="$HERE/.."
RUST="$IOS/../rust"
OUT="$IOS/build"
PROFILE="${XPR_RUST_PROFILE:-release}"
export IPHONEOS_DEPLOYMENT_TARGET=17.0
# Xcode's build environment points these at the iOS SDK; cargo's build
# scripts must compile for the host.
unset SDKROOT LIBRARY_PATH 2>/dev/null || true
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

cd "$RUST"
TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim)
for t in "${TARGETS[@]}"; do
  if [ "$PROFILE" = "release" ]; then
    cargo build -p xpr-mobile --lib --release --target "$t"
  else
    cargo build -p xpr-mobile --lib --target "$t"
  fi
done
DIR="$PROFILE"; [ "$PROFILE" = "dev" ] && DIR=debug
mkdir -p "$OUT/include"
cp "$RUST/crates/xpr-mobile/include/xpr_mobile.h" "$OUT/include/"
cat > "$OUT/include/module.modulemap" <<MM
module XPRCoreFFI {
    header "xpr_mobile.h"
    export *
}
MM
# only re-wrap when a library changed
STAMP="$OUT/.stamp"
NEWER=1
if [ -f "$STAMP" ] && [ -d "$OUT/XPRCore.xcframework" ]; then
  NEWER=0
  for t in "${TARGETS[@]}"; do
    if [ "$RUST/target/$t/$DIR/libxpr_mobile.a" -nt "$STAMP" ]; then NEWER=1; fi
  done
  if [ "$RUST/crates/xpr-mobile/include/xpr_mobile.h" -nt "$STAMP" ]; then NEWER=1; fi
fi
if [ "$NEWER" = 1 ]; then
  rm -rf "$OUT/XPRCore.xcframework"
  xcodebuild -create-xcframework \
    -library "$RUST/target/aarch64-apple-ios/$DIR/libxpr_mobile.a" -headers "$OUT/include" \
    -library "$RUST/target/aarch64-apple-ios-sim/$DIR/libxpr_mobile.a" -headers "$OUT/include" \
    -output "$OUT/XPRCore.xcframework" >/dev/null
  touch "$STAMP"
fi
echo "XPRCore.xcframework ready"
