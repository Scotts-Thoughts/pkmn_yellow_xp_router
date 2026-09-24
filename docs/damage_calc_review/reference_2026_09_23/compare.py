"""Run the harness over the generated cases and diff against the references."""
import json
import os
import subprocess
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ref_calcs  # noqa: E402

ROOT = os.environ.get('XPR_ROOT', os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '../../..')))
OUT = os.environ.get('XPR_SWEEP_OUT', os.path.dirname(os.path.abspath(__file__)))
SWEEP = os.path.join(ROOT, 'rust/target/release/examples/sweep')


def to_dict(arr):
    if arr is None:
        return None
    return {int(k): int(v) for k, v in arr}


def run(version):
    cases = os.path.join(OUT, f'cases_{version}.json')
    out = os.path.join(OUT, f'results_{version}.json')
    subprocess.run([SWEEP, cases, out], check=True, capture_output=True)
    return json.load(open(out))


def describe(r):
    a, d = r['attacker'], r['defender']
    bits = [f"{a['name']} L{a['level']}"]
    if a.get('held_item'):
        bits.append(f"@{a['held_item']}")
    if a.get('ability'):
        bits.append(f"[{a['ability']}]")
    if a.get('badges'):
        bits.append('badges')
    bits.append(f"-> {d['name']} L{d['level']}")
    if d.get('held_item'):
        bits.append(f"@{d['held_item']}")
    if d.get('ability'):
        bits.append(f"[{d['ability']}]")
    if r['custom']:
        bits.append(f"custom={r['custom']!r}")
    if r['weather'] != 'None':
        bits.append(f"weather={r['weather']}")
    if r['doubles']:
        bits.append('doubles')
    for k in ('attacking_stages', 'defending_stages', 'attacking_field', 'defending_field'):
        if r.get(k):
            bits.append(f"{k}={r[k]}")
    if r.get('attacker_is_enemy'):
        bits.append('enemy-attacker')
    return ' '.join(bits)


def summarize(dist):
    if dist is None:
        return 'None'
    if dist == 'SKIP':
        return 'SKIP'
    ks = sorted(dist)
    tot = sum(dist.values())
    if len(ks) <= 6:
        return '{' + ', '.join(f'{k}:{dist[k]}' for k in ks) + '}'
    return f'{ks[0]}..{ks[-1]} ({len(ks)} vals, {tot} rolls)'


def main(versions):
    grand = 0
    grand_mismatch = 0
    for version in versions:
        results = run(version)
        gen = ref_calcs.REF[results[0]['gen']]
        mismatches = defaultdict(list)
        n = 0
        skipped = 0
        for r in results:
            if 'error' in r:
                mismatches[('ERROR', r['error'])].append(r)
                continue
            for crit in (False, True):
                n += 1
                try:
                    exp = gen(r, crit)
                except Exception as e:  # noqa: BLE001
                    mismatches[(r['move']['name'], f'REF EXC {type(e).__name__}: {e}')].append((r, crit, None, None))
                    continue
                if exp == 'SKIP':
                    skipped += 1
                    continue
                got = to_dict(r['crit_damage'] if crit else r['damage'])
                if exp != got:
                    kind = 'None-vs-value' if (exp is None) != (got is None) else ('minmax' if (exp and got and (min(exp) != min(got) or max(exp) != max(got))) else 'distribution')
                    mismatches[(r['move']['name'], kind)].append((r, crit, exp, got))
        total_mm = sum(len(v) for v in mismatches.values())
        grand += n
        grand_mismatch += total_mm
        print(f'===== {version}: {n} checks, {skipped} skipped, {total_mm} mismatches in {len(mismatches)} (move, kind) buckets')
        for (move, kind), items in sorted(mismatches.items(), key=lambda kv: -len(kv[1])):
            print(f'--- {move} [{kind}]: {len(items)}')
            for item in items[:3]:
                if isinstance(item, dict):
                    print('   ', item)
                    continue
                r, crit, exp, got = item
                print(f"    crit={crit} {describe(r)}\n       game {summarize(exp)}\n       app  {summarize(got)}")
    print(f'TOTAL {grand} checks, {grand_mismatch} mismatches')


if __name__ == '__main__':
    main(sys.argv[1:] or ['Yellow', 'Crystal', 'Gold', 'Emerald', 'Platinum'])
