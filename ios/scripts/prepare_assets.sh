#!/bin/bash
# Prescale the shared artwork (assets/pkmn_icons, assets/box_art) into the
# app bundle's resource folder. Re-runs only for files that changed.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$HERE/../.."
OUT="$HERE/../XPRouter/Resources/Art"
mkdir -p "$OUT/pkmn" "$OUT/boxart"
cp "$ROOT/assets/pkmn_icons/dex_lookup.json" "$OUT/dex_lookup.json"
for f in "$ROOT"/assets/pkmn_icons/*.png; do
  dst="$OUT/pkmn/$(basename "$f")"
  if [ ! -f "$dst" ] || [ "$f" -nt "$dst" ]; then
    sips -Z 120 "$f" --out "$dst" >/dev/null
  fi
done
for f in "$ROOT"/assets/box_art/*.png; do
  # "1-Yellow.png" -> "Yellow.png"
  name="$(basename "$f")"; name="${name#*-}"
  dst="$OUT/boxart/$name"
  if [ ! -f "$dst" ] || [ "$f" -nt "$dst" ]; then
    sips -Z 300 "$f" --out "$dst" >/dev/null
  fi
done
echo "art ready"
