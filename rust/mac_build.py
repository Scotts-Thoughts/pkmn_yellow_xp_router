"""Build the standalone macOS program from the Rust port.

Mirrors ``windows_build.py`` (the Windows exe build) for macOS
(rust_port_plan.md §4.5, "one universal .app"):

1. ``cargo build --release -p xpr-app`` for both Apple targets
   (``aarch64-apple-darwin``, ``x86_64-apple-darwin``; added with
   ``rustup target add`` if missing) and ``lipo -create`` them into one
   universal binary.
2. Checks the binary links nothing but macOS system frameworks/libraries
   (``otool -L``).
3. Assembles ``pkmn_xp_router.app``: ``Contents/Info.plist``, an
   ``.icns`` built from ``icons/app_icon.png`` (via ``sips``/
   ``iconutil``) and the universal binary as
   ``Contents/MacOS/pkmn_xp_router``. Not code-signed (rust_port_plan.md
   §4.5 leaves signing/notarization for later; unsigned apps need
   right-click → Open on first launch).
4. Copies it to ``dist/rust/pkmn_xp_router.app`` and zips it as
   ``dist/rust/macos_pkmn_xp_router_<version>.zip`` — one top-level
   ``.app``, the layout the updater expects (rust_port_plan.md §4.4/§4.5);
   the asset name must start with ``macos``.
5. ``--smoke``: runs the bundle's own binary from an empty temp directory
   (no repo files in reach, a scratch config dir) and requires a
   screenshot back.

    python3 rust/mac_build.py [--no-build] [--arch universal|arm64|x86_64] [--smoke] [--out DIR]
"""
from __future__ import annotations

import argparse
import json
import os
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile

RUST_DIR = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(RUST_DIR)
APP_NAME = "pkmn_xp_router"
BUNDLE_NAME = f"{APP_NAME}.app"
BUNDLE_ID = "com.scottsthoughts.pkmn-xp-router"
TARGET_TRIPLES = {"arm64": "aarch64-apple-darwin", "x86_64": "x86_64-apple-darwin"}
SYSTEM_LIB_PREFIXES = ("/System/Library/", "/usr/lib/")


def app_version() -> str:
    consts = open(os.path.join(RUST_DIR, "crates", "xpr-core", "src", "consts.rs"), encoding="utf-8").read()
    m = re.search(r'pub const APP_VERSION: &str = "([^"]+)"', consts)
    if not m:
        raise SystemExit("APP_VERSION not found in xpr-core consts.rs")
    return m.group(1)


def archs_for(arch: str) -> list[str]:
    return ["arm64", "x86_64"] if arch == "universal" else [arch]


def ensure_target(triple: str):
    installed = subprocess.run(
        ["rustup", "target", "list", "--installed"], capture_output=True, text=True, check=True
    ).stdout.split()
    if triple not in installed:
        print(f"rustup target add {triple}")
        subprocess.run(["rustup", "target", "add", triple], check=True)


def build(archs: list[str]):
    for arch in archs:
        triple = TARGET_TRIPLES[arch]
        ensure_target(triple)
        print(f"cargo build --release -p xpr-app --target {triple}")
        subprocess.run(["cargo", "build", "--release", "-p", "xpr-app", "--target", triple], cwd=RUST_DIR, check=True)


def built_exe(arch: str) -> str:
    return os.path.join(RUST_DIR, "target", TARGET_TRIPLES[arch], "release", "xpr-app")


def make_binary(archs: list[str], dst: str):
    srcs = [built_exe(a) for a in archs]
    for s in srcs:
        if not os.path.exists(s):
            raise SystemExit(f"missing {s}")
    if len(srcs) == 1:
        shutil.copy2(srcs[0], dst)
    else:
        print(f"lipo -create -output {dst} {' '.join(srcs)}")
        subprocess.run(["lipo", "-create", "-output", dst, *srcs], check=True)
    os.chmod(dst, 0o755)


def check_self_contained(path: str):
    out = subprocess.run(["otool", "-L", path], capture_output=True, text=True, check=True).stdout
    # first line is the binary's own path; skip it
    deps = [line.split()[0] for line in out.splitlines()[1:] if line.strip()]
    foreign = [d for d in deps if not d.startswith(SYSTEM_LIB_PREFIXES)]
    if foreign:
        raise SystemExit(f"{path} links outside system frameworks/libs: {foreign}")
    print(f"links only system frameworks/libs: {', '.join(deps)}")


def make_icns(scratch_dir: str) -> str:
    """Build app_icon.icns from icons/app_icon.png (24x24 pixel art) via sips/iconutil."""
    src = os.path.join(ROOT, "icons", "app_icon.png")
    iconset = os.path.join(scratch_dir, "app_icon.iconset")
    if os.path.exists(iconset):
        shutil.rmtree(iconset)
    os.makedirs(iconset)
    # (output size, iconset filename) pairs; iconset requires both the 1x
    # and 2x names even where the pixel size is shared with another entry.
    entries = [
        (16, "icon_16x16.png"),
        (32, "icon_16x16@2x.png"),
        (32, "icon_32x32.png"),
        (64, "icon_32x32@2x.png"),
        (128, "icon_128x128.png"),
        (256, "icon_128x128@2x.png"),
        (256, "icon_256x256.png"),
        (512, "icon_256x256@2x.png"),
        (512, "icon_512x512.png"),
        (1024, "icon_512x512@2x.png"),
    ]
    rendered: dict[int, str] = {}
    for size, name in entries:
        dst = os.path.join(iconset, name)
        if size not in rendered:
            subprocess.run(["sips", "-z", str(size), str(size), src, "--out", dst], check=True, capture_output=True)
            rendered[size] = dst
        else:
            shutil.copy2(rendered[size], dst)
    icns = os.path.join(scratch_dir, "app_icon.icns")
    subprocess.run(["iconutil", "-c", "icns", iconset, "-o", icns], check=True)
    return icns


def make_info_plist(version: str) -> bytes:
    short_version = version.lstrip("v")
    plist = {
        "CFBundleName": "Pkmn XP Router",
        "CFBundleDisplayName": "Pkmn XP Router",
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleExecutable": APP_NAME,
        "CFBundleIconFile": "app_icon.icns",
        "CFBundlePackageType": "APPL",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleShortVersionString": short_version,
        "CFBundleVersion": short_version,
        "NSHighResolutionCapable": True,
        "LSApplicationCategoryType": "public.app-category.utilities",
        "NSHumanReadableCopyright": "",
    }
    return plistlib.dumps(plist)


def make_bundle(archs: list[str], scratch_dir: str, version: str) -> str:
    bundle = os.path.join(scratch_dir, BUNDLE_NAME)
    if os.path.exists(bundle):
        shutil.rmtree(bundle)
    macos_dir = os.path.join(bundle, "Contents", "MacOS")
    resources_dir = os.path.join(bundle, "Contents", "Resources")
    os.makedirs(macos_dir)
    os.makedirs(resources_dir)

    make_binary(archs, os.path.join(macos_dir, APP_NAME))
    check_self_contained(os.path.join(macos_dir, APP_NAME))

    icns = make_icns(scratch_dir)
    shutil.copy2(icns, os.path.join(resources_dir, "app_icon.icns"))

    with open(os.path.join(bundle, "Contents", "Info.plist"), "wb") as f:
        f.write(make_info_plist(version))

    return bundle


def package(bundle: str, out_dir: str, version: str) -> tuple[str, str]:
    os.makedirs(out_dir, exist_ok=True)
    dst_bundle = os.path.join(out_dir, BUNDLE_NAME)
    if os.path.exists(dst_bundle):
        shutil.rmtree(dst_bundle)
    shutil.copytree(bundle, dst_bundle, symlinks=True)

    zip_path = os.path.join(out_dir, f"macos_{APP_NAME}_{version}.zip")
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for dirpath, _dirnames, filenames in os.walk(dst_bundle):
            for name in filenames:
                path = os.path.join(dirpath, name)
                arcname = os.path.join(BUNDLE_NAME, os.path.relpath(path, dst_bundle))
                z.write(path, arcname)  # preserves the unix mode bits (exe permission) on this platform
    with zipfile.ZipFile(zip_path) as z:
        names = z.namelist()
    top_dirs = {n.split("/", 1)[0] for n in names}
    if top_dirs != {BUNDLE_NAME}:
        raise SystemExit(f"unexpected zip layout: {sorted(top_dirs)}")
    size = sum(os.path.getsize(os.path.join(dp, f)) for dp, _dn, fn in os.walk(dst_bundle) for f in fn)
    print(f"app: {dst_bundle} ({size / 1e6:.1f} MB)")
    print(f"zip: {zip_path} ({os.path.getsize(zip_path) / 1e6:.1f} MB)")
    return dst_bundle, zip_path


def smoke(bundle: str) -> bool:
    """Run the bundled binary from an empty directory: nothing from the repo in reach."""
    tmp = tempfile.mkdtemp(prefix="xpr_standalone_")
    run_dir = os.path.join(tmp, "run")
    cfg_dir = os.path.join(tmp, "config")
    data_dir = os.path.join(tmp, "data")
    os.makedirs(run_dir)
    os.makedirs(cfg_dir)
    with open(os.path.join(cfg_dir, "config.json"), "w", encoding="utf-8") as f:
        json.dump({"user_data_location": data_dir, "auto_load_most_recent_route": False}, f)
    run_bundle = os.path.join(run_dir, BUNDLE_NAME)
    shutil.copytree(bundle, run_bundle, symlinks=True)
    exe = os.path.join(run_bundle, "Contents", "MacOS", APP_NAME)
    shot = os.path.join(tmp, "smoke.png")
    env = dict(os.environ)
    env.update({
        "XPR_GLOBAL_CONFIG_DIR": cfg_dir,
        "XPR_DISABLE_AUTO_UPDATE": "1",
        "XPR_SMOKE_SCREENSHOT": shot,
        # a route from the embedded data, rendered with the embedded icons
        "XPR_SMOKE_NEW_ROUTE": "Yellow|Charmander",
    })
    print(f"smoke: running {exe} from {run_dir}")
    t0 = time.time()
    proc = subprocess.run([exe], cwd=run_dir, env=env, timeout=120)
    log_path = os.path.join(cfg_dir, "pkmn_router_logs.log")
    log = open(log_path, encoding="utf-8", errors="replace").read() if os.path.exists(log_path) else ""
    errors = [line for line in log.splitlines() if " ERROR " in line and "new version info" not in line]
    size = png_size(shot) if os.path.exists(shot) else None
    ok = proc.returncode == 0 and size is not None and min(size) >= 400 and "smoke: creating a new Yellow route" in log and not errors
    print(f"smoke: exit {proc.returncode} after {time.time() - t0:.1f}s, screenshot {size or 'MISSING'}, errors: {errors or 'none'}")
    if ok:
        print(f"smoke: ok ({shot})")
    else:
        print(f"smoke: FAILED, see {log_path}")
    return ok


def png_size(path: str) -> tuple[int, int] | None:
    head = open(path, "rb").read(24)
    if head[:8] != bytes.fromhex("89504e470d0a1a0a") or head[12:16] != b"IHDR":
        return None
    import struct
    return struct.unpack(">II", head[16:24])


def main():
    if sys.platform != "darwin":
        raise SystemExit("mac_build.py must run on macOS (uses lipo/sips/iconutil)")
    ap = argparse.ArgumentParser()
    ap.add_argument("--no-build", action="store_true", help="package the existing target/<triple>/release/xpr-app build(s)")
    ap.add_argument("--arch", choices=["universal", "arm64", "x86_64"], default="universal")
    ap.add_argument("--smoke", action="store_true", help="run the bundled binary from an empty directory afterwards")
    ap.add_argument("--out", default=os.path.join(ROOT, "dist", "rust"))
    args = ap.parse_args()
    archs = archs_for(args.arch)
    version = app_version()
    print(f"version {version}, arch(es): {', '.join(archs)}")
    if not args.no_build:
        build(archs)
    with tempfile.TemporaryDirectory(prefix="xpr_mac_bundle_") as scratch_dir:
        bundle = make_bundle(archs, scratch_dir, version)
        app, _zip = package(bundle, args.out, version)
    if args.smoke and not smoke(app):
        sys.exit(1)


if __name__ == "__main__":
    main()
