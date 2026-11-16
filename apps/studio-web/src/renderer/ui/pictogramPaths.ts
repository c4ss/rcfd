// The geometry of the state pictograms and interface icons, on the pictogram grid of DESIGN.md (Shapes): a 14-unit
// square, strokes only at 0, 45 and 90 degrees, square caps, mitred joins. The one circle is the queued ring.
// Plain data, so tests/unit/pictograms.spec.ts can check every segment against the grid.
//
// The state paths are the ones recorded in .impeccable/design.json (pictogramGrid.states); the icons are drawn for
// this app on the same grid.

export const RUN_STATES = ['queued', 'running', 'converged', 'finished', 'failed'] as const;
export type RunState = (typeof RUN_STATES)[number];

/** The word that always stands beside each pictogram (G8: shape and word carry the state, colour reinforces it). */
export const STATE_WORD: Record<RunState, string> = {
  queued: 'Queued',
  running: 'Running',
  converged: 'Converged',
  finished: 'Finished',
  failed: 'Failed',
};

export type Shape = { readonly path: string } | { readonly circle: { cx: number; cy: number; r: number } };

export const STATE_SHAPES: Record<RunState, Shape> = {
  queued: { circle: { cx: 7, cy: 7, r: 5 } },
  running: { path: 'M2 12V3H11M8 0L11 3L8 6' },
  converged: { path: 'M2 7L6 11L13 4' },
  finished: { path: 'M1 7H11M11 2V12' },
  failed: { path: 'M2 2L6 6M8 8L12 12M12 2L8 6' },
};

/**
 * F-806: the runner's two states that DESIGN.md has no pictogram for yet (PRODUCT.md lists paused; the runner also
 * writes cancelled). Drawn on the same grid, from the same family, and proposed for DESIGN.md (not recorded there or in
 * design.json yet, so they stay apart from RUN_STATES):
 *   paused     the running mark's L held at a gate: the path stops short and two bars stand where the arrow was
 *   cancelled  the running mark's L run into an end bar (the finished mark's bar) before its arrow: stopped on request
 */
export const RUNNER_STATES = ['paused', 'cancelled'] as const;
export type RunnerState = (typeof RUNNER_STATES)[number];

/** Every state a run can be shown in: the five of DESIGN.md and the two of RUNNER_STATES. */
export type MarkState = RunState | RunnerState;

export const RUNNER_STATE_WORD: Record<RunnerState, string> = { paused: 'Paused', cancelled: 'Cancelled' };

export const RUNNER_STATE_SHAPES: Record<RunnerState, { readonly path: string }> = {
  paused: { path: 'M2 12V3H5M8 0V6M12 0V6' },
  cancelled: { path: 'M2 12V3H10M10 0V6' },
};

/** The word and shape of any MarkState. */
export const markWord = (s: MarkState): string => (s === 'paused' || s === 'cancelled' ? RUNNER_STATE_WORD[s] : STATE_WORD[s]);
export const markShape = (s: MarkState): Shape => (s === 'paused' || s === 'cancelled' ? RUNNER_STATE_SHAPES[s] : STATE_SHAPES[s]);

export const ICON_NAMES = [
  'plus',
  'minus',
  'check',
  'close',
  'caret-down',
  'sort',
  'sort-ascending',
  'sort-descending',
  'play',
  'pause',
  'back',
  'forward',
  'first',
  'last',
  'send',
] as const;
export type IconName = (typeof ICON_NAMES)[number];

/** Icon paths. `filled` icons are filled as well as stroked (the play triangle). */
export const ICON_PATHS: Record<IconName, { readonly path: string; readonly filled?: boolean }> = {
  plus: { path: 'M7 2V12M2 7H12' },
  minus: { path: 'M2 7H12' },
  check: { path: 'M2 7L5 10L12 3' },
  close: { path: 'M3 3L11 11M11 3L3 11' },
  'caret-down': { path: 'M3 5L7 9L11 5' },
  sort: { path: 'M4 5L7 2L10 5M4 9L7 12L10 9' },
  'sort-ascending': { path: 'M7 2V12M3 6L7 2L11 6' },
  'sort-descending': { path: 'M7 2V12M3 8L7 12L11 8' },
  play: { path: 'M4 2L9 7L4 12Z', filled: true },
  pause: { path: 'M4 2V12M10 2V12' },
  back: { path: 'M9 2L4 7L9 12' },
  forward: { path: 'M5 2L10 7L5 12' },
  first: { path: 'M3 2V12M11 2L6 7L11 12' },
  last: { path: 'M11 2V12M3 2L8 7L3 12' },
  send: { path: 'M2 7H12M8 3L12 7L8 11' },
};
