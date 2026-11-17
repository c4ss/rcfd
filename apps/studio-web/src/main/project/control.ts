// Run control of one open project (F-807): the actions of src/shared/actions/runs.ts, made against the engine. The
// project-mode tools (start_run, pause_run, resume_run, cancel_run) go to a second project server for the folder,
// `rcfd-mcp --project <folder>` without --read-only, started on the first action and kept while the project is open;
// the reading server of F-805 stays read-only. Starting the runner is `rcfd-runner ensure --project <folder>`, the
// engine's CLI (no MCP tool does it alone). The runner owns the runs (G7): nothing here holds a run.
//
// Every action is checked against its schema and against what the run list says (availability), then sent; the reply,
// or the refusal in words that name the problem and the way out (G19), becomes a RunRequest the page shows in the run
// list. Whether the runner has acted on a request is read from the run list as the feed reports it (settle).
//
// No Electron import: the unit tests drive it with a fake server.
import { spawn } from 'node:child_process';
import {
  contextOf,
  getRunAction,
  runFacts,
  isTerminalState,
  runAvailability,
  runEquivalents,
  runParamsProblem,
  type RegisteredRunAction,
} from '../../shared/actions/runs.js';
import { getFirstRunAction } from '../../shared/actions/first.js';
import type { JsonObject } from '../../shared/actions/params.js';
import { RUN_SLOW_MS, type ProjectView, type RunActionFailure, type RunActionResult, type RunRequest } from '../../shared/project.js';
import { RpcError } from '../service/rpc.js';
import { StartFailure, type ProjectServerLike } from './server.js';

export interface ControlDeps {
  /** Starts the folder's run-control server: rcfd-mcp --project <folder>, not read-only. */
  startControlServer(folder: string): Promise<ProjectServerLike>;
  /** `rcfd-runner ensure --project <folder>`: resolves when the runner holds the project, or with the reason it does not. */
  ensureRunner(folder: string): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }>;
  now(): string;
  /** ms since the epoch, fractional (default performance.timeOrigin + performance.now()). */
  clock?(): number;
}

const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));
type Obj = Record<string, unknown>;
const obj = (v: unknown): Obj => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Obj) : {});
const strOrNull = (v: unknown): string | null => (typeof v === 'string' ? v : null);

export const preciseNow = (): number => performance.timeOrigin + performance.now();

/** A run-control action of F-807, or of the first path (F-813, src/shared/actions/first.ts). */
const lookup = (id: string): RegisteredRunAction | undefined => getRunAction(id) ?? getFirstRunAction(id);

/** What the action is about, for messages: "run r-...", "cases/cavity.toml", "the project". */
const target = (p: Readonly<Record<string, unknown>>): string =>
  typeof p['run'] === 'string' ? `run ${p['run']}` : typeof p['case'] === 'string' ? (p['case'] as string) : 'the project';

const VERB: Record<string, string> = {
  'project.run_case': 'start a run of',
  'project.queue_case': 'queue',
  'project.pause_run': 'pause',
  'project.resume_run': 'resume',
  'project.stop_run': 'stop',
  'project.cancel_run': 'cancel',
  'project.start_runner': 'start the runner of',
};

/** An engine refusal or failure in words: the problem (with the engine's own message) and the way out. */
export function engineFailure(a: RegisteredRunAction, params: Readonly<Record<string, unknown>>, e: unknown): RunActionFailure {
  const verb = VERB[a.id] ?? a.title.toLowerCase();
  const what = target(params);
  if (e instanceof StartFailure) {
    return {
      code: 'engine',
      problem: `The engine's project server for run control could not be started, so the Studio could not ${verb} ${what}: ${e.message}${e.detail ? ` (${e.detail})` : ''}`,
      recovery:
        'Nothing was sent to the runner. Check that rcfd-mcp, rcfd-runner and rcfd are built side by side (cargo build --release -p rcfd-mcp -p rcfd-runner -p rcfd-cli), then try again.',
      engine: null,
    };
  }
  if (!(e instanceof RpcError)) {
    return {
      code: 'engine',
      problem: `The Studio could not ${verb} ${what}: ${message(e)}`,
      recovery: 'The run list shows the runner’s state as it is; try again, and if it fails again see .rcfd/runner.log in the project folder.',
      engine: null,
    };
  }
  const engine = { code: e.code, message: e.message };
  const problem = `The engine refused to ${verb} ${what}: ${e.message}`;
  const isCase = typeof params['case'] === 'string';
  switch (e.code) {
    case 1002:
      return {
        code: 'bad_state',
        problem,
        recovery: 'The run ended before the request reached the runner; the run list shows how it ended, and its results are kept. To run its case again, use Run.',
        engine,
      };
    case 1202:
      return {
        code: 'not_found',
        problem,
        recovery: isCase
          ? 'The case file was moved or deleted outside the Studio; the project home lists the cases that are there now. Choose one of them.'
          : 'The run folder was removed or renamed outside the Studio; the run list shows the runs that are there now.',
        engine,
      };
    case 1004:
      return { code: 'refused_path', problem, recovery: 'Choose a case file inside the project’s cases/ folder.', engine };
    case 1003:
    case -32602:
      return {
        code: 'bad_request',
        problem,
        recovery: 'Nothing was queued. Change what the engine names (the case, or a per-run setting written path=value) and try again.',
        engine,
      };
    case -32000: {
      const queued = strOrNull(obj(e.data)['run']);
      if (queued) {
        return {
          code: 'runner',
          problem,
          recovery: `Run ${queued} is queued and waits for a runner. The engine’s message ends with the runner’s own words (from .rcfd/runner.log): fix what they say, then Start the runner.`,
          engine,
        };
      }
      break;
    }
    case 'disconnected':
    case 'timeout':
      return {
        code: 'disconnected',
        problem: `The engine did not answer the request to ${verb} ${what}: ${e.message}`,
        recovery: 'The request may or may not have reached the runner: the run list shows its state as the runner writes it. If nothing changed, try again.',
        engine,
      };
  }
  return {
    code: 'engine',
    problem,
    recovery: 'The message is the engine’s. Try again; if it repeats, the project’s .rcfd/runner.log and the run’s log.txt say more.',
    engine,
  };
}

/**
 * Updates `settled` and `slow` of each request from the run list: a new run is settled when the list has it, a pause
 * when the run is paused, a resume when it is not paused any more, a stop or cancel when the run has ended. A request
 * the engine refused, and a started runner, are settled at once. Returns `requests` itself when nothing changed.
 */
export function settle(requests: readonly RunRequest[], view: ProjectView, nowMs: number): readonly RunRequest[] {
  let changed = false;
  const out = requests.map((r) => {
    if (r.settled) return r;
    const a = lookup(r.action);
    let done = r.outcome === 'refused' || !a || a.expect === 'runner';
    if (!done && r.run) {
      const f = runFacts(view, r.run);
      switch (a!.expect) {
        case 'new_run':
          done = f !== null;
          break;
        case 'paused':
          done = f !== null && (f.state === 'paused' || isTerminalState(f.state));
          break;
        case 'resumed':
          done = f !== null && f.state !== 'paused';
          break;
        case 'ended':
          done = f !== null && isTerminalState(f.state);
          break;
      }
    }
    const slow = !done && nowMs - r.answeredAt >= RUN_SLOW_MS;
    if (done === r.settled && slow === r.slow) return r;
    changed = true;
    return { ...r, settled: done, slow: done ? false : slow };
  });
  return changed ? out : requests;
}

/** The run-control half of an open project: the not-read-only server, and the actions made through it. */
export class RunControl {
  private server: ProjectServerLike | null = null;
  private starting: Promise<ProjectServerLike> | null = null;
  private seq = 0;
  private closed = false;
  private readonly clock: () => number;

  constructor(
    readonly folder: string,
    private readonly deps: ControlDeps,
  ) {
    this.clock = deps.clock ?? preciseNow;
  }

  /** The run-control server, started on first use and again after it stopped. */
  private async controlServer(): Promise<ProjectServerLike> {
    if (this.server && !this.server.exited) return this.server;
    if (!this.starting) {
      this.starting = this.deps.startControlServer(this.folder).finally(() => {
        this.starting = null;
      });
    }
    const s = await this.starting;
    if (this.closed) {
      await s.close();
      throw new Error('the project was closed');
    }
    this.server = s;
    return s;
  }

  /** Whether the run-control server is running (for tests and diagnostics). */
  get serverRunning(): boolean {
    return this.server !== null && !this.server.exited;
  }

  /**
   * One action: checked, sent, answered. Never rejects. A refusal before the engine (unknown action, wrong parameters)
   * has no request; every action that was checked against the run list has one, accepted or refused.
   */
  async dispatch(id: string, params: Readonly<Record<string, unknown>>, view: ProjectView): Promise<RunActionResult> {
    const a = lookup(id);
    if (!a) {
      return { ok: false, request: null, failure: { code: 'unknown_action', problem: `'${id}' is not a run-control action.`, recovery: 'Use one of the actions the Runs view offers.', engine: null } };
    }
    const bad = runParamsProblem(a, params);
    if (bad) {
      return { ok: false, request: null, failure: { code: 'bad_request', problem: `${a.title}: ${bad}.`, recovery: 'Nothing was sent; give the parameters the action describes.', engine: null } };
    }
    const p = JSON.parse(JSON.stringify(params)) as JsonObject;
    const eq = runEquivalents(a, p, this.folder);
    const seq = ++this.seq;
    const at = this.deps.now();
    const t0 = this.clock();
    const base = {
      seq,
      action: a.id,
      params: p,
      case: typeof p['case'] === 'string' ? (p['case'] as string) : null,
      at,
      mcp: eq.mcp,
      cli: eq.cli,
      slow: false,
    };
    const refused = (failure: RunActionFailure, run: string | null, state: string | null = null): RunActionResult => {
      const answeredAt = this.clock();
      const request: RunRequest = { ...base, run, answeredAt, ms: answeredAt - t0, outcome: 'refused', state, settled: true, failure };
      return { ok: false, failure, request };
    };
    const runId = typeof p['run'] === 'string' ? (p['run'] as string) : null;
    const av = runAvailability(a, contextOf(view, p), p);
    if (!av.available) {
      return refused({ code: 'unavailable', problem: `${a.title}: not available for ${target(p)}.`, recovery: av.reason, engine: null }, runId, runId ? (runFacts(view, runId)?.state ?? null) : null);
    }
    try {
      let result: Obj;
      if (a.engine.kind === 'mcp') {
        const server = await this.controlServer();
        result = obj(await server.call(a.engine.tool, eq.mcp!.arguments as Record<string, unknown>));
      } else {
        const r = await this.deps.ensureRunner(this.folder);
        if (!r.ok) {
          return refused(
            {
              code: 'runner',
              problem: `The runner of the project did not start: ${r.message}`,
              recovery: 'Fix what the runner says (its log is .rcfd/runner.log in the project folder), then Start the runner again.',
              engine: null,
            },
            null,
          );
        }
        result = { started: true };
      }
      const answeredAt = this.clock();
      const request: RunRequest = {
        ...base,
        run: strOrNull(result['run']) ?? runId,
        answeredAt,
        ms: answeredAt - t0,
        outcome: 'accepted',
        state: strOrNull(result['state']),
        settled: a.expect === 'runner',
        failure: null,
      };
      return { ok: true, request, result };
    } catch (e) {
      const failure = engineFailure(a, p, e);
      const queued = e instanceof RpcError ? strOrNull(obj(e.data)['run']) : null;
      return refused(failure, queued ?? runId);
    }
  }

  async close(): Promise<void> {
    this.closed = true;
    const s = this.server;
    this.server = null;
    if (s) await s.close();
  }
}

/**
 * `rcfd-runner ensure --project <folder>`, as the engine's CLI does it. Its standard input and output are the null
 * device; its standard error is read until it exits (the runner it starts inherits the handle on Windows, ADR 0146, so the
 * stream is closed here at the exit rather than at its end).
 */
export function ensureRunnerCli(exe: string, folder: string, timeoutMs = 20_000): Promise<{ ok: true } | { ok: false; message: string }> {
  return new Promise((resolve) => {
    let err = '';
    let done = false;
    const finish = (r: { ok: true } | { ok: false; message: string }) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      child.stderr?.destroy();
      resolve(r);
    };
    const child = spawn(exe, ['ensure', '--project', folder], { cwd: folder, stdio: ['ignore', 'ignore', 'pipe'], windowsHide: true });
    child.stderr!.on('data', (d: Buffer) => {
      err = (err + d.toString('utf8')).slice(-4096);
    });
    child.once('error', (e) => finish({ ok: false, message: `${exe} could not be started (${e.message}); build it with cargo build --release -p rcfd-runner.` }));
    child.once('exit', (code) =>
      finish(code === 0 ? { ok: true } : { ok: false, message: err.trim().split(/\r?\n/).slice(-4).join(' ') || `rcfd-runner ensure ended with exit code ${code}` }),
    );
    const timer = setTimeout(() => finish({ ok: false, message: `rcfd-runner ensure did not end within ${timeoutMs / 1000} s` }), timeoutMs);
  });
}
