// @ts-check
'use strict';
// Route R2 of ADR 0350 (F-447): a reader of the time-series field store of ADR 0124 (`store.rts`) for Node.
// Layout (crates/rcfd-tsstore/src/lib.rs):
//   header   "RCFDTSS\0" | u32 version | u64 fnv64(previous 12 bytes)                      20 bytes
//   frame    kind[4] | u64 len | u64 fnv64(payload) | u64 fnv64(previous 20 bytes) | payload
//   trailer  u64 FOOT offset | "RTSE" | u64 fnv64(previous 12 bytes)                       20 bytes
// The reader needs the footer (a finished store); it verifies the header, every frame head it reads and the payload
// checksum of every SNAP, FOOT and DATA frame. The MESH payload (hundreds of megabytes) is read in parts and its
// checksum is not verified (ADR 0350 amendment 1, A1.3).

const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');

const HEADER_LEN = 20;
const HEAD_LEN = 28;
const TRAILER_LEN = 20;

/**
 * FNV-1a, 64 bit, as two unsigned 32-bit halves [low, high] (offset basis cbf29ce484222325, prime 100000001b3).
 * @param {Uint8Array} bytes
 * @returns {[number, number]}
 */
function fnv64(bytes) {
  let lo = 0x84222325 | 0;
  let hi = 0xcbf29ce4 | 0;
  for (let i = 0; i < bytes.length; i++) {
    lo ^= bytes[i];
    // (hi, lo) * (2^40 + 0x1b3) modulo 2^64
    const p0 = (lo & 0xffff) * 0x1b3;
    const p1 = (lo >>> 16) * 0x1b3;
    const carry = ((p0 >>> 16) + p1) >>> 16;
    hi = (Math.imul(hi, 0x1b3) + carry + (lo << 8)) | 0;
    lo = Math.imul(lo, 0x1b3);
  }
  return [lo >>> 0, hi >>> 0];
}

/**
 * @param {Uint8Array} b
 * @param {number} off
 */
function u32(b, off) {
  return (b[off] | (b[off + 1] << 8) | (b[off + 2] << 16) | (b[off + 3] << 24)) >>> 0;
}

/**
 * @param {Uint8Array} b
 * @param {number} off
 */
function u64(b, off) {
  return u32(b, off + 4) * 4294967296 + u32(b, off);
}

/**
 * @param {Uint8Array} b
 * @param {number} off
 * @param {[number, number]} sum
 */
function sumIs(b, off, sum) {
  return u32(b, off) === sum[0] && u32(b, off + 4) === sum[1];
}

/** @param {string} msg */
function bad(msg) {
  return new Error(`tsstore: ${msg}`);
}

class Store {
  /** @param {string} dir the fields folder holding `store.rts` */
  constructor(dir) {
    this.fd = fs.openSync(path.join(dir, 'store.rts'), 'r');
    this.len = fs.fstatSync(this.fd).size;
    /** Test hook of mutation M3 (never set by the bench). */
    this.wrongStride = false;
    const h = this.read(0, HEADER_LEN);
    if (Buffer.from(h.subarray(0, 8)).toString('latin1') !== 'RCFDTSS\0') throw bad('bad magic');
    if (!sumIs(h, 12, fnv64(h.subarray(0, 12)))) throw bad('header checksum mismatch');
    if (u32(h, 8) !== 1) throw bad(`unsupported store version ${u32(h, 8)}`);
    const mesh = this.head(HEADER_LEN, 'MESH');
    const p = this.read(HEADER_LEN + HEAD_LEN, 24);
    this.meshLen = mesh.len;
    this.meshEnd = HEADER_LEN + HEAD_LEN + mesh.len;
    this.nCells = u64(p, 8);
    this.nPoints = u64(p, 16);
    const t = this.read(this.len - TRAILER_LEN, TRAILER_LEN);
    if (Buffer.from(t.subarray(8, 12)).toString('latin1') !== 'RTSE' || !sumIs(t, 12, fnv64(t.subarray(0, 12)))) {
      throw bad('no valid trailer (the spike reads finished stores only)');
    }
    const foot = JSON.parse(Buffer.from(this.payload(u64(t, 0), 'FOOT')).toString('utf8'));
    /** @type {{step: number, time: number, offset: number, end: number}[]} */
    this.snaps = foot.snapshots.map((/** @type {number[]} */ s) => ({ step: s[0], time: s[1], offset: s[2], end: s[3] }));
  }

  /**
   * @param {number} off
   * @param {number} n
   * @returns {Uint8Array} a fresh array (its buffer starts at the data)
   */
  read(off, n) {
    const out = new Uint8Array(n);
    let got = 0;
    while (got < n) {
      const k = fs.readSync(this.fd, out, got, n - got, off + got);
      if (k <= 0) throw bad(`short read at offset ${off}`);
      got += k;
    }
    return out;
  }

  /**
   * A verified frame head.
   * @param {number} off
   * @param {string} kind
   */
  head(off, kind) {
    const h = this.read(off, HEAD_LEN);
    if (!sumIs(h, 20, fnv64(h.subarray(0, 20)))) throw bad(`frame head checksum mismatch at offset ${off}`);
    if (Buffer.from(h.subarray(0, 4)).toString('latin1') !== kind) throw bad(`expected a ${kind} frame at offset ${off}`);
    return { len: u64(h, 4), sumLo: u32(h, 12), sumHi: u32(h, 16) };
  }

  /**
   * A verified payload.
   * @param {number} off
   * @param {string} kind
   */
  payload(off, kind) {
    const h = this.head(off, kind);
    const p = this.read(off + HEAD_LEN, h.len);
    const s = fnv64(p);
    if (s[0] !== h.sumLo || s[1] !== h.sumHi) throw bad(`payload checksum mismatch in frame at offset ${off}`);
    return p;
  }

  /**
   * The directory of snapshot `i`.
   * @param {number} i
   * @returns {{step: number, time: number, fields: {name: string, unit: string, location: string, n: number,
   *   precision: string, codec: string, block_values: number, blocks: number[]}[]}}
   */
  dir(i) {
    const s = this.snaps[i];
    if (!s) throw bad(`no snapshot ${i} (the store has ${this.snaps.length})`);
    return JSON.parse(Buffer.from(this.payload(s.offset, 'SNAP')).toString('utf8'));
  }

  /**
   * Field `name` of snapshot `i` as `f32` (an `f64` field is rounded to the nearest single, as `as f32` does).
   * @param {number} i
   * @param {string} name
   * @returns {ArrayBuffer}
   */
  field(i, name) {
    const f = this.dir(i).fields.find((x) => x.name === name);
    if (!f) throw bad(`snapshot ${i} has no field '${name}'`);
    const w = f.precision === 'f32' ? 4 : 8;
    const out = new Float32Array(f.n);
    const outBytes = new Uint8Array(out.buffer);
    for (let b = 0; b < f.blocks.length; b++) {
      const count = Math.min(f.block_values, f.n - b * f.block_values);
      let p = this.payload(f.blocks[b], 'DATA');
      if (f.codec === 'shuffle_deflate') {
        const s = zlib.inflateRawSync(p);
        if (s.length !== count * w) throw bad('deflate block has the wrong length');
        const r = new Uint8Array(count * w);
        const stride = this.wrongStride ? count + 1 : count;
        for (let j = 0; j < w; j++) {
          const base = j * stride;
          for (let k = 0; k < count; k++) r[k * w + j] = s[base + k];
        }
        p = r;
      } else if (f.codec !== 'raw') {
        throw bad(`unknown codec ${f.codec}`);
      }
      if (p.length !== count * w) throw bad('data block has the wrong length');
      if (w === 4) {
        outBytes.set(p, 4 * b * f.block_values);
      } else {
        out.set(new Float64Array(p.buffer, p.byteOffset, count), b * f.block_values);
      }
    }
    return out.buffer;
  }

  /**
   * The boundary faces of the stored mesh as triangles (a fan per face), as `data.surface` gives them: points `f32`,
   * triangle corners `u32`, owner cell of each triangle `u32`. Mesh payload (crates/rcfd-io/src/container.rs):
   * u64 hash | u64 cells | u64 points | u64 json length | json | points (3 f64 each) | face_offsets (u64, faces + 1) |
   * face_points (u64) | owner (u64, faces) | neighbour (u64, internal faces). Boundary faces come after the internal ones.
   */
  surface() {
    const base = HEADER_LEN + HEAD_LEN + 24;
    const jl = u64(this.read(base, 8), 0);
    const meta = JSON.parse(Buffer.from(this.read(base + 8, jl)).toString('utf8'));
    const pointsAt = base + 8 + jl;
    const offsetsAt = pointsAt + 24 * meta.n_points;
    const facePointsAt = offsetsAt + 8 * (meta.n_faces + 1);
    const ownerAt = facePointsAt + 8 * meta.n_face_points;
    const first = meta.n_neighbour;
    const nb = meta.n_faces - first;
    /** the low words of little-endian u64 values (the high words must be zero)
     * @param {number} at
     * @param {number} n */
    const words = (at, n) => {
      const w = new Uint32Array(this.read(at, 8 * n).buffer);
      const out = new Uint32Array(n);
      for (let i = 0; i < n; i++) {
        if (w[2 * i + 1] !== 0) throw bad('index beyond 2^32');
        out[i] = w[2 * i];
      }
      return out;
    };
    const xyz = new Float64Array(this.read(pointsAt, 24 * meta.n_points).buffer);
    const offsets = words(offsetsAt + 8 * first, nb + 1);
    const facePoints = words(facePointsAt + 8 * offsets[0], offsets[nb] - offsets[0]);
    const owner = words(ownerAt + 8 * first, nb);
    const newId = new Uint32Array(meta.n_points).fill(0xffffffff);
    const nTri = offsets[nb] - offsets[0] - 2 * nb;
    const triangles = new Uint32Array(3 * nTri);
    const cells = new Uint32Array(nTri);
    /** @type {number[]} */
    const used = [];
    let t = 0;
    for (let f = 0; f < nb; f++) {
      const a = offsets[f] - offsets[0];
      const n = offsets[f + 1] - offsets[f];
      for (let k = 0; k < n; k++) {
        const c = facePoints[a + k];
        if (newId[c] === 0xffffffff) {
          newId[c] = used.length;
          used.push(c);
        }
      }
      for (let k = 1; k + 1 < n; k++) {
        triangles[3 * t] = newId[facePoints[a]];
        triangles[3 * t + 1] = newId[facePoints[a + k]];
        triangles[3 * t + 2] = newId[facePoints[a + k + 1]];
        cells[t] = owner[f];
        t++;
      }
    }
    const points = new Float32Array(3 * used.length);
    for (let i = 0; i < used.length; i++) {
      points[3 * i] = xyz[3 * used[i]];
      points[3 * i + 1] = xyz[3 * used[i] + 1];
      points[3 * i + 2] = xyz[3 * used[i] + 2];
    }
    return { points: points.buffer, triangles: triangles.buffer, cells: cells.buffer };
  }

  close() {
    fs.closeSync(this.fd);
  }
}

module.exports = { Store, fnv64 };
