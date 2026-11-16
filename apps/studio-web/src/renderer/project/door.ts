// The project home's one door to the main process (F-805): the only renderer code that touches window.studioProject.
// The project folder is not reached through the action dispatcher (src/renderer/actions, F-827): the folder dialog, the
// recent list and the project server's read-only queries are main-process operations behind the project port's
// allow-list (src/shared/project.ts, src/main/project/bridge.ts), not service methods, and have no action-registry
// entry yet (G16; reported with F-805).
import type { ProjectState } from '../../shared/project.js';

const bridge = () => window.studioProject;

/** Follows the main process's project state; returns the function that stops following. */
export function watchProjectState(listener: (s: ProjectState) => void): () => void {
  const b = bridge();
  if (!b) return () => undefined;
  let live = true;
  const id = b.onState((s) => {
    if (live) listener(s);
  });
  b.state()
    .then((s) => {
      if (live) listener(s);
    })
    .catch(() => undefined);
  return () => {
    live = false;
    b.offState(id);
  };
}

/** The home's requests; each reply arrives as a new state, so the calls are fire and forget. */
export const projectActions = {
  pick: () => void bridge()?.pick(),
  open: (path: string) => void bridge()?.open(path),
  forget: (path: string) => void bridge()?.forget(path),
  close: () => void bridge()?.close(),
  refresh: () => void bridge()?.refresh(),
  dismiss: () => void bridge()?.dismiss(),
};
