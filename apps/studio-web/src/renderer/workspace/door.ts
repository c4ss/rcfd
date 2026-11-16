// The run workspace's one door to a runner run's histories (F-810, window.studioRunData, src/shared/run.ts): the only
// renderer code that names the bridge (tests/unit/workspace.spec.ts), as data/door.ts is for window.studioData and
// project/door.ts for window.studioProject. Every answer is the bridge's own `{ok, result}` or `{ok: false, error}`.
//
// The run port is a main-process read through the open project's read-only project server (`read_residuals`,
// `read_project_file`), not a service method of SERVICE_METHODS, so it has no entry in the action registry (G16;
// reported with F-810, as F-805 and F-812 report theirs).
import type { RunComparison, RunReply, RunSeries, RunValuesAt } from '../../shared/run.js';

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
