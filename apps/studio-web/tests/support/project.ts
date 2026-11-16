// The project home's e2e support (F-805): the engine's project server (rcfd-mcp), copies of the example projects in a
// temporary folder, the folder dialog stubbed in the main process, and a snapshot of a folder to show the Studio wrote
// nothing to it (G12).
import type { ElectronApplication, Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { cp, mkdir, mkdtemp, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import type { ProjectState } from '../../src/shared/project.js';
import { ProjectServer } from '../../src/main/project/server.js';
import { appRoot } from './rcfd.js';

const exe = process.platform === 'win32' ? 'rcfd-mcp.exe' : 'rcfd-mcp';
export const repoRoot = path.resolve(appRoot, '..', '..');

/** rcfd-mcp: RCFD_MCP_BIN, then $CARGO_TARGET_DIR/release, then target/release of this checkout or a folder above. */
export function findMcp(): string | null {
  const candidates: string[] = [];
  if (process.env['RCFD_MCP_BIN']) candidates.push(process.env['RCFD_MCP_BIN']);
  if (process.env['CARGO_TARGET_DIR']) candidates.push(path.join(process.env['CARGO_TARGET_DIR'], 'release', exe));
  for (let dir = appRoot; ; dir = path.dirname(dir)) {
    candidates.push(path.join(dir, 'target', 'release', exe));
    if (path.dirname(dir) === dir) break;
  }
  return candidates.find((c) => existsSync(c)) ?? null;
}

export const NO_MCP = 'rcfd-mcp was not found: build it with `cargo build --release -p rcfd-mcp`, or set RCFD_MCP_BIN to its path';

/** A fresh temporary folder for one test file. */
export const scratch = (tag: string): Promise<string> => mkdtemp(path.join(tmpdir(), `rcfd-f805-${tag}-`));

/** A copy of example-projects/<name> in `dir`. */
export async function copyExample(name: string, dir: string): Promise<string> {
  const to = path.join(dir, name);
  await cp(path.join(repoRoot, 'example-projects', name), to, { recursive: true });
  return to;
}

/** An empty project folder: the manifest and the folders of the contract, nothing in them. */
export async function emptyProject(dir: string, name = 'Empty project'): Promise<string> {
  const root = path.join(dir, 'empty-project');
  for (const d of ['cases', 'meshes', 'runs', 'designs', 'notes']) await mkdir(path.join(root, d), { recursive: true });
  await writeFile(
    path.join(root, 'rcfd-project.json'),
    JSON.stringify({ schema_version: 1, name, created: '2026-11-16T09:00:00.000Z', solver_version_created: 'rcfd 0.1.0' }, null, 2) + '\n',
  );
  return root;
}

/** Makes the main process's folder dialog answer `folder` (null: cancelled) without showing. */
export async function stubDialog(app: ElectronApplication, folder: string | null): Promise<void> {
  await app.evaluate(({ dialog }, f) => {
    (dialog as unknown as { showOpenDialog: unknown }).showOpenDialog = async () => (f === null ? { canceled: true, filePaths: [] } : { canceled: false, filePaths: [f] });
  }, folder);
}

export const projectState = (page: Page): Promise<ProjectState> => page.evaluate(() => window.studioProject.state());

/** Every file below `root` with its size and a hash of its content (the folder as the Studio must leave it). */
export async function snapshot(root: string): Promise<Record<string, string>> {
  const out: Record<string, string> = {};
  const walk = async (rel: string): Promise<void> => {
    for (const e of await readdir(path.join(root, rel), { withFileTypes: true })) {
      const r = rel ? `${rel}/${e.name}` : e.name;
      if (e.isDirectory()) {
        out[`${r}/`] = 'dir';
        await walk(r);
      } else {
        const p = path.join(root, r);
        const s = await stat(p);
        out[r] = `${s.size}:${createHash('sha256').update(await readFile(p)).digest('hex').slice(0, 16)}:${s.mtimeMs}`;
      }
    }
  };
  await walk('');
  return out;
}

/** What the engine itself answers for `folder`, asked directly (not through the Studio). */
export async function engineAnswers(mcp: string, folder: string) {
  const server = await ProjectServer.start(mcp, folder);
  try {
    const info = (await server.call('project_info')) as { name: string; counts: Record<string, number> };
    const runs = (await server.call('list_runs', { limit: 100 })) as { runs: { id: string; state: string }[]; total: number };
    const meshes = (await server.call('list_meshes', { limit: 100 })) as { meshes: { path: string }[]; total: number };
    return { info, runs, meshes };
  } finally {
    await server.close();
  }
}

/** The process ids of rcfd-mcp processes whose command line names `folder`. */
export function mcpPids(folder: string): number[] {
  if (process.platform === 'win32') {
    const ps = `Get-CimInstance Win32_Process -Filter "Name='rcfd-mcp.exe'" | ForEach-Object { "$($_.ProcessId)|$($_.CommandLine)" }`;
    const out = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', ps], { encoding: 'utf8' });
    return out
      .split(/\r?\n/)
      .filter((l) => l.toLowerCase().includes(folder.toLowerCase()))
      .map((l) => Number(l.split('|')[0]))
      .filter((n) => Number.isInteger(n) && n > 0);
  }
  const out = execFileSync('ps', ['-eo', 'pid=,args='], { encoding: 'utf8' });
  return out
    .split('\n')
    .filter((l) => l.includes('rcfd-mcp') && l.includes(folder))
    .map((l) => Number(l.trim().split(/\s+/)[0]));
}

/** A copy of one run folder under a new id (as the runner would have written it), for the live-update test. */
export async function copyRun(root: string, from: string, to: string): Promise<void> {
  await cp(path.join(root, 'runs', from), path.join(root, 'runs', to), { recursive: true });
  const file = path.join(root, 'runs', to, 'run.json');
  const run = JSON.parse(await readFile(file, 'utf8')) as { id: string };
  run.id = to;
  await writeFile(file, JSON.stringify(run, null, 2) + '\n');
}
