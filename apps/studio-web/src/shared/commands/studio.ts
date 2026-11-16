// The Studio's own commands (F-831; G20): moving between the views, the project folder operations of the project home
// (F-805) and the Claude Code panel's operations (F-814). They are commands of the palette like the actions of the
// registries, so every thing the window can do is findable by name (G20), but they are not engine actions: a view
// change reaches no result, the folder operations are the main process's (the folder dialog, the recent list, the
// read-only project server), and Claude Code is the `claude` CLI (no engine entry point yet, reported with F-814).
// G16 still holds: none of them makes a result the CLI or Claude Code could not make; `engine`, `cli` and `mcp` name
// what the command uses, or say there is nothing.
//
// Shared by the renderer and the tests: no Node or DOM types.

/** The kinds of a command's parameter, as the palette asks for them. The first five are chosen from a list. */
export type ParamKind =
  | 'session'
  | 'case'
  | 'run'
  | 'recent'
  | 'claude-session'
  | 'string'
  | 'integer'
  | 'number'
  | 'boolean'
  | 'string-array'
  | 'object'
  | 'json';

/** Parameters whose value is chosen from what the engine or the main process lists, never typed or made up. */
export const CHOICE_KINDS: ReadonlySet<ParamKind> = new Set(['session', 'case', 'run', 'recent', 'claude-session']);

export interface PaletteParam {
  readonly name: string;
  readonly kind: ParamKind;
  readonly optional: boolean;
  /** One sentence for the palette's form (G19). */
  readonly description: string;
  readonly minimum?: number;
  readonly maximum?: number;
  readonly enum?: readonly string[];
}

export type PaletteGroup = 'Go to' | 'Project' | 'Project runs' | 'Case file' | 'Claude Code' | 'Service session' | 'Help';

/** The order of the groups when nothing is typed: the frequent ones first. */
export const GROUP_ORDER: readonly PaletteGroup[] = ['Go to', 'Project', 'Project runs', 'Case file', 'Claude Code', 'Service session', 'Help'];

export interface StudioCommand {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly group: PaletteGroup;
  /** `navigate`: changes what the window shows, nothing else. */
  readonly effect: 'navigate' | 'change';
  readonly params: readonly PaletteParam[];
  /** What it uses in the engine or the main process, or null when it uses nothing (a view change). */
  readonly engine: string | null;
  readonly cli: string | null;
  readonly mcp: string | null;
  /** Why there is no CLI or MCP equivalent, or what differs. */
  readonly note: string | null;
}

const NAV_NOTE = 'A view of the Studio: no engine call, nothing to script.';
const CLAUDE_NOTE = 'Claude Code itself: the Studio runs the `claude` CLI named by RCFD_STUDIO_CLAUDE; the engine has no entry point for it (F-814).';

export const STUDIO_COMMANDS: readonly StudioCommand[] = [
  // ---- going places
  {
    id: 'go.home',
    title: 'Go to Home',
    description: 'The index of the open project folder: its cases, meshes, runs and notes, and the lane of changes.',
    group: 'Go to',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
  },
  {
    id: 'go.runs',
    title: 'Go to Runs',
    description: 'The runs of the open project, with the run bar and the selected run’s panel.',
    group: 'Go to',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
  },
  {
    id: 'go.claude',
    title: 'Go to Claude Code',
    description: 'Show the Claude Code panel and put the focus in its message field.',
    group: 'Go to',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
  },
  {
    id: 'view.toggle_claude',
    title: 'Show or hide the Claude Code panel',
    description: 'Hide the panel to a slim strip beside the project views, or show it again.',
    group: 'Go to',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
  },
  {
    id: 'go.settings',
    title: 'Go to Case settings',
    description: 'The settings of a service session, every key of the engine’s case schema (#/settings).',
    group: 'Go to',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
  },
  {
    id: 'go.case_editor',
    title: 'Go to the case editor',
    description: 'The case files of the project open in the editor (#/case).',
    group: 'Go to',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
  },

  // ---- the project folder (F-805)
  {
    id: 'project.open_folder',
    title: 'Open project folder…',
    description: 'Choose a project folder in the system’s folder dialog; the engine’s project server reads it, read-only.',
    group: 'Project',
    effect: 'change',
    params: [],
    engine: 'rcfd-mcp --project <folder> --read-only: project_info, list_runs, list_meshes, read_feed',
    cli: 'rcfd-mcp --project <folder> --read-only',
    mcp: 'project_info',
    note: 'The folder dialog is the main process’s; what is shown is what the project server answers Claude Code too.',
  },
  {
    id: 'project.open_recent',
    title: 'Open a recent project',
    description: 'Open one of the folders opened before, from the recent list (paths and times only).',
    group: 'Project',
    effect: 'change',
    params: [{ name: 'path', kind: 'recent', optional: false, description: 'A folder of the recent list.' }],
    engine: 'rcfd-mcp --project <folder> --read-only: project_info, list_runs, list_meshes, read_feed',
    cli: 'rcfd-mcp --project <folder> --read-only',
    mcp: 'project_info',
    note: 'The recent list is the Studio’s own (paths and times, in its user data), never the project’s.',
  },
  {
    id: 'project.forget_recent',
    title: 'Forget a recent project',
    description: 'Take a folder off the recent list; the folder itself is not touched.',
    group: 'Project',
    effect: 'change',
    params: [{ name: 'path', kind: 'recent', optional: false, description: 'A folder of the recent list.' }],
    engine: null,
    cli: null,
    mcp: null,
    note: 'The recent list is the Studio’s own; nothing in the engine or the folder changes.',
  },
  {
    id: 'project.refresh',
    title: 'Read the project again',
    description: 'Read the folder through the engine again; when its project server has stopped, start a new one (Reconnect).',
    group: 'Project',
    effect: 'change',
    params: [],
    engine: 'rcfd-mcp --project <folder> --read-only: project_info, list_runs, list_meshes, read_feed',
    cli: 'rcfd-mcp --project <folder> --read-only',
    mcp: 'project_info',
    note: null,
  },
  {
    id: 'project.close',
    title: 'Close the project',
    description: 'Stop reading the folder and go back to the empty window. Runs go on with the runner (G7); nothing in the folder changes.',
    group: 'Project',
    effect: 'change',
    params: [],
    engine: 'stops the Studio’s project server for the folder',
    cli: null,
    mcp: null,
    note: 'Nothing to script: the folder is unchanged and the runner keeps every run.',
  },

  // ---- Claude Code (F-814)
  {
    id: 'claude.new_chat',
    title: 'New Claude Code chat',
    description: 'Start a new Claude Code session in the project folder at the next message, and put the focus in the message field.',
    group: 'Claude Code',
    effect: 'change',
    params: [],
    engine: 'claude -p --session-id <uuid> (at the first message)',
    cli: 'claude -p --session-id <uuid>',
    mcp: null,
    note: CLAUDE_NOTE,
  },
  {
    id: 'claude.select',
    title: 'Choose a Claude Code session',
    description: 'Show the transcript of one of the project’s Claude Code sessions; one open elsewhere is mirrored read-only.',
    group: 'Claude Code',
    effect: 'change',
    params: [{ name: 'session', kind: 'claude-session', optional: false, description: 'A Claude Code session of this project.' }],
    engine: 'the transcripts under Claude Code’s home and claude agents --json',
    cli: 'claude -p --resume <id>',
    mcp: null,
    note: CLAUDE_NOTE,
  },
  {
    id: 'claude.stop',
    title: 'Stop the Claude Code reply',
    description: 'Stop the reply being written: the claude process of the turn is ended.',
    group: 'Claude Code',
    effect: 'change',
    params: [],
    engine: 'ends the running claude -p process tree',
    cli: null,
    mcp: null,
    note: CLAUDE_NOTE,
  },
  {
    id: 'claude.refresh',
    title: 'List the Claude Code sessions again',
    description: 'Read the project’s Claude Code sessions and which ones are open elsewhere now.',
    group: 'Claude Code',
    effect: 'change',
    params: [],
    engine: 'the transcripts under Claude Code’s home and claude agents --json',
    cli: 'claude agents --json',
    mcp: null,
    note: CLAUDE_NOTE,
  },

  // ---- help
  {
    id: 'palette.open',
    title: 'Open the command palette',
    description: 'Every command of the Studio, found by its name or its id, with its shortcut and its CLI or Claude Code equivalent.',
    group: 'Help',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
  },
  {
    id: 'help.shortcuts',
    title: 'Show the keyboard shortcuts',
    description: 'Every keyboard shortcut of the Studio by where it works, and the keys kept for the system and other items.',
    group: 'Help',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
  },
];
