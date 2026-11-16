// The runs of an open project as the engine answers them (F-806): the run list (`list_runs`, paged) and each run's
// `run_status` (its status.json, its record and its last events). The runner owns every run (G7); the Studio only reads
// what it wrote, through the engine's project server, and keeps no run state of its own (G12).
//
// No Electron import: the unit tests drive it with a fake server.
import { RUN_EVENTS, type Listing, type RunDetail, type RunEntry, type RunFinish, type RunStateChange } from '../../shared/project.js';
import type { ProjectServerLike } from './server.js';

/** Pages the Studio reads of a paged list (100 entries each): at most 2000 runs. */
export const MAX_RUN_PAGES = 20;
const PAGE = 100;

type Obj = Record<string, unknown>;
const obj = (v: unknown): Obj => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Obj) : {});
const str = (v: unknown, d = ''): string => (typeof v === 'string' ? v : d);
const num = (v: unknown, d = 0): number => (typeof v === 'number' && Number.isFinite(v) ? v : d);
/** A number the engine gave, or null when it gave none (JSON null is how it writes a NaN). */
const finite = (v: unknown): number | null => (typeof v === 'number' && Number.isFinite(v) ? v : null);
const strOrNull = (v: unknown): string | null => (typeof v === 'string' ? v : null);

/** The runs states after which nothing changes (rcfd-project's RunState::is_terminal). */
export const TERMINAL_STATES: ReadonlySet<string> = new Set(['finished', 'failed', 'cancelled']);

export const isActive = (state: string): boolean => !TERMINAL_STATES.has(state);

/** One run of `list_runs`. */
export function runEntry(x: unknown): RunEntry {
  const o = obj(x);
  const metrics: Record<string, number> = {};
  for (const [k, v] of Object.entries(obj(o['metrics']))) if (typeof v === 'number' && Number.isFinite(v)) metrics[k] = v;
  return { id: str(o['id']), case: str(o['case']), created: str(o['created']), state: str(o['state']), step: num(o['step']), time: finite(o['time']), metrics };
}

export interface RunsRead {
  readonly runs: Listing<RunEntry>;
  readonly skippedRuns: readonly { readonly path: string; readonly message: string }[];
}

/** The project's runs, paged to the end (at most MAX_RUN_PAGES pages). Throws when the engine refuses. */
export async function readRuns(server: ProjectServerLike): Promise<RunsRead> {
  const items: RunEntry[] = [];
  const skippedRuns: { path: string; message: string }[] = [];
  let total = 0;
  let offset: number | null = 0;
  for (let page = 0; page < MAX_RUN_PAGES && offset !== null; page++) {
    const r = obj(await server.call('list_runs', { offset, limit: PAGE }));
    total = num(r['total']);
    for (const x of (r['runs'] as unknown[] | undefined) ?? []) items.push(runEntry(x));
    if (page === 0) for (const s of (r['skipped'] as unknown[] | undefined) ?? []) skippedRuns.push({ path: str(obj(s)['path']), message: str(obj(s)['message']) });
    offset = typeof r['next_offset'] === 'number' ? r['next_offset'] : null;
  }
  return { runs: { items, total, error: null }, skippedRuns };
}

/** `run_status` of one run, as the Studio keeps it. */
export function runDetail(id: string, answer: unknown, readAt: string): RunDetail {
  const a = obj(answer);
  if (!('status' in a)) throw new Error(`run_status of ${id} gave no status`);
  const st = obj(a['status']);
  let steady: boolean | null = null;
  let finish: RunFinish | null = null;
  const history: RunStateChange[] = [];
  for (const e of (a['events'] as unknown[] | undefined) ?? []) {
    const ev = obj(e);
    const kind = str(ev['kind'], str(ev['event']));
    if (kind === 'state') {
      history.push({ t: str(ev['t']), from: strOrNull(ev['from']), to: str(ev['to'], str(ev['state'])), reason: strOrNull(ev['reason']) });
    } else if (kind === 'solver') {
      const s = obj(ev['solver']);
      if (s['event'] === 'start' && typeof s['steady'] === 'boolean') steady = s['steady'];
      if (s['event'] === 'finish') {
        finish = { state: str(s['state']), iterations: finite(s['iterations']), elapsed: finite(s['elapsed_s']), reason: strOrNull(s['reason']) };
      }
    }
  }
  return {
    id: str(a['run'], id),
    state: str(st['state']),
    step: num(st['step']),
    time: finite(st['time']),
    updated: str(st['updated']),
    pid: finite(st['pid']),
    reason: strOrNull(st['reason']),
    exitCode: finite(st['exit_code']),
    solverVersion: str(a['solver_version']),
    steady,
    finish,
    history,
    readAt,
  };
}

/** Whether a run's last `run_status` answer may no longer be what its status.json holds. */
export function stale(entry: RunEntry, detail: RunDetail | undefined): boolean {
  return detail === undefined || detail.state !== entry.state || detail.step !== entry.step;
}

/** Reads `run_status` of one run. */
export async function readRunDetail(server: ProjectServerLike, id: string, now: () => string): Promise<RunDetail> {
  return runDetail(id, await server.call('run_status', { run: id, events: RUN_EVENTS }), now());
}
