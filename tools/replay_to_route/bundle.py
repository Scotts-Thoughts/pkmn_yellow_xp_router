"""Make the replay tools bundle that the Windows router downloads the first time it needs it.

The bundle holds everything "Route from Replay" needs besides the user's ROM:

    xpr-replay-worker.exe       reads a replay's memory (build.sh, stripped here)
    pab-host/pab-host.exe       Poke-A-Byte's mapper engine, self-contained (no .NET install needed)
    mappers/...                 the mappers xpr_replay::game chooses, with their scripts
    licenses/...                the licences of what is in here, and where its source is

Build the worker first (build.sh --skip-pab-host), then:

    py -3.14 tools/replay_to_route/bundle.py [--mappers DIR] [--out DIR]

This writes replay_tools_windows_<sha>.zip into rust/replay_tools/ (git-ignored) and records it
in rust/replay_tools.json, which the app is built with: it tells the app which bundle to fetch.
The name holds the content's hash, so the same input gives the same file, and an update that keeps
the file downloads nothing. Attach a new bundle to a release with the `gh release upload` command
printed at the end, then commit rust/replay_tools.json.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
RUST_DIR = os.path.join(ROOT, "rust")
LOCK = os.path.join(RUST_DIR, "replay_tools.json")
SUPERSHUCKIE = "A:/Programs/supershuckie"
POKEABYTE = "A:/Programs/PokeAByte"
STRIP = "C:/msys64/ucrt64/bin/strip.exe"

# the folders of the mappers in rust/crates/xpr-replay/src/game.rs (with their .js and patch tables)
MAPPER_DIRS = ["STANDARD/gen1", "STANDARD/gen2", "STANDARD/gen5", "ST/gen3", "ST/gen4"]

NOTICE = """\
Replay tools for Pokemon Solo Challenge Router
==============================================

"Route from Replay" runs these programs. They are separate programs with licences of their own;
the full texts are in this folder.

xpr-replay-worker.exe (GNU GPL 3)
  Plays Super Shuckie replays with Super Shuckie's emulator cores, which it links.
  Its own source: tools/replay_to_route/worker in
    https://github.com/Scotts-Thoughts/pkmn_yellow_xp_router
  Super Shuckie {supershuckie_rev}: https://github.com/Scotts-Thoughts/supershuckie
  The Game Boy Advance BIOS and boot ROMs in it are Super Shuckie's open replacements, not
  Nintendo's: see gba-bios.txt and SameBoy-boot-rom-LICENSE.txt.

pab-host\\pab-host.exe (GNU AGPL 3)
  Poke-A-Byte's mapper engine (PokeAByte.Domain) driven from stdin, with the .NET runtime
  (MIT) inside.
  Its own source: tools/replay_to_route/pab-host in
    https://github.com/Scotts-Thoughts/pkmn_yellow_xp_router
  Poke-A-Byte {pokeabyte_rev}: https://github.com/PokeAByte/PokeAByte

mappers\\
  From https://github.com/PokeAByte/mappers at {mappers_rev}, with local changes for solo-run
  recording (docs/rust_port/recording in the router's repository).
"""


def git_rev(path: str) -> str:
    return subprocess.run(["git", "-C", path, "rev-parse", "--short", "HEAD"], check=True, capture_output=True, text=True).stdout.strip()


def publish_pab_host(dest: str):
    # one exe with the runtime inside. It is not compressed on its own: the bundle's zip does that,
    # and a compressed single-file program spends every start decompressing itself.
    subprocess.run(
        [
            "dotnet", "publish", os.path.join(HERE, "pab-host", "PabHost.csproj"),
            "-c", "Release", "-r", "win-x64", "--self-contained", "true",
            "-p:PublishSingleFile=true", "-p:IncludeNativeLibrariesForSelfExtract=true", "-p:DebugType=none",
            "-o", dest, "-nologo", "-v", "quiet",
        ],
        check=True,
    )


def stage(staging: str, worker: str, mappers: str):
    pab = os.path.join(staging, "pab-host")
    publish_pab_host(pab)
    for name in os.listdir(pab):
        if name != "pab-host.exe":
            raise SystemExit(f"pab-host published more than its exe: {name}")
    subprocess.run([STRIP, "-o", os.path.join(staging, "xpr-replay-worker.exe"), worker], check=True)
    for rel in MAPPER_DIRS:
        shutil.copytree(os.path.join(mappers, rel), os.path.join(staging, "mappers", rel))

    licenses = os.path.join(staging, "licenses")
    os.makedirs(licenses)
    copies = {
        f"{SUPERSHUCKIE}/COPYING": "GPL-3.0.txt",
        f"{POKEABYTE}/LICENSE.txt": "AGPL-3.0.txt",
        f"{SUPERSHUCKIE}/licenses/notices/gba-bios.txt": "gba-bios.txt",
        f"{SUPERSHUCKIE}/licenses/texts/GPL-2.0.txt": "GPL-2.0.txt",
        f"{SUPERSHUCKIE}/licenses/notices/melonDS-third-party.txt": "melonDS-third-party.txt",
        f"{SUPERSHUCKIE}/licenses/notices/mGBA-third-party.txt": "mGBA-third-party.txt",
        f"{SUPERSHUCKIE}/bootrom/cgb/cgb_boot/LICENSE": "SameBoy-boot-rom-LICENSE.txt",
    }
    for src, name in copies.items():
        shutil.copy2(src, os.path.join(licenses, name))
    revision_file = os.path.join(HERE, ".vendor", "PokeAByte", "REVISION")
    with open(os.path.join(licenses, "README.txt"), "w", encoding="utf-8", newline="\r\n") as f:
        f.write(NOTICE.format(
            supershuckie_rev=git_rev(SUPERSHUCKIE),
            pokeabyte_rev=open(revision_file, encoding="ascii").read().strip()[:10],
            mappers_rev=git_rev(mappers),
        ))


def write_zip(staging: str, path: str):
    files = []
    for folder, _, names in os.walk(staging):
        for name in names:
            full = os.path.join(folder, name)
            files.append((os.path.relpath(full, staging).replace(os.sep, "/"), full))
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for rel, full in sorted(files):
            # a fixed date: the zip's hash names the bundle, so it must follow the content alone
            info = zipfile.ZipInfo(rel, date_time=(2020, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            with open(full, "rb") as src, z.open(info, "w", force_zip64=True) as dst:
                shutil.copyfileobj(src, dst, 1 << 20)


def sha256_of(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mappers", default=os.path.join(os.environ.get("APPDATA", ""), "PokeAByte", "mappers"))
    ap.add_argument("--worker", default=os.path.join(HERE, ".target", "release", "xpr-replay-worker.exe"))
    ap.add_argument("--out", default=os.path.join(RUST_DIR, "replay_tools"))
    args = ap.parse_args()

    os.makedirs(args.out, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="xpr_replay_bundle_") as tmp:
        staging = os.path.join(tmp, "bundle")
        os.makedirs(staging)
        stage(staging, args.worker, args.mappers)
        built = os.path.join(tmp, "bundle.zip")
        write_zip(staging, built)
        sha = sha256_of(built)
        asset = f"replay_tools_windows_{sha[:16]}.zip"
        final = os.path.join(args.out, asset)
        shutil.move(built, final)

    size = os.path.getsize(final)
    with open(LOCK, "w", encoding="utf-8", newline="\n") as f:
        json.dump({"asset": asset, "sha256": sha, "bytes": size}, f, indent=2)
        f.write("\n")
    print(f"{final} ({size / 1e6:.1f} MB)")
    print("to publish it (the next release needs it there):")
    print(f'  gh release upload <tag> "{final}" --repo Scotts-Thoughts/pkmn_yellow_xp_router')


if __name__ == "__main__":
    main()
