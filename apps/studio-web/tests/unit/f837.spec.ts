// F-837 without a window: the run's log and events (warnings named), the runner's budgets through the engine (ADR 0733:
// the engine's answers as the panel shows them, a conflict, a refusal, undo by the values before, and nothing under
// src/main writing the file), the run port's new requests, the project argument, Save PNG's file, the
// field view's range, domain and scale bar, the latest residuals of run_status, and the palette's new commands.
import { expect, test } from '@playwright/test';
import { mkdtemp, readFile, readdir, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { BUDGET_KEYS, RUN_COMPARE_OPS, RUN_EXTRA_OPS, RUN_OPS } from '../../src/shared/run.js';
import { parseRunRequest } from '../../src/main/run/bridge.js';
import { budgetsOf, eventLine, readRunLog, RunnerBudgetsEngine, savePng, wholeRect, type BudgetsEngine } from '../../src/main/run/extras.js';
import type { Query } from '../../src/main/run/series.js';
import { projectArgument } from '../../src/main/project/argv.js';
import { appRoot } from '../support/rcfd.js';
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
  /** An engine answer of runner_config / set_runner_config (crates/rcfd-runner/src/config.rs). */
  const state = (values: Record<string, unknown> | null, hash: string | null, more: Record<string, unknown> = {}) => ({
    path: '.rcfd/runner-config.json',
    exists: values !== null,
    hash,
    values: values ?? {},
    problem: null,
    needs_replace: false,
    defaults: { max_runs: 2 },
    in_use: null,
    applies: 'next_start',
    ...more,
  });
  const engineError = (code: number, message: string) => Object.assign(new Error(message), { code });

  test('the engine’s answer as the panel shows it', () => {
    expect(BUDGET_KEYS).toEqual(['max_runs', 'cpu_threads', 'mem_budget_mb', 'mem_base_mb', 'mem_kb_per_cell', 'poll_ms', 'adopt_probe_ms', 'cancel_grace_ms']);
    const b = budgetsOf(
      state({ max_runs: 3 }, 'fnv64:0000000000000001', { in_use: { values: { max_runs: 2, cpu_threads: 8 }, source: 'the default (no runner-config.json)', started: 'S' } }),
      false,
    );
    expect(b).toEqual({
      path: '.rcfd/runner-config.json',
      exists: true,
      values: { max_runs: 3 },
      hash: 'fnv64:0000000000000001',
      problem: null,
      needsReplace: false,
      inUse: { values: { max_runs: 2, cpu_threads: 8 }, source: 'the default (no runner-config.json)', started: 'S' },
      undo: false,
    });
    const bad = budgetsOf(state({}, 'fnv64:0000000000000002', { problem: 'unknown field `gpu`', needs_replace: true }), false);
    expect(bad).toMatchObject({ problem: 'unknown field `gpu`', needsReplace: true, values: {} });
  });

  test('written by the engine, never over another writer’s change; undo sends the values before back', async () => {
    const calls: Record<string, unknown>[] = [];
    let now: ReturnType<typeof state> = state(null, null);
    const answers: (() => unknown)[] = [];
    const engine: BudgetsEngine = {
      read: async () => now,
      write: async (args) => {
        calls.push(args);
        const a = answers.shift();
        if (!a) throw new Error('no answer');
        return a();
      },
    };
    const f = new RunnerBudgetsEngine(engine);
    expect(await f.read('/p')).toMatchObject({ exists: false, hash: null, values: {}, undo: false });
    answers.push(() => (now = state({ max_runs: 3, mem_budget_mb: 2048 }, 'fnv64:00000000000000a1')));
    const w = await f.write('/p', { max_runs: 3, mem_budget_mb: 2048 }, null, false);
    expect(calls.at(-1)).toEqual({ set: { max_runs: 3, mem_budget_mb: 2048 }, base_hash: null });
    expect(w).toMatchObject({ kind: 'written', budgets: { hash: 'fnv64:00000000000000a1', undo: true } });
    expect(await f.read('/p')).toMatchObject({ undo: true });
    expect(await f.read('/other')).toMatchObject({ undo: false });
    // a base that is not the file's now: a conflict, nothing sent
    const n = calls.length;
    expect(await f.write('/p', { max_runs: 4 }, 'fnv64:0000000000000bad', false)).toMatchObject({ kind: 'conflict', budgets: { hash: 'fnv64:00000000000000a1' } });
    expect(calls.length).toBe(n);
    // the engine's conflict (another writer between the read and the write): the file now
    answers.push(() => {
      now = state({ max_runs: 5 }, 'fnv64:00000000000000b2');
      throw engineError(1200, '.rcfd/runner-config.json: changed since it was read');
    });
    expect(await f.write('/p', { max_runs: 4 }, 'fnv64:00000000000000a1', false)).toMatchObject({ kind: 'conflict', budgets: { hash: 'fnv64:00000000000000b2' } });
    // the runner's refusal, in its words
    answers.push(() => {
      throw engineError(1003, '.rcfd/runner-config.json: max_runs, cpu_threads and poll_ms must be positive; nothing was written');
    });
    expect(await f.write('/p', { cpu_threads: 0 }, 'fnv64:00000000000000b2', false)).toEqual({
      kind: 'refused',
      message: '.rcfd/runner-config.json: max_runs, cpu_threads and poll_ms must be positive; nothing was written',
    });
    // undo with the hash of this window's write as base: the engine's conflict, then Undo anyway on the file as it is
    answers.push(() => {
      throw engineError(1200, 'changed');
    });
    expect((await f.undo('/p', false)).kind).toBe('conflict');
    expect(calls.at(-1)).toEqual({ set: { max_runs: null, mem_budget_mb: null }, base_hash: 'fnv64:00000000000000a1' });
    answers.push(() => (now = state({}, 'fnv64:00000000000000c3')));
    expect(await f.undo('/p', true)).toMatchObject({ kind: 'written', budgets: { values: {}, undo: false } });
    expect(calls.at(-1)).toEqual({ set: { max_runs: null, mem_budget_mb: null }, base_hash: 'fnv64:00000000000000b2' });
    expect((await f.undo('/p', false)).kind).toBe('refused');
    // a key that had a value goes back to it
    answers.push(() => (now = state({ max_runs: 7 }, 'fnv64:00000000000000d4')));
    await f.write('/p', { max_runs: 7 }, 'fnv64:00000000000000c3', false);
    now = state({ max_runs: 7 }, 'fnv64:00000000000000d4');
    answers.push(() => (now = state({ max_runs: 9 }, 'fnv64:00000000000000e5')));
    await f.write('/p', { max_runs: 9 }, 'fnv64:00000000000000d4', false);
    answers.push(() => (now = state({ max_runs: 7 }, 'fnv64:00000000000000f6')));
    await f.undo('/p', false);
    expect(calls.at(-1)).toEqual({ set: { max_runs: 7 }, base_hash: 'fnv64:00000000000000e5' });
    // a file the runner cannot read: replaced whole, and that replacement is not undone by values
    now = state({}, 'fnv64:0000000000000107', { problem: 'not JSON', needs_replace: true });
    answers.push(() => (now = state({ max_runs: 2 }, 'fnv64:0000000000000108')));
    const all = Object.fromEntries(BUDGET_KEYS.map((k) => [k, k === 'max_runs' ? 2 : null]));
    expect(await f.write('/p', all, 'fnv64:0000000000000107', true)).toMatchObject({ kind: 'written', budgets: { undo: false } });
    expect(calls.at(-1)).toEqual({ set: all, base_hash: 'fnv64:0000000000000107', replace: true });
  });

  test('nothing under src/main writes the runner’s budgets file: the runner does (ADR 0733)', async () => {
    const root = path.join(appRoot, 'src', 'main');
    const files: string[] = [];
    const walk = async (d: string): Promise<void> => {
      for (const e of await readdir(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory()) await walk(p);
        else if (/\.(ts|cts|mts)$/.test(e.name)) files.push(p);
      }
    };
    await walk(root);
    expect(files.length).toBeGreaterThan(10);
    const code = (t: string) => t.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');
    const offenders: string[] = [];
    for (const f of files) {
      const c = code(await readFile(f, 'utf8'));
      if (/runner-config\.json|runner-config/.test(c)) offenders.push(path.relative(root, f));
    }
    expect(offenders, 'src/main names the runner’s budgets file in code').toEqual([]);
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
    expect(parseRunRequest({ id: 1, op: 'setBudgets', values: { max_runs: 2, poll_ms: null }, base: null, replace: false }).ok).toBe(true);
    expect(parseRunRequest({ id: 1, op: 'setBudgets', values: { max_runs: 2 }, base: 'fnv64:00000000000000a1', replace: false }).ok).toBe(true);
    expect(parseRunRequest({ id: 1, op: 'setBudgets', values: { max_runs: 2 }, base: null }).ok).toBe(false);
    expect(parseRunRequest({ id: 1, op: 'setBudgets', values: { gpu: 2 }, base: null, replace: false }).ok).toBe(false);
    expect(parseRunRequest({ id: 1, op: 'setBudgets', values: {}, base: 'not a hash', replace: false }).ok).toBe(false);
    expect(parseRunRequest({ id: 1, op: 'setBudgets', values: {}, base: 'a'.repeat(64), replace: false }).ok).toBe(false);
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
