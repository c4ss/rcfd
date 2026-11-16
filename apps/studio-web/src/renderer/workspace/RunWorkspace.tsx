// The run workspace (F-810; DESIGN.md Layout and Run bands, the Studio's signature surface; the prototype's
// RunWorkspace for layout and behaviour): the project's runs on the left, the selected run's field in the centre, and
// the time panel below, where every run is one band on a shared axis and one scrubber line moves the field, the
// residual cursor, the monitor readout and the snapshot counter of the selected run together.
//
// Everything shown is the engine's: the runs from the project state (list_runs and run_status through the project
// door), the field through the data door (the run's store, runs/<id>/fields, route local), the residual and monitor
// histories through the run door (read_residuals of the project server). The scrubber stops only at the run's
// snapshots, and the residual and monitor values shown at a stop are the engine's samples at that snapshot's step,
// never interpolated (G3). The workspace keeps only display state: the selection, the stop, Follow live (G12).
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ListBox, ListBoxItem } from 'react-aria-components';
import type { StoreInfo, StoreRef } from '../../shared/data.js';
import type { ProjectView } from '../../shared/project.js';
import type { RunSeries } from '../../shared/run.js';
import { openStore, cancelLoads } from '../data/door.js';
import { FieldViewport } from '../field/FieldViewport.js';
import { caseName, runRows, shownState, unknownWord, type RunRow } from '../runs/model.js';
import { Select, StateMark, StatePictogram } from '../ui/index.js';
import { runSeries, runValuesAt } from './door.js';
import { bandOf, defaultField, lastStep, liveRun, positionsOf, sampleAt, sharedAxis, storePath, type Position } from './model.js';
import { TimePanel, type CursorValue } from './TimePanel.js';
import './workspace.css';

export interface RunWorkspaceProps {
  readonly view: ProjectView;
  /** The selected run, shared with the Runs view (kept by the project view). */
  readonly selected: string | null;
  readonly onSelect: (id: string | null) => void;
}

const ACTIVE = new Set(['queued', 'running', 'paused']);

/** How soon a running run's store is opened again after the last open answered (Follow live sees new snapshots). */
const LIVE_STORE_MS = 250;

/**
 * The store of a run: opened again whenever the runner reports a new step of an active run (a live store grows), and
 * while the run is running every LIVE_STORE_MS after the last open answered (the run list is read once a second).
 */
function useStore(folder: string, row: RunRow | null): { ref: StoreRef | null; info: StoreInfo | null; error: string | null } {
  const [got, setGot] = useState<{ run: string; info: StoreInfo | null; error: string | null } | null>(null);
  const [tick, setTick] = useState(0);
  const ref = useMemo<StoreRef | null>(() => (row ? { route: 'local', path: storePath(folder, row.id) } : null), [folder, row?.id]);
  const live = row ? `${row.state}/${row.step}` : '';
  const running = row?.state === 'running';
  useEffect(() => {
    if (!row || !ref || row.state === 'queued') return;
    let alive = true;
    let again: ReturnType<typeof setTimeout> | undefined;
    const tag = `ws-store-${row.id}-${live}-${tick}`;
    void openStore(ref, { tag }).then((r) => {
      if (!alive) return;
      setGot((g) => {
        // the same store as before (no new snapshot): keep it, so nothing below renders again
        const same = g && g.run === row.id && g.info && r.ok && g.info.meshHash === r.result.meshHash && g.info.snapshots.length === r.result.snapshots.length;
        return same ? g : { run: row.id, info: r.ok ? r.result : null, error: r.ok ? null : r.error.message };
      });
      if (running) again = setTimeout(() => setTick((n) => n + 1), LIVE_STORE_MS);
    });
    return () => {
      alive = false;
      clearTimeout(again);
      void cancelLoads(tag);
    };
    // the run is named by its id; `live` changes with its state and step, `tick` while it runs
  }, [ref, row?.id, live, tick, running]);
  const own = got && row && got.run === row.id ? got : null;
  return { ref, info: own?.info ?? null, error: own?.error ?? null };
}

/** The run's residual and monitor histories: read again whenever the runner reports a new step of an active run. */
function useSeries(row: RunRow | null): { series: RunSeries | null; error: string | null; version: number } {
  const [got, setGot] = useState<{ run: string; series: RunSeries | null; error: string | null; version: number } | null>(null);
  const live = row ? `${row.state}/${row.step}` : '';
  useEffect(() => {
    if (!row || row.state === 'queued') return;
    let alive = true;
    void runSeries(row.id).then((r) => {
      if (!alive) return;
      setGot((g) => {
        const version = (g?.version ?? 0) + 1;
        // a failed read keeps what was read before for this run, with the reason
        if (!r.ok) return { run: row.id, series: g && g.run === row.id ? g.series : null, error: r.error.message, version };
        return { run: row.id, series: r.result, error: null, version };
      });
    });
    return () => {
      alive = false;
    };
    // the run is named by its id; `live` changes with its state and step
  }, [row?.id, live]);
  const own = got && row && got.run === row.id ? got : null;
  return { series: own?.series ?? null, error: own?.error ?? null, version: own?.version ?? 0 };
}

/**
 * The value of every series at the stop's step. A series the engine sent whole is looked up in what is held (exact);
 * when a series was decimated, or the step lies past what was read (a live run's store can be ahead of the last read
 * of its residuals), the engine is asked for that step (`read_residuals {since_step}`), latest stop wins.
 */
function useCursorValues(run: string | null, series: RunSeries | null, step: number | null): ReadonlyMap<string, CursorValue> {
  const [asked, setAsked] = useState<{ key: string; values: Readonly<Record<string, number>> | null; error: string | null } | null>(null);
  const past = (s: RunSeries['series'][number]): boolean => step !== null && (s.x.length === 0 || step > s.x[s.x.length - 1]!);
  const local = (s: RunSeries['series'][number]): boolean => !s.decimated && !past(s);
  const needEngine = !!series && series.series.some((s) => !local(s));
  const key = `${run}/${step}/${series?.readAt}`;
  useEffect(() => {
    if (!needEngine || run === null || step === null) return;
    let alive = true;
    const t = setTimeout(() => {
      void runValuesAt(run, step).then((r) => {
        if (alive) setAsked({ key, values: r.ok ? r.result.values : null, error: r.ok ? null : r.error.message });
      });
    }, 30);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [needEngine, run, step, key]);
  return useMemo(() => {
    const out = new Map<string, CursorValue>();
    if (!series || step === null) return out;
    for (const s of series.series) {
      if (local(s)) {
        const v = sampleAt(s.x, s.y, step);
        out.set(s.name, v === null ? { kind: 'none', step } : { kind: 'value', value: v, step });
      } else if (asked && asked.key === key) {
        const v = asked.values?.[s.name];
        out.set(s.name, asked.error !== null ? { kind: 'error', step, message: asked.error } : v === undefined ? { kind: 'none', step } : { kind: 'value', value: v, step });
      } else {
        out.set(s.name, { kind: 'reading', step });
      }
    }
    return out;
  }, [series, step, asked, key]);
}

export default function RunWorkspace({ view, selected, onSelect }: RunWorkspaceProps) {
  const rows = useMemo(() => runRows(view.runs.items, view.runDetails, view.runDetailErrors), [view.runs.items, view.runDetails, view.runDetailErrors]);
  const live = liveRun(rows, selected);
  // with nothing selected the workspace brings forward the live run, else the newest run
  useEffect(() => {
    if (selected !== null || rows.length === 0) return;
    const newest = [...rows].sort((a, b) => (a.created < b.created ? 1 : -1))[0]!;
    onSelect(live ?? newest.id);
  }, [selected, rows, live, onSelect]);
  const row = rows.find((r) => r.id === selected) ?? null;
  const steady = row?.detail?.steady ?? null;
  const store = useStore(view.folder, row);
  const { series, error: seriesError, version } = useSeries(row);
  const positions = useMemo(() => positionsOf(store.info, series, steady), [store.info, series, steady]);
  const running = row !== null && ACTIVE.has(row.state) && row.state !== 'queued';

  // the stop of the selected run, and Follow live (on when a running run is selected)
  const [stop, setStop] = useState<{ run: string | null; index: number }>({ run: null, index: 0 });
  const [follow, setFollow] = useState(false);
  const [playing, setPlaying] = useState(false);
  const last = positions.length - 1;
  // the user moved the line since the run was selected: its start (seen after the selection) no longer turns Follow on
  const moved = useRef(false);
  useEffect(() => {
    moved.current = false;
    setPlaying(false);
    // a new run: its latest stop (set below once its stops are known)
  }, [row?.id]);
  useEffect(() => {
    if (!moved.current) setFollow(row?.state === 'running');
  }, [row?.id, row?.state]);
  const index = stop.run === row?.id ? Math.max(0, Math.min(last, stop.index)) : Math.max(0, last);
  useEffect(() => {
    // a run's first stop is set once its stops are known (its newest snapshot), not before
    if (!row || last < 0) return;
    if (stop.run !== row.id || (follow && stop.index !== last)) setStop({ run: row.id, index: Math.max(0, last) });
  }, [row?.id, last, follow, stop.run, stop.index]);
  const position: Position | null = positions[index] ?? null;

  const moveTo = useCallback(
    (i: number, from: 'user' | 'play' = 'user') => {
      if (!row || last < 0) return;
      const k = Math.max(0, Math.min(last, Math.round(i)));
      setStop({ run: row.id, index: k });
      if (from === 'user') {
        moved.current = true;
        setPlaying(false);
        setFollow(row.state === 'running' && k === last);
      }
    },
    [row, last],
  );

  // Play: one stop after another, each once the field of the one before is drawn (every 150 ms for a run without fields)
  const drawnAt = useRef<{ run: string | undefined; snapshot: number } | null>(null);
  const [drawTick, setDrawTick] = useState(0);
  const snap = position?.snapshot ?? null;
  useEffect(() => {
    if (!playing) return;
    if (index >= last) {
      setPlaying(false);
      return;
    }
    if (snap !== null && !(drawnAt.current?.run === row?.id && drawnAt.current?.snapshot === snap)) return; // its draw re-runs this
    const t = setTimeout(() => moveTo(index + 1, 'play'), snap === null ? 150 : 0);
    return () => clearTimeout(t);
  }, [playing, index, last, snap, row?.id, moveTo, drawTick]);

  // timing marks for P5 and P6 (scripts/measure-scrub.mjs): the stop asked for, and its field drawn
  useEffect(() => {
    if (position?.snapshot !== null && position?.snapshot !== undefined) performance.mark('rcfd:scrub:request', { detail: { run: row?.id, snapshot: position.snapshot, step: position.step } });
  }, [row?.id, position?.snapshot, position?.step]);
  const onDrawn = useCallback(
    (snapshot: number) => {
      drawnAt.current = { run: row?.id, snapshot };
      performance.mark('rcfd:scrub:drawn', { detail: { run: row?.id, snapshot } });
      setDrawTick((n) => n + 1);
    },
    [row?.id],
  );

  // the field: p, or the store's first cell field; the user's choice while the run is selected
  const [fieldChoice, setFieldChoice] = useState<{ run: string; field: string } | null>(null);
  const cellFields = store.info?.fields.filter((f) => f.location === 'cell') ?? [];
  const field = fieldChoice && fieldChoice.run === row?.id && cellFields.some((f) => f.name === fieldChoice.field) ? fieldChoice.field : defaultField(store.info);

  const cursor = useCursorValues(row?.id ?? null, series, position?.step ?? null);

  // the bands: every run on one axis of steps
  const ends = useMemo(() => {
    const m = new Map<string, number>();
    for (const r of rows) m.set(r.id, r.id === row?.id ? lastStep(r, positions) : r.step);
    return m;
  }, [rows, row?.id, positions]);
  const axis = useMemo(() => sharedAxis([...ends.values()]), [ends]);
  const bands = useMemo(() => rows.map((r) => bandOf(r, ends.get(r.id) ?? r.step, axis.hi, r.id === live)), [rows, ends, axis.hi, live]);

  if (rows.length === 0) {
    return (
      <div className="ws ws--empty">
        <p className="ws-empty">
          No runs yet. The run workspace shows a run’s field, residuals and monitors together once the project holds a run: start one from Claude Code (
          <span className="ws-file">start_run</span>) or the CLI (<span className="ws-file">rcfd-runner start</span>).
        </p>
      </div>
    );
  }

  return (
    <div className="ws" data-run={row?.id} data-index={index} data-step={position?.step} data-snapshot={position?.snapshot ?? undefined}>
      <div className="ws-top">
        <section className="ws-runs" aria-labelledby="ws-runs-title">
          <h2 id="ws-runs-title" className="ws-head">
            Runs <span className="ws-head-note">{rows.length}</span>
          </h2>
          <ListBox
            aria-label="Runs of the project"
            className="ws-list"
            selectionMode="single"
            selectionBehavior="replace"
            disallowEmptySelection
            selectedKeys={selected === null ? [] : [selected]}
            onSelectionChange={(keys) => {
              if (keys === 'all') return;
              const k = [...keys][0];
              if (k !== undefined) onSelect(String(k));
            }}
            items={rows}
          >
            {(r) => <RunItem row={r} faded={selected !== null && r.id !== selected} />}
          </ListBox>
          {row && <RunNumbers row={row} store={store.info} storeError={store.error} series={series} seriesError={seriesError} position={position} />}
        </section>
        <section className="ws-field" aria-label="Field view">
          {row && store.ref && store.info && field !== null && position?.snapshot !== null && position?.snapshot !== undefined ? (
            <>
              <div className="ws-field-bar">
                <Select
                  aria-label="Field"
                  className="ws-field-select"
                  options={cellFields.map((f) => ({ id: f.name, label: f.unit ? `${f.name}, ${f.unit}` : f.name }))}
                  selected={field}
                  onChange={(f) => setFieldChoice({ run: row.id, field: f })}
                />
                <span className="ws-field-note">
                  {caseName(row.case)} · {row.id} · stored fields of runs/{row.id}/fields
                </span>
              </div>
              <FieldViewport store={store.ref} snapshot={position.snapshot} field={field} reopen={store.info.snapshots.length} onDrawn={onDrawn} />
            </>
          ) : (
            <NoField row={row} storeError={store.error} store={store.info} hasStops={positions.length > 0} />
          )}
        </section>
      </div>
      {row && (
        <TimePanel
          rows={rows}
          bands={bands}
          axis={axis}
          row={row}
          onSelect={onSelect}
          positions={positions}
          index={index}
          onMove={moveTo}
          series={series}
          seriesError={seriesError}
          version={version}
          cursor={cursor}
          running={running}
          follow={follow}
          onFollow={(on) => {
            moved.current = true;
            setFollow(on);
            if (on) moveTo(last, 'play');
          }}
          playing={playing}
          onPlay={setPlaying}
        />
      )}
    </div>
  );
}

function RunItem({ row, faded }: { row: RunRow; faded: boolean }) {
  const s = shownState(row);
  return (
    <ListBoxItem id={row.id} textValue={`${caseName(row.case)} ${row.id}`} className={`ws-item${row.state === 'failed' ? ' ws-item--failed' : ''}`}>
      <span className="ws-item-case" data-run={row.id}>
        {caseName(row.case) || 'case not recorded'}
      </span>
      <span className="ws-item-id">{row.id}</span>
      <span className="ws-item-state">{s ? <StateMark state={s} faded={faded} /> : <span className="ui-state">{unknownWord(row.state)}</span>}</span>
    </ListBoxItem>
  );
}

function RunNumbers({
  row,
  store,
  storeError,
  series,
  seriesError,
  position,
}: {
  row: RunRow;
  store: StoreInfo | null;
  storeError: string | null;
  series: RunSeries | null;
  seriesError: string | null;
  position: Position | null;
}) {
  const decimated = series?.series.some((s) => s.decimated) ?? false;
  return (
    <div className="ws-numbers" aria-label={`Numbers of ${row.id}`}>
      <dl className="ws-grid">
        <div>
          <dt>{row.detail?.steady === true ? 'Iterations' : 'Steps'}</dt>
          <dd>{row.step}</dd>
        </div>
        <div>
          <dt>Snapshots stored</dt>
          <dd>{store ? store.snapshots.length : storeError ? 'none' : '…'}</dd>
        </div>
        <div>
          <dt>Residual lines</dt>
          <dd>{series ? series.lines : seriesError ? 'not read' : '…'}</dd>
        </div>
        <div>
          <dt>At the line, step</dt>
          <dd>{position ? position.step : 'none'}</dd>
        </div>
      </dl>
      <p className="ws-source">
        Fields from runs/{row.id}/fields{store ? ` (${store.nCells} cells, index ${store.index ?? 'not said'})` : ''}; residuals and monitors from runs/{row.id}
        /residuals.jsonl, read by the engine’s read_residuals
        {decimated ? `, decimated by the engine to ${series!.maxPoints} samples per series for the lines (the values at the line are the engine’s at that step)` : ''}.
        {series?.unitsNote ? ` Monitor units: ${series.unitsNote}.` : ''}
      </p>
      {seriesError && (
        <p className="ws-error" role="alert">
          <StatePictogram state="failed" /> Failed to read the histories: {seriesError}
        </p>
      )}
    </div>
  );
}

function NoField({ row, storeError, store, hasStops }: { row: RunRow | null; storeError: string | null; store: StoreInfo | null; hasStops: boolean }) {
  if (!row) return <p className="ws-nofield">Select a run to see its field, residuals and monitors.</p>;
  const s = shownState(row);
  const name = caseName(row.case) || row.id;
  let text: string;
  if (row.state === 'queued') text = `${name} has not started. Its field appears with its first stored snapshot.`;
  else if (store && store.snapshots.length === 0) text = `${name} has stored no snapshot yet. Its field appears with the first one.`;
  else if (store && defaultField(store) === null) text = `${name} stores no cell field the viewport draws (${store.fields.map((f) => f.name).join(', ') || 'none'}).`;
  else if (storeError !== null)
    text = `${name} has no saved field: runs/${row.id}/fields cannot be read (${storeError}). A run stores fields when its case sets output.store_interval or output.store_time_interval.${hasStops ? ' Its residual and monitor samples are in the time panel below.' : ''}`;
  else text = `Reading the stored fields of ${name}…`;
  return (
    <div className="ws-nofield" role="status">
      {s && <StatePictogram state={s} size={28} />}
      <p>{text}</p>
    </div>
  );
}
