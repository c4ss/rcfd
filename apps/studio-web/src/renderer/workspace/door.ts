// The run workspace's one door to a runner run's histories (F-810, window.studioRunData, src/shared/run.ts): the only
// renderer code that names the bridge (tests/unit/workspace.spec.ts), as data/door.ts is for window.studioData and
// project/door.ts for window.studioProject. Every answer is the bridge's own `{ok, result}` or `{ok: false, error}`.
//
// The run port is a main-process read through the open project's read-only project server (`read_residuals`,
// `read_project_file`), not a service method of SERVICE_METHODS, so it has no entry in the action registry (G16;
// reported with F-810, as F-805 and F-812 report theirs).
import type { BudgetKey, BudgetsOutcome, PageRect, RunComparison, RunLog, RunnerBudgets, RunReply, RunSeries, RunValuesAt, SavedPng } from '../../shared/run.js';

const bridge = () => window.studioRunData;

const noBridge = <T>(): RunReply<T> => ({ ok: false, error: { code: 'no_port', message: 'this page has no run bridge (window.studioRunData)' } });

/** The residual and monitor series of a run of the open project. */
export function runSeries(run: string): Promise<RunReply<RunSeries>> {
  const b = bridge();
  return b ? b.series(run) : Promise.resolve(noBridge());
}

/** The value of each series of a run at one step. */
export function runValuesAt(run: string, step: number): Promise<RunReply<RunValuesAt>> {
  const b = bridge();
  return b ? b.at(run, step) : Promise.resolve(noBridge());
}

/** F-815: the engine's compare_runs of the baseline run and the compared run. */
export function runComparison(run: string, other: string): Promise<RunReply<RunComparison>> {
  const b = bridge();
  return b ? b.compare(run, other) : Promise.resolve(noBridge());
}

// ---- F-837 (the same port; the run list and the field view reach it through here)

/** The end of a run's log.txt, its last events (warnings named) and whether its report.html is there. */
export function runLog(run: string): Promise<RunReply<RunLog>> {
  const b = bridge();
  return b ? b.log(run) : Promise.resolve(noBridge());
}

/** Opens the run's output/report.html in the system's program for HTML. */
export function openRunReport(run: string): Promise<RunReply<{ readonly path: string }>> {
  const b = bridge();
  return b ? b.openReport(run) : Promise.resolve(noBridge());
}

/** The runner's budgets of the open project. */
export function runnerBudgets(): Promise<RunReply<RunnerBudgets>> {
  const b = bridge();
  return b ? b.budgets() : Promise.resolve(noBridge());
}

/** Sets (number) or clears (null) budgets, if the file still has `base`. */
export function setRunnerBudgets(values: Readonly<Partial<Record<BudgetKey, number | null>>>, base: string | null): Promise<RunReply<BudgetsOutcome>> {
  const b = bridge();
  return b ? b.setBudgets(values, base) : Promise.resolve(noBridge());
}

/** Puts back the budgets file as it was before this window's last write. */
export function undoRunnerBudgets(force: boolean): Promise<RunReply<BudgetsOutcome>> {
  const b = bridge();
  return b ? b.undoBudgets(force) : Promise.resolve(noBridge());
}

/** Saves a picture of a rectangle of the page as a PNG the user names. */
export function savePagePng(rect: PageRect, name: string): Promise<RunReply<SavedPng>> {
  const b = bridge();
  return b ? b.savePng(rect, name) : Promise.resolve(noBridge());
}
