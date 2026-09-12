"""Build the standalone Windows program from the Rust port.

Mirrors the repo's ``windows_build.py`` (the PyInstaller build of the Python
app) for the Rust app:

1. ``cargo build --release -p xpr-app`` — one exe with the Pokémon data and
   the image assets embedded (``xpr-data``'s ``embed-data`` feature and
   ``xpr-app/build.rs``) and the MSVC CRT linked statically (``.cargo/config.toml``).
2. Checks the exe imports nothing but Windows system DLLs.
3. Copies it to ``dist/rust/pkmn_xp_router.exe`` and zips it as
   ``dist/rust/windows_pkmn_xp_router_<version>.zip`` — one exe at the top
   level, the layout both the old Python updater and the Rust updater expect
   (rust_port_plan.md §4.4); the asset name must start with ``windows``.
4. ``--smoke``: runs the packaged exe from an empty temp directory (no repo
   files in reach, a scratch config dir) and requires a screenshot back.

    py -3.14 rust/windows_build.py [--no-build] [--smoke] [--out DIR]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import zipfile

RUST_DIR = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(RUST_DIR)
EXE_NAME = "pkmn_xp_router.exe"
BUILT_EXE = os.path.join(RUST_DIR, "target", "release", "xpr-app.exe")


def app_version() -> str:
    consts = open(os.path.join(RUST_DIR, "crates", "xpr-core", "src", "consts.rs"), encoding="utf-8").read()
    m = re.search(r'pub const APP_VERSION: &str = "([^"]+)"', consts)
    if not m:
        raise SystemExit("APP_VERSION not found in xpr-core consts.rs")
    return m.group(1)


def pe_imports(path: str) -> list[str]:
    """DLL names in the exe's import directory (no dependencies beyond the stdlib)."""
    data = open(path, "rb").read()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    nsec = struct.unpack_from("<H", data, pe + 6)[0]
    opt_off = pe + 24
    magic = struct.unpack_from("<H", data, opt_off)[0]
    dd_off = opt_off + (112 if magic == 0x20B else 96)
    imp_rva = struct.unpack_from("<I", data, dd_off + 8)[0]
    sec_off = opt_off + struct.unpack_from("<H", data, pe + 20)[0]
    secs = []
    for i in range(nsec):
        s = data[sec_off + 40 * i: sec_off + 40 * (i + 1)]
        vsize, va, rsize, raw = struct.unpack_from("<IIII", s, 8)
        secs.append((va, vsize, raw, rsize))

    def rva2off(rva):
        for va, vsize, raw, rsize in secs:
            if va <= rva < va + max(vsize, rsize):
                return raw + (rva - va)
        raise ValueError(f"rva {rva:#x} outside every section")

    off = rva2off(imp_rva)
    dlls = []
    while True:
        name_rva = struct.unpack_from("<I", data, off + 12)[0]
        if name_rva == 0:
            break
        n = rva2off(name_rva)
        dlls.append(data[n:data.index(b"\0", n)].decode("ascii"))
        off += 20
    return dlls


def check_self_contained(path: str):
    dlls = pe_imports(path)
    runtime = [d for d in dlls if d.upper().startswith(("VCRUNTIME", "MSVCP", "MSVCR")) or d.lower().startswith("api-ms-win-crt")]
    if runtime:
        raise SystemExit(f"{path} still needs the MSVC runtime: {runtime} (is rust/.cargo/config.toml in effect?)")
    print(f"imports only system DLLs: {', '.join(dlls)}")


def build():
    print("cargo build --release -p xpr-app")
    subprocess.run(["cargo", "build", "--release", "-p", "xpr-app"], cwd=RUST_DIR, check=True)


def package(out_dir: str, version: str) -> tuple[str, str]:
    os.makedirs(out_dir, exist_ok=True)
    exe = os.path.join(out_dir, EXE_NAME)
    shutil.copy2(BUILT_EXE, exe)
    zip_path = os.path.join(out_dir, f"windows_{EXE_NAME[:-4]}_{version}.zip")
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        z.write(exe, EXE_NAME)
    with zipfile.ZipFile(zip_path) as z:
        names = z.namelist()
    top_exes = [n for n in names if "/" not in n and n.lower().endswith(".exe")]
    if len(top_exes) != 1 or names != top_exes:
        raise SystemExit(f"unexpected zip layout: {names}")
    digest = hashlib.sha256(open(exe, "rb").read()).hexdigest()
    print(f"exe: {exe} ({os.path.getsize(exe) / 1e6:.1f} MB, sha256 {digest})")
    print(f"zip: {zip_path} ({os.path.getsize(zip_path) / 1e6:.1f} MB, contains {top_exes[0]})")
    return exe, zip_path


def smoke(exe: str) -> bool:
    """Run the packaged exe from an empty directory: nothing from the repo in reach."""
    tmp = tempfile.mkdtemp(prefix="xpr_standalone_")
    run_dir = os.path.join(tmp, "run")
    cfg_dir = os.path.join(tmp, "config")
    data_dir = os.path.join(tmp, "data")
    os.makedirs(run_dir)
    os.makedirs(cfg_dir)
    with open(os.path.join(cfg_dir, "config.json"), "w", encoding="utf-8") as f:
        json.dump({"user_data_location": data_dir, "auto_load_most_recent_route": False}, f)
    exe_copy = os.path.join(run_dir, EXE_NAME)
    shutil.copy2(exe, exe_copy)
    shot = os.path.join(tmp, "smoke.png")
    env = dict(os.environ)
    env.update({
        "XPR_GLOBAL_CONFIG_DIR": cfg_dir,
        "XPR_DISABLE_AUTO_UPDATE": "1",
        "XPR_SMOKE_SCREENSHOT": shot,
        # a route from the embedded data, rendered with the embedded icons
        "XPR_SMOKE_NEW_ROUTE": "Yellow|Charmander",
    })
    print(f"smoke: running {exe_copy} from {run_dir}")
    t0 = time.time()
    proc = subprocess.run([exe_copy], cwd=run_dir, env=env, timeout=120)
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
    return struct.unpack(">II", head[16:24])



def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--no-build", action="store_true", help="package the existing target/release/xpr-app.exe")
    ap.add_argument("--smoke", action="store_true", help="run the packaged exe from an empty directory afterwards")
    ap.add_argument("--out", default=os.path.join(ROOT, "dist", "rust"))
    args = ap.parse_args()
    version = app_version()
    print(f"version {version}")
    if not args.no_build:
        build()
    if not os.path.exists(BUILT_EXE):
        raise SystemExit(f"missing {BUILT_EXE}")
    check_self_contained(BUILT_EXE)
    exe, _zip = package(args.out, version)
    if args.smoke and not smoke(exe):
        sys.exit(1)


if __name__ == "__main__":
    main()
