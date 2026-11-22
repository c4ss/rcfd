"""ADR 0769 (F-883): the time means of the wall-pressure rows of the FAN-01 record.

Reads `mic_kulite/mic_kulite_n1ug_1_4.mat` of `mic_kulite.tar` (Zenodo doi:10.5281/zenodo.23943494, v6) with a reader
of the MATLAB level-5 format written from the published file format (128-byte header; data elements with an 8-byte
tag or a 4 + 4 small element; miCOMPRESSED = 15 holds a zlib stream of one element; miMATRIX = 14 holds array flags,
dimensions, name and the real part); standard library only (the numeric part through `array`). Writes, for each row of the wall-pressure
variable, the mean of all samples, the standard deviation about it, the root of the mean square and the means of 30
consecutive batches of 48 000 samples, as registered in ADR 0769.

Usage: python -I kulite_means.py <path of the .mat> <path of the tar> <output csv>   (or --list <path of the .mat>)
"""
import array
import hashlib
import math
import struct
import sys
import zlib

DT = {1: 'b', 2: 'B', 3: 'h', 4: 'H', 5: 'i', 6: 'I', 7: 'f', 9: 'd', 12: 'q', 13: 'Q'}
BATCH, NBATCH = 48000, 30
X_MM = {n: 10 * n - 33 for n in range(1, 15)}  # Table 4 of the description: x = 10 n - 33 mm, r = 250 mm


def tag(buf, pos):
    a, = struct.unpack_from('<I', buf, pos)
    if a >> 16:
        return a & 0xFFFF, a >> 16, pos + 4, pos + 8
    n, = struct.unpack_from('<I', buf, pos + 4)
    return a, n, pos + 8, pos + 8 + ((n + 7) // 8) * 8


def num(buf, pos):
    t, n, d, nxt = tag(buf, pos)
    a = array.array(DT[t])
    a.frombytes(bytes(buf[d:d + n]))  # little-endian file on a little-endian machine (checked in read)
    return a, nxt


def matrix(buf, load):
    flags, pos = num(buf, 0)
    cls = int(flags[0]) & 0xFF
    dims, pos = num(buf, pos)
    t, n, d, pos = tag(buf, pos)
    name = bytes(buf[d:d + n]).decode('ascii')
    if not load or cls < 6 or cls > 15:
        return name, cls, [int(x) for x in dims], None
    vals, _ = num(buf, pos)
    return name, cls, [int(x) for x in dims], vals


def read(path):
    raw = memoryview(open(path, 'rb').read())
    if bytes(raw[126:128]) != b'IM' or sys.byteorder != 'little':
        raise SystemExit('not a little-endian MAT level-5 file')
    out, pos = {}, 128
    while pos < len(raw):
        t, n, d, _ = tag(raw, pos)
        body = raw[d:d + n]
        pos = d + n if t == 15 else d + ((n + 7) // 8) * 8
        if t == 15:
            body = memoryview(zlib.decompress(body))
            t, n, d2, _ = tag(body, 0)
            body = body[d2:d2 + n]
        if t == 14:
            name, cls, dims, vals = matrix(body, True)
            out[name] = (cls, dims, vals)
    return out


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 22), b''):
            h.update(chunk)
    return h.hexdigest()


def main():
    if sys.argv[1] == '--list':
        for k, (cls, dims, v) in read(sys.argv[2]).items():
            print(k, 'class', cls, 'dims', dims, '' if v is None else 'values %d first %r' % (len(v), v[:3].tolist()))
        return
    mat, tar, out = sys.argv[1:4]
    m = read(mat)
    cls, dims, v = m['kulite_pressure_in_Pa']
    if cls != 6 or len(dims) != 2 or dims[1] != 15 or len(v) != dims[0] * dims[1]:
        raise SystemExit('unexpected class or shape %r %r' % (cls, dims))
    n = dims[0]  # MAT stores (sample, channel), column-major: channel i is v[i n : (i + 1) n]
    lines = [
        '# FAN-01 wall pressure at the design point (1.4 m^3/s): time means of kulite_pressure_in_Pa ("wall pressure, differential',
        '# to the chamber pressure, mean value included", description Appendix A.2), transcribed for ADR 0769 (F-883) from',
        '# mic_kulite/mic_kulite_n1ug_1_4.mat of mic_kulite.tar of the Zenodo record "FAN-01 Benchmark: Low-pressure axial fan',
        '# in a short duct" (v6), doi:10.5281/zenodo.23943494, Kroemer, Junger, Czwielong, Heidegger, Kaltenbacher, Schoder, Becker,',
        '# licence CC BY 4.0, fetched 2026-11-22. tar sha256 %s; .mat sha256 %s.' % (sha256(tar), sha256(mat)),
        '# x_mm = 10 n - 33 (Table 4; r = 250 mm, the casing; row n = transducer n, inferred by the record); row 0 holds no signal.',
        '# mean: of all n samples; std: about the mean; rms: root of the mean square; b01..b30: means of 30 batches of 48000 samples.',
        '# Pa throughout. Written by kulite_means.py.',
        'row,x_mm,n,mean,std,rms,' + ','.join('b%02d' % (k + 1) for k in range(NBATCH)),
    ]
    for i in range(15):
        s = v[i * n:(i + 1) * n]
        mean = math.fsum(s) / n
        std = math.sqrt(math.fsum((x - mean) ** 2 for x in s) / n)
        rms = math.sqrt(math.fsum(x * x for x in s) / n)
        b = [math.fsum(s[k * BATCH:(k + 1) * BATCH]) / BATCH for k in range(NBATCH)]
        cells = [str(i), str(X_MM[i]) if i else '', str(n)] + ['%.6f' % x for x in [mean, std, rms] + b]
        lines.append(','.join(cells))
    with open(out, 'w', encoding='utf-8', newline='\n') as f:
        f.write('\n'.join(lines) + '\n')


if __name__ == '__main__':
    main()
