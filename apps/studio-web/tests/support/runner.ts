// The runs e2e support (F-806): the project's runner (`rcfd-runner`, ADR 0123), started and driven from its CLI the way
// a user or Claude Code would, outside the Studio (the Studio cannot start runs until F-807). The runner owns the runs
// (G7): the tests close and kill the Studio and read the run's true state from the runner's own records.
import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { findMcp } from './project.js';
import { alive, killPid } from './rcfd.js';

const exe = process.platform === 'win32' ? 'rcfd-runner.exe' : 'rcfd-runner';

/** rcfd-runner: RCFD_RUNNER_BIN, else next to rcfd-mcp (cargo build --release puts them together). */
export function findRunner(): string | null {
  if (process.env['RCFD_RUNNER_BIN'] && existsSync(process.env['RCFD_RUNNER_BIN'])) return process.env['RCFD_RUNNER_BIN'];
  const mcp = findMcp();
  const next = mcp ? path.join(path.dirname(mcp), exe) : null;
  return next && existsSync(next) ? next : null;
}

export const NO_RUNNER = 'rcfd-runner was not found next to rcfd-mcp: build it with `cargo build --release -p rcfd-runner`, or set RCFD_RUNNER_BIN';

export interface Status {
  state: string;
  step: number;
  time: number | null;
  pid: number | null;
  updated: string;
  reason?: string;
}

export class Runner {
  constructor(
    readonly exe: string,
    readonly project: string,
  ) {}

  /** A runner command that does not start the service (its output is read to the end). */
  cli(...args: string[]): string {
    return execFileSync(this.exe, [...args, '--project', this.project], { cwd: this.project, encoding: 'utf8', windowsHide: true }).trim();
  }

  /**
   * Starts the project's runner, detached, unless one runs. No pipe is handed to it: on Windows the service inherits
   * every inheritable handle of the command that starts it (ADR 0146), so a pipe would stay open as long as it runs.
   */
  ensure(): void {
    const r = spawnSync(this.exe, ['ensure', '--project', this.project, '--quiet'], { cwd: this.project, stdio: 'ignore', windowsHide: true });
    if (r.status !== 0) throw new Error(`rcfd-runner ensure ended with ${r.status}`);
  }

  /** Queues a run of `caseFile` with `set` overrides (the runner, already started, launches it); returns its id. */
  submit(caseFile: string, set: readonly string[] = []): string {
    return this.cli('start', caseFile, '--no-runner', ...set.flatMap((s) => ['--set', s]));
  }

  /** status.json of a run, as the runner wrote it. */
  status(id: string): Status {
    return JSON.parse(readFileSync(path.join(this.project, 'runs', id, 'status.json'), 'utf8')) as Status;
  }

  /** The runner's process id while it holds the project (`.rcfd/runner.json`), or null. */
  pid(): number | null {
    try {
      const j = JSON.parse(readFileSync(path.join(this.project, '.rcfd', 'runner.json'), 'utf8')) as { pid?: unknown };
      return typeof j.pid === 'number' ? j.pid : null;
    } catch {
      return null;
    }
  }

  /**
   * Ends what the test started: cancels the run if it is not over, waits for it, and stops the runner cleanly; whatever is
   * still alive after that (the solver, the runner) is killed. Only processes this test started are touched.
   */
  async cleanUp(id: string | null): Promise<void> {
    if (id) {
      try {
        const st = this.status(id);
        if (!['finished', 'failed', 'cancelled'].includes(st.state)) this.cli('cancel', id);
      } catch {
        // already over, or never written
      }
      for (let i = 0; i < 60; i++) {
        try {
          if (['finished', 'failed', 'cancelled'].includes(this.status(id).state)) break;
        } catch {
          break;
        }
        await new Promise((d) => setTimeout(d, 500));
      }
    }
    const runnerPid = this.pid();
    try {
      this.cli('shutdown');
    } catch {
      // not running
    }
    if (id) {
      try {
        const pid = this.status(id).pid;
        if (pid !== null && alive(pid)) killPid(pid);
      } catch {
        // gone
      }
    }
    if (runnerPid !== null && alive(runnerPid)) killPid(runnerPid);
  }
}
