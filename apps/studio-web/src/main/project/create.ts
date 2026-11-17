// F-835: a new project, made by the engine (ADR 0731): `rcfd project init <folder>`, which calls rcfd_project's
// Project::create unchanged (the MCP equivalent is `create_project`). The Studio never writes rcfd-project.json itself
// (G4, G12): it only runs the command and reads its answer.
//
// The `rcfd` is the one beside the engine's project server (as the runner is, home.ts), never one from PATH.
import { existsSync } from 'node:fs';
import { spawn } from 'node:child_process';
import path from 'node:path';

export const RCFD_EXE = process.platform === 'win32' ? 'rcfd.exe' : 'rcfd';

/** The `rcfd` beside the engine's project server at `mcp`. */
export const rcfdBeside = (mcp: string): string => path.join(path.dirname(mcp), RCFD_EXE);

/** What `rcfd project init` answered: its one JSON line, or the engine's message on standard error. */
export type InitResult = { readonly ok: true; readonly result: Readonly<Record<string, unknown>> } | { readonly ok: false; readonly message: string };

/** The arguments the Studio runs `rcfd` with. */
export const initArgv = (folder: string): string[] => ['project', 'init', folder];

/** Runs `<rcfd> project init <folder>` and resolves with its answer (never rejects). */
export function initProjectCli(rcfd: string, folder: string, timeoutMs = 20_000): Promise<InitResult> {
  if (!existsSync(rcfd)) {
    return Promise.resolve({ ok: false, message: `there is no ${RCFD_EXE} beside the engine's project server (${rcfd}); build it with cargo build --release -p rcfd-cli` });
  }
  return new Promise((resolve) => {
    let out = '';
    let err = '';
    let done = false;
    const finish = (r: InitResult) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      resolve(r);
    };
    const child = spawn(rcfd, initArgv(folder), { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
    child.stdout!.on('data', (d: Buffer) => {
      out = (out + d.toString('utf8')).slice(-65_536);
    });
    child.stderr!.on('data', (d: Buffer) => {
      err = (err + d.toString('utf8')).slice(-4096);
    });
    child.once('error', (e) => finish({ ok: false, message: `${rcfd} could not be started (${e.message}); build it with cargo build --release -p rcfd-cli` }));
    child.once('close', (code) => {
      if (code !== 0) {
        finish({ ok: false, message: err.trim().split(/\r?\n/).slice(-4).join(' ') || `rcfd project init ended with exit code ${code}` });
        return;
      }
      try {
        const v = JSON.parse(out.trim()) as unknown;
        finish(
          v !== null && typeof v === 'object' && !Array.isArray(v)
            ? { ok: true, result: v as Record<string, unknown> }
            : { ok: false, message: 'rcfd project init printed no result object' },
        );
      } catch {
        finish({ ok: false, message: 'rcfd project init printed no JSON result' });
      }
    });
    const timer = setTimeout(() => {
      child.kill();
      finish({ ok: false, message: `rcfd project init did not end within ${timeoutMs / 1000} s` });
    }, timeoutMs);
  });
}
