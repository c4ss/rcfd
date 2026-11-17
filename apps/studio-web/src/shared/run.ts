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

/**
 * F-815: two runs' metrics side by side as the engine's `compare_runs` gives them (the metrics of each run's
 * metrics.json, the differences to the first run), for the baseline A and the compared run B.
 */
export interface RunComparison {
  readonly baseline: string;
  readonly compared: string;
  /** Each metric: A's value, B's value and the engine's difference B - A; null where the engine gives none. */
  readonly metrics: readonly { readonly name: string; readonly a: number | null; readonly b: number | null; readonly difference: number | null }[];
  /** Metrics the engine left out of its reply (its text budget). */
  readonly omitted: number;
}

export interface RunError {
  /**
   * `bad_request` (wrong shape), `no_project` (no project open), `engine` (the project server refused or failed); F-837:
   * `not_found` (no report.html), `io` (a file the main process reads or writes, or the system's program for it, failed).
   */
  readonly code: 'bad_request' | 'no_project' | 'engine' | 'no_port' | 'timeout' | 'not_found' | 'io';
  readonly message: string;
}

export type RunReply<T = unknown> = { readonly ok: true; readonly result: T } | { readonly ok: false; readonly error: RunError };

/** Preload -> main. */
export type RunRequest =
  | { readonly op: 'series'; readonly id: number; readonly run: string }
  | { readonly op: 'at'; readonly id: number; readonly run: string; readonly step: number }
  | { readonly op: 'compare'; readonly id: number; readonly run: string; readonly other: string }
  // F-837
  | { readonly op: 'log'; readonly id: number; readonly run: string }
  | { readonly op: 'openReport'; readonly id: number; readonly run: string }
  | { readonly op: 'budgets'; readonly id: number }
  | { readonly op: 'setBudgets'; readonly id: number; readonly values: Readonly<Partial<Record<BudgetKey, number | null>>>; readonly base: string | null }
  | { readonly op: 'undoBudgets'; readonly id: number; readonly force: boolean }
  | { readonly op: 'savePng'; readonly id: number; readonly rect: PageRect; readonly name: string };

export type RunOp = RunRequest['op'];
/** The operations of F-810. */
export const RUN_OPS = ['series', 'at'] as const satisfies readonly RunOp[];
/** The operation F-815 added (kept apart so that F-810's list stays as its tests read it). */
export const RUN_COMPARE_OPS = ['compare'] as const satisfies readonly RunOp[];
/** The operations F-837 added (kept apart, as F-815's). */
export const RUN_EXTRA_OPS = ['log', 'openReport', 'budgets', 'setBudgets', 'undoBudgets', 'savePng'] as const satisfies readonly RunOp[];
/** The file name Save PNG proposes: letters, digits, `.`, `_` and `-`, ending in .png. */
export const PNG_NAME = /^[A-Za-z0-9._-]{1,120}\.png$/;

// ---- F-837: a run's log tail, its events with the solver's warnings, its report, the runner's budgets and the field view's PNG

/** Bytes of the end of runs/<id>/log.txt the Studio reads (in one page of the engine's read_project_file). */
export const LOG_TAIL_BYTES = 16_000;
/** Lines of that tail it keeps (the last ones). */
export const LOG_LINES = 200;
/** Events of runs/<id>/events.jsonl it asks run_status for (the last ones): the most run_status gives (its schema's maximum). */
export const LOG_EVENTS = 50;

/** One line of a run's events.jsonl, in words: a state change, a warning (the runner's or the solver's), a stop, or another event. */
export interface RunEventLine {
  /** When the runner wrote it (RFC 3339); empty when the line has no time. */
  readonly t: string;
  /** The event's `kind` (`state`, `warning`, `stop`, `solver`, `estimate`, `launched`, ...). */
  readonly kind: string;
  /** `state queued -> running: launched`, `warning: <message>`, `stop: <reason>`, or the event's JSON cut at 160 characters. */
  readonly text: string;
  /** A warning of the runner (`kind: warning`) or of the solver (`kind: solver`, `solver.event: warning`). */
  readonly warning: boolean;
}

/** What `log` answers: the end of runs/<id>/log.txt, the last events, and whether runs/<id>/output/report.html is there. */
export interface RunLog {
  readonly run: string;
  /** The tail of log.txt (null when the run has none yet, or it could not be read: `logError`). */
  readonly log: { readonly path: string; readonly size: number; readonly lines: readonly string[]; readonly cut: boolean } | null;
  readonly logError: string | null;
  /** The last LOG_EVENTS events, oldest first. */
  readonly events: readonly RunEventLine[];
  readonly eventsError: string | null;
  /** runs/<id>/output/report.html (the solver's report, rcfd-api's standard result files), or null when it is not there. */
  readonly report: { readonly path: string; readonly size: number } | null;
  readonly readAt: string;
}

/**
 * The runner's budgets, `.rcfd/runner-config.json` (rcfd-runner's `Config`, ADR 0123 section 6): every key optional,
 * no other key allowed (the runner refuses the file then). The runner reads the file when it starts.
 */
export const BUDGET_KEYS = ['max_runs', 'cpu_threads', 'mem_budget_mb', 'mem_base_mb', 'mem_kb_per_cell', 'poll_ms', 'adopt_probe_ms', 'cancel_grace_ms'] as const;
export type BudgetKey = (typeof BUDGET_KEYS)[number];

/** A budget's kind as the runner reads it: a positive whole number, a whole number, or a number not below zero. */
export const BUDGET_KIND: Readonly<Record<BudgetKey, 'positive' | 'count' | 'real'>> = {
  max_runs: 'positive',
  cpu_threads: 'positive',
  mem_budget_mb: 'count',
  mem_base_mb: 'real',
  mem_kb_per_cell: 'real',
  poll_ms: 'positive',
  adopt_probe_ms: 'count',
  cancel_grace_ms: 'count',
};

/** What `budgets` answers. */
export interface RunnerBudgets {
  /** `.rcfd/runner-config.json`, project-relative. */
  readonly path: string;
  /** False when the file is not there: the runner uses its defaults. */
  readonly exists: boolean;
  /** The file's values (keys the file sets); empty when it is absent or unreadable. */
  readonly values: Readonly<Partial<Record<BudgetKey, number>>>;
  /** sha256 of the file's bytes as read (null: no file): the base of the next write, which refuses a file changed since. */
  readonly hash: string | null;
  /** Why the runner would refuse the file (it is not JSON, an unknown key, a value of the wrong kind), or null. */
  readonly problem: string | null;
  /**
   * The budgets the runner used when it last started (`.rcfd/runner.json`, `config`), and where it took them from
   * (`config_source`); null when no runner has started in this project.
   */
  readonly inUse: { readonly values: Readonly<Partial<Record<BudgetKey, number>>>; readonly source: string; readonly started: string } | null;
  /** True when this window wrote the file last and that write can be undone. */
  readonly undo: boolean;
}

export type BudgetsOutcome =
  | { readonly kind: 'written'; readonly budgets: RunnerBudgets }
  /** Another writer changed the file since it was read: nothing written; `budgets` is the file now. */
  | { readonly kind: 'conflict'; readonly budgets: RunnerBudgets }
  /** A value the runner would refuse, or nothing to undo: nothing written. */
  | { readonly kind: 'refused'; readonly message: string };

/** A rectangle of the page in CSS pixels (the field view's frame and its legend, for Save PNG). */
export interface PageRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** What `savePng` answers: where the picture was written, or null when the dialog was cancelled. */
export interface SavedPng {
  readonly path: string | null;
  readonly width: number;
  readonly height: number;
  readonly bytes: number;
}

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
  /** F-815: `compare_runs` of the baseline `run` and the compared run `other`. */
  compare(run: string, other: string): Promise<RunReply<RunComparison>>;
  /** F-837: the end of the run's log.txt, its last events and whether its report.html is there. */
  log(run: string): Promise<RunReply<RunLog>>;
  /** F-837: opens the run's output/report.html in the system's program for it (a browser). */
  openReport(run: string): Promise<RunReply<{ readonly path: string }>>;
  /** F-837: the project's runner budgets (.rcfd/runner-config.json) and the ones the runner last started with. */
  budgets(): Promise<RunReply<RunnerBudgets>>;
  /** F-837: writes the budgets (null removes a key: the runner's default), only if the file still has `base`. */
  setBudgets(values: Readonly<Partial<Record<BudgetKey, number | null>>>, base: string | null): Promise<RunReply<BudgetsOutcome>>;
  /** F-837: puts back the file as it was before this window's last write (`force`: even when another writer changed it since). */
  undoBudgets(force: boolean): Promise<RunReply<BudgetsOutcome>>;
  /** F-837: saves a picture of a rectangle of the page (the field view) as a PNG file the user names in a dialog. */
  savePng(rect: PageRect, name: string): Promise<RunReply<SavedPng>>;
}
