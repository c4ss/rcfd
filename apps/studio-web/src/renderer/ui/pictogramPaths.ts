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
  // F-837
  'log',
  'report',
  'image',
  'budgets',
  'range-snapshot',
  'range-run',
  'range-fixed',
  'face-on',
  'log-scale',
  'stored',
  // F-818
  'theme',
  'settings',
  // F-835
  'new-project',
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
  // F-837: a run's log (lines of text), its report (a page with a folded corner), Save PNG (a picture), the runner's
  // budgets (two sliders), the colour range of one snapshot (one frame), of the whole run (frames stacked), fixed (a span
  // with two stops), the face-on view (a square seen square, its centre marked), a log scale (equal steps of a staircase)
  log: { path: 'M2 3H12M2 6H12M2 9H12M2 12H8' },
  report: { path: 'M3 1H9L12 4V13H3ZM9 1V4H12M5 7H10M5 10H10' },
  image: { path: 'M1 2H13V12H1ZM1 10L5 6L8 9L10 7L13 10' },
  budgets: { path: 'M2 4H12M2 10H12M5 2V6M9 8V12' },
  'range-snapshot': { path: 'M4 2H10V12H4Z' },
  'range-run': { path: 'M2 4H8V12H2ZM5 4V2H11V10H8' },
  'range-fixed': { path: 'M7 2V12M4 2H10M4 12H10' },
  'face-on': { path: 'M2 2H12V12H2ZM6 6H8V8H6Z' },
  'log-scale': { path: 'M2 12H5V9H8V6H11V3' },
  // F-837: Run again with fields stored (the play mark over a shelf of stored snapshots)
  stored: { path: 'M4 1L8 5L4 9ZM1 12H13M1 9V12M13 9V12' },
  // F-818: the theme (a square cut on its diagonal, the lower half hatched at 45 degrees: one ground light, one dark)
  theme: { path: 'M2 2H12V12H2ZM2 12L12 2M5 12L12 5M8 12L12 8' },
  // F-818: the Studio's settings (three rails, each with its stop at a different place)
  settings: { path: 'M1 3H13M1 7H13M1 11H13M4 1V5M10 5V9M6 9V13' },
  // F-835: New project… (a folder with its tab, a plus inside)
  'new-project': { path: 'M1 3H5L6 4H13V12H1ZM7 6V10M5 8H9' },
};
