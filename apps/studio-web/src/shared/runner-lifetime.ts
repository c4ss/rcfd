// The runner's lifetime with the Studio (guardrail G7 as amended 2026-11-17; ADR 0801 Addendum "Runner closes with the
// Studio"). The maintainer: "the runner should close with studio by default if no external (existing) runner env var is
// specified". Pure, no Electron and no Node import: the main process (src/main/project/runner-lifetime.ts), the page's
// close prompt (src/renderer/case/closeGuard.tsx) and the unit tests share it.
//
// Ownership (the maintainer's answer to Q-RL-2, 2026-11-17). The runner records who started it in .rcfd/runner.json:
// `started_by` "studio", "cli" or "mcp", and `studio_pid` for the Studio (the Studio sets RCFD_RUNNER_STARTED_BY to
// studio:<its pid> on the rcfd-runner ensure and the rcfd-mcp it starts). The Studio owns a runner whose record says
// "studio" with this Studio's pid, from any look; a record with another `started_by`, or another Studio's pid, is never
// owned. Only a record with no `started_by` (an older runner, or one started by hand with rcfd-runner serve) falls back
// to the watch rule of before: the Studio owns the runner that came up while it had the project open, with no runner
// alive at its look before (it looks at the open, once a second, and around each of its own run-control actions, at the
// pid and start time the runner writes once it holds the project's lock). By that rule a runner alive when the project
// opened is adopted, never owned and never stopped, and one that replaced another between two looks is not owned.
//
// Closing (the window, Alt+F4, quit, Close project). For an owned runner, unless the setting "Keep the runner going after
// the Studio closes" or --keep-runner is on: no run queued, running or paused, it is shut down quietly (rcfd-runner
// shutdown); runs active, the close asks (stop them, keep them running, or cancel). A close that cannot ask (the test
// mode's quit, a page that does not answer) keeps a runner with active runs and shuts down an idle one.

/**
 * The variable that tells the engine who starts a runner (Q-RL-2; crates/rcfd-runner/src/started_by.rs): `rcfd-runner
 * serve` writes it to runner.json as `started_by` (and `studio_pid`).
 */
export const STARTED_BY_ENV = 'RCFD_RUNNER_STARTED_BY';

/** Its value for the Studio whose main process has pid `pid`. */
export const studioStartedBy = (pid: number): string => `studio:${pid}`;

/** What .rcfd/runner.json says of the runner holding the project: its process and when it started. */
export interface RunnerSeen {
  readonly pid: number;
  readonly started: string;
  /** Who started it (`started_by`: "studio", "cli", "mcp"); absent in the record of an older runner. */
  readonly startedBy?: string;
  /** The Studio's pid, when `startedBy` is "studio" (`studio_pid`). */
  readonly studioPid?: number;
}

export const sameRunner = (a: RunnerSeen | null, b: RunnerSeen | null): boolean =>
  a !== null && b !== null && a.pid === b.pid && a.started === b.started;

/** What the Studio knows of a project's runner since it opened the project. */
export interface Ownership {
  /** Whether the Studio has looked at least once (the look at the open). */
  readonly looked: boolean;
  /** The runner alive at the last look, or null. */
  readonly last: RunnerSeen | null;
  /** The runner the Studio owns, or null. */
  readonly owned: RunnerSeen | null;
}

export const NOT_LOOKED: Ownership = { looked: false, last: null, owned: null };

/** Whether the runner's record says this Studio (pid `self`) started it; null when the record does not say who did. */
export function startedByThisStudio(seen: RunnerSeen, self: number): boolean | null {
  if (seen.startedBy === undefined) return null;
  return seen.startedBy === 'studio' && seen.studioPid === self;
}

/**
 * One look at the project's runner by the Studio whose pid is `self`. A runner whose record says who started it is owned
 * exactly when it says this Studio, at any look. A record that does not say falls back to the looks: the first look (at
 * the open) owns nothing, so a runner alive then is adopted; after it, a runner seen for the first time is owned when no
 * runner was alive at the look before, and one that replaced another between two looks is not (the Studio never stops a
 * runner it cannot tell it started). An owned runner that is gone is not owned any more.
 */
export function observeRunner(o: Ownership, seen: RunnerSeen | null, self: number): Ownership {
  const mine = seen === null ? null : startedByThisStudio(seen, self);
  if (mine !== null) return { looked: true, last: seen, owned: mine ? seen : null };
  if (!o.looked) return { looked: true, last: seen, owned: null };
  const owned = o.owned !== null && sameRunner(o.owned, seen) ? o.owned : null;
  if (seen === null) return { looked: true, last: null, owned: null };
  if (sameRunner(o.last, seen)) return { looked: true, last: seen, owned };
  return { looked: true, last: seen, owned: o.last === null ? seen : null };
}

/** The runner's states of a run that is not over. */
export const ACTIVE_RUN_STATES = ['queued', 'running', 'paused'] as const;
export const isActiveRunState = (s: string): boolean => (ACTIVE_RUN_STATES as readonly string[]).includes(s);

/** A run queued, running or paused, as the runner's records say at the close. */
export interface ActiveRun {
  readonly id: string;
  /** The case file, project-relative (run.json), or null when it cannot be read. */
  readonly case: string | null;
  readonly state: string;
  readonly step: number;
  /** The simulated time of the last step; null or 0 for a steady run. */
  readonly time: number | null;
}

/** Why the runner is kept whatever its runs: the Studio setting or the --keep-runner switch. */
export type KeepRunner = 'setting' | 'switch' | null;

/** What a close of the open project means for its runner (the project port's `runnerPlan`). */
export interface RunnerPlan {
  /** The Studio owns the project's runner, and it is alive. */
  readonly owned: boolean;
  /** The override that keeps the runner going, if any. */
  readonly kept: KeepRunner;
  /** The active runs of an owned runner (empty when not owned or kept). */
  readonly runs: readonly ActiveRun[];
}

export const NO_PLAN: RunnerPlan = { owned: false, kept: null, runs: [] };

/** The user's answer for the runs: stop them, keep them running, or not asked (no prompt was possible or needed). */
export const RUNNER_CHOICES = ['stop', 'keep', 'unasked'] as const;
export type RunnerChoice = (typeof RUNNER_CHOICES)[number];

/** What the close does to the runner. */
export type RunnerStep = 'leave' | 'shutdown' | 'stop-then-shutdown';

/** Whether a close must ask about the runner's runs before it goes on. */
export const runnerNeedsAsk = (plan: RunnerPlan): boolean => plan.owned && plan.kept === null && plan.runs.length > 0;

/**
 * The close's step for the runner. Not owned, or kept by the setting or the switch: left alone. Owned with no active
 * run: shut down. Owned with active runs: stopped and shut down on `stop`, left on `keep`, and left when the close did
 * not ask (nothing a user did not choose is stopped).
 */
export function runnerCloseStep(plan: RunnerPlan, choice: RunnerChoice): RunnerStep {
  if (!plan.owned || plan.kept !== null) return 'leave';
  if (plan.runs.length === 0) return 'shutdown';
  return choice === 'stop' ? 'stop-then-shutdown' : 'leave';
}

const fmt = (x: number): string => (Math.abs(x) >= 1e4 || (Math.abs(x) < 1e-3 && x !== 0) ? x.toExponential(3) : String(Number(x.toPrecision(4))));

/** One run in the close prompt: id, case, state, and its step or simulated time ("r-…: cases/a.toml, running, step 120"). */
export function runLine(r: ActiveRun): string {
  const where = r.state === 'queued' ? 'not started' : r.time !== null && r.time > 0 ? `step ${r.step}, t = ${fmt(r.time)} s` : `step ${r.step}`;
  return `${r.id}: ${r.case ?? 'case not known'}, ${r.state}, ${where}`;
}

const plural = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`;

/** The prompt's title: what closes, with the changes not saved and the runs going. */
export function closeTitle(what: string, changes: number, runs: number): string {
  const parts = [changes > 0 ? `${plural(changes, 'change', 'changes')} not saved` : null, runs > 0 ? `${plural(runs, 'run', 'runs')} going` : null].filter(
    (p): p is string => p !== null,
  );
  return `Close ${what} with ${parts.join(' and ')}?`;
}
