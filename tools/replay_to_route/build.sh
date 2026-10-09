#!/usr/bin/env bash
# Builds the two helper programs replay_to_route runs into tools/replay_to_route/out:
#
#   out/pab-host/pab-host.exe     Poke-A-Byte's mapper engine (PokeAByte.Domain), driven from stdin
#   out/xpr-replay-worker.exe     reads a Super Shuckie replay's memory (links Super Shuckie's cores)
#
# usage: tools/replay_to_route/build.sh [--pokeabyte <checkout>] [--skip-worker] [--skip-pab-host]
#
# Needs: the .NET 10 SDK; MSYS2's UCRT64 Rust toolchain (/c/msys64/ucrt64/bin) for the worker,
# whose emulator cores are MinGW archives; a Super Shuckie checkout at A:/Programs/supershuckie that
# has been built once (its build/ folder holds the cores, see worker/Cargo.toml).
#
# Poke-A-Byte's sources are AGPL and stay out of this repository: the committed tree of the
# checkout is snapshotted into .vendor/PokeAByte (git-ignored) and built from there, so
# uncommitted work in the checkout never reaches the build.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
pokeabyte="${POKEABYTE:-A:/Programs/PokeAByte}"
build_pab=1
build_worker=1
while [ $# -gt 0 ]; do
    case "$1" in
        --pokeabyte) pokeabyte="$2"; shift 2 ;;
        --skip-worker) build_worker=0; shift ;;
        --skip-pab-host) build_pab=0; shift ;;
        *) echo "unknown argument $1" >&2; exit 2 ;;
    esac
done

mkdir -p "$here/out"

if [ "$build_pab" = 1 ]; then
    echo "== pab-host (Poke-A-Byte $(git -C "$pokeabyte" rev-parse --short HEAD))"
    rm -rf "$here/.vendor/PokeAByte"
    mkdir -p "$here/.vendor/PokeAByte"
    git -C "$pokeabyte" archive HEAD src/PokeAByte.Domain src/Directory.Build.props | tar -x -C "$here/.vendor/PokeAByte"
    git -C "$pokeabyte" rev-parse HEAD > "$here/.vendor/PokeAByte/REVISION"
    dotnet build "$here/pab-host/PabHost.csproj" -c Release -o "$here/out/pab-host" -nologo -v quiet
fi

if [ "$build_worker" = 1 ]; then
    echo "== xpr-replay-worker"
    (
        export PATH="/c/msys64/ucrt64/bin:$PATH"
        export CARGO_TARGET_DIR="$here/.target"
        cargo build --release --manifest-path "$here/worker/Cargo.toml"
    )
    exe=xpr-replay-worker
    [ -f "$here/.target/release/$exe.exe" ] && exe="$exe.exe"
    cp "$here/.target/release/$exe" "$here/out/"
fi

echo "built into $here/out"
