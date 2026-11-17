// A learner's first path (F-813; docs/studio-guardrails.md G2, G6, G15, G16, G17): the example case it makes, the run
// action it starts, and the words of each step. Shared by the renderer, the main process and the tests: no Node or DOM
// types.
//
// The path makes nothing itself. The example case is the engine's built-in template `lid_driven_cavity` (the case of
// docs/tutorials/quick-start-cli.md), written by the engine's `new_case` through the case port (F-812). The run is the
// engine's `start_run` through run control (F-807) with the engine's start plan (`store: plan`), so the run's fields are
// stored and the first result can be a picture; the plan's rule and its numbers are the engine's (crates/rcfd-workspace
// src/plan.rs), the Studio only asks for it.
//
// `first.run_example` is a run-control action like those of runs.ts, kept in its own list: RUN_ACTIONS is the F-807
// list its test pins, and this one differs only in asking for the plan. src/main/project/control.ts looks actions up in
// both lists.
import type { ParamsSpec } from './params.js';
import type { RegisteredRunAction, RunActionContext, RunActionId, RunActionSpec } from './runs.js';

/** The engine's built-in template the path makes its case from (`new_case {template}`). */
export const FIRST_TEMPLATE = 'lid_driven_cavity';
/** The name the path gives the case: `cases/first-cavity.toml`. */
export const FIRST_CASE_NAME = 'first-cavity';
export const FIRST_CASE = `cases/${FIRST_CASE_NAME}.toml`;
/** The stored field the first picture shows (the engine's default store keeps ux, uy, uz and p). */
export const FIRST_FIELD = 'ux';

const NO_PROJECT = 'No project is open: open a project folder first (Open project folder…).';

const caseParam = { type: 'string', description: 'The case file, project-relative (cases/first-cavity.toml).' } as const;

const RUN_EXAMPLE: RunActionSpec<{ readonly case: typeof caseParam }> = {
  title: 'Run the first example',
  description:
    "Start a run of a project case with the engine's start plan: the run is queued with the project's runner (started if none is alive), and the plan stores its fields (about 100 pictures over the run) so that the result can be shown, not only its numbers.",
  group: 'run',
  params: { case: caseParam },
  engine: { kind: 'mcp', tool: 'start_run' },
  mcpArgs: (p) => ({ case: p.case, store: 'plan' }),
  cli: (p, project) => ['rcfd-runner', 'start', p.case, '--project', project],
  cliNote:
    'rcfd-runner start has no start plan: add the --set entries the engine answered with (store.set, shown under the result) for the same run. It writes no line in the project journal; start_run does.',
  unavailable: (ctx: RunActionContext, p) => {
    if (ctx.project === false) return NO_PROJECT;
    if (p.case !== undefined && ctx.cases !== undefined && !ctx.cases.includes(p.case)) {
      return `${p.case} is not in the project's cases/ folder: it may have been moved or deleted. Make the example again.`;
    }
    return null;
  },
  expect: 'new_run',
};

/**
 * The first path's run-control actions. The id is not one of runs.ts's RunActionId (that union is RUN_ACTIONS's own); the
 * cast is the one place this list is joined to the F-807 machinery, which looks actions up by string.
 */
export const FIRST_RUN_ACTIONS: readonly RegisteredRunAction[] = [
  Object.freeze({ ...(RUN_EXAMPLE as unknown as RunActionSpec<ParamsSpec>), id: 'first.run_example' as unknown as RunActionId, effect: 'change' as const }),
];

export const FIRST_RUN_EXAMPLE = 'first.run_example';

export function getFirstRunAction(id: string): RegisteredRunAction | undefined {
  return FIRST_RUN_ACTIONS.find((a) => (a.id as string) === id);
}

/**
 * Where the full control of each step lives (G15: the short path hides nothing for good). Only places that exist in this
 * build are named; tests/unit/first-path.spec.ts checks the labels against the controls the Studio shows.
 */
export const FULL_CONTROL = {
  make: 'New case in the header band offers every built-in template and shipped example; Edit on the case opens the case editor with every setting.',
  run: 'The run bar in Runs: Run, and Queue… with a priority, threads, a memory budget and settings for one run; Pause, Stop… and Cancel… on the selected run.',
  watch: 'Runs: the run list, sorted and filtered, with every number the runner recorded and its state changes.',
  result: 'The run workspace: every stored picture of the run with its residuals and monitors on one time line; Runs: every number of the run; the case editor: every setting, to change one and run again.',
} as const;
