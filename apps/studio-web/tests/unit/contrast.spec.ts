// WCAG 2.2 AA contrast of every text and graphic token pair against its ground (guardrail G8, F-802): 4.5:1 for text
// (1.4.3), 3:1 for graphics and the visual states of controls (1.4.11). The pairs are design/contrast.ts PAIRS; every
// role must be a foreground there, a ground, or exempt with a reason, so a new role cannot slip past.
import { expect, test } from '@playwright/test';
import {
  AA,
  composite,
  contrastRatio,
  evaluate,
  evaluateAll,
  EXEMPT,
  GROUND_ROLES,
  groundName,
  PAIRS,
  passesAA,
  type Ground,
} from '../../src/renderer/design/contrast.js';
import { palette, role, type RoleName } from '../../src/renderer/design/tokens.js';

const r2 = (x: number): number => Math.round(x * 100) / 100;

test.describe('the contrast formula', () => {
  test('matches WCAG 2.x at the extremes', () => {
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21, 10);
    expect(contrastRatio('#ffffff', '#ffffff')).toBe(1);
    expect(contrastRatio('#1d3a5f', '#ffffff')).toBe(contrastRatio('#ffffff', '#1d3a5f'));
    // #767676 on white is the well-known 4.54:1 grey.
    expect(r2(contrastRatio('#767676', '#ffffff'))).toBe(4.54);
  });

  test('reproduces every ratio DESIGN.md states', () => {
    const p = palette;
    const stated: Array<[string, string, number]> = [
      [p.ink, p.white, 11.52], // computed here; DESIGN.md names ink the only dark
      [p['ink-2'], p.white, 6.18],
      [p.sky, p.white, 2.28],
      [p.ink, p.sky, 5.05], // the prototype's note: ink on sky 5.05:1
      [p.ink, p.meadow, 5.18], // ink on meadow 5.18:1
      [p['meadow-deep'], p.white, 5.1],
      [p.ink, p.orange, 4.59],
      [p['orange-deep'], p.white, 4.8],
      [p.fail, p.white, 8.12],
      [p.fail, p['fail-wash'], 6.65],
      [p['ink-tint'], p.ink, 7.6],
    ];
    for (const [fg, bg, ratio] of stated) expect(r2(contrastRatio(fg, bg)), `${fg} on ${bg}`).toBe(ratio);
  });

  test('composites a tint over its ground as the browser draws it', () => {
    expect(composite('#ffffff', 0, '#62b4e8')).toBe('#62b4e8');
    expect(composite('#ffffff', 1, '#62b4e8')).toBe('#ffffff');
    expect(composite('#ffffff', 0.5, '#000000')).toBe('#808080');
  });

  test('fails the contrast debt DESIGN.md recorded, so the check would catch it', () => {
    const p = palette;
    expect(passesAA(p.orange, p.white, 'graphic')).toBe(false); // running pictogram plain orange on white, 2.51:1
    expect(passesAA(p.white, p.sky, 'text')).toBe(false); // text selection white on sky, 2.28:1
    expect(passesAA(p.sky, p.white, 'graphic')).toBe(false); // residual u line sky on white, 2.28:1
    expect(passesAA(p.silver, p.white, 'graphic')).toBe(false); // silver input border, 1.60:1
    // Either side of the text line: #767676 on white is 4.54:1, #777777 is 4.48:1.
    expect(passesAA('#767676', '#ffffff', 'text')).toBe(true);
    expect(passesAA('#777777', '#ffffff', 'text')).toBe(false);
  });
});

test.describe('every token pair meets WCAG 2.2 AA', () => {
  test('no text pair below 4.5:1 and no graphic pair below 3:1', () => {
    const results = evaluateAll();
    expect(results.length).toBe(PAIRS.length);
    const below = results
      .filter((r) => !r.pass)
      .map((r) => `${r.kind} ${r.fg} (${r.fgHex}) on ${groundName(r.bg)} (${r.bgHex}): ${r2(r.ratio)} < ${r.required}`);
    expect(below).toEqual([]);
    for (const r of results) expect(r.required).toBe(AA[r.kind]);
  });

  test('every role is a foreground of a pair, only a ground, or exempt with a reason', () => {
    const fg = new Set(PAIRS.map((p) => p.fg));
    const exempt = new Set(EXEMPT.map((e) => e.role));
    const grounds = new Set(GROUND_ROLES);
    const unclassified = (Object.keys(role) as RoleName[]).filter((r) => !fg.has(r) && !exempt.has(r) && !grounds.has(r));
    expect(unclassified).toEqual([]);
    // An exempt or ground-only role is not also tested as a foreground (that would hide which list is meant).
    expect([...exempt].filter((r) => fg.has(r))).toEqual([]);
    expect([...grounds].filter((r) => fg.has(r) || exempt.has(r))).toEqual([]);
    for (const e of EXEMPT) expect(e.why.length, e.role).toBeGreaterThan(20);
  });

  test('every text role is held to 4.5:1 and every state mark to 3:1', () => {
    const textRoles: RoleName[] = ['text', 'text-2', 'text-inverse', 'text-inverse-2', 'text-fail', 'placeholder', 'selection-text', 'tooltip-text'];
    for (const r of textRoles) {
      const pairs = PAIRS.filter((p) => p.fg === r);
      expect(pairs.length, r).toBeGreaterThan(0);
      for (const p of pairs) expect(p.kind, `${r} on ${groundName(p.bg)}`).toBe('text');
    }
    for (const s of ['state-queued', 'state-running', 'state-converged', 'state-finished', 'state-failed', 'state-faded'] as const) {
      expect(PAIRS.some((p) => p.fg === s && p.bg === 'ground' && p.kind === 'graphic'), s).toBe(true);
    }
  });

  test('the contrast debt is fixed in the product tokens', () => {
    const on = (fg: RoleName, bg: Ground, kind: 'text' | 'graphic') => evaluate({ fg, bg, kind, use: 'debt' });
    // Running pictogram: orange-deep, on every light ground it sits on.
    expect(role['state-running']).not.toBe('orange');
    for (const g of ['ground', 'hover', 'row-hover', 'row-selected'] as const) expect(on('state-running', g, 'graphic').pass, g).toBe(true);
    // Text selection: ink on sky.
    expect(on('selection-text', 'selection-bg', 'text').pass).toBe(true);
    // Focus: one of the ring's two colours contrasts with every ground and control it can sit next to.
    for (const g of ['ground', 'header-band', 'hover', 'row-selected', 'row-failed', 'sub-tab-selected', 'viewport'] as const) {
      expect(on('focus-edge', g, 'graphic').pass, g).toBe(true);
    }
    expect(on('focus-ring', 'primary', 'graphic').pass).toBe(true);
    // Caret and field border.
    expect(on('caret', 'ground', 'graphic').pass).toBe(true);
    expect(on('field-border', 'ground', 'graphic').pass).toBe(true);
  });
});
