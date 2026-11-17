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
  DERIVE_KINDS,
  type DeriveArgs,
  type DeriveKind,
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

/** Most seeds of a rake and steps of a streamline the Studio asks for (the engine's limits, ADR 0738 Q7). */
export const MAX_RAKE_SEEDS = 1000;
export const MAX_STREAM_STEPS = 10_000;

const isNum = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const isPoint = (v: unknown): v is [number, number, number] => Array.isArray(v) && v.length === 3 && v.every(isNum);

/** ADR 0738: checks the arguments of a derived picture; the reason as a string when they are not exactly DeriveArgs. */
export function parseDeriveArgs(v: unknown): DeriveArgs | string {
  if (!isPlainObject(v)) return 'args must be a plain object';
  const kind = v['kind'];
  if (!(DERIVE_KINDS as readonly unknown[]).includes(kind)) return `args.kind must be one of ${DERIVE_KINDS.join(', ')}`;
  const allowed: Record<DeriveKind, readonly string[]> = {
    cut_surface: ['axis', 'at', 'keep', 'vectors'],
    iso: ['field', 'level'],
    streamlines: ['rake', 'direction', 'maxSteps'],
  };
  const k = exactKeys(v, ['kind'], allowed[kind as DeriveKind]);
  if (k) return `args: ${k}`;
  const field = v['field'];
  if (field !== undefined && (typeof field !== 'string' || field.length === 0 || field.length > MAX_FIELD_NAME || field.startsWith('-'))) return `args.field must be a field name of 1 to ${MAX_FIELD_NAME} characters`;
  if (kind === 'iso' && (field === undefined || !isNum(v['level']))) return 'args: an iso-surface needs field and a finite level';
  const axis = v['axis'];
  if (axis !== undefined && axis !== 0 && axis !== 1 && axis !== 2) return 'args.axis must be 0, 1 or 2';
  if (v['at'] !== undefined && !isNum(v['at'])) return 'args.at must be a finite number';
  if ((axis === undefined) !== (v['at'] === undefined)) return 'args: a cut needs both axis and at';
  if (v['keep'] !== undefined && v['keep'] !== 'below' && v['keep'] !== 'above') return 'args.keep must be below or above';
  if (v['vectors'] !== undefined && typeof v['vectors'] !== 'boolean') return 'args.vectors must be a boolean';
  const rake = v['rake'];
  if (kind === 'streamlines') {
    if (!isPlainObject(rake)) return 'args.rake must be {p0, p1, n}';
    const rk = exactKeys(rake, ['p0', 'p1', 'n'], []);
    if (rk) return `args.rake: ${rk}`;
    if (!isPoint(rake['p0']) || !isPoint(rake['p1'])) return 'args.rake: p0 and p1 must be three finite numbers';
    const n = rake['n'];
    if (!Number.isInteger(n) || (n as number) < 1 || (n as number) > MAX_RAKE_SEEDS) return `args.rake.n must be an integer from 1 to ${MAX_RAKE_SEEDS}`;
  }
  const d = v['direction'];
  if (d !== undefined && d !== 'forward' && d !== 'backward' && d !== 'both') return 'args.direction must be forward, backward or both';
  const m = v['maxSteps'];
  if (m !== undefined && !(Number.isInteger(m) && (m as number) >= 1 && (m as number) <= MAX_STREAM_STEPS)) return `args.maxSteps must be an integer from 1 to ${MAX_STREAM_STEPS}`;
  return v as unknown as DeriveArgs;
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
  if (op !== 'open' && op !== 'field' && op !== 'surface' && op !== 'surfaceField' && op !== 'patchSurface' && op !== 'difference' && op !== 'derive') return bad(`unknown op '${String(op).slice(0, 40)}'`);
  const required =
    op === 'field' || op === 'surfaceField'
      ? ['op', 'id', 'store', 'snapshot', 'field']
      : op === 'difference'
        ? ['op', 'id', 'store', 'snapshot', 'other', 'otherSnapshot', 'field']
        : op === 'derive'
          ? ['op', 'id', 'store', 'snapshot', 'args']
          : ['op', 'id', 'store'];
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
  if (op === 'difference') {
    // F-815: two stores, two snapshots, one field (ADR 0801, addendum F-815)
    const op2 = storeProblem(raw['other']);
    if (op2) return bad(`difference: other ${op2}`);
    const o = raw['other'] as Record<string, unknown>;
    const other: StoreRef = { route: o['route'] as StoreRef['route'], path: o['path'] as string };
    const snapshot = raw['snapshot'];
    const otherSnapshot = raw['otherSnapshot'];
    const field = raw['field'];
    if (!isId(snapshot) || !isId(otherSnapshot)) return bad('difference: snapshot and otherSnapshot must be non-negative integers');
    if (typeof field !== 'string' || field.length === 0 || field.length > MAX_FIELD_NAME) return bad(`difference: the field name must be a string of 1 to ${MAX_FIELD_NAME} characters`);
    return { ok: true, request: { op, store, snapshot, other, otherSnapshot, field, ...common } };
  }
  if (op === 'derive') {
    // ADR 0738: the arguments of one `rcfd post derive`, member by member (parseDeriveArgs)
    const snapshot = raw['snapshot'];
    if (!isId(snapshot)) return bad('derive: snapshot must be a non-negative integer');
    const a = parseDeriveArgs(raw['args']);
    if (typeof a === 'string') return bad(`derive: ${a}`);
    return { ok: true, request: { op, store, snapshot, args: a, ...common } };
  }
  if (op === 'field' || op === 'surfaceField') {
    // F-819b: surfaceField has the arguments of field (the values come gathered on the store's surface)
    const snapshot = raw['snapshot'];
    const field = raw['field'];
    if (!isId(snapshot)) return bad(`${op}: snapshot must be a non-negative integer`);
    if (typeof field !== 'string' || field.length === 0 || field.length > MAX_FIELD_NAME) return bad(`${op}: the field name must be a string of 1 to ${MAX_FIELD_NAME} characters`);
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
