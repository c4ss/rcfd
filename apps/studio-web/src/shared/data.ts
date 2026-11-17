// Bulk field data for the renderer (F-804; ADR 0350 addendum 1, QA2; ADR 0801 addendum 3): the messages that cross the
// data port and the bridge the preload script puts on `window.studioData`. Shared by all three processes, so it uses
// neither Node nor DOM types.
//
// Two routes, one shape of reply:
// - `local` (R2): the main process reads `store.rts` of a project on this machine itself, in a worker thread
//   (src/main/data/tsstore.ts, a port of the Rust reader of crates/rcfd-tsstore). No service is needed to look at a run.
// - `service` (R1): the opt-in `data.*` methods of the connected service (`rcfd-data/1`, protocol 1.2): the call goes on
//   the service client's call connection, the bytes come over the service's data connection (docs/service-protocol.md).
//
// Every value the page receives is the Rust reader's value cast to `f32`, bit for bit (G3, ADR 0350 P12): the same
// bytes, little-endian on both sides. The renderer never reads a file or opens a connection (G10); the main process
// checks every request (src/main/data/bridge.ts) and does all I/O off its own thread or asynchronously (G21).

/** The one IPC channel main uses to hand the preload script the data port (main -> renderer only). */
export const DATA_PORT_CHANNEL = 'studio.data-port';

/** The routes of ADR 0350: R2 (`local`) and R1 (`service`). */
export const DATA_ROUTES = ['local', 'service'] as const;
export type DataRoute = (typeof DATA_ROUTES)[number];

/**
 * A field store: the folder that holds `store.rts` (a run's `fields` folder, `runs/<id>/fields`).
 * - `local`: an absolute path on this machine, inside one of the Studio's data roots (src/main/data/roots.ts).
 * - `service`: the path as the service takes it: relative to its root, or absolute inside it (its own sandbox applies).
 */
export interface StoreRef {
  readonly route: DataRoute;
  readonly path: string;
}

export interface FieldInfo {
  readonly name: string;
  readonly unit: string;
  readonly location: 'cell' | 'point';
  /** Number of values. */
  readonly n: number;
  readonly precision: 'f32' | 'f64';
  readonly codec: 'raw' | 'shuffle_deflate';
}

export interface SnapshotInfo {
  readonly step: number;
  readonly time: number;
}

/** What `open` returns (the members of `data.open`, in camel case). */
export interface StoreInfo {
  readonly route: DataRoute;
  readonly path: string;
  readonly nCells: number;
  readonly nPoints: number;
  /** The stored mesh's content hash, 16 hexadecimal digits. */
  readonly meshHash: string;
  readonly snapshots: readonly SnapshotInfo[];
  /** The fields of the first snapshot (empty when there is none). */
  readonly fields: readonly FieldInfo[];
  /**
   * `local` only: `footer` for a finished store, `scanned` when the index was rebuilt from the frame heads (a live or
   * interrupted run; a later `open` picks up new snapshots). `null` for `service` (data.open does not say).
   */
  readonly index: 'footer' | 'scanned' | null;
  /**
   * ADR 0738 P5: the unit of the stored mesh's coordinates as the engine names it (`data.open`'s `length_unit`).
   * `null` for `local`: store.rts does not write it (the engine's derived pictures name it, `length_unit` of their reply).
   */
  readonly lengthUnit: string | null;
}

/** One field of one snapshot, as `f32` (a field stored as `f64` is rounded to the nearest single, as `as f32` does). */
export interface FieldData {
  readonly snapshot: number;
  readonly field: string;
  readonly values: Float32Array<ArrayBuffer>;
}

/**
 * The boundary faces of the stored mesh cut into triangles (a fan per face), as `data.surface` gives them: points
 * (three `f32` each, used points only, in order of first use), triangle corners (`u32`, three per triangle) and the
 * owner cell of each triangle (`u32`).
 */
/**
 * F-815: the difference of one cell field between a snapshot of a store (the baseline A) and a snapshot of another store
 * on the same mesh (the compared run B): `B - A` in `f64` of the stored values, every cell (ADR 0801, addendum F-815,
 * C2 and C3; src/shared/compare.ts). Route `local` only.
 */
export interface DifferenceData {
  readonly snapshot: number;
  readonly otherSnapshot: number;
  readonly field: string;
  readonly values: Float64Array<ArrayBuffer>;
}

export interface SurfaceData {
  readonly points: Float32Array<ArrayBuffer>;
  readonly triangles: Uint32Array<ArrayBuffer>;
  readonly cells: Uint32Array<ArrayBuffer>;
}

/**
 * F-819b: one cell field of one snapshot on the boundary surface only: `values[t]` is the stored value (as `f32`, bit for
 * bit the same as `FieldData`'s) of the owner cell of triangle `t` of the store's surface (`SurfaceData.cells[t]`).
 * `meshHash` names the stored mesh the surface was made from, so a page can tell it holds that surface. Route `local`
 * only (the store reader gathers it where it read the whole field).
 */
export interface SurfaceFieldData {
  readonly snapshot: number;
  readonly field: string;
  readonly meshHash: string;
  readonly values: Float32Array<ArrayBuffer>;
}

/**
 * F-836: the boundary surface of a store's mesh as `SurfaceData` gives it, with the patch of each triangle: `patch[t]` is
 * the index, in the stored mesh's patch order (the order of `mesh.preview`'s `patches`), of the patch whose boundary face
 * triangle `t` was cut from. Route `local` only.
 */
export interface PatchSurfaceData extends SurfaceData {
  readonly patch: Uint32Array<ArrayBuffer>;
  /** The stored mesh's content hash, 16 hexadecimal digits. */
  readonly meshHash: string;
}

/** Errors of the data bridge itself; errors of the service keep their JSON-RPC codes (for example -32602, 1004). */
export type DataErrorCode =
  | 'bad_request' // the message or its arguments have the wrong shape
  | 'not_allowed' // a local path outside the data roots, or not absolute
  | 'store' // the local store cannot be read, or failed a check (the reader's message says which)
  | 'cancelled' // cancelled by `cancel(tag)`
  | 'no_data' // the service has no `data.*` methods (rcfd serve does not switch them on; ADR 0350 QA4)
  | 'not_configured' // no service configured (route `service`)
  | 'disconnected' // the service or its data connection is not there right now
  | 'timeout' // no answer within the request's time limit
  | 'protocol' // the service's data connection sent something that is not rcfd-data/1
  | 'no_port' // the preload script has no data port (main never sent one)
  | 'internal'; // the reader's worker failed

export interface DataError {
  readonly code: number | DataErrorCode;
  readonly message: string;
  readonly data?: unknown;
}

export type DataReply<T = unknown> = { readonly ok: true; readonly result: T } | { readonly ok: false; readonly error: DataError };

/** Default and bounds of a request's time limit in milliseconds. */
export const DEFAULT_DATA_TIMEOUT_MS = 60_000;
export const MIN_DATA_TIMEOUT_MS = 1;
export const MAX_DATA_TIMEOUT_MS = 600_000;
/** Longest path and field name a request may carry. */
export const MAX_DATA_PATH = 4096;
export const MAX_FIELD_NAME = 256;
/** Requests one window may have waiting at once; more are refused with `bad_request`. */
export const MAX_DATA_PENDING = 256;

export interface DataOptions {
  /**
   * A label of the caller's choosing; `cancel(tag)` cancels every waiting request that carries it (G21: a load can be
   * cancelled, for example when the user scrubs on). A string of at most 64 characters or a non-negative integer.
   */
  readonly tag?: string | number;
  /** Time limit in milliseconds (MIN_DATA_TIMEOUT_MS to MAX_DATA_TIMEOUT_MS; default DEFAULT_DATA_TIMEOUT_MS). */
  readonly timeoutMs?: number;
}

// ---- Messages on the data port (preload <-> main). The main process validates every request (src/main/data/bridge.ts).

type Common = { readonly id: number; readonly tag?: string | number; readonly timeoutMs?: number };

/** Preload -> main. */
export type DataRequest =
  | ({ readonly op: 'open'; readonly store: StoreRef } & Common)
  | ({ readonly op: 'field'; readonly store: StoreRef; readonly snapshot: number; readonly field: string } & Common)
  | ({ readonly op: 'surface'; readonly store: StoreRef } & Common)
  | ({ readonly op: 'surfaceField'; readonly store: StoreRef; readonly snapshot: number; readonly field: string } & Common)
  | ({ readonly op: 'patchSurface'; readonly store: StoreRef } & Common)
  | ({
      readonly op: 'difference';
      readonly store: StoreRef;
      readonly snapshot: number;
      readonly other: StoreRef;
      readonly otherSnapshot: number;
      readonly field: string;
    } & Common)
  | ({ readonly op: 'derive'; readonly store: StoreRef; readonly snapshot: number; readonly args: DeriveArgs } & Common)
  | { readonly op: 'cancel'; readonly id: number; readonly tag: string | number };

/**
 * Main -> preload. Bulk values travel as ArrayBuffers (one structured-clone copy across the process boundary: Electron's
 * MessagePortMain transfers ports only); the preload script wraps them in typed arrays.
 */
export type DataWireResult =
  | { readonly kind: 'open'; readonly info: StoreInfo }
  | { readonly kind: 'field'; readonly snapshot: number; readonly field: string; readonly values: ArrayBuffer }
  | { readonly kind: 'surface'; readonly points: ArrayBuffer; readonly triangles: ArrayBuffer; readonly cells: ArrayBuffer }
  | { readonly kind: 'surfaceField'; readonly snapshot: number; readonly field: string; readonly meshHash: string; readonly values: ArrayBuffer }
  | { readonly kind: 'patchSurface'; readonly points: ArrayBuffer; readonly triangles: ArrayBuffer; readonly cells: ArrayBuffer; readonly patch: ArrayBuffer; readonly meshHash: string }
  | { readonly kind: 'difference'; readonly snapshot: number; readonly otherSnapshot: number; readonly field: string; readonly values: ArrayBuffer }
  | { readonly kind: 'derive'; readonly glb: ArrayBuffer; readonly summary: string }
  | { readonly kind: 'cancel'; readonly cancelled: number };

export type DataPortMessage = { readonly type: 'reply'; readonly id: number; readonly reply: DataReply<DataWireResult> };

/** What the preload script puts on `window.studioData`. Each function's arguments are checked again in main. */
export interface DataBridge {
  /** Sizes, snapshots and fields of a store. */
  open(store: StoreRef, options?: DataOptions): Promise<DataReply<StoreInfo>>;
  /** One field of one snapshot (0-based index into `StoreInfo.snapshots`). */
  field(store: StoreRef, snapshot: number, field: string, options?: DataOptions): Promise<DataReply<FieldData>>;
  /** The boundary surface of the stored mesh. */
  surface(store: StoreRef, options?: DataOptions): Promise<DataReply<SurfaceData>>;
  /** Cancels every waiting request with this tag; each answers `cancelled`. Resolves with how many there were. */
  cancel(tag: string | number): Promise<DataReply<number>>;
}

/**
 * F-815: what the preload script puts on `window.studioCompare`, over the same data port as `window.studioData` (kept
 * apart so that F-804's bridge keeps exactly its four functions). Its arguments are checked again in main.
 */
export interface CompareBridge {
  /**
   * `other`'s field at `otherSnapshot` minus `store`'s at `snapshot`, in `f64`, when the two stores have the same mesh
   * (route `local` only; a refusal says why). A `tag` is cancelled by `window.studioData.cancel`.
   */
  difference(store: StoreRef, snapshot: number, other: StoreRef, otherSnapshot: number, field: string, options?: DataOptions): Promise<DataReply<DifferenceData>>;
}

/**
 * F-819b: what the preload script puts on `window.studioSurfaceValues`, over the same data port as `window.studioData`
 * (kept apart, as `window.studioCompare` is, so that F-804's bridge keeps exactly its four functions). Its arguments
 * are checked again in main.
 */
export interface SurfaceValuesBridge {
  /**
   * Field `field` of snapshot `snapshot` on the store's boundary surface: one value per triangle of `surface(store)`
   * (route `local` only; the service route answers `bad_request`, and the whole field is read with `studioData.field`).
   * A `tag` is cancelled by `window.studioData.cancel`.
   */
  field(store: StoreRef, snapshot: number, field: string, options?: DataOptions): Promise<DataReply<SurfaceFieldData>>;
}

/**
 * F-836 (ADR 0732): what the preload script puts on `window.studioMeshPreview`, over the same data port as
 * `window.studioData` (kept apart, as `window.studioCompare` is, so that F-804's bridge keeps exactly its four functions).
 * The case editor draws the mesh store `mesh.preview` wrote with it, one colour per patch. Its arguments are checked
 * again in main.
 */
export interface MeshPreviewBridge {
  /** The boundary surface of the store's mesh with the patch of each triangle (route `local` only). */
  surface(store: StoreRef, options?: DataOptions): Promise<DataReply<PatchSurfaceData>>;
}

// ---- ADR 0738 (F-809, E-809d): derived pictures made by the engine ------------------------------------------------------

/** The pictures the Studio asks the engine for (`rcfd post derive --kind`). */
export const DERIVE_KINDS = ['cut_surface', 'iso', 'streamlines'] as const;
export type DeriveKind = (typeof DERIVE_KINDS)[number];

/**
 * What `rcfd post derive` is asked to make of one snapshot (docs/mcp-tools.md, `derive_picture`). Main checks every
 * member and passes it as the command's flags; nothing else reaches the command line.
 * - `cut_surface`: the boundary surface the Studio draws, cut by `axis`/`at`/`keep` when given, with the arrows when
 *   `vectors` (the engine's T2 and T3, ADR 0738 P4);
 * - `iso`: where the cell field `field` (a stored cell field or `speed`) equals `level`;
 * - `streamlines`: from `rake` (`n` points from `p0` to `p1`), `direction`, at most `maxSteps` steps each.
 */
export interface DeriveArgs {
  readonly kind: DeriveKind;
  readonly field?: string;
  readonly level?: number;
  readonly axis?: 0 | 1 | 2;
  readonly at?: number;
  readonly keep?: 'below' | 'above';
  readonly vectors?: boolean;
  readonly rake?: { readonly p0: readonly [number, number, number]; readonly p1: readonly [number, number, number]; readonly n: number };
  readonly direction?: 'forward' | 'backward' | 'both';
  readonly maxSteps?: number;
}

/** The engine's GLB and its reply (`kind`, `snapshot`, `bounds`, `length_unit`, the counts; JSON). */
export interface DerivedData {
  readonly glb: ArrayBuffer;
  readonly summary: Record<string, unknown>;
}

/**
 * ADR 0738: what the preload script puts on `window.studioDerived`, over the same data port as `window.studioData` (kept
 * apart, as `window.studioCompare` is). Route `local` only: main runs the engine's `rcfd post derive` on the store's
 * folder (inside a data root); without an engine configured, or on the service route, it answers `not_configured` or
 * `bad_request` with the reason. A `tag` is cancelled by `window.studioData.cancel` (the command is stopped).
 */
export interface DerivedBridge {
  derive(store: StoreRef, snapshot: number, args: DeriveArgs, options?: DataOptions): Promise<DataReply<DerivedData>>;
}
