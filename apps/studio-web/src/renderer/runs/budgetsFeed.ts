// F-891 (ADR 0788): what the runner-budgets panel (./RunExtras.tsx) does when the project's change feed has a new entry
// about .rcfd/runner-config.json, and when values count as typed. No DOM and no React, so the unit tests check it
// (tests/unit/f891.spec.ts).
//
// An entry is a reason to look, not a change: every write of the file through the engine journals a line, this window's
// own writes among them, and the entry reaches the panel up to one feed poll later (FEED_POLL_MS). The panel reads the
// file again and compares hashes, which is the engine's own test of "changed since it was read" (ADR 0733: base_hash,
// conflict 1200), so what the panel says agrees with what a Write would be answered.
import { BUDGET_KEYS, type BudgetKey, type RunnerBudgets } from '../../shared/run.js';

/** The panel's fields: one number per key, NaN for an empty field (the runner's default). */
export type Draft = Readonly<Record<BudgetKey, number>>;

/** The fields as a file read from the engine fills them. */
export const draftOf = (b: RunnerBudgets): Draft => Object.fromEntries(BUDGET_KEYS.map((k) => [k, b.values[k] ?? Number.NaN])) as Draft;

/**
 * Whether a field differs from the value read from the file. For a file the runner accepts this is "a Write would send
 * a key"; for one it would refuse (no values read) it is true only once a field holds a value.
 */
export function typedIn(draft: Draft | null, b: RunnerBudgets | null): boolean {
  if (!draft || !b) return false;
  return BUDGET_KEYS.some((k) => {
    const now = draft[k];
    const was = b.values[k];
    return Number.isNaN(now) ? was !== undefined : now !== was;
  });
}

/** `nothing`: the file is as the panel holds it; `take`: show the values read; `say`: another writer's change, typed values kept. */
export type FeedStep = 'nothing' | 'take' | 'say';

/** What to do with the file as read after a feed entry about it: `held` is what the panel holds (null: nothing yet). */
export function feedStep(held: RunnerBudgets | null, read: RunnerBudgets, typed: boolean): FeedStep {
  if (held === null) return 'take';
  if (held.hash === read.hash) return 'nothing';
  return typed ? 'say' : 'take';
}
