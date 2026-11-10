#!/bin/bash
# Build the app for the simulator (xcodegen + xcodebuild). Prints only errors.
set -uo pipefail
cd "$(dirname "$0")/.."
xcodegen generate --quiet
xcodebuild -project XPRouter.xcodeproj -scheme XPRouter -destination "platform=iOS Simulator,name=${XPR_SIM:-iPhone 17 Pro}" -derivedDataPath build/DerivedData build 2>&1 \
  | grep -E "error:|BUILD (SUCCEEDED|FAILED)" | sort -u | head -${XPR_MAX_ERRORS:-60}
