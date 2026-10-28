"""Stamp the release version into the Rust app before a CI build.

The release workflow (``.github/workflows/release.yml``) runs this with the
version taken from the commit message (``v6.2a``), so the built program
reports exactly the version of the GitHub release it ships in, and the
updater never offers a release as "newer" than itself. Sets
``APP_VERSION`` and ``APP_RELEASE_DATE`` (today, UTC) in
``rust/crates/xpr-core/src/consts.rs``; the change is not committed.

    python .github/scripts/stamp_version.py v6.2a
"""
from __future__ import annotations

import datetime
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CONSTS = os.path.join(ROOT, "rust", "crates", "xpr-core", "src", "consts.rs")


def main():
    if len(sys.argv) != 2 or not re.fullmatch(r"v\d+\.\d+[a-z]", sys.argv[1]):
        raise SystemExit("usage: stamp_version.py vX.Yz")
    version = sys.argv[1]
    date = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%b-%d")
    with open(CONSTS, encoding="utf-8", newline="") as f:
        src = f.read()
    old = re.search(r'pub const APP_VERSION: &str = "([^"]+)";', src)
    if not old:
        raise SystemExit("APP_VERSION not found in consts.rs")
    if old.group(1) != version:
        print(f"note: consts.rs says {old.group(1)}; building as {version}")
    src, n1 = re.subn(r'(pub const APP_VERSION: &str = ")[^"]+(";)', rf"\g<1>{version}\g<2>", src)
    src, n2 = re.subn(r'(pub const APP_RELEASE_DATE: &str = ")[^"]+(";)', rf"\g<1>{date}\g<2>", src)
    if n1 != 1 or n2 != 1:
        raise SystemExit("could not stamp consts.rs")
    with open(CONSTS, "w", encoding="utf-8", newline="") as f:
        f.write(src)
    print(f"stamped {version} ({date})")


if __name__ == "__main__":
    main()
