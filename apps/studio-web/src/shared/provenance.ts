// Provenance of the numbers on screen and the command that reproduces a run (F-828; guardrails G17, G3, G2, G16).
// Shared by all three processes, so it uses neither Node nor DOM types.
//
// A number's provenance is what the engine recorded about it: the run, the snapshot, the file it was read from, the
// engine version that made it and its unit, never a value the Studio worked out (G2: no unit is converted, none is
// guessed). A run's reproduce command is built only from the run's own record, runs/<id>/run.json as the runner wrote it,
// read through the open project's read-only project server (`read_project_file`, the tool Claude Code uses). A fact the
// record does not hold is said, never filled in (G6).
//
// A fifth MessagePort (`studio.provenance-port`, after the service, project, case and run ports): the renderer asks for
// a run by its id, or hands over text to copy; the main process checks every message (src/main/provenance/bridge.ts).

/** The one IPC channel main uses to hand the preload script the provenance port (main -> renderer only). */
export const PROVENANCE_PORT_CHANNEL = 'studio.provenance-port';

/** Largest run.json or case file the Studio reads for a run's provenance (the engine pages files at 32 000 bytes). */
export const RECORD_MAX_BYTES = 1_000_000;

/** Largest text the Copy buttons hand to the main process for the clipboard. */
export const COPY_MAX_CHARS = 16_000;

/** `solver_version` of a run.json that has none (crates/rcfd-project, UNKNOWN_SOLVER_VERSION). */
export const UNKNOWN_SOLVER_VERSION = 'unknown';

/**
 * The settings keys of run.json that `rcfd-runner start` takes, and its option for each (crates/rcfd-runner, RunSettings,
 * LaunchSettings and the start command; continue_from and max_steps since ADR 0734 P3). Any other key the record holds
 * (a later one) has no option there, so a command cannot carry it: that is a gap, said in words.
 */
export const RUNNER_START_OPTIONS = {
  threads: '--threads',
  priority: '--priority',
  mem_mb: '--mem-mb',
  set: '--set',
  continue_from: '--continue-from',
  max_steps: '--max-steps',
} as const;

/** Labels of a run record that name a design study's run; they do not change what the solver computes. */
export const RECORD_LABELS = ['study', 'variant', 'condition'] as const;

/** runs/<id>/run.json, the fields provenance uses, verbatim (crates/rcfd-project RunRecord). */
export interface RunRecordFacts {
  readonly id: string;
  /** Project-relative (`cases/cavity.toml`), or absolute for a case outside the project. */
  readonly case: string;
  /** Hash of runs/<id>/case.snapshot, as the runner wrote it (`fnv64:<hex>`, legacy `fnv1a64:<hex>`). */
  readonly caseSnapshotHash: string;
  readonly caseSource: string | null;
  readonly caseDir: string | null;
  /** `rcfd --version` of the solver the runner used when it made the run, or `unknown`. */
  readonly solverVersion: string;
  readonly created: string;
  /** `settings` of run.json, verbatim. */
  readonly settings: Readonly<Record<string, unknown>>;
  /** A mesh the record names, with its hash (records made by rcfd-project), or null. */
  readonly mesh: { readonly path: string; readonly hash: string | null } | null;
  /** study, variant and condition when the record holds them. */
  readonly labels: Readonly<Record<string, string>>;
  /** The run this one reran from its case snapshot (`rerun_of`, ADR 0734 P3), or null. */
  readonly rerunOf: string | null;
}

/**
 * A run's launch record as the runner wrote it (runs/<id>/launch.json, ADR 0734 P1), read through run_status `launch`:
 * the solver's command line exactly as the runner gave it, its working folder and the solver's build.
 */
export interface LaunchFacts {
  readonly launchSeq: number | null;
  /** Program first, every path as passed. */
  readonly argv: readonly string[];
  readonly cwd: string | null;
  readonly solverPath: string | null;
  /** `rcfd --version` of that solver, or null when it did not answer. */
  readonly solverVersion: string | null;
  /** `fnv64:` of the solver executable's bytes, or null when the runner could not read it. */
  readonly exeHash: string | null;
  /** The build's git revision (`rcfd version --json`), or null when the build set none. */
  readonly gitRev: string | null;
  readonly started: string | null;
}

/** A file's hash as the engine's read_project_file gave it, against the hash the run recorded. */
export interface HashCheck {
  readonly path: string;
  /** The engine's hash of the file now, or null when it could not be read. */
  readonly hash: string | null;
  /** The hash the record holds. */
  readonly recorded: string;
  /** Whether they name the same bytes; null when the file could not be read. */
  readonly matches: boolean | null;
  readonly error: string | null;
}

/** The command that runs a run again, built from its record (never from the Studio's own settings or a guess). */
export interface Reproduce {
  /** The argument list, or null when the record cannot give one. */
  readonly argv: readonly string[] | null;
  /** The same as one line of POSIX shell (also read by PowerShell: single quotes, no `$`), or null. */
  readonly text: string | null;
  /** Why the command would not give the same result, in words; empty when it reproduces the run as recorded. */
  readonly gaps: readonly string[];
  /** Recorded facts the command leaves out because they do not change the result (labels of a design study). */
  readonly leftOut: readonly string[];
}

/** What the provenance of a run's numbers draws on: the record, its checks, the engine now and the units. */
export interface RunProvenance {
  readonly run: string;
  /** The project folder as opened. */
  readonly folder: string;
  /** `runs/<id>/run.json`, and the engine's hash of it. */
  readonly recordPath: string;
  readonly recordHash: string | null;
  readonly record: RunRecordFacts | null;
  readonly recordError: string | null;
  /** runs/<id>/case.snapshot against the record's hash. */
  readonly snapshotCheck: HashCheck | null;
  /** The project's case file now against the record's hash (null for a case outside the project or no record). */
  readonly caseCheck: HashCheck | null;
  /** The engine the Studio reads the project with (the project server's MCP serverInfo), or null. */
  readonly engineNow: { readonly name: string; readonly version: string } | null;
  /** The units of runs/<id>/output/monitors.json by series name, and why there are none when so. */
  readonly units: Readonly<Record<string, string>>;
  readonly unitsNote: string | null;
  readonly reproduce: Reproduce;
  /** ADR 0734: the run's latest launch record (null: not launched yet, launched before the runner recorded it, or not read). */
  readonly launch: LaunchFacts | null;
  /** Why there is no launch record, in words, when there is none. */
  readonly launchNote: string | null;
  /** The mesh file now against run.json mesh_hash (`hash_project_file`), or null when the record names no mesh. */
  readonly meshCheck: HashCheck | null;
  /** The unit of the run's time as the engine gives it (run_status `time_unit`), or null when it gives none. */
  readonly timeUnit: string | null;
  /** `rcfd-runner rerun <id>`: the run again from its own case snapshot and settings, and what would still differ. */
  readonly rerun: Reproduce;
  /** When the main process read it (RFC 3339). */
  readonly readAt: string;
}

export interface ProvenanceError {
  readonly code: 'bad_request' | 'no_project' | 'engine' | 'no_port' | 'timeout' | 'clipboard';
  readonly message: string;
}

export type ProvenanceReply<T = unknown> = { readonly ok: true; readonly result: T } | { readonly ok: false; readonly error: ProvenanceError };

/** Preload -> main. */
export type ProvenanceRequest =
  | { readonly op: 'run'; readonly id: number; readonly run: string }
  | { readonly op: 'copy'; readonly id: number; readonly text: string };

export type ProvenanceOp = ProvenanceRequest['op'];
export const PROVENANCE_OPS = ['run', 'copy'] as const satisfies readonly ProvenanceOp[];

/** Main -> preload. */
export type ProvenancePortMessage = { readonly type: 'reply'; readonly id: number; readonly reply: ProvenanceReply };

/** What the preload script puts on `window.studioProvenance`. Each function's arguments are checked again in main. */
export interface ProvenanceBridge {
  /** The provenance of a run of the open project: its record, the checks, the units and the reproduce command. */
  run(run: string): Promise<ProvenanceReply<RunProvenance>>;
  /** Puts text on the system clipboard (the main process's clipboard; the page has no clipboard permission, G10). */
  copy(text: string): Promise<ProvenanceReply<{ readonly chars: number }>>;
}

// ---- the reproduce command

/** One word of a command line, quoted for a POSIX shell when it needs it (as src/shared/actions/runs.ts commandLine). */
export function shellWord(w: string): string {
  return /^[A-Za-z0-9_./:=@+,-]+$/.test(w) ? w : `'${w.replaceAll("'", `'\\''`)}'`;
}

export const shellLine = (argv: readonly string[]): string => argv.map(shellWord).join(' ');

/** Two hash strings name the same bytes: `fnv64:<hex>` and the runner's legacy `fnv1a64:<hex>` are one hash (ADR 0130 D2). */
export function sameHash(a: string, b: string): boolean {
  const hex = (h: string) => (h.startsWith('fnv1a64:') ? `fnv64:${h.slice('fnv1a64:'.length)}` : h);
  return hex(a) === hex(b);
}

/** `rcfd 0.1.0` as `rcfd --version` prints it, from an MCP serverInfo or a service.info `{name, version}`. */
export const versionLine = (e: { readonly name: string; readonly version: string }): string => `${e.name} ${e.version}`;

const json = (v: unknown): string => JSON.stringify(v) ?? String(v);
const isWhole = (v: unknown, min: number): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v >= min;

export interface ReproduceInput {
  readonly record: RunRecordFacts;
  /** The case file the command names: the record's case, resolved against the project folder by the main process. */
  readonly casePath: string;
  readonly folder: string;
  readonly caseCheck: HashCheck | null;
  readonly snapshotCheck: HashCheck | null;
  readonly engineNow: { readonly name: string; readonly version: string } | null;
  /** The mesh now against the record's mesh_hash (null: no mesh recorded, or a mesh outside the project). */
  readonly meshCheck?: HashCheck | null;
}

/** The gaps a recorded mesh leaves (ADR 0734 P4, P5): none when the engine's hash of it now is the recorded one. */
function meshGaps(record: RunRecordFacts, meshCheck: HashCheck | null | undefined): string[] {
  const m = record.mesh;
  if (!m) return [];
  if (m.hash === null) return [`The run names the mesh ${m.path} but run.json records no hash of it, so the Studio cannot check that it is unchanged.`];
  if (!meshCheck) return [`The run names the mesh ${m.path}, outside the project folder, so the Studio cannot check that it is unchanged.`];
  if (meshCheck.matches === false) return [`The mesh ${m.path} changed since the run (hash ${meshCheck.hash} now, ${m.hash} when the run was made).`];
  if (meshCheck.matches === null) return [`The mesh ${m.path} cannot be hashed now (${meshCheck.error ?? 'no reason given'}).`];
  return [];
}

/** The engine-version gap of a recorded run, or none. */
function engineGaps(record: RunRecordFacts, engineNow: { readonly name: string; readonly version: string } | null): string[] {
  if (record.solverVersion === UNKNOWN_SOLVER_VERSION) return ['run.json does not say which engine version made the run.'];
  if (engineNow && versionLine(engineNow) !== record.solverVersion) {
    return [`The run was made by ${record.solverVersion}; the engine here is ${versionLine(engineNow)}, which may compute other values.`];
  }
  return [];
}

/**
 * `rcfd-runner rerun <id> --project <folder>` (ADR 0734 P3): a new run of the run's own case.snapshot with its recorded
 * settings and case folder, so a case file changed since does not matter. The gaps say what still would.
 */
export function rerunCommand(input: { readonly run: string; readonly folder: string; readonly record: RunRecordFacts; readonly snapshotCheck: HashCheck | null; readonly meshCheck?: HashCheck | null; readonly engineNow: { readonly name: string; readonly version: string } | null }): Reproduce {
  const { run, folder, record, snapshotCheck, meshCheck, engineNow } = input;
  const argv = ['rcfd-runner', 'rerun', run, '--project', folder];
  const gaps: string[] = [];
  if (snapshotCheck && snapshotCheck.matches === false) {
    gaps.push(`${snapshotCheck.path} does not match the hash in run.json (${snapshotCheck.hash} now, ${snapshotCheck.recorded} recorded).`);
  } else if (snapshotCheck && snapshotCheck.matches === null) {
    gaps.push(`${snapshotCheck.path} cannot be read now (${snapshotCheck.error ?? 'no reason given'}); the rerun needs it.`);
  }
  gaps.push(...meshGaps(record, meshCheck), ...engineGaps(record, engineNow));
  return { argv, text: shellLine(argv), gaps, leftOut: [] };
}

/**
 * `rcfd-runner start <case> --project <folder> --threads n --priority n [--mem-mb n] [--set path=value]...`, every
 * option from the record's settings, in the record's order for `set`. The gaps say what would make it give another
 * result: a settings key with no option, a case file changed since the run, a snapshot that does not match its record,
 * another engine version.
 */
export function reproduceCommand(input: ReproduceInput): Reproduce {
  const { record, casePath, folder, caseCheck, snapshotCheck, engineNow } = input;
  const gaps: string[] = [];
  const leftOut: string[] = [];
  const s = record.settings;
  const argv: string[] = ['rcfd-runner', 'start', casePath, '--project', folder];
  // threads and priority: the runner's defaults (1 and 0) when the record has none, as RunSettings reads it
  const threads = s['threads'] ?? 1;
  if (isWhole(threads, 1)) argv.push('--threads', String(threads));
  else gaps.push(`run.json settings.threads is ${json(threads)}, not a number of threads rcfd-runner start takes.`);
  const priority = s['priority'] ?? 0;
  if (typeof priority === 'number' && Number.isSafeInteger(priority)) argv.push('--priority', String(priority));
  else gaps.push(`run.json settings.priority is ${json(priority)}, not a priority rcfd-runner start takes.`);
  const mem = s['mem_mb'] ?? null;
  if (mem !== null) {
    if (isWhole(mem, 0)) argv.push('--mem-mb', String(mem));
    else gaps.push(`run.json settings.mem_mb is ${json(mem)}, not a size rcfd-runner start takes.`);
  }
  const set = s['set'] ?? [];
  if (Array.isArray(set) && set.every((x) => typeof x === 'string')) for (const x of set as string[]) argv.push('--set', x);
  else gaps.push(`run.json settings.set is ${json(set)}, not a list of path=value lines.`);
  // ADR 0734 P3: the launch settings of ADR 0200, options of rcfd-runner start since
  const from = s['continue_from'] ?? null;
  if (from !== null) {
    if (typeof from === 'string') argv.push('--continue-from', from);
    else gaps.push(`run.json settings.continue_from is ${json(from)}, not a run id rcfd-runner start takes.`);
  }
  const cap = s['max_steps'] ?? null;
  if (cap !== null) {
    if (isWhole(cap, 1)) argv.push('--max-steps', String(cap));
    else gaps.push(`run.json settings.max_steps is ${json(cap)}, not a step count rcfd-runner start takes.`);
  }
  for (const [k, v] of Object.entries(s)) {
    if (k in RUNNER_START_OPTIONS || v === null || v === undefined) continue;
    gaps.push(`run.json settings.${k} is ${json(v)}; rcfd-runner start has no option for it, so the command runs without it.`);
  }
  gaps.push(...meshGaps(record, input.meshCheck));
  if (snapshotCheck && snapshotCheck.matches === false) {
    gaps.push(`${snapshotCheck.path} does not match the hash in run.json (${snapshotCheck.hash} now, ${snapshotCheck.recorded} recorded).`);
  }
  if (caseCheck === null) {
    gaps.push(`The case ${record.case} is outside the project folder, so the Studio cannot check that it is unchanged since the run.`);
  } else if (caseCheck.matches === false) {
    gaps.push(`${caseCheck.path} changed since the run (hash ${caseCheck.hash} now, ${caseCheck.recorded} when the run was made); the command runs the case as it is now.`);
  } else if (caseCheck.matches === null) {
    gaps.push(`${caseCheck.path} cannot be read now (${caseCheck.error ?? 'no reason given'}); the command needs it.`);
  }
  gaps.push(...engineGaps(record, engineNow));
  for (const k of RECORD_LABELS) if (record.labels[k] !== undefined) leftOut.push(`${k} ${record.labels[k]}`);
  return { argv, text: shellLine(argv), gaps, leftOut };
}

// ---- a number's provenance

/** Where a number on screen was read: the engine's file and the reader that read it. */
export interface NumberSource {
  /** The quantity, in the engine's name (`lid_drag`, `res_u`, `p`, `step`). */
  readonly quantity: string;
  /** The value as shown, and the engine's exact value when it differs (more digits). */
  readonly shown: string;
  readonly exact: string | null;
  /**
   * The unit as the engine gave it: a string (possibly empty, as the engine wrote it), or null with the reason when the
   * engine gave none for this value.
   */
  readonly unit:
    | { readonly kind: 'engine'; readonly unit: string; readonly from: string }
    | { readonly kind: 'none'; readonly why: string }
    /** The unit of this series in the run's output/monitors.json, looked up in the run's provenance when it opens. */
    | { readonly kind: 'monitors'; readonly name: string }
    /** A run's or a snapshot's time: the unit run_status names (`time_unit`, ADR 0734 P2), looked up when it opens. */
    | { readonly kind: 'time' };
  readonly run: string;
  /** The snapshot the value belongs to (index from 0, of `of`), or why it has none. */
  readonly snapshot: { readonly index: number; readonly of: number; readonly step: number; readonly time: number | null } | { readonly none: string };
  /** The step (iteration) of the value, when it has one. */
  readonly step: number | null;
  /** Project-relative file the value was read from. */
  readonly file: string;
  /** The reader: the engine's tool or the Studio's reader of the store. */
  readonly readBy: string;
  /** More facts of this value (a cell index, a sample count), label and text. */
  readonly more?: readonly (readonly [string, string])[];
}

/** One row of the provenance: a label and its text. */
export type Fact = readonly [label: string, text: string];

/** The unit as the engine gave it, in words: the string, where it is written, or why there is none. */
export function unitText(src: NumberSource, run: RunProvenance | null): string {
  const u = src.unit;
  if (u.kind === 'none') return `none: ${u.why}`;
  if (u.kind === 'engine') return u.unit === '' ? `none, empty in ${u.from}` : `${u.unit}, ${u.from}`;
  if (u.kind === 'time') {
    if (!run) return 'reading…';
    return run.timeUnit !== null
      ? `${run.timeUnit}, time_unit of run_status (simulated time; 0 for a steady run)`
      : `none: run_status gives no time_unit (an engine before ADR 0734)`;
  }
  const from = `runs/${src.run}/output/monitors.json`;
  if (!run) return 'reading…';
  const got = run.units[u.name];
  if (got === undefined) return `none: ${run.unitsNote ?? `${from} has no series ${u.name}`}`;
  return got === '' ? `none, empty in ${from}` : `${got}, ${from}`;
}

/** The rows of a number's provenance, with the run's facts when they are known. */
export function numberFacts(src: NumberSource, run: RunProvenance | null): Fact[] {
  const rows: Fact[] = [];
  rows.push(['Value', src.exact !== null && src.exact !== src.shown ? `${src.shown} (${src.exact})` : src.shown]);
  rows.push(['Unit', unitText(src, run)]);
  rows.push(['Run', src.run]);
  rows.push([
    'Snapshot',
    'none' in src.snapshot
      ? src.snapshot.none
      : `${src.snapshot.index + 1} of ${src.snapshot.of}, step ${src.snapshot.step}${src.snapshot.time !== null && src.snapshot.time !== 0 ? `, t = ${src.snapshot.time}` : ''}`,
  ]);
  if (src.step !== null && ('none' in src.snapshot || src.snapshot.step !== src.step)) rows.push(['Step', String(src.step)]);
  rows.push(['File', src.file]);
  rows.push(['Read by', src.readBy]);
  for (const m of src.more ?? []) rows.push(m);
  rows.push(['Engine', run?.record ? run.record.solverVersion : run ? `not known: ${run.recordError ?? 'no run record'}` : 'reading…']);
  return rows;
}

/** The rows of a run's own provenance: its record as the runner wrote it. */
export function runFacts(p: RunProvenance): Fact[] {
  const r = p.record;
  if (!r) return [['Run', p.run], ['Record', `${p.recordPath}: ${p.recordError ?? 'not read'}`]];
  const rows: Fact[] = [
    ['Run', p.run],
    ['Case', r.case],
    ['Created', r.created],
    ['Engine', r.solverVersion],
    ['Settings', JSON.stringify(r.settings)],
    ['Record', `${p.recordPath}${p.recordHash ? `, ${p.recordHash}` : ''}`],
    ['Case snapshot', `runs/${p.run}/case.snapshot, ${r.caseSnapshotHash}`],
  ];
  for (const [k, v] of Object.entries(r.labels)) rows.push([k.charAt(0).toUpperCase() + k.slice(1), v]);
  return rows;
}

/** Why a run has no launch record to show (run_status gave `launch: null`). */
export const NO_LAUNCH_RECORD =
  'Not recorded: the runner writes runs/<id>/launch.json when it launches a run (ADR 0734); this run has not been launched, or was launched before.';

/**
 * "How this ran" (ADR 0734 P5): the solver's command line, its build and the mesh, as the runner recorded them, and the
 * rerun command. A fact not recorded is said, never filled in (G6).
 */
export function howItRanFacts(p: RunProvenance): Fact[] {
  const rows: Fact[] = [];
  const l = p.launch;
  if (l) {
    rows.push(['Command', shellLine(l.argv)]);
    if (l.cwd !== null) rows.push(['Folder', l.cwd]);
    rows.push(['Solver', [l.solverVersion ?? 'version not given', l.solverPath].filter((x) => x !== null).join(', ')]);
    rows.push(['Build', `${l.exeHash ?? 'no executable hash'}, ${l.gitRev !== null ? `git ${l.gitRev}` : 'no git revision (the build set none)'}`]);
    if (l.started !== null) rows.push(['Launched', l.started]);
  } else {
    rows.push(['Command', p.launchNote ?? NO_LAUNCH_RECORD]);
  }
  const m = p.record?.mesh ?? null;
  if (!m) rows.push(['Mesh', 'no mesh file recorded: the case makes its mesh, or the run was made before the runner recorded one']);
  else {
    const c = p.meshCheck;
    const now = m.hash === null ? 'no hash recorded' : !c ? 'not checked: outside the project' : c.matches === true ? 'unchanged' : c.matches === false ? `changed: ${c.hash} now` : `not checked: ${c.error ?? 'not read'}`;
    rows.push(['Mesh', `${m.path}, ${m.hash ?? 'no hash'}; ${now}`]);
  }
  if (p.record?.rerunOf) rows.push(['Rerun of', p.record.rerunOf]);
  if (p.rerun.text) rows.push(['Rerun', p.rerun.text]);
  return rows;
}

/** The provenance as text, for Copy: the facts, then the reproduce command and its gaps. */
export function provenanceText(title: string, facts: readonly Fact[], run: RunProvenance | null): string {
  const lines = [title, ...facts.map(([k, v]) => `${k}: ${v}`)];
  if (run) {
    lines.push(`Record: ${run.recordPath}${run.recordHash ? ` (${run.recordHash})` : ''}`);
    if (run.reproduce.text) lines.push(`Reproduce: ${run.reproduce.text}`);
    for (const g of run.reproduce.gaps) lines.push(`Not reproduced exactly: ${g}`);
  }
  return lines.join('\n');
}

// ---- a number of a service session (F-828: the settings surface, the plots' development view)

/**
 * Where a number of a live service session was read. A session is not a project run: it has no run.json, no stored
 * snapshot and no reproduce command of its own; the facts are the session, the service method that answered, the engine
 * that answered it (service.info) and the unit the engine gave. What the service does not record is said, not filled in.
 */
export interface SessionNumberSource {
  /** The quantity, in the engine's name (`solver.alpha_u`, `res_u`, `lid_drag`, `iteration`). */
  readonly quantity: string;
  readonly shown: string;
  readonly exact: string | null;
  readonly unit: { readonly kind: 'engine'; readonly unit: string; readonly from: string } | { readonly kind: 'none'; readonly why: string };
  readonly session: string;
  /** The iteration the value belongs to, or why it has none. */
  readonly at: { readonly iteration: number } | { readonly none: string };
  /** The service method that answered, with the service's name. */
  readonly readBy: string;
  readonly more?: readonly Fact[];
}

/**
 * What an engine without ADR 0735 records about the file a session was made from: session.create reads a file, but
 * neither session.list nor journal.get names it. An engine with ADR 0735 answers journal.get with `source`.
 */
export const SESSION_FILE_NOT_RECORDED = 'not recorded: the service names no file for a session (session.list and journal.get carry none)';

/** Why a session number has no reproduce command: the service keeps no journal file (`file` null or absent). */
export const SESSION_NO_REPRODUCE =
  'Not recorded: a service session has no reproduce command. journal.get gives its edits, but the service writes no journal file for rcfd run --journal to read.';

/** The session's journal as journal.get answers it (ADR 0735: `source` and `file`; absent from an older engine). */
export interface SessionJournal {
  readonly entries: number;
  readonly caseHash: string;
  /** `{kind, path, hash}` of the session's source; undefined when the engine gives none. */
  readonly source?: { readonly kind: string; readonly path: string | null; readonly hash: string | null };
  /** The journal file the service keeps (`rcfd serve --journal-dir`); null when it keeps none. */
  readonly file?: string | null;
}

/** The session's journal from a journal.get reply, or why it cannot be read. */
export function sessionJournal(reply: unknown): SessionJournal | { readonly error: string } {
  const o = (reply ?? {}) as { entries?: unknown; case_hash?: unknown; source?: unknown; file?: unknown };
  if (!Array.isArray(o.entries) || typeof o.case_hash !== 'string') return { error: 'journal.get answered without entries or case_hash' };
  const src = o.source as { kind?: unknown; path?: unknown; hash?: unknown } | null | undefined;
  const str = (v: unknown) => (typeof v === 'string' ? v : null);
  return {
    entries: o.entries.length,
    caseHash: o.case_hash,
    ...(src && typeof src.kind === 'string' ? { source: { kind: src.kind, path: str(src.path), hash: str(src.hash) } } : {}),
    ...(o.file === undefined ? {} : { file: str(o.file) }),
  };
}

/** The command that reproduces a session from the journal file the service keeps, or null when it keeps none. */
export function sessionReproduce(journal: SessionJournal | { readonly error: string } | null): string | null {
  if (journal === null || 'error' in journal || !journal.file) return null;
  return shellLine(['rcfd', 'run', '--journal', journal.file]);
}

const SOURCE_KIND_WORDS: Readonly<Record<string, string>> = {
  toml: 'none: the session was made from case text (journal.get)',
  settings: 'none: the session was made from settings (journal.get)',
  empty: 'none: the session was made from the engine’s defaults (journal.get)',
};

function caseFileFact(journal: SessionJournal | { readonly error: string } | null): string {
  if (journal === null || 'error' in journal || journal.source === undefined) return SESSION_FILE_NOT_RECORDED;
  const s = journal.source;
  if (s.kind === 'path') return `${s.path ?? 'no path'}${s.hash ? `, hash ${s.hash}` : ''} (journal.get)`;
  return SOURCE_KIND_WORDS[s.kind] ?? `none: the source is ${s.kind} (journal.get)`;
}

/** The facts of a session number, with the engine that answered (service.info) and the session's journal when read. */
export function sessionFacts(
  src: SessionNumberSource,
  engine: { readonly name: string; readonly version: string } | null | 'reading',
  journal: SessionJournal | { readonly error: string } | null,
): Fact[] {
  const u = src.unit;
  const rows: Fact[] = [
    ['Value', src.exact !== null && src.exact !== src.shown ? `${src.shown} (${src.exact})` : src.shown],
    ['Unit', u.kind === 'none' ? `none: ${u.why}` : u.unit === '' ? `none, empty in ${u.from}` : `${u.unit}, ${u.from}`],
    ['Session', src.session],
    ['Iteration', 'none' in src.at ? src.at.none : String(src.at.iteration)],
    ['Read by', src.readBy],
    ['Case file', caseFileFact(journal)],
  ];
  if (journal !== null) {
    rows.push(['Journal', 'error' in journal ? `not read: ${journal.error}` : `${journal.entries} ${journal.entries === 1 ? 'edit' : 'edits'} since the session was made, case hash ${journal.caseHash} (journal.get)`]);
    if (!('error' in journal) && journal.file) rows.push(['Journal file', `${journal.file} (the service keeps it)`]);
  }
  for (const m of src.more ?? []) rows.push(m);
  rows.push(['Engine', engine === 'reading' ? 'reading…' : engine ? `${versionLine(engine)} (service.info)` : 'not known: no service answered service.info']);
  return rows;
}
