// Bulk field data in the packaged Studio (F-804; guardrails G2, G3 (P12), G10, G21): the page asks `window.studioData`
// for every field of every snapshot of the ADR 0350 data set of 1e5 cells (raw and deflated, made afresh by the Rust
// example `f447_bench gen`) by route R2 (`local`: the main process's worker reads `store.rts`) and route R1 (`service`:
// the `data.*` methods of `f447_bench serve`, the same Service and serve_tcp as `rcfd serve` with data.* on), and every
// value it receives equals the Rust reader's, bit for bit: SHA-256 of every field computed in the page against the
// reference bytes, and the differing bits counted in the page (XOR and population count, as ADR 0350 did) for all
// snapshots of `p`, with the negative control NC2. Also: the bridge's shape, the refusals, cancellation, a reload.
//
// Needs `f447_bench` (tests/support/f447.ts); without it these tests are skipped and say why.
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { FIELDS, findBench, generate, NO_BENCH, refBytes, servicePath, sha256, SNAPSHOTS, startBenchServer, storeDir, type BenchServer } from '../support/f447.js';
import { launchStudio, untilState } from '../support/studio.js';
import type { StoreRef } from '../../src/shared/data.js';

const bench = findBench();
test.skip(bench === null, NO_BENCH);
test.describe.configure({ mode: 'serial' });

let root = '';
let server: BenchServer;
let app: ElectronApplication;
let page: Page;

const stores = (size: string): { name: string; store: StoreRef }[] =>
  (['raw', 'deflate'] as const).flatMap((codec) => [
    { name: `R2 ${codec}`, store: { route: 'local' as const, path: storeDir(root, size, codec) } },
    { name: `R1 ${codec}`, store: { route: 'service' as const, path: servicePath(size, codec) } },
  ]);

test.beforeAll(async () => {
  test.setTimeout(300_000);
  root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f804-e2e-'));
  await generate(bench!, root, ['tiny', '1e5']);
  server = await startBenchServer(bench!, root);
  ({ app, page } = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${server.rpc}`, RCFD_STUDIO_DATA_ROOTS: root }));
  await untilState(page, (s) => s.status === 'connected', 15_000, 'connected');
});

test.afterAll(async () => {
  await app?.close();
  await server?.stop();
  if (root) rmSync(root, { recursive: true, force: true });
});

test('the data bridge is window.studioData with exactly its four functions, frozen; the shell bridge is unchanged', async () => {
  const shape = await page.evaluate(() => {
    const s = window.studioData as unknown as Record<string, unknown>;
    return {
      keys: Object.keys(s).sort(),
      types: [...new Set(Object.values(s).map((v) => typeof v))],
      frozen: Object.isFrozen(s),
      studio: Object.keys(window.studio).sort(),
      service: Object.keys(window.studioService).sort(),
    };
  });
  expect(shape).toEqual({
    keys: ['cancel', 'field', 'open', 'surface'],
    types: ['function'],
    frozen: true,
    studio: ['app', 'theme'], // F-818: the theme's two channels
    service: ['call', 'info', 'offState', 'onState', 'state', 'subscribe', 'unsubscribe'],
  });
  const names = await app.evaluate(({ ipcMain }) => {
    const handlers = (ipcMain as unknown as { _invokeHandlers?: Map<string, unknown> })._invokeHandlers;
    return handlers ? [...handlers.keys()].filter((n) => !/^[A-Z0-9_]+$/.test(n)).sort() : null;
  });
  expect(names).toEqual(['app.info', 'app.ready', 'theme.get', 'theme.set']);
});

test('open: both routes describe the same store', async () => {
  const infos = await page.evaluate(async (list) => Promise.all(list.map((x) => window.studioData.open(x.store))), stores('1e5'));
  for (const r of infos) expect(r.ok).toBe(true);
  const plain = infos.map((r) => (r.ok ? { ...r.result, route: '', path: '', index: null } : null));
  expect(plain[1]).toEqual(plain[0]);
  expect(plain[3]).toEqual(plain[2]);
  const first = infos[0]!;
  expect(first.ok && [first.result.nCells, first.result.snapshots.length, first.result.fields.map((f) => f.name), first.result.index]).toEqual([
    100_000,
    SNAPSHOTS,
    [...FIELDS],
    'footer',
  ]);
});

test('P12 in the renderer: every value of every field of the 1e5 box, by R2 and R1, raw and deflated, equals the Rust reader bit for bit', async () => {
  test.setTimeout(180_000);
  const list = stores('1e5');
  const got = await page.evaluate(
    async ({ list, fields, snapshots }) => {
      const hex = (b: ArrayBuffer) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, '0')).join('');
      const out: Record<string, string[]> = {};
      const problems: string[] = [];
      for (const { name, store } of list) {
        out[name] = [];
        for (let i = 0; i < snapshots; i++) {
          for (const f of fields) {
            const r = await window.studioData.field(store, i, f);
            if (!r.ok) {
              problems.push(`${name} ${i}-${f}: ${r.error.code} ${r.error.message}`);
              out[name]!.push('');
              continue;
            }
            const v = r.result.values;
            if (!(v instanceof Float32Array) || r.result.snapshot !== i || r.result.field !== f) problems.push(`${name} ${i}-${f}: not a Float32Array of that field`);
            out[name]!.push(hex(await crypto.subtle.digest('SHA-256', v)));
          }
        }
      }
      return { out, problems };
    },
    { list, fields: [...FIELDS], snapshots: SNAPSHOTS },
  );
  expect(got.problems).toEqual([]);
  const want: string[] = [];
  for (let i = 0; i < SNAPSHOTS; i++) for (const f of FIELDS) want.push(sha256(refBytes(root, '1e5', i, f)));
  for (const { name } of list) {
    const differ = got.out[name]!.filter((h, k) => h !== want[k]).length;
    expect({ name, differ, n: got.out[name]!.length }).toEqual({ name, differ: 0, n: SNAPSHOTS * FIELDS.length });
  }
});

test('P12 counted in bits in the renderer for every snapshot of p (both routes, both codecs), with NC2 reporting a rotated block', async () => {
  test.setTimeout(120_000);
  const refs = Array.from({ length: SNAPSHOTS }, (_, i) => refBytes(root, '1e5', i, 'p').toString('base64'));
  const r = await page.evaluate(
    async ({ list, refs }) => {
      const popcount = (a: Uint32Array, b: Uint32Array) => {
        if (a.length !== b.length) return Number.POSITIVE_INFINITY;
        let n = 0;
        for (let i = 0; i < a.length; i++) {
          let x = (a[i]! ^ b[i]!) >>> 0;
          x -= (x >>> 1) & 0x55555555;
          x = (x & 0x33333333) + ((x >>> 2) & 0x33333333);
          n += Math.imul((x + (x >>> 4)) & 0x0f0f0f0f, 0x01010101) >>> 24;
        }
        return n;
      };
      const ref = refs.map((s) => {
        const bin = atob(s);
        const u = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i);
        return new Uint32Array(u.buffer);
      });
      let bits = 0;
      let values = 0;
      let nc2 = 0;
      for (const { store } of list) {
        for (let i = 0; i < ref.length; i++) {
          const got = await window.studioData.field(store, i, 'p');
          if (!got.ok) return { error: got.error.message };
          const v = new Uint32Array(got.result.values.buffer);
          bits += popcount(v, ref[i]!);
          values += v.length;
          if (i === 3) {
            // NC2: rotate the bytes of block 1 (values 65536 onwards) by one
            const b = new Uint8Array(v.slice().buffer);
            const block = b.slice(4 * 65536);
            b.set([...block.subarray(1), block[0]!], 4 * 65536);
            nc2 += popcount(new Uint32Array(b.buffer), ref[i]!);
          }
        }
      }
      return { bits, values, nc2 };
    },
    { list: stores('1e5'), refs },
  );
  expect(r).toMatchObject({ bits: 0, values: 4 * SNAPSHOTS * 100_000 });
  expect((r as { nc2: number }).nc2).toBeGreaterThan(0);
});

test('the surface: R2 and R1 give the page the same bits (the Rust triangulation)', async () => {
  const r = await page.evaluate(async (list) => {
    const all = await Promise.all(list.map((x) => window.studioData.surface(x.store)));
    const bad = all.find((x) => !x.ok);
    if (bad && !bad.ok) return { error: bad.error.message };
    const s = all.map((x) => (x.ok ? x.result : null)!);
    const same = (a: ArrayBufferView, b: ArrayBufferView) => {
      const x = new Uint8Array(a.buffer, a.byteOffset, a.byteLength);
      const y = new Uint8Array(b.buffer, b.byteOffset, b.byteLength);
      return x.length === y.length && x.every((v, i) => v === y[i]);
    };
    return {
      types: s.map((x) => [x.points.constructor.name, x.triangles.constructor.name, x.cells.constructor.name]),
      sizes: s.map((x) => [x.points.length, x.triangles.length, x.cells.length]),
      same: s.slice(1).map((x) => same(x.points, s[0]!.points) && same(x.triangles, s[0]!.triangles) && same(x.cells, s[0]!.cells)),
    };
  }, stores('1e5'));
  expect(r).toMatchObject({ same: [true, true, true] });
  const ok = r as { types: string[][]; sizes: number[][] };
  for (const t of ok.types) expect(t).toEqual(['Float32Array', 'Uint32Array', 'Uint32Array']);
  // the box of 50 x 50 x 40 cells: 2 (50*50 + 50*40 + 50*40) boundary quadrilaterals, two triangles each
  const quads = 2 * (50 * 50 + 50 * 40 + 50 * 40);
  expect(ok.sizes[0]).toEqual([3 * (51 * 51 * 2 + 51 * 39 * 2 + 49 * 39 * 2), 3 * 2 * quads, 2 * quads]);
});

test('refusals: a route or argument out of shape, a local path outside the data roots, a service path outside its sandbox', async () => {
  const outside = path.join(os.tmpdir(), 'not-a-root', 'fields');
  const r = await page.evaluate(
    async ({ outside, rootStore }) => {
      const d = window.studioData as unknown as {
        open(s: unknown, o?: unknown): Promise<{ ok: boolean; error?: { code: unknown; message: string } }>;
        field(s: unknown, i: unknown, f: unknown, o?: unknown): Promise<{ ok: boolean; error?: { code: unknown; message: string } }>;
      };
      const code = (x: { ok: boolean; error?: { code: unknown } }) => (x.ok ? 'ok' : x.error!.code);
      return {
        badRoute: code(await d.open({ route: 'file', path: outside })),
        extra: code(await d.open({ route: 'local', path: outside, also: 1 })),
        badSnapshot: code(await d.field({ route: 'local', path: rootStore }, -1, 'p')),
        badField: code(await d.field({ route: 'local', path: rootStore }, 0, { name: 'p' })),
        badTimeout: code(await d.open({ route: 'local', path: rootStore }, { timeoutMs: 0 })),
        notCloneable: code(await d.open({ route: 'local', path: rootStore }, { tag: () => 1 })),
        outside: code(await d.open({ route: 'local', path: outside })),
        relative: code(await d.open({ route: 'local', path: 'project/runs/box-1e5-raw/fields' })),
        dotdot: code(await d.open({ route: 'local', path: `${rootStore}/../../box-1e5-raw/fields` })),
        serviceOutside: code(await d.open({ route: 'service', path: outside })),
        serviceDotdot: code(await d.open({ route: 'service', path: '../x' })),
        serviceMissing: code(await d.open({ route: 'service', path: 'project/runs/none/fields' })),
        noField: code(await d.field({ route: 'local', path: rootStore }, 0, 'T')),
        noSnapshot: code(await d.field({ route: 'service', path: 'project/runs/box-tiny-raw/fields' }, 99, 'p')),
      };
    },
    { outside, rootStore: storeDir(root, 'tiny', 'raw') },
  );
  expect(r).toEqual({
    badRoute: 'bad_request',
    extra: 'bad_request',
    badSnapshot: 'bad_request',
    badField: 'bad_request',
    badTimeout: 'bad_request',
    notCloneable: 'bad_request',
    outside: 'not_allowed',
    relative: 'not_allowed',
    dotdot: 'not_allowed',
    serviceOutside: 1004,
    serviceDotdot: 1004,
    serviceMissing: -32602,
    noField: 'store',
    noSnapshot: -32602,
  });
});

test('cancel(tag): waiting loads answer cancelled at once (G21), the others still arrive', async () => {
  const r = await page.evaluate(async (store) => {
    const loads = Array.from({ length: 8 }, (_, i) => window.studioData.field(store, i, 'p', { tag: 'scrub' }));
    const keep = window.studioData.field(store, 19, 'uz', { tag: 7 });
    const c = await window.studioData.cancel('scrub');
    const done = await Promise.all(loads);
    const k = await keep;
    return {
      cancelled: c.ok ? c.result : -1,
      codes: done.map((x) => (x.ok ? 'ok' : x.error.code)),
      keep: k.ok ? k.result.values.length : k.error.code,
      after: await window.studioData.cancel('scrub'),
    };
  }, stores('1e5')[0]!.store);
  // the first load may already be done before the cancel arrives; every one not done answers cancelled
  expect(r.cancelled).toBeGreaterThanOrEqual(7);
  expect(r.codes.filter((c) => c === 'cancelled').length).toBe(r.cancelled);
  expect(r.codes.every((c) => c === 'ok' || c === 'cancelled')).toBe(true);
  expect(r.keep).toBe(100_000);
  expect(r.after).toEqual({ ok: true, result: 0 });
});

test('after a reload the page gets a new data port it never sees, and the loads work as before', async () => {
  await page.addInitScript(() => {
    const w = window as unknown as { __portsSeen: number };
    w.__portsSeen = 0;
    window.addEventListener('message', (e) => (w.__portsSeen += 1 + e.ports.length), true);
  });
  await page.reload();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
  const r = await page.evaluate(async (store) => {
    const f = await window.studioData.field(store, 0, 'p');
    await new Promise((done) => setTimeout(done, 200));
    return { ok: f.ok, n: f.ok ? f.result.values.length : 0, ports: (window as unknown as { __portsSeen: number }).__portsSeen };
  }, stores('tiny')[1]!.store);
  expect(r).toEqual({ ok: true, n: 120, ports: 0 });
});
