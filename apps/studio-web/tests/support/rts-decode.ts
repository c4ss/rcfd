// An independent decoder of one field of a `store.rts` (F-815's tests): written from the format comment of
// crates/rcfd-tsstore/src/lib.rs, sharing no code with the Studio's reader (src/main/data/tsstore.ts), so that the
// difference field can be checked against an f64 computation made straight from the stored bytes (ADR 0801, addendum
// F-815, C3). It walks the frames from the header on, takes the SNAP directories in order, and decodes the DATA blocks
// of one field (raw little-endian, or byte-shuffled then raw-deflated). No checksum is checked: the reader's own tests
// do that; this only has to read the same numbers by another path.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

interface Dir {
  readonly step: number;
  readonly time: number;
  readonly fields: readonly {
    readonly name: string;
    readonly unit: string;
    readonly location: string;
    readonly precision: 'f32' | 'f64';
    readonly codec: 'raw' | 'shuffle_deflate';
    readonly n: number;
    readonly block_values: number;
    readonly blocks: readonly number[];
  }[];
}

const HEADER = 20;
const HEAD = 28;

function frames(buf: Buffer): { kind: string; at: number; len: number }[] {
  const out: { kind: string; at: number; len: number }[] = [];
  let at = HEADER;
  while (at + HEAD <= buf.length) {
    const kind = buf.toString('latin1', at, at + 4);
    if (!['MESH', 'DATA', 'SNAP', 'FOOT'].includes(kind)) break;
    const len = Number(buf.readBigUInt64LE(at + 4));
    if (at + HEAD + len > buf.length) break;
    out.push({ kind, at, len });
    at += HEAD + len;
  }
  return out;
}

/** The snapshot directories of the store in `dir`, in order. */
export function snapshotDirs(dir: string): Dir[] {
  const buf = readFileSync(path.join(dir, 'store.rts'));
  return frames(buf)
    .filter((f) => f.kind === 'SNAP')
    .map((f) => JSON.parse(buf.toString('utf8', f.at + HEAD, f.at + HEAD + f.len)) as Dir);
}

/** Every value of field `name` of snapshot `i` as f64 (an f32 value widened), decoded from the stored bytes. */
export function decodeField64(dir: string, i: number, name: string): Float64Array {
  const buf = readFileSync(path.join(dir, 'store.rts'));
  const snaps = frames(buf).filter((f) => f.kind === 'SNAP');
  const d = JSON.parse(buf.toString('utf8', snaps[i]!.at + HEAD, snaps[i]!.at + HEAD + snaps[i]!.len)) as Dir;
  const f = d.fields.find((x) => x.name === name);
  if (!f) throw new Error(`snapshot ${i} has no field ${name}`);
  const w = f.precision === 'f32' ? 4 : 8;
  const out = new Float64Array(f.n);
  f.blocks.forEach((off, b) => {
    const count = Math.min(f.block_values, f.n - b * f.block_values);
    const len = Number(buf.readBigUInt64LE(off + 4));
    let bytes = Buffer.from(buf.subarray(off + HEAD, off + HEAD + len));
    if (f.codec === 'shuffle_deflate') {
      const s = zlib.inflateRawSync(bytes);
      const r = Buffer.alloc(count * w);
      for (let j = 0; j < w; j++) for (let k = 0; k < count; k++) r[k * w + j] = s[j * count + k]!;
      bytes = r;
    }
    for (let k = 0; k < count; k++) out[b * f.block_values + k] = w === 4 ? bytes.readFloatLE(4 * k) : bytes.readDoubleLE(8 * k);
  });
  return out;
}

/** The bits of an f64, as 16 hexadecimal digits (to compare two values bit for bit, NaN and -0 included). */
export function bits64(v: number): string {
  const b = Buffer.alloc(8);
  b.writeDoubleLE(v);
  return b.readBigUInt64LE().toString(16).padStart(16, '0');
}
