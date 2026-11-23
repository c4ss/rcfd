// F-819: the item of this spec, by the commit that added it (3dea64ae, 2026-11-17).
// F-819b: the two FNV-1a 64 loops of src/main/data/fnv.ts give the same value. The WebAssembly loop (eight bytes per
// load, copied into the module's memory 1 MiB at a time) is compared with the reference loop of two 32-bit halves on
// every length from 0 to 80 bytes past the size where it takes over, on unaligned views, across the 1 MiB copy, and
// from states other than the basis; and with the published test vectors.
import { expect, test } from '@playwright/test';
import { FNV_BASIS, fnv64, fnv64Update, fnv64UpdateJs, fnvLoop, type Fnv } from '../../src/main/data/fnv.js';

const hex = (s: Fnv) => s[1].toString(16).padStart(8, '0') + s[0].toString(16).padStart(8, '0');

/** Bytes of a fixed pseudo-random sequence (xorshift32), so a failure can be replayed. */
function bytesOf(n: number, seed = 0x9e3779b9): Uint8Array {
  const out = new Uint8Array(n);
  let x = seed >>> 0 || 1;
  for (let i = 0; i < n; i++) {
    x ^= x << 13;
    x >>>= 0;
    x ^= x >>> 17;
    x ^= x << 5;
    x >>>= 0;
    out[i] = x & 0xff;
  }
  return out;
}

test('Node runs the WebAssembly loop', () => {
  expect(fnvLoop()).toBe('wasm');
});

test('the published test vectors, short and through the WebAssembly loop', () => {
  expect(hex(fnv64(new Uint8Array(0)))).toBe('cbf29ce484222325');
  expect(hex(fnv64(new TextEncoder().encode('a')))).toBe('af63dc4c8601ec8c');
  expect(hex(fnv64(new TextEncoder().encode('foobar')))).toBe('85944171f73967e8');
  // the same text continued from its state equals the text whole, on both sides of the size the loops change at
  const long = new TextEncoder().encode('foobar'.repeat(2000));
  expect(hex(fnv64Update(fnv64(long.subarray(0, 6000)), long.subarray(6000)))).toBe(hex(fnv64UpdateJs(FNV_BASIS, long)));
});

test('every length from 0 to 80 past 4096 bytes, on views at every offset 0 to 7, equals the reference loop', () => {
  const base = bytesOf(4096 + 80 + 8);
  const bad: string[] = [];
  for (const n of [...Array.from({ length: 81 }, (_, k) => k), ...Array.from({ length: 81 }, (_, k) => 4096 + k)]) {
    for (let off = 0; off < 8; off++) {
      const v = base.subarray(off, off + n);
      const a = hex(fnv64Update(FNV_BASIS, v));
      const b = hex(fnv64UpdateJs(FNV_BASIS, v));
      if (a !== b) bad.push(`n=${n} off=${off}: ${a} != ${b}`);
    }
  }
  expect(bad).toEqual([]);
});

test('across the 1 MiB copy into the module, and from other states, equals the reference loop', () => {
  const big = bytesOf((3 << 20) + 13, 12345);
  for (const n of [(1 << 20) - 1, 1 << 20, (1 << 20) + 1, (2 << 20) + 7, big.length]) {
    expect(hex(fnv64Update(FNV_BASIS, big.subarray(0, n))), `n=${n}`).toBe(hex(fnv64UpdateJs(FNV_BASIS, big.subarray(0, n))));
  }
  for (const state of [[0, 0], [0xffffffff, 0xffffffff], [0x12345678, 0x9abcdef0], [1, 0x80000000]] as Fnv[]) {
    const v = big.subarray(3, 3 + 70_001);
    expect(hex(fnv64Update(state, v)), hex(state)).toBe(hex(fnv64UpdateJs(state, v)));
  }
  // streamed in chunks of any size, the value is the value of the whole
  let s: Fnv = [FNV_BASIS[0], FNV_BASIS[1]];
  for (let at = 0; at < big.length; at += 777_777) s = fnv64Update(s, big.subarray(at, Math.min(big.length, at + 777_777)));
  expect(hex(s)).toBe(hex(fnv64UpdateJs(FNV_BASIS, big)));
});

test('one changed byte changes the value (the comparison is not blind)', () => {
  const v = bytesOf(100_000, 7);
  const a = hex(fnv64(v));
  v[54_321] = v[54_321]! ^ 0x01;
  expect(hex(fnv64(v))).not.toBe(a);
  expect(hex(fnv64(v))).toBe(hex(fnv64UpdateJs(FNV_BASIS, v)));
});
