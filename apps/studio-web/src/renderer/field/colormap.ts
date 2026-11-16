// The field colour map (F-808; DESIGN.md, the Data Owns the Field Rule; guardrail G3): viridis as rcfd-render draws it,
// in a 256-entry table that colours the surface on the GPU and draws the legend.
//
// The source: rcfd-render's `ColorMap::viridis()` (crates/rcfd-render/src/lib.rs) is the 9 integer stops of
// `rcfd_post::render::colormap` (crates/rcfd-post/src/render.rs), evenly spaced, and `ColorMap::eval(t)` interpolates
// them linearly and rounds to the nearest integer. Entry k of the table is `eval(k / 255)`, computed here the way the Rust
// computes it (f64, the same operations in the same order). tests/unit/field-colormap.spec.ts reads the stops out of the
// Rust source and checks every entry and both ends.
//
// How a value finds its entry (the shader of FieldViewport.tsx does exactly this, in f32; `lutIndex` below mirrors it):
//   t = clamp((v - lo) * inv, 0, 1),  inv = f32(1 / (hi - lo)) (1 when hi <= lo, as rcfd-render),  k = floor(t * 255 + 0.5)
// so a value takes the entry nearest to its place in the range, and the colour range is the same `(v - lo) / span` that
// rcfd-render's shader uses.
//
// Tolerance (G3, stated here before the shader was written): the colour of a value is rcfd-render's colour at the
// nearest of the 256 table points t = k / 255. At those points, and at both ends of the range (and beyond them, clamped),
// it equals rcfd-render's colour exactly. Between them it can differ from rcfd-render's colour of the same t by at most
// 3 levels of 255 in one channel (on the steep last segment, 122 -> 253 in red): a table of 256 entries cannot hold the
// 635 distinct colours of the piecewise-linear map. The unit test measures that bound over a dense sweep of t.
//
// Pure: no three.js, no DOM, so the unit tests import it directly.

/** rcfd-post's viridis stops (`colormap`'s STOPS), the stops of rcfd-render's `ColorMap::viridis()`. */
export const VIRIDIS_STOPS: readonly (readonly [number, number, number])[] = [
  [68, 1, 84],
  [72, 36, 117],
  [65, 68, 135],
  [53, 95, 141],
  [42, 120, 142],
  [34, 168, 132],
  [68, 191, 112],
  [122, 209, 81],
  [253, 231, 37],
];

export const LUT_SIZE = 256;

/** The most a table colour differs from rcfd-render's colour of the same t, per channel (see the tolerance above). */
export const LUT_TOLERANCE_LEVELS = 3;

/** rcfd-render's `ColorMap::eval` on these stops: t clamped to [0, 1], non-finite gives the first stop. */
function evalStops(stops: readonly (readonly [number, number, number])[], t: number): [number, number, number] {
  const tt = Number.isFinite(t) ? Math.min(Math.max(t, 0), 1) : 0;
  const x = tt * (stops.length - 1);
  const i = Math.min(Math.floor(x), stops.length - 2);
  const f = x - i;
  const a = stops[i]!;
  const b = stops[i + 1]!;
  // Rust's f64::round rounds half away from zero; the values are non-negative, where Math.round does the same
  return [0, 1, 2].map((k) => Math.round(a[k]! * (1 - f) + b[k]! * f)) as [number, number, number];
}

/** The table: 256 RGBA entries (alpha 255), entry k = rcfd-render's viridis at k / 255. */
export function lutBytes(): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(4 * LUT_SIZE);
  for (let k = 0; k < LUT_SIZE; k++) {
    const [r, g, b] = evalStops(VIRIDIS_STOPS, k / (LUT_SIZE - 1));
    out.set([r, g, b, 255], 4 * k);
  }
  return out;
}

/** A colour range as the shader gets it: `lo` and `inv` as f32. */
export interface ColourRange {
  readonly lo: number;
  readonly hi: number;
  /** f32(lo). */
  readonly loF32: number;
  /** f32(1 / (hi - lo)), 1 when hi <= lo. */
  readonly invF32: number;
}

export function colourRange(lo: number, hi: number): ColourRange {
  const span = hi > lo ? hi - lo : 1;
  return { lo, hi, loF32: Math.fround(lo), invF32: Math.fround(1 / span) };
}

/** The min and max of the finite values (null when there is none). */
export function finiteRange(values: ArrayLike<number>): { lo: number; hi: number } | null {
  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (let i = 0; i < values.length; i++) {
    const v = values[i]!;
    if (v < lo) lo = v;
    if (v > hi) hi = v;
  }
  return lo <= hi ? { lo, hi } : null;
}

/**
 * The table entry of value `v` (an f32), computed in f32 as the shader computes it. Each JavaScript operation is done in
 * f64 and rounded to f32 by Math.fround, which gives the correctly rounded f32 result of the operation (f64 has more than
 * twice f32's precision, so the double rounding is exact). Non-finite values take entry 0, as rcfd-render's map does.
 */
export function lutIndex(v: number, r: ColourRange): number {
  if (!Number.isFinite(v)) return 0;
  const t = Math.fround(Math.fround(v - r.loF32) * r.invF32);
  const c = Math.min(Math.max(t, 0), 1);
  const k = Math.floor(Math.fround(Math.fround(c * 255) + 0.5));
  return Math.min(Math.max(k, 0), LUT_SIZE - 1);
}

/** The value's place in the range before it is rounded to an entry, `t * 255` (for the tie check of the tests). */
export function lutPlace(v: number, r: ColourRange): number {
  const t = Math.fround(Math.fround(v - r.loF32) * r.invF32);
  return Math.fround(Math.min(Math.max(t, 0), 1) * 255);
}

/** The colour the Studio draws for value `v`. */
export function colourOf(lut: Uint8Array, v: number, r: ColourRange): [number, number, number] {
  const k = 4 * lutIndex(v, r);
  return [lut[k]!, lut[k + 1]!, lut[k + 2]!];
}

/** A legend tick label as rcfd-studio's `tick_label` writes it (`{:.4e}`): "1.2345e-3", "0.0000e0". */
export function tickLabel(v: number): string {
  if (!Number.isFinite(v)) return String(v);
  return v.toExponential(4).replace('e+', 'e');
}

/** `n` evenly spaced ticks from lo to hi (both included). */
export function ticks(lo: number, hi: number, n = 5): number[] {
  if (!(hi > lo)) return [lo];
  return Array.from({ length: n }, (_, i) => (i === n - 1 ? hi : lo + ((hi - lo) * i) / (n - 1)));
}

/**
 * The shortest decimal text that reads back as the same f32 (`Math.fround(Number(text)) === v`), as Rust prints an f32
 * with `{}` up to the notation: the hover readout shows the stored value, not a rounded one (G3).
 */
export function f32Text(v: number): string {
  if (!Number.isFinite(v)) return String(v);
  if (v === 0) return Object.is(v, -0) ? '-0' : '0';
  for (let p = 1; p <= 9; p++) {
    const s = Number(v.toPrecision(p));
    if (Math.fround(s) === v) return String(s);
  }
  return String(v);
}
