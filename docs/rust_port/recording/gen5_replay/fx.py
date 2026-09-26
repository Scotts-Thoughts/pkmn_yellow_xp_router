"""Helpers for driving shuckie-feeder over HTTP."""
import json, sys, urllib.request, urllib.parse
import os
BASE = os.environ.get("FX_BASE", "http://127.0.0.1:30190")
def get(route, **q):
    url = BASE + route + ("?" + urllib.parse.urlencode(q) if q else "")
    with urllib.request.urlopen(url, timeout=3600) as r:
        return r.read().decode()
def goto(f): return json.loads(get("/goto", frame=f))
def status(): return json.loads(get("/status"))
def read(addr, n): return bytes.fromhex(get("/read", addr=hex(addr), len=n))
def u8(a): return read(a, 1)[0]
def u16(a): return int.from_bytes(read(a, 2), "little")
def u32(a): return int.from_bytes(read(a, 4), "little")
def shot(path): return get("/screenshot", path=path)
def bsearch(lo, hi, pred):
    """first frame in (lo, hi] where pred() is true, assuming pred false at lo and true at hi."""
    while hi - lo > 1:
        mid = (lo + hi) // 2
        goto(mid)
        if pred(): hi = mid
        else: lo = mid
    return hi
