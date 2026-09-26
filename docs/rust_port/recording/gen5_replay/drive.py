"""Drive a replay-less feeder: py -3.14 drive.py <port> <step> [<step> ...]
step: key[+key]:hold[:wait]  (e.g. a:4:60, start:4:120, l+r+start+select:10:300)
      wait:N                  (run N frames with nothing pressed)
      shot:name               (screenshot to shots/<name>.png)
      tap@X,Y[:hold[:wait]]   (touch the bottom screen)
      paced                   (following steps run paced at the feeder's speed)"""
import sys, json, os, urllib.request, urllib.parse
port = sys.argv[1]; B = f"http://127.0.0.1:{port}"
S = os.path.dirname(os.path.abspath(__file__))
paced = "0"
def get(route, **q):
    with urllib.request.urlopen(B + route + "?" + urllib.parse.urlencode(q), timeout=3600) as r:
        return r.read().decode()
for step in sys.argv[2:]:
    if step == "paced":
        paced = "1"; continue
    kind, _, rest = step.partition(":")
    if kind == "wait":
        out = get("/press", keys="", frames=rest, paced=paced)
    elif kind.startswith("tap@"):
        hold, _, wait = rest.partition(":")
        get("/press", keys="", touch=kind[4:], frames=hold or "4", paced=paced)
        out = get("/press", keys="", frames=wait or "30", paced=paced)
    elif kind == "shot":
        out = get("/screenshot", path=os.path.join(S, "shots", rest + ".png"))
    else:
        hold, _, wait = rest.partition(":")
        get("/press", keys=kind.replace("+", ","), frames=hold or "4", paced=paced)
        out = get("/press", keys="", frames=wait or "30", paced=paced)
    print(step, json.loads(out).get("frame"))
