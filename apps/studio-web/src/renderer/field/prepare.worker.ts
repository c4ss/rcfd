// The field viewport's data work, off the renderer's main thread (F-832, G21): the arrays the GPU draws are made here
// from F-804's surface and field (./surface.ts, unchanged arithmetic), and only handed back, transferred without a
// copy. The worker keeps the owner cells of the surface it was given last, so a field (one value per cell) needs only
// its values.
import { bounds, cornerPositions, cornerScalars, shownRange } from './surface.js';

export type PrepRequest =
  | { readonly op: 'surface'; readonly id: number; readonly points: Float32Array; readonly triangles: Uint32Array; readonly cells: Uint32Array }
  | { readonly op: 'field'; readonly id: number; readonly surface: number; readonly values: Float32Array<ArrayBuffer> };

export type PrepReply =
  | {
      readonly op: 'surface';
      readonly id: number;
      readonly positions: Float32Array<ArrayBuffer>;
      readonly min: [number, number, number];
      readonly max: [number, number, number];
      readonly centre: [number, number, number];
      readonly radius: number;
    }
  | {
      readonly op: 'field';
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

let cells: Uint32Array | null = null;
let surfaceId = 0;

scope.onmessage = (e) => {
  const r = e.data;
  try {
    if (r.op === 'surface') {
      cells = r.cells;
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
      scope.postMessage({ op: 'surface', id: r.id, positions, min: b.min, max: b.max, centre, radius: Math.sqrt(r2) }, [positions.buffer]);
      return;
    }
    if (!cells || r.surface !== surfaceId) {
      scope.postMessage({ op: 'error', id: r.id, message: 'the field came for a surface this worker does not hold' }, []);
      return;
    }
    const range = shownRange(cells, r.values);
    const scalars = cornerScalars(cells, r.values);
    scope.postMessage({ op: 'field', id: r.id, scalars, values: r.values, range }, [scalars.buffer, r.values.buffer]);
  } catch (err) {
    scope.postMessage({ op: 'error', id: r.id, message: err instanceof Error ? err.message : String(err) }, []);
  }
};
