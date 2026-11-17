// The main-process end of the service port (src/shared/service.ts): every message the renderer's preload script sends is
// checked here before anything reaches the service. A request must have exactly the fields of its operation, a method
// on SERVICE_METHODS, and parameters made of plain JSON values only. Replies, state changes and event batches go back
// the same way.
//
// No Electron import: the port is any object with postMessage and message and close callbacks, so the unit tests use
// Node's MessageChannel; src/main/index.ts adapts Electron's MessagePortMain.
import {
  DEFAULT_CALL_TIMEOUT_MS,
  MAX_CALL_TIMEOUT_MS,
  MIN_CALL_TIMEOUT_MS,
  SERVICE_METHODS,
  type PortMessage,
  type PortRequest,
  type ServiceError,
  type ServiceMethod,
  type ServiceReply,
  type ServiceState,
} from '../../shared/service.js';
import type { ClientEventBatch } from './client.js';
import { RpcError } from './rpc.js';
import { packResult } from '../../shared/monitors.js';

const ALLOWED = new Set<string>(SERVICE_METHODS);

/** Limits of the parameters of one call: nesting depth and number of values. */
export const MAX_PARAM_DEPTH = 64;
export const MAX_PARAM_VALUES = 1_000_000;

export interface PortLike {
  postMessage(message: PortMessage): void;
  onMessage(listener: (data: unknown) => void): void;
  onClose(listener: () => void): void;
  close(): void;
}

/** The part of ServiceClient the bridge uses. */
export interface BridgeClient {
  readonly state: ServiceState;
  call(method: string, params?: Record<string, unknown>, timeoutMs?: number): Promise<unknown>;
  onState(listener: (s: ServiceState) => void): () => void;
  onEvents(listener: (b: ClientEventBatch) => void): () => void;
}

const isPlainObject = (v: unknown): v is Record<string, unknown> => {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
  const proto = Object.getPrototypeOf(v);
  return proto === Object.prototype || proto === null;
};

const isId = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;

/** Null when `v` is made of JSON values only (null, booleans, finite numbers, strings, arrays, plain objects). */
export function jsonProblem(v: unknown): string | null {
  let values = 0;
  const walk = (x: unknown, depth: number, where: string): string | null => {
    if (++values > MAX_PARAM_VALUES) return `more than ${MAX_PARAM_VALUES} values`;
    if (depth > MAX_PARAM_DEPTH) return `nested deeper than ${MAX_PARAM_DEPTH}`;
    if (x === null || typeof x === 'boolean' || typeof x === 'string') return null;
    if (typeof x === 'number') return Number.isFinite(x) ? null : `${where} is not a finite number`;
    if (Array.isArray(x)) {
      for (let i = 0; i < x.length; i++) {
        if (!(i in x)) return `${where}[${i}] is a hole`;
        const p = walk(x[i], depth + 1, `${where}[${i}]`);
        if (p) return p;
      }
      return null;
    }
    if (isPlainObject(x)) {
      for (const k of Object.keys(x)) {
        const p = walk(x[k], depth + 1, `${where}.${k}`);
        if (p) return p;
      }
      return null;
    }
    return `${where} is not a JSON value (${Object.prototype.toString.call(x)})`;
  };
  return walk(v, 0, 'params');
}

const exactKeys = (o: Record<string, unknown>, required: readonly string[], optional: readonly string[]): string | null => {
  for (const k of required) if (!Object.hasOwn(o, k)) return `missing '${k}'`;
  for (const k of Object.keys(o)) if (!required.includes(k) && !optional.includes(k)) return `unexpected '${k}'`;
  return null;
};

export type Parsed = { readonly ok: true; readonly request: PortRequest } | { readonly ok: false; readonly id: number | null; readonly error: ServiceError };

/** Checks one message from the port. Anything not exactly a PortRequest is refused, with the reason. */
export function parseRequest(raw: unknown): Parsed {
  if (!isPlainObject(raw)) return { ok: false, id: null, error: { code: 'bad_request', message: 'a request is a plain object' } };
  const id = isId(raw['id']) ? raw['id'] : null;
  const bad = (message: string, code: ServiceError['code'] = 'bad_request'): Parsed => ({ ok: false, id, error: { code, message } });
  if (id === null) return bad("'id' must be a non-negative integer");
  switch (raw['op']) {
    case 'call': {
      const k = exactKeys(raw, ['op', 'id', 'method'], ['params', 'timeoutMs']);
      if (k) return bad(`call: ${k}`);
      const method = raw['method'];
      if (typeof method !== 'string') return bad('call: the method must be a string');
      if (!ALLOWED.has(method)) return bad(`call: '${method.slice(0, 80)}' is not on the Studio's list of service methods`, 'not_allowed');
      const params = raw['params'];
      if (params !== undefined) {
        if (!isPlainObject(params)) return bad('call: params must be an object');
        const p = jsonProblem(params);
        if (p) return bad(`call: ${p}`);
      }
      const t = raw['timeoutMs'];
      if (t !== undefined && !(Number.isInteger(t) && (t as number) >= MIN_CALL_TIMEOUT_MS && (t as number) <= MAX_CALL_TIMEOUT_MS)) {
        return bad(`call: timeoutMs must be an integer from ${MIN_CALL_TIMEOUT_MS} to ${MAX_CALL_TIMEOUT_MS}`);
      }
      const request: PortRequest = {
        op: 'call',
        id,
        method: method as ServiceMethod,
        ...(params !== undefined ? { params: params as Record<string, unknown> } : {}),
        ...(t !== undefined ? { timeoutMs: t as number } : {}),
      };
      return { ok: true, request };
    }
    case 'state': {
      const k = exactKeys(raw, ['op', 'id'], []);
      return k ? bad(`state: ${k}`) : { ok: true, request: { op: 'state', id } };
    }
    case 'subscribe': {
      const k = exactKeys(raw, ['op', 'id', 'subscription'], ['session']);
      if (k) return bad(`subscribe: ${k}`);
      if (!isId(raw['subscription'])) return bad('subscribe: subscription must be a non-negative integer');
      const session = raw['session'];
      if (session !== undefined && (typeof session !== 'string' || session.length > 256)) return bad('subscribe: session must be a string');
      return {
        ok: true,
        request: { op: 'subscribe', id, subscription: raw['subscription'], ...(session !== undefined ? { session } : {}) },
      };
    }
    case 'unsubscribe': {
      const k = exactKeys(raw, ['op', 'id', 'subscription'], []);
      if (k) return bad(`unsubscribe: ${k}`);
      if (!isId(raw['subscription'])) return bad('unsubscribe: subscription must be a non-negative integer');
      return { ok: true, request: { op: 'unsubscribe', id, subscription: raw['subscription'] } };
    }
    default:
      return bad(`unknown op '${String(raw['op']).slice(0, 40)}'`);
  }
}

/** Most subscriptions one port may hold. */
export const MAX_SUBSCRIPTIONS = 64;

/** Serves one port: requests in, replies, state and events out. Ends when the port closes. */
export class PortBridge {
  private readonly subs = new Map<number, { readonly session: string | undefined }>();
  private readonly offState: () => void;
  private readonly offEvents: () => void;
  private disposed = false;
  /** Requests refused by parseRequest (for tests and diagnostics). */
  refused = 0;

  constructor(
    private readonly port: PortLike,
    private readonly client: BridgeClient,
  ) {
    port.onMessage((data) => void this.onMessage(data));
    port.onClose(() => this.dispose());
    this.offState = client.onState((state) => this.post({ type: 'state', state }));
    this.offEvents = client.onEvents((b) => this.deliver(b));
    this.post({ type: 'state', state: client.state });
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.offState();
    this.offEvents();
    this.subs.clear();
    try {
      this.port.close();
    } catch {
      // already closed
    }
  }

  private post(m: PortMessage): void {
    if (this.disposed) return;
    try {
      this.port.postMessage(m);
    } catch {
      this.dispose();
    }
  }

  private reply(id: number, reply: ServiceReply): void {
    this.post({ type: 'reply', id, reply });
  }

  private async onMessage(data: unknown): Promise<void> {
    const parsed = parseRequest(data);
    if (!parsed.ok) {
      this.refused++;
      if (parsed.id !== null) this.reply(parsed.id, { ok: false, error: parsed.error });
      return;
    }
    const r = parsed.request;
    switch (r.op) {
      case 'call':
        try {
          const result = await this.client.call(r.method, r.params ?? {}, r.timeoutMs ?? DEFAULT_CALL_TIMEOUT_MS);
          // monitors.get: series as Float64Array, the same numbers in one block (F-811, src/shared/monitors.ts)
          this.reply(r.id, { ok: true, result: packResult(r.method, result) });
        } catch (e) {
          const error: ServiceError =
            e instanceof RpcError ? e.toServiceError() : { code: 'protocol', message: e instanceof Error ? e.message : String(e) };
          this.reply(r.id, { ok: false, error });
        }
        return;
      case 'state':
        this.reply(r.id, { ok: true, result: this.client.state });
        return;
      case 'subscribe':
        if (!this.subs.has(r.subscription) && this.subs.size >= MAX_SUBSCRIPTIONS) {
          this.reply(r.id, { ok: false, error: { code: 'bad_request', message: `at most ${MAX_SUBSCRIPTIONS} subscriptions per window` } });
          return;
        }
        this.subs.set(r.subscription, { session: r.session });
        this.reply(r.id, { ok: true, result: r.subscription });
        return;
      case 'unsubscribe':
        this.subs.delete(r.subscription);
        this.reply(r.id, { ok: true, result: null });
        return;
    }
  }

  private deliver(b: ClientEventBatch): void {
    for (const [subscription, { session }] of this.subs) {
      const events = session === undefined ? b.events : b.events.filter((e) => e.session === session);
      if (events.length === 0 && b.missed === 0) continue;
      this.post({ type: 'events', subscription, instance: b.instance, events, missed: b.missed });
    }
  }
}
