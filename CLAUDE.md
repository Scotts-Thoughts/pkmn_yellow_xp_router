# CLAUDE.md

## Python version is deprecated

The Python version of this program (the `gui`/`controllers` tkinter app and
its Qt rewrite under `gui_qt`) is deprecated. It should NOT receive updates
going forward by default. All new work should go into the Rust port instead,
unless the user explicitly says otherwise for a specific task.

## Rust app: raw input must respect modal dialogs

Every dialog goes through `dialogs::modal()` (an `egui::Modal`). An egui modal
only blocks *widgets* under it: hover/click/drag responses and keyboard focus.
Code that reads raw input still sees every click and key while a dialog is
open. That includes `ui.input(|i| i.pointer…)`, hand-rolled hit-testing against
`interact_pos()`, `key_pressed`, `consume_key` and `i.events`. Pages also draw
before dialogs each frame, so that code acts first. For example, clicking on the
"Assign Move to Slot N" dialog used to select the route-list row underneath,
which changed where the move event was inserted.

- Prefer real widgets (`ui.interact`, `allocate_response`,
  `Response::clicked()`), which modals block automatically.
- If you must read raw input on anything a dialog can cover (pages, panels,
  popovers), check `xpr_ui_kit::modal::behind_modal(ui)` first, or
  `layer_behind_modal(ctx, layer)` when there is no `Ui`, and ignore the input
  when it returns true. The only exceptions are reads that run only while the
  widget has keyboard focus, or during a drag it started. egui already drops
  focus and blocks new drags under a modal.
- Secondary OS windows (`show_viewport_immediate`) are separate viewports that
  the main window's modal can't cover. While `self.dialog` or `self.message` is
  set, call `dialogs::block_secondary_window` inside them.
- Cover new raw-input code with a case in
  `rust/crates/xpr-app/tests/modal_input.rs`.

## Game decompilation repos (reference)

Local clones of the Pokémon decomp projects. Consult these whenever we need to
verify actual game behavior — formulas, move effects, trainer/encounter data,
EXP and money calculations, AI logic, etc. Prefer reading the real game code
over guessing or relying on wiki-level summaries.

### Gen 1
- **Red & Blue**: `A:\Cygwin\home\scott\pokered`
- **Yellow**: `A:\Cygwin\home\scott\pokeyellow`

### Gen 2
- **Gold & Silver**: `A:\Cygwin\home\scott\pokegold`
- **Crystal**: `A:\Cygwin\home\scott\pokecrystal`

### Gen 3
- **Ruby & Sapphire**: `A:\decomps\pokeruby`
- **Emerald**: `A:\decomps\pokeemerald`
- **FireRed & LeafGreen**: `A:\decomps\pokefirered`

### Gen 4
- **Diamond & Pearl**: `A:\Dropbox\stp-projects\programs\poke_map\repos\pokediamond`
- **Platinum**: `A:\decomps\pokeplatinum`
- **HeartGold & SoulSilver**: `A:\Dropbox\stp-projects\programs\poke_map\repos\pokeheartgold`

Gen 1/2 repos are assembly (`.asm`) based; Gen 3 is C; Gen 4 is C/assembly with
data in `.narc`/`.c` files. These are outside the project working directory, so
read them with absolute paths.

# Sub-Agents
When starting sub-agents for complex tasks use Sonnet at high effort unless the user states otherwise.