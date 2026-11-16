// Opens the packaged Studio on the primitives gallery (#/dev/primitives, F-802) for the maintainer's view-and-report,
// at 1440 by 900, and waits until the window is closed.
//
//   npm run build && npm run package:dir && npm run gallery
//
// The window has no address bar, so the script drives it with Playwright's Electron launcher (a dev dependency already)
// instead of adding a route switch to the main process.
import { _electron as electron } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
let executable;
try {
  ({ executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8')));
} catch {
  console.error('out/packaged.json is missing: run npm run build and npm run package:dir first');
  process.exit(1);
}

const app = await electron.launch({ executablePath: executable, args: [] });
const page = await app.firstWindow();
await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]?.setContentSize(1440, 900));
await page.evaluate(() => {
  location.hash = '#/dev/primitives';
});
console.log('The primitives gallery is open. Close the window to finish.');
await new Promise((done) => app.process().once('exit', done));
