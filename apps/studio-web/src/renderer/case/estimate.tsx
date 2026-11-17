// ADR 0737 (F-837): the case editor's "estimate before you run" line and Refine x2, both the engine's. `case.facts`
// gives the cells, the thinnest cell, the speed at the start, the time step and step count the engine would set (the
// run-time rule of ADR 0153, which the engine owns: the Studio computes none of it), its Courant estimate and warning,
// and the runner's memory estimate; `mesh.refine {apply: true}` doubles the generator's cell counts in the session, or
// says why it cannot (a mesh file, a generator without counts). Both go through the F-827 dispatcher (rcfd serve --ui).
// A failure shows the engine's message and, under it, the error catalogue's line for its code (F-830, G19): a mesh-file
// case's refusal is 1009 MESH_FILE (ADR 0737, change of 2026-11-17).
import { useEffect, useState } from 'react';
import { dispatch } from '../actions/index.js';
import { EngineRecovery } from '../help/Recovery.js';
import { StatePictogram } from '../ui/index.js';
import { estimateOf, estimateParts, estimateWords, num, obj, type Estimate } from './estimateModel.js';


/** The estimate line: asked again whenever the session's changes change. */
export function RunEstimate({ session, changes }: { session: string; changes: number }) {
  const [est, setEst] = useState<Estimate | { failed: string; code: unknown } | null>(null);
  useEffect(() => {
    let live = true;
    void dispatch('case.facts', { session }).then((r) => {
      if (!live) return;
      setEst(r.ok ? estimateOf(r.result) : { failed: r.error.message, code: r.error.code });
    });
    return () => {
      live = false;
    };
  }, [session, changes]);
  if (est === null) return null;
  if ('failed' in est) {
    return (
      <div className="ce-estimate">
        <p className="st-small" data-words="exception">
          No estimate: {est.failed}
        </p>
        <EngineRecovery code={est.code} />
      </div>
    );
  }
  const words = estimateWords(est);
  return (
    <div className="ce-estimate">
      <p className="st-small" data-words="data" title={words} aria-label={words}>
        {estimateParts(est).join(' · ')}
      </p>
      {est.warning && (
        <p className="ce-refused" role="status" data-words="exception">
          <StatePictogram state="failed" />
          <span>{est.warning}</span>
        </p>
      )}
    </div>
  );
}

export type RefineResult =
  | { readonly ok: true; readonly cells: number | null; readonly cellsAfter: number | null; readonly edits: readonly string[]; readonly overLimit: boolean }
  | { readonly ok: false; readonly message: string; readonly code: unknown };

/** Refine x2 of the session's mesh, applied (an action of the registry, so it can be undone). */
export async function refineMesh(session: string): Promise<RefineResult> {
  const r = await dispatch('mesh.refine', { session, factor: 2, apply: true });
  if (!r.ok) return { ok: false, message: r.error.message, code: r.error.code };
  const x = obj(r.result);
  const edits = Array.isArray(x['edits']) ? x['edits'].map((e) => `${String(obj(e)['path'])} ${String(obj(e)['before'])} → ${String(obj(e)['after'])}`) : [];
  return { ok: true, cells: num(x['cells']), cellsAfter: num(x['cells_after']), edits, overLimit: num(x['over_limit']) !== null };
}

export function RefineResultView({ result }: { result: RefineResult | null }) {
  if (result === null) return null;
  if (!result.ok) {
    return (
      <div role="alert">
        <p className="ce-refused" data-words="exception">
          <StatePictogram state="failed" />
          <span>Refine x2 refused: {result.message}</span>
        </p>
        <EngineRecovery code={result.code} />
      </div>
    );
  }
  return (
    <p className="st-small" role="status" data-words="data">
      Refined x2: {result.cells ?? '?'} → {result.cellsAfter ?? '?'} cells ({result.edits.join(', ')}), not saved yet.
      {result.overLimit ? ' Above the cell limit of the service.' : ''}
    </p>
  );
}
