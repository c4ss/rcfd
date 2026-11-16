// The files of a project folder that change what `claude -p` runs or reads (ADR 0134 W48-10 (b) and Amendment 2), a
// port of crates/rcfd-claude/src/trust.rs: `.mcp.json`, everything under `.claude/` and `.claude-plugin/`, and
// `CLAUDE.md`, at any depth, plus the allow-list file `.rcfd/claude-allow.json`. A received folder can carry hooks or
// instructions there, so the panel lists them with their hashes and starts no chat until the user has confirmed exactly
// those files.
//
// The confirmed lists live in one file in the Studio's user data (`claude-trust.json`), keyed by the real path of the
// project, never inside the project folder: a record shipped inside a folder confirms nothing. Unlike the Rust record
// (fnv64), the hashes here are SHA-256.
//
// Known limits (as in the Rust crate): `CLAUDE.md` files in folders above the project are not scanned; two Studio
// processes confirming at the same moment can lose one confirmation (which then asks again).
import { createHash } from 'node:crypto';
import { lstat, mkdir, readdir, readFile, readlink, realpath, rename, writeFile } from 'node:fs/promises';
import path from 'node:path';
import type { ControlFileView, FolderTrust } from '../../shared/claude.js';
import { ALLOW_FILE } from './allow.js';

/** Names that change Claude Code's behaviour, lowercase, matched per path component. */
const NAMES = ['.mcp.json', 'claude.md', '.claude', '.claude-plugin'];

const fold = (name: string) => name.replace(/[. ]+$/, '').toLowerCase();
const isControlName = (name: string) => NAMES.includes(fold(name));
export const isAllowFile = (rel: string) => rel.split('/').map(fold).join('/') === ALLOW_FILE;

const sha = (b: Buffer | string) => `sha256:${createHash('sha256').update(b).digest('hex')}`;

/**
 * Every control file and link under `project`, sorted by path. Links are listed and never followed (the scan cannot see
 * what is behind one, and Claude Code may follow it). A directory or control file that cannot be read fails the scan, so
 * an unreadable folder is never taken for a clean one.
 */
export async function scan(project: string): Promise<ControlFileView[]> {
  const out: ControlFileView[] = [];
  const stack: { dir: string; rel: string; under: boolean }[] = [{ dir: project, rel: '', under: false }];
  while (stack.length > 0) {
    const { dir, rel, under } = stack.pop()!;
    for (const e of await readdir(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      const r = rel ? `${rel}/${e.name}` : e.name;
      const control = under || isControlName(e.name);
      if (e.isSymbolicLink()) {
        const target = await readlink(p);
        out.push({ path: r, kind: 'link', hash: sha(target), bytes: Buffer.byteLength(target) });
      } else if (e.isDirectory()) {
        stack.push({ dir: p, rel: r, under: control });
      } else if (control || isAllowFile(r)) {
        const bytes = await readFile(p);
        out.push({ path: r, kind: 'file', hash: sha(bytes), bytes: bytes.length });
      }
    }
  }
  return out.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
}

const same = (a: readonly ControlFileView[], b: readonly ControlFileView[]) =>
  a.length === b.length && a.every((x, i) => x.path === b[i]!.path && x.kind === b[i]!.kind && x.hash === b[i]!.hash && x.bytes === b[i]!.bytes);

/** The token of a scan: the hash of its entries, so a confirmation names exactly what was shown. */
export const scanToken = (files: readonly ControlFileView[]) => sha(JSON.stringify(files)).slice('sha256:'.length, 'sha256:'.length + 32);

interface StoreFile {
  version: 1;
  projects: Record<string, ControlFileView[]>;
}

/** The per-user record of confirmed folders: `{"version": 1, "projects": {<real path>: [entries]}}`, replaced whole. */
export class TrustStore {
  constructor(readonly file: string) {}

  static async key(project: string): Promise<string> {
    return realpath(project);
  }

  private async refuseInside(project: string): Promise<void> {
    const norm = (p: string) => {
      const t = path.resolve(p);
      return process.platform === 'win32' ? t.toLowerCase() : t;
    };
    const bases = [norm(project)];
    try {
      bases.push(norm(await realpath(project)));
    } catch {
      // the lexical form is checked
    }
    const f = norm(this.file);
    if (bases.some((b) => f === b || f.startsWith(b + path.sep))) throw new Error('the trust record must not be inside the project folder');
  }

  private async load(): Promise<StoreFile> {
    let text: string;
    try {
      text = await readFile(this.file, 'utf8');
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code === 'ENOENT') return { version: 1, projects: {} };
      throw e;
    }
    const v = JSON.parse(text) as Partial<StoreFile>;
    if (v.version !== 1 || typeof v.projects !== 'object' || v.projects === null) throw new Error(`${this.file}: trust record unreadable`);
    return v as StoreFile;
  }

  async record(project: string): Promise<ControlFileView[] | null> {
    await this.refuseInside(project);
    return (await this.load()).projects[await TrustStore.key(project)] ?? null;
  }

  async confirm(project: string, files: readonly ControlFileView[]): Promise<void> {
    await this.refuseInside(project);
    const s = await this.load();
    s.projects[await TrustStore.key(project)] = [...files];
    await mkdir(path.dirname(this.file), { recursive: true });
    const tmp = `${this.file}.${process.pid}.tmp`;
    await writeFile(tmp, JSON.stringify(s, null, 2) + '\n');
    await rename(tmp, this.file);
  }
}

/** Compares the scan with the record: a folder without control files, or with exactly the confirmed ones, may chat. */
export async function checkFolder(project: string, store: TrustStore): Promise<{ trust: FolderTrust; files: ControlFileView[] }> {
  let files: ControlFileView[];
  let record: ControlFileView[] | null;
  try {
    // the project folder itself must not be a link (the scan never follows one)
    if ((await lstat(project)).isSymbolicLink()) throw new Error('the project folder is a link; open the folder it points to');
    files = await scan(project);
    if (files.length === 0) return { trust: { state: 'confirmed', files: 0 }, files };
    record = await store.record(project);
  } catch (e) {
    return { trust: { state: 'error', message: `the folder's Claude Code control files cannot be checked: ${(e as Error).message}` }, files: [] };
  }
  if (record && same(record, files)) return { trust: { state: 'confirmed', files: files.length }, files };
  const old = new Map((record ?? []).map((c) => [c.path, c]));
  const now = new Map(files.map((c) => [c.path, c]));
  const changed = [
    ...files.filter((c) => {
      const o = old.get(c.path);
      return !o || o.hash !== c.hash || o.kind !== c.kind || o.bytes !== c.bytes;
    }).map((c) => c.path),
    ...[...old.keys()].filter((p) => !now.has(p)),
  ].sort();
  return { trust: { state: 'pending', token: scanToken(files), files, changed }, files };
}
