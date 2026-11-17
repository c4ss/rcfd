// The field viewport (F-808; DESIGN.md Field view; guardrails G3, G8, G9, G21): one field of one snapshot of a store,
// drawn on the boundary surface of the stored mesh, with its legend, caption and the value under the cursor.
//
// Data comes only through the data door (../data/door.ts, F-804's window.studioData): open the store, its surface once
// per store, then the field. Every request carries this view's tag, and a superseded or cancelled load is cancelled
// by it (G21). What is shown traces to the store: the colours come from the stored values through the table of
// ./colormap.ts, the range is the min and max of the shown values (rcfd-studio's snapshot range), the readout is the
// stored value of the cell under the cursor, and the caption names the store, quantity, unit, snapshot, step and time.
//
// F-819b: a stored field of a local store comes as surface values, one per triangle (the store reader gathers them
// where it read the whole field, src/main/data/worker.ts), since everything drawn, ranged and read out here is a value
// of a cell that owns a surface triangle; the colour range is the min and max over those same values (F-808's), and
// the readout at a triangle is its owner cell's stored value, bit for bit. The whole field is read for the service
// route and for a comparison's values (`load`), as before. F-809's derived pictures are the engine's (ADR 0738): the
// cut surface and arrows, the iso-surface and the streamlines come as GLB from `rcfd post derive` (useDerived), so
// none needs the shown field's other cells here. No pick runs while a button is
// held (an orbit or a pan): the readout is cleared at the drag's first move and taken again where the button is let go.
//
// Timing marks (performance.mark / measure, read by scripts/measure-field.mjs for P7): `rcfd:field:open` when a store is
// opened, then the measure `rcfd:field:first-picture` when its first field is drawn, finished on the GPU and one
// animation frame has passed.
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import type { DataOptions, DataReply, FieldInfo, StoreInfo, StoreRef, SurfaceData } from '../../shared/data.js';
import { cancelLoads, loadField, loadSurface, loadSurfaceField, openStore } from '../data/door.js';
import { Button, IconButton, NumberField, Quiet, Toggle } from '../ui/index.js';
import { IconToggle } from '../ui/IconToggle.js';
import { useCommandHandlers } from '../commands/host.js';
import { savePagePng } from '../workspace/door.js';
import { rangeOf, shownRange } from './surface.js';
import { AXIS, LENGTH_UNIT, LENGTH_UNIT_WHY, cellsOutside, cellsOutsideOf, domainText, fixedRange, lengthText, pngName, scaleBar, type RangeMode } from './viewExtras.js';
import { colourRange, f32Text, lutBytes, type ColourRange } from './colormap.js';
import { FieldCanvas, type Backend, type FieldCanvasApi, type Pick } from './FieldCanvas.js';
import { CutPanel, DerivedControls, IsoPanel, StreamPanel, pictureKey, useDerived } from './DerivedControls.js';
import { availability, middleCut, rakeEnds, startRake, type Cut, type Rake } from './derived.js';
import { cutOf, isoOf, linesOf } from './glb.js';
import type { DeriveArgs } from '../../shared/data.js';
import { Legend } from './Legend.js';
import { Provenance, cellValue, snapshotFactValue } from '../provenance/index.js';
import { FieldPrep, type PreparedSurface } from './prepare.js';
import { beginWork } from '../work/work.js';
import { WorkProgress } from '../work/WorkProgress.js';
import './field.css';

export interface FieldViewportProps {
  readonly store: StoreRef;
  /** 0-based index into the store's snapshots. */
  readonly snapshot: number;
  readonly field: string;
  /** Use three's WebGL 2 back end even where WebGPU works (tests of the fallback). */
  readonly forceWebGL?: boolean;
  /**
   * F-810: a number that changes when the store may hold more snapshots (a live run): the store's snapshots are read
   * again (the surface is kept while the stored mesh is the same). A snapshot past the known ones waits for that read.
   */
  readonly reopen?: number;
  /** F-810: called when the field of a snapshot is drawn, finished on the GPU and one animation frame has passed. */
  readonly onDrawn?: (snapshot: number) => void;
  /** F-815: the colour range to draw with (a comparison's one range); absent, the range of the shown values (F-808). */
  readonly range?: ColourRange;
  /** F-815: called in the render that shows new values, with their own F-808 range (a comparison unites two). */
  readonly onShown?: (shown: { readonly snapshot: number; readonly lo: number; readonly hi: number }) => void;
  /** F-815: the legend under the view (default on; a comparison draws one legend for both views). */
  readonly legend?: boolean;
  /** F-815: where the values come from instead of the snapshot's stored field (a difference), and its key. */
  readonly load?: FieldLoad;
  readonly loadKey?: string;
  /** F-815: the quantity's name and the caption's name when the values are not one run's stored field. */
  readonly quantity?: string;
  readonly title?: string;
  /** F-815: the derived-picture controls (default on). */
  readonly derived?: boolean;
  /**
   * F-813: the learner's caption. `quantity` is the plain word for the stored field (`Velocity to the right`); the
   * precise terms (the field's stored name, snapshot, step, time, the colour range, the cells) are said to readers, in
   * the numbers' provenance and behind the first path's Explain (G22, G19, PRODUCT Principle 5). The values are the same.
   */
  readonly learner?: { readonly quantity: string };
  /**
   * F-832 (G21): the user cancelled the load of `snapshot`; the picture shown before stays, untouched, and this names
   * its snapshot (null when nothing was shown), so a host can put its line back there (no stale selection).
   */
  readonly onCancelled?: (shownSnapshot: number | null) => void;
}

/** F-815: values for the viewport instead of a stored field (the difference of two runs, as f32 to draw). */
export type FieldLoad = (snapshot: number, field: string, options: DataOptions) => Promise<DataReply<{ readonly values: Float32Array<ArrayBuffer> }>>;

type Phase =
  | { readonly kind: 'loading'; readonly what: string }
  | { readonly kind: 'error'; readonly message: string; readonly hint: string }
  | { readonly kind: 'ready' };

interface Shown {
  readonly info: StoreInfo;
  readonly fieldInfo: FieldInfo;
  readonly surface: SurfaceData;
  readonly values: Float32Array;
  /** F-819b: `values` holds one value per cell, or one per triangle of `surface` (surface values). */
  readonly per: 'cell' | 'triangle';
  /** F-832: the corner scalars, made off the main thread. */
  readonly scalars: Float32Array;
  readonly range: ColourRange;
  readonly snapshot: number;
  /** F-832: what it was loaded for (a repeated request for the same picture loads nothing). */
  readonly key: string;
}

let views = 0;


/** The run a store belongs to: the folder above `fields` (runs/<id>/fields), else the last segment. */
function runName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  const i = parts.lastIndexOf('fields');
  return (i > 0 ? parts[i - 1] : parts[parts.length - 1]) ?? path;
}

export function FieldViewport({
  store,
  snapshot,
  field,
  forceWebGL = false,
  reopen = 0,
  onDrawn: onDrawnSnapshot,
  range: rangeProp,
  onShown,
  legend = true,
  load,
  loadKey = '',
  quantity: quantityProp,
  title,
  derived = true,
  onCancelled,
  learner,
}: FieldViewportProps) {
  const tagBase = useMemo(() => `field-view-${++views}`, []);
  const lut = useMemo(lutBytes, []);
  // F-832 (G21): the arrays the GPU draws are made in a worker, one per view
  const prep = useMemo(() => new FieldPrep(), []);
  useEffect(() => () => prep.dispose(), [prep]);
  const cancelledCb = useRef(onCancelled);
  cancelledCb.current = onCancelled;
  const shownRef = useRef<Shown | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: 'loading', what: 'Opening the store' });
  const [opened, setOpened] = useState<{ key: string; info: StoreInfo; surface: SurfaceData; prepared: PreparedSurface } | null>(null);
  const [shown, setShownState] = useState<Shown | null>(null);
  const setShown = useCallback((s: Shown | null) => {
    shownRef.current = s;
    setShownState(s);
  }, []);
  const [backend, setBackend] = useState<Backend | null>(null);
  const [moved, setMoved] = useState(false);
  const [grid, setGrid] = useState(false);
  const [gridState, setGridState] = useState<'off' | 'computing' | 'on'>('off');
  // F-809: the derived pictures (./derived.ts): rcfd-render's cut plane and the vector arrows
  const [cutAsked, setCut] = useState<Cut | null>(null);
  const [vectorsAsked, setVectors] = useState(false);
  // ADR 0738: the engine's iso-surface (its level, null for off) and streamlines (their rake, null for off)
  const [isoLevel, setIsoLevel] = useState<number | null>(null);
  const [rakeAsked, setRake] = useState<Rake | null>(null);
  // the engine picture the canvas last drew, finished on the GPU (data-cut, data-arrows, data-iso, data-streamlines)
  const [pictureDrawn, setPictureDrawn] = useState<object | null>(null);
  const [pointerAt, setPointerAt] = useState('');
  const [hover, setHover] = useState<{ pick: Pick; by: 'pointer' | 'keyboard' } | null>(null);
  const [attempt, setAttempt] = useState(0);
  // F-837: the colour range mode, the whole run's range when found, the fixed range typed, the face-on scale, Save PNG
  const [rangeMode, setRangeMode] = useState<RangeMode>('snapshot');
  const [runRange, setRunRange] = useState<{ key: string; lo: number; hi: number; snapshots: number } | null>(null);
  const [runRangeError, setRunRangeError] = useState<string | null>(null);
  const [fixedLo, setFixedLo] = useState(Number.NaN);
  const [fixedHi, setFixedHi] = useState(Number.NaN);
  const [scale, setScale] = useState<ReturnType<FieldCanvasApi['faceScale']>>(null);
  const [saved, setSaved] = useState<{ ok: boolean; text: string } | null>(null);
  const api = useRef<FieldCanvasApi>(null);
  // F-828: the readout stays while its provenance is open (the pointer leaves the surface for the popover)
  const pinned = useRef(false);
  const tag = useRef('');
  const firstPicture = useRef(false);
  const reopened = useRef(reopen);
  const reopening = useRef(false);
  const drawnFor = useRef<number | null>(null);
  // the values the canvas last drew: the view is ready (data-ready) only once the shown field is on screen. The canvas
  // draws after its renderer is initialised (asynchronously, WebGPU or WebGL 2), and until then it has no handle, so a
  // key pressed in the frame would orbit nothing and read out nothing.
  const [drawnValues, setDrawnValues] = useState<Float32Array | null>(null);
  const drawnCb = useRef(onDrawnSnapshot);
  drawnCb.current = onDrawnSnapshot;
  // F-815: the comparison's hooks, read when used
  const shownCb = useRef(onShown);
  shownCb.current = onShown;
  const loadRef = useRef(load);
  loadRef.current = load;
  const storeKey = `${store.route}\u0000${store.path}`;
  // F-832: the layers are made in the worker with the surface's arrays (F-809's meshLayers, unchanged)
  const layers = opened?.prepared.layers ?? null;
  const derivedCtx = useMemo(
    () =>
      opened && layers
        ? { layers, fields: opened.info.fields, nCells: opened.info.nCells, route: store.route, storedField: load ? null : field }
        : null,
    [opened?.info.fields, opened?.info.nCells, layers, store.route, load, field],
  );
  // a picture that does not apply to this store (another store was opened) is off
  const cut = derivedCtx && cutAsked && availability('cut', derivedCtx).ok ? cutAsked : null;
  const vectorsOn = !!derivedCtx && vectorsAsked && availability('vectors', derivedCtx).ok;
  const isoOn = !!derivedCtx && isoLevel !== null && availability('iso', derivedCtx).ok;
  const rake = derivedCtx && rakeAsked && availability('streamlines', derivedCtx).ok ? rakeAsked : null;
  // ADR 0738: what the engine is asked for, of the shown snapshot
  const cutArgs: DeriveArgs | null =
    cut || vectorsOn ? { kind: 'cut_surface', ...(cut ? { axis: cut.axis, at: cut.at, keep: cut.keep } : {}), ...(vectorsOn ? { vectors: true } : {}) } : null;
  const isoArgs: DeriveArgs | null = isoOn ? { kind: 'iso', field, level: isoLevel! } : null;
  const streamArgs: DeriveArgs | null =
    rake && layers ? { kind: 'streamlines', rake: { ...rakeEnds(layers, rake), n: rake.n }, direction: 'both', maxSteps: 2000 } : null;
  const shownSnapshot = shown?.snapshot ?? null;
  const cutPic = useDerived(store, shownSnapshot, cutArgs, `${tagBase}-cut`, cut ? 'The cut plane' : 'The arrows');
  const isoPic = useDerived(store, shownSnapshot, isoArgs, `${tagBase}-iso`, 'The iso-surface');
  const streamPic = useDerived(store, shownSnapshot, streamArgs, `${tagBase}-stream`, 'The streamlines');
  const cutGlb = cutPic.kind === 'on' ? cutPic.glb : null;
  const engineCutAll = useMemo(() => (cutGlb ? cutOf(cutGlb) : null), [cutGlb]);
  const isoGlb = isoPic.kind === 'on' ? isoPic.glb : null;
  const engineIso = useMemo(() => (isoGlb ? isoOf(isoGlb) : null), [isoGlb]);
  const streamGlb = streamPic.kind === 'on' ? streamPic.glb : null;
  const engineLines = useMemo(() => (streamGlb ? linesOf(streamGlb) : null), [streamGlb]);
  // the last picture made stays while a newer one is made; it is drawn only while its kind is still asked for
  const engineCut = cut && engineCutAll ? engineCutAll : null;
  const engineArrows = vectorsOn && engineCutAll ? engineCutAll.arrows : null;
  const iso = isoOn ? engineIso : null;
  const streamlines = rake && engineLines ? engineLines.segments : null;
  const current = (pic: typeof cutPic, args: DeriveArgs | null) =>
    pic.kind === 'on' && !pic.loading && args !== null && shownSnapshot !== null && pic.key === pictureKey(storeKey, shownSnapshot, args);
  const drawnNow = pictureDrawn !== null && pictureDrawn === (engineCut ?? iso ?? engineArrows ?? streamlines);
  const derivedErrors = [cutPic, isoPic, streamPic].flatMap((p) => (p.kind === 'error' ? [p.message] : []));

  // the store: open it and read its surface (once per store)
  useEffect(() => {
    let live = true;
    const t = `${tagBase}-store-${attempt}`;
    tag.current = t;
    performance.clearMarks('rcfd:field:open');
    performance.clearMeasures('rcfd:field:first-picture');
    performance.mark('rcfd:field:open');
    firstPicture.current = true;
    setOpened(null);
    setShown(null);
    setHover(null);
    setPhase({ kind: 'loading', what: 'Opening the store' });
    // F-832: one work from the open to the surface's arrays; Cancel cancels the door's requests by their tag
    const work = beginWork({
      kind: 'field.load',
      scope: tagBase,
      label: `Opening the stored fields of ${runName(store.path)}`,
      word: 'Opening',
      onCancel: () => void cancelLoads(t),
    });
    const cancelled = () => {
      setPhase({ kind: 'error', message: 'Opening was cancelled.', hint: 'Nothing was drawn; try again to open the store.' });
    };
    void (async () => {
      const info = await openStore(store, { tag: t });
      if (!live) return;
      if (work.cancelled) return cancelled();
      if (!info.ok) {
        work.end();
        setPhase({ kind: 'error', message: `The store could not be opened: ${info.error.message}`, hint: 'Check the folder holds store.rts and is inside a data root, then try again.' });
        return;
      }
      setPhase({ kind: 'loading', what: `Reading the boundary surface of ${info.result.nCells} cells` });
      const surface = await loadSurface(store, { tag: t });
      if (!live) return;
      if (work.cancelled) return cancelled();
      if (!surface.ok) {
        work.end();
        setPhase({ kind: 'error', message: `The surface could not be read: ${surface.error.message}`, hint: 'The stored mesh may be damaged; the store’s run can be checked with rcfd.' });
        return;
      }
      let prepared: PreparedSurface;
      try {
        prepared = await prep.surface(surface.result, info.result.nCells);
      } catch (e) {
        if (!live) return;
        work.end();
        setPhase({ kind: 'error', message: `The surface could not be prepared: ${e instanceof Error ? e.message : String(e)}`, hint: 'Try again to open the store.' });
        return;
      }
      if (!live) return;
      if (work.cancelled) return cancelled();
      work.end();
      setOpened({ key: storeKey, info: info.result, surface: surface.result, prepared });
    })();
    return () => {
      live = false;
      work.end();
      void cancelLoads(t);
    };
    // the store is named by its key; a new object with the same route and path is the same store
  }, [storeKey, tagBase, attempt]);

  // F-810: the store's snapshots again (a live run writes more); the surface stays while the stored mesh is the same
  useEffect(() => {
    if (reopen === reopened.current) return;
    if (!opened || opened.key !== storeKey) {
      reopened.current = reopen;
      return;
    }
    let live = true;
    const t = `${tagBase}-reopen-${reopen}`;
    reopening.current = true;
    void (async () => {
      const info = await openStore(store, { tag: t });
      if (!live) return;
      reopening.current = false;
      reopened.current = reopen;
      if (!info.ok) {
        // the snapshots read before stay shown; the next reopen tries again
        setOpened((o) => (o ? { ...o } : o));
        return;
      }
      if (info.result.meshHash !== opened.info.meshHash) setAttempt((a) => a + 1);
      else setOpened((o) => (o && o.key === storeKey ? { ...o, info: info.result } : o));
    })();
    return () => {
      live = false;
      reopening.current = false;
      void cancelLoads(t);
    };
    // `store` is named by storeKey; `opened` is read when the reopen is asked for
  }, [reopen, storeKey, tagBase]);

  // the field of the snapshot
  useEffect(() => {
    if (!opened || opened.key !== storeKey) return;
    let live = true;
    const t = `${tagBase}-field-${snapshot}-${field}-${attempt}`;
    tag.current = t;
    const { info, surface } = opened;
    const fi = info.fields.find((f) => f.name === field);
    if (!fi) {
      setPhase({ kind: 'error', message: `The store has no field “${field}”.`, hint: `Its fields: ${info.fields.map((f) => f.name).join(', ') || 'none'}.` });
      return;
    }
    if (fi.location !== 'cell') {
      setPhase({
        kind: 'error',
        message: `“${field}” is a point field; the viewport draws cell fields only.`,
        hint: 'The surface of data.surface does not name the mesh points it uses, so point values cannot be placed on it yet.',
      });
      return;
    }
    if (!(snapshot >= 0 && snapshot < info.snapshots.length)) {
      if (reopening.current || reopen !== reopened.current) {
        setPhase({ kind: 'loading', what: 'Reading the store’s new snapshots' });
        return;
      }
      setPhase({ kind: 'error', message: `The store has no snapshot ${snapshot + 1}.`, hint: `It holds ${info.snapshots.length} snapshots.` });
      return;
    }
    // F-832: the same picture asked for again (a host putting its line back after a cancel) loads nothing
    const key = `${storeKey}\u0000${snapshot}\u0000${field}\u0000${attempt}\u0000${info.snapshots.length}`;
    const was = shownRef.current;
    if (was && was.key === key && was.surface === surface) {
      setPhase({ kind: 'ready' });
      return;
    }
    setPhase({ kind: 'loading', what: `Reading ${field} of snapshot ${snapshot + 1} of ${info.snapshots.length}` });
    // F-810's quiet scrubbing (no notice for a load shorter than 300 ms) is now the work tracker's PROGRESS_AFTER_MS
    // F-832: the load is a work, shown after PROGRESS_AFTER_MS with Cancel, which cancels the door's request by its tag
    const work = beginWork({
      kind: 'field.load',
      scope: tagBase,
      label: `Reading ${field} of snapshot ${snapshot + 1} of ${info.snapshots.length}`,
      word: `Reading ${field}`,
      onCancel: () => void cancelLoads(t),
    });
    const cancelled = () => {
      // the picture shown before stays as it was (no half-drawn field); the host may put its line back on it
      const before = shownRef.current;
      const kept = before !== null && before.surface === surface;
      if (kept) setPhase({ kind: 'ready' });
      else setPhase({ kind: 'error', message: 'Loading was cancelled.', hint: 'Nothing was drawn; try again to load it.' });
      cancelledCb.current?.(kept ? before.snapshot : null);
    };
    // F-819b: a local store's stored field comes as surface values (one per triangle); others whole, as before
    const per: Shown['per'] = !loadRef.current && store.route === 'local' ? 'triangle' : 'cell';
    void (async () => {
      const r =
        per === 'triangle'
          ? await loadSurfaceField(store, snapshot, field, { tag: t })
          : await (loadRef.current ? loadRef.current(snapshot, field, { tag: t }) : loadField(store, snapshot, field, { tag: t }));
      if (!live) return;
      if (work.cancelled) return cancelled();
      if (!r.ok) {
        work.end();
        setPhase({
          kind: 'error',
          message: r.error.code === 'cancelled' ? 'Loading was cancelled.' : `The field could not be read: ${r.error.message}`,
          hint: 'Try again to load it.',
        });
        return;
      }
      if (per === 'cell' && r.result.values.length !== info.nCells) {
        work.end();
        setPhase({ kind: 'error', message: `The field has ${r.result.values.length} values for ${info.nCells} cells.`, hint: 'The store does not match its mesh.' });
        return;
      }
      if (per === 'triangle' && !('meshHash' in r.result && r.result.meshHash === info.meshHash && r.result.values.length === surface.cells.length)) {
        work.end();
        setPhase({ kind: 'error', message: 'The store’s mesh is not the one its surface was read from.', hint: 'The store was replaced; try again to open it.' });
        return;
      }
      let made: Awaited<ReturnType<FieldPrep['field']>>;
      try {
        made = per === 'triangle' ? await prep.surfaceField(opened.prepared, r.result.values) : await prep.field(opened.prepared, r.result.values);
      } catch (e) {
        if (!live) return;
        work.end();
        setPhase({ kind: 'error', message: `The field could not be prepared: ${e instanceof Error ? e.message : String(e)}`, hint: 'Try again to load it.' });
        return;
      }
      if (!live) return;
      if (work.cancelled) return cancelled();
      work.end();
      const span = made.range;
      if (!span) {
        setPhase({ kind: 'error', message: 'The surface has no triangles to draw.', hint: 'The stored mesh has no boundary faces.' });
        return;
      }
      drawnFor.current = snapshot;
      shownCb.current?.({ snapshot, lo: span.lo, hi: span.hi });
      setShown({ info, fieldInfo: fi, surface, values: made.values, per, scalars: made.scalars, range: colourRange(span.lo, span.hi), snapshot, key });
      setPhase({ kind: 'ready' });
    })();
    return () => {
      live = false;
      work.end();
      void cancelLoads(t);
    };
    // `store` is named by storeKey; `reopen` is read to tell a snapshot not read yet from one that does not exist;
    // F-815: `loadKey` names the values when they are not the stored field
  }, [opened, storeKey, snapshot, field, tagBase, attempt, loadKey]);

  // F-837: the whole run's range, the min and max of the shown values over every stored snapshot (rcfd-studio's run
  // range); a work with Cancel (G21), found again when the store holds more snapshots
  const ranged = rangeProp === undefined && !load;
  const runKey = opened ? `${opened.key}\u0000${field}\u0000${opened.info.snapshots.length}` : '';
  useEffect(() => {
    if (!ranged || rangeMode !== 'run' || !opened || opened.key !== storeKey || runRange?.key === runKey) return;
    const fi = opened.info.fields.find((f) => f.name === field);
    if (!fi || fi.location !== 'cell') return;
    let live = true;
    const n = opened.info.snapshots.length;
    const t = `${tagBase}-range-${field}-${n}`;
    setRunRangeError(null);
    const work = beginWork({ kind: 'field.load', scope: tagBase, label: `Finding the range of ${field} over ${n} snapshots`, word: 'Range of the run', onCancel: () => void cancelLoads(t) });
    void (async () => {
      let lo = Number.POSITIVE_INFINITY;
      let hi = Number.NEGATIVE_INFINITY;
      // F-819b: a local store's snapshots as surface values (the same values shownRange takes from the whole field)
      const onSurface = store.route === 'local';
      for (let i = 0; i < n; i++) {
        const r = onSurface ? await loadSurfaceField(store, i, field, { tag: t }) : await loadField(store, i, field, { tag: t });
        if (!live) return;
        if (work.cancelled) {
          setRangeMode('snapshot');
          return;
        }
        if (!r.ok) {
          work.end();
          setRunRangeError(`The whole run’s range was not found: snapshot ${i + 1} could not be read (${r.error.message}).`);
          return;
        }
        if (onSurface && !('meshHash' in r.result && r.result.meshHash === opened.info.meshHash && r.result.values.length === opened.surface.cells.length)) {
          work.end();
          setRunRangeError(`The whole run’s range was not found: the store’s mesh is not the one its surface was read from.`);
          return;
        }
        const span = onSurface ? rangeOf(r.result.values) : shownRange(opened.surface.cells, r.result.values);
        if (span) {
          if (span.lo < lo) lo = span.lo;
          if (span.hi > hi) hi = span.hi;
        }
      }
      work.end();
      if (lo <= hi) setRunRange({ key: runKey, lo, hi, snapshots: n });
      else setRunRangeError('The whole run’s range was not found: no snapshot has a value on the surface.');
    })();
    return () => {
      live = false;
      work.end();
      void cancelLoads(t);
    };
    // `store` is named by storeKey
  }, [ranged, rangeMode, opened, storeKey, runKey, field, tagBase, runRange?.key]);
  const fixed = fixedRange(fixedLo, fixedHi);
  const fixedLoOk = 'problem' in fixed ? Number.NaN : fixed.lo;
  const fixedHiOk = 'problem' in fixed ? Number.NaN : fixed.hi;
  const modeRange = useMemo(() => {
    if (!ranged) return null;
    if (rangeMode === 'run' && runRange && runRange.key === runKey) return colourRange(runRange.lo, runRange.hi);
    if (rangeMode === 'fixed' && Number.isFinite(fixedLoOk)) return colourRange(fixedLoOk, fixedHiOk);
    return null;
  }, [ranged, rangeMode, runRange, runKey, fixedLoOk, fixedHiOk]);
  const chooseRange = (m: RangeMode) => {
    setRangeMode(m);
    // a fixed range starts as the range drawn now
    if (m === 'fixed' && shownRef.current && (Number.isNaN(fixedLo) || Number.isNaN(fixedHi))) {
      const r = modeRange ?? shownRef.current.range;
      setFixedLo(r.lo);
      setFixedHi(r.hi);
    }
  };
  // F-837: the scale bar follows the camera (read once per frame at most)
  const scaleFrame = useRef(0);
  const onView = useCallback(() => {
    if (scaleFrame.current) return;
    scaleFrame.current = requestAnimationFrame(() => {
      scaleFrame.current = 0;
      setScale(api.current?.faceScale() ?? null);
    });
  }, []);
  useEffect(() => () => cancelAnimationFrame(scaleFrame.current), []);

  const onDrawn = useCallback((values: Float32Array) => {
    setDrawnValues(values);
    if (drawnFor.current !== null) drawnCb.current?.(drawnFor.current);
    if (!firstPicture.current) return;
    firstPicture.current = false;
    try {
      performance.measure('rcfd:field:first-picture', 'rcfd:field:open');
    } catch {
      // the mark was cleared by a newer open
    }
  }, []);

  const readAt = (x: number, y: number) => {
    const pick = api.current?.pickAt(x, y) ?? null;
    setHover(pick ? { pick, by: 'pointer' } : null);
    // F-809: where the readout was taken (the e2e tests wait for the readout of the point they moved to)
    setPointerAt(`${x},${y}`);
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    // F-819b: no pick while a button is held (the view orbits or pans under the cursor): the readout goes at the first
    // move, as it would name a cell that is no longer under the cursor, and comes back where the button is let go
    if (e.buttons !== 0) {
      setHover((h) => (h === null || pinned.current ? h : null));
      return;
    }
    readAt(e.clientX, e.clientY);
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    // a release on the canvas only (a press on a control drawn over the view is the control's)
    if (pinned.current || !(e.target instanceof HTMLCanvasElement)) return;
    const r = e.currentTarget.getBoundingClientRect();
    // released outside the frame (the orbit holds the pointer): nothing is under the cursor there
    if (e.clientX < r.left || e.clientX >= r.right || e.clientY < r.top || e.clientY >= r.bottom) setHover(null);
    else readAt(e.clientX, e.clientY);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    // a chord with Ctrl, Cmd or Alt is someone else's (Ctrl+O opens a folder, Ctrl+0 is the actual size)
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (!api.current?.key(e.key)) return;
    e.preventDefault();
    const pick = api.current.pickCentre();
    setHover(pick ? { pick, by: 'keyboard' } : null);
  };
  const fi = shown?.fieldInfo;
  const quantity = quantityProp ?? fi?.name ?? field;
  // F-815: the range drawn and shown in the legend: a comparison's one range, else the shown values' own
  // F-837: or the whole run's, or the fixed one
  const drawnRange = shown ? (rangeProp ?? modeRange ?? shown.range) : null;
  const unit = fi?.unit ?? '';
  // F-813: the word a learner reads for the quantity (the stored name stays in the provenance and for readers)
  const word = learner?.quantity ?? quantity;
  const snap = shown ? shown.info.snapshots[shown.snapshot] : undefined;
  // F-828: the caption's step and time open their provenance (the store's snapshot record), as the time panel's do
  const snapFact = (q: 'step' | 'time', v: number | null | undefined) =>
    shown && snap
      ? ({
          kind: 'number',
          source: snapshotFactValue({
            run: runName(shown.info.path),
            quantity: q,
            shown: String(v),
            value: v ?? null,
            snapshot: shown.snapshot,
            of: shown.info.snapshots.length,
            step: snap.step,
            time: snap.time ?? null,
          }),
        } as const)
      : null;
  const reset = () => {
    api.current?.reset();
    setHover(null);
  };
  // F-837: Save PNG of the frame with its legend and caption (DESIGN.md: a field is never shown without them)
  const sectionRef = useRef<HTMLElement>(null);
  const savePng = async (): Promise<string[]> => {
    const el = sectionRef.current;
    const frame = el?.querySelector('.fv-frame');
    const info = el?.querySelector('.fv-info');
    if (!el || !frame || !info || !shown) throw new Error('Nothing is drawn yet.');
    const a = frame.getBoundingClientRect();
    const b = info.getBoundingClientRect();
    const left = Math.min(a.left, b.left);
    const top = Math.min(a.top, b.top);
    const rect = { x: left, y: top, width: Math.max(a.right, b.right) - left, height: Math.max(a.bottom, b.bottom) - top };
    const name = pngName(title ?? runName(shown.info.path), quantity, snap?.step);
    setSaved(null);
    const r = await savePagePng(rect, name);
    if (!r.ok) {
      setSaved({ ok: false, text: `The picture was not saved: ${r.error.message}` });
      throw new Error(r.error.message);
    }
    if (r.result.path === null) return ['Not saved: the dialog was cancelled.'];
    setSaved({ ok: true, text: r.result.path });
    return [`Saved ${r.result.path} (${r.result.width} by ${r.result.height} pixels).`];
  };
  const twoD = layers?.twoD === true;
  // Addendum 22: Face on only turns the view (the centre and the zoom stay); Reset view is the full reset
  const faceOn = () => api.current?.faceOn();
  const NOTHING = 'No field is drawn yet: open a run in the workspace and wait for its picture.';
  useCommandHandlers({
    'field.save_png': { unavailable: () => (shown ? null : NOTHING), run: savePng },
    'field.range': {
      unavailable: () => (!shown ? NOTHING : ranged ? null : 'In compare mode both views share one range; leave compare mode first.'),
      run: (args) => chooseRange((['snapshot', 'run', 'fixed'] as const).find((m) => m === args['mode']) ?? 'snapshot'),
    },
    'field.face_on': { unavailable: () => (shown ? null : NOTHING), run: faceOn },
    'field.reset_view': { unavailable: () => (shown ? null : NOTHING), run: reset },
  });
  const outside =
    ranged && rangeMode === 'fixed' && shown && !('problem' in fixed)
      ? (shown.per === 'triangle' ? cellsOutsideOf : cellsOutside)(shown.surface.cells, shown.values, fixed.lo, fixed.hi)
      : null;
  const bar = twoD && layers && scale && scale.axis === layers.thin ? scaleBar(scale.perPixel, scale.widthPx) : null;
  const rangeWords = rangeProp
    ? 'viridis, one range for both runs'
    : rangeMode === 'run'
      ? modeRange && runRange
        ? `viridis, range of the whole run (${runRange.snapshots} snapshots)`
        : 'viridis, range of the shown values (the whole run’s is being found)'
      : rangeMode === 'fixed'
        ? modeRange
          ? 'viridis, a fixed range'
          : 'viridis, range of the shown values (the fixed range is not complete)'
        : 'viridis, range of the shown values';

  return (
    <section className="fv" aria-label={`Field view: ${quantity}`} ref={sectionRef} data-range-mode={ranged ? rangeMode : 'shared'}>
      <div className="fv-tools">
        <Toggle isSelected={grid} onChange={setGrid} isDisabled={!shown}>
          Mesh grid
        </Toggle>
        {grid && gridState === 'computing' && <span className="fv-note">Finding the face edges…</span>}
        {derived && (
          <DerivedControls
            ctx={derivedCtx}
            cut={cut}
            onCut={setCut}
            vectors={vectorsOn}
            onVectors={setVectors}
            startCut={layers ? () => middleCut(layers, 0) : null}
            iso={isoOn}
            onIso={(on) => setIsoLevel(on && drawnRange ? Math.fround(0.5 * (drawnRange.lo + drawnRange.hi)) : null)}
            streamlines={rake !== null}
            onStreamlines={(on) => setRake(on && layers ? startRake(layers) : null)}
          />
        )}
        {derivedErrors.map((m) => (
          <span key={m} className="fv-note fv-note--error" role="alert" data-words="exception">
            {m}
          </span>
        ))}
        {ranged && (
          // F-837: the colour range of the shown snapshot (F-808's), of the whole run, or fixed; icons only (G22)
          <div className="fv-range" role="group" aria-label="Colour range">
            <IconToggle icon="range-snapshot" label="Colour range of the shown snapshot" isSelected={rangeMode === 'snapshot'} onChange={() => chooseRange('snapshot')} isDisabled={!shown} />
            <IconToggle icon="range-run" label="Colour range of the whole run" isSelected={rangeMode === 'run'} onChange={() => chooseRange('run')} isDisabled={!shown} />
            <IconToggle icon="range-fixed" label="Fixed colour range" isSelected={rangeMode === 'fixed'} onChange={() => chooseRange('fixed')} isDisabled={!shown} />
          </div>
        )}
        <IconButton
          variant="ghost"
          icon="face-on"
          label={twoD && layers ? `View face on (along ${AXIS[layers.thin]})` : 'View face on (the nearest axis)'}
          onPress={faceOn}
          isDisabled={!shown}
          className="fv-face-on"
        />
        <IconButton variant="ghost" icon="image" label="Save the view as PNG…" onPress={() => void savePng().catch(() => undefined)} isDisabled={!shown} className="fv-save-png" />
        {runRangeError && (
          <span className="fv-note fv-note--error" role="alert" data-words="exception">
            {runRangeError}
          </span>
        )}
        {saved && (
          <span className={saved.ok ? 'fv-note' : 'fv-note fv-note--error'} role={saved.ok ? 'status' : 'alert'} data-words={saved.ok ? 'data' : 'exception'} data-saved={saved.ok ? saved.text : undefined}>
            {saved.text}
          </span>
        )}
        <span className="fv-tools-gap" />
        {backend && (
          // G22: the normal renderer is said to readers; the fallback, a loss, is shown
          <span className={backend.kind === 'webgpu' ? 'fv-backend sr-only' : 'fv-backend'} data-backend={backend.kind} data-words={backend.kind === 'webgpu' ? undefined : 'exception'}>
            {backend.kind === 'webgpu' ? 'Drawn by WebGPU' : `Drawn by WebGL 2 (fallback: ${backend.reason})`}
          </span>
        )}
      </div>
      <div
        className="fv-frame"
        role="application"
        aria-roledescription="field view"
        aria-label={`${quantity} on the boundary surface. ${
          twoD ? 'Arrow keys pan' : 'Arrow keys orbit'
        }, plus and minus zoom, F frames the domain, O turns the view face on, Home resets the view; the value at the centre is read out.`}
        tabIndex={shown ? 0 : -1}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => setHover((h) => (h?.by === 'pointer' && !pinned.current ? null : h))}
        onKeyDown={onKeyDown}
        data-ready={phase.kind === 'ready' && shown !== null && drawnValues === shown.values ? '' : undefined}
        data-snapshot={shown ? shown.snapshot : undefined}
        data-step={snap?.step}
        data-arrows={vectorsOn && engineCutAll && current(cutPic, cutArgs) && drawnNow ? engineCutAll.arrowCount : undefined}
        data-cut={cut && engineCut && current(cutPic, cutArgs) && drawnNow ? `${cut.axis} ${cut.at} ${cut.keep}` : undefined}
        data-iso={iso && current(isoPic, isoArgs) && drawnNow ? iso.triangles : undefined}
        data-streamlines={streamlines && engineLines && current(streamPic, streamArgs) && drawnNow ? engineLines.lines : undefined}
        data-range={drawnRange ? `${drawnRange.lo} ${drawnRange.hi}` : undefined}
        data-scale={bar ? bar.length : undefined}
        data-provenance="fv-readout"
      >
        <FieldCanvas
          ref={api}
          surface={opened?.surface ?? null}
          values={shown?.values ?? null}
          valuesPer={shown?.per ?? 'cell'}
          range={drawnRange}
          grid={grid}
          forceWebGL={forceWebGL}
          onBackend={setBackend}
          onMoved={setMoved}
          onView={onView}
          onDrawn={onDrawn}
          onGridState={setGridState}
          cut={cut}
          engineCut={engineCut}
          engineArrows={engineArrows}
          iso={iso}
          streamlines={streamlines}
          onPictureDrawn={setPictureDrawn}
          prepared={opened?.prepared ?? null}
          scalars={shown?.scalars ?? null}
          thin={twoD && layers ? layers.thin : null}
        />
        {(cut || isoOn || rake) && layers && shown && (
          <div className="fv-panels">
            {cut && <CutPanel cut={cut} layers={layers} onCut={setCut} />}
            {isoOn && drawnRange && (
              <IsoPanel level={isoLevel!} lo={drawnRange.lo} hi={drawnRange.hi} quantity={word} unit={unit} onLevel={(v) => setIsoLevel(Math.fround(v))} />
            )}
            {rake && <StreamPanel rake={rake} layers={layers} onRake={setRake} />}
          </div>
        )}
        {ranged && rangeMode === 'fixed' && shown && (
          // F-837: the fixed range's two ends; keys and pointer stay in the panel (not the orbit)
          <div className="fv-fixed" role="group" aria-label="Fixed colour range" onKeyDown={(e) => e.stopPropagation()} onPointerMove={(e) => e.stopPropagation()} onPointerDown={(e) => e.stopPropagation()}>
            <NumberField aria-label={`Bottom of the fixed range${unit ? `, ${unit}` : ''}`} value={fixedLo} onChange={setFixedLo} formatOptions={{ useGrouping: false, maximumSignificantDigits: 9 }} className="fv-fixed-field" />
            <NumberField aria-label={`Top of the fixed range${unit ? `, ${unit}` : ''}`} value={fixedHi} onChange={setFixedHi} formatOptions={{ useGrouping: false, maximumSignificantDigits: 9 }} className="fv-fixed-field" />
            {'problem' in fixed && (
              <span className="fv-note fv-note--error" data-words="exception">
                {fixed.problem}
              </span>
            )}
          </div>
        )}
        {bar && (
          // F-837: the scale bar of a face-on 2D view (the egui Studio's rule), in the picture Save PNG writes
          <div className="fv-scalebar" data-length={bar.length} title={LENGTH_UNIT_WHY}>
            <span className="fv-scalebar-bar" style={{ width: `${bar.px}px` }} aria-hidden="true" />
            <span data-words="data">{`${lengthText(bar.length)} ${LENGTH_UNIT}`}</span>
          </div>
        )}
        <WorkProgress scope={tagBase} />
        {moved && shown && <IconButton icon="reset-view" label="Reset view" onPress={reset} className="fv-reset" />}
        {hover?.by === 'keyboard' && <span className="fv-crosshair" aria-hidden="true" />}
        {/* F-832: a load's progress is WorkProgress's (above), shown after PROGRESS_AFTER_MS; this box is for errors */}
        {phase.kind === 'error' && (
          <div className="fv-status" role="alert" data-words="exception">
            <p>{phase.message}</p>
            <p className="fv-hint">{phase.hint}</p>
            <Button onPress={() => setAttempt((a) => a + 1)}>Try again</Button>
          </div>
        )}
      </div>
      {/* The legend, caption and readout keep their place while loading: the canvas never resizes when data arrives. */}
      <div className="fv-info">
        {shown && (
          <>
            {legend && drawnRange && <Legend lut={lut} quantity={word} unit={unit} lo={drawnRange.lo} hi={drawnRange.hi} />}
            {learner ? (
              // F-813: the learner's caption, plain at rest; the precise terms are said to readers and behind Explain
              <p className="fv-caption" data-caption="learner">
                <b data-words="data">{title ?? runName(shown.info.path)}</b> ·{' '}
                <span data-words="data">
                  {word}
                  {unit ? `, ${unit}` : ''}
                </span>
                <Quiet>{` (stored as ${quantity})`}</Quiet> · round{' '}
                <span data-words="data">
                  <Provenance subject={() => snapFact('step', snap?.step)}>{snap?.step}</Provenance>
                </span>
                <Quiet>
                  {' '}
                  (step {snap?.step}, stored picture: snapshot {shown.snapshot + 1} of {shown.info.snapshots.length}, t = {snap?.time}) · {rangeWords} ·{' '}
                  {shown.surface.cells.length} triangles of {shown.info.nCells} cells
                </Quiet>
                {outside && outside.outside > 0 && (
                  <span data-words="exception" data-outside={outside.outside}>
                    {` · ${outside.outside} of ${outside.of} surface cells outside the fixed range, drawn in its end colours`}
                  </span>
                )}
              </p>
            ) : (
            <p className="fv-caption">
              <b data-words="data">{title ?? runName(shown.info.path)}</b> ·{' '}
              <span data-words="data">
                {quantity}
                {unit ? `, ${unit}` : ''}
              </span>{' '}
              <Quiet>
                · snapshot {shown.snapshot + 1} of {shown.info.snapshots.length}
              </Quiet>{' '}
              · step{' '}
              <span data-words="data">
                <Provenance subject={() => snapFact('step', snap?.step)}>{snap?.step}</Provenance>
              </span>{' '}
              · t ={' '}
              <span data-words="data">
                <Provenance subject={() => snapFact('time', snap?.time)}>{snap?.time}</Provenance>
              </span>
              {/* G22: the colour map and the mesh size are said to readers (the legend shows the range) */}
              <Quiet>
                {' '}
                · {rangeWords} · {shown.surface.cells.length} triangles of {shown.info.nCells}{' '}
                cells
              </Quiet>
              {layers && (
                // F-837: the domain's size (the egui Studio's "domain 2 x 1 m"): the stored mesh's extents, m from the
                // engine's SI settings (the store names no unit), said in the tooltip
                <span className="fv-domain" data-words="data" data-domain={domainText(layers)} title={`Domain${layers.twoD ? ' (2D: the two in-plane extents)' : ''}. ${LENGTH_UNIT_WHY}`}>
                  {` · ${domainText(layers)} ${LENGTH_UNIT}`}
                </span>
              )}
              {outside && outside.outside > 0 && (
                <span data-words="exception" data-outside={outside.outside}>
                  {` · ${outside.outside} of ${outside.of} surface cells outside the fixed range, drawn in its end colours`}
                </span>
              )}
            </p>
            )}
            <div className="fv-readout" aria-live={hover?.by === 'keyboard' ? 'polite' : 'off'} data-pointer={pointerAt || undefined}>
              <span className="fv-readout-label">
                <span data-words="data">{word}</span>
                <Quiet>{` at the ${hover?.by === 'keyboard' ? 'centre' : 'cursor'}`}</Quiet>
                {unit ? <span data-words="data">{`, ${unit}`}</span> : ''}
              </span>
              {hover ? (
                <>
                  <output
                    className="fv-readout-value"
                    data-cell={hover.pick.cell}
                    data-triangle={hover.pick.triangle}
                    data-point={hover.pick.point ? hover.pick.point.join(' ') : undefined}
                  >
                    <Provenance
                      pkey="fv-readout"
                      onOpenChange={(o) => (pinned.current = o)}
                      subject={{
                        kind: 'number',
                        source: cellValue({
                          run: runName(shown.info.path),
                          quantity,
                          shown: f32Text(hover.pick.value),
                          value: hover.pick.value,
                          unit,
                          cell: hover.pick.cell,
                          triangle: hover.pick.triangle,
                          snapshot: shown.snapshot,
                          of: shown.info.snapshots.length,
                          step: snap?.step ?? 0,
                          time: snap?.time ?? null,
                        }),
                      }}
                    >
                      {f32Text(hover.pick.value)}
                    </Provenance>
                  </output>
                  {learner ? (
                    <Quiet>{`cell ${hover.pick.cell} · triangle ${hover.pick.triangle}`}</Quiet>
                  ) : (
                    <span className="fv-readout-where" data-words="data">
                      cell {hover.pick.cell} · triangle {hover.pick.triangle}
                    </span>
                  )}
                </>
              ) : (
                <Quiet>Point at the surface to read the stored value.</Quiet>
              )}
            </div>
          </>
        )}
      </div>
    </section>
  );
}
