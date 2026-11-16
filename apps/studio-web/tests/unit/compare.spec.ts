// Comparing two runs (F-815) without a window: the rules of ADR 0801, addendum F-815 (src/shared/compare.ts) on the
// stores of the example projects; the mesh rule read from rcfd-post's source; the difference through the main
// process's reader worker against an independent f64 decode of the stored bytes (tests/support/rts-decode.ts); the
// data and run ports' new requests; compare_runs through the real project server; the palette's Compare runs; the
// pictograms and the stylesheet.
import { expect, test } from '@playwright/test';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { counterpart, counterpartWords, difference, fieldsMatch, meshMatch, timeKind, unionSpan, type Snap } from '../../src/shared/compare.js';
import { parseDataRequest } from '../../src/main/data/bridge.js';
import { LocalData } from '../../src/main/data/local.js';
import { DataFailure } from '../../src/main/data/queue.js';
import { DataRoots } from '../../src/main/data/roots.js';
import { StudioData } from '../../src/main/data/studio-data.js';
import { Store } from '../../src/main/data/tsstore.js';
import { parseRunRequest } from '../../src/main/run/bridge.js';
import { readComparison, type Query } from '../../src/main/run/series.js';
import { ProjectServer } from '../../src/main/project/server.js';
import { RUN_COMPARE_OPS, RUN_OPS } from '../../src/shared/run.js';
import { PROJECT_TOOLS } from '../../src/shared/project.js';
import { STUDIO_COMMANDS } from '../../src/shared/commands/studio.js';
import { paletteModel, searchPalette } from '../../src/shared/commands/palette.js';
import { COMPARE_ICON, COMPARE_NAME } from '../../src/renderer/workspace/compare-model.js';
import { cssCustomProperties, pictogram } from '../../src/renderer/design/tokens.js';
import { appRoot } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, repoRoot } from '../support/project.js';
import { killPid } from '../support/rcfd.js';
import { findBench, generate, storeDir } from '../support/f447.js';
import { bits64, decodeField64, snapshotDirs } from '../support/rts-decode.js';

test.describe.configure({ mode: 'serial' });

const WORKER = new URL(`file:///${path.join(appRoot, 'dist', 'main', 'data', 'worker.js').replace(/\\/g, '/')}`);
const examples = path.join(repoRoot, 'example-projects');
const fields = (project: string, run: string): string => path.join(examples, project, 'runs', run, 'fields');
/** quick-start: the two lid-driven cavities share their 32 x 32 mesh; the heated cavities have 40 x 40 cells. */
const RE100 = fields('quick-start', 'r-20261028T152641-f8c2');
const RE400 = fields('quick-start', 'r-20261028T152641-97f1');
const HEATED = fields('quick-start', 'r-20261028T152641-3900');
const HEATED3 = fields('quick-start', 'r-20261028T152641-5cc1');
/** wind-tunnel: two transient cylinders on one mesh, a steady run, and two runs of 6144 cells on different meshes. */
const CYL_A = fields('wind-tunnel', 'r-20261028T152128-d5cb');
const CYL_B = fields('wind-tunnel', 'r-20261028T152128-db3f');
const STEADY = fields('wind-tunnel', 'r-20261028T152128-37c8');
const FOIL_A = fields('wind-tunnel', 'r-20261029T230332-0566');
const FOIL_B = fields('wind-tunnel', 'r-20261029T230332-2b1f');

const open = (dir: string): Store => new Store(dir);
const snaps = (dir: string): Snap[] => {
  const s = open(dir);
  try {
    return s.snapshots.map((x) => ({ step: x.step, time: x.time }));
  } finally {
    s.close();
  }
};
const facts = (dir: string) => {
  const s = open(dir);
  try {
    return { nCells: s.nCells, nPoints: s.nPoints, meshHash: s.meshHash };
  } finally {
    s.close();
  }
};

test('C2 is rcfd-post’s identity rule: equal cell counts and equal content hashes (read from the Rust source)', () => {
  const remap = readFileSync(path.join(repoRoot, 'crates', 'rcfd-post', 'src', 'remap.rs'), 'utf8');
  expect(remap).toContain('if src.n_cells() == tgt.n_cells() && src.content_hash() == tgt.content_hash() {');
  const compare = readFileSync(path.join(repoRoot, 'crates', 'rcfd-post', 'src', 'compare.rs'), 'utf8');
  expect(compare).toContain('identical meshes (equal content hash) are copied bit for bit');
  expect(compare).toContain('let d: Vec<f64> = values.iter().zip(&m).map(|(a, b)| a - b).collect();');
  expect(compare).toContain('is not finite');
  // the hash the stores record is Mesh::content_hash (rcfd-tsstore's MESH frame)
  const store = readFileSync(path.join(repoRoot, 'crates', 'rcfd-tsstore', 'src', 'lib.rs'), 'utf8');
  expect(store).toContain('MESH  u64 mesh_hash');
});

test('C2 on the example runs: same mesh, different cell counts, and the same count on different meshes', () => {
  expect(meshMatch(facts(RE100), facts(RE400))).toEqual({ ok: true });
  expect(meshMatch(facts(HEATED), facts(HEATED3))).toEqual({ ok: true });
  expect(meshMatch(facts(RE100), facts(HEATED))).toEqual({ ok: false, reason: 'the meshes differ: 1024 and 1600 cells' });
  const a = facts(FOIL_A);
  const b = facts(FOIL_B);
  expect(a.nCells).toBe(b.nCells);
  expect(meshMatch(a, b)).toEqual({ ok: false, reason: `the meshes differ: mesh ${a.meshHash} and ${b.meshHash}, both of 6144 cells` });
});

test('C1, steady runs: the same iteration, never the nearest; past the compared run’s last snapshot there is none', () => {
  const a = snaps(RE400);
  const b = snaps(RE100);
  expect(timeKind(a, null)).toBe('iteration');
  expect(timeKind(a, true)).toBe('iteration');
  const bSteps = new Map(b.map((s, i) => [s.step, i]));
  let matched = 0;
  for (const at of a) {
    const c = counterpart(at, 'iteration', b, 'iteration');
    if (bSteps.has(at.step)) {
      expect(c).toEqual({ kind: 'match', snapshot: bSteps.get(at.step)!, step: at.step, time: b[bSteps.get(at.step)!]!.time });
      matched++;
    } else {
      expect(c).toEqual({ kind: 'none', by: 'iteration', step: at.step, time: at.time });
      if (c.kind === 'none') expect(counterpartWords(c)).toBe(`No snapshot at it ${at.step}`);
    }
  }
  expect(matched).toBeGreaterThan(20);
  expect(matched).toBeLessThan(a.length);
  // a step between two of B's snapshots: none (nothing is taken from the nearest)
  expect(counterpart({ step: 7, time: 0 }, 'iteration', b, 'iteration').kind).toBe('none');
});

test('C1, transient runs: exactly the same stored time; a steady run is never compared with a transient one', () => {
  const a = snaps(CYL_A);
  const b = snaps(CYL_B);
  expect(timeKind(a, null)).toBe('time');
  expect(timeKind(snaps(STEADY), null)).toBe('iteration');
  for (const at of a) {
    const c = counterpart(at, 'time', b, 'time');
    const j = b.findIndex((s) => s.time === at.time);
    if (j >= 0) expect(c).toMatchObject({ kind: 'match', time: at.time });
    else expect(c.kind).toBe('none');
  }
  expect(counterpart(a[0]!, 'time', b, 'time').kind).toBe('match');
  // a time between two snapshots is not interpolated
  const c = counterpart({ step: 1, time: (b[0]!.time + b[1]!.time) / 2 }, 'time', b, 'time');
  expect(c.kind).toBe('none');
  if (c.kind === 'none') expect(counterpartWords(c)).toBe(`No snapshot at t = ${(b[0]!.time + b[1]!.time) / 2} s`);
  // several snapshots at one time: the last of them
  expect(counterpart({ step: 9, time: 2 }, 'time', [{ step: 4, time: 1 }, { step: 5, time: 2 }, { step: 6, time: 2 }], 'time')).toMatchObject({ kind: 'match', snapshot: 2, step: 6 });
  const kinds = counterpart(a[0]!, 'time', snaps(STEADY), 'iteration');
  expect(kinds).toEqual({ kind: 'kinds', a: 'time', b: 'iteration' });
  if (kinds.kind === 'kinds') expect(counterpartWords(kinds)).toBe('Transient against steady: no common time');
  // the runner's word wins over the times
  expect(timeKind([{ step: 1, time: 0 }], false)).toBe('time');
});

test('C3: B - A in f64, refused for a value that is not finite, a different count, a unit or a point field', () => {
  const a = new Float64Array([1, 0.1, -0, 1e300, 5e-324]);
  const b = new Float64Array([1.5, 0.3, 0, -1e300, 5e-324]);
  const d = difference(a, b);
  expect(d.values).not.toBeNull();
  expect([...d.values!].map(bits64)).toEqual([0.5, 0.3 - 0.1, 0, -2e300, 0].map(bits64));
  expect(difference(new Float64Array([1, Number.NaN]), new Float64Array([1, 2]))).toEqual({ values: null, reason: 'the baseline value of cell 1 is not finite (NaN)' });
  expect(difference(new Float64Array([1]), new Float64Array([Infinity]))).toEqual({ values: null, reason: 'the compared value of cell 0 is not finite (Infinity)' });
  expect(difference(new Float64Array(2), new Float64Array(3)).values).toBeNull();
  const p = { name: 'p', unit: 'Pa', location: 'cell' as const, n: 4 };
  expect(fieldsMatch('p', p, p, 4)).toEqual({ ok: true });
  expect(fieldsMatch('p', undefined, p, 4)).toEqual({ ok: false, reason: 'the baseline run has no field p at that snapshot' });
  expect(fieldsMatch('p', p, undefined, 4)).toEqual({ ok: false, reason: 'the compared run has no field p at that snapshot' });
  expect(fieldsMatch('p', p, { ...p, unit: 'kPa' }, 4)).toEqual({ ok: false, reason: 'p is in Pa and in kPa; the Studio converts no unit' });
  expect(fieldsMatch('p', p, { ...p, location: 'point' }, 4).ok).toBe(false);
  expect(fieldsMatch('p', p, { ...p, n: 5 }, 4).ok).toBe(false);
});

test('C4: one range is exactly the union of the two views’ ranges', () => {
  expect(unionSpan({ lo: -1, hi: 2 }, { lo: 0.5, hi: 3 })).toEqual({ lo: -1, hi: 3 });
  expect(unionSpan({ lo: -1, hi: 2 }, null)).toEqual({ lo: -1, hi: 2 });
  expect(unionSpan(null, null)).toBeNull();
});

test.describe('the difference through the reader worker', () => {
  let local: LocalData;
  test.beforeAll(() => {
    expect(existsSync(WORKER), `the built worker ${WORKER.href} (run npm run build)`).toBe(true);
    local = new LocalData(DataRoots.fromEnv({ RCFD_STUDIO_DATA_ROOTS: examples }), WORKER);
  });
  test.afterAll(async () => {
    await local?.close();
  });

  const values = async (p: Promise<unknown>): Promise<Float64Array> => {
    const r = (await p) as { kind: string; values: ArrayBuffer };
    expect(r.kind).toBe('difference');
    return new Float64Array(r.values);
  };
  const failure = async (p: Promise<unknown>): Promise<DataFailure> => {
    try {
      await p;
    } catch (e) {
      if (e instanceof DataFailure) return e;
      throw e;
    }
    throw new Error('expected a refusal');
  };

  test('equals an independent f64 computation from the stored bytes, bit for bit, at every common snapshot', async () => {
    const a = snapshotDirs(RE100);
    const b = snapshotDirs(RE400);
    let n = 0;
    for (let i = 0; i < a.length; i++) {
      const j = b.findIndex((s) => s.step === a[i]!.step);
      if (j < 0) continue;
      for (const f of ['p', 'ux', 'uy']) {
        const got = await values(local.difference(RE100, i, RE400, j, f).promise);
        const va = decodeField64(RE100, i, f);
        const vb = decodeField64(RE400, j, f);
        expect(got.length).toBe(1024);
        const want = vb.map((x, c) => x - va[c]!);
        expect([...got].map(bits64)).toEqual([...want].map(bits64));
        n++;
      }
    }
    expect(n).toBeGreaterThan(60);
    // the stores hold f32: the widened values are the f32 the Studio draws elsewhere (F-804), exactly
    const s = open(RE100);
    try {
      const f32 = new Float32Array(s.field(3, 'p'));
      const f64 = new Float64Array(s.field64(3, 'p'));
      expect([...f64]).toEqual([...f32]);
      expect([...decodeField64(RE100, 3, 'p')]).toEqual([...f32]);
    } finally {
      s.close();
    }
  });

  test('refuses different meshes, a field one run lacks and a different number of values, saying why', async () => {
    expect((await failure(local.difference(RE100, 0, HEATED, 0, 'p').promise)).message).toBe('no difference: the meshes differ: 1024 and 1600 cells');
    const fa = facts(FOIL_A);
    const fb = facts(FOIL_B);
    expect((await failure(local.difference(FOIL_A, 0, FOIL_B, 0, 'p').promise)).message).toBe(
      `no difference: the meshes differ: mesh ${fa.meshHash} and ${fb.meshHash}, both of 6144 cells`,
    );
    expect((await failure(local.difference(HEATED, 0, HEATED3, 0, 'nope').promise)).message).toBe('no difference: the baseline run has no field nope at that snapshot');
    const f = await failure(local.difference(RE100, 0, path.join(os.tmpdir(), 'not-a-root'), 0, 'p').promise);
    expect(f.code).toBe('not_allowed');
  });

  test('two runs of one mesh: the heated cavities’ T, and the identical copies of a store give exactly zero', async () => {
    const got = await values(local.difference(HEATED, 4, HEATED3, 4, 'T').promise);
    const want = decodeField64(HEATED3, 4, 'T').map((x, c) => x - decodeField64(HEATED, 4, 'T')[c]!);
    expect([...got].map(bits64)).toEqual([...want].map(bits64));
    // a store against itself: rcfd-post's difference of identical solutions is exactly zero
    const zero = await values(local.difference(HEATED, 4, HEATED, 4, 'T').promise);
    expect(zero.every((x) => Object.is(x, 0))).toBe(true);
  });

  test('the byte-shuffled, deflated store of f447_bench decodes the same way (skipped without f447_bench)', async () => {
    const bench = findBench();
    test.skip(!bench, 'the f447_bench example was not found: build it with `cargo build --release -p rcfd-api --example f447_bench`');
    test.setTimeout(240_000);
    const root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f815-'));
    const reader = new LocalData(DataRoots.fromEnv({ RCFD_STUDIO_DATA_ROOTS: root }), WORKER);
    try {
      await generate(bench!, root, ['tiny']);
      const raw = storeDir(root, 'tiny', 'raw');
      const deflated = storeDir(root, 'tiny', 'deflate');
      expect(snapshotDirs(deflated)[0]!.fields[0]!.codec).toBe('shuffle_deflate');
      // the raw store and its deflated copy hold the same values: their difference is zero everywhere
      const same = await values(reader.difference(raw, 19, deflated, 19, 'p').promise);
      expect(same.every((x) => Object.is(x, 0))).toBe(true);
      const got = await values(reader.difference(raw, 10, deflated, 19, 'ux').promise);
      const want = decodeField64(deflated, 19, 'ux').map((x, c) => x - decodeField64(raw, 10, 'ux')[c]!);
      expect([...got].map(bits64)).toEqual([...want].map(bits64));
      expect(got.some((x) => x !== 0)).toBe(true);
    } finally {
      await reader.close();
      rmSync(root, { recursive: true, force: true });
    }
  });
});

test('the data port takes a difference of exactly two stores, two snapshots and a field; the service route refuses it', async () => {
  const local = { route: 'local', path: path.join(os.tmpdir(), 'a') } as const;
  const other = { route: 'local', path: path.join(os.tmpdir(), 'b') } as const;
  const ok = { op: 'difference', id: 1, store: local, snapshot: 3, other, otherSnapshot: 4, field: 'p', tag: 'x' };
  expect(parseDataRequest(ok)).toEqual({ ok: true, request: ok });
  for (const bad of [
    { ...ok, other: undefined },
    { ...ok, other: { route: 'file', path: 'x' } },
    { ...ok, otherSnapshot: -1 },
    { ...ok, snapshot: 1.5 },
    { ...ok, field: '' },
    { ...ok, extra: 1 },
  ]) {
    const r = parseDataRequest(JSON.parse(JSON.stringify(bad)));
    expect(r.ok, JSON.stringify(bad)).toBe(false);
  }
  const data = new StudioData({ state: { address: null, instance: 0 }, call: async () => ({}) } as never, DataRoots.fromEnv({}), WORKER);
  try {
    const svc = { route: 'service', path: 'runs/a/fields' } as const;
    await expect(data.run({ op: 'difference', id: 2, store: svc, snapshot: 0, other: svc, otherSnapshot: 0, field: 'p' }, 1000).promise).rejects.toMatchObject({ code: 'bad_request' });
    await expect(data.run({ op: 'difference', id: 3, store: local, snapshot: 0, other: svc, otherSnapshot: 0, field: 'p' }, 1000).promise).rejects.toMatchObject({ code: 'bad_request' });
  } finally {
    await data.close();
  }
});

test('the run port takes compare of two different runs; compare_runs’ reply becomes the metrics of both and the engine’s difference', async () => {
  expect(RUN_OPS).toEqual(['series', 'at']);
  expect(RUN_COMPARE_OPS).toEqual(['compare']);
  expect(PROJECT_TOOLS).toContain('compare_runs');
  expect(parseRunRequest({ id: 1, op: 'compare', run: 'r-1', other: 'r-2' })).toEqual({ ok: true, request: { id: 1, op: 'compare', run: 'r-1', other: 'r-2' } });
  for (const bad of [
    { id: 1, op: 'compare', run: 'r-1', other: 'r-1' },
    { id: 1, op: 'compare', run: 'r-1' },
    { id: 1, op: 'compare', run: 'r-1', other: '../x' },
    { id: 1, op: 'compare', run: 'r-1', other: 'r-2', x: 1 },
  ]) {
    expect(parseRunRequest(bad).ok, JSON.stringify(bad)).toBe(false);
  }
  const calls: unknown[] = [];
  const query: Query = async (tool, args) => {
    calls.push([tool, args]);
    return { baseline: 'r-1', runs: [], metrics: { cd: { 'r-1': 1.5, 'r-2': 1.25 }, only_b: { 'r-2': 3 } }, differences: { cd: { 'r-2': -0.25 }, only_b: {} }, metrics_total: 2, metrics_omitted: 0 };
  };
  expect(await readComparison(query, 'r-1', 'r-2')).toEqual({
    baseline: 'r-1',
    compared: 'r-2',
    metrics: [
      { name: 'cd', a: 1.5, b: 1.25, difference: -0.25 },
      { name: 'only_b', a: null, b: 3, difference: null },
    ],
    omitted: 0,
  });
  expect(calls).toEqual([['compare_runs', { runs: ['r-1', 'r-2'] }]]);
});

test('compare_runs of the real project server: the two cavities’ metrics and the engine’s difference', async () => {
  const mcp = findMcp();
  test.skip(!mcp, NO_MCP);
  const dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f815-cr-'));
  const quick = await copyExample('quick-start', dir);
  const server = await ProjectServer.start(mcp!, quick);
  try {
    const query: Query = (tool, args) => server.call(tool, args);
    const c = await readComparison(query, 'r-20261028T152641-f8c2', 'r-20261028T152641-97f1');
    const m = (run: string) => JSON.parse(readFileSync(path.join(quick, 'runs', run, 'metrics.json'), 'utf8')) as { reports: Record<string, number> };
    const a = m('r-20261028T152641-f8c2').reports;
    const b = m('r-20261028T152641-97f1').reports;
    const drag = c.metrics.find((x) => x.name === 'lid_drag')!;
    expect(drag).toEqual({ name: 'lid_drag', a: a['lid_drag'], b: b['lid_drag'], difference: b['lid_drag']! - a['lid_drag']! });
    expect(c.omitted).toBe(0);
  } finally {
    await server.close();
    for (const p of mcpPids(quick)) killPid(p);
    rmSync(dir, { recursive: true, force: true });
  }
});

test('the palette has Compare runs, found first by its title and its id; the project view runs it', () => {
  const c = STUDIO_COMMANDS.find((x) => x.id === 'workspace.compare')!;
  expect(c).toMatchObject({ title: 'Compare runs', group: 'Project runs', mcp: 'compare_runs' });
  expect(c.params).toEqual([expect.objectContaining({ name: 'run', kind: 'run', optional: false })]);
  const palette = paletteModel();
  expect(searchPalette(palette, 'Compare runs')[0]!.id).toBe('workspace.compare');
  expect(searchPalette(palette, 'workspace.compare')[0]!.id).toBe('workspace.compare');
  const renderer = path.join(appRoot, 'src', 'renderer');
  expect(readFileSync(path.join(renderer, 'project', 'ProjectHome.tsx'), 'utf8')).toContain(`'workspace.compare':`);
  expect(readFileSync(path.join(renderer, 'commands', 'run.ts'), 'utf8')).toContain(`case 'workspace.compare':`);
});

test('the comparison’s pictograms are on the 14-unit grid at 0, 45 and 90 degrees; every control has a name', () => {
  for (const [name, d] of Object.entries(COMPARE_ICON)) {
    const tokens = d.match(/[MLHVZ]|-?\d+/g) ?? [];
    expect(tokens.join(''), name).toBe(d.replace(/[\s,]/g, ''));
    let i = 0;
    let cur = [0, 0];
    let start = [0, 0];
    let cmd = '';
    while (i < tokens.length) {
      if (/[MLHVZ]/.test(tokens[i]!)) cmd = tokens[i++]!;
      let next: number[];
      if (cmd === 'Z') next = start;
      else if (cmd === 'M' || cmd === 'L') next = [Number(tokens[i++]), Number(tokens[i++])];
      else if (cmd === 'H') next = [Number(tokens[i++]), cur[1]!];
      else next = [cur[0]!, Number(tokens[i++])];
      for (const v of next) expect(v >= 0 && v <= pictogram.units && Number.isInteger(v), `${name}: ${v}`).toBe(true);
      if (cmd === 'M') {
        start = next;
        cmd = 'L';
      } else {
        const deg = ((Math.atan2(next[1]! - cur[1]!, next[0]! - cur[0]!) * 180) / Math.PI + 360) % 180;
        expect([0, 45, 90, 135].some((a) => Math.abs(deg - a) < 1e-9), `${name}: ${deg}`).toBe(true);
        if (cmd === 'Z') cmd = '';
      }
      cur = next;
    }
  }
  expect(Object.values(COMPARE_NAME).every((n) => n.length > 0)).toBe(true);
});

test('workspace/compare.css uses only defined tokens and no colour literals; corners square, no shadow', () => {
  const css = readFileSync(path.join(appRoot, 'src', 'renderer', 'workspace', 'compare.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(10);
  expect(used.filter((v) => !defined.has(v))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  expect(css).not.toMatch(/border-radius|box-shadow/);
});
