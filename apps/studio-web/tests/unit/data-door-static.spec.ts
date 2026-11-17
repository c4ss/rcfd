// A static check over src/renderer (F-808): bulk field data is reached only through the data door. One module,
// src/renderer/data/door.ts, may name window.studioData (F-804's bridge), as src/renderer/actions/index.ts is the one
// for window.studioService (F-827) and src/renderer/project/door.ts for window.studioProject (F-805). Reaching a global
// by computed name is refused for every renderer file by tests/unit/actions-static.spec.ts.
import { expect, test } from '@playwright/test';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { appRoot } from '../support/rcfd.js';

const renderer = path.join(appRoot, 'src', 'renderer');
const DOOR = 'data/door.ts';

function sources(dir: string): { file: string; text: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((d) => {
    const p = path.join(dir, d.name);
    if (d.isDirectory()) return sources(p);
    if (!/\.(ts|tsx|js|jsx|mts|cts)$/.test(d.name) || d.name.endsWith('.d.ts')) return [];
    return [{ file: path.relative(renderer, p).split(path.sep).join('/'), text: readFileSync(p, 'utf8') }];
  });
}

/** Comments out of the way (a comment may mention the bridge; code may not). Strings stay: a name in a string counts. */
const code = (t: string) => t.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');

const naming = (files: readonly { file: string; text: string }[]) => files.filter((f) => /studioData/.test(code(f.text))).map((f) => f.file);

test('only data/door.ts of the renderer names window.studioData', () => {
  const files = sources(renderer);
  expect(files.map((f) => f.file)).toContain(DOOR);
  expect(naming(files)).toEqual([DOOR]);
  expect(readFileSync(path.join(renderer, DOOR), 'utf8')).toContain('window.studioData');
});

test('the check catches a use outside the door and ignores comments', () => {
  expect(
    naming([
      { file: 'field/X.tsx', text: 'await window.studioData.field(store, 0, "p");' },
      { file: 'field/Y.ts', text: "const k = 'studioData';" },
      { file: 'field/Z.ts', text: '// window.studioData is reached through ../data/door\nimport { loadField } from "../data/door.js";' },
    ]),
  ).toEqual(['field/X.tsx', 'field/Y.ts']);
});
