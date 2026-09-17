"""A stand-in for GameHook / Poke-A-Byte that replays a scripted session.

Only the surface the router's recorder clients use is served:

* ``POST /updates/negotiate``   -> SignalR negotiate response
* ``GET  /mapper``              -> the mapper (meta, glossary, current property values)
* ``PUT  /mapper/properties/*`` -> 200 (property edits are ignored)
* ``GET  /updates?id=...``      -> the SignalR hub over a websocket (JSON protocol):
  handshake, pings, and ``PropertiesChanged`` invocations driven by the scenario

The same scenario is played to whichever client connects (the Python app's
signalrcore client or the Rust app's hand-written client), which is what makes
the two implementations comparable: same inputs, diff the saved routes.

Standard library only. Usage::

    py -3.14 mock_gamehook.py --port 8099 --scenario scenarios/emerald_geodude.py
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import importlib.util
import json
import socket
import struct
import sys
import threading
import time
import urllib.parse

RS = "\x1e"  # SignalR record separator
WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


# ----------------------------------------------------------------------------
# scenario DSL
# ----------------------------------------------------------------------------
class Scenario:
    """Recorded steps: ``set`` (one PropertiesChanged batch), ``wait`` (real seconds)
    and ``note`` (logged). Built by the scenario module's ``build()`` function."""

    def __init__(self, game_name: str, properties: dict, tick_key: str, tick_delay: float = 0.06, step_delay: float = 0.06):
        self.game_name = game_name
        self.initial = dict(properties)
        self.tick_key = tick_key
        self.tick_delay = tick_delay
        self.step_delay = step_delay
        self.steps: list[tuple] = []
        self._seconds = int(properties.get(tick_key) or 0)

    # -- building --------------------------------------------------------
    def set(self, *pairs, **kw):
        """``s.set("path", value, "path2", value2)`` or ``s.set(**{...})``: one batch."""
        changes = {}
        if len(pairs) % 2 != 0:
            raise ValueError("set() takes path/value pairs")
        for i in range(0, len(pairs), 2):
            changes[pairs[i]] = pairs[i + 1]
        changes.update(kw)
        for p in changes:
            if p not in self.initial:
                raise KeyError(f"scenario sets unknown property {p!r}")
        self.steps.append(("set", changes, self.step_delay))
        return self

    def tick(self, n: int = 1, delay: float | None = None):
        """Advance the in-game seconds counter ``n`` times (one message each)."""
        for _ in range(n):
            self._seconds += 1
            self.steps.append(("set", {self.tick_key: self._seconds}, self.tick_delay if delay is None else delay))
        return self

    def wait(self, seconds: float):
        self.steps.append(("wait", seconds))
        return self

    def note(self, text: str):
        self.steps.append(("note", text))
        return self

    @property
    def seconds(self):
        return self._seconds


def load_scenario(path: str) -> Scenario:
    spec = importlib.util.spec_from_file_location("scenario_module", path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["scenario_module"] = mod
    spec.loader.exec_module(mod)
    return mod.build()


# ----------------------------------------------------------------------------
# server
# ----------------------------------------------------------------------------
def log(msg: str):
    print(f"[mock-gamehook {time.strftime('%H:%M:%S')}] {msg}", flush=True)


def _read_request(conn: socket.socket, initial: bytes = b""):
    """Read one HTTP request; returns (method, target, headers, body, leftover) or None at EOF."""
    head = initial
    while b"\r\n\r\n" not in head:
        chunk = conn.recv(4096)
        if not chunk:
            return None
        head += chunk
    header_blob, _, rest = head.partition(b"\r\n\r\n")
    lines = header_blob.decode("latin-1").split("\r\n")
    method, target, _ = lines[0].split(" ", 2)
    headers = {}
    for line in lines[1:]:
        if ":" in line:
            k, v = line.split(":", 1)
            headers[k.strip().lower()] = v.strip()
    length = int(headers.get("content-length", "0") or 0)
    body = rest
    while len(body) < length:
        chunk = conn.recv(4096)
        if not chunk:
            break
        body += chunk
    return method, target, headers, body[:length], body[length:]


class MockGameHook:
    def __init__(self, port: int, scenario: Scenario, start_delay: float = 1.5, loop: bool = False, mapper_loaded: bool = True, shared_playback: bool = False):
        self.port = port
        self.scenario = scenario
        self.start_delay = start_delay
        self.loop = loop
        self.mapper_loaded = mapper_loaded
        # shared playback: one run of the scenario, broadcast to every client
        # connected at the time of each step (for flows that hand the session
        # from one client to another, like the app's "Start Recording")
        self.shared_playback = shared_playback
        self.values = dict(scenario.initial)
        self.lock = threading.Lock()
        self.mapper_fetched = threading.Event()
        self.playback_done = threading.Event()
        self._addresses = {p: 0x2000000 + i * 4 for i, p in enumerate(scenario.initial)}
        self._played = False
        self._clients: dict[int, object] = {}
        self._clients_lock = threading.Lock()

    # -- HTTP -------------------------------------------------------------
    def mapper_json(self) -> dict:
        with self.lock:
            props = [
                {"path": p, "value": v, "bytes": self._bytes_of(v), "address": self._addresses[p], "size": 1, "frozen": False}
                for p, v in self.values.items()
            ]
        return {"meta": {"gameName": self.scenario.game_name, "id": "mock"}, "glossary": {}, "properties": props}

    @staticmethod
    def _bytes_of(v):
        if isinstance(v, bool):
            return [1 if v else 0]
        if isinstance(v, int):
            return [v & 0xFF, (v >> 8) & 0xFF]
        if v is None:
            return None
        return [0]

    def serve_forever(self):
        srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        srv.bind(("127.0.0.1", self.port))
        srv.listen(8)
        log(f"listening on http://127.0.0.1:{self.port} ({len(self.scenario.steps)} scenario steps)")
        while True:
            conn, _ = srv.accept()
            threading.Thread(target=self._handle, args=(conn,), daemon=True).start()

    def _handle(self, conn: socket.socket):
        try:
            conn.settimeout(30)
            leftover = b""
            while True:
                req = _read_request(conn, leftover)
                if req is None:
                    return
                method, target, headers, body, leftover = req
                parsed = urllib.parse.urlsplit(target)
                if headers.get("upgrade", "").lower() == "websocket":
                    self._websocket(conn, headers, parsed)
                    return
                self._rest(conn, method, parsed.path)
                if headers.get("connection", "").lower() == "close":
                    return
        except (ConnectionResetError, ConnectionAbortedError, socket.timeout):
            pass
        except Exception as e:  # keep serving other clients
            log(f"connection error: {type(e).__name__}: {e}")
        finally:
            try:
                conn.close()
            except OSError:
                pass

    def _rest(self, conn, method, path):
        if method == "POST" and path.rstrip("/").endswith("/updates/negotiate"):
            payload = {
                "negotiateVersion": 1,
                "connectionId": "mock-conn-1",
                "connectionToken": "mock-conn-1",
                "availableTransports": [{"transport": "WebSockets", "transferFormats": ["Text", "Binary"]}],
            }
            self._respond(conn, 200, payload)
        elif method == "GET" and path == "/mapper":
            if self.mapper_loaded:
                log("GET /mapper")
                self._respond(conn, 200, self.mapper_json())
                self.mapper_fetched.set()
            else:
                self._respond(conn, 400, {"title": "MAPPER_NOT_LOADED", "status": 400, "detail": "Please load a mapper file first."})
        elif method == "PUT" and path.startswith("/mapper/properties/"):
            self._respond(conn, 200, {})
        elif method == "GET" and path == "/mock/status":
            # for the launchers: has the scenario been played to completion?
            self._respond(conn, 200, {"played": self._played, "done": self.playback_done.is_set(), "steps": len(self.scenario.steps)})
        else:
            self._respond(conn, 404, {"detail": f"no route for {method} {path}"})

    @staticmethod
    def _respond(conn, status, payload):
        data = json.dumps(payload).encode("utf-8")
        reason = {200: "OK", 400: "Bad Request", 404: "Not Found"}.get(status, "OK")
        head = (
            f"HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {len(data)}\r\n"
            "Access-Control-Allow-Origin: *\r\n\r\n"
        ).encode("latin-1")
        conn.sendall(head + data)

    # -- websocket ----------------------------------------------------------
    def _websocket(self, conn: socket.socket, headers: dict, parsed):
        key = headers.get("sec-websocket-key", "")
        accept = base64.b64encode(hashlib.sha1((key + WS_GUID).encode()).digest()).decode()
        conn.sendall(
            ("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n" f"Sec-WebSocket-Accept: {accept}\r\n\r\n").encode("latin-1")
        )
        log(f"websocket client connected ({parsed.query})")
        conn.settimeout(0.25)
        send_lock = threading.Lock()
        alive = threading.Event()
        alive.set()

        def send_text(text: str):
            with send_lock:
                self._send_frame(conn, 0x1, text.encode("utf-8"))

        handshake_done = threading.Event()
        client_id = id(conn)
        player = threading.Thread(target=self._play, args=(send_text, handshake_done, alive, client_id), daemon=True)
        player.start()
        buf = b""
        last_ping = time.time()
        try:
            while alive.is_set():
                try:
                    chunk = conn.recv(65536)
                    if not chunk:
                        break
                    buf += chunk
                except socket.timeout:
                    pass
                except OSError:
                    break
                while True:
                    frame = self._parse_frame(buf)
                    if frame is None:
                        break
                    opcode, payload, buf = frame
                    if opcode == 0x8:
                        log("client sent close")
                        alive.clear()
                        break
                    elif opcode == 0x9:
                        with send_lock:
                            self._send_frame(conn, 0xA, payload)
                    elif opcode == 0x1:
                        for record in payload.decode("utf-8").split(RS):
                            if not record.strip():
                                continue
                            try:
                                msg = json.loads(record)
                            except json.JSONDecodeError:
                                continue
                            if "protocol" in msg and not handshake_done.is_set():
                                send_text("{}" + RS)
                                handshake_done.set()
                                log("signalr handshake complete")
                            elif msg.get("type") == 6:
                                send_text('{"type":6}' + RS)
                if time.time() - last_ping > 10:
                    last_ping = time.time()
                    try:
                        send_text('{"type":6}' + RS)
                    except OSError:
                        break
        finally:
            alive.clear()
            with self._clients_lock:
                self._clients.pop(client_id, None)
            log("websocket client gone")

    @staticmethod
    def _send_frame(conn, opcode: int, payload: bytes):
        head = bytes([0x80 | opcode])
        n = len(payload)
        if n < 126:
            head += bytes([n])
        elif n < 65536:
            head += bytes([126]) + struct.pack(">H", n)
        else:
            head += bytes([127]) + struct.pack(">Q", n)
        conn.sendall(head + payload)

    @staticmethod
    def _parse_frame(buf: bytes):
        if len(buf) < 2:
            return None
        b0, b1 = buf[0], buf[1]
        opcode = b0 & 0x0F
        masked = b1 & 0x80
        n = b1 & 0x7F
        idx = 2
        if n == 126:
            if len(buf) < 4:
                return None
            n = struct.unpack(">H", buf[2:4])[0]
            idx = 4
        elif n == 127:
            if len(buf) < 10:
                return None
            n = struct.unpack(">Q", buf[2:10])[0]
            idx = 10
        mask = b""
        if masked:
            if len(buf) < idx + 4:
                return None
            mask = buf[idx:idx + 4]
            idx += 4
        if len(buf) < idx + n:
            return None
        payload = buf[idx:idx + n]
        if masked:
            payload = bytes(payload[i] ^ mask[i % 4] for i in range(n))
        return opcode, payload, buf[idx + n:]

    # -- playback -----------------------------------------------------------
    def _play(self, send_text, handshake_done: threading.Event, alive: threading.Event, client_id: int = 0):
        if not handshake_done.wait(15):
            log("no handshake; not playing")
            return
        with self._clients_lock:
            self._clients[client_id] = send_text
        if not self.mapper_fetched.wait(15):
            log("client never fetched the mapper; not playing")
            return
        if self.shared_playback:
            # one broadcast run, started by the first ready client and outliving it
            with self._clients_lock:
                if self._played:
                    log("shared playback already running; this client joins it")
                    return
                self._played = True

            def broadcast(text: str):
                with self._clients_lock:
                    targets = list(self._clients.items())
                for cid, send in targets:
                    try:
                        send(text)
                    except OSError:
                        with self._clients_lock:
                            self._clients.pop(cid, None)

            always = threading.Event()
            always.set()
            threading.Thread(target=self._play_steps, args=(broadcast, always), daemon=True).start()
            return
        if self._played and not self.loop:
            log("scenario already played to an earlier client; idle")
            return
        self._played = True
        self._play_steps(send_text, alive)

    def _play_steps(self, send_text, alive: threading.Event):
        log(f"starting playback in {self.start_delay:.1f}s")
        time.sleep(self.start_delay)
        for step in self.scenario.steps:
            if not alive.is_set():
                return
            kind = step[0]
            if kind == "wait":
                time.sleep(step[1])
            elif kind == "note":
                log(f"--- {step[1]}")
            elif kind == "set":
                changes, delay = step[1], step[2]
                batch = []
                with self.lock:
                    for p, v in changes.items():
                        self.values[p] = v
                        batch.append({"path": p, "address": self._addresses[p], "value": v, "bytes": self._bytes_of(v), "frozen": False, "fieldsChanged": ["value", "bytes"]})
                msg = {"type": 1, "target": "PropertiesChanged", "arguments": [batch]}
                try:
                    send_text(json.dumps(msg) + RS)
                except OSError:
                    return
                time.sleep(delay)
        log("scenario playback finished")
        self.playback_done.set()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8099)
    ap.add_argument("--scenario", required=True)
    ap.add_argument("--start-delay", type=float, default=1.5)
    ap.add_argument("--loop", action="store_true", help="replay the scenario to every client (default: only the first)")
    ap.add_argument("--shared-playback", action="store_true", help="play the scenario once, to whichever clients are connected as it goes")
    args = ap.parse_args()
    scenario = load_scenario(args.scenario)
    MockGameHook(args.port, scenario, start_delay=args.start_delay, loop=args.loop, shared_playback=args.shared_playback).serve_forever()


if __name__ == "__main__":
    main()
