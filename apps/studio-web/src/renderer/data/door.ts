// The renderer's one door to bulk field data (F-804's `window.studioData`, src/shared/data.ts): the only renderer code
// that names the bridge (tests/unit/data-door-static.spec.ts), as src/renderer/actions is for the service (F-827) and
// src/renderer/project/door.ts for the project folder (F-805). Surfaces (the field viewport of F-808, scrubbing later)
// import these functions; every answer is the bridge's own `{ok, result}` or `{ok: false, error}`, never a throw.
//
// The data port is a main-process read of a store (`local`, R2) or the service's `data.*` methods (`service`, R1), not a
// service method of SERVICE_METHODS, so it has no entry in the action registry (G16; reported with F-804 and F-808).
import type { DataOptions, DataReply, DeriveArgs, DerivedData, DifferenceData, FieldData, PatchSurfaceData, StoreInfo, StoreRef, SurfaceData, SurfaceFieldData } from '../../shared/data.js';

const bridge = () => window.studioData;

const noBridge = <T>(): DataReply<T> => ({ ok: false, error: { code: 'no_port', message: 'this page has no data bridge (window.studioData)' } });

/** Sizes, snapshots and fields of a store. */
export function openStore(store: StoreRef, options?: DataOptions): Promise<DataReply<StoreInfo>> {
  const b = bridge();
  return b ? b.open(store, options) : Promise.resolve(noBridge());
}

/** One field of one snapshot (0-based index into `StoreInfo.snapshots`), as `f32`. */
export function loadField(store: StoreRef, snapshot: number, field: string, options?: DataOptions): Promise<DataReply<FieldData>> {
  const b = bridge();
  return b ? b.field(store, snapshot, field, options) : Promise.resolve(noBridge());
}

/** The boundary surface of the stored mesh (triangles, their points and owner cells). */
export function loadSurface(store: StoreRef, options?: DataOptions): Promise<DataReply<SurfaceData>> {
  const b = bridge();
  return b ? b.surface(store, options) : Promise.resolve(noBridge());
}

/**
 * F-819b: one field of one snapshot on the store's boundary surface only, one value per triangle of `loadSurface`'s
 * surface (route `local`; the service route answers `bad_request`, and `loadField` reads the whole field there).
 */
export function loadSurfaceField(store: StoreRef, snapshot: number, field: string, options?: DataOptions): Promise<DataReply<SurfaceFieldData>> {
  // F-819b: the surface values have their own bridge over the same data port (window.studioSurfaceValues)
  const b = window.studioSurfaceValues as typeof window.studioSurfaceValues | undefined;
  return b ? b.field(store, snapshot, field, options) : Promise.resolve(noBridge());
}

/** F-815: `other`'s field at `otherSnapshot` minus `store`'s at `snapshot`, in f64, on the same mesh (route local). */
export function loadDifference(
  store: StoreRef,
  snapshot: number,
  other: StoreRef,
  otherSnapshot: number,
  field: string,
  options?: DataOptions,
): Promise<DataReply<DifferenceData>> {
  // F-815: the difference has its own bridge over the same data port (window.studioCompare)
  const b = window.studioCompare as typeof window.studioCompare | undefined;
  return b ? b.difference(store, snapshot, other, otherSnapshot, field, options) : Promise.resolve(noBridge());
}

/** F-836: the boundary surface of a mesh preview's store with the patch of each triangle (route local). */
export function loadPatchSurface(store: StoreRef, options?: DataOptions): Promise<DataReply<PatchSurfaceData>> {
  // F-836: its own bridge over the same data port (window.studioMeshPreview)
  const b = window.studioMeshPreview as typeof window.studioMeshPreview | undefined;
  return b ? b.surface(store, options) : Promise.resolve(noBridge());
}

/** ADR 0738: a derived picture of one snapshot made by the engine (`rcfd post derive`, route local), as GLB bytes. */
export function loadDerived(store: StoreRef, snapshot: number, args: DeriveArgs, options?: DataOptions): Promise<DataReply<DerivedData>> {
  // ADR 0738: its own bridge over the same data port (window.studioDerived)
  const b = window.studioDerived as typeof window.studioDerived | undefined;
  return b ? b.derive(store, snapshot, args, options) : Promise.resolve(noBridge());
}

/** Cancels every waiting request with this tag (G21: a superseded load is dropped). */
export function cancelLoads(tag: string | number): Promise<DataReply<number>> {
  const b = bridge();
  return b ? b.cancel(tag) : Promise.resolve(noBridge());
}
