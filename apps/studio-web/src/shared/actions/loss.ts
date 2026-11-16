// What an action that cannot be undone loses (F-829, guardrail G18): the words the Studio shows before it runs one, so
// the user confirms knowing what goes. Every change action of the registry either has an inverse (undo.ts can take it
// back) or an entry here; tests/unit/history.spec.ts fails when one has neither.
//
// The facts are the engine's (run.status: state and iteration; the history: the edits made in the Studio). The rules
// quoted are crates/rcfd-api/src/service.rs: run.initialize always builds the simulation again; run.start and run.step
// build it again from a finished, stopped or failed run; settings.set (except live turbulence) and settings.remove
// discard the built simulation; session.close removes the session and everything it holds.
//
// Shared by the renderer, the main process and the tests: no Node or DOM types.
import type { JsonObject } from './params.js';

/** What the Studio knows of a session when it asks what an action would lose. Anything left out counts as unknown. */
export interface SessionFacts {
  /** run.status `state`. */
  readonly state?: string | null;
  /** run.status `iteration`. */
  readonly iteration?: number | null;
  /** Titles of the edits made in the Studio that only the session holds (the history's steps), newest last. */
  readonly edits?: readonly string[];
}

const ENDED = new Set(['finished', 'stopped', 'failed']);

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

/** The facts of a run.status result: `state`, and the iteration of its built `simulation` (none before one is built). */
export function statusFacts(result: unknown): SessionFacts {
  if (!isObject(result)) return {};
  const sim = isObject(result['simulation']) ? result['simulation'] : null;
  const it = sim && typeof sim['iteration'] === 'number' ? sim['iteration'] : typeof result['iteration'] === 'number' ? result['iteration'] : null;
  return { state: typeof result['state'] === 'string' ? result['state'] : null, iteration: it };
}

/** Whether the session holds computed results: an ended run, or a run that has taken steps. */
export function hasResults(f: SessionFacts): boolean {
  return (typeof f.iteration === 'number' && f.iteration > 0) || (typeof f.state === 'string' && ENDED.has(f.state));
}

/** "its results (finished, iteration 340)". */
export function resultsWords(f: SessionFacts): string {
  const bits = [f.state ?? null, typeof f.iteration === 'number' ? `iteration ${f.iteration}` : null].filter((x): x is string => x !== null);
  return `its results${bits.length ? ` (${bits.join(', ')})` : ''}`;
}

const LISTED = 4;

function editsWords(edits: readonly string[]): string {
  if (edits.length === 0) return '';
  const shown = edits.slice(-LISTED).reverse();
  return ` (${edits.length} ${edits.length === 1 ? 'change' : 'changes'} made here: ${shown.join(', ')}${edits.length > LISTED ? ', …' : ''})`;
}

/** The change actions that have no inverse: undo cannot take them back. */
export type NoInverseAction = 'session.close' | 'run.initialize' | 'run.start' | 'run.step' | 'run.pause' | 'run.resume' | 'run.stop';

/**
 * For each change action without an inverse: what it loses with these parameters and facts, in words, or null when it
 * loses nothing (then it runs without asking). A loss is never guessed down: unknown facts count as "may hold".
 */
export const NO_INVERSE: Readonly<Record<NoInverseAction, (p: JsonObject, f: SessionFacts) => string | null>> = {
  'session.close': (p, f) => {
    const s = String(p['session'] ?? '');
    const lost = [`its settings, which exist only in the service${editsWords(f.edits ?? [])}`];
    if (f.state !== 'defined') lost.push('its built mesh and fields');
    if (hasResults(f) || f.state === undefined || f.state === null) lost.push(hasResults(f) ? resultsWords(f) : 'any results it holds');
    return (
      `Session ${s} is removed from the service. Lost: ${lost.join('; ')}. Closing cannot be undone: ` +
      'the session journal (journal.get) can make a new session with the same settings, not the same results.'
    );
  },
  'run.initialize': (p, f) =>
    hasResults(f) ? `Initializing builds session ${String(p['session'] ?? '')} again from its settings: ${resultsWords(f)} are discarded.` : null,
  'run.start': (p, f) =>
    typeof f.state === 'string' && ENDED.has(f.state)
      ? `Session ${String(p['session'] ?? '')} starts again from its initial state: ${resultsWords(f)} are replaced by the new run's.`
      : null,
  'run.step': (p, f) =>
    typeof f.state === 'string' && ENDED.has(f.state)
      ? `Session ${String(p['session'] ?? '')} is built again before the steps: ${resultsWords(f)} are discarded.`
      : null,
  // pausing, resuming and stopping keep everything computed (a stopped run's results stay in the session)
  'run.pause': () => null,
  'run.resume': () => null,
  'run.stop': () => null,
};

export function isNoInverseAction(id: string): id is NoInverseAction {
  return Object.hasOwn(NO_INVERSE, id);
}

/** What an action loses (null: nothing, or it can be undone and loses nothing on the way). */
export function lossOf(action: string, params: JsonObject, facts: SessionFacts): string | null {
  if (isNoInverseAction(action)) return NO_INVERSE[action](params, facts);
  if (action === 'settings.set' || action === 'settings.remove') return settingsLoss(String(params['session'] ?? ''), facts, String(params['path'] ?? ''));
  return null;
}

/**
 * A settings change (or its undo) returns the session to defined and discards its built simulation, results included
 * (live turbulence settings of a ready or paused run excepted, which the engine applies in place).
 */
export function settingsLoss(session: string, f: SessionFacts, path = ''): string | null {
  if (path.startsWith('turbulence.') && (f.state === 'ready' || f.state === 'paused')) return null;
  return hasResults(f) ? `A settings change returns session ${session} to defined and discards its built simulation and ${resultsWords(f)}.` : null;
}
