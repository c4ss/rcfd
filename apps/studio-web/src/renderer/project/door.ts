// The project home's one door to the main process (F-805): the only renderer code that touches window.studioProject.
// The project folder is not reached through the action dispatcher (src/renderer/actions, F-827): the folder dialog, the
// recent list and the project server's read-only queries are main-process operations behind the project port's
// allow-list (src/shared/project.ts, src/main/project/bridge.ts), not service methods, and have no action-registry
// entry yet (G16; reported with F-805).
// F-807: run control goes over the same project port (window.studioRuns, op `action`): every control calls
// `runAction(id, params)`, an action of src/shared/actions/runs.ts, and asks `runActionAvailability` and
// `runActionConfirmation` for its state and words; the main process checks the action again and makes the engine call.
import type { ProjectState, ProjectView, RunActionResult } from '../../shared/project.js';
import {
  contextOf,
  getRunAction,
  runAvailability,
  runConfirmation,
  type Confirmation,
  type RunActionId,
  type RunActionParams,
  type RunAvailability,
} from '../../shared/actions/runs.js';

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

/**
 * F-832 (G21): the function that cancels the opening asked for last (the project port's `cancel`, handed over by the
 * preload script); the opening shows its progress and Cancel from the state's `opening`.
 */
let cancelLastOpen: (() => void) | null = null;
const keepCancel = { onCancel: (cancel: () => void) => void (cancelLastOpen = cancel) };

/** F-832: cancels the opening that is going (nothing when none is). */
export function cancelOpening(): void {
  cancelLastOpen?.();
}

/** The home's requests; each reply arrives as a new state, so the calls are fire and forget. */
export const projectActions = {
  pick: () => void bridge()?.pick(keepCancel),
  open: (path: string) => void bridge()?.open(path, keepCancel),
  forget: (path: string) => void bridge()?.forget(path),
  close: () => void bridge()?.close(),
  refresh: () => void bridge()?.refresh(),
  dismiss: () => void bridge()?.dismiss(),
  /** F-835: the engine makes a project in a folder chosen in the folder dialog, then it opens. */
  create: () => void bridge()?.create('pick', keepCancel),
  /** F-835: the engine makes a project in the folder it has just refused as no project, then it opens. */
  createHere: () => void bridge()?.create('here', keepCancel),
};

/** F-807: dispatches a run-control action; resolves with the engine's answer or the refusal in words (never rejects). */
export async function runAction<Id extends RunActionId>(id: Id, params: RunActionParams<Id>): Promise<RunActionResult> {
  const b = window.studioRuns;
  if (!b) {
    return {
      ok: false,
      request: null,
      failure: { code: 'disconnected', problem: 'This window has no run control.', recovery: 'Reload the window (Ctrl+R); runs are not affected.', engine: null },
    };
  }
  return b.dispatch(id, params as Readonly<Record<string, unknown>>);
}

/** F-807: whether a run-control action is available on this view, and why not. */
export function runActionAvailability<Id extends RunActionId>(id: Id, params: Partial<RunActionParams<Id>>, view: ProjectView | null): RunAvailability {
  const a = getRunAction(id)!;
  const p = params as Record<string, unknown>;
  return runAvailability(a, view ? contextOf(view, p) : { project: false }, p);
}

/** F-807: the words of the confirmation the action asks for (G18), or null. */
export function runActionConfirmation<Id extends RunActionId>(id: Id, params: RunActionParams<Id>, view: ProjectView): Confirmation | null {
  const p = params as Record<string, unknown>;
  return runConfirmation(getRunAction(id)!, contextOf(view, p), p);
}
