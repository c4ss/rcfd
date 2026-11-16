// The frame of one line plot (F-811): the three.js canvas (linePlot.ts), its hairline grid and its tick labels in HTML,
// and the range rule: the axes are computed from what is drawn, so they hold every value drawn (range.ts). The
// figure carries its range, counts and backend as data attributes, which the tests read back.
import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { LinePlot, PLOT_INSET, type Backend } from './linePlot.js';
import { Extent, linearAxis, logAxis, type Axis } from './range.js';
import type { SeriesData } from './series.js';
import type { LineStyle } from './style.js';

export interface PlotFrameProps {
  /** The series to draw, by name; the map's identity or `version` changes when they change. */
  readonly series: ReadonlyMap<string, { data: SeriesData; style: LineStyle }>;
  readonly version: number;
  readonly log: boolean;
  /** The abscissa's caption ("iteration", "time, s"). */
  readonly xCaption: string;
  /** Height of the drawing area in CSS pixels. */
  readonly height: number;
  /** What stands under the drawing (caption, legend, words), from what is drawn (null: nothing drawn yet). */
  readonly children?: (d: Drawn | null) => ReactNode;
  /**
   * F-810: a fixed abscissa axis (the time panel's shared axis), used when it holds every abscissa drawn; otherwise the
   * axis is computed from what is drawn, as without it, so the range rule holds either way.
   */
  readonly xAxis?: Axis;
  /** F-810: the abscissa of the cursor (the scrubber's step), drawn as a line across the plot; null or absent: none. */
  readonly cursor?: number | null;
}

export interface Drawn {
  readonly x: Axis;
  readonly y: Axis;
  readonly extent: Extent;
  /** What is drawn of each series (for the legend's counts). */
  readonly perSeries: ReadonlyMap<string, Extent>;
}

type RendererState = { kind: 'starting' } | { kind: 'ready'; backend: Backend } | { kind: 'failed'; reason: string };

const pct = (v: number, a: Axis) => ((v - a.lo) / (a.hi - a.lo)) * 100;

export function PlotFrame({ series, version, log, xCaption, height, children, xAxis, cursor = null }: PlotFrameProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<LinePlot | null>(null);
  const [renderer, setRenderer] = useState<RendererState>({ kind: 'starting' });
  const [drawn, setDrawn] = useState<Drawn | null>(null);
  /** Width of the drawing in CSS pixels, rounded to 50 px (the number of x ticks follows it). */
  const [width, setWidth] = useState(0);

  // one LinePlot per mounted frame (made before the first sync below); its renderer starts asynchronously and never
  // blocks the page (G21)
  useLayoutEffect(() => {
    const canvas = canvasRef.current!;
    const area = areaRef.current!;
    const plot = new LinePlot(canvas, log);
    plotRef.current = plot;
    const size = () => {
      const r = area.getBoundingClientRect();
      plot.resize(r.width, r.height, window.devicePixelRatio || 1);
      setWidth(Math.round(r.width / 50) * 50);
      plot.requestDraw();
    };
    size();
    const ro = new ResizeObserver(size);
    ro.observe(area);
    let alive = true;
    plot.init().then(
      (backend) => {
        if (!alive) return;
        setRenderer({ kind: 'ready', backend });
        size();
      },
      (e: unknown) => {
        if (alive) setRenderer({ kind: 'failed', reason: e instanceof Error ? e.message : String(e) });
      },
    );
    return () => {
      alive = false;
      ro.disconnect();
      plot.dispose();
      plotRef.current = null;
    };
  }, [log]);

  // bring the drawing up to date and compute the axes from what is drawn
  useLayoutEffect(() => {
    const plot = plotRef.current;
    if (!plot) return;
    const extent = plot.sync(series);
    const perSeries = new Map<string, Extent>();
    for (const name of series.keys()) perSeries.set(name, plot.extentOf(name) ?? new Extent());
    if (extent.empty) {
      setDrawn({ x: linearAxis(0, 1), y: log ? logAxis(0, 1) : linearAxis(0, 1), extent, perSeries });
      return;
    }
    const x =
      xAxis && xAxis.lo <= extent.xlo && extent.xhi <= xAxis.hi ? xAxis : linearAxis(extent.xlo, extent.xhi, Math.max(2, Math.min(10, Math.round(width / 110))));
    const y = log ? logAxis(extent.ylo, extent.yhi) : linearAxis(extent.ylo, extent.yhi, 3);
    plot.setRange({ xlo: x.lo, xhi: x.hi, ylo: y.lo, yhi: y.hi });
    plot.requestDraw();
    setDrawn({ x, y, extent, perSeries });
    // `version` tells a change inside the same map
  }, [series, version, log, renderer, width, xAxis]);

  const shown = drawn && !drawn.extent.empty ? drawn : null;
  const inset = PLOT_INSET;
  return (
    <div
      className="plot-frame"
      data-scale={log ? 'log' : 'linear'}
      data-backend={renderer.kind === 'ready' ? renderer.backend : renderer.kind}
      data-range={shown ? `${shown.x.lo} ${shown.x.hi} ${shown.y.lo} ${shown.y.hi}` : ''}
      data-drawn={drawn?.extent.drawn ?? 0}
      data-skipped={drawn?.extent.skipped ?? 0}
    >
      <div className="plot-yticks" aria-hidden="true" style={{ height }}>
        <div className="plot-ticks-inner" style={{ top: inset.top, bottom: inset.bottom }}>
          {shown?.y.ticks.map((t, i) => (
            <span key={t} className="plot-ytick" style={{ bottom: `${pct(t, shown.y)}%` }}>
              {shown.y.labels[i]}
            </span>
          ))}
        </div>
      </div>
      <div className="plot-area" ref={areaRef} style={{ height }}>
        <div className="plot-grid" aria-hidden="true" style={{ top: inset.top, right: inset.right, bottom: inset.bottom, left: inset.left }}>
          {shown?.y.ticks.map((t) => (
            <i key={t} className="plot-gridline" style={{ bottom: `${pct(t, shown.y)}%` }} />
          ))}
        </div>
        <canvas ref={canvasRef} className="plot-canvas" aria-hidden="true" />
        {shown && cursor !== null && cursor >= shown.x.lo && cursor <= shown.x.hi && (
          <i
            className="plot-cursor"
            aria-hidden="true"
            data-cursor={cursor}
            style={{ left: `calc(${inset.left}px + (100% - ${inset.left + inset.right}px) * ${(cursor - shown.x.lo) / (shown.x.hi - shown.x.lo)})` }}
          />
        )}
        {renderer.kind === 'failed' && <p className="plot-failed">This plot cannot be drawn here: {renderer.reason}. The values are listed under it.</p>}
      </div>
      <div className="plot-xticks" aria-hidden="true">
        <div className="plot-ticks-inner" style={{ left: inset.left, right: inset.right }}>
          {shown?.x.ticks.map((t, i) => (
            <span key={t} className="plot-xtick" style={{ left: `${pct(t, shown.x)}%` }}>
              {shown.x.labels[i]}
            </span>
          ))}
        </div>
        <span className="plot-xcaption">{xCaption}</span>
      </div>
      {children && <div className="plot-under">{children(drawn)}</div>}
    </div>
  );
}
