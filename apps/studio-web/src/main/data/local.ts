// Route R2 in the main process (F-804): local requests go, one at a time, to the reader's worker thread (worker.ts);
// this side only checks the path against the data roots (asynchronously) and passes messages, so the main process's
// thread never reads a store (G21). A worker that fails is replaced; the task it held fails with `internal`.
import { Worker } from 'node:worker_threads';
import type { DataWireResult, StoreInfo } from '../../shared/data.js';
import { DataFailure, SerialQueue, type Job } from './queue.js';
import type { DataRoots } from './roots.js';
import type { WorkerReply, WorkerTask } from './worker.js';

type TaskBody = WorkerTask extends infer T ? (T extends { id: number } ? Omit<T, 'id'> : never) : never;

/** The compiled worker beside this file (dist/main/data/worker.js). */
export const WORKER_URL = new URL('./worker.js', import.meta.url);

export class LocalData {
  private worker: Worker | null = null;
  private nextId = 1;
  private readonly waiting = new Map<number, { resolve: (r: WorkerReply) => void; reject: (e: unknown) => void }>();
  private readonly queue = new SerialQueue();
  private closed = false;

  constructor(
    readonly roots: DataRoots,
    private readonly workerUrl: URL = WORKER_URL,
  ) {}

  open(p: string): Job<DataWireResult> {
    return this.job(p, (dir) => ({ op: 'open', dir, path: p }));
  }

  field(p: string, snapshot: number, field: string): Job<DataWireResult> {
    return this.job(p, (dir) => ({ op: 'field', dir, snapshot, field }));
  }

  surface(p: string): Job<DataWireResult> {
    return this.job(p, (dir) => ({ op: 'surface', dir }));
  }

  /** F-836: the surface with the patch of each triangle. */
  patchSurface(p: string): Job<DataWireResult> {
    return this.job(p, (dir) => ({ op: 'patchSurface', dir }));
  }

  /** F-819b: field `field` of snapshot `snapshot` on the store's surface, one value per triangle, gathered in the worker. */
  surfaceField(p: string, snapshot: number, field: string): Job<DataWireResult> {
    return this.job(p, (dir) => ({ op: 'surfaceField', dir, snapshot, field }));
  }

  /** F-815: `other`'s field minus `p`'s, in f64, on the same mesh; both folders are checked against the data roots. */
  difference(p: string, snapshot: number, other: string, otherSnapshot: number, field: string): Job<DataWireResult> {
    return this.queue.push(async () => {
      const a = await this.roots.resolve(p);
      if (!a.ok) throw new DataFailure('not_allowed', a.message);
      const b = await this.roots.resolve(other);
      if (!b.ok) throw new DataFailure('not_allowed', b.message);
      const reply = await this.send({ op: 'difference', dir: a.dir, snapshot, otherDir: b.dir, otherSnapshot, field });
      if (!reply.ok) throw new DataFailure(reply.store ? 'store' : 'internal', reply.message);
      if (reply.result.kind !== 'difference') throw new DataFailure('internal', 'unexpected reply of the reader');
      return reply.result;
    });
  }

  /** Test hook (mutation M3 of ADR 0350): the worker's unshuffle uses the wrong stride. Never called by the Studio. */
  setWrongStrideForTest(on: boolean): Promise<unknown> {
    return this.queue.push(() => this.send({ op: 'hooks', wrongStride: on })).promise;
  }

  /** The worker's thread id, or null before the first task (for the tests: the reads are not on the main thread). */
  get workerThreadId(): number | null {
    return this.worker?.threadId ?? null;
  }

  async close(): Promise<void> {
    this.closed = true;
    const w = this.worker;
    this.worker = null;
    if (w) await w.terminate();
  }

  private job(p: string, body: (dir: string) => TaskBody): Job<DataWireResult> {
    return this.queue.push(async () => {
      const r = await this.roots.resolve(p);
      if (!r.ok) throw new DataFailure('not_allowed', r.message);
      const reply = await this.send(body(r.dir));
      if (!reply.ok) throw new DataFailure(reply.store ? 'store' : 'internal', reply.message);
      const res = reply.result;
      if (res.kind === 'hooks') throw new DataFailure('internal', 'unexpected reply of the reader');
      if (res.kind === 'open') {
        // the page named the store by `p`; the worker read its real path
        const info: StoreInfo = { ...res.info, path: p };
        return { kind: 'open', info };
      }
      return res;
    });
  }

  private ensureWorker(): Worker {
    if (this.closed) throw new DataFailure('internal', 'the local reader is closed');
    if (this.worker) return this.worker;
    const w = new Worker(this.workerUrl, { name: 'rcfd-studio-store-reader' });
    w.unref();
    w.on('message', (m: WorkerReply) => {
      const p = this.waiting.get(m.id);
      if (!p) return;
      this.waiting.delete(m.id);
      p.resolve(m);
    });
    const fail = (why: string): void => {
      if (this.worker === w) this.worker = null;
      for (const [id, p] of this.waiting) {
        this.waiting.delete(id);
        p.reject(new DataFailure('internal', `the store reader's worker ${why}`));
      }
    };
    w.on('error', (e) => fail(`failed: ${e.message}`));
    w.on('exit', (code) => fail(`ended (code ${code})`));
    this.worker = w;
    return w;
  }

  private send(body: TaskBody): Promise<WorkerReply> {
    const w = this.ensureWorker();
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      w.postMessage({ ...body, id } as WorkerTask);
    });
  }
}
