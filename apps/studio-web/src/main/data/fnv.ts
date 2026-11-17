// FNV-1a 64, the checksum of every frame of the time-series store (ADR 0124; crates/rcfd-tsstore/src/lib.rs `fnv64`):
// offset basis cbf29ce484222325, prime 100000001b3, one byte at a time, h = (h ^ byte) * prime modulo 2^64.
//
// Two loops compute the same value. `fnv64UpdateJs` is the reference: 64 bits as two unsigned 32-bit halves, no BigInt.
// `fnv64Update` runs a WebAssembly loop of 64-bit integers when the bytes are many (F-819b: the 773 MB mesh frame of a
// 4e6-cell store took 1.6 s in the reference loop): eight bytes per load, then the eight byte steps in order, so the
// only chain from byte to byte is one xor and one 64-bit multiply. The module is assembled below from its instructions
// (no binary file, no dependency); where WebAssembly is missing or refuses it, the reference loop runs. FNV-1a has no
// shortcut: every byte is still taken in order, so the result is the reference's by construction, and
// tests/unit/data-fnv.spec.ts compares the two on lengths, offsets and chunk boundaries.

/** FNV-1a 64 state as two unsigned 32-bit halves [low, high]. */
export type Fnv = [number, number];
/** Offset basis cbf29ce484222325. */
export const FNV_BASIS: Readonly<Fnv> = [0x84222325, 0xcbf29ce4];

/** Continues FNV-1a 64 (prime 100000001b3 = 2^40 + 0x1b3) over `bytes` from `state`: the reference loop. */
export function fnv64UpdateJs(state: Readonly<Fnv>, bytes: Uint8Array): Fnv {
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

// ---- the WebAssembly loop

/** Bytes copied into the module's memory per call (its memory is this plus one page). */
const WASM_CHUNK = 1 << 20;
/** Below this many bytes the reference loop is as fast (no copy, no call). */
const WASM_MIN = 4096;

/** Unsigned LEB128. */
const uleb = (v: number): number[] => {
  const out: number[] = [];
  do {
    let b = v & 0x7f;
    v >>>= 7;
    if (v !== 0) b |= 0x80;
    out.push(b);
  } while (v !== 0);
  return out;
};

/** Signed LEB128 of a 64-bit constant. */
const sleb = (v: bigint): number[] => {
  const out: number[] = [];
  for (;;) {
    const b = Number(v & 0x7fn);
    v >>= 7n;
    const done = (v === 0n && (b & 0x40) === 0) || (v === -1n && (b & 0x40) !== 0);
    out.push(done ? b : b | 0x80);
    if (done) return out;
  }
};

/**
 * The module: memory `m` (17 pages, exported) and `f(p: i32, n: i32, h: i64) -> i64`, which continues FNV-1a 64 from `h`
 * over the `n` bytes at `p`. Locals: 0 p, 1 n, 2 h (parameters), 3 end, 4 end8 (i32), 5 w (i64).
 */
function moduleBytes(): Uint8Array {
  const I32_CONST = 0x41, I64_CONST = 0x42;
  const get = (i: number) => [0x20, i];
  const set = (i: number) => [0x21, i];
  const I32_ADD = 0x6a, I32_AND = 0x71, I32_GE_U = 0x4f;
  const I64_XOR = 0x85, I64_MUL = 0x7e, I64_AND = 0x83, I64_SHR_U = 0x88;
  const I64_LOAD = [0x29, 0x03, 0x00]; // align 2^3, offset 0
  const I64_LOAD8_U = [0x31, 0x00, 0x00];
  const BLOCK = [0x02, 0x40], LOOP = [0x03, 0x40], END = 0x0b;
  const BR = (d: number) => [0x0c, d], BR_IF = (d: number) => [0x0d, d];
  const PRIME = [I64_CONST, ...sleb(0x100000001b3n)];
  // h = (h ^ ((w >> shift) & 0xff)) * prime; the last byte needs no mask
  const byteStep = (shift: number) => [
    ...get(2),
    ...get(5),
    ...(shift > 0 ? [I64_CONST, ...sleb(BigInt(shift)), I64_SHR_U] : []),
    ...(shift < 56 ? [I64_CONST, ...sleb(0xffn), I64_AND] : []),
    I64_XOR,
    ...PRIME,
    I64_MUL,
    ...set(2),
  ];
  const code = [
    0x02, 0x02, 0x7f, 0x01, 0x7e, // locals: 2 x i32, 1 x i64
    ...get(0), ...get(1), I32_ADD, ...set(3), // end = p + n
    ...get(0), ...get(1), I32_CONST, ...sleb(-8n), I32_AND, I32_ADD, ...set(4), // end8 = p + (n & ~7)
    ...BLOCK, ...LOOP,
    ...get(0), ...get(4), I32_GE_U, ...BR_IF(1), // while p < end8
    ...get(0), ...I64_LOAD, ...set(5), // w = the next 8 bytes, little-endian: byte k is (w >> 8k) & 0xff
    ...[0, 8, 16, 24, 32, 40, 48, 56].flatMap(byteStep),
    ...get(0), I32_CONST, 8, I32_ADD, ...set(0),
    ...BR(0), END, END,
    ...BLOCK, ...LOOP,
    ...get(0), ...get(3), I32_GE_U, ...BR_IF(1), // while p < end: the last 0 to 7 bytes
    ...get(2), ...get(0), ...I64_LOAD8_U, I64_XOR, ...PRIME, I64_MUL, ...set(2),
    ...get(0), I32_CONST, 1, I32_ADD, ...set(0),
    ...BR(0), END, END,
    ...get(2), END,
  ];
  const section = (id: number, body: number[]) => [id, ...uleb(body.length), ...body];
  const name = (s: string) => [...uleb(s.length), ...Array.from(s, (c) => c.charCodeAt(0))];
  const pages = WASM_CHUNK / 65536 + 1;
  return new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // "\0asm", version 1
    ...section(1, [0x01, 0x60, 0x03, 0x7f, 0x7f, 0x7e, 0x01, 0x7e]), // type 0: (i32, i32, i64) -> i64
    ...section(3, [0x01, 0x00]), // function 0 has type 0
    ...section(5, [0x01, 0x00, ...uleb(pages)]), // one memory, at least `pages` pages
    ...section(7, [0x02, ...name('m'), 0x02, 0x00, ...name('f'), 0x00, 0x00]), // exports m (memory 0), f (function 0)
    ...section(10, [0x01, ...uleb(code.length), ...code]),
  ]);
}

interface Loop {
  readonly memory: Uint8Array;
  readonly f: (p: number, n: number, h: bigint) => bigint;
}

let loop: Loop | null | undefined;

/** The part of the WebAssembly global used here (this project's Node types do not declare it). */
interface Wasm {
  readonly Module: new (bytes: Uint8Array) => object;
  readonly Instance: new (module: object, imports: object) => { readonly exports: Record<string, unknown> };
}

function wasmLoop(): Loop | null {
  if (loop !== undefined) return loop;
  try {
    const wa = (globalThis as { WebAssembly?: Wasm }).WebAssembly;
    if (!wa) throw new Error('no WebAssembly');
    const inst = new wa.Instance(new wa.Module(moduleBytes()), {});
    const memory = new Uint8Array((inst.exports['m'] as { buffer: ArrayBuffer }).buffer);
    loop = { memory, f: inst.exports['f'] as Loop['f'] };
  } catch {
    loop = null;
  }
  return loop;
}

/** Which loop `fnv64Update` uses for many bytes (for the tests and the measurement). */
export function fnvLoop(): 'wasm' | 'js' {
  return wasmLoop() ? 'wasm' : 'js';
}

/** Continues FNV-1a 64 over `bytes` from `state`: the WebAssembly loop for many bytes, else the reference loop. */
export function fnv64Update(state: Readonly<Fnv>, bytes: Uint8Array): Fnv {
  const w = bytes.length >= WASM_MIN ? wasmLoop() : null;
  if (!w) return fnv64UpdateJs(state, bytes);
  let h = (BigInt(state[1] >>> 0) << 32n) | BigInt(state[0] >>> 0);
  for (let at = 0; at < bytes.length; at += WASM_CHUNK) {
    const part = bytes.subarray(at, Math.min(bytes.length, at + WASM_CHUNK));
    w.memory.set(part, 0);
    h = w.f(0, part.length, BigInt.asIntN(64, h));
  }
  const u = BigInt.asUintN(64, h);
  return [Number(u & 0xffffffffn), Number(u >> 32n)];
}

export const fnv64 = (bytes: Uint8Array): Fnv => fnv64Update(FNV_BASIS, bytes);
