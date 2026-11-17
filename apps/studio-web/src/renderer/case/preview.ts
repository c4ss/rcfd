// The pure part of the case editor's mesh preview (F-836, ADR 0732; MeshPreview.tsx draws it): the reply of
// `mesh.preview`, the project's previews folder, the surface of the patches shown and the colour range of the patches.
// No React, no DOM: tests/unit/mesh-preview.spec.ts imports it directly.
import type { PatchSurfaceData, SurfaceData } from '../../shared/data.js';
import { colourRange, type ColourRange } from '../field/colormap.js';

/** What `mesh.preview` replies (docs/service-protocol.md). */
export interface PreviewReply {
  readonly path: string;
  readonly mesh_hash: string;
  readonly cells: number;
  readonly faces: number;
  readonly points: number;
  readonly patches: readonly { readonly name: string; readonly kind: string; readonly faces: number }[];
  readonly bounds: { readonly min: readonly number[]; readonly max: readonly number[] };
  readonly two_d: boolean;
  readonly adapt: null | { readonly pending: boolean; readonly cycles: number; readonly solves: number };
  readonly reused: boolean;
}

export const isReply = (v: unknown): v is PreviewReply => {
  const r = v as Partial<PreviewReply> | null;
  return !!r && typeof r.path === 'string' && typeof r.mesh_hash === 'string' && typeof r.cells === 'number' && Array.isArray(r.patches);
};

/** The previews folder of a project (ADR 0732 Q2 (a)), in the folder's own separator: the engine adds `<mesh hash>/`. */
export function previewsDir(folder: string): string {
  const sep = folder.includes('\\') && !folder.includes('/') ? '\\' : '/';
  return [folder.replace(/[\\/]+$/, ''), '.rcfd', 'previews'].join(sep);
}

/** The surface of the patches not hidden (by index), and the value (patch index) of each of its triangles. */
export function shownSurface(s: PatchSurfaceData, hidden: ReadonlySet<number>): { surface: SurfaceData; values: Float32Array<ArrayBuffer> } {
  const n = s.patch.length;
  let keep = 0;
  for (let t = 0; t < n; t++) if (!hidden.has(s.patch[t]!)) keep++;
  const triangles = new Uint32Array(3 * keep);
  const cells = new Uint32Array(keep);
  const values = new Float32Array(keep);
  let k = 0;
  for (let t = 0; t < n; t++) {
    const p = s.patch[t]!;
    if (hidden.has(p)) continue;
    triangles.set(s.triangles.subarray(3 * t, 3 * t + 3), 3 * k);
    cells[k] = s.cells[t]!;
    values[k] = p;
    k++;
  }
  return { surface: { points: s.points, triangles, cells }, values };
}

/** The colour range that gives each of `n` patches its own entry of the table. */
export const patchRange = (n: number): ColourRange => colourRange(0, Math.max(1, n - 1));

/** The axis along which the bounds are thinnest (a 2D mesh's one-cell slab). */
export function thinAxis(b: PreviewReply['bounds']): 0 | 1 | 2 {
  const d = [0, 1, 2].map((k) => (b.max[k] ?? 0) - (b.min[k] ?? 0));
  return d[2]! <= d[0]! && d[2]! <= d[1]! ? 2 : d[1]! <= d[0]! ? 1 : 0;
}
