// The words of the first path (F-813) that are not computed: the disclaimer in the repository's own wording, the limits
// of the example, and which glossary term stands one step away from each plain word. The precise terms are the engine's
// (F-830, G19): their meaning is the row of docs/glossary.md the engine returns from `help.glossary` (rcfd serve --ui),
// shown with its source; this file keeps no text of its own for them. tests/unit/first-path.spec.ts checks the facts
// these words name against the engine's sources (the template's text, the README's disclaimer, the glossary's terms), so
// a change there shows up here.
import { FIRST_CASE } from '../../shared/actions/first.js';

/** README.md, verbatim (the project's own disclaimer, G6). */
export const DISCLAIMER =
  'rcfd is early-stage software. It has not been validated for engineering decisions and must not be used for safety-critical work. It comes with no warranty of any kind (see the licences).';

/** What this result is not (PRODUCT.md, Evidence on Hand; example-projects/quick-start/notes, Limits): a known miss (G6). */
export const EXAMPLE_LIMITS =
  'Not a validation: a coarse grid, and no check on a finer one. Settled means the calculation stopped changing, not that the answer is right.';

/**
 * What each step is and what it will do, in plain words: help on demand behind Explain (G19, G22), not standing text.
 */
export const STEP_HELP: Readonly<Record<'make' | 'run' | 'watch' | 'result', readonly string[]>> = {
  make: [
    'The engine writes a small example into this project: a square box of fluid whose top, the lid, slides sideways and drags the fluid round inside it. It is the first example of most flow solvers, and it finishes in seconds.',
    `What happens: the engine checks the example and writes one file, ${FIRST_CASE}, into the project. Nothing else changes, and nothing is computed yet.`,
  ],
  run: [
    'Running hands the example to the project’s runner, which solves it in the background: the solver repeats one round of calculation after another until the answer stops changing, and stores pictures of the flow along the way.',
    'What happens: a new run folder appears under runs/, the runner starts if it is not already going, and the run goes on even if you close the Studio.',
  ],
  watch: [
    'The round count goes up as the solver works; the run usually ends within seconds, and the result then appears here. Runs shows the same run with every number the runner keeps.',
  ],
  result: [
    'The numbers are the engine’s own: the rounds of calculation, the solver’s time, and the report the case asks for, the force of the fluid on the lid along its motion. The picture is the stored field at the end of the run; each colour is the value the legend gives it, and pointing at the box reads the value there.',
    'Under the picture: the run, what the picture shows with its unit, and the round of calculation it was stored at. Clicking a number, or Ctrl+I on it, shows where the engine wrote it. Open the run shows every stored picture of the run, with its rounds and numbers moving together.',
  ],
};

export interface Term {
  /** The plain word on the path. */
  readonly plain: string;
  /** The term of docs/glossary.md whose meaning is shown for it (help.glossary). */
  readonly term: string;
}

/** Where the precise terms come from (G19). */
export const TERMS_SOURCE = 'docs/glossary.md';

export const TERMS: Readonly<Record<'make' | 'run' | 'watch' | 'result', readonly Term[]>> = {
  make: [
    { plain: 'the example', term: 'Case file' },
    { plain: 'a square box whose lid slides', term: 'Lid-driven cavity' },
    { plain: 'made by the engine', term: 'Template' },
  ],
  run: [
    { plain: 'a run', term: 'Run' },
    { plain: 'pictures along the way', term: 'Field store' },
    { plain: 'rounds of calculation', term: 'Iteration' },
  ],
  watch: [
    { plain: 'Waiting its turn', term: 'Queued' },
    { plain: 'Working', term: 'Running' },
    { plain: 'the runner', term: 'Runner' },
  ],
  result: [
    { plain: 'settled', term: 'Converged' },
    { plain: 'velocity to the right', term: 'Field' },
    { plain: 'round, under the picture', term: 'Snapshot' },
    { plain: 'the picture', term: 'Field view' },
    { plain: 'the force of the fluid on the lid', term: 'Report' },
  ],
};
