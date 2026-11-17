// The field viewport's data work, off the renderer's main thread (F-832, G21): the arrays the GPU draws are made here
// from F-804's surface and field (./surface.ts, unchanged arithmetic), and only handed back, transferred without a
// copy. The worker keeps the surface it was given last, so a field (one value per cell) needs only its values. F-809's
// derived pictures loop over the whole surface too: the mesh's layers (with the surface) and the vector arrows (./derived.ts,
// unchanged arithmetic) are made here as well.
//
// F-819b: a field can also come as surface values, one per triangle (`surfaceField`, gathered by the store reader where
// it read the whole field); its arrays are made with ./surface.ts cornerScalarsOf and rangeOf, the same values in the
// same order as cornerScalars and shownRange take them from the whole field.
import { meshLayers, vectorGlyphs, type Glyphs, type Layers } from './derived.js';
import { bounds, cornerPositions, cornerScalars, cornerScalarsOf, rangeOf, shownRange } from './surface.js';

export type PrepRequest =
  | {
      readonly op: 'surface';
      readonly id: number;
      readonly points: Float32Array;
      readonly triangles: Uint32Array;
      readonly cells: Uint32Array;
      /** The store's cell count (F-809's layers). */
      readonly nCells: number;
    }
  | { readonly op: 'field'; readonly id: number; readonly surface: number; readonly values: Float32Array<ArrayBuffer> }
  /** F-819b: one value per triangle of the surface held (src/shared/surface-values.ts). */
  | { readonly op: 'surfaceField'; readonly id: number; readonly surface: number; readonly values: Float32Array<ArrayBuffer> }
  | {
      readonly op: 'glyphs';
      readonly id: number;
      readonly surface: number;
      readonly components: readonly (Float32Array<ArrayBuffer> | null)[];
      readonly layers: Layers;
      readonly nCells: number;
    };

export type PrepReply =
  | {
      readonly op: 'surface';
      readonly id: number;
      readonly positions: Float32Array<ArrayBuffer>;
      readonly min: [number, number, number];
      readonly max: [number, number, number];
      readonly centre: [number, number, number];
      readonly radius: number;
      readonly layers: Layers;
    }
  | { readonly op: 'glyphs'; readonly id: number; readonly glyphs: Glyphs }
  | {
      readonly op: 'field' | 'surfaceField';
      readonly id: number;
      readonly scalars: Float32Array<ArrayBuffer>;
      readonly values: Float32Array<ArrayBuffer>;
      readonly range: { lo: number; hi: number } | null;
    }
  | { readonly op: 'error'; readonly id: number; readonly message: string };

const scope = self as unknown as {
  onmessage: ((e: MessageEvent<PrepRequest>) => void) | null;
  postMessage(message: PrepReply, transfer: Transferable[]): void;
};

let held: { points: Float32Array; triangles: Uint32Array; cells: Uint32Array } | null = null;
let surfaceId = 0;

scope.onmessage = (e) => {
  const r = e.data;
  try {
    if (r.op === 'surface') {
      held = { points: r.points, triangles: r.triangles, cells: r.cells };
      surfaceId = r.id;
      const positions = cornerPositions(r);
      const b = bounds(r.points);
      const centre: [number, number, number] = [(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2];
      // as three's computeBoundingSphere: the box's centre, the farthest drawn corner
      let r2 = 0;
      for (let i = 0; i + 2 < positions.length; i += 3) {
        const dx = positions[i]! - centre[0];
        const dy = positions[i + 1]! - centre[1];
        const dz = positions[i + 2]! - centre[2];
        const d = dx * dx + dy * dy + dz * dz;
        if (d > r2) r2 = d;
      }
      const layers = meshLayers(r, r.nCells);
      scope.postMessage({ op: 'surface', id: r.id, positions, min: b.min, max: b.max, centre, radius: Math.sqrt(r2), layers }, [positions.buffer]);
      return;
    }
    if (!held || r.surface !== surfaceId) {
      scope.postMessage({ op: 'error', id: r.id, message: `the ${r.op} came for a surface this worker does not hold` }, []);
      return;
    }
    if (r.op === 'glyphs') {
      const glyphs = vectorGlyphs(held, r.components, r.layers, r.nCells);
      scope.postMessage({ op: 'glyphs', id: r.id, glyphs }, [glyphs.triangles.buffer, glyphs.cells.buffer, glyphs.segments.buffer]);
      return;
    }
    if (r.op === 'surfaceField') {
      if (r.values.length !== held.cells.length) throw new Error(`${r.values.length} surface values for ${held.cells.length} triangles`);
      const range = rangeOf(r.values);
      const scalars = cornerScalarsOf(r.values);
      scope.postMessage({ op: 'surfaceField', id: r.id, scalars, values: r.values, range }, [scalars.buffer, r.values.buffer]);
      return;
    }
    const cells = held.cells;
    const range = shownRange(cells, r.values);
    const scalars = cornerScalars(cells, r.values);
    scope.postMessage({ op: 'field', id: r.id, scalars, values: r.values, range }, [scalars.buffer, r.values.buffer]);
  } catch (err) {
    scope.postMessage({ op: 'error', id: r.id, message: err instanceof Error ? err.message : String(err) }, []);
  }
};
