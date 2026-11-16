// The action dispatcher (F-827, G16): the one path from the Studio's UI to the engine. A surface names an action of the
// registry with its parameters; the dispatcher checks the parameters against the action's schema and its availability,
// makes the engine call through a transport (in the renderer: window.studioService, src/renderer/actions/), writes the
// call to the action log, and gives back the engine's reply as a value. The log can be exported as a script (JSON, or
// a POSIX shell script of `rcfd call` commands) and a JSON script can be replayed through a dispatcher, so every action
// the Studio takes is recordable and scriptable.
//
// Transport-agnostic and free of Node and DOM types, so the unit tests drive it against the fake service and the real
// `rcfd serve`, and the main process could use it too.
import type { CallOptions, ServiceError, ServiceMethod, ServiceReply } from '../service.js';
import { sessionParams, type JsonObject, type JsonValue } from './params.js';
import {
  ACTIONS,
  actionParamsProblem,
  availability,
  DEFAULT_CLI_ADDRESS,
  equivalents,
  getAction,
  sessionOf,
  type ActionCall,
  type ActionContext,
  type ActionId,
  type ActionParams,
  type RegisteredAction,
} from './registry.js';

/** What the dispatcher needs from the service: window.studioService has it (ServiceBridge.call). */
export interface ActionTransport {
  call(method: ServiceMethod, params?: Record<string, unknown>, options?: CallOptions): Promise<ServiceReply>;
}

export type ActionErrorCode = ServiceError['code'] | 'unknown_action' | 'unavailable';

export interface ActionError {
  readonly code: ActionErrorCode;
  readonly message: string;
  readonly data?: unknown;
}

/** Who dispatched an action: the UI, a replayed script, or undo and redo (F-829). */
export type ActionOrigin = 'ui' | 'replay' | 'undo' | 'redo';

/** One dispatched action, as the log keeps it. Results are not kept (they can be large); the engine has them. */
export interface LogEntry {
  /** Increases by one with each dispatched action, reads and changes alike. */
  readonly seq: number;
  readonly action: string;
  readonly params: JsonObject;
  readonly effect: 'read' | 'change';
  readonly origin: ActionOrigin;
  /** When it was dispatched (ISO 8601). */
  readonly at: string;
  /** Milliseconds until the reply. */
  readonly ms: number;
  /** False when the dispatcher refused it (unknown action, bad parameters, unavailable): nothing reached the engine. */
  readonly sent: boolean;
  readonly ok: boolean;
  readonly error?: ActionError;
  /** The session the action made (an action with `produces: 'session'`). */
  readonly session?: string;
  /** The actions that undo this one, in order (actions with an undo hook, after success); null when there are none. */
  readonly inverse?: readonly ActionCall[] | null;
}

export type ActionReply<T = unknown> =
  | { readonly ok: true; readonly result: T; readonly entry: LogEntry }
  | { readonly ok: false; readonly error: ActionError; readonly entry: LogEntry };

export interface DispatchOptions {
  /** Time limit of the engine call (default: the action's, else the service port's default). */
  readonly timeoutMs?: number;
  readonly origin?: ActionOrigin;
  /** Merged over the dispatcher's own context (the service state) for the availability check. */
  readonly context?: ActionContext;
}

// ---- the log

/** Changes kept (oldest dropped first and counted); reads kept, in a ring of their own so polling never pushes out a change. */
export const MAX_LOGGED_CHANGES = 10_000;
export const MAX_LOGGED_READS = 500;

export class ActionLog {
  private changes: LogEntry[] = [];
  private reads: LogEntry[] = [];
  private listeners = new Set<(e: LogEntry) => void>();
  /** Entries dropped because the log was full. */
  droppedChanges = 0;
  droppedReads = 0;

  add(e: LogEntry): void {
    const list = e.effect === 'change' ? this.changes : this.reads;
    list.push(e);
    if (e.effect === 'change' && list.length > MAX_LOGGED_CHANGES) {
      list.shift();
      this.droppedChanges++;
    }
    if (e.effect === 'read' && list.length > MAX_LOGGED_READS) {
      list.shift();
      this.droppedReads++;
    }
    for (const l of this.listeners) l(e);
  }

  /** Every kept entry, oldest first. */
  entries(): LogEntry[] {
    return [...this.changes, ...this.reads].sort((a, b) => a.seq - b.seq);
  }

  /** Calls `listener` with every later entry; returns the function that stops it (an action log view, F-829). */
  onEntry(listener: (e: LogEntry) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  clear(): void {
    this.changes = [];
    this.reads = [];
    this.droppedChanges = 0;
    this.droppedReads = 0;
  }
}

// ---- scripts

export const SCRIPT_FORMAT = 'rcfd-studio-actions';
export const SCRIPT_VERSION = 1;

export interface ScriptStep {
  readonly action: string;
  readonly params: JsonObject;
  /** The session this step made when it was recorded; a replay maps it to the session its own run makes. */
  readonly session?: string;
  /** The same request from the CLI, as recorded (G16, G17). */
  readonly cli: readonly string[];
  /** The same from Claude Code, when an MCP tool has it. */
  readonly mcp: { readonly tool: string; readonly arguments: JsonObject } | null;
}

export interface ActionScript {
  readonly format: typeof SCRIPT_FORMAT;
  readonly version: typeof SCRIPT_VERSION;
  readonly exported: string;
  /** The service the actions went to, as the CLI commands name it. */
  readonly address: string;
  readonly steps: readonly ScriptStep[];
}

export interface ExportOptions {
  /** Include read actions (default false: a script that changes nothing does not need them). */
  readonly includeReads?: boolean;
  /** Include actions that failed or were refused (default false). */
  readonly includeFailed?: boolean;
  /** The service address for the CLI commands (default the address `rcfd call` uses). */
  readonly address?: string;
  /** The export's time stamp (default now). */
  readonly exported?: string;
}

/** A log as a JSON script that `replay` runs again. */
export function toScript(entries: readonly LogEntry[], options: ExportOptions = {}): ActionScript {
  const address = options.address ?? DEFAULT_CLI_ADDRESS;
  const steps: ScriptStep[] = [];
  for (const e of entries) {
    if (e.effect === 'read' && !options.includeReads) continue;
    if (!e.ok && !options.includeFailed) continue;
    const a = getAction(e.action);
    if (!a) continue;
    const eq = equivalents(a, e.params, address);
    steps.push({ action: e.action, params: e.params, ...(e.session ? { session: e.session } : {}), cli: eq.cli, mcp: eq.mcp });
  }
  return { format: SCRIPT_FORMAT, version: SCRIPT_VERSION, exported: options.exported ?? new Date().toISOString(), address, steps };
}

/** Single-quotes a word for a POSIX shell. */
export function shQuote(s: string): string {
  return `'${s.replaceAll("'", `'\\''`)}'`;
}

/**
 * A script as a POSIX shell script of `rcfd call` commands: the same requests in the same order against a running
 * `rcfd serve` (address in RCFD_ADDR, default the recorded one). A session a step made is kept in a shell variable
 * (S1, S2, ...) and the later steps use it, so the script works on a fresh service. Stops at the first failing call.
 */
export function toShell(script: ActionScript): string {
  const vars = new Map<string, string>();
  const lines = [
    '#!/bin/sh',
    `# rcfd Studio actions (${SCRIPT_FORMAT} ${SCRIPT_VERSION}), exported ${script.exported}.`,
    '# Each line is one Studio action as the CLI sends it; run it against `rcfd serve` (set RCFD_ADDR to its address).',
    'set -e',
    `ADDR="\${RCFD_ADDR:-${script.address.replaceAll(/[^A-Za-z0-9.:[\]_-]/g, '')}}"`,
  ];
  for (const [i, step] of script.steps.entries()) {
    const a = getAction(step.action);
    if (!a) throw new Error(`step ${i + 1}: unknown action '${step.action}'`);
    // the parameters' JSON as one shell word: literal text single-quoted, a session made earlier as "$S<n>" (session ids
    // are letters and digits, so the variable needs no JSON escaping)
    const names = new Set(sessionParams(a.params));
    let lit = '{';
    let word = '';
    for (const [j, [k, v]] of Object.entries(step.params).entries()) {
      lit += `${j ? ',' : ''}${JSON.stringify(k)}:`;
      if (names.has(k) && typeof v === 'string' && vars.has(v)) {
        word += `${shQuote(lit + '"')}"$${vars.get(v)}"`;
        lit = '"';
      } else {
        lit += JSON.stringify(v);
      }
    }
    word += shQuote(lit + '}');
    const call = `rcfd call --addr "$ADDR" ${a.engine.service} ${word}`;
    lines.push(`# ${i + 1}. ${a.title} (${a.id})`);
    if (step.session) {
      const v = `S${vars.size + 1}`;
      vars.set(step.session, v);
      lines.push(`${v}=$(${call} | sed -n 's/.*"session":"\\([^"]*\\)".*/\\1/p')`);
      lines.push(`test -n "$${v}"`);
    } else {
      lines.push(call);
    }
  }
  return lines.join('\n') + '\n';
}

/** Null when `v` is a script this version can replay, else what is wrong with it. */
export function scriptProblem(v: unknown): string | null {
  if (typeof v !== 'object' || v === null) return 'a script is a JSON object';
  const s = v as Partial<ActionScript>;
  if (s.format !== SCRIPT_FORMAT) return `format must be '${SCRIPT_FORMAT}'`;
  if (s.version !== SCRIPT_VERSION) return `version ${String(s.version)} is not supported (this Studio reads ${SCRIPT_VERSION})`;
  if (!Array.isArray(s.steps)) return 'steps must be a list';
  for (const [i, step] of s.steps.entries()) {
    if (typeof step !== 'object' || step === null) return `step ${i + 1} is not an object`;
    if (typeof step.action !== 'string' || !getAction(step.action)) return `step ${i + 1}: unknown action '${String(step.action)}'`;
    if (typeof step.params !== 'object' || step.params === null || Array.isArray(step.params)) return `step ${i + 1}: params must be an object`;
    if (step.session !== undefined && typeof step.session !== 'string') return `step ${i + 1}: session must be a string`;
  }
  return null;
}

export interface ReplayResult {
  readonly ok: boolean;
  /** One per step run, in order; a replay that stops at a failure has fewer than the script. */
  readonly steps: readonly { readonly action: string; readonly ok: boolean; readonly error?: ActionError }[];
  /** Recorded session id -> the id this replay made. */
  readonly sessions: Readonly<Record<string, string>>;
  /** Why the script was not run at all. */
  readonly problem?: string;
}

// ---- the dispatcher

export interface DispatcherOptions {
  /** What the dispatcher knows of the service (its state) for the availability check. */
  readonly context?: () => ActionContext;
  readonly log?: ActionLog;
  /** The clock of the log (tests). */
  readonly now?: () => Date;
}

export class Dispatcher {
  readonly log: ActionLog;
  private seq = 0;
  private readonly context: () => ActionContext;
  private readonly now: () => Date;

  constructor(
    private readonly transport: ActionTransport,
    options: DispatcherOptions = {},
  ) {
    this.log = options.log ?? new ActionLog();
    this.context = options.context ?? (() => ({}));
    this.now = options.now ?? (() => new Date());
  }

  /** Every registered action (the palette's source, F-831). */
  get actions(): readonly RegisteredAction[] {
    return ACTIONS;
  }

  /** Whether `id` can be dispatched now, and why not. */
  availability(id: ActionId, params: Record<string, unknown> = {}, context: ActionContext = {}) {
    const a = getAction(id)!;
    return availability(a, { ...this.context(), ...context }, params);
  }

  /** Dispatches a registered action with typed parameters. */
  dispatch<Id extends ActionId>(id: Id, params: ActionParams<Id>, options: DispatchOptions = {}): Promise<ActionReply> {
    return this.dispatchCall({ action: id, params: params as JsonObject }, options);
  }

  /** Dispatches an action by name with parameters of unknown shape (a replayed script, undo). Never rejects. */
  async dispatchCall(call: { readonly action: string; readonly params?: unknown }, options: DispatchOptions = {}): Promise<ActionReply> {
    const seq = ++this.seq;
    const started = this.now();
    const t0 = Date.now();
    const origin = options.origin ?? 'ui';
    const a = getAction(call.action);
    const params = clean(call.params ?? {});
    const refuse = (error: ActionError, effect: 'read' | 'change'): ActionReply => {
      const entry: LogEntry = { seq, action: call.action, params, effect, origin, at: started.toISOString(), ms: 0, sent: false, ok: false, error };
      this.log.add(entry);
      return { ok: false, error, entry };
    };
    if (!a) return refuse({ code: 'unknown_action', message: `'${call.action}' is not a registered action` }, 'change');
    const problem = actionParamsProblem(a, call.params ?? {});
    if (problem) return refuse({ code: 'bad_request', message: `${a.id}: ${problem}` }, a.effect);
    const av = availability(a, { ...this.context(), ...options.context }, params);
    if (!av.available) return refuse({ code: 'unavailable', message: av.reason }, a.effect);

    let before: ServiceReply | null = null;
    const pre = a.undo?.before?.(params as never) ?? null;
    if (pre) {
      const b = getAction(pre.action);
      before = b ? await this.send(b, pre.params, undefined) : null;
    }
    const reply = await this.send(a, params, options.timeoutMs);
    const base = { seq, action: a.id, params, effect: a.effect, origin, at: started.toISOString(), ms: Date.now() - t0, sent: true } as const;
    if (!reply.ok) {
      const entry: LogEntry = { ...base, ok: false, error: reply.error };
      this.log.add(entry);
      return { ok: false, error: reply.error, entry };
    }
    const made = a.produces === 'session' ? sessionOf(reply.result) : null;
    const entry: LogEntry = {
      ...base,
      ok: true,
      ...(made ? { session: made } : {}),
      ...(a.undo ? { inverse: a.undo.inverse(params as never, reply.result, before) } : {}),
    };
    this.log.add(entry);
    return { ok: true, result: reply.result, entry };
  }

  /** The log as a JSON script. */
  exportScript(options: ExportOptions = {}): ActionScript {
    return toScript(this.log.entries(), options);
  }

  /** The log as a shell script of `rcfd call` commands. */
  exportShell(options: ExportOptions = {}): string {
    return toShell(this.exportScript(options));
  }

  /**
   * Runs a JSON script again through this dispatcher (so the replayed actions are logged too, origin `replay`). The
   * sessions the script made are mapped to the sessions this run makes; a session it did not make (one that existed
   * before the recording) keeps its id. Stops at the first failing step unless `stopOnError` is false.
   */
  async replay(script: unknown, options: { readonly stopOnError?: boolean; readonly context?: ActionContext } = {}): Promise<ReplayResult> {
    const problem = scriptProblem(script);
    if (problem) return { ok: false, steps: [], sessions: {}, problem };
    const s = script as ActionScript;
    const sessions: Record<string, string> = {};
    const steps: { action: string; ok: boolean; error?: ActionError }[] = [];
    for (const step of s.steps) {
      const a = getAction(step.action)!;
      const params: Record<string, JsonValue> = { ...step.params };
      for (const k of sessionParams(a.params)) {
        const v = params[k];
        if (typeof v === 'string' && Object.hasOwn(sessions, v)) params[k] = sessions[v]!;
      }
      const r = await this.dispatchCall({ action: step.action, params }, { origin: 'replay', ...(options.context ? { context: options.context } : {}) });
      if (r.ok && step.session && r.entry.session) sessions[step.session] = r.entry.session;
      steps.push(r.ok ? { action: step.action, ok: true } : { action: step.action, ok: false, error: r.error });
      if (!r.ok && options.stopOnError !== false) break;
    }
    return { ok: steps.length === s.steps.length && steps.every((x) => x.ok), steps, sessions };
  }

  private async send(a: RegisteredAction, params: JsonObject, timeoutMs: number | undefined): Promise<ServiceReply> {
    const limit = timeoutMs ?? a.engine.timeoutMs;
    try {
      return await this.transport.call(a.engine.service, params as Record<string, unknown>, limit !== undefined ? { timeoutMs: limit } : undefined);
    } catch (e) {
      // the service bridge answers with values and never rejects; a transport that throws is reported the same way
      return { ok: false, error: { code: 'disconnected', message: `the call did not reach the service: ${String(e)}` } };
    }
  }
}

/** A JSON copy of the parameters without undefined members: what is sent, logged and exported. */
function clean(params: unknown): JsonObject {
  if (typeof params !== 'object' || params === null || Array.isArray(params)) return {};
  try {
    return JSON.parse(JSON.stringify(params)) as JsonObject;
  } catch {
    return {};
  }
}
