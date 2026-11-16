// Unit tests of F-832 (G21): the work tracker every long operation goes through (shown only after PROGRESS_AFTER_MS,
// Cancel aborts and runs the work's own undo), the helpers that drop a result or hand a cancel through a door, the
// tests' delay hook, the cancel requests of the project and case ports (what a cancel can still stop, and that a save
// cancelled before the engine was asked writes nothing), and the field worker's arrays
// (the same as ./surface.ts computes on the main thread). Plus the stylesheet of the progress row: tokens only.
import { expect, test } from '@playwright/test';
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { LONG_FRAME_LIMIT_MS, LONG_TASK_LIMIT_MS, PROGRESS_AFTER_MS, WORK_KINDS } from '../../src/shared/work.js';
import { abortable, cancelHook, isAbort, WorkTracker } from '../../src/renderer/work/work.js';
import { parseDelays, testDelay } from '../../src/main/work/delay.js';
import { parseCaseRequest } from '../../src/main/case/bridge.js';
import { CaseEditor } from '../../src/main/case/editor.js';
import type { ToolServerLike } from '../../src/main/case/server.js';
import { parseProjectRequest } from '../../src/main/project/bridge.js';
import { ProjectHome, type RecentLike } from '../../src/main/project/home.js';
import type { ProjectServerLike } from '../../src/main/project/server.js';
import type { StoredRecent } from '../../src/main/project/recent.js';
import { benchSoup, cornerPositions, cornerScalars, shownRange } from '../../src/renderer/field/surface.js';
import { meshLayers, vectorGlyphs } from '../../src/renderer/field/derived.js';
import { FieldPrep } from '../../src/renderer/field/prepare.js';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { appRoot } from '../support/rcfd.js';

/** Timers driven by hand. */
function clock() {
  let now = 0;
  const timers = new Map<number, { at: number; fn: () => void }>();
  let n = 0;
  return {
    now: () => now,
    setTimer: (fn: () => void, ms: number) => {
      timers.set(++n, { at: now + ms, fn });
      return n;
    },
    clearTimer: (t: unknown) => void timers.delete(t as number),
    advance(ms: number) {
      now += ms;
      for (const [id, t] of [...timers]) {
        if (t.at <= now) {
          timers.delete(id);
          t.fn();
        }
      }
    },
  };
}

test.describe('the budget', () => {
  test('the registered limits and the kinds of work', () => {
    expect([LONG_TASK_LIMIT_MS, LONG_FRAME_LIMIT_MS, PROGRESS_AFTER_MS]).toEqual([100, 150, 300]);
    expect([...WORK_KINDS]).toEqual(['project.open', 'run.open', 'field.load', 'case.check', 'case.save', 'mesh.check']);
  });
});

test.describe('the work tracker', () => {
  const spec = { kind: 'run.open' as const, scope: 'ws', label: 'Opening run r1', word: 'Opening run' };

  test('a work is shown only once it has lasted PROGRESS_AFTER_MS; one ended before is never shown', () => {
    const c = clock();
    const t = new WorkTracker(c);
    let changes = 0;
    t.subscribe(() => changes++);
    const quick = t.begin(spec);
    const slow = t.begin({ ...spec, label: 'Opening run r2' });
    c.advance(PROGRESS_AFTER_MS - 1);
    quick.end();
    expect(t.shown()).toEqual([]);
    expect(changes).toBe(0);
    c.advance(1);
    expect(t.shown().map((w) => w.label)).toEqual(['Opening run r2']);
    expect(t.shown()).toBe(t.shown());
    expect(changes).toBe(1);
    slow.end();
    slow.end();
    expect(t.shown()).toEqual([]);
    expect(t.going()).toEqual([]);
    expect(changes).toBe(2);
  });

  test('Cancel aborts the signal, runs the undo once, and ends the work; end after it does nothing more', () => {
    const c = clock();
    const t = new WorkTracker(c);
    const undone: string[] = [];
    const h = t.begin({ ...spec, onCancel: () => undone.push('back to r0') });
    c.advance(PROGRESS_AFTER_MS);
    expect(t.shown()).toHaveLength(1);
    t.shown()[0]!.cancel();
    expect(h.signal.aborted).toBe(true);
    expect(h.cancelled).toBe(true);
    expect(undone).toEqual(['back to r0']);
    expect(t.shown()).toEqual([]);
    h.cancel();
    h.end();
    expect(undone).toEqual(['back to r0']);
    // an ended work cannot be cancelled
    const done = t.begin({ ...spec, onCancel: () => undone.push('never') });
    done.end();
    done.cancel();
    expect(done.signal.aborted).toBe(false);
    expect(undone).toEqual(['back to r0']);
  });

  test('abortable drops a result on cancel; cancelHook calls a door’s cancel when the signal aborts, or at once', async () => {
    const a = new AbortController();
    let resolve!: (v: number) => void;
    const p = abortable(new Promise<number>((r) => (resolve = r)), a.signal);
    a.abort();
    resolve(7);
    const e = await p.catch((x: unknown) => x);
    expect(isAbort(e)).toBe(true);
    expect(await abortable(Promise.resolve(3), new AbortController().signal)).toBe(3);
    expect(isAbort(await abortable(Promise.resolve(3), a.signal).catch((x: unknown) => x))).toBe(true);

    const calls: string[] = [];
    const b = new AbortController();
    cancelHook(b.signal).onCancel!(() => calls.push('later'));
    expect(calls).toEqual([]);
    b.abort();
    expect(calls).toEqual(['later']);
    cancelHook(b.signal).onCancel!(() => calls.push('at once'));
    expect(calls).toEqual(['later', 'at once']);
    expect(cancelHook(undefined)).toEqual({});
  });
});

test.describe('the tests’ delay', () => {
  test('parsed from RCFD_STUDIO_TEST_DELAY, bounded; bad values are no delay; unset, nothing waits', async () => {
    expect(parseDelays(undefined)).toEqual({ data: 0, projectOpen: 0, caseCheck: 0, caseSave: 0 });
    expect(parseDelays('not json')).toEqual({ data: 0, projectOpen: 0, caseCheck: 0, caseSave: 0 });
    expect(parseDelays('{"data": 1500.4, "projectOpen": -1, "caseCheck": "9", "caseSave": 1e9, "other": 5}')).toEqual({ data: 1500, projectOpen: 0, caseCheck: 0, caseSave: 60_000 });
    // this process has no RCFD_STUDIO_TEST_DELAY: the delay resolves at once
    expect(process.env['RCFD_STUDIO_TEST_DELAY']).toBeUndefined();
    expect(await testDelay('caseSave', AbortSignal.abort())).toBe(true);
  });
});

test.describe('the ports’ cancel requests', () => {
  test('the case port takes cancel {target}; the project port takes cancel with no arguments', () => {
    expect(parseCaseRequest({ op: 'cancel', id: 9, target: 4 })).toEqual({ ok: true, request: { op: 'cancel', id: 9, target: 4 } });
    for (const bad of [{ op: 'cancel', id: 9 }, { op: 'cancel', id: 9, target: -1 }, { op: 'cancel', id: 9, target: '4' }, { op: 'cancel', id: 9, target: 4, session: 's' }]) {
      expect(parseCaseRequest(bad).ok, JSON.stringify(bad)).toBe(false);
    }
    expect(parseProjectRequest({ op: 'cancel', id: 3 })).toMatchObject({ ok: true, request: { op: 'cancel', id: 3 } });
    expect(parseProjectRequest({ op: 'cancel', id: 3, path: '/x' }).ok).toBe(false);
  });
});

test.describe('cancelling a case save or check', () => {
  const FOLDER = path.resolve('/projects/demo');
  const REL = 'cases/a.toml';

  function setup(check: () => Promise<unknown> = async () => ({ ok: true, exit_code: 0, event: { diagnostics: [], cells: 8, mesh: { errors: [], warnings: [] } } })) {
    const calls: string[] = [];
    const server: ToolServerLike = {
      exited: false,
      callTool: async (tool) => {
        calls.push(tool);
        if (tool === 'read_project_file') return { path: REL, hash: 'h1', size: 10, text: 'x', offset: 0, next_offset: 1 };
        if (tool === 'edit_case') return { path: REL, written: true, valid: true, form: 'minimal', old_hash: 'h1', new_hash: 'h2', values: {} };
        if (tool === 'check_case_headless') return check();
        throw new Error(`no ${tool}`);
      },
      listTools: async () => [],
      close: async () => undefined,
    };
    const journal: Record<string, unknown>[] = [{ op: 'create', source: { path: path.join(FOLDER, 'cases', 'a.toml') } }];
    const editor = new CaseEditor({
      project: () => ({ folder: FOLDER }),
      engine: { configured: true, path: '/bin/rcfd-mcp', from: 'RCFD_STUDIO_MCP' },
      service: {
        call: async (method: string, params: Record<string, unknown> = {}) => {
          if (method === 'journal.get') return { format: 'rcfd-journal', version: 1, entries: journal, case_hash: 'x' };
          if (method === 'settings.get') return { value: params['path'] === 'physics.nu' ? 0.02 : {} };
          throw new Error(`no ${method}`);
        },
      },
      startServer: async () => server,
      now: () => '2026-11-16T12:00:00.000Z',
    });
    return { editor, calls, set: () => void journal.push({ op: 'set', path: 'physics.nu', value: 0.02 }) };
  }

  test('a save cancelled before the engine was asked is refused as cancelled and sends no edit_case', async () => {
    const t = setup();
    const p = await t.editor.prepare(REL);
    await t.editor.attach('s1', p.path, p.hash);
    t.set();
    await expect(t.editor.save('s1', 'opened', AbortSignal.abort())).rejects.toMatchObject({ code: 'cancelled' });
    expect(t.calls).not.toContain('edit_case');
    // not cancelled, the same save writes
    expect(await t.editor.save('s1', 'opened', new AbortController().signal)).toMatchObject({ kind: 'saved' });
    expect(t.calls.filter((c) => c === 'edit_case')).toHaveLength(1);
  });

  test('a check cancelled while the engine runs it rejects as cancelled at once; its result is dropped', async () => {
    let finish!: (v: unknown) => void;
    const t = setup(() => new Promise((r) => (finish = r)));
    const a = new AbortController();
    const going = t.editor.check(REL, a.signal);
    await expect.poll(() => t.calls.includes('check_case_headless')).toBe(true);
    a.abort();
    await expect(going).rejects.toMatchObject({ code: 'cancelled' });
    finish({ ok: true, exit_code: 0, event: { diagnostics: [], cells: 8, mesh: { errors: [], warnings: [] } } });
    // cancelled before it starts, the engine is not asked
    const before = t.calls.length;
    await expect(t.editor.check(REL, AbortSignal.abort())).rejects.toMatchObject({ code: 'cancelled' });
    expect(t.calls.length).toBe(before);
  });
});

test.describe('cancelling a project opening', () => {
  test('the project open before stays open, the recent list is not touched, and the late server is closed', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f832-home-'));
    try {
      const a = path.join(dir, 'a');
      const b = path.join(dir, 'b');
      await mkdir(a);
      await mkdir(b);
      const list: StoredRecent[] = [];
      const recent: RecentLike = { load: async () => list, save: async (l) => void list.splice(0, list.length, ...l) };
      let release!: () => void;
      const closed: string[] = [];
      const server = (folder: string): ProjectServerLike => ({
        info: { name: 'rcfd', version: '0' },
        exited: false,
        call: async (tool) => {
          if (tool === 'project_info') return { name: 'p', root: folder, schema_version: 1, created: 'x', solver_version_created: 'x', migrated_from: null, counts: {}, runs_by_state: {}, issues: [], issues_total: 0 };
          if (tool === 'list_runs') return { runs: [], total: 0, next_offset: null, skipped: [] };
          if (tool === 'list_meshes') return { meshes: [], total: 0, next_offset: null, unread: [] };
          return { cursor: 0, entries: [], total: 0 };
        },
        onExit: () => undefined,
        close: async () => void closed.push(folder),
      });
      let pick = a;
      const home = new ProjectHome({
        engine: { configured: true, path: '/e/rcfd-mcp', from: 'RCFD_STUDIO_MCP' },
        recent,
        pickFolder: async () => pick,
        startServer: async (_e, f) => {
          if (f === b) await new Promise<void>((r) => (release = r));
          return server(f);
        },
        listFiles: async () => ({ names: [], total: 0 }),
        pollMs: 50,
      });
      await home.init();
      await home.pick();
      expect(home.state.project?.folder).toBe(a);
      const recentBefore = JSON.stringify(list);
      expect(home.cancelOpen()).toBe(false);
      pick = b;
      const opening = home.pick();
      await expect.poll(() => home.state.opening).toBe(b);
      expect(home.cancelOpen()).toBe(true);
      expect(home.state.opening).toBeNull();
      expect(home.state.project?.folder).toBe(a);
      await expect.poll(() => typeof release).toBe('function');
      release();
      await opening;
      expect(home.state.project?.folder).toBe(a);
      expect(home.state.failure).toBeNull();
      expect(JSON.stringify(list)).toBe(recentBefore);
      await expect.poll(() => closed).toContain(b);
      await home.close();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

test.describe('the field worker', () => {
  test('its arrays are the ones ./surface.ts and ./derived.ts make on the main thread, and the fallback gives the same', async () => {
    const { surface, values } = benchSoup(40, 20);
    const nCells = Math.max(...surface.cells) + 1;
    // the worker module run here, with a stand-in for its global scope
    const replies: unknown[] = [];
    const scope: { onmessage: ((e: { data: unknown }) => void) | null; postMessage: (m: unknown) => void } = { onmessage: null, postMessage: (m) => replies.push(m) };
    (globalThis as { self?: unknown }).self = scope;
    try {
      await import('../../src/renderer/field/prepare.worker.js');
    } finally {
      delete (globalThis as { self?: unknown }).self;
    }
    scope.onmessage!({ data: { op: 'surface', id: 1, points: surface.points.slice(), triangles: surface.triangles.slice(), cells: surface.cells.slice(), nCells } });
    scope.onmessage!({ data: { op: 'field', id: 2, surface: 1, values: values.slice() } });
    scope.onmessage!({ data: { op: 'field', id: 3, surface: 99, values: values.slice() } });
    const layers = meshLayers(surface, nCells);
    const comps = [values.slice(), values.map((v) => -v), values.map((v) => v / 2)];
    scope.onmessage!({ data: { op: 'glyphs', id: 4, surface: 1, components: comps.map((c) => c?.slice() ?? null), layers, nCells } });
    const [s, f, bad, g] = replies as [
      { positions: Float32Array; radius: number; layers: unknown },
      { scalars: Float32Array; values: Float32Array; range: unknown },
      { op: string },
      { glyphs: { segments: Float32Array; triangles: Uint32Array; vmax: number } },
    ];
    expect(s.layers).toEqual(layers);
    const ref = vectorGlyphs(surface, comps, layers, nCells);
    expect(ref.segments.length).toBeGreaterThan(0);
    expect(Array.from(g.glyphs.segments)).toEqual(Array.from(ref.segments));
    expect(Array.from(g.glyphs.triangles)).toEqual(Array.from(ref.triangles));
    expect(g.glyphs.vmax).toBe(ref.vmax);
    expect(Array.from(s.positions)).toEqual(Array.from(cornerPositions(surface)));
    expect(Array.from(f.scalars)).toEqual(Array.from(cornerScalars(surface.cells, values)));
    expect(Array.from(f.values)).toEqual(Array.from(values));
    expect(f.range).toEqual(shownRange(surface.cells, values));
    expect(bad.op).toBe('error');

    // this process has no Worker: FieldPrep computes the same arrays itself
    const prep = new FieldPrep();
    const ps = await prep.surface(surface, nCells);
    expect(ps.layers).toEqual(layers);
    expect(prep.onMainThread).toBe(true);
    expect(Array.from(ps.positions)).toEqual(Array.from(s.positions));
    expect(ps.radius).toBeCloseTo(s.radius, 6);
    const pf = await prep.field(ps, values.slice());
    expect(Array.from(pf.scalars)).toEqual(Array.from(f.scalars));
    expect(pf.range).toEqual(f.range);
    expect(Array.from((await prep.glyphs(ps, comps, layers, nCells)).segments)).toEqual(Array.from(ref.segments));
    prep.dispose();
  });
});

test('work.css uses only defined tokens, no colour literals, square corners and no drop shadow', () => {
  const css = readFileSync(path.join(appRoot, 'src', 'renderer', 'work', 'work.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(8);
  expect(used.filter((u) => !defined.has(u))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  expect(css.match(/border-radius|box-shadow/g) ?? []).toEqual([]);
});
