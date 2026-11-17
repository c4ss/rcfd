// The run-control actions of an open project (F-807; docs/studio-guardrails.md G2, G7, G16, G18, G19): quick run,
// queue, pause, resume, stop, cancel and start the runner. They are named actions like the service actions of
// registry.ts (F-827), with a second kind of engine entry point: not a method of the solver service but a tool of the
// engine's project server (`rcfd-mcp --project <folder>`, the project-mode tools of docs/mcp-tools.md that Claude Code
// uses) or a command of the project's runner (`rcfd-runner`). The runner owns every run (G7); these actions only ask it.
//
// Each action names its engine entry point, its MCP equivalent (the same tool with the same arguments) and its CLI
// equivalent (the `rcfd-runner` command that does the same from the project folder), a predicate that says when it is
// unavailable and why in words with the way out (G19), and, for the actions that lose something, the words of the
// confirmation (G18). tests/unit/run-control.spec.ts checks every action against the engine's sources and its inventory
// (G16); tests/e2e/run-control.spec.ts runs each one against the real engine.
//
// They live beside the service registry rather than in ACTIONS: ACTIONS is the one action per service method of the
// allow-list (tests/unit/actions-registry.spec.ts), and these call no service method. RUN_ACTIONS is the list a command
// palette (F-831) adds to ACTIONS.
//
// Shared by the renderer, the main process and the tests: no Node or DOM types.
import { paramsProblem, toJsonSchema, type JsonObject, type JsonValue, type ParamsOf, type ParamsSpec } from './params.js';
import type { ProjectView } from '../project.js';

/** The tools of the engine's project server that run control calls; all of them have the access class `execute`. */
export const RUN_CONTROL_TOOLS = ['start_run', 'pause_run', 'resume_run', 'cancel_run'] as const;
export type RunControlTool = (typeof RUN_CONTROL_TOOLS)[number];

/** The engine entry point of a run-control action. */
export type RunEngine =
  /** A tool of `rcfd-mcp --project <folder>` (not read-only), called with the action's arguments. */
  | { readonly kind: 'mcp'; readonly tool: RunControlTool }
  /**
   * A command of the project's runner, `rcfd-runner <command> --project <folder>`. F-848: `start` is the remote start
   * of a run on a site (src/shared/sites.ts), the one start the project server's start_run does not give the Studio.
   */
  | { readonly kind: 'runner'; readonly command: 'ensure' | 'start' };

/** What the caller knows of the run an action is about (the run list's answer); anything left out counts as unknown. */
export interface RunFacts {
  readonly id: string;
  /** The runner's state word (status.json). */
  readonly state: string;
  readonly step?: number;
  /** The solver's process id when the runner recorded one (status.json `pid`); null when none. */
  readonly pid?: number | null;
}

export interface RunActionContext {
  /** Whether a project is open (false: every action is unavailable). Unknown counts as open. */
  readonly project?: boolean;
  /** The run the action is about, as last read. */
  readonly run?: RunFacts | null;
  /** The case files the project lists (`cases/...`); a case not among them is unavailable. */
  readonly cases?: readonly string[];
}

/** The words of a confirmation for an action that loses something (G18): what is lost, named. */
export interface Confirmation {
  readonly title: string;
  readonly body: string;
  /** The label of the button that goes ahead (it names the action). */
  readonly confirm: string;
  /** The label of the button that keeps things as they are. */
  readonly keep: string;
}

/** How the action shows in the run list once the engine accepted it: what the runner is expected to do next. */
export type RunExpect = 'new_run' | 'paused' | 'resumed' | 'ended' | 'runner';

/** A CLI equivalent: the argv (program first) and the folder it is run from. */
export interface CliEquivalent {
  readonly argv: readonly string[];
  /** The folder the command is run from: the project folder (a case path is relative to it). */
  readonly cwd: string;
}

export interface RunActionSpec<P extends ParamsSpec> {
  readonly title: string;
  readonly description: string;
  readonly group: 'run';
  readonly params: P;
  readonly engine: RunEngine;
  /** The MCP tool's arguments for these parameters (default: the parameters as they are). */
  mcpArgs?(p: ParamsOf<P>): JsonObject;
  /** What the MCP equivalent does differently, or why there is none. */
  readonly mcpNote?: string;
  /** The `rcfd-runner` command that does the same; `project` is the project folder. */
  cli(p: ParamsOf<P>, project: string): readonly string[];
  /** What the CLI command does differently, when anything does. */
  readonly cliNote?: string;
  unavailable?(ctx: RunActionContext, p: Partial<ParamsOf<P>>): string | null;
  confirm?(ctx: RunActionContext, p: ParamsOf<P>): Confirmation;
  readonly expect: RunExpect;
}

export interface RegisteredRunAction extends RunActionSpec<ParamsSpec> {
  readonly id: RunActionId;
  readonly effect: 'change';
}

function action<const P extends ParamsSpec>(spec: RunActionSpec<P>): RunActionSpec<P> {
  return spec;
}

/** The runner's states after which nothing changes (rcfd-project's RunState::is_terminal). */
const TERMINAL: ReadonlySet<string> = new Set(['finished', 'failed', 'cancelled']);
export const isTerminalState = (s: string): boolean => TERMINAL.has(s);

/**
 * Whether the runner has launched the run's solver: a running run, or a paused one that has a solver process or a step
 * (a run paused while queued is held before launch and has neither).
 */
export function launched(run: RunFacts): boolean {
  if (run.state === 'running') return true;
  if (run.state !== 'paused') return false;
  return (run.pid ?? null) !== null || (run.step ?? 0) > 0;
}

const run = { type: 'string', description: 'The run id (r-<yyyymmddThhmmss>-<4 hex>), as the run list shows it.' } as const;
const ON_RUN = { run } as const;
const caseParam = { type: 'string', description: 'The case file, project-relative (cases/cavity.toml).' } as const;

const NO_PROJECT = 'No project is open: open a project folder first (Open project folder…).';

/** "has already finished": the ended run's state in words, and what to do instead. */
const ended = (state: string, what: string): string =>
  `The run has already ${state === 'finished' ? 'finished' : state === 'failed' ? 'failed' : 'been cancelled'}: there is nothing to ${what}. Run its case again for a new run.`;

const caseProblem = (ctx: RunActionContext, c: string | undefined): string | null => {
  if (ctx.project === false) return NO_PROJECT;
  if (c !== undefined && ctx.cases !== undefined && !ctx.cases.includes(c)) {
    return `${c} is not a case file of this project (cases/): it may have been moved or deleted. Choose one of the cases the project lists.`;
  }
  return null;
};

const runner = (...args: string[]) => ['rcfd-runner', ...args];

const SPECS = {
  'project.run_case': action({
    title: 'Run the case',
    description:
      "Start a run of a project case now (a quick run): the run is queued with the project's runner, which is started if none is alive, and launched as soon as its slots allow.",
    group: 'run',
    params: { case: caseParam },
    engine: { kind: 'mcp', tool: 'start_run' },
    // ADR 0737 (the maintainer's report, 2026-11-17: a transient case without [output] ran and stored no field, a dead
    // end in the Studio): the Studio's Run is start_run's store plan (ADR 0140, about 100 f32 snapshots), unless the case
    // or a set entry gives the store. The case file is not changed; start_run's own default (none) and the CLI's stay.
    mcpArgs: (p) => ({ case: p.case, store: 'plan' }),
    cli: (p, project) => runner('start', p.case, '--project', project),
    cliNote:
      'rcfd-runner start writes no line in the project journal; start_run does (who: start_run). rcfd-runner start has no store plan: the settings the plan added are in start_run’s answer (store.set) and in the run’s run.json, to pass as --set.',
    unavailable: (ctx, p) => caseProblem(ctx, p.case),
    expect: 'new_run',
  }),
  'project.queue_case': action({
    title: 'Queue the case',
    description:
      'Add a run of a project case to the runner’s queue with a priority, threads, a memory budget or per-run settings; the runner launches queued runs by priority (highest first), then in the order they were queued.',
    group: 'run',
    params: {
      case: caseParam,
      priority: { type: 'integer', optional: true, description: 'Higher runs first among the queued runs (default 0).' },
      threads: { type: 'integer', optional: true, minimum: 1, maximum: 256, description: 'Solver threads of the run (default 1).' },
      mem_mb: { type: 'integer', optional: true, minimum: 1, description: 'Memory the runner books for the run, MB (default: its estimate).' },
      set: { type: 'string-array', optional: true, description: 'Settings for this run only, each path=value (rcfd run --set), in order.' },
      // F-837: the egui Studio's "Re-run with fields stored"; start_run's own store plan (ADR 0140), not the Studio's
      store: {
        type: 'string',
        optional: true,
        enum: ['none', 'plan'],
        description: 'plan (default, ADR 0737): the engine’s start plan stores the run’s fields (about 100 snapshots) unless the case or set gives the store; none: only what the case asks for.',
      },
    },
    engine: { kind: 'mcp', tool: 'start_run' },
    mcpArgs: (p) => {
      const out: Record<string, JsonValue> = { case: p.case, store: p.store ?? 'plan' };
      if (p.priority !== undefined) out['priority'] = p.priority;
      if (p.threads !== undefined) out['threads'] = p.threads;
      if (p.mem_mb !== undefined) out['mem_mb'] = p.mem_mb;
      if (p.set !== undefined && p.set.length > 0) out['set'] = [...p.set];
      return out;
    },
    cli: (p, project) =>
      runner(
        'start',
        p.case,
        ...(p.priority !== undefined ? ['--priority', String(p.priority)] : []),
        ...(p.threads !== undefined ? ['--threads', String(p.threads)] : []),
        ...(p.mem_mb !== undefined ? ['--mem-mb', String(p.mem_mb)] : []),
        ...(p.set ?? []).flatMap((s) => ['--set', s]),
        '--project',
        project,
      ),
    cliNote:
      'rcfd-runner start writes no line in the project journal; start_run does (who: start_run). rcfd-runner start has no store plan: with store plan, the settings the plan added are in start_run’s answer (store.set) and in the run’s run.json, to pass as --set.',
    unavailable: (ctx, p) => {
      const c = caseProblem(ctx, p.case);
      if (c) return c;
      const bad = (p.set ?? []).find((s) => !s.includes('='));
      return bad !== undefined ? `“${bad}” is not a setting for the run: write each one as path=value (solver.max_iterations=500).` : null;
    },
    expect: 'new_run',
  }),
  // F-828 (ADR 0734 P3): the run again as it ran, from its own case snapshot, through the engine
  'project.rerun_run': action({
    title: 'Rerun the project run',
    description:
      'Start a new run of a run’s case as it was when the run was made (runs/<id>/case.snapshot) with the run’s recorded settings, so a case file changed since does not matter; the new run’s run.json names the run it reruns (rerun_of).',
    group: 'run',
    params: ON_RUN,
    engine: { kind: 'mcp', tool: 'start_run' },
    mcpArgs: (p) => ({ from_run: p.run }),
    cli: (p, project) => runner('rerun', p.run, '--project', project),
    cliNote: 'rcfd-runner rerun writes no line in the project journal; start_run does (who: start_run, with rerun_of).',
    unavailable: (ctx) => (ctx.project === false ? NO_PROJECT : null),
    expect: 'new_run',
  }),
  'project.pause_run': action({
    title: 'Pause the project run',
    description:
      'Hold a run: a running solver stops between two steps and waits, keeping everything it computed; a queued run is not launched until it is resumed.',
    group: 'run',
    params: ON_RUN,
    engine: { kind: 'mcp', tool: 'pause_run' },
    cli: (p, project) => runner('pause', p.run, '--project', project),
    cliNote: 'rcfd-runner pause writes no line in the project journal; pause_run does.',
    unavailable: (ctx) => {
      if (ctx.project === false) return NO_PROJECT;
      const r = ctx.run;
      if (!r) return null;
      if (isTerminalState(r.state)) return ended(r.state, 'pause');
      if (r.state === 'paused') return 'The run is already paused: Resume continues it.';
      return null;
    },
    expect: 'paused',
  }),
  'project.resume_run': action({
    title: 'Resume the project run',
    description: 'Release a paused run: a held solver goes on from the step where it waited; a run held before launch goes back to the queue.',
    group: 'run',
    params: ON_RUN,
    engine: { kind: 'mcp', tool: 'resume_run' },
    cli: (p, project) => runner('resume', p.run, '--project', project),
    cliNote: 'rcfd-runner resume writes no line in the project journal; resume_run does.',
    unavailable: (ctx) => {
      if (ctx.project === false) return NO_PROJECT;
      const r = ctx.run;
      if (!r) return null;
      if (isTerminalState(r.state)) return ended(r.state, 'resume');
      if (r.state !== 'paused') return `The run is ${r.state}, not paused: only a paused run can be resumed.`;
      return null;
    },
    expect: 'resumed',
  }),
  'project.stop_run': action({
    title: 'Stop the project run',
    description:
      'End a run that has started: the solver stops cleanly after its current step and writes its results so far; the run ends as cancelled and cannot be resumed.',
    group: 'run',
    params: ON_RUN,
    engine: { kind: 'mcp', tool: 'cancel_run' },
    cli: (p, project) => runner('cancel', p.run, '--project', project),
    cliNote: 'rcfd-runner cancel writes no line in the project journal; cancel_run does.',
    unavailable: (ctx) => {
      if (ctx.project === false) return NO_PROJECT;
      const r = ctx.run;
      if (!r) return null;
      if (isTerminalState(r.state)) return ended(r.state, 'stop');
      if (!launched(r)) return 'The run has not started yet: there is nothing to stop. Cancel takes it out of the queue.';
      return null;
    },
    confirm: (ctx, p) => {
      const step = ctx.run?.step;
      return {
        title: `Stop run ${p.run}?`,
        body:
          `The solver stops after its current step${step !== undefined ? ` (step ${step} now)` : ''} and keeps what it has computed: its results so far stay in runs/${p.run}/. ` +
          'What is lost: the steps it has not computed yet. The run ends as cancelled and cannot be resumed; to go on, run the case again. ' +
          'A solver that does not stop within the runner’s grace time is killed, and what it had not written by then is lost too.',
        confirm: 'Stop the run',
        keep: 'Keep it running',
      };
    },
    expect: 'ended',
  }),
  'project.cancel_run': action({
    title: 'Cancel the queued run',
    description: 'Take a run that has not started out of the queue for good: it ends as cancelled without a step; its folder stays.',
    group: 'run',
    params: ON_RUN,
    engine: { kind: 'mcp', tool: 'cancel_run' },
    cli: (p, project) => runner('cancel', p.run, '--project', project),
    cliNote: 'rcfd-runner cancel writes no line in the project journal; cancel_run does.',
    unavailable: (ctx) => {
      if (ctx.project === false) return NO_PROJECT;
      const r = ctx.run;
      if (!r) return null;
      if (isTerminalState(r.state)) return ended(r.state, 'cancel');
      if (launched(r)) return 'The run has started: Stop ends it and keeps its results so far.';
      return null;
    },
    confirm: (_ctx, p) => ({
      title: `Cancel queued run ${p.run}?`,
      body:
        'It has not started, so no result is lost. What is lost: its place in the queue. It ends as cancelled and will never be launched; ' +
        `its folder runs/${p.run}/ stays. To run the case later, run it again.`,
      confirm: 'Cancel the run',
      keep: 'Keep it queued',
    }),
    expect: 'ended',
  }),
  'project.start_runner': action({
    title: 'Start the runner',
    description:
      "Start the project's runner, detached, unless one is alive. The runner launches queued runs and acts on pause, resume and cancel requests. A runner the Studio started closes with it: at once when it holds no run, after asking when it does (the Studio setting or --keep-runner keeps it going).",
    group: 'run',
    params: {},
    engine: { kind: 'runner', command: 'ensure' },
    mcpNote: 'No MCP tool: start_run starts the runner when none is alive.',
    cli: (_p, project) => runner('ensure', '--project', project),
    unavailable: (ctx) => (ctx.project === false ? NO_PROJECT : null),
    expect: 'runner',
  }),
} satisfies Record<string, RunActionSpec<ParamsSpec>>;

export type RunActionId = keyof typeof SPECS;
export type RunActionParams<Id extends RunActionId> = ParamsOf<(typeof SPECS)[Id]['params']>;

export const RUN_ACTIONS: readonly RegisteredRunAction[] = Object.entries(SPECS).map(([id, spec]) =>
  Object.freeze({ ...(spec as RunActionSpec<ParamsSpec>), id: id as RunActionId, effect: 'change' as const }),
);

const BY_ID = new Map(RUN_ACTIONS.map((a) => [a.id as string, a]));

export function getRunAction(id: string): RegisteredRunAction | undefined {
  return BY_ID.get(id);
}

export function isRunActionId(id: string): id is RunActionId {
  return BY_ID.has(id);
}

/** Null when `params` fits the action's schema. */
export function runParamsProblem(a: RegisteredRunAction, params: unknown): string | null {
  return paramsProblem(a.params, params);
}

export type RunAvailability = { readonly available: true } | { readonly available: false; readonly reason: string };

/** Whether the action can be dispatched in this context, and why not (in words, with the way out). */
export function runAvailability(a: RegisteredRunAction, ctx: RunActionContext = {}, params: Record<string, unknown> = {}): RunAvailability {
  const reason = a.unavailable?.(ctx, params as never) ?? null;
  return reason ? { available: false, reason } : { available: true };
}

/** The confirmation an action asks for before it is dispatched (G18), or null when it needs none. */
export function runConfirmation(a: RegisteredRunAction, ctx: RunActionContext, params: Record<string, unknown>): Confirmation | null {
  return a.confirm ? a.confirm(ctx, params as never) : null;
}

/** The equivalents of one call (G16, G17): the MCP tool call and the CLI command. */
export interface RunEquivalents {
  readonly mcp: { readonly tool: string; readonly arguments: JsonObject } | null;
  readonly mcpNote: string | null;
  readonly cli: CliEquivalent;
  readonly cliNote: string | null;
}

export function runEquivalents(a: RegisteredRunAction, params: JsonObject, project: string): RunEquivalents {
  const mcp =
    a.engine.kind === 'mcp' ? { tool: a.engine.tool, arguments: a.mcpArgs ? a.mcpArgs(params as never) : params } : null;
  return { mcp, mcpNote: a.mcpNote ?? null, cli: { argv: a.cli(params as never, project), cwd: project }, cliNote: a.cliNote ?? null };
}

/** A command line for display: each word quoted when it needs it (POSIX shell rules). */
export function commandLine(argv: readonly string[]): string {
  return argv.map((w) => (/^[A-Za-z0-9_./:=@+,-]+$/.test(w) ? w : `'${w.replaceAll("'", `'\\''`)}'`)).join(' ');
}

/** The palette and help view of a run action. */
export function describeRunAction(a: RegisteredRunAction) {
  return {
    id: a.id,
    title: a.title,
    description: a.description,
    group: a.group,
    effect: a.effect,
    params: toJsonSchema(a.params),
    engine: a.engine.kind === 'mcp' ? `mcp:${a.engine.tool}` : `cli:rcfd-runner ${a.engine.command}`,
    mcp: a.engine.kind === 'mcp' ? a.engine.tool : null,
    mcpNote: a.mcpNote ?? null,
    cli: `rcfd-runner ${a.cli({ case: '<case>', run: '<run>' } as never, '<project>').slice(1).join(' ')}`,
    confirms: a.confirm !== undefined,
  };
}

// ---- the context from the project view (main process and renderer alike)

/** The run an action is about, as the run list last answered it (the list entry is read after the detail it updates). */
export function runFacts(view: ProjectView, id: string): RunFacts | null {
  const entry = view.runs.items.find((r) => r.id === id);
  const detail = view.runDetails[id];
  if (!entry && !detail) return null;
  return {
    id,
    state: entry?.state ?? detail!.state,
    step: entry?.step ?? detail!.step,
    pid: detail && detail.state === (entry?.state ?? detail.state) ? detail.pid : null,
  };
}

/** What the availability predicates know of the open project. */
export function contextOf(view: ProjectView, params: Readonly<Record<string, unknown>>): RunActionContext {
  const id = typeof params['run'] === 'string' ? params['run'] : null;
  return {
    project: true,
    run: id ? runFacts(view, id) : null,
    ...(view.cases.error === null && view.cases.total <= view.cases.items.length ? { cases: view.cases.items.map((c) => c.path) } : {}),
  };
}

