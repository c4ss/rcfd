// The provenance of each kind of number the Studio shows (F-828), as pure functions of what the surface already holds:
// the engine's answer it read the number from. Nothing here reads a file or computes a value; a unit is the engine's
// string or the reason there is none (G2), and the reader named is the one that read the number.
import type { NumberSource, SessionNumberSource } from '../../shared/provenance.js';

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
  /** The engine's unit when its key says one (elapsed_s), else null with why. */
  unit: { unit: string; from: string } | { why: string };
  file?: string;
  readBy?: string;
}): NumberSource {
  return {
    quantity: o.quantity,
    shown: o.shown,
    exact: o.value === null ? null : String(o.value),
    unit: 'unit' in o.unit ? { kind: 'engine', unit: o.unit.unit, from: o.unit.from } : { kind: 'none', why: o.unit.why },
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
        : { kind: 'none', why: `not recorded: runs/${o.run}/fields/store.rts gives a snapshot’s time without a unit` },
    run: o.run,
    snapshot: { index: o.snapshot, of: o.of, step: o.step, time: o.time },
    step: o.step,
    file: `runs/${o.run}/fields/store.rts`,
    readBy: STORE_READER,
  };
}

export { READ_RESIDUALS, STORE_READER };
