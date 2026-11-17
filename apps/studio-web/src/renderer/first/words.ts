// The words of the first path (F-813) that are not computed: the disclaimer in the repository's own wording, the limits
// of the example, and the precise terms one step away from each plain word (G19: from the engine's documentation, named
// with their source). tests/unit/first-path.spec.ts checks the facts these words name against the engine's sources (the
// template's text, the README's disclaimer), so a change there shows up here.
import { FIRST_CASE, FIRST_TEMPLATE } from '../../shared/actions/first.js';

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
  /** The precise term and what it means. */
  readonly precise: string;
  /** Where the engine's documentation says it. */
  readonly source: string;
}

export const TERMS: Readonly<Record<'make' | 'run' | 'watch' | 'result', readonly Term[]>> = {
  make: [
    {
      plain: 'the example',
      precise: `a case file, ${FIRST_CASE}: a TOML file that describes one simulation (mesh, physics, boundaries, solver and reports)`,
      source: 'docs/glossary.md, Case file',
    },
    {
      plain: 'a square box whose lid slides',
      precise: `a lid-driven cavity: the engine’s built-in template ${FIRST_TEMPLATE}, the case of the command-line quick start`,
      source: 'docs/tutorials/quick-start-cli.md',
    },
    {
      plain: 'made by the engine',
      precise: `new_case {name, template: "${FIRST_TEMPLATE}"}: the engine checks the case before it writes the file, never replaces a file, and keeps a line in the project journal`,
      source: 'docs/mcp-tools.md, new_case',
    },
  ],
  run: [
    {
      plain: 'a run',
      precise: 'one solve of a case by the project’s runner (rcfd-runner), kept in runs/<run id>/; it goes on when the Studio is closed',
      source: 'docs/ui-restart-plan.md §8',
    },
    {
      plain: 'pictures along the way',
      precise: 'the engine’s start plan (start_run with store: plan): the fields are stored about 100 times over the run, in single precision (output.store_interval)',
      source: 'docs/mcp-tools.md, start_run',
    },
    {
      plain: 'rounds of calculation',
      precise: 'iterations: the solver (SIMPLEC in this case) repeats until the residuals fall below the case’s tolerances (solver.tolerance_u, solver.tolerance_p) or it reaches solver.max_iterations',
      source: 'docs/case-schema.md, solver',
    },
  ],
  watch: [
    { plain: 'Waiting its turn', precise: 'queued: the runner launches it when a slot is free', source: 'docs/run-control.md' },
    { plain: 'Working', precise: 'running: the solver is computing; the round count is its iteration', source: 'docs/run-control.md' },
    {
      plain: 'the runner',
      precise: 'rcfd-runner, the project’s run service: it owns every run, and Start the runner is rcfd-runner ensure',
      source: 'rcfd-runner --help',
    },
  ],
  result: [
    { plain: 'settled', precise: 'converged: the engine’s words “residual targets met”, with the residuals it reached', source: 'the run’s status.json' },
    { plain: 'velocity to the right', precise: 'ux: the x component of the velocity, as stored in the run’s field store (runs/<run id>/fields)', source: 'the store’s field list' },
    {
      plain: 'round, under the picture',
      precise: 'step: the solver iteration at which the picture was stored, the last of the snapshots in the run’s field store (output.store_interval); the snapshot number and its time t are said to screen readers and in the number’s provenance',
      source: 'docs/mcp-tools.md, start_run',
    },
    {
      plain: 'the picture',
      precise: 'the boundary surface of the mesh, drawn as triangles, each coloured by the stored value of the cell that owns it, on the viridis colour map from the lowest to the highest value shown',
      source: 'docs/adr/0801-studio-web-dependencies-and-layout.md, Addendum 5 (the field viewport)',
    },
    {
      plain: 'the force of the fluid on the lid',
      precise: 'lid_drag: the case’s report, the viscous force on the lid along x (reports.lid_drag); the engine gives no unit for report values',
      source: `${FIRST_CASE}, [reports.lid_drag]`,
    },
  ],
};
