// ADR 0738 (F-809, E-809d): the derived pictures the engine makes of a stored snapshot. The Studio does not compute a
// cut, arrows, an iso-surface or streamlines itself: main runs the engine's `rcfd post derive <store dir> ... --out
// <file.glb>` (the library of the project tool `derive_picture`, G16) on a store inside a data root, reads the GLB it
// wrote and hands its bytes and the command's JSON reply to the page. The command runs off main's thread (a child
// process); a cancel stops it.
//
// Where rcfd is: RCFD_STUDIO_RCFD, else the rcfd beside RCFD_STUDIO_MCP, else the engine the Studio ships with
// (<resources>/bin, F-820). Nothing is looked for on PATH (README "The project home").
import { execFile, type ChildProcess } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import type { DataWireResult, DeriveArgs } from '../../shared/data.js';
import { RCFD_EXE } from '../project/config.js';
import { DataFailure, type Job } from './queue.js';

export type DeriveEngine = { readonly ok: true; readonly rcfd: string } | { readonly ok: false; readonly reason: string };

/** Where the engine's rcfd is, from the Studio's environment (pure: the unit tests pass plain objects). */
export function deriveEngine(env: Readonly<Record<string, string | undefined>>, isFile: (p: string) => boolean, bundled: string | null): DeriveEngine {
  const rcfd = env['RCFD_STUDIO_RCFD']?.trim() ?? '';
  if (rcfd) return isFile(rcfd) ? { ok: true, rcfd } : { ok: false, reason: `RCFD_STUDIO_RCFD names ${rcfd}, which is not a file.` };
  const mcp = env['RCFD_STUDIO_MCP']?.trim() ?? '';
  if (mcp) {
    const beside = path.join(path.dirname(mcp), RCFD_EXE);
    return isFile(beside) ? { ok: true, rcfd: beside } : { ok: false, reason: `There is no ${RCFD_EXE} beside RCFD_STUDIO_MCP (${mcp}).` };
  }
  if (bundled) {
    const shipped = path.join(bundled, RCFD_EXE);
    if (isFile(shipped)) return { ok: true, rcfd: shipped };
  }
  return { ok: false, reason: `The Studio was not told where the engine is: set RCFD_STUDIO_RCFD to the path of ${RCFD_EXE} (cargo build --release -p rcfd-cli).` };
}

/** The flags of `rcfd post derive` for these (checked, src/main/data/bridge.ts) arguments. */
export function deriveFlags(snapshot: number, a: DeriveArgs): string[] {
  const f = ['--kind', a.kind, '--snapshot', String(snapshot)];
  if (a.field !== undefined) f.push('--field', a.field);
  if (a.level !== undefined) f.push('--level', String(a.level));
  if (a.axis !== undefined && a.at !== undefined) f.push('--axis', 'xyz'[a.axis]!, '--at', String(a.at), '--keep', a.keep ?? 'below');
  if (a.vectors) f.push('--vectors');
  if (a.rake) f.push('--rake', [...a.rake.p0, ...a.rake.p1, a.rake.n].map(String).join(','));
  if (a.direction) f.push('--direction', a.direction);
  if (a.maxSteps !== undefined) f.push('--max-steps', String(a.maxSteps));
  return f;
}

/** The engine's error as the page gets it: its JSON-RPC code (1003, 1004, 1202, -32602), message and data. */
function engineFailure(stderr: string, code: number | null): DataFailure {
  try {
    const e = (JSON.parse(stderr.trim()) as { error?: { code?: unknown; message?: unknown; data?: unknown } }).error;
    if (e && typeof e.code === 'number' && typeof e.message === 'string') return new DataFailure(e.code, e.message, e.data);
  } catch {
    // not the engine's JSON: a usage error or a crash
  }
  return new DataFailure(code === 2 ? 'bad_request' : 'internal', `rcfd post derive ended with code ${code}: ${stderr.trim().slice(0, 400)}`);
}

/** Runs one `rcfd post derive` on the store folder `dir`; a cancel stops the command. */
export function runDerive(rcfd: string, dir: string, snapshot: number, args: DeriveArgs): Job<DataWireResult> {
  let child: ChildProcess | null = null;
  let stop: ((e: DataFailure) => void) | null = null;
  let cancelled = false;
  const promise = (async (): Promise<DataWireResult> => {
    const tmp = await mkdtemp(path.join(os.tmpdir(), 'rcfd-studio-derive-'));
    try {
      const out = path.join(tmp, 'picture.glb');
      const stdout = await new Promise<string>((resolve, reject) => {
        stop = reject;
        if (cancelled) return reject(new DataFailure('cancelled', 'cancelled'));
        child = execFile(rcfd, ['post', 'derive', dir, ...deriveFlags(snapshot, args), '--out', out], { windowsHide: true, maxBuffer: 1 << 20 }, (err, so, se) => {
          child = null;
          if (cancelled) return;
          if (err) return reject(engineFailure(String(se), typeof err.code === 'number' ? err.code : null));
          resolve(String(so));
        });
      });
      const bytes = await readFile(out);
      const glb = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
      return { kind: 'derive', glb, summary: stdout.trim() };
    } finally {
      await rm(tmp, { recursive: true, force: true });
    }
  })();
  return {
    promise,
    cancel: (reason = new DataFailure('cancelled', 'cancelled')) => {
      cancelled = true;
      (child as ChildProcess | null)?.kill();
      stop?.(reason);
    },
  };
}
