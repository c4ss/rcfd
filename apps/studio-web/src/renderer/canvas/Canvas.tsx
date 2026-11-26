// Claude's canvas (F-976, ADR 0805; G23): the view an MCP client asked this project's Studio to show with show_view,
// laid out as its specification says, each panel drawn by the Studio's existing view of its kind:
//
//   case     the project home's case row (CasesTable)          run      the run workspace with the run selected
//   field    the field view of the run's store (FieldViewport) mesh     the project home's mesh row (MeshesTable)
//   plot     the run's residual lines (PlotFrame, as the time panel draws them)
//   monitor  one monitor line of the run (PlotFrame)           table    the project home's rows of the runs (RunsTable)
//
// F-979 (ADR 0807 D3) makes it live, as MCP clients and the runner change what it shows:
//
//   mesh {case}       the case's mesh drawn by the case editor's MeshPreview (the engine's mesh.preview store), on a
//                     session made from the case file, made again whenever the feed reports a change of the case's mesh
//   setting           one setting's value as the engine read it (read_view `settings`, read again by the session on a
//                     change of the case)
//   monitor {case}    the monitor of the newest run of the case; every monitor and plot panel reads its series again
//                     whenever the run's state or step changes, as the run workspace does
//
// and each of these panels names the latest change of what it shows: the command, the actor (the client and its
// process, or the runner) and the reason when one was given.
//
// The engine validated the specification (crates/rcfd-workspace/src/view.rs); the canvas computes nothing of its own
// (G16, G23): every value is what those views read from the engine. Test hooks: the root's data-canvas-view (the
// showing's id) and data-panels, each panel's data-canvas-panel and data-canvas-kind; F-979's data-mesh-build,
// data-setting, data-value, data-run, data-state, data-last-step, data-last-value and the change line's data-n,
// data-who, data-actor, data-pid, data-reason, data-old and data-new.
import { useEffect, useMemo, useRef, useState } from 'react';
import type { CanvasPanel, CanvasView, ChangeEntry, ProjectView } from '../../shared/project.js';
import type { StoreRef } from '../../shared/data.js';
import type { RunSeries } from '../../shared/run.js';
import { CasesTable, MeshesTable, RunsTable, changedAt } from '../project/ProjectHome.js';
import RunWorkspace from '../workspace/RunWorkspace.js';
import { FieldViewport } from '../field/FieldViewport.js';
import { PlotFrame } from '../plots/PlotFrame.js';
import { MONITOR_STYLE, residualStyles } from '../plots/style.js';
import { useSeriesMap } from '../workspace/TimePanel.js';
import { runSeries } from '../workspace/door.js';
import { openStore } from '../data/door.js';
import { storePath } from '../workspace/model.js';
import { runRows } from '../runs/model.js';
import { MeshPreview } from '../case/MeshPreview.js';
import { dispatch } from '../actions/index.js';
import { actorOf, changeOf, panelRun, settingChange } from './model.js';
// the mesh preview's own styles, as the case editor loads them
import '../case/case.css';
import './canvas.css';

const noop = () => undefined;

export default function Canvas({ view, canvas }: { view: ProjectView; canvas: CanvasView }) {
  const spec = canvas.view;
  const panels = spec.panels;
  const columns = Math.max(1, Math.min(4, spec.layout?.columns ?? Math.min(2, panels.length)));
  return (
    <section className="cv" aria-label={spec.title ?? 'Canvas'} data-canvas-view={canvas.id} data-panels={panels.length}>
      {spec.title && <h2 className="cv-title">{spec.title}</h2>}
      <div className="cv-grid" data-columns={columns}>
        {panels.map((p, i) => (
          <div key={`${canvas.id}-${i}`} className="cv-panel" data-canvas-panel={i} data-canvas-kind={p.kind}>
            {p.title && <h3 className="cv-panel-title">{p.title}</h3>}
            <Panel view={view} panel={p} index={i} canvas={canvas} />
          </div>
        ))}
      </div>
    </section>
  );
}

function Panel({ view, panel, index, canvas }: { view: ProjectView; panel: CanvasPanel; index: number; canvas: CanvasView }) {
  const changed = useMemo(() => changedAt(view.changes), [view.changes]);
  switch (panel.kind) {
    case 'case':
      return <CasesTable cases={view.cases.items.filter((c) => c.path === panel.case)} changed={changed} view={view} onRunResult={noop} unsaved={new Map()} />;
    case 'mesh':
      if (typeof panel.case === 'string') return <CaseMeshPanel view={view} panel={panel} />;
      return <MeshesTable meshes={view.meshes.items.filter((m) => m.path === panel.mesh)} changed={changed} view={view} />;
    case 'setting':
      return <SettingPanel view={view} panel={panel} index={index} canvas={canvas} />;
    case 'table': {
      const want = new Set(panel.runs ?? []);
      const rows = runRows(view.runs.items, view.runDetails, view.runDetailErrors).filter((r) => want.has(r.id));
      return <RunsTable runs={rows} changed={changed} folder={view.folder} />;
    }
    case 'run':
      return <RunWorkspace view={view} selected={panel.run ?? null} onSelect={noop} />;
    case 'field':
      return <FieldPanel folder={view.folder} run={panel.run ?? ''} field={panel.field ?? ''} snapshot={panel.snapshot} />;
    case 'plot':
    case 'monitor':
      return <SeriesPanel view={view} panel={panel} />;
  }
}

/** The field view of a run's store at a snapshot (default the last, from the store's own list). */
function FieldPanel({ folder, run, field, snapshot }: { folder: string; run: string; field: string; snapshot: number | undefined }) {
  const store = useMemo<StoreRef>(() => ({ route: 'local', path: storePath(folder, run) }), [folder, run]);
  const [count, setCount] = useState<number | null>(null);
  useEffect(() => {
    let alive = true;
    void openStore(store, { tag: `canvas-${run}` }).then((r) => {
      if (alive && r.ok) setCount(r.result.snapshots.length);
    });
    return () => {
      alive = false;
    };
  }, [store, run]);
  if (count === null || count === 0) return null;
  return <FieldViewport store={store} snapshot={snapshot ?? count - 1} field={field} />;
}

/** F-979: the latest change of what a panel shows: the command, who made it, and why when the entry says. */
function ChangeLine({ entry, setting }: { entry: ChangeEntry | null; setting?: string }) {
  if (!entry) return null;
  const actor = actorOf(entry);
  const values = setting ? settingChange(entry, setting) : null;
  const who = entry.who ?? entry.kind;
  return (
    <p
      className="cv-change"
      data-words="data"
      data-n={entry.n}
      data-who={who}
      data-actor={actor?.client}
      data-pid={actor?.pid ?? undefined}
      data-reason={entry.reason ?? undefined}
      data-old={values ? JSON.stringify(values.old) : undefined}
      data-new={values ? JSON.stringify(values.new) : undefined}
    >
      {who}
      {actor && ` by ${actor.client}${actor.pid !== null ? ` (pid ${actor.pid})` : ''}`}
      {values && `: ${JSON.stringify(values.old)} to ${JSON.stringify(values.new)}`}
      {entry.reason && <span className="cv-change-reason">: {entry.reason}</span>}
    </p>
  );
}

/** The case file of the project, in the folder's own separator (the service reads it as `session.create {path}`). */
function caseFile(folder: string, kase: string): string {
  const sep = folder.includes('\\') && !folder.includes('/') ? '\\' : '/';
  return [folder.replace(/[\\/]+$/, ''), ...kase.split('/')].join(sep);
}

/**
 * F-979: the picture of a case's mesh, drawn by the case editor's MeshPreview on a service session made from the case
 * file. A new session (and so a new picture, MeshPreview keyed by it) whenever the feed reports a change of the case's
 * mesh; the session before it is closed once the new one is made, the last when the panel goes.
 */
function CaseMeshPanel({ view, panel }: { view: ProjectView; panel: CanvasPanel }) {
  const entry = useMemo(() => changeOf(panel, view.changes), [panel, view.changes]);
  const build = entry?.n ?? 0;
  const kase = panel.case ?? '';
  const [made, setMade] = useState<{ session: string; build: number } | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const current = useRef<string | null>(null);
  useEffect(() => {
    let alive = true;
    void dispatch('session.create', { path: caseFile(view.folder, kase) }, { undo: false }).then((r) => {
      const s = r.ok ? (r.result as { session?: unknown }).session : null;
      if (typeof s !== 'string') {
        if (alive) setFailure(r.ok ? 'session.create answered without a session' : r.error.message);
        return;
      }
      if (!alive) {
        void dispatch('session.close', { session: s }, { undo: false });
        return;
      }
      const old = current.current;
      current.current = s;
      setMade({ session: s, build });
      setFailure(null);
      if (old) void dispatch('session.close', { session: old }, { undo: false });
    });
    return () => {
      alive = false;
    };
  }, [view.folder, kase, build]);
  useEffect(
    () => () => {
      if (current.current) void dispatch('session.close', { session: current.current }, { undo: false });
      current.current = null;
    },
    [],
  );
  return (
    <div className="cv-mesh" data-mesh-build={made?.build ?? undefined}>
      {made && <MeshPreview key={made.session} session={made.session} folder={view.folder} settingsEpoch={0} />}
      {failure && (
        <p className="cv-change" role="alert" data-words="exception">
          The mesh of {kase} could not be read: {failure}
        </p>
      )}
      <ChangeLine entry={entry} />
    </div>
  );
}

/** F-979: one setting's value as the engine read it (read_view `settings`), with its latest change. */
function SettingPanel({ view, panel, index, canvas }: { view: ProjectView; panel: CanvasPanel; index: number; canvas: CanvasView }) {
  const got = canvas.settings?.find((s) => s.panel === index) ?? null;
  const entry = useMemo(() => changeOf(panel, view.changes), [panel, view.changes]);
  const value = got && got.error === undefined ? JSON.stringify(got.value) : null;
  return (
    <div className="cv-setting" data-setting={panel.setting} data-value={value ?? undefined}>
      <p className="cv-setting-value" data-words="data">
        <span className="cv-setting-path">{panel.setting}</span> = <output>{value ?? ''}</output>
      </p>
      {got?.error !== undefined && (
        <p className="cv-change" role="alert" data-words="exception">
          {got.error}
        </p>
      )}
      <ChangeLine entry={entry} setting={panel.setting} />
    </div>
  );
}

/**
 * The run's residual lines, or one monitor's line, as the run workspace's time panel draws them. F-979: read again
 * whenever the run's state or step changes (the key of the run workspace's useSeries), and a panel of a case follows
 * the newest run of the case.
 */
function SeriesPanel({ view, panel }: { view: ProjectView; panel: CanvasPanel }) {
  const run = panelRun(panel, view.runs.items);
  const monitor = panel.kind === 'monitor' ? (panel.monitor ?? '') : null;
  const row = useMemo(() => (run === null ? null : (runRows(view.runs.items, view.runDetails, view.runDetailErrors).find((r) => r.id === run) ?? null)), [run, view.runs.items, view.runDetails, view.runDetailErrors]);
  const live = row ? `${row.state}/${row.step}` : '';
  const [got, setGot] = useState<{ run: string; series: RunSeries | null; version: number }>({ run: '', series: null, version: 0 });
  useEffect(() => {
    if (run === null || row?.state === 'queued') return;
    let alive = true;
    void runSeries(run).then((r) => {
      if (alive && r.ok) setGot((g) => ({ run, series: r.result, version: g.version + 1 }));
    });
    return () => {
      alive = false;
    };
    // the run is named by its id; `live` changes with its state and step
  }, [run, live]);
  const series = got.run === run ? got.series : null;
  const residuals = useSeriesMap(series, got.version, residualStyles, 'residual');
  const monitors = useSeriesMap(series, got.version, (n) => new Map(n.map((k) => [k, MONITOR_STYLE])), 'monitor', monitor ?? '');
  const line = monitor === null ? null : (series?.series.find((s) => s.kind === 'monitor' && s.name === monitor) ?? null);
  const last = line && line.x.length > 0 ? { step: line.x[line.x.length - 1]!, value: line.y[line.y.length - 1]! } : null;
  const entry = useMemo(() => changeOf(panel, view.changes, run), [panel, view.changes, run]);
  return (
    <div className="cv-series" data-run={run ?? undefined} data-state={row?.state} data-last-step={last?.step} data-last-value={last ? String(last.value) : undefined}>
      {monitor === null ? (
        <PlotFrame series={residuals} version={got.version} log xCaption="iteration" height={120} />
      ) : (
        <PlotFrame series={monitors} version={got.version} log={false} xCaption="iteration" height={80} />
      )}
      <ChangeLine entry={entry} />
    </div>
  );
}
