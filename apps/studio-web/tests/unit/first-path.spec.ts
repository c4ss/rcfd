// Unit tests of what stays of a learner's first path (F-813) after ADR 0808 (F-981) retired its surface (the maintainer's
// answer of 2026-11-26, ADR 0803): the path's run action first.run_example against the engine's own sources and inventory
// (G16) and through run control with a fake project server (start_run with the engine's start plan); the home's watch of
// a folder that is not a project yet (it says what the folder holds, opens it once its project file appears, and stops on
// dismiss). The tests of the surface (the step a project is at, the words of the path, its stylesheet) went with it.
import { expect, test } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import {
  FIRST_CASE,
  FIRST_FIELD,
  FIRST_RUN_ACTIONS,
  FIRST_RUN_EXAMPLE,
  FIRST_TEMPLATE,
  getFirstRunAction,
} from '../../src/shared/actions/first.js';
import { RUN_ACTIONS, getRunAction, runEquivalents, runParamsProblem } from '../../src/shared/actions/runs.js';
import type { ProjectTool, ProjectView, RunEntry } from '../../src/shared/project.js';
import { RunControl, settle } from '../../src/main/project/control.js';
import { countEntries, PROJECT_FILE, ProjectHome, type RecentLike } from '../../src/main/project/home.js';
import { StartFailure, type ProjectServerLike } from '../../src/main/project/server.js';
import type { StoredRecent } from '../../src/main/project/recent.js';
import { until } from '../support/fake-service.js';
import { appRoot } from '../support/rcfd.js';

const repo = path.resolve(appRoot, '..', '..');
const read = (...p: string[]) => readFileSync(path.join(repo, ...p), 'utf8');

// ---- views

const listing = <T>(items: T[]) => ({ items, total: items.length, error: null });

function view(cases: string[], runs: Partial<RunEntry>[] = [], extra: Partial<ProjectView> = {}): ProjectView {
  return {
    folder: 'C:\\p\\first',
    engine: { name: 'rcfd', version: '0.1.0', path: '/e/rcfd-mcp' },
    info: null,
    infoError: null,
    cases: listing(cases.map((p) => ({ path: p, runs: 0, meshes: [], unread: null }))),
    meshes: listing([]),
    runs: listing(runs.map((r, i) => ({ id: `r-2026111${i}T120000-000${i}`, case: FIRST_CASE, created: `2026-11-1${i}T12:00:00Z`, state: 'queued', step: 0, time: null, metrics: {}, ...r }))),
    notes: listing([]),
    skippedRuns: [],
    runDetails: {},
    runDetailErrors: {},
    changes: [],
    openedAt: '2026-11-16T12:00:00Z',
    readAt: '2026-11-16T12:00:00Z',
    liveness: { live: true, since: '2026-11-16T12:00:00Z' },
    ...extra,
  };
}

test.describe('first.run_example (G16)', () => {
  const a = getFirstRunAction(FIRST_RUN_EXAMPLE)!;

  test('it is start_run with the engine’s start plan, an execute tool the workspace dispatches; its CLI equivalent is rcfd-runner start', () => {
    expect(FIRST_RUN_ACTIONS.map((x) => x.id)).toEqual([FIRST_RUN_EXAMPLE]);
    expect(getRunAction(FIRST_RUN_EXAMPLE)).toBeUndefined();
    expect(RUN_ACTIONS.some((x) => (x.id as string) === FIRST_RUN_EXAMPLE)).toBe(false);
    expect(runParamsProblem(a, { case: FIRST_CASE })).toBeNull();
    expect(runParamsProblem(a, {})).not.toBeNull();
    const eq = runEquivalents(a, { case: FIRST_CASE }, '/p');
    expect(eq.mcp).toEqual({ tool: 'start_run', arguments: { case: FIRST_CASE, store: 'plan' } });
    expect(eq.cli.argv).toEqual(['rcfd-runner', 'start', FIRST_CASE, '--project', '/p']);
    const mcp = read('crates', 'rcfd-mcp', 'src', 'project.rs');
    const at = mcp.indexOf('name: "start_run",');
    const desc = mcp.slice(at, mcp.indexOf('Tool {', at));
    expect(desc).toContain('access: Access::Execute');
    expect(desc).toContain('"store": {"type": "string", "enum": ["plan", "none"]');
    expect(read('crates', 'rcfd-workspace', 'src', 'runs.rs')).toContain('Some("plan") => true');
    const inv = JSON.parse(readFileSync(path.join(appRoot, 'capabilities', 'inventory.json'), 'utf8')) as { capabilities: { id: string; access?: { project?: string } }[] };
    const byId = new Map(inv.capabilities.map((c) => [c.id, c]));
    expect(byId.get('mcp:start_run')?.access?.project).toBe('execute');
    expect(byId.get('mcp:new_case')?.access?.project).toBeDefined();
    expect(byId.has('cli:rcfd-runner start')).toBe(true);
    expect(byId.has('cli:rcfd-runner --project')).toBe(true);
  });

  test('the engine offers the template the path makes its case from, with the facts the words name', () => {
    const t = read('crates', 'rcfd-workspace', 'src', 'templates.rs');
    expect(t).toContain(`("${FIRST_TEMPLATE}", `);
    const body = t.slice(t.indexOf('pub const LID_DRIVEN_CAVITY'));
    for (const fact of ['generator = "cavity_2d"', '[boundaries.lid]', 'algorithm = "simplec"', 'tolerance_u', 'tolerance_p', 'max_iterations', '[reports.lid_drag]', 'component = "viscous"', 'direction = [1.0, 0.0, 0.0]']) {
      expect(body, fact).toContain(fact);
    }
    // the template stores no fields itself: the start plan is what gives the first result its picture
    expect(body.slice(0, body.indexOf('"#;'))).not.toContain('store_interval');
    const fields = /DEFAULT_FIELDS: \[&str; \d+\] = \[([^\]]*)\]/.exec(read('crates', 'rcfd-api', 'src', 'fieldstore.rs'))?.[1] ?? '';
    expect(fields).toContain(`"${FIRST_FIELD}"`);
  });

  test('through run control, with a fake project server: one start_run with the plan, refused for a case the project does not list', async () => {
    const calls: { tool: string; args: unknown }[] = [];
    const server: ProjectServerLike = {
      info: { name: 'rcfd', version: 'test' },
      exited: false,
      async call(tool: ProjectTool | string, args?: Record<string, unknown>) {
        calls.push({ tool, args });
        return { run: 'r-20261116T120000-abcd', case: FIRST_CASE, state: 'queued', store: { applied: true, set: ['output.store_interval=6', 'output.store_precision=f32'] } };
      },
      onExit() {},
      async close() {},
    } as unknown as ProjectServerLike;
    const rc = new RunControl('/p', { startControlServer: async () => server, ensureRunner: async () => ({ ok: true }), now: () => '2026-11-16T12:00:00Z' });
    const r = await rc.dispatch(FIRST_RUN_EXAMPLE, { case: FIRST_CASE }, view([FIRST_CASE]));
    expect(r.ok).toBe(true);
    expect(calls).toEqual([{ tool: 'start_run', args: { case: FIRST_CASE, store: 'plan' } }]);
    expect(r.ok && r.request.run).toBe('r-20261116T120000-abcd');
    expect(r.ok && (r.result['store'] as { set: string[] }).set).toEqual(['output.store_interval=6', 'output.store_precision=f32']);
    // settled once the list holds the run (the F-807 rule, found through the joined lookup)
    const listed = view([FIRST_CASE], [{ state: 'queued' }]);
    const asked = { ...(r.ok ? r.request : null)!, run: listed.runs.items[0]!.id };
    expect(settle([asked], listed, asked.answeredAt + 10)[0]!.settled).toBe(true);
    const refused = await rc.dispatch(FIRST_RUN_EXAMPLE, { case: FIRST_CASE }, view(['cases/other.toml']));
    expect(refused.ok).toBe(false);
    expect(!refused.ok && refused.failure.code).toBe('unavailable');
    expect(!refused.ok && refused.failure.recovery).toContain('Make the example again');
    expect(calls).toHaveLength(1);
    await rc.close();
  });
});

// ---- a folder that is not a project yet

class Server implements ProjectServerLike {
  readonly info = { name: 'rcfd', version: '0.0.0-test' };
  exited = false;
  async call(tool: ProjectTool): Promise<unknown> {
    switch (tool) {
      case 'project_info':
        return {
          name: 'made elsewhere',
          root: '/f',
          schema_version: 1,
          created: '2026-11-16T10:00:00Z',
          solver_version_created: 'rcfd 0.1.0',
          migrated_from: null,
          counts: { cases: 0, meshes: 0, designs: 0, notes: 0, runs: 0, skipped_runs: 0 },
          runs_by_state: {},
          issues: [],
          issues_total: 0,
        };
      case 'list_runs':
        return { runs: [], total: 0, next_offset: null, skipped: [] };
      case 'list_meshes':
        return { meshes: [], total: 0, next_offset: null, unread: [] };
      case 'read_feed':
        return { cursor: 0, entries: [], total: 0 };
      default:
        return {};
    }
  }
  onExit(): void {}
  async close(): Promise<void> {
    this.exited = true;
  }
}

const memoryRecent = (): RecentLike & { list: StoredRecent[] } => {
  const r = {
    list: [] as StoredRecent[],
    load: async () => r.list,
    save: async (l: readonly StoredRecent[]) => {
      r.list = [...l];
    },
  };
  return r;
};

test.describe('the home and a folder that is not a project (nothing written)', () => {
  test('an empty folder: the failure says so; once a project file appears the home opens the folder by itself', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-first-'));
    try {
      const empty = path.join(dir, 'empty');
      await mkdir(empty);
      const home = new ProjectHome({
        engine: { configured: true, path: '/e/rcfd-mcp', from: 'RCFD_STUDIO_MCP' },
        recent: memoryRecent(),
        pickFolder: async () => empty,
        startServer: async (_e, f) => {
          if (!existsSync(path.join(f, PROJECT_FILE))) throw new StartFailure('not_a_project', 'no', 'error: --project: not an rcfd project folder');
          return new Server();
        },
        listFiles: async () => ({ names: [], total: 0 }),
        pollMs: 50,
        firstWatchMs: 50,
      });
      await home.init();
      await home.pick();
      expect(home.state.failure).toMatchObject({ kind: 'not_a_project', folder: empty, folderState: { entries: 0, watching: true } });
      expect(home.state.project).toBeNull();
      // the test stands in for the command the engine does not have yet: the project file appears
      await writeFile(path.join(empty, PROJECT_FILE), '{}\n');
      await until(async () => (home.state.project?.folder === empty ? true : null), 5000, 'the folder to open by itself');
      expect(home.state.failure).toBeNull();
      await home.shutdown();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  test('a folder with files counts them; dismiss stops the watch; a project file the engine refuses is not watched', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-first-'));
    try {
      const some = path.join(dir, 'some');
      await mkdir(some);
      await writeFile(path.join(some, 'a.txt'), 'a');
      await writeFile(path.join(some, 'b.txt'), 'b');
      const broken = path.join(dir, 'broken');
      await mkdir(broken);
      await writeFile(path.join(broken, PROJECT_FILE), 'not json');
      let pick = some;
      let starts = 0;
      const home = new ProjectHome({
        engine: { configured: true, path: '/e/rcfd-mcp', from: 'RCFD_STUDIO_MCP' },
        recent: memoryRecent(),
        pickFolder: async () => pick,
        startServer: async () => {
          starts++;
          throw new StartFailure('not_a_project', 'no', 'error: not an rcfd project folder');
        },
        listFiles: async () => ({ names: [], total: 0 }),
        firstWatchMs: 30,
      });
      await home.init();
      await home.pick();
      expect(home.state.failure?.folderState).toEqual({ entries: 2, watching: true });
      home.dismiss();
      await writeFile(path.join(some, PROJECT_FILE), '{}');
      await new Promise((r) => setTimeout(r, 200));
      expect(starts).toBe(1);
      expect(home.state.failure).toBeNull();
      pick = broken;
      await home.pick();
      expect(home.state.failure?.folderState).toEqual({ entries: 1, watching: false });
      await new Promise((r) => setTimeout(r, 150));
      expect(starts).toBe(2);
      expect(await countEntries(path.join(dir, 'no-such'))).toBeNull();
      await home.shutdown();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
