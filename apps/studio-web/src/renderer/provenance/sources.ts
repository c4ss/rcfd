// The provenance of each kind of number the Studio shows (F-828), as pure functions of what the surface already holds:
// the engine's answer it read the number from. Nothing here reads a file or computes a value; a unit is the engine's
// string or the reason there is none (G2), and the reader named is the one that read the number.
import type { Fact, NumberSource, ProjectNumberSource, SessionNumberSource } from '../../shared/provenance.js';

/** A stop of the time panel: a snapshot of the store (its index), or a residual sample of a run without fields. */
export interface StopFacts {
  readonly step: number;
  readonly time: number | null;
  readonly snapshot: number | null;
  /** Stops in all (the store's snapshots, or the samples). */
  readonly of: number;
}

const snapshotOf = (stop: StopFacts | null, run: string): NumberSource['snapshot'] =>
  stop === null
    ? { none: 'none: no stop of the time line' }
    : stop.snapshot === null
      ? { none: `none: runs/${run}/fields holds no snapshot; a residual sample` }
      : { index: stop.snapshot, of: stop.of, step: stop.step, time: stop.time };

const READ_RESIDUALS = 'read_residuals (rcfd-mcp, read-only), the engine’s reader of residuals.jsonl';
const STORE_READER = 'the Studio’s reader of store.rts (route local), equal bit for bit to rcfd-tsstore’s (ADR 0350 P12)';

/** A residual or monitor value at the time line's stop, read from runs/<id>/residuals.jsonl. */
export function seriesValue(o: {
  run: string;
  name: string;
  kind: 'residual' | 'monitor' | 'other';
  shown: string;
  value: number | null;
  stop: StopFacts | null;
  step: number;
  /** The series unit as the run port read it from output/monitors.json, or null with the note. */
  unit: string | null;
  unitsNote: string | null;
}): NumberSource {
  const from = `runs/${o.run}/output/monitors.json`;
  return {
    quantity: o.name,
    shown: o.shown,
    exact: o.value === null ? null : String(o.value),
    unit: o.unit !== null ? { kind: 'engine', unit: o.unit, from } : { kind: 'none', why: o.unitsNote ?? `${from} names no unit for ${o.name}` },
    run: o.run,
    snapshot: snapshotOf(o.stop, o.run),
    step: o.step,
    file: `runs/${o.run}/residuals.jsonl`,
    readBy: READ_RESIDUALS,
  };
}

/** The snapshot counter ("snapshot 3 of 24"): the store's index, or the residual samples of a run without fields. */
export function counterValue(o: { run: string; shown: string; stop: StopFacts | null }): NumberSource {
  const fields = o.stop !== null && o.stop.snapshot !== null;
  return {
    quantity: fields ? 'snapshot' : 'sample',
    shown: o.shown,
    exact: null,
    unit: { kind: 'none', why: fields ? 'a count of the store’s snapshots' : 'a count of residuals.jsonl’s lines' },
    run: o.run,
    snapshot: snapshotOf(o.stop, o.run),
    step: o.stop?.step ?? null,
    file: fields ? `runs/${o.run}/fields/store.rts` : `runs/${o.run}/residuals.jsonl`,
    readBy: fields ? STORE_READER : READ_RESIDUALS,
  };
}

/** The stored value of one cell of one snapshot (the field viewport's readout). */
export function cellValue(o: {
  run: string;
  quantity: string;
  shown: string;
  value: number;
  unit: string;
  cell: number;
  triangle: number;
  snapshot: number;
  of: number;
  step: number;
  time: number | null;
}): NumberSource {
  return {
    quantity: o.quantity,
    shown: o.shown,
    exact: String(o.value),
    unit: { kind: 'engine', unit: o.unit, from: `runs/${o.run}/fields/store.rts (the field’s unit)` },
    run: o.run,
    snapshot: { index: o.snapshot, of: o.of, step: o.step, time: o.time },
    step: o.step,
    file: `runs/${o.run}/fields/store.rts`,
    readBy: STORE_READER,
    more: [
      ['Cell', String(o.cell)],
      ['Triangle', String(o.triangle)],
    ],
  };
}

/** A number of the run's status.json or its run list entry (steps, time, the solver's process, exit code). */
export function statusValue(o: {
  run: string;
  quantity: string;
  shown: string;
  value: number | null;
  /** The engine's unit when its key says one (elapsed_s), the run's time unit (`time`, ADR 0734), else null with why. */
  unit: { unit: string; from: string } | { why: string } | 'time';
  file?: string;
  readBy?: string;
}): NumberSource {
  return {
    quantity: o.quantity,
    shown: o.shown,
    exact: o.value === null ? null : String(o.value),
    unit: o.unit === 'time' ? { kind: 'time' } : 'unit' in o.unit ? { kind: 'engine', unit: o.unit.unit, from: o.unit.from } : { kind: 'none', why: o.unit.why },
    run: o.run,
    snapshot: { none: 'none: a fact of the run, not of a snapshot' },
    step: null,
    file: o.file ?? `runs/${o.run}/status.json`,
    readBy: o.readBy ?? 'list_runs and run_status (rcfd-mcp, read-only)',
  };
}

/** A final report value of metrics.json; its unit is the series' unit in output/monitors.json. */
export function reportValue(o: { run: string; name: string; shown: string; value: number }): NumberSource {
  return {
    quantity: o.name,
    shown: o.shown,
    exact: String(o.value),
    unit: { kind: 'monitors', name: o.name },
    run: o.run,
    snapshot: { none: 'none: the run’s final report value' },
    step: null,
    file: `runs/${o.run}/metrics.json`,
    readBy: 'list_runs (rcfd-mcp, read-only), metrics.json reports',
  };
}

/** A count the workspace read: snapshots stored, residual lines. */
export function countValue(o: { run: string; quantity: string; shown: string; file: string; readBy: string; what: string }): NumberSource {
  return {
    quantity: o.quantity,
    shown: o.shown,
    exact: null,
    unit: { kind: 'none', why: o.what },
    run: o.run,
    snapshot: { none: 'none: a count over the run' },
    step: null,
    file: o.file,
    readBy: o.readBy,
  };
}

// ---- numbers of a service session (no run behind them)

const SERVICE = 'rcfd serve (rcfd-service/1)';

/** A setting's value as settings.get answered it; its unit is the case schema's (settings.schema), never the Studio's. */
export function settingValue(o: {
  session: string;
  path: string;
  shown: string;
  value: unknown;
  /** The schema's pattern the key matched (`boundaries.*.velocity`), for where the unit is written. */
  pattern: string;
  /** x-rcfd-unit, else x-rcfd-si-dimension, of the schema node; null when it names neither. */
  unit: { unit: string; kind: 'x-rcfd-unit' | 'x-rcfd-si-dimension' } | null;
  /** Whether the case sets it, or the value is the schema's default. */
  set: boolean;
}): SessionNumberSource {
  return {
    quantity: o.path,
    shown: o.shown,
    exact: o.value === undefined ? null : JSON.stringify(o.value),
    unit: o.unit
      ? { kind: 'engine', unit: o.unit.unit, from: `settings.schema, ${o.unit.kind} of ${o.pattern}` }
      : { kind: 'none', why: `the case schema names no unit for ${o.pattern}` },
    session: o.session,
    at: { none: 'none: a setting of the case, not a value of an iteration' },
    readBy: `settings.get of ${SERVICE}`,
    more: [['Source', o.set ? 'set in the session’s case (settings.get)' : 'not set: the schema’s default (settings.schema)']],
  };
}

/** The latest sample of a residual or monitor series of a session (monitors.get), or its iteration count (run.status). */
export function sessionSeriesValue(o: {
  session: string;
  name: string;
  shown: string;
  value: number;
  /** The abscissa of the sample and what it is (`iteration`, or the time of a transient run as monitors.get names it). */
  x: number;
  xLabel: string;
  /** The series' unit as monitors.get gave it, or null. */
  unit: string | null;
  residual: boolean;
}): SessionNumberSource {
  return {
    quantity: o.name,
    shown: o.shown,
    exact: String(o.value),
    unit:
      o.unit !== null
        ? { kind: 'engine', unit: o.unit, from: 'monitors.get (the series’ unit)' }
        : { kind: 'none', why: o.residual ? 'monitors.get gives no unit for a residual' : `monitors.get gives no unit for ${o.name}` },
    session: o.session,
    at: o.xLabel === 'iteration' ? { iteration: o.x } : { none: `none: the sample is at ${o.xLabel} ${o.x}` },
    readBy: `monitors.get of ${SERVICE}, the latest sample`,
  };
}

/** The session's iteration as run.status answered it. */
export function sessionIterationValue(o: { session: string; iteration: number }): SessionNumberSource {
  return {
    quantity: 'iteration',
    shown: String(o.iteration),
    exact: null,
    unit: { kind: 'none', why: 'a count of the solver’s iterations' },
    session: o.session,
    at: { iteration: o.iteration },
    readBy: `run.status of ${SERVICE} (simulation.iteration)`,
  };
}

/** A number of the field caption: the step or the time of the shown snapshot, as the store records it. */
export function snapshotFactValue(o: {
  run: string;
  quantity: 'step' | 'time';
  shown: string;
  value: number | null;
  snapshot: number;
  of: number;
  step: number;
  time: number | null;
}): NumberSource {
  return {
    quantity: o.quantity,
    shown: o.shown,
    exact: o.value === null ? null : String(o.value),
    unit:
      o.quantity === 'step'
        ? { kind: 'none', why: 'a count of the solver’s steps' }
        : // ADR 0734 P2: store.rts keeps its bytes; the engine names the unit of a snapshot's time in its replies
          { kind: 'time' },
    run: o.run,
    snapshot: { index: o.snapshot, of: o.of, step: o.step, time: o.time },
    step: o.step,
    file: `runs/${o.run}/fields/store.rts`,
    readBy: STORE_READER,
  };
}

// ---- ADR 0795: the rest of the numbers on screen

/** The latest residual of an equation in the runs table (run_status, the last line of residuals.jsonl). */
export function residualValue(o: { run: string; equation: string; shown: string; value: number }): NumberSource {
  return {
    quantity: `residual of ${o.equation}`,
    shown: o.shown,
    exact: String(o.value),
    unit: { kind: 'monitors', name: `res_${o.equation}` },
    run: o.run,
    snapshot: { none: 'none: the last line of residuals.jsonl, not a snapshot' },
    step: null,
    file: `runs/${o.run}/residuals.jsonl`,
    readBy: `run_status (rcfd-mcp, read-only), last_residual.residuals.${o.equation}, the file’s last line`,
    more: [['Rounded', 'by the Studio, to 3 significant figures']],
  };
}

/** Where the two ends of a legend come from. */
export type LegendRange =
  | { readonly kind: 'snapshot' }
  | { readonly kind: 'run'; readonly snapshots: number }
  | { readonly kind: 'fixed' }
  /** One range of two runs (a comparison side by side): the other run and its snapshot. */
  | { readonly kind: 'shared'; readonly other: string; readonly snapshot: number; readonly of: number }
  /** The difference of two runs: the baseline and its snapshot (the number's own run is the compared one). */
  | { readonly kind: 'difference'; readonly baseline: string; readonly snapshot: number; readonly of: number };

/** A tick of a field's legend: an even division of the drawn range. */
export function legendTick(o: {
  run: string;
  quantity: string;
  shown: string;
  value: number;
  index: number;
  ticks: number;
  lo: number;
  hi: number;
  unit: string;
  range: LegendRange;
  snapshot: number;
  of: number;
  step: number | null;
  time: number | null;
}): NumberSource {
  const r = o.range;
  const at = `${o.snapshot + 1} of ${o.of}`;
  const range =
    r.kind === 'snapshot'
      ? 'the least and the greatest of the values shown of this snapshot, found by the Studio'
      : r.kind === 'run'
        ? `the least and the greatest of the shown values over ${r.snapshots} snapshots, found by the Studio`
        : r.kind === 'fixed'
          ? 'typed in this window, not recorded by the engine'
          : r.kind === 'shared'
            ? 'one range of both runs: the least and the greatest of the values shown of each, found by the Studio'
            : 'the least and the greatest of the differences shown (this run less the baseline, subtracted by the Studio in f64 and drawn as f32)';
  const more: Fact[] = [
    ['Tick', `${o.index + 1} of ${o.ticks}: ${o.index === 0 ? 'the bottom' : o.index === o.ticks - 1 ? 'the top' : 'an even division'} of the range`],
    ['Range', `${o.lo} to ${o.hi}, ${range}`],
    ['Rounded', 'by the Studio, to 5 significant figures'],
  ];
  if (r.kind === 'shared') more.push(['Compared run', `${r.other}, snapshot ${r.snapshot + 1} of ${r.of}, runs/${r.other}/fields/store.rts`]);
  if (r.kind === 'difference') more.push(['Baseline run', `${r.baseline}, snapshot ${r.snapshot + 1} of ${r.of}, runs/${r.baseline}/fields/store.rts`]);
  return {
    quantity: `legend of ${o.quantity}`,
    shown: o.shown,
    exact: String(o.value),
    unit: { kind: 'engine', unit: o.unit, from: `runs/${o.run}/fields/store.rts (the field’s unit)` },
    run: o.run,
    snapshot:
      r.kind === 'run'
        ? { none: `none: the range is over ${r.snapshots} snapshots (shown: ${at})` }
        : r.kind === 'fixed'
          ? { none: `none: a typed range (shown: ${at})` }
          : { index: o.snapshot, of: o.of, step: o.step ?? 0, time: o.time },
    step: null,
    file: `runs/${o.run}/fields/store.rts`,
    readBy: STORE_READER,
    more,
  };
}

/** Where the unit of the stored mesh's lengths comes from, for the popover's Unit. */
const lengthFrom = (from: 'data.open' | 'derive' | 'documented'): string =>
  from === 'data.open'
    ? 'length_unit of data.open'
    : from === 'derive'
      ? 'length_unit of the engine’s derived picture of this store'
      : 'the engine’s documented unit of a stored mesh (ADR 0738); not recorded in store.rts';

interface LengthOf {
  readonly run: string;
  readonly shown: string;
  readonly unit: { readonly unit: string; readonly from: 'data.open' | 'derive' | 'documented' };
}

/** The domain's size: the extents of the stored mesh's points. */
export function domainValue(o: LengthOf & { extents: readonly number[]; axes: readonly string[]; twoD: boolean }): NumberSource {
  return {
    quantity: 'domain',
    shown: o.shown,
    exact: o.extents.map(String).join(' × '),
    unit: { kind: 'engine', unit: o.unit.unit, from: lengthFrom(o.unit.from) },
    run: o.run,
    snapshot: { none: 'none: one mesh for every snapshot' },
    step: null,
    file: `runs/${o.run}/fields/store.rts`,
    readBy: STORE_READER,
    more: [
      ['Counted', `by the Studio: the greatest less the least coordinate of the stored mesh’s points along ${o.axes.join(', ')}${o.twoD ? ' (2D: the two in-plane axes)' : ''}`],
      ['Rounded', 'by the Studio, to 4 significant figures'],
    ],
  };
}

/** The scale bar's length: chosen by the Studio, in the unit of the stored mesh's lengths. */
export function scaleBarValue(o: LengthOf & { value: number }): NumberSource {
  return {
    quantity: 'scale bar',
    shown: o.shown,
    exact: String(o.value),
    unit: { kind: 'engine', unit: o.unit.unit, from: lengthFrom(o.unit.from) },
    run: o.run,
    snapshot: { none: 'none: a length of the view, not of a snapshot' },
    step: null,
    file: `not recorded: no file holds it (the mesh drawn is runs/${o.run}/fields/store.rts)`,
    readBy: 'not recorded: chosen by the Studio from the view’s scale',
    more: [['Chosen', 'the longest of 1, 2 or 5 times a power of ten that is at most a quarter of the view’s width']],
  };
}

interface OfProject {
  readonly folder: string;
  readonly engine: { readonly name: string; readonly version: string } | null;
}

const NO_BUDGET_UNIT = 'runner_config names no unit of its keys';

/** A budget of the runner as the budgets file sets it, or as typed and not written. */
export function budgetValue(o: OfProject & { key: string; path: string; hash: string | null; exists: boolean; inFile: number | undefined; typed: number }): ProjectNumberSource {
  const empty = Number.isNaN(o.typed);
  const file = o.inFile === undefined ? (o.exists ? 'the file does not set it' : 'there is no file') : `the file holds ${o.inFile}`;
  const more: Fact[] = [];
  if (empty) more.push(['Default', 'not recorded in the file: the runner uses its own default, which this panel does not hold']);
  if (empty ? o.inFile !== undefined : o.typed !== o.inFile) more.push(['Typed', `not written: ${file}`]);
  more.push(['Hash', o.hash ?? 'none: there is no file']);
  return {
    quantity: o.key,
    shown: empty ? 'not set' : String(o.typed),
    exact: null,
    unit: { kind: 'unrecorded', why: NO_BUDGET_UNIT },
    folder: o.folder,
    file: o.path,
    readBy: `runner_config (rcfd-mcp, read-only), values.${o.key}`,
    readAt: null,
    engine: o.engine,
    more,
  };
}

/** A budget the runner uses, as it recorded it when it started. */
export function budgetInUse(o: OfProject & { key: string; value: number; source: string; started: string }): ProjectNumberSource {
  return {
    quantity: `${o.key} in use`,
    shown: String(o.value),
    exact: null,
    unit: { kind: 'unrecorded', why: NO_BUDGET_UNIT },
    folder: o.folder,
    file: '.rcfd/runner.json',
    readBy: `runner_config (rcfd-mcp, read-only): the running runner’s record, config.${o.key}`,
    readAt: null,
    engine: o.engine,
    more: [
      ['Runner started', o.started],
      ['Config source', o.source],
    ],
  };
}

/** A count of the project home. */
export function homeCount(
  o: OfProject & {
    quantity: string;
    value: number;
    /** Project-relative file or folder counted. */
    file: string;
    readBy: string;
    /** What was counted, and by whom. */
    counted: string;
    /** The engine's own count of the same, when project_info gives one. */
    engineCount?: { readonly field: string; readonly value: number } | undefined;
    readAt: string;
  },
): ProjectNumberSource {
  const more: Fact[] = [['Counted', o.counted]];
  if (o.engineCount) more.push(['Engine’s count', `${o.engineCount.value}, ${o.engineCount.field} of project_info`]);
  return {
    quantity: o.quantity,
    shown: String(o.value),
    exact: null,
    unit: { kind: 'none', why: 'a count' },
    folder: o.folder,
    file: o.file,
    readBy: o.readBy,
    readAt: o.readAt,
    engine: o.engine,
    more,
  };
}

export { READ_RESIDUALS, STORE_READER };
