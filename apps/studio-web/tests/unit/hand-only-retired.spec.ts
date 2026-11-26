// ADR 0808 (F-981): the Studio's hand-only surfaces retired on the maintainer's answer of 2026-11-26 (ADR 0803: the
// views stay, hand navigation goes). N3: the learner's first path (F-813, src/renderer/first/) is gone, nothing in the
// source imports or draws it, and the notice for a folder that is not a project, which it held, is the project home's.
// Reads committed files only.
import { expect, test } from '@playwright/test';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { appRoot } from '../support/rcfd.js';

const src = path.join(appRoot, 'src');
const RETIRED = ['FirstPath.tsx', 'model.ts', 'target.ts', 'words.ts', 'first.css'].map((f) => path.join(src, 'renderer', 'first', f));

/** Every source file under src/ (TypeScript, TSX and CSS), with its path relative to src/. */
function sources(dir = src): { rel: string; text: string }[] {
  const out: { rel: string; text: string }[] = [];
  for (const name of readdirSync(dir)) {
    const p = path.join(dir, name);
    if (statSync(p).isDirectory()) out.push(...sources(p));
    else if (/\.(tsx?|css)$/.test(name)) out.push({ rel: path.relative(src, p).replace(/\\/g, '/'), text: readFileSync(p, 'utf8') });
  }
  return out;
}

test('the first path is retired: its files are gone, no source imports or draws it, and the not-a-project notice is the project home’s', () => {
  for (const f of RETIRED) expect(existsSync(f), f).toBe(false);
  expect(existsSync(path.join(src, 'renderer', 'first')), 'src/renderer/first/').toBe(false);
  const all = sources();
  expect(all.length).toBeGreaterThan(100);
  // an import of a module in a first/ folder (src/shared/actions/first.ts, the G16 action, is a file, not a folder)
  const importsFirst = all.filter((f) => /from\s+'(\.\.?\/)+(renderer\/)?first\//.test(f.text) || /import\(\s*'(\.\.?\/)+(renderer\/)?first\//.test(f.text));
  expect(importsFirst.map((f) => f.rel)).toEqual([]);
  // the path's components drawn, or its two roots styled or rendered
  const draws = all.filter((f) => /<FirstPath(Intro)?\b|\bFirstPathIntro\b|fp--intro|fp--open/.test(f.text));
  expect(draws.map((f) => f.rel)).toEqual([]);
  const notice = readFileSync(path.join(src, 'renderer', 'project', 'NotAProject.tsx'), 'utf8');
  expect(notice).toMatch(/export function NotAProjectYet\(/);
  const home = readFileSync(path.join(src, 'renderer', 'project', 'ProjectHome.tsx'), 'utf8');
  expect(home).toContain("import { NotAProjectYet } from './NotAProject.js';");
  expect(home).toContain('<NotAProjectYet failure={failure} />');
});
