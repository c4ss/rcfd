// Packages the built app by hand into out/packaged/, the way ADR 0350 packaged the spike for P10: Electron's dist folder
// copied, the executable renamed to rcfd-studio (so app.isPackaged is true), and the app in resources/app (package.json
// and dist/). No packaging dependency; the installers (F-820) are scripts/package-installers.mjs, which ships the same
// app folder (runtimePackage() of scripts/installer-config.mjs, so the userData folder is the same too).
//
//   node scripts/package-dir.mjs [--engine=<folder with rcfd, rcfd-mcp, rcfd-runner>] [--no-engine]   (after npm run build)
//
// The engine goes beside the app as <resources>/bin, as the installers put it, so out/packaged finds its engine the way
// an installed Studio does (ADR 0801 Addendum 21, "Engine found without setup"). It is taken from --engine, else from
// the checkout's engine folder, by the Studio's own rule (checkoutRelease of src/main/engine/locate.ts, through dist/:
// $CARGO_TARGET_DIR/release, else <repo>/target/release). A part that is not there fails the packaging, naming the file
// and the cargo command that builds it. --no-engine packages without one (CI's studio-web job, which builds no Rust):
// that copy then says on its home that its engine is missing, as an installed one with a damaged bin folder would.
//
// Writes out/packaged.json with the absolute path of the executable, which the Playwright tests and
// scripts/measure-cold-start.mjs read.
import { cpSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runtimePackage } from './installer-config.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const out = path.join(root, 'out');
const packaged = path.join(out, 'packaged');

if (!existsSync(path.join(root, 'dist', 'main', 'index.js'))) {
  console.error('dist/ is missing: run npm run build first');
  process.exit(1);
}

const arg = (name) => process.argv.find((x) => x.startsWith(`--${name}=`))?.slice(name.length + 3);
const { buildCommand, checkoutRelease, ENGINE_PARTS, exe: exeName, missingParts } = await import(
  new URL('../dist/main/engine/locate.js', import.meta.url).href
);
const noEngine = process.argv.includes('--no-engine');
const engine = noEngine ? null : path.resolve(arg('engine') ?? checkoutRelease(process.env, root));
if (engine) {
  const missing = missingParts(engine, (p) => existsSync(p) && statSync(p).isFile());
  if (missing.length) {
    const files = missing.map((p) => exeName(p.name)).join(', ');
    console.error(`engine: ${files} not in ${engine}. Build with: ${buildCommand(missing)}`);
    console.error('(or pass --engine=<folder> to take it from elsewhere, or --no-engine to package without it)');
    process.exit(1);
  }
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
const runtimePkg = runtimePackage(pkg);
writeFileSync(path.join(app, 'package.json'), JSON.stringify(runtimePkg, null, 2) + '\n');
cpSync(path.join(root, 'dist'), path.join(app, 'dist'), { recursive: true });

// The engine, beside the app as <resources>/bin (the installers' extraResources)
if (engine) {
  mkdirSync(path.join(resources, 'bin'), { recursive: true });
  for (const p of ENGINE_PARTS) cpSync(path.join(engine, exeName(p.name)), path.join(resources, 'bin', exeName(p.name)));
  console.log(`engine: ${ENGINE_PARTS.map((p) => exeName(p.name)).join(', ')} from ${engine}`);
} else {
  console.log('engine: not included (--no-engine)');
}

writeFileSync(path.join(out, 'packaged.json'), JSON.stringify({ executable: exe }, null, 2) + '\n');
console.log(`packaged: ${exe}`);
