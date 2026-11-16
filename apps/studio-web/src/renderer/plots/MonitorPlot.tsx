// The monitor chart (F-811; DESIGN.md Time panel, the monitor column; the prototype's MonitorChart for the look): one
// monitor series of one run (a report value such as a force, or a solver quantity such as dt), its latest value in the
// display size, and the chart on a linear scale whose range holds every value drawn. A run with more than one monitor
// series offers them in a select; a run with none says so with its pictogram, and a series of one run is never shown
// for another (the choice belongs to the run: the component is keyed by run where it is used).
import { useMemo, useState } from 'react';
import { Select } from '../ui/index.js';
import type { FeedSnapshot } from './feed.js';
import { NoSeries, atAbscissa, fmt, notDrawn, xCaptionOf } from './parts.js';
import { PlotFrame } from './PlotFrame.js';
import { tickLabel } from './range.js';
import type { SeriesData } from './series.js';
import { isResidual, MONITOR_STYLE, type LineStyle } from './style.js';
import type { RunStatus } from './useRunFeed.js';
import { noSeriesSentence, pictogramOf, runName } from './words.js';

export interface MonitorPlotProps {
  readonly session: string;
  readonly feed: FeedSnapshot | null;
  readonly status: RunStatus | null;
  readonly height?: number;
}

export function MonitorPlot({ session, feed, status, height = 64 }: MonitorPlotProps) {
  const names = useMemo(() => (feed && feed.status === 'ready' ? [...feed.series.keys()].filter((n) => !isResidual(n)).sort() : []), [feed]);
  const [chosen, setChosen] = useState<string | null>(null);
  const name = chosen !== null && names.includes(chosen) ? chosen : (names[0] ?? null);
  const data = name !== null ? feed!.series.get(name)! : null;
  const series = useMemo(() => {
    const out = new Map<string, { data: SeriesData; style: LineStyle }>();
    if (name !== null && data) out.set(name, { data, style: MONITOR_STYLE });
    return out;
  }, [name, data]);

  const title = `Monitor · ${runName(session)}`;
  const sentence = noSeriesSentence(feed, status, session, 'monitor values', names.length > 0);
  if (sentence !== null || !data || name === null) {
    return (
      <section className="plot plot--monitor" aria-label={title} data-run={session}>
        <h3 className="plot-head">{title}</h3>
        <NoSeries picto={pictogramOf(status)} sentence={sentence ?? 'No monitor values.'} height={height} />
      </section>
    );
  }
  const xCaption = xCaptionOf(feed!.xLabel);
  const last = data.length - 1;
  const label = data.unit ? `${name}, ${data.unit}` : name;
  return (
    <section className="plot plot--monitor" aria-label={title} data-run={session} data-series={name}>
      <h3 className="plot-head">
        {title} <small>{label}</small>
      </h3>
      {names.length > 1 && (
        <Select aria-label="Monitor series" className="plot-select" options={names.map((n) => ({ id: n, label: n }))} selected={name} onChange={setChosen} />
      )}
      <div className="plot-readout">
        <small>{last >= 0 ? `${label} at ${atAbscissa(feed!.xLabel, data.xAt(last))}, the latest sample` : label}</small>
        <div className="plot-readout-value">{last >= 0 ? fmt(data.yAt(last)) : 'no samples'}</div>
      </div>
      <PlotFrame series={series} version={feed!.version} log={false} xCaption={xCaption} height={height}>
        {(d) => {
          const e = d?.perSeries.get(name);
          return (
            <>
              <p className="plot-caption">
                {d && !d.extent.empty
                  ? `${xCaption} ${tickLabel(d.x.lo, d.x.hi - d.x.lo)} to ${tickLabel(d.x.hi, d.x.hi - d.x.lo)} · ${label} ${tickLabel(d.y.lo, d.y.hi - d.y.lo)} to ${tickLabel(d.y.hi, d.y.hi - d.y.lo)}`
                  : `nothing drawn: no value of ${name} is a finite number`}
              </p>
              {e && e.skipped > 0 && <p className="plot-note">{notDrawn(e.skipped, 'not finite')}</p>}
            </>
          );
        }}
      </PlotFrame>
      {feed!.lastError && <p className="plot-note">The last update failed and the chart shows what was read before: {feed!.lastError}</p>}
    </section>
  );
}
