// Starting `claude`: the session listing (`claude agents --json`) and one `claude -p` process per message, in the
// project folder, the message on stdin as one stream-json line (never an argument), events to a callback, cancel by
// killing the process tree. A port of crates/rcfd-claude/src/driver.rs (ADR 0126, ADR 0128 finding 5, ADR 0134).
//
// - Every argument list is built by `chatArgs` and checked by `checkArgsIn` (allow.ts) right before the spawn.
// - The CLI's first line must be a `system/init` that reports `permissionMode` `dontAsk`; anything with an action
//   before it, or another mode, kills the turn at once (`aborted`) and the offending line is not passed on.
// - No window opens on Windows (`windowsHide`, the CREATE_NO_WINDOW of ADR 0128 finding 17); no shell is used.
//
// No Electron import: unit-tested in Node against the fake claude of tests/support/fake-claude.mjs.
import { spawn, type ChildProcess } from 'node:child_process';
import type { AllowList } from './allow.js';
import { checkArgsIn } from './allow.js';
import type { ClaudeConfig } from './config.js';
import { parseAgents, type AgentEntry } from './discover.js';
import { parseLine, uncheckedProblem, type StreamEvent } from './stream.js';

/** Time limit of `claude agents --json` (ms). */
export const LISTING_TIMEOUT_MS = 20_000;
/** Bytes of stderr kept for the panel. */
const STDERR_TAIL = 8192;

/**
 * The full argument list of a chat process: headless stream-json in and out, the app-owned MCP config with
 * `--strict-mcp-config` (so a project's `.mcp.json` starts nothing), the permission flags and the allow-list, then the
 * session (`--resume <id>` or `--session-id <id>`).
 */
export function chatArgs(flag: '--resume' | '--session-id', session: string, allow: AllowList, mcpConfig: string): string[] {
  return [
    '-p',
    '--output-format',
    'stream-json',
    '--verbose',
    '--include-partial-messages',
    '--input-format',
    'stream-json',
    '--strict-mcp-config',
    '--mcp-config',
    mcpConfig,
    ...allow.permissionArgs(),
    flag,
    session,
  ];
}

const childEnv = (cfg: ClaudeConfig): NodeJS.ProcessEnv => ({ ...process.env, ...cfg.env });

/** Kills `child` and everything it started (tool shells, MCP servers), if it is still running. */
export function killTree(child: ChildProcess): void {
  if (child.exitCode !== null || child.signalCode !== null || child.pid === undefined) return;
  if (process.platform === 'win32') {
    const k = spawn('taskkill', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
    k.on('error', () => child.kill());
  } else {
    try {
      process.kill(-child.pid, 'SIGKILL'); // the group made by `detached`
    } catch {
      child.kill('SIGKILL');
    }
  }
}

export type Listing = { ok: true; agents: AgentEntry[] } | { ok: false; reason: string };

/** `claude agents --json`, run in `cwd`, killed after `timeoutMs`. Never throws. */
export function listAgents(cfg: ClaudeConfig, cwd: string, timeoutMs = LISTING_TIMEOUT_MS): Promise<Listing> {
  if (!cfg.command) return Promise.resolve({ ok: false, reason: 'claude is not configured (RCFD_STUDIO_CLAUDE)' });
  return new Promise((resolve) => {
    let out = '';
    let err = '';
    let done = false;
    const finish = (l: Listing) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      resolve(l);
    };
    let child: ChildProcess;
    try {
      child = spawn(cfg.command!, [...cfg.prefix, 'agents', '--json'], {
        cwd,
        env: childEnv(cfg),
        windowsHide: true,
        stdio: ['ignore', 'pipe', 'pipe'],
        detached: process.platform !== 'win32',
      });
    } catch (e) {
      resolve({ ok: false, reason: `${cfg.command}: ${(e as Error).message}` });
      return;
    }
    const timer = setTimeout(() => {
      killTree(child);
      finish({ ok: false, reason: `no answer within ${timeoutMs} ms` });
    }, timeoutMs);
    child.stdout!.setEncoding('utf8').on('data', (d: string) => (out += d));
    child.stderr!.setEncoding('utf8').on('data', (d: string) => (err = (err + d).slice(-2000)));
    child.on('error', (e) => finish({ ok: false, reason: `${cfg.command}: ${e.message}` }));
    child.on('close', (code) => {
      if (code !== 0) return finish({ ok: false, reason: `exit ${code}: ${err.trim()}` });
      try {
        finish({ ok: true, agents: parseAgents(out) });
      } catch (e) {
        finish({ ok: false, reason: (e as Error).message });
      }
    });
  });
}

/** What a turn reports, in order; `exit` is always the last. */
export type TurnEvent = StreamEvent | { readonly kind: 'aborted'; readonly reason: string } | { readonly kind: 'exit'; readonly code: number | null; readonly stderrTail: string };

/** A running turn: one `claude -p` process. */
export interface Turn {
  readonly pid: number | null;
  /** The arguments after the program (and after a script's path). */
  readonly argv: readonly string[];
  cancel(): void;
  readonly done: Promise<void>;
}

/**
 * Checks the argument list (checkArgsIn) and starts one turn. Throws ForbiddenArgument when the check fails, before any
 * process exists, and an Error when the program cannot be started.
 */
export async function startTurn(
  cfg: ClaudeConfig,
  project: string,
  argv: readonly string[],
  text: string,
  onEvent: (e: TurnEvent) => void,
): Promise<Turn> {
  if (!cfg.command) throw new Error('claude is not configured (RCFD_STUDIO_CLAUDE)');
  await checkArgsIn(argv, project);
  const child = spawn(cfg.command, [...cfg.prefix, ...argv], {
    cwd: project,
    env: childEnv(cfg),
    windowsHide: true,
    stdio: ['pipe', 'pipe', 'pipe'],
    detached: process.platform !== 'win32',
  });
  const started = new Promise<void>((resolve, reject) => {
    child.once('spawn', () => resolve());
    child.once('error', (e) => reject(new Error(`${cfg.command}: ${e.message}`)));
  });
  await started;
  child.on('error', () => undefined); // after the spawn, failures show as the exit
  // one message per process, then stdin is closed
  child.stdin!.on('error', () => undefined); // a child that died at once is reported through `exit`
  child.stdin!.end(JSON.stringify({ type: 'user', message: { role: 'user', content: text } }) + '\n');
  let stderr = '';
  child.stderr!.setEncoding('utf8').on('data', (d: string) => (stderr = (stderr + d).slice(-STDERR_TAIL)));
  let checked = false;
  let stopped = false;
  let buffer = '';
  const onLine = (line: string) => {
    if (stopped) return;
    for (const ev of parseLine(line)) {
      if (!checked) {
        const why = uncheckedProblem(ev);
        if (why) {
          if (ev.kind === 'init') onEvent(ev); // shows the mode the CLI applied
          stopped = true;
          killTree(child);
          onEvent({ kind: 'aborted', reason: why });
          return;
        }
        if (ev.kind === 'init') checked = true;
      }
      onEvent(ev);
    }
  };
  child.stdout!.setEncoding('utf8').on('data', (d: string) => {
    buffer += d;
    let nl: number;
    while ((nl = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, nl);
      buffer = buffer.slice(nl + 1);
      onLine(line);
    }
  });
  const done = new Promise<void>((resolve) => {
    child.on('close', (code) => {
      if (buffer) onLine(buffer);
      buffer = '';
      onEvent({ kind: 'exit', code, stderrTail: stderr });
      resolve();
    });
  });
  return { pid: child.pid ?? null, argv: [...argv], cancel: () => killTree(child), done };
}
