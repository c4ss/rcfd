// The conservative allow-list (docs/ui-restart-plan.md section 8, decision 2; G11) and its mapping onto `claude -p`
// flags. A TypeScript port of the rules of crates/rcfd-claude/src/allow.rs (ADR 0126, ADR 0128 finding 2, ADR 0134;
// that crate was removed and the Rust rules are test support of rcfd-mcp now, crates/rcfd-mcp/tests/claude_allow/),
// because the engine has no entry point the Studio can call for them (F-814 report: missing engine entry point).
//
// Every chat runs with `--permission-mode dontAsk --permission-prompts none`: a tool call no rule allows is denied,
// never prompted and never approved. The rules are the only thing the Studio adds, one `--allowedTools` value each.
// `checkArgs` is an independent check of the whole argument list before every spawn: it refuses any bypass or
// approve-everything flag, any other permission mode, and any `--allowedTools` value that is not a valid rule.
//
// No Electron import: unit-tested in Node.
import { createHash, randomBytes } from 'node:crypto';
import { mkdir, readFile, realpath, rename, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';

/** One rule. Shell and file rules always carry a pattern; a bare `Bash` or `Edit` cannot be expressed. */
export type AllowRule =
  | { readonly kind: 'tool'; readonly name: string }
  | { readonly kind: 'bash' | 'powershell' | 'edit' | 'read'; readonly pattern: string };

/** Tools whose bare name would allow every use of them; they need a pattern. */
const NEED_PATTERN = ['Bash', 'PowerShell', 'Edit', 'Write', 'MultiEdit', 'Read', 'NotebookEdit', 'WebFetch'];

/** Flags the Studio never passes: they change who approves or widen access beyond the project folder. */
export const FORBIDDEN_FLAGS = ['--settings', '--permission-prompt-tool', '--add-dir', '--allow-dangerously-skip-permissions'];

/** Names no file rule may reach, lowercase (ADR 0128 finding 2). */
const CONTROL = ['.mcp.json', 'claude.md', '.claude', '.rcfd', 'rcfd-project.json'];

/** Where the per-project allow-list file lives, relative to the project folder (ADR 0134 W48-11 (a)). */
export const ALLOW_FILE = '.rcfd/claude-allow.json';

/** The one MCP server of the app-owned config (ADR 0134 W48-10 (a)): its tools are `mcp__rcfd_project__<tool>`. */
export const PINNED_SERVER = 'rcfd_project';

/**
 * The read-class project tools: the Studio's first allow-list when the folder has no allow-list file (ADR 0134
 * W48-11 (a), ADR 0195 P5). Read-only tools of the engine's project server, nothing that edits, runs or deletes.
 */
export const PROJECT_READ_TOOLS = [
  'project_info',
  'list_runs',
  'run_status',
  'read_metrics',
  'read_residuals',
  'field_summary',
  'render_field',
  'compare_runs',
  'list_changes',
  'read_project_file',
  'wait_run',
  'read_feed',
  'check_case_mesh',
  'preview_mesh',
  'list_meshes',
  // ADR 0733 (the maintainer, 2026-11-17): the runner's budgets, read-only
  'runner_config',
] as const;

/**
 * ADR 0736 (the maintainer, 2026-11-17): the project tools of the class `delete`. No allow-list holds them, by default
 * or by a rule added later ("Allow this from now on" included), so Claude Code is never allowed one without asking;
 * with this panel's `--permission-mode dontAsk` it is refused. The same list is crates/rcfd-mcp/tests/claude_allow.
 */
export const DELETE_TOOLS = ['empty_trash'] as const;

/** An MCP tool name (`mcp__<server>__<tool>`) whose tool is of the class delete, whatever the server is called. */
export function isDeleteTool(name: string): boolean {
  if (!name.startsWith('mcp__')) return false;
  const rest = name.slice('mcp__'.length);
  const at = rest.lastIndexOf('__');
  return at >= 0 && (DELETE_TOOLS as readonly string[]).includes(rest.slice(at + 2));
}

/** A whole MCP server (`mcp__<server>`, no tool): Claude Code reads it as every tool of the server, delete included. */
export function isWholeServer(name: string): boolean {
  return name.startsWith('mcp__') && !name.slice('mcp__'.length).includes('__');
}

export class RuleError extends Error {
  constructor(
    readonly rule: string,
    readonly reason: string,
  ) {
    super(`allow-list rule ${JSON.stringify(rule)} rejected: ${reason}`);
  }
}

export class ForbiddenArgument extends Error {
  constructor(readonly argument: string) {
    super(`refusing to start claude: forbidden argument ${JSON.stringify(argument)}`);
  }
}

// eslint-disable-next-line no-control-regex
const CONTROL_CHARS = /[\u0000-\u001f\u007f-\u009f]/;

function checkText(s: string): string | null {
  if (s.length === 0) return 'empty pattern';
  if (s.trim() !== s) return 'leading or trailing whitespace';
  if (CONTROL_CHARS.test(s) || /[(),]/.test(s)) return "contains '(', ')', ',' or a control character";
  if (s.startsWith('-')) return "starts with '-'";
  const low = s.toLowerCase();
  if (low.includes('dangerously') || low.includes('bypasspermissions')) return 'contains a permission-bypass token';
  return null;
}

function checkCommand(s: string): string | null {
  const t = checkText(s);
  if (t) return t;
  const first = s.split(/\s+/)[0] ?? '';
  if (/[*?]/.test(first)) return 'the command name must be literal (no wildcard-only shell rule)';
  return null;
}

const stripDot = (s: string): string => s.replace(/^(\.\/)+/, '');

/** Glob match of one segment: `*` any run, `?` one character, `[...]` any one character (wider than a class). */
function glob(p: string, s: string): boolean {
  if (p.length === 0) return s.length === 0;
  const c = p[0]!;
  const rest = p.slice(1);
  if (c === '*') {
    for (let i = 0; i <= s.length; i++) if (glob(rest, s.slice(i))) return true;
    return false;
  }
  if (c === '?') return s.length > 0 && glob(rest, s.slice(1));
  if (c === '[') {
    const k = rest.indexOf(']');
    if (k >= 0) return s.length > 0 && glob(rest.slice(k + 1), s.slice(1));
    return s[0] === '[' && glob(rest, s.slice(1));
  }
  return s[0] === c && glob(rest, s.slice(1));
}

/** Does one path segment name a control file? ASCII case and trailing dots and spaces are ignored (Windows does). */
function controlSegment(seg: string, first: boolean): boolean {
  const s = seg.replace(/[. ]+$/, '').toLowerCase();
  if (/[*?[]/.test(s)) return first && CONTROL.some((c) => glob(s, c));
  return CONTROL.includes(s);
}

function checkPath(s: string): string | null {
  const t = checkText(s);
  if (t) return t;
  const p = stripDot(s);
  if (/^[/~]/.test(p) || /[\\:]/.test(p)) return "path must be project-relative with '/' separators";
  if (p.split('/').some((seg) => seg === '..')) return 'path must not leave the project folder';
  if (!/[\p{L}\p{N}]/u.test(p)) return 'pattern must name something (no wildcard-only file rule)';
  if (p.includes('~')) return "'~' is refused (a short 8.3 name can reach a control file)";
  const segs = p.split('/').filter((x) => x !== '' && x !== '.');
  if (segs.some((x, i) => controlSegment(x, i === 0))) {
    return 'path reaches a file that configures Claude or the project (.mcp.json, CLAUDE.md, .claude, .rcfd, rcfd-project.json)';
  }
  return null;
}

/** The `--allowedTools` value of a rule. */
export function flagValue(r: AllowRule): string {
  switch (r.kind) {
    case 'tool':
      return r.name;
    case 'bash':
      return `Bash(${r.pattern})`;
    case 'powershell':
      return `PowerShell(${r.pattern})`;
    case 'edit':
      return `Edit(./${stripDot(r.pattern)})`;
    case 'read':
      return `Read(./${stripDot(r.pattern)})`;
  }
}

/** Why the conservative policy rejects `r`, or null. */
export function ruleProblem(r: AllowRule): string | null {
  switch (r.kind) {
    case 'tool':
      if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(r.name)) return "tool name must be letters, digits and '_'";
      if (NEED_PATTERN.includes(r.name) || checkText(r.name)) return 'this tool needs a pattern, e.g. Bash(cmd) or Edit(path)';
      if (isDeleteTool(r.name)) return 'a tool of the class delete is never on an allow-list: Claude Code asks for every call (ADR 0736)';
      if (isWholeServer(r.name)) return 'a rule naming a whole MCP server would allow its tools of the class delete: name each tool (ADR 0736)';
      return null;
    case 'bash':
    case 'powershell':
      return checkCommand(r.pattern);
    case 'edit':
    case 'read':
      return checkPath(r.pattern);
  }
}

/** The rule a flag value has the shape of, not yet validated. */
export function ruleShape(s: string): AllowRule | null {
  const open = s.indexOf('(');
  if (open < 0) return { kind: 'tool', name: s };
  if (!s.endsWith(')')) return null;
  const inner = s.slice(open + 1, -1);
  switch (s.slice(0, open)) {
    case 'Bash':
      return { kind: 'bash', pattern: inner };
    case 'PowerShell':
      return { kind: 'powershell', pattern: inner };
    case 'Edit':
      return { kind: 'edit', pattern: stripDot(inner) };
    case 'Read':
      return { kind: 'read', pattern: stripDot(inner) };
    default:
      return null;
  }
}

/** Parses a flag value into a valid rule; null when it is not rule-shaped or the policy rejects it. */
export function parseRule(s: string): AllowRule | null {
  const r = ruleShape(s);
  return r && ruleProblem(r) === null ? r : null;
}

/** A validated list of rules, in insertion order, without duplicates. */
export class AllowList {
  private readonly list: AllowRule[] = [];

  add(r: AllowRule): this {
    const why = ruleProblem(r);
    if (why) throw new RuleError(flagValue(r), why);
    if (!this.list.some((x) => flagValue(x) === flagValue(r))) this.list.push(r);
    return this;
  }

  get rules(): readonly AllowRule[] {
    return this.list;
  }

  get values(): string[] {
    return this.list.map(flagValue);
  }

  /** The permission flags for `claude -p`: always `dontAsk` with prompts off, then one argument per rule. */
  permissionArgs(): string[] {
    const a = ['--permission-mode', 'dontAsk', '--permission-prompts', 'none'];
    if (this.list.length > 0) a.push('--allowedTools', ...this.values);
    return a;
  }

  /** The Studio's first list: the read-class project tools of the app-owned server, nothing else. */
  static initial(): AllowList {
    const l = new AllowList();
    for (const t of PROJECT_READ_TOOLS) l.add({ kind: 'tool', name: `mcp__${PINNED_SERVER}__${t}` });
    return l;
  }
}

/**
 * Independent check of a complete argument list before spawn. Refuses any bypass or approve-everything flag, the
 * forbidden flags, `--allowedTools=` forms (the CLI splits those on commas), any permission mode other than one
 * `--permission-mode dontAsk`, prompts other than one `--permission-prompts none`, and any `--allowedTools` value that
 * is not a valid rule. Throws ForbiddenArgument.
 */
export function checkArgs(args: readonly string[]): void {
  let modes = 0;
  let prompts = 0;
  let inTools = false;
  args.forEach((a, i) => {
    const low = a.toLowerCase();
    if (low.includes('dangerously') || low.includes('bypasspermissions') || low.includes('skip-permissions')) throw new ForbiddenArgument(a);
    if (FORBIDDEN_FLAGS.some((f) => low === f || low.startsWith(`${f}=`))) throw new ForbiddenArgument(a);
    if (low.startsWith('--allowedtools=') || low.startsWith('--allowed-tools=')) throw new ForbiddenArgument(a);
    if (a.startsWith('-')) inTools = false;
    else if (inTools && parseRule(a) === null) throw new ForbiddenArgument(a);
    if (low.startsWith('--permission-mode')) {
      modes++;
      if (a !== '--permission-mode' || args[i + 1] !== 'dontAsk') throw new ForbiddenArgument(a);
    }
    if (low.startsWith('--permission-prompts')) {
      prompts++;
      if (a !== '--permission-prompts' || args[i + 1] !== 'none') throw new ForbiddenArgument(a);
    }
    if (a === '--allowedTools' || a === '--allowed-tools') inTools = true;
  });
  if (modes !== 1 || prompts !== 1) {
    throw new ForbiddenArgument("exactly one '--permission-mode dontAsk' and one '--permission-prompts none' are required");
  }
}

/** The components of `p`, `.` dropped and `..` applied, lowercased on Windows. */
function lexical(p: string): string[] {
  const parts = path.resolve(p).split(/[\\/]+/).filter((x) => x !== '');
  return process.platform === 'win32' ? parts.map((x) => x.toLowerCase()) : parts;
}

const inside = (base: string[], p: string[]): boolean => p.length >= base.length && base.every((b, i) => p[i] === b);

/**
 * `checkArgs` plus the MCP pinning rules of ADR 0134 W48-10 (a) for a chat whose working folder is `project`:
 * `--strict-mcp-config` and `--mcp-config` only as separate arguments, each at most once, `--mcp-config` only with
 * `--strict-mcp-config` and followed by exactly one value, an absolute path of an existing file outside the project
 * (lexically and after resolving links). Throws ForbiddenArgument.
 */
export async function checkArgsIn(args: readonly string[], project: string): Promise<void> {
  checkArgs(args);
  let strict = 0;
  let configs = 0;
  for (let i = 0; i < args.length; i++) {
    const a = args[i]!;
    const low = a.toLowerCase();
    if (low.startsWith('--strict-mcp-config')) {
      if (a !== '--strict-mcp-config') throw new ForbiddenArgument(a);
      strict++;
    } else if (low.startsWith('--mcp-config')) {
      if (a !== '--mcp-config') throw new ForbiddenArgument(a);
      configs++;
      const value = args[i + 1];
      if (value === undefined) throw new ForbiddenArgument('--mcp-config without a file');
      const next = args[i + 2];
      if (next !== undefined && !next.startsWith('-')) throw new ForbiddenArgument(next);
      const why = await mcpConfigOutside(value, project);
      if (why) throw new ForbiddenArgument(`--mcp-config ${value}: ${why}`);
    }
  }
  if (strict > 1 || configs > 1) throw new ForbiddenArgument('--strict-mcp-config and --mcp-config may each appear only once');
  if (configs === 1 && strict === 0) throw new ForbiddenArgument('--mcp-config without --strict-mcp-config');
}

async function mcpConfigOutside(value: string, project: string): Promise<string | null> {
  const t = value.trimStart();
  if (t === '' || /^[-{[]/.test(t)) return 'a config file path is required (no inline JSON, no flag)';
  if (!path.isAbsolute(value)) return 'the path must be absolute (a relative one resolves inside the project folder)';
  if (inside(lexical(project), lexical(value))) return 'the file is inside the project folder';
  try {
    const [cp, cf] = await Promise.all([realpath(project), realpath(value)]);
    if (!(await stat(cf)).isFile()) return 'not a file';
    if (inside(lexical(cp), lexical(cf))) return 'the file resolves inside the project folder';
  } catch (e) {
    return (e as Error).message;
  }
  return null;
}

/** `file` relative to `project` with '/' separators, or null when it is outside (ASCII case-insensitive on Windows). */
export function projectRelative(project: string, file: string): string | null {
  const norm = (s: string) => s.replace(/\\/g, '/').replace(/\/+$/, '');
  const f = norm(file);
  if (!path.isAbsolute(file) && !f.startsWith('/')) return f;
  const p = norm(project);
  const win = process.platform === 'win32';
  const pc = win ? p.toLowerCase() : p;
  const fc = win ? f.toLowerCase() : f;
  return fc.startsWith(`${pc}/`) ? f.slice(p.length + 1) : null;
}

/**
 * The narrowest rule that would allow a denied call, for display ("this rule would allow it"). Null when no safe exact
 * rule exists (a file outside the project, a command with wildcards). The Studio adds it to the allow-list file only
 * when the user confirms that exact rule ("Allow this from now on", `addAllowRule` below; the maintainer, 2026-11-17).
 */
export function suggest(tool: string, input: unknown, project: string): AllowRule | null {
  const field = (k: string): string | null => {
    if (typeof input !== 'object' || input === null) return null;
    const v = (input as Record<string, unknown>)[k];
    return typeof v === 'string' ? v : null;
  };
  let rule: AllowRule;
  switch (tool) {
    case 'Bash':
    case 'PowerShell': {
      const cmd = field('command');
      if (cmd === null || /[*?]/.test(cmd)) return null;
      rule = { kind: tool === 'Bash' ? 'bash' : 'powershell', pattern: cmd };
      break;
    }
    case 'Write':
    case 'Edit':
    case 'MultiEdit':
    case 'NotebookEdit':
    case 'Read': {
      const f = field('file_path') ?? field('notebook_path');
      const rel = f === null ? null : projectRelative(project, f);
      if (rel === null || /[*?[\]!]/.test(rel)) return null;
      rule = { kind: tool === 'Read' ? 'read' : 'edit', pattern: rel };
      break;
    }
    default:
      rule = { kind: 'tool', name: tool };
  }
  return ruleProblem(rule) === null ? rule : null;
}

/** What the allow-list file gave: the list, or why the file is refused as a whole. */
export type AllowFileResult =
  | { readonly kind: 'absent' }
  | { readonly kind: 'list'; readonly list: AllowList }
  | { readonly kind: 'refused'; readonly reason: string };

/**
 * Reads `.rcfd/claude-allow.json` (`{"version": 1, "rules": ["<flag value>", ...]}`, nothing else). Every rule is
 * validated by the same policy; one invalid rule, an unknown key, another version or malformed JSON refuses the whole
 * file, so nothing from a tampered file is passed.
 */
export async function readAllowFile(project: string): Promise<AllowFileResult> {
  let text: string;
  try {
    text = await readFile(path.join(project, ...ALLOW_FILE.split('/')), 'utf8');
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === 'ENOENT') return { kind: 'absent' };
    return { kind: 'refused', reason: `${ALLOW_FILE}: ${(e as Error).message}` };
  }
  return parseAllowFile(text);
}

export function parseAllowFile(text: string): AllowFileResult {
  const refuse = (reason: string): AllowFileResult => ({ kind: 'refused', reason: `${ALLOW_FILE}: ${reason}; the allow-list file is not used` });
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch (e) {
    return refuse(`not JSON (${(e as Error).message})`);
  }
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return refuse('not a JSON object');
  const o = v as Record<string, unknown>;
  const extra = Object.keys(o).filter((k) => k !== 'version' && k !== 'rules');
  if (extra.length > 0) return refuse(`unknown key ${JSON.stringify(extra[0])}`);
  if (o['version'] !== 1) return refuse(`version ${JSON.stringify(o['version'])} is not 1, the only version this build reads`);
  if (!Array.isArray(o['rules'])) return refuse("'rules' must be an array of strings");
  const list = new AllowList();
  for (const [k, s] of (o['rules'] as unknown[]).entries()) {
    if (typeof s !== 'string') return refuse(`rule ${k + 1} is not a string`);
    const shape = ruleShape(s);
    if (!shape) return refuse(`rule ${k + 1}, ${JSON.stringify(s)}, is not a rule (a tool name, Bash(..), PowerShell(..), Edit(..) or Read(..))`);
    const why = ruleProblem(shape);
    if (why) return refuse(`rule ${k + 1}, ${JSON.stringify(s)}: ${why}`);
    list.add(shape);
  }
  return { kind: 'list', list };
}

// ---- adding a rule to the allow-list file (F-814, the maintainer's approval of 2026-11-17)
//
// "Allow this from now on" on a denied call: the rule the Studio suggested for it, after the user confirmed that exact
// rule, is added to `.rcfd/claude-allow.json`. The main process writes the file: never over a file another writer
// changed since it was read (G18, the hash the panel was shown), as a whole new file renamed over the old one (the one
// rename in a project folder, tests/unit/history.spec.ts), and only a list that passes the same validation as a list
// read from the file, and the argument check of every spawn (checkArgs).

/** The SHA-256 (hex) of the allow-list file's text, or null when there is no file. */
export const allowFileHash = (text: string | null): string | null => (text === null ? null : createHash('sha256').update(text).digest('hex'));

/** The allow-list file's text, or null when there is none. */
export async function readAllowText(project: string): Promise<string | null> {
  try {
    return await readFile(path.join(project, ...ALLOW_FILE.split('/')), 'utf8');
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === 'ENOENT') return null;
    throw e;
  }
}

/** The text of an allow-list file holding exactly `rules` (as flag values), in the one shape `parseAllowFile` reads. */
export const allowFileText = (rules: readonly string[]): string => `${JSON.stringify({ version: 1, rules }, null, 2)}
`;

/**
 * The new text of the allow-list file with `rule` added, or why it is refused. `text` is the file as it is (null: no
 * file, so the list in use is the Studio's first list, which the new file keeps, then the rule). Refused: a value that
 * is not a valid rule (the policy of the file's own rules: no bare shell or file tool, no wildcard-only rule, no control
 * file, nothing outside the project), a rule already in the list, a file that is refused as it is, and a list the
 * argument check of a spawn would refuse.
 */
export function withRule(text: string | null, rule: string): { readonly text: string; readonly rules: readonly string[] } | { readonly refused: string } {
  const shape = ruleShape(rule);
  if (!shape) return { refused: `${JSON.stringify(rule)} is not a rule (a tool name, Bash(..), PowerShell(..), Edit(..) or Read(..))` };
  const why = ruleProblem(shape);
  if (why) return { refused: `the rule ${JSON.stringify(rule)} is refused: ${why}` };
  const current = text === null ? { kind: 'list' as const, list: AllowList.initial() } : parseAllowFile(text);
  if (current.kind === 'refused') return { refused: `${current.reason}; correct the file before a rule is added to it` };
  if (current.kind !== 'list') return { refused: `${ALLOW_FILE} cannot be read` };
  const value = flagValue(shape);
  if (current.list.values.includes(value)) return { refused: `${value} is already allowed by ${ALLOW_FILE}` };
  const rules = [...current.list.values, value];
  const next = allowFileText(rules);
  // the same validation as a file read from the folder, and the argument check of every spawn
  const again = parseAllowFile(next);
  if (again.kind !== 'list') return { refused: again.kind === 'refused' ? again.reason : 'the new list cannot be read back' };
  try {
    checkArgs(again.list.permissionArgs());
  } catch (e) {
    return { refused: (e as Error).message };
  }
  return { text: next, rules: again.list.values };
}

/**
 * Writes `text` as the allow-list file if the file still has the hash `base` (null: there is still no file): a new
 * file beside it, then renamed over it. Returns false, and writes nothing, when the file changed since it was read.
 * The check and the rename are moments apart; a writer in that gap is not seen (as with the service's settings).
 */
export async function writeAllowFile(project: string, text: string, base: string | null): Promise<boolean> {
  const file = path.join(project, ...ALLOW_FILE.split('/'));
  if (allowFileHash(await readAllowText(project)) !== base) return false;
  await mkdir(path.dirname(file), { recursive: true });
  const tmp = `${file}.${process.pid}.${randomBytes(4).toString('hex')}.tmp`;
  await writeFile(tmp, text, 'utf8');
  await rename(tmp, file);
  return true;
}
