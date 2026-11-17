// The renderer's way to Claude Code for the open project (F-814): what the Claude Code panel shows, the messages that
// cross the Claude port, and the bridge the preload script puts on `window.studioClaude`. Shared by all three processes,
// so it uses neither Node nor DOM types.
//
// The rules (docs/studio-guardrails.md G11; docs/ui-restart-plan.md section 8, decisions 1 and 2):
// - a session another process has open is mirrored read-only; the Studio types only into a closed session (resumed with
//   `claude -p --resume <id>`) or a new one it starts (`--session-id <id>`), and never into one whose state is unknown;
// - one writer per session: the main process holds the lock (`ClaudeView.writer`), and a second message to a session
//   that is being written is refused;
// - tool approvals come from a conservative allow-list passed to claude as `--allowedTools`, under
//   `--permission-mode dontAsk --permission-prompts none`; nothing passes a bypass or approve-everything flag, and the
//   main process checks every argument list before it starts claude;
// - a denied call's suggested rule is added to the project's allow-list file only when the user confirms that exact
//   rule ("Allow this from now on"), by the main process, never over a file changed since it was read, and the last
//   addition can be undone (the maintainer, 2026-11-17);
// - claude is started only by the main process, with the project folder as its working folder. The renderer never names
//   a program, a flag or a file: it sends the operations below, and the main process checks every message
//   (src/main/claude/bridge.ts).

/** The one IPC channel main uses to hand the preload script the Claude port (main -> renderer only). */
export const CLAUDE_PORT_CHANNEL = 'studio.claude-port';

/** How often the main process lists the project's sessions (ms), while a project is open. */
export const DISCOVER_MS = 2000;
/** How often the selected session's transcript is read for new lines (ms). */
export const MIRROR_MS = 500;
/** Entries read from the end of a transcript when a session is selected. */
export const TAIL = 50;
/** The longest message the panel sends (characters). */
export const MAX_MESSAGE_CHARS = 20_000;
/** Characters of a tool result or tool input kept for display. */
export const SHOWN_CHARS = 2000;
/** The Claude Code version the stream and transcript shapes were observed on (ADR 0126); others get a warning. */
export const PINNED_CLI_VERSION = '2.1.310';

/** What the panel says about the allow-list's reach (ADR 0126 observed limits). */
export const FLOOR_CAVEAT =
  'The allow-list is the floor this Studio controls, not the whole gate: Claude Code also approves some read-only shell commands by itself, and your user and project settings can add rules.';

/** Where the `claude` program comes from (src/main/claude/config.ts). */
export type ClaudeProgram =
  | { readonly configured: true; readonly path: string; readonly from: 'RCFD_STUDIO_CLAUDE' }
  | { readonly configured: false; readonly reason: string };

/**
 * Whether another process has a session open:
 * `open` (listed by `claude agents --json` with a live process: mirror only), `closed` (the Studio may resume it),
 * `unknown` (the listing failed or claude is not configured: not written to), `studio` (this Studio's own turn has it).
 */
export type SessionOpen = 'open' | 'closed' | 'unknown' | 'studio';

export interface SessionRow {
  /** The session id (a UUID). */
  readonly id: string;
  /** The name Claude Code gave it (`claude agents --json`), if listed. */
  readonly name: string | null;
  /** The start of its first message (from the transcript), if any. */
  readonly title: string | null;
  /** When its transcript last changed (RFC 3339), if it has one. */
  readonly modified: string | null;
  readonly open: SessionOpen;
  /** The process that has it open (`open` and `studio`). */
  readonly pid: number | null;
  /** Why the panel cannot type into it; null when it can. */
  readonly readOnly: string | null;
}

export type FeedKind = 'user' | 'assistant' | 'tool' | 'tool_result' | 'denied' | 'note' | 'unrecognised';

/** One item of the transcript area. */
export interface FeedItem {
  /** Stable within one selection (React key). */
  readonly key: string;
  readonly kind: FeedKind;
  readonly text: string;
  /** Tools called in this message. */
  readonly tools: readonly string[];
  /** The transcript's timestamp, when it has one. */
  readonly at: string | null;
  /** From the running turn's stream; replaced by the transcript's lines once the CLI has written them. */
  readonly provisional: boolean;
  /** A denied call: the narrowest rule that would allow it, when a safe one exists. */
  readonly suggestion: string | null;
}

export type TurnStatus = 'starting' | 'running' | 'finished' | 'failed' | 'aborted' | 'cancelled' | 'refused';

/** The Studio's last message to a session: one `claude -p` process. */
export interface TurnView {
  readonly session: string;
  readonly isNew: boolean;
  readonly status: TurnStatus;
  /** Why it was refused, aborted or failed, in words. */
  readonly message: string | null;
  readonly pid: number | null;
  /** The arguments claude was started with (after the program). */
  readonly argv: readonly string[];
  readonly started: string;
  readonly exitCode: number | null;
  readonly stderrTail: string;
  /** From the CLI's system/init line: the permission mode it applied, its version and model. */
  readonly mode: string | null;
  readonly version: string | null;
  readonly model: string | null;
  readonly costUsd: number | null;
  /**
   * F-837: the costs the CLI reported (`total_cost_usd` of each result line) for the turns this Studio sent to the session
   * since it started, summed; null when none reported one. Turns made elsewhere (a terminal) are not in it.
   */
  readonly sessionCostUsd: number | null;
}

/** The allow-list passed to claude (`--allowedTools`). */
export interface AllowView {
  /** `file`: the project's `.rcfd/claude-allow.json`; `initial`: the Studio's first list (no file in the folder). */
  readonly source: 'file' | 'initial';
  /** The allow-list file of the project, project-relative. */
  readonly file: string;
  /** The rules as flag values, in order. */
  readonly rules: readonly string[];
  /** Why the file is refused (then no chat is started); null when it is used or absent. */
  readonly error: string | null;
  /** The SHA-256 (hex) of the file as last read; null when there is no file. `allow` is refused when it differs. */
  readonly hash: string | null;
  /** The rule this Studio last added to this folder's file, while `undoAllow` can take it out; null when none. */
  readonly undo: string | null;
}

/** A file or link in the folder that changes what `claude -p` runs or reads. */
export interface ControlFileView {
  readonly path: string;
  readonly kind: 'file' | 'link';
  /** `sha256:<hex>` of the contents (file) or of the target text (link). */
  readonly hash: string;
  readonly bytes: number;
}

/**
 * Whether a chat may start in this folder (ADR 0134 Amendment 2): the folder's Claude Code control files (`.mcp.json`,
 * `CLAUDE.md`, `.claude/`, `.claude-plugin/`, the allow-list file) must be the ones the user confirmed. The record lives
 * in the Studio's user data, never in the project.
 */
export type FolderTrust =
  | { readonly state: 'confirmed'; readonly files: number }
  | {
      readonly state: 'pending';
      /** Names this scan; `confirm` records it only while the folder still matches. */
      readonly token: string;
      readonly files: readonly ControlFileView[];
      /** Paths added, changed or removed since the confirmed record (all of them when there is none). */
      readonly changed: readonly string[];
    }
  | { readonly state: 'error'; readonly message: string };

/** The Claude Code side of the open project. */
export interface ClaudeView {
  readonly folder: string;
  /** Claude Code's transcript folder for this project. */
  readonly transcripts: string;
  /** Newest first. */
  readonly sessions: readonly SessionRow[];
  /** Problems with either source of the listing; what the other gave is still shown. */
  readonly warnings: readonly string[];
  /** When the sessions were last listed (RFC 3339). */
  readonly listedAt: string | null;
  /** The selected session, or null for a new chat. */
  readonly selected: string | null;
  /** The selected session's transcript (last TAIL entries, then every new line) and the running turn's stream. */
  readonly feed: readonly FeedItem[];
  /** Why the transcript could not be read, when it could not. */
  readonly feedError: string | null;
  /** The last or running turn of this project. */
  readonly turn: TurnView | null;
  /** The one-writer lock: the session the Studio is writing to, and its process. */
  readonly writer: { readonly session: string; readonly pid: number | null } | null;
  readonly allow: AllowView;
  readonly trust: FolderTrust;
  /** The app-owned MCP config passed with --strict-mcp-config: the file, and the server it starts (null: none). */
  readonly mcp: { readonly file: string | null; readonly server: string | null; readonly note: string | null };
  /** The CLI version the last turn reported, when it is not PINNED_CLI_VERSION. */
  readonly versionWarning: string | null;
}

export interface ClaudeState {
  readonly program: ClaudeProgram;
  /** Claude Code's home (CLAUDE_CONFIG_DIR, else ~/.claude); null when it is not known. */
  readonly home: string | null;
  readonly project: ClaudeView | null;
}

// ---- Messages on the Claude port (preload <-> main). The main process validates every request (src/main/claude/bridge.ts).

export type ClaudeRequest =
  | { readonly op: 'state'; readonly id: number }
  | { readonly op: 'select'; readonly id: number; readonly session: string | null }
  | { readonly op: 'send'; readonly id: number; readonly session: string | null; readonly text: string }
  | { readonly op: 'cancel'; readonly id: number }
  | { readonly op: 'confirm'; readonly id: number; readonly token: string }
  | { readonly op: 'refresh'; readonly id: number }
  /** Adds `rule`, the suggestion of a denied call shown in the feed, to the allow-list file that has the hash `base`. */
  | { readonly op: 'allow'; readonly id: number; readonly rule: string; readonly base: string | null }
  /** Takes the last added rule out again (puts the file back as it was); `force` even when another writer changed it. */
  | { readonly op: 'undoAllow'; readonly id: number; readonly force: boolean };

export type ClaudeOp = ClaudeRequest['op'];
export const CLAUDE_OPS = ['state', 'select', 'send', 'cancel', 'confirm', 'refresh', 'allow', 'undoAllow'] as const satisfies readonly ClaudeOp[];
/** The longest rule `allow` takes (characters). */
export const MAX_RULE_CHARS = 2000;

export interface ClaudeError {
  /**
   * `bad_request` (wrong shape), `no_project`, `not_configured` (no claude), `read_only` (open in another process, or its
   * state is unknown), `busy` (the one-writer lock is held), `not_confirmed` (the folder's control files), `refused`
   * (an argument or allow-list check failed), `stale` (a confirmation of a scan that changed), `no_port`, `timeout`.
   */
  readonly code:
    | 'bad_request'
    | 'no_project'
    | 'not_configured'
    | 'read_only'
    | 'busy'
    | 'not_confirmed'
    | 'refused'
    | 'stale'
    | 'no_port'
    | 'timeout';
  readonly message: string;
}

export type ClaudeReply<T = unknown> = { readonly ok: true; readonly result: T } | { readonly ok: false; readonly error: ClaudeError };

/** Main -> preload. */
export type ClaudePortMessage =
  | { readonly type: 'reply'; readonly id: number; readonly reply: ClaudeReply }
  | { readonly type: 'state'; readonly state: ClaudeState };

/** What `send` resolves with: the session written to (the new id for a new chat). */
export type SendResult = { readonly session: string };

/** What the preload script puts on `window.studioClaude`. Each function's arguments are checked again in main. */
export interface ClaudeBridge {
  state(): Promise<ClaudeState>;
  onState(listener: (state: ClaudeState) => void): number;
  offState(id: number): void;
  /** Mirrors a session of the open project (null: a new chat). */
  select(session: string | null): Promise<ClaudeReply<null>>;
  /** Sends a message: resumes a closed session, or starts a new one when `session` is null. */
  send(session: string | null, text: string): Promise<ClaudeReply<SendResult>>;
  /** Stops the running turn (kills its claude process and what it started). */
  cancel(): Promise<ClaudeReply<null>>;
  /** Records the folder's control files, as shown, as confirmed. */
  confirm(token: string): Promise<ClaudeReply<null>>;
  /** Lists the sessions and checks the folder again now. */
  refresh(): Promise<ClaudeReply<null>>;
  /** Adds a denied call's suggested rule to the project's allow-list file, if the file still has the hash `base`. */
  allow(rule: string, base: string | null): Promise<ClaudeReply<null>>;
  /** Takes the rule this Studio last added out of the file again. */
  undoAllow(force: boolean): Promise<ClaudeReply<null>>;
}

/** True when `s` is a UUID in the 8-4-4-4-12 hex form (session ids are passed to claude only after this check). */
export function isUuid(s: string): boolean {
  return /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(s);
}
