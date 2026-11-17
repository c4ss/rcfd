// Finding and starting the real `rcfd` binary for the tests that use the service itself. Built by
// `cargo build --release -p rcfd-cli`; looked for in RCFD_BIN, then where the Studio itself finds the engine of its
// checkout (checkoutRelease of src/main/engine/locate.ts: $CARGO_TARGET_DIR/release, else <repo>/target/release), then
// target/release of any folder above the app (a git worktree inside the main checkout can share its target folder).
import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkoutRelease } from '../../src/main/engine/locate.js';

const exe = process.platform === 'win32' ? 'rcfd.exe' : 'rcfd';
export const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

export function findRcfd(): string | null {
  const candidates: string[] = [];
  if (process.env['RCFD_BIN']) candidates.push(process.env['RCFD_BIN']);
  candidates.push(path.join(checkoutRelease(process.env, appRoot), exe));
  for (let dir = appRoot; ; dir = path.dirname(dir)) {
    candidates.push(path.join(dir, 'target', 'release', exe));
    if (path.dirname(dir) === dir) break;
  }
  return candidates.find((c) => existsSync(c)) ?? null;
}

export const NO_RCFD =
  'the rcfd binary was not found: build it with `cargo build --release -p rcfd-cli`, or set RCFD_BIN to its path';

export interface RunningRcfd {
  readonly child: ChildProcess;
  readonly port: number;
  readonly pid: number;
  stop(): Promise<void>;
}

/** Starts `rcfd serve --addr 127.0.0.1:<port>` (0: any free port) and resolves once it listens. */
export function startRcfd(rcfd: string, port = 0, extra: string[] = []): Promise<RunningRcfd> {
  return new Promise((resolve, reject) => {
    const child = spawn(rcfd, ['serve', '--addr', `127.0.0.1:${port}`, ...extra], { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
    let out = '';
    const onData = (d: Buffer) => {
      out += d.toString('utf8');
      const m = /listening on 127\.0\.0\.1:(\d+)/.exec(out);
      if (m) {
        child.stdout!.off('data', onData);
        resolve({
          child,
          port: Number(m[1]),
          pid: child.pid!,
          stop: () => stopChild(child),
        });
      }
    };
    child.stdout!.on('data', onData);
    child.stderr!.on('data', (d: Buffer) => (out += d.toString('utf8')));
    child.once('error', reject);
    child.once('exit', (code) => reject(new Error(`rcfd serve ended with ${code}: ${out}`)));
  });
}

export function stopChild(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  return new Promise((resolve) => {
    child.once('exit', () => resolve());
    child.kill();
  });
}

/** Kills a process by id (one the test did not start itself, such as a service the Studio started). */
export function killPid(pid: number): void {
  try {
    process.kill(pid);
  } catch {
    // already gone
  }
}

/** Kills `pid` only if it is still an rcfd process (a process id can be reused once its process has ended). */
export function killIfRcfd(pid: number): void {
  if (!alive(pid)) return;
  let name = '';
  try {
    name =
      process.platform === 'win32'
        ? execFileSync('tasklist', ['/FI', `PID eq ${pid}`, '/FO', 'CSV', '/NH'], { encoding: 'utf8' })
        : execFileSync('ps', ['-p', String(pid), '-o', 'comm='], { encoding: 'utf8' });
  } catch {
    return;
  }
  // tasklist's CSV starts each row with the quoted image name
  const isRcfd = process.platform === 'win32' ? /^"rcfd\.exe",/im.test(name) : path.basename(name.trim()) === 'rcfd';
  if (isRcfd) killPid(pid);
}

export function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === 'EPERM';
  }
}

/** A lid-driven box case (the case of ADR 0350's spike, written here): `n` cells per direction. */
export function boxToml(n: [number, number, number]): string {
  const walls = ['xmin', 'xmax', 'ymin', 'zmin', 'zmax'].map((p) => `[boundaries.${p}]\nvelocity = "fixed"\n`).join('');
  return `title = "F-803 lid-driven box"
[mesh]
generator = "box_3d"
nx = ${n[0]}
ny = ${n[1]}
nz = ${n[2]}
lx = 1.0
ly = 1.0
lz = 1.0
[physics]
nu = 0.01
${walls}[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 1000000
tolerance_u = 1e-14
tolerance_p = 1e-14
`;
}
