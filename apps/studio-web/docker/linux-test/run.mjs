// The Studio on Linux, in Docker (F-821, ADR 0801 Addendum 17): builds the image of ./Dockerfile, hands the container the
// repository's committed files (git archive of HEAD, LF line endings as the repository stores them), and runs ./inside.sh
// there: the engine's release binaries, the packaged Linux Studio, the unit tests and the e2e suite under Xvfb.
//
//   node docker/linux-test/run.mjs [--steps="cargo build unit e2e"] [--specs="shell.spec.ts field.spec.ts"] [--bench]
//                                  [--out=<folder>] [--cpus=0-7] [--memory=24g] [--image=<tag>] [--worktree]
//                                  [--gpu-blocklist=ignore] [--rebuild-image]
//
// --worktree archives the working tree's tracked files as they are (git stash create), not HEAD.
// There is no GPU in the container. By default Chromium then turns WebGL off as well as WebGPU (the Studio's field views
// cannot draw; what a GPU-less Linux machine gets). --gpu-blocklist=ignore passes --ignore-gpu-blocklist to the tests'
// Electron (RCFD_TEST_ELECTRON_ARGS, tests/support/launch.ts): WebGL 2 on Mesa's software rasteriser (llvmpipe), which
// runs the drawing code and its pixel checks but says nothing about GPU speed.
// Cargo's target folder and registry, npm's cache and Electron's download live in named volumes
// (rcfd-studio-linux-target, rcfd-studio-linux-registry, rcfd-studio-linux-dl), so a second run rebuilds little.
// Results: <out>/summary.txt (counts), <out>/<step>.log, <out>/unit.json and e2e.json (Playwright's JSON reports).
// Exit code: the container's (0 only when every step passed).
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const app = path.resolve(here, '..', '..');
const arg = (name, dflt) => {
  const a = process.argv.find((x) => x.startsWith(`--${name}=`));
  return a ? a.slice(name.length + 3) : dflt;
};
const flag = (name) => process.argv.includes(`--${name}`);

const image = arg('image', 'rcfd-studio-linux-test:ubuntu24.04');
const out = path.resolve(arg('out', path.join(app, 'test-results-linux')));
const steps = arg('steps', 'cargo build unit e2e');
const specs = arg('specs', '');
const cpus = arg('cpus', '');
const memory = arg('memory', '24g');

const repo = execFileSync('git', ['rev-parse', '--show-toplevel'], { cwd: app, encoding: 'utf8' }).trim();
let tree = 'HEAD';
if (flag('worktree')) tree = execFileSync('git', ['stash', 'create'], { cwd: repo, encoding: 'utf8' }).trim() || 'HEAD';
const commit = execFileSync('git', ['rev-parse', '--short', tree], { cwd: repo, encoding: 'utf8' }).trim();

if (spawnSync('docker', ['image', 'inspect', image], { stdio: 'ignore' }).status !== 0 || flag('rebuild-image')) {
  console.log(`building ${image} from ${path.relative(repo, path.join(here, 'Dockerfile'))}`);
  const b = spawnSync('docker', ['build', '-t', image, '-f', path.join(here, 'Dockerfile'), here], { stdio: 'inherit' });
  if (b.status !== 0) process.exit(b.status ?? 1);
}

const io = path.join(out, '_io');
rmSync(out, { recursive: true, force: true });
mkdirSync(io, { recursive: true });
const tar = path.join(io, 'src.tar');
// core.autocrlf off: the files as the repository stores them (.gitattributes: LF except .bat/.ps1/.cmd).
execFileSync('git', ['-c', 'core.autocrlf=false', 'archive', '--format=tar', '-o', tar, tree], { cwd: repo, stdio: 'inherit' });
console.log(`source: ${tree === 'HEAD' ? 'HEAD' : 'working tree'} ${commit}`);

const cmd = [
  'run', '--rm', '--init',
  '--memory', memory,
  ...(cpus ? ['--cpuset-cpus', cpus] : []),
  '--shm-size', '2g', // Chromium's shared memory; Docker's 64 MB default crashes renderers
  // Chromium's sandbox needs user namespaces, which Docker's default seccomp profile refuses (G10: the tests run the
  // Studio with its sandbox on, so the container's filter gives way, not the app's)
  '--security-opt', 'seccomp=unconfined',
  '-v', `${tar}:/io/src.tar:ro`,
  '-v', `${out}:/io/out`,
  '-v', 'rcfd-studio-linux-target:/cache/target',
  '-v', 'rcfd-studio-linux-registry:/usr/local/cargo/registry',
  '-v', 'rcfd-studio-linux-dl:/cache/dl',
  '-e', 'npm_config_cache=/cache/dl/npm',
  '-e', `STUDIO_STEPS=${steps}`,
  '-e', `STUDIO_SPECS=${specs}`,
  '-e', `STUDIO_BENCH=${flag('bench') ? '1' : '0'}`,
  '-e', `RCFD_TEST_ELECTRON_ARGS=${arg('gpu-blocklist', '') === 'ignore' ? '--ignore-gpu-blocklist' : ''}`,
  image,
  'bash', '-c', 'tar -xOf /io/src.tar apps/studio-web/docker/linux-test/inside.sh > /tmp/inside.sh && bash /tmp/inside.sh',
];
console.log(`docker ${cmd.join(' ')}`);
const r = spawnSync('docker', cmd, { stdio: 'inherit' });
rmSync(io, { recursive: true, force: true });
try {
  const s = readFileSync(path.join(out, 'summary.txt'), 'utf8');
  writeFileSync(path.join(out, 'summary.txt'), `source: ${commit}\n${s}`);
  console.log(`\n---- ${path.join(out, 'summary.txt')}\nsource: ${commit}\n${s}`);
} catch {
  console.log('no summary.txt: the container ended before writing it');
}
process.exit(r.status ?? 1);
