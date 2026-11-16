// The run workspace's way to a runner run's histories (F-810): the residual and monitor series of `runs/<id>/` and the
// values of one step, as the engine's project server reads them from the run's residuals.jsonl (`read_residuals`, ADR
// 0173), with the monitors' units from the run's output/monitors.json (`read_project_file`). Shared by all three
// processes, so it uses neither Node nor DOM types.
//
// A fourth MessagePort (`studio.run-port`, after the service, project and case ports): the renderer asks for a run by
// its id only; the main process checks every message (src/main/run/bridge.ts) and asks the open project's read-only
// project server. The renderer never names a tool or a file.

/** The one IPC channel main uses to hand the preload script the run port (main -> renderer only). */
export const RUN_PORT_CHANNEL = 'studio.run-port';

/**
 * Samples per series the Studio asks `read_residuals` for. Every reply stays under the engine's text budget (36 000
 * bytes, crates/rcfd-workspace TEXT_BUDGET): one series per call, 800 samples of at most 34 characters each. Over the
 * budget the engine would write the series to a file in the run folder, which a read-only Studio must not cause (G12).
 * A series with more samples comes back decimated by the engine (first, last and each bucket's minimum and maximum).
 */
export const SERIES_POINTS = 800;

/** Largest output/monitors.json the Studio reads for the units (its pages are 32 000 bytes). */
export const UNITS_MAX_BYTES = 1_000_000;

/** What a series is: a residual of an equation, a monitor (a report value), or another per-step value of the solver. */
export type SeriesKind = 'residual' | 'monitor' | 'other';

export interface RunSeriesEntry {
  /**
   * The name the plots use, as the engine's monitors.json and `monitors.get` name it: `continuity`, `res_<eq>` for the
   * other residuals, the monitor's own name for a monitor.
   */
  readonly name: string;
  /** The engine's name in `read_residuals` (`residuals.u`, `monitors.lid_drag`, `correctors`). */
  readonly source: string;
  readonly kind: SeriesKind;
  /** The unit from output/monitors.json; null when the Studio has none (see RunSeries.unitsNote). Residuals have "". */
  readonly unit: string | null;
  /** Steps (the engine's x). */
  readonly x: readonly number[];
  /** Values; NaN where the engine wrote null. */
  readonly y: readonly number[];
  /** The engine decimated this series to SERIES_POINTS samples. */
  readonly decimated: boolean;
}

/** A run's histories as `series` answers. */
export interface RunSeries {
  readonly run: string;
  /** Lines of residuals.jsonl the engine read, and lines it could not read. */
  readonly lines: number;
  readonly skippedLines: number;
  readonly maxPoints: number;
  readonly series: readonly RunSeriesEntry[];
  /** Why the units are missing (no monitors.json yet, too large, unreadable), or null when they were read. */
  readonly unitsNote: string | null;
  /** When the main process read it (RFC 3339). */
  readonly readAt: string;
}

/** The values of every series at one step (`at`), read with `read_residuals {since_step: step}`. */
export interface RunValuesAt {
  readonly run: string;
  readonly step: number;
  /** By plot name; a series with no sample at that step is missing here. */
  readonly values: Readonly<Record<string, number>>;
}

export interface RunError {
  /** `bad_request` (wrong shape), `no_project` (no project open), `engine` (the project server refused or failed). */
  readonly code: 'bad_request' | 'no_project' | 'engine' | 'no_port' | 'timeout';
  readonly message: string;
}

export type RunReply<T = unknown> = { readonly ok: true; readonly result: T } | { readonly ok: false; readonly error: RunError };

/** Preload -> main. */
export type RunRequest =
  | { readonly op: 'series'; readonly id: number; readonly run: string }
  | { readonly op: 'at'; readonly id: number; readonly run: string; readonly step: number };

export type RunOp = RunRequest['op'];
export const RUN_OPS = ['series', 'at'] as const satisfies readonly RunOp[];

/** Main -> preload. */
export type RunPortMessage = { readonly type: 'reply'; readonly id: number; readonly reply: RunReply };

/** A run id as the runner writes them (`r-20261028T152641-3900`): letters, digits, `.`, `_` and `-`, not `.` or `..`. */
export const RUN_ID = /^(?!\.{1,2}$)[A-Za-z0-9._-]{1,128}$/;

/** What the preload script puts on `window.studioRunData`. Each function's arguments are checked again in main. */
export interface RunBridge {
  /** The residual and monitor series of a run of the open project. */
  series(run: string): Promise<RunReply<RunSeries>>;
  /** The value of each series at one step of a run of the open project. */
  at(run: string, step: number): Promise<RunReply<RunValuesAt>>;
}
