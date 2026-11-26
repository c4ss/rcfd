// F-979 (ADR 0807): the live canvas's pure parts and its session, without Electron.
//
// - U1: a ProjectSession on a fake project server, with a shown view holding a setting panel of cases/a.toml: a feed
//   entry of cases/a.toml makes the session read read_view again, so the view's canvas.settings is the engine's new
//   value as the change arrives; an entry of cases/b.toml reads nothing; the change entries keep the feed's reason and
//   changes.
// - U2: followedRun (the newest run of a case), changeOf (the change each panel shows) and actorOf (who made it).
import { expect, test } from '@playwright/test';
import type { CanvasPanel, ChangeEntry, ProjectTool, RunEntry } from '../../src/shared/project.js';
import { ProjectSession } from '../../src/main/project/session.js';
import type { ProjectServerLike } from '../../src/main/project/server.js';
import { actorOf, changeOf, followedRun, panelRun, settingChange } from '../../src/renderer/canvas/model.js';
import { until } from '../support/fake-service.js';

interface World {
  nu: number;
  feed: Record<string, unknown>[];
}

const VIEW = { version: 1, panels: [{ kind: 'setting', case: 'cases/a.toml', setting: 'physics.nu' }] };

class FakeServer implements ProjectServerLike {
  readonly info = { name: 'rcfd', version: '0.0.0-test' };
  exited = false;
  calls: ProjectTool[] = [];
  constructor(private readonly w: World) {}
  async call(tool: string, args?: Record<string, unknown>): Promise<unknown> {
    this.calls.push(tool as ProjectTool);
    const w = this.w;
    switch (tool) {
      case 'project_info':
        return { name: 'fake', root: '/f', counts: {}, runs_by_state: {}, issues: [], issues_total: 0 };
      case 'list_runs':
        return { runs: [], total: 0, next_offset: null, skipped: [] };
      case 'list_meshes':
        return { meshes: [], total: 0, next_offset: null, unread: [] };
      case 'list_trash':
        return { items: [], total: 0, bytes: 0 };
      case 'read_view':
        return {
          exists: true,
          path: '.rcfd/views/current.json',
          id: 'fnv64:v1',
          hash: 'fnv64:v1',
          shown: '2026-11-26T10:00:00Z',
          actor: { client: 'mcp', instance: { pid: 9 } },
          reason: null,
          view: VIEW,
          settings: [{ panel: 0, case: 'cases/a.toml', setting: 'physics.nu', value: w.nu }],
        };
      case 'read_feed': {
        const cursor = w.feed.length ? (w.feed[w.feed.length - 1]!['n'] as number) : 0;
        if (args?.['since'] === undefined) return { cursor, entries: [], total: w.feed.length };
        const since = Number(args['since']);
        const entries = w.feed.filter((e) => (e['n'] as number) > since);
        return { cursor, entries, next: entries.length ? entries[entries.length - 1]!['n'] : null, dropped: 0 };
      }
    }
    return {};
  }
  onExit(): void {}
  async close(): Promise<void> {
    this.exited = true;
  }
}

test('U1: an entry of a setting panel’s case reads read_view again; an entry of another case does not', async () => {
  const w: World = { nu: 0.01, feed: [] };
  const server = new FakeServer(w);
  const s = await ProjectSession.open(
    '/f',
    '/e/rcfd-mcp',
    { startServer: async () => server, listFiles: async () => ({ names: [], total: 0 }), now: () => new Date().toISOString(), pollMs: 20 },
    () => undefined,
  );
  const reads = () => server.calls.filter((c) => c === 'read_view').length;
  try {
    await until(() => (s.view.canvas?.settings?.[0]?.value === 0.01 ? true : null), 5000, 'the first read_view');
    const first = reads();
    w.feed.push({ n: 1, source: 'journal', kind: 'edit', who: 'edit_case', path: 'cases/b.toml', text: 'edit_case by mcp: cases/b.toml' });
    await until(() => (s.view.changes.length === 1 ? true : null), 5000, 'the entry of cases/b.toml');
    expect(reads(), 'an entry of another case read read_view').toBe(first);
    w.nu = 0.005;
    w.feed.push({
      n: 2,
      source: 'journal',
      kind: 'edit',
      who: 'edit_case',
      path: 'cases/a.toml',
      actor: { client: 'mcp', instance: { pid: 42, started: 'x' } },
      reason: 'Re 200',
      changes: [{ path: 'physics.nu', old: 0.01, new: 0.005 }],
      text: 'edit_case by mcp: cases/a.toml; why: Re 200',
    });
    await until(() => (s.view.changes.length === 2 ? true : null), 5000, 'the entry of cases/a.toml');
    expect(reads(), 'the entry of the setting panel’s case did not read read_view').toBe(first + 1);
    // read before the change is put in the view: the value comes with its change
    expect(s.view.canvas?.settings).toEqual([{ panel: 0, case: 'cases/a.toml', setting: 'physics.nu', value: 0.005 }]);
    expect(s.view.changes[0]).toMatchObject({ n: 2, reason: 'Re 200', changes: [{ path: 'physics.nu', old: 0.01, new: 0.005 }], actor: { client: 'mcp', pid: 42 } });
    expect(s.view.changes[1]!.reason).toBeUndefined();
  } finally {
    await s.close();
  }
});

const run = (id: string, kase: string, created: string): RunEntry => ({ id, case: kase, created, state: 'finished', step: 1, time: 0, metrics: {} }) as RunEntry;
const change = (o: Partial<ChangeEntry> & { n: number }): ChangeEntry => ({ source: 'journal', kind: 'edit', text: '', path: null, run: null, t: null, seen: '', who: null, actor: null, ...o });

test('U2: the run a case panel follows, the change each panel shows, and who made it', () => {
  const runs = [
    run('r-1', 'cases/a.toml', '2026-11-26T10:00:00Z'),
    run('r-3', 'cases\\a.toml', '2026-11-26T11:00:00Z'),
    run('r-2', 'cases/a.toml', '2026-11-26T11:00:00Z'),
    run('r-9', 'cases/b.toml', '2026-11-26T12:00:00Z'),
  ];
  expect(followedRun(runs, 'cases/a.toml')?.id).toBe('r-3');
  expect(followedRun(runs, 'cases\\b.toml')?.id).toBe('r-9');
  expect(followedRun(runs, 'cases/c.toml')).toBeNull();
  const monitorOfCase: CanvasPanel = { kind: 'monitor', case: 'cases/a.toml', monitor: 'lid_drag' };
  expect(panelRun(monitorOfCase, runs)).toBe('r-3');
  expect(panelRun({ kind: 'monitor', run: 'r-1', monitor: 'lid_drag' }, runs)).toBe('r-1');

  // newest first, as the view keeps them
  const changes = [
    change({ n: 6, source: 'run', kind: 'run_state', run: 'r-3', reason: 'max steps' }),
    change({ n: 5, who: 'edit_case', path: 'cases/a.toml', changes: [{ path: 'physics.nu', old: 0.01, new: 0.005 }], reason: 'Re 200', actor: { client: 'mcp', pid: 42 } }),
    change({ n: 4, who: 'edit_case', path: 'cases/a.toml', changes: [{ path: 'mesh.nx', old: 32, new: 64 }], reason: 'refine', actor: { client: 'mcp', pid: 42 } }),
    change({ n: 3, who: 'edit_case', path: 'cases/b.toml', changes: [{ path: 'physics.nu', old: 1, new: 2 }] }),
    change({ n: 2, source: 'external', kind: 'external', path: 'cases\\a.toml' }),
    change({ n: 1, kind: 'mesh_imported', who: 'import_mesh', path: 'meshes/m.msh' }),
  ];
  const setting: CanvasPanel = { kind: 'setting', case: 'cases/a.toml', setting: 'physics.nu' };
  const mesh: CanvasPanel = { kind: 'mesh', case: 'cases/a.toml' };
  expect(changeOf(setting, changes)?.n).toBe(5);
  expect(settingChange(changeOf(setting, changes)!, 'physics.nu')).toEqual({ old: 0.01, new: 0.005 });
  // a setting panel passes over a change of another setting of its case, and takes an entry without changes
  expect(changeOf({ ...setting, setting: 'physics.rho' }, changes)?.n).toBe(2);
  // a mesh panel takes a mesh.* change and passes over physics.nu; an entry without changes; a meshes/ entry
  expect(changeOf(mesh, changes)?.n).toBe(4);
  expect(changeOf(mesh, changes.filter((c) => c.n !== 4))?.n).toBe(2);
  expect(changeOf(mesh, changes.filter((c) => c.n < 2))?.n).toBe(1);
  // the F-976 mesh panel of a file shows no change; a monitor panel the entries of its run
  expect(changeOf({ kind: 'mesh', mesh: 'meshes/m.msh' }, changes)).toBeNull();
  expect(changeOf(monitorOfCase, changes, 'r-3')?.n).toBe(6);
  expect(changeOf(monitorOfCase, changes, 'r-1')).toBeNull();
  // who made it: the journal's actor, the runner for a run's own event, none for an external change
  expect(actorOf(changes[0]!)).toEqual({ client: 'runner', pid: null });
  expect(actorOf(changes[1]!)).toEqual({ client: 'mcp', pid: 42 });
  expect(actorOf(changes[4]!)).toBeNull();
});
