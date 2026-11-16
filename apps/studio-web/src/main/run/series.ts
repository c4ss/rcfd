// A runner run's residual and monitor histories, read through the engine (F-810): `read_residuals` of the open
// project's read-only project server (crates/rcfd-workspace, the tool Claude Code uses) parses runs/<id>/residuals.jsonl;
// `read_project_file` reads runs/<id>/output/monitors.json for the monitors' units (residuals.jsonl carries none). The
// Studio parses neither file itself (G2) and keeps nothing of them (G12).
//
// Every `read_residuals` call is kept under the engine's text budget (SERIES_POINTS, src/shared/run.ts): over it the
// engine writes the series to a file in the run folder, and the Studio writes nothing to the folder. A reply that holds
// a `path` instead of the series is refused here, with the reason.
//
// No Electron import: the unit tests drive it with a fake query.
import { SERIES_POINTS, UNITS_MAX_BYTES, type RunComparison, type RunSeries, type RunSeriesEntry, type RunValuesAt, type SeriesKind } from '../../shared/run.js';
import type { ProjectTool } from '../../shared/project.js';

/** One query of the open project's read-only server (ProjectHome.query). */
export type Query = (tool: ProjectTool, args: Record<string, unknown>) => Promise<unknown>;

type Obj = Record<string, unknown>;
const obj = (v: unknown): Obj => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Obj) : {});
const num = (v: unknown, d = 0): number => (typeof v === 'number' && Number.isFinite(v) ? v : d);

/**
 * The name a plot uses for an engine series name, and its kind: `residuals.continuity` -> `continuity`, `residuals.u`
 * -> `res_u` (as the engine's monitors.json and `monitors.get` name them), `monitors.lid_drag` -> `lid_drag`; any other
 * per-step value (`correctors`, `corrector_cap`) keeps its name.
 */
export function plotName(source: string): { name: string; kind: SeriesKind } {
  if (source === 'residuals.continuity') return { name: 'continuity', kind: 'residual' };
  if (source.startsWith('residuals.')) return { name: `res_${source.slice('residuals.'.length)}`, kind: 'residual' };
  if (source.startsWith('monitors.')) return { name: source.slice('monitors.'.length), kind: 'monitor' };
  return { name: source, kind: 'other' };
}

/** A JSON array of numbers and nulls (null: NaN on the wire), or null when it is not one. */
function values(a: unknown): number[] | null {
  if (!Array.isArray(a)) return null;
  const out: number[] = new Array<number>(a.length);
  for (let i = 0; i < a.length; i++) {
    const v: unknown = a[i];
    if (typeof v === 'number') out[i] = v;
    else if (v === null) out[i] = Number.NaN;
    else return null;
  }
  return out;
}

/** The `series` object of a `read_residuals` reply, checked; throws when the engine wrote it to a file instead. */
function seriesOf(run: string, reply: unknown): { lines: number; skipped: number; decimated: boolean; series: Map<string, { x: number[]; y: number[] }> } {
  const r = obj(reply);
  if (typeof r['path'] === 'string' || !('series' in r)) {
    throw new Error(`read_residuals of ${run} answered over the engine's size budget (${String(r['path'] ?? 'no series')}); the Studio asks for fewer samples`);
  }
  const out = new Map<string, { x: number[]; y: number[] }>();
  for (const [k, v] of Object.entries(obj(r['series']))) {
    const s = obj(v);
    const x = values(s['x']);
    const y = values(s['y']);
    if (!x || !y || x.length !== y.length) throw new Error(`read_residuals of ${run}: series ${k} is not two arrays of equal length`);
    out.set(k, { x, y });
  }
  // `decimated`: the engine cut a series of this reply to max_points (it counts the longest series of the run's file)
  return { lines: num(r['lines']), skipped: num(r['skipped_lines']), decimated: r['decimated'] === true, series: out };
}

/** The units of output/monitors.json by series name, or why there are none. */
export async function readUnits(query: Query, run: string): Promise<{ units: Map<string, string>; note: string | null }> {
  const path = `runs/${run}/output/monitors.json`;
  let text = '';
  let hash: unknown = null;
  let offset: number | null = 0;
  try {
    for (let page = 0; offset !== null; page++) {
      const r = obj(await query('read_project_file', { path, offset }));
      if (page === 0) {
        hash = r['hash'];
        if (num(r['size']) > UNITS_MAX_BYTES) {
          return { units: new Map(), note: `the units are not read: ${path} is ${num(r['size'])} bytes, more than the Studio reads for them (${UNITS_MAX_BYTES})` };
        }
      } else if (r['hash'] !== hash) {
        return { units: new Map(), note: `the units are not read: ${path} changed while it was read` };
      }
      text += typeof r['text'] === 'string' ? r['text'] : '';
      offset = typeof r['next_offset'] === 'number' ? r['next_offset'] : null;
    }
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    // the usual case while a run goes on: the file is not there yet
    if (/not found|cannot find|no such file|os error [23]\b/i.test(m)) return { units: new Map(), note: `no units: the run has no ${path} (the runner writes it when a run ends)` };
    return { units: new Map(), note: `no units: ${path} cannot be read (${m})` };
  }
  try {
    const doc = obj(JSON.parse(text));
    if (doc['format'] !== 'rcfd-monitors') return { units: new Map(), note: `no units: ${path} is not an rcfd-monitors document` };
    const units = new Map<string, string>();
    for (const [k, v] of Object.entries(obj(doc['series']))) {
      const u = obj(v)['unit'];
      if (typeof u === 'string') units.set(k, u);
    }
    return { units, note: null };
  } catch {
    return { units: new Map(), note: `no units: ${path} is not JSON` };
  }
}

/**
 * Every series of a run: the names first (two samples each, a small reply), then each series on its own with at most
 * SERIES_POINTS samples, then the units.
 */
export async function readRunSeries(query: Query, run: string, now: () => string): Promise<RunSeries> {
  const names = seriesOf(run, await query('read_residuals', { run, max_points: 2 }));
  const series: RunSeriesEntry[] = [];
  let lines = names.lines;
  let skipped = names.skipped;
  const { units, note } = names.series.size > 0 ? await readUnits(query, run) : { units: new Map<string, string>(), note: null };
  for (const source of names.series.keys()) {
    const r = seriesOf(run, await query('read_residuals', { run, names: [source], max_points: SERIES_POINTS }));
    lines = Math.max(lines, r.lines);
    skipped = Math.max(skipped, r.skipped);
    const s = r.series.get(source);
    if (!s) continue; // gone between the two reads (a run folder rewritten): not shown
    const { name, kind } = plotName(source);
    series.push({ name, source, kind, unit: units.get(name) ?? (kind === 'residual' ? '' : null), x: s.x, y: s.y, decimated: r.decimated });
  }
  return { run, lines, skippedLines: skipped, maxPoints: SERIES_POINTS, series, unitsNote: series.length > 0 ? note : null, readAt: now() };
}

/** The value of each series at one step: `read_residuals {since_step: step, max_points: 2}` keeps the sample at that step first. */
export async function readRunAt(query: Query, run: string, step: number): Promise<RunValuesAt> {
  const r = seriesOf(run, await query('read_residuals', { run, since_step: step, max_points: 2 }));
  const out: Record<string, number> = {};
  for (const [source, s] of r.series) if (s.x.length > 0 && s.x[0] === step) out[plotName(source).name] = s.y[0]!;
  return { run, step, values: out };
}

/**
 * F-815: the engine's `compare_runs` of the baseline and the compared run: each metric's two values and the engine's own
 * difference (the compared run's value minus the baseline's). Nothing is computed here.
 */
export async function readComparison(query: Query, run: string, other: string): Promise<RunComparison> {
  const r = obj(await query('compare_runs', { runs: [run, other] }));
  if (r['baseline'] !== run) throw new Error(`compare_runs answered for ${String(r['baseline'])}, not ${run}`);
  const metrics = obj(r['metrics']);
  const diffs = obj(r['differences']);
  const value = (v: unknown): number | null => (typeof v === 'number' ? v : null);
  const out: RunComparison['metrics'][number][] = [];
  for (const name of Object.keys(metrics).sort()) {
    const m = obj(metrics[name]);
    out.push({ name, a: value(m[run]), b: value(m[other]), difference: value(obj(diffs[name])[other]) });
  }
  return { baseline: run, compared: other, metrics: out, omitted: num(r['metrics_omitted']) };
}
