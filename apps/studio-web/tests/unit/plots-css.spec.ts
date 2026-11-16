// The plots' stylesheets (F-811) keep the rule of the design tokens (F-802, tests/unit/tokens.spec.ts): only defined
// tokens, no colour literals, square corners. A new file, so the F-802 test is not edited (house rule 2).
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { appRoot } from '../support/rcfd.js';

const renderer = path.join(appRoot, 'src', 'renderer');

for (const sheet of [
  ['plots', 'plots.css'],
  ['dev', 'plots-dev.css'],
]) {
  test(`${sheet.join('/')} uses only defined tokens and no colour literals`, () => {
    const css = readFileSync(path.join(renderer, ...sheet), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    const defined = new Set(cssCustomProperties().map(([k]) => k));
    const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
    expect(used.length).toBeGreaterThan(0);
    expect(used.filter((v) => !defined.has(v))).toEqual([]);
    expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
    const radii = [...css.matchAll(/border-radius:\s*([^;]+)/g)].map((m) => m[1]!.trim());
    expect(radii.filter((v) => v !== 'var(--radius-none)' && v !== '0')).toEqual([]);
  });
}

test('the plot line roles exist and are styled for their swatches', () => {
  const css = readFileSync(path.join(renderer, 'plots', 'plots.css'), 'utf8');
  const style = readFileSync(path.join(renderer, 'plots', 'style.ts'), 'utf8');
  const roles = new Set([...style.matchAll(/role: '([a-z0-9-]+)'/g)].map((m) => m[1]!));
  expect(roles.size).toBeGreaterThan(1);
  for (const r of roles) expect(css, `.plot-role--${r}`).toContain(`.plot-role--${r} {\n  color: var(--role-${r});`);
});
