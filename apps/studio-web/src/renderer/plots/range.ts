// Plot ranges and ticks (F-811). The rule: every range holds every value the plot draws (G3). A value the plot cannot
// draw (not finite, or not above zero on a log scale) is not drawn, is counted, and the plot says so in words; it never
// widens or narrows the range and is never clamped onto the edge. Pure code.

/** What a plot draws of its series, in plot space (log10 of the value on a log scale). */
export class Extent {
  xlo = Infinity;
  xhi = -Infinity;
  ylo = Infinity;
  yhi = -Infinity;
  /** Samples drawn. */
  drawn = 0;
  /** Samples not drawn: their x or y is not finite, or y is not above zero on a log scale. */
  skipped = 0;

  get empty(): boolean {
    return this.drawn === 0;
  }

  /** Adds one sample; returns its plot-space ordinate, or NaN when it is not drawn. */
  add(x: number, y: number, log: boolean): number {
    const py = log ? (y > 0 ? Math.log10(y) : Number.NaN) : y;
    if (!Number.isFinite(x) || !Number.isFinite(py)) {
      this.skipped++;
      return Number.NaN;
    }
    this.drawn++;
    if (x < this.xlo) this.xlo = x;
    if (x > this.xhi) this.xhi = x;
    if (py < this.ylo) this.ylo = py;
    if (py > this.yhi) this.yhi = py;
    return py;
  }

  merge(o: Extent): void {
    this.drawn += o.drawn;
    this.skipped += o.skipped;
    if (o.drawn === 0) return;
    this.xlo = Math.min(this.xlo, o.xlo);
    this.xhi = Math.max(this.xhi, o.xhi);
    this.ylo = Math.min(this.ylo, o.ylo);
    this.yhi = Math.max(this.yhi, o.yhi);
  }
}

export interface Axis {
  /** The range in plot space; lo < hi, and every drawn value v has lo <= v <= hi. */
  readonly lo: number;
  readonly hi: number;
  /** Tick positions in plot space, inside [lo, hi]. */
  readonly ticks: readonly number[];
  /** The text of each tick. */
  readonly labels: readonly string[];
}

/** 1, 2 or 5 times a power of ten, at least `raw` (raw > 0). */
export function niceStep(raw: number): number {
  const p = 10 ** Math.floor(Math.log10(raw));
  for (const m of [1, 2, 5, 10]) if (m * p >= raw) return m * p;
  return 10 * p;
}

/** A tick value as text: plain decimals for ordinary sizes, else d.ddde±x; tabular figures come from CSS. */
export function tickLabel(v: number, step: number): string {
  if (v === 0) return '0';
  const a = Math.abs(v);
  if (a >= 1e5 || a < 1e-3) {
    const digits = Math.max(0, Math.min(6, Math.floor(Math.log10(a)) - Math.floor(Math.log10(step))));
    return v.toExponential(digits).replace('e+', 'e');
  }
  const decimals = Math.max(0, Math.min(10, -Math.floor(Math.log10(step))));
  return v.toFixed(decimals);
}

/**
 * A linear axis that holds [lo, hi]: rounded out to a 1-2-5 step (about `target` intervals). A single value gets a
 * span around it. The rounding is checked after the arithmetic, so a range never ends inside the data by a rounding
 * error.
 */
export function linearAxis(lo: number, hi: number, target = 4): Axis {
  if (!(lo <= hi) || !Number.isFinite(lo) || !Number.isFinite(hi)) throw new Error(`linearAxis: no range in [${lo}, ${hi}]`);
  if (lo === hi) {
    const pad = lo === 0 ? 1 : Math.abs(lo) * 0.05;
    lo -= pad;
    hi += pad;
  }
  // hi / target - lo / target: no overflow for a span near the largest f64
  const step = niceStep(hi / target - lo / target);
  let a = Math.floor(lo / step) * step;
  let b = Math.ceil(hi / step) * step;
  for (let i = 0; i < 4 && a > lo; i++) a -= step;
  for (let i = 0; i < 4 && b < hi; i++) b += step;
  if (!(a <= lo && hi <= b && a < b && Number.isFinite(a) && Number.isFinite(b))) {
    // a span too small for the f64 grid of the step: keep the data's own bounds
    a = lo;
    b = hi;
  }
  const ticks: number[] = [];
  const labels: string[] = [];
  const first = Math.ceil(a / step);
  const count = Math.floor(b / step) - first;
  if (Number.isFinite(first) && Math.abs(first) < 2 ** 52 && count >= 0 && count <= 50) {
    for (let k = first; k <= first + count; k++) {
      const t = k * step;
      if (t < a || t > b || !Number.isFinite(t)) continue;
      ticks.push(t);
      labels.push(tickLabel(t, step));
    }
  } else {
    // the step's grid is too fine for the numbers (or not finite): the two ends only
    ticks.push(a, b);
    labels.push(tickLabel(a, b - a || Math.abs(a)), tickLabel(b, b - a || Math.abs(b)));
  }
  return { lo: a, hi: b, ticks, labels };
}

/**
 * A log axis over plot space (decades, log10 of the value) that holds [lo, hi]: whole decades out from the data, at
 * least one decade. Ticks on decades, every one when there are at most `maxTicks`, else every second, fifth, ...
 */
export function logAxis(lo: number, hi: number, maxTicks = 8): Axis {
  if (!(lo <= hi) || !Number.isFinite(lo) || !Number.isFinite(hi)) throw new Error(`logAxis: no range in [${lo}, ${hi}]`);
  let a = Math.floor(lo);
  let b = Math.ceil(hi);
  if (a === b) b = a + 1;
  if (a > lo) a -= 1;
  if (b < hi) b += 1;
  const span = b - a;
  let every = 1;
  for (const e of [1, 2, 5, 10, 20, 50, 100]) {
    every = e;
    if (span / e + 1 <= maxTicks) break;
  }
  const ticks: number[] = [];
  const labels: string[] = [];
  for (let d = Math.ceil(a / every) * every; d <= b; d += every) {
    ticks.push(d);
    labels.push(`1e${d}`);
  }
  return { lo: a, hi: b, ticks, labels };
}

/** Whether `axis` holds every drawn value of `e` on the given side (the test of the rule above). */
export function holds(axisX: Axis, axisY: Axis, e: Extent): boolean {
  if (e.empty) return true;
  return axisX.lo <= e.xlo && e.xhi <= axisX.hi && axisY.lo <= e.ylo && e.yhi <= axisY.hi;
}
