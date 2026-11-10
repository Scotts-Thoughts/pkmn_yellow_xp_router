#!/bin/bash
# Run the UI flow tests; failure screenshots land in ${XPR_SHOTS:-/tmp}.
cd "$(dirname "$0")/.."
xcodegen generate --quiet
TEST_RUNNER_XPR_TOUR="${XPR_TOUR:-}" TEST_RUNNER_XPR_NET="${XPR_NET:-}" TEST_RUNNER_XPR_SHOTS="${XPR_SHOTS:-/tmp}" xcodebuild -project XPRouter.xcodeproj -scheme XPRouter -destination "platform=iOS Simulator,name=${XPR_SIM:-iPhone 17 Pro}" -derivedDataPath build/DerivedData test ${XPR_ONLY:+-only-testing:XPRouterUITests/FlowTests/$XPR_ONLY} 2>&1 | grep -E "error:|Test Case.*(passed|failed)|TEST (SUCCEEDED|FAILED)" | head -40
