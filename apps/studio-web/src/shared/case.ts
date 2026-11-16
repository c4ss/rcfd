// The case editor's way to the case files of the open project (F-812): the operations that cross the case port, their
// replies, and the bridge the preload script puts on `window.studioCase`. Shared by all three processes, so it uses
// neither Node nor DOM types.
//
// How a case is edited (docs/studio-guardrails.md G2, G12, G18): the Studio never reads or writes the case text itself.
//
// - Open: the main process asks the engine's project server for the file's hash (`read_project_file`), the renderer
//   makes a service session from the file (`session.create {path}`, an action of the F-827 registry, so it is logged and
//   replayable), and the main process binds the session to the file once the engine's journal of the session names that
//   file and its hash has not changed in between (`attach`).
// - Edit and check: the F-825 settings surface on that session (`settings.set`, `settings.remove`, `settings.validate`,
//   `mesh.check`), all through the action layer.
// - Save: the main process reads the session's journal (`journal.get`), takes the settings set since the last save, asks
//   the session for their values (`settings.get`) and writes them with the engine's own edit of a case file
//   (`edit_case` of the project server: validated, comments kept, replaced only if the file still has the hash the
//   editor read, under the project's edit lock, with a line in `.rcfd/edits.jsonl`). A file another writer changed is a
//   conflict (1200), shown with a choice, never overwritten (G18).
// - New case and duplicate: the project server's `new_case` (a built-in template, a shipped example, or a copy), which
//   validates first and never replaces a file.
//
// The project server for these calls is started without `--read-only`, one per open project, and the main process
// calls only the tools of CASE_TOOLS on it (src/main/case/server.ts). The project home keeps its own read-only server.
import type { CancelOptions } from './work.js';

/** The one IPC channel main uses to hand the preload script the case port (main -> renderer only). */
export const CASE_PORT_CHANNEL = 'studio.case-port';

/** The tools of the engine's project server the case editor calls; nothing else is sent to it. */
export const CASE_TOOLS = ['read_project_file', 'edit_case', 'new_case', 'check_case_headless'] as const;
export type CaseTool = (typeof CASE_TOOLS)[number];
/** The tools of CASE_TOOLS that write to the project folder. */
export const CASE_WRITE_TOOLS = ['edit_case', 'new_case'] as const satisfies readonly CaseTool[];

/** The `reason` the editor gives with every write: kept in the project journal and shown by the change feed. */
export const SAVE_REASON = 'rcfd Studio case editor: save';
export const NEW_REASON = 'rcfd Studio case editor: new case';
export const DUPLICATE_REASON = 'rcfd Studio case editor: duplicate';

/** Longest path, session id and case name the port accepts (the engine checks them again). */
export const MAX_PATH_CHARS = 4096;
export const MAX_NAME_CHARS = 64;
export const MAX_SESSION_CHARS = 64;

/** A file read for opening: its project-relative path, its absolute path (for `session.create {path}`) and its hash. */
export interface PreparedCase {
  readonly path: string;
  readonly file: string;
  readonly hash: string;
  readonly size: number;
}

/** A session of the service bound to a case file of the project. */
export interface CaseBinding {
  readonly session: string;
  /** The project folder the file belongs to. */
  readonly folder: string;
  /** Project-relative, `/`-separated (`cases/cavity.toml`). */
  readonly path: string;
  /** The absolute path the session was made from. */
  readonly file: string;
  /** The file's hash when it was opened or last saved (the `base_hash` of the next save). */
  readonly hash: string;
  readonly openedAt: string;
  readonly savedAt: string | null;
}

/** The changes of a session since it was opened or last saved, from its journal (`journal.get`). */
export interface PendingChanges {
  /** Accepted settings changes since then (every `settings.set` and `settings.remove`, from any writer). */
  readonly count: number;
  /** The setting paths a save would write, in the order they were first changed. */
  readonly paths: readonly string[];
  /** Named entries removed since then (`settings.remove`). */
  readonly removals: readonly string[];
}

/**
 * The file on disk as the engine reads it now: `same` (the hash the editor read or last wrote), `changed` (another writer
 * changed it since), `missing` (no such file now), `unknown` (it could not be read: the project is not open, or the
 * engine's project server failed; `message` says which).
 */
export type FileNow =
  | { readonly state: 'same' | 'changed'; readonly hash: string }
  | { readonly state: 'missing' | 'unknown'; readonly message: string };

export interface CaseStatus {
  readonly binding: CaseBinding;
  readonly pending: PendingChanges;
  readonly file: FileNow;
  /** False when the binding's project is not the open one: saving needs it open. */
  readonly projectOpen: boolean;
}

/** One setting the editor changed, with its value in the session and in the file as it is now (null: not in it). */
export interface ConflictRow {
  readonly path: string;
  readonly mine: unknown;
  readonly file: unknown;
  /** False when the file holds no such setting now. */
  readonly inFile: boolean;
}

export type ConflictView =
  | { readonly changed: false }
  | {
      readonly changed: true;
      /** The file's hash now; null when it is gone. */
      readonly currentHash: string | null;
      readonly rows: readonly ConflictRow[];
      readonly removals: readonly string[];
      /** Why the file's values could not be read (it is gone, or the engine does not accept it as a case now). */
      readonly fileError: string | null;
    };

/** An engine diagnostic: the setting path, the line when the engine knows it, and the engine's message. */
export interface Diagnostic {
  readonly path: string;
  readonly line: number | null;
  readonly message: string;
}

/** What a save did. */
export type SaveOutcome =
  | {
      readonly kind: 'saved';
      readonly path: string;
      readonly oldHash: string;
      readonly newHash: string;
      /** `minimal`: the changed lines edited in place, comments kept (edit_case). */
      readonly form: string;
      /** The settings written, with the value the engine stored. */
      readonly values: Readonly<Record<string, unknown>>;
      /** The engine's check of the edited mesh, when a mesh setting was among them. */
      readonly mesh: { readonly ok: boolean } | null;
      /** True when the file held another writer's changes too: open it again to see them in the editor. */
      readonly reopen: boolean;
    }
  | { readonly kind: 'nothing' }
  | { readonly kind: 'conflict'; readonly currentHash: string | null; readonly message: string }
  | { readonly kind: 'refused'; readonly message: string; readonly diagnostics: readonly Diagnostic[] }
  | { readonly kind: 'cannot'; readonly message: string; readonly removals: readonly string[] };

/** Where a new case comes from: a name the engine's `new_case` offers. */
export type NewCaseSource = { readonly kind: 'template'; readonly name: string } | { readonly kind: 'example'; readonly name: string };

/** The engine's own lists of templates and shipped examples (the enumerations of `new_case`'s input schema). */
export interface CaseSources {
  readonly templates: readonly string[];
  readonly examples: readonly string[];
  /** The engine's descriptions of the two arguments. */
  readonly templateHelp: string;
  readonly exampleHelp: string;
  readonly nameHelp: string;
}

export interface CreatedCase {
  readonly path: string;
  readonly hash: string;
  /** The engine's words for where it came from (`template lid_driven_cavity`, `copy cases/a.toml`). */
  readonly from: string;
  readonly dataFiles: readonly string[];
}

/** The check-only launch of the saved file (`check_case_headless`, as `rcfd run --check-only`). */
export interface FileCheck {
  readonly path: string;
  readonly ok: boolean;
  /** 0 ok, 3 invalid, 4 bad mesh (docs/run-control.md). */
  readonly exitCode: number | null;
  readonly diagnostics: readonly Diagnostic[];
  readonly cells: number | null;
  readonly meshErrors: readonly string[];
  readonly meshWarnings: readonly string[];
}

export interface CaseDoorState {
  /** The open project, or null. */
  readonly project: { readonly folder: string } | null;
  /** Whether the editor can reach the engine's project server, and why not. */
  readonly engine: { readonly available: true } | { readonly available: false; readonly reason: string };
  readonly bindings: readonly CaseBinding[];
}

// ---- Messages on the case port (preload <-> main). The main process validates every request (src/main/case/bridge.ts).

export type CaseRequest =
  | { readonly op: 'state'; readonly id: number }
  | { readonly op: 'sources'; readonly id: number }
  | { readonly op: 'prepare'; readonly id: number; readonly path: string }
  | { readonly op: 'attach'; readonly id: number; readonly session: string; readonly path: string; readonly hash: string }
  | { readonly op: 'status'; readonly id: number; readonly session: string }
  | { readonly op: 'conflict'; readonly id: number; readonly session: string }
  | { readonly op: 'save'; readonly id: number; readonly session: string; readonly onto: 'opened' | 'current' }
  | { readonly op: 'create'; readonly id: number; readonly name: string; readonly source: NewCaseSource }
  | { readonly op: 'duplicate'; readonly id: number; readonly path: string; readonly name: string }
  | { readonly op: 'check'; readonly id: number; readonly path: string }
  | { readonly op: 'forget'; readonly id: number; readonly session: string }
  /**
   * F-832 (G21): cancels the save or check of request `target`. A check is answered `cancelled` at once (the engine's
   * project server has no way to stop a tool call, so its result is dropped when it comes). A save is cancelled only
   * while nothing has been sent to the engine's `edit_case`; once it has, the save completes and its own reply says
   * so (a file is written whole or not at all, by the engine). The reply's result is `{cancelled: boolean}`: whether
   * `target` was a save or check still going.
   */
  | { readonly op: 'cancel'; readonly id: number; readonly target: number };

export type CaseOp = CaseRequest['op'];
export const CASE_OPS = ['state', 'sources', 'prepare', 'attach', 'status', 'conflict', 'save', 'create', 'duplicate', 'check', 'forget', 'cancel'] as const satisfies readonly CaseOp[];

export interface CaseError {
  /**
   * `bad_request` (the message has the wrong shape), `no_project` (no project open, or not the binding's), `not_bound`
   * (the session is not bound to a case file), `changed` (the file changed while it was being opened: open it again),
   * `not_from_file` (the session was not made from that file), `not_configured` (no project server), `no_port`,
   * `timeout`, or the engine's own code (1003 invalid, 1004 refused path, 1200 changed since read, 1201 lock busy, 1202
   * not found, ...).
   */
  readonly code: 'bad_request' | 'no_project' | 'not_bound' | 'changed' | 'not_from_file' | 'not_configured' | 'no_port' | 'timeout' | 'engine' | 'cancelled' | number;
  readonly message: string;
  readonly data?: unknown;
}

export type CaseReply<T = unknown> = { readonly ok: true; readonly result: T } | { readonly ok: false; readonly error: CaseError };

/** Main -> preload. */
export type CasePortMessage = { readonly type: 'reply'; readonly id: number; readonly reply: CaseReply };

/** What the preload script puts on `window.studioCase`. Each function's arguments are checked again in main. */
export interface CaseBridge {
  state(): Promise<CaseReply<CaseDoorState>>;
  /** The engine's templates and shipped examples for a new case (starts the project server when needed). */
  sources(): Promise<CaseReply<CaseSources>>;
  /** Reads a case file's hash for opening; `path` is project-relative (`cases/<name>.toml` or `.json`). */
  prepare(path: string): Promise<CaseReply<PreparedCase>>;
  /** Binds a session made from the prepared file (`session.create {path: file}`) to it. */
  attach(session: string, path: string, hash: string): Promise<CaseReply<CaseBinding>>;
  status(session: string): Promise<CaseReply<CaseStatus>>;
  /** The editor's changes beside the file's values now, when the file changed since it was read. */
  conflict(session: string): Promise<CaseReply<ConflictView>>;
  /**
   * `onto: 'opened'` writes only if the file is as it was read; `'current'` writes onto the file as it is now. F-832:
   * `options.onCancel` receives the function that cancels this save (the port's `cancel`; see there what it can undo).
   */
  save(session: string, onto: 'opened' | 'current', options?: CancelOptions): Promise<CaseReply<SaveOutcome>>;
  create(name: string, source: NewCaseSource): Promise<CaseReply<CreatedCase>>;
  duplicate(path: string, name: string): Promise<CaseReply<CreatedCase>>;
  /** F-832: `options.onCancel` receives the function that cancels this check. */
  check(path: string, options?: CancelOptions): Promise<CaseReply<FileCheck>>;
  forget(session: string): Promise<CaseReply<null>>;
}

// ---- The case-level actions and their engine equivalents (G16). They are not in the F-827 registry, whose actions are
// one service method each (src/shared/actions/registry.ts); these combine project-server tools and service methods.

export interface CaseAction {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  /** What the main process and the renderer ask the engine, in order. */
  readonly engine: readonly string[];
  /** The same from Claude Code (the project server's tools). */
  readonly mcp: string;
  /** The same from the command line, or why there is none. */
  readonly cli: string;
}

export const CASE_ACTIONS: readonly CaseAction[] = [
  {
    id: 'case.open',
    title: 'Open a case file',
    description: 'Make a service session from a case file of the project and bind it to the file, with the hash the next save compares.',
    engine: ['read_project_file', 'session.create {path}', 'journal.get'],
    mcp: 'read_project_file (the hash); the project server has no session tools (docs/mcp-tools.md, Project mode)',
    cli: 'rcfd validate <case> reads and checks the same file; rcfd call session.create \'{"path": "<file>"}\' makes the same session',
  },
  {
    id: 'case.save',
    title: 'Save the case',
    description: 'Write the settings changed since the last save into the file, in place, only if the file is as it was read.',
    engine: ['journal.get', 'settings.get', 'edit_case {path, base_hash, set}'],
    mcp: 'edit_case {path, base_hash, set}',
    cli: 'none: no rcfd command writes a case file (rcfd run --set changes a run, not the file)',
  },
  {
    id: 'case.new',
    title: 'New case',
    description: 'Make a case file in cases/ from a built-in template or a shipped example; the engine checks it first and never replaces a file.',
    engine: ['new_case {name, template | example}'],
    mcp: 'new_case {name, template | example}',
    cli: 'none: rcfd cases lists the shipped cases; copying one into cases/ by hand is the same file',
  },
  {
    id: 'case.duplicate',
    title: 'Duplicate the case',
    description: 'Copy the saved case file byte for byte to a new name in cases/.',
    engine: ['new_case {name, copy}'],
    mcp: 'new_case {name, copy}',
    cli: 'none: a file copy in the shell gives the same bytes, without the journal line',
  },
  {
    id: 'case.check_file',
    title: 'Check the saved file',
    description: 'The check-only launch of the file as saved: settings, simulation set-up and mesh quality, with the exit code the CLI would give.',
    engine: ['check_case_headless {path}'],
    mcp: 'check_case_headless {path}',
    cli: 'rcfd run <case> --check-only',
  },
  {
    id: 'case.close',
    title: 'Close the session',
    description: 'Remove the session from the service; unsaved changes are lost, so the editor asks first and names them.',
    engine: ['session.close'],
    mcp: 'none in project mode',
    cli: 'rcfd call session.close \'{"session": "<id>"}\'',
  },
];
