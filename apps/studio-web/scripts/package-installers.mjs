// Builds the Studio's installers for this platform with electron-builder (F-820, ADR 0801 Addendum 17): NSIS on Windows,
// AppImage and .deb on Linux. macOS is not built (the maintainer has not asked for it); builderConfig() has no mac section.
//
//   node scripts/package-installers.mjs [--engine=<folder with rcfd, rcfd-mcp, rcfd-runner>] [--no-engine]
//                                       [--version=X.Y.Z] [--out=<folder>]
//
// After npm run build. The engine is taken from --engine, else $CARGO_TARGET_DIR/release, else target/release here or
// above (cargo build --release -p rcfd-cli -p rcfd-mcp -p rcfd-runner). Installers land in out/installers/.
//
// Signing (Windows) stays off unless RCFD_STUDIO_SIGN=1, and then needs CSC_LINK or WIN_CSC_LINK (a .pfx, path or
// base64) and CSC_KEY_PASSWORD. With signing off those variables are removed before electron-builder starts, so a
// certificate in the environment is never used by accident. Nothing is published (publish: null).
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { builderConfig, INSTALLER_METADATA, runtimePackage } from './installer-config.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const arg = (name) => process.argv.find((x) => x.startsWith(`--${name}=`))?.slice(name.length + 3);

if (!existsSync(path.join(root, 'dist', 'main', 'index.js'))) {
  console.error('dist/ is missing: run npm run build first');
  process.exit(1);
}

const sign = process.env.RCFD_STUDIO_SIGN === '1';
const certVars = ['CSC_LINK', 'WIN_CSC_LINK', 'CSC_KEY_PASSWORD', 'WIN_CSC_KEY_PASSWORD'];
if (sign) {
  if (process.platform !== 'win32') {
    console.error('RCFD_STUDIO_SIGN=1: only the Windows installer is signed; unset it on this platform');
    process.exit(1);
  }
  if (!(process.env.CSC_LINK || process.env.WIN_CSC_LINK) || !(process.env.CSC_KEY_PASSWORD || process.env.WIN_CSC_KEY_PASSWORD)) {
    console.error('RCFD_STUDIO_SIGN=1 needs CSC_LINK (or WIN_CSC_LINK) and CSC_KEY_PASSWORD: no certificate, no signed build');
    process.exit(1);
  }
} else {
  for (const v of certVars) delete process.env[v];
  process.env.CSC_IDENTITY_AUTO_DISCOVERY = 'false';
}

const pkg = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));
const version = arg('version') ?? pkg.version;
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`--version=${version} is not X.Y.Z`);
  process.exit(1);
}
const stage = path.join(root, 'out', 'stage');
const output = path.resolve(arg('out') ?? path.join(root, 'out', 'installers'));

// The engine, beside the app as <resources>/bin.
const exe = (n) => (process.platform === 'win32' ? `${n}.exe` : n);
const engineNames = ['rcfd', 'rcfd-mcp', 'rcfd-runner'].map(exe);
function findEngine() {
  const given = arg('engine');
  if (given) return path.resolve(given);
  const candidates = [];
  if (process.env.CARGO_TARGET_DIR) candidates.push(path.join(process.env.CARGO_TARGET_DIR, 'release'));
  for (let dir = root; ; dir = path.dirname(dir)) {
    candidates.push(path.join(dir, 'target', 'release'));
    if (path.dirname(dir) === dir) break;
  }
  return candidates.find((c) => engineNames.every((n) => existsSync(path.join(c, n)))) ?? null;
}

rmSync(stage, { recursive: true, force: true });
mkdirSync(path.join(stage, 'app'), { recursive: true });
mkdirSync(path.join(stage, 'bin'), { recursive: true });
writeFileSync(path.join(stage, 'app', 'package.json'), JSON.stringify({ ...runtimePackage(pkg), version, ...INSTALLER_METADATA }, null, 2) + '\n');
cpSync(path.join(root, 'dist'), path.join(stage, 'app', 'dist'), { recursive: true });

if (process.argv.includes('--no-engine')) {
  console.log('engine: not included (--no-engine)');
} else {
  const engine = findEngine();
  if (!engine) {
    console.error(`engine not found (${engineNames.join(', ')}): cargo build --release -p rcfd-cli -p rcfd-mcp -p rcfd-runner, or pass --engine=<folder> or --no-engine`);
    process.exit(1);
  }
  for (const n of engineNames) {
    if (!existsSync(path.join(engine, n))) {
      console.error(`engine: ${path.join(engine, n)} is missing`);
      process.exit(1);
    }
    cpSync(path.join(engine, n), path.join(stage, 'bin', n));
  }
  console.log(`engine: ${engineNames.join(', ')} from ${engine}`);
}

const { build, Platform } = await import('electron-builder');
const platform = process.platform === 'win32' ? Platform.WINDOWS : process.platform === 'linux' ? Platform.LINUX : null;
if (!platform) {
  console.error(`no installer is defined for ${process.platform} (macOS is not built: ADR 0801 Addendum 17)`);
  process.exit(1);
}
console.log(`signing: ${sign ? 'on' : 'off (RCFD_STUDIO_SIGN is not 1)'}; version ${version}; output ${output}`);
const files = await build({
  targets: platform.createTarget(),
  projectDir: root,
  config: builderConfig({ pkg, stage, output, sign }),
  publish: 'never',
});
for (const f of files) console.log(`installer: ${f}`);
