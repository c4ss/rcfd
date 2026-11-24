// The runs e2e support (F-806): the project's runner (`rcfd-runner`, ADR 0123), started and driven from its CLI the way
// a user or Claude Code would, outside the Studio (the Studio cannot start runs until F-807). The runner owns the runs
// (G7): the tests close and kill the Studio and read the run's true state from the runner's own records.
//
// F-893 (ADR 0790): each Runner notes its project folder in the run's registry (leftovers.ts, rule 3), so that the global
// teardown ends a solver under it that outlives the test; endAll() ends every run of the project that is not over.
import { test } from '@playwright/test';
import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { noteProject } from './leftovers.js';
import { findMcp } from './project.js';
import { alive, killIfRcfd, killPid } from './rcfd.js';

const TERMINAL = ['finished', 'failed', 'cancelled'];
const pause = (ms: number) => new Promise((d) => setTimeout(d, ms));

/** The spec file of the test or hook that is running; null outside one. */
function currentSpec(): string | null {
  try {
    return path.basename(test.info().file);
  } catch {
    return null;
  }
}

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
  ) {
    noteProject(project, currentSpec());
  }

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

  /** The ids of the project's runs (the folders of runs/ that have a status.json). */
  runIds(): string[] {
    try {
      return readdirSync(path.join(this.project, 'runs')).filter((id) => existsSync(path.join(this.project, 'runs', id, 'status.json')));
    } catch {
      return [];
    }
  }

  /** status.json of a run, or null while it cannot be read (the runner replaces it as it writes). */
  private statusOrNull(id: string): Status | null {
    try {
      return this.status(id);
    } catch {
      return null;
    }
  }

  /**
   * F-893 (ADR 0790): ends every run of the project that is not over, whether a test noted its id or failed before it
   * could, then the runner. Each such run is cancelled (`rcfd-runner cancel`; where the command fails, the solver's stop
   * file is written directly) and waited for until it is over or its solver is gone, at most `limitMs` for all; a solver
   * still alive then is ended by its pid if it is still an rcfd process. Then the runner is shut down, and killed if it is
   * still alive. Returns the runs whose solver had to be ended by its pid.
   */
  async endAll(limitMs = 30_000): Promise<string[]> {
    const open: string[] = [];
    for (const id of this.runIds()) {
      // a status that cannot be read now is taken as not over
      const st = this.statusOrNull(id);
      if (!st || !TERMINAL.includes(st.state)) open.push(id);
    }
    for (const id of open) {
      try {
        this.cli('cancel', id);
      } catch {
        try {
          writeFileSync(path.join(this.project, 'runs', id, 'cancel'), '');
        } catch {
          // the folder is gone
        }
      }
    }
    const over = (id: string): boolean => {
      const st = this.statusOrNull(id);
      if (!st) return !existsSync(path.join(this.project, 'runs', id));
      return TERMINAL.includes(st.state) || st.pid === null || !alive(st.pid);
    };
    const deadline = Date.now() + limitMs;
    while (open.some((id) => !over(id)) && Date.now() < deadline) await pause(100);
    const killed: string[] = [];
    for (const id of open) {
      let st = this.statusOrNull(id);
      for (let i = 0; st === null && i < 10; i++) {
        await pause(50);
        st = this.statusOrNull(id);
      }
      if (st && !TERMINAL.includes(st.state) && st.pid !== null && alive(st.pid)) {
        killIfRcfd(st.pid);
        killed.push(id);
      }
    }
    const runnerPid = this.pid();
    try {
      this.cli('shutdown');
    } catch {
      // not running
    }
    if (runnerPid !== null && alive(runnerPid)) killPid(runnerPid);
    return killed;
  }
}
