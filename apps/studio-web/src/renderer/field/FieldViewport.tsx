// The field viewport (F-808; DESIGN.md Field view; guardrails G3, G8, G9, G21): one field of one snapshot of a store,
// drawn on the boundary surface of the stored mesh, with its legend, caption and the value under the cursor.
//
// Data comes only through the data door (../data/door.ts, F-804's window.studioData): open the store, its surface once
// per store, then the field. Every request carries this view's tag, and a superseded or cancelled load is cancelled
// by it (G21). What is shown traces to the store: the colours come from the stored values through the table of
// ./colormap.ts, the range is the min and max of the shown values (rcfd-studio's snapshot range), the readout is the
// stored value of the cell under the cursor, and the caption names the store, quantity, unit, snapshot, step and time.
//
// Timing marks (performance.mark / measure, read by scripts/measure-field.mjs for P7): `rcfd:field:open` when a store is
// opened, then the measure `rcfd:field:first-picture` when its first field is drawn, finished on the GPU and one
// animation frame has passed.
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import type { FieldInfo, StoreInfo, StoreRef, SurfaceData } from '../../shared/data.js';
import { cancelLoads, loadField, loadSurface, openStore } from '../data/door.js';
import { Button, Toggle } from '../ui/index.js';
import { colourRange, f32Text, lutBytes, type ColourRange } from './colormap.js';
import { FieldCanvas, type Backend, type FieldCanvasApi, type Pick } from './FieldCanvas.js';
import { CutPanel, DerivedControls, useArrows } from './DerivedControls.js';
import { availability, meshLayers, middleCut, type Cut } from './derived.js';
import { Legend } from './Legend.js';
import { shownRange } from './surface.js';
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
}

type Phase =
  | { readonly kind: 'loading'; readonly what: string }
  | { readonly kind: 'error'; readonly message: string; readonly hint: string }
  | { readonly kind: 'ready' };

interface Shown {
  readonly info: StoreInfo;
  readonly fieldInfo: FieldInfo;
  readonly surface: SurfaceData;
  readonly values: Float32Array;
  readonly range: ColourRange;
  readonly snapshot: number;
}

let views = 0;

/** F-810: how long a new snapshot's load keeps the shown picture without the loading notice (ms). */
const QUIET_MS = 300;

/** The run a store belongs to: the folder above `fields` (runs/<id>/fields), else the last segment. */
function runName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  const i = parts.lastIndexOf('fields');
  return (i > 0 ? parts[i - 1] : parts[parts.length - 1]) ?? path;
}

export function FieldViewport({ store, snapshot, field, forceWebGL = false, reopen = 0, onDrawn: onDrawnSnapshot }: FieldViewportProps) {
  const tagBase = useMemo(() => `field-view-${++views}`, []);
  const lut = useMemo(lutBytes, []);
  const [phase, setPhase] = useState<Phase>({ kind: 'loading', what: 'Opening the store' });
  const [opened, setOpened] = useState<{ key: string; info: StoreInfo; surface: SurfaceData } | null>(null);
  const [shown, setShown] = useState<Shown | null>(null);
  const [backend, setBackend] = useState<Backend | null>(null);
  const [moved, setMoved] = useState(false);
  const [grid, setGrid] = useState(false);
  const [gridState, setGridState] = useState<'off' | 'computing' | 'on'>('off');
  // F-809: the derived pictures (./derived.ts): rcfd-render's cut plane and the vector arrows
  const [cutAsked, setCut] = useState<Cut | null>(null);
  const [vectorsAsked, setVectors] = useState(false);
  const [pointerAt, setPointerAt] = useState('');
  const [hover, setHover] = useState<{ pick: Pick; by: 'pointer' | 'keyboard' } | null>(null);
  const [attempt, setAttempt] = useState(0);
  const api = useRef<FieldCanvasApi>(null);
  const tag = useRef('');
  const firstPicture = useRef(false);
  // F-810: while scrubbing, a new snapshot's load keeps the last picture without the loading notice for QUIET_MS
  const [quiet, setQuiet] = useState(false);
  const reopened = useRef(reopen);
  const reopening = useRef(false);
  const drawnFor = useRef<number | null>(null);
  const drawnCb = useRef(onDrawnSnapshot);
  drawnCb.current = onDrawnSnapshot;
  const storeKey = `${store.route}\u0000${store.path}`;
  const layers = useMemo(() => (opened ? meshLayers(opened.surface, opened.info.nCells) : null), [opened?.surface, opened?.info.nCells]);
  const derivedCtx = useMemo(
    () => (opened && layers ? { layers, fields: opened.info.fields, nCells: opened.info.nCells } : null),
    [opened?.info.fields, opened?.info.nCells, layers],
  );
  // a picture that does not apply to this store (another store was opened) is off
  const cut = derivedCtx && cutAsked && availability('cut', derivedCtx).ok ? cutAsked : null;
  const vectorsOn = !!derivedCtx && vectorsAsked && availability('vectors', derivedCtx).ok;
  const arrows = useArrows(store, shown?.snapshot ?? null, shown?.surface ?? null, layers, opened?.info.nCells ?? 0, vectorsOn, tagBase);

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
    void (async () => {
      const info = await openStore(store, { tag: t });
      if (!live) return;
      if (!info.ok) {
        setPhase({ kind: 'error', message: `The store could not be opened: ${info.error.message}`, hint: 'Check the folder holds store.rts and is inside a data root, then try again.' });
        return;
      }
      setPhase({ kind: 'loading', what: `Reading the boundary surface of ${info.result.nCells} cells` });
      const surface = await loadSurface(store, { tag: t });
      if (!live) return;
      if (!surface.ok) {
        setPhase({ kind: 'error', message: `The surface could not be read: ${surface.error.message}`, hint: 'The stored mesh may be damaged; the store’s run can be checked with rcfd.' });
        return;
      }
      setOpened({ key: storeKey, info: info.result, surface: surface.result });
    })();
    return () => {
      live = false;
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
    setPhase({ kind: 'loading', what: `Reading ${field} of snapshot ${snapshot + 1} of ${info.snapshots.length}` });
    // a picture is shown: keep it without the loading notice for a moment (scrubbing loads one field after another)
    setQuiet(true);
    const quietTimer = setTimeout(() => setQuiet(false), QUIET_MS);
    void (async () => {
      const r = await loadField(store, snapshot, field, { tag: t });
      if (!live) return;
      if (!r.ok) {
        setPhase({
          kind: 'error',
          message: r.error.code === 'cancelled' ? 'Loading was cancelled.' : `The field could not be read: ${r.error.message}`,
          hint: 'Try again to load it.',
        });
        return;
      }
      const values = r.result.values;
      if (values.length !== info.nCells) {
        setPhase({ kind: 'error', message: `The field has ${values.length} values for ${info.nCells} cells.`, hint: 'The store does not match its mesh.' });
        return;
      }
      const span = shownRange(surface.cells, values);
      if (!span) {
        setPhase({ kind: 'error', message: 'The surface has no triangles to draw.', hint: 'The stored mesh has no boundary faces.' });
        return;
      }
      drawnFor.current = snapshot;
      setShown({ info, fieldInfo: fi, surface, values, range: colourRange(span.lo, span.hi), snapshot });
      setPhase({ kind: 'ready' });
    })();
    return () => {
      live = false;
      clearTimeout(quietTimer);
      void cancelLoads(t);
    };
    // `store` is named by storeKey; `reopen` is read to tell a snapshot not read yet from one that does not exist
  }, [opened, storeKey, snapshot, field, tagBase, attempt]);

  const onDrawn = useCallback(() => {
    if (drawnFor.current !== null) drawnCb.current?.(drawnFor.current);
    if (!firstPicture.current) return;
    firstPicture.current = false;
    try {
      performance.measure('rcfd:field:first-picture', 'rcfd:field:open');
    } catch {
      // the mark was cleared by a newer open
    }
  }, []);

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const pick = api.current?.pickAt(e.clientX, e.clientY) ?? null;
    setHover(pick ? { pick, by: 'pointer' } : null);
    // F-809: where the readout was taken (the e2e tests wait for the readout of the point they moved to)
    setPointerAt(`${e.clientX},${e.clientY}`);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!api.current?.key(e.key)) return;
    e.preventDefault();
    const pick = api.current.pickCentre();
    setHover(pick ? { pick, by: 'keyboard' } : null);
  };
  const loading = phase.kind === 'loading';
  const fi = shown?.fieldInfo;
  const quantity = fi?.name ?? field;
  const unit = fi?.unit ?? '';
  const snap = shown ? shown.info.snapshots[shown.snapshot] : undefined;
  const reset = () => {
    api.current?.reset();
    setHover(null);
  };

  return (
    <section className="fv" aria-label={`Field view: ${quantity}`}>
      <div className="fv-tools">
        <Toggle isSelected={grid} onChange={setGrid} isDisabled={!shown}>
          Mesh grid
        </Toggle>
        {grid && gridState === 'computing' && <span className="fv-note">Finding the face edges…</span>}
        <DerivedControls
          ctx={derivedCtx}
          cut={cut}
          onCut={setCut}
          vectors={vectorsOn}
          onVectors={setVectors}
          startCut={layers ? () => middleCut(layers, 0) : null}
        />
        {arrows.kind === 'error' && (
          <span className="fv-note fv-note--error" role="alert">
            {arrows.message}
          </span>
        )}
        <span className="fv-tools-gap" />
        {backend && (
          <span className="fv-backend" data-backend={backend.kind}>
            {backend.kind === 'webgpu' ? 'Drawn by WebGPU' : `Drawn by WebGL 2 (fallback: ${backend.reason})`}
          </span>
        )}
      </div>
      <div
        className="fv-frame"
        role="application"
        aria-roledescription="field view"
        aria-label={`${quantity} on the boundary surface. Arrow keys orbit, plus and minus zoom, Home resets the view; the value at the centre is read out.`}
        tabIndex={shown ? 0 : -1}
        onPointerMove={onPointerMove}
        onPointerLeave={() => setHover((h) => (h?.by === 'pointer' ? null : h))}
        onKeyDown={onKeyDown}
        data-ready={phase.kind === 'ready' ? '' : undefined}
        data-snapshot={shown ? shown.snapshot : undefined}
        data-step={snap?.step}
        data-arrows={arrows.kind === 'on' && arrows.snapshot === shown?.snapshot ? arrows.glyphs.cells.length : undefined}
      >
        <FieldCanvas
          ref={api}
          surface={opened?.surface ?? null}
          values={shown?.values ?? null}
          range={shown?.range ?? null}
          grid={grid}
          forceWebGL={forceWebGL}
          onBackend={setBackend}
          onMoved={setMoved}
          onDrawn={onDrawn}
          onGridState={setGridState}
          cut={cut}
          arrows={arrows.kind === 'on' && arrows.snapshot === shown?.snapshot ? arrows.glyphs.segments : null}
        />
        {cut && layers && shown && <CutPanel cut={cut} layers={layers} onCut={setCut} />}
        {moved && shown && (
          <Button className="fv-reset" onPress={reset}>
            Reset view
          </Button>
        )}
        {hover?.by === 'keyboard' && <span className="fv-crosshair" aria-hidden="true" />}
        {phase.kind !== 'ready' && !(phase.kind === 'loading' && quiet && shown) && (
          <div className="fv-status" role={phase.kind === 'error' ? 'alert' : 'status'}>
            {loading ? (
              <>
                <p>{phase.what}…</p>
                <Button onPress={() => void cancelLoads(tag.current)}>Cancel</Button>
              </>
            ) : (
              <>
                <p>{phase.message}</p>
                <p className="fv-hint">{phase.hint}</p>
                <Button onPress={() => setAttempt((a) => a + 1)}>Try again</Button>
              </>
            )}
          </div>
        )}
      </div>
      {/* The legend, caption and readout keep their place while loading: the canvas never resizes when data arrives. */}
      <div className="fv-info">
        {shown && (
          <>
            <Legend lut={lut} quantity={quantity} unit={unit} lo={shown.range.lo} hi={shown.range.hi} />
            <p className="fv-caption">
              <b>{runName(shown.info.path)}</b> · {quantity}
              {unit ? `, ${unit}` : ''} · snapshot {shown.snapshot + 1} of {shown.info.snapshots.length} · step {snap?.step} · t = {snap?.time} ·
              viridis, range of the shown values · {shown.surface.cells.length} triangles of {shown.info.nCells} cells
            </p>
            <div className="fv-readout" aria-live={hover?.by === 'keyboard' ? 'polite' : 'off'} data-pointer={pointerAt || undefined}>
              <span className="fv-readout-label">{`${quantity} at the ${hover?.by === 'keyboard' ? 'centre' : 'cursor'}${unit ? `, ${unit}` : ''}`}</span>
              {hover ? (
                <>
                  <output
                    className="fv-readout-value"
                    data-cell={hover.pick.cell}
                    data-triangle={hover.pick.triangle}
                    data-point={hover.pick.point ? hover.pick.point.join(' ') : undefined}
                  >
                    {f32Text(hover.pick.value)}
                  </output>
                  <span className="fv-readout-where">
                    cell {hover.pick.cell} · triangle {hover.pick.triangle}
                  </span>
                </>
              ) : (
                <span className="fv-readout-where">Point at the surface to read the stored value.</span>
              )}
            </div>
          </>
        )}
      </div>
    </section>
  );
}
