// Unit tests of a learner's first path (F-813): which step a project is at, read only from the engine's lists (G12);
// the empty states on the way; the path's run action against the engine's own sources and inventory (G16) and through
// run control with a fake project server (start_run with the engine's start plan); the home's watch of a folder that is
// not a project yet (it says what the folder holds, opens it once its project file appears, and stops on dismiss); the
// words the path names checked against the engine's sources (the template's text, the README's disclaimer, the docs it
// cites, the controls it says hold the full control: G6, G15, G19); and its stylesheet (tokens only).
import { expect, test } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import {
  FIRST_CASE,
  FIRST_CASE_NAME,
  FIRST_FIELD,
  FIRST_RUN_ACTIONS,
  FIRST_RUN_EXAMPLE,
  FIRST_TEMPLATE,
  FULL_CONTROL,
  getFirstRunAction,
} from '../../src/shared/actions/first.js';
import { RUN_ACTIONS, getRunAction, runEquivalents, runParamsProblem } from '../../src/shared/actions/runs.js';
import type { ProjectTool, ProjectView, RunDetail, RunEntry, RunRequest } from '../../src/shared/project.js';
import { RunControl, settle } from '../../src/main/project/control.js';
import { countEntries, PROJECT_FILE, ProjectHome, type RecentLike } from '../../src/main/project/home.js';
import { StartFailure, type ProjectServerLike } from '../../src/main/project/server.js';
import type { StoredRecent } from '../../src/main/project/recent.js';
import { fieldsFolder, firstStep, outcomeSentence, plainState, sameRunCommand, STATIONS } from '../../src/renderer/first/model.js';
import { DISCLAIMER, EXAMPLE_LIMITS, TERMS, TERMS_SOURCE } from '../../src/renderer/first/words.js';
import { RESULT_LINK } from '../../src/renderer/first/target.js';
import { requestWords, withStartedRuns } from '../../src/renderer/runs/control-model.js';
import { runRows } from '../../src/renderer/runs/model.js';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { until } from '../support/fake-service.js';
import { appRoot } from '../support/rcfd.js';

const repo = path.resolve(appRoot, '..', '..');
const read = (...p: string[]) => readFileSync(path.join(repo, ...p), 'utf8');
const src = (...p: string[]) => readFileSync(path.join(appRoot, 'src', ...p), 'utf8');
const squash = (s: string) => s.replace(/\s+/g, ' ').trim();

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

const detail = (id: string, over: Partial<RunDetail> = {}): RunDetail => ({
  id,
  state: 'finished',
  step: 88,
  time: 0,
  updated: '2026-11-16T12:00:01Z',
  pid: null,
  reason: 'converged: residual targets met: res_u 6.081e-7 < 1.000e-6, res_p 9.413e-7 < 1.000e-6',
  exitCode: 0,
  solverVersion: 'rcfd 0.1.0',
  steady: true,
  finish: { state: 'converged', iterations: 88, elapsed: 0.65, reason: 'residual targets met' },
  history: [],
  readAt: '2026-11-16T12:00:02Z',
  ...over,
});

test.describe('the step a project is at (read from the engine’s lists only)', () => {
  test('empty project: make; the example case and no run: run; a run going: watch; ended: result', () => {
    expect(firstStep(view([]))).toEqual({ at: 'make' });
    expect(firstStep(view([FIRST_CASE]))).toEqual({ at: 'run', casePath: FIRST_CASE });
    const going = firstStep(view([FIRST_CASE], [{ state: 'running', step: 12 }]));
    expect(going?.at).toBe('watch');
    expect(going && 'run' in going ? going.run.step : null).toBe(12);
    const id = 'r-20261110T120000-0000';
    const done = firstStep(view([FIRST_CASE], [{ state: 'finished', step: 88, metrics: { lid_drag: -0.0085 } }], { runDetails: { [id]: detail(id) } }));
    expect(done?.at).toBe('result');
    expect(done && 'run' in done ? plainState(done.run).mark : null).toBe('converged');
  });

  test('the newest run of the example decides; an absolute or backslashed case path is the same case', () => {
    const s = firstStep(view([FIRST_CASE], [{ state: 'finished', created: '2026-11-10T00:00:00Z' }, { state: 'running', created: '2026-11-12T00:00:00Z', case: `C:\\p\\first\\cases\\${FIRST_CASE_NAME}.toml` }]));
    expect(s?.at).toBe('watch');
  });

  test('past the path: another case, a run of another case, a list cut short or one that failed to read', () => {
    expect(firstStep(view(['cases/mine.toml']))).toBeNull();
    expect(firstStep(view([FIRST_CASE, 'cases/mine.toml']))).toBeNull();
    expect(firstStep(view([FIRST_CASE], [{ case: 'cases/mine.toml' }]))).toBeNull();
    expect(firstStep(view([], [{ case: FIRST_CASE }]))).toBeNull();
    const v = view([]);
    expect(firstStep({ ...v, cases: { items: [], total: 0, error: 'cases/ cannot be read' } })).toBeNull();
    expect(firstStep({ ...v, runs: { items: [], total: 3000, error: null } })).toBeNull();
  });

  test('a run this window started shows as watched before the engine’s list holds it', () => {
    const req: RunRequest = {
      seq: 1,
      action: FIRST_RUN_EXAMPLE,
      params: { case: FIRST_CASE },
      run: 'r-20261116T120000-abcd',
      case: FIRST_CASE,
      at: '2026-11-16T12:00:00Z',
      answeredAt: 1,
      ms: 1,
      outcome: 'accepted',
      state: 'queued',
      settled: false,
      slow: false,
      failure: null,
      mcp: { tool: 'start_run', arguments: { case: FIRST_CASE, store: 'plan' } },
      cli: { argv: ['rcfd-runner', 'start', FIRST_CASE, '--project', 'C:\\p\\first'], cwd: 'C:\\p\\first' },
    };
    const v = view([FIRST_CASE], [], { runRequests: [req] });
    expect(withStartedRuns(v.runs.items, v.runRequests).map((r) => r.id)).toEqual(['r-20261116T120000-abcd']);
    expect(firstStep(v)?.at).toBe('watch');
    expect(requestWords(req)).toMatch(/^Run the first example from the Studio at /);
  });

  test('every state has a plain word, its pictogram, and the precise word one step away; outcomes use the engine’s numbers', () => {
    const rows = runRows(
      ['queued', 'running', 'paused', 'finished', 'failed', 'cancelled'].map((state, i) => ({ id: `r${i}`, case: FIRST_CASE, created: 'x', state, step: 7, time: null, metrics: {} })),
      { r3: detail('r3', { finish: { state: 'converged', iterations: 88, elapsed: 0.6, reason: null } }) },
      {},
    );
    const words = rows.map((r) => plainState(r));
    expect(words.map((w) => w.mark)).toEqual(['queued', 'running', 'paused', 'converged', 'failed', 'cancelled']);
    for (const w of words) expect(w.precise.length).toBeGreaterThan(10);
    expect(outcomeSentence(rows[3]!)).toContain('88 rounds of calculation');
    expect(outcomeSentence(rows[4]!)).toMatch(/^The run failed after 7 rounds/);
    // a finished run whose record is not read yet is not called unsettled
    const bare = runRows([{ id: 'r9', case: FIRST_CASE, created: 'x', state: 'finished', step: 5, time: null, metrics: {} }], {}, {})[0]!;
    expect(outcomeSentence(bare)).toMatch(/^Done\. Reading/);
    expect(STATIONS.map((s) => s.title)).toEqual(['Make the example', 'Run it', 'Watch it work', 'See the result']);
  });

  test('the fields folder of a run and the same run from the command line', () => {
    expect(fieldsFolder('C:\\p\\first\\', 'r-1')).toBe('C:\\p\\first\\runs\\r-1\\fields');
    expect(fieldsFolder('/p/first', 'r-1')).toBe('/p/first/runs/r-1/fields');
    expect(sameRunCommand(FIRST_CASE, '/p', ['output.store_interval=6', 'output.store_precision=f32'])).toEqual([
      'rcfd-runner', 'start', FIRST_CASE, '--set', 'output.store_interval=6', '--set', 'output.store_precision=f32', '--project', '/p',
    ]);
  });
});

// ---- the path's run action against the engine

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

// ---- the words: checked against the engine's sources and the Studio's own controls

test.describe('the words of the path (G6, G15, G19)', () => {
  test('the disclaimer is the README’s own, and the limits say the result is no validation', () => {
    expect(squash(read('README.md'))).toContain(DISCLAIMER);
    expect(EXAMPLE_LIMITS).toContain('Not a validation');
    expect(EXAMPLE_LIMITS).toContain('not that the answer is right');
  });

  test('every precise term is a row of the engine’s glossary, and words.ts keeps no help text of its own (F-830, G19)', () => {
    expect(TERMS_SOURCE).toBe('docs/glossary.md');
    const glossary = read(TERMS_SOURCE);
    const rows = new Set([...glossary.matchAll(/^\| ([^|]+?) \| [^|]+ \| [^|]+ \|$/gm)].map((m) => m[1]!));
    for (const [step, terms] of Object.entries(TERMS)) {
      expect(terms.length, step).toBeGreaterThan(0);
      for (const t of terms) {
        expect(rows.has(t.term), `${step}: '${t.term}' is not a term of docs/glossary.md`).toBe(true);
        expect(Object.keys(t).sort(), `${step}: ${t.plain}`).toEqual(['plain', 'term']);
      }
    }
    // the meanings are the engine's (help.glossary): none of them is written in the Studio's words file
    const words = src('renderer', 'first', 'words.ts');
    for (const m of glossary.matchAll(/^\| ([^|]+?) \| ([^|]+?) \| [^|]+ \|$/gm)) {
      expect(words, `the meaning of ${m[1]}`).not.toContain(m[2]!.slice(0, 40));
    }
    expect(words).not.toMatch(/precise:/);
  });

  test('the full controls each step names are controls this Studio has', () => {
    const home = src('renderer', 'project', 'ProjectHome.tsx');
    const runs = src('renderer', 'runs', 'RunControls.tsx');
    expect(FULL_CONTROL.make).toContain('New case');
    expect(home).toMatch(/>\s*New case\s*</);
    expect(home).toMatch(/>\s*Edit\s*</);
    for (const label of ['Queue…', 'Stop…', 'Cancel…']) {
      expect(FULL_CONTROL.run).toContain(label);
      expect(runs, label).toContain(label);
    }
    expect(runs).toContain('label="Pause"');
    // no command palette is named until one exists in this build (G6: never suggest a capability that does not exist)
    expect(Object.values(FULL_CONTROL).join(' ')).not.toMatch(/palette/i);
    expect(RESULT_LINK.label).toBe('Open the run');
    expect(RESULT_LINK.name).toBe('Open the run in the workspace');
    // the workspace the result opens exists in this build (G6)
    expect(src('renderer', 'project', 'ProjectHome.tsx')).toMatch(/label: 'Workspace'/);
  });

  test('the path makes and writes nothing itself: its renderer code names no file operation, tool or bridge', () => {
    const code = ['FirstPath.tsx', 'model.ts', 'words.ts', 'target.ts'].map((f) => src('renderer', 'first', f)).join('\n');
    for (const bad of [/window\.studio/, /writeFile|mkdir|unlink/, /'new_case'|"new_case"/, /callTool|\.call\('/]) expect(code).not.toMatch(bad);
    // it reaches the engine only through the doors: the case door (new_case), the project door (run control), the data door
    expect(code).toContain("from '../case/door.js'");
    expect(code).toContain("from '../project/door.js'");
    expect(code).toContain("from '../data/door.js'");
    // the empty-state words of the F-805 home are not repeated (they stay unique on the page)
    for (const phrase of ['No cases yet.', 'No runs yet.', 'No mesh files.', 'No notes.', 'No recent projects.', 'The engine said', 'Open project folder…<']) {
      expect(code, phrase).not.toContain(phrase);
    }
  });

  test('first/first.css uses only defined tokens and no colour literals; corners stay square', () => {
    const css = src('renderer', 'first', 'first.css').replace(/\/\*[\s\S]*?\*\//g, '');
    const defined = new Set(cssCustomProperties().map(([k]) => k));
    for (const m of css.matchAll(/var\((--[a-z0-9-]+)/g)) expect(defined.has(m[1]!), m[1]).toBe(true);
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i);
    expect(css).not.toMatch(/border-radius/);
  });
});
