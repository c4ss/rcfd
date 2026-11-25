// F-828, ADR 0795 (row U): the provenance of the numbers that had none, without a window: the facts of a project number
// (a count, a budget) and of the new run numbers (a residual of the runs table, a legend's tick, the domain, the scale
// bar); the order in which Ctrl+I steps through the numbers of a focused element (src/renderer/provenance/registry.ts);
// the static rules: no tab stop in the passive form, no new port operation, no unit made or converted by the new code.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { numberFacts, PROJECT_NO_REPRODUCE, projectFacts, PROVENANCE_OPS, type Fact, type RunProvenance } from '../../src/shared/provenance.js';
import { budgetInUse, budgetValue, domainValue, homeCount, legendTick, residualValue, scaleBarValue, type LegendRange } from '../../src/renderer/provenance/sources.js';
import { numbersOf, type TriggerEntry } from '../../src/renderer/provenance/registry.js';
import { lengthUnitOf } from '../../src/renderer/field/viewExtras.js';
import { tickLabel, ticks } from '../../src/renderer/field/colormap.js';
import { appRoot } from '../support/rcfd.js';

const renderer = path.join(appRoot, 'src', 'renderer');
const code = (f: string) =>
  readFileSync(path.join(renderer, f), 'utf8')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');
const of = (rows: readonly Fact[]): Record<string, string> => Object.fromEntries(rows);
const RUN = 'r-20261028T152641-f8c2';
const STORE = `runs/${RUN}/fields/store.rts`;
const ENGINE = { name: 'rcfd-mcp', version: '0.1.0' };

/** A run's provenance as the port gives it, as far as a number's facts read it. */
const run = (units: Record<string, string>): RunProvenance =>
  ({ run: RUN, record: { solverVersion: 'rcfd 0.1.0' }, recordError: null, units, unitsNote: null, timeUnit: 's' }) as unknown as RunProvenance;

test('a count of the home: what was counted, by whom, the engine’s own count, and no run, snapshot or command', () => {
  const count = homeCount({
    folder: '/p/quick',
    engine: ENGINE,
    quantity: 'cases',
    value: 4,
    file: 'cases/',
    readBy: 'the Studio',
    counted: 'by the Studio: every file',
    engineCount: { field: 'counts.cases', value: 5 },
    readAt: '2026-11-25T10:00:00Z',
  });
  expect(of(projectFacts(count))).toEqual({
    Value: '4',
    Unit: 'none: a count',
    Run: 'none: a number of the project, not of a run',
    Snapshot: 'none: a number of the project, not of a snapshot',
    Project: '/p/quick',
    File: 'cases/',
    'Read by': 'the Studio',
    Counted: 'by the Studio: every file',
    'Engine’s count': '5, counts.cases of project_info',
    'Read at': '2026-11-25T10:00:00Z',
    Engine: 'rcfd-mcp 0.1.0',
  });
  expect(PROJECT_NO_REPRODUCE).toMatch(/^Not recorded: /);
  const none = of(projectFacts({ ...homeCount({ folder: '/p', engine: null, quantity: 'x', value: 1, file: 'f', readBy: 'r', counted: 'c', readAt: 't' }), readAt: null }));
  expect(none['Engine']).toMatch(/^not recorded: /);
  expect('Read at' in none).toBe(false);
  expect('Engine’s count' in none).toBe(false);
});

test('a budget: the file’s value, a typed value not written, an empty key; the runner’s own record of the number in use', () => {
  const o = { folder: '/p', engine: ENGINE, key: 'mem_budget_mb', path: '.rcfd/runner-config.json', hash: 'fnv64:01', exists: true };
  const held = of(projectFacts(budgetValue({ ...o, inFile: 100, typed: 100 })));
  // the key ends in _mb; the unit is not read from it
  expect(held['Unit']).toBe('not recorded: runner_config names no unit of its keys');
  expect([held['Value'], held['File'], held['Hash'], held['Typed']]).toEqual(['100', '.rcfd/runner-config.json', 'fnv64:01', undefined]);
  const typed = of(projectFacts(budgetValue({ ...o, inFile: 100, typed: 200 })));
  expect([typed['Value'], typed['Typed']]).toEqual(['200', 'not written: the file holds 100']);
  const cleared = of(projectFacts(budgetValue({ ...o, inFile: 100, typed: Number.NaN })));
  expect([cleared['Value'], cleared['Typed']]).toEqual(['not set', 'not written: the file holds 100']);
  expect(cleared['Default']).toMatch(/^not recorded in the file: /);
  const empty = of(projectFacts(budgetValue({ ...o, exists: false, hash: null, inFile: undefined, typed: Number.NaN })));
  expect([empty['Value'], empty['Typed'], empty['Hash']]).toEqual(['not set', undefined, 'none: there is no file']);
  const inUse = of(projectFacts(budgetInUse({ folder: '/p', engine: ENGINE, key: 'max_runs', value: 2, source: 'file', started: '2026-11-25T10:00:00Z' })));
  expect([inUse['Value'], inUse['File'], inUse['Config source'], inUse['Runner started']]).toEqual(['2', '.rcfd/runner.json', 'file', '2026-11-25T10:00:00Z']);
});

test('a residual of the runs table: run_status’s value of that run, the unit of that run’s series', () => {
  const src = residualValue({ run: RUN, equation: 'u', shown: '7.40e-7', value: 7.395e-7 });
  expect(of(numberFacts(src, run({ res_u: '', res_p: 'Pa' })))).toEqual({
    Value: '7.40e-7 (7.395e-7)',
    Unit: `none, empty in runs/${RUN}/output/monitors.json`,
    Run: RUN,
    Snapshot: 'none: the last line of residuals.jsonl, not a snapshot',
    File: `runs/${RUN}/residuals.jsonl`,
    'Read by': 'run_status (rcfd-mcp, read-only), last_residual.residuals.u, the file’s last line',
    Rounded: 'by the Studio, to 3 significant figures',
    Engine: 'rcfd 0.1.0',
  });
  const p = residualValue({ run: RUN, equation: 'p', shown: 'x', value: 1 });
  expect(of(numberFacts(p, run({ res_u: 'm', res_p: 'Pa' })))['Unit']).toBe(`Pa, runs/${RUN}/output/monitors.json`);
  expect(of(numberFacts(src, run({})))['Unit']).toMatch(/^none: .* has no series res_u$/);
});

test('a legend’s tick: an even division of the drawn range, and where the range comes from', () => {
  const [lo, hi] = [-0.5, 1];
  const all = ticks(lo, hi);
  expect(all).toHaveLength(5);
  const tick = (i: number, range: LegendRange) => {
    const src = legendTick({ run: RUN, quantity: 'ux', shown: tickLabel(all[i]!), value: all[i]!, index: i, ticks: 5, lo, hi, unit: 'm/s', range, snapshot: 3, of: 24, step: 20, time: 0 });
    return of(numberFacts(src, run({})));
  };
  const one = tick(1, { kind: 'snapshot' });
  expect(one['Value']).toBe(`${tickLabel(-0.125)} (-0.125)`);
  expect(one['Unit']).toBe(`m/s, ${STORE} (the field’s unit)`);
  expect([one['Run'], one['File'], one['Snapshot'], one['Tick']]).toEqual([RUN, STORE, '4 of 24, step 20', '2 of 5: an even division of the range']);
  expect(one['Range']).toBe('-0.5 to 1, the least and the greatest of the values shown of this snapshot, found by the Studio');
  expect(tick(0, { kind: 'snapshot' })['Tick']).toBe('1 of 5: the bottom of the range');
  expect(tick(4, { kind: 'snapshot' })['Tick']).toBe('5 of 5: the top of the range');
  const whole = tick(0, { kind: 'run', snapshots: 24 });
  expect([whole['Snapshot'], whole['Range']]).toEqual([
    'none: the range is over 24 snapshots (shown: 4 of 24)',
    '-0.5 to 1, the least and the greatest of the shown values over 24 snapshots, found by the Studio',
  ]);
  const fixed = tick(0, { kind: 'fixed' });
  expect([fixed['Snapshot'], fixed['Range']]).toEqual(['none: a typed range (shown: 4 of 24)', '-0.5 to 1, typed in this window, not recorded by the engine']);
  expect(tick(0, { kind: 'shared', other: 'r-b', snapshot: 7, of: 9 })['Compared run']).toBe('r-b, snapshot 8 of 9, runs/r-b/fields/store.rts');
  expect(tick(0, { kind: 'difference', baseline: 'r-a', snapshot: 1, of: 9 })['Baseline run']).toBe('r-a, snapshot 2 of 9, runs/r-a/fields/store.rts');
});

test('the domain and the scale bar: the unit of the stored mesh as the engine names it, on each route', () => {
  const unitOf = (opened: string | null, derived: string | null) => {
    const unit = lengthUnitOf(opened, derived);
    const d = of(numberFacts(domainValue({ run: RUN, shown: '1 × 0.3333', unit, extents: [1, 1 / 3], axes: ['x', 'y'], twoD: true }), run({})));
    const s = of(numberFacts(scaleBarValue({ run: RUN, shown: '0.5', value: 0.5, unit }), run({})));
    expect(s['Unit']).toBe(d['Unit']);
    expect(d['Value']).toBe(`1 × 0.3333 (1 × ${1 / 3})`);
    expect(d['Counted']).toContain('along x, y (2D');
    expect(s['File']).toMatch(/^not recorded: /);
    expect(s['Read by']).toMatch(/^not recorded: /);
    return d['Unit'];
  };
  expect(unitOf('mm', null)).toBe('mm, length_unit of data.open');
  expect(unitOf('mm', 'm')).toBe('mm, length_unit of data.open');
  expect(unitOf(null, 'm')).toBe('m, length_unit of the engine’s derived picture of this store');
  expect(unitOf(null, null)).toBe('m, the engine’s documented unit of a stored mesh (ADR 0738); not recorded in store.rts');
});

// ---- the registry's order, on stand-in elements

interface El {
  readonly name: string;
  readonly parent: El | null;
  contains(o: El): boolean;
  closest(): El | null;
  compareDocumentPosition(o: El): number;
  readonly dataset: Record<string, string | undefined>;
}
function el(name: string, parent: El | null, keys?: string): El {
  const e: El = {
    name,
    parent,
    dataset: keys === undefined ? {} : { provenance: keys },
    contains: (o) => {
      for (let a: El | null = o; a; a = a.parent) if (a === e) return true;
      return false;
    },
    closest: () => {
      for (let a: El | null = e; a; a = a.parent) if (a.dataset['provenance'] !== undefined) return a;
      return null;
    },
    // in this test a name is its place in the document
    compareDocumentPosition: (o) => (o.name > e.name ? 4 : 2),
  };
  return e;
}
const entry = (e: El, key: string | null, member: boolean): TriggerEntry => ({ el: () => e as unknown as HTMLElement, key, member, open: () => undefined });
const steps = (all: TriggerEntry[], at: El) => numbersOf(all, at as unknown as Element).map((t) => (t.el() as unknown as El).name);

test('the numbers of a focused element: its own; the row’s; the named ones in order; then the passive ones inside, in document order', () => {
  const row = el('row', null);
  const cell = el('cell', row);
  const result = entry(el('result', row), null, false);
  const b = entry(el('m2', row), null, true);
  const a = entry(el('m1', cell), null, true);
  const all = [b, result, a];
  expect(steps(all, row)).toEqual(['result', 'm1', 'm2']);
  expect(steps(all, cell)).toEqual(['m1']);
  expect(steps(all, result.el() as unknown as El)).toEqual(['result']);
  // a view that names its readout, its domain, a scale bar (inside it) and the ticks: the readout first, the bar once
  const view = el('view', null, 'readout domain scale tick-0 tick-1 not-here');
  const listed = [
    entry(el('tick1', null), 'tick-1', true),
    entry(el('bar', view), 'scale', true),
    entry(el('tick0', null), 'tick-0', true),
    entry(el('domain', null), 'domain', true),
    entry(el('readout', null), 'readout', false),
    entry(el('other', null), 'other', true),
  ];
  expect(steps(listed, view)).toEqual(['readout', 'domain', 'bar', 'tick0', 'tick1']);
  expect(steps(listed, el('inner', view))).toEqual(['readout', 'domain', 'bar', 'tick0', 'tick1']);
  expect(steps(listed, el('elsewhere', null))).toEqual([]);
  // a key with a space (a case key's path) is one key when a number has it
  const key = el('key', null, 'st:boundaries.my wall.velocity');
  expect(steps([entry(el('v', null), 'st:boundaries.my wall.velocity', false)], key)).toEqual(['v']);
});

// ---- static

test('the new form adds no tab stop and no old trigger; no port operation; no unit made or converted', () => {
  expect(PROVENANCE_OPS).toEqual(['run', 'copy']);
  const p = code('provenance/Provenance.tsx');
  const traced = p.slice(p.indexOf('export function Traced'), p.indexOf('export function Provenance('));
  expect(traced).not.toMatch(/tabIndex|onClick|role=|<button|data-provenance-trigger/);
  expect(traced).toContain('data-provenance-member');
  // the old trigger's places, as tests/unit/provenance.spec.ts counts them
  const old = (f: string) => (code(f).match(/<Provenance\b/g) ?? []).length;
  expect([old('runs/RunsView.tsx'), old('workspace/RunWorkspace.tsx'), old('field/FieldViewport.tsx')]).toEqual([8, 5, 4]);
  const traceds = (f: string) => (code(f).match(/<Traced\b/g) ?? []).length;
  expect(traceds('runs/RunExtras.tsx')).toBe(3);
  expect(traceds('field/FieldViewport.tsx')).toBe(2);
  expect(traceds('field/Legend.tsx')).toBe(1);
  expect(traceds('workspace/TimePanel.tsx')).toBe(1);
  expect(traceds('project/HomeCounts.tsx')).toBe(1);
  // no factor between units: the sources do no arithmetic but the snapshot and tick numbers (+ 1, - 1)
  const s = code('provenance/sources.ts');
  const ours = s.slice(s.indexOf('export function residualValue'));
  expect(ours).not.toMatch(/ [*/] |toFixed|toPrecision|1e\d|1000/);
  expect(code('project/HomeCounts.tsx')).not.toMatch(/ [*/] \d|1000|1024/);
});
