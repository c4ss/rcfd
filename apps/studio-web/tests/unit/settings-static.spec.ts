// Static checks of the settings surface (F-825):
// - no Studio-side table of case keys (G2, G15): no string in src/renderer/settings names a section of the engine's case
//   schema or a key path of it (the F-824 inventory lists them); the surface learns them from settings.schema at run time
// - its stylesheet keeps the rules of the F-802 stylesheets (tests/unit/tokens.spec.ts): defined tokens only, no colour
//   literal, square corners, no drop shadow (DESIGN.md, the Flat Field Rule; an inset 1px line is the selected-row edge)
import { expect, test } from '@playwright/test';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import type { Inventory } from '../support/capabilities.js';
import { appRoot } from '../support/rcfd.js';

const dir = path.join(appRoot, 'src', 'renderer', 'settings');
const read = (f: string) => readFileSync(path.join(dir, f), 'utf8');

/** String literals of a source file, comments left out. */
function literals(text: string): string[] {
  const code = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');
  return [...code.matchAll(/'((?:[^'\\\n]|\\.)*)'|"((?:[^"\\\n]|\\.)*)"/g)].map((m) => m[1] ?? m[2] ?? '');
}

test('the settings surface holds no table of case keys: every key comes from settings.schema', () => {
  const inv = JSON.parse(readFileSync(path.join(appRoot, 'capabilities', 'inventory.json'), 'utf8')) as Inventory;
  const keys = inv.capabilities.filter((c) => c.kind === 'case_key').map((c) => c.id.slice('case:'.length));
  const sections = new Set(keys.map((k) => k.split('.')[0]!));
  const paths = new Set(keys.filter((k) => k.includes('.')));
  expect(sections.size).toBeGreaterThan(10);
  const sources = readdirSync(dir).filter((f) => /\.(ts|tsx)$/.test(f));
  expect(sources.length).toBeGreaterThanOrEqual(5);
  // the JSON Schema words the model reads are not case keys, though the case has a section called `properties`
  const vocabulary = new Set(['type', 'properties', 'required', 'additionalProperties', 'items', 'enum', 'default', 'minimum', 'maximum', 'description']);
  const named: string[] = [];
  for (const f of sources) {
    for (const s of literals(read(f))) if (!vocabulary.has(s) && (sections.has(s) || paths.has(s))) named.push(`${f}: '${s}'`);
  }
  expect(named).toEqual([]);
});

test('settings.css uses only defined tokens, no colour literals, square corners and no drop shadow', () => {
  const css = read('settings.css').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(50);
  expect(used.filter((u) => !defined.has(u))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  expect([...css.matchAll(/border-radius:\s*([^;]+)/g)].map((m) => m[1]!.trim()).filter((r) => r !== '0' && r !== 'var(--radius-none)')).toEqual([]);
  const shadows = [...css.matchAll(/box-shadow:\s*([^;]+)/g)].map((m) => m[1]!.trim());
  expect(shadows.filter((s) => !s.startsWith('inset '))).toEqual([]);
});
