"""Download Pokemon HOME menu icons from Bulbagarden Archives.

URL pattern: https://archives.bulbagarden.net/media/upload/...
via the wiki File page for each icon: File:Menu_HOME_{NNNN}.png

Usage:
    python scripts/download_pkmn_icons.py
"""

import os
import sys
import time
import urllib.request
import urllib.error
import json
import re

DEST_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets", "pkmn_icons")
MAX_DEX = 493

# Bulbagarden archives direct URL pattern for HOME menu icons
# The actual file URL can be scraped from the wiki File page
WIKI_FILE_URL = "https://archives.bulbagarden.net/wiki/File:HOME{dex:04d}.png"
# Direct media URL pattern (most common path)
DIRECT_URL = "https://archives.bulbagarden.net/media/upload/{path}/HOME{dex:04d}.png"

# Known upload path prefixes (Bulbagarden uses hash-based subdirectories)
def get_hash_path(filename):
    """Compute Mediawiki hash path for a filename."""
    import hashlib
    md5 = hashlib.md5(filename.encode()).hexdigest()
    return f"{md5[0]}/{md5[:2]}"


def download_icon(dex_num, dest_dir):
    dest_path = os.path.join(dest_dir, f"{dex_num}.png")
    if os.path.exists(dest_path) and os.path.getsize(dest_path) > 100:
        return True  # Already downloaded

    filename = f"HOME{dex_num:04d}.png"
    hash_path = get_hash_path(filename)
    url = f"https://archives.bulbagarden.net/media/upload/{hash_path}/{filename}"

    try:
        req = urllib.request.Request(url, headers={"User-Agent": "PkmnXPRouter/1.0"})
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = resp.read()
            if len(data) < 100:
                print(f"  #{dex_num}: too small ({len(data)} bytes), skipping")
                return False
            with open(dest_path, "wb") as f:
                f.write(data)
            return True
    except urllib.error.HTTPError as e:
        if e.code == 404:
            print(f"  #{dex_num}: 404 not found at {url}")
        else:
            print(f"  #{dex_num}: HTTP {e.code}")
        return False
    except Exception as e:
        print(f"  #{dex_num}: {e}")
        return False


def main():
    os.makedirs(DEST_DIR, exist_ok=True)

    print(f"Downloading Pokemon HOME icons to: {DEST_DIR}")
    print(f"Range: 1-{MAX_DEX}")

    success = 0
    fail = 0
    for dex in range(1, MAX_DEX + 1):
        if download_icon(dex, DEST_DIR):
            success += 1
            if success % 50 == 0:
                print(f"  ...downloaded {success} so far")
        else:
            fail += 1
        # Be polite to the server
        time.sleep(0.15)

    print(f"\nDone: {success} downloaded, {fail} failed")


if __name__ == "__main__":
    main()
