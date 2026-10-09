"""Import a set of real Super Shuckie replays and report how each went.

    py -3.14 run_corpus.py --exe <replay_to_route.exe> --work <dir> [--only name,...] [--ref] [--extra "--stride 2"]

For every replay in CORPUS (or --corpus <json>: [{"name", "replay", "args"}]):
writes <work>/<name>.json (the route), <work>/<name>.out (the tool's report)
and <work>/logs_<name>/ (the log). With --ref it also builds the reference
route (every frame, one mapper instance: --dense-all --single-chunk) as
<work>/<name>.ref.json and diffs the two with route_summary.py. The table
goes to stdout and <work>/summary.md.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
UD = r"A:\Dropbox\stp-projects\programs\supershuckie 2\UserData"

CORPUS = [
    {"name": "red", "replay": UD + r"\1-Red v1.0.1.gb-data\replays\r-mewtwo-1-3950.replay"},
    {"name": "yellow", "replay": UD + r"\Yellow-2027 v1.0.2.gbc-data\replays\Austin - 2026-09-19 01.31.24.replay"},
    {"name": "crystal", "replay": UD + r"\2-Crystal v2.2.1.gbc-data\replays\c-chikorita-1-20904.replay"},
    {"name": "emerald", "replay": UD + r"\3-Emerald v1.6.0.gba-data\replays\e-nosepass-2-35147.replay"},
    {"name": "firered", "replay": UD + r"\3-FireRed v1.6.0.gba-data\replays\f-parasect-line-1-14140.replay"},
    {"name": "heartgold", "replay": UD + r"\4-HeartGold v6.nds-data\replays\h-meganium-line-2-21608.replay"},
    {"name": "platinum", "replay": UD + r"\4-Platinum v6.nds-data\replays\p-eevee-1-22928.replay"},
    {"name": "black", "replay": UD + r"\5-Black v1.nds-data\replays\b-victini-1-12015.replay"},
    {"name": "white", "replay": UD + r"\White-2026 v1.nds-data\replays\w-basculin-0-13300.replay"},
    {"name": "white2", "replay": UD + r"\5-White2 v1.nds-data\replays\w-smeargle-2-22943.replay"},
]


def run(exe, case, work, extra, ref=False):
    name = case["name"] + (".ref" if ref else "")
    out_json = os.path.join(work, name + ".json")
    args = [exe, case["replay"], "--out", out_json, "--log-dir", os.path.join(work, "logs_" + name)]
    args += case.get("args", []) + extra
    if ref:
        args += ["--dense-all", "--single-chunk"]
    t = time.time()
    p = subprocess.run(args, capture_output=True, text=True, encoding="utf-8", errors="replace")
    secs = time.time() - t
    with open(os.path.join(work, name + ".out"), "w", encoding="utf-8") as f:
        f.write(p.stdout)
        f.write("\n--- stderr (last 4000 chars) ---\n")
        f.write(p.stderr[-4000:])
    row = {"name": name, "secs": secs, "ok": p.returncode == 0, "events": "", "check": "", "detail": ""}
    if p.returncode != 0:
        row["detail"] = (p.stderr.strip().splitlines() or ["?"])[-1][:160]
        return row
    m = re.search(r"^events: (\d+)(.*)$", p.stdout, re.M)
    if m:
        row["events"] = m.group(1) + (" (final)" if "final trainer" in m.group(2) else "")
    m = re.search(r"^check against the game: (\w+)$", p.stdout, re.M)
    if m:
        row["check"] = m.group(1)
    m = re.search(r"^time: (.*)$", p.stdout, re.M)
    if m:
        row["detail"] = m.group(1)
    fails = re.findall(r"^  (level|exp|moves): game.*$", p.stdout, re.M)
    if fails:
        row["detail"] += " | " + "; ".join(l.strip() for l in re.findall(r"^  (?:level|exp|moves): game.*$", p.stdout, re.M))
    return row


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--work", required=True)
    ap.add_argument("--only", default="")
    ap.add_argument("--corpus")
    ap.add_argument("--ref", action="store_true")
    ap.add_argument("--extra", default="")
    a = ap.parse_args()
    corpus = CORPUS
    if a.corpus:
        with open(a.corpus, encoding="utf-8") as f:
            corpus = json.load(f)
    if a.only:
        names = set(a.only.split(","))
        corpus = [c for c in corpus if c["name"] in names]
    os.makedirs(a.work, exist_ok=True)
    extra = a.extra.split() if a.extra else []
    rows = []
    for case in corpus:
        row = run(a.exe, case, a.work, extra)
        rows.append(row)
        print(f"{row['name']:12s} {row['secs']:7.1f}s ok={row['ok']} events={row['events']} check={row['check']} {row['detail']}", flush=True)
        if a.ref and row["ok"]:
            ref = run(a.exe, case, a.work, extra, ref=True)
            rows.append(ref)
            print(f"{ref['name']:12s} {ref['secs']:7.1f}s ok={ref['ok']} events={ref['events']} check={ref['check']} {ref['detail']}", flush=True)
            if ref["ok"]:
                d = subprocess.run([sys.executable, os.path.join(HERE, "route_summary.py"), os.path.join(a.work, case["name"] + ".json"), os.path.join(a.work, case["name"] + ".ref.json")], capture_output=True, text=True, encoding="utf-8")
                with open(os.path.join(a.work, case["name"] + ".diff"), "w", encoding="utf-8") as f:
                    f.write(d.stdout)
                print(f"    diff vs reference: {d.stdout.strip().splitlines()[-1]}", flush=True)
    with open(os.path.join(a.work, "summary.md"), "w", encoding="utf-8") as f:
        f.write("| replay | seconds | ok | events | check | detail |\n|---|---|---|---|---|---|\n")
        for r in rows:
            f.write(f"| {r['name']} | {r['secs']:.1f} | {r['ok']} | {r['events']} | {r['check']} | {r['detail']} |\n")


if __name__ == "__main__":
    main()
