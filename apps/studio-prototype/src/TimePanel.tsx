import { Slider, SliderTrack, SliderThumb, Button, ToggleButton, Label } from 'react-aria-components';
import { RUNS, SNAPSHOTS, T_END, DT_SNAPSHOT, SERIES, type Run, type RunSeries } from './data';
import { Icon, Pictogram } from './Pictogram';

interface Props {
  selected: string;
  liveIteration: number;
  snapshot: number;
  available: number;
  onScrub: (k: number) => void;
  follow: boolean;
  onFollow: (on: boolean) => void;
  playing: boolean;
  onPlay: (on: boolean) => void;
}

const TICKS = [0, 5, 10, 15, 20];
const LOG_TOP = -1;
const LOG_BOTTOM = -7;

function pct(t: number) { return `${(t / T_END) * 100}%`; }

/** Where a band ends and what happened there, as a pictogram and a word (the THESIS reaches the bands too). */
function bandEvent(run: Run, liveIteration: number, liveT: number) {
  switch (run.state) {
    case 'converged': return { t: run.endTime, text: `converged · ${run.endTime.toFixed(1)} s` };
    case 'finished': return { t: run.endTime, text: `finished · ${run.endTime.toFixed(1)} s` };
    case 'failed': return { t: run.endTime, text: `failed · it ${run.iterations}` };
    case 'running': return { t: liveT, text: `running · it ${liveIteration}` };
    default: return { t: 0, text: 'queued · not started' };
  }
}

function Band({ run, focus, liveIteration, liveT }: { run: Run; focus: boolean; liveIteration: number; liveT: number }) {
  const ev = bandEvent(run, liveIteration, liveT);
  // After the band's end on the white ground (or over the pending hatch) when there is room; otherwise inside the
  // end with no chip, so the label never hides the band's own geometry.
  const labelLeft = ev.t < 16;
  return (
    <div className={`band${focus ? ' focus' : ' faded'}`}>
      {run.phases.map((p0, i) => {
        const p = run.state === 'running' ? { ...p0, to: liveT } : p0;
        return <span key={i} className={`seg seg-${run.state === 'running' ? 'running' : p.kind}`}
          style={{ left: pct(p.from), width: pct(p.to - p.from), animationDelay: `${i * 90}ms` }} />;
      })}
      {run.state === 'running' && (
        <span className="seg seg-pending" style={{ left: pct(liveT), width: pct(T_END - liveT) }}><em>not computed yet</em></span>
      )}
      <span className={`band-event ev-${run.state}${labelLeft ? ' after' : ' inside'}`}
        style={labelLeft ? { left: `calc(${pct(ev.t)} + 6px)` } : { right: `calc(${pct(T_END - ev.t)} + 6px)` }}>
        <Pictogram state={run.state} size={12} />{ev.text}
      </span>
    </div>
  );
}

function ResidualPlot({ series, last, running }: { series: RunSeries; last: number; running: boolean }) {
  const W = 1000, H = 100;
  const y = (v: number) => ((Math.min(LOG_TOP, Math.max(LOG_BOTTOM, v)) - LOG_TOP) / (LOG_BOTTOM - LOG_TOP)) * H;
  const lines = (['continuity', 'u', 'p'] as const).map((key) => {
    let d = '';
    for (let k = 0; k <= last; k++) {
      const r = series.residuals!(k)[key];
      d += `${k === 0 ? 'M' : 'L'}${((k / SNAPSHOTS) * W).toFixed(1)} ${y(r).toFixed(1)}`;
    }
    return { key, d };
  });
  const lastT = last * DT_SNAPSHOT;
  return (
    <div className="resid-wrap">
      <svg className="resid" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img"
        aria-label={`Residuals, log scale 1e-1 to 1e-7, up to t = ${lastT.toFixed(1)} s`}>
        {[-1, -3, -5, -7].map((v) => <line key={v} x1="0" x2={W} y1={y(v)} y2={y(v)} className="gridline" />)}
        {lines.map((s) => <path key={s.key} d={s.d} className={`res res-${s.key}`} vectorEffect="non-scaling-stroke" />)}
      </svg>
      {running && <div className="resid-veil" style={{ left: pct(lastT) }}><span>not computed yet</span></div>}
    </div>
  );
}

function MonitorChart({ series, snapshot }: { series: RunSeries; snapshot: number }) {
  const m = series.monitor!;
  const W = 240, H = 64, t1 = snapshot * DT_SNAPSHOT, t0 = Math.max(0, t1 - 4);
  const vals = Array.from({ length: 161 }, (_, i) => m.at(t0 + ((t1 - t0) * i) / 160));
  // The range always holds every value shown, so the readout can never sit off the chart.
  const lo = Math.min(m.lo, ...vals), hi = Math.max(m.hi, ...vals);
  let d = '';
  vals.forEach((v, i) => { d += `${i === 0 ? 'M' : 'L'}${((i / 160) * W).toFixed(1)} ${(H - ((v - lo) / (hi - lo)) * H).toFixed(1)}`; });
  return (
    <figure className="cd-chart">
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img" aria-label={`${m.label} from ${t0.toFixed(1)} to ${t1.toFixed(1)} s`}>
        <path d={d} vectorEffect="non-scaling-stroke" />
      </svg>
      <figcaption>{t0.toFixed(1)} to {t1.toFixed(1)} s · {m.label} {lo.toFixed(2)} to {hi.toFixed(2)}</figcaption>
    </figure>
  );
}

export function TimePanel({ selected, liveIteration, snapshot, available, onScrub, follow, onFollow, playing, onPlay }: Props) {
  const run = RUNS.find((r) => r.id === selected)!;
  const series = SERIES[selected];
  const last = series.last(available);
  const running = run.state === 'running';
  const t = snapshot * DT_SNAPSHOT;
  const clamp = (k: number) => Math.max(0, Math.min(last, Math.round(k)));
  const m = series.monitor;
  const hasData = series.residuals !== null && last > 0;

  return (
    <section className="time" aria-label={`Time of ${run.caseName}`}>
      <div className="time-labels">
        <h2 className="panel-head">Run bands</h2>
        <div className="flag-lane-lab" aria-hidden="true" />
        {RUNS.map((r) => <div key={r.id} className={`lab${r.id === selected ? ' focus' : ''}`}>{r.caseName}</div>)}
        <div className="lab axis-lab">run time, s</div>
        <div className="lab resid-lab"><span>Residuals<br /><b>{run.caseName}</b></span><span className="ticks"><i>1e-1</i><i>1e-3</i><i>1e-5</i><i>1e-7</i></span></div>
      </div>

      <div className="time-main">
        <div className="panel-head">Time <small>one band per run · drag the line to move field, residuals and monitor together</small></div>
        <div className="scrub-area">
          {RUNS.map((r) => <Band key={r.id} run={r} focus={r.id === selected} liveIteration={liveIteration} liveT={available * DT_SNAPSHOT} />)}
          <div className="axis">{TICKS.map((v) => <i key={v} style={{ left: pct(v) }}>{v}</i>)}</div>
          {hasData
            ? <ResidualPlot series={series} last={last} running={running} />
            : <div className="resid-empty"><Pictogram state={run.state} />No residuals: {run.caseName} has not started.</div>}
          {hasData && (
            <Slider className="scrubber" aria-label={`Time of ${run.caseName}`} minValue={0} maxValue={SNAPSHOTS} value={snapshot}
              onChange={(v) => { const k = clamp(v as number); onScrub(k); onFollow(running && k === last); }}>
              <SliderTrack className="scrub-track">
                <SliderThumb className="scrub-thumb"><span className={`scrub-flag${t < 3 ? ' flag-right' : ''}`}>t = {t.toFixed(2)} s</span></SliderThumb>
              </SliderTrack>
            </Slider>
          )}
        </div>
      </div>

      <div className="monitor">
        <div className="panel-head">Monitor <small>{m ? `${m.label} · ${run.caseName}` : run.caseName}</small></div>
        {m && hasData ? (
          <div className="cd-read">
            <div><small>t = {t.toFixed(2)} s · it {series.iterationAt(snapshot)}</small>
              <div className="big">{m.label === 'Cd' ? <>C<sub>d</sub></> : m.label === 'Cl' ? <>C<sub>l</sub></> : m.label} = {m.at(t).toFixed(3)}{m.unit && <span className="unit"> {m.unit}</span>}</div>
            </div>
            <MonitorChart series={series} snapshot={snapshot} />
          </div>
        ) : (
          <p className="monitor-empty"><Pictogram state={run.state} />No monitor values yet. They appear with the first snapshot.</p>
        )}
        {run.state === 'failed' && <p className="monitor-note"><Pictogram state="failed" />Values after iteration 190 grow without bound; the run stopped at 211.</p>}
        <div className="transport" role="group" aria-label="Playback">
          <Button aria-label="First snapshot" isDisabled={!hasData} onPress={() => { onScrub(0); onFollow(false); }}><Icon name="first" /></Button>
          <Button aria-label="Previous snapshot" isDisabled={!hasData} onPress={() => { onScrub(clamp(snapshot - 1)); onFollow(false); }}><Icon name="back" /></Button>
          <Button aria-label={playing ? 'Pause' : 'Play'} isDisabled={!hasData} className="primary" onPress={() => onPlay(!playing)}>
            <Icon name={playing ? 'pause' : 'play'} />{playing ? 'Pause' : 'Play'}
          </Button>
          <Button aria-label="Next snapshot" isDisabled={!hasData} onPress={() => { const k = clamp(snapshot + 1); onScrub(k); onFollow(running && k === last); }}><Icon name="forward" /></Button>
          {running && (
            <ToggleButton isSelected={follow} onChange={(on) => { onFollow(on); if (on) onScrub(last); }} className="follow">
              <Icon name="live" />Follow live
            </ToggleButton>
          )}
          <Label className="snap">{run.hasField ? 'snapshot' : 'sample'} {snapshot}/{last}{running ? ' computed so far' : ''}{run.hasField ? '' : ' · no field saved'}</Label>
        </div>
        <ul className="legend-lines" aria-label="Residual series">
          <li><svg width="26" height="6" aria-hidden="true"><path d="M0 3H26" className="res res-continuity" /></svg>continuity</li>
          <li><svg width="26" height="6" aria-hidden="true"><path d="M0 3H26" className="res res-u" /></svg>u</li>
          <li><svg width="26" height="6" aria-hidden="true"><path d="M0 3H26" className="res res-p" /></svg>p</li>
        </ul>
      </div>
    </section>
  );
}
