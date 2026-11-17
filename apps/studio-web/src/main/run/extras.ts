// F-837: what the removed egui Studio showed of a run and of the runner beside the run list, rebuilt on the run port
// (src/shared/run.ts) without a new engine entry point.
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
//   optional, unknown keys refused, read when the runner starts). No engine command writes it, so the main process reads
//   and writes it itself, only the keys the runner reads, with the runner's own checks, and never over a file another
//   writer changed since it was read (G18). The last write of this window can be undone. The write is not atomic (a
//   write-then-rename would be: the Studio does not rename in project folders, tests/unit/history.spec.ts); the runner
//   reads the file only when it starts. Proposed engine entry point: an rcfd-runner (or MCP) command that sets budgets.
// - Save PNG: the main process captures a rectangle of the page (the field view with its legend and caption: DESIGN.md,
//   a field is never shown without them) and writes it where the user says in the save dialog.
//
// No Electron import: src/main/index.ts hands in the capture, the dialog and the system's opener.
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import {
  BUDGET_KEYS,
  BUDGET_KIND,
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

export const BUDGETS_FILE = '.rcfd/runner-config.json';
export const RUNNER_RECORD = '.rcfd/runner.json';

const sha256 = (b: Uint8Array | string): string => createHash('sha256').update(b).digest('hex');

/** Why the runner would refuse `value` for `key` (its serde types and its load checks), or null. */
export function budgetProblem(key: string, value: unknown): string | null {
  if (!(BUDGET_KEYS as readonly string[]).includes(key)) return `unknown key '${key}' (the runner reads ${BUDGET_KEYS.join(', ')})`;
  const kind = BUDGET_KIND[key as BudgetKey];
  if (typeof value !== 'number' || !Number.isFinite(value)) return `${key} must be a number`;
  if (kind === 'real') return value >= 0 ? null : `${key} must not be negative`;
  if (!Number.isSafeInteger(value)) return `${key} must be a whole number`;
  if (kind === 'positive' && value < 1) return `${key} must be positive (the runner refuses 0)`;
  if (value < 0) return `${key} must not be negative`;
  return null;
}

/** The values of a budgets object and the first problem the runner would have with it. */
function valuesOf(o: unknown, file: string): { values: Partial<Record<BudgetKey, number>>; problem: string | null } {
  if (typeof o !== 'object' || o === null || Array.isArray(o)) return { values: {}, problem: `${file}: not a JSON object` };
  const values: Partial<Record<BudgetKey, number>> = {};
  let problem: string | null = null;
  for (const [k, v] of Object.entries(o)) {
    const p = budgetProblem(k, v);
    if (p) problem ??= `${file}: ${p}`;
    else values[k as BudgetKey] = v as number;
  }
  return { values, problem };
}

/** The budgets as read: the file's text (null: no file) and the runner's record (null: none). */
export function budgetsOf(text: string | null, runnerRecord: string | null, undo: boolean): RunnerBudgets {
  let values: Partial<Record<BudgetKey, number>> = {};
  let problem: string | null = null;
  if (text !== null) {
    try {
      ({ values, problem } = valuesOf(JSON.parse(text), BUDGETS_FILE));
    } catch (e) {
      problem = `${BUDGETS_FILE}: not JSON (${message(e)})`;
    }
  }
  let inUse: RunnerBudgets['inUse'] = null;
  if (runnerRecord !== null) {
    try {
      const r = obj(JSON.parse(runnerRecord));
      const c = obj(r['config']);
      const used: Partial<Record<BudgetKey, number>> = {};
      for (const k of BUDGET_KEYS) if (typeof c[k] === 'number') used[k] = c[k] as number;
      inUse = { values: used, source: str(r['config_source']), started: str(r['started']) };
    } catch {
      // a record being written: shown as none
    }
  }
  return { path: BUDGETS_FILE, exists: text !== null, values, hash: text === null ? null : sha256(text), problem, inUse, undo };
}

async function readText(file: string): Promise<string | null> {
  try {
    return await readFile(file, 'utf8');
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === 'ENOENT') return null;
    throw e;
  }
}

/** The budgets of the open project folder, and this window's last write (for undo). */
export class RunnerBudgetsFile {
  /** The last write: the folder, the text before it (null: there was no file) and the hash it left. */
  private last: { folder: string; before: string | null; after: string } | null = null;

  async read(folder: string): Promise<RunnerBudgets> {
    const text = await readText(path.join(folder, BUDGETS_FILE));
    const record = await readText(path.join(folder, RUNNER_RECORD)).catch(() => null);
    const undo = this.last !== null && this.last.folder === folder;
    return budgetsOf(text, record, undo);
  }

  /** Sets (a number) or removes (null) keys, if the file still has `base` (null: if there is still no file). */
  async write(folder: string, set: Readonly<Partial<Record<BudgetKey, number | null>>>, base: string | null): Promise<BudgetsOutcome> {
    for (const [k, v] of Object.entries(set)) {
      if (v === null) {
        if (!(BUDGET_KEYS as readonly string[]).includes(k)) return { kind: 'refused', message: budgetProblem(k, 0)! };
        continue;
      }
      const p = budgetProblem(k, v);
      if (p) return { kind: 'refused', message: p };
    }
    const file = path.join(folder, BUDGETS_FILE);
    const text = await readText(file);
    const hash = text === null ? null : sha256(text);
    if (hash !== base) return { kind: 'conflict', budgets: await this.read(folder) };
    let current: Obj = {};
    if (text !== null) {
      try {
        const o: unknown = JSON.parse(text);
        if (typeof o === 'object' && o !== null && !Array.isArray(o)) current = o as Obj;
      } catch {
        // the runner refuses this file as it is: it is replaced by the keys set here (undo puts it back)
      }
    }
    const next: Obj = {};
    // the runner's order of keys, then any other key the file held (kept as it was: the runner names it when it refuses)
    for (const k of BUDGET_KEYS) {
      const v = Object.hasOwn(set, k) ? set[k] : current[k];
      if (v !== null && v !== undefined) next[k] = v;
    }
    for (const [k, v] of Object.entries(current)) if (!(BUDGET_KEYS as readonly string[]).includes(k)) next[k] = v;
    const out = `${JSON.stringify(next, null, 2)}\n`;
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, out, 'utf8');
    this.last = { folder, before: text, after: sha256(out) };
    return { kind: 'written', budgets: await this.read(folder) };
  }

  /**
   * Puts back what this window's last write replaced. A file changed since by another writer is a conflict unless
   * `force`. Where there was no file, `{}` is written (every key at the runner's default): the Studio removes no file of a
   * project.
   */
  async undo(folder: string, force: boolean): Promise<BudgetsOutcome> {
    const last = this.last;
    if (!last || last.folder !== folder) return { kind: 'refused', message: 'Nothing to undo: this window has not changed the runner’s budgets of this project.' };
    const file = path.join(folder, BUDGETS_FILE);
    const text = await readText(file);
    if (!force && (text === null ? null : sha256(text)) !== last.after) return { kind: 'conflict', budgets: await this.read(folder) };
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, last.before ?? '{}\n', 'utf8');
    this.last = null;
    return { kind: 'written', budgets: await this.read(folder) };
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
