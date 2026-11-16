// Where the first path's result leads (F-813): the one function every "Open the run" link of the path calls, so the
// destination can be changed in one place. Today it is the run selected in the Runs view (F-806), whose panel holds every
// number the runner recorded; when the run workspace (F-810: field viewport, run bands, scrubber and plots of a project
// run, opened from the Runs view) is merged, repoint this function at it.

export interface ResultNavigation {
  /** Selects the run in the Runs view and shows that view (the project view's own openRun). */
  openRun(run: string): void;
}

/** The words of the link, kept with its target so they change together. */
export const RESULT_LINK = { label: 'Open the run in Runs', where: 'the Runs view, with this run selected' } as const;

export function openFirstResult(run: string, nav: ResultNavigation): void {
  nav.openRun(run);
}
