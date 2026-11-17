// The main-process end of the project port (src/shared/project.ts): every message the renderer's preload script sends is
// checked here before the home acts on it. A request must have exactly the fields of its operation; a path is a string
// of at most 4096 characters, and `open` accepts only a folder of the recent list (home.ts). State changes go back the
// same way.
//
// No Electron import: the port is any object with postMessage and message and close callbacks (the unit tests use
// Node's MessageChannel; src/main/index.ts adapts Electron's MessagePortMain).
import {
  CLAUDE_CODE_OPS,
  CREATE_WHERE,
  type ClaudeCodeWhat,
  TRASH_OPS,
  type CreateWhere,
  type ProjectError,
  type ProjectPortMessage,
  type ProjectReply,
  type ProjectRequest,
  type ProjectState,
  type RunActionResult,
  type TrashOutcome,
  type TrashWhat,
} from '../../shared/project.js';
import { HomeRefusal } from './home.js';

export const MAX_PATH_CHARS = 4096;

export interface ProjectPortLike {
  postMessage(message: ProjectPortMessage): void;
  onMessage(listener: (data: unknown) => void): void;
  onClose(listener: () => void): void;
  close(): void;
}

/** The part of ProjectHome the bridge uses. */
export interface BridgeHome {
  readonly state: ProjectState;
  onState(listener: (s: ProjectState) => void): () => void;
  pick(): Promise<unknown>;
  openRecent(path: string): Promise<void>;
  forget(path: string): Promise<void>;
  close(): Promise<void>;
  refresh(): Promise<void>;
  dismiss(): void;
  /** F-807: a run-control action (the home answers it; optional so that a home without run control refuses it). */
  action?(id: string, params: Readonly<Record<string, unknown>>): Promise<RunActionResult>;
  /** F-832: cancels the opening that is going; true when there was one. */
  cancelOpen?(): boolean;
  /** F-835: makes a project with the engine and opens it (optional so that a home without it refuses it). */
  create?(where: CreateWhere): Promise<unknown>;
  /** ADR 0736: a trash operation of the engine on the open project (optional so that a home without it refuses it). */
  trash?(what: TrashWhat, params: Readonly<Record<string, unknown>>): Promise<TrashOutcome>;
}

/** ADR 0801 Addendum 23: Claude Code's registration and launch (src/main/claudecode/register.ts). */
export interface BridgeClaudeCode {
  do(what: ClaudeCodeWhat): Promise<unknown>;
}

/** F-807: the longest action id the port accepts, and the most bytes of an action's parameters as JSON. */
export const MAX_ACTION_CHARS = 64;
export const MAX_ACTION_PARAMS_BYTES = 64 * 1024;

/** Null when `v` is a plain object of JSON values (no functions, no class instances, nothing nested deeper than 8). */
function jsonObjectProblem(v: unknown, depth = 0): string | null {
  if (depth > 8) return 'nested too deep';
  if (v === null || typeof v === 'boolean' || typeof v === 'string') return null;
  if (typeof v === 'number') return Number.isFinite(v) ? null : 'not a finite number';
  if (Array.isArray(v)) {
    for (const x of v) {
      const p = jsonObjectProblem(x, depth + 1);
      if (p) return p;
    }
    return null;
  }
  if (isPlainObject(v)) {
    for (const x of Object.values(v)) {
      const p = jsonObjectProblem(x, depth + 1);
      if (p) return p;
    }
    return null;
  }
  return 'not a JSON value';
}

const isPlainObject = (v: unknown): v is Record<string, unknown> => {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
  const proto = Object.getPrototypeOf(v);
  return proto === Object.prototype || proto === null;
};

const isId = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;

export type ParsedProject =
  | { readonly ok: true; readonly request: ProjectRequest }
  | { readonly ok: false; readonly id: number | null; readonly error: ProjectError };

/** Checks one message from the port. Anything not exactly a ProjectRequest is refused, with the reason. */
export function parseProjectRequest(raw: unknown): ParsedProject {
  if (!isPlainObject(raw)) return { ok: false, id: null, error: { code: 'bad_request', message: 'a request is a plain object' } };
  const id = isId(raw['id']) ? raw['id'] : null;
  const bad = (message: string): ParsedProject => ({ ok: false, id, error: { code: 'bad_request', message } });
  if (id === null) return bad("'id' must be a non-negative integer");
  const op = raw['op'];
  const keys = Object.keys(raw).sort().join(',');
  switch (op) {
    case 'state':
    case 'pick':
    case 'close':
    case 'refresh':
    case 'dismiss':
    case 'cancel':
      if (keys !== 'id,op') return bad(`${op}: takes no arguments`);
      return { ok: true, request: { op, id } };
    case 'open':
    case 'forget': {
      if (keys !== 'id,op,path') return bad(`${op}: takes exactly a path`);
      const path = raw['path'];
      if (typeof path !== 'string' || path.length === 0 || path.length > MAX_PATH_CHARS || path.includes('\0')) {
        return bad(`${op}: the path must be a non-empty string of at most ${MAX_PATH_CHARS} characters`);
      }
      return { ok: true, request: { op, id, path } };
    }
    case 'create': {
      if (keys !== 'id,op,where') return bad('create: takes exactly where');
      const where = raw['where'];
      if (typeof where !== 'string' || !(CREATE_WHERE as readonly string[]).includes(where)) return bad(`create: where must be one of ${CREATE_WHERE.join(', ')}`);
      return { ok: true, request: { op, id, where: where as CreateWhere } };
    }
    case 'action': {
      if (keys !== 'action,id,op,params') return bad('action: takes exactly an action and its params');
      const action = raw['action'];
      const params = raw['params'];
      if (typeof action !== 'string' || !/^[a-z]+(\.[a-z_]+)+$/.test(action) || action.length > MAX_ACTION_CHARS) {
        return bad(`action: the action must be an action id of at most ${MAX_ACTION_CHARS} characters`);
      }
      if (!isPlainObject(params)) return bad('action: params must be a plain object');
      const problem = jsonObjectProblem(params);
      if (problem) return bad(`action: params: ${problem}`);
      if (JSON.stringify(params).length > MAX_ACTION_PARAMS_BYTES) return bad(`action: params are larger than ${MAX_ACTION_PARAMS_BYTES} bytes`);
      return { ok: true, request: { op, id, action, params } };
    }
    case 'trash': {
      if (keys !== 'id,op,params,what') return bad('trash: takes exactly what and its params');
      const what = raw['what'];
      const params = raw['params'];
      if (typeof what !== 'string' || !Object.hasOwn(TRASH_OPS, what)) return bad(`trash: what must be one of ${Object.keys(TRASH_OPS).join(', ')}`);
      if (!isPlainObject(params)) return bad('trash: params must be a plain object');
      const problem = jsonObjectProblem(params);
      if (problem) return bad(`trash: params: ${problem}`);
      if (JSON.stringify(params).length > MAX_ACTION_PARAMS_BYTES) return bad(`trash: params are larger than ${MAX_ACTION_PARAMS_BYTES} bytes`);
      return { ok: true, request: { op, id, what: what as TrashWhat, params } };
    }
    case 'claudeCode': {
      if (keys !== 'id,op,what') return bad('claudeCode: takes exactly what');
      const what = raw['what'];
      if (typeof what !== 'string' || !(CLAUDE_CODE_OPS as readonly string[]).includes(what)) return bad(`claudeCode: what must be one of ${CLAUDE_CODE_OPS.join(', ')}`);
      return { ok: true, request: { op, id, what: what as ClaudeCodeWhat } };
    }
    default:
      return bad(`unknown op '${String(op).slice(0, 40)}'`);
  }
}

/** Serves one port: requests in, replies and state out. Ends when the port closes. */
export class ProjectPortBridge {
  private readonly offState: () => void;
  private disposed = false;
  /** Requests refused by parseProjectRequest (for tests and diagnostics). */
  refused = 0;

  constructor(
    private readonly port: ProjectPortLike,
    private readonly home: BridgeHome,
    private readonly claudeCode: BridgeClaudeCode | null = null,
  ) {
    port.onMessage((data) => void this.onMessage(data));
    port.onClose(() => this.dispose());
    this.offState = home.onState((state) => this.post({ type: 'state', state }));
    this.post({ type: 'state', state: home.state });
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.offState();
    try {
      this.port.close();
    } catch {
      // already closed
    }
  }

  private post(m: ProjectPortMessage): void {
    if (this.disposed) return;
    try {
      this.port.postMessage(m);
    } catch {
      this.dispose();
    }
  }

  private reply(id: number, reply: ProjectReply): void {
    this.post({ type: 'reply', id, reply });
  }

  private async onMessage(data: unknown): Promise<void> {
    const parsed = parseProjectRequest(data);
    if (!parsed.ok) {
      this.refused++;
      if (parsed.id !== null) this.reply(parsed.id, { ok: false, error: parsed.error });
      return;
    }
    const r = parsed.request;
    try {
      let result: unknown = null;
      switch (r.op) {
        case 'state':
          result = this.home.state;
          break;
        case 'pick':
          result = await this.home.pick();
          break;
        case 'open':
          await this.home.openRecent(r.path);
          break;
        case 'forget':
          await this.home.forget(r.path);
          break;
        case 'close':
          await this.home.close();
          break;
        case 'refresh':
          await this.home.refresh();
          break;
        case 'dismiss':
          this.home.dismiss();
          break;
        case 'cancel':
          result = { cancelled: this.home.cancelOpen?.() ?? false };
          break;
        case 'create':
          if (!this.home.create) throw new Error('this Studio cannot make a project');
          result = await this.home.create(r.where);
          break;
        case 'action':
          result = this.home.action
            ? await this.home.action(r.action, r.params)
            : ({
                ok: false,
                request: null,
                failure: { code: 'engine', problem: 'This Studio has no run control.', recovery: 'Start a Studio built with F-807 or later.', engine: null },
              } satisfies RunActionResult);
          break;
        case 'trash':
          result = this.home.trash
            ? await this.home.trash(r.what, r.params)
            : ({ ok: false, code: 'unavailable', message: 'This Studio has no project trash.', data: null } satisfies TrashOutcome);
          break;
        case 'claudeCode':
          if (!this.claudeCode) throw new Error('this Studio cannot register or open Claude Code');
          result = await this.claudeCode.do(r.what);
          break;
      }
      this.reply(r.id, { ok: true, result });
    } catch (e) {
      const error: ProjectError =
        e instanceof HomeRefusal ? { code: e.code, message: e.message } : { code: 'bad_request', message: e instanceof Error ? e.message : String(e) };
      this.reply(r.id, { ok: false, error });
    }
  }
}
