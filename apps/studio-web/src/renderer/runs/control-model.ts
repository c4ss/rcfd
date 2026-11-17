// What the run list shows of the run-control requests (F-807), as pure functions of the view the main process pushes
// (src/shared/project.ts, RunRequest), so the unit tests check them without a window. Every state shown is the
// runner's, as the engine answered; a request only says what was asked and whether the runner has acted on it yet.
import { getRunAction as getF807Action } from '../../shared/actions/runs.js';
import { getFirstRunAction } from '../../shared/actions/first.js';
import { getSiteRunAction } from '../../shared/sites.js';
import type { RunEntry, RunRequest } from '../../shared/project.js';
import { clock } from './model.js';

/** F-813: run control's actions and the first path's (src/shared/actions/first.ts); F-848: the site's (src/shared/sites.ts). */
const getRunAction = (id: string) => getF807Action(id) ?? getFirstRunAction(id) ?? getSiteRunAction(id);

/**
 * The run list with the runs this window started that the engine's list does not hold yet: start_run answered with the
 * run id, its case and its state, and the next read of the list brings its record. Such an entry has no step, no time
 * and no result yet; `created` is when the Studio sent the request (the record's own time replaces it).
 */
export function withStartedRuns(items: readonly RunEntry[], requests: readonly RunRequest[] | undefined): readonly RunEntry[] {
  if (!requests || requests.length === 0) return items;
  const listed = new Set(items.map((r) => r.id));
  const extra: RunEntry[] = [];
  for (const q of requests) {
    if (q.outcome !== 'accepted' || q.run === null || listed.has(q.run) || getRunAction(q.action)?.expect !== 'new_run') continue;
    listed.add(q.run);
    extra.push({ id: q.run, case: q.case ?? '', created: q.at, state: q.state ?? 'queued', step: 0, time: null, metrics: {} });
  }
  return extra.length ? [...items, ...extra] : items;
}

/** The latest request of each run that the runner has not acted on yet (accepted, not settled). */
export function pendingByRun(requests: readonly RunRequest[] | undefined): Map<string, RunRequest> {
  const m = new Map<string, RunRequest>();
  for (const q of requests ?? []) {
    if (q.run === null || m.has(q.run)) continue;
    if (q.outcome === 'accepted' && !q.settled) m.set(q.run, q);
  }
  return m;
}

/** The latest request about a run (any outcome), for the selected run's panel. */
export function latestFor(requests: readonly RunRequest[] | undefined, run: string): RunRequest | null {
  return (requests ?? []).find((q) => q.run === run) ?? null;
}

const WAITING: Record<string, string> = {
  'project.run_case': 'the runner launches it when a slot is free',
  'project.queue_case': 'the runner launches it by priority when a slot is free',
  'project.rerun_run': 'the runner launches it when a slot is free',
  'project.pause_run': 'waiting for the runner to hold it',
  'project.resume_run': 'waiting for the runner to release it',
  'project.stop_run': 'waiting for the solver to stop after its current step',
  'project.cancel_run': 'waiting for the runner to take it out of the queue',
  'site.run_case': 'the runner sends it to the site',
};

const ASKED: Record<string, string> = {
  'project.run_case': 'Started',
  'project.queue_case': 'Queued',
  'project.rerun_run': 'Rerun started',
  'project.pause_run': 'Pause requested',
  'project.resume_run': 'Resume requested',
  'project.stop_run': 'Stop requested',
  'project.cancel_run': 'Cancel requested',
  'project.start_runner': 'Runner started',
  'site.run_case': 'Started on a site',
};

/** The sentence of a request in the run list: what was asked, when, and what it waits for. */
export function requestWords(q: RunRequest): string {
  const asked = ASKED[q.action] ?? getRunAction(q.action)?.title ?? q.action;
  if (q.outcome === 'refused') return `${asked.replace(/ requested$/, '')} refused: ${q.failure?.problem ?? 'the engine refused it.'}`;
  // ADR 0737: what the engine's store plan applies to a run the Studio started (the case set no store)
  const s = q.store;
  const stored =
    s?.applied === true
      ? `, its fields stored by the engine's plan${s.snapshots !== null ? ` (about ${s.snapshots} snapshots${s.estimatedBytes !== null ? `, ${mb(s.estimatedBytes)}` : ''})` : ''}`
      : '';
  const when = `${asked} from the Studio at ${clock(q.at)}${stored}`;
  if (q.settled) return `${when}; the runner has acted on it.`;
  if (q.slow) {
    return `${when}; the runner has not acted on it after 3 s. The project’s runner may not be running (the engine has no query that says whether it is): Start the runner, and it acts on the request when it starts.`;
  }
  return `${when}; ${WAITING[q.action] ?? 'waiting for the runner'}.`;
}

/** Splits `path=value` lines (one per line, blank lines left out) for the queue form's per-run settings. */
export function setEntries(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l !== '');
}

/** Bytes as MB, one decimal below 10 MB. */
function mb(bytes: number): string {
  const m = bytes / (1024 * 1024);
  return `${m < 10 ? m.toFixed(1) : Math.round(m)} MB`;
}
