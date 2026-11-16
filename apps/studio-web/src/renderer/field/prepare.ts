// The client of ./prepare.worker.ts (F-832, G21): the field viewport asks it for the arrays the GPU draws, so no loop
// over the surface's triangles or the field's cells runs on the renderer's main thread. One worker per viewport, ended
// with it. Copies of the surface go to the worker (the page keeps its own for the pick and the mesh grid); a field's
// values go and come back transferred. Where this window has no worker the same functions run here instead
// (`onMainThread` says so; the Studio's windows have workers).
import type { SurfaceData } from '../../shared/data.js';
import { bounds, cornerPositions, cornerScalars, shownRange } from './surface.js';
import type { PrepReply, PrepRequest } from './prepare.worker.js';

export interface PreparedSurface {
  /** The surface it was made from. */
  readonly surface: SurfaceData;
  readonly id: number;
  /** 9 floats per triangle (./surface.ts cornerPositions). */
  readonly positions: Float32Array<ArrayBuffer>;
  readonly min: readonly [number, number, number];
  readonly max: readonly [number, number, number];
  readonly centre: readonly [number, number, number];
  readonly radius: number;
}

export interface PreparedField {
  /** 3 per triangle: each corner's owner cell value, unchanged (./surface.ts cornerScalars). */
  readonly scalars: Float32Array<ArrayBuffer>;
  /** The values as given (back from the worker). */
  readonly values: Float32Array<ArrayBuffer>;
  /** Min and max of the shown values (./surface.ts shownRange); null when the surface has no triangle. */
  readonly range: { lo: number; hi: number } | null;
}

type Waiter = { resolve: (r: PrepReply) => void; reject: (e: Error) => void };

let prepIds = 0;

export class FieldPrep {
  private worker: Worker | null = null;
  private failed = false;
  private readonly waiting = new Map<number, Waiter>();
  private disposed = false;

  /** True when this window could not start the worker and the work runs on the main thread. */
  get onMainThread(): boolean {
    return this.failed;
  }

  private ensure(): Worker | null {
    if (this.failed || this.disposed) return null;
    if (this.worker) return this.worker;
    try {
      const w = new Worker(new URL('./prepare.worker.ts', import.meta.url), { type: 'module', name: 'rcfd-studio-field-prep' });
      w.onmessage = (e: MessageEvent<PrepReply>) => {
        const p = this.waiting.get(e.data.id);
        if (!p) return;
        this.waiting.delete(e.data.id);
        p.resolve(e.data);
      };
      w.onerror = (e) => {
        e.preventDefault();
        this.fail(`the field worker failed: ${e.message}`);
      };
      this.worker = w;
      return w;
    } catch {
      this.failed = true;
      return null;
    }
  }

  private fail(why: string): void {
    this.worker?.terminate();
    this.worker = null;
    for (const [id, p] of this.waiting) {
      this.waiting.delete(id);
      p.reject(new Error(why));
    }
  }

  private ask(w: Worker, r: PrepRequest, transfer: Transferable[]): Promise<PrepReply> {
    return new Promise((resolve, reject) => {
      this.waiting.set(r.id, { resolve, reject });
      w.postMessage(r, transfer);
    });
  }

  async surface(surface: SurfaceData): Promise<PreparedSurface> {
    const id = ++prepIds;
    const w = this.ensure();
    if (!w) {
      const b = bounds(surface.points);
      const positions = cornerPositions(surface);
      const centre: [number, number, number] = [(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2];
      let r2 = 0;
      for (let i = 0; i + 2 < positions.length; i += 3) r2 = Math.max(r2, (positions[i]! - centre[0]) ** 2 + (positions[i + 1]! - centre[1]) ** 2 + (positions[i + 2]! - centre[2]) ** 2);
      return { surface, id, positions, min: b.min, max: b.max, centre, radius: Math.sqrt(r2) };
    }
    const points = surface.points.slice();
    const triangles = surface.triangles.slice();
    const cells = surface.cells.slice();
    const r = await this.ask(w, { op: 'surface', id, points, triangles, cells }, [points.buffer, triangles.buffer, cells.buffer]);
    if (r.op === 'error') throw new Error(r.message);
    if (r.op !== 'surface') throw new Error('unexpected reply of the field worker');
    return { surface, id, positions: r.positions, min: r.min, max: r.max, centre: r.centre, radius: r.radius };
  }

  /** The arrays of one field on `prepared`'s surface. `values` is handed over (transferred) and comes back in the result. */
  async field(prepared: PreparedSurface, values: Float32Array<ArrayBuffer>): Promise<PreparedField> {
    const w = this.ensure();
    if (!w) return { scalars: cornerScalars(prepared.surface.cells, values), values, range: shownRange(prepared.surface.cells, values) };
    const id = ++prepIds;
    const own = values.byteOffset === 0 && values.byteLength === values.buffer.byteLength ? values : values.slice();
    const r = await this.ask(w, { op: 'field', id, surface: prepared.id, values: own }, [own.buffer]);
    if (r.op === 'error') throw new Error(r.message);
    if (r.op !== 'field') throw new Error('unexpected reply of the field worker');
    return { scalars: r.scalars, values: r.values, range: r.range };
  }

  dispose(): void {
    this.disposed = true;
    this.fail('the field view closed');
  }
}
