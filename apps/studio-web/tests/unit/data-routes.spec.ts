// P12 of ADR 0350 for the product's two bulk routes (F-804, guardrail G3), in Node: every value that route R2 (the
// TypeScript reader of `store.rts`, in its worker thread, src/main/data/local.ts) and route R1 (the `data.*` methods
// of a real service, src/main/data/remote.ts) return equals the Rust reader's value bit for bit, on data sets made by
// the existing Rust example `f447_bench gen` (the 1e5-cell box of ADR 0350 and a tiny one, raw and deflated, 20
// snapshots, four fields). The reader's own checks are the Rust reader's: damaged checksums are refused, an unfinished
// store is scanned to the same snapshots as Rust scans it, and the negative controls NC2 and M3 of ADR 0350 are caught.
// The Electron path to the page is tests/e2e/data.spec.ts.
//
// Needs `f447_bench` (tests/support/f447.ts says where it is looked for) and the built worker (dist/, `npm run build`);
// without the example these tests are skipped and say why.
import { expect, test } from '@playwright/test';
import { appendFileSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, openSync, closeSync, readFileSync, readSync, rmSync, statSync, utimesSync, writeFileSync, writeSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { isMainThread, threadId } from 'node:worker_threads';
import { LocalData } from '../../src/main/data/local.js';
import { DataFailure } from '../../src/main/data/queue.js';
import { RemoteData } from '../../src/main/data/remote.js';
import { DataRoots } from '../../src/main/data/roots.js';
import { fnv64, meshChecks, Store, unshuffle } from '../../src/main/data/tsstore.js';
import { gatherSurface } from '../../src/shared/surface-values.js';
import { StudioData } from '../../src/main/data/studio-data.js';
import { cornerScalars, cornerScalarsOf, rangeOf, shownRange } from '../../src/renderer/field/surface.js';
import type { DataWireResult, StoreInfo } from '../../src/shared/data.js';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { appRoot, findRcfd, startRcfd } from '../support/rcfd.js';
import { differingBits, FIELDS, findBench, generate, NO_BENCH, refBytes, servicePath, SNAPSHOTS, startBenchServer, storeDir, type BenchServer } from '../support/f447.js';

const bench = findBench();
test.skip(bench === null, NO_BENCH);
test.describe.configure({ mode: 'serial' });

const WORKER = new URL(`file:///${path.join(appRoot, 'dist', 'main', 'data', 'worker.js').replace(/\\/g, '/')}`);
const SIZES = ['tiny', '1e5'] as const;
const CODECS = ['raw', 'deflate'] as const;

let root = '';
let server: BenchServer;
let rpc: RpcConnection;
let local: LocalData;
let remote: RemoteData;

const bytes = (b: ArrayBuffer): Uint8Array => new Uint8Array(b);
const fieldOf = (r: DataWireResult): ArrayBuffer => {
  if (r.kind !== 'field') throw new Error(`not a field: ${r.kind}`);
  return r.values;
};
const infoOf = (r: DataWireResult): StoreInfo => {
  if (r.kind !== 'open') throw new Error(`not an open reply: ${r.kind}`);
  return r.info;
};
const surfaceOf = (r: DataWireResult) => {
  if (r.kind !== 'surface') throw new Error(`not a surface: ${r.kind}`);
  return r;
};
const failureOf = async (p: Promise<unknown>): Promise<DataFailure> => {
  try {
    await p;
  } catch (e) {
    if (e instanceof DataFailure) return e;
    throw e;
  }
  throw new Error('expected a failure');
};

test.beforeAll(async () => {
  test.setTimeout(300_000);
  expect(existsSync(WORKER), `the built worker ${WORKER.href} (run npm run build)`).toBe(true);
  root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f804-'));
  await generate(bench!, root, SIZES);
  server = await startBenchServer(bench!, root);
  rpc = await RpcConnection.connectTcp('127.0.0.1', server.rpc);
  local = new LocalData(DataRoots.fromEnv({ RCFD_STUDIO_DATA_ROOTS: root }), WORKER);
  remote = new RemoteData({ state: { address: `127.0.0.1:${server.rpc}`, instance: 1 }, call: (m, p, t) => rpc.request(m, p, t) });
});

test.afterAll(async () => {
  remote?.close();
  await local?.close();
  rpc?.close();
  await server?.stop();
  if (root) rmSync(root, { recursive: true, force: true });
});

test('FNV-1a 64 matches the published test vectors', () => {
  const hex = (s: [number, number]) => s[1].toString(16).padStart(8, '0') + s[0].toString(16).padStart(8, '0');
  expect(hex(fnv64(new Uint8Array(0)))).toBe('cbf29ce484222325');
  expect(hex(fnv64(new TextEncoder().encode('a')))).toBe('af63dc4c8601ec8c');
  expect(hex(fnv64(new TextEncoder().encode('foobar')))).toBe('85944171f73967e8');
});

for (const size of SIZES) {
  for (const codec of CODECS) {
    test(`P12, ${size} ${codec}: every value of R2 and of R1 equals the Rust reader's, bit for bit`, async () => {
      test.setTimeout(120_000);
      const dir = storeDir(root, size, codec);
      const a = infoOf(await local.open(dir).promise);
      const b = infoOf(await remote.open(servicePath(size, codec), 30_000).promise);
      // the same store seen by both routes; only `route`, `path` and `index` differ
      expect({ ...a, route: 'x', path: 'x', index: null }).toEqual({ ...b, route: 'x', path: 'x', index: null });
      expect(a.index).toBe('footer');
      expect(a.snapshots.length).toBe(SNAPSHOTS);
      expect(a.fields.map((f) => [f.name, f.precision, f.codec])).toEqual(FIELDS.map((f) => [f, 'f32', codec === 'raw' ? 'raw' : 'shuffle_deflate']));
      let values = 0;
      let bitsR2 = 0;
      let bitsR1 = 0;
      for (let i = 0; i < SNAPSHOTS; i++) {
        for (const f of FIELDS) {
          const ref = refBytes(root, size, i, f);
          const r2 = bytes(fieldOf(await local.field(dir, i, f).promise));
          const r1 = bytes(fieldOf(await remote.field(servicePath(size, codec), i, f, 30_000).promise));
          expect(r2.byteLength, `${i}-${f}`).toBe(ref.byteLength);
          expect(r1.byteLength, `${i}-${f}`).toBe(ref.byteLength);
          bitsR2 += differingBits(r2, ref);
          bitsR1 += differingBits(r1, ref);
          values += ref.byteLength / 4;
        }
      }
      expect(values).toBe(SNAPSHOTS * FIELDS.length * a.nCells);
      expect({ bitsR2, bitsR1 }).toEqual({ bitsR2: 0, bitsR1: 0 });
    });

    test(`the surface, ${size} ${codec}: R2 equals R1 (the Rust triangulation) bit for bit`, async () => {
      const r2 = surfaceOf(await local.surface(storeDir(root, size, codec)).promise);
      const r1 = surfaceOf(await remote.surface(servicePath(size, codec), 30_000).promise);
      for (const k of ['points', 'triangles', 'cells'] as const) {
        expect(r2[k].byteLength, k).toBe(r1[k].byteLength);
        expect(r2[k].byteLength, k).toBeGreaterThan(0);
        expect(differingBits(bytes(r2[k]), bytes(r1[k])), k).toBe(0);
      }
    });
  }
}

test('the local reads run in the worker thread, not on the thread that asked', async () => {
  await local.open(storeDir(root, 'tiny', 'raw')).promise;
  expect(isMainThread).toBe(true);
  expect(local.workerThreadId).not.toBeNull();
  expect(local.workerThreadId).not.toBe(threadId);
});

test('NC2: one block rotated by one byte is reported (the comparison is not blind)', () => {
  const s = new Store(storeDir(root, '1e5', 'raw'));
  try {
    const f = s.fields(0).find((x) => x.name === 'p')!;
    expect(f.blocks.length).toBeGreaterThanOrEqual(2);
    const got = bytes(s.field(0, 'p'));
    const ref = refBytes(root, '1e5', 0, 'p');
    expect(differingBits(got, ref)).toBe(0);
    const at = 4 * f.block_values;
    const block = got.slice(at, at + 4 * Math.min(f.block_values, f.n - f.block_values));
    const rotated = new Uint8Array(got);
    rotated.set([...block.subarray(1), block[0]!], at);
    expect(differingBits(rotated, ref)).toBeGreaterThan(0);
  } finally {
    s.close();
  }
});

test('M3: an unshuffle with the wrong stride makes every deflated field differ, and the worker catches it the same way', async () => {
  const s = new Store(storeDir(root, '1e5', 'deflate'));
  let differ = 0;
  try {
    s.hooks = { wrongStride: true };
    for (let i = 0; i < SNAPSHOTS; i++) for (const f of FIELDS) if (differingBits(bytes(s.field(i, f)), refBytes(root, '1e5', i, f)) > 0) differ++;
  } finally {
    s.close();
  }
  expect(differ).toBe(SNAPSHOTS * FIELDS.length);
  await local.setWrongStrideForTest(true);
  try {
    const v = bytes(fieldOf(await local.field(storeDir(root, '1e5', 'deflate'), 3, 'ux').promise));
    expect(differingBits(v, refBytes(root, '1e5', 3, 'ux'))).toBeGreaterThan(0);
  } finally {
    await local.setWrongStrideForTest(false);
  }
  const v = bytes(fieldOf(await local.field(storeDir(root, '1e5', 'deflate'), 3, 'ux').promise));
  expect(differingBits(v, refBytes(root, '1e5', 3, 'ux'))).toBe(0);
  // the unshuffle itself: values of w bytes regrouped by significance, and back
  const raw = Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
  expect([...unshuffle(Uint8Array.from([1, 5, 9, 2, 6, 10, 3, 7, 11, 4, 8, 12]), 3, 4)]).toEqual([...raw]);
});

// ---- damaged and unfinished stores: the TypeScript reader refuses and scans as the Rust reader does

/** A copy of the store of (size, codec) under <root>/copies/<name>/fields, with `edit` applied to its bytes. */
function copyStore(name: string, size: string, codec: 'raw' | 'deflate', edit: (b: Buffer) => Buffer): { dir: string; servicePath: string } {
  const dir = path.join(root, 'copies', name, 'fields');
  mkdirSync(dir, { recursive: true });
  const b = readFileSync(path.join(storeDir(root, size, codec), 'store.rts'));
  writeFileSync(path.join(dir, 'store.rts'), edit(Buffer.from(b)));
  return { dir, servicePath: `copies/${name}/fields` };
}

const flip = (at: number) => (b: Buffer) => {
  b[at] = b[at]! ^ 0x01;
  return b;
};

test('damaged checksums are refused by both readers: header, frame head, data payload, mesh payload', async () => {
  const s = new Store(storeDir(root, 'tiny', 'raw'));
  const p0 = s.fields(0).find((f) => f.name === 'p')!.blocks[0]!;
  s.close();
  const cases = [
    { name: 'magic', at: 0, r2: /bad magic/ },
    { name: 'header', at: 9, r2: /header checksum mismatch/ },
    { name: 'datahead', at: p0 + 5, r2: /frame head checksum mismatch at offset/, op: 'field' as const },
    { name: 'datapayload', at: p0 + 28 + 7, r2: /payload checksum mismatch in frame at offset/, op: 'field' as const },
    { name: 'meshpayload', at: 20 + 28 + 24 + 8 + 40, r2: /payload checksum mismatch in frame at offset 20/, op: 'surface' as const },
  ];
  for (const c of cases) {
    const copy = copyStore(`damaged-${c.name}`, 'tiny', 'raw', flip(c.at));
    const run = (route: 'r2' | 'r1') => {
      if (c.op === 'field') return route === 'r2' ? local.field(copy.dir, 0, 'p').promise : remote.field(copy.servicePath, 0, 'p', 30_000).promise;
      if (c.op === 'surface') return route === 'r2' ? local.surface(copy.dir).promise : remote.surface(copy.servicePath, 30_000).promise;
      return route === 'r2' ? local.open(copy.dir).promise : remote.open(copy.servicePath, 30_000).promise;
    };
    const e2 = await failureOf(run('r2'));
    expect(e2.code, c.name).toBe('store');
    expect(e2.message, c.name).toMatch(c.r2);
    const e1 = await failureOf(run('r1'));
    expect(e1.code, `${c.name} (Rust)`).toBe(-32602);
    expect(e1.message, `${c.name} (Rust)`).toMatch(/tsstore|mesh/);
  }
});

test('an unfinished store (live or interrupted run) is scanned to the same snapshots as the Rust reader scans it', async () => {
  test.setTimeout(120_000);
  const full = new Store(storeDir(root, '1e5', 'raw'));
  const snaps = [...full.snapshots];
  full.close();
  const cut = (k: number, extra: number) => (b: Buffer) => b.subarray(0, snaps[k]!.end + extra);
  const cases = [
    { name: 'torn-after-9', edit: cut(9, 1000), want: 10 },
    { name: 'zero-tail-after-3', edit: (b: Buffer) => Buffer.concat([b.subarray(0, snaps[3]!.end), Buffer.alloc(64)]), want: 4 },
    { name: 'no-footer', edit: (b: Buffer) => b.subarray(0, snaps[SNAPSHOTS - 1]!.end), want: SNAPSHOTS },
    { name: 'damaged-trailer', edit: (b: Buffer) => flip(b.length - 13)(b), want: SNAPSHOTS },
  ];
  for (const c of cases) {
    const copy = copyStore(c.name, '1e5', 'raw', c.edit);
    const a = infoOf(await local.open(copy.dir).promise);
    const b = infoOf(await remote.open(copy.servicePath, 30_000).promise);
    expect(a.index, c.name).toBe('scanned');
    expect(a.snapshots.length, c.name).toBe(c.want);
    expect(a.snapshots, c.name).toEqual(b.snapshots);
    expect(a.snapshots, c.name).toEqual(snaps.slice(0, c.want).map((s) => ({ step: s.step, time: s.time })));
    const last = c.want - 1;
    const v = bytes(fieldOf(await local.field(copy.dir, last, 'uy').promise));
    expect(differingBits(v, refBytes(root, '1e5', last, 'uy')), c.name).toBe(0);
  }
});

test('a live store that grows: a later open picks up the new snapshots, as Reader::refresh does', async () => {
  const src = readFileSync(path.join(storeDir(root, '1e5', 'raw'), 'store.rts'));
  const full = new Store(storeDir(root, '1e5', 'raw'));
  const snaps = [...full.snapshots];
  full.close();
  const dir = path.join(root, 'copies', 'live', 'fields');
  mkdirSync(dir, { recursive: true });
  const file = path.join(dir, 'store.rts');
  writeFileSync(file, src.subarray(0, snaps[4]!.end + 333));
  expect(infoOf(await local.open(dir).promise).snapshots.length).toBe(5);
  // the writer goes on: append (not replace) up to the middle of snapshot 12's frames
  appendFileSync(file, src.subarray(snaps[4]!.end + 333, snaps[11]!.end + 50));
  const later = infoOf(await local.open(dir).promise);
  expect(later.index).toBe('scanned');
  expect(later.snapshots.length).toBe(12);
  const rust = infoOf(await remote.open('copies/live/fields', 30_000).promise);
  expect(later.snapshots).toEqual(rust.snapshots);
  const v = bytes(fieldOf(await local.field(dir, 11, 'p').promise));
  expect(differingBits(v, refBytes(root, '1e5', 11, 'p'))).toBe(0);
  // finished: the rest arrives, footer and trailer included
  appendFileSync(file, src.subarray(snaps[11]!.end + 50));
  const done = infoOf(await local.open(dir).promise);
  expect(done.snapshots.length).toBe(SNAPSHOTS);
});

test('a store replaced by a new one is read anew', async () => {
  const dir = path.join(root, 'copies', 'replaced', 'fields');
  mkdirSync(dir, { recursive: true });
  copyFileSync(path.join(storeDir(root, 'tiny', 'raw'), 'store.rts'), path.join(dir, 'store.rts'));
  const a = infoOf(await local.open(dir).promise);
  rmSync(path.join(dir, 'store.rts'));
  copyFileSync(path.join(storeDir(root, '1e5', 'deflate'), 'store.rts'), path.join(dir, 'store.rts'));
  const b = infoOf(await local.open(dir).promise);
  expect([a.nCells, b.nCells]).toEqual([120, 100_000]);
  const v = bytes(fieldOf(await local.field(dir, 7, 'uz').promise));
  expect(differingBits(v, refBytes(root, '1e5', 7, 'uz'))).toBe(0);
});

test('errors carry their reason: a missing field or snapshot, a store that is not there', async () => {
  const dir = storeDir(root, 'tiny', 'raw');
  expect((await failureOf(local.field(dir, 0, 'T').promise)).message).toMatch(/has no field 'T'/);
  expect((await failureOf(local.field(dir, 20, 'p').promise)).message).toMatch(/no snapshot 20 \(the store has 20\)/);
  expect((await failureOf(local.open(path.join(root, 'nowhere')).promise)).code).toBe('not_allowed');
  const r1 = await failureOf(remote.field(servicePath('tiny', 'raw'), 0, 'T', 30_000).promise);
  expect([r1.code, r1.message]).toEqual([-32602, expect.stringMatching(/has no field 'T'/)]);
});

test('R1 against a service without data.* answers no_data', async () => {
  const rcfd = findRcfd();
  test.skip(rcfd === null, 'the rcfd binary was not found (cargo build --release -p rcfd-cli)');
  const s = await startRcfd(rcfd!);
  const c = await RpcConnection.connectTcp('127.0.0.1', s.port);
  const r = new RemoteData({ state: { address: `127.0.0.1:${s.port}`, instance: 1 }, call: (m, p, t) => c.request(m, p, t) });
  try {
    for (const job of [r.open('x', 5000), r.field('x', 0, 'p', 5000), r.surface('x', 5000)]) {
      const e = await failureOf(job.promise);
      expect(e.code).toBe('no_data');
    }
  } finally {
    r.close();
    c.close();
    await s.stop();
  }
});

// ---- F-819b: surface values gathered by the reader, and the mesh check remembered for a file that did not change

for (const size of SIZES) {
  for (const codec of CODECS) {
    test(`F-819b, ${size} ${codec}: surfaceField is one value per surface triangle, its owner cell's Rust value bit for bit`, async () => {
      test.setTimeout(120_000);
      const dir = storeDir(root, size, codec);
      const info = infoOf(await local.open(dir).promise);
      const s = surfaceOf(await local.surface(dir).promise);
      const cells = new Uint32Array(s.cells);
      let bits = 0;
      for (let i = 0; i < SNAPSHOTS; i++) {
        for (const f of FIELDS) {
          const r = await local.surfaceField(dir, i, f).promise;
          if (r.kind !== 'surfaceField') throw new Error(`not surface values: ${r.kind}`);
          expect([r.snapshot, r.field, r.meshHash]).toEqual([i, f, info.meshHash]);
          const ref = refBytes(root, size, i, f);
          const whole = new Float32Array(ref.buffer.slice(ref.byteOffset, ref.byteOffset + ref.byteLength));
          const want = gatherSurface(cells, whole);
          const got = new Float32Array(r.values);
          expect(got.length, `${i}-${f}`).toBe(cells.length);
          bits += differingBits(bytes(r.values), new Uint8Array(want.buffer));
          // what the viewport makes of them: the corner scalars and F-808's range of the whole field
          if (i === SNAPSHOTS - 1) {
            expect(Buffer.from(cornerScalarsOf(got).buffer).equals(Buffer.from(cornerScalars(cells, whole).buffer)), `${f} scalars`).toBe(true);
            expect(rangeOf(got), `${f} range`).toEqual(shownRange(cells, whole));
          }
        }
      }
      expect(bits).toBe(0);
    });
  }
}

test('F-819b: surfaceField before any surface is read (the reader makes the surface itself), and its refusals', async () => {
  const copy = copyStore('surface-first', 'tiny', 'deflate', (b) => b);
  const r = await local.surfaceField(copy.dir, 3, 'uy').promise;
  if (r.kind !== 'surfaceField') throw new Error(`not surface values: ${r.kind}`);
  const s = surfaceOf(await local.surface(copy.dir).promise);
  const ref = refBytes(root, 'tiny', 3, 'uy');
  const want = gatherSurface(new Uint32Array(s.cells), new Float32Array(ref.buffer.slice(ref.byteOffset, ref.byteOffset + ref.byteLength)));
  expect(differingBits(bytes(r.values), new Uint8Array(want.buffer))).toBe(0);
  expect((await failureOf(local.surfaceField(copy.dir, 0, 'T').promise)).message).toMatch(/has no field 'T'/);
  expect((await failureOf(local.surfaceField(copy.dir, 20, 'p').promise)).message).toMatch(/no snapshot 20/);
  // the service route gathers nothing: refused with the reason, the page reads the whole field there
  const data = new StudioData({ state: { address: '127.0.0.1:1', instance: 1 }, call: () => Promise.reject(new Error('not called')) }, DataRoots.fromEnv({ RCFD_STUDIO_DATA_ROOTS: root }), WORKER);
  try {
    const e = await failureOf(data.run({ op: 'surfaceField', id: 1, store: { route: 'service', path: 'x' }, snapshot: 0, field: 'p' }, 5000).promise);
    expect([e.code, e.message]).toEqual(['bad_request', expect.stringMatching(/service route has no surface values/)]);
  } finally {
    await data.close();
  }
});

test('F-819b: a mesh frame verified in this process is not hashed again while its file is unchanged; a changed or replaced file is', () => {
  const copy = copyStore('memo', 'tiny', 'raw', (b) => b);
  const file = path.join(copy.dir, 'store.rts');
  // whole seconds, so that setting the times back below restores them exactly
  const T = 1_700_000_000;
  utimesSync(file, T, T);
  const check = () => {
    const before = { ...meshChecks };
    const s = new Store(copy.dir);
    try {
      s.verifyMesh();
      return { hashed: meshChecks.hashed - before.hashed, remembered: meshChecks.remembered - before.remembered, error: null as string | null };
    } catch (e) {
      return { hashed: meshChecks.hashed - before.hashed, remembered: meshChecks.remembered - before.remembered, error: (e as Error).message };
    } finally {
      s.close();
    }
  };
  expect(check()).toEqual({ hashed: 1, remembered: 0, error: null });
  expect(check()).toEqual({ hashed: 0, remembered: 1, error: null });
  // the same bytes under another path are another file: verified
  const other = copyStore('memo-other', 'tiny', 'raw', (b) => b);
  const so = new Store(other.dir);
  const h0 = meshChecks.hashed;
  so.verifyMesh();
  so.close();
  expect(meshChecks.hashed - h0).toBe(1);

  // one byte of the mesh payload changed in place, the size and the modification time kept (set back): the change time
  // moved, so it is verified again, and refused
  const at = 20 + 28 + 24 + 8 + 40;
  const was = statSync(file, { bigint: true });
  const poke = (b: (x: number) => number) => {
    // Linux stamps a file's times from a coarse clock (a kernel tick, a few ms): a write within the tick of the last
    // change keeps the change time. Wait past it, so that each poke is a change the memo can see (ADR 0801 Addendum 17).
    if (process.platform !== 'win32') Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 50);
    const fd = openSync(file, 'r+');
    const one = Buffer.alloc(1);
    readSync(fd, one, 0, 1, at);
    writeSync(fd, Buffer.from([b(one[0]!)]), 0, 1, at);
    closeSync(fd);
    utimesSync(file, T, T);
  };
  poke((x) => x ^ 0x01);
  const now = statSync(file, { bigint: true });
  expect([now.size, now.mtimeNs, now.ino]).toEqual([was.size, was.mtimeNs, was.ino]);
  expect(now.ctimeNs).not.toBe(was.ctimeNs);
  expect(check()).toEqual({ hashed: 1, remembered: 0, error: expect.stringMatching(/payload checksum mismatch in frame at offset 20/) });
  expect(check()).toEqual({ hashed: 1, remembered: 0, error: expect.stringMatching(/payload checksum mismatch/) });
  // the byte put back: verified (and remembered) again
  poke((x) => x ^ 0x01);
  expect(check()).toEqual({ hashed: 1, remembered: 0, error: null });
  expect(check()).toEqual({ hashed: 0, remembered: 1, error: null });

  // replaced by a copy of the same bytes (a new file under the same path, the copy keeping the modification time)
  rmSync(file);
  copyFileSync(path.join(storeDir(root, 'tiny', 'raw'), 'store.rts'), file);
  expect(check()).toEqual({ hashed: 1, remembered: 0, error: null });

  // a store that grows (a live run appends a snapshot): verified again
  appendFileSync(file, Buffer.alloc(64));
  expect(check()).toEqual({ hashed: 1, remembered: 0, error: null });
  expect(check()).toEqual({ hashed: 0, remembered: 1, error: null });
});
