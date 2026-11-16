// The command palette's model (F-831; G20, G16, G19): one entry per registered action and Studio command, generated from
// the registries themselves, so nothing is left out (tests/unit/commands-palette.spec.ts fails when one is):
//
//   ACTIONS          src/shared/actions/registry.ts  one per service method (F-827)
//   RUN_ACTIONS      src/shared/actions/runs.ts      run control of a project (F-807)
//   CASE_ACTIONS     src/shared/case.ts              the case files of a project (F-812)
//   STUDIO_COMMANDS  ./studio.ts                     views, the project folder, Claude Code (F-805, F-814)
//
// Each entry carries what the palette shows (title, id, description, group, the shortcut from ./shortcuts.ts, the CLI
// and Claude Code equivalents the registry gives) and the parameters it asks for. `searchPalette` ranks the entries for
// what was typed, by title and by id: exact, then prefix, then word prefixes, then contained, then a fuzzy subsequence.
// Availability is not here: it depends on the window's state, and the renderer asks the registries (src/renderer/commands).
//
// Shared by the renderer and the tests: no Node or DOM types.
import { ACTIONS, type RegisteredAction } from '../actions/registry.js';
import { describeRunAction, RUN_ACTIONS, type RegisteredRunAction } from '../actions/runs.js';
import type { ParamSpec, ParamsSpec } from '../actions/params.js';
import { CASE_ACTIONS, type CaseAction } from '../case.js';
import { GROUP_ORDER, STUDIO_COMMANDS, type PaletteGroup, type PaletteParam, type ParamKind, type StudioCommand } from './studio.js';
import { globalShortcutOf, shortcutsOf, type ShortcutScope } from './shortcuts.js';

export type CommandSource = 'service' | 'run' | 'case' | 'studio';

export interface PaletteEntry {
  /** The registry id (`run.start`, `project.run_case`, `case.save`, `go.runs`). */
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly source: CommandSource;
  readonly group: PaletteGroup;
  readonly effect: 'read' | 'change' | 'navigate';
  /** What the palette asks for before it runs the command, in order. */
  readonly params: readonly PaletteParam[];
  /** The global shortcut, when the command has one. */
  readonly shortcut: string | null;
  /** Every shortcut that runs it, global or of a surface (the case editor's Ctrl+S). */
  readonly shortcuts: readonly { readonly keys: string; readonly scope: ShortcutScope }[];
  /** The engine call, in words; null for a view change. */
  readonly engine: string | null;
  /** The CLI equivalent (a template: <json-params>, <case>, <run>), or null when there is none. */
  readonly cli: string | null;
  /** The Claude Code (MCP) equivalent: the tool, or null when there is none. */
  readonly mcp: string | null;
  /** Why an equivalent is missing, or what differs. */
  readonly note: string | null;
  /** Asks for a confirmation in words before it is dispatched (G18). */
  readonly confirms: boolean;
}

const SERVICE_GROUP_WORDS: Readonly<Record<RegisteredAction['group'], string>> = {
  service: 'service',
  session: 'sessions',
  settings: 'settings',
  mesh: 'mesh',
  run: 'run control',
  results: 'results',
};

/** The palette's parameter of an action parameter: a session id or a project's case or run is chosen from a list. */
function paramOf(name: string, s: ParamSpec, choice?: ParamKind): PaletteParam {
  const kind: ParamKind = choice ?? (s.type === 'session' ? 'session' : s.type);
  return {
    name,
    kind,
    optional: s.optional === true,
    description: s.description,
    ...('minimum' in s && s.minimum !== undefined ? { minimum: s.minimum } : {}),
    ...('maximum' in s && s.maximum !== undefined ? { maximum: s.maximum } : {}),
    ...('enum' in s && s.enum !== undefined ? { enum: s.enum } : {}),
  };
}

const paramsOf = (spec: ParamsSpec, choices: Readonly<Record<string, ParamKind>> = {}): PaletteParam[] =>
  Object.entries(spec).map(([k, s]) => paramOf(k, s, choices[k]));

function shortcutFields(id: string) {
  return {
    shortcut: globalShortcutOf(id)?.keys ?? null,
    shortcuts: shortcutsOf(id).map((s) => ({ keys: s.keys, scope: s.scope })),
  };
}

function fromService(a: RegisteredAction): PaletteEntry {
  return {
    id: a.id,
    title: a.title,
    description: a.description,
    source: 'service',
    group: 'Service session',
    effect: a.effect,
    params: paramsOf(a.params),
    ...shortcutFields(a.id),
    engine: `service method ${a.engine.service} (${SERVICE_GROUP_WORDS[a.group]})`,
    cli: `rcfd call ${a.engine.service} '<json-params>'`,
    mcp: a.mcp.tool,
    note: a.mcp.note ?? null,
    confirms: false,
  };
}

function fromRun(a: RegisteredRunAction): PaletteEntry {
  const d = describeRunAction(a);
  return {
    id: a.id,
    title: a.title,
    description: a.description,
    source: 'run',
    group: 'Project runs',
    effect: 'change',
    params: paramsOf(a.params, { case: 'case', run: 'run' }),
    ...shortcutFields(a.id),
    engine: d.engine,
    cli: d.cli,
    mcp: d.mcp,
    note: [d.mcpNote, a.cliNote].filter(Boolean).join(' ') || null,
    confirms: d.confirms,
  };
}

/** The case file a case action is about: chosen from the project's cases when the palette opens it. */
const CASE_PARAMS: Readonly<Record<string, readonly PaletteParam[]>> = {
  'case.open': [{ name: 'case', kind: 'case', optional: false, description: 'A case file of the project (cases/...).' }],
};

function fromCase(a: CaseAction): PaletteEntry {
  const none = a.mcp.startsWith('none') || a.cli.startsWith('none');
  return {
    id: a.id,
    title: a.title,
    description: a.description,
    source: 'case',
    group: 'Case file',
    effect: 'change',
    params: CASE_PARAMS[a.id] ?? [],
    ...shortcutFields(a.id),
    engine: a.engine.join(', '),
    cli: a.cli.startsWith('none') ? null : a.cli,
    mcp: a.mcp.startsWith('none') ? null : a.mcp,
    note: none ? [a.mcp.startsWith('none') ? `MCP: ${a.mcp}` : null, a.cli.startsWith('none') ? `CLI: ${a.cli}` : null].filter(Boolean).join('. ') : null,
    confirms: a.id === 'case.close',
  };
}

function fromStudio(c: StudioCommand): PaletteEntry {
  return {
    id: c.id,
    title: c.title,
    description: c.description,
    source: 'studio',
    group: c.group,
    effect: c.effect,
    params: c.params,
    ...shortcutFields(c.id),
    engine: c.engine,
    cli: c.cli,
    mcp: c.mcp,
    note: c.note,
    confirms: false,
  };
}

/** Every command of the palette: each registry entry once, in the order of GROUP_ORDER, then registry order. */
export function paletteModel(): PaletteEntry[] {
  const all = [
    ...STUDIO_COMMANDS.map(fromStudio),
    ...RUN_ACTIONS.map(fromRun),
    ...CASE_ACTIONS.map(fromCase),
    ...ACTIONS.map(fromService),
  ];
  return GROUP_ORDER.flatMap((g) => all.filter((e) => e.group === g));
}

/** The ids of every registered action and command the palette must hold (the coverage test's list, G20). */
export function registeredIds(): string[] {
  return [...ACTIONS.map((a) => a.id), ...RUN_ACTIONS.map((a) => a.id), ...CASE_ACTIONS.map((a) => a.id), ...STUDIO_COMMANDS.map((c) => c.id)];
}

// ---- search

const words = (s: string): string[] => s.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);

/** How compactly `q` occurs in `text` as a subsequence (higher is tighter), or -1 when it does not. */
function subsequence(q: string, text: string): number {
  let i = 0;
  let first = -1;
  let last = -1;
  for (let j = 0; j < text.length && i < q.length; j++) {
    if (text[j] === q[i]) {
      if (first < 0) first = j;
      last = j;
      i++;
    }
  }
  if (i < q.length) return -1;
  return 100 - Math.min(99, last - first + 1 - q.length);
}

/** The score of one entry for a query (lower case, trimmed, not empty), or 0 when it does not match. */
export function matchScore(e: PaletteEntry, q: string): number {
  const title = e.title.toLowerCase();
  const id = e.id.toLowerCase();
  if (title === q || id === q) return 1000;
  if (title.startsWith(q)) return 800;
  if (id.startsWith(q)) return 760;
  const tokens = q.split(/\s+/).filter(Boolean);
  const tw = words(e.title);
  const iw = words(e.id);
  const prefixOf = (ws: string[]) => tokens.every((t) => ws.some((w) => w.startsWith(t.replace(/[^a-z0-9]/g, ''))));
  if (prefixOf(tw)) return 600;
  if (prefixOf(iw)) return 560;
  if (id.includes(q)) return 500;
  if (title.includes(q)) return 450;
  const compact = q.replace(/\s+/g, '');
  const ts = subsequence(compact, title);
  if (ts >= 0) return 200 + ts;
  const is = subsequence(compact, id);
  if (is >= 0) return 150 + is;
  const dw = [...words(e.description), ...words(e.group)];
  if (prefixOf([...tw, ...iw, ...dw])) return 50;
  return 0;
}

/** The entries that match `query`, best first (ties in model order); with nothing typed, all of them in model order. */
export function searchPalette(entries: readonly PaletteEntry[], query: string): PaletteEntry[] {
  const q = query.trim().toLowerCase();
  if (q === '') return [...entries];
  return entries
    .map((e, i) => ({ e, i, s: matchScore(e, q) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s || a.i - b.i)
    .map((x) => x.e);
}
