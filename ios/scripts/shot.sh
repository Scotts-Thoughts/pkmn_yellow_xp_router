#!/bin/bash
# shot.sh <route> <path> <out.png> [wait]: launch on the simulator at a screen and screenshot it
set -uo pipefail
SIM="${XPR_SIM:-iPhone 17 Pro}"
xcrun simctl terminate "$SIM" com.scottsthoughts.xprouter 2>/dev/null
SIMCTL_CHILD_XPR_SMOKE_ROUTE="$1" SIMCTL_CHILD_XPR_SMOKE_PATH="$2" SIMCTL_CHILD_XPR_SMOKE_B="${XPR_B:-}" xcrun simctl launch "$SIM" com.scottsthoughts.xprouter >/dev/null
sleep "${4:-5}"
xcrun simctl io "$SIM" screenshot "$3" >/dev/null 2>&1
sips -Z 1100 "$3" >/dev/null 2>&1
echo "$3"
