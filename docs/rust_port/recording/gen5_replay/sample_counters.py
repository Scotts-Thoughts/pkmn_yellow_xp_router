"""Sample u16 counters through a replay and print where they change.
py -3.14 sample_counters.py <port> <from> <to> <step> <addr> [<addr> ...]"""
import sys, fx
fx.BASE = f"http://127.0.0.1:{sys.argv[1]}"
lo, hi, step = (int(x) for x in sys.argv[2:5])
addrs = [int(a, 16) for a in sys.argv[5:]]
last = None
for f in range(lo, hi, step):
    fx.goto(f); print("at", f, file=sys.stderr, flush=True)
    vals = tuple(fx.u16(a) for a in addrs)
    if vals != last:
        print(f, " ".join(f"{a:#x}={v}" for a, v in zip(addrs, vals)), flush=True)
        last = vals
