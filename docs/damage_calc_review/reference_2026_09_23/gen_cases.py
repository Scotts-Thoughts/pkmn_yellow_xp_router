"""Generate sweep cases for the Rust harness (one JSON array)."""
import itertools
import json
import os
import sys

ROOT = os.environ.get('XPR_ROOT', os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '../../..')))
OUT = os.environ.get('XPR_SWEEP_OUT', os.path.dirname(os.path.abspath(__file__)))

SPECIAL_FLAVORS = {'one_hit_ko', 'fixed_damage', 'level_damage', 'psywave', 'super_fang', 'counter', 'bide',
                   'mirror_coat', 'endeavor', 'nature_power'}

POOLS = {
    'Yellow': ['Pikachu', 'Nidoking', 'Alakazam', 'Charizard', 'Onix', 'Snorlax', 'Gengar', 'Pidgeotto', 'Rattata',
               'Machamp', 'Lapras', 'Dragonite', 'Jynx', 'Weedle', 'Golem', 'Raticate', 'Cloyster', 'Aerodactyl',
               'Zubat', 'Chansey'],
    'Crystal': ['Totodile', 'Feraligatr', 'Pidgey', 'Sentret', 'Geodude', 'Onix', 'Machamp', 'Snorlax', 'Ampharos',
                'Alakazam', 'Scizor', 'Umbreon', 'Espeon', 'Marowak', 'Cubone', 'Pikachu', 'Ditto', 'Delibird',
                'Togepi', 'Tyranitar', 'Lugia', 'Miltank', 'Sudowoodo', "Farfetch'd", 'Chansey', 'Gengar', 'Skarmory'],
    'Emerald': ['Mudkip', 'Swampert', 'Torchic', 'Blaziken', 'Machop', 'Machamp', 'Ralts', 'Gardevoir', 'Aron',
                'Aggron', 'Zigzagoon', 'Linoone', 'Shedinja', 'Castform', 'Latios', 'Clamperl', 'Cubone', 'Pikachu',
                'Ditto', 'Azumarill', 'Slaking', 'Whismur', 'Electrike', 'Sableye', 'Nosepass', 'Wingull', 'Swellow',
                'Rayquaza', 'Vulpix', 'Poliwag', 'Skarmory', 'Gengar', 'Steelix', 'Chansey', "Farfetch'd"],
    'Platinum': ['Chimchar', 'Infernape', 'Piplup', 'Starly', 'Staraptor', 'Bidoof', 'Gastly', 'Geodude', 'Bronzor',
                 'Bronzong', 'Scizor', 'Cherrim', 'Shedinja', 'Pikachu', 'Magikarp', 'Latios', 'Dialga', 'Palkia',
                 'Giratina', 'Arceus', 'Cranidos', 'Rampardos', 'Lucario', 'Garchomp', 'Gible', 'Shinx', 'Luxray',
                 'Ambipom', 'Breloom', 'Ditto', 'Clamperl', 'Cubone', 'Marowak', 'Chansey', "Farfetch'd", 'Vulpix',
                 'Poliwag', 'Electrode', 'Skarmory', 'Kingdra', 'Lanturn', 'Snorlax', 'Steelix'],
}
POOLS['Gold'] = POOLS['Crystal']
LEVELS = [5, 20, 50, 80, 100]
GEN_OF = {'Yellow': 1, 'Crystal': 2, 'Gold': 2, 'Emerald': 3, 'Platinum': 4}
DIR_OF = {1: 'gen_one', 2: 'gen_two', 3: 'gen_three', 4: 'gen_four'}
BADGES = {1: ['boulder', 'cascade', 'thunder', 'rainbow', 'soul', 'marsh', 'volcano', 'earth'],
          2: ['zephyr', 'hive', 'plain', 'fog', 'storm', 'mineral', 'glacier', 'rising', 'boulder', 'cascade',
              'thunder', 'rainbow', 'soul', 'marsh', 'volcano', 'earth'],
          3: ['stone', 'knuckle', 'dynamo', 'heat', 'balance', 'feather', 'mind', 'rain'], 4: []}
WEATHERS = {1: [], 2: ['Rain', 'Harsh Sunlight'], 3: ['Rain', 'Harsh Sunlight', 'Sandstorm', 'Hail'],
            4: ['Rain', 'Harsh Sunlight', 'Sandstorm', 'Hail', 'Fog']}


def load_moves(gen):
    d = json.load(open(os.path.join(ROOT, 'raw_pkmn_data', DIR_OF[gen], 'moves.json')))
    if gen == 4:
        return [(k, m['power'], m['attack_flavor'], m['category'] != 'Status') for k, m in d.items()]
    return [(m['name'], m['base_power'], m['attack_flavor'], (m['base_power'] or 0) > 0 or any(f in SPECIAL_FLAVORS for f in m['attack_flavor']))
            for m in d['moves']]


def load_options(version):
    return json.load(open(os.path.join(OUT, f'options_{version}.json')))


def pick_options(name, opts):
    if opts is None:
        return ['']
    if len(opts) > 8:
        return [opts[0], opts[len(opts) // 2], opts[-1]]
    return opts


def gen_cases(version):
    gen = GEN_OF[version]
    moves = load_moves(gen)
    options = load_options(version)
    pool = POOLS[version]
    cases = []
    cid = 0

    def add(a, d, mv, custom='', **kw):
        nonlocal cid
        cid += 1
        case = {'id': cid, 'version': version, 'attacker': a, 'defender': d, 'move': mv, 'custom': custom}
        case.update(kw)
        cases.append(case)

    idx = 0
    for mi, (name, power, flavors, damaging) in enumerate(moves):
        if not damaging:
            continue
        opts = options.get(name)
        if opts is None and 'multi_hit' in flavors:
            opts = ['2 Hits', '3 Hits', '4 Hits', '5 Hits']
        for custom in pick_options(name, opts):
            for k in range(3):
                a_name = pool[(idx * 7 + k * 3) % len(pool)]
                d_name = pool[(idx * 11 + k * 5 + 1) % len(pool)]
                la = LEVELS[(idx + k) % len(LEVELS)]
                ld = LEVELS[(idx * 3 + k + 1) % len(LEVELS)]
                idx += 1
                a = {'name': a_name, 'level': la}
                d = {'name': d_name, 'level': ld}
                add(a, d, name, custom)
        # modifier variants on a rotating subset of matchups
        variants = []
        a = {'name': pool[(mi * 5) % len(pool)], 'level': 50}
        d = {'name': pool[(mi * 3 + 2) % len(pool)], 'level': 50}
        custom = pick_options(name, opts)[0]
        variants.append(dict(attacking_stages={'atk': 2, 'spa': 2}, defending_stages={'def': -1, 'spd': -1}))
        variants.append(dict(attacking_stages={'atk': -2, 'spa': -2}, defending_stages={'def': 2, 'spd': 2}))
        variants.append(dict(attacking_stages={'atk': 6, 'spa': 6}, defending_stages={'def': 6, 'spd': 6}))
        variants.append(dict(attacking_stages={'atk': 1, 'spa': 1}, defending_stages={'def': 1, 'spd': 1}))
        variants.append(dict(defending_field={'reflect': True, 'light_screen': True}))
        variants.append(dict(defending_field={'reflect': True, 'light_screen': True}, attacking_stages={'atk': 3, 'spa': 3}))
        variants.append(dict(defending_field={'reflect': True, 'light_screen': True}, defending_stages={'def': 6, 'spd': 6}))
        if BADGES[gen]:
            variants.append(dict(attacker_extra={'badges': BADGES[gen]}))
            variants.append(dict(attacker_extra={'badges': BADGES[gen]}, attacking_stages={'atk': 2, 'spa': 2}))
            variants.append(dict(defender_extra={'badges': BADGES[gen]}))
        for w in WEATHERS[gen]:
            variants.append(dict(weather=w))
        if gen >= 3:
            variants.append(dict(doubles=True))
            variants.append(dict(doubles=True, defending_field={'reflect': True, 'light_screen': True}))
        if gen == 1:
            variants.append(dict(attacker_extra={'name': 'Alakazam', 'level': 80}, attacking_stages={'spa': 6, 'atk': 6}, defender_extra={'name': 'Onix', 'level': 60}, defending_field={'reflect': True, 'light_screen': True}))
            variants.append(dict(attacker_is_enemy=True))
        if gen == 2:
            variants.append(dict(attacker_extra={'name': 'Marowak', 'held_item': 'Thick Club'}))
            variants.append(dict(attacker_extra={'name': 'Cubone', 'held_item': 'Thick Club'}))
            variants.append(dict(attacker_extra={'name': 'Pikachu', 'held_item': 'Light Ball'}))
            variants.append(dict(defender_extra={'name': 'Ditto', 'held_item': 'Metal Powder'}))
            variants.append(dict(defender_extra={'name': 'Ditto', 'held_item': 'Metal Powder'}, defending_stages={'def': 6, 'spd': 6}))
            variants.append(dict(attacker_extra={'held_item': 'Pink Bow'}))
            variants.append(dict(attacker_extra={'held_item': 'Charcoal'}))
            variants.append(dict(attacker_extra={'held_item': 'Mystic Water'}))
            variants.append(dict(attacker_extra={'held_item': 'Dragon Scale', 'name': 'Dragonite'}))
            variants.append(dict(attacker_extra={'name': 'Tyranitar', 'level': 100}, attacking_stages={'atk': 6, 'spa': 6}, defender_extra={'name': 'Snorlax', 'level': 100}, defending_field={'reflect': True, 'light_screen': True}))
            variants.append(dict(attacker_extra={'name': 'Tyranitar', 'level': 100}, defender_extra={'name': 'Skarmory', 'level': 100}, defending_stages={'def': 6, 'spd': 6}, defending_field={'reflect': True, 'light_screen': True}))
            variants.append(dict(attacker_extra={'name': 'Delibird'}, defender_extra={'name': 'Umbreon'}))
            variants.append(dict(attacker_extra={'name': 'Togepi'}, defender_extra={'name': 'Onix'}))
            variants.append(dict(attacker_extra={'name': "Farfetch'd", 'held_item': 'Stick'}))
            variants.append(dict(attacker_extra={'held_item': 'Scope Lens'}))
        if gen == 3:
            variants.append(dict(attacker_extra={'ability': 'Huge Power'}))
            variants.append(dict(attacker_extra={'ability': 'Hustle'}))
            variants.append(dict(attacker_extra={'ability': 'Huge Power', 'held_item': 'Choice Band'}, attacking_stages={'atk': 1, 'spa': 1}))
            variants.append(dict(attacker_extra={'held_item': 'Choice Band'}, attacking_stages={'atk': -1, 'spa': -1}))
            variants.append(dict(defender_extra={'ability': 'Thick Fat'}))
            variants.append(dict(defender_extra={'ability': 'Levitate'}))
            variants.append(dict(defender_extra={'name': 'Shedinja', 'ability': 'Wonder Guard'}))
            variants.append(dict(defender_extra={'ability': 'Battle Armor'}))
            variants.append(dict(defender_extra={'ability': 'Volt Absorb'}))
            variants.append(dict(defender_extra={'ability': 'Water Absorb'}))
            variants.append(dict(defender_extra={'ability': 'Flash Fire'}))
            variants.append(dict(defender_extra={'ability': 'Soundproof'}))
            variants.append(dict(defender_extra={'ability': 'Damp'}))
            variants.append(dict(attacker_extra={'ability': 'Cloud Nine'}, weather='Rain'))
            variants.append(dict(attacker_extra={'name': 'Castform', 'ability': 'Forecast'}, weather='Harsh Sunlight'))
            variants.append(dict(defender_extra={'name': 'Castform', 'ability': 'Forecast'}, weather='Rain'))
            variants.append(dict(attacker_extra={'name': 'Latios', 'held_item': 'Soul Dew'}))
            variants.append(dict(defender_extra={'name': 'Latios', 'held_item': 'Soul Dew'}))
            variants.append(dict(attacker_extra={'name': 'Clamperl', 'held_item': 'DeepSeaTooth'}))
            variants.append(dict(defender_extra={'name': 'Clamperl', 'held_item': 'DeepSeaScale'}))
            variants.append(dict(attacker_extra={'name': 'Pikachu', 'held_item': 'Light Ball'}))
            variants.append(dict(defender_extra={'name': 'Ditto', 'held_item': 'Metal Powder'}))
            variants.append(dict(attacker_extra={'name': 'Cubone', 'held_item': 'Thick Club'}))
            variants.append(dict(attacker_extra={'held_item': 'Sea Incense'}))
            variants.append(dict(attacker_extra={'held_item': 'Charcoal'}, attacking_stages={'spa': 1}))
            variants.append(dict(attacker_extra={'held_item': 'Black Belt'}, attacking_stages={'atk': 1}))
            variants.append(dict(attacker_extra={'held_item': 'Mystic Water', 'badges': BADGES[3]}))
            variants.append(dict(attacker_extra={'ability': 'Huge Power', 'badges': BADGES[3]}))
        if gen == 4:
            variants.append(dict(attacker_extra={'ability': 'Technician'}))
            variants.append(dict(attacker_extra={'ability': 'Technician'}, attacking_stages={'atk': 1, 'spa': 1}))
            variants.append(dict(attacker_extra={'ability': 'Huge Power'}))
            variants.append(dict(attacker_extra={'ability': 'Hustle', 'held_item': 'Choice Band'}))
            variants.append(dict(attacker_extra={'ability': 'Adaptability'}))
            variants.append(dict(attacker_extra={'ability': 'Sniper'}))
            variants.append(dict(attacker_extra={'ability': 'Iron Fist'}))
            variants.append(dict(attacker_extra={'ability': 'Reckless'}))
            variants.append(dict(attacker_extra={'ability': 'Tinted Lens'}))
            variants.append(dict(attacker_extra={'ability': 'Normalize'}))
            variants.append(dict(attacker_extra={'ability': 'Scrappy'}, defender_extra={'name': 'Gastly'}))
            variants.append(dict(attacker_extra={'ability': 'Klutz', 'held_item': 'Choice Band'}))
            variants.append(dict(attacker_extra={'name': 'Arceus', 'ability': 'Multitype', 'held_item': 'Earth Plate'}))
            variants.append(dict(attacker_extra={'name': 'Arceus', 'ability': 'Multitype', 'held_item': 'Zap Plate'}, defender_extra={'name': 'Starly'}))
            variants.append(dict(defender_extra={'ability': 'Thick Fat'}))
            variants.append(dict(defender_extra={'ability': 'Heatproof'}))
            variants.append(dict(defender_extra={'ability': 'Dry Skin'}))
            variants.append(dict(defender_extra={'ability': 'Filter'}))
            variants.append(dict(defender_extra={'ability': 'Solid Rock'}, attacker_extra={'held_item': 'Expert Belt'}))
            variants.append(dict(defender_extra={'ability': 'Levitate'}))
            variants.append(dict(defender_extra={'ability': 'Levitate'}, defending_field={'gravity': True}))
            variants.append(dict(defender_extra={'name': 'Shedinja', 'ability': 'Wonder Guard'}))
            variants.append(dict(defender_extra={'ability': 'Battle Armor'}))
            variants.append(dict(defender_extra={'ability': 'Motor Drive'}))
            variants.append(dict(defender_extra={'ability': 'Soundproof'}))
            variants.append(dict(defender_extra={'ability': 'Flash Fire'}))
            variants.append(dict(defender_extra={'name': 'Starly'}, defending_field={'roost': True}))
            variants.append(dict(defender_extra={'name': 'Starly'}, defending_field={'gravity': True}))
            variants.append(dict(defender_extra={'name': 'Bronzor'}, defending_field={'magnet_rise': True}))
            variants.append(dict(defender_extra={'name': 'Cherrim', 'ability': 'Flower Gift'}, weather='Harsh Sunlight'))
            variants.append(dict(attacker_extra={'name': 'Cherrim', 'ability': 'Flower Gift'}, weather='Harsh Sunlight'))
            variants.append(dict(attacker_extra={'ability': 'Solar Power'}, weather='Harsh Sunlight'))
            variants.append(dict(defender_extra={'name': 'Geodude'}, weather='Sandstorm'))
            variants.append(dict(attacker_extra={'held_item': 'Life Orb'}))
            variants.append(dict(attacker_extra={'held_item': 'Expert Belt'}))
            variants.append(dict(attacker_extra={'held_item': 'Muscle Band'}))
            variants.append(dict(attacker_extra={'held_item': 'Wise Glasses'}))
            variants.append(dict(attacker_extra={'held_item': 'Choice Specs'}))
            variants.append(dict(attacker_extra={'held_item': 'Charcoal'}, attacking_stages={'spa': 1, 'atk': 1}))
            variants.append(dict(attacker_extra={'held_item': 'Flame Plate'}))
            variants.append(dict(attacker_extra={'name': 'Dialga', 'held_item': 'Adamant Orb'}))
            variants.append(dict(attacker_extra={'name': 'Palkia', 'held_item': 'Lustrous Orb'}))
            variants.append(dict(attacker_extra={'name': 'Giratina', 'held_item': 'Griseous Orb'}))
            variants.append(dict(attacker_extra={'name': 'Latios', 'held_item': 'Soul Dew'}))
            variants.append(dict(defender_extra={'name': 'Latios', 'held_item': 'Soul Dew'}))
            variants.append(dict(attacker_extra={'name': 'Pikachu', 'held_item': 'Light Ball'}))
            variants.append(dict(attacker_extra={'name': 'Cubone', 'held_item': 'Thick Club'}))
            variants.append(dict(defender_extra={'name': 'Ditto', 'held_item': 'Metal Powder'}))
            variants.append(dict(defender_extra={'name': 'Clamperl', 'held_item': 'DeepSeaScale'}))
            variants.append(dict(attacker_extra={'name': 'Clamperl', 'held_item': 'DeepSeaTooth'}))
            variants.append(dict(defender_extra={'name': 'Starly', 'held_item': 'Iron Ball'}))
            variants.append(dict(attacking_field={'power_trick': True}))
            variants.append(dict(attacking_field={'slow_start': True}))
            variants.append(dict(attacking_field={'worry_seed': True}, attacker_extra={'ability': 'Huge Power'}))
            variants.append(dict(defending_field={'gastro_acid': True}, defender_extra={'ability': 'Levitate'}))
            variants.append(dict(attacking_field={'trick_room': True}, defending_field={'trick_room': True}))
            variants.append(dict(attacker_extra={'name': 'Bronzor'}, defender_extra={'name': 'Staraptor'}))
            variants.append(dict(defending_stages={'atk': 2, 'def': 2, 'spa': 1, 'spd': 1, 'spe': 3, 'acc': 1, 'eva': 2}))
            variants.append(dict(defending_field={'miracle_eye': True}, defender_extra={'name': 'Umbreon'}))
        # rotate: each move gets a slice of the variant list (all moves together cover everything)
        n_var = len(variants)
        take = 6 if (mi % 4) else n_var
        for vi in range(n_var):
            if take != n_var and ((vi + mi) % n_var) >= take:
                continue
            v = dict(variants[vi])
            aa = dict(a)
            aa.update(v.pop('attacker_extra', {}))
            dd = dict(d)
            dd.update(v.pop('defender_extra', {}))
            add(aa, dd, name, custom, **v)
    return cases


if __name__ == '__main__':
    versions = sys.argv[1:] or ['Yellow', 'Crystal', 'Gold', 'Emerald', 'Platinum']
    for v in versions:
        cases = gen_cases(v)
        with open(os.path.join(OUT, f'cases_{v}.json'), 'w') as f:
            json.dump(cases, f)
        print(v, len(cases), 'cases')
