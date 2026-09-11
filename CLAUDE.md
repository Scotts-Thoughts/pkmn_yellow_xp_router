# CLAUDE.md

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
