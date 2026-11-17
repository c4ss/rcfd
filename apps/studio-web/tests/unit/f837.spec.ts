// F-837 without a window: the run's log and events (warnings named), the runner's budgets file (the runner's checks, no
// write over another writer's change, undo), the run port's new requests, the project argument, Save PNG's file, the
// field view's range, domain and scale bar, the latest residuals of run_status, and the palette's new commands.
import { expect, test } from '@playwright/test';
import { mkdtemp, readFile, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { BUDGET_KEYS, RUN_COMPARE_OPS, RUN_EXTRA_OPS, RUN_OPS } from '../../src/shared/run.js';
import { parseRunRequest } from '../../src/main/run/bridge.js';
import { budgetProblem, budgetsOf, eventLine, readRunLog, RunnerBudgetsFile, savePng, wholeRect } from '../../src/main/run/extras.js';
import type { Query } from '../../src/main/run/series.js';
import { projectArgument } from '../../src/main/project/argv.js';
import { entriesOf } from '../../src/main/project/listing.js';
import { runDetail } from '../../src/main/project/runs.js';
import { cellsOutside, domainText, fixedRange, lengthText, pngName, scaleBar } from '../../src/renderer/field/viewExtras.js';
import { ICON_NAMES } from '../../src/renderer/ui/pictogramPaths.js';
import { STUDIO_COMMANDS, GROUP_ORDER } from '../../src/shared/commands/studio.js';
import { getRunAction, runEquivalents } from '../../src/shared/actions/runs.js';

test.describe('a run’s log and events', () => {
  test('events in words: state changes, the runner’s and the solver’s warnings, stops, other events cut', () => {
    expect(eventLine({ t: 'T', kind: 'state', from: 'queued', to: 'running', reason: 'launched' })).toEqual({ t: 'T', kind: 'state', text: 'state queued -> running: launched', warning: false });
    expect(eventLine({ t: 'T', kind: 'warning', message: 'memory budget exceeded' })).toEqual({ t: 'T', kind: 'warning', text: 'warning: memory budget exceeded', warning: true });
    expect(eventLine({ t: 'T', kind: 'solver', solver: { event: 'warning', message: 'Courant number 3.2' } })).toMatchObject({ text: 'warning: Courant number 3.2', warning: true });
    expect(eventLine({ t: 'T', kind: 'stop', reason: 'cancel requested' }).text).toBe('stop: cancel requested');
    expect(eventLine({ t: 'T', kind: 'solver', solver: { event: 'checkpoint', iteration: 121, why: 'converged', path: 'x' } }).text).toBe('solver checkpoint at iteration 121: converged');
    expect(eventLine({ t: 'T', kind: 'solver', solver: { event: 'start', cells: 1024 } }).text).toBe('solver start');
    const long = eventLine({ t: 'T', kind: 'estimate', note: 'x'.repeat(400) });
    expect(long.warning).toBe(false);
    expect(long.text.startsWith('estimate {')).toBe(true);
    expect(long.text.length).toBeLessThan(200);
  });

  test('the end of log.txt in one page from its end, the first piece of a cut line dropped; the report found', async () => {
    const lines = Array.from({ length: 3000 }, (_, i) => `line ${i}`);
    const text = lines.join('\n') + '\n';
    const asked: unknown[] = [];
    const query: Query = async (tool, args) => {
      asked.push({ tool, args });
      if (tool === 'run_status') return { events: [{ kind: 'solver', t: 'T', solver: { event: 'warning', message: 'w1' } }, { kind: 'state', t: 'T', from: null, to: 'queued' }] };
      const p = String(args['path']);
      if (p.endsWith('report.html')) return { text: '<', size: 1200, hash: 'h2' };
      const offset = Number(args['offset'] ?? 0);
      const limit = Number(args['limit'] ?? 32000);
      const bytes = Buffer.from(text, 'utf8');
      const end = Math.min(bytes.length, offset + limit);
      return { text: bytes.subarray(offset, end).toString('utf8'), size: bytes.length, hash: 'h1', ...(end < bytes.length ? { next_offset: end } : {}) };
    };
    const log = await readRunLog(query, 'r-1', () => 'now');
    expect(log.log?.cut).toBe(true);
    expect(log.log?.lines.at(-1)).toBe('line 2999');
    expect(log.log?.lines.length).toBe(200);
    expect(log.events.map((e) => e.warning)).toEqual([true, false]);
    expect(log.report).toEqual({ path: 'runs/r-1/output/report.html', size: 1200 });
    expect(asked).toContainEqual({ tool: 'run_status', args: { run: 'r-1', events: 50 } });
  });

  test('no log and no report yet: nothing, and no error for a file that is not there', async () => {
    const query: Query = async (tool) => {
      if (tool === 'run_status') return { events: [] };
      throw Object.assign(new Error('not found'), { code: 1202 });
    };
    const log = await readRunLog(query, 'r-2', () => 'now');
    expect(log).toMatchObject({ log: null, logError: null, report: null, events: [], eventsError: null });
  });

  test('run_status’s last_residual is the run’s latest residuals', () => {
    const d = runDetail('r-1', { status: { state: 'running', step: 3 }, last_residual: { step: 3, residuals: { u: 1e-3, p: 2e-4, continuity: 'x' } }, events: [] }, 'x');
    expect(d.lastResidual).toEqual({ u: 1e-3, p: 2e-4 });
    expect(runDetail('r-1', { status: { state: 'queued', step: 0 }, last_residual: null, events: [] }, 'x').lastResidual).toBeUndefined();
  });
});

test.describe('the runner’s budgets (.rcfd/runner-config.json)', () => {
  test('the runner’s checks: known keys, whole numbers where it reads u64 or usize, no zero runs, threads or poll', () => {
    expect(BUDGET_KEYS).toEqual(['max_runs', 'cpu_threads', 'mem_budget_mb', 'mem_base_mb', 'mem_kb_per_cell', 'poll_ms', 'adopt_probe_ms', 'cancel_grace_ms']);
    expect(budgetProblem('max_runs', 2)).toBeNull();
    expect(budgetProblem('max_runs', 0)).toMatch(/positive/);
    expect(budgetProblem('cpu_threads', 1.5)).toMatch(/whole/);
    expect(budgetProblem('mem_budget_mb', 0)).toBeNull();
    expect(budgetProblem('mem_base_mb', 12.5)).toBeNull();
    expect(budgetProblem('mem_kb_per_cell', -1)).toMatch(/negative/);
    expect(budgetProblem('gpu', 1)).toMatch(/unknown key/);
    expect(budgetsOf('{"max_runs": 0}', null, false).problem).toMatch(/max_runs must be positive/);
    expect(budgetsOf('{', null, false).problem).toMatch(/not JSON/);
    const b = budgetsOf('{"max_runs":3}', JSON.stringify({ config: { max_runs: 2, cpu_threads: 8 }, config_source: 'defaults', started: 'S' }), false);
    expect(b.values).toEqual({ max_runs: 3 });
    expect(b.inUse).toEqual({ values: { max_runs: 2, cpu_threads: 8 }, source: 'defaults', started: 'S' });
  });

  test('written with only the keys set, never over another writer’s change; the last write undone', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f837-budgets-'));
    try {
      const f = new RunnerBudgetsFile();
      const file = path.join(dir, '.rcfd', 'runner-config.json');
      const none = await f.read(dir);
      expect(none).toMatchObject({ exists: false, hash: null, values: {}, undo: false });
      const w = await f.write(dir, { max_runs: 3, mem_budget_mb: 2048 }, null);
      expect(w.kind).toBe('written');
      expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({ max_runs: 3, mem_budget_mb: 2048 });
      const now = w.kind === 'written' ? w.budgets : none;
      expect(now.undo).toBe(true);
      // a key cleared goes back to the runner's default
      const w2 = await f.write(dir, { mem_budget_mb: null }, now.hash);
      expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({ max_runs: 3 });
      // another writer since the read: refused, nothing written
      await writeFile(file, '{"max_runs": 5}\n');
      const c = await f.write(dir, { max_runs: 4 }, w2.kind === 'written' ? w2.budgets.hash : null);
      expect(c.kind).toBe('conflict');
      expect(await readFile(file, 'utf8')).toBe('{"max_runs": 5}\n');
      // a value the runner refuses: refused
      expect(await f.write(dir, { cpu_threads: 0 }, c.kind === 'conflict' ? c.budgets.hash : null)).toMatchObject({ kind: 'refused' });
      // undo over the other writer's change is a conflict unless forced
      expect((await f.undo(dir, false)).kind).toBe('conflict');
      expect((await f.undo(dir, true)).kind).toBe('written');
      expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({ max_runs: 3, mem_budget_mb: 2048 });
      expect((await f.undo(dir, false)).kind).toBe('refused');
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

test.describe('the run port’s F-837 requests', () => {
  test('kept apart from F-810’s and F-815’s; each takes exactly its fields', () => {
    expect(RUN_OPS).toEqual(['series', 'at']);
    expect(RUN_COMPARE_OPS).toEqual(['compare']);
    expect(RUN_EXTRA_OPS).toEqual(['log', 'openReport', 'budgets', 'setBudgets', 'undoBudgets', 'savePng']);
    expect(parseRunRequest({ id: 1, op: 'log', run: 'r-1' })).toEqual({ ok: true, request: { id: 1, op: 'log', run: 'r-1' } });
    expect(parseRunRequest({ id: 1, op: 'openReport', run: '../x' }).ok).toBe(false);
    expect(parseRunRequest({ id: 1, op: 'budgets' }).ok).toBe(true);
    expect(parseRunRequest({ id: 1, op: 'setBudgets', values: { max_runs: 2, poll_ms: null }, base: null }).ok).toBe(true);
    expect(parseRunRequest({ id: 1, op: 'setBudgets', values: { gpu: 2 }, base: null }).ok).toBe(false);
    expect(parseRunRequest({ id: 1, op: 'setBudgets', values: {}, base: 'not a hash' }).ok).toBe(false);
    expect(parseRunRequest({ id: 1, op: 'undoBudgets', force: 'yes' }).ok).toBe(false);
    expect(parseRunRequest({ id: 1, op: 'savePng', rect: { x: 0, y: 0, width: 10, height: 10 }, name: 'a-b.png' }).ok).toBe(true);
    expect(parseRunRequest({ id: 1, op: 'savePng', rect: { x: 0, y: 0, width: 10, height: 10 }, name: '../a.png' }).ok).toBe(false);
    expect(parseRunRequest({ id: 1, op: 'savePng', rect: { x: 0, y: 0, width: 10 }, name: 'a.png' }).ok).toBe(false);
  });

  test('Save PNG: the picture first, then the dialog; a cancelled dialog writes nothing', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f837-png-'));
    try {
      const order: string[] = [];
      const deps = (answer: string | null) => ({
        capture: async () => (order.push('capture'), { png: new Uint8Array([137, 80, 78, 71]), width: 20, height: 10 }),
        savePath: async () => (order.push('dialog'), answer),
      });
      const to = path.join(dir, 'v.png');
      expect(await savePng(deps(to), { x: 0.5, y: 1, width: 19.2, height: 9 }, 'v.png')).toEqual({ path: to, width: 20, height: 10, bytes: 4 });
      expect(order).toEqual(['capture', 'dialog']);
      expect([...(await readFile(to))]).toEqual([137, 80, 78, 71]);
      expect((await savePng(deps(null), { x: 0, y: 0, width: 5, height: 5 }, 'v.png')).path).toBeNull();
      expect(wholeRect({ x: 0.5, y: 1.2, width: 10, height: 0 })).toBeNull();
      expect(wholeRect({ x: 0.5, y: 1.2, width: 10, height: 2 })).toEqual({ x: 0, y: 1, width: 11, height: 3 });
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

test.describe('the start with a project argument', () => {
  test('the first argument that is not a switch, packaged or not, resolved against the working folder', () => {
    expect(projectArgument(['rcfd-studio.exe', '--inspect=0', '--remote-debugging-port=0', 'C:\\p\\quick'], false, 'C:\\')).toBe(path.resolve('C:\\', 'C:\\p\\quick'));
    expect(projectArgument(['electron', '.', 'proj'], true, '/home/u')).toBe(path.resolve('/home/u', 'proj'));
    expect(projectArgument(['rcfd-studio.exe'], false, '/')).toBeNull();
    expect(projectArgument(['electron', '.'], true, '/')).toBeNull();
  });

  test('designs/ is listed as the engine counts it: top-level entries, files and folders, no temporary file', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f837-designs-'));
    try {
      expect(await entriesOf(dir, 'designs')).toEqual({ entries: [], total: 0 });
      await mkdir(path.join(dir, 'designs', 'fan-a'), { recursive: true });
      await writeFile(path.join(dir, 'designs', 'fan-a', 'blade.json'), '{}');
      await writeFile(path.join(dir, 'designs', 'variants.ledger.jsonl'), '');
      await writeFile(path.join(dir, 'designs', '.x.1.2.tmp'), '');
      expect(await entriesOf(dir, 'designs')).toEqual({
        entries: [
          { path: 'designs/fan-a', folder: true },
          { path: 'designs/variants.ledger.jsonl', folder: false },
        ],
        total: 2,
      });
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

test.describe('the field view’s range, domain and scale bar', () => {
  test('a scale bar is 1, 2 or 5 times a power of ten, at most a quarter of the width', () => {
    expect(scaleBar(0.001, 800)).toEqual({ length: 0.2, px: 200 });
    expect(scaleBar(0.01, 1000)).toEqual({ length: 2, px: 200 });
    expect(scaleBar(1 / 700, 1000)!.length).toBe(0.2);
    expect(scaleBar(3e-5, 640)).toEqual({ length: 0.002, px: 0.002 / 3e-5 });
    expect(scaleBar(0, 100)).toBeNull();
    for (const [pp, w] of [[0.0137, 911], [2.5, 333], [7e-7, 1440]] as const) {
      const b = scaleBar(pp, w)!;
      expect(b.px).toBeLessThanOrEqual(w / 4 + 1e-9);
      expect(b.px).toBeGreaterThan(w / 4 / 2.5 - 1e-9);
      const m = b.length / 10 ** Math.floor(Math.log10(b.length) + 1e-12);
      expect([1, 2, 5].some((x) => Math.abs(m - x) < 1e-9)).toBe(true);
    }
  });

  test('the domain of a 2D case is its two in-plane extents; of a 3D case all three', () => {
    expect(domainText({ twoD: true, thin: 2, min: [0, 0, 0], max: [2, 1, 0.05] })).toBe('2 × 1');
    expect(domainText({ twoD: false, thin: 2, min: [0, 0, 0], max: [1, 0.5, 0.25] })).toBe('1 × 0.5 × 0.25');
    expect(lengthText(0.1 + 0.2)).toBe('0.3');
  });

  test('a fixed range needs two ends, the top above the bottom; the cells outside it are counted once each', () => {
    expect(fixedRange(Number.NaN, 1)).toEqual({ problem: 'Type both ends of the range.' });
    expect(fixedRange(1, 1)).toEqual({ problem: 'The top of the range must be above its bottom.' });
    expect(fixedRange(-1, 1)).toEqual({ lo: -1, hi: 1 });
    expect(cellsOutside(new Uint32Array([0, 0, 1, 2, 2]), new Float32Array([-2, 0.5, 3]), -1, 1)).toEqual({ outside: 2, of: 3 });
  });

  test('Save PNG proposes run, field and step, in letters, digits, dot, underscore and dash', () => {
    expect(pngName('r-20261028T152641-f8c2', 'U', 278)).toBe('r-20261028T152641-f8c2-U-step278.png');
    expect(pngName('a b/c', 'p:x', undefined)).toBe('a_b_c-p_x.png');
  });
});

test.describe('the palette and the run action', () => {
  test('each new command is in a known group, in GROUP_ORDER; the new icons are named', () => {
    const ids = ['runs.log', 'runs.report', 'runs.budgets', 'field.save_png', 'field.range', 'field.face_on', 'plots.monitor_log'];
    for (const id of ids) {
      const c = STUDIO_COMMANDS.find((x) => x.id === id);
      expect(c, id).toBeTruthy();
      expect(GROUP_ORDER).toContain(c!.group);
    }
    for (const n of ['log', 'report', 'image', 'budgets', 'range-snapshot', 'range-run', 'range-fixed', 'face-on', 'log-scale', 'stored']) expect(ICON_NAMES).toContain(n);
  });

  test('Run again with fields stored is start_run’s store plan; the default stays store none', () => {
    const q = getRunAction('project.queue_case')!;
    expect(runEquivalents(q, { case: 'cases/a.toml', store: 'plan' } as never, '/p').mcp).toEqual({ tool: 'start_run', arguments: { case: 'cases/a.toml', store: 'plan' } });
    expect(runEquivalents(q, { case: 'cases/a.toml' } as never, '/p').mcp).toEqual({ tool: 'start_run', arguments: { case: 'cases/a.toml', store: 'none' } });
  });
});
