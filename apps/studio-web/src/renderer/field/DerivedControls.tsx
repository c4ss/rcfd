// The derived-picture controls of the field viewport (F-809; guardrails G3, G6, G8, G22): one control per derived
// picture (cut plane, vectors, iso-surface, streamlines), each a pictogram whose name is its accessible name and
// tooltip. One that does not apply to this store is a dashed, focusable chip that says why in its accessible name and
// in the tooltip shown on hover and keyboard focus, not as standing text (G22). When the cut plane is on, a small panel
// in the frame's top left corner sets its axis, its position (a value to act on) and the side that is kept.
//
// The arrows' data comes through the data door like the field (useArrows): the stored velocity components of the shown
// snapshot, with the view's tag so that a superseded load is cancelled (G21).
import { useEffect, useMemo, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { Focusable, ToggleButton, TooltipTrigger } from 'react-aria-components';
import type { StoreRef, SurfaceData } from '../../shared/data.js';
import { cancelLoads, loadField } from '../data/door.js';
import { IconButton, SegmentedToggle, Slider, Tooltip } from '../ui/index.js';
import {
  availability,
  AXIS_NAME,
  COMPONENT_FIELD,
  DERIVED_ICON,
  DERIVED_KINDS,
  DERIVED_NAME,
  vectorComponents,
  vectorGlyphs,
  type Axis,
  type Cut,
  type DerivedContext,
  type DerivedKind,
  type Glyphs,
  type Layers,
} from './derived.js';

function DerivedIcon({ kind }: { kind: DerivedKind }) {
  return (
    <svg className="ui-icon" width={14} height={14} viewBox="0 0 14 14" aria-hidden="true" focusable="false" data-icon={`derived-${kind}`}>
      <path d={DERIVED_ICON[kind]} />
    </svg>
  );
}

export interface DerivedControlsProps {
  /** Null until the store and its surface are read (every control is then unavailable, saying so). */
  readonly ctx: DerivedContext | null;
  readonly cut: Cut | null;
  readonly onCut: (cut: Cut | null) => void;
  readonly vectors: boolean;
  readonly onVectors: (on: boolean) => void;
  /** The cut a new cut plane starts from. */
  readonly startCut: (() => Cut) | null;
}

/** The controls in the viewport's tool row. */
export function DerivedControls({ ctx, cut, onCut, vectors, onVectors, startCut }: DerivedControlsProps) {
  const ready: DerivedContext = ctx ?? { layers: null, fields: [], nCells: 0 };
  return (
    <div className="fv-derived" role="group" aria-label="Derived pictures">
      {DERIVED_KINDS.map((kind) => {
        const a = availability(kind, ready);
        const name = DERIVED_NAME[kind];
        if (!a.ok) {
          const words = `${name}, unavailable: ${a.reason}`;
          return (
            <TooltipTrigger key={kind} delay={300}>
              <Focusable>
                <span className="ui-unavailable fv-derived-off" role="button" aria-disabled="true" tabIndex={0} aria-label={words} data-derived={kind}>
                  <DerivedIcon kind={kind} />
                </span>
              </Focusable>
              <Tooltip>{words}</Tooltip>
            </TooltipTrigger>
          );
        }
        const on = kind === 'cut' ? cut !== null : vectors;
        const set = (v: boolean) => {
          if (kind === 'cut') onCut(v && startCut ? startCut() : null);
          else onVectors(v);
        };
        return (
          <TooltipTrigger key={kind} delay={600}>
            <ToggleButton className="ui-segment ui-segment--single fv-derived-on" aria-label={name} isSelected={on} onChange={set} data-derived={kind}>
              <DerivedIcon kind={kind} />
            </ToggleButton>
            <Tooltip>{name}</Tooltip>
          </TooltipTrigger>
        );
      })}
    </div>
  );
}

const AXES = AXIS_NAME.map((n, i) => ({ id: String(i), label: n }));

/** The cut plane's panel in the frame: axis, position, side kept. Keys and pointer stay in the panel (not the orbit). */
export function CutPanel({ cut, layers, onCut }: { cut: Cut; layers: Layers; onCut: (cut: Cut) => void }) {
  const lo = layers.min[cut.axis];
  const hi = layers.max[cut.axis];
  const keep = (e: KeyboardEvent | PointerEvent) => e.stopPropagation();
  return (
    <div className="fv-cut" role="group" aria-label="Cut plane" onKeyDown={keep} onPointerMove={keep} onPointerDown={keep}>
      <SegmentedToggle
        aria-label="Cut plane normal axis"
        options={AXES}
        selected={String(cut.axis)}
        onChange={(id) => {
          const axis = Number(id) as Axis;
          onCut({ axis, at: Math.fround(0.5 * (layers.min[axis] + layers.max[axis])), keep: cut.keep });
        }}
      />
      <Slider
        className="fv-cut-slider"
        label={`Plane at ${AXIS_NAME[cut.axis]}`}
        minValue={lo}
        maxValue={hi}
        step={(hi - lo) / 200 || 1}
        value={cut.at}
        formatOptions={{ maximumSignificantDigits: 6 }}
        onChange={(v) => onCut({ ...cut, at: Math.fround(v) })}
      />
      <IconButton
        icon="sort"
        label={cut.keep === 'below' ? `Keep the side above the plane (now below, ${AXIS_NAME[cut.axis]} up to the plane)` : `Keep the side below the plane (now above)`}
        onPress={() => onCut({ ...cut, keep: cut.keep === 'below' ? 'above' : 'below' })}
      />
    </div>
  );
}

export type ArrowsState =
  | { readonly kind: 'off' }
  | { readonly kind: 'loading' }
  | { readonly kind: 'error'; readonly message: string }
  | { readonly kind: 'on'; readonly glyphs: Glyphs; readonly snapshot: number };

/**
 * The arrows of one snapshot: the stored velocity components through the data door, then `vectorGlyphs` (T3). Null
 * inputs or `on` false give `off`.
 */
export function useArrows(
  store: StoreRef,
  snapshot: number | null,
  surface: SurfaceData | null,
  layers: Layers | null,
  nCells: number,
  on: boolean,
  tagBase: string,
  /** F-832: where the arrows are made (the field worker); without it, here. */
  glyphsOf?: (surface: SurfaceData, components: readonly (Float32Array | null)[], layers: Layers, nCells: number) => Promise<Glyphs>,
): ArrowsState {
  const [state, setState] = useState<ArrowsState>({ kind: 'off' });
  const storeKey = `${store.route}\u0000${store.path}`;
  const need = useMemo(() => (layers ? vectorComponents(layers) : []), [layers]);
  useEffect(() => {
    if (!on || snapshot === null || !surface || !layers) {
      setState({ kind: 'off' });
      return;
    }
    let live = true;
    const tag = `${tagBase}-arrows-${snapshot}`;
    setState((s) => (s.kind === 'on' ? s : { kind: 'loading' }));
    void (async () => {
      const comps: (Float32Array | null)[] = [null, null, null];
      for (const k of need) {
        const r = await loadField(store, snapshot, COMPONENT_FIELD[k], { tag });
        if (!live) return;
        if (!r.ok) {
          setState({ kind: 'error', message: `${COMPONENT_FIELD[k]} could not be read: ${r.error.message}` });
          return;
        }
        if (r.result.values.length !== nCells) {
          setState({ kind: 'error', message: `${COMPONENT_FIELD[k]} has ${r.result.values.length} values for ${nCells} cells.` });
          return;
        }
        comps[k] = r.result.values;
      }
      let glyphs: Glyphs;
      try {
        glyphs = glyphsOf ? await glyphsOf(surface, comps, layers, nCells) : vectorGlyphs(surface, comps, layers, nCells);
      } catch (e) {
        if (!live) return;
        setState({ kind: 'error', message: `The arrows could not be made: ${e instanceof Error ? e.message : String(e)}` });
        return;
      }
      if (!live) return;
      setState({ kind: 'on', glyphs, snapshot });
    })();
    return () => {
      live = false;
      void cancelLoads(tag);
    };
    // `store` is named by storeKey
  }, [storeKey, snapshot, surface, layers, nCells, on, tagBase, need, glyphsOf]);
  return state;
}
