// The main-process end of the run port (F-810, src/shared/run.ts): every message the renderer's preload script sends is
// checked here before the engine is asked. A request must have exactly the fields of its operation; a run is a run id
// (RUN_ID), a step a non-negative safe integer. The open project's read-only project server answers (series.ts).
//
// No Electron import: the port is any object with postMessage and message and close callbacks (the unit tests use
// Node's MessageChannel; src/main/index.ts adapts Electron's MessagePortMain).
import path from 'node:path';
import { BUDGET_KEYS, PNG_NAME, RUN_ID, type BudgetKey, type PageRect, type RunError, type RunPortMessage, type RunReply, type RunRequest } from '../../shared/run.js';
import { readRunLog, reportFile, savePng, type PngDeps, type RunnerBudgetsFile } from './extras.js';
import { readComparison, readRunAt, readRunSeries, type Query } from './series.js';

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
  /** F-837: the open project's folder (null: none), its budgets file, the system's opener and the picture of the page. */
  readonly extras?: {
    folder(): string | null;
    budgets: RunnerBudgetsFile;
    /** Opens a file in the system's program for it; resolves to '' when it did, else to the reason (Electron's shell.openPath). */
    openPath(file: string): Promise<string>;
    png: PngDeps;
  };
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
    case 'compare': {
      // F-815
      if (keys !== 'id,op,other,run') return bad('compare: takes exactly a run and another run');
      const other = raw['other'];
      if (!runOk || typeof other !== 'string' || !RUN_ID.test(other)) return bad('compare: both runs must be run ids (letters, digits, ".", "_" and "-", at most 128)');
      if (other === run) return bad('compare: the two runs must differ');
      return { ok: true, request: { op, id, run, other } };
    }
    // F-837
    case 'log':
    case 'openReport':
      if (keys !== 'id,op,run') return bad(`${op}: takes exactly a run`);
      if (!runOk) return bad(`${op}: the run must be a run id (letters, digits, ".", "_" and "-", at most 128)`);
      return { ok: true, request: { op, id, run } };
    case 'budgets':
      if (keys !== 'id,op') return bad('budgets: takes nothing');
      return { ok: true, request: { op, id } };
    case 'setBudgets': {
      if (keys !== 'base,id,op,values') return bad('setBudgets: takes exactly values and a base');
      const base = raw['base'];
      if (base !== null && (typeof base !== 'string' || !/^[0-9a-f]{64}$/.test(base))) return bad('setBudgets: the base is a sha256 in hex, or null');
      const values = raw['values'];
      if (!isPlainObject(values)) return bad('setBudgets: the values are an object');
      const out: Partial<Record<BudgetKey, number | null>> = {};
      for (const [k, v] of Object.entries(values)) {
        if (!(BUDGET_KEYS as readonly string[]).includes(k)) return bad(`setBudgets: unknown key '${k.slice(0, 40)}'`);
        if (v !== null && (typeof v !== 'number' || !Number.isFinite(v))) return bad(`setBudgets: ${k} is a number or null`);
        out[k as BudgetKey] = v;
      }
      return { ok: true, request: { op, id, values: out, base } };
    }
    case 'undoBudgets': {
      if (keys !== 'force,id,op') return bad('undoBudgets: takes exactly force');
      const force = raw['force'];
      if (typeof force !== 'boolean') return bad('undoBudgets: force is true or false');
      return { ok: true, request: { op, id, force } };
    }
    case 'savePng': {
      if (keys !== 'id,name,op,rect') return bad('savePng: takes exactly a rectangle and a name');
      const rect = raw['rect'];
      if (!isPlainObject(rect) || Object.keys(rect).sort().join(',') !== 'height,width,x,y') return bad('savePng: the rectangle has exactly x, y, width and height');
      if (!['x', 'y', 'width', 'height'].every((k) => typeof rect[k] === 'number' && Number.isFinite(rect[k]))) return bad('savePng: the rectangle is four finite numbers');
      const name = raw['name'];
      if (typeof name !== 'string' || !PNG_NAME.test(name)) return bad('savePng: the name is letters, digits, ".", "_" and "-", ending in .png');
      return { ok: true, request: { op, id, rect: rect as unknown as PageRect, name } };
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
    if (r.op === 'savePng' || r.op === 'budgets' || r.op === 'setBudgets' || r.op === 'undoBudgets' || r.op === 'openReport') {
      this.reply(r.id, await this.extra(r));
      return;
    }
    const query = this.source.query();
    if (!query) {
      this.reply(r.id, { ok: false, error: { code: 'no_project', message: 'no project is open' } });
      return;
    }
    try {
      const result =
        r.op === 'series'
          ? await readRunSeries(query, r.run, this.source.now)
          : r.op === 'at'
            ? await readRunAt(query, r.run, r.step)
            : r.op === 'log'
              ? await readRunLog(query, r.run, this.source.now)
              : await readComparison(query, r.run, r.other);
      this.reply(r.id, { ok: true, result });
    } catch (e) {
      this.reply(r.id, { ok: false, error: { code: 'engine', message: e instanceof Error ? e.message : String(e) } });
    }
  }

  /** F-837's operations that read or write files of the folder, or open a program, in the main process. */
  private async extra(r: Extract<RunRequest, { op: 'savePng' | 'budgets' | 'setBudgets' | 'undoBudgets' | 'openReport' }>): Promise<RunReply> {
    const x = this.source.extras;
    if (!x) return { ok: false, error: { code: 'io', message: 'this window cannot read or write files' } };
    const io = (e: unknown): RunReply => ({ ok: false, error: { code: 'io', message: e instanceof Error ? e.message : String(e) } });
    if (r.op === 'savePng') {
      try {
        return { ok: true, result: await savePng(x.png, r.rect, r.name) };
      } catch (e) {
        return io(e);
      }
    }
    const folder = x.folder();
    if (folder === null) return { ok: false, error: { code: 'no_project', message: 'no project is open' } };
    try {
      if (r.op === 'budgets') return { ok: true, result: await x.budgets.read(folder) };
      if (r.op === 'setBudgets') return { ok: true, result: await x.budgets.write(folder, r.values, r.base) };
      if (r.op === 'undoBudgets') return { ok: true, result: await x.budgets.undo(folder, r.force) };
      // openReport: only a report the engine finds in the run's folder (the run id is checked: no '..', no separator)
      const query = this.source.query();
      if (!query) return { ok: false, error: { code: 'no_project', message: 'no project is open' } };
      const rel = reportFile(r.run);
      try {
        await query('read_project_file', { path: rel, offset: 0, limit: 1 });
      } catch (e) {
        return { ok: false, error: { code: 'not_found', message: `${rel} is not there: ${e instanceof Error ? e.message : String(e)}` } };
      }
      const why = await x.openPath(path.join(folder, ...rel.split('/')));
      return why === '' ? { ok: true, result: { path: rel } } : { ok: false, error: { code: 'io', message: `${rel} was not opened: ${why}` } };
    } catch (e) {
      return io(e);
    }
  }
}
