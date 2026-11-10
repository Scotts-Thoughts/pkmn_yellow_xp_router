#!/bin/bash
# Install and launch the built app on the simulator, seeding the sample
# routes into its Documents/saved_routes on first run.
set -euo pipefail
cd "$(dirname "$0")/.."
SIM="${XPR_SIM:-iPhone 17 Pro}"
xcrun simctl boot "$SIM" 2>/dev/null || true
APP=build/DerivedData/Build/Products/Debug-iphonesimulator/XP\ Router.app
xcrun simctl install "$SIM" "$APP"
DATA=$(xcrun simctl get_app_container "$SIM" com.scottsthoughts.xprouter data)
mkdir -p "$DATA/Documents/saved_routes"
cp -n ../tests/test_data/*.json "$DATA/Documents/saved_routes/" 2>/dev/null || true
xcrun simctl terminate "$SIM" com.scottsthoughts.xprouter 2>/dev/null || true
xcrun simctl launch "$SIM" com.scottsthoughts.xprouter ${XPR_ARGS:-} >/dev/null
echo "$DATA"
