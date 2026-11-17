// F-837: what the removed egui Studio showed of a run and of the runner beside the run list, rebuilt on the run port
// (src/shared/run.ts); the runner's budgets through the engine commands of ADR 0733.
//
// - A run's log and events: the end of runs/<id>/log.txt (`read_project_file`, one page from the end) and the last lines
//   of its events.jsonl (`run_status {events}`), through the open project's read-only project server, the tools Claude
//   Code uses. A warning of the runner or of the solver is named as one (the egui Studio's event text, ADR 0147
//   amendment 3).
// - A run's report: runs/<id>/output/report.html, which the solver writes with its standard result files
//   (rcfd-api runctl `write_results`); whether it is there is asked of the engine, and Open hands the file to the
//   system's program for HTML (outside the Studio's window: G10 keeps remote content out of the window, not out of the
//   user's browser).
// - The runner's budgets: `.rcfd/runner-config.json`, owned by rcfd-runner (its `Config`, ADR 0123 section 6: every key
//   optional, unknown keys refused, read when the runner starts). The runner reads and writes it itself (ADR 0733):
//   `runner_config` on the read-only project server reads it, `set_runner_config` on the run-control server (not
//   read-only) writes it with the runner's own checks, atomically, and never over a file another writer changed since it
//   was read (G18, error 1200). The last write of this window can be undone: the keys it set are sent back with their
//   values before it (ADR 0733 Q2 (a)). The Studio writes nothing into .rcfd/ itself.
// - Save PNG: the main process captures a rectangle of the page (the field view with its legend and caption: DESIGN.md,
//   a field is never shown without them) and writes it where the user says in the save dialog.
//
// No Electron import: src/main/index.ts hands in the capture, the dialog and the system's opener.
import { writeFile } from 'node:fs/promises';
import {
  BUDGET_KEYS,
  LOG_EVENTS,
  LOG_LINES,
  LOG_TAIL_BYTES,
  type BudgetKey,
  type BudgetsOutcome,
  type PageRect,
  type RunEventLine,
  type RunLog,
  type RunnerBudgets,
  type SavedPng,
} from '../../shared/run.js';
import type { Query } from './series.js';

type Obj = Record<string, unknown>;
const obj = (v: unknown): Obj => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Obj) : {});
const str = (v: unknown, d = ''): string => (typeof v === 'string' ? v : d);
const num = (v: unknown, d = 0): number => (typeof v === 'number' && Number.isFinite(v) ? v : d);
const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));
/** The engine's "no such file" (1202), or a path that is not there. */
const notFound = (e: unknown): boolean => (e as { code?: unknown })?.code === 1202 || /not found|no such file|cannot find/i.test(message(e));

// ---- events and log

/** The JSON of an event cut at 160 characters, as the egui Studio showed an event it had no words for. */
function cut(s: string): string {
  return s.length > 160 ? `${s.slice(0, 160)}...` : s;
}

/** One line of events.jsonl in words (the egui Studio's `event_text`, ADR 0147 amendment 3): warnings in full. */
export function eventLine(e: unknown): RunEventLine {
  const o = obj(e);
  const kind = str(o['kind'], str(o['event'], 'event'));
  const t = str(o['t']);
  if (kind === 'state') {
    const from = typeof o['from'] === 'string' ? o['from'] : '-';
    const to = str(o['to'], str(o['state']));
    const reason = typeof o['reason'] === 'string' ? `: ${o['reason']}` : '';
    return { t, kind, text: `state ${from} -> ${to}${reason}`, warning: false };
  }
  if (kind === 'warning') return { t, kind, text: `warning: ${str(o['message'], cut(JSON.stringify(o)))}`, warning: true };
  if (kind === 'stop') return { t, kind, text: `stop: ${str(o['reason'], cut(JSON.stringify(o)))}`, warning: false };
  if (kind === 'solver') {
    const s = obj(o['solver']);
    if (s['event'] === 'warning' && typeof s['message'] === 'string') return { t, kind, text: `warning: ${s['message']}`, warning: true };
    // the solver's own events by name, with the step and the reason it gives (its full record is in events.jsonl)
    if (typeof s['event'] === 'string') {
      const at = typeof s['iteration'] === 'number' ? ` at iteration ${s['iteration']}` : typeof s['step'] === 'number' ? ` at step ${s['step']}` : '';
      const why = typeof s['why'] === 'string' ? `: ${s['why']}` : typeof s['reason'] === 'string' ? `: ${s['reason']}` : typeof s['message'] === 'string' ? `: ${s['message']}` : '';
      return { t, kind, text: cut(`solver ${s['event']}${at}${why}`), warning: false };
    }
  }
  const rest: Obj = { ...o };
  delete rest['kind'];
  delete rest['t'];
  delete rest['event'];
  return { t, kind, text: `${kind} ${cut(JSON.stringify(rest))}`, warning: false };
}

/** The end of a project text file: at most `bytes` from its end, in pages of the engine's read_project_file. */
async function tail(query: Query, file: string, bytes: number): Promise<{ size: number; text: string; cut: boolean }> {
  const head = obj(await query('read_project_file', { path: file, offset: 0, limit: 1 }));
  const size = num(head['size']);
  const hash = head['hash'];
  let start = Math.max(0, size - bytes);
  let text = '';
  let offset: number | null = null;
  // a start inside a character is refused by the engine: the next byte, at most three times (UTF-8 is at most 4 bytes)
  for (let k = 0; k < 4 && offset === null; k++) {
    try {
      const r = obj(await query('read_project_file', { path: file, offset: start }));
      if (r['hash'] !== hash) throw new Error(`${file} changed while it was read; read it again`);
      text = str(r['text']);
      offset = typeof r['next_offset'] === 'number' ? r['next_offset'] : -1;
    } catch (e) {
      if (k < 3 && /character boundary/.test(message(e))) start += 1;
      else throw e;
    }
  }
  for (let page = 0; offset !== null && offset >= 0 && page < 8; page++) {
    const r = obj(await query('read_project_file', { path: file, offset }));
    if (r['hash'] !== hash) throw new Error(`${file} changed while it was read; read it again`);
    text += str(r['text']);
    offset = typeof r['next_offset'] === 'number' ? r['next_offset'] : null;
  }
  return { size, text, cut: start > 0 };
}

/** A run's log tail, last events and report, through the engine's project server. */
export async function readRunLog(query: Query, run: string, now: () => string): Promise<RunLog> {
  const logPath = `runs/${run}/log.txt`;
  let log: RunLog['log'] = null;
  let logError: string | null = null;
  try {
    const t = await tail(query, logPath, LOG_TAIL_BYTES);
    let lines = t.text.split(/\r?\n/);
    // a tail that starts inside a line drops that piece; the file's last newline leaves an empty last piece
    if (t.cut) lines = lines.slice(1);
    if (lines.length > 0 && lines[lines.length - 1] === '') lines.pop();
    const many = lines.length > LOG_LINES;
    log = { path: logPath, size: t.size, lines: many ? lines.slice(-LOG_LINES) : lines, cut: t.cut || many };
  } catch (e) {
    if (!notFound(e)) logError = `${logPath} could not be read: ${message(e)}`;
  }
  let events: RunEventLine[] = [];
  let eventsError: string | null = null;
  try {
    const st = obj(await query('run_status', { run, events: LOG_EVENTS }));
    events = ((st['events'] as unknown[] | undefined) ?? []).map(eventLine);
  } catch (e) {
    eventsError = `run_status of ${run} failed: ${message(e)}`;
  }
  const reportPath = reportFile(run);
  let report: RunLog['report'] = null;
  try {
    const r = obj(await query('read_project_file', { path: reportPath, offset: 0, limit: 1 }));
    report = { path: reportPath, size: num(r['size']) };
  } catch {
    // not there (or not readable as text): no report to open
  }
  return { run, log, logError, events, eventsError, report, readAt: now() };
}

/** A run's report, project-relative. */
export const reportFile = (run: string): string => `runs/${run}/output/report.html`;

// ---- the runner's budgets

/** The engine's error code of a write over a file changed since its base hash was read. */
const CONFLICT = 1200;
/** The engine's error codes of a refusal: the runner's checks (1003), an argument it does not take (-32602). */
const REFUSED = [1003, -32602];

const codeOf = (e: unknown): unknown => (e as { code?: unknown })?.code;

/** The engine's answer of `runner_config` (or `set_runner_config`) as the panel shows it. */
export function budgetsOf(state: unknown, undo: boolean): RunnerBudgets {
  const o = obj(state);
  const values: Partial<Record<BudgetKey, number>> = {};
  const v = obj(o['values']);
  for (const k of BUDGET_KEYS) if (typeof v[k] === 'number') values[k] = v[k] as number;
  const u = o['in_use'];
  let inUse: RunnerBudgets['inUse'] = null;
  if (typeof u === 'object' && u !== null) {
    const used: Partial<Record<BudgetKey, number>> = {};
    const c = obj(obj(u)['values']);
    for (const k of BUDGET_KEYS) if (typeof c[k] === 'number') used[k] = c[k] as number;
    inUse = { values: used, source: str(obj(u)['source']), started: str(obj(u)['started']) };
  }
  return {
    path: str(o['path']),
    exists: o['exists'] === true,
    values,
    hash: typeof o['hash'] === 'string' ? o['hash'] : null,
    problem: typeof o['problem'] === 'string' ? o['problem'] : null,
    needsReplace: o['needs_replace'] === true,
    inUse,
    undo,
  };
}

/** How the main process reaches the engine for the budgets of the open project. */
export interface BudgetsEngine {
  /** `runner_config` on the open project's read-only project server. */
  read(): Promise<unknown>;
  /** `set_runner_config` on the open project's run-control server (not read-only). */
  write(args: Record<string, unknown>): Promise<unknown>;
}

type BudgetSet = Partial<Record<BudgetKey, number | null>>;

/** The budgets of the open project folder through the engine, and this window's last write (for undo). */
export class RunnerBudgetsEngine {
  /** The last write: the folder, the keys it set with their values before it (null: unset), and the hash it left. */
  private last: { folder: string; restore: BudgetSet; after: string } | null = null;

  constructor(private readonly engine: BudgetsEngine) {}

  private undoable(folder: string): boolean {
    return this.last !== null && this.last.folder === folder;
  }

  async read(folder: string): Promise<RunnerBudgets> {
    return budgetsOf(await this.engine.read(), this.undoable(folder));
  }

  /** The engine's answer to one `set_runner_config`: written, a conflict (the file now), or the runner's refusal. */
  private async send(folder: string, args: Record<string, unknown>): Promise<{ outcome: BudgetsOutcome; state: unknown }> {
    try {
      const state = await this.engine.write(args);
      return { outcome: { kind: 'written', budgets: budgetsOf(state, true) }, state };
    } catch (e) {
      if (codeOf(e) === CONFLICT) return { outcome: { kind: 'conflict', budgets: await this.read(folder) }, state: null };
      if (REFUSED.includes(codeOf(e) as number)) return { outcome: { kind: 'refused', message: message(e) }, state: null };
      throw e;
    }
  }

  /**
   * The runner sets (a number) or removes (null) keys, if the file still has `base` (null: if there is still no file);
   * `replace`: exactly these keys, every one, over a file the runner cannot read. Its checks are the runner's own.
   */
  async write(folder: string, set: Readonly<BudgetSet>, base: string | null, replace: boolean): Promise<BudgetsOutcome> {
    const before = await this.read(folder);
    if (before.hash !== base) return { kind: 'conflict', budgets: before };
    const { outcome, state } = await this.send(folder, { set: { ...set }, base_hash: base, ...(replace ? { replace: true } : {}) });
    if (outcome.kind === 'written') {
      // a file the runner could not read is not put back by values: no undo of its replacement
      if (before.needsReplace) this.last = null;
      else {
        const restore: BudgetSet = {};
        for (const k of Object.keys(set) as BudgetKey[]) restore[k] = before.values[k] ?? null;
        this.last = { folder, restore, after: str(obj(state)['hash']) };
      }
      return { kind: 'written', budgets: { ...outcome.budgets, undo: this.undoable(folder) } };
    }
    return outcome;
  }

  /**
   * Sets the keys of this window's last write back to their values before it, with the hash that write left as the base.
   * A file changed since by another writer is a conflict unless `force` (then the file as it is now is the base). Where
   * there was no file, the keys are removed and `{}` stays (every key at the runner's default): the Studio removes no file
   * of a project.
   */
  async undo(folder: string, force: boolean): Promise<BudgetsOutcome> {
    const last = this.last;
    if (!last || last.folder !== folder) return { kind: 'refused', message: 'Nothing to undo: this window has not changed the runner’s budgets of this project.' };
    const base = force ? (await this.read(folder)).hash : last.after;
    const { outcome } = await this.send(folder, { set: { ...last.restore }, base_hash: base });
    if (outcome.kind === 'written') {
      this.last = null;
      return { kind: 'written', budgets: { ...outcome.budgets, undo: false } };
    }
    return outcome;
  }
}

// ---- Save PNG

export interface PngDeps {
  /** A PNG of the rectangle of the page (CSS pixels), at the window's scale. */
  capture(rect: PageRect): Promise<{ png: Uint8Array; width: number; height: number }>;
  /** The save dialog with a proposed name; the path chosen, or null when cancelled. */
  savePath(name: string): Promise<string | null>;
}

/** The rectangle rounded out to whole pixels; refused when it is empty or not finite. */
export function wholeRect(r: PageRect): PageRect | null {
  const vals = [r.x, r.y, r.width, r.height];
  if (!vals.every((v) => Number.isFinite(v)) || !(r.width > 0 && r.height > 0)) return null;
  const x = Math.max(0, Math.floor(r.x));
  const y = Math.max(0, Math.floor(r.y));
  const width = Math.ceil(r.x + r.width) - x;
  const height = Math.ceil(r.y + r.height) - y;
  return width > 0 && height > 0 && width <= 16384 && height <= 16384 ? { x, y, width, height } : null;
}

export async function savePng(deps: PngDeps, rect: PageRect, name: string): Promise<SavedPng> {
  const r = wholeRect(rect);
  if (!r) throw new Error('the picture has no area');
  // the picture first, so it is the view as it was when Save PNG was pressed
  const shot = await deps.capture(r);
  const where = await deps.savePath(name);
  if (where === null) return { path: null, width: shot.width, height: shot.height, bytes: shot.png.length };
  await writeFile(where, shot.png);
  return { path: where, width: shot.width, height: shot.height, bytes: shot.png.length };
}
