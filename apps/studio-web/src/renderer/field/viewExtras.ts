// F-837: the field view's extras from the removed egui Studio, as plain functions (tests/unit/f837.spec.ts):
//
// - the colour range modes: the shown snapshot's values (F-808's range, the default), the whole run's (the min and max
//   of the shown values over every stored snapshot, rcfd-studio's run range), or a fixed range the user types;
// - the domain's size, from the stored surface's bounding box (for a 2D case its two in-plane extents);
// - the scale bar of a face-on 2D view: the longest 1, 2 or 5 times a power of ten that is at most a quarter of the
//   view's width (the egui Studio's rule).
//
// ADR 0738 P5 (E-809e, F-837): the lengths are the stored mesh's coordinates, whose unit the engine names
// (`length_unit`: data.open on the service route, the reply of every derived picture). store.rts does not write it
// (Q5), so a local store before its first derived picture shows the engine's documented store unit, held to the
// engine's replies by tests/unit/derive-bridge.spec.ts. Each length says where its unit comes from (G6).
import type { Layers } from './derived.js';

export type RangeMode = 'snapshot' | 'run' | 'fixed';
export const RANGE_MODES: readonly RangeMode[] = ['snapshot', 'run', 'fixed'];

/**
 * The engine's documented unit of a stored mesh's coordinates (`rcfd_tsstore::LENGTH_UNIT`: the solver's mesh after
 * mesh.scale and [mesh.transform], in metres), for a store no engine reply has named one for yet.
 */
export const STORE_LENGTH_UNIT = 'm';

/** The unit of a store's lengths and where it comes from. */
export interface LengthUnit {
  readonly unit: string;
  readonly from: 'data.open' | 'derive' | 'documented';
  /** Said in the tooltips of the lengths (G6). */
  readonly why: string;
}

/**
 * The unit of the stored mesh's lengths: the one `data.open` named (the service route), else the one the engine's
 * derived picture of this store named (`length_unit` of its reply), else the engine's documented store unit.
 */
export function lengthUnitOf(opened: string | null, derived: string | null): LengthUnit {
  if (opened) return { unit: opened, from: 'data.open', why: `Lengths in ${opened}, the unit the engine names for the stored mesh (data.open, length_unit).` };
  if (derived) return { unit: derived, from: 'derive', why: `Lengths in ${derived}, the unit the engine names for the stored mesh (its derived picture, length_unit).` };
  return {
    unit: STORE_LENGTH_UNIT,
    from: 'documented',
    why: `Lengths in ${STORE_LENGTH_UNIT}: the engine stores the mesh in metres, after mesh.scale and [mesh.transform] (ADR 0738); store.rts does not write the unit.`,
  };
}

/** Axis names. */
export const AXIS = ['x', 'y', 'z'] as const;

/** A length in at most four significant figures, without trailing zeros. */
export function lengthText(v: number): string {
  if (!Number.isFinite(v)) return String(v);
  return String(Number(v.toPrecision(4)));
}

/** The domain's extents, for a 2D case only the two in-plane axes (the egui Studio's "domain 2 x 1 m"). */
export function domainExtents(l: Layers): { axes: readonly number[]; extents: readonly number[] } {
  const axes = l.twoD ? [0, 1, 2].filter((k) => k !== l.thin) : [0, 1, 2];
  return { axes, extents: axes.map((k) => l.max[k]! - l.min[k]!) };
}

export function domainText(l: Layers): string {
  return domainExtents(l).extents.map(lengthText).join(' × ');
}

/** The scale bar: the longest of 1, 2, 5 times 10^k at most a quarter of the width, and its length in pixels. */
export function scaleBar(perPixel: number, widthPx: number): { length: number; px: number } | null {
  if (!(perPixel > 0) || !(widthPx > 0) || !Number.isFinite(perPixel)) return null;
  const most = (widthPx / 4) * perPixel;
  let k = Math.floor(Math.log10(most));
  let best = 0;
  for (let tries = 0; tries < 3 && best === 0; tries++, k--) {
    for (const m of [5, 2, 1]) {
      const v = m * 10 ** k;
      if (v <= most * (1 + 1e-12)) {
        best = v;
        break;
      }
    }
  }
  if (best === 0) return null;
  // a clean decimal (5e-3, not 0.005000000000000001)
  const length = Number(best.toPrecision(1));
  return { length, px: length / perPixel };
}

/** The range of a fixed mode, or why the typed numbers are not one. */
export function fixedRange(lo: number, hi: number): { lo: number; hi: number } | { problem: string } {
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) return { problem: 'Type both ends of the range.' };
  if (!(hi > lo)) return { problem: 'The top of the range must be above its bottom.' };
  return { lo, hi };
}

/** How many of the surface's cells (each counted once) hold a value outside [lo, hi] (drawn in the end colours). */
export function cellsOutside(cells: Uint32Array, values: Float32Array, lo: number, hi: number): { outside: number; of: number } {
  const seen = new Set<number>();
  let outside = 0;
  for (let t = 0; t < cells.length; t++) {
    const c = cells[t]!;
    if (seen.has(c)) continue;
    seen.add(c);
    const v = values[c]!;
    if (v < lo || v > hi) outside++;
  }
  return { outside, of: seen.size };
}

/** F-819b: `cellsOutside` of surface values (one per triangle, `values[t]` the value of `cells[t]`). */
export function cellsOutsideOf(cells: Uint32Array, values: Float32Array, lo: number, hi: number): { outside: number; of: number } {
  const seen = new Set<number>();
  let outside = 0;
  for (let t = 0; t < cells.length; t++) {
    const c = cells[t]!;
    if (seen.has(c)) continue;
    seen.add(c);
    const v = values[t]!;
    if (v < lo || v > hi) outside++;
  }
  return { outside, of: seen.size };
}

/** The file name Save PNG proposes: `<run>-<field>-step<k>.png`, only letters, digits, `.`, `_` and `-`. */
export function pngName(run: string, field: string, step: number | undefined): string {
  const clean = (s: string) => s.replace(/[^A-Za-z0-9._-]+/g, '_').replace(/^[._]+/, '') || 'field';
  return `${clean(run)}-${clean(field)}${step === undefined ? '' : `-step${step}`}.png`.slice(-124);
}
