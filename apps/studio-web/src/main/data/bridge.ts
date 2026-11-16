// The main-process end of the data port (src/shared/data.ts): every message the renderer's preload script sends is
// checked here before any file is read or any request reaches the service. A request must have exactly the fields of
// its operation; a store is `{route, path}` with a known route and a bounded string path; a snapshot is a non-negative
// integer; a field name a bounded string. Local paths are checked again against the data roots (roots.ts) by the
// local route.
//
// No Electron import: the port is any object with postMessage and message and close callbacks, so the unit tests use
// Node's MessageChannel; src/main/data/port.ts adapts Electron's MessagePortMain.
import {
  DATA_ROUTES,
  DEFAULT_DATA_TIMEOUT_MS,
  MAX_DATA_PATH,
  MAX_DATA_PENDING,
  MAX_DATA_TIMEOUT_MS,
  MAX_FIELD_NAME,
  MIN_DATA_TIMEOUT_MS,
  type DataError,
  type DataPortMessage,
  type DataReply,
  type DataRequest,
  type DataWireResult,
  type StoreRef,
} from '../../shared/data.js';
import { DataFailure, type Job } from './queue.js';

export interface DataPortLike {
  postMessage(message: DataPortMessage): void;
  onMessage(listener: (data: unknown) => void): void;
  onClose(listener: () => void): void;
  close(): void;
}

/** What serves the requests: src/main/data/service.ts (the two routes). */
export interface DataBackend {
  run(request: Exclude<DataRequest, { op: 'cancel' }>, timeoutMs: number): Job<DataWireResult>;
}

const isPlainObject = (v: unknown): v is Record<string, unknown> => {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
  const proto = Object.getPrototypeOf(v);
  return proto === Object.prototype || proto === null;
};

const isId = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;
const isTag = (v: unknown): v is string | number => (typeof v === 'string' && v.length <= 64) || isId(v);

const exactKeys = (o: Record<string, unknown>, required: readonly string[], optional: readonly string[]): string | null => {
  for (const k of required) if (!Object.hasOwn(o, k)) return `missing '${k}'`;
  for (const k of Object.keys(o)) if (!required.includes(k) && !optional.includes(k)) return `unexpected '${k}'`;
  return null;
};

function storeProblem(v: unknown): string | null {
  if (!isPlainObject(v)) return 'store must be {route, path}';
  const k = exactKeys(v, ['route', 'path'], []);
  if (k) return `store: ${k}`;
  if (!(DATA_ROUTES as readonly unknown[]).includes(v['route'])) return `store: route must be one of ${DATA_ROUTES.join(', ')}`;
  const p = v['path'];
  if (typeof p !== 'string' || p.length === 0 || p.length > MAX_DATA_PATH) return `store: path must be a string of 1 to ${MAX_DATA_PATH} characters`;
  if (p.includes('\0')) return 'store: path holds a NUL character';
  return null;
}

export type ParsedData = { readonly ok: true; readonly request: DataRequest } | { readonly ok: false; readonly id: number | null; readonly error: DataError };

/** Checks one message from the port. Anything not exactly a DataRequest is refused, with the reason. */
export function parseDataRequest(raw: unknown): ParsedData {
  if (!isPlainObject(raw)) return { ok: false, id: null, error: { code: 'bad_request', message: 'a request is a plain object' } };
  const id = isId(raw['id']) ? raw['id'] : null;
  const bad = (message: string): ParsedData => ({ ok: false, id, error: { code: 'bad_request', message } });
  if (id === null) return bad("'id' must be a non-negative integer");
  const op = raw['op'];
  if (op === 'cancel') {
    const k = exactKeys(raw, ['op', 'id', 'tag'], []);
    if (k) return bad(`cancel: ${k}`);
    if (!isTag(raw['tag'])) return bad('cancel: tag must be a string of at most 64 characters or a non-negative integer');
    return { ok: true, request: { op: 'cancel', id, tag: raw['tag'] } };
  }
  if (op !== 'open' && op !== 'field' && op !== 'surface') return bad(`unknown op '${String(op).slice(0, 40)}'`);
  const required = op === 'field' ? ['op', 'id', 'store', 'snapshot', 'field'] : ['op', 'id', 'store'];
  const k = exactKeys(raw, required, ['tag', 'timeoutMs']);
  if (k) return bad(`${op}: ${k}`);
  const sp = storeProblem(raw['store']);
  if (sp) return bad(`${op}: ${sp}`);
  const tag = raw['tag'];
  if (tag !== undefined && !isTag(tag)) return bad(`${op}: tag must be a string of at most 64 characters or a non-negative integer`);
  const t = raw['timeoutMs'];
  if (t !== undefined && !(Number.isInteger(t) && (t as number) >= MIN_DATA_TIMEOUT_MS && (t as number) <= MAX_DATA_TIMEOUT_MS)) {
    return bad(`${op}: timeoutMs must be an integer from ${MIN_DATA_TIMEOUT_MS} to ${MAX_DATA_TIMEOUT_MS}`);
  }
  const s = raw['store'] as Record<string, unknown>;
  const store: StoreRef = { route: s['route'] as StoreRef['route'], path: s['path'] as string };
  const common = { id, ...(tag !== undefined ? { tag } : {}), ...(t !== undefined ? { timeoutMs: t as number } : {}) };
  if (op === 'field') {
    const snapshot = raw['snapshot'];
    const field = raw['field'];
    if (!isId(snapshot)) return bad('field: snapshot must be a non-negative integer');
    if (typeof field !== 'string' || field.length === 0 || field.length > MAX_FIELD_NAME) return bad(`field: the field name must be a string of 1 to ${MAX_FIELD_NAME} characters`);
    return { ok: true, request: { op, store, snapshot, field, ...common } };
  }
  return { ok: true, request: { op, store, ...common } };
}

interface Pending {
  readonly tag: string | number | undefined;
  readonly job: Job<DataWireResult>;
  readonly timer: NodeJS.Timeout;
}

/** Serves one port: requests in, replies out. Ends when the port closes; its waiting requests are cancelled then. */
export class DataPortBridge {
  private readonly pending = new Map<number, Pending>();
  private disposed = false;
  /** Requests refused by parseDataRequest (for tests and diagnostics). */
  refused = 0;

  constructor(
    private readonly port: DataPortLike,
    private readonly backend: DataBackend,
  ) {
    port.onMessage((data) => this.onMessage(data));
    port.onClose(() => this.dispose());
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const p of this.pending.values()) {
      clearTimeout(p.timer);
      p.job.cancel();
    }
    this.pending.clear();
    try {
      this.port.close();
    } catch {
      // already closed
    }
  }

  private reply(id: number, reply: DataReply<DataWireResult>): void {
    if (this.disposed) return;
    try {
      this.port.postMessage({ type: 'reply', id, reply });
    } catch {
      this.dispose();
    }
  }

  private onMessage(data: unknown): void {
    const parsed = parseDataRequest(data);
    if (!parsed.ok) {
      this.refused++;
      if (parsed.id !== null) this.reply(parsed.id, { ok: false, error: parsed.error });
      return;
    }
    const r = parsed.request;
    if (r.op === 'cancel') {
      let n = 0;
      for (const [id, p] of this.pending) {
        if (p.tag !== r.tag) continue;
        n++;
        this.settle(id, { ok: false, error: { code: 'cancelled', message: `cancelled (tag ${String(r.tag).slice(0, 64)})` } });
        p.job.cancel();
      }
      this.reply(r.id, { ok: true, result: { kind: 'cancel', cancelled: n } });
      return;
    }
    if (this.pending.has(r.id)) {
      this.reply(r.id, { ok: false, error: { code: 'bad_request', message: `request ${r.id} is already waiting` } });
      return;
    }
    if (this.pending.size >= MAX_DATA_PENDING) {
      this.reply(r.id, { ok: false, error: { code: 'bad_request', message: `at most ${MAX_DATA_PENDING} data requests may wait per window` } });
      return;
    }
    const timeoutMs = r.timeoutMs ?? DEFAULT_DATA_TIMEOUT_MS;
    let job: Job<DataWireResult>;
    try {
      job = this.backend.run(r, timeoutMs);
    } catch (e) {
      this.reply(r.id, { ok: false, error: failure(e) });
      return;
    }
    const timer = setTimeout(() => {
      this.settle(r.id, { ok: false, error: { code: 'timeout', message: `no data within ${timeoutMs} ms` } });
      job.cancel(new DataFailure('timeout', 'timeout'));
    }, timeoutMs);
    this.pending.set(r.id, { tag: r.tag, job, timer });
    job.promise.then(
      (result) => this.settle(r.id, { ok: true, result }),
      (e: unknown) => this.settle(r.id, { ok: false, error: failure(e) }),
    );
  }

  /** The first answer of a request wins; later ones (a result after a cancel) are dropped. */
  private settle(id: number, reply: DataReply<DataWireResult>): void {
    const p = this.pending.get(id);
    if (!p) return;
    this.pending.delete(id);
    clearTimeout(p.timer);
    this.reply(id, reply);
  }
}

function failure(e: unknown): DataError {
  if (e instanceof DataFailure) return e.toDataError();
  return { code: 'internal', message: e instanceof Error ? e.message : String(e) };
}
