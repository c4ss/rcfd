// What the run workspace shows (F-810), as pure functions of the engine's answers, so the unit tests check them without
// a window: the positions the scrubber stops at (the store's snapshots, or the residual samples of a run without
// fields), the shared axis, the run bands of DESIGN.md (segments by phase, the end's pictogram and word) and the value of
// a series at a step. Every number is the engine's; nothing is interpolated or smoothed (G3).
import type { StoreInfo } from '../../shared/data.js';
import type { RunSeries, RunSeriesEntry } from '../../shared/run.js';
import { linearAxis, type Axis } from '../plots/range.js';
import { markWord, type MarkState } from '../ui/pictogramPaths.js';
import { plain, shownState, unknownWord, type RunRow } from '../runs/model.js';

/** One place the scrubber stops: a snapshot of the store, or (a run without fields) a residual sample. */
export interface Position {
  readonly step: number;
  /** The simulated time, or null when the run has none (a steady run's solver writes 0) or the source gives none. */
  readonly time: number | null;
  /** 0-based index into the store's snapshots; null for a residual sample of a run without fields. */
  readonly snapshot: number | null;
}

/** A time the engine gave that is a simulated time: not for a steady run, and not the 0 of a run not known transient. */
export function simulatedTime(time: number, steady: boolean | null): number | null {
  if (!Number.isFinite(time) || steady === true) return null;
  if (steady === null && time === 0) return null;
  return time;
}

/** The series whose steps stand for the run when it saved no fields: the first residual, else the first series. */
export function stepSeries(series: RunSeries | null): RunSeriesEntry | null {
  if (!series) return null;
  return series.series.find((s) => s.kind === 'residual') ?? series.series[0] ?? null;
}

/**
 * The scrubber's stops: every snapshot of the store in order; with no snapshot, every sample of the step series (each
 * is a line of residuals.jsonl; a decimated series stops only at the samples the engine kept).
 */
export function positionsOf(store: StoreInfo | null, series: RunSeries | null, steady: boolean | null): Position[] {
  if (store && store.snapshots.length > 0) return store.snapshots.map((s, i) => ({ step: s.step, time: simulatedTime(s.time, steady), snapshot: i }));
  const s = stepSeries(series);
  if (!s) return [];
  return s.x.map((x) => ({ step: x, time: null, snapshot: null }));
}

/** The index of the stop nearest `step` (the first of two equally near); -1 when there are none. */
export function nearestPosition(positions: readonly Position[], step: number): number {
  if (positions.length === 0) return -1;
  let lo = 0;
  let hi = positions.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (positions[mid]!.step < step) lo = mid + 1;
    else hi = mid;
  }
  if (lo > 0 && Math.abs(positions[lo - 1]!.step - step) <= Math.abs(positions[lo]!.step - step)) return lo - 1;
  return lo;
}

/** The value of a series at exactly `step`, by binary search over its steps; null when it has no sample there. */
export function sampleAt(x: readonly number[], y: readonly number[], step: number): number | null {
  let lo = 0;
  let hi = x.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (x[mid]! < step) lo = mid + 1;
    else hi = mid;
  }
  return lo < x.length && x[lo] === step ? y[lo]! : null;
}

/**
 * Play's pace (ADR 0801 Addendum 22, the maintainer's choice of 2026-11-17): at 1x one stored snapshot per wall-clock
 * second. The alternative offered was one second of simulated time per second. A run without fields steps through its
 * residual samples every PLAY_SAMPLE_MS at 1x, as before.
 */
export const PLAY_SNAPSHOT_MS = 1000;
export const PLAY_SAMPLE_MS = 150;

/** The Play speeds of the menu beside Play (the maintainer, 2026-11-17); 1 is the default. */
export const PLAY_SPEEDS = [0.25, 0.5, 1, 2, 4] as const;
export type PlaySpeed = (typeof PLAY_SPEEDS)[number];

/** A speed as the menu and its name say it: "0.25×", "1×". */
export const speedText = (s: number): string => `${s}×`;

/** A speed in words for the button's name and tooltip: "Play speed 2×, two snapshots a second". */
export function speedWords(s: number): string {
  const per = s >= 1 ? `${s === 1 ? 'one snapshot' : `${s} snapshots`} a second` : `one snapshot every ${1 / s} seconds`;
  return `Play speed ${speedText(s)}, ${per}`;
}

/**
 * How long Play waits before the next stop, `sinceMs` after it moved to the one shown (whose field is drawn): a snapshot
 * stays PLAY_SNAPSHOT_MS / speed, a sample PLAY_SAMPLE_MS / speed, and under reduced motion no stop is shorter than
 * `reducedMs` (so 2x and 4x are held to two stops a second there).
 */
export function playWait(snapshot: boolean, sinceMs: number, reduceMotion: boolean, reducedMs: number, speed = 1): number {
  const pace = (snapshot ? PLAY_SNAPSHOT_MS : PLAY_SAMPLE_MS) / (speed > 0 ? speed : 1);
  return Math.max(0, Math.max(pace, reduceMotion ? reducedMs : 0) - Math.max(0, sinceMs));
}

/** The last step a run reached, as the runner recorded it (status.json), and its stored or sampled steps. */
export function lastStep(row: Pick<RunRow, 'step'>, positions: readonly Position[]): number {
  const p = positions.length > 0 ? positions[positions.length - 1]!.step : 0;
  return Math.max(row.step, p);
}

/** The shared axis of the time panel: steps from 0 to past the furthest step of any run, on a 1-2-5 grid. */
export function sharedAxis(ends: readonly number[], target = 6): Axis {
  const hi = Math.max(1, ...ends.filter((e) => Number.isFinite(e)));
  return linearAxis(0, hi, target);
}

/**
 * The One Live Thing Rule: orange fills exactly one band segment, the live segment of the running run. With several
 * running, the selected one if it runs, else the newest. Null when none runs (a paused run is not live).
 */
export function liveRun(rows: readonly Pick<RunRow, 'id' | 'state' | 'created'>[], selected: string | null): string | null {
  const running = rows.filter((r) => r.state === 'running');
  if (running.length === 0) return null;
  if (selected !== null && running.some((r) => r.id === selected)) return selected;
  return [...running].sort((a, b) => (a.created < b.created ? 1 : a.created > b.created ? -1 : 0))[0]!.id;
}

export type SegmentKind = 'iterating' | 'converged' | 'live' | 'failed' | 'queued' | 'pending';

/** A part of a band on the shared axis (steps). A `cap` is the end mark of a phase: drawn a fixed width inside `to`. */
export interface Segment {
  readonly kind: SegmentKind;
  readonly from: number;
  readonly to: number;
  readonly cap?: boolean;
}

export interface Band {
  readonly id: string;
  readonly state: MarkState | null;
  /** The end's words, starting with the state word: "converged · it 278", "failed · it 0", "running · it 1450". */
  readonly label: string;
  /** Where the band ends on the axis (the end its label marks). */
  readonly end: number;
  readonly segments: readonly Segment[];
}

/** The end's word for a state (lower case, as in "converged · 13.2 s"). */
const word = (state: MarkState | null, raw: string): string => (state ? markWord(state) : unknownWord(raw)).toLowerCase();

/** Where on the run's own axis its end lies, in words: a simulated time for a transient run, else the iteration. */
function at(row: Pick<RunRow, 'step' | 'time' | 'detail'>): string {
  const t = row.time === null ? null : simulatedTime(row.time, row.detail?.steady ?? null);
  return t !== null ? `${plain(t)} s` : `it ${row.step}`;
}

/** One run's band on an axis that ends at `axisHi` (DESIGN.md Run bands). `live`: this run's segment is the live one. */
export function bandOf(row: RunRow, end: number, axisHi: number, live: boolean): Band {
  const state = shownState(row);
  const w = word(state, row.state);
  switch (row.state) {
    case 'queued':
      return { id: row.id, state, label: `${w} · not started`, end: 0, segments: [{ kind: 'queued', from: 0, to: axisHi }] };
    case 'running':
    case 'paused': {
      const segs: Segment[] = [];
      if (end > 0) segs.push({ kind: live ? 'live' : 'iterating', from: 0, to: end });
      if (end < axisHi) segs.push({ kind: 'pending', from: end, to: axisHi });
      return { id: row.id, state, label: `${w} · ${at({ ...row, step: end })}`, end, segments: segs };
    }
    default: {
      const segs: Segment[] = [];
      if (end > 0) segs.push({ kind: 'iterating', from: 0, to: end });
      if (state === 'converged') segs.push({ kind: 'converged', from: end, to: end, cap: true });
      if (row.state === 'failed') segs.push({ kind: 'failed', from: end, to: end, cap: true });
      return { id: row.id, state, label: `${w} · ${at({ ...row, step: end })}`, end, segments: segs };
    }
  }
}

/** The fraction of the axis at `v` (0 at lo, 1 at hi). */
export const fraction = (v: number, axis: Pick<Axis, 'lo' | 'hi'>): number => (v - axis.lo) / (axis.hi - axis.lo);

/** A band's end label goes after the end on a white chip when there is room, else inside the end (DESIGN.md). */
export const labelAfter = (end: number, axis: Pick<Axis, 'lo' | 'hi'>): boolean => fraction(end, axis) <= 0.72;

/** The flag's text: the iteration, and the simulated time when the run has one ("it 145 · t = 1.25 s"). */
export function flagText(p: Position): string {
  return p.time !== null ? `t = ${plain(p.time, 4)} s · it ${p.step}` : `it ${p.step}`;
}

/** The snapshot counter: "snapshot 3 of 20", or for a run without fields "sample 3 of 158 · no field saved". */
export function counterText(index: number, positions: readonly Position[], running: boolean): string {
  if (positions.length === 0) return running ? 'nothing computed yet' : 'no snapshot and no sample';
  const p = positions[index]!;
  const so = running ? ' computed so far' : '';
  return p.snapshot !== null ? `snapshot ${p.snapshot + 1} of ${positions.length}${so}` : `sample ${index + 1} of ${positions.length}${so} · no field saved`;
}

/** The text a screen reader reads for the scrubber's value. */
export function valueText(index: number, positions: readonly Position[], running: boolean): string {
  if (positions.length === 0) return counterText(index, positions, running);
  return `${counterText(index, positions, running)}, ${flagText(positions[index]!)}`;
}

/** A store field to show first: p when the store has it as a cell field, else its first cell field. */
export function defaultField(store: StoreInfo | null): string | null {
  const cells = store?.fields.filter((f) => f.location === 'cell') ?? [];
  return (cells.find((f) => f.name === 'p') ?? cells[0])?.name ?? null;
}

/** The absolute folder of a run's field store inside the project folder, with the folder's own separator. */
export function storePath(folder: string, run: string): string {
  const sep = folder.includes('\\') && !folder.includes('/') ? '\\' : '/';
  const base = folder.endsWith(sep) ? folder.slice(0, -1) : folder;
  return [base, 'runs', run, 'fields'].join(sep);
}
