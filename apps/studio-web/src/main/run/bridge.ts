// The main-process end of the run port (F-810, src/shared/run.ts): every message the renderer's preload script sends is
// checked here before the engine is asked. A request must have exactly the fields of its operation; a run is a run id
// (RUN_ID), a step a non-negative safe integer. The open project's read-only project server answers (series.ts).
//
// No Electron import: the port is any object with postMessage and message and close callbacks (the unit tests use
// Node's MessageChannel; src/main/index.ts adapts Electron's MessagePortMain).
import { RUN_ID, type RunError, type RunPortMessage, type RunReply, type RunRequest } from '../../shared/run.js';
import { readRunAt, readRunSeries, type Query } from './series.js';

export interface RunPortLike {
  postMessage(message: RunPortMessage): void;
  onMessage(listener: (data: unknown) => void): void;
  onClose(listener: () => void): void;
  close(): void;
}

/** Where the bridge asks: the open project's query, or null when no project is open. */
export interface RunSource {
  query(): Query | null;
  now(): string;
}

const isPlainObject = (v: unknown): v is Record<string, unknown> => {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return false;
  const proto = Object.getPrototypeOf(v);
  return proto === Object.prototype || proto === null;
};

const isId = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;

export type ParsedRun = { readonly ok: true; readonly request: RunRequest } | { readonly ok: false; readonly id: number | null; readonly error: RunError };

/** Checks one message from the port. Anything not exactly a RunRequest is refused, with the reason. */
export function parseRunRequest(raw: unknown): ParsedRun {
  if (!isPlainObject(raw)) return { ok: false, id: null, error: { code: 'bad_request', message: 'a request is a plain object' } };
  const id = isId(raw['id']) ? raw['id'] : null;
  const bad = (message: string): ParsedRun => ({ ok: false, id, error: { code: 'bad_request', message } });
  if (id === null) return bad("'id' must be a non-negative integer");
  const op = raw['op'];
  const keys = Object.keys(raw).sort().join(',');
  const run = raw['run'];
  const runOk = typeof run === 'string' && RUN_ID.test(run);
  switch (op) {
    case 'series':
      if (keys !== 'id,op,run') return bad('series: takes exactly a run');
      if (!runOk) return bad('series: the run must be a run id (letters, digits, ".", "_" and "-", at most 128)');
      return { ok: true, request: { op, id, run } };
    case 'at': {
      if (keys !== 'id,op,run,step') return bad('at: takes exactly a run and a step');
      if (!runOk) return bad('at: the run must be a run id (letters, digits, ".", "_" and "-", at most 128)');
      const step = raw['step'];
      if (!isId(step)) return bad('at: the step must be a non-negative integer');
      return { ok: true, request: { op, id, run, step } };
    }
    default:
      return bad(`unknown op '${String(op).slice(0, 40)}'`);
  }
}

/** Serves one port: requests in, replies out. Ends when the port closes. */
export class RunPortBridge {
  private disposed = false;
  /** Requests refused by parseRunRequest (for tests and diagnostics). */
  refused = 0;

  constructor(
    private readonly port: RunPortLike,
    private readonly source: RunSource,
  ) {
    port.onMessage((data) => void this.onMessage(data));
    port.onClose(() => this.dispose());
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    try {
      this.port.close();
    } catch {
      // already closed
    }
  }

  private reply(id: number, reply: RunReply): void {
    if (this.disposed) return;
    try {
      this.port.postMessage({ type: 'reply', id, reply });
    } catch {
      this.dispose();
    }
  }

  private async onMessage(data: unknown): Promise<void> {
    const parsed = parseRunRequest(data);
    if (!parsed.ok) {
      this.refused++;
      if (parsed.id !== null) this.reply(parsed.id, { ok: false, error: parsed.error });
      return;
    }
    const r = parsed.request;
    const query = this.source.query();
    if (!query) {
      this.reply(r.id, { ok: false, error: { code: 'no_project', message: 'no project is open' } });
      return;
    }
    try {
      const result = r.op === 'series' ? await readRunSeries(query, r.run, this.source.now) : await readRunAt(query, r.run, r.step);
      this.reply(r.id, { ok: true, result });
    } catch (e) {
      this.reply(r.id, { ok: false, error: { code: 'engine', message: e instanceof Error ? e.message : String(e) } });
    }
  }
}
