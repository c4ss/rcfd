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

/** The one IPC channel main uses to hand the preload script the project port (main -> renderer only). */
export const PROJECT_PORT_CHANNEL = 'studio.project-port';

/** The tools of the engine's project server that the main process calls; all of them only read. `run_status` (F-806):
 * one run's status.json, its last events and its record, for the runs table. */
export const PROJECT_TOOLS = ['project_info', 'list_runs', 'list_meshes', 'read_feed', 'run_status'] as const;
export type ProjectTool = (typeof PROJECT_TOOLS)[number];

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

/** Where the engine's project server comes from (src/main/project/config.ts). */
export type EngineStatus =
  | { readonly configured: true; readonly path: string; readonly from: 'RCFD_STUDIO_MCP' | 'RCFD_STUDIO_RCFD' }
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
  /** When the Studio read it (RFC 3339). */
  readonly readAt: string;
}

/** A file under `notes/`. */
export interface NoteEntry {
  readonly path: string;
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
  /** Run folders the engine could not read (`list_runs`, `skipped`). */
  readonly skippedRuns: readonly { readonly path: string; readonly message: string }[];
  /**
   * F-806: `run_status` of each listed run, by run id (read after each read of the list; a run whose state and step
   * did not change keeps its last answer). A run missing here has not been read yet.
   */
  readonly runDetails: Readonly<Record<string, RunDetail>>;
  /** F-806: runs whose `run_status` failed, with the engine's message. */
  readonly runDetailErrors: Readonly<Record<string, string>>;
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
  readonly kind: 'not_configured' | 'not_found' | 'not_a_project' | 'engine';
  readonly folder: string;
  readonly message: string;
  /** The engine's own message, when it gave one. */
  readonly detail: string | null;
}

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
  | { readonly op: 'dismiss'; readonly id: number };

export type ProjectOp = ProjectRequest['op'];
export const PROJECT_OPS = ['state', 'pick', 'open', 'forget', 'close', 'refresh', 'dismiss'] as const satisfies readonly ProjectOp[];

export interface ProjectError {
  /** `bad_request` (the message has the wrong shape), `not_recent` (open of a folder the recent list does not hold). */
  readonly code: 'bad_request' | 'not_recent' | 'no_port' | 'timeout';
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
  /** Shows the folder dialog of the main process and opens the folder chosen. */
  pick(): Promise<ProjectReply<PickResult>>;
  /** Opens a folder of the recent list (any other path is refused with `not_recent`). */
  open(path: string): Promise<ProjectReply<null>>;
  /** Removes a folder from the recent list. */
  forget(path: string): Promise<ProjectReply<null>>;
  /** Closes the open project (stops its project server). */
  close(): Promise<ProjectReply<null>>;
  /** Reads the project again now; when the project server stopped, starts it again. */
  refresh(): Promise<ProjectReply<null>>;
  /** Clears the message of a failed open. */
  dismiss(): Promise<ProjectReply<null>>;
}
