// The field legend (DESIGN.md Field view: an ink-ruled bar with ticks; the Data Owns the Field Rule): the bar is the
// colour table itself, its 256 entries written into a 256 x 1 canvas and stretched without smoothing, so every colour
// of the bar is a colour of the surface. Tick labels are rcfd-studio's (`{:.4e}`), in tabular figures; the unit sits in
// the title (the Tabular Figures Rule).
import { useEffect, useRef } from 'react';
import { LUT_SIZE, tickLabel, ticks } from './colormap.js';

export interface LegendProps {
  readonly lut: Uint8Array;
  readonly quantity: string;
  readonly unit: string;
  readonly lo: number;
  readonly hi: number;
}

export function Legend({ lut, quantity, unit, lo, hi }: LegendProps) {
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
      <span className="fv-legend-title">{title}</span>
      <span className="fv-legend-scale">
        <canvas ref={bar} className="fv-legend-bar" width={LUT_SIZE} height={1} aria-hidden="true" data-legend-bar="" />
        <span className="fv-legend-ticks" aria-hidden="true">
          {marks.map((v, i) => (
            <span key={i}>{tickLabel(v)}</span>
          ))}
        </span>
      </span>
    </div>
  );
}
