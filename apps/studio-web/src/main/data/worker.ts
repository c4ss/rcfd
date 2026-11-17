// The worker thread of route R2 (F-804): runs the synchronous reader of tsstore.ts so that no file read, checksum,
// inflate or triangulation runs on the main process's thread (G21). One task at a time, sent by local.ts; results go
// back with their ArrayBuffers transferred (zero copy between the threads).
//
// The worker keeps the stores it has opened (at most MAX_OPEN, least recently used closed first). Before each task it
// looks at the file again: a store that grew is refreshed (a scanned index picks up the snapshots a live run committed),
// a store that was replaced or changed otherwise is opened anew.
//
// F-819b: with a store it keeps the owner cells of its surface's triangles, the first time the surface is made, so a
// field asked for on the surface (`surfaceField`) is gathered here, where the whole field was read, and only the
// surface's values cross to the renderer. A store opened anew makes its surface anew.
import fs from 'node:fs';
import { parentPort } from 'node:worker_threads';
import type { FieldInfo, StoreInfo } from '../../shared/data.js';
import { Store, StoreError, STORE_FILE } from './tsstore.js';
import { difference, fieldsMatch, meshMatch } from '../../shared/compare.js';
import { gatherSurface } from '../../shared/surface-values.js';
import path from 'node:path';

/** main -> worker */
export type WorkerTask =
  | { readonly id: number; readonly op: 'open'; readonly dir: string; readonly path: string }
  | { readonly id: number; readonly op: 'field'; readonly dir: string; readonly snapshot: number; readonly field: string }
  | { readonly id: number; readonly op: 'surface'; readonly dir: string }
  | { readonly id: number; readonly op: 'surfaceField'; readonly dir: string; readonly snapshot: number; readonly field: string }
  | {
      readonly id: number;
      readonly op: 'difference';
      readonly dir: string;
      readonly snapshot: number;
      readonly otherDir: string;
      readonly otherSnapshot: number;
      readonly field: string;
    }
  | { readonly id: number; readonly op: 'hooks'; readonly wrongStride: boolean };

/** worker -> main */
export type WorkerReply =
  | { readonly id: number; readonly ok: true; readonly result: WorkerResult }
  | { readonly id: number; readonly ok: false; readonly store: boolean; readonly message: string };

export type WorkerResult =
  | { readonly kind: 'open'; readonly info: StoreInfo }
  | { readonly kind: 'field'; readonly snapshot: number; readonly field: string; readonly values: ArrayBuffer }
  | { readonly kind: 'surface'; readonly points: ArrayBuffer; readonly triangles: ArrayBuffer; readonly cells: ArrayBuffer }
  | { readonly kind: 'surfaceField'; readonly snapshot: number; readonly field: string; readonly meshHash: string; readonly values: ArrayBuffer }
  | { readonly kind: 'difference'; readonly snapshot: number; readonly otherSnapshot: number; readonly field: string; readonly values: ArrayBuffer }
  | { readonly kind: 'hooks' };

const MAX_OPEN = 16;

interface Open {
  readonly store: Store;
  readonly ino: number;
  readonly birth: number;
  /** F-819b: the owner cell of each triangle of this store's surface, once the surface was made. */
  cells: Uint32Array | null;
}

const open = new Map<string, Open>();
let wrongStride = false;

function closeOne(dir: string): void {
  const o = open.get(dir);
  open.delete(dir);
  try {
    o?.store.close();
  } catch {
    // already closed
  }
}

function storeFor(dir: string): Store {
  return openFor(dir).store;
}

function openFor(dir: string): Open {
  let st: fs.Stats;
  try {
    st = fs.statSync(path.join(dir, STORE_FILE));
  } catch (e) {
    closeOne(dir);
    throw new StoreError(`cannot open ${path.join(dir, STORE_FILE)}: ${(e as NodeJS.ErrnoException).code ?? (e as Error).message}`);
  }
  let o = open.get(dir);
  if (o && (o.ino !== st.ino || o.birth !== st.birthtimeMs || st.size < o.store.length)) {
    closeOne(dir);
    o = undefined;
  }
  if (o && st.size !== o.store.length) {
    if (o.store.indexSource.kind === 'scanned') {
      try {
        o.store.refresh();
      } catch {
        closeOne(dir);
        o = undefined;
      }
    } else {
      closeOne(dir);
      o = undefined;
    }
  }
  if (!o) {
    const store = new Store(dir);
    o = { store, ino: st.ino, birth: st.birthtimeMs, cells: null };
    while (open.size >= MAX_OPEN) closeOne(open.keys().next().value as string);
  } else open.delete(dir);
  open.set(dir, o); // most recently used last
  o.store.hooks = { wrongStride };
  return o;
}

/** F-819b: the surface's owner cells of an open store, kept (a copy: the surface's own arrays go to the renderer). */
function keepCells(o: Open, cells: ArrayBuffer): void {
  o.cells = new Uint32Array(cells.slice(0));
}

function info(s: Store, p: string): StoreInfo {
  const fields: FieldInfo[] =
    s.snapshots.length === 0
      ? []
      : s.fields(0).map((f) => ({ name: f.name, unit: f.unit, location: f.location, n: f.n, precision: f.precision, codec: f.codec }));
  return {
    route: 'local',
    path: p,
    nCells: s.nCells,
    nPoints: s.nPoints,
    meshHash: s.meshHash,
    snapshots: s.snapshots.map((x) => ({ step: x.step, time: x.time })),
    fields,
    index: s.indexSource.kind,
  };
}

/**
 * F-815: B's field at snapshot j minus A's at snapshot i, in f64 (ADR 0801, addendum F-815: C2, the same mesh; C3, the
 * difference of the stored values, refused where rcfd_post's `difference` refuses or a unit would need converting).
 */
/** F-815: a difference the rules refuse (reported like a store error, with its reason as the message). */
class Refusal extends Error {}

function differenceOf(a: Store, i: number, b: Store, j: number, field: string): ArrayBuffer {
  const mesh = meshMatch(a, b);
  if (!mesh.ok) throw new Refusal(`no difference: ${mesh.reason}`);
  const fa = a.fields(i).find((f) => f.name === field);
  const fb = b.fields(j).find((f) => f.name === field);
  const fit = fieldsMatch(field, fa, fb, a.nCells);
  if (!fit.ok) throw new Refusal(`no difference: ${fit.reason}`);
  const d = difference(new Float64Array(a.field64(i, field)), new Float64Array(b.field64(j, field)));
  if (d.values === null) throw new Refusal(`no difference: ${d.reason}`);
  return d.values.buffer;
}

function run(t: WorkerTask): { result: WorkerResult; transfer: ArrayBuffer[] } {
  switch (t.op) {
    case 'open':
      return { result: { kind: 'open', info: info(storeFor(t.dir), t.path) }, transfer: [] };
    case 'field': {
      const values = storeFor(t.dir).field(t.snapshot, t.field);
      return { result: { kind: 'field', snapshot: t.snapshot, field: t.field, values }, transfer: [values] };
    }
    case 'surface': {
      const o = openFor(t.dir);
      const s = o.store.surface();
      keepCells(o, s.cells);
      return { result: { kind: 'surface', ...s }, transfer: [s.points, s.triangles, s.cells] };
    }
    case 'surfaceField': {
      const o = openFor(t.dir);
      if (o.store.fieldInfo(t.snapshot, t.field).location !== 'cell') {
        throw new StoreError(`field '${t.field}' is a point field; the surface's values are the values of its triangles' cells`);
      }
      if (!o.cells) keepCells(o, o.store.surface().cells);
      const values = gatherSurface(o.cells!, new Float32Array(o.store.field(t.snapshot, t.field))).buffer;
      return { result: { kind: 'surfaceField', snapshot: t.snapshot, field: t.field, meshHash: o.store.meshHash, values }, transfer: [values] };
    }
    case 'difference': {
      const values = differenceOf(storeFor(t.dir), t.snapshot, storeFor(t.otherDir), t.otherSnapshot, t.field);
      return { result: { kind: 'difference', snapshot: t.snapshot, otherSnapshot: t.otherSnapshot, field: t.field, values }, transfer: [values] };
    }
    case 'hooks':
      wrongStride = t.wrongStride;
      return { result: { kind: 'hooks' }, transfer: [] };
  }
}

const port = parentPort;
if (port) {
  port.on('message', (t: WorkerTask) => {
    let reply: WorkerReply;
    let transfer: ArrayBuffer[] = [];
    try {
      const r = run(t);
      reply = { id: t.id, ok: true, result: r.result };
      transfer = r.transfer;
    } catch (e) {
      reply = { id: t.id, ok: false, store: e instanceof StoreError || e instanceof Refusal, message: e instanceof Error ? e.message : String(e) };
    }
    port.postMessage(reply, transfer);
  });
}
