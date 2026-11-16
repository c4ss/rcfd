// The time panel of the run workspace (F-810; DESIGN.md Run bands, the signature, and Layout: 200px row labels, the
// shared axis, a 380px monitor): every run of the project as one 20px band on one axis of iterations, the selected
// run's residual plot under the bands on the same axis, and one deep ink scrubber line with its own 18px flag lane
// that crosses bands, plot and axis. Moving the line (pointer, arrow keys, Page Up and Down, Home and End, the transport
// buttons, Play, Follow live) moves the field, the residual cursor, the monitor readout and the snapshot counter
// together, directly and with no easing: they all read the same stop.
import { useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import type { RunSeries } from '../../shared/run.js';
import { fmt, fmtResidual, Swatch } from '../plots/parts.js';
import { PlotFrame } from '../plots/PlotFrame.js';
import type { Axis } from '../plots/range.js';
import { SeriesData } from '../plots/series.js';
import { MONITOR_STYLE, residualOrder, residualStyles, type LineStyle } from '../plots/style.js';
import { caseName, type RunRow } from '../runs/model.js';
import { Button, IconButton, Select, StatePictogram, Toggle } from '../ui/index.js';
import '../plots/plots.css';
import { counterText, flagText, fraction, labelAfter, nearestPosition, valueText, type Band, type Position } from './model.js';
import { Provenance, counterValue, seriesValue, type StopFacts } from '../provenance/index.js';

/** The value of one series at the line's step. */
export type CursorValue =
  | { readonly kind: 'value'; readonly value: number; readonly step: number }
  | { readonly kind: 'none'; readonly step: number }
  | { readonly kind: 'reading'; readonly step: number }
  | { readonly kind: 'error'; readonly step: number; readonly message: string };

export interface TimePanelProps {
  readonly rows: readonly RunRow[];
  readonly bands: readonly Band[];
  readonly axis: Axis;
  readonly row: RunRow;
  readonly onSelect: (id: string) => void;
  readonly positions: readonly Position[];
  readonly index: number;
  readonly onMove: (index: number) => void;
  readonly series: RunSeries | null;
  readonly seriesError: string | null;
  readonly version: number;
  readonly cursor: ReadonlyMap<string, CursorValue>;
  readonly running: boolean;
  readonly follow: boolean;
  readonly onFollow: (on: boolean) => void;
  readonly playing: boolean;
  readonly onPlay: (on: boolean) => void;
}

const pct = (f: number): string => `${(Math.max(0, Math.min(1, f)) * 100).toFixed(4)}%`;

/** The series of one kind as the plots hold them, rebuilt when the run's histories are read again. */
function useSeriesMap(series: RunSeries | null, version: number, pick: (names: string[]) => Map<string, LineStyle>, kind: 'residual' | 'monitor', only?: string | null) {
  return useMemo(() => {
    const out = new Map<string, { data: SeriesData; style: LineStyle }>();
    if (!series) return out;
    const entries = series.series.filter((s) => (kind === 'residual' ? s.kind === 'residual' : s.kind !== 'residual') && (only === undefined || s.name === only));
    const styles = pick(entries.map((e) => e.name));
    for (const name of kind === 'residual' ? residualOrder(entries.map((e) => e.name)) : entries.map((e) => e.name)) {
      const e = entries.find((x) => x.name === name)!;
      const d = new SeriesData(e.name, e.unit ?? '', Math.max(4, e.x.length));
      d.replace(e.unit ?? '', e.x, e.y);
      out.set(name, { data: d, style: styles.get(name)! });
    }
    return out;
    // `version` is raised on every read of the run's histories
  }, [series, version, kind, only]);
}

function valueOf(v: CursorValue | undefined, format: (x: number) => string): string {
  if (!v) return 'no series';
  switch (v.kind) {
    case 'value':
      return format(v.value);
    case 'none':
      return 'no sample here';
    case 'reading':
      return 'reading…';
    case 'error':
      return 'not read';
  }
}

export function TimePanel(props: TimePanelProps) {
  const { rows, bands, axis, row, onSelect, positions, index, onMove, series, seriesError, version, cursor, running, follow, onFollow, playing, onPlay } = props;
  const name = caseName(row.case) || row.id;
  const position = positions[index] ?? null;
  const last = positions.length - 1;
  const bandsRef = useRef<HTMLDivElement>(null);
  const residuals = useSeriesMap(series, version, residualStyles, 'residual');
  const monitorNames = useMemo(() => (series ? series.series.filter((s) => s.kind !== 'residual').map((s) => s.name) : []), [series]);
  const [monitorChoice, setMonitorChoice] = useState<{ run: string; name: string } | null>(null);
  const monitor = monitorChoice && monitorChoice.run === row.id && monitorNames.includes(monitorChoice.name) ? monitorChoice.name : (monitorNames[0] ?? null);
  const monitorSeries = useSeriesMap(series, version, (n) => new Map(n.map((k) => [k, MONITOR_STYLE])), 'monitor', monitor);
  const monitorEntry = series?.series.find((s) => s.name === monitor) ?? null;
  const ownEnd = bands.find((b) => b.id === row.id)?.end ?? 0;
  // F-828: the stop every number of the panel belongs to, for its provenance
  const stop: StopFacts | null = position ? { ...position, of: positions.length } : null;
  const valueProvenance = (n: string, v: CursorValue | undefined, shown: string, entryKind: 'residual' | 'monitor' | 'other') => {
    const e = series?.series.find((x) => x.name === n);
    return v?.kind === 'value'
      ? seriesValue({ run: row.id, name: n, kind: entryKind, shown, value: v.value, stop, step: v.step, unit: e?.unit ?? null, unitsNote: series?.unitsNote ?? null })
      : null;
  };

  return (
    <section className="tp" aria-label={`Time of ${name}`}>
      <div className="tp-main">
        <div className="tp-head">
          <h2 className="tp-title">Run bands</h2>
          <small>one band per run on a shared axis of iterations · move the line to move field, residuals and monitor together</small>
        </div>
        <div className="tp-body">
          <div className="tp-lane" aria-hidden="true" />
          <div className="tp-bands" ref={bandsRef}>
            {bands.map((b, i) => {
              const r = rows.find((x) => x.id === b.id)!;
              const selected = b.id === row.id;
              return (
                <div key={b.id} className={`tp-band-row${selected ? ' tp-band-row--selected' : ''}`} data-run={b.id} data-state={r.state}>
                  <div className="tp-band-name" onClick={() => onSelect(b.id)} title={`${caseName(r.case)} · ${b.id}`}>
                    {caseName(r.case) || b.id}
                  </div>
                  <div className={`tp-band${selected ? '' : ' tp-band--faded'}`}>
                    <div className="tp-track">
                      {b.segments.map((s, k) => (
                        <span
                          key={`${s.kind}-${k}`}
                          className={`tp-seg tp-seg--${s.kind}${s.cap ? ' tp-seg--cap' : ''}`}
                          data-kind={s.kind}
                          style={
                            s.cap
                              ? { left: `max(0px, calc(${pct(fraction(s.to, axis))} - var(--tp-cap)))`, animationDelay: `calc(var(--motion-band-stagger) * ${i})` }
                              : { left: pct(fraction(s.from, axis)), width: pct(fraction(s.to, axis) - fraction(s.from, axis)), animationDelay: `calc(var(--motion-band-stagger) * ${i})` }
                          }
                        />
                      ))}
                      <span
                        className={`tp-end tp-end--${b.state ?? 'other'} ${labelAfter(b.end, axis) ? 'tp-end--after' : 'tp-end--inside'}`}
                        style={labelAfter(b.end, axis) ? { left: `calc(${pct(fraction(b.end, axis))} + var(--space-sm))` } : { right: `calc(${pct(1 - fraction(b.end, axis))} + var(--space-md))` }}
                      >
                        {b.state && <StatePictogram state={b.state} size={12} />}
                        {b.label}
                      </span>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
          <div className="tp-resid">
            <div className="tp-resid-label">
              <span className="tp-resid-title">
                Residuals · <b>{name}</b>
              </span>
              <span className="tp-resid-at">{position ? `at it ${position.step}, log scale` : 'log scale'}</span>
              <ul className="tp-resid-values" aria-label={`Residuals of ${name} at the line`}>
                {[...residuals].map(([n, { style }]) => {
                  const v = cursor.get(n);
                  return (
                    <li key={n} data-name={n} data-step={v?.step} data-value={v?.kind === 'value' ? String(v.value) : undefined}>
                      <Swatch style={style} />
                      <span className="tp-resid-name">{n}</span>
                      <span className="tp-resid-value">
                        <ProvenanceOr source={valueProvenance(n, v, valueOf(v, fmtResidual), 'residual')}>{valueOf(v, fmtResidual)}</ProvenanceOr>
                      </span>
                    </li>
                  );
                })}
              </ul>
            </div>
            <div className="tp-resid-plot">
              {residuals.size > 0 ? (
                <PlotFrame series={residuals} version={version} log xCaption="iteration" height={56} xAxis={axis} cursor={position?.step ?? null} />
              ) : (
                <p className="tp-empty" role="status">
                  <StatePictogram state={row.state === 'failed' ? 'failed' : 'queued'} />
                  {seriesError
                    ? `The residuals of ${name} could not be read: ${seriesError}`
                    : row.state === 'queued'
                      ? `No residuals: ${name} has not started.`
                      : series
                        ? `No residuals: ${name} recorded none${row.state === 'failed' ? ' before it failed' : ' yet'}.`
                        : `Reading the residuals of ${name}…`}
                </p>
              )}
              {running && ownEnd < axis.hi && residuals.size > 0 && (
                <div className="tp-veil" style={{ left: `calc(var(--tp-x0) + (100% - var(--tp-x0) - var(--tp-x1)) * ${fraction(ownEnd, axis)})` }} aria-hidden="true">
                  <span>not computed yet</span>
                </div>
              )}
            </div>
          </div>
          <Scrubber name={name} axis={axis} positions={positions} index={index} onMove={onMove} running={running} bandsRef={bandsRef} />
        </div>
      </div>

      <div className="tp-monitor">
        <div className="tp-head">
          <h2 className="tp-title">Monitor</h2>
          {monitorNames.length > 1 ? (
            <Select
              aria-label="Monitor series"
              className="tp-monitor-select"
              options={monitorNames.map((n) => ({ id: n, label: n }))}
              selected={monitor}
              onChange={(n) => setMonitorChoice({ run: row.id, name: n })}
            />
          ) : (
            <small>{monitor ?? name}</small>
          )}
        </div>
        {monitor !== null && monitorEntry ? (
          <>
            <div className="tp-readout">
              <small>
                {monitorEntry.unit ? `${monitor}, ${monitorEntry.unit}` : `${monitor}, no unit given`}
                {position ? ` · at it ${position.step}` : ''}
              </small>
              <output
                className="tp-readout-value"
                data-name={monitor}
                data-step={cursor.get(monitor)?.step}
                data-value={cursor.get(monitor)?.kind === 'value' ? String((cursor.get(monitor) as { value: number }).value) : undefined}
              >
                <ProvenanceOr source={valueProvenance(monitor, cursor.get(monitor), valueOf(cursor.get(monitor), fmt), monitorEntry.kind)}>
                  {valueOf(cursor.get(monitor), fmt)}
                </ProvenanceOr>
              </output>
            </div>
            <div className="tp-monitor-plot">
              <PlotFrame series={monitorSeries} version={version} log={false} xCaption="iteration" height={36} cursor={position?.step ?? null} />
            </div>
          </>
        ) : (
          <p className="tp-empty tp-empty--monitor" role="status">
            <StatePictogram state={row.state === 'failed' ? 'failed' : 'queued'} />
            {series ? `No monitor values: the case of ${name} records no monitor besides its residuals.` : `Reading the monitors of ${name}…`}
          </p>
        )}
        <div className="tp-transport" role="group" aria-label="Snapshots">
          <IconButton label="First snapshot" icon="first" isDisabled={last < 0 || index === 0} onPress={() => onMove(0)} />
          <IconButton label="Previous snapshot" icon="back" isDisabled={last < 0 || index === 0} onPress={() => onMove(index - 1)} />
          <Button variant="primary" icon={playing ? 'pause' : 'play'} isDisabled={last < 1 || (!playing && index >= last)} onPress={() => onPlay(!playing)}>
            {playing ? 'Pause' : 'Play'}
          </Button>
          <IconButton label="Next snapshot" icon="forward" isDisabled={last < 0 || index >= last} onPress={() => onMove(index + 1)} />
          {running && (
            <Toggle isSelected={follow} onChange={onFollow} className="tp-follow">
              Follow live
            </Toggle>
          )}
        </div>
        <output className="tp-counter" data-step={position?.step} data-snapshot={position?.snapshot ?? undefined} aria-live="off">
          {stop ? (
            <Provenance pkey="tp-counter" subject={{ kind: 'number', source: counterValue({ run: row.id, shown: counterText(index, positions, running), stop }) }}>
              {counterText(index, positions, running)}
            </Provenance>
          ) : (
            counterText(index, positions, running)
          )}
        </output>
      </div>
    </section>
  );
}

/**
 * The scrubber: one line over the bands, the plot and the axis, with its flag in its own lane. A slider to assistive
 * technology (its value in words); the arrow keys move one stop, Page Up and Down ten, Home and End to the ends.
 */
function Scrubber({
  name,
  axis,
  positions,
  index,
  onMove,
  running,
  bandsRef,
}: {
  name: string;
  axis: Axis;
  positions: readonly Position[];
  index: number;
  onMove: (i: number) => void;
  running: boolean;
  bandsRef: React.RefObject<HTMLDivElement | null>;
}) {
  const [dragging, setDragging] = useState(false);
  const last = positions.length - 1;
  const p = positions[index] ?? null;
  const f = p ? fraction(p.step, axis) : 0;
  const at = (e: PointerEvent<HTMLDivElement>): number => {
    const r = e.currentTarget.getBoundingClientRect();
    const step = axis.lo + ((e.clientX - r.left) / Math.max(1, r.width)) * (axis.hi - axis.lo);
    return nearestPosition(positions, step);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (last < 0) return;
    const to: Record<string, number> = { ArrowRight: index + 1, ArrowUp: index + 1, ArrowLeft: index - 1, ArrowDown: index - 1, PageUp: index + 10, PageDown: index - 10, Home: 0, End: last };
    const k = to[e.key];
    if (k === undefined) return;
    e.preventDefault();
    onMove(k);
  };
  return (
    <div
      className={`tp-scrubber${dragging ? ' tp-scrubber--dragging' : ''}`}
      role="slider"
      tabIndex={0}
      aria-label={`Time of ${name}`}
      aria-orientation="horizontal"
      aria-valuemin={0}
      aria-valuemax={Math.max(0, last)}
      aria-valuenow={last < 0 ? 0 : index}
      aria-valuetext={valueText(index, positions, running)}
      aria-disabled={last < 0 ? true : undefined}
      data-step={p?.step}
      data-snapshot={p?.snapshot ?? undefined}
      data-provenance="tp-counter"
      onKeyDown={onKeyDown}
      onPointerDown={(e) => {
        if (last < 0 || e.button !== 0) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        setDragging(true);
        onMove(at(e));
      }}
      onPointerMove={(e) => {
        if (dragging) onMove(at(e));
      }}
      onPointerUp={() => setDragging(false)}
      onPointerCancel={() => setDragging(false)}
      onWheel={(e) => {
        if (bandsRef.current) bandsRef.current.scrollTop += e.deltaY;
      }}
    >
      {p && (
        <span className="tp-line" style={{ left: pct(f) }}>
          <span className={`tp-flag${f < 0.18 ? ' tp-flag--right' : ''}`}>{flagText(p)}</span>
        </span>
      )}
    </div>
  );
}

/** F-828: a value with its provenance when it is one; words (no sample, reading) stay plain. */
function ProvenanceOr({ source, children }: { source: ReturnType<typeof seriesValue> | null; children: React.ReactNode }) {
  return source ? <Provenance subject={{ kind: 'number', source }}>{children}</Provenance> : <>{children}</>;
}
