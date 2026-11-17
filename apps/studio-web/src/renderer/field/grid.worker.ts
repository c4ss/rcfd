// The mesh grid's edges, computed off the renderer's main thread (G21): the field viewport posts the surface, this
// answers the segments (src/renderer/field/surface.ts, gridEdges), transferred without a copy.
import { gridEdges } from './surface.js';

interface GridRequest {
  readonly id: number;
  readonly points: Float32Array;
  readonly triangles: Uint32Array;
  readonly cells: Uint32Array;
}

const scope = self as unknown as {
  onmessage: ((e: MessageEvent<GridRequest>) => void) | null;
  postMessage(message: unknown, transfer: Transferable[]): void;
};

scope.onmessage = (e) => {
  const { id, points, triangles, cells } = e.data;
  const segments = gridEdges({ points, triangles, cells });
  scope.postMessage({ id, segments }, [segments.buffer]);
};
