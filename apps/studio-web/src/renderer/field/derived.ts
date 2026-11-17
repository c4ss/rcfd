// Derived pictures of the field viewport (F-809; guardrails G2, G3, G6): which apply to a store, and the two the Studio
// draws. Pure: typed arrays in, typed arrays out, no three.js and no DOM, so the unit tests import it.
//
// What the engine computes for a stored field, read before this code (ADR 0801, addendum F-809):
// - iso-surfaces and streamlines: no entry point at all (rcfd-post's `cut_implicit` cuts by a function of position, not
//   by a cell field; there is no streamline integrator). The Studio does not draw a picture it cannot check against the
//   engine, so both are unavailable, with the reason (G6), and recorded as missing engine entry points.
// - the cut plane: rcfd-render's rule, `View::clip = (n, d)`: only points with `n . p <= d` are drawn, per fragment
//   (`crates/rcfd-render/src/shader.wgsl`, removed by F-838). The Studio applies the same rule on the GPU and in the pick (T2).
// - vectors: rcfd-post's `overlay_vectors` (the arrows of the service's `render.png`), carried to the boundary surface
//   (T3). The arrows carry the stored components unchanged.
//
// The tolerances T1 to T3 were written into the addendum before this file.
import type { FieldInfo } from '../../shared/data.js';
import { bounds, type SurfaceArrays } from './surface.js';

export const DERIVED_KINDS = ['cut', 'vectors', 'iso', 'streamlines'] as const;
export type DerivedKind = (typeof DERIVED_KINDS)[number];

/** The names (the accessible names and tooltips of the controls; G22: no standing words). */
export const DERIVED_NAME: Record<DerivedKind, string> = {
  cut: 'Cut plane',
  vectors: 'Vectors',
  iso: 'Iso-surface',
  streamlines: 'Streamlines',
};

export type Axis = 0 | 1 | 2;
export const AXIS_NAME = ['x', 'y', 'z'] as const;

/**
 * The side of the thinnest direction the fitted view looks from (FieldCanvas's `fit`: the camera sits along
 * (1, -1.6, 1.1)), so on a 2D case the arrows are drawn on the face that is seen.
 */
export const FRONT_SIGN: readonly [1 | -1, 1 | -1, 1 | -1] = [1, -1, 1];

/** T1: the stored mesh's layers along its thinnest direction. */
export interface Layers {
  /** One cell layer (render_field's "2D case"). */
  readonly twoD: boolean;
  /** The thinnest direction of the surface's bounding box (the first of equals, as rcfd-render's `view`). */
  readonly thin: Axis;
  readonly min: readonly [number, number, number];
  readonly max: readonly [number, number, number];
}

/**
 * T1: a case is 2D when every cell of the store owns a surface triangle in the plane `p[a] = min[a]` and one in
 * `p[a] = max[a]` (all three corners on the bound, compared as stored `f32`), `a` the thinnest direction.
 */
export function meshLayers(s: SurfaceArrays, nCells: number): Layers {
  const b = bounds(s.points);
  const ext = [0, 1, 2].map((k) => b.max[k]! - b.min[k]!);
  let thin: Axis = 0;
  for (const k of [1, 2] as const) if (ext[k]! < ext[thin]!) thin = k;
  const lo = b.min[thin]!;
  const hi = b.max[thin]!;
  const onLo = new Uint8Array(nCells);
  const onHi = new Uint8Array(nCells);
  const p = s.points;
  const t = s.triangles;
  for (let i = 0; i < s.cells.length; i++) {
    const c = s.cells[i]!;
    if (c >= nCells) continue;
    const a = p[3 * t[3 * i]! + thin]!;
    const bb = p[3 * t[3 * i + 1]! + thin]!;
    const cc = p[3 * t[3 * i + 2]! + thin]!;
    if (a === lo && bb === lo && cc === lo) onLo[c] = 1;
    if (a === hi && bb === hi && cc === hi) onHi[c] = 1;
  }
  let twoD = nCells > 0 && hi > lo;
  for (let c = 0; twoD && c < nCells; c++) if (!onLo[c] || !onHi[c]) twoD = false;
  return { twoD, thin, min: b.min, max: b.max };
}

export type Availability = { readonly ok: true } | { readonly ok: false; readonly reason: string };

export interface DerivedContext {
  /** Null while the surface is not read yet. */
  readonly layers: Layers | null;
  readonly fields: readonly FieldInfo[];
  readonly nCells: number;
}

const ISO_REASON =
  'The engine has no entry point that makes an iso-surface of a stored field (rcfd-post cuts by functions of position only), so the Studio has no reference to check one against and does not draw it.';
const STREAM_REASON =
  'The engine has no streamline integrator, so the Studio has no reference to check streamlines against and does not draw them.';

/** The velocity components a vector picture needs: the in-plane two on a 2D case, all three otherwise. */
export function vectorComponents(layers: Layers): readonly Axis[] {
  return layers.twoD ? ([0, 1, 2] as const).filter((k) => k !== layers.thin) : [0, 1, 2];
}

/** The stored field that holds a velocity component (the run path's names). */
export const COMPONENT_FIELD = ['ux', 'uy', 'uz'] as const;

/** Whether a derived picture applies to this store, and if not, why in words (G6). */
export function availability(kind: DerivedKind, ctx: DerivedContext): Availability {
  const { layers } = ctx;
  switch (kind) {
    case 'iso':
      return { ok: false, reason: layers?.twoD ? `A 2D case (one cell layer) has no volume for an iso-surface. ${ISO_REASON}` : ISO_REASON };
    case 'streamlines':
      return { ok: false, reason: STREAM_REASON };
    case 'cut':
      if (!layers) return { ok: false, reason: 'The surface is not read yet.' };
      if (layers.twoD)
        return {
          ok: false,
          reason: `This is a 2D case (one cell layer, thinnest along ${AXIS_NAME[layers.thin]}): it is drawn whole, and a cut plane would only hide part of it.`,
        };
      return { ok: true };
    case 'vectors': {
      if (!layers) return { ok: false, reason: 'The surface is not read yet.' };
      const need = vectorComponents(layers).map((k) => COMPONENT_FIELD[k]);
      const missing = need.filter((n) => !ctx.fields.some((f) => f.name === n && f.location === 'cell' && f.n === ctx.nCells));
      if (missing.length > 0)
        return {
          ok: false,
          reason: `The store holds no cell field ${missing.join(' or ')}; ${layers.twoD ? 'in-plane arrows of a 2D case' : 'arrows of a 3D case'} need ${need.join(', ')} (output.store_fields).`,
        };
      return { ok: true };
    }
  }
}

// ---- T2, the cut plane ------------------------------------------------------------------------------------------

export interface Cut {
  readonly axis: Axis;
  /** The plane's coordinate, an `f32` (the shader's uniform holds it exactly). */
  readonly at: number;
  /** Which side is kept: `below` is rcfd-render's `n = e_a`, `above` is `n = -e_a`. */
  readonly keep: 'below' | 'above';
}

/** rcfd-render's `(n, d)` for a cut: only points with `n . p <= d` are drawn. */
export function clipPlane(cut: Cut): { n: [number, number, number]; d: number } {
  const s = cut.keep === 'below' ? 1 : -1;
  const n: [number, number, number] = [0, 0, 0];
  n[cut.axis] = s;
  const at = Math.fround(cut.at);
  return { n, d: s === 1 ? at : -at };
}

/** T2: whether rcfd-render's rule keeps a point (exact: `n` is an axis, so `n . p` is `p[a]` or `-p[a]`). */
export function keeps(cut: Cut | null, p: ArrayLike<number>): boolean {
  if (!cut) return true;
  const { n, d } = clipPlane(cut);
  return n[0] * p[0]! + n[1] * p[1]! + n[2] * p[2]! <= d;
}

/** A cut through the middle of the bounding box along an axis, keeping the side below. */
export function middleCut(layers: Layers, axis: Axis): Cut {
  return { axis, at: Math.fround(0.5 * (layers.min[axis] + layers.max[axis])), keep: 'below' };
}

// ---- T3, vectors ------------------------------------------------------------------------------------------------

/** `render.png`'s lattice: `overlay_vectors(.., 24, ..)` in crates/rcfd-api/src/service.rs. */
export const VECTOR_DENSITY = 24;
/** The arrow of the largest vector, in lattice steps (`fr.w / density * 0.8` in overlay_vectors). */
export const VECTOR_ARROW = 0.8;
/** The head: back along the arrow (`0.3 * s`), and out to each side by this part of that (`* 0.5`). */
export const VECTOR_HEAD_BACK = 0.3;
export const VECTOR_HEAD_SIDE = 0.5;

export interface Glyphs {
  /** The surface triangle each arrow stands on, and its owner cell. */
  readonly triangles: Uint32Array<ArrayBuffer>;
  readonly cells: Uint32Array<ArrayBuffer>;
  /** Three strokes per arrow (shaft, two head strokes), 6 floats each: 18 per arrow, the GPU's line segments. */
  readonly segments: Float32Array<ArrayBuffer>;
  /** The largest vector length over every cell (the arrows' scale). */
  readonly vmax: number;
  /** One lattice step, in mesh coordinates. */
  readonly step: number;
}

/**
 * T3: the arrows. `components[k]` is the stored cell field of velocity component k (ux, uy, uz), or null for a component
 * a 2D case does not draw.
 */
export function vectorGlyphs(s: SurfaceArrays, components: readonly (Float32Array | null)[], layers: Layers, nCells: number): Glyphs {
  const use = vectorComponents(layers);
  const comp = (c: number, k: number): number => (use.includes(k as Axis) ? components[k]![c]! : 0);
  const mag = (c: number): number => Math.hypot(comp(c, 0), comp(c, 1), comp(c, 2));
  let vmax = 0;
  for (let c = 0; c < nCells; c++) {
    const m = mag(c);
    if (m > vmax) vmax = m;
  }
  const lo = layers.min;
  const hi = layers.max;
  const big = Math.max(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]);
  const step = big / VECTOR_DENSITY;
  const longest = VECTOR_ARROW * step;
  const p = s.points;
  const t = s.triangles;
  const front = FRONT_SIGN[layers.thin] > 0 ? hi[layers.thin] : lo[layers.thin];
  const taken = new Set<number>();
  const tris: number[] = [];
  const cells: number[] = [];
  const segs: number[] = [];
  const corner = (i: number, k: number): number => p[3 * t[i]! + k]!;
  for (let i = 0; i < s.cells.length; i++) {
    const c = s.cells[i]!;
    if (c >= nCells) continue;
    if (layers.twoD) {
      const a = layers.thin;
      if (corner(3 * i, a) !== front || corner(3 * i + 1, a) !== front || corner(3 * i + 2, a) !== front) continue;
    }
    const centre = [0, 1, 2].map((k) => (corner(3 * i, k) + corner(3 * i + 1, k) + corner(3 * i + 2, k)) / 3);
    // the lattice bin, as overlay_vectors: (x - lo) / extent * density, truncated; an empty extent is bin 0
    let key = 0;
    for (let k = 2; k >= 0; k--) {
      const bin = Math.floor(((centre[k]! - lo[k]!) / Math.max(hi[k]! - lo[k]!, 1e-300)) * VECTOR_DENSITY);
      key = key * (VECTOR_DENSITY + 1) + Math.max(0, bin);
    }
    if (taken.has(key)) continue;
    taken.add(key);
    const v = [comp(c, 0), comp(c, 1), comp(c, 2)];
    const m = Math.hypot(v[0]!, v[1]!, v[2]!);
    if (!(m > 0) || !Number.isFinite(m) || !(vmax > 0)) continue;
    const d = v.map((x) => x / m);
    const len = (longest * m) / vmax;
    const tip = centre.map((x, k) => x + len * d[k]!);
    // the head's side: the triangle's normal crossed with the arrow (in the face), else any perpendicular
    const e1 = [0, 1, 2].map((k) => corner(3 * i + 1, k) - corner(3 * i, k));
    const e2 = [0, 1, 2].map((k) => corner(3 * i + 2, k) - corner(3 * i, k));
    const nrm = cross(e1, e2);
    let side = cross(nrm, d);
    let sl = Math.hypot(side[0]!, side[1]!, side[2]!);
    if (!(sl > 1e-9 * Math.hypot(nrm[0]!, nrm[1]!, nrm[2]!))) {
      const ax = [0, 1, 2].reduce((best, k) => (Math.abs(d[k]!) < Math.abs(d[best]!) ? k : best), 0);
      const e = [0, 0, 0];
      e[ax] = 1;
      side = cross(d, e);
      sl = Math.hypot(side[0]!, side[1]!, side[2]!);
    }
    side = side.map((x) => x / sl);
    const back = VECTOR_HEAD_BACK * len;
    const out = VECTOR_HEAD_SIDE * back;
    const h1 = tip.map((x, k) => x - back * d[k]! + out * side[k]!);
    const h2 = tip.map((x, k) => x - back * d[k]! - out * side[k]!);
    segs.push(...centre, ...tip, ...tip, ...h1, ...tip, ...h2);
    tris.push(i);
    cells.push(c);
  }
  return { triangles: Uint32Array.from(tris), cells: Uint32Array.from(cells), segments: Float32Array.from(segs), vmax, step };
}

function cross(a: readonly number[], b: readonly number[]): number[] {
  return [a[1]! * b[2]! - a[2]! * b[1]!, a[2]! * b[0]! - a[0]! * b[2]!, a[0]! * b[1]! - a[1]! * b[0]!];
}

/** The glyphs of the derived-picture controls, on the 14-unit pictogram grid (DESIGN.md Shapes; 0, 45, 90 degrees). */
export const DERIVED_ICON: Record<DerivedKind, string> = {
  // a box with a plane through its front face
  cut: 'M2 5H8V12H2ZM2 5L5 2H11V9L8 12M8 5L11 2M5 5V12',
  // two arrows
  vectors: 'M2 10L7 5M4 5H7V8M7 12L12 7M9 7H12V10',
  // a closed level outline with an inner one
  iso: 'M4 2H10L12 4V10L10 12H4L2 10V4ZM6 5H8L9 6V8L8 9H6L5 8V6Z',
  // three stepped flow lines with heads
  streamlines: 'M1 4H5L7 2H12M1 8H6L8 6H12M1 12H4L6 10H12M11 1L12 2L11 3',
};
