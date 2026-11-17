// The main-process end of the Claude port (src/shared/claude.ts): every message the renderer's preload script sends is
// checked here before the panel acts on it. A request must have exactly the fields of its operation: a session is null
// or a UUID, a message a string of at most MAX_MESSAGE_CHARS characters, a token 32 hex characters. The renderer names
// no program, flag or file; state changes go back the same way.
//
// No Electron import: the port is any object with postMessage and message and close callbacks (the unit tests use
// Node's MessageChannel; port.ts adapts Electron's MessagePortMain).
import { isUuid, MAX_MESSAGE_CHARS, type ClaudeError, type ClaudePortMessage, type ClaudeReply, type ClaudeRequest, type ClaudeState } from '../../shared/claude.js';
import { ClaudeRefusal } from './panel.js';

export interface ClaudePortLike {
  postMessage(message: ClaudePortMessage): void;
  onMessage(listener: (data: unknown) => void): void;
  onClose(listener: () => void): void;
  close(): void;
}

/** The part of ClaudePanel the bridge uses. */
export interface BridgePanel {
  readonly state: ClaudeState;
  onState(listener: (s: ClaudeState) => void): () => void;
  select(session: string | null): void;
  send(session: string | null, text: string): Promise<{ session: string }>;
  cancel(): void;
  confirm(token: string): Promise<void>;
  refresh(): Promise<void>;
}

const isPlainObject = (v: unknown): v is Record<string, unknown> => {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
  const proto = Object.getPrototypeOf(v);
  return proto === Object.prototype || proto === null;
};

const isId = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;

export type ParsedClaude =
  | { readonly ok: true; readonly request: ClaudeRequest }
  | { readonly ok: false; readonly id: number | null; readonly error: ClaudeError };

/** Checks one message from the port. Anything not exactly a ClaudeRequest is refused, with the reason. */
export function parseClaudeRequest(raw: unknown): ParsedClaude {
  if (!isPlainObject(raw)) return { ok: false, id: null, error: { code: 'bad_request', message: 'a request is a plain object' } };
  const id = isId(raw['id']) ? raw['id'] : null;
  const bad = (message: string): ParsedClaude => ({ ok: false, id, error: { code: 'bad_request', message } });
  if (id === null) return bad("'id' must be a non-negative integer");
  const op = raw['op'];
  const keys = Object.keys(raw).sort().join(',');
  const session = raw['session'];
  const sessionOk = session === null || (typeof session === 'string' && isUuid(session));
  switch (op) {
    case 'state':
    case 'cancel':
    case 'refresh':
      if (keys !== 'id,op') return bad(`${op}: takes no arguments`);
      return { ok: true, request: { op, id } };
    case 'select':
      if (keys !== 'id,op,session') return bad('select: takes exactly a session');
      if (!sessionOk) return bad('select: the session is null or a session id (a UUID)');
      return { ok: true, request: { op, id, session: session as string | null } };
    case 'send': {
      if (keys !== 'id,op,session,text') return bad('send: takes exactly a session and a text');
      if (!sessionOk) return bad('send: the session is null (a new chat) or a session id (a UUID)');
      const text = raw['text'];
      if (typeof text !== 'string' || text.length === 0 || text.length > MAX_MESSAGE_CHARS) {
        return bad(`send: the text must be a non-empty string of at most ${MAX_MESSAGE_CHARS} characters`);
      }
      return { ok: true, request: { op, id, session: session as string | null, text } };
    }
    case 'confirm': {
      if (keys !== 'id,op,token') return bad('confirm: takes exactly a token');
      const token = raw['token'];
      if (typeof token !== 'string' || !/^[0-9a-f]{32}$/.test(token)) return bad('confirm: the token is the one the state shows');
      return { ok: true, request: { op, id, token } };
    }
    default:
      return bad(`unknown op '${String(op).slice(0, 40)}'`);
  }
}

/** Serves one port: requests in, replies and state out. Ends when the port closes. */
export class ClaudePortBridge {
  private readonly offState: () => void;
  private disposed = false;
  /** Requests refused by parseClaudeRequest (for tests and diagnostics). */
  refused = 0;

  constructor(
    private readonly port: ClaudePortLike,
    private readonly panel: BridgePanel,
  ) {
    port.onMessage((data) => void this.onMessage(data));
    port.onClose(() => this.dispose());
    this.offState = panel.onState((state) => this.post({ type: 'state', state }));
    this.post({ type: 'state', state: panel.state });
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

  private post(m: ClaudePortMessage): void {
    if (this.disposed) return;
    try {
      this.port.postMessage(m);
    } catch {
      this.dispose();
    }
  }

  private reply(id: number, reply: ClaudeReply): void {
    this.post({ type: 'reply', id, reply });
  }

  private async onMessage(data: unknown): Promise<void> {
    const parsed = parseClaudeRequest(data);
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
          result = this.panel.state;
          break;
        case 'select':
          this.panel.select(r.session);
          break;
        case 'send':
          result = await this.panel.send(r.session, r.text);
          break;
        case 'cancel':
          this.panel.cancel();
          break;
        case 'confirm':
          await this.panel.confirm(r.token);
          break;
        case 'refresh':
          await this.panel.refresh();
          break;
      }
      this.reply(r.id, { ok: true, result });
    } catch (e) {
      const error: ClaudeError =
        e instanceof ClaudeRefusal ? { code: e.code, message: e.message } : { code: 'refused', message: e instanceof Error ? e.message : String(e) };
      this.reply(r.id, { ok: false, error });
    }
  }
}
