// The renderer's one door to provenance (F-828, window.studioProvenance, src/shared/provenance.ts): the only renderer
// code that names the bridge (tests/unit/provenance.spec.ts), as workspace/door.ts is for window.studioRunData. Every
// answer is the bridge's own `{ok, result}` or `{ok: false, error}`.
//
// The port is a main-process read through the open project's read-only project server (`read_project_file` of
// runs/<id>/run.json, its case.snapshot and the case file), not a service method, so it has no entry in the action
// registry (G16; reported with F-828, as F-805, F-810 and F-812 report theirs). The engine version of the attached
// service comes from the registry's `service.info` action (../actions), logged like every other read.
import type { ProvenanceReply, RunProvenance } from '../../shared/provenance.js';
import { dispatch } from '../actions/index.js';

const bridge = () => window.studioProvenance;

const noBridge = <T>(): ProvenanceReply<T> => ({ ok: false, error: { code: 'no_port', message: 'this page has no provenance bridge (window.studioProvenance)' } });

/** The provenance of a run of the open project, read again on every call (a run's record never changes; its checks do). */
export function runProvenance(run: string): Promise<ProvenanceReply<RunProvenance>> {
  const b = bridge();
  return b ? b.run(run) : Promise.resolve(noBridge());
}

/** Puts text on the clipboard through the main process. */
export function copyText(text: string): Promise<ProvenanceReply<{ readonly chars: number }>> {
  const b = bridge();
  return b ? b.copy(text) : Promise.resolve(noBridge());
}

let serviceEngine: Promise<{ name: string; version: string } | null> | null = null;

/**
 * The name and version of the solver service the Studio is attached to (`service.info`), once per page; null when no
 * service answers. Asked only when a provenance is opened.
 */
export function attachedEngine(): Promise<{ name: string; version: string } | null> {
  serviceEngine ??= dispatch('service.info', {}, { undo: false }).then((r) => {
    if (!r.ok) {
      serviceEngine = null; // asked again at the next opening: the service may come up
      return null;
    }
    const o = r.result as { name?: unknown; version?: unknown };
    return typeof o.name === 'string' && typeof o.version === 'string' ? { name: o.name, version: o.version } : null;
  });
  return serviceEngine;
}
