"""Minimal reader of MATLAB level-5 MAT files (numeric arrays only), from the published file format
(a 128-byte header, then data elements: tag of 8 bytes or a small element of 4 + 4; miCOMPRESSED = 15 holds a
zlib stream of one element; miMATRIX = 14 holds array flags, dimensions, name, real part). Standard library only."""
import struct
import sys
import zlib

MI = {1: ('b', 1), 2: ('B', 1), 3: ('h', 2), 4: ('H', 2), 5: ('i', 4), 6: ('I', 4), 7: ('f', 4), 9: ('d', 8), 12: ('q', 8), 13: ('Q', 8)}


def _tag(buf, pos, e):
    a, = struct.unpack_from(e + 'I', buf, pos)
    if a >> 16:  # small data element: the two upper bytes are the byte count
        return a & 0xFFFF, a >> 16, pos + 4, pos + 8
    n, = struct.unpack_from(e + 'I', buf, pos + 4)
    return a, n, pos + 8, pos + 8 + ((n + 7) // 8) * 8


def _num(buf, pos, e):
    t, n, d, nxt = _tag(buf, pos, e)
    c, w = MI[t]
    return list(struct.unpack_from(e + c * (n // w), buf, d)), nxt


def _matrix(buf, e):
    pos = 0
    flags, pos = _num(buf, pos, e)
    cls = flags[0] & 0xFF
    dims, pos = _num(buf, pos, e)
    t, n, d, pos = _tag(buf, pos, e)
    name = bytes(buf[d:d + n]).decode('ascii')
    if cls < 6 or cls > 15:
        return name, dims, None  # not a numeric array
    vals, pos = _num(buf, pos, e)
    return name, dims, vals


def read(path):
    """-> {name: (dims, values in column-major order)}"""
    raw = open(path, 'rb').read()
    e = '<' if raw[126:128] == b'IM' else '>'
    out, pos = {}, 128
    while pos < len(raw):
        t, n, d, _ = _tag(raw, pos, e)
        body = raw[d:d + n]
        pos = d + n if t == 15 else d + ((n + 7) // 8) * 8
        if t == 15:
            body = zlib.decompress(body)
            t, n, d2, _ = _tag(body, 0, e)
            body = body[d2:d2 + n]
        if t == 14:
            name, dims, vals = _matrix(body, e)
            out[name] = (dims, vals)
    return out


if __name__ == '__main__':
    for p in sys.argv[1:]:
        print(p)
        for k, (dims, v) in read(p).items():
            print('  ', k, dims, None if v is None else (len(v), min(v), max(v)))
