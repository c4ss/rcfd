// Which folders route R2 may read (F-804, G10): the data roots. A local request names an absolute folder; it is read
// only when its real path (symbolic links and junctions resolved) lies inside one of the roots. Pure apart from the
// file system's realpath; no Electron import.
//
//   RCFD_STUDIO_DATA_ROOTS=<dir>[;<dir>...]   (the platform's path delimiter: ';' on Windows, ':' elsewhere)
//
// Until the project folder exists (F-805), the roots come from the environment only; opening a project adds its folder
// with `allow`.
import { realpath } from 'node:fs/promises';
import path from 'node:path';

const fold = (p: string): string => (process.platform === 'win32' ? p.toLowerCase() : p);

/** True when `child` is `root` or lies under it (both real, absolute paths). */
export function isInside(root: string, child: string): boolean {
  const r = fold(path.resolve(root));
  const c = fold(path.resolve(child));
  if (c === r) return true;
  return c.startsWith(r.endsWith(path.sep) ? r : r + path.sep);
}

/** Why a local path is refused before the file system is asked, or null. */
export function pathProblem(p: string): string | null {
  if (p.length === 0) return 'the path is empty';
  if (p.includes('\0')) return 'the path holds a NUL character';
  if (!path.isAbsolute(p)) return 'a local store is named by an absolute path';
  if (p.split(/[\\/]+/).some((s) => s === '..')) return "the path holds a '..' segment";
  if (process.platform === 'win32' && /^\\\\[?.]\\/.test(p)) return 'device and verbatim paths are not accepted';
  return null;
}

export class DataRoots {
  private readonly roots: string[] = [];

  /** The roots named in RCFD_STUDIO_DATA_ROOTS. */
  static fromEnv(env: Readonly<Record<string, string | undefined>>): DataRoots {
    const r = new DataRoots();
    for (const p of (env['RCFD_STUDIO_DATA_ROOTS'] ?? '').split(path.delimiter)) {
      const t = p.trim();
      if (t && pathProblem(t) === null) r.roots.push(path.resolve(t));
    }
    return r;
  }

  /** Adds a root (an absolute folder); F-805 calls this when a project is opened. */
  allow(dir: string): void {
    const problem = pathProblem(dir);
    if (problem) throw new Error(`data root '${dir}': ${problem}`);
    const d = path.resolve(dir);
    if (!this.roots.some((r) => fold(r) === fold(d))) this.roots.push(d);
  }

  get list(): readonly string[] {
    return this.roots;
  }

  /**
   * The real path of `p` when it may be read, else an error message. The roots are resolved too, so a root given
   * through a link still matches.
   */
  async resolve(p: string): Promise<{ ok: true; dir: string } | { ok: false; message: string }> {
    const problem = pathProblem(p);
    if (problem) return { ok: false, message: problem };
    if (this.roots.length === 0) return { ok: false, message: 'no data roots: the Studio reads local stores only inside an open project (RCFD_STUDIO_DATA_ROOTS)' };
    let real: string;
    try {
      real = await realpath(p);
    } catch (e) {
      return { ok: false, message: `cannot resolve the path: ${(e as NodeJS.ErrnoException).code ?? (e as Error).message}` };
    }
    for (const r of this.roots) {
      let rr: string;
      try {
        rr = await realpath(r);
      } catch {
        continue;
      }
      if (isInside(rr, real)) return { ok: true, dir: real };
    }
    return { ok: false, message: 'the path is outside the Studio\'s data roots' };
  }
}
