// The mesh preview of the case editor (F-836, ADR 0732): the mesh the session's case runs on, drawn before any run. The
// engine builds it (`mesh.preview`, the solver's own mesh code; G2) and writes it as a mesh-only field store into the
// project's `.rcfd/previews/<mesh hash>/`; the Studio reads that store with its store reader (route local, the reader
// that matches the Rust one bit for bit, G3) and draws its boundary surface on the field canvas, one colour per patch,
// with the mesh grid over it; a 2D mesh is drawn orthographic along its thin axis (ADR 0801 Addendum 22).
//
// - The session's settings are previewed, saved or not: a preview is asked for when the editor opens and again
//   PREVIEW_DEBOUNCE_MS after the last settings change. A newer request makes the Studio ignore an older reply (the
//   engine call itself runs to its end).
// - A case with [mesh.adapt] shows the mesh the run starts on, says that the first step adapts it, and offers the final
//   mesh with its cost said first (the run's adaptive loop: cycles + 1 solves), ADR 0732 Q3 (a).
// - A mesh whose counts give more cells than the engine's preview limit is refused by the engine; the refusal is shown
//   with Build anyway (force), Q4 (b).
// - G22: at rest the panel carries no chrome words: its heading is for readers, the counts and patch names are data.
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { PatchSurfaceData } from '../../shared/data.js';
import type { FieldCanvasApi } from '../field/FieldCanvas.js';
import { isReply, patchRange, previewsDir, shownSurface, thinAxis, type PreviewReply } from './preview.js';
import { dispatch } from '../actions/index.js';
import { loadPatchSurface } from '../data/door.js';
import { colourOf, lutBytes } from '../field/colormap.js';
import { EngineRecovery } from '../help/Recovery.js';
import { Button, StatePictogram } from '../ui/index.js';
import { abortable, beginWork } from '../work/work.js';

const FieldCanvas = lazy(() => import('../field/FieldCanvas.js').then((m) => ({ default: m.FieldCanvas })));

/** Time after the last settings change before the preview is built again (the egui preview's 500 ms, ADR 0144). */
export const PREVIEW_DEBOUNCE_MS = 500;

interface Shown {
  readonly reply: PreviewReply;
  readonly surface: PatchSurfaceData;
}

interface Failure {
  readonly code: unknown;
  readonly message: string;
  /** Refused by the cell estimate: Build anyway offers `force`. */
  readonly tooLarge: boolean;
}

const LUT = lutBytes();
const hex = (c: readonly number[]) => `#${c.map((x) => x.toString(16).padStart(2, '0')).join('')}`;

const number = (n: number) => n.toLocaleString('en-US');

export function MeshPreview({ session, folder, settingsEpoch }: { session: string; folder: string; settingsEpoch: number }) {
  // the last mesh that built stays on screen when a later build fails (marked by the failure below it)
  const [shown, setShown] = useState<Shown | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [building, setBuilding] = useState(false);
  /** Hidden patches by name, so a rebuild with the same patches keeps the choice. */
  const [hiddenNames, setHiddenNames] = useState<ReadonlySet<string>>(new Set());
  const [adapt, setAdapt] = useState<'base' | 'final'>('base');
  /** The values the canvas last finished drawing (its first picture of them is on screen). */
  const [painted, setPainted] = useState<Float32Array | null>(null);
  const generation = useRef(0);
  const canvas = useRef<FieldCanvasApi>(null);

  const build = useCallback(
    async (opts: { adapt: 'base' | 'final'; force: boolean }) => {
      const mine = ++generation.current;
      setBuilding(true);
      const work = beginWork({ kind: 'mesh.preview', scope: 'case-editor', label: `Building the mesh preview of session ${session}`, word: 'Building the mesh' });
      let next: { shown: Shown } | { failure: Failure } | null = null;
      try {
        const r = await abortable(dispatch('mesh.preview', { session, dir: previewsDir(folder), adapt: opts.adapt, ...(opts.force ? { force: true } : {}) }), work.signal);
        if (!r.ok) {
          next = { failure: { code: r.error.code, message: r.error.message, tooLarge: r.error.code === 1004 && /pass force: true/.test(r.error.message) } };
        } else if (!isReply(r.result)) {
          next = { failure: { code: 'internal', message: 'the engine’s reply is not a mesh preview', tooLarge: false } };
        } else {
          const reply = r.result;
          const s = await abortable(loadPatchSurface({ route: 'local', path: reply.path }, { tag: `mesh-preview-${session}` }), work.signal);
          if (!s.ok) next = { failure: { code: s.error.code, message: s.error.message, tooLarge: false } };
          else if (s.result.meshHash !== reply.mesh_hash) next = { failure: { code: 'store', message: `the store holds mesh ${s.result.meshHash}, not ${reply.mesh_hash}`, tooLarge: false } };
          else next = { shown: { reply, surface: s.result } };
        }
      } catch {
        // cancelled: the reply is dropped, the mesh on screen stays
      } finally {
        work.end();
      }
      if (mine !== generation.current) return; // a newer request: this reply is ignored
      setBuilding(false);
      if (next && 'shown' in next) {
        setShown(next.shown);
        setFailure(null);
      } else if (next) setFailure(next.failure);
    },
    [session, folder],
  );

  // on open, and after each settings change (debounced); a change goes back to the starting mesh of a [mesh.adapt] case,
  // so that an edit never starts the adaptive loop's solves unasked
  const buildRef = useRef(build);
  buildRef.current = build;
  const first = useRef(true);
  const epoch = useRef(settingsEpoch);
  useEffect(() => {
    if (epoch.current !== settingsEpoch && adapt === 'final') {
      epoch.current = settingsEpoch;
      setAdapt('base');
      return;
    }
    epoch.current = settingsEpoch;
    const now = first.current;
    first.current = false;
    const t = setTimeout(() => void buildRef.current({ adapt, force: false }), now ? 0 : PREVIEW_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [settingsEpoch, session, adapt]);

  const hidden = useMemo(() => new Set((shown?.reply.patches ?? []).flatMap((p, i) => (hiddenNames.has(p.name) ? [i] : []))), [shown, hiddenNames]);
  const drawn = useMemo(() => (shown ? shownSurface(shown.surface, hidden) : null), [shown, hidden]);
  const range = useMemo(() => patchRange(shown?.reply.patches.length ?? 1), [shown]);

  if (failure?.code === -32601) {
    return (
      <p className="mp-note" data-words="exception">
        This engine has no mesh preview; the Studio starts it with rcfd serve --ui.
      </p>
    );
  }

  const a = shown?.reply.adapt ?? null;
  return (
    <div className="mp" aria-busy={building} data-mesh-hash={shown?.reply.mesh_hash} data-triangles={drawn ? drawn.values.length : undefined} data-drawn={drawn !== null && painted === drawn.values}>
      {/* an empty view only while the first mesh builds; a failure with nothing built shows the failure alone */}
      {(shown || !failure) && (
        <div className="mp-view">
          {drawn && (
            <Suspense fallback={null}>
              <FieldCanvas
                ref={canvas}
                surface={drawn.surface}
                values={drawn.values}
                valuesPer="triangle"
                range={range}
                grid
                forceWebGL={false}
                onBackend={() => undefined}
                onMoved={() => undefined}
                // ADR 0801 Addendum 22: a 2D mesh is drawn orthographic, straight along its thin axis, and pans and zooms
                thin={shown?.reply.two_d ? thinAxis(shown.reply.bounds) : null}
                onDrawn={setPainted}
              />
            </Suspense>
          )}
        </div>
      )}
      {shown && (
        <>
          <p className="mp-facts" data-words="data">
            {number(shown.reply.cells)} cells · {number(shown.reply.points)} points{shown.reply.two_d ? ' · 2D' : ''}
            <span className="sr-only"> · mesh {shown.reply.mesh_hash}</span>
          </p>
          <ul className="mp-patches" aria-label="Patches shown">
            {shown.reply.patches.map((p, i) => (
              <li key={p.name}>
                <label data-words="data">
                  <input
                    type="checkbox"
                    checked={!hiddenNames.has(p.name)}
                    onChange={(e) => {
                      const next = new Set(hiddenNames);
                      if (e.target.checked) next.delete(p.name);
                      else next.add(p.name);
                      setHiddenNames(next);
                    }}
                  />
                  <span className="mp-swatch" style={{ background: hex(colourOf(LUT, i, range)) }} aria-hidden="true" />
                  {p.name} <span className="mp-faint">{p.kind} · {number(p.faces)}</span>
                </label>
              </li>
            ))}
          </ul>
          {a && (
            <div className="mp-adapt">
              {a.pending ? (
                <>
                  <p>[mesh.adapt]: the run starts on this mesh and adapts it in its first step.</p>
                  <Button variant="chip" onPress={() => setAdapt('final')} isDisabled={building}>
                    Show the adapted mesh ({a.cycles + 1} solves)
                  </Button>
                </>
              ) : (
                <>
                  <p>[mesh.adapt]: the mesh after the run’s first step ({a.solves} solves).</p>
                  <Button variant="chip" onPress={() => setAdapt('base')} isDisabled={building}>
                    Show the starting mesh
                  </Button>
                </>
              )}
            </div>
          )}
        </>
      )}
      {failure && (
        <div className="mp-failed" role="alert" data-words="exception">
          <p className="ce-refused">
            <StatePictogram state="failed" />
            <span>
              Mesh preview failed{shown ? ' (the mesh shown is the last one that built)' : ''}: {failure.message}
            </span>
          </p>
          <EngineRecovery code={failure.code} />
          {failure.tooLarge && (
            <Button onPress={() => void build({ adapt, force: true })} isDisabled={building}>
              Build anyway
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
