// The monitor histories of `monitors.get` (format `rcfd-monitors` version 1, crates/rcfd-post/src/monitors.rs) as they
// cross the service port (F-811). Shared by the main process, which packs them, and the renderer, which plots them; no
// Node or DOM types.
//
// On the wire the service writes every number with the shortest round-trip form and every non-finite number as `null`
// (ADR 0353). JSON.parse gives back the same f64 for each finite number, so a Float64Array holding those numbers, with
// NaN where the wire had `null`, carries exactly the information of the reply. The main process makes that array
// (`packMonitors`) because a typed array crosses the port and the context bridge as one block of bytes, where an array
// of 1e5 numbers is copied number by number twice (ADR 0801 addendum 3, P3). Any reply that is not the documented
// shape is passed on unchanged.

/** One series of a `monitors.get` reply: abscissa and ordinate of equal length, and the unit (may be empty). */
export interface MonitorSeriesWire {
  readonly unit: string;
  readonly x: Float64Array | readonly (number | null)[];
  readonly y: Float64Array | readonly (number | null)[];
}

/** A `monitors.get` reply. `x_label` is `iteration` or `time` (the schema of rcfd-post's monitor_json_schema). */
export interface MonitorsReply {
  readonly format: string;
  readonly version: number;
  readonly x_label: string;
  readonly series: Readonly<Record<string, MonitorSeriesWire>>;
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

/** The numbers of a JSON array as f64, `null` as NaN; null when an element is neither a number nor null. */
function toFloat64(a: readonly unknown[]): Float64Array | null {
  const out = new Float64Array(a.length);
  for (let i = 0; i < a.length; i++) {
    const v = a[i];
    if (typeof v === 'number') out[i] = v;
    else if (v === null) out[i] = Number.NaN;
    else return null;
  }
  return out;
}

/**
 * A `monitors.get` result with every series' `x` and `y` as Float64Array. Returns `doc` itself when it is not the
 * documented shape (a series without a text unit or without arrays, arrays of unequal length, an element that is not a number or null), so
 * nothing the service sent is changed or dropped when the format is not what this code knows.
 */
export function packMonitors(doc: unknown): unknown {
  if (!isRecord(doc) || doc['format'] !== 'rcfd-monitors' || !isRecord(doc['series'])) return doc;
  const series: Record<string, MonitorSeriesWire> = {};
  for (const [name, s] of Object.entries(doc['series'])) {
    if (!isRecord(s) || typeof s['unit'] !== 'string' || !Array.isArray(s['x']) || !Array.isArray(s['y'])) return doc;
    if (s['x'].length !== s['y'].length) return doc;
    const x = toFloat64(s['x']);
    const y = toFloat64(s['y']);
    if (!x || !y) return doc;
    series[name] = { ...s, x, y } as unknown as MonitorSeriesWire;
  }
  return { ...doc, series };
}

/** Methods whose results the main process packs before they cross the service port. */
export const PACKED_METHODS: ReadonlySet<string> = new Set(['monitors.get']);

/** The result of `method` as it crosses the port: packed for the methods above, else unchanged. */
export function packResult(method: string, result: unknown): unknown {
  return PACKED_METHODS.has(method) ? packMonitors(result) : result;
}
