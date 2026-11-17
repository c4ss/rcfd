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

export type PaletteGroup = 'Go to' | 'Project' | 'Project runs' | 'Run workspace' | 'Case file' | 'Claude Code' | 'Service session' | 'Help';

/** The order of the groups when nothing is typed: the frequent ones first. */
export const GROUP_ORDER: readonly PaletteGroup[] = ['Go to', 'Project', 'Project runs', 'Run workspace', 'Case file', 'Claude Code', 'Service session', 'Help'];

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

  // ---- comparing two runs (F-815)
  {
    id: 'workspace.compare',
    title: 'Compare runs',
    description:
      'Show the run selected in the workspace and another run at the same time, side by side in one colour range, with their difference field where the meshes match; the residual lines of both, and the metrics of both from the engine.',
    group: 'Project runs',
    effect: 'navigate',
    params: [{ name: 'run', kind: 'run', optional: false, description: 'The run to compare with the one selected in the workspace (the baseline).' }],
    engine: 'compare_runs of the project server (the metrics and their differences); the stored fields of both runs through the Studio’s reader of store.rts',
    cli: null,
    mcp: 'compare_runs',
    note: 'The metrics are the engine’s compare_runs, as Claude Code gets them; the difference field (B − A of the stored values, ADR 0801 addendum F-815) has no engine entry point yet.',
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

  // ---- provenance (F-828)
  {
    id: 'provenance.show',
    title: 'Show provenance',
    description:
      'Where the focused or pointed number came from: its run, snapshot, file, the engine version that made it and its unit as the engine gave it, and the command that reproduces the run.',
    group: 'Project runs',
    effect: 'navigate',
    params: [],
    engine: 'read_project_file of runs/<id>/run.json, its case.snapshot and the case file (rcfd-mcp --project <folder> --read-only); service.info',
    cli: 'rcfd-runner start <case> --project <folder> --threads <n> --priority <n> [--mem-mb <n>] [--set path=value]...',
    mcp: 'read_project_file',
    note: 'Reads only: the facts are the run record the runner wrote; the command is built from it, never from the Studio’s settings.',
  },
  {
    id: 'provenance.copy_command',
    title: 'Copy reproduce command',
    description: 'Copy the command that runs a run again from its record (rcfd-runner start with the recorded case and settings), and say what it would not reproduce.',
    group: 'Project runs',
    effect: 'navigate',
    params: [{ name: 'run', kind: 'run', optional: false, description: 'A run of the open project (the run selected in Runs first).' }],
    engine: 'read_project_file of runs/<id>/run.json, its case.snapshot and the case file (rcfd-mcp --project <folder> --read-only)',
    cli: 'rcfd-runner start <case> --project <folder> --threads <n> --priority <n> [--mem-mb <n>] [--set path=value]...',
    mcp: 'read_project_file',
    note: 'The command is the run record’s; the clipboard is written by the main process.',
  },

  // ---- F-837: the run list's and the run workspace's extras of the removed egui Studio
  {
    id: 'runs.log',
    title: 'Show the run log',
    description: 'Show or hide the end of the selected run’s log.txt and its last events, with the runner’s and the solver’s warnings named as warnings.',
    group: 'Project runs',
    effect: 'navigate',
    params: [],
    engine: 'read_project_file of runs/<id>/log.txt and run_status {events} (rcfd-mcp --project <folder> --read-only)',
    cli: 'rcfd-runner status <run> --project <folder>',
    mcp: 'run_status',
    note: 'Reads only. The CLI prints the status and the last events; the log is the file runs/<id>/log.txt.',
  },
  {
    id: 'runs.report',
    title: 'Open the run report',
    description: 'Open the selected run’s report (runs/<id>/output/report.html, written by the solver with its results) in the system’s program for HTML.',
    group: 'Project runs',
    effect: 'navigate',
    params: [],
    engine: 'read_project_file of runs/<id>/output/report.html (whether it is there); the system’s program for HTML opens it',
    cli: null,
    mcp: 'read_project_file',
    note: 'Opening a file in a browser is the system’s, not the engine’s; Claude Code reads the same report with read_project_file.',
  },
  {
    id: 'runs.budgets',
    title: 'Runner budgets',
    description: 'Show or hide the runner’s budgets (.rcfd/runner-config.json: runs at once, threads, memory, polling) to read and set them; the runner reads them when it starts.',
    group: 'Project runs',
    effect: 'change',
    params: [],
    engine: 'the main process reads and writes .rcfd/runner-config.json (rcfd-runner’s Config, ADR 0123 section 6) and reads .rcfd/runner.json',
    cli: null,
    mcp: null,
    note: 'No engine command sets the budgets (proposed: an rcfd-runner or MCP command); the file is the runner’s own, edited by hand or by Claude Code’s file tools. Every key is optional; the runner refuses unknown keys.',
  },
  {
    id: 'field.save_png',
    title: 'Save the field view as PNG',
    description: 'Save the field view as it is drawn, with its legend and caption, as a PNG file you name in a dialog.',
    group: 'Run workspace',
    effect: 'change',
    params: [],
    engine: 'the main process captures the window’s picture and writes the file the save dialog names',
    cli: null,
    mcp: null,
    note: 'The picture the Studio shows; the engine’s own renderer (rcfd-post’s, behind the MCP render_field tool) draws a store’s field with its own camera and colours.',
  },
  {
    id: 'field.range',
    title: 'Set the colour range',
    description: 'Colour the field by the range of the shown snapshot, of the whole run (every stored snapshot), or by a fixed range you type.',
    group: 'Run workspace',
    effect: 'navigate',
    params: [{ name: 'mode', kind: 'string', optional: false, enum: ['snapshot', 'run', 'fixed'], description: 'snapshot (the shown values), run (all stored snapshots) or fixed (typed in the view).' }],
    engine: 'data.field of each stored snapshot (route local or the service), for the whole run',
    cli: null,
    mcp: null,
    note: 'A view setting: it changes how the stored values are coloured, not the values.',
  },
  {
    id: 'field.face_on',
    title: 'View the field face on',
    description: 'Look straight at a 2D case’s plane (along its one-cell axis), with a scale bar; Reset view goes back to the fitted view.',
    group: 'Run workspace',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
  },
  {
    id: 'plots.monitor_log',
    title: 'Monitor on a log scale',
    description: 'Draw the run workspace’s monitor plot on a log scale, or back on a linear one; values not above zero are not drawn on a log scale, and how many is said.',
    group: 'Run workspace',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: NAV_NOTE,
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
