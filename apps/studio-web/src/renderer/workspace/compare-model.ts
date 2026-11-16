// The comparison's pictograms and names (F-815), pure so that the unit tests import them (Compare.tsx draws them; the
// rules themselves are src/shared/compare.ts).

export type CompareMode = 'side' | 'difference';

/** The pictograms of the comparison, on DESIGN.md's 14-unit grid (strokes at 0, 45 and 90 degrees). */
export const COMPARE_ICON = {
  // two frames side by side
  compare: 'M1 3H6V11H1ZM8 3H13V11H8Z',
  side: 'M1 3H6V11H1ZM8 3H13V11H8Z',
  // a delta
  difference: 'M2 10L7 5L12 10Z',
} as const;

/** The names of the controls (their accessible names and tooltips; G22: no standing words). */
export const COMPARE_NAME = {
  compare: 'Compare runs',
  other: 'Compared run',
  mode: 'Comparison view',
  side: 'Side by side',
  difference: 'Difference',
} as const;
