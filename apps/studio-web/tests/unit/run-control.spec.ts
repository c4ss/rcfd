// Unit tests of run control (F-807): the run-control actions (src/shared/actions/runs.ts) against the engine's own
// sources and inventory (G16: each names a real engine entry point and its CLI equivalent), their availability, the
// words of their confirmations (G18) and of their refusals (G19); the main process's run control and the session with a
// fake project server (the request in the view at once, the runner's change settling it, a slow runner); the port's
// check of the `action` operation; what the run list shows of a request; and that the renderer reaches run control only
// through the project door.
import { expect, test } from '@playwright/test';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import {
  ACTIONS,
  RUN_ACTIONS,
  RUN_CONTROL_TOOLS,
  commandLine,
  describeRunAction,
  getRunAction,
  launched,
  runAvailability,
  runConfirmation,
  runEquivalents,
  runParamsProblem,
  type RegisteredRunAction,
  type RunActionContext,
} from '../../src/shared/actions/index.js';
import { RUN_SLOW_MS, type ProjectTool, type ProjectView, type RunEntry, type RunRequest } from '../../src/shared/project.js';
import { RunControl, engineFailure, settle, type ControlDeps } from '../../src/main/project/control.js';
import { parseProjectRequest } from '../../src/main/project/bridge.js';
import { ProjectSession } from '../../src/main/project/session.js';
import { StartFailure, type ProjectServerLike } from '../../src/main/project/server.js';
import { RpcError } from '../../src/main/service/rpc.js';
import { pendingByRun, requestWords, setEntries, withStartedRuns } from '../../src/renderer/runs/control-model.js';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { until } from '../support/fake-service.js';
import { appRoot } from '../support/rcfd.js';

const repo = path.resolve(appRoot, '..', '..');
const rust = (rel: string) => readFileSync(path.join(repo, rel), 'utf8');
const PROJECT = '/p/quick';

// ---- the engine, from its sources and the committed inventory (itself checked against the binaries, F-824)

interface Engine {
  /** The text of each project tool's descriptor in crates/rcfd-mcp/src/project.rs. */
  tool(name: string): string | null;
  /** Tools the workspace dispatches (crates/rcfd-workspace/src/lib.rs). */
  dispatched: Set<string>;
  inventory: Map<string, { kind: string; access?: { project?: string } }>;
}

function engine(): Engine {
  const src = rust('crates/rcfd-mcp/src/project.rs');
  const ws = rust('crates/rcfd-workspace/src/lib.rs');
  const inv = JSON.parse(readFileSync(path.join(appRoot, 'capabilities', 'inventory.json'), 'utf8')) as {
    capabilities: { id: string; kind: string; access?: { project?: string } }[];
  };
  return {
    tool: (name) => {
      const at = src.indexOf(`name: "${name}",`);
      if (at < 0) return null;
      const next = src.indexOf('Tool {', at);
      return src.slice(at, next < 0 ? undefined : next);
    },
    // the match arms of the workspace's dispatch: `"start_run" => …` and `"pause_run" | "resume_run" | "cancel_run" => …`
    dispatched: new Set(
      [...ws.matchAll(/\n\s+((?:"[a-z_]+"\s*\|\s*)*"[a-z_]+") => /g)].flatMap((m) => [...m[1]!.matchAll(/"([a-z_]+)"/g)].map((x) => x[1]!)),
    ),
    inventory: new Map(inv.capabilities.map((c) => [c.id, c])),
  };
}

/** Why a run action has no engine equivalent (G16), or an empty list. */
function engineProblems(a: RegisteredRunAction, e: Engine): string[] {
  const out: string[] = [];
  const sample: Record<string, unknown> = { case: 'cases/a.toml', run: 'r-20261116T120000-abcd', priority: 2, threads: 4, mem_mb: 512, set: ['solver.max_iterations=5'] };
  const params = Object.fromEntries(Object.keys(a.params).map((k) => [k, sample[k]]));
  expect(runParamsProblem(a, params), a.id).toBeNull();
  const eq = runEquivalents(a, params as never, PROJECT);
  if (a.engine.kind === 'mcp') {
    const t = a.engine.tool;
    const desc = e.tool(t);
    if (desc === null) out.push(`${a.id}: the MCP server has no project tool ${t}`);
    else {
      if (!desc.includes('access: Access::Execute')) out.push(`${a.id}: ${t} is not of the access class execute`);
      for (const k of Object.keys(eq.mcp!.arguments)) if (!desc.includes(`"${k}":`)) out.push(`${a.id}: ${t} takes no argument ${k}`);
    }
    if (!e.dispatched.has(t)) out.push(`${a.id}: the workspace does not dispatch ${t}`);
    if (e.inventory.get(`mcp:${t}`)?.access?.project !== 'execute') out.push(`${a.id}: the inventory has no execute tool mcp:${t}`);
  } else if (!e.inventory.has(`cli:rcfd-runner ${a.engine.command}`)) out.push(`${a.id}: rcfd-runner has no command ${a.engine.command}`);
  const [prog, cmd, ...rest] = eq.cli.argv;
  if (prog !== 'rcfd-runner') out.push(`${a.id}: the CLI equivalent is not rcfd-runner`);
  if (!e.inventory.has(`cli:rcfd-runner ${cmd}`)) out.push(`${a.id}: rcfd-runner has no command ${cmd}`);
  for (const w of rest.filter((x) => x.startsWith('--'))) {
    if (!e.inventory.has(`cli:rcfd-runner ${w}`) && !e.inventory.has(`cli:rcfd-runner ${cmd} ${w}`)) out.push(`${a.id}: rcfd-runner ${cmd} has no option ${w}`);
  }
  if (eq.cli.cwd !== PROJECT) out.push(`${a.id}: the CLI equivalent does not run in the project folder`);
  return out;
}

test.describe('the run-control actions (G16)', () => {
  test('every action names a real engine entry point (an execute tool of the project server, or a runner command) and an rcfd-runner equivalent', () => {
    const e = engine();
    expect(RUN_ACTIONS.map((a) => a.id)).toEqual([
      'project.run_case',
      'project.queue_case',
      'project.pause_run',
      'project.resume_run',
      'project.stop_run',
      'project.cancel_run',
      'project.start_runner',
    ]);
    expect(RUN_ACTIONS.flatMap((a) => engineProblems(a, e))).toEqual([]);
    // every run-control tool is used, and only those
    expect(new Set(RUN_ACTIONS.flatMap((a) => (a.engine.kind === 'mcp' ? [a.engine.tool] : [])))).toEqual(new Set(RUN_CONTROL_TOOLS));
  });

  test('the G16 check fails on an action without an engine equivalent', () => {
    const e = engine();
    const base = getRunAction('project.pause_run')!;
    const noTool = { ...base, id: 'project.x', engine: { kind: 'mcp', tool: 'freeze_run' } } as unknown as RegisteredRunAction;
    const readTool = { ...base, id: 'project.y', engine: { kind: 'mcp', tool: 'wait_run' } } as unknown as RegisteredRunAction;
    const badCli = { ...base, id: 'project.z', cli: (p: { run: string }, project: string) => ['rcfd-runner', 'freeze', p.run, '--fast', '--project', project] } as unknown as RegisteredRunAction;
    const noCmd = { ...base, id: 'project.w', engine: { kind: 'runner', command: 'warm' } } as unknown as RegisteredRunAction;
    expect(engineProblems(noTool, e)).toEqual(['project.x: the MCP server has no project tool freeze_run', 'project.x: the workspace does not dispatch freeze_run', 'project.x: the inventory has no execute tool mcp:freeze_run']);
    expect(engineProblems(readTool, e)).toEqual(['project.y: wait_run is not of the access class execute', 'project.y: the inventory has no execute tool mcp:wait_run']);
    expect(engineProblems(badCli, e)).toEqual(['project.z: rcfd-runner has no command freeze', 'project.z: rcfd-runner freeze has no option --fast']);
    expect(engineProblems(noCmd, e)).toEqual(['project.w: rcfd-runner has no command warm']);
  });

  test('ids, titles and descriptions for the palette and help; no title repeats a service action', () => {
    const titles = new Set(ACTIONS.map((a) => a.title));
    for (const a of RUN_ACTIONS) {
      expect(a.id).toMatch(/^project\.[a-z_]+$/);
      expect(a.title).toMatch(/^[A-Z][^.]+$/);
      expect(titles.has(a.title), a.title).toBe(false);
      expect(a.description.endsWith('.')).toBe(true);
      expect(a.effect).toBe('change');
      const d = describeRunAction(a);
      expect(JSON.parse(JSON.stringify(d))).toEqual(d);
    }
    expect(new Set(RUN_ACTIONS.map((a) => a.title)).size).toBe(RUN_ACTIONS.length);
    expect(ACTIONS.some((a) => a.id.startsWith('project.'))).toBe(false);
  });

  test('the MCP and CLI equivalents of each action', () => {
    const eq = (id: string, p: Record<string, unknown>) => runEquivalents(getRunAction(id)!, p as never, PROJECT);
    expect(eq('project.run_case', { case: 'cases/a.toml' })).toEqual({
      // restated for ADR 0737: the Studio's Run is start_run's store plan (the maintainer's report of a run with no field)
      mcp: { tool: 'start_run', arguments: { case: 'cases/a.toml', store: 'plan' } },
      mcpNote: null,
      cli: { argv: ['rcfd-runner', 'start', 'cases/a.toml', '--project', PROJECT], cwd: PROJECT },
      cliNote:
        'rcfd-runner start writes no line in the project journal; start_run does (who: start_run). rcfd-runner start has no store plan: the settings the plan added are in start_run’s answer (store.set) and in the run’s run.json, to pass as --set.',
    });
    const q = eq('project.queue_case', { case: 'cases/a.toml', priority: -2, threads: 4, mem_mb: 800, set: ['solver.max_iterations=5', 'physics.nu=0.02'] });
    expect(q.mcp).toEqual({ tool: 'start_run', arguments: { case: 'cases/a.toml', store: 'plan', priority: -2, threads: 4, mem_mb: 800, set: ['solver.max_iterations=5', 'physics.nu=0.02'] } });
    expect(q.cli.argv).toEqual(['rcfd-runner', 'start', 'cases/a.toml', '--priority', '-2', '--threads', '4', '--mem-mb', '800', '--set', 'solver.max_iterations=5', '--set', 'physics.nu=0.02', '--project', PROJECT]);
    expect(eq('project.queue_case', { case: 'cases/a.toml' }).cli.argv).toEqual(['rcfd-runner', 'start', 'cases/a.toml', '--project', PROJECT]);
    for (const [id, tool, cmd] of [
      ['project.pause_run', 'pause_run', 'pause'],
      ['project.resume_run', 'resume_run', 'resume'],
      ['project.stop_run', 'cancel_run', 'cancel'],
      ['project.cancel_run', 'cancel_run', 'cancel'],
    ] as const) {
      const r = eq(id, { run: 'r-1' });
      expect(r.mcp).toEqual({ tool, arguments: { run: 'r-1' } });
      expect(r.cli.argv).toEqual(['rcfd-runner', cmd, 'r-1', '--project', PROJECT]);
    }
    const ensure = eq('project.start_runner', {});
    expect(ensure.mcp).toBeNull();
    expect(ensure.mcpNote).toBe('No MCP tool: start_run starts the runner when none is alive.');
    expect(ensure.cli.argv).toEqual(['rcfd-runner', 'ensure', '--project', PROJECT]);
    expect(commandLine(['rcfd-runner', 'start', 'cases/a b.toml', '--project', 'C:\\x'])).toBe("rcfd-runner start 'cases/a b.toml' --project 'C:\\x'");
  });

  test('parameters: shapes only', () => {
    const q = getRunAction('project.queue_case')!;
    expect(runParamsProblem(q, { case: 'cases/a.toml', threads: 0 })).toBe('threads must be at least 1');
    expect(runParamsProblem(q, { case: 'cases/a.toml', threads: 300 })).toBe('threads must be at most 256');
    expect(runParamsProblem(q, { case: 'cases/a.toml', set: 'x=1' })).toBe('set must be a list of strings');
    expect(runParamsProblem(getRunAction('project.pause_run')!, {})).toBe("missing parameter 'run': The run id (r-<yyyymmddThhmmss>-<4 hex>), as the run list shows it.");
    expect(runParamsProblem(getRunAction('project.start_runner')!, { run: 'x' })).toBe("unknown parameter 'run' (this action takes none)");
  });
});

test.describe('availability and confirmation (G18, G19)', () => {
  const at = (state: string, extra: Partial<{ step: number; pid: number | null }> = {}): RunActionContext => ({ project: true, run: { id: 'r-1', state, step: 0, pid: null, ...extra } });
  const ok = (id: string, ctx: RunActionContext, p: Record<string, unknown> = { run: 'r-1' }) => runAvailability(getRunAction(id)!, ctx, p).available;

  test('which control applies in which runner state; an unknown run is the engine’s to answer', () => {
    const running = at('running', { step: 40, pid: 123 });
    const heldBefore = at('paused');
    const heldAfter = at('paused', { step: 40, pid: 123 });
    const table: [string, RunActionContext, Record<string, boolean>][] = [
      ['queued', at('queued'), { pause: true, resume: false, stop: false, cancel: true }],
      ['running', running, { pause: true, resume: false, stop: true, cancel: false }],
      ['paused before launch', heldBefore, { pause: false, resume: true, stop: false, cancel: true }],
      ['paused after launch', heldAfter, { pause: false, resume: true, stop: true, cancel: false }],
      ['finished', at('finished', { step: 9 }), { pause: false, resume: false, stop: false, cancel: false }],
      ['failed', at('failed'), { pause: false, resume: false, stop: false, cancel: false }],
      ['cancelled', at('cancelled'), { pause: false, resume: false, stop: false, cancel: false }],
      ['unknown', { project: true, run: null }, { pause: true, resume: true, stop: true, cancel: true }],
    ];
    for (const [name, ctx, want] of table) {
      for (const [k, v] of Object.entries(want)) expect(ok(`project.${k}_run`, ctx), `${k} when ${name}`).toBe(v);
    }
    expect(launched({ id: 'x', state: 'paused', step: 0, pid: 7 })).toBe(true);
    expect(launched({ id: 'x', state: 'queued' })).toBe(false);
  });

  test('every reason names the problem and the way out', () => {
    const reason = (id: string, ctx: RunActionContext, p: Record<string, unknown> = { run: 'r-1' }) => {
      const a = runAvailability(getRunAction(id)!, ctx, p);
      return a.available ? null : a.reason;
    };
    expect(reason('project.stop_run', at('finished'))).toBe('The run has already finished: there is nothing to stop. Run its case again for a new run.');
    expect(reason('project.pause_run', at('failed'))).toBe('The run has already failed: there is nothing to pause. Run its case again for a new run.');
    expect(reason('project.stop_run', at('queued'))).toBe('The run has not started yet: there is nothing to stop. Cancel takes it out of the queue.');
    expect(reason('project.cancel_run', at('running', { pid: 3 }))).toBe('The run has started: Stop ends it and keeps its results so far.');
    expect(reason('project.resume_run', at('running'))).toBe('The run is running, not paused: only a paused run can be resumed.');
    expect(reason('project.pause_run', at('paused'))).toBe('The run is already paused: Resume continues it.');
    expect(reason('project.run_case', { project: false }, { case: 'cases/a.toml' })).toBe('No project is open: open a project folder first (Open project folder…).');
    expect(reason('project.run_case', { project: true, cases: ['cases/b.toml'] }, { case: 'cases/a.toml' })).toBe(
      'cases/a.toml is not a case file of this project (cases/): it may have been moved or deleted. Choose one of the cases the project lists.',
    );
    expect(reason('project.queue_case', { project: true }, { case: 'cases/a.toml', set: ['solver.max_iterations'] })).toBe(
      '“solver.max_iterations” is not a setting for the run: write each one as path=value (solver.max_iterations=500).',
    );
    expect(reason('project.start_runner', { project: true }, {})).toBeNull();
  });

  test('Stop and Cancel ask first, in words that name what is lost; the others do not', () => {
    const stop = runConfirmation(getRunAction('project.stop_run')!, at('running', { step: 812, pid: 4 }), { run: 'r-1' })!;
    expect(stop.title).toBe('Stop run r-1?');
    expect(stop.body).toContain('(step 812 now)');
    expect(stop.body).toContain('its results so far stay in runs/r-1/');
    expect(stop.body).toContain('What is lost: the steps it has not computed yet.');
    expect(stop.body).toContain('cannot be resumed');
    expect(stop.body).toContain('killed');
    expect([stop.confirm, stop.keep]).toEqual(['Stop the run', 'Keep it running']);
    const cancel = runConfirmation(getRunAction('project.cancel_run')!, at('queued'), { run: 'r-2' })!;
    expect(cancel.body).toContain('It has not started, so no result is lost. What is lost: its place in the queue.');
    expect(cancel.body).toContain('runs/r-2/ stays');
    expect([cancel.confirm, cancel.keep]).toEqual(['Cancel the run', 'Keep it queued']);
    expect(RUN_ACTIONS.filter((a) => a.confirm).map((a) => a.id)).toEqual(['project.stop_run', 'project.cancel_run']);
  });

  test('engine refusals in words: each names the problem with the engine’s message, and the way out', () => {
    const a = (id: string) => getRunAction(id)!;
    const ended = engineFailure(a('project.stop_run'), { run: 'r-1' }, new RpcError(1002, 'run r-1 is already finished'));
    expect(ended).toEqual({
      code: 'bad_state',
      problem: 'The engine refused to stop run r-1: run r-1 is already finished',
      recovery: 'The run ended before the request reached the runner; the run list shows how it ended, and its results are kept. To run its case again, use Run.',
      engine: { code: 1002, message: 'run r-1 is already finished' },
    });
    const gone = engineFailure(a('project.run_case'), { case: 'cases/x.toml' }, new RpcError(1202, 'cases/x.toml: no such case file'));
    expect(gone.code).toBe('not_found');
    expect(gone.recovery).toContain('The case file was moved or deleted outside the Studio');
    const noRunner = engineFailure(a('project.run_case'), { case: 'cases/x.toml' }, new RpcError(-32000, 'run r-9 is queued, but the run service did not start: exit 2', { run: 'r-9' }));
    expect(noRunner.code).toBe('runner');
    expect(noRunner.recovery).toContain('Run r-9 is queued and waits for a runner');
    expect(engineFailure(a('project.pause_run'), { run: 'r-1' }, new RpcError('timeout', 'no answer within 30 s')).code).toBe('disconnected');
    expect(engineFailure(a('project.run_case'), { case: 'cases/x.toml' }, new RpcError(1003, 'mesh.nx: must be at least 1')).code).toBe('bad_request');
    const start = engineFailure(a('project.pause_run'), { run: 'r-1' }, new StartFailure('engine', 'ended at start (exit code 2)', 'boom'));
    expect(start.problem).toContain('for run control could not be started');
    expect(start.recovery).toContain('Nothing was sent to the runner');
    for (const f of [ended, gone, noRunner, start]) {
      expect(f.problem.length).toBeGreaterThan(20);
      expect(f.recovery.length).toBeGreaterThan(20);
    }
  });
});

// ---- the main process: run control and the session, with a fake engine

interface World {
  runs: { id: string; case: string; state: string; step: number }[];
  calls: { server: 'read' | 'control'; tool: string; args: Record<string, unknown> | undefined }[];
  feed: { n: number; source: string; kind: string; text: string; run?: string }[];
  /** What the control server answers instead of acting (an RpcError), once. */
  refuse: RpcError | null;
  /** Whether a runner acts on requests (the fake runner below). */
  runnerAlive: boolean;
}

class Fake implements ProjectServerLike {
  readonly info = { name: 'rcfd', version: '0.0.0-test' };
  exited = false;
  constructor(
    private readonly w: World,
    private readonly kind: 'read' | 'control',
  ) {}
  async call(tool: string, args?: Record<string, unknown>): Promise<unknown> {
    const w = this.w;
    w.calls.push({ server: this.kind, tool, args });
    if (this.kind === 'read') {
      switch (tool as ProjectTool) {
        case 'project_info':
          return { name: 'fake', counts: {}, issues: [], issues_total: 0 };
        case 'list_runs':
          return { runs: w.runs.map((r) => ({ ...r, created: '2026-11-16T10:00:00Z', time: 0, metrics: {} })), total: w.runs.length, next_offset: null, skipped: [] };
        case 'list_meshes':
          return { meshes: [], total: 0, next_offset: null, unread: [] };
        case 'run_status': {
          const r = w.runs.find((x) => x.id === args?.['run']);
          if (!r) throw new RpcError(1202, 'no such run');
          return { run: r.id, status: { state: r.state, step: r.step, time: 0, pid: r.state === 'running' ? 42 : null, updated: 't' }, events: [] };
        }
        case 'read_feed': {
          const cursor = w.feed.length ? w.feed[w.feed.length - 1]!.n : 0;
          if (args?.['since'] === undefined) return { cursor, entries: [] };
          const since = Number(args['since']);
          const entries = w.feed.filter((e) => e.n > since);
          return { cursor, entries, next: entries.length ? entries[entries.length - 1]!.n : null, dropped: 0 };
        }
      }
      throw new Error(`the read server was asked ${tool}`);
    }
    if (w.refuse) {
      const e = w.refuse;
      w.refuse = null;
      throw e;
    }
    switch (tool) {
      case 'start_run': {
        const id = `r-20261116T1200${String(w.runs.length).padStart(2, '0')}-abcd`;
        w.runs.push({ id, case: String(args?.['case']), state: 'queued', step: 0 });
        return { run: id, case: args?.['case'], state: 'queued' };
      }
      case 'pause_run':
      case 'resume_run':
      case 'cancel_run': {
        const r = w.runs.find((x) => x.id === args?.['run'])!;
        return { run: r.id, requested: tool.replace('_run', ''), state: r.state };
      }
    }
    throw new Error(`the control server was asked ${tool}`);
  }
  onExit(): void {}
  async close(): Promise<void> {
    this.exited = true;
  }
}

/** The runner of the fake world: acts on a request after `ms`, and reports it in the feed. */
function act(w: World, id: string, state: string, ms = 30): void {
  setTimeout(() => {
    if (!w.runnerAlive) return;
    const r = w.runs.find((x) => x.id === id)!;
    r.state = state;
    w.feed.push({ n: w.feed.length + 1, source: 'run', kind: 'state', text: `${id} ${state}`, run: id });
  }, ms);
}

const newWorld = (): World => ({
  runs: [
    { id: 'r-a', case: 'cases/a.toml', state: 'running', step: 40 },
    { id: 'r-b', case: 'cases/a.toml', state: 'finished', step: 90 },
  ],
  calls: [],
  feed: [],
  refuse: null,
  runnerAlive: true,
});

async function open(w: World, extra: Partial<ControlDeps> = {}) {
  const ensured: string[] = [];
  const control: ControlDeps = {
    startControlServer: async () => new Fake(w, 'control'),
    ensureRunner: async (f) => {
      ensured.push(f);
      w.runnerAlive = true;
      return { ok: true };
    },
    now: () => new Date().toISOString(),
    ...extra,
  };
  const s = await ProjectSession.open(
    PROJECT,
    '/e/rcfd-mcp',
    { startServer: async () => new Fake(w, 'read'), listFiles: async (_r, top) => (top === 'cases' ? { names: ['cases/a.toml'], total: 1 } : { names: [], total: 0 }), now: () => new Date().toISOString(), pollMs: 400, control },
    () => undefined,
  );
  return { s, ensured };
}

test.describe('run control in the main process', () => {
  test('each action: the request is in the view at once with the engine’s answer, then the runner’s change settles it', async () => {
    const w = newWorld();
    const { s } = await open(w);
    try {
      // the reading server never gets a run-control tool; the control server gets only those
      const start = await s.dispatch('project.run_case', { case: 'cases/a.toml' });
      expect(start.ok).toBe(true);
      const req = s.view.runRequests![0]!;
      expect(req).toMatchObject({ seq: 1, action: 'project.run_case', outcome: 'accepted', state: 'queued', case: 'cases/a.toml', failure: null });
      expect(req.mcp).toEqual({ tool: 'start_run', arguments: { case: 'cases/a.toml', store: 'plan' } });
      expect(req.cli.argv).toEqual(['rcfd-runner', 'start', 'cases/a.toml', '--project', PROJECT]);
      expect(req.answeredAt).toBeGreaterThan(Date.now() - 5000);
      const id = req.run!;
      await until(async () => (s.view.runRequests![0]!.settled ? true : null), 5000, 'the new run listed');

      act(w, 'r-a', 'paused');
      expect((await s.dispatch('project.pause_run', { run: 'r-a' })).ok).toBe(true);
      expect(s.view.runRequests![0]).toMatchObject({ action: 'project.pause_run', run: 'r-a', state: 'running', settled: false });
      await until(async () => (s.view.runRequests![0]!.settled ? true : null), 5000, 'paused');

      act(w, 'r-a', 'running');
      expect((await s.dispatch('project.resume_run', { run: 'r-a' })).ok).toBe(true);
      await until(async () => (s.view.runRequests![0]!.settled ? true : null), 5000, 'resumed');

      act(w, 'r-a', 'cancelled');
      expect((await s.dispatch('project.stop_run', { run: 'r-a' })).ok).toBe(true);
      await until(async () => (s.view.runRequests![0]!.settled ? true : null), 5000, 'stopped');

      act(w, id, 'cancelled');
      expect((await s.dispatch('project.cancel_run', { run: id })).ok).toBe(true);
      await until(async () => (s.view.runRequests![0]!.settled ? true : null), 5000, 'cancelled');

      expect(w.calls.filter((c) => c.server === 'control').map((c) => c.tool)).toEqual(['start_run', 'pause_run', 'resume_run', 'cancel_run', 'cancel_run']);
      expect(w.calls.filter((c) => c.server === 'read').every((c) => !(RUN_CONTROL_TOOLS as readonly string[]).includes(c.tool))).toBe(true);
      expect(s.view.runRequests!.map((r) => r.seq)).toEqual([5, 4, 3, 2, 1]);
    } finally {
      await s.close();
    }
  });

  test('refusals: before the engine (unknown, malformed, unavailable) and by the engine; only checked actions are kept', async () => {
    const w = newWorld();
    const { s } = await open(w);
    try {
      const unknown = await s.dispatch('project.freeze_run', { run: 'r-a' });
      expect(unknown).toMatchObject({ ok: false, request: null, failure: { code: 'unknown_action' } });
      const bad = await s.dispatch('project.queue_case', { case: 'cases/a.toml', threads: 0 });
      expect(bad).toMatchObject({ ok: false, request: null, failure: { code: 'bad_request', problem: 'Queue the case: threads must be at least 1.' } });
      const ended = await s.dispatch('project.stop_run', { run: 'r-b' });
      expect(ended.ok).toBe(false);
      if (!ended.ok) {
        expect(ended.failure).toEqual({
          code: 'unavailable',
          problem: 'Stop the project run: not available for run r-b.',
          recovery: 'The run has already finished: there is nothing to stop. Run its case again for a new run.',
          engine: null,
        });
        expect(ended.request).toMatchObject({ outcome: 'refused', settled: true, state: 'finished' });
      }
      const notCase = await s.dispatch('project.run_case', { case: 'cases/zz.toml' });
      expect(notCase.ok ? null : notCase.failure.code).toBe('unavailable');
      w.refuse = new RpcError(1002, 'run r-a is already finished');
      const race = await s.dispatch('project.pause_run', { run: 'r-a' });
      expect(race.ok ? null : race.failure).toMatchObject({ code: 'bad_state', engine: { code: 1002 } });
      // nothing reached the control server but the one the engine refused
      expect(w.calls.filter((c) => c.server === 'control').map((c) => c.tool)).toEqual(['pause_run']);
      expect(s.view.runRequests!.map((r) => r.outcome)).toEqual(['refused', 'refused', 'refused']);
    } finally {
      await s.close();
    }
  });

  test('no runner: a request the runner does not act on turns slow after RUN_SLOW_MS; Start the runner settles it', async () => {
    test.setTimeout(20_000);
    const w = newWorld();
    w.runnerAlive = false;
    const { s, ensured } = await open(w);
    try {
      act(w, 'r-a', 'paused');
      await s.dispatch('project.pause_run', { run: 'r-a' });
      await until(async () => (s.view.runRequests![0]!.slow ? true : null), RUN_SLOW_MS + 2000, 'slow');
      expect(s.view.runRequests![0]!.settled).toBe(false);
      const ensure = await s.dispatch('project.start_runner', {});
      expect(ensure.ok).toBe(true);
      expect(ensured).toEqual([PROJECT]);
      expect(s.view.runRequests![0]).toMatchObject({ action: 'project.start_runner', settled: true });
      act(w, 'r-a', 'paused');
      await until(async () => (s.view.runRequests![1]!.settled ? true : null), 5000, 'the pause acted on');
      expect(s.view.runRequests![1]!.slow).toBe(false);
    } finally {
      await s.close();
    }
  });

  test('a runner that does not start, and a control server that does not start, are refusals in words', async () => {
    const w = newWorld();
    const { s } = await open(w, {
      ensureRunner: async () => ({ ok: false, message: 'rcfd-runner: max_runs, cpu_threads and poll_ms must be positive' }),
      startControlServer: async () => {
        throw new StartFailure('engine', "The engine's project server ended at start (exit code 2).", 'error: no rcfd-project.json');
      },
    });
    try {
      const r = await s.dispatch('project.start_runner', {});
      expect(r.ok ? null : r.failure).toMatchObject({ code: 'runner', problem: 'The runner of the project did not start: rcfd-runner: max_runs, cpu_threads and poll_ms must be positive' });
      const c = await s.dispatch('project.pause_run', { run: 'r-a' });
      expect(c.ok ? null : c.failure.problem).toContain('for run control could not be started');
    } finally {
      await s.close();
    }
  });

  test('the control server is started once, on the first action, not read-only; a session without run control refuses', async () => {
    const w = newWorld();
    let started = 0;
    const control = new RunControl(PROJECT, {
      startControlServer: async () => (started++, new Fake(w, 'control')),
      ensureRunner: async () => ({ ok: true }),
      now: () => 't',
    });
    expect(control.serverRunning).toBe(false);
    const view = { runs: { items: [], total: 0, error: null }, runDetails: {}, cases: { items: [], total: 0, error: null } } as unknown as ProjectView;
    await control.dispatch('project.pause_run', { run: 'r-a' }, view);
    await control.dispatch('project.resume_run', { run: 'r-a' }, view);
    expect(started).toBe(1);
    await control.close();
    const server = readFileSync(path.join(appRoot, 'src', 'main', 'project', 'server.ts'), 'utf8');
    expect(server).toContain("options.readOnly === false ? ['--project', folder] : ['--project', folder, '--read-only']");
    const home = readFileSync(path.join(appRoot, 'src', 'main', 'project', 'home.ts'), 'utf8');
    expect(home).toContain('{ readOnly: false }');
  });
});

test.describe('the run list’s view of requests', () => {
  const req = (over: Partial<RunRequest>): RunRequest => ({
    seq: 1,
    action: 'project.pause_run',
    params: { run: 'r-a' },
    run: 'r-a',
    case: null,
    at: '2026-11-16T12:00:00Z',
    answeredAt: 1,
    ms: 3,
    outcome: 'accepted',
    state: 'running',
    settled: false,
    slow: false,
    failure: null,
    mcp: null,
    cli: { argv: [], cwd: PROJECT },
    ...over,
  });
  const entry = (id: string, state = 'running'): RunEntry => ({ id, case: 'cases/a.toml', created: 't', state, step: 1, time: 0, metrics: {} });

  test('a run started here shows from start_run’s answer until the list has it', () => {
    const items = [entry('r-a')];
    const started = req({ seq: 2, action: 'project.run_case', run: 'r-new', case: 'cases/b.toml', state: 'queued' });
    expect(withStartedRuns(items, [started]).map((r) => [r.id, r.case, r.state, r.step])).toEqual([
      ['r-a', 'cases/a.toml', 'running', 1],
      ['r-new', 'cases/b.toml', 'queued', 0],
    ]);
    expect(withStartedRuns([...items, entry('r-new', 'running')], [started])).toHaveLength(2);
    expect(withStartedRuns(items, [{ ...started, outcome: 'refused' }])).toBe(items);
    expect(withStartedRuns(items, [req({})])).toBe(items);
  });

  test('settle: by the run list, and slow after RUN_SLOW_MS', () => {
    const view = (state: string) => ({ runs: { items: [entry('r-a', state)], total: 1, error: null }, runDetails: {} }) as unknown as ProjectView;
    const pause = req({ answeredAt: 1000 });
    expect(settle([pause], view('running'), 1000 + RUN_SLOW_MS - 1)[0]).toBe(pause);
    expect(settle([pause], view('running'), 1000 + RUN_SLOW_MS)[0]).toMatchObject({ settled: false, slow: true });
    expect(settle([pause], view('paused'), 1000 + RUN_SLOW_MS)[0]).toMatchObject({ settled: true, slow: false });
    expect(settle([req({ action: 'project.resume_run', state: 'paused' })], view('running'), 0)[0]!.settled).toBe(true);
    expect(settle([req({ action: 'project.stop_run' })], view('paused'), 0)[0]!.settled).toBe(false);
    expect(settle([req({ action: 'project.cancel_run' })], view('cancelled'), 0)[0]!.settled).toBe(true);
    expect(settle([req({ action: 'project.run_case', run: 'r-new' })], view('running'), 0)[0]!.settled).toBe(false);
    const same = [req({ settled: true })];
    expect(settle(same, view('running'), 1e9)).toBe(same);
  });

  test('the words of a request: what was asked, when, and what it waits for', () => {
    expect(requestWords(req({}))).toMatch(/^Pause requested from the Studio at \d\d:\d\d:\d\d; waiting for the runner to hold it\.$/);
    expect(requestWords(req({ slow: true }))).toContain('the runner has not acted on it after 3 s. The project’s runner may not be running');
    expect(requestWords(req({ settled: true }))).toContain('the runner has acted on it.');
    // ADR 0737: what the engine's store plan applies to a run the Studio started; a case with its own store says nothing
    const start = { action: 'project.run_case', run: 'r-new', settled: true };
    expect(requestWords(req({ ...start, store: { applied: true, snapshots: 100, estimatedBytes: 6553600, note: null } }))).toMatch(
      /^Started from the Studio at \d\d:\d\d:\d\d, its fields stored by the engine's plan \(about 100 snapshots, 6\.3 MB\); the runner has acted on it\.$/,
    );
    expect(requestWords(req({ ...start, store: { applied: false, snapshots: null, estimatedBytes: null, note: 'the case sets output.store_interval' } }))).not.toContain('stored');
    expect(requestWords(req({ outcome: 'refused', failure: { code: 'bad_state', problem: 'The engine refused to pause run r-a: run r-a is already finished', recovery: 'x', engine: null } }))).toBe(
      'Pause refused: The engine refused to pause run r-a: run r-a is already finished',
    );
    expect(pendingByRun([req({ seq: 3, settled: true }), req({ seq: 2 }), req({ seq: 1 })]).get('r-a')!.seq).toBe(2);
    expect(setEntries(' a=1 \n\n b=2\r\n')).toEqual(['a=1', 'b=2']);
  });
});

test.describe('the port and the renderer', () => {
  test('the project port takes exactly an action id and plain JSON params', () => {
    expect(parseProjectRequest({ op: 'action', id: 1, action: 'project.pause_run', params: { run: 'r-a' } })).toEqual({
      ok: true,
      request: { op: 'action', id: 1, action: 'project.pause_run', params: { run: 'r-a' } },
    });
    const refused = (m: unknown) => {
      const r = parseProjectRequest(m);
      return r.ok ? null : r.error.message;
    };
    expect(refused({ op: 'action', id: 1, action: 'project.pause_run' })).toBe('action: takes exactly an action and its params');
    expect(refused({ op: 'action', id: 1, action: 'Pause!', params: {} })).toContain('the action must be an action id');
    expect(refused({ op: 'action', id: 1, action: 'project.pause_run', params: [] })).toBe('action: params must be a plain object');
    expect(refused({ op: 'action', id: 1, action: 'project.pause_run', params: { run: Number.NaN } })).toBe('action: params: not a finite number');
    expect(refused({ op: 'action', id: 1, action: 'project.pause_run', params: { run: 'x'.repeat(70_000) } })).toContain('larger than');
  });

  test('only project/door.ts names window.studioRuns; run controls dispatch named actions through the door', () => {
    const renderer = path.join(appRoot, 'src', 'renderer');
    const walk = (d: string): string[] =>
      readdirSync(d, { withFileTypes: true }).flatMap((e) =>
        e.isDirectory() ? walk(path.join(d, e.name)) : /\.(ts|tsx)$/.test(e.name) && !e.name.endsWith('.d.ts') ? [path.join(d, e.name)] : [],
      );
    const code = (t: string) => t.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');
    const rel = (f: string) => path.relative(renderer, f).split(path.sep).join('/');
    const files = walk(renderer);
    expect(files.filter((f) => /studioRuns/.test(code(readFileSync(f, 'utf8')))).map(rel)).toEqual(['project/door.ts']);
    const controls = code(readFileSync(path.join(renderer, 'runs', 'RunControls.tsx'), 'utf8'));
    const ids = new Set([...controls.matchAll(/'(project\.[a-z_]+)'/g)].map((m) => m[1]!));
    expect([...ids].sort()).toEqual(RUN_ACTIONS.map((a) => a.id).sort());
    for (const id of ids) expect(getRunAction(id), id).toBeDefined();
  });

  test('runs/control.css uses only defined tokens and no colour literals; corners stay square; no shadow', () => {
    const css = readFileSync(path.join(appRoot, 'src', 'renderer', 'runs', 'control.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    const defined = new Set(cssCustomProperties().map(([k]) => k));
    const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
    expect(used.filter((u) => !defined.has(u))).toEqual([]);
    expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
    expect(css).not.toMatch(/border-radius|box-shadow|opacity/);
  });
});
