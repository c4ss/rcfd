// F-816 (G8, G5): axe-core (MPL-2.0, ADR 0801 Addendum 12) run on a surface of the packaged Studio. The maintainer's
// word of 2026-11-17: keep axe-core, without going on to full accessibility work. So the check fails only on violations
// of impact "serious" or "critical", and leaves out:
//
// - color-contrast (and its enhanced form): there is no contrast requirement (the maintainer, 2026-11-16), and the
//   colours of DESIGN.md change only with the maintainer's approval;
// - the rules of EXCLUDED below, each with its reason: a violation whose fix would change DESIGN.md or a visible design.
//
// axe is put into the page by the DevTools protocol (Playwright's evaluate), which the page's Content-Security-Policy
// does not govern; nothing is added to the app itself.
import type { Page } from '@playwright/test';
import type { AxeResults, ImpactValue } from 'axe-core';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const AXE_SOURCE = readFileSync(createRequire(import.meta.url).resolve('axe-core/axe.min.js'), 'utf8');

/** Rules never run: no contrast requirement, and DESIGN.md's colours are the maintainer's. */
export const OFF = ['color-contrast', 'color-contrast-enhanced'] as const;

/**
 * Serious or critical rules left out because the fix would change DESIGN.md or a visible design (rule id: reason).
 * Each entry is a maintainer's question, listed in ADR 0801 Addendum 12.
 */
export const EXCLUDED: Readonly<Record<string, string>> = {};

/** The impacts that fail the check. */
export const FAILING: ReadonlySet<ImpactValue> = new Set<ImpactValue>(['serious', 'critical']);

export interface AxeFinding {
  readonly rule: string;
  readonly impact: string;
  readonly help: string;
  /** The CSS selectors of the nodes (at most 8), with axe's summary of the first. */
  readonly nodes: string[];
  readonly summary: string;
  /** The first node's HTML, cut short. */
  readonly html: string;
}

export interface AxeReport {
  /** Serious or critical, not excluded: these fail. */
  readonly failing: AxeFinding[];
  /** Serious or critical, excluded with a reason. */
  readonly excluded: AxeFinding[];
  /** Minor or moderate: recorded, not acted on. */
  readonly other: AxeFinding[];
  readonly passes: number;
}

/** Runs axe on the whole document (or `include`, a selector) and sorts the violations. */
export async function axeCheck(page: Page, include?: string): Promise<AxeReport> {
  const injected = await page.evaluate(() => typeof (window as unknown as { axe?: unknown }).axe === 'object');
  if (!injected) await page.evaluate(AXE_SOURCE);
  const res = (await page.evaluate(
    async ({ include, off }) => {
      const axe = (window as unknown as { axe: { run(ctx: unknown, opts: unknown): Promise<unknown> } }).axe;
      const rules = Object.fromEntries(off.map((r) => [r, { enabled: false }]));
      return axe.run(include ? { include: [include] } : document, { rules, resultTypes: ['violations'] });
    },
    { include: include ?? null, off: [...OFF] },
  )) as AxeResults;
  const finding = (v: AxeResults['violations'][number]): AxeFinding => ({
    rule: v.id,
    impact: v.impact ?? 'unknown',
    help: v.help,
    nodes: v.nodes.slice(0, 8).map((n) => n.target.join(' ')),
    summary: (v.nodes[0]?.failureSummary ?? '').replace(/\s+/g, ' ').slice(0, 300),
    html: (v.nodes[0]?.html ?? '').replace(/\s+/g, ' ').slice(0, 300),
  });
  const serious = res.violations.filter((v) => v.impact && FAILING.has(v.impact));
  return {
    failing: serious.filter((v) => !(v.id in EXCLUDED)).map(finding),
    excluded: serious.filter((v) => v.id in EXCLUDED).map(finding),
    other: res.violations.filter((v) => !(v.impact && FAILING.has(v.impact))).map(finding),
    passes: res.passes.length,
  };
}
