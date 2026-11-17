// The arrays the field viewport draws, made from F-804's surface and field (F-808). Pure: typed arrays in, typed arrays
// out, no three.js and no DOM, so the unit tests import it and the mesh-grid worker runs it.
//
// The surface is `data.surface`'s: the boundary faces of the stored mesh cut into triangles (a fan per face), the points
// they use, and the owner cell of each triangle. A cell field colours each triangle with its owner cell's value, as
// rcfd-render's per-face colouring and rcfd-studio's viewport do: what is drawn is exactly the stored cell value.
import type { SurfaceData } from '../../shared/data.js';

/** A surface's arrays, of any buffer (the worker receives copies). */
export interface SurfaceArrays {
  readonly points: Float32Array;
  readonly triangles: Uint32Array;
  readonly cells: Uint32Array;
}

/** The triangles without an index: 9 floats (three corners) per triangle, as the GPU draws them. */
export function cornerPositions(s: Pick<SurfaceArrays, 'points' | 'triangles'>): Float32Array<ArrayBuffer> {
  const t = s.triangles;
  const p = s.points;
  const out = new Float32Array(3 * t.length);
  for (let c = 0; c < t.length; c++) {
    const i = 3 * t[c]!;
    out[3 * c] = p[i]!;
    out[3 * c + 1] = p[i + 1]!;
    out[3 * c + 2] = p[i + 2]!;
  }
  return out;
}

/**
 * The raw scalar at each corner (3 per triangle): the owner cell's value, unchanged (the shader maps it to a colour). The
 * values are written into `into` when it has the right length (scrubbing reuses the buffer).
 */
export function cornerScalars(cells: Uint32Array, values: Float32Array, into?: Float32Array<ArrayBuffer>): Float32Array<ArrayBuffer> {
  const out = into && into.length === 3 * cells.length ? into : new Float32Array(3 * cells.length);
  for (let t = 0; t < cells.length; t++) {
    const v = values[cells[t]!]!;
    out[3 * t] = v;
    out[3 * t + 1] = v;
    out[3 * t + 2] = v;
  }
  return out;
}

/** Min and max of the shown values (the cells that own a surface triangle), as rcfd-studio's snapshot range. */
export function shownRange(cells: Uint32Array, values: Float32Array): { lo: number; hi: number } | null {
  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (let t = 0; t < cells.length; t++) {
    const v = values[cells[t]!]!;
    if (v < lo) lo = v;
    if (v > hi) hi = v;
  }
  return lo <= hi ? { lo, hi } : null;
}

/**
 * F-819b: `cornerScalars` of surface values (one per triangle, `values[t]` the owner cell's value, as
 * src/shared/surface-values.ts gathers them): the same arrays, from the same values in the same order.
 */
export function cornerScalarsOf(values: Float32Array, into?: Float32Array<ArrayBuffer>): Float32Array<ArrayBuffer> {
  const out = into && into.length === 3 * values.length ? into : new Float32Array(3 * values.length);
  for (let t = 0; t < values.length; t++) {
    const v = values[t]!;
    out[3 * t] = v;
    out[3 * t + 1] = v;
    out[3 * t + 2] = v;
  }
  return out;
}

/** F-819b: `shownRange` of surface values (one per triangle): the same comparisons over the same values in the same order. */
export function rangeOf(values: Float32Array): { lo: number; hi: number } | null {
  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (let t = 0; t < values.length; t++) {
    const v = values[t]!;
    if (v < lo) lo = v;
    if (v > hi) hi = v;
  }
  return lo <= hi ? { lo, hi } : null;
}

/** A surface's bounding box. */
export function bounds(points: Float32Array): { min: [number, number, number]; max: [number, number, number] } {
  const min: [number, number, number] = [Infinity, Infinity, Infinity];
  const max: [number, number, number] = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i + 2 < points.length; i += 3) {
    for (let k = 0; k < 3; k++) {
      const v = points[i + k]!;
      if (v < min[k]!) min[k] = v;
      if (v > max[k]!) max[k] = v;
    }
  }
  if (!(min[0] <= max[0])) return { min: [0, 0, 0], max: [0, 0, 0] };
  return { min, max };
}

function unitNormal(p: Float32Array, a: number, b: number, c: number): [number, number, number] {
  const ax = p[3 * a]!, ay = p[3 * a + 1]!, az = p[3 * a + 2]!;
  const ux = p[3 * b]! - ax, uy = p[3 * b + 1]! - ay, uz = p[3 * b + 2]! - az;
  const vx = p[3 * c]! - ax, vy = p[3 * c + 1]! - ay, vz = p[3 * c + 2]! - az;
  const nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
  const l = Math.hypot(nx, ny, nz) || 1;
  return [nx / l, ny / l, nz / l];
}

/**
 * The mesh grid: the edges of the stored mesh's boundary faces, as line segments (6 floats each). A triangle edge is a
 * face edge unless it is the diagonal of a face's fan: shared by exactly two triangles of the same owner cell that lie in
 * one plane. Edges of the boundary (one triangle) and edges where faces of different cells, or two faces of one cell at
 * an angle, meet are drawn.
 */
export function gridEdges(s: SurfaceArrays): Float32Array<ArrayBuffer> {
  const { points: p, triangles: tri, cells } = s;
  const nT = cells.length;
  const nP = p.length / 3;
  // edge key (lo * nP + hi) -> the first triangle that has it, or -1 once it is decided
  const first = new Map<number, number>();
  const keep: number[] = [];
  const normals = new Float32Array(3 * nT);
  for (let t = 0; t < nT; t++) {
    const n = unitNormal(p, tri[3 * t]!, tri[3 * t + 1]!, tri[3 * t + 2]!);
    normals.set(n, 3 * t);
  }
  const interior = (t: number, u: number) =>
    cells[t] === cells[u] && normals[3 * t]! * normals[3 * u]! + normals[3 * t + 1]! * normals[3 * u + 1]! + normals[3 * t + 2]! * normals[3 * u + 2]! > 1 - 1e-6;
  for (let t = 0; t < nT; t++) {
    for (let e = 0; e < 3; e++) {
      const a = tri[3 * t + e]!;
      const b = tri[3 * t + ((e + 1) % 3)]!;
      const key = a < b ? a * nP + b : b * nP + a;
      const seen = first.get(key);
      if (seen === undefined) first.set(key, t);
      else if (seen >= 0) {
        if (!interior(t, seen)) keep.push(key);
        first.set(key, -1);
      } else keep.push(key); // a third triangle on one edge (a non-manifold surface): draw it
    }
  }
  for (const [key, t] of first) if (t >= 0) keep.push(key);
  const out = new Float32Array(6 * keep.length);
  keep.forEach((key, i) => {
    const a = Math.floor(key / nP);
    const b = key - a * nP;
    out.set(p.subarray(3 * a, 3 * a + 3), 6 * i);
    out.set(p.subarray(3 * b, 3 * b + 3), 6 * i + 3);
  });
  return out;
}

/**
 * The P8 surface of ADR 0350 (measurement only, `#/dev/field?bench=p8`): a 1000 x 500 grid of quads in two triangles each,
 * 1e6 triangles, on a height field; each triangle's value is its height. Synthetic: never shown as a result.
 */
export function benchSoup(nx = 1000, ny = 500): { surface: SurfaceData; values: Float32Array<ArrayBuffer> } {
  const z = (x: number, y: number) => 0.25 * Math.sin(3 * Math.PI * x) * Math.cos(3 * Math.PI * y);
  const points = new Float32Array(3 * (nx + 1) * (ny + 1));
  for (let j = 0; j <= ny; j++) {
    for (let i = 0; i <= nx; i++) {
      const k = 3 * (j * (nx + 1) + i);
      points[k] = i / nx;
      points[k + 1] = j / ny;
      points[k + 2] = z(i / nx, j / ny);
    }
  }
  const triangles = new Uint32Array(6 * nx * ny);
  const cells = new Uint32Array(2 * nx * ny);
  const values = new Float32Array(2 * nx * ny);
  let t = 0;
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const a = j * (nx + 1) + i;
      const b = a + 1;
      const c = a + nx + 2;
      const d = a + nx + 1;
      triangles.set([a, b, c, a, c, d], 3 * t);
      for (let half = 0; half < 2; half++) {
        cells[t] = t;
        values[t] = z((i + 0.5) / nx, (j + 0.5) / ny);
        t++;
      }
    }
  }
  return { surface: { points, triangles, cells }, values };
}
