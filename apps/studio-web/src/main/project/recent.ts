// The recent list: the folders the Studio opened, most recent first, with the time each was last opened. It is a
// preference of this Studio, not project state (G12): it holds no name, count or content of a project, only where the
// folder is, so nothing in it can disagree with the folder.
import { mkdir, readFile, rename, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import type { RecentProject } from '../../shared/project.js';

export const MAX_RECENT = 10;
const FORMAT = 'rcfd-studio-recent';

export interface StoredRecent {
  readonly path: string;
  readonly opened: string;
}

/** Same folder: Windows and macOS file systems ignore letter case by default. */
export const sameFolder = (a: string, b: string, platform: string = process.platform): boolean => {
  const n = (p: string) => path.resolve(p).replace(/[\\/]+$/, '');
  return platform === 'win32' || platform === 'darwin' ? n(a).toLowerCase() === n(b).toLowerCase() : n(a) === n(b);
};

/** The list after opening `folder` at `when`: it moves to the front, the oldest beyond MAX_RECENT drop off. */
export function withOpened(list: readonly StoredRecent[], folder: string, when: string): StoredRecent[] {
  return [{ path: folder, opened: when }, ...list.filter((r) => !sameFolder(r.path, folder))].slice(0, MAX_RECENT);
}

export function without(list: readonly StoredRecent[], folder: string): StoredRecent[] {
  return list.filter((r) => !sameFolder(r.path, folder));
}

/** Parses the file; anything unexpected is dropped rather than trusted. */
export function parseRecent(text: string): StoredRecent[] {
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch {
    return [];
  }
  const o = v as { format?: unknown; projects?: unknown };
  if (o?.format !== FORMAT || !Array.isArray(o.projects)) return [];
  const out: StoredRecent[] = [];
  for (const p of o.projects as unknown[]) {
    const r = p as { path?: unknown; opened?: unknown };
    if (typeof r?.path === 'string' && path.isAbsolute(r.path) && typeof r.opened === 'string') out.push({ path: r.path, opened: r.opened });
    if (out.length === MAX_RECENT) break;
  }
  return out;
}

export const formatRecent = (list: readonly StoredRecent[]): string =>
  JSON.stringify({ format: FORMAT, version: 1, projects: list.map((r) => ({ path: r.path, opened: r.opened })) }, null, 2) + '\n';

export class RecentStore {
  constructor(private readonly file: string) {}

  async load(): Promise<StoredRecent[]> {
    try {
      return parseRecent(await readFile(this.file, 'utf8'));
    } catch {
      return [];
    }
  }

  /** Written whole to a temporary file and renamed over the old one. */
  async save(list: readonly StoredRecent[]): Promise<void> {
    await mkdir(path.dirname(this.file), { recursive: true });
    const tmp = `${this.file}.${process.pid}.tmp`;
    await writeFile(tmp, formatRecent(list));
    await rename(tmp, this.file);
  }
}

/** The list as the renderer sees it, with whether each folder is still there. */
export async function withExistence(list: readonly StoredRecent[]): Promise<RecentProject[]> {
  return Promise.all(
    list.map(async (r) => {
      let exists = false;
      try {
        exists = (await stat(r.path)).isDirectory();
      } catch {
        exists = false;
      }
      return { path: r.path, opened: r.opened, exists };
    }),
  );
}
