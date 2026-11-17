// The two bulk data routes behind the data port (F-804; ADR 0350 addendum 1, QA2 (a)): `local` reads `store.rts` of a
// project on this machine itself (R2, local.ts), `service` asks the connected service's `data.*` methods (R1,
// remote.ts). The page chooses the route per store; nothing falls back from one to the other silently.
import type { DataRequest, DataWireResult } from '../../shared/data.js';
import type { DataBackend } from './bridge.js';
import { LocalData, WORKER_URL } from './local.js';
import { DataFailure, type Job } from './queue.js';
import { RemoteData, type RemoteService } from './remote.js';
import type { DataRoots } from './roots.js';
import { DataFailure as Failure } from './queue.js';
import { testDelayMs } from '../work/delay.js';

/**
 * F-832: the tests' delay before a request reaches its route (none outside them). A cancel during it sends nothing to
 * the reader or the service; the job rejects at once, as SerialQueue's do.
 */
function delayed(ms: number, make: () => Job<DataWireResult>): Job<DataWireResult> {
  let inner: Job<DataWireResult> | null = null;
  let stop: ((e: DataFailure) => void) | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const promise = new Promise<DataWireResult>((resolve, reject) => {
    stop = reject;
    timer = setTimeout(() => {
      stop = null;
      inner = make();
      inner.promise.then(resolve, reject);
    }, ms);
  });
  return {
    promise,
    cancel: (reason = new Failure('cancelled', 'cancelled')) => {
      if (inner) return inner.cancel(reason);
      clearTimeout(timer);
      stop?.(reason);
      stop = null;
    },
  };
}

/** A job that fails at once with `bad_request` (F-815: a difference the route cannot give). */
function failed(message: string): Job<DataWireResult> {
  const promise = Promise.reject(new DataFailure('bad_request', message));
  return { promise, cancel: () => undefined };
}

export class StudioData implements DataBackend {
  readonly local: LocalData;
  readonly remote: RemoteData;

  constructor(service: RemoteService, roots: DataRoots, workerUrl: URL = WORKER_URL) {
    this.local = new LocalData(roots, workerUrl);
    this.remote = new RemoteData(service);
  }

  run(r: Exclude<DataRequest, { op: 'cancel' }>, timeoutMs: number): Job<DataWireResult> {
    const ms = testDelayMs('data');
    return ms > 0 ? delayed(ms, () => this.route(r, timeoutMs)) : this.route(r, timeoutMs);
  }

  private route(r: Exclude<DataRequest, { op: 'cancel' }>, timeoutMs: number): Job<DataWireResult> {
    const p = r.store.path;
    if (r.store.route === 'local') {
      switch (r.op) {
        case 'open':
          return this.local.open(p);
        case 'field':
          return this.local.field(p, r.snapshot, r.field);
        case 'surface':
          return this.local.surface(p);
        case 'surfaceField':
          return this.local.surfaceField(p, r.snapshot, r.field);
        case 'patchSurface':
          return this.local.patchSurface(p);
        case 'difference':
          if (r.other.route !== 'local') return failed('difference: both stores must be on the same route (local)');
          return this.local.difference(p, r.snapshot, r.other.path, r.otherSnapshot, r.field);
      }
    }
    switch (r.op) {
      case 'open':
        return this.remote.open(p, timeoutMs);
      case 'field':
        return this.remote.field(p, r.snapshot, r.field, timeoutMs);
      case 'surface':
        return this.remote.surface(p, timeoutMs);
      case 'surfaceField':
        // F-819b: the service's data.* methods gather nothing; the page reads the whole field there (data.field)
        return failed('surfaceField: the service route has no surface values; read the whole field (data.field)');
      case 'patchSurface':
        // F-836: the mesh preview's store is a local file the engine wrote into the project
        return failed('patchSurface: the service route has no patch surface; the mesh preview reads its store locally');
      case 'difference':
        // F-815: the service's data.* methods have no difference (ADR 0801, addendum F-815)
        return failed('difference: the service route has no difference of two stores; it is computed by the local reader only');
    }
  }

  async close(): Promise<void> {
    this.remote.close();
    await this.local.close();
  }
}
