import difflib
import re
import json
import os
import logging

logger = logging.getLogger(__name__)

BACKPORT_FORMAT_ERROR = (
    "ASM file must be in Scott's Thoughts backport format in order to automatically import."
)

# Crystal move tutor moves (not TMs/HMs, but taught by NPC tutor)
CRYSTAL_TUTOR_MOVES = {"flamethrower", "thunderbolt", "icebeam"}


def _normalize_name(name):
    """Remove non-alphanumeric chars and lowercase for fuzzy matching."""
    return re.sub(r'[^a-z0-9]', '', name.lower())


def _build_move_lookup(moves_json_path):
    """Build a lookup from normalized name to actual JSON name."""
    with open(moves_json_path, 'r') as f:
        data = json.load(f)

    lookup = {}
    for move in data.get('moves', []):
        name = move['name']
        normalized = _normalize_name(name)
        lookup[normalized] = name
    return lookup


def _resolve_move_name(asm_name, move_lookup, new_move_names=None):
    """Convert an ASM move constant name to the proper JSON name."""
    asm_name = asm_name.strip()

    # Check if it's a NEW_MOVE_X reference
    if new_move_names and asm_name in new_move_names:
        return new_move_names[asm_name]

    # Normalize and look up
    normalized = _normalize_name(asm_name)
    if normalized in move_lookup:
        return move_lookup[normalized]

    # Try removing _M suffix (e.g., PSYCHIC_M -> PSYCHIC)
    if asm_name.endswith('_M'):
        normalized_no_m = _normalize_name(asm_name[:-2])
        if normalized_no_m in move_lookup:
            return move_lookup[normalized_no_m]

    # Fuzzy match for typos (e.g., PIN_MISSLE -> Pin Missile)
    close = difflib.get_close_matches(normalized, move_lookup.keys(), n=1, cutoff=0.8)
    if close:
        logger.info(f"Fuzzy matched ASM move '{asm_name}' to '{move_lookup[close[0]]}'")
        return move_lookup[close[0]]

    # Fallback: title case with underscores as spaces
    return asm_name.replace('_', ' ').title()


def _extract_macro(content, macro_name):
    """Extract the lines between MACRO macro_name and the next ENDM."""
    pattern = rf'MACRO\s+{re.escape(macro_name)}\b[^\n]*\n(.*?)^\s*ENDM\b'
    match = re.search(pattern, content, re.DOTALL | re.MULTILINE)
    if match:
        return match.group(1)
    return None


def _strip_comment(line):
    """Remove ASM comment (everything after ;)."""
    idx = line.find(';')
    if idx >= 0:
        return line[:idx].strip()
    return line.strip()


def _get_comment(line):
    """Get the comment part of a line."""
    idx = line.find(';')
    if idx >= 0:
        return line[idx + 1:].strip()
    return ""


def parse_backports_asm(content):
    """Parse a backports.asm file and return structured data.

    Raises ValueError if the file is not in the expected format.
    """
    result = {}

    # Validate format
    for macro in ['backport_name', 'backport_base_stats', 'backport_level_up_learnset']:
        if not re.search(rf'MACRO\s+{macro}\b', content):
            raise ValueError(BACKPORT_FORMAT_ERROR)

    # Parse name from backport_name macro
    name_section = _extract_macro(content, 'backport_name')
    if name_section is None:
        raise ValueError(BACKPORT_FORMAT_ERROR)

    for line in name_section.split('\n'):
        match = re.search(r'db\s+"([^"]+)"', line)
        if match and not line.strip().startswith(';'):
            raw_name = match.group(1).replace('@', '').strip()
            result['name'] = raw_name.title()
            break

    if 'name' not in result:
        raise ValueError(BACKPORT_FORMAT_ERROR)

    # Parse base stats from def lines
    stat_pattern = r'def\s+backport_(\w+)\s+EQU\s+(\d+)'
    stats = {}
    for match in re.finditer(stat_pattern, content):
        stat_name = match.group(1).lower()
        stat_value = int(match.group(2))
        stats[stat_name] = stat_value

    result['base_hp'] = stats.get('hit', 0)
    result['base_atk'] = stats.get('atk', 0)
    result['base_def'] = stats.get('def', 0)
    result['base_spc_atk'] = stats.get('spa', 0)
    result['base_spc_def'] = stats.get('spd', 0)
    result['base_spd'] = stats.get('spe', 0)

    # Parse base_stats macro
    base_stats_section = _extract_macro(content, 'backport_base_stats')
    if base_stats_section is None:
        raise ValueError(BACKPORT_FORMAT_ERROR)

    # Collect db, dn, and tmhm lines from the macro
    db_lines = []
    dn_lines = []
    tmhm_line = None

    for line in base_stats_section.split('\n'):
        stripped = line.strip()
        if not stripped or stripped.startswith(';'):
            continue

        clean = _strip_comment(stripped)
        if not clean:
            continue

        clean_lower = clean.lower()
        if clean_lower.startswith('tmhm'):
            tmhm_line = clean
        elif clean_lower.startswith('db ') or clean_lower.startswith('db\t'):
            db_lines.append(clean)
        elif clean_lower.startswith('dn ') or clean_lower.startswith('dn\t'):
            dn_lines.append(clean)
        # Skip INCBIN, dw, etc.

    # db line order after the first (stat refs):
    # [0] stat refs (skip), [1] types, [2] catch_rate, [3] base_exp,
    # [4] items, [5] gender_ratio, [6] unknown, [7] egg_cycles,
    # [8] unknown, then growth_rate somewhere after

    if len(db_lines) >= 2:
        types_str = re.sub(r'^db\s+', '', db_lines[1], flags=re.IGNORECASE).strip()
        types = [t.strip().title() for t in types_str.split(',')]
        result['type_1'] = types[0] if types else "Normal"
        result['type_2'] = types[1] if len(types) > 1 else result['type_1']

    if len(db_lines) >= 3:
        result['catch_rate'] = int(re.sub(r'^db\s+', '', db_lines[2], flags=re.IGNORECASE).strip())

    if len(db_lines) >= 4:
        result['base_xp'] = int(re.sub(r'^db\s+', '', db_lines[3], flags=re.IGNORECASE).strip())

    if len(db_lines) >= 5:
        items_str = re.sub(r'^db\s+', '', db_lines[4], flags=re.IGNORECASE).strip()
        items = [i.strip() for i in items_str.split(',')]
        result['common_item'] = items[0] if items else "NO_ITEM"
        result['rare_item'] = items[1] if len(items) > 1 else "NO_ITEM"

    if len(db_lines) >= 6:
        result['gender_ratio'] = re.sub(r'^db\s+', '', db_lines[5], flags=re.IGNORECASE).strip()

    # db_lines[6] = unknown, skip
    if len(db_lines) >= 8:
        result['egg_cycles'] = int(re.sub(r'^db\s+', '', db_lines[7], flags=re.IGNORECASE).strip())

    # Growth rate: find a db line containing GROWTH_
    for clean in db_lines:
        val = re.sub(r'^db\s+', '', clean, flags=re.IGNORECASE).strip()
        if val.upper().startswith('GROWTH_'):
            result['growth_rate'] = val.lower()
            break

    # Egg groups from dn line
    if dn_lines:
        eggs_str = re.sub(r'^dn\s+', '', dn_lines[0], flags=re.IGNORECASE).strip()
        eggs = [e.strip() for e in eggs_str.split(',')]
        result['egg_group_1'] = eggs[0] if eggs else "EGG_NONE"
        result['egg_group_2'] = eggs[1] if len(eggs) > 1 else "EGG_NONE"

    # TM/HM learnset
    if tmhm_line:
        tmhm_str = re.sub(r'^tmhm\s+', '', tmhm_line, flags=re.IGNORECASE).strip()
        result['tmhm_asm_names'] = [m.strip() for m in tmhm_str.split(',') if m.strip()]
    else:
        result['tmhm_asm_names'] = []

    # Parse level up learnset
    learnset_section = _extract_macro(content, 'backport_level_up_learnset')
    if learnset_section is None:
        raise ValueError(BACKPORT_FORMAT_ERROR)

    levelup = []
    for line in learnset_section.split('\n'):
        stripped = line.strip()
        if not stripped or stripped.startswith(';'):
            continue

        clean = _strip_comment(stripped)
        if not clean.lower().startswith('db'):
            continue

        parts_str = re.sub(r'^db\s+', '', clean, flags=re.IGNORECASE).strip()
        parts = parts_str.split(',')
        if len(parts) >= 2:
            level = int(parts[0].strip())
            move_asm = parts[1].strip()
            levelup.append((level, move_asm))

    result['levelup_asm'] = levelup
    return result


def parse_backport_moves_asm(content):
    """Parse a backport_moves.asm file and return a list of move dicts.

    Raises ValueError if the file is not in the expected format.
    """
    if not re.search(r'MACRO\s+backport_moves\b', content):
        raise ValueError(BACKPORT_FORMAT_ERROR)

    moves = []
    move_names = []

    # Parse move definitions
    moves_section = _extract_macro(content, 'backport_moves')
    if moves_section:
        for line in moves_section.split('\n'):
            stripped = line.strip()
            if not stripped or stripped.startswith(';'):
                continue

            match = re.match(
                r'move\s+(\w+)\s*,\s*(\w+)\s*,\s*(\d+)\s*,\s*(\w+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)',
                stripped
            )
            if match:
                comment = _get_comment(stripped)
                moves.append({
                    'asm_name': match.group(1),
                    'effect': match.group(2),
                    'base_power': int(match.group(3)),
                    'type': match.group(4).title(),
                    'accuracy': int(match.group(5)),
                    'pp': int(match.group(6)),
                    'effect_chance': int(match.group(7)),
                    'comment_name': comment if comment else None,
                })

    # Parse move display names
    names_section = _extract_macro(content, 'backport_move_names')
    if names_section:
        for line in names_section.split('\n'):
            match = re.search(r'li\s+"([^"]+)"', line.strip())
            if match:
                move_names.append(match.group(1))

    # Assign proper names: prefer comment, fall back to display name
    for i, move in enumerate(moves):
        if move['comment_name']:
            move['name'] = move['comment_name'].title()
        elif i < len(move_names):
            move['name'] = move_names[i].title()
        else:
            move['name'] = move['asm_name'].replace('_', ' ').title()

    return moves


def get_backport_name(backport_asm_content):
    """Extract the pokemon name from a backports.asm file.

    Returns the parsed name (title-cased, padding removed).
    Raises ValueError if the file is not in the expected format.
    """
    parsed = parse_backports_asm(backport_asm_content)
    return parsed['name']


def import_backport(custom_gen_path, backport_asm_content, backport_moves_asm_content=None, species_name=None):
    """Import backport data into a custom gen's pokemon.json and moves.json.

    Args:
        custom_gen_path: Path to the custom gen folder.
        backport_asm_content: String contents of backports.asm.
        backport_moves_asm_content: Optional string contents of backport_moves.asm.
        species_name: Optional override for the pokemon name.

    Returns:
        A success message string.

    Raises:
        ValueError: If the ASM files are not in the expected format.
    """
    from utils.constants import const

    # Parse the ASM files
    parsed_pokemon = parse_backports_asm(backport_asm_content)

    new_moves = []
    new_move_name_map = {}  # NEW_MOVE_X -> proper name

    if backport_moves_asm_content:
        new_moves = parse_backport_moves_asm(backport_moves_asm_content)
        for move in new_moves:
            new_move_name_map[move['asm_name']] = move['name']

    # Load existing moves.json to build move name lookup
    moves_json_path = os.path.join(custom_gen_path, const.MOVE_DB_FILE_NAME)
    move_lookup = _build_move_lookup(moves_json_path)

    # Resolve move names for TM/HM learnset, splitting out tutor moves
    tm_hm_learnset = []
    tutor_learnset = []

    for asm_name in parsed_pokemon.get('tmhm_asm_names', []):
        resolved = _resolve_move_name(asm_name, move_lookup, new_move_name_map)
        normalized = _normalize_name(resolved)
        if normalized in CRYSTAL_TUTOR_MOVES:
            tutor_learnset.append(resolved)
        else:
            tm_hm_learnset.append(resolved)

    # Resolve move names for level-up learnset
    levelup_moveset = []
    for level, asm_name in parsed_pokemon.get('levelup_asm', []):
        resolved = _resolve_move_name(asm_name, move_lookup, new_move_name_map)
        levelup_moveset.append([level, resolved])

    # Build pokemon entry
    pokemon_entry = {
        'name': species_name if species_name else parsed_pokemon['name'],
        'base_hp': parsed_pokemon['base_hp'],
        'base_atk': parsed_pokemon['base_atk'],
        'base_def': parsed_pokemon['base_def'],
        'base_spc_atk': parsed_pokemon['base_spc_atk'],
        'base_spc_def': parsed_pokemon['base_spc_def'],
        'base_spd': parsed_pokemon['base_spd'],
        'type_1': parsed_pokemon.get('type_1', 'Normal'),
        'type_2': parsed_pokemon.get('type_2', 'Normal'),
        'catch_rate': parsed_pokemon.get('catch_rate', 0),
        'base_xp': parsed_pokemon.get('base_xp', 0),
        'common_item': parsed_pokemon.get('common_item', 'NO_ITEM'),
        'rare_item': parsed_pokemon.get('rare_item', 'NO_ITEM'),
        'gender_ratio': parsed_pokemon.get('gender_ratio', 'GENDER_UNKNOWN'),
        'egg_cycles': parsed_pokemon.get('egg_cycles', 0),
        'growth_rate': parsed_pokemon.get('growth_rate', 'growth_medium_fast'),
        'egg_group_1': parsed_pokemon.get('egg_group_1', 'EGG_NONE'),
        'egg_group_2': parsed_pokemon.get('egg_group_2', 'EGG_NONE'),
        'tm_hm_learnset': tm_hm_learnset,
        'tutor_learnset': tutor_learnset,
        'levelup_moveset': levelup_moveset,
        'egg_moves': [],
    }

    # Append to existing pokemon.json
    pokemon_json_path = os.path.join(custom_gen_path, const.POKEMON_DB_FILE_NAME)
    with open(pokemon_json_path, 'r') as f:
        pokemon_data = json.load(f)

    pokemon_list = pokemon_data.get('pokemon', [])
    pokemon_list.append(pokemon_entry)
    pokemon_data['pokemon'] = pokemon_list

    with open(pokemon_json_path, 'w') as f:
        json.dump(pokemon_data, f, indent=4)

    # Add new moves if any
    if new_moves:
        with open(moves_json_path, 'r') as f:
            moves_data = json.load(f)

        moves_list = moves_data.get('moves', [])

        max_rom_id = max((m.get('rom_id', 0) for m in moves_list), default=0)

        for i, move in enumerate(new_moves):
            move_entry = {
                'name': move['name'],
                'accuracy': move['accuracy'],
                'pp': move['pp'],
                'base_power': move['base_power'],
                'type': move['type'],
                'attack_flavor': [],
                'effects': [],
                'rom_id': max_rom_id + 1 + i,
            }
            moves_list.append(move_entry)

        moves_data['moves'] = moves_list

        with open(moves_json_path, 'w') as f:
            json.dump(moves_data, f, indent=4)

    pokemon_name = species_name if species_name else parsed_pokemon['name']
    move_count = len(new_moves)
    msg = f"Successfully imported '{pokemon_name}'"
    if move_count > 0:
        msg += f" with {move_count} new move(s)"
    return msg
