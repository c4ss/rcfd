// A learner's first path (F-813), as pure functions of the view the main process pushes (src/shared/project.ts): which
// step the project is at, and the plain words of the states, numbers and terms on the way. The step comes only from what
// the engine lists (the project's cases and runs, G12): the path keeps nothing of its own, so a case or run made by
// Claude Code or the CLI moves it on the same way. No DOM types, so the unit tests check it without a window.
import type { ProjectView } from '../../shared/project.js';
import { FIRST_CASE, FIRST_RUN_EXAMPLE } from '../../shared/actions/first.js';
import { runRows, shownState, type RunRow } from '../runs/model.js';
import { withStartedRuns } from '../runs/control-model.js';
import type { MarkState } from '../ui/pictogramPaths.js';

/** The four stations of the path, in order. */
export const STATIONS = [
  { id: 'make', title: 'Make the example' },
  { id: 'run', title: 'Run it' },
  { id: 'watch', title: 'Watch it work' },
  { id: 'result', title: 'See the result' },
] as const;
export type StationId = (typeof STATIONS)[number]['id'];

export type FirstStep =
  | { readonly at: 'make' }
  | { readonly at: 'run'; readonly casePath: string }
  | { readonly at: 'watch'; readonly casePath: string; readonly run: RunRow }
  | { readonly at: 'result'; readonly casePath: string; readonly run: RunRow };

const norm = (p: string): string => p.replace(/\\/g, '/');
/** Whether a run's case (project-relative, or an absolute path as some records hold it) is the path's case. */
export const isFirstCase = (c: string): boolean => {
  const p = norm(c);
  return p === FIRST_CASE || p.endsWith(`/${FIRST_CASE}`);
};

const ENDED = new Set(['finished', 'failed', 'cancelled']);
export const ended = (state: string): boolean => ENDED.has(state);

/**
 * The step the open project is at, or null when it is past the first path (it holds another case, or runs of another
 * case) or when its lists could not be read (the home shows that failure itself).
 */
export function firstStep(view: ProjectView): FirstStep | null {
  if (view.cases.error !== null || view.runs.error !== null) return null;
  if (view.cases.total > view.cases.items.length || view.runs.total > view.runs.items.length) return null;
  const other = view.cases.items.filter((c) => c.path !== FIRST_CASE);
  if (other.length > 0) return null;
  const items = withStartedRuns(view.runs.items, view.runRequests);
  if (items.some((r) => !isFirstCase(r.case))) return null;
  const hasCase = view.cases.items.length > 0;
  if (!hasCase && items.length === 0) return { at: 'make' };
  if (!hasCase) return null;
  if (items.length === 0) return { at: 'run', casePath: FIRST_CASE };
  const rows = runRows(items, view.runDetails, view.runDetailErrors);
  // the newest run of the case: run ids sort by their creation time (r-<yyyymmddThhmmss>-<hex>)
  const run = [...rows].sort((a, b) => (a.created < b.created ? 1 : a.created > b.created ? -1 : a.id < b.id ? 1 : -1))[0]!;
  return ended(run.state) ? { at: 'result', casePath: FIRST_CASE, run } : { at: 'watch', casePath: FIRST_CASE, run };
}

/** The index of a station (0..3). */
export const stationIndex = (s: StationId): number => STATIONS.findIndex((x) => x.id === s);

/** The latest first-path run request of this window, if any (newest first in the view). */
export function firstRequest(view: ProjectView) {
  return (view.runRequests ?? []).find((q) => q.action === FIRST_RUN_EXAMPLE) ?? null;
}

/**
 * A run's state in plain words with its pictogram (G8: shape and word). The precise word (DESIGN.md's state word, the
 * runner's state) is one step away, in the step's terms.
 */
export function plainState(run: RunRow): { readonly mark: MarkState | null; readonly word: string; readonly precise: string } {
  const s = shownState(run);
  switch (s) {
    case 'queued':
      return { mark: s, word: 'Waiting its turn', precise: 'queued: the runner has it and launches it when a slot is free' };
    case 'running':
      return { mark: s, word: 'Working', precise: 'running: the solver is computing' };
    case 'paused':
      return { mark: s, word: 'On hold', precise: 'paused: held between two steps' };
    case 'converged':
      return { mark: s, word: 'Done, settled', precise: 'converged: finished with the residual targets met' };
    case 'finished':
      if (run.detail === null) return { mark: s, word: 'Done', precise: 'finished: the runner’s record of how it ended is being read' };
      return { mark: s, word: 'Done, not settled', precise: 'finished, not converged: it stopped at its limit or end time' };
    case 'failed':
      return { mark: s, word: 'Failed', precise: 'failed: the run ended with an error' };
    case 'cancelled':
      return { mark: s, word: 'Stopped', precise: 'cancelled: stopped on request before it ended' };
    default:
      return { mark: null, word: run.state, precise: `${run.state}: a state this Studio has no word for` };
  }
}

/** The plain sentence of a run's outcome, from the engine's numbers only (G3). */
export function outcomeSentence(run: RunRow): string {
  const s = shownState(run);
  const n = run.detail?.finish?.iterations ?? run.step;
  const rounds = `${n} ${n === 1 ? 'round' : 'rounds'} of calculation`;
  switch (s) {
    case 'converged':
      return `Done. The calculation settled after ${rounds}: from one round to the next, the answer stopped changing by more than the case allows.`;
    case 'finished':
      if (run.detail === null) return 'Done. Reading what the engine recorded about how it ended…';
      return `Done, but not settled: it stopped after ${rounds}, at the limit the case sets, while the answer was still changing. What is shown is where it stopped.`;
    case 'failed':
      return `The run failed after ${rounds}. Nothing below is a result; the engine’s words say why.`;
    case 'cancelled':
      return `The run was stopped after ${rounds}, before it ended. What is shown is where it stopped.`;
    default:
      return `The run is ${run.state}.`;
  }
}

/** Plain names of the fields the engine's default store keeps (rcfd_api::fieldstore::DEFAULT_FIELDS); others by name. */
export const FIELD_WORDS: Readonly<Record<string, string>> = {
  ux: 'Velocity to the right',
  uy: 'Velocity upwards',
  uz: 'Velocity out of the screen',
  p: 'Pressure',
};
export const fieldWord = (name: string): string => FIELD_WORDS[name] ?? name;

/** The folder of a run's stored fields, in the folder's own separator style. */
export function fieldsFolder(projectFolder: string, run: string): string {
  const sep = projectFolder.includes('\\') ? '\\' : '/';
  const base = projectFolder.replace(/[\\/]+$/, '');
  return [base, 'runs', run, 'fields'].join(sep);
}

/** The shell command that starts the same run again (G17): the CLI equivalent plus the plan's entries the engine answered. */
export function sameRunCommand(casePath: string, folder: string, planSet: readonly string[]): string[] {
  return ['rcfd-runner', 'start', casePath, ...planSet.flatMap((s) => ['--set', s]), '--project', folder];
}
