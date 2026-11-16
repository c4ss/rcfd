// A static check over src/renderer (F-827, G16): the engine is reached only through the action dispatcher. One module,
// src/renderer/actions/index.ts, may touch window.studioService; no other renderer file may name it, reach the window's
// members by computed name, or call a service method by name. A surface calls `dispatch('<action id>', params)`.
import { expect, test } from '@playwright/test';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { SERVICE_METHODS } from '../../src/shared/service.js';
import { appRoot } from '../support/rcfd.js';

const renderer = path.join(appRoot, 'src', 'renderer');
/** The dispatcher module: the renderer's one door to the service. */
const DOOR = 'actions/index.ts';

function sources(dir: string, base = dir): { file: string; text: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((d) => {
    const p = path.join(dir, d.name);
    if (d.isDirectory()) return sources(p, base);
    if (!/\.(ts|tsx|js|jsx|mts|cts)$/.test(d.name)) return [];
    return [{ file: path.relative(base, p).split(path.sep).join('/'), text: readFileSync(p, 'utf8') }];
  });
}

/** Comments out of the way (a comment may mention the bridge; code may not). Strings stay: a name in a string counts. */
function code(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');
}

/** Every way a renderer file outside the door reaches the engine, as `file: what`. */
function engineUses(files: readonly { file: string; text: string }[]): string[] {
  const methods = SERVICE_METHODS.map((m) => m.replaceAll('.', '\\.')).join('|');
  const rules: [RegExp, string][] = [
    [/studioService/, 'names window.studioService'],
    [/(?<![.\w$])(window|globalThis|self|top|parent|frames)\s*\[/, 'reaches a global by computed name'],
    [/\b(Reflect\.get|Object\.getOwnPropertyDescriptors?|Object\.entries|Object\.values|Object\.keys)\s*\(\s*(window|globalThis|self)\b/, 'reads a global by reflection'],
    [new RegExp(`\\.call\\s*\\(\\s*['"\`](${methods})['"\`]`), 'calls a service method by name'],
    [/\bshared\/actions\/dispatcher(\.js)?['"]/, 'builds its own dispatcher (import the door, ../actions)'],
  ];
  const out: string[] = [];
  for (const { file, text } of files) {
    // the door, and declaration files (env.d.ts types the globals; a declaration runs nothing)
    if (file === DOOR || file.endsWith('.d.ts')) continue;
    const c = code(text);
    for (const [re, what] of rules) if (re.test(c)) out.push(`${file}: ${what}`);
  }
  return out;
}

test.describe('the renderer reaches the engine only through the dispatcher (G16)', () => {
  test('no renderer file but the dispatcher module touches window.studioService or calls a service method', () => {
    const files = sources(renderer);
    expect(files.map((f) => f.file)).toContain(DOOR);
    expect(files.length).toBeGreaterThan(10);
    expect(engineUses(files)).toEqual([]);
  });

  test('the door is the only importer of the dispatcher class, and it uses the bridge', () => {
    const door = readFileSync(path.join(renderer, DOOR), 'utf8');
    expect(door).toContain('window.studioService');
    expect(door).toMatch(/from '\.\.\/\.\.\/shared\/actions\/dispatcher\.js'/);
  });

  test('the check catches each way round it', () => {
    const bad = [
      { file: 'surfaces/Run.tsx', text: "await window.studioService.call('run.start', { session });" },
      { file: 'surfaces/Alias.tsx', text: 'const s = window["studio" + "Service"];' },
      { file: 'surfaces/Glob.ts', text: "const b = globalThis ['studio'];" },
      { file: 'surfaces/Reflect.ts', text: 'Reflect.get(window, key);' },
      { file: 'surfaces/Direct.ts', text: "bridge.call('settings.set', p);" },
      { file: 'surfaces/Own.ts', text: "import { Dispatcher } from '../../shared/actions/dispatcher.js';" },
      { file: 'surfaces/String.ts', text: "const name = 'studioService';" },
    ];
    expect(engineUses(bad)).toEqual([
      'surfaces/Run.tsx: names window.studioService',
      'surfaces/Run.tsx: calls a service method by name',
      'surfaces/Alias.tsx: reaches a global by computed name',
      'surfaces/Glob.ts: reaches a global by computed name',
      'surfaces/Reflect.ts: reads a global by reflection',
      'surfaces/Direct.ts: calls a service method by name',
      'surfaces/Own.ts: builds its own dispatcher (import the door, ../actions)',
      'surfaces/String.ts: names window.studioService',
    ]);
    const fine = [
      { file: 'surfaces/Ok.tsx', text: "// window.studioService is reached through ../actions\nawait dispatch('run.start', { session });\nconst url = 'app://studio/x';" },
      { file: DOOR, text: 'window.studioService.call("run.start")' },
    ];
    expect(engineUses(fine)).toEqual([]);
  });
});
