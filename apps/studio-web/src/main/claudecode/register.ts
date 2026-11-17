// Claude Code for the open project (ADR 0801 Addendum 23): the Studio embeds no Claude Code. It registers the engine's
// project server with Claude Code itself and opens Claude Code in a terminal in the project folder; what Claude Code
// then changes through the server comes back on the engine's change feed like any writer's (src/main/project/session.ts).
//
// - Registration: `claude mcp add rcfd-project --scope local -- <rcfd-mcp> --project <folder>`, run with the project
//   folder as the working folder (local scope is keyed by the folder). No --read-only: Claude Code's own permission
//   prompts govern the tools. Its status is `claude mcp get rcfd-project`, run in the folder: "registered" (this
//   engine, this folder), "not registered", or registered with another command (Re-register removes it from the local
//   scope, `claude mcp remove rcfd-project --scope local`, and adds it again). Checked on Claude Code 2.1.326: `get`
//   exits 1 with `No MCP server named "rcfd-project"` when there is none, and prints `Scope:`, `Command:` and `Args:`
//   lines when there is one; `add` of a name already there exits 1.
// - `claude` is found by src/main/engine/locate.ts (locateClaude): RCFD_STUDIO_CLAUDE, --claude, its installer's
//   folder; never PATH. Not found, the page shows the command to copy, and says why.
// - Launch: ./terminal.ts.
//
// No Electron import: the unit tests drive it with stand-ins.
import path from 'node:path';
import { CLAUDE_MCP_NAME, type ClaudeCodeOutcome, type ClaudeCodeStatus, type ClaudeCodeWhat, type ClaudeRegistration } from '../../shared/project.js';
import type { ClaudeLocation } from '../engine/locate.js';

/** What one run of claude gave. */
export interface RunResult {
  readonly code: number | null;
  readonly stdout: string;
  readonly stderr: string;
}

export interface ClaudeCodeDeps {
  /** Where claude is (read once at start). */
  readonly claude: ClaudeLocation;
  /** The engine's rcfd-mcp, or null when the Studio has none. */
  engine(): string | null;
  /** The open project's folder, or null. */
  folder(): string | null;
  /** Runs `exe args` in `cwd` and resolves with its exit code and output (never rejects). */
  run(exe: string, args: readonly string[], cwd: string): Promise<RunResult>;
  /** Puts text on the system clipboard. */
  copy(text: string): void;
  /** Opens a terminal in `folder` running `claude` (./terminal.ts). */
  launch(claude: string, folder: string): Promise<ClaudeCodeOutcome>;
  now(): string;
  platform?: NodeJS.Platform;
}

/** The arguments of the registration after `claude`. */
export function addArgs(engine: string, folder: string): string[] {
  return ['mcp', 'add', CLAUDE_MCP_NAME, '--scope', 'local', '--', engine, '--project', folder];
}

/** The command a person runs in the project folder, each path in double quotes. */
export function addCommand(engine: string, folder: string): string {
  return `claude mcp add ${CLAUDE_MCP_NAME} --scope local -- "${engine}" --project "${folder}"`;
}

/** What `claude mcp get <name>` printed, as far as the Studio reads it. */
export interface GetAnswer {
  readonly found: boolean;
  readonly scope: string | null;
  readonly command: string | null;
  readonly args: string | null;
}

/** Reads the answer of `claude mcp get`; null when it is neither a server nor "No MCP server named". */
export function parseGet(r: RunResult): GetAnswer | null {
  const text = `${r.stdout}\n${r.stderr}`;
  if (/No MCP server named/i.test(text)) return { found: false, scope: null, command: null, args: null };
  if (r.code !== 0) return null;
  const line = (key: string): string | null => {
    const m = new RegExp(`^\\s*${key}:[ \\t]*(.*)$`, 'mi').exec(r.stdout);
    return m ? m[1]!.trim() : null;
  };
  const command = line('Command');
  if (command === null) return null;
  return { found: true, scope: line('Scope'), command, args: line('Args') ?? '' };
}

const samePath = (a: string, b: string, platform: NodeJS.Platform): boolean => {
  const n = (p: string) => {
    const r = (platform === 'win32' ? path.win32 : path.posix).normalize(p.trim()).replace(/[\\/]+$/, '');
    return platform === 'win32' ? r.toLowerCase() : r;
  };
  return n(a) === n(b);
};

/** Whether a registration found is this engine for this folder. */
export function registrationOf(a: GetAnswer, engine: string, folder: string, platform: NodeJS.Platform): ClaudeRegistration {
  if (!a.found) return 'not_registered';
  // a server of that name in another scope (user, project) is not this folder's local registration
  if (a.scope !== null && !/^local/i.test(a.scope)) return 'not_registered';
  const args = (a.args ?? '').trim();
  const m = /^--project\s+(.+)$/.exec(args);
  return a.command !== null && samePath(a.command, engine, platform) && m !== null && samePath(m[1]!, folder, platform) ? 'registered' : 'other';
}

export class ClaudeCode {
  private last: ClaudeCodeStatus | null = null;
  /** One operation at a time: a status read while a registration runs would read the half-done state. */
  private queue: Promise<unknown> = Promise.resolve();

  constructor(private readonly deps: ClaudeCodeDeps) {}

  private get platform(): NodeJS.Platform {
    return this.deps.platform ?? process.platform;
  }

  /** One operation of the project port's `claudeCode`. */
  do(what: ClaudeCodeWhat): Promise<ClaudeCodeStatus | ClaudeCodeOutcome> {
    const next = this.queue.then(() => this.run(what));
    this.queue = next.catch(() => undefined);
    return next;
  }

  private async run(what: ClaudeCodeWhat): Promise<ClaudeCodeStatus | ClaudeCodeOutcome> {
    switch (what) {
      case 'status':
        return this.status(null);
      case 'register':
        return this.register();
      case 'copy': {
        const folder = this.deps.folder();
        const engine = this.deps.engine();
        if (!folder || !engine) return this.status('Nothing to copy: open a project first.');
        this.deps.copy(addCommand(engine, folder));
        return this.status('Copied. Run it in a terminal in the project folder.', this.last);
      }
      case 'open':
        return this.open();
    }
  }

  private base(detail: string | null): Omit<ClaudeCodeStatus, 'registration' | 'registeredAs'> {
    const folder = this.deps.folder();
    const engine = this.deps.engine();
    return { claude: this.deps.claude, folder, command: folder && engine ? addCommand(engine, folder) : null, detail, readAt: this.deps.now() };
  }

  /** Reads the registration (or keeps `reuse`'s, for a copy), with `detail` to show. */
  private async status(detail: string | null, reuse: ClaudeCodeStatus | null = null): Promise<ClaudeCodeStatus> {
    const folder = this.deps.folder();
    const engine = this.deps.engine();
    const claude = this.deps.claude;
    let s: ClaudeCodeStatus;
    if (!folder || !engine) s = { ...this.base(detail), registration: 'none', registeredAs: null };
    else if (reuse && reuse.folder === folder) s = { ...reuse, ...this.base(detail), registration: reuse.registration, registeredAs: reuse.registeredAs };
    else if (!claude.found) s = { ...this.base(detail ?? claude.reason), registration: 'unknown', registeredAs: null };
    else {
      const r = await this.deps.run(claude.path, ['mcp', 'get', CLAUDE_MCP_NAME], folder);
      const a = parseGet(r);
      if (a === null) {
        const said = (r.stderr || r.stdout).trim().split(/\r?\n/).slice(-3).join(' ');
        s = { ...this.base(detail ?? `claude mcp get did not answer as expected (exit ${r.code ?? 'none'}): ${said || 'no output'}`), registration: 'unknown', registeredAs: null };
      } else {
        const registration = registrationOf(a, engine, folder, this.platform);
        s = { ...this.base(detail), registration, registeredAs: registration === 'other' ? `${a.command} ${a.args ?? ''}`.trim() : null };
      }
    }
    this.last = s;
    return s;
  }

  private async register(): Promise<ClaudeCodeStatus> {
    const folder = this.deps.folder();
    const engine = this.deps.engine();
    const claude = this.deps.claude;
    if (!folder || !engine) return this.status('Open a project first: the registration is for its folder.');
    if (!claude.found) return this.status(claude.reason);
    const now = await this.status(null);
    if (now.registration === 'registered') return this.status('Already registered for this folder.');
    if (now.registration === 'other') {
      const rm = await this.deps.run(claude.path, ['mcp', 'remove', CLAUDE_MCP_NAME, '--scope', 'local'], folder);
      if (rm.code !== 0) return this.status(`claude mcp remove failed (exit ${rm.code ?? 'none'}): ${(rm.stderr || rm.stdout).trim()}`);
    }
    const add = await this.deps.run(claude.path, addArgs(engine, folder), folder);
    if (add.code !== 0) return this.status(`claude mcp add failed (exit ${add.code ?? 'none'}): ${(add.stderr || add.stdout).trim()}`);
    return this.status(now.registration === 'other' ? 'Registered again with this engine.' : 'Registered. Claude Code started in this folder now has the project tools.');
  }

  private async open(): Promise<ClaudeCodeOutcome> {
    const folder = this.deps.folder();
    const claude = this.deps.claude;
    if (!folder) return { ok: false, message: 'Open a project first: Claude Code opens in its folder.' };
    if (!claude.found) return { ok: false, message: claude.reason };
    return this.deps.launch(claude.path, folder);
  }
}
