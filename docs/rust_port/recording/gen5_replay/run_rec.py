"""Record a stretch of a replay with the Rust app against the patched Poke-A-Byte.

py -3.14 run_rec.py --work DIR --start F --until F [--speed 4] [--exe path] [--mode quickstart|record --route VERSION|MON]
The feeder must already be running (paused) with the replay loaded, and the
patched Poke-A-Byte on :8095 must have the right mapper loaded.
"""
import argparse, json, os, subprocess, sys, time
import fx

ap = argparse.ArgumentParser()
ap.add_argument("--work", required=True)
ap.add_argument("--start", type=int, required=True)
ap.add_argument("--until", type=int, required=True)
ap.add_argument("--speed", type=float, default=4)
ap.add_argument("--exe", default=r"A:\pkmn_yellow_xp_router\rust\target\release\xpr-app.exe")
ap.add_argument("--mode", default="quickstart")
ap.add_argument("--route", default=None, help="VERSION|MON for --mode record (new route)")
ap.add_argument("--save-name", default="recorded")
ap.add_argument("--pab", default="http://127.0.0.1:8095")
ap.add_argument("--mapper", default="STANDARD/gen5/pokemon_black.xml")
ap.add_argument("--script", default=None, help="a file of drive.py steps played (paced) instead of the replay; /done ends it")
ap.add_argument("--no-seek", action="store_true", help="leave the feeder where it is (a replay-less feeder cannot seek)")
args = ap.parse_args()

work = os.path.abspath(args.work)
cfg_dir = os.path.join(work, "config"); data_dir = os.path.join(work, "data")
os.makedirs(cfg_dir, exist_ok=True); os.makedirs(os.path.join(data_dir, "saved_routes"), exist_ok=True)
json.dump({"user_data_location": data_dir, "debug_mode": True, "auto_load_most_recent_route": False,
           "recording_auto_stop_enabled": False,
           # keep the test window off the user's screens
           "tkinter_window_geometry": "1400x900+12000+12000"}, open(os.path.join(cfg_dir, "config.json"), "w"), indent=4)

fx.get("/pause")
fx.get("/done", v=0)
if not args.no_seek:
    fx.goto(args.start)
fx.get("/speed", x=args.speed)
import urllib.request
def load_mapper():
    req = urllib.request.Request(args.pab + "/mapper-service/change-mapper", data=json.dumps(args.mapper).encode(), method="PUT", headers={"Content-Type": "application/json"})
    urllib.request.urlopen(req, timeout=60).read()
load_mapper()
for _ in range(60):
    if fx.status()["session"]:
        break
    time.sleep(0.5)
else:
    sys.exit("Poke-A-Byte never set up a session with the feeder")
time.sleep(2)
tc = urllib.request.urlopen(args.pab + "/mapper/values/player.team_count/").read().decode()
print("session up; team_count via Poke-A-Byte =", tc, flush=True)
env = dict(os.environ)
env.update({
    "XPR_GLOBAL_CONFIG_DIR": cfg_dir,
    "XPR_GAMEHOOK_URL": args.pab,
    "XPR_DISABLE_AUTO_UPDATE": "1",
    "XPR_SMOKE_SCREENSHOT": os.path.join(work, "final.png"),
    "XPR_SMOKE_ACTION": args.mode,
    "XPR_SMOKE_SAVE_NAME": args.save_name,
    "XPR_SMOKE_RECORD_SECS": str(int((args.until - args.start) / (60 * max(args.speed, 0.5)) + 120)),
    "XPR_SMOKE_STOP_URL": fx.BASE + "/status",
})
if args.route:
    env["XPR_SMOKE_NEW_ROUTE"] = args.route
out = open(os.path.join(work, "app.out"), "w", encoding="utf-8")
si = subprocess.STARTUPINFO()
si.dwFlags |= subprocess.STARTF_USESHOWWINDOW
si.wShowWindow = 4  # SW_SHOWNOACTIVATE: do not take focus from the user
app = subprocess.Popen([args.exe], env=env, cwd=r"A:\pkmn_yellow_xp_router", stdout=out, stderr=subprocess.STDOUT, startupinfo=si)
time.sleep(8)  # let the app connect and arm the quick start
t0 = time.time()
if args.script:
    steps = [l.strip() for l in open(args.script, encoding="utf-8") if l.strip() and not l.lstrip().startswith("#")]
    port = fx.BASE.rsplit(":", 1)[1]
    here = os.path.dirname(os.path.abspath(__file__))
    subprocess.run([sys.executable, os.path.join(here, "drive.py"), port, "paced", *steps], cwd=here, stdout=out, stderr=subprocess.STDOUT)
    fx.get("/done")
else:
    fx.get("/runto", frame=args.until)
while app.poll() is None:
    time.sleep(5)
    st = fx.status()
    print(f"[{time.time()-t0:5.0f}s] frame {st['frame']} done={st['done']}", flush=True)
print("app exit", app.returncode, "after", round(time.time() - t0), "s")
