// The field viewport's development page (F-808), at #/dev/field: the viewport of src/renderer/field on a real store, for
// the e2e tests, the maintainer's view-and-report (npm run field) and the P7 and P8 measurements (npm run bench:field).
// Loaded on demand, so it costs the project home nothing. The store is named in the hash and read through the data door
// like any surface (route `local` needs RCFD_STUDIO_DATA_ROOTS to hold it):
//
//   #/dev/field?route=local&path=<folder of store.rts>&field=p&snapshot=0   (snapshot: 0-based)
//   ...&backend=webgl          three's WebGL 2 back end instead of WebGPU (the fallback, tested)
//   #/dev/field?bench=p8[&uncapped=1]   P8 of ADR 0350: a synthetic surface of 1e6 triangles orbiting (measurement only;
//                                       the result is written into the page as JSON, never shown as a solver result)
import { useEffect, useMemo, useRef, useState } from 'react';
import type { DataRoute, StoreRef } from '../../shared/data.js';
import { Button, NumberField, Select, TextField } from '../ui/index.js';
import { colourRange, finiteRange } from '../field/colormap.js';
import { FieldCanvas, type Backend, type BenchResult, type FieldCanvasApi } from '../field/FieldCanvas.js';
import { FieldViewport } from '../field/FieldViewport.js';
import { benchSoup } from '../field/surface.js';
import './field-dev.css';

export const FIELD_DEV_HASH = '#/dev/field';

interface Query {
  readonly route: DataRoute;
  readonly path: string;
  readonly field: string;
  readonly snapshot: number;
  readonly webgl: boolean;
  readonly bench: string;
  readonly uncapped: boolean;
}

function readQuery(hash: string): Query {
  const q = new URLSearchParams(hash.slice(hash.indexOf('?') + 1 || hash.length));
  const snapshot = Number(q.get('snapshot') ?? '0');
  return {
    route: q.get('route') === 'service' ? 'service' : 'local',
    path: q.get('path') ?? '',
    field: q.get('field') ?? 'p',
    snapshot: Number.isInteger(snapshot) && snapshot >= 0 ? snapshot : 0,
    webgl: q.get('backend') === 'webgl',
    bench: q.get('bench') ?? '',
    uncapped: q.get('uncapped') === '1',
  };
}

function writeQuery(q: Omit<Query, 'bench' | 'uncapped'>): string {
  const p = new URLSearchParams({ route: q.route, path: q.path, field: q.field, snapshot: String(q.snapshot) });
  if (q.webgl) p.set('backend', 'webgl');
  return `${FIELD_DEV_HASH}?${p.toString()}`;
}

const ROUTES = [
  { id: 'local', label: 'local (R2, this machine)' },
  { id: 'service', label: 'service (R1, data.*)' },
] as const;

export default function FieldDev({ hash }: { hash: string }) {
  const q = useMemo(() => readQuery(hash), [hash]);
  const [route, setRoute] = useState<DataRoute>(q.route);
  const [path, setPath] = useState(q.path);
  const [field, setField] = useState(q.field);
  const [snapshot, setSnapshot] = useState(q.snapshot);
  useEffect(() => {
    setRoute(q.route);
    setPath(q.path);
    setField(q.field);
    setSnapshot(q.snapshot);
  }, [q]);
  const store = useMemo<StoreRef>(() => ({ route: q.route, path: q.path }), [q.route, q.path]);
  const show = () => {
    location.hash = writeQuery({ route, path, field, snapshot, webgl: q.webgl });
  };
  return (
    <div className="fd-page">
      <header className="fd-top">
        <div className="fd-brand">
          rcfd <span>Studio</span>
        </div>
        <h1 className="fd-title">Field viewport (development page)</h1>
      </header>
      {q.bench === 'p8' ? (
        <BenchP8 uncapped={q.uncapped} />
      ) : (
        <main className="fd-main">
          <form
            className="fd-form"
            onSubmit={(e) => {
              e.preventDefault();
              show();
            }}
          >
            <Select label="Route" options={ROUTES} selected={route} onChange={(id) => setRoute(id as DataRoute)} />
            <TextField className="fd-path" label="Store folder (holds store.rts)" value={path} onChange={setPath} />
            <TextField label="Field" value={field} onChange={setField} />
            <NumberField label="Snapshot (from 0)" minValue={0} step={1} value={snapshot} onChange={(v) => setSnapshot(Number.isFinite(v) ? v : 0)} />
            <Button type="submit" variant="primary">
              Show
            </Button>
          </form>
          {q.path ? (
            <div className="fd-view">
              <FieldViewport key={q.webgl ? 'webgl' : 'webgpu'} store={store} snapshot={q.snapshot} field={q.field} forceWebGL={q.webgl} />
            </div>
          ) : (
            <p className="fd-empty">Name a store folder to show one of its fields. A local folder must be inside RCFD_STUDIO_DATA_ROOTS.</p>
          )}
        </main>
      )}
      <footer className="fd-foot">Development page: the store is read through the Studio’s data door; nothing here is a sample.</footer>
    </div>
  );
}

/** P8 (ADR 0350): the canvas of the field viewport on 1e6 synthetic triangles, orbiting; the result as JSON in the page. */
function BenchP8({ uncapped }: { uncapped: boolean }) {
  const soup = useMemo(() => benchSoup(), []);
  const range = useMemo(() => {
    const r = finiteRange(soup.values)!;
    return colourRange(r.lo, r.hi);
  }, [soup]);
  const api = useRef<FieldCanvasApi>(null);
  const [backend, setBackend] = useState<Backend | null>(null);
  const [result, setResult] = useState<(BenchResult & { backend: Backend | null }) | null>(null);
  const started = useRef(false);
  const onDrawn = () => {
    if (started.current || !api.current) return;
    started.current = true;
    void api.current.bench({ warm: 120, frames: 1200, uncapped }).then((r) => setResult({ ...r, backend: backendRef.current }));
  };
  const backendRef = useRef<Backend | null>(null);
  backendRef.current = backend;
  return (
    <main className="fd-main">
      <p className="fd-empty">
        P8 of ADR 0350: a synthetic surface of {soup.surface.cells.length} triangles, orbiting one turn per 600 frames
        {uncapped ? ', each frame waiting for the GPU' : ''}. Measurement only; not a result.
        {backend && ` Drawn by ${backend.kind === 'webgpu' ? 'WebGPU' : `WebGL 2 (${backend.reason})`}.`}
      </p>
      <div className="fd-view fd-view--bench">
        <FieldCanvas
          ref={api}
          surface={soup.surface}
          values={soup.values}
          range={range}
          grid={false}
          forceWebGL={false}
          onBackend={setBackend}
          onMoved={() => undefined}
          onDrawn={onDrawn}
        />
      </div>
      {result && (
        <output className="fd-bench" data-bench="p8">
          {JSON.stringify(result)}
        </output>
      )}
    </main>
  );
}
