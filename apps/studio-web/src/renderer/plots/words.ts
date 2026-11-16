// The words and pictograms the plots use for a run's state and for a run without series (F-811, G6, G8). The service
// knows seven run states (crates/rcfd-api/src/service.rs RunState); DESIGN.md draws five pictograms. The pictogram
// never stands alone: the sentence beside it names the service's own state word.
import type { RunState } from '../ui/index.js';
import type { FeedSnapshot } from './feed.js';
import type { RunStatus } from './useRunFeed.js';

/** The pictogram of a service run state. */
export function pictogramOf(status: RunStatus | null): RunState {
  switch (status?.state) {
    case 'running':
    case 'paused':
      return 'running';
    case 'finished':
      return status.converged ? 'converged' : 'finished';
    case 'stopped':
      return 'finished';
    case 'failed':
      return 'failed';
    default:
      // defined, ready, unknown: waiting
      return 'queued';
  }
}

/** The run state in words, for sentences ("is running", "has finished without converging"). */
export function stateWords(status: RunStatus | null): string {
  switch (status?.state) {
    case undefined:
      return 'is in a state the Studio has not read yet';
    case 'defined':
      return 'is defined and has no simulation yet';
    case 'ready':
      return 'is ready and not running';
    case 'running':
      return 'is running';
    case 'paused':
      return 'is paused';
    case 'finished':
      return status.converged ? 'has finished and converged' : 'has finished without converging';
    case 'stopped':
      return 'was stopped';
    case 'failed':
      return 'has failed';
    default:
      return `is ${status?.state}`;
  }
}

/** The name of a run in sentences. */
export const runName = (session: string): string => `Session ${session}`;

/**
 * Why a plot has nothing to draw, as one sentence, or null when it has something. `what` is "residuals" or "monitor
 * values"; `have` is whether the feed holds a series of that kind.
 */
export function noSeriesSentence(feed: FeedSnapshot | null, status: RunStatus | null, session: string, what: string, have: boolean): string | null {
  const who = `session ${session}`;
  if (!feed || feed.status === 'loading') return `Reading the ${what} of ${who}.`;
  if (feed.status === 'unavailable') {
    if (!status) return `No ${what} for ${who}. The service says: ${feed.message}`;
    return `No ${what}: ${who} ${stateWords(status)}. The service says: ${feed.message}`;
  }
  if (feed.status === 'error') return `The ${what} of ${who} could not be read: ${feed.message}`;
  if (have) return null;
  if (status?.state === 'failed') return `No ${what}: ${who} failed before it recorded any.`;
  if (what === 'residuals') return `No residuals yet: ${who} ${stateWords(status)}. They appear with its first iteration.`;
  return `No monitor values: ${who} ${stateWords(status)}, and it records no series besides its residuals. The reports of a case (forces, averages) appear here.`;
}
