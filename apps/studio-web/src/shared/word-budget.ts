// F-833 (G22, The Quiet Rule of DESIGN.md): the budget of visible chrome words at rest, per built surface, and the
// marks that tell the counter which visible words are not chrome.
//
// What is counted (written before the first count, 2026-11-16):
//
// - A word is a whitespace-separated token with at least one letter or digit in it ("Ctrl+K" is one word, "·" and "…"
//   are none).
// - It is counted when it is visible chrome of the surface at rest: in a text node, or in the placeholder of an empty
//   text field, inside the surface's root element (and outside the roots of the surfaces nested in it), and rendered:
//   the element is laid out (Element.checkVisibility with visibility and opacity), and the text's box is at least 2 by 2
//   CSS pixels, and no ancestor clips it away (a box under 2 pixels with overflow or a clip path, or the .sr-only
//   class's clip-path inset(50%)), so visually hidden text is not counted. Words outside the window are counted too
//   (a scrolled list is still the surface).
// - Not counted, because they are not chrome: text inside an element whose nearest `data-words` attribute is
//     data        a value from the engine, the project or the user: names, paths, ids, numbers with their units,
//                 times, counts, the user's and Claude Code's messages, the schema's key names and options
//     exception   a failure, a loss, a conflict, something blocked or unavailable, or a value the user must act on
//                 (G22: these speak)
//     disclaimer  G6's disclaimer (the footer band)
//     help        help the user asked for: an opened Explain region, the command palette's list of commands and
//                 the facts of the focused one, the shortcut list (G22 moves words there)
//   A nearest `data-words="chrome"` counts again (a caption inside a data block).
// - Never counted: accessible names, descriptions and tooltips that are not open (they are not on the screen).
//
// Each surface's budget is the AFTER count of its screenshots (test-results-visual/f833/after/), approved by the
// maintainer on 2026-11-17 (ADR 0801 Addendum 11) and enforced: tests/e2e/word-budget.spec.ts counts every surface from
// the DOM and fails past its budget. Raising a budget needs the maintainer's word (G22).

/** The attribute that marks visible words that are not chrome (see above). */
export const WORDS_ATTR = 'data-words';

export type WordKind = 'data' | 'exception' | 'disclaimer' | 'help' | 'chrome';

/** Props for an element whose visible words are of one kind: `<span {...words('data')}>`. */
export const words = (kind: WordKind): { 'data-words': WordKind } => ({ 'data-words': kind });

export interface SurfaceBudget {
  /** Where the surface is, in the words of F-833's acceptance. */
  readonly what: string;
  /** The surface's state at rest, as the word-budget test brings it there. */
  readonly rest: string;
  /** The largest number of visible chrome words at rest. */
  readonly budget: number;
}

/** APPROVED BY THE MAINTAINER 2026-11-17 (F-833): each budget is the AFTER count of the sweep, enforced by the test. */
export const BUDGET_STATUS = 'approved by the maintainer 2026-11-17, enforced' as const;

export const WORD_BUDGETS = {
  'home-empty': { what: 'project home, no project open', rest: 'first start, an empty recent list', budget: 6 },
  'first-path-intro': { what: 'learner’s first path, no project open', rest: 'the four stations, Explain closed', budget: 22 },
  'home-open': { what: 'project home, a project open', rest: 'the quick-start project, Home view, no filter', budget: 49 },
  'first-path': { what: 'learner’s first path, an empty project', rest: 'the make step, Explain closed', budget: 22 },
  runs: { what: 'runs', rest: 'Runs view, nothing selected', budget: 14 },
  'runs-selected': { what: 'runs, the selected run', rest: 'a converged run selected', budget: 16 },
  workspace: { what: 'run workspace', rest: 'a converged run, its last snapshot drawn', budget: 17 },
  compare: { what: 'run workspace, compare mode', rest: 'two runs side by side', budget: 8 },
  'derived-controls': { what: 'run workspace, derived-picture controls', rest: 'the field toolbar, no cut, no arrows', budget: 0 },
  provenance: { what: 'provenance popover', rest: 'opened on a run’s step count', budget: 11 },
  claude: { what: 'Claude Code panel', rest: 'a new chat in the quick-start project', budget: 13 },
  palette: { what: 'command palette', rest: 'opened, nothing typed', budget: 4 },
  settings: { what: 'settings', rest: 'a session from the engine’s defaults, Set in this case', budget: 23 },
  'case-editor': { what: 'case editor', rest: 'a case file opened, nothing changed', budget: 51 },
  undo: { what: 'undo controls', rest: 'nothing to undo or redo', budget: 0 },
  decision: { what: 'decision panel', rest: 'Close session asked', budget: 0 },
  progress: { what: 'progress rows', rest: 'a project opening that lasts', budget: 3 },
} as const satisfies Readonly<Record<string, SurfaceBudget>>;

export type SurfaceId = keyof typeof WORD_BUDGETS;
