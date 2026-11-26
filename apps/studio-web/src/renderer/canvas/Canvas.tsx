// Claude's canvas (F-976, ADR 0805; G23): the view an MCP client asked this project's Studio to show with show_view,
// laid out as its specification says, each panel drawn by the Studio's existing view of its kind:
//
//   case     the project home's case row (CasesTable)          run      the run workspace with the run selected
//   field    the field view of the run's store (FieldViewport) mesh     the project home's mesh row (MeshesTable)
//   plot     the run's residual lines (PlotFrame, as the time panel draws them)
//   monitor  one monitor line of the run (PlotFrame)           table    the project home's rows of the runs (RunsTable)
//
// The engine validated the specification (crates/rcfd-workspace/src/view.rs); the canvas computes nothing of its own
// (G16, G23): every value is what those views read from the engine. Test hooks: the root's data-canvas-view (the
// showing's id) and data-panels, each panel's data-canvas-panel and data-canvas-kind.
import { useEffect, useMemo, useState } from 'react';
import type { CanvasPanel, CanvasView, ProjectView } from '../../shared/project.js';
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
            <Panel view={view} panel={p} />
          </div>
        ))}
      </div>
    </section>
  );
}

function Panel({ view, panel }: { view: ProjectView; panel: CanvasPanel }) {
  const changed = useMemo(() => changedAt(view.changes), [view.changes]);
  switch (panel.kind) {
    case 'case':
      return <CasesTable cases={view.cases.items.filter((c) => c.path === panel.case)} changed={changed} view={view} onRunResult={noop} unsaved={new Map()} />;
    case 'mesh':
      return <MeshesTable meshes={view.meshes.items.filter((m) => m.path === panel.mesh)} changed={changed} view={view} />;
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
      return <SeriesPanel run={panel.run ?? ''} monitor={panel.kind === 'monitor' ? (panel.monitor ?? '') : null} />;
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

/** The run's residual lines, or one monitor's line, as the run workspace's time panel draws them. */
function SeriesPanel({ run, monitor }: { run: string; monitor: string | null }) {
  const [got, setGot] = useState<{ series: RunSeries | null; version: number }>({ series: null, version: 0 });
  useEffect(() => {
    let alive = true;
    void runSeries(run).then((r) => {
      if (alive && r.ok) setGot((g) => ({ series: r.result, version: g.version + 1 }));
    });
    return () => {
      alive = false;
    };
  }, [run]);
  const residuals = useSeriesMap(got.series, got.version, residualStyles, 'residual');
  const monitors = useSeriesMap(got.series, got.version, (n) => new Map(n.map((k) => [k, MONITOR_STYLE])), 'monitor', monitor ?? '');
  if (monitor === null) return <PlotFrame series={residuals} version={got.version} log xCaption="iteration" height={120} />;
  return <PlotFrame series={monitors} version={got.version} log={false} xCaption="iteration" height={80} />;
}
