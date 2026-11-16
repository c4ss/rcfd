// The monitor histories of one run as the plots hold them (F-811): one growable pair of Float64Arrays per series, the
// values exactly as the service sent them (G3). Pure code, no DOM and no engine call, so the unit tests run it in Node.

/** One series of one run. Grows by `append`, or is replaced whole by `replace` (which raises `generation`). */
export class SeriesData {
  private xs: Float64Array;
  private ys: Float64Array;
  private n = 0;
  /** Raised on every `replace`: a plot that holds a copy rebuilds it, otherwise it only adds samples from its length. */
  generation = 0;

  constructor(
    readonly name: string,
    public unit: string,
    capacity = 256,
  ) {
    this.xs = new Float64Array(capacity);
    this.ys = new Float64Array(capacity);
  }

  get length(): number {
    return this.n;
  }

  /** The abscissae held, as a view (valid until the next change). */
  get x(): Float64Array {
    return this.xs.subarray(0, this.n);
  }

  /** The ordinates held, as a view (valid until the next change). */
  get y(): Float64Array {
    return this.ys.subarray(0, this.n);
  }

  xAt(i: number): number {
    return this.xs[i]!;
  }

  yAt(i: number): number {
    return this.ys[i]!;
  }

  /** Appends samples `from` .. end of `x` and `y` (equal lengths). */
  append(x: ArrayLike<number>, y: ArrayLike<number>, from = 0): void {
    const add = x.length - from;
    if (add <= 0) return;
    this.reserve(this.n + add);
    for (let i = from; i < x.length; i++) {
      this.xs[this.n] = x[i]!;
      this.ys[this.n] = y[i]!;
      this.n++;
    }
  }

  /** Replaces every sample (a full fetch of this series). */
  replace(unit: string, x: ArrayLike<number>, y: ArrayLike<number>): void {
    this.unit = unit;
    this.n = 0;
    this.generation++;
    this.append(x, y);
  }

  private reserve(size: number): void {
    if (size <= this.xs.length) return;
    let cap = this.xs.length;
    while (cap < size) cap *= 2;
    const nx = new Float64Array(cap);
    const ny = new Float64Array(cap);
    nx.set(this.xs.subarray(0, this.n));
    ny.set(this.ys.subarray(0, this.n));
    this.xs = nx;
    this.ys = ny;
  }
}

/** A JSON array of numbers and nulls (null: non-finite on the wire) or a packed Float64Array, as f64 values. */
export function asFloat64(a: unknown): Float64Array | null {
  if (a instanceof Float64Array) return a;
  if (!Array.isArray(a)) return null;
  const out = new Float64Array(a.length);
  for (let i = 0; i < a.length; i++) {
    const v: unknown = a[i];
    if (typeof v === 'number') out[i] = v;
    else if (v === null) out[i] = Number.NaN;
    else return null;
  }
  return out;
}

/** The series of a `monitors.get` result, checked: name, unit, x and y of equal length. Null when it is not that shape. */
export function readSeries(result: unknown): { xLabel: string; series: Map<string, { unit: string; x: Float64Array; y: Float64Array }> } | null {
  if (typeof result !== 'object' || result === null) return null;
  const doc = result as { format?: unknown; version?: unknown; x_label?: unknown; series?: unknown };
  // version 1 only: a later version may mean something else by the same keys
  if (doc.format !== 'rcfd-monitors' || doc.version !== 1 || typeof doc.series !== 'object' || doc.series === null) return null;
  const out = new Map<string, { unit: string; x: Float64Array; y: Float64Array }>();
  for (const [name, s] of Object.entries(doc.series as Record<string, unknown>)) {
    if (typeof s !== 'object' || s === null) return null;
    const { unit, x, y } = s as { unit?: unknown; x?: unknown; y?: unknown };
    const fx = asFloat64(x);
    const fy = asFloat64(y);
    if (typeof unit !== 'string' || !fx || !fy || fx.length !== fy.length) return null;
    out.set(name, { unit, x: fx, y: fy });
  }
  return { xLabel: typeof doc.x_label === 'string' ? doc.x_label : '', series: out };
}

/** The same f64 value (SameValue: NaN equals NaN, 0 and -0 differ). */
export function sameValue(a: number, b: number): boolean {
  return Object.is(a, b);
}

/**
 * The first index whose value is not <= `after`, by binary search: Rust's `slice::partition_point(|v| *v <= after)`,
 * which the service uses for `since` (ADR 0353). For values that do not decrease it is the count of values <= after.
 */
export function partitionPoint(xs: ArrayLike<number>, after: number): number {
  let lo = 0;
  let hi = xs.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (xs[mid]! <= after) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** The largest f64 below `x` (finite x); NaN for NaN. */
export function nextDown(x: number): number {
  if (Number.isNaN(x) || x === Number.NEGATIVE_INFINITY) return x;
  if (x === 0) return -Number.MIN_VALUE;
  const f = new Float64Array([x]);
  const b = new BigInt64Array(f.buffer);
  b[0] = b[0]! + (x > 0 ? -1n : 1n);
  return f[0]!;
}
