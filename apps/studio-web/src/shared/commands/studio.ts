// The Studio's own commands (F-831; G20): moving between the views, the project folder operations of the project home
// (F-805) and Claude Code's registration and terminal (ADR 0801 Addendum 23). They are commands of the palette like the actions of the
// registries, so every thing the window can do is findable by name (G20), but they are not engine actions: a view
// change reaches no result, the folder operations are the main process's (the folder dialog, the recent list, the
// read-only project server), and Claude Code is the `claude` CLI, run in its own terminal (no engine entry point).
// G16 still holds: none of them makes a result the CLI or Claude Code could not make; `engine`, `cli` and `mcp` name
// what the command uses, or say there is nothing.
//
// Shared by the renderer and the tests: no Node or DOM types.

/** The kinds of a command's parameter, as the palette asks for them. The first four are chosen from a list. */
export type ParamKind =
  | 'session'
  | 'case'
  | 'run'
  | 'recent'
  | 'string'
  | 'integer'
  | 'number'
  | 'boolean'
  | 'string-array'
  | 'object'
  | 'json';

/** Parameters whose value is chosen from what the engine or the main process lists, never typed or made up. */
export const CHOICE_KINDS: ReadonlySet<ParamKind> = new Set(['session', 'case', 'run', 'recent']);

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

export type PaletteGroup = 'Go to' | 'Project' | 'Project runs' | 'Run workspace' | 'Case file' | 'Claude Code' | 'Service session' | 'View' | 'Help';

/** The order of the groups when nothing is typed: the frequent ones first. */
export const GROUP_ORDER: readonly PaletteGroup[] = ['Go to', 'Project', 'Project runs', 'Run workspace', 'Case file', 'Claude Code', 'Service session', 'View', 'Help'];

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
const THEME_NOTE = 'A preference of the Studio (its user data, beside the recent list): no engine call, nothing to script.';

/** F-818: one command per value of the theme setting (src/shared/theme.ts), so typing the theme's name finds it. */
const theme = (id: string, title: string, description: string): StudioCommand => ({
  id,
  title,
  description,
  group: 'View',
  effect: 'change',
  params: [],
  engine: null,
  cli: null,
  mcp: null,
  note: THEME_NOTE,
});
const CLAUDE_NOTE =
  'Claude Code itself, in its own terminal: the Studio runs the `claude` CLI (RCFD_STUDIO_CLAUDE, --claude, or its installer’s folder); the engine has no entry point for it (ADR 0801 Addendum 23).';

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
    id: 'go.studio_settings',
    title: 'Settings',
    description: 'The settings of the Studio itself (#/studio-settings): its theme. Not a case: the case settings are Go to Case settings.',
    group: 'Go to',
    effect: 'navigate',
    params: [],
    engine: null,
    cli: null,
    mcp: null,
    note: 'A page of the Studio: its settings are the Studio’s own preferences (its user data), no engine call, nothing to script.',
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
    id: 'project.new',
    title: 'New project…',
    description:
      'Choose or make a folder in the system’s folder dialog; the engine makes it a project (rcfd-project.json and the folders cases, meshes, runs, designs and notes; what the folder holds already stays) and the Studio opens it.',
    group: 'Project',
    effect: 'change',
    params: [],
    engine: 'rcfd project init <folder> (ADR 0731), then rcfd-mcp --project <folder> --read-only',
    cli: 'rcfd project init <folder> [--name <name>]',
    mcp: 'create_project',
    note: 'The engine writes the project file, never the Studio (G4, G12); the project is named after its folder. Claude Code makes one with create_project {path, name} on a server without --project.',
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
    description: 'Show or hide the runner’s budgets (.rcfd/runner-config.json: runs at once, threads, memory, polling) to read and set them; a change applies at the next runner start.',
    group: 'Project runs',
    effect: 'change',
    params: [],
    engine: 'runner_config (rcfd-mcp --project <folder> --read-only) reads the budgets; set_runner_config (the run-control server, rcfd-mcp --project <folder>) has the runner write them with its own checks (ADR 0733)',
    cli: 'rcfd-runner config set <key>=<number>... [--unset <key>]... [--base <hash>] --project <folder>',
    mcp: 'set_runner_config',
    note: 'The runner owns the file and writes it atomically, never over a change made since it was read (error 1200, exit 4). Every key is optional; the runner refuses unknown keys. Undo sends the keys back with their values before.',
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

  // ---- Claude Code (ADR 0801 Addendum 23): registered with Claude Code itself, used from its own terminal
  {
    id: 'claude_code.register',
    title: 'Use with Claude Code',
    description:
      'Show whether this folder’s project server is registered with Claude Code, the command to run, and Register (local scope, full access; Claude Code asks before each tool).',
    group: 'Claude Code',
    effect: 'change',
    params: [],
    engine: 'claude mcp add rcfd-project --scope local -- <rcfd-mcp> --project <folder>, run in the folder',
    cli: 'claude mcp add rcfd-project --scope local -- <rcfd-mcp> --project <folder>',
    mcp: null,
    note: CLAUDE_NOTE,
  },
  {
    id: 'claude_code.open',
    title: 'Open Claude Code here',
    description: 'Open a terminal window in the project folder running claude.',
    group: 'Claude Code',
    effect: 'change',
    params: [],
    engine: 'wt.exe -d <folder> claude, or cmd /k claude (Windows); x-terminal-emulator -e claude (Linux)',
    cli: 'cd <folder> && claude',
    mcp: null,
    note: CLAUDE_NOTE,
  },

  // ---- the theme (F-818)
  theme('view.theme_light', 'Theme: Light', 'Draw the Studio in its light theme (DESIGN.md, the default).'),
  theme('view.theme_dark', 'Theme: Dark', 'Draw the Studio in its dark theme; the field colour maps stay the same.'),
  theme('view.theme_contrast', 'Theme: High contrast', 'Draw the Studio in its high-contrast light theme, for bright rooms and projectors.'),
  theme('view.theme_system', 'Theme: System', 'Follow the system: dark when the system is dark, light otherwise.'),

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
