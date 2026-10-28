"""Stage the gen 4/5 map pictures as release assets and say which to upload.

The executable no longer carries the map pictures (~380 MB for the five
games); the app downloads a game's pictures the first time its map is shown
(``xpr_update::assets``) and keeps them across updates. Each game's zip is a
release asset named after its content,
``imagery_<game>_<first 16 hex of sha256>.zip``, and the app finds it on
whichever release has it, so a release only needs the pictures that changed
since the last one. The same files serve Windows and macOS.

``windows_build.py`` and ``mac_build.py`` run this after packaging; it can
also run on its own:

    py -3.14 rust/release_assets.py [--out DIR] [--tag vX.Ya] [--new-only]

1. Checks every ``map_data/<game>/imagery.zip`` against the size and sha256
   its ``manifest.json`` records (a mismatch would make the app refuse the
   download).
2. Links (or copies) them into ``<out>/imagery/`` under their asset names,
   removing names that are no longer current.
3. Asks GitHub which of those names no release has yet, and prints the
   ``gh release upload`` command for just those (``--new-only`` also leaves
   only those staged: the release workflow attaches whatever is there).
   Never delete an older release that still holds pictures a current
   build uses.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import urllib.request

RUST_DIR = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(RUST_DIR)
RELEASES_URL = "https://api.github.com/repos/Scotts-Thoughts/pkmn_yellow_xp_router/releases"
SHA_PREFIX_LEN = 16  # matches xpr_update::assets::imagery_asset_name


def asset_name(game: str, sha256: str) -> str:
    return f"imagery_{game}_{sha256[:SHA_PREFIX_LEN]}.zip"


def sha256_of(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def imagery_assets() -> list[dict]:
    """(game, asset name, source zip, bytes) for every pack with pictures, checked against its manifest."""
    out = []
    map_data = os.path.join(ROOT, "map_data")
    for game in sorted(os.listdir(map_data)):
        manifest = os.path.join(map_data, game, "manifest.json")
        if not os.path.isfile(manifest):
            continue
        info = json.load(open(manifest, encoding="utf-8")).get("imagery")
        if not info:
            continue
        zip_path = os.path.join(map_data, game, info.get("zip", "imagery.zip"))
        if not os.path.isfile(zip_path):
            raise SystemExit(f"{game}: the manifest names {zip_path}, which is missing")
        size = os.path.getsize(zip_path)
        if size != info["bytes"]:
            raise SystemExit(f"{game}: imagery.zip is {size} bytes, the manifest says {info['bytes']}")
        if sha256_of(zip_path) != info["sha256"].lower():
            raise SystemExit(f"{game}: imagery.zip does not match the manifest's sha256")
        out.append({"game": game, "name": asset_name(game, info["sha256"].lower()), "src": zip_path, "bytes": size})
    return out


def stage(assets: list[dict], out_dir: str) -> str:
    """Put the assets in ``<out_dir>/imagery/`` under their release names."""
    dest = os.path.abspath(os.path.join(out_dir, "imagery"))
    os.makedirs(dest, exist_ok=True)
    current = {a["name"] for a in assets}
    for name in os.listdir(dest):
        if name.startswith("imagery_") and name not in current:
            os.remove(os.path.join(dest, name))
    for a in assets:
        target = os.path.join(dest, a["name"])
        if os.path.exists(target) and os.path.getsize(target) == a["bytes"]:
            continue
        if os.path.exists(target):
            os.remove(target)
        try:
            os.link(a["src"], target)
        except OSError:
            shutil.copy2(a["src"], target)
    return dest


def published_asset_names() -> set[str] | None:
    """Every asset name on any published release, or None if GitHub can't be asked."""
    names: set[str] = set()
    try:
        for page in range(1, 11):
            headers = {"User-Agent": "pkmn_xp_router-release"}
            token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
            if token:  # CI: the anonymous rate limit is shared by the runner pool
                headers["Authorization"] = f"Bearer {token}"
            req = urllib.request.Request(f"{RELEASES_URL}?per_page=100&page={page}", headers=headers)
            with urllib.request.urlopen(req, timeout=20) as r:
                data = json.load(r)
            if not data:
                break
            for rel in data:
                if rel.get("draft"):
                    continue
                names.update(a.get("name", "") for a in rel.get("assets", []))
    except Exception as e:  # offline, rate-limited, ...
        print(f"could not ask GitHub which pictures are already published: {e}")
        return None
    return names


def app_version() -> str:
    consts = open(os.path.join(RUST_DIR, "crates", "xpr-core", "src", "consts.rs"), encoding="utf-8").read()
    m = re.search(r'pub const APP_VERSION: &str = "([^"]+)"', consts)
    return m.group(1) if m else "<tag>"


def stage_and_report(out_dir: str, tag: str, new_only: bool = False) -> list[str]:
    """Stage the pictures and print what the release needs; returns the paths to upload.
    ``new_only`` leaves only those in the staging folder (the release workflow
    attaches everything in it)."""
    assets = imagery_assets()
    total = sum(a["bytes"] for a in assets)
    published = published_asset_names()
    if published is None:
        todo = assets
        print("  upload any of these that no release has yet:")
    else:
        todo = [a for a in assets if a["name"] not in published]
        for a in assets:
            if a["name"] in published:
                print(f"  {a['name']}: already on GitHub, nothing to upload")
    dest = stage(todo if new_only else assets, out_dir)
    print(f"map pictures: {len(assets)} games, {total / 1e6:.1f} MB, staged in {dest}")
    paths = [os.path.join(dest, a["name"]) for a in todo]
    for a in todo:
        print(f"  {a['name']}: NEW ({a['bytes'] / 1e6:.1f} MB) - attach it to the {tag} release")
    if paths:
        quoted = " ".join(f'"{p}"' for p in paths)
        print(f"  gh release upload {tag} {quoted}")
    else:
        print("  the release needs only the program zip")
    return paths


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.join(ROOT, "dist", "rust"))
    ap.add_argument("--tag", default=None, help="the release tag (default: APP_VERSION)")
    ap.add_argument("--new-only", action="store_true", help="stage only the pictures no release has yet")
    args = ap.parse_args()
    stage_and_report(args.out, args.tag or app_version(), args.new_only)


if __name__ == "__main__":
    main()
