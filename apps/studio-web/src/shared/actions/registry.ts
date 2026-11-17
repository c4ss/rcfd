// The Studio's action registry (F-827; docs/studio-guardrails.md G16, G18, G20, G2): every UI action is one entry here,
// with a stable id, a title and a description (for the command palette of F-831 and help on the spot, G19), a parameter
// schema, the engine call it makes, its equivalent from Claude Code (an MCP tool) and from the CLI (`rcfd call`), a
// predicate that says when it is not available and why, and an optional undo hook (F-829 builds undo on it).
//
// The engine call of every action today is a method of the service allow-list (SERVICE_METHODS, src/shared/service.ts):
// the renderer reaches the engine only through the service port (ADR 0801 addendum 2). The parameters of an action are
// the parameters of its method, unchanged, so the CLI equivalent is exact: `rcfd call --addr <service> <method> <json>`
// sends the same request the Studio sends. Where the MCP server (crates/rcfd-mcp) has a session tool that calls the same
// method, it is named, with the arguments it takes; where it has none, `mcp.tool` is null and the note says so.
//
// Availability (`available`): a hint for the palette and the controls, never a rule of its own. A predicate says
// "unavailable" only when the engine would certainly refuse the call or it would do nothing, and gives the engine's
// reason in words with the way out (G19); when the Studio does not know (no session state given), the action is
// available and the engine answers (G2). The run-state rules quoted below are those of crates/rcfd-api/src/service.rs
// (require_idle, run.pause, run.resume, Service::sim) and docs/service-protocol.md, "Sessions and run states".
//
// Shared by the renderer, the main process and the tests: no Node or DOM types.
import {
  SERVICE_CONTROL_METHODS,
  SERVICE_METHODS,
  type ServiceMethod,
  type ServiceReply,
  type ServiceState,
} from '../service.js';
import { paramsProblem, sessionParams, toJsonSchema, type JsonObject, type JsonValue, type ParamsOf, type ParamsSpec } from './params.js';

/** The run states of a session (`run.status`'s `state`). */
export type RunStateName = 'defined' | 'ready' | 'running' | 'paused' | 'finished' | 'stopped' | 'failed';

/** What the caller knows when it asks whether an action is available. Anything left out counts as unknown. */
export interface ActionContext {
  /** The service connection (window.studioService.state()); unknown when left out. */
  readonly service?: Pick<ServiceState, 'status' | 'lastError'> | null;
  /** The session the surface is on, and its run state when the surface knows it (from run.status or the events). */
  readonly session?: { readonly id: string; readonly state?: string } | null;
}

export type Availability = { readonly available: true } | { readonly available: false; readonly reason: string };

/** One action with its parameters: a log entry, a step of a script, an undo step. */
export interface ActionCall {
  readonly action: string;
  readonly params: JsonObject;
}

export type ActionGroup = 'service' | 'session' | 'settings' | 'mesh' | 'run' | 'results';

/** The Claude Code (MCP) equivalent of an action. */
export type McpSpec<P extends ParamsSpec> =
  | {
      /** A session tool of crates/rcfd-mcp that calls the same service method. */
      readonly tool: string;
      /** What differs from the Studio's call, when anything does. */
      readonly note?: string;
      /** The tool's arguments for these parameters; null when the tool cannot express them. Default: the same. */
      args?(p: ParamsOf<P>): JsonObject | null;
    }
  | { readonly tool: null; readonly note: string };

/** The undo hook F-829 uses: the action that reverses a dispatched one. */
export interface UndoSpec<P extends ParamsSpec> {
  /**
   * A read action dispatched just before this one, whose reply the inverse needs (the old value of a setting). It goes
   * through the dispatcher like any action but is not written to the action log.
   */
  before?(p: ParamsOf<P>): ActionCall | null;
  /**
   * The actions that undo this one, in order, or null when it cannot be undone this way (then F-829 confirms in words,
   * G18).
   */
  inverse(p: ParamsOf<P>, result: unknown, before: ServiceReply | null): readonly ActionCall[] | null;
  /** What the inverse does not restore, in words. */
  readonly note?: string;
}

export interface ActionSpec<P extends ParamsSpec> {
  /** A short imperative label for the palette and the controls. */
  readonly title: string;
  /** One or two sentences: what it does, in the engine's terms (G19). */
  readonly description: string;
  readonly group: ActionGroup;
  readonly params: P;
  /** The engine call: a method of SERVICE_METHODS, sent the action's parameters as they are. */
  readonly engine: { readonly service: ServiceMethod; readonly timeoutMs?: number };
  readonly mcp: McpSpec<P>;
  /** A check across parameters that the schema cannot state (for example "at most one of"), shapes only. */
  check?(p: ParamsOf<P>): string | null;
  /** The action makes a session; its id is the result's `session` (a replay maps it to the new id). */
  readonly produces?: 'session';
  /** Why the action is unavailable in this context, or null (see the header). */
  unavailable?(ctx: ActionContext, p: Partial<ParamsOf<P>>): string | null;
  readonly undo?: UndoSpec<P>;
}

/** An entry of the registry: a spec with its id and what follows from its method. */
export interface RegisteredAction extends ActionSpec<ParamsSpec> {
  readonly id: string;
  /** `read` for a method of SERVICE_READ_METHODS, `change` for one of SERVICE_CONTROL_METHODS. */
  readonly effect: 'read' | 'change';
}

/** Keeps the literal type of a spec's parameters, so ACTIONS gives each action its parameter type. */
function action<const P extends ParamsSpec>(spec: ActionSpec<P>): ActionSpec<P> {
  return spec;
}

const session = { type: 'session', description: 'The session (run.status, session.list).' } as const;
const ON_SESSION = { session } as const;

const IN_PROGRESS = 'A run is in progress in this session; stop it first.';
const NO_SIMULATION = 'The session has no built simulation yet; initialize or start the run first.';

/** Running or paused: what the service's require_idle refuses. */
const busy = (ctx: ActionContext): boolean => ctx.session?.state === 'running' || ctx.session?.state === 'paused';

const SPECS = {
  // ---- the service
  'service.info': action({
    title: 'Show service information',
    description: "The connected rcfd service's name, version, protocol and method list.",
    group: 'service',
    params: {},
    engine: { service: 'service.info' },
    mcp: { tool: null, note: 'The MCP server reports its own version in its initialize reply; it has no tool for this.' },
  }),
  'settings.schema': action({
    title: 'Show the case schema',
    description: 'The JSON Schema of a case and of the monitor format, from the engine.',
    group: 'service',
    params: {},
    engine: { service: 'settings.schema' },
    mcp: { tool: null, note: 'No MCP tool; `rcfd call settings.schema` prints it.' },
  }),
  'data.info': action({
    title: 'Show the data route',
    description: "The service's clock and bulk-data port, when its data methods are on (protocol 1.2); otherwise the service answers that the method is unknown.",
    group: 'service',
    params: {},
    engine: { service: 'data.info' },
    mcp: { tool: null, note: 'No MCP tool: the data methods are for an embedding client.' },
  }),

  // ---- sessions
  'session.list': action({
    title: 'List sessions',
    description: 'Every session the service holds, with its run state.',
    group: 'session',
    params: {},
    engine: { service: 'session.list' },
    mcp: { tool: null, note: 'No MCP tool: the MCP server keeps track of the sessions it made itself.' },
  }),
  'session.create': action({
    title: 'New session',
    description: 'Create a simulation session from case text (TOML), a settings object or a case file; with none of them, from the defaults.',
    group: 'session',
    params: {
      toml: { type: 'string', optional: true, description: 'The case file text (TOML).' },
      settings: { type: 'object', optional: true, description: 'The case settings as JSON.' },
      path: { type: 'string', optional: true, description: 'A .toml or .json case file inside the service sandbox.' },
      base_dir: {
        type: 'string',
        optional: true,
        description: 'With toml or settings: the folder that relative data paths of the case resolve against (sandboxed).',
      },
    },
    engine: { service: 'session.create' },
    mcp: {
      tool: 'create_case',
      note: 'create_case also takes a template name and makes the session its default; it has no base_dir.',
      args: (p) => (p.base_dir === undefined ? stripUndefined({ toml: p.toml, settings: p.settings, path: p.path }) : null),
    },
    check: (p) => {
      const given = (['toml', 'settings', 'path'] as const).filter((k) => p[k] !== undefined);
      if (given.length > 1) return `give at most one of toml, settings and path (got ${given.join(', ')})`;
      if (p.base_dir !== undefined && p.path !== undefined) return 'base_dir goes with toml or settings; a case file resolves against its own folder';
      return null;
    },
    produces: 'session',
    undo: {
      inverse: (_p, result) => {
        const id = sessionOf(result);
        return id ? [{ action: 'session.close', params: { session: id } }] : null;
      },
    },
  }),
  'session.close': action({
    title: 'Close session',
    description: 'Stop any run of the session and remove it from the service. Its settings and results in the service are lost.',
    group: 'session',
    params: ON_SESSION,
    engine: { service: 'session.close' },
    mcp: { tool: null, note: 'No MCP tool: MCP sessions end with the server.' },
    undo: {
      inverse: () => null,
      note: 'Closing cannot be undone: the session journal (journal.get) can make a new session with the same settings, not the same results.',
    },
  }),

  // ---- settings
  'settings.get': action({
    title: 'Read a setting',
    description: 'One setting by dotted path (for example solver.alpha_u), or the whole settings tree without a path.',
    group: 'settings',
    params: { session, path: { type: 'string', optional: true, description: 'Dotted setting path; leave out for the whole tree.' } },
    engine: { service: 'settings.get' },
    mcp: { tool: 'get_setting' },
  }),
  'settings.set': action({
    title: 'Change a setting',
    description: 'Set one setting by dotted path. Named entries (boundaries.<patch>, reports.<name>) are made on demand. The built simulation is discarded and the session returns to defined, except turbulence settings of a ready or paused run, which apply live.',
    group: 'settings',
    params: {
      session,
      path: { type: 'string', description: 'Dotted setting path, for example solver.alpha_u or boundaries.lid.velocity_value.' },
      value: { type: 'json', description: 'The new value (any JSON value the schema allows at that path).' },
    },
    engine: { service: 'settings.set' },
    mcp: { tool: 'set_setting' },
    unavailable: (ctx) => (ctx.session?.state === 'running' ? 'The run is going; stop it before changing settings.' : null),
    undo: {
      before: (p) => ({ action: 'settings.get', params: { session: p.session, path: p.path } }),
      inverse: (p, _result, before) =>
        before?.ok && isObject(before.result) && 'value' in before.result
          ? [{ action: 'settings.set', params: { session: p.session, path: p.path, value: before.result['value'] as JsonValue } }]
          : null,
      note: 'Restores the old value; a setting that did not exist before (a new named entry) has no inverse here (F-829).',
    },
  }),
  'settings.remove': action({
    title: 'Remove a named entry',
    description: 'Delete a named entry such as boundaries.lid or reports.drag. The session returns to defined.',
    group: 'settings',
    params: { session, path: { type: 'string', description: "The entry's path: '<collection>.<name>'." } },
    engine: { service: 'settings.remove' },
    mcp: { tool: null, note: 'No MCP tool in session mode; in project mode edit_case changes the case file.' },
    unavailable: (ctx) => (busy(ctx) ? IN_PROGRESS : null),
    undo: {
      before: (p) => ({ action: 'settings.get', params: { session: p.session, path: p.path } }),
      inverse: (p, _result, before) =>
        before?.ok && isObject(before.result) && isObject(before.result['value'])
          ? leaves(p.path, before.result['value'] as JsonObject).map(([path, value]) => ({ action: 'settings.set', params: { session: p.session, path, value } }))
          : null,
      note: 'Sets each value the entry had again, one setting at a time (the service makes a named entry from its first setting).',
    },
  }),
  'settings.validate': action({
    title: 'Check the case',
    description: "Check the session's settings against the engine's rules and build the mesh; lists every problem with its setting path.",
    group: 'settings',
    params: ON_SESSION,
    engine: { service: 'settings.validate', timeoutMs: 600_000 },
    mcp: { tool: null, note: 'No session tool; check_case_headless (run.check) checks a case the same way and also builds it.' },
  }),

  // ---- mesh
  'mesh.check': action({
    title: 'Check the mesh',
    description: 'Build the mesh and report its quality (non-orthogonality, skewness, aspect ratio, errors, warnings) and its patches.',
    group: 'mesh',
    params: ON_SESSION,
    engine: { service: 'mesh.check', timeoutMs: 600_000 },
    mcp: { tool: 'check_mesh' },
  }),

  // ---- run control
  'run.initialize': action({
    title: 'Initialize the run',
    description: 'Build the simulation (mesh, fields, solver) without taking a step; the session becomes ready.',
    group: 'run',
    params: ON_SESSION,
    engine: { service: 'run.initialize', timeoutMs: 600_000 },
    mcp: { tool: null, note: 'No MCP tool; run and run_workflow_task initialize as needed.' },
    unavailable: (ctx) => (busy(ctx) ? IN_PROGRESS : null),
  }),
  'run.start': action({
    title: 'Start the run',
    description: 'Start the solver in the background (building the simulation first when needed); a finished, stopped or failed run starts again from its initial state.',
    group: 'run',
    params: { session, max_steps: { type: 'integer', optional: true, minimum: 1, description: 'Stop after this many steps (the service caps it).' } },
    engine: { service: 'run.start' },
    mcp: {
      tool: 'run',
      note: 'run waits up to wait_seconds after run.start; with wait_seconds 0 it returns at once, as the Studio does.',
      args: (p) => stripUndefined({ session: p.session, max_steps: p.max_steps, wait_seconds: 0 }),
    },
    unavailable: (ctx) => (busy(ctx) ? IN_PROGRESS : null),
  }),
  'run.pause': action({
    title: 'Pause the run',
    description: 'Hold the running solver between two steps.',
    group: 'run',
    params: ON_SESSION,
    engine: { service: 'run.pause' },
    mcp: { tool: null, note: 'No session tool; in project mode pause_run pauses a project run.' },
    unavailable: (ctx) => (ctx.session?.state !== undefined && ctx.session.state !== 'running' ? 'The session is not running.' : null),
  }),
  'run.resume': action({
    title: 'Resume the run',
    description: 'Continue a paused run.',
    group: 'run',
    params: ON_SESSION,
    engine: { service: 'run.resume' },
    mcp: { tool: null, note: 'No session tool; in project mode resume_run resumes a project run.' },
    unavailable: (ctx) => (ctx.session?.state !== undefined && ctx.session.state !== 'paused' ? 'The session is not paused.' : null),
  }),
  'run.stop': action({
    title: 'Stop the run',
    description: 'Stop the solver; the results so far stay in the session.',
    group: 'run',
    params: ON_SESSION,
    engine: { service: 'run.stop' },
    mcp: { tool: null, note: 'No session tool; in project mode cancel_run stops a project run.' },
    unavailable: (ctx) => (ctx.session?.state !== undefined && !busy(ctx) ? 'No run is in progress in this session.' : null),
  }),
  'run.step': action({
    title: 'Take steps',
    description: 'Take a number of solver steps now and return (building the simulation first when needed).',
    group: 'run',
    params: { session, count: { type: 'integer', optional: true, minimum: 1, description: 'How many steps (default 1; the service caps it).' } },
    engine: { service: 'run.step', timeoutMs: 600_000 },
    mcp: { tool: null, note: 'No MCP tool; run with max_steps runs that many steps in the background.' },
    unavailable: (ctx) => (busy(ctx) ? IN_PROGRESS : null),
  }),
  'run.status': action({
    title: 'Show run status',
    description: 'Run state, iteration, time and the latest residuals of the session.',
    group: 'run',
    params: ON_SESSION,
    engine: { service: 'run.status' },
    mcp: { tool: 'status', note: 'status adds the report values (reports.get).' },
  }),

  // ---- results
  'monitors.get': action({
    title: 'Read monitors',
    description: 'Monitor histories (residuals and report values) as x/y series; names and since select what is returned.',
    group: 'results',
    params: {
      session,
      names: { type: 'string-array', optional: true, description: 'Only these series.' },
      since: { type: 'number', optional: true, description: 'Only samples after this x value.' },
    },
    engine: { service: 'monitors.get' },
    mcp: { tool: 'get_residuals' },
    unavailable: (ctx) => (ctx.session?.state === 'defined' ? NO_SIMULATION : null),
  }),
  'reports.get': action({
    title: 'Read report values',
    description: 'The current value and unit of every report of the case (forces, averages, ...).',
    group: 'results',
    params: ON_SESSION,
    engine: { service: 'reports.get' },
    mcp: { tool: 'status', note: 'status returns the report values together with the run state.', args: (p) => ({ session: p.session }) },
    unavailable: (ctx) => (ctx.session?.state === 'defined' ? NO_SIMULATION : null),
  }),
  'field.stats': action({
    title: 'Field statistics',
    description: 'Minimum, maximum and mean of a field (ux, uy, uz, p, speed, ...).',
    group: 'results',
    params: { session, name: { type: 'string', description: 'The field name.' } },
    engine: { service: 'field.stats' },
    mcp: { tool: 'get_field_stats' },
    unavailable: (ctx) => (ctx.session?.state === 'defined' ? NO_SIMULATION : null),
  }),
  'log.tail': action({
    title: 'Show the solver log',
    description: "The last lines of the session's solver log.",
    group: 'results',
    params: { session, lines: { type: 'integer', optional: true, minimum: 0, description: 'How many lines (default 50).' } },
    engine: { service: 'log.tail' },
    mcp: { tool: null, note: 'No MCP tool; `rcfd call log.tail` prints it.' },
  }),
  'panel.snapshot': action({
    title: 'Refresh the run panel',
    description: 'Status, new monitor samples, report values, the log tail and optionally a coarse field grid, in one call.',
    group: 'results',
    params: {
      session,
      since: { type: 'number', optional: true, description: 'Only monitor samples after this x value.' },
      field: { type: 'string', optional: true, description: 'A field to average onto a coarse grid.' },
      cols: { type: 'integer', optional: true, minimum: 1, description: 'Grid columns.' },
      rows: { type: 'integer', optional: true, minimum: 1, description: 'Grid rows.' },
      log_lines: { type: 'integer', optional: true, minimum: 0, description: 'Log lines (default 8).' },
    },
    engine: { service: 'panel.snapshot' },
    mcp: { tool: null, note: 'No MCP tool; status, get_residuals and the log methods give the same parts.' },
  }),
  'journal.get': action({
    title: 'Show the session journal',
    description: 'How the session was made and every accepted settings change, with the hash of the resulting case.',
    group: 'results',
    params: ON_SESSION,
    engine: { service: 'journal.get' },
    mcp: { tool: null, note: 'No session tool; in project mode read_feed lists the project journal.' },
  }),
} satisfies Record<string, ActionSpec<ParamsSpec>>;

/** The id of a registered action. */
export type ActionId = keyof typeof SPECS;
/** The parameter type of an action. */
export type ActionParams<Id extends ActionId> = ParamsOf<(typeof SPECS)[Id]['params']>;

const CONTROL = new Set<string>(SERVICE_CONTROL_METHODS);

/** Every registered action, in registry order. */
export const ACTIONS: readonly RegisteredAction[] = Object.entries(SPECS).map(([id, spec]) => {
  const s = spec as ActionSpec<ParamsSpec>;
  return Object.freeze({ ...s, id, effect: CONTROL.has(s.engine.service) ? 'change' : 'read' });
});

const BY_ID = new Map(ACTIONS.map((a) => [a.id, a]));

export function getAction(id: string): RegisteredAction | undefined {
  return BY_ID.get(id);
}

export function isActionId(id: string): id is ActionId {
  return BY_ID.has(id);
}

// ---- what follows from a spec

/** Null when `params` fits the action: its schema, then its own check. */
export function actionParamsProblem(a: RegisteredAction, params: unknown): string | null {
  return paramsProblem(a.params, params) ?? (a.check ? a.check(params as never) : null);
}

/** Whether the action can be dispatched in this context, and why not (see the header). */
export function availability(a: RegisteredAction, ctx: ActionContext = {}, params: Record<string, unknown> = {}): Availability {
  const service = ctx.service;
  if (service && service.status !== 'connected') {
    const reason =
      service.status === 'off'
        ? 'No rcfd service is configured: start the Studio with RCFD_STUDIO_SERVICE (attach) or RCFD_STUDIO_RCFD (start one).'
        : `The rcfd service is not connected (${service.status}${service.lastError ? `: ${service.lastError}` : ''}).`;
    return { available: false, reason };
  }
  const needsSession = sessionParams(a.params).filter((k) => !a.params[k]!.optional);
  if (needsSession.length && needsSession.some((k) => params[k] === undefined) && !ctx.session) {
    return { available: false, reason: 'No session is open; create or choose a session first.' };
  }
  // the context's run state is about its own session: for another session the state is unknown
  const other = typeof params['session'] === 'string' && ctx.session && ctx.session.id !== params['session'];
  const own = other ? { ...ctx, session: { id: params['session'] as string } } : ctx;
  const reason = a.unavailable?.(own, params as never) ?? null;
  return reason ? { available: false, reason } : { available: true };
}

/** The address `rcfd call` uses when it is given none. */
export const DEFAULT_CLI_ADDRESS = '127.0.0.1:7878';

/** The equivalents of one call (G16, G17): the service request, the CLI command and the MCP tool. */
export interface Equivalents {
  readonly service: { readonly method: ServiceMethod; readonly params: JsonObject };
  /** argv of the CLI command, `rcfd` first. */
  readonly cli: readonly string[];
  readonly mcp: { readonly tool: string; readonly arguments: JsonObject } | null;
  /** Why there is no MCP tool, or what differs. */
  readonly mcpNote: string | null;
}

export function equivalents(a: RegisteredAction, params: JsonObject, address: string = DEFAULT_CLI_ADDRESS): Equivalents {
  const method = a.engine.service;
  const cli = ['rcfd', 'call', '--addr', address, method, JSON.stringify(params)];
  if (a.mcp.tool === null) return { service: { method, params }, cli, mcp: null, mcpNote: a.mcp.note };
  const args = a.mcp.args ? a.mcp.args(params as never) : params;
  return {
    service: { method, params },
    cli,
    mcp: args ? { tool: a.mcp.tool, arguments: args } : null,
    mcpNote: args ? (a.mcp.note ?? null) : `${a.mcp.tool} cannot express these parameters. ${a.mcp.note ?? ''}`.trim(),
  };
}

/** The palette and help view of an action: everything but the functions. */
export function describeAction(a: RegisteredAction) {
  return {
    id: a.id,
    title: a.title,
    description: a.description,
    group: a.group,
    effect: a.effect,
    params: toJsonSchema(a.params),
    service: a.engine.service,
    cli: `rcfd call ${a.engine.service} <json-params>`,
    mcp: a.mcp.tool,
    mcpNote: a.mcp.note ?? null,
    undoable: a.undo !== undefined,
    undoNote: a.undo?.note ?? null,
  };
}

// ---- helpers

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** The settings under `path` as [dotted path, value] pairs, objects opened down to their non-object values. */
function leaves(path: string, v: JsonObject): [string, JsonValue][] {
  return Object.entries(v).flatMap(([k, x]): [string, JsonValue][] =>
    isObject(x) && Object.keys(x).length ? leaves(`${path}.${k}`, x as JsonObject) : [[`${path}.${k}`, x]],
  );
}

function stripUndefined(o: Record<string, JsonValue | undefined>): JsonObject {
  const out: Record<string, JsonValue> = {};
  for (const [k, v] of Object.entries(o)) if (v !== undefined) out[k] = v;
  return out;
}

/** The `session` of a result, when it has one. */
export function sessionOf(result: unknown): string | null {
  return isObject(result) && typeof result['session'] === 'string' ? result['session'] : null;
}

/** Every method of the allow-list that no action calls (a test keeps it empty; F-824 reports it). */
export function methodsWithoutAction(): ServiceMethod[] {
  const used = new Set(ACTIONS.map((a) => a.engine.service));
  return SERVICE_METHODS.filter((m) => !used.has(m));
}
