// The file names under `cases/` and `notes/` of a project folder.
//
// A stopgap, recorded as decision D2 of F-805 (README, "The project home"): the engine's project server has no query
// that lists the case files or the note files of a project (`project_info` counts them, `list_meshes` names only the
// cases that use a mesh file or that it cannot read). Until it has one, the main process lists the names in those two
// folders of the folder contract (docs/ui-restart-plan.md section 8) the way the engine counts them
// (`rcfd_project::Project::cases` and `rcfd_workspace` `count_files`): every file below the folder, recursively, except
// the temporary files of an atomic write (`.<name>.<pid>.<n>.tmp`). It reads names only, never a file's content, and
// writes nothing. When the engine gains such a query, this file goes and the session asks the engine instead.
import { readdir } from 'node:fs/promises';
import path from 'node:path';

/** Most names listed per folder; the rest are counted (the engine's count is in `project_info`). */
export const MAX_NAMES = 5000;

/** The engine's rule for the temporary file of an atomic write (`rcfd_project::atomic::is_temp_name`). */
export const isTempName = (name: string): boolean => name.startsWith('.') && name.endsWith('.tmp');

export interface Names {
  /** Project-relative, `/`-separated, sorted by code unit (as the engine sorts its lists). */
  readonly names: readonly string[];
  readonly total: number;
}

/** Files below `<root>/<top>`, as `<top>/<...>` paths. A missing folder lists as empty (the engine counts it as 0). */
export async function filesUnder(root: string, top: string, max = MAX_NAMES): Promise<Names> {
  const out: string[] = [];
  let total = 0;
  const stack = [top];
  while (stack.length > 0) {
    const rel = stack.pop()!;
    let entries;
    try {
      entries = await readdir(path.join(root, rel), { withFileTypes: true });
    } catch (e) {
      if (rel === top && (e as NodeJS.ErrnoException).code === 'ENOENT') return { names: [], total: 0 };
      throw e;
    }
    for (const e of entries) {
      const child = `${rel}/${e.name}`;
      if (e.isDirectory()) stack.push(child);
      else if (e.isFile() && !isTempName(e.name)) {
        total++;
        if (out.length < max) out.push(child);
      }
    }
  }
  out.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  return { names: out, total };
}

/**
 * F-837: the top-level entries of `<root>/<top>` (files and folders), as the engine lists `designs/`
 * (`rcfd_project::Project::designs`, which `project_info` counts): names only, no temporary file, sorted. Missing: empty.
 */
export async function entriesOf(root: string, top: string, max = MAX_NAMES): Promise<{ entries: readonly { path: string; folder: boolean }[]; total: number }> {
  let list;
  try {
    list = await readdir(path.join(root, top), { withFileTypes: true });
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === 'ENOENT') return { entries: [], total: 0 };
    throw e;
  }
  const all = list.filter((e) => !isTempName(e.name)).map((e) => ({ path: `${top}/${e.name}`, folder: e.isDirectory() }));
  all.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  return { entries: all.slice(0, max), total: all.length };
}
