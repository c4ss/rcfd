// The main-process end of the project port (src/shared/project.ts): every message the renderer's preload script sends is
// checked here before the home acts on it. A request must have exactly the fields of its operation; a path is a string
// of at most 4096 characters, and `open` accepts only a folder of the recent list (home.ts). State changes go back the
// same way.
//
// No Electron import: the port is any object with postMessage and message and close callbacks (the unit tests use
// Node's MessageChannel; src/main/index.ts adapts Electron's MessagePortMain).
import type { ProjectError, ProjectPortMessage, ProjectReply, ProjectRequest, ProjectState } from '../../shared/project.js';
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
      }
      this.reply(r.id, { ok: true, result });
    } catch (e) {
      const error: ProjectError =
        e instanceof HomeRefusal ? { code: e.code, message: e.message } : { code: 'bad_request', message: e instanceof Error ? e.message : String(e) };
      this.reply(r.id, { ok: false, error });
    }
  }
}
