// The residual and monitor plots (F-811). A surface shows a run's plots with:
//
//   const view = useRunFeed({ instance, session });          // monitors.get once, then by increments; run.status
//   <ResidualPlot session={session} feed={view.feed} status={view.status} />
//   <MonitorPlot key={runKey(run)} session={session} feed={view.feed} status={view.status} />
//
// The data layer (feed.ts, series.ts, range.ts) has no DOM and no engine call of its own; linePlot.ts draws on three.js.
import './plots.css';

export { MonitorFeed, runKey, type FeedSnapshot, type FeedStats, type FeedStatus, type MonitorsGet, type RunRef } from './feed.js';
export { MonitorPlot } from './MonitorPlot.js';
export { ResidualPlot } from './ResidualPlot.js';
export { useRunFeed, type RunStatus, type RunView } from './useRunFeed.js';
