// The renderer's way to the open project folder (F-805): what the project home shows, the messages that cross the
// project port, and the bridge the preload script puts on `window.studioProject`. Shared by all three processes, so it
// uses neither Node nor DOM types.
//
// The Studio reads a project folder through the engine (docs/studio-guardrails.md G2, G12): the main process starts the
// engine's project server, `rcfd-mcp --project <folder> --read-only` (the workspace of ADR 0173, its tools in
// docs/mcp-tools.md), and asks it `project_info`, `list_runs`, `list_meshes` and `read_feed`. The server is read-only:
// this item writes nothing to the folder. The renderer never names a tool or a file: it asks for the operations below,
// and the main process checks every message on the port (src/main/project/bridge.ts). The page can open a folder only
// through the folder dialog of the main process or from the recent list the main process keeps.
//
// F-835: a new project is made by the engine, never by the Studio (G4, G12): `create` runs the engine's
// `rcfd project init <folder>` (ADR 0731; the MCP equivalent is `create_project`) on a folder chosen in the main
// process's folder dialog, or on the folder the engine has just refused as no project, and then opens it as `open` does.
import type { CancelOptions } from './work.js';

/** The one IPC channel main uses to hand the preload script the project port (main -> renderer only). */
export const PROJECT_PORT_CHANNEL = 'studio.project-port';

/** The tools of the engine's project server that the main process calls; all of them only read. `run_status` (F-806):
 * one run's status.json, its last events and its record, for the runs table. `read_residuals` and `read_project_file`
 * (F-810): a run's residual and monitor histories and its output/monitors.json, for the run workspace (src/shared/run.ts).
 * `runner_config` (F-837, ADR 0733): the runner's budgets, which the runner itself writes (`set_runner_config`, on the
 * run-control server: BUDGETS_WRITE_TOOL). `hash_project_file` (F-828, ADR 0734 P4): the hash of the mesh a run names
 * now, against its run.json mesh_hash. */
export const PROJECT_TOOLS = ['project_info', 'list_runs', 'list_meshes', 'read_feed', 'run_status', 'read_residuals', 'read_project_file', 'compare_runs', 'runner_config', 'hash_project_file'] as const;
export type ProjectTool = (typeof PROJECT_TOOLS)[number];

/** F-837 (ADR 0733): the one write tool of the runner's budgets, called on the run-control server only. */
export const BUDGETS_WRITE_TOOL = 'set_runner_config';
export type BudgetsWriteTool = typeof BUDGETS_WRITE_TOOL;

/** How often the main process asks the engine's change feed what changed (ms). */
export const FEED_POLL_MS = 400;

/**
 * F-806: while a run is queued, running or paused, how often the main process reads the run list again (ms). The feed
 * reports state changes (ADR 0188: `state`, `warning` and `stop` events), not progress; the step of a running run comes
 * from its status.json, which the runner replaces as the solver goes.
 */
export const RUN_POLL_MS = 1000;

/** Events of a run's events.jsonl the Studio asks `run_status` for (its state changes and the solver's start event). */
export const RUN_EVENTS = 50;

/** Where the engine's project server comes from (src/main/project/config.ts); `bundled`: an installed Studio's own. */
export type EngineStatus =
  | { readonly configured: true; readonly path: string; readonly from: 'RCFD_STUDIO_MCP' | 'RCFD_STUDIO_RCFD' | 'bundled' }
  | { readonly configured: false; readonly reason: string };

/** One folder of the recent list: the folder and when it was last opened. Nothing of the project itself is kept. */
export interface RecentProject {
  readonly path: string;
  /** RFC 3339, when the Studio last opened it. */
  readonly opened: string;
  /** False when the folder is no longer there (checked when the list is read). */
  readonly exists: boolean;
}

/** `project_info` of the engine (docs/mcp-tools.md, Project tools). */
export interface ProjectInfo {
  readonly name: string;
  readonly root: string;
  readonly schema_version: number;
  readonly created: string;
  readonly solver_version_created: string;
  readonly migrated_from: number | null;
  readonly counts: {
    readonly cases: number;
    readonly meshes: number;
    readonly designs: number;
    readonly notes: number;
    readonly runs: number;
    readonly skipped_runs: number;
  };
  readonly runs_by_state: Readonly<Record<string, number>>;
  readonly issues: readonly { readonly path: string; readonly message: string }[];
  readonly issues_total: number;
}

/** A case file under `cases/`, with what the engine says about it. */
export interface CaseEntry {
  /** Project-relative, `/`-separated (`cases/cavity.toml`). */
  readonly path: string;
  /** Runs of the project whose case is this file (`list_runs`). */
  readonly runs: number;
  /** Mesh entries of `meshes/` this case's `mesh.file` names (`list_meshes`, `used_by`). */
  readonly meshes: readonly string[];
  /** The engine could not read this case (`list_meshes`, `unread`): its message. */
  readonly unread: string | null;
}

/** An entry of `list_meshes`. */
export interface MeshEntry {
  readonly path: string;
  /** `file`, or `openfoam` for a folder holding a polyMesh. */
  readonly kind: string;
  readonly bytes: number;
  readonly files: number;
  readonly used_by: readonly string[];
}

/** A run of `list_runs`. */
export interface RunEntry {
  readonly id: string;
  readonly case: string;
  readonly created: string;
  /** queued, running, paused, finished, failed or cancelled (the runner's states). */
  readonly state: string;
  readonly step: number;
  /**
   * The simulated time of the last step as status.json holds it; null when the engine gives none (a `null` time, which
   * it writes for a NaN). A steady run's solver reports 0 here: it has no simulated time (see RunDetail.steady).
   */
  readonly time: number | null;
  /** The final report values (`list_runs` `metrics`: `reports` of metrics.json); empty while the run has none. */
  readonly metrics: Readonly<Record<string, number>>;
}

/** A state change of a run, from its events.jsonl as `run_status` returns it. */
export interface RunStateChange {
  /** When the runner wrote it (RFC 3339). */
  readonly t: string;
  readonly from: string | null;
  readonly to: string;
  readonly reason: string | null;
}

/** The solver's finish event of a run (`kind: solver`, `solver.event: finish`), as the runner forwarded it. */
export interface RunFinish {
  /** The solver's own word for how it ended: `converged`, `not_converged`, `finished`, `interrupted`, ... */
  readonly state: string;
  readonly iterations: number | null;
  /** Solver wall time, s (`elapsed_s`). */
  readonly elapsed: number | null;
  readonly reason: string | null;
}

/** One run as `run_status` answered (F-806): its status.json, record and last events. */
export interface RunDetail {
  readonly id: string;
  /** status.json: state, step, time (null when the engine has none), updated, pid, reason, exit code. */
  readonly state: string;
  readonly step: number;
  readonly time: number | null;
  readonly updated: string;
  readonly pid: number | null;
  readonly reason: string | null;
  readonly exitCode: number | null;
  readonly solverVersion: string;
  /**
   * The solver's start event says whether the run is steady (`solver.steady`). Null when the events read hold no start
   * event (a run not launched yet, or one whose start is older than the last RUN_EVENTS events).
   */
  readonly steady: boolean | null;
  readonly finish: RunFinish | null;
  /** State changes among the last RUN_EVENTS events, oldest first. */
  readonly history: readonly RunStateChange[];
  /**
   * F-837: the residuals of the last line of residuals.jsonl (run_status `last_residual.residuals`), by the engine's
   * equation name (`u`, `p`, `continuity`, ...); absent when the run has written none.
   */
  readonly lastResidual?: Readonly<Record<string, number>> | undefined;
  /** ADR 0734 P2: the unit of `time` as run_status names it (`s`); absent from an engine that names none. */
  readonly timeUnit?: string | undefined;
  /** When the Studio read it (RFC 3339). */
  readonly readAt: string;
}

/** A file under `notes/`. */
export interface NoteEntry {
  readonly path: string;
}

/** F-837: a top-level entry of `designs/` (a file, or a folder of a design's files), as the engine counts them. */
export interface DesignEntry {
  readonly path: string;
  readonly folder: boolean;
}

/** One list of the project home: its entries, how many there are in all, and why it could not be read, if so. */
export interface Listing<T> {
  readonly items: readonly T[];
  /** Entries in all (more than `items` when the list was cut at the Studio's limit). */
  readonly total: number;
  /** Why the list could not be read; `items` is then empty. */
  readonly error: string | null;
}

/** An entry of the engine's change feed (`read_feed`, ADR 0188), with the time the Studio received it. */
export interface ChangeEntry {
  readonly n: number;
  /** `journal`, `run` or `external`. */
  readonly source: string;
  readonly kind: string;
  /** The engine's own sentence: who changed what and why. */
  readonly text: string;
  readonly path: string | null;
  readonly run: string | null;
  /** When the change was written, if the engine knows (journal lines and run events). */
  readonly t: string | null;
  /** When the Studio received the entry (RFC 3339). */
  readonly seen: string;
}

/** The live link to the folder: the feed is being read, or why it is not. */
export type Liveness =
  | { readonly live: true; readonly since: string }
  | { readonly live: false; readonly since: string; readonly reason: string; readonly serverStopped: boolean };

/** The open project as the engine answered it, last read at `readAt`. */
export interface ProjectView {
  /** The folder as opened. */
  readonly folder: string;
  /** The engine project server's version (MCP `serverInfo.version`) and its path. */
  readonly engine: { readonly name: string; readonly version: string; readonly path: string };
  readonly info: ProjectInfo | null;
  readonly infoError: string | null;
  readonly cases: Listing<CaseEntry>;
  readonly meshes: Listing<MeshEntry>;
  readonly runs: Listing<RunEntry>;
  readonly notes: Listing<NoteEntry>;
  /** F-837: the top-level entries of designs/ (absent from a view made before F-837). */
  readonly designs?: Listing<DesignEntry>;
  /** Run folders the engine could not read (`list_runs`, `skipped`). */
  readonly skippedRuns: readonly { readonly path: string; readonly message: string }[];
  /**
   * F-806: `run_status` of each listed run, by run id (read after each read of the list; a run whose state and step
   * did not change keeps its last answer). A run missing here has not been read yet.
   */
  readonly runDetails: Readonly<Record<string, RunDetail>>;
  /** F-806: runs whose `run_status` failed, with the engine's message. */
  readonly runDetailErrors: Readonly<Record<string, string>>;
  /**
   * F-807: the run-control actions dispatched from this window since the project was opened, newest first (at most
   * RUN_REQUESTS_KEPT), each with the engine's answer and whether the runner has acted on it yet. The Studio's own
   * action log, not project state: the runs themselves are the runner's (G7, G12).
   */
  readonly runRequests?: readonly RunRequest[];
  /** Feed entries since the project was opened, newest first (at most MAX_CHANGES). */
  readonly changes: readonly ChangeEntry[];
  readonly openedAt: string;
  /** When the lists were last read from the engine. */
  readonly readAt: string;
  readonly liveness: Liveness;
}

/** Feed entries the Studio keeps for the home (the engine keeps 1000). */
export const MAX_CHANGES = 100;

/** Why a folder could not be opened, in words that name the problem and the next step (G6, G19). */
export interface OpenFailure {
  /** F-835: `not_created`, the engine's `rcfd project init` refused the folder or failed (its message is `detail`). */
  readonly kind: 'not_configured' | 'not_found' | 'not_a_project' | 'engine' | 'not_created';
  readonly folder: string;
  readonly message: string;
  /** The engine's own message, when it gave one. */
  readonly detail: string | null;
  /**
   * F-813: for `not_a_project`, what the folder holds as the main process saw it (names counted, nothing read), and
   * whether the home is watching for the project file to appear (it then opens the folder by itself).
   */
  readonly folderState?: FolderState;
}

/** F-813: a folder that is not a project yet. */
export interface FolderState {
  /** Entries at the folder's top level; null when it could not be listed. */
  readonly entries: number | null;
  /** True while the home checks, every FIRST_WATCH_MS, whether rcfd-project.json has appeared. */
  readonly watching: boolean;
}

/** F-813: how often the home checks whether a folder that was not a project has become one (ms). */
export const FIRST_WATCH_MS = 1000;

export interface ProjectState {
  readonly engine: EngineStatus;
  readonly recent: readonly RecentProject[];
  /** The folder being opened, while the engine starts. */
  readonly opening: string | null;
  readonly failure: OpenFailure | null;
  readonly project: ProjectView | null;
}

// ---- Messages on the project port (preload <-> main). The main process validates every request (src/main/project/bridge.ts).

/** Preload -> main. `id` pairs a request with its reply. */
export type ProjectRequest =
  | { readonly op: 'state'; readonly id: number }
  | { readonly op: 'pick'; readonly id: number }
  | { readonly op: 'open'; readonly id: number; readonly path: string }
  | { readonly op: 'forget'; readonly id: number; readonly path: string }
  | { readonly op: 'close'; readonly id: number }
  | { readonly op: 'refresh'; readonly id: number }
  | { readonly op: 'dismiss'; readonly id: number }
  /** F-807: a run-control action (src/shared/actions/runs.ts) with its parameters; the reply's result is a RunActionResult. */
  | { readonly op: 'action'; readonly id: number; readonly action: string; readonly params: Readonly<Record<string, unknown>> }
  /**
   * F-832 (G21): cancels the opening of a folder that is going (`opening` in the state): the project open before stays
   * open, the recent list is not touched, and a project server started for it is stopped. The reply's result is
   * `{cancelled: boolean}` (false when nothing was being opened).
   */
  | { readonly op: 'cancel'; readonly id: number }
  /**
   * F-835: makes a project with the engine (`rcfd project init <folder>`, ADR 0731) and opens it. `pick`: in a folder
   * chosen in the main process's folder dialog (a new or an existing one); `here`: in the folder of the failed open the
   * engine refused as no project. The reply's result is a PickResult (null when the dialog was cancelled).
   */
  | { readonly op: 'create'; readonly id: number; readonly where: CreateWhere };

/** F-835: where `create` makes the project. */
export const CREATE_WHERE = ['pick', 'here'] as const;
export type CreateWhere = (typeof CREATE_WHERE)[number];

export type ProjectOp = ProjectRequest['op'];
export const PROJECT_OPS = ['state', 'pick', 'open', 'forget', 'close', 'refresh', 'dismiss', 'action', 'cancel', 'create'] as const satisfies readonly ProjectOp[];

export interface ProjectError {
  /** `bad_request` (the message has the wrong shape), `not_recent` (open of a folder the recent list does not hold). */
  /** F-835: `no_folder` (create `here` with no folder the engine refused as no project). */
  readonly code: 'bad_request' | 'not_recent' | 'no_port' | 'timeout' | 'no_folder';
  readonly message: string;
}

export type ProjectReply<T = unknown> = { readonly ok: true; readonly result: T } | { readonly ok: false; readonly error: ProjectError };

/** Main -> preload. */
export type ProjectPortMessage =
  | { readonly type: 'reply'; readonly id: number; readonly reply: ProjectReply }
  | { readonly type: 'state'; readonly state: ProjectState };

/** What `pick` resolves with: the folder chosen, or null when the dialog was cancelled. */
export type PickResult = { readonly folder: string | null };

/** What the preload script puts on `window.studioProject`. Each function's arguments are checked again in main. */
export interface ProjectBridge {
  /** The current state (also pushed to `onState` listeners on every change). */
  state(): Promise<ProjectState>;
  onState(listener: (state: ProjectState) => void): number;
  offState(id: number): void;
  /**
   * Shows the folder dialog of the main process and opens the folder chosen. F-832: `options.onCancel` receives the
   * function that cancels the opening (the project port's `cancel`).
   */
  pick(options?: CancelOptions): Promise<ProjectReply<PickResult>>;
  /** Opens a folder of the recent list (any other path is refused with `not_recent`). F-832: `options` as for pick. */
  open(path: string, options?: CancelOptions): Promise<ProjectReply<null>>;
  /** Removes a folder from the recent list. */
  forget(path: string): Promise<ProjectReply<null>>;
  /** Closes the open project (stops its project server). */
  close(): Promise<ProjectReply<null>>;
  /** Reads the project again now; when the project server stopped, starts it again. */
  refresh(): Promise<ProjectReply<null>>;
  /** Clears the message of a failed open. */
  dismiss(): Promise<ProjectReply<null>>;
  /**
   * F-835: makes a project with the engine and opens it: in a folder chosen in the main process's folder dialog
   * (`pick`), or in the folder the engine has just refused as no project (`here`). `options` as for pick.
   */
  create(where: CreateWhere, options?: CancelOptions): Promise<ProjectReply<PickResult>>;
}

// ---- F-807: run control (src/shared/actions/runs.ts; src/main/project/control.ts)

/** Run-control requests the view keeps (newest first). */
export const RUN_REQUESTS_KEPT = 20;

/**
 * After this long (ms) without the change a request asks for, the request is marked slow: the runner has not acted on it,
 * and the page says that the runner may not be running (the engine has no query for whether it is) and offers to start it.
 */
export const RUN_SLOW_MS = 3000;

/** For this long (ms) after a run-control action, the main process reads the feed at RUN_FAST_POLL_MS. */
export const RUN_BOOST_MS = 5000;
export const RUN_FAST_POLL_MS = 100;

/** Why a run-control action was refused or failed, in words that name the problem and the way to recover (G19). */
export interface RunActionFailure {
  readonly code:
    | 'unknown_action'
    | 'bad_request'
    | 'unavailable'
    | 'no_project'
    | 'not_found'
    | 'bad_state'
    | 'refused_path'
    | 'runner'
    | 'engine'
    | 'disconnected';
  readonly problem: string;
  readonly recovery: string;
  /** The engine's own code and message, when the refusal is the engine's. */
  readonly engine: { readonly code: number | string; readonly message: string } | null;
}

/** One run-control action as the view keeps it: what was asked, the engine's answer, and what the runner did since. */
export interface RunRequest {
  /** Increases by one with each action dispatched in this project. */
  readonly seq: number;
  /** The action id (src/shared/actions/runs.ts). */
  readonly action: string;
  readonly params: Readonly<Record<string, unknown>>;
  /** The run it is about; for a run started by it, the new run (null when the engine made none). */
  readonly run: string | null;
  readonly case: string | null;
  /** When it was dispatched (RFC 3339). */
  readonly at: string;
  /** When the main process had the engine's answer: ms since the epoch, fractional (performance.timeOrigin + now). */
  readonly answeredAt: number;
  /** Milliseconds from the dispatch to the engine's answer. */
  readonly ms: number;
  readonly outcome: 'accepted' | 'refused';
  /** The run's state in the engine's answer (start_run, pause_run, ... return it). */
  readonly state: string | null;
  /** The runner has done what was asked (the run list shows the run, held, released or ended). */
  readonly settled: boolean;
  /** Not settled after RUN_SLOW_MS. */
  readonly slow: boolean;
  readonly failure: RunActionFailure | null;
  /** The same from Claude Code and from the CLI (G16, G17). */
  readonly mcp: { readonly tool: string; readonly arguments: Readonly<Record<string, unknown>> } | null;
  readonly cli: { readonly argv: readonly string[]; readonly cwd: string };
}

/** What the `action` operation answers. */
export type RunActionResult =
  | { readonly ok: true; readonly request: RunRequest; readonly result: Readonly<Record<string, unknown>> }
  | { readonly ok: false; readonly failure: RunActionFailure; readonly request: RunRequest | null };

/**
 * What the preload script puts on `window.studioRuns` (F-807): run-control actions over the project port. A separate
 * global, so that `window.studioProject` keeps exactly the functions of F-805.
 */
export interface RunsBridge {
  dispatch(action: string, params?: Readonly<Record<string, unknown>>): Promise<RunActionResult>;
}
