import type { RunState } from './data';
import type { JSX } from 'react';

// State pictograms drawn on a 14-unit grid, strokes only at 0, 45 and 90 degrees (a circle for "waiting").
// Each is always shown next to its word: shape and word carry the state, colour only reinforces it (guardrail G8).
const PATHS: Record<RunState, JSX.Element> = {
  queued: <circle cx="7" cy="7" r="5" />,
  running: <path d="M2 12V3H11M8 0L11 3L8 6" />,
  converged: <path d="M2 7L6 11L13 4" />,
  finished: <path d="M1 7H11M11 2V12" />,
  failed: <path d="M2 2L6 6M8 8L12 12M12 2L8 6" />,
};

export function Pictogram({ state, size = 14 }: { state: RunState; size?: number }) {
  return (
    <svg className={`picto picto-${state}`} width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
      {PATHS[state]}
    </svg>
  );
}

type IconName = 'first' | 'back' | 'play' | 'pause' | 'forward' | 'last' | 'live' | 'send' | 'plus';

const ICONS: Record<IconName, JSX.Element> = {
  first: <path d="M3 2V12M12 2L6 7L12 12" />,
  back: <path d="M10 2L5 7L10 12" />,
  play: <path d="M4 2L11 7L4 12Z" />,
  pause: <path d="M4 2V12M10 2V12" />,
  forward: <path d="M4 2L9 7L4 12" />,
  last: <path d="M11 2V12M2 2L8 7L2 12" />,
  live: <><circle cx="7" cy="7" r="2" /><path d="M2 2L4 4M12 2L10 4M2 12L4 10M12 12L10 10" /></>,
  send: <path d="M2 7H12M8 3L12 7L8 11" />,
  plus: <path d="M7 2V12M2 7H12" />,
};

export function Icon({ name }: { name: IconName }) {
  return (
    <svg className="icon" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
      {ICONS[name]}
    </svg>
  );
}
