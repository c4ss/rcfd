// Opens the packaged Studio on the field viewport's development page (#/dev/field, F-808) for the maintainer's
// view-and-report, at 1440 by 900, on a store of a data folder, and waits until the window is closed.
//
//   npm run build && npm run package:dir
//   npm run field -- --data=<folder> [--store=<folder of store.rts inside it>] [--field=p] [--snapshot=0] [--webgl]
//
// Without --store it opens the first `runs/*/fields` store it finds under <folder>/project (f447_bench gen's layout).
// The folder becomes the Studio's data root (RCFD_STUDIO_DATA_ROOTS) for this window only.
import { _electron as electron } from '@playwright/test';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3) ?? '';
const data = arg('data') ? path.resolve(arg('data')) : '';
if (!data) {
  console.error('usage: npm run field -- --data=<folder> [--store=<folder of store.rts>] [--field=p] [--snapshot=0] [--webgl]');
  process.exit(2);
}
let store = arg('store') ? path.resolve(arg('store')) : '';
if (!store) {
  const runs = path.join(data, 'project', 'runs');
  const found = existsSync(runs) ? readdirSync(runs).map((r) => path.join(runs, r, 'fields')).find((f) => existsSync(path.join(f, 'store.rts'))) : undefined;
  if (!found) {
    console.error(`no runs/*/fields/store.rts under ${path.join(data, 'project')}: name one with --store`);
    process.exit(2);
  }
  store = found;
}
let executable;
try {
  ({ executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8')));
} catch {
  console.error('out/packaged.json is missing: run npm run build and npm run package:dir first');
  process.exit(1);
}
const env = {};
for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_')) env[k] = v;
const app = await electron.launch({ executablePath: executable, args: [], env: { ...env, RCFD_STUDIO_DATA_ROOTS: data } });
const page = await app.firstWindow();
await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]?.setContentSize(1440, 900));
const q = new URLSearchParams({ route: 'local', path: store, field: arg('field') || 'p', snapshot: arg('snapshot') || '0' });
if (process.argv.includes('--webgl')) q.set('backend', 'webgl');
await page.evaluate((h) => (location.hash = h), `#/dev/field?${q.toString()}`);
console.log(`The field viewport is open on ${store}. Close the window to finish.`);
await new Promise((done) => app.process().once('exit', done));
