// Packages the built app by hand into out/packaged/, the way ADR 0350 packaged the spike for P10: Electron's dist folder
// copied, the executable renamed to rcfd-studio (so app.isPackaged is true), and the app in resources/app (package.json
// and dist/). No packaging dependency; installers, signing and fuses are F-820's.
//
//   node scripts/package-dir.mjs        (after npm run build)
//
// Writes out/packaged.json with the absolute path of the executable, which the Playwright tests and
// scripts/measure-cold-start.mjs read.
import { cpSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const out = path.join(root, 'out');
const packaged = path.join(out, 'packaged');

if (!existsSync(path.join(root, 'dist', 'main', 'index.js'))) {
  console.error('dist/ is missing: run npm run build first');
  process.exit(1);
}

// require('electron') from Node returns the path of the binary, downloading it on first use.
const electronExe = createRequire(import.meta.url)('electron');
const electronDist = path.join(root, 'node_modules', 'electron', 'dist');

rmSync(packaged, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
cpSync(electronDist, packaged, { recursive: true });

const relExe = path.relative(electronDist, electronExe);
let exe = path.join(packaged, relExe);
let resources;
if (process.platform === 'darwin') {
  // Electron.app/Contents/MacOS/Electron: keep the bundle's names (renaming a bundle needs its Info.plist changed too).
  resources = path.join(packaged, 'Electron.app', 'Contents', 'Resources');
} else {
  resources = path.join(packaged, 'resources');
  const renamed = path.join(packaged, process.platform === 'win32' ? 'rcfd-studio.exe' : 'rcfd-studio');
  renameSync(exe, renamed);
  exe = renamed;
}

const app = path.join(resources, 'app');
mkdirSync(app, { recursive: true });
const pkg = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));
const runtimePkg = {
  name: pkg.name,
  productName: 'rcfd Studio',
  version: pkg.version,
  private: true,
  license: pkg.license,
  type: pkg.type,
  main: pkg.main,
};
writeFileSync(path.join(app, 'package.json'), JSON.stringify(runtimePkg, null, 2) + '\n');
cpSync(path.join(root, 'dist'), path.join(app, 'dist'), { recursive: true });

writeFileSync(path.join(out, 'packaged.json'), JSON.stringify({ executable: exe }, null, 2) + '\n');
console.log(`packaged: ${exe}`);
