"""Gen 5 memory decoding helpers for traces made by shuckie-feeder /trace."""
import re
import struct
import sys
import xml.etree.ElementTree as ET

MAPPERS = r"C:\Users\scott\AppData\Roaming\PokeAByte\mappers\STANDARD\gen5"

_refs_cache = {}


def refs(game="black"):
    if game in _refs_cache:
        return _refs_cache[game]
    txt = open(f"{MAPPERS}\\pokemon_{game}.xml", encoding="utf-8").read()
    root = ET.fromstring(txt.encode("utf-8"))
    out = {}
    for table in root.find("references"):
        d = {}
        for e in table.findall("entry"):
            k = e.get("key")
            if k is None:
                continue
            try:
                k = int(k, 0)
            except ValueError:
                continue
            d[k] = e.get("value")
        out[table.tag] = d
    _refs_cache[game] = out
    return out


SHUFFLE = ["ABCD", "ABDC", "ACBD", "ACDB", "ADBC", "ADCB", "BACD", "BADC", "BCAD", "BCDA", "BDAC", "BDCA",
           "CABD", "CADB", "CBAD", "CBDA", "CDAB", "CDBA", "DABC", "DACB", "DBAC", "DBCA", "DCAB", "DCBA"]


def _crypt(data, seed):
    out = bytearray(data)
    for i in range(0, len(out), 2):
        seed = (seed * 0x41C64E6D + 0x6073) & 0xFFFFFFFF
        w = struct.unpack_from("<H", out, i)[0] ^ (seed >> 16)
        struct.pack_into("<H", out, i, w)
    return bytes(out)


def decrypt_pkm(raw):
    """220-byte encrypted party Pokémon -> 220-byte decrypted, unshuffled."""
    if len(raw) < 220 or raw == bytes(len(raw)):
        return None
    pid, flags, checksum = struct.unpack_from("<IHH", raw, 0)
    blocks = _crypt(raw[8:136], checksum)
    sv = ((pid & 0x3E000) >> 13) % 24
    order = SHUFFLE[sv]
    unshuffled = b"".join(blocks[order.index(x) * 32:(order.index(x) + 1) * 32] for x in "ABCD")
    party = _crypt(raw[136:220], pid)
    return raw[:8] + unshuffled + party


def pkm_info(raw, game="black"):
    d = decrypt_pkm(raw)
    if d is None:
        return None
    r = refs(game)
    species, held, otid, sid, exp = struct.unpack_from("<HHHHI", d, 8)
    friendship, ability = d[0x14], d[0x15]
    evs = list(d[0x18:0x1E])
    moves = struct.unpack_from("<4H", d, 0x28)
    ivs_raw = struct.unpack_from("<I", d, 0x38)[0]
    ivs = [(ivs_raw >> (5 * i)) & 31 for i in range(6)]
    nature = d[0x41]
    status = struct.unpack_from("<I", d, 0x88)[0]
    level = d[0x8C]
    hp, maxhp = struct.unpack_from("<HH", d, 0x8E)
    return {
        "pid": struct.unpack_from("<I", d, 0)[0],
        "species": r["species"].get(species, species),
        "held": r["items"].get(held, held) if held else None,
        "exp": exp, "level": level, "hp": hp, "maxhp": maxhp,
        "moves": [r["moves"].get(m, m) for m in moves if m],
        "friendship": friendship, "evs": evs, "ivs": ivs, "nature": nature,
        "status": status,
    }


class Trace:
    """Replays a trace file: iterate (frame, {region: bytes}, changed_names)."""

    def __init__(self, path):
        self.path = path

    def __iter__(self):
        state = {}
        frame = None
        changed = set()
        with open(self.path, encoding="utf-8") as f:
            for line in f:
                line = line.rstrip("\n")
                if line.startswith("F "):
                    if frame is not None:
                        yield frame, state, changed
                    frame = int(line[2:])
                    changed = set()
                    continue
                parts = line.split(" ")
                name = parts[0]
                buf = state.get(name)
                for p in parts[1:]:
                    off, hx = p.split(":")
                    off = int(off)
                    data = bytes.fromhex(hx)
                    if buf is None or (off == 0 and len(data) != len(buf) and buf is not None and False):
                        buf = bytearray(data)
                    else:
                        if len(buf) < off + len(data):
                            buf.extend(bytes(off + len(data) - len(buf)))
                        buf[off:off + len(data)] = data
                state[name] = buf
                changed.add(name)
        if frame is not None:
            yield frame, state, changed


def u16(b, o):
    return struct.unpack_from("<H", b, o)[0]


def u32(b, o):
    return struct.unpack_from("<I", b, o)[0]


POCKETS = {
    "black": (0x2233FAC, {"items": (0x2233FAC, 310), "key": (0x2234484, 83), "tmhm": (0x22345D0, 109), "medicine": (0x2234784, 48), "berries": (0x2234844, 64)}),
    "white_2": (0x221DA24, {"items": (0x221DA24, 310), "key": (0x221DEFC, 83), "tmhm": (0x221E048, 109), "medicine": (0x221E1FC, 48), "berries": (0x221E2BC, 64)}),
}


def bag_items(bag, base=None, game="black"):
    """{pocket: [(item, qty)]} from the bag region (starting at the items pocket)."""
    r = refs(game)
    base0, pockets = POCKETS[game]
    base = base0 if base is None else base
    out = {}
    for name, (addr, n) in pockets.items():
        o = addr - base
        lst = []
        for i in range(n):
            if o + 4 * i + 4 > len(bag):
                break
            item, qty = struct.unpack_from("<HH", bag, o + 4 * i)
            if item:
                lst.append((r["items"].get(item, item), qty))
        out[name] = lst
    return out


def fmt_time(t):
    return "%d:%02d:%02d" % (u16(t, 0), t[2], t[3])
