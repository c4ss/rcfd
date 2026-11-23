// F-891, ADR 0788 (check U1): what the runner-budgets panel does with a change-feed entry about its file, without a
// window. An entry is a reason to look at the file, not a change: the journal line of this window's own write gives one
// too, up to a feed poll later. The panel acts only when the file read has another hash than the file it holds, and it
// calls values typed only when a field differs from the value read (src/renderer/runs/budgetsFeed.ts).
import { expect, test } from '@playwright/test';
import { draftOf, feedStep, typedIn } from '../../src/renderer/runs/budgetsFeed.js';
import type { BudgetKey, RunnerBudgets } from '../../src/shared/run.js';

const file = (hash: string | null, values: Partial<Record<BudgetKey, number>> = {}, refused = false): RunnerBudgets => ({
  path: '.rcfd/runner-config.json',
  exists: hash !== null,
  values,
  hash,
  problem: refused ? 'unknown field `gpu`' : null,
  needsReplace: refused,
  inUse: null,
  undo: false,
});

test.describe('a feed entry about the runner’s budgets file', () => {
  test('the file as the panel holds it: nothing, whether values are typed or not', () => {
    const held = file('fnv64:01', { max_runs: 2 });
    expect(feedStep(held, file('fnv64:01', { max_runs: 2 }), false)).toBe('nothing');
    expect(feedStep(held, file('fnv64:01', { max_runs: 2 }), true)).toBe('nothing');
    // no file held and none read
    expect(feedStep(file(null), file(null), true)).toBe('nothing');
    // a file the runner would refuse, read already
    expect(feedStep(file('fnv64:09', {}, true), file('fnv64:09', {}, true), false)).toBe('nothing');
  });

  test('another file: its values with nothing typed, the note with values typed; nothing held yet: its values', () => {
    const held = file('fnv64:01', { max_runs: 2 });
    expect(feedStep(held, file('fnv64:02', { max_runs: 3 }), false)).toBe('take');
    expect(feedStep(held, file('fnv64:02', { max_runs: 3 }), true)).toBe('say');
    expect(feedStep(file(null), file('fnv64:02', { max_runs: 3 }), false)).toBe('take');
    expect(feedStep(held, file(null), true)).toBe('say');
    expect(feedStep(null, file('fnv64:02', { max_runs: 3 }), false)).toBe('take');
  });

  test('typed: a field that differs from the value read; never a refused file with its fields as read', () => {
    const b = file('fnv64:01', { max_runs: 2 });
    expect(typedIn(null, b)).toBe(false);
    expect(typedIn(draftOf(b), null)).toBe(false);
    expect(typedIn(draftOf(b), b)).toBe(false);
    expect(typedIn({ ...draftOf(b), cpu_threads: 2 }, b)).toBe(true);
    expect(typedIn({ ...draftOf(b), max_runs: 3 }, b)).toBe(true);
    // a field emptied: the key would be removed
    expect(typedIn({ ...draftOf(b), max_runs: Number.NaN }, b)).toBe(true);
    // the file the runner would refuse: no values are read, and none is typed until a field holds one
    const refused = file('fnv64:09', {}, true);
    expect(typedIn(draftOf(refused), refused)).toBe(false);
    expect(typedIn({ ...draftOf(refused), max_runs: 2 }, refused)).toBe(true);
  });
});
