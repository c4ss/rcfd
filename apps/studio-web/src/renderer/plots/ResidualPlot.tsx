// The residual plot (F-811; DESIGN.md Time panel; the prototype's ResidualPlot for the look): every residual series of
// one run on a log scale, drawn by three.js, with its decades, its legend (each line's own dash and name, and its last
// value) and its range in words. A run without residuals says so with its pictogram; it never shows another run's lines.
import { useMemo } from 'react';
import type { FeedSnapshot } from './feed.js';
import { NoSeries, Swatch, fmtResidual, notDrawn, xCaptionOf } from './parts.js';
import { PlotFrame, type Drawn } from './PlotFrame.js';
import { tickLabel } from './range.js';
import type { SeriesData } from './series.js';
import { isResidual, residualOrder, residualStyles, type LineStyle } from './style.js';
import type { RunStatus } from './useRunFeed.js';
import { noSeriesSentence, pictogramOf, runName } from './words.js';
// F-828: the latest value of each line opens its provenance (the session, monitors.get, the engine)
import { Provenance, sessionSeriesValue } from '../provenance/index.js';

export interface ResidualPlotProps {
  readonly session: string;
  readonly feed: FeedSnapshot | null;
  readonly status: RunStatus | null;
  /** Height of the drawing in CSS pixels. */
  readonly height?: number;
}

export function ResidualPlot({ session, feed, status, height = 120 }: ResidualPlotProps) {
  const series = useMemo(() => {
    const out = new Map<string, { data: SeriesData; style: LineStyle }>();
    if (!feed || feed.status !== 'ready') return out;
    const names = residualOrder([...feed.series.keys()].filter(isResidual));
    const styles = residualStyles(names);
    for (const n of names) out.set(n, { data: feed.series.get(n)!, style: styles.get(n)! });
    return out;
    // the names change only with the feed's version
  }, [feed, feed?.version]);

  const sentence = noSeriesSentence(feed, status, session, 'residuals', series.size > 0);
  const title = `Residuals · ${runName(session)}`;
  if (sentence !== null) {
    return (
      <section className="plot plot--residuals" aria-label={title} data-run={session}>
        <h3 className="plot-head">{title}</h3>
        <NoSeries picto={pictogramOf(status)} sentence={sentence} height={height} />
      </section>
    );
  }
  const xCaption = xCaptionOf(feed!.xLabel);
  return (
    <section className="plot plot--residuals" aria-label={title} data-run={session}>
      <h3 className="plot-head">
        {title} <small>log scale</small>
      </h3>
      <PlotFrame series={series} version={feed!.version} log xCaption={xCaption} height={height}>
        {(d) => <ResidualLegend session={session} xLabel={feed!.xLabel} drawn={d} series={series} xCaption={xCaption} />}
      </PlotFrame>
      {feed!.lastError && <p className="plot-note">The last update failed and the lines show what was read before: {feed!.lastError}</p>}
    </section>
  );
}

function ResidualLegend({
  session,
  xLabel,
  drawn,
  series,
  xCaption,
}: {
  session: string;
  xLabel: string | null;
  drawn: Drawn | null;
  series: ReadonlyMap<string, { data: SeriesData; style: LineStyle }>;
  xCaption: string;
}) {
  const range =
    drawn && !drawn.extent.empty
      ? `log scale, 1e${drawn.y.lo} to 1e${drawn.y.hi}; ${xCaption} ${tickLabel(drawn.x.lo, drawn.x.hi - drawn.x.lo)} to ${tickLabel(drawn.x.hi, drawn.x.hi - drawn.x.lo)}`
      : 'nothing drawn: no residual is a positive finite number';
  return (
    <>
      <p className="plot-caption">{range}</p>
      <ul className="plot-legend" aria-label="Residual series">
        {[...series].map(([name, { data, style }]) => {
          const e = drawn?.perSeries.get(name);
          const last = data.length > 0 ? data.yAt(data.length - 1) : Number.NaN;
          return (
            <li key={name}>
              <Swatch style={style} />
              <span className="plot-legend-name">{name}</span>
              <span className="plot-legend-value">
                {data.length > 0 ? (
                  <Provenance
                    subject={{
                      kind: 'session',
                      source: sessionSeriesValue({ session, name, shown: fmtResidual(last), value: last, x: data.xAt(data.length - 1), xLabel: xLabel ?? 'x', unit: data.unit, residual: true }),
                    }}
                  >
                    {fmtResidual(last)}
                  </Provenance>
                ) : (
                  'no samples'
                )}
              </span>
              {e && e.skipped > 0 && <span className="plot-legend-note">{notDrawn(e.skipped, 'zero, negative or not finite')}</span>}
            </li>
          );
        })}
      </ul>
    </>
  );
}
