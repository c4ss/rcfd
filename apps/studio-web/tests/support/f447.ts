// The data sets of ADR 0350 (amendment 1, A1.2) for the bulk data tests (F-804), made by the existing Rust example
// `crates/rcfd-api/examples/f447_bench.rs`:
//
//   gen <folder> <sizes>   the lid-driven box run through the run path (raw store), its deflated copy written by the Rust
//                          writer, and the reference bytes ref/<size>/<snapshot>-<field>.f32: `Reader::read_field` cast
//                          to f32, little-endian (what P12 compares against)
//   serve <folder>         a solver service confined to <folder> with the data.* methods on (route R1)
//
// Built by `cargo build --release -p rcfd-api --example f447_bench`; looked for in RCFD_F447_BENCH, then
// $CARGO_TARGET_DIR/release/examples, then target/release/examples of this checkout or of any folder above it. The tests
// generate their data afresh in a temporary folder on every run (nothing is cached between runs).
import { spawn, type ChildProcess } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { appRoot } from './rcfd.js';

const exe = process.platform === 'win32' ? 'f447_bench.exe' : 'f447_bench';

export const FIELDS = ['p', 'ux', 'uy', 'uz'] as const;
export const SNAPSHOTS = 20;

export function findBench(): string | null {
  const candidates: string[] = [];
  if (process.env['RCFD_F447_BENCH']) candidates.push(process.env['RCFD_F447_BENCH']);
  if (process.env['CARGO_TARGET_DIR']) candidates.push(path.join(process.env['CARGO_TARGET_DIR'], 'release', 'examples', exe));
  for (let dir = appRoot; ; dir = path.dirname(dir)) {
    candidates.push(path.join(dir, 'target', 'release', 'examples', exe));
    if (path.dirname(dir) === dir) break;
  }
  return candidates.find((c) => existsSync(c)) ?? null;
}

export const NO_BENCH =
  'the f447_bench example was not found: build it with `cargo build --release -p rcfd-api --example f447_bench`, or set RCFD_F447_BENCH to its path';

/** Runs `f447_bench gen <root> <sizes...>` and resolves when it has written every store and reference. */
export function generate(bench: string, root: string, sizes: readonly string[], timeoutMs = 240_000): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(bench, ['gen', root, ...sizes], { stdio: ['ignore', 'ignore', 'pipe'], windowsHide: true });
    let err = '';
    child.stderr!.on('data', (d: Buffer) => (err += d.toString('utf8')));
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error(`f447_bench gen took longer than ${timeoutMs} ms: ${err}`));
    }, timeoutMs);
    child.once('error', reject);
    child.once('exit', (code) => {
      clearTimeout(timer);
      if (code === 0) resolve(err);
      else reject(new Error(`f447_bench gen ended with ${code}: ${err}`));
    });
  });
}

export interface BenchServer {
  readonly child: ChildProcess;
  /** Port of the JSON-RPC service. */
  readonly rpc: number;
  /** Port of the data connection. */
  readonly data: number;
  /** The canonical root the service is confined to. */
  readonly root: string;
  stop(): Promise<void>;
}

/** Starts `f447_bench serve <root>` (data.* on); it ends when its standard input closes. */
export function startBenchServer(bench: string, root: string): Promise<BenchServer> {
  return new Promise((resolve, reject) => {
    const child = spawn(bench, ['serve', root], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
    let out = '';
    let err = '';
    child.stderr!.on('data', (d: Buffer) => (err += d.toString('utf8')));
    child.stdout!.on('data', (d: Buffer) => {
      out += d.toString('utf8');
      const nl = out.indexOf('\n');
      if (nl < 0) return;
      const p = JSON.parse(out.slice(0, nl)) as { rpc: number; data: number; root: string };
      resolve({
        child,
        rpc: p.rpc,
        data: p.data,
        root: p.root,
        stop: async () => {
          child.stdin!.end();
          const t = setTimeout(() => child.kill(), 5000);
          await stopChildGently(child);
          clearTimeout(t);
        },
      });
    });
    child.once('error', reject);
    child.once('exit', (code) => reject(new Error(`f447_bench serve ended with ${code}: ${err}`)));
  });
}

function stopChildGently(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  return new Promise((resolve) => child.once('exit', () => resolve()));
}

/** The folder holding `store.rts` of the box of `size` (`raw`: the run path's store; `deflate`: the Rust writer's copy). */
export const storeDir = (root: string, size: string, codec: 'raw' | 'deflate'): string => path.join(root, 'project', 'runs', `box-${size}-${codec}`, 'fields');

/** The same folder as the bench server names it (relative to its root). */
export const servicePath = (size: string, codec: 'raw' | 'deflate'): string => `project/runs/box-${size}-${codec}/fields`;

/** The reference bytes of snapshot `i`, field `f`: `Reader::read_field` cast to f32, little-endian. */
export function refBytes(root: string, size: string, i: number, f: string): Buffer {
  return readFileSync(path.join(root, 'ref', size, `${i}-${f}.f32`));
}

export const sha256 = (b: Uint8Array): string => createHash('sha256').update(b).digest('hex');

/** Number of bits in which two byte arrays differ (XOR and population count over 32-bit words; ADR 0350 P12). */
export function differingBits(a: Uint8Array, b: Uint8Array): number {
  if (a.byteLength !== b.byteLength) return Number.POSITIVE_INFINITY;
  let n = 0;
  const words = a.byteLength >>> 2;
  const wa = new Uint32Array(a.buffer.slice(a.byteOffset, a.byteOffset + 4 * words));
  const wb = new Uint32Array(b.buffer.slice(b.byteOffset, b.byteOffset + 4 * words));
  for (let i = 0; i < words; i++) {
    let x = (wa[i]! ^ wb[i]!) >>> 0;
    x -= (x >>> 1) & 0x55555555;
    x = (x & 0x33333333) + ((x >>> 2) & 0x33333333);
    n += Math.imul((x + (x >>> 4)) & 0x0f0f0f0f, 0x01010101) >>> 24;
  }
  for (let i = 4 * words; i < a.byteLength; i++) {
    let x = a[i]! ^ b[i]!;
    while (x) {
      n += x & 1;
      x >>>= 1;
    }
  }
  return n;
}
