"""Scrape Gen V item data and write raw_pkmn_data/gen_five/items.json.

Sources:
  - Bulbapedia "List of items by index number in Generation V" (via MediaWiki API)
      -> ROM index numbers and canonical item names.
  - PokeAPI (https://pokeapi.co/api/v2/...)
      -> Pocket category, and (for TMs/HMs) the BW2 move used to format
         names like "TM01 Hone Claws".
  - raw_pkmn_data/gen_four/items.json (already in repo)
      -> Historical buy prices for items that carried over from Gen 4. PokeAPI's
         `cost` field reports modern (Gen 7+) prices, which differ from BW/B2W2
         for many items, so we prefer the Gen 4 number when one exists.

Output schema matches raw_pkmn_data/gen_four/items.json:
    {"items": [{"rom_id", "purchase_price", "key_item", "pocket", "name"}, ...]}

Usage:
    python scripts/scrape_gen5_items.py
"""

import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_PATH = os.path.join(REPO_ROOT, "raw_pkmn_data", "gen_five", "items.json")
GEN4_ITEMS_PATH = os.path.join(REPO_ROOT, "raw_pkmn_data", "gen_four", "items.json")

USER_AGENT = "PkmnXPRouter/1.0 (gen5 items scraper)"
BULBA_API = (
    "https://bulbapedia.bulbagarden.net/w/api.php"
    "?action=parse&format=json&prop=wikitext"
    "&page=List_of_items_by_index_number_in_Generation_V"
)
POKEAPI = "https://pokeapi.co/api/v2"

# Polite delay between PokeAPI requests (seconds).
REQUEST_DELAY = 0.05

# PokeAPI item-pocket name -> our gen_four-style pocket label.
POCKET_MAP = {
    "misc": "Items",
    "medicine": "Medicine",
    "pokeballs": "Balls",
    "machines": "TMs and HMs",
    "berries": "Berries",
    "mail": "Mail",
    "battle": "Battle Items",
    "key": "Key Items",
}

# Items where the Bulbapedia in-game display name does not slugify cleanly
# to the PokeAPI slug. Bulbapedia display name -> PokeAPI slug.
NAME_TO_SLUG_OVERRIDES = {
    "Parlyz Heal": "paralyze-heal",
    "EnergyPowder": "energy-powder",
    "X Defend": "x-defense",
    "X Defend 2": "x-defense-2",
    "X Defend 3": "x-defense-3",
    "X Defend 6": "x-defense-6",
    "X Special": "x-sp-atk",
    "X Special 2": "x-sp-atk-2",
    "X Special 3": "x-sp-atk-3",
    "X Special 6": "x-sp-atk-6",
    "Guard Spec.": "guard-spec",
    "RageCandyBar": "rage-candy-bar",
    "SilverPowder": "silver-powder",
    "BlackGlasses": "black-glasses",
    "BrightPowder": "bright-powder",
    "DeepSeaTooth": "deep-sea-tooth",
    "DeepSeaScale": "deep-sea-scale",
    "TinyMushroom": "tiny-mushroom",
    "BalmMushroom": "balm-mushroom",
    "Dowsing MCHN": "dowsing-machine",
    "Colress MCHN": "colress-machine",
    "Pretty Wing": "pretty-wing",
    "Thunderstone": "thunder-stone",
    "NeverMeltIce": "never-melt-ice",
    "TwistedSpoon": "twisted-spoon",
    "SecretPotion": "secret-potion",
    "SlowpokeTail": "slowpoke-tail",
    "SquirtBottle": "squirt-bottle",
    "Blu Apricorn": "blue-apricorn",
    "Ylw Apricorn": "yellow-apricorn",
    "Grn Apricorn": "green-apricorn",
    "Pnk Apricorn": "pink-apricorn",
    "Wht Apricorn": "white-apricorn",
    "Blk Apricorn": "black-apricorn",
    "BridgeMail S": "bridge-mail-s",
    "BridgeMail D": "bridge-mail-d",
    "BridgeMail T": "bridge-mail-t",
    "BridgeMail V": "bridge-mail-v",
    "BridgeMail M": "bridge-mail-m",
}

# Items the Gen V index lists as "unknown" (unused ROM slots). Skipped entirely.
UNKNOWN_NAME = "unknown"


def http_get(url, retries=3):
    last_err = None
    for attempt in range(retries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
            with urllib.request.urlopen(req, timeout=30) as resp:
                return resp.read()
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return None
            last_err = e
        except Exception as e:
            last_err = e
        time.sleep(0.5 * (attempt + 1))
    raise RuntimeError(f"GET failed after retries: {url}: {last_err}")


def get_json(url):
    raw = http_get(url)
    if raw is None:
        return None
    return json.loads(raw)


# ----- Bulbapedia parsing ---------------------------------------------------

HEXLIST_RE = re.compile(r"^\{\{hexlist\|(.*?)\}\}\s*(?:<!--.*?-->)?\s*$")


def parse_bulba_index_table():
    """Return list of (rom_id, display_name) tuples from the Gen V index page."""
    data = get_json(BULBA_API)
    if data is None or "parse" not in data:
        raise RuntimeError("Bulbapedia API returned no data")
    wikitext = data["parse"]["wikitext"]["*"]

    items = []
    seen_ids = set()
    for line in wikitext.splitlines():
        m = HEXLIST_RE.match(line)
        if not m:
            continue
        body = m.group(1)
        # Replace template-pipe escapes with a sentinel that survives the split.
        body = body.replace("{{!}}", "\x00")
        parts = body.split("|")
        if len(parts) < 3:
            continue

        # Strip "k=v" template kwargs (e.g. "8=no", "6=Display Name").
        positional = [p for p in parts if not re.match(r"^\d+=", p)]
        if len(positional) < 3:
            continue

        name_field = positional[0]
        try:
            rom_id = int(positional[2])
        except ValueError:
            continue

        # If the name field is "Page\x00Display", prefer the display text;
        # otherwise it's just the page/display name.
        if "\x00" in name_field:
            _page, display = name_field.split("\x00", 1)
            display = display.strip()
        else:
            display = name_field.strip()
        # Strip wiki-link brackets if present.
        display = display.strip("[]")
        # Match gen_four/items.json convention: ASCII-only display names.
        display = display.replace("\u00e9", "e").replace("\u00c9", "E")

        if rom_id == 0:
            continue  # "None" sentinel
        if display.lower() == UNKNOWN_NAME:
            continue  # unused ROM slots
        if rom_id in seen_ids:
            continue  # duplicate (Xtransceiver, etc.)
        seen_ids.add(rom_id)
        items.append((rom_id, display))

    items.sort(key=lambda x: x[0])
    return items


# ----- PokeAPI lookups ------------------------------------------------------

def slugify(name):
    if name in NAME_TO_SLUG_OVERRIDES:
        return NAME_TO_SLUG_OVERRIDES[name]
    s = name.lower()
    s = s.replace("\u00e9", "e")  # é
    s = s.replace("'", "")
    s = s.replace(".", "")
    s = re.sub(r"[^a-z0-9]+", "-", s)
    s = s.strip("-")
    return s


def fetch_category_pocket_map():
    """Map PokeAPI item-category name -> item-pocket name (e.g. 'standard-balls' -> 'pokeballs')."""
    print("Fetching PokeAPI item pockets...")
    pocket_list = get_json(f"{POKEAPI}/item-pocket?limit=100")
    cat_to_pocket = {}
    for entry in pocket_list["results"]:
        pocket_data = get_json(entry["url"])
        pocket_name = pocket_data["name"]
        for cat in pocket_data["categories"]:
            cat_to_pocket[cat["name"]] = pocket_name
        time.sleep(REQUEST_DELAY)
    return cat_to_pocket


def fetch_item(slug):
    return get_json(f"{POKEAPI}/item/{slug}/")


# ----- TM/HM move resolution ------------------------------------------------

# Gen V version groups, in priority order (B2W2 takes precedence so post-BW
# TM additions resolve to their B2W2 move).
GEN5_VERSION_GROUPS = ("black-2-white-2", "black-white")


def get_machine_move(item_data):
    """Given a PokeAPI item dict for a TM/HM, return the move name used in Gen V."""
    machines = item_data.get("machines", [])
    chosen_url = None
    for vg in GEN5_VERSION_GROUPS:
        for m in machines:
            if m["version_group"]["name"] == vg:
                chosen_url = m["machine"]["url"]
                break
        if chosen_url:
            break
    if not chosen_url:
        return None
    machine = get_json(chosen_url)
    time.sleep(REQUEST_DELAY)
    move_slug = machine["move"]["name"]
    # Convert "hone-claws" -> "Hone Claws".
    return " ".join(w.capitalize() for w in move_slug.split("-"))


# ----- Main -----------------------------------------------------------------

def load_gen4_price_lookup():
    """Map of historical buy price by item name from gen_four/items.json.

    Both the raw name and (for TM/HM "TM01 Focus Punch") the bare prefix are
    keyed so the Gen 5 entry can match either form.
    """
    with open(GEN4_ITEMS_PATH, "r", encoding="utf-8") as f:
        gen4 = json.load(f)
    out = {}
    for item in gen4["items"]:
        name = item["name"]
        out[name] = item["purchase_price"]
        # For TM/HM, also key by the prefix in case the Gen 5 move differs.
        m = re.match(r"^((?:TM|HM)\d+)\s", name)
        if m:
            out.setdefault(m.group(1), item["purchase_price"])
    return out


def build_items():
    print("Loading Gen 4 price lookup...")
    g4_prices = load_gen4_price_lookup()

    print("Fetching Bulbapedia Gen V item index...")
    bulba_items = parse_bulba_index_table()
    print(f"  Parsed {len(bulba_items)} item rows")

    cat_to_pocket = fetch_category_pocket_map()

    out = []
    missing = []
    for rom_id, display_name in bulba_items:
        slug = slugify(display_name)
        item_data = fetch_item(slug)
        time.sleep(REQUEST_DELAY)

        if item_data is None:
            # Pokestar/B2W2-exclusive props PokeAPI doesn't expose. Emit a stub
            # in the "Items" pocket so the file stays a complete index. Treat
            # plot-only items as key items.
            missing.append((rom_id, display_name, slug))
            out.append({
                "rom_id": rom_id,
                "purchase_price": g4_prices.get(display_name, 0),
                "key_item": True,
                "pocket": "Key Items",
                "name": display_name,
            })
            continue

        cat = item_data["category"]["name"]
        pocket_api = cat_to_pocket.get(cat, "misc")
        pocket = POCKET_MAP.get(pocket_api, "Items")
        is_key = pocket == "Key Items"

        name = display_name
        if pocket == "TMs and HMs" and re.match(r"^(TM|HM)\d+$", display_name):
            move_name = get_machine_move(item_data)
            if move_name:
                name = f"{display_name} {move_name}"

        # Prefer historical Gen 4 price (also valid for BW/B2W2 in nearly all
        # cases); fall back to PokeAPI's modern cost when no Gen 4 entry exists.
        bare_tm = re.match(r"^(TM|HM)\d+", name)
        price = g4_prices.get(name)
        if price is None and bare_tm:
            price = g4_prices.get(bare_tm.group(0))
        if price is None:
            price = item_data.get("cost", 0) or 0

        out.append({
            "rom_id": rom_id,
            "purchase_price": price,
            "key_item": is_key,
            "pocket": pocket,
            "name": name,
        })

        if len(out) % 50 == 0:
            print(f"  ...processed {len(out)}/{len(bulba_items)}")

    if missing:
        print(f"\n{len(missing)} items not found in PokeAPI (emitted as Key Items stubs):")
        for rom_id, name, slug in missing:
            print(f"  #{rom_id:3d}  {name!r}  (tried slug {slug!r})")

    return out


def main():
    items = build_items()
    os.makedirs(os.path.dirname(OUT_PATH), exist_ok=True)
    with open(OUT_PATH, "w", encoding="utf-8") as f:
        json.dump({"items": items}, f, indent=4)
    print(f"\nWrote {len(items)} items -> {OUT_PATH}")


if __name__ == "__main__":
    sys.exit(main())
