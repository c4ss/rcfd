// The main-process end of the case port (src/shared/case.ts): every message the renderer's preload script sends is
// checked here before the editor acts on it. A request must have exactly the fields of its operation, strings of bounded
// length; a path must be a case file of the project (editor.ts, caseRelPath). Replies go back the same way.
//
// No Electron import: the port is any object with postMessage and message and close callbacks (the unit tests use
// Node's MessageChannel; src/main/index.ts adapts Electron's MessagePortMain).
import {
  MAX_NAME_CHARS,
  MAX_PATH_CHARS,
  MAX_SESSION_CHARS,
  type CaseError,
  type CasePortMessage,
  type CaseReply,
  type CaseRequest,
  type NewCaseSource,
} from '../../shared/case.js';
import { CaseRefusal, type CaseEditor } from './editor.js';

export interface CasePortLike {
  postMessage(message: CasePortMessage): void;
  onMessage(listener: (data: unknown) => void): void;
  onClose(listener: () => void): void;
  close(): void;
}

const isPlainObject = (v: unknown): v is Record<string, unknown> => {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
  const proto = Object.getPrototypeOf(v);
  return proto === Object.prototype || proto === null;
};

const isId = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;

const text = (v: unknown, max: number): v is string => typeof v === 'string' && v.length > 0 && v.length <= max && !v.includes('\0');

export type ParsedCase =
  | { readonly ok: true; readonly request: CaseRequest }
  | { readonly ok: false; readonly id: number | null; readonly error: CaseError };

/** The fields of each operation besides `id` and `op`, sorted. */
const FIELDS: Record<CaseRequest['op'], string> = {
  state: '',
  sources: '',
  prepare: 'path',
  attach: 'hash,path,session',
  status: 'session',
  conflict: 'session',
  save: 'onto,session',
  create: 'name,source',
  duplicate: 'name,path',
  check: 'path',
  forget: 'session',
  cancel: 'target',
  pickFile: '',
  attachFile: 'path,session',
};

/** Checks one message from the port. Anything not exactly a CaseRequest is refused, with the reason. */
export function parseCaseRequest(raw: unknown): ParsedCase {
  if (!isPlainObject(raw)) return { ok: false, id: null, error: { code: 'bad_request', message: 'a request is a plain object' } };
  const id = isId(raw['id']) ? raw['id'] : null;
  const bad = (message: string): ParsedCase => ({ ok: false, id, error: { code: 'bad_request', message } });
  if (id === null) return bad("'id' must be a non-negative integer");
  const op = raw['op'];
  if (typeof op !== 'string' || !Object.hasOwn(FIELDS, op)) return bad(`unknown op '${String(op).slice(0, 40)}'`);
  const o = op as CaseRequest['op'];
  const keys = Object.keys(raw)
    .filter((k) => k !== 'id' && k !== 'op')
    .sort()
    .join(',');
  if (keys !== FIELDS[o]) return bad(`${o}: takes exactly ${FIELDS[o] === '' ? 'no arguments' : FIELDS[o].split(',').join(', ')}`);
  if ('path' in raw && !text(raw['path'], MAX_PATH_CHARS)) return bad(`${o}: the path must be a non-empty string of at most ${MAX_PATH_CHARS} characters`);
  if ('session' in raw && !(text(raw['session'], MAX_SESSION_CHARS) && /^[A-Za-z0-9_.-]+$/.test(raw['session'] as string))) {
    return bad(`${o}: the session must be a service session id (letters, digits, '_', '.', '-'; at most ${MAX_SESSION_CHARS})`);
  }
  if ('hash' in raw && !text(raw['hash'], 200)) return bad(`${o}: the hash must be a non-empty string`);
  if ('name' in raw && !text(raw['name'], MAX_NAME_CHARS)) return bad(`${o}: the name must be a non-empty string of at most ${MAX_NAME_CHARS} characters`);
  if (o === 'save' && raw['onto'] !== 'opened' && raw['onto'] !== 'current') return bad("save: onto is 'opened' or 'current'");
  if (o === 'cancel' && !isId(raw['target'])) return bad('cancel: target must be the id of a request');
  if (o === 'create') {
    const s = raw['source'];
    if (!isPlainObject(s) || Object.keys(s).sort().join(',') !== 'kind,name' || (s['kind'] !== 'template' && s['kind'] !== 'example') || !text(s['name'], MAX_PATH_CHARS)) {
      return bad("create: source is {kind: 'template' | 'example', name}");
    }
    return { ok: true, request: { op: 'create', id, name: raw['name'] as string, source: { kind: s['kind'], name: s['name'] } as NewCaseSource } };
  }
  return { ok: true, request: { ...raw, op: o, id } as CaseRequest };
}

/** Serves one port: requests in, replies out. Ends when the port closes. */
export class CasePortBridge {
  private disposed = false;
  /** F-832: the saves and checks going, by request id, with what cancels them. */
  private readonly going = new Map<number, { readonly op: 'save' | 'check'; readonly controller: AbortController; readonly cancelled: () => void }>();
  /** Requests refused by parseCaseRequest (for tests and diagnostics). */
  refused = 0;

  constructor(
    private readonly port: CasePortLike,
    private readonly editor: CaseEditor,
  ) {
    port.onMessage((data) => void this.onMessage(data));
    port.onClose(() => this.dispose());
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const g of this.going.values()) g.controller.abort();
    this.going.clear();
    try {
      this.port.close();
    } catch {
      // already closed
    }
  }

  private reply(id: number, reply: CaseReply): void {
    if (this.disposed) return;
    try {
      this.port.postMessage({ type: 'reply', id, reply });
    } catch {
      this.dispose();
    }
  }

  private async onMessage(data: unknown): Promise<void> {
    const parsed = parseCaseRequest(data);
    if (!parsed.ok) {
      this.refused++;
      if (parsed.id !== null) this.reply(parsed.id, { ok: false, error: parsed.error });
      return;
    }
    const r = parsed.request;
    const e = this.editor;
    if (r.op === 'cancel') {
      const g = this.going.get(r.target);
      if (g) {
        g.controller.abort();
        g.cancelled();
      }
      this.reply(r.id, { ok: true, result: { cancelled: g !== undefined } });
      return;
    }
    // F-832: a save or a check can be cancelled while it is going (`cancel` above)
    let signal: AbortSignal | undefined;
    if (r.op === 'save' || r.op === 'check') {
      const controller = new AbortController();
      signal = controller.signal;
      let answered = false;
      const settle = (reply: CaseReply) => {
        if (answered) return;
        answered = true;
        this.going.delete(r.id);
        this.reply(r.id, reply);
      };
      this.going.set(r.id, {
        op: r.op,
        controller,
        // a check is answered at once (its result, when the engine gives it, is dropped); a save answers for itself
        cancelled: () => {
          if (r.op === 'check') settle({ ok: false, error: { code: 'cancelled', message: 'The check was cancelled; its result is not shown.' } });
        },
      });
      try {
        const result = r.op === 'save' ? await e.save(r.session, r.onto, signal) : await e.check(r.path, signal);
        settle({ ok: true, result });
      } catch (err) {
        settle({ ok: false, error: caseError(err) });
      }
      return;
    }
    try {
      let result: unknown = null;
      switch (r.op) {
        case 'state':
          result = e.state();
          break;
        case 'sources':
          result = await e.sources();
          break;
        case 'prepare':
          result = await e.prepare(r.path);
          break;
        case 'attach':
          result = await e.attach(r.session, r.path, r.hash);
          break;
        case 'status':
          result = await e.status(r.session);
          break;
        case 'conflict':
          result = await e.conflict(r.session);
          break;
        case 'create':
          result = await e.create(r.name, r.source);
          break;
        case 'duplicate':
          result = await e.duplicate(r.path, r.name);
          break;
        case 'forget':
          e.forget(r.session);
          break;
        case 'pickFile':
          result = await e.pickFile();
          break;
        case 'attachFile':
          result = await e.attachFile(r.session, r.path);
          break;
      }
      this.reply(r.id, { ok: true, result });
    } catch (err) {
      this.reply(r.id, { ok: false, error: caseError(err) });
    }
  }
}

function caseError(err: unknown): CaseError {
  return err instanceof CaseRefusal
    ? { code: err.code, message: err.message, ...(err.data !== undefined ? { data: err.data } : {}) }
    : { code: 'engine', message: err instanceof Error ? err.message : String(err) };
}
