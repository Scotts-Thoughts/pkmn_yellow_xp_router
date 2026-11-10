# XP Router for iPhone

A native SwiftUI app over the Rust port's business logic. The route engine,
damage calculator, map compositor and the desktop's UI-free controllers
(`MainController`, `BattleController`, the PP ledger, the route index, the
map's route state) are compiled into `rust/crates/xpr-mobile` straight from
`rust/crates/xpr-app/src` (`#[path]` modules), so a fix there reaches both
apps. Swift only renders what the core returns.

```
ios/
  project.yml              XcodeGen spec (XPRouter.xcodeproj is generated from it)
  scripts/build_rust.sh    builds xpr-mobile for device + simulator -> build/XPRCore.xcframework
  scripts/prepare_assets.sh  prescales assets/pkmn_icons + assets/box_art into the bundle
  scripts/build_app.sh     xcodegen + xcodebuild for the simulator (errors only)
  scripts/run_sim.sh       install + launch, seeding tests/test_data routes
  scripts/shot.sh          launch at a screen (XPR_SMOKE_ROUTE / _PATH) and screenshot
  scripts/ui_test.sh       UI flow tests (XPR_TOUR=1 / XPR_NET=1 for the extra ones)
  scripts/make_icon.py     draws the app icon
  XPRouter/Core            FFI bridge (Core.swift), JSON model, app state, theme, art
  XPRouter/Views           screens
  XPRouterUITests          tap-through tests
rust/crates/xpr-mobile     C ABI (include/xpr_mobile.h) + the JSON command session
```

## Build

Needs Xcode 26, `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`
and `brew install xcodegen`.

```
cd ios
scripts/build_app.sh && scripts/run_sim.sh     # simulator
open XPRouter.xcodeproj                        # device: pick your team under Signing
```

The Xcode build runs `build_rust.sh` and `prepare_assets.sh` first (release
Rust by default; `XPR_RUST_PROFILE=dev` for faster debug builds).

Tests: `cd rust && cargo test -p xpr-mobile` (drives every command over the
sample routes) and `ios/scripts/ui_test.sh`.

## The core API

`xpr_call("<group>.<method>", "<json params>")` returns `{"ok": ...}` or
`{"err": "..."}`; `xpr_png` returns map pictures. Groups (see
`rust/crates/xpr-mobile/src/session/`): `app`, `library` / `route` /
`newroute`, `tree` / `edit` / `folder`, `event` (detail, create / update via
"drafts"), `opts` (picker lists), `battle`, `summary`, `compare`, `map`,
`settings` / `customgen`.

Events are created and edited as *drafts* (`session/drafts.rs`): a flat
`{"kind": ..., fields}` object per event kind, built into an
`EventDefinition` by the core. Edits start from the existing definition, so
battle-summary setup, tags and recorded times survive.

## Files and sharing

Routes live in the app's Documents folder (Files ▸ On My iPhone ▸ XP Router
▸ saved_routes) in the desktop's JSON format; backups go to
`outdated_routes`, custom games to `custom_gens`. Open a route JSON from
Files, Mail or AirDrop to import it; share any route from the library
(swipe right) or the editor's menu. Named routes are saved automatically
when the app goes to the background.

## Not on iPhone

Desktop-only features that need a PC: recording from an emulator
(GameHook / Super Shuckie, quick start, final-trainer auto-stop), route from
replay, in-app updates and window / font / keyboard-shortcut settings. Theme colours follow iOS light / dark mode.
Map overworld sprites are drawn as coloured markers.
