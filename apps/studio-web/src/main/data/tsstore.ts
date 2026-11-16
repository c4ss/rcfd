// Route R2 of ADR 0350 (F-804): a reader of the time-series field store of ADR 0124 (`store.rts`) for Node, ported from
// the spike's bench/electron/io/tsstore.js and brought to the checks of the Rust reader (crates/rcfd-tsstore/src/lib.rs,
// `Reader`). It is synchronous and seek-based; the Studio runs it in a worker thread (worker.ts), never on the main
// process's thread (G21). No Electron import: the unit tests run it in Node.
//
// Layout (crates/rcfd-tsstore/src/lib.rs):
//   header   "RCFDTSS\0" | u32 version | u64 fnv64(previous 12 bytes)                      20 bytes
//   frame    kind[4] | u64 len | u64 fnv64(payload) | u64 fnv64(previous 20 bytes) | payload
//              MESH  u64 mesh_hash | u64 n_cells | u64 n_points | rcfd_io::container::mesh_to_bytes
//              DATA  one block of one field (raw little-endian, or byte-shuffled then deflated)
//              SNAP  JSON directory of one snapshot: step, time, fields (unit, location, block offsets)
//              FOOT  JSON index of all snapshots (written by `Writer::finish` only)
//   trailer  u64 FOOT offset | "RTSE" | u64 fnv64(previous 12 bytes)                       20 bytes
//
// What the Rust reader does, this one does: the header, every frame head it reads, the payload checksum of every SNAP,
// FOOT and DATA frame; the footer index when it is valid, else a lenient scan of the frame heads (a live or interrupted
// run; `refresh` picks up snapshots committed since), with the same consistency checks of the footer and of each
// snapshot directory; the same size limits of JSON and data frames. Beyond the spike: the scan of an unfinished store,
// and the payload checksum of the MESH frame before `surface` uses it (ADR 0350 addendum 1 named both as missing).
// `surface` decodes the mesh frame's arrays only as far as the boundary faces need them; it does not rebuild the mesh's
// content hash (`Reader::mesh` does: the bytes are covered by the checksum, the hash would need the mesh code of Rust).
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

export const STORE_FILE = 'store.rts';
export const VERSION = 1;
export const HEADER_LEN = 20;
export const FRAME_HEAD_LEN = 28;
export const TRAILER_LEN = 20;
const MAX_BLOCK_VALUES = 1 << 24;
const MAX_JSON_FRAME = 2 ** 28;
/** Chunk of the streamed MESH checksum. */
const MESH_CHUNK = 1 << 24;

export class StoreError extends Error {
  constructor(message: string) {
    super(`tsstore: ${message}`);
    this.name = 'StoreError';
  }
}

const bad = (msg: string): StoreError => new StoreError(msg);

/** FNV-1a 64 state as two unsigned 32-bit halves [low, high]. */
export type Fnv = [number, number];
/** Offset basis cbf29ce484222325. */
export const FNV_BASIS: Readonly<Fnv> = [0x84222325, 0xcbf29ce4];

/** Continues FNV-1a 64 (prime 100000001b3 = 2^40 + 0x1b3) over `bytes` from `state`. */
export function fnv64Update(state: Readonly<Fnv>, bytes: Uint8Array): Fnv {
  let lo = state[0] | 0;
  let hi = state[1] | 0;
  for (let i = 0; i < bytes.length; i++) {
    lo ^= bytes[i]!;
    // (hi, lo) * (2^40 + 0x1b3) modulo 2^64
    const p0 = (lo & 0xffff) * 0x1b3;
    const p1 = (lo >>> 16) * 0x1b3;
    const carry = ((p0 >>> 16) + p1) >>> 16;
    hi = (Math.imul(hi, 0x1b3) + carry + (lo << 8)) | 0;
    lo = Math.imul(lo, 0x1b3);
  }
  return [lo >>> 0, hi >>> 0];
}

export const fnv64 = (bytes: Uint8Array): Fnv => fnv64Update(FNV_BASIS, bytes);

const u32 = (b: Uint8Array, off: number): number => (b[off]! | (b[off + 1]! << 8) | (b[off + 2]! << 16) | (b[off + 3]! << 24)) >>> 0;

/** A little-endian u64 that must fit a JavaScript number exactly (below 2^53; offsets and counts always do). */
function u64(b: Uint8Array, off: number): number {
  const hi = u32(b, off + 4);
  if (hi >= 0x200000) throw bad(`a 64-bit value at byte ${off} is beyond 2^53`);
  return hi * 4294967296 + u32(b, off);
}

const sumIs = (b: Uint8Array, off: number, s: Readonly<Fnv>): boolean => u32(b, off) === s[0] && u32(b, off + 4) === s[1];
const latin1 = (b: Uint8Array): string => Buffer.from(b.buffer, b.byteOffset, b.byteLength).toString('latin1');
const hex16 = (b: Uint8Array, off: number): string => {
  let s = '';
  for (let i = 7; i >= 0; i--) s += b[off + i]!.toString(16).padStart(2, '0');
  return s;
};

interface Head {
  readonly kind: string;
  readonly len: number;
  readonly sum: Fnv;
}

type Found = { readonly t: 'head'; readonly head: Head } | { readonly t: 'short' } | { readonly t: 'damaged' };

export interface StoreFieldInfo {
  readonly name: string;
  readonly unit: string;
  readonly location: 'cell' | 'point';
  readonly n: number;
  readonly precision: 'f32' | 'f64';
  readonly codec: 'raw' | 'shuffle_deflate';
  readonly block_values: number;
  readonly blocks: readonly number[];
}

interface SnapDir {
  readonly step: number;
  readonly time: number;
  readonly fields: readonly StoreFieldInfo[];
}

export interface SnapInfo {
  readonly step: number;
  readonly time: number;
  readonly offset: number;
  readonly end: number;
}

interface MeshPrefix {
  readonly end: number;
  readonly hash: string;
  readonly nCells: number;
  readonly nPoints: number;
}

export type IndexSource = { readonly kind: 'footer' } | { readonly kind: 'scanned'; readonly reason: string; readonly tailBytes: number };

/** Test hooks (mutations of ADR 0350 A1.4); never set by the Studio. */
export interface ReaderHooks {
  /** M3: the unshuffle of a deflated block uses the wrong stride. */
  wrongStride?: boolean;
}

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const isCount = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;

/** A snapshot directory as serde would accept it into `SnapDir` (types and enum names), else a StoreError. */
function parseDir(text: string, where: string): SnapDir {
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch (e) {
    throw bad(`${where}: ${(e as Error).message}`);
  }
  const fail = (what: string): never => {
    throw bad(`${where}: ${what}`);
  };
  if (!isObj(v)) return fail('not an object');
  if (!isCount(v['step'])) fail('step is not an unsigned integer');
  if (typeof v['time'] !== 'number') fail('time is not a number');
  if (!Array.isArray(v['fields'])) fail('fields is not a list');
  const fields = (v['fields'] as unknown[]).map((f, i): StoreFieldInfo => {
    if (!isObj(f)) return fail(`field ${i} is not an object`);
    if (typeof f['name'] !== 'string' || typeof f['unit'] !== 'string') fail(`field ${i} has no name or unit`);
    if (f['location'] !== 'cell' && f['location'] !== 'point') fail(`field ${i} has an unknown location`);
    if (f['precision'] !== 'f32' && f['precision'] !== 'f64') fail(`field ${i} has an unknown precision`);
    if (f['codec'] !== 'raw' && f['codec'] !== 'shuffle_deflate') fail(`field ${i} has an unknown codec`);
    if (!isCount(f['n']) || !isCount(f['block_values'])) fail(`field ${i} has a bad size`);
    if (!Array.isArray(f['blocks']) || !(f['blocks'] as unknown[]).every(isCount)) fail(`field ${i} has a bad block list`);
    return f as unknown as StoreFieldInfo;
  });
  return { step: v['step'] as number, time: v['time'] as number, fields };
}

/** Random-access reader of one `store.rts` (seek-based; holds no field data between calls). */
export class Store {
  readonly dir: string;
  readonly file: string;
  private fd: number;
  private len: number;
  private readonly mesh: MeshPrefix;
  private snaps: SnapInfo[];
  private source: IndexSource;
  private readonly footerReason: string;
  private meshVerified = false;
  hooks: ReaderHooks = {};

  /** Opens `dir/store.rts`: the footer index when it is valid, otherwise a scan (as `Reader::open`). */
  constructor(dir: string) {
    this.dir = dir;
    this.file = path.join(dir, STORE_FILE);
    let fd: number;
    try {
      fd = fs.openSync(this.file, 'r');
    } catch (e) {
      throw bad(`cannot open ${this.file}: ${(e as NodeJS.ErrnoException).code ?? (e as Error).message}`);
    }
    this.fd = fd;
    try {
      this.len = fs.fstatSync(fd).size;
      this.checkHeader();
      this.mesh = this.meshPrefix();
      const footer = this.readFooter();
      if (typeof footer !== 'string') {
        this.snaps = footer;
        this.source = { kind: 'footer' };
        this.footerReason = '';
      } else {
        const s = this.scan(this.mesh.end, null);
        this.snaps = s.snaps;
        this.footerReason = footer;
        this.source = { kind: 'scanned', reason: scanReason(footer, s.stoppedAt), tailBytes: this.len - s.committedEnd };
      }
    } catch (e) {
      fs.closeSync(fd);
      throw e;
    }
  }

  get nCells(): number {
    return this.mesh.nCells;
  }
  get nPoints(): number {
    return this.mesh.nPoints;
  }
  /** The recorded mesh content hash, 16 hexadecimal digits (as `data.open` prints it). */
  get meshHash(): string {
    return this.mesh.hash;
  }
  get snapshots(): readonly SnapInfo[] {
    return this.snaps;
  }
  get indexSource(): IndexSource {
    return this.source;
  }
  /** File length the index was built on. */
  get length(): number {
    return this.len;
  }

  close(): void {
    if (this.fd >= 0) fs.closeSync(this.fd);
    this.fd = -1;
  }

  /**
   * Picks up snapshots a live writer committed since the last open or refresh (scanned index only, as
   * `Reader::refresh`); returns how many were added.
   */
  refresh(): number {
    if (this.source.kind === 'footer') return 0;
    const len = fs.fstatSync(this.fd).size;
    const last = this.snaps[this.snaps.length - 1] ?? null;
    const start = last ? last.end : this.mesh.end;
    const prevLen = this.len;
    this.len = len;
    let s: ScanOut;
    try {
      s = this.scan(start, last);
    } catch (e) {
      this.len = prevLen;
      throw e;
    }
    this.source = { kind: 'scanned', reason: scanReason(this.footerReason, s.stoppedAt), tailBytes: len - s.committedEnd };
    this.snaps.push(...s.snaps);
    return s.snaps.length;
  }

  // ------------------------------------------------------------ framing

  private read(off: number, n: number): Uint8Array {
    const out = new Uint8Array(n);
    let got = 0;
    while (got < n) {
      const k = fs.readSync(this.fd, out, got, n - got, off + got);
      if (k <= 0) throw bad(`short read at offset ${off + got}`);
      got += k;
    }
    return out;
  }

  private checkHeader(): void {
    if (this.len < HEADER_LEN) throw bad('file shorter than the header');
    const h = this.read(0, HEADER_LEN);
    if (latin1(h.subarray(0, 8)) !== 'RCFDTSS\0') throw bad('not an rcfd time-series store (bad magic)');
    if (!sumIs(h, 12, fnv64(h.subarray(0, 12)))) throw bad('header checksum mismatch');
    const v = u32(h, 8);
    if (v === 0 || v > VERSION) throw bad(`unsupported store version ${v} (this build reads up to ${VERSION})`);
  }

  private findHead(off: number, len: number): Found {
    if (len < off || len - off < FRAME_HEAD_LEN) return { t: 'short' };
    const h = this.read(off, FRAME_HEAD_LEN);
    if (!sumIs(h, 20, fnv64(h.subarray(0, 20)))) return { t: 'damaged' };
    const head: Head = { kind: latin1(h.subarray(0, 4)), len: u64(h, 4), sum: [u32(h, 12), u32(h, 16)] };
    if (head.len > len - off - FRAME_HEAD_LEN) return { t: 'short' };
    return { t: 'head', head };
  }

  private headAt(off: number, len: number): Head | null {
    const f = this.findHead(off, len);
    if (f.t === 'damaged') throw bad(`frame head checksum mismatch at offset ${off}`);
    return f.t === 'head' ? f.head : null;
  }

  /** The complete, verified payload of a frame of `kind` at `off` (as `payload_at`). */
  private payloadAt(off: number, len: number, kind: string, max: number): Uint8Array {
    const h = this.headAt(off, len);
    if (!h) throw bad(`frame at offset ${off} runs past the end of the file`);
    if (h.kind !== kind) throw bad(`expected a ${kind} frame at offset ${off}`);
    if (h.len > max) throw bad(`frame at offset ${off} declares ${h.len} bytes`);
    const p = this.read(off + FRAME_HEAD_LEN, h.len);
    const s = fnv64(p);
    if (s[0] !== h.sum[0] || s[1] !== h.sum[1]) throw bad(`payload checksum mismatch in frame at offset ${off}`);
    return p;
  }

  private meshPrefix(): MeshPrefix {
    const h = this.headAt(HEADER_LEN, this.len);
    if (!h) throw bad('the mesh frame is incomplete (the writer stopped before the mesh was written)');
    if (h.kind !== 'MESH' || h.len < 24) throw bad('the first frame is not a mesh frame');
    const p = this.read(HEADER_LEN + FRAME_HEAD_LEN, 24);
    return { end: HEADER_LEN + FRAME_HEAD_LEN + h.len, hash: hex16(p, 0), nCells: u64(p, 8), nPoints: u64(p, 16) };
  }

  private checkDir(d: SnapDir, snapOff: number): void {
    if (!Number.isFinite(d.time)) throw bad('snapshot time is not finite');
    d.fields.forEach((f, i) => {
      const want = f.location === 'cell' ? this.mesh.nCells : this.mesh.nPoints;
      if (f.n !== want || f.block_values === 0 || f.block_values > MAX_BLOCK_VALUES) {
        throw bad(`field '${f.name}' of step ${d.step} has an inconsistent size`);
      }
      if (f.blocks.length !== Math.ceil(f.n / f.block_values) || f.blocks.some((b) => b < this.mesh.end || b >= snapOff)) {
        throw bad(`field '${f.name}' of step ${d.step} has an inconsistent block list`);
      }
      if (d.fields.slice(0, i).some((g) => g.name === f.name)) throw bad(`field '${f.name}' appears twice in step ${d.step}`);
    });
  }

  /** A lenient scan from `start` (as `scan(.., lenient = true)`): stops at a damaged frame head or a short tail. */
  private scan(start: number, prev: SnapInfo | null): ScanOut {
    const out: ScanOut = { snaps: [], committedEnd: start, stoppedAt: null };
    let pos = start;
    for (;;) {
      const f = this.findHead(pos, this.len);
      if (f.t === 'short') break;
      if (f.t === 'damaged') {
        out.stoppedAt = pos;
        break;
      }
      const h = f.head;
      const next = pos + FRAME_HEAD_LEN + h.len;
      if (h.kind === 'SNAP') {
        const p = this.payloadAt(pos, this.len, 'SNAP', MAX_JSON_FRAME);
        const d = parseDir(Buffer.from(p).toString('utf8'), `snapshot at offset ${pos}`);
        this.checkDir(d, pos);
        if (!monotone(out.snaps[out.snaps.length - 1] ?? prev, d.step, d.time)) {
          throw bad(`snapshot at offset ${pos} does not follow its predecessor in step and time`);
        }
        out.snaps.push({ step: d.step, time: d.time, offset: pos, end: next });
        out.committedEnd = next;
      } else if (h.kind !== 'DATA' && h.kind !== 'FOOT') {
        throw bad(`unexpected ${h.kind} frame at offset ${pos}`);
      }
      pos = next;
    }
    return out;
  }

  /** The footer index, or why it cannot be used (as `read_footer`). */
  private readFooter(): SnapInfo[] | string {
    const m = this.mesh;
    const len = this.len;
    if (len < m.end + FRAME_HEAD_LEN + TRAILER_LEN) return 'no footer (the writer has not finished: live or interrupted run)';
    const t = this.read(len - TRAILER_LEN, TRAILER_LEN);
    if (latin1(t.subarray(8, 12)) !== 'RTSE' || !sumIs(t, 12, fnv64(t.subarray(0, 12)))) {
      return 'no valid trailer (the writer has not finished: live or interrupted run, or a damaged trailer)';
    }
    let off: number;
    try {
      off = u64(t, 0);
    } catch {
      return 'trailer points outside the file';
    }
    if (off < m.end || off > len - TRAILER_LEN - FRAME_HEAD_LEN) return 'trailer points outside the file';
    const bodyLen = len - TRAILER_LEN;
    let p: Uint8Array;
    try {
      p = this.payloadAt(off, bodyLen, 'FOOT', MAX_JSON_FRAME);
    } catch (e) {
      return (e as Error).message;
    }
    if (off + FRAME_HEAD_LEN + p.length !== bodyLen) return 'footer frame does not end at the trailer';
    let foot: unknown;
    try {
      foot = JSON.parse(Buffer.from(p).toString('utf8'));
    } catch (e) {
      return `footer: ${(e as Error).message}`;
    }
    if (!isObj(foot) || !isCount(foot['mesh']) || !Array.isArray(foot['snapshots'])) return 'footer: not a footer';
    if (foot['mesh'] !== HEADER_LEN) return 'footer names a different mesh frame';
    const snaps: SnapInfo[] = [];
    for (const s of foot['snapshots'] as unknown[]) {
      if (!Array.isArray(s) || s.length !== 4 || !isCount(s[0]) || typeof s[1] !== 'number' || !isCount(s[2]) || !isCount(s[3])) {
        return 'footer: a snapshot entry is not (step, time, offset, end)';
      }
      const [step, time, offset, end] = s as [number, number, number, number];
      const last = snaps[snaps.length - 1];
      const lo = last ? last.end : m.end;
      if (!Number.isFinite(time) || !monotone(last ?? null, step, time) || offset < lo || end <= offset || end > off) {
        return 'footer index is not consistent';
      }
      snaps.push({ step, time, offset, end });
    }
    return snaps;
  }

  // ------------------------------------------------------------ snapshots and fields

  private snapDir(i: number): SnapDir {
    const s = this.snaps[i];
    if (!Number.isSafeInteger(i) || !s) throw bad(`no snapshot ${i} (the store has ${this.snaps.length})`);
    const p = this.payloadAt(s.offset, this.len, 'SNAP', MAX_JSON_FRAME);
    if (s.offset + FRAME_HEAD_LEN + p.length !== s.end) throw bad(`snapshot ${i} does not match the index`);
    const d = parseDir(Buffer.from(p).toString('utf8'), `snapshot ${i}`);
    this.checkDir(d, s.offset);
    if (d.step !== s.step || !Object.is(d.time, s.time)) throw bad(`snapshot ${i} does not match the index`);
    return d;
  }

  /** The fields of snapshot `i`. */
  fields(i: number): readonly StoreFieldInfo[] {
    return this.snapDir(i).fields;
  }

  private info(i: number, name: string): StoreFieldInfo {
    const d = this.snapDir(i);
    const f = d.fields.find((x) => x.name === name);
    if (!f) throw bad(`snapshot ${i} (step ${d.step}) has no field '${name}'`);
    return f;
  }

  /**
   * All values of field `name` of snapshot `i` as `f32` bytes, little-endian: `Reader::read_field` cast with `as f32`
   * (an `f32` field's bits unchanged; an `f64` field rounded to nearest, ties to even, as Float32Array does).
   */
  field(i: number, name: string): ArrayBuffer {
    const f = this.info(i, name);
    const w = f.precision === 'f32' ? 4 : 8;
    const out = new Float32Array(f.n);
    const outBytes = new Uint8Array(out.buffer);
    for (let b = 0; b < f.blocks.length; b++) {
      const count = Math.min(f.block_values, f.n - b * f.block_values);
      const max = count * 16 + 1024;
      let p = this.payloadAt(f.blocks[b]!, this.len, 'DATA', max);
      if (f.codec === 'shuffle_deflate') {
        let s: Buffer;
        try {
          s = zlib.inflateRawSync(p, { maxOutputLength: count * w });
        } catch (e) {
          throw bad(`deflate block: ${(e as Error).message}`);
        }
        if (s.length !== count * w) throw bad('deflate block has the wrong length');
        p = unshuffle(s, count, w, this.hooks.wrongStride === true);
      }
      if (p.length !== count * w) throw bad('data block has the wrong length');
      if (w === 4) outBytes.set(p, 4 * b * f.block_values);
      else {
        // `p` is a fresh array (offset 0), so the f64 view is aligned
        const v = p.byteOffset % 8 === 0 ? new Float64Array(p.buffer, p.byteOffset, count) : new Float64Array(p.slice().buffer);
        out.set(v, b * f.block_values);
      }
    }
    return out.buffer;
  }

  /** F-815: the stored information of field `name` of snapshot `i` (unit, location, count, precision). */
  fieldInfo(i: number, name: string): StoreFieldInfo {
    return this.info(i, name);
  }

  /**
   * F-815: all values of field `name` of snapshot `i` as `f64` bytes, little-endian: `Reader::read_field` itself (an
   * `f64` field's bits unchanged; an `f32` field widened, which is exact). The same blocks and checks as `field`.
   */
  field64(i: number, name: string): ArrayBuffer {
    const f = this.info(i, name);
    const w = f.precision === 'f32' ? 4 : 8;
    const out = new Float64Array(f.n);
    for (let b = 0; b < f.blocks.length; b++) {
      const count = Math.min(f.block_values, f.n - b * f.block_values);
      const max = count * 16 + 1024;
      let p = this.payloadAt(f.blocks[b]!, this.len, 'DATA', max);
      if (f.codec === 'shuffle_deflate') {
        let s: Buffer;
        try {
          s = zlib.inflateRawSync(p, { maxOutputLength: count * w });
        } catch (e) {
          throw bad(`deflate block: ${(e as Error).message}`);
        }
        if (s.length !== count * w) throw bad('deflate block has the wrong length');
        p = unshuffle(s, count, w, this.hooks.wrongStride === true);
      }
      if (p.length !== count * w) throw bad('data block has the wrong length');
      const bytes = p.byteOffset % w === 0 ? p : p.slice();
      const v = w === 4 ? new Float32Array(bytes.buffer, bytes.byteOffset, count) : new Float64Array(bytes.buffer, bytes.byteOffset, count);
      out.set(v, b * f.block_values);
    }
    return out.buffer;
  }

  /** Verifies the payload checksum of the MESH frame (streamed; once per open store). */
  verifyMesh(): void {
    if (this.meshVerified) return;
    const h = this.headAt(HEADER_LEN, this.len);
    if (!h || h.kind !== 'MESH') throw bad('the first frame is not a mesh frame');
    let s: Fnv = [FNV_BASIS[0], FNV_BASIS[1]];
    const start = HEADER_LEN + FRAME_HEAD_LEN;
    for (let at = 0; at < h.len; at += MESH_CHUNK) s = fnv64Update(s, this.read(start + at, Math.min(MESH_CHUNK, h.len - at)));
    if (s[0] !== h.sum[0] || s[1] !== h.sum[1]) throw bad(`payload checksum mismatch in frame at offset ${HEADER_LEN}`);
    const p = this.read(start, 24);
    if (hex16(p, 0) !== this.mesh.hash || u64(p, 8) !== this.mesh.nCells || u64(p, 16) !== this.mesh.nPoints) {
      throw bad('mesh frame changed while open');
    }
    this.meshVerified = true;
  }

  /**
   * The boundary faces of the stored mesh as triangles (a fan per face), as `data.surface` gives them: points `f32`,
   * triangle corners `u32`, owner cell of each triangle `u32`. Mesh payload (crates/rcfd-io/src/container.rs,
   * `mesh_to_bytes`): u64 hash | u64 cells | u64 points | u64 json length | json | points (3 f64 each) | face_offsets (u64,
   * faces + 1) | face_points (u64) | owner (u64, faces) | neighbour (u64, internal faces). Boundary faces come after the
   * internal ones. The MESH frame's checksum is verified first.
   */
  surface(): { points: ArrayBuffer; triangles: ArrayBuffer; cells: ArrayBuffer } {
    this.verifyMesh();
    const frameEnd = this.mesh.end;
    const base = HEADER_LEN + FRAME_HEAD_LEN + 24;
    const jl = u64(this.read(base, 8), 0);
    if (base + 8 + jl > frameEnd) throw bad('truncated mesh section');
    let meta: Record<string, unknown>;
    try {
      meta = JSON.parse(Buffer.from(this.read(base + 8, jl)).toString('utf8')) as Record<string, unknown>;
    } catch (e) {
      throw bad(`mesh metadata: ${(e as Error).message}`);
    }
    const nPoints = meta['n_points'];
    const nFaces = meta['n_faces'];
    const nNeighbour = meta['n_neighbour'];
    const nFacePoints = meta['n_face_points'];
    if (!isCount(nPoints) || !isCount(nFaces) || !isCount(nNeighbour) || !isCount(nFacePoints) || nNeighbour > nFaces) {
      throw bad('mesh metadata: bad sizes');
    }
    if (nPoints >= 0xffffffff || this.mesh.nCells >= 0xffffffff) throw bad('the mesh has more than 2^32 - 1 points or cells');
    const pointsAt = base + 8 + jl;
    const offsetsAt = pointsAt + 24 * nPoints;
    const facePointsAt = offsetsAt + 8 * (nFaces + 1);
    const ownerAt = facePointsAt + 8 * nFacePoints;
    if (ownerAt + 8 * nFaces + 8 * nNeighbour > frameEnd) throw bad('truncated mesh section');
    const first = nNeighbour;
    const nb = nFaces - first;
    /** the low words of little-endian u64 values (the high words must be zero) */
    const words = (at: number, n: number): Uint32Array => {
      const w = new Uint32Array(this.read(at, 8 * n).buffer);
      const out = new Uint32Array(n);
      for (let i = 0; i < n; i++) {
        if (w[2 * i + 1] !== 0) throw bad('index beyond 2^32');
        out[i] = w[2 * i]!;
      }
      return out;
    };
    const xyz = new Float64Array(this.read(pointsAt, 24 * nPoints).buffer);
    const offsets = words(offsetsAt + 8 * first, nb + 1);
    for (let f = 0; f < nb; f++) if (offsets[f + 1]! < offsets[f]!) throw bad('face offsets are not increasing');
    if (offsets[nb]! > nFacePoints) throw bad('face offsets point past the face points');
    const facePoints = words(facePointsAt + 8 * offsets[0]!, offsets[nb]! - offsets[0]!);
    const owner = words(ownerAt + 8 * first, nb);
    for (let k = 0; k < facePoints.length; k++) if (facePoints[k]! >= nPoints) throw bad('a face names a point that does not exist');
    for (let k = 0; k < owner.length; k++) if (owner[k]! >= this.mesh.nCells) throw bad('a face names a cell that does not exist');
    const newId = new Uint32Array(nPoints).fill(0xffffffff);
    let nTri = 0;
    for (let f = 0; f < nb; f++) nTri += Math.max(0, offsets[f + 1]! - offsets[f]! - 2);
    const triangles = new Uint32Array(3 * nTri);
    const cells = new Uint32Array(nTri);
    const used: number[] = [];
    let t = 0;
    for (let f = 0; f < nb; f++) {
      const a = offsets[f]! - offsets[0]!;
      const n = offsets[f + 1]! - offsets[f]!;
      for (let k = 0; k < n; k++) {
        const c = facePoints[a + k]!;
        if (newId[c] === 0xffffffff) {
          newId[c] = used.length;
          used.push(c);
        }
      }
      for (let k = 1; k + 1 < n; k++) {
        triangles[3 * t] = newId[facePoints[a]!]!;
        triangles[3 * t + 1] = newId[facePoints[a + k]!]!;
        triangles[3 * t + 2] = newId[facePoints[a + k + 1]!]!;
        cells[t] = owner[f]!;
        t++;
      }
    }
    const points = new Float32Array(3 * used.length);
    for (let i = 0; i < used.length; i++) {
      const u = used[i]!;
      points[3 * i] = xyz[3 * u]!;
      points[3 * i + 1] = xyz[3 * u + 1]!;
      points[3 * i + 2] = xyz[3 * u + 2]!;
    }
    return { points: points.buffer, triangles: triangles.buffer, cells: cells.buffer };
  }
}

interface ScanOut {
  snaps: SnapInfo[];
  committedEnd: number;
  stoppedAt: number | null;
}

function scanReason(footer: string, stoppedAt: number | null): string {
  return stoppedAt === null ? footer : `${footer}; the scan stopped at a damaged frame head at offset ${stoppedAt} (tail ignored)`;
}

const monotone = (prev: SnapInfo | null, step: number, time: number): boolean => prev === null || (step > prev.step && time >= prev.time);

/** Undoes the byte shuffle of a block: all first bytes, then all second bytes, ... back to values of `w` bytes. */
export function unshuffle(s: Uint8Array, count: number, w: number, wrongStride = false): Uint8Array {
  const r = new Uint8Array(count * w);
  const stride = wrongStride ? count + 1 : count;
  for (let j = 0; j < w; j++) {
    const base = j * stride;
    for (let k = 0; k < count; k++) r[k * w + j] = s[base + k]!;
  }
  return r;
}
