// The two bulk data routes behind the data port (F-804; ADR 0350 addendum 1, QA2 (a)): `local` reads `store.rts` of a
// project on this machine itself (R2, local.ts), `service` asks the connected service's `data.*` methods (R1,
// remote.ts). The page chooses the route per store; nothing falls back from one to the other silently.
import type { DataRequest, DataWireResult } from '../../shared/data.js';
import type { DataBackend } from './bridge.js';
import { LocalData, WORKER_URL } from './local.js';
import type { Job } from './queue.js';
import { RemoteData, type RemoteService } from './remote.js';
import type { DataRoots } from './roots.js';

export class StudioData implements DataBackend {
  readonly local: LocalData;
  readonly remote: RemoteData;

  constructor(service: RemoteService, roots: DataRoots, workerUrl: URL = WORKER_URL) {
    this.local = new LocalData(roots, workerUrl);
    this.remote = new RemoteData(service);
  }

  run(r: Exclude<DataRequest, { op: 'cancel' }>, timeoutMs: number): Job<DataWireResult> {
    const p = r.store.path;
    if (r.store.route === 'local') {
      switch (r.op) {
        case 'open':
          return this.local.open(p);
        case 'field':
          return this.local.field(p, r.snapshot, r.field);
        case 'surface':
          return this.local.surface(p);
      }
    }
    switch (r.op) {
      case 'open':
        return this.remote.open(p, timeoutMs);
      case 'field':
        return this.remote.field(p, r.snapshot, r.field, timeoutMs);
      case 'surface':
        return this.remote.surface(p, timeoutMs);
    }
  }

  async close(): Promise<void> {
    this.remote.close();
    await this.local.close();
  }
}
