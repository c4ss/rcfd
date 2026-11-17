// Opens Claude Code in a terminal window in the project folder (ADR 0801 Addendum 23, "Open Claude Code here"), and runs
// `claude` for the registration (./register.ts). Nothing is embedded: the terminal is the system's.
//
// - Windows: Windows Terminal when it is installed (`wt.exe -d <folder> <claude>`; wt.exe is the app alias in
//   %LOCALAPPDATA%\Microsoft\WindowsApps), else a new console window running `cmd.exe /k <claude>` in the folder.
// - Linux: `x-terminal-emulator -e <claude>` in the folder (/usr/bin/x-terminal-emulator, the Debian alternative); else
//   the page is told that no terminal was found and what to run.
// - Elsewhere: the page is told what to run.
//
// Test mode (RCFD_TEST_HEADLESS=1) never opens a window: the launch is written to the file RCFD_STUDIO_LAUNCH_RECORD
// names (one JSON line of argv and cwd), or not done at all when that is unset. A claude program ending in .mjs (the
// tests' fake claude) is run by this Electron as Node (ELECTRON_RUN_AS_NODE=1).
import { execFile, spawn } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import path from 'node:path';
import type { ClaudeCodeOutcome } from '../../shared/project.js';
import type { RunResult } from './register.js';

/** How long one `claude mcp ...` may take (its `get` starts the server to check it). */
export const CLAUDE_RUN_TIMEOUT_MS = 60_000;

/** The program and arguments that run `exe args` (a .mjs through this Electron as Node). */
export function commandOf(exe: string, args: readonly string[], execPath: string): { file: string; args: string[]; node: boolean } {
  return exe.toLowerCase().endsWith('.mjs') ? { file: execPath, args: [exe, ...args], node: true } : { file: exe, args: [...args], node: false };
}

/** Runs claude with `args` in `cwd`; never rejects. */
export function runClaude(exe: string, args: readonly string[], cwd: string, execPath: string, env: NodeJS.ProcessEnv = process.env): Promise<RunResult> {
  const c = commandOf(exe, args, execPath);
  return new Promise((resolve) => {
    try {
      execFile(
        c.file,
        c.args,
        { cwd, timeout: CLAUDE_RUN_TIMEOUT_MS, windowsHide: true, maxBuffer: 1024 * 1024, env: c.node ? { ...env, ELECTRON_RUN_AS_NODE: '1' } : env },
        (error, stdout, stderr) => {
          const code = error ? (typeof (error as { code?: unknown }).code === 'number' ? ((error as { code: number }).code) : null) : 0;
          resolve({ code, stdout: String(stdout), stderr: String(stderr) || (error && code === null ? error.message : '') });
        },
      );
    } catch (e) {
      resolve({ code: null, stdout: '', stderr: String(e) });
    }
  });
}

/** The terminal launch for this platform: the program, its arguments and the working folder; or why there is none. */
export type LaunchPlan = { readonly argv: readonly string[]; readonly cwd: string; readonly what: string } | { readonly argv: null; readonly message: string };

export function launchPlan(
  claude: string,
  folder: string,
  env: Readonly<Record<string, string | undefined>>,
  isFile: (p: string) => boolean,
  platform: NodeJS.Platform = process.platform,
): LaunchPlan {
  if (platform === 'win32') {
    const apps = env['LOCALAPPDATA']?.trim();
    const wt = apps ? path.win32.join(apps, 'Microsoft', 'WindowsApps', 'wt.exe') : null;
    if (wt && isFile(wt)) return { argv: [wt, '-d', folder, claude], cwd: folder, what: 'Windows Terminal' };
    const system = env['SystemRoot']?.trim() || env['windir']?.trim() || 'C:\\Windows';
    return { argv: [path.win32.join(system, 'System32', 'cmd.exe'), '/k', claude], cwd: folder, what: 'a new command window' };
  }
  if (platform === 'linux') {
    const xt = '/usr/bin/x-terminal-emulator';
    if (isFile(xt)) return { argv: [xt, '-e', claude], cwd: folder, what: 'a terminal' };
    return { argv: null, message: `No terminal found (/usr/bin/x-terminal-emulator is not there). In a terminal, run: cd "${folder}" && "${claude}"` };
  }
  return { argv: null, message: `The Studio opens no terminal on this system. In a terminal, run: cd "${folder}" && "${claude}"` };
}

export interface LaunchOptions {
  readonly env: Readonly<Record<string, string | undefined>>;
  readonly isFile: (p: string) => boolean;
  /** Test mode: never open a window. */
  readonly headless: boolean;
  /** Test mode: the file each launch is recorded in. */
  readonly record: string | null;
  readonly platform?: NodeJS.Platform;
}

/** Opens the terminal (or, in test mode, records what would be opened). */
export function launchTerminal(claude: string, folder: string, o: LaunchOptions): Promise<ClaudeCodeOutcome> {
  const plan = launchPlan(claude, folder, o.env, o.isFile, o.platform);
  if (plan.argv === null) return Promise.resolve({ ok: false, message: plan.message });
  if (o.headless) {
    if (o.record) appendFileSync(o.record, `${JSON.stringify({ argv: plan.argv, cwd: plan.cwd })}\n`);
    return Promise.resolve({ ok: true, message: `Opened Claude Code in ${plan.what} in ${folder} (test mode: recorded, not opened).` });
  }
  return new Promise((resolve) => {
    try {
      const child = spawn(plan.argv[0]!, plan.argv.slice(1), { cwd: plan.cwd, detached: true, stdio: 'ignore', windowsHide: false });
      child.once('error', (e) => resolve({ ok: false, message: `${plan.what} could not be opened: ${e.message}` }));
      child.once('spawn', () => {
        child.unref();
        resolve({ ok: true, message: `Opened Claude Code in ${plan.what} in ${folder}.` });
      });
    } catch (e) {
      resolve({ ok: false, message: `${plan.what} could not be opened: ${String(e)}` });
    }
  });
}
