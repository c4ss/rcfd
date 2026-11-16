// Which series a plot draws and how each line looks (F-811). Colours are roles of tokens.ts (never palette names), and
// every series also has its own dash pattern and its name beside its swatch, so colour is never the only thing that
// tells two lines apart (G8). Pure code.
import { palette, role, type RoleName } from '../design/tokens.js';

/** The residual series of the engine: `continuity` and every `res_*` (crates/rcfd-api/src/sim.rs, rcfd-post html_report). */
export function isResidual(name: string): boolean {
  return name === 'continuity' || name.startsWith('res_');
}

export interface LineStyle {
  /** The role that colours the line (and its swatch). */
  readonly role: RoleName;
  /** Width in CSS pixels. */
  readonly width: number;
  /** Dash and gap lengths in CSS pixels; empty for a solid line. */
  readonly dash: readonly [number, number] | readonly [];
}

// The prototype's residual plot (apps/studio-prototype, styles.css .res-*) drew continuity solid in ink, u dashed 6 3 and
// p dotted 2 3 in a deep green; DESIGN.md gives meadow-deep to "the related residual p line". The prototype's sky for
// u is not used: sky's job is the header band and iterating (the Colour Has a Job Rule), so u is ink, told apart by its
// dashes. Further residuals (T, k, omega, nut, rho, ...) take ink-2 with their own dash lengths, in name order.
const RESIDUAL_STYLES: Readonly<Record<string, LineStyle>> = {
  continuity: { role: 'text', width: 2, dash: [] },
  res_u: { role: 'text', width: 2, dash: [6, 3] },
  res_p: { role: 'state-converged', width: 2, dash: [2, 3] },
};
const FURTHER_DASHES: readonly (readonly [number, number])[] = [
  [10, 3],
  [4, 4],
  [1, 3],
  [8, 6],
  [3, 2],
  [12, 4],
];

/** The style of each residual series, by name; the further ones in name order (stable for a run). */
export function residualStyles(names: readonly string[]): Map<string, LineStyle> {
  const out = new Map<string, LineStyle>();
  let k = 0;
  for (const name of [...names].sort()) {
    const fixed = RESIDUAL_STYLES[name];
    if (fixed) out.set(name, fixed);
    else out.set(name, { role: 'text-2', width: 1.75, dash: FURTHER_DASHES[k++ % FURTHER_DASHES.length]! });
  }
  return out;
}

/** The monitor chart's one line: ink, 1.75 px, solid (the prototype's .cd-chart). */
export const MONITOR_STYLE: LineStyle = { role: 'text', width: 1.75, dash: [] };

/** The hex colour of a role, for three.js materials (CSS uses var(--role-*)). */
export function roleHex(r: RoleName): string {
  return palette[role[r]];
}

/** The order in which a run's residual series are listed: continuity, then u, p, then the rest by name. */
export function residualOrder(names: readonly string[]): string[] {
  const rank = (n: string) => (n === 'continuity' ? 0 : n === 'res_u' ? 1 : n === 'res_p' ? 2 : 3);
  return [...names].sort((a, b) => rank(a) - rank(b) || (a < b ? -1 : a > b ? 1 : 0));
}
