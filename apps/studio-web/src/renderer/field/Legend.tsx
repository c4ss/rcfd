// The field legend (DESIGN.md Field view: an ink-ruled bar with ticks; the Data Owns the Field Rule): the bar is the
// colour table itself, its 256 entries written into a 256 x 1 canvas and stretched without smoothing, so every colour
// of the bar is a colour of the surface. Tick labels are rcfd-studio's (`{:.4e}`), in tabular figures; the unit sits in
// the title (the Tabular Figures Rule).
import { useEffect, useRef } from 'react';
import { Traced, type ProvenanceSubject } from '../provenance/index.js';
import { LUT_SIZE, tickLabel, ticks } from './colormap.js';

/** The ticks of a legend (`ticks` of ./colormap.ts gives five). */
export const TICKS = 5;
/** ADR 0795: the keys of a legend's ticks, as the view that shows it names them. */
export const tickKeys = (prefix: string): string => Array.from({ length: TICKS }, (_, i) => `${prefix}-tick-${i}`).join(' ');

/** A tick as its provenance needs it. */
export interface Tick {
  readonly index: number;
  readonly ticks: number;
  readonly value: number;
  readonly shown: string;
  readonly lo: number;
  readonly hi: number;
}

export interface LegendProps {
  readonly lut: Uint8Array;
  readonly quantity: string;
  readonly unit: string;
  readonly lo: number;
  readonly hi: number;
  /** ADR 0795: the provenance of each tick, and the prefix of its key. */
  readonly provenance: { readonly prefix: string; readonly of: (t: Tick) => ProvenanceSubject | null };
}

export function Legend({ lut, quantity, unit, lo, hi, provenance }: LegendProps) {
  const bar = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = bar.current;
    const ctx = c?.getContext('2d');
    if (!c || !ctx) return;
    ctx.putImageData(new ImageData(new Uint8ClampedArray(lut), LUT_SIZE, 1), 0, 0);
  }, [lut]);
  const title = unit ? `${quantity}, ${unit}` : quantity;
  const marks = ticks(lo, hi);
  return (
    <div className="fv-legend" role="group" aria-label={`Colour map viridis, ${title}, from ${tickLabel(lo)} to ${tickLabel(hi)}`}>
      <span className="fv-legend-title" data-words="data">
        {title}
      </span>
      <span className="fv-legend-scale">
        <canvas ref={bar} className="fv-legend-bar" width={LUT_SIZE} height={1} aria-hidden="true" data-legend-bar="" />
        <span className="fv-legend-ticks" aria-hidden="true" data-words="data">
          {marks.map((v, i) => (
            <span key={i}>
              <Traced pkey={`${provenance.prefix}-tick-${i}`} subject={() => provenance.of({ index: i, ticks: marks.length, value: v, shown: tickLabel(v), lo, hi })}>
                {tickLabel(v)}
              </Traced>
            </span>
          ))}
        </span>
      </span>
    </div>
  );
}
