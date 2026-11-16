// The run list and run state (F-806) without a window: the engine's answers as the Studio reads them (src/main/project/
// runs.ts), what the runs table shows of them (src/renderer/runs/model.ts: the state, the result, the time in words where
// the engine has no simulated time, sort and filter), the session's live reads of active runs, the paused and cancelled
// pictograms on the DESIGN.md grid, and the stylesheet's tokens.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { RUN_EVENTS, type ProjectTool, type RunDetail, type RunEntry } from '../../src/shared/project.js';
import { RpcError } from '../../src/main/service/rpc.js';
import { isActive, readRuns, runDetail, runEntry, stale } from '../../src/main/project/runs.js';
import { ProjectSession } from '../../src/main/project/session.js';
import type { ProjectServerLike } from '../../src/main/project/server.js';
import { filterRows, result, runRows, shownState, sortRows, stateCounts, timeNote, timeText, type RunRow } from '../../src/renderer/runs/model.js';
import { pictogram } from '../../src/renderer/design/tokens.js';
import { ICON_PATHS, RUN_STATES, RUNNER_STATES, RUNNER_STATE_SHAPES, STATE_SHAPES, markShape, markWord } from '../../src/renderer/ui/pictogramPaths.js';
import { appRoot } from '../support/rcfd.js';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { until } from '../support/fake-service.js';

// ---- the engine's answers (shapes from docs/mcp-tools.md and example-projects/quick-start)

const steadyStatus = {
  run: 'r-1',
  case: 'cases/cavity-re100.toml',
  created: '2026-10-28T15:26:41.773Z',
  solver_version: 'rcfd 0.1.0',
  settings: {},
  status: { state: 'finished', step: 278, time: 0.0, pid: null, updated: '2026-10-28T15:26:47.516Z', reason: 'converged: residual targets met: res_u 5.920e-7 < 1.000e-6', exit_code: 0 },
  last_residual: null,
  events: [
    { event: 'state', kind: 'state', from: null, to: 'queued', state: 'queued', reason: 'created', t: '2026-10-28T15:26:41.773Z' },
    { kind: 'estimate', cells: 1600, t: '2026-10-28T15:26:42.114Z' },
    { kind: 'launched', pid: 67540, t: '2026-10-28T15:26:45.443Z' },
    { kind: 'state', from: 'queued', to: 'running', reason: 'launched', t: '2026-10-28T15:26:45.444Z' },
    { kind: 'solver', solver: { event: 'start', steady: true, cells: 1600 }, t: '2026-10-28T15:26:45.569Z' },
    { kind: 'solver', solver: { event: 'finish', state: 'converged', iterations: 278, elapsed_s: 1.9237942, reason: 'residual targets met' }, t: '2026-10-28T15:26:47.497Z' },
    { kind: 'state', from: 'running', to: 'finished', reason: 'converged: residual targets met', t: '2026-10-28T15:26:47.517Z' },
  ],
};

test.describe('the engine answers', () => {
  test('run_status: status.json, the steady flag of the solver start, the finish, the state changes', () => {
    const d = runDetail('r-1', steadyStatus, '2026-11-16T10:00:00Z');
    expect(d).toMatchObject({ id: 'r-1', state: 'finished', step: 278, time: 0, pid: null, exitCode: 0, steady: true, solverVersion: 'rcfd 0.1.0' });
    expect(d.finish).toEqual({ state: 'converged', iterations: 278, elapsed: 1.9237942, reason: 'residual targets met' });
    expect(d.history.map((h) => [h.from, h.to])).toEqual([
      [null, 'queued'],
      ['queued', 'running'],
      ['running', 'finished'],
    ]);
    // a NaN time is written as null: no number, never 0
    expect(runDetail('r-2', { ...steadyStatus, status: { ...steadyStatus.status, time: null } }, 'x').time).toBeNull();
    // no start event among the events read: unknown, not steady
    expect(runDetail('r-3', { ...steadyStatus, events: [] }, 'x').steady).toBeNull();
    expect(() => runDetail('r-4', undefined, 'x')).toThrow(/no status/);
  });

  test('list_runs: a null time stays null, report values are kept, pages are read to the end', async () => {
    expect(runEntry({ id: 'a', case: 'cases/x.toml', created: 't', state: 'running', step: 3, time: null, metrics: { cd: 1.5, bad: 'x' } })).toEqual({
      id: 'a',
      case: 'cases/x.toml',
      created: 't',
      state: 'running',
      step: 3,
      time: null,
      metrics: { cd: 1.5 },
    });
    const calls: unknown[] = [];
    const server = {
      info: { name: 'rcfd', version: 't' },
      exited: false,
      call: async (_t: ProjectTool, args?: Record<string, unknown>) => {
        calls.push(args);
        const off = Number(args?.['offset']);
        return { runs: [{ id: `r${off}`, state: 'finished', step: 1, time: 0 }], total: 3, next_offset: off < 2 ? off + 1 : null, skipped: [] };
      },
      onExit: () => undefined,
      close: async () => undefined,
    } satisfies ProjectServerLike;
    const read = await readRuns(server);
    expect(read.runs.items.map((r) => r.id)).toEqual(['r0', 'r1', 'r2']);
    expect(calls).toHaveLength(3);
  });

  test('which runs are active and which details are stale', () => {
    expect(['queued', 'running', 'paused'].every(isActive)).toBe(true);
    expect(['finished', 'failed', 'cancelled'].some(isActive)).toBe(false);
    const e = runEntry({ id: 'r-1', state: 'finished', step: 278 });
    const d = runDetail('r-1', steadyStatus, 'x');
    expect(stale(e, undefined)).toBe(true);
    expect(stale(e, d)).toBe(false);
    expect(stale({ ...e, step: 279 }, d)).toBe(true);
    expect(stale({ ...e, state: 'failed' }, d)).toBe(true);
  });
});

// ---- what the table shows

const detail = (over: Partial<RunDetail>): RunDetail => ({
  id: 'r',
  state: 'running',
  step: 10,
  time: 0,
  updated: '2026-11-16T10:00:00Z',
  pid: 42,
  reason: null,
  exitCode: null,
  solverVersion: 'rcfd 0.1.0',
  steady: null,
  finish: null,
  history: [],
  readAt: '2026-11-16T10:00:01Z',
  ...over,
});

const row = (state: string, over: Partial<RunDetail> | null, entry: Partial<RunEntry> = {}): RunRow => {
  const e: RunEntry = { id: entry.id ?? `r-${state}`, case: entry.case ?? 'cases/a.toml', created: 't', state, step: 10, time: 0, metrics: {}, ...entry };
  const d = over === null ? {} : { [e.id]: detail({ id: e.id, state, step: e.step, time: e.time, ...over }) };
  return runRows([e], d, {})[0]!;
};

test.describe('the runs table', () => {
  test('state: the runner state, Converged when the solver says so, paused and cancelled with their own marks', () => {
    expect(shownState(row('finished', { finish: { state: 'converged', iterations: 10, elapsed: 1, reason: null } }))).toBe('converged');
    expect(shownState(row('finished', { finish: { state: 'not_converged', iterations: 10, elapsed: 1, reason: null } }))).toBe('finished');
    expect(shownState(row('finished', null))).toBe('finished');
    for (const s of ['queued', 'running', 'paused', 'failed', 'cancelled'] as const) expect(shownState(row(s, null))).toBe(s);
    expect(shownState(row('sleeping', null))).toBeNull();
  });

  test('G3: time is the engine number only where it is a simulated time; otherwise words, never 0', () => {
    // the F-805 finding: a steady run's solver reports time 0.0
    expect(timeText(row('finished', { steady: true }))).toBe('none, steady');
    expect(timeText(row('running', { steady: true }))).toBe('none, steady');
    expect(timeText(row('finished', { steady: false, time: 80 }, { time: 80 }))).toBe('80');
    expect(timeText(row('running', { steady: false, time: 0.4125 }, { time: 0.4125 }))).toBe('0.4125');
    expect(timeText(row('queued', null))).toBe('none yet');
    expect(timeText(row('failed', { steady: null, step: 0 }, { step: 0 }))).toBe('none');
    expect(timeText(row('running', { steady: null, time: null }, { time: null }))).toBe('none reported');
    // unknown steadiness: a 0 is not shown as a number; any other value is a simulated time
    expect(timeText(row('finished', { steady: null }))).toBe('not known');
    expect(timeNote(row('finished', { steady: null, time: 3 }, { time: 3 }))).toBeNull();
  });

  test('result: where the run is, and the runner or solver sentence', () => {
    expect(result(row('finished', { steady: true, step: 278, reason: 'converged: residual targets met', finish: { state: 'converged', iterations: 278, elapsed: 1.9, reason: null } }, { step: 278 }))).toEqual({
      head: '278 iterations',
      note: 'residual targets met',
    });
    expect(result(row('finished', { steady: true, reason: 'not_converged: iteration limit 3000 reached', finish: { state: 'not_converged', iterations: 3000, elapsed: 55, reason: null } })).note).toBe(
      'Not converged: iteration limit 3000 reached',
    );
    expect(result(row('finished', { steady: false, time: 80, reason: 'finished: end time 80 reached', finish: { state: 'finished', iterations: 1280, elapsed: 9, reason: null } }, { time: 80, step: 1280 }))).toEqual({
      head: 't = 80 s, 1280 steps',
      note: 'end time 80 reached',
    });
    expect(result(row('running', { steady: true, step: 2244 }, { step: 2244 })).head).toBe('iteration 2244');
    expect(result(row('failed', { step: 0, reason: 'pre-launch check failed (exit 3)' }, { step: 0 }))).toEqual({ head: 'Failed before its first step', note: 'pre-launch check failed (exit 3)' });
    expect(result(row('cancelled', { steady: true, step: 2463, reason: 'cancelled on request' }, { step: 2463 })).head).toBe('Stopped at iteration 2463');
    expect(result(row('queued', null)).head).toBe('Not started');
  });

  test('sort by case and run, by state (what needs a look first) and by result; filter by text and state', () => {
    const rows = [
      row('finished', { finish: { state: 'converged', iterations: 5, elapsed: 1, reason: null } }, { id: 'r-b', case: 'cases/b.toml', step: 5 }),
      row('failed', {}, { id: 'r-a2', case: 'cases/a.toml', step: 0 }),
      row('running', {}, { id: 'r-a1', case: 'cases/a.toml', step: 50 }),
      row('cancelled', {}, { id: 'r-c', case: 'cases/c.toml', step: 7 }),
    ];
    const ids = (rs: RunRow[]) => rs.map((r) => r.id);
    expect(ids(sortRows(rows, { column: 'case', direction: 'ascending' }))).toEqual(['r-a1', 'r-a2', 'r-b', 'r-c']);
    expect(ids(sortRows(rows, { column: 'case', direction: 'descending' }))).toEqual(['r-c', 'r-b', 'r-a2', 'r-a1']);
    expect(ids(sortRows(rows, { column: 'state', direction: 'ascending' }))).toEqual(['r-a1', 'r-a2', 'r-c', 'r-b']);
    expect(ids(sortRows(rows, { column: 'result', direction: 'descending' }))).toEqual(['r-a1', 'r-c', 'r-b', 'r-a2']);
    expect(ids(filterRows(rows, 'A.TOML', 'all'))).toEqual(['r-a2', 'r-a1']);
    expect(ids(filterRows(rows, 'r-c', 'all'))).toEqual(['r-c']);
    expect(ids(filterRows(rows, '', 'converged'))).toEqual(['r-b']);
    expect(ids(filterRows(rows, '', 'failed'))).toEqual(['r-a2']);
    expect(Object.fromEntries(stateCounts(rows))).toEqual({ converged: 1, failed: 1, running: 1, cancelled: 1 });
  });

  test('the run_status answer, read after the list, wins over the list entry', () => {
    const e: RunEntry = { id: 'r', case: 'cases/a.toml', created: 't', state: 'running', step: 10, time: 0, metrics: {} };
    const [r] = runRows([e], { r: detail({ state: 'paused', step: 12 }) }, { r: 'x' });
    expect(r).toMatchObject({ state: 'paused', step: 12, detailError: 'x' });
    expect(runRows([e], {}, {})[0]).toMatchObject({ state: 'running', step: 10, detail: null, detailError: null });
  });
});

// ---- the session: active runs are read live, ended ones once

class RunsServer implements ProjectServerLike {
  readonly info = { name: 'rcfd', version: 't' };
  exited = false;
  calls: { tool: ProjectTool; args?: Record<string, unknown> }[] = [];
  runs: { id: string; state: string; step: number }[] = [
    { id: 'r-done', state: 'finished', step: 278 },
    { id: 'r-live', state: 'running', step: 1 },
  ];
  async call(tool: ProjectTool, args?: Record<string, unknown>): Promise<unknown> {
    this.calls.push({ tool, args });
    switch (tool) {
      case 'project_info':
        return { name: 'p', counts: {}, issues: [], issues_total: 0 };
      case 'list_runs':
        return { runs: this.runs.map((r) => ({ ...r, case: 'cases/a.toml', created: 't', time: 0 })), total: this.runs.length, next_offset: null, skipped: [] };
      case 'list_meshes':
        return { meshes: [], total: 0, next_offset: null, unread: [] };
      case 'read_feed':
        return { cursor: 0, entries: [], next: null, dropped: 0 };
      case 'run_status': {
        const r = this.runs.find((x) => x.id === args?.['run']);
        if (!r) throw new RpcError(1202, 'no such run');
        expect(args?.['events']).toBe(RUN_EVENTS);
        return { ...steadyStatus, run: r.id, status: { ...steadyStatus.status, state: r.state, step: r.step } };
      }
    }
  }
  onExit(): void {}
  async close(): Promise<void> {
    this.exited = true;
  }
}

test('the session reads an active run again as it goes, an ended run once, and drops a run that is gone', async () => {
  const server = new RunsServer();
  const s = await ProjectSession.open('/f', '/e/rcfd-mcp', { startServer: async () => server, listFiles: async () => ({ names: [], total: 0 }), now: () => new Date().toISOString(), pollMs: 20, runPollMs: 50 }, () => undefined);
  try {
    await until(async () => (Object.keys(s.view.runDetails).length === 2 ? true : null), 5000, 'both details');
    server.runs[1]!.step = 40;
    await until(async () => (s.view.runDetails['r-live']?.step === 40 ? true : null), 5000, 'the live step');
    expect(s.view.runs.items.find((r) => r.id === 'r-live')?.step).toBe(40);
    server.runs[1]!.state = 'cancelled';
    server.runs[1]!.step = 41;
    await until(async () => (s.view.runDetails['r-live']?.state === 'cancelled' ? true : null), 5000, 'the end of the live run');
    const statusCalls = (id: string) => server.calls.filter((c) => c.tool === 'run_status' && c.args?.['run'] === id).length;
    expect(statusCalls('r-done')).toBe(1);
    // nothing is active any more: no more reads of the list without a feed entry
    const lists = server.calls.filter((c) => c.tool === 'list_runs').length;
    await new Promise((d) => setTimeout(d, 300));
    expect(server.calls.filter((c) => c.tool === 'list_runs').length).toBe(lists);
    // a run removed from the folder loses its detail on the next read
    server.runs = server.runs.filter((r) => r.id !== 'r-done');
    await s.refresh();
    expect(Object.keys(s.view.runDetails)).toEqual(['r-live']);
    // the engine's error for one run is kept with that run, in its words
    server.runs.push({ id: 'r-gone', state: 'finished', step: 1 });
    const orig = server.call.bind(server);
    server.call = async (tool, args) => (tool === 'run_status' && args?.['run'] === 'r-gone' ? Promise.reject(new RpcError(1202, 'runs/r-gone/status.json: not found')) : orig(tool, args));
    await s.refresh();
    expect(s.view.runDetailErrors).toEqual({ 'r-gone': 'runs/r-gone/status.json: not found' });
  } finally {
    await s.close();
  }
});

// ---- the paused and cancelled pictograms (proposed for DESIGN.md)

type Pt = readonly [number, number];
function segments(d: string): Array<[Pt, Pt]> {
  const tokens = d.match(/[MLHVZ]|-?\d+(\.\d+)?/g) ?? [];
  expect(tokens.join('')).toBe(d.replace(/[\s,]/g, ''));
  const out: Array<[Pt, Pt]> = [];
  let i = 0;
  let cur: Pt = [0, 0];
  let cmd = '';
  const num = (): number => Number(tokens[i++]);
  while (i < tokens.length) {
    if (/[MLHVZ]/.test(tokens[i]!)) cmd = tokens[i++]!;
    let next: Pt;
    if (cmd === 'M' || cmd === 'L') next = [num(), num()];
    else if (cmd === 'H') next = [num(), cur[1]];
    else next = [cur[0], num()];
    if (cmd === 'M') cmd = 'L';
    else out.push([cur, next]);
    cur = next;
  }
  return out;
}

test('paused and cancelled: on the 14-unit grid, strokes at 0, 45 and 90 degrees, unlike every other mark', () => {
  expect([...RUNNER_STATES]).toEqual(['paused', 'cancelled']);
  for (const s of RUNNER_STATES) {
    const d = RUNNER_STATE_SHAPES[s].path;
    const segs = segments(d);
    expect(segs.length).toBeGreaterThan(0);
    for (const [a, b] of segs) {
      for (const v of [...a, ...b]) {
        expect(Number.isInteger(v), `${s}: ${v}`).toBe(true);
        expect(v >= 0 && v <= pictogram.units, `${s}: ${v} inside the square`).toBe(true);
      }
      const deg = ((Math.atan2(b[1] - a[1], b[0] - a[0]) * 180) / Math.PI + 360) % 180;
      expect(pictogram.angles.some((g) => Math.abs(deg - g) < 1e-9) || Math.abs(deg - 135) < 1e-9, `${s}: ${deg}`).toBe(true);
    }
    expect(markWord(s).toLowerCase()).toBe(s);
    expect(markShape(s)).toEqual(RUNNER_STATE_SHAPES[s]);
  }
  const others = [...RUN_STATES.map((s) => JSON.stringify(STATE_SHAPES[s])), ...Object.values(ICON_PATHS).map((i) => JSON.stringify({ path: i.path }))];
  for (const s of RUNNER_STATES) expect(others).not.toContain(JSON.stringify(RUNNER_STATE_SHAPES[s]));
  expect(RUNNER_STATE_SHAPES.paused.path).not.toBe(RUNNER_STATE_SHAPES.cancelled.path);
});

test('runs/runs.css uses only defined tokens and no colour literals; corners stay square', () => {
  const css = readFileSync(path.join(appRoot, 'src', 'renderer', 'runs', 'runs.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(0);
  expect(used.filter((v) => !defined.has(v))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  expect(css).not.toMatch(/border-radius|box-shadow|opacity/);
});
