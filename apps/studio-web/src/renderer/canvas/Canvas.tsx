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
// data-who, data-actor, data-pid, data-reason, data-old and data-new. F-977 (ADR 0817 D3): each panel's data-canvas-field
// and data-canvas-monitor (its field or monitor, where it names one), so a test reads which views a layout chose.
//
// F-978 (ADR 0826 D5): the kind `process`, the ordered steps of a case from its creation to its last result as the
// engine gave them (read_view `process`); a step picked draws the view of its state (the engine's `opens`) under it
// with the panels above, and picked again closes it. Hooks: ol.cv-process data-steps and data-steps-total, each li's
// data-step, data-step-kind, data-phase, data-source, data-seq, data-run and data-t, the opened view's
// data-opened-step and data-panels, and each opened panel's data-opened-panel and data-canvas-kind.
import { useEffect, useMemo, useRef, useState } from 'react';
import type { CanvasPanel, CanvasView, ChangeEntry, ProcessStep, ProjectView } from '../../shared/project.js';
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
import { whyOf } from '../project/activity.js';
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
          <div key={`${canvas.id}-${i}`} className="cv-panel" data-canvas-panel={i} data-canvas-kind={p.kind} data-canvas-field={p.field} data-canvas-monitor={p.monitor}>
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
    case 'process':
      return <ProcessPanel view={view} index={index} canvas={canvas} />;
  }
}

/** F-978: what a step holds beyond its text, as the engine wrote it. */
function stepFacts(s: ProcessStep): string[] {
  const out: string[] = [];
  for (const c of s.changes ?? []) out.push(`${c.path}: ${JSON.stringify(c.old ?? null)} to ${JSON.stringify(c.new ?? null)}`);
  if (s.step === 'state' && s.reason) out.push(s.reason);
  if (s.last) for (const [k, v] of Object.entries(s.last)) out.push(`${k} ${String(v[1])} at step ${v[0]}`);
  if (s.fields) out.push(`fields ${s.fields.join(', ')} at the last of ${s.snapshots ?? 0} snapshots`);
  return out;
}

/** F-273: the process id of a step's actor ({client, instance {pid}}), or null. */
function stepPid(actor: unknown): number | null {
  const pid = actor && typeof actor === 'object' ? (actor as { instance?: { pid?: unknown } }).instance?.pid : undefined;
  return typeof pid === 'number' ? pid : null;
}

/**
 * F-978 (ADR 0826 D5): the process of a case, its steps in the engine's order; a picked step draws the view of its
 * state (`opens`) with the canvas's own panels. Picking writes nothing.
 */
function ProcessPanel({ view, index, canvas }: { view: ProjectView; index: number; canvas: CanvasView }) {
  const got = canvas.process?.find((p) => p.panel === index) ?? null;
  const [open, setOpen] = useState<number | null>(null);
  const steps = got?.steps ?? [];
  return (
    <ol className="cv-process" data-steps={steps.length} data-steps-total={got?.steps_total ?? 0}>
      {steps.map((s) => {
        const actor = s.actor && typeof s.actor === 'object' ? (s.actor as { client?: unknown }).client : undefined;
        const pid = stepPid(s.actor);
        const why = whyOf(s.reason);
        const opened = open === s.i && s.opens !== null;
        return (
          <li key={s.i} className="cv-step" data-step={s.i} data-step-kind={s.step} data-phase={s.phase} data-source={s.source} data-seq={s.seq} data-run={s.run} data-t={s.t ?? undefined}>
            <button type="button" className="cv-step-pick" aria-expanded={opened} disabled={s.opens === null} onClick={() => setOpen(open === s.i ? null : s.i)} data-words="data">
              <span className="cv-step-phase">{s.phase}</span> {s.text}
            </button>
            {s.source === 'journal' ? (
              // F-273 (ADR 0837 D4): who and why of every journaled step; the reason only as text (D3)
              <p className="cv-change" data-words="data" data-actor={typeof actor === 'string' ? actor : undefined} data-pid={pid ?? undefined} data-why={why.given ? 'given' : 'none'} data-reason-cut={why.cut ? 'true' : undefined}>
                {typeof actor === 'string' && `by ${actor}${pid !== null ? ` (pid ${pid})` : ''}; `}
                <span className="cv-change-reason">why: {why.text}</span>
              </p>
            ) : (
              (typeof actor === 'string' || s.reason) &&
              s.step !== 'state' && (
                <p className="cv-change" data-words="data">
                  {typeof actor === 'string' && `by ${actor}`}
                  {s.reason && <span className="cv-change-reason"> : {s.reason}</span>}
                </p>
              )
            )}
            {stepFacts(s).map((f, j) => (
              <p key={j} className="cv-step-fact" data-words="data">
                {f}
              </p>
            ))}
            {opened && s.opens && (
              <div className="cv-opened" data-opened-step={s.i} data-panels={s.opens.panels.length}>
                {s.opens.panels.map((p, j) => (
                  <div key={`${canvas.id}-${s.i}-${j}`} className="cv-panel" data-opened-panel={j} data-canvas-kind={p.kind}>
                    <Panel view={view} panel={p} index={-1} canvas={canvas} />
                  </div>
                ))}
              </div>
            )}
          </li>
        );
      })}
    </ol>
  );
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
