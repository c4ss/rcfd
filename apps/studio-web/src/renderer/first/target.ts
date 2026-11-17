// Where the first path's links lead (F-813): the one place each destination is decided. The result opens the run in the
// run workspace (F-810: the field viewport, run bands, scrubber and plots of the run); a run still going is followed in
// the Runs view (F-806), whose panel shows its state changes as the runner records them.

export interface ResultNavigation {
  /** Selects the run in the Runs view and shows that view (the project view's own openRun). */
  openRun(run: string): void;
  /** Selects the run and shows the run workspace (F-810). */
  openWorkspace(run: string): void;
}

/** The words of the result link, kept with its target so they change together. */
export const RESULT_LINK = { label: 'Open the run', name: 'Open the run in the workspace', where: 'the run workspace, with this run selected' } as const;

/** The words of the link of a run still going. */
export const FOLLOW_LINK = { label: 'Follow it in Runs', where: 'the Runs view, with this run selected' } as const;

/** The result: the run in the run workspace. */
export function openFirstResult(run: string, nav: ResultNavigation): void {
  nav.openWorkspace(run);
}

/** A run still going: the run in the Runs view. */
export function followFirstRun(run: string, nav: ResultNavigation): void {
  nav.openRun(run);
}
