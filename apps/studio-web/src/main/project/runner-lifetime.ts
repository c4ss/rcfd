// The runner's lifetime with the Studio in the main process (src/shared/runner-lifetime.ts has the rules): the looks at
// the project's runner, its active runs at a close, and the close's step, through the engine's own CLI (rcfd-runner
// list, cancel, wait and shutdown; the run tools' cancel_run when the run-control server is there). No Electron import:
// the unit tests drive it with fakes.
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import {
  isActiveRunState,
  NOT_LOOKED,
  observeRunner,
  runnerCloseStep,
  type ActiveRun,
  type KeepRunner,
  type Ownership,
  type RunnerChoice,
  type RunnerPlan,
  type RunnerSeen,
  type RunnerStep,
} from '../../shared/runner-lifetime.js';

/** How often the open project's runner is looked at. */
export const RUNNER_LOOK_MS = 1000;
/** The longest the close waits for cancelled runs to end (a solver has cancel_grace_ms, 10 s by default, to stop). */
export const STOP_WAIT_S = 30;

const processAlive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === 'EPERM';
  }
};

/**
 * The runner holding `folder`, from .rcfd/runner.json (written once it holds the lock, removed at a clean shutdown), if
 * its process is alive; null when there is none (no file, or its process is gone). Undefined when the file is there but
 * cannot be read now (a write or a virus scan holding it): the look says nothing, and the ownership stays as it was.
 */
export async function readRunnerSeen(folder: string, alive: (pid: number) => boolean = processAlive): Promise<RunnerSeen | null | undefined> {
  let text: string;
  try {
    text = await readFile(path.join(folder, '.rcfd', 'runner.json'), 'utf8');
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === 'ENOENT' ? null : undefined;
  }
  try {
    const j = JSON.parse(text) as { pid?: unknown; started?: unknown };
    if (!Number.isSafeInteger(j.pid) || (j.pid as number) <= 0 || typeof j.started !== 'string') return undefined;
    return alive(j.pid as number) ? { pid: j.pid as number, started: j.started } : null;
  } catch {
    return undefined;
  }
}

/** One rcfd-runner command for `folder`; resolves with its exit code and standard output. Nothing is inherited. */
export function runnerCommand(exe: string, folder: string, args: readonly string[], timeoutMs: number): Promise<{ code: number | null; out: string }> {
  return new Promise((resolve) => {
    let out = '';
    let done = false;
    const child = spawn(exe, [...args, '--project', folder], { cwd: folder, stdio: ['ignore', 'pipe', 'ignore'], windowsHide: true });
    const finish = (code: number | null) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      resolve({ code, out });
    };
    child.stdout!.on('data', (d: Buffer) => {
      out += d.toString('utf8');
    });
    child.once('error', () => finish(null));
    child.once('close', (code) => finish(code));
    const timer = setTimeout(() => {
      child.kill();
      finish(null);
    }, timeoutMs);
  });
}

/** The engine's side of a close, replaceable in the unit tests. */
export interface RunnerEngine {
  /** The runner seen now; null for none, undefined when it cannot be told (readRunnerSeen). */
  look(folder: string): Promise<RunnerSeen | null | undefined>;
  /** The runs not over (`rcfd-runner list`, with each run's case from its run.json). */
  activeRuns(folder: string): Promise<ActiveRun[]>;
  /** Cancels one run (the run tools' cancel_run, else `rcfd-runner cancel`). */
  cancel(folder: string, run: string): Promise<void>;
  /** Waits until the run has ended, at most `seconds` (`rcfd-runner wait`). */
  wait(folder: string, run: string, seconds: number): Promise<void>;
  /** `rcfd-runner shutdown`: the runner exits cleanly. */
  shutdown(folder: string): Promise<void>;
}

/** The engine through rcfd-runner at `exe`; `cancelRun` is the run tools' cancel_run of the open project, when there. */
export function cliRunnerEngine(exe: string, cancelRun?: (run: string) => Promise<boolean>): RunnerEngine {
  return {
    look: (folder) => readRunnerSeen(folder),
    async activeRuns(folder) {
      const r = await runnerCommand(exe, folder, ['list'], 15_000);
      const out: ActiveRun[] = [];
      for (const line of r.out.split(/\r?\n/)) {
        if (!line.trim()) continue;
        try {
          const j = JSON.parse(line) as { id?: unknown; status?: { state?: unknown; step?: unknown; time?: unknown } };
          const state = typeof j.status?.state === 'string' ? j.status.state : '';
          if (typeof j.id !== 'string' || !isActiveRunState(state)) continue;
          let kase: string | null = null;
          try {
            const run = JSON.parse(await readFile(path.join(folder, 'runs', j.id, 'run.json'), 'utf8')) as { case?: unknown };
            kase = typeof run.case === 'string' ? run.case : null;
          } catch {
            // no record yet: the prompt says the case is not known
          }
          out.push({
            id: j.id,
            case: kase,
            state,
            step: typeof j.status?.step === 'number' ? j.status.step : 0,
            time: typeof j.status?.time === 'number' ? j.status.time : null,
          });
        } catch {
          // not a line of the list
        }
      }
      return out;
    },
    async cancel(folder, run) {
      if (cancelRun && (await cancelRun(run).catch(() => false))) return;
      await runnerCommand(exe, folder, ['cancel', run], 15_000);
    },
    async wait(folder, run, seconds) {
      await runnerCommand(exe, folder, ['wait', run, '--timeout', String(seconds)], (seconds + 5) * 1000);
    },
    async shutdown(folder) {
      await runnerCommand(exe, folder, ['shutdown'], 35_000);
    },
  };
}

/** The open project's runner: the looks that tell whether the Studio owns it, and what a close does to it. */
export class RunnerWatch {
  private ownership: Ownership = NOT_LOOKED;
  private timer: ReturnType<typeof setInterval> | null = null;
  private looking: Promise<void> = Promise.resolve();
  private closed = false;

  constructor(
    readonly folder: string,
    private readonly engine: RunnerEngine,
    private readonly keep: () => KeepRunner,
    lookMs = RUNNER_LOOK_MS,
  ) {
    void this.look();
    if (lookMs > 0) {
      this.timer = setInterval(() => void this.look(), lookMs);
      this.timer.unref?.();
    }
  }

  /** One look at the runner, after the ones already going (they are taken in order). */
  look(): Promise<void> {
    this.looking = this.looking.then(async () => {
      if (this.closed) return;
      const seen = await this.engine.look(this.folder).catch(() => undefined);
      // a look that cannot tell changes nothing: a runner would otherwise seem to go and come back, and be owned
      if (seen !== undefined) this.ownership = observeRunner(this.ownership, seen);
    });
    return this.looking;
  }

  /** The owned runner (for tests and diagnostics). */
  get owned(): RunnerSeen | null {
    return this.ownership.owned;
  }

  /** What a close of the project means for its runner now: a fresh look, and the active runs of an owned runner. */
  async plan(): Promise<RunnerPlan> {
    await this.look();
    const kept = this.keep();
    const owned = this.ownership.owned !== null;
    if (!owned || kept !== null) return { owned, kept, runs: [] };
    return { owned, kept, runs: await this.engine.activeRuns(this.folder).catch(() => []) };
  }

  /** Ends the watch and does the close's step for the runner; resolves with it. */
  async close(choice: RunnerChoice): Promise<RunnerStep> {
    const plan = await this.plan();
    this.stop();
    const step = runnerCloseStep(plan, choice);
    if (step === 'stop-then-shutdown') {
      await Promise.all(plan.runs.map((r) => this.engine.cancel(this.folder, r.id).catch(() => undefined)));
      await Promise.all(plan.runs.map((r) => this.engine.wait(this.folder, r.id, STOP_WAIT_S).catch(() => undefined)));
    }
    if (step !== 'leave') await this.engine.shutdown(this.folder).catch(() => undefined);
    return step;
  }

  /** Stops looking (the project closed without a close step: a later project replaced it, or the engine is gone). */
  stop(): void {
    this.closed = true;
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }
}
