// What the runs table shows of each run (F-806), as pure functions of the engine's answers (`list_runs` and
// `run_status`, src/shared/project.ts), so the unit tests check them without a window. Display only: every number is
// the engine's, and where the engine has no value the text says so in words (G3, G6).
import type { RunDetail, RunEntry } from '../../shared/project.js';
import type { MarkState } from '../ui/pictogramPaths.js';

const two = (n: number): string => String(n).padStart(2, '0');

/** "14:03:22", local time; the input as given when it is not a time (as project/format.ts writes it). */
export function clock(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`;
}

/** One run with the newest of its two answers: `run_status` is read after `list_runs`, so it wins when present. */
export interface RunRow {
  readonly id: string;
  readonly case: string;
  readonly created: string;
  /** The runner's state word (status.json). */
  readonly state: string;
  readonly step: number;
  readonly time: number | null;
  readonly metrics: Readonly<Record<string, number>>;
  readonly detail: RunDetail | null;
  readonly detailError: string | null;
}

export function runRows(items: readonly RunEntry[], details: Readonly<Record<string, RunDetail>>, errors: Readonly<Record<string, string>>): RunRow[] {
  return items.map((r) => {
    const d = details[r.id] ?? null;
    return {
      id: r.id,
      case: r.case,
      created: r.created,
      state: d?.state ?? r.state,
      step: d?.step ?? r.step,
      time: d ? d.time : r.time,
      metrics: r.metrics,
      detail: d,
      detailError: errors[r.id] ?? null,
    };
  });
}

/** The state as the table shows it: the runner's state, and for a finished run whose solver says so, Converged. */
export function shownState(row: Pick<RunRow, 'state' | 'detail'>): MarkState | null {
  switch (row.state) {
    case 'queued':
    case 'running':
    case 'paused':
    case 'failed':
    case 'cancelled':
      return row.state;
    case 'finished':
      return row.detail?.finish?.state === 'converged' ? 'converged' : 'finished';
    default:
      return null;
  }
}

/** The engine's word for a state it knows and the Studio does not (a newer runner): capitalised, as given. */
export const unknownWord = (state: string): string => (state.length > 0 ? state[0]!.toUpperCase() + state.slice(1) : 'Unknown state');

/** An engine code (`not_converged`) in words (`Not converged`). */
export const codeWords = (code: string): string => unknownWord(code.replace(/_/g, ' '));

/** A number as the engine gave it, without grouping, at most `digits` decimals (tabular figures do the aligning). */
export const plain = (v: number, digits = 4): string => v.toLocaleString('en', { useGrouping: false, maximumFractionDigits: digits });

/**
 * The simulated time of a run in words when there is no number to show. A steady run's solver reports 0 (it has no
 * simulated time); a run that has not started has none yet; a NaN reads as null. Null when the number is true.
 */
export function timeNote(row: Pick<RunRow, 'state' | 'step' | 'time' | 'detail'>): string | null {
  if (row.time === null) return 'none reported';
  const steady = row.detail?.steady ?? null;
  if (steady === true) return 'none, steady';
  if (row.state === 'queued') return 'none yet';
  if (row.step === 0 && steady !== false) return row.state === 'running' || row.state === 'paused' ? 'none yet' : 'none';
  // a time other than 0 is a simulated time whether or not the start event was read; a 0 is one only for a transient run
  if (steady === null && row.time === 0) return 'not known';
  return null;
}

/** The time column's text: the engine's number, or the words of timeNote. */
export const timeText = (row: Pick<RunRow, 'state' | 'step' | 'time' | 'detail'>): string => timeNote(row) ?? plain(row.time!);

/** Steps of a steady run are iterations. */
const stepsWord = (row: Pick<RunRow, 'detail'>, n: number): string =>
  row.detail?.steady === true ? `${n} ${n === 1 ? 'iteration' : 'iterations'}` : `${n} ${n === 1 ? 'step' : 'steps'}`;

/** The result cell: a headline (the numbers that say where the run is) and the runner's or solver's sentence. */
export interface Result {
  readonly head: string;
  readonly note: string;
}

/** The reason of a finished run without the solver's state code the state column already says ("converged: ..."). */
function finishedNote(row: RunRow): string {
  const reason = row.detail?.reason ?? null;
  const fin = row.detail?.finish ?? null;
  if (reason === null) return fin?.reason ?? 'The runner recorded no reason.';
  const code = fin?.state ?? reason.split(':')[0]!;
  const rest = reason.startsWith(`${code}: `) ? reason.slice(code.length + 2) : reason;
  if (code === 'converged' || code === 'finished') return rest;
  return `${codeWords(code)}: ${rest}`;
}

export function result(row: RunRow): Result {
  const t = timeNote(row) === null ? `t = ${plain(row.time!)} s, ` : '';
  const d = row.detail;
  switch (row.state) {
    case 'queued':
      return { head: 'Not started', note: d?.reason && d.reason !== 'created' ? `Queued: ${d.reason}.` : 'Waiting for the runner to start it.' };
    case 'running':
      return { head: `${t}${row.detail?.steady === true ? 'iteration' : 'step'} ${row.step}`, note: d ? `Updated by the runner at ${clock(d.updated)}.` : 'Reading the run’s status…' };
    case 'paused':
      return { head: `${t}${row.detail?.steady === true ? 'iteration' : 'step'} ${row.step}`, note: d?.reason ? `Paused: ${d.reason}.` : 'Paused on request; it goes on when resumed.' };
    case 'finished':
      return { head: `${t}${stepsWord(row, row.step)}`, note: d ? finishedNote(row) : 'Reading the run’s status…' };
    case 'failed':
      return {
        head: row.step === 0 ? 'Failed before its first step' : `Stopped at ${row.detail?.steady === true ? 'iteration' : 'step'} ${row.step}`,
        note: d?.reason ?? (d ? 'The runner recorded no reason.' : 'Reading the run’s status…'),
      };
    case 'cancelled':
      return { head: `Stopped at ${row.detail?.steady === true ? 'iteration' : 'step'} ${row.step}`, note: d?.reason ?? 'Cancelled on request.' };
    default:
      return { head: `${stepsWord(row, row.step)}`, note: d?.reason ?? `The runner reports the state “${row.state}”, which this Studio does not know.` };
  }
}

// ---- sort and filter

export type SortColumn = 'case' | 'state' | 'result';
export interface Sort {
  readonly column: SortColumn;
  readonly direction: 'ascending' | 'descending';
}

/** State order for sorting: what needs a look first (running, paused, queued, failed), then what ended. */
const STATE_ORDER: Readonly<Record<string, number>> = { running: 0, paused: 1, queued: 2, failed: 3, cancelled: 4, converged: 5, finished: 6 };

const stateKey = (r: RunRow): number => STATE_ORDER[shownState(r) ?? ''] ?? 7;

export function sortRows(rows: readonly RunRow[], sort: Sort): RunRow[] {
  const dir = sort.direction === 'descending' ? -1 : 1;
  const byCase = (a: RunRow, b: RunRow) => a.case.localeCompare(b.case) || a.id.localeCompare(b.id);
  return [...rows].sort((a, b) => {
    if (sort.column === 'state') return dir * (stateKey(a) - stateKey(b)) || byCase(a, b);
    if (sort.column === 'result') return dir * (a.step - b.step) || byCase(a, b);
    return dir * byCase(a, b);
  });
}

/** The state filter: every state, or one shown state (Converged and Finished apart, as the table shows them). */
export type StateFilter = 'all' | MarkState | 'other';

export function filterRows(rows: readonly RunRow[], text: string, state: StateFilter): RunRow[] {
  const q = text.trim().toLowerCase();
  return rows.filter((r) => {
    const s = shownState(r);
    if (state !== 'all' && (state === 'other' ? s !== null : s !== state)) return false;
    return q === '' || r.case.toLowerCase().includes(q) || r.id.toLowerCase().includes(q);
  });
}

/** How many runs are in each shown state. */
export function stateCounts(rows: readonly RunRow[]): Map<StateFilter, number> {
  const m = new Map<StateFilter, number>();
  for (const r of rows) {
    const s: StateFilter = shownState(r) ?? 'other';
    m.set(s, (m.get(s) ?? 0) + 1);
  }
  return m;
}

/** The case of a run without its `cases/` folder, `/`-separated. */
export function caseName(c: string): string {
  const p = c.replace(/\\/g, '/');
  return p.startsWith('cases/') ? p.slice(6) : p;
}
