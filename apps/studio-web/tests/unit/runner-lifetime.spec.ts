// G7 as amended 2026-11-17 (the maintainer: "the runner should close with studio by default if no external (existing)
// runner env var is specified"): the ownership and close rules of src/shared/runner-lifetime.ts as pure functions, the
// main process's watch of the project's runner (src/main/project/runner-lifetime.ts) with a fake engine, the runner
// setting kept in studio-preferences.json, and the project port's close with the runs' answer.
import { expect, test } from '@playwright/test';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import {
  closeTitle,
  NOT_LOOKED,
  observeRunner,
  runLine,
  runnerCloseStep,
  runnerNeedsAsk,
  type ActiveRun,
  type Ownership,
  type RunnerPlan,
  type RunnerSeen,
} from '../../src/shared/runner-lifetime.js';
import { readRunnerSeen, RunnerWatch, type RunnerEngine } from '../../src/main/project/runner-lifetime.js';
import { formatPreferences, parseKeepRunner, parsePreferences, ThemePreference } from '../../src/main/theme.js';
import { parseProjectRequest } from '../../src/main/project/bridge.js';
import { keptForSolves } from '../../src/main/service/client.js';

const A: RunnerSeen = { pid: 101, started: '2026-11-17T10:00:00Z' };
const B: RunnerSeen = { pid: 202, started: '2026-11-17T11:00:00Z' };
const run = (id: string, state: string, step = 0, time: number | null = 0): ActiveRun => ({ id, case: 'cases/cavity-re100.toml', state, step, time });

const looks = (...seen: (RunnerSeen | null)[]): Ownership => seen.reduce<Ownership>((o, s) => observeRunner(o, s), NOT_LOOKED);

test.describe('ownership: the Studio owns only the runner that came up while it had the project open', () => {
  test('a runner alive when the project opened is adopted, never owned, however long it lives', () => {
    expect(looks(A).owned).toBeNull();
    expect(looks(A, A, A).owned).toBeNull();
  });
  test('no runner at the open, then one: owned (the Studio’s Run, or Claude Code’s start_run while it is open)', () => {
    expect(looks(null, A).owned).toEqual(A);
    expect(looks(null, null, A, A).owned).toEqual(A);
  });
  test('an owned runner that is gone is not owned; the next one, after a look without one, is', () => {
    expect(looks(null, A, null).owned).toBeNull();
    expect(looks(null, A, null, B).owned).toEqual(B);
  });
  test('a runner that replaced another between two looks is not owned (the Studio cannot tell it started it)', () => {
    expect(looks(null, A, B).owned).toBeNull();
    expect(looks(A, B).owned).toBeNull();
  });
  test('an adopted runner that ends, then a new one: owned', () => {
    expect(looks(A, null, B).owned).toEqual(B);
  });
  test('the same pid with another start time is another runner', () => {
    expect(looks(null, A, { pid: A.pid, started: 'later' }).owned).toBeNull();
  });
});

test.describe('the close decision', () => {
  const owned = (runs: ActiveRun[], kept: RunnerPlan['kept'] = null): RunnerPlan => ({ owned: true, kept, runs });
  const adopted: RunnerPlan = { owned: false, kept: null, runs: [] };
  test('not owned: left alone, nothing asked, whatever the answer', () => {
    for (const c of ['stop', 'keep', 'unasked'] as const) expect(runnerCloseStep(adopted, c)).toBe('leave');
    expect(runnerNeedsAsk(adopted)).toBe(false);
  });
  test('owned and idle: shut down quietly, with no question', () => {
    expect(runnerNeedsAsk(owned([]))).toBe(false);
    for (const c of ['stop', 'keep', 'unasked'] as const) expect(runnerCloseStep(owned([]), c)).toBe('shutdown');
  });
  test('owned with runs queued, running or paused: asked; stop cancels and shuts down, keep and unasked leave it', () => {
    const p = owned([run('r-1', 'running', 40), run('r-2', 'paused', 3), run('r-3', 'queued')]);
    expect(runnerNeedsAsk(p)).toBe(true);
    expect(runnerCloseStep(p, 'stop')).toBe('stop-then-shutdown');
    expect(runnerCloseStep(p, 'keep')).toBe('leave');
    expect(runnerCloseStep(p, 'unasked')).toBe('leave');
  });
  test('the setting or --keep-runner: left alone and nothing asked, idle or not', () => {
    for (const kept of ['setting', 'switch'] as const) {
      expect(runnerNeedsAsk(owned([run('r-1', 'running')], kept))).toBe(false);
      expect(runnerCloseStep(owned([], kept), 'unasked')).toBe('leave');
      expect(runnerCloseStep(owned([run('r-1', 'running')], kept), 'stop')).toBe('leave');
    }
  });
});

test.describe('the prompt’s words', () => {
  test('each run: id, case, state, and its step or its simulated time', () => {
    expect(runLine(run('r-1', 'running', 120, 0))).toBe('r-1: cases/cavity-re100.toml, running, step 120');
    expect(runLine(run('r-2', 'paused', 40, 0.25))).toBe('r-2: cases/cavity-re100.toml, paused, step 40, t = 0.25 s');
    expect(runLine(run('r-3', 'queued'))).toBe('r-3: cases/cavity-re100.toml, queued, not started');
    expect(runLine({ ...run('r-4', 'running', 1, null), case: null })).toBe('r-4: case not known, running, step 1');
  });
  test('the title names the changes and the runs, one prompt for both', () => {
    expect(closeTitle('rcfd Studio', 0, 1)).toBe('Close rcfd Studio with 1 run going?');
    expect(closeTitle('Quick start', 3, 2)).toBe('Close Quick start with 3 changes not saved and 2 runs going?');
    expect(closeTitle('rcfd Studio', 1, 0)).toBe('Close rcfd Studio with 1 change not saved?');
  });
});

/** A fake engine: the runner seen at each look, the runs, and the calls the close made. */
function fakeEngine(seen: (RunnerSeen | null | undefined)[], runs: ActiveRun[]) {
  const calls: string[] = [];
  let i = 0;
  const engine: RunnerEngine = {
    look: async () => seen[Math.min(i++, seen.length - 1)],
    activeRuns: async () => runs,
    cancel: async (_f, r) => void calls.push(`cancel ${r}`),
    wait: async (_f, r) => void calls.push(`wait ${r}`),
    shutdown: async () => void calls.push('shutdown'),
  };
  return { engine, calls };
}

test.describe('RunnerWatch (the main process, a fake engine)', () => {
  test('started by the Studio and idle at the close: shut down, nothing else', async () => {
    const { engine, calls } = fakeEngine([null, A], []);
    const w = new RunnerWatch('/p', engine, () => null, 0);
    await w.look();
    expect(w.owned).toEqual(A);
    expect(await w.plan()).toEqual({ owned: true, kept: null, runs: [] });
    expect(await w.close('unasked')).toBe('shutdown');
    expect(calls).toEqual(['shutdown']);
  });
  test('Stop runs and close: each active run cancelled, waited for, then the runner shut down', async () => {
    const runs = [run('r-1', 'running', 10), run('r-2', 'paused', 2)];
    const { engine, calls } = fakeEngine([null, A], runs);
    const w = new RunnerWatch('/p', engine, () => null, 0);
    await w.look();
    expect((await w.plan()).runs).toEqual(runs);
    expect(await w.close('stop')).toBe('stop-then-shutdown');
    expect(calls).toEqual(['cancel r-1', 'cancel r-2', 'wait r-1', 'wait r-2', 'shutdown']);
  });
  test('Keep them running, and a close that did not ask: nothing is stopped', async () => {
    for (const choice of ['keep', 'unasked'] as const) {
      const { engine, calls } = fakeEngine([null, A], [run('r-1', 'running')]);
      const w = new RunnerWatch('/p', engine, () => null, 0);
      await w.look();
      expect(await w.close(choice)).toBe('leave');
      expect(calls).toEqual([]);
    }
  });
  test('a runner alive at the open (the CLI’s, an earlier Studio’s): never asked about, never stopped', async () => {
    const { engine, calls } = fakeEngine([A], [run('r-1', 'running')]);
    const w = new RunnerWatch('/p', engine, () => null, 0);
    await w.look();
    expect(await w.plan()).toEqual({ owned: false, kept: null, runs: [] });
    expect(await w.close('stop')).toBe('leave');
    expect(calls).toEqual([]);
  });
  test('a look that cannot tell changes nothing: an adopted runner is not owned after it', async () => {
    const { engine, calls } = fakeEngine([A, undefined, A], [run('r-1', 'running')]);
    const w = new RunnerWatch('/p', engine, () => null, 0);
    await w.look();
    await w.look();
    expect(await w.plan()).toEqual({ owned: false, kept: null, runs: [] });
    expect(await w.close('stop')).toBe('leave');
    expect(calls).toEqual([]);
  });
  test('the setting keeps an owned runner: no runs read, nothing stopped', async () => {
    const { engine, calls } = fakeEngine([null, A], [run('r-1', 'running')]);
    const w = new RunnerWatch('/p', engine, () => 'setting', 0);
    await w.look();
    expect(await w.plan()).toEqual({ owned: true, kept: 'setting', runs: [] });
    expect(await w.close('stop')).toBe('leave');
    expect(calls).toEqual([]);
  });
});

test.describe('the runner’s record and the setting’s file', () => {
  let dir = '';
  test.beforeAll(async () => {
    dir = await mkdtemp(path.join(tmpdir(), 'rcfd-runner-lifetime-'));
  });
  test.afterAll(async () => {
    await rm(dir, { recursive: true, force: true });
  });
  test('.rcfd/runner.json: the pid and start time, when the process is alive; nothing otherwise', async () => {
    const p = path.join(dir, 'p');
    await mkdir(path.join(p, '.rcfd'), { recursive: true });
    expect(await readRunnerSeen(p)).toBeNull();
    await writeFile(path.join(p, '.rcfd', 'runner.json'), JSON.stringify({ pid: process.pid, started: '2026-11-17T10:00:00Z' }));
    expect(await readRunnerSeen(p)).toEqual({ pid: process.pid, started: '2026-11-17T10:00:00Z' });
    expect(await readRunnerSeen(p, () => false)).toBeNull();
    // there but not readable as a record (a write half done): cannot tell, not “none”
    await writeFile(path.join(p, '.rcfd', 'runner.json'), '{"pid": "x"}');
    expect(await readRunnerSeen(p)).toBeUndefined();
    await writeFile(path.join(p, '.rcfd', 'runner.json'), '{"pid": 12');
    expect(await readRunnerSeen(p)).toBeUndefined();
  });
  test('the setting is off by default, kept only when on, and a theme change keeps it', async () => {
    expect(formatPreferences('light')).toBe(JSON.stringify({ format: 'rcfd-studio-preferences', version: 1, theme: 'light' }, null, 2) + '\n');
    expect(parseKeepRunner(formatPreferences('dark'))).toBe(false);
    expect(parseKeepRunner(formatPreferences('dark', true))).toBe(true);
    expect(parsePreferences(formatPreferences('dark', true))).toBe('dark');
    expect(parseKeepRunner('{"keepRunner": true}')).toBe(false);
    const file = path.join(dir, 'prefs.json');
    const p = new ThemePreference(file);
    expect(p.keepRunner).toBe(false);
    expect(await p.setKeepRunner(true)).toBe(true);
    await p.set('dark', { setSource: () => undefined, systemDark: () => false, setBackground: () => undefined });
    expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({ format: 'rcfd-studio-preferences', version: 1, theme: 'dark', keepRunner: true });
    expect(new ThemePreference(file).keepRunner).toBe(true);
    await expect(p.setKeepRunner('yes')).rejects.toThrow('takes true or false');
  });
});

test.describe('the project port', () => {
  test('close takes nothing, or the runs’ answer stop or keep; runnerPlan takes nothing', () => {
    expect(parseProjectRequest({ op: 'close', id: 1 })).toEqual({ ok: true, request: { op: 'close', id: 1 } });
    for (const runs of ['stop', 'keep'] as const) expect(parseProjectRequest({ op: 'close', id: 2, runs })).toEqual({ ok: true, request: { op: 'close', id: 2, runs } });
    for (const raw of [{ op: 'close', id: 3, runs: 'kill' }, { op: 'close', id: 3, runs: 'stop', path: 'x' }, { op: 'runnerPlan', id: 4, runs: 'stop' }]) {
      expect(parseProjectRequest(raw).ok, JSON.stringify(raw)).toBe(false);
    }
    expect(parseProjectRequest({ op: 'runnerPlan', id: 5 })).toEqual({ ok: true, request: { op: 'runnerPlan', id: 5 } });
  });
});

test.describe('the Studio’s own rcfd serve at its quit', () => {
  test('kept only for a solve going in it (a session running or paused), or a list that cannot be read', () => {
    expect(keptForSolves([])).toBe(false);
    expect(keptForSolves([{ session: 's1', state: 'ready' }, { session: 's2', state: 'defined' }, { session: 's3', state: 'stopped' }])).toBe(false);
    expect(keptForSolves([{ session: 's1', state: 'ready' }, { session: 's2', state: 'running' }])).toBe(true);
    expect(keptForSolves([{ session: 's1', state: 'paused' }])).toBe(true);
    expect(keptForSolves(undefined)).toBe(true);
  });
});
