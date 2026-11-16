// The run workspace (F-810) without a window: the scrubber's stops and the shared axis (src/renderer/workspace/model.ts),
// the run bands of DESIGN.md with One Live Thing, the run port's request check and the reading of a run's histories
// through the engine's read_residuals and read_project_file (src/main/run/), the one renderer module that names
// window.studioRunData, and the stylesheet's tokens.
import { expect, test } from '@playwright/test';
import { readdirSync, readFileSync } from 'node:fs';
import { MessageChannel } from 'node:worker_threads';
import path from 'node:path';
import type { StoreInfo } from '../../src/shared/data.js';
import type { RunPortMessage } from '../../src/shared/run.js';
import { RUN_OPS, SERIES_POINTS, UNITS_MAX_BYTES } from '../../src/shared/run.js';
import { parseRunRequest, RunPortBridge, type RunPortLike } from '../../src/main/run/bridge.js';
import { plotName, readRunAt, readRunSeries, readUnits, type Query } from '../../src/main/run/series.js';
import {
  bandOf,
  counterText,
  defaultField,
  flagText,
  fraction,
  labelAfter,
  lastStep,
  liveRun,
  nearestPosition,
  positionsOf,
  sampleAt,
  sharedAxis,
  simulatedTime,
  storePath,
  valueText,
} from '../../src/renderer/workspace/model.js';
import type { RunRow } from '../../src/renderer/runs/model.js';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { appRoot } from '../support/rcfd.js';
import { until } from '../support/fake-service.js';

const row = (o: Partial<RunRow> & Pick<RunRow, 'id' | 'state' | 'step'>): RunRow => ({ case: 'cases/c.toml', created: '2026-11-16T10:00:00Z', time: null, metrics: {}, detail: null, detailError: null, ...o });
const converged = { steady: true, finish: { state: 'converged' } } as unknown as RunRow['detail'];

const store = (steps: number[], times?: number[]): StoreInfo =>
  ({
    route: 'local',
    path: '/p/runs/r-1/fields',
    nCells: 4,
    nPoints: 9,
    meshHash: '0123456789abcdef',
    snapshots: steps.map((step, i) => ({ step, time: times?.[i] ?? 0 })),
    fields: [
      { name: 'ux', location: 'cell', components: 1, precision: 'f32' },
      { name: 'p', location: 'cell', components: 1, precision: 'f32' },
    ],
    index: 'footer',
  }) as unknown as StoreInfo;

// ---- stops, values, axis

test('the stops are the store’s snapshots; a run without fields stops at its residual samples', () => {
  expect(positionsOf(store([5, 10, 15]), null, true)).toEqual([
    { step: 5, time: null, snapshot: 0 },
    { step: 10, time: null, snapshot: 1 },
    { step: 15, time: null, snapshot: 2 },
  ]);
  // a transient run keeps its simulated times
  expect(positionsOf(store([1, 2], [0.01, 0.02]), null, false).map((p) => p.time)).toEqual([0.01, 0.02]);
  const series = { series: [{ name: 'lid_drag', kind: 'monitor', x: [1, 2], y: [0, 0] }, { name: 'continuity', kind: 'residual', x: [1, 2, 3], y: [1, 1, 1] }] } as never;
  expect(positionsOf(store([]), series, true)).toEqual([1, 2, 3].map((step) => ({ step, time: null, snapshot: null })));
  expect(positionsOf(null, null, null)).toEqual([]);
});

test('a time is a simulated time only for a transient run; 0 of a run not known transient is none', () => {
  expect(simulatedTime(0.5, false)).toBe(0.5);
  expect(simulatedTime(0.5, true)).toBeNull();
  expect(simulatedTime(0, null)).toBeNull();
  expect(simulatedTime(0.25, null)).toBe(0.25);
  expect(simulatedTime(Number.NaN, false)).toBeNull();
});

test('the nearest stop, and the exact value of a series at a step (never interpolated)', () => {
  const ps = positionsOf(store([5, 10, 15, 20]), null, true);
  expect(nearestPosition(ps, 0)).toBe(0);
  expect(nearestPosition(ps, 7)).toBe(0);
  expect(nearestPosition(ps, 7.5)).toBe(0);
  expect(nearestPosition(ps, 8)).toBe(1);
  expect(nearestPosition(ps, 99)).toBe(3);
  expect(nearestPosition([], 3)).toBe(-1);
  const x = [1, 2, 4, 8];
  const y = [0.1, 0.2, 0.4, 0.8];
  expect(sampleAt(x, y, 4)).toBe(0.4);
  expect(sampleAt(x, y, 3)).toBeNull();
  expect(sampleAt(x, y, 9)).toBeNull();
  expect(sampleAt([], [], 1)).toBeNull();
});

test('one shared axis from 0 past the furthest run; a band ends at its last step', () => {
  const a = sharedAxis([121, 158, 339, 278, 0]);
  expect(a.lo).toBe(0);
  expect(a.hi).toBeGreaterThanOrEqual(339);
  expect(sharedAxis([]).hi).toBeGreaterThanOrEqual(1);
  expect(fraction(a.hi, a)).toBe(1);
  expect(fraction(0, a)).toBe(0);
  expect(lastStep({ step: 95 }, positionsOf(store([105, 110]), null, true))).toBe(110);
  expect(lastStep({ step: 121 }, positionsOf(store([120]), null, true))).toBe(121);
  expect(labelAfter(0.5 * a.hi, a)).toBe(true);
  expect(labelAfter(0.9 * a.hi, a)).toBe(false);
});

// ---- the run bands

test('the bands: iterating then a converged or failed end cap, each end with its state word; queued hatched', () => {
  const c = bandOf(row({ id: 'a', state: 'finished', step: 121, detail: converged }), 121, 400, false);
  expect(c.label).toBe('converged · it 121');
  expect(c.state).toBe('converged');
  expect(c.segments).toEqual([
    { kind: 'iterating', from: 0, to: 121 },
    { kind: 'converged', from: 121, to: 121, cap: true },
  ]);
  const f = bandOf(row({ id: 'b', state: 'failed', step: 0 }), 0, 400, false);
  expect(f.label).toBe('failed · it 0');
  expect(f.segments).toEqual([{ kind: 'failed', from: 0, to: 0, cap: true }]);
  const q = bandOf(row({ id: 'c', state: 'queued', step: 0 }), 0, 400, false);
  expect(q.label).toBe('queued · not started');
  expect(q.segments).toEqual([{ kind: 'queued', from: 0, to: 400 }]);
  // a transient run's end in simulated time
  const t = bandOf(row({ id: 'd', state: 'finished', step: 300, time: 13.2, detail: { steady: false, finish: { state: 'converged' } } as never }), 300, 400, false);
  expect(t.label).toBe('converged · 13.2 s');
});

test('One Live Thing: only the live run’s band is orange; the rest of its axis is not computed yet', () => {
  const rows = [
    row({ id: 'old', state: 'running', step: 50, created: '2026-11-16T09:00:00Z' }),
    row({ id: 'new', state: 'running', step: 20, created: '2026-11-16T11:00:00Z' }),
    row({ id: 'p', state: 'paused', step: 30 }),
    row({ id: 'f', state: 'finished', step: 99 }),
  ];
  expect(liveRun(rows, null)).toBe('new');
  expect(liveRun(rows, 'old')).toBe('old');
  expect(liveRun(rows, 'f')).toBe('new');
  expect(liveRun(rows.filter((r) => r.state !== 'running'), 'p')).toBeNull();
  const bands = rows.map((r) => bandOf(r, r.step, 100, r.id === liveRun(rows, 'f')));
  expect(bands.flatMap((b) => b.segments).filter((s) => s.kind === 'live')).toEqual([{ kind: 'live', from: 0, to: 20 }]);
  expect(bands.find((b) => b.id === 'old')!.segments).toEqual([
    { kind: 'iterating', from: 0, to: 50 },
    { kind: 'pending', from: 50, to: 100 },
  ]);
  expect(bands.find((b) => b.id === 'p')!.label).toBe('paused · it 30');
});

test('the flag, the counter and the slider’s words', () => {
  const ps = positionsOf(store([5, 10]), null, true);
  expect(flagText(ps[1]!)).toBe('it 10');
  expect(flagText({ step: 145, time: 1.25, snapshot: 3 })).toBe('t = 1.25 s · it 145');
  expect(counterText(1, ps, false)).toBe('snapshot 2 of 2');
  expect(counterText(1, ps, true)).toBe('snapshot 2 of 2 computed so far');
  expect(counterText(0, [{ step: 1, time: null, snapshot: null }], false)).toBe('sample 1 of 1 · no field saved');
  expect(counterText(0, [], true)).toBe('nothing computed yet');
  expect(valueText(0, ps, false)).toBe('snapshot 1 of 2, it 5');
  expect(defaultField(store([5]))).toBe('p');
  expect(defaultField(null)).toBeNull();
  expect(storePath('C:\\p\\quick', 'r-1')).toBe('C:\\p\\quick\\runs\\r-1\\fields');
  expect(storePath('/p/quick/', 'r-1')).toBe('/p/quick/runs/r-1/fields');
});

// ---- the run port

test('the run port takes exactly a series or an at request with a run id and a step', () => {
  expect(RUN_OPS).toEqual(['series', 'at']);
  expect(parseRunRequest({ id: 1, op: 'series', run: 'r-1' })).toEqual({ ok: true, request: { id: 1, op: 'series', run: 'r-1' } });
  expect(parseRunRequest({ id: 2, op: 'at', run: 'r-1', step: 40 })).toEqual({ ok: true, request: { id: 2, op: 'at', run: 'r-1', step: 40 } });
  const refused = [
    null,
    [],
    { op: 'series', run: 'r-1' },
    { id: -1, op: 'series', run: 'r-1' },
    { id: 3, op: 'series', run: '../etc' },
    { id: 3, op: 'series', run: 'r-1', extra: 1 },
    { id: 3, op: 'at', run: 'r-1' },
    { id: 3, op: 'at', run: 'r-1', step: 1.5 },
    { id: 3, op: 'at', run: 'r-1', step: -2 },
    { id: 3, op: 'read_file', run: 'r-1' },
    { id: 3, op: 'series', run: 'x'.repeat(129) },
  ];
  for (const r of refused) expect(parseRunRequest(r).ok, JSON.stringify(r)).toBe(false);
  expect(parseRunRequest({ id: 3, op: 'series', run: 'r/1' })).toMatchObject({ ok: false, id: 3, error: { code: 'bad_request' } });
});

// a fake engine: read_residuals and read_project_file of one run, recording what the Studio asked
function fakeEngine(lines: { step: number; residuals: Record<string, number>; monitors: Record<string, number> }[], monitorsJson: string | null) {
  const calls: { tool: string; args: Record<string, unknown> }[] = [];
  const query: Query = async (tool, args) => {
    calls.push({ tool, args });
    if (tool === 'read_project_file') {
      if (monitorsJson === null) throw new Error('read C:\\p\\runs\\r-1\\output\\monitors.json: The system cannot find the path specified. (os error 3)');
      const offset = (args['offset'] as number) ?? 0;
      const page = monitorsJson.slice(offset, offset + 10);
      return { path: args['path'], size: monitorsJson.length, hash: 'h', text: page, next_offset: offset + 10 < monitorsJson.length ? offset + 10 : null };
    }
    const since = (args['since_step'] as number | undefined) ?? -Infinity;
    const max = args['max_points'] as number;
    const names = args['names'] as string[] | undefined;
    const kept = lines.filter((l) => l.step >= since);
    const all = new Map<string, { x: number[]; y: number[] }>();
    for (const l of kept) {
      const entries = [...Object.entries(l.residuals).map(([k, v]) => [`residuals.${k}`, v] as const), ...Object.entries(l.monitors).map(([k, v]) => [`monitors.${k}`, v] as const)];
      for (const [k, v] of entries) {
        if (!all.has(k)) all.set(k, { x: [], y: [] });
        all.get(k)!.x.push(l.step);
        all.get(k)!.y.push(v);
      }
    }
    const decimated = kept.length > max;
    const series: Record<string, { x: number[]; y: number[] }> = {};
    for (const [k, s] of all) {
      if (names && !names.includes(k)) continue;
      series[k] = decimated ? { x: s.x.slice(0, max), y: s.y.slice(0, max) } : s;
    }
    return { run: args['run'], lines: kept.length, skipped_lines: 0, decimated, series };
  };
  return { query, calls };
}

const LINES = [5, 10, 15].map((step) => ({ step, residuals: { continuity: 1 / step, u: 2 / step, p: 3 / step }, monitors: { lid_drag: -step / 1000 } }));
const MONITORS = JSON.stringify({ format: 'rcfd-monitors', version: 1, x_label: 'iteration', series: { lid_drag: { unit: 'N' }, continuity: {} } });

test('a run’s histories: names first, then each series on its own under SERIES_POINTS, with the units of monitors.json', async () => {
  expect(plotName('residuals.continuity')).toEqual({ name: 'continuity', kind: 'residual' });
  expect(plotName('residuals.u')).toEqual({ name: 'res_u', kind: 'residual' });
  expect(plotName('monitors.lid_drag')).toEqual({ name: 'lid_drag', kind: 'monitor' });
  expect(plotName('correctors')).toEqual({ name: 'correctors', kind: 'other' });
  const e = fakeEngine(LINES, MONITORS);
  const s = await readRunSeries(e.query, 'r-1', () => 'now');
  expect(e.calls[0]).toEqual({ tool: 'read_residuals', args: { run: 'r-1', max_points: 2 } });
  for (const c of e.calls.filter((c) => c.tool === 'read_residuals').slice(1)) {
    expect(c.args['max_points']).toBe(SERIES_POINTS);
    expect((c.args['names'] as string[]).length).toBe(1);
  }
  expect(s.series.map((x) => [x.name, x.kind, x.unit, x.decimated])).toEqual([
    ['continuity', 'residual', '', false],
    ['res_u', 'residual', '', false],
    ['res_p', 'residual', '', false],
    ['lid_drag', 'monitor', 'N', false],
  ]);
  expect(s.series[3]!.y).toEqual([-0.005, -0.01, -0.015]);
  expect(s.unitsNote).toBeNull();
  expect(s.readAt).toBe('now');
});

test('the value at a step comes from read_residuals {since_step}: the sample at that step, or none', async () => {
  const e = fakeEngine(LINES, MONITORS);
  expect(await readRunAt(e.query, 'r-1', 10)).toEqual({ run: 'r-1', step: 10, values: { continuity: 0.1, res_u: 0.2, res_p: 0.3, lid_drag: -0.01 } });
  expect(e.calls.at(-1)).toEqual({ tool: 'read_residuals', args: { run: 'r-1', since_step: 10, max_points: 2 } });
  expect((await readRunAt(e.query, 'r-1', 11)).values).toEqual({});
});

test('units: none yet while the run goes on; a too large or changing or foreign file is not read; over budget is refused', async () => {
  expect((await readUnits(fakeEngine(LINES, null).query, 'r-1')).note).toBe('no units: the run has no runs/r-1/output/monitors.json (the runner writes it when a run ends)');
  const osError2: Query = async () => {
    throw new Error('read monitors.json: os error 2');
  };
  expect((await readUnits(osError2, 'r-1')).note).toMatch(/^no units: the run has no /);
  const denied: Query = async () => {
    throw new Error('read monitors.json: os error 5');
  };
  expect((await readUnits(denied, 'r-1')).note).toMatch(/cannot be read \(read monitors.json: os error 5\)/);
  const big: Query = async () => ({ size: UNITS_MAX_BYTES + 1, hash: 'h', text: '{', next_offset: 1 });
  expect((await readUnits(big, 'r-1')).note).toMatch(/more than the Studio reads/);
  let n = 0;
  const changing: Query = async () => ({ size: 20, hash: `h${n++}`, text: '{"a":', next_offset: n < 3 ? n * 5 : null });
  expect((await readUnits(changing, 'r-1')).note).toMatch(/changed while it was read/);
  const foreign: Query = async () => ({ size: 2, hash: 'h', text: '{}', next_offset: null });
  expect((await readUnits(foreign, 'r-1')).note).toMatch(/not an rcfd-monitors document/);
  // the engine writes an over-budget reply to a file: the Studio refuses it rather than read that file
  const overBudget: Query = async () => ({ path: 'runs/r-1/mcp/residuals-1.json', bytes: 50_000 });
  await expect(readRunSeries(overBudget, 'r-1', () => 'now')).rejects.toThrow(/size budget/);
});

test('the bridge answers on its port, refuses a bad request with the reason, and says when no project is open', async () => {
  const ch = new MessageChannel();
  const adapt = (p: typeof ch.port1): RunPortLike => ({
    postMessage: (m) => p.postMessage(m),
    onMessage: (l) => p.on('message', l),
    onClose: (l) => p.on('close', l),
    close: () => p.close(),
  });
  let open = true;
  const e = fakeEngine(LINES, MONITORS);
  const bridge = new RunPortBridge(adapt(ch.port1), { query: () => (open ? e.query : null), now: () => 'now' });
  const got: RunPortMessage[] = [];
  ch.port2.on('message', (m: RunPortMessage) => got.push(m));
  ch.port2.postMessage({ id: 1, op: 'at', run: 'r-1', step: 5 });
  ch.port2.postMessage({ id: 2, op: 'at', run: '../x', step: 5 });
  await until(() => (got.length >= 2 ? true : null), 5_000, 'two replies');
  const byId = new Map(got.map((m) => [m.id, m.reply]));
  expect(byId.get(1)).toEqual({ ok: true, result: { run: 'r-1', step: 5, values: { continuity: 0.2, res_u: 0.4, res_p: 0.6, lid_drag: -0.005 } } });
  expect(byId.get(2)).toMatchObject({ ok: false, error: { code: 'bad_request' } });
  expect(bridge.refused).toBe(1);
  open = false;
  ch.port2.postMessage({ id: 3, op: 'series', run: 'r-1' });
  await until(() => (got.length >= 3 ? true : null), 5_000, 'the third reply');
  expect(got[2]!.reply).toMatchObject({ ok: false, error: { code: 'no_project' } });
  bridge.dispose();
  ch.port2.close();
});

// ---- the door and the stylesheet

const renderer = path.join(appRoot, 'src', 'renderer');
function sources(dir: string): { file: string; text: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((d) => {
    const p = path.join(dir, d.name);
    if (d.isDirectory()) return sources(p);
    if (!/\.(ts|tsx|js|jsx|mts|cts)$/.test(d.name) || d.name.endsWith('.d.ts')) return [];
    return [{ file: path.relative(renderer, p).split(path.sep).join('/'), text: readFileSync(p, 'utf8') }];
  });
}
const code = (t: string) => t.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');

test('only workspace/door.ts of the renderer names window.studioRunData (not to be confused with F-807’s studioRuns)', () => {
  const naming = sources(renderer)
    .filter((f) => /\bstudioRunData\b/.test(code(f.text)))
    .map((f) => f.file);
  expect(naming).toEqual(['workspace/door.ts']);
});

test('workspace/workspace.css uses only defined tokens and no colour literals; corners square, no shadow', () => {
  const css = readFileSync(path.join(renderer, 'workspace', 'workspace.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  // the sheet's own custom properties (the time panel's abscissa and fills) are defined in it
  const own = new Set([...css.matchAll(/(--tp-[a-z0-9-]+)\s*:/g)].map((m) => m[1]!));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(0);
  expect(used.filter((v) => !defined.has(v) && !own.has(v))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  expect(css).not.toMatch(/border-radius|box-shadow/);
  // the bands' DESIGN.md tokens
  for (const t of ['--size-time-panel', '--size-run-band', '--size-flag-lane', '--role-band-iterating', '--role-band-converged', '--role-band-live', '--role-band-failed', '--line-hatch', '--motion-band-grow', '--type-display-size']) {
    expect(used).toContain(t);
  }
  expect(readFileSync(path.join(renderer, 'workspace', 'TimePanel.tsx'), 'utf8')).toContain('var(--motion-band-stagger)');
  // the bands grow in, except under reduced motion
  expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\)\s*\{\s*\.tp-seg\s*\{\s*animation: none;/);
});
