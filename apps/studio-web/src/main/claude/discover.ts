// The Claude Code sessions of a project folder, from the two sources crates/rcfd-claude/src/discover.rs reads (a port:
// the engine has no entry point the Studio can call for it; F-814 report), each allowed to fail:
//
// 1. the transcript folder `<claude home>/projects/<folder with every non-alphanumeric character replaced by '-'>`
//    (documented naming), each session confirmed by the `cwd` inside its transcript because the naming is not
//    injective;
// 2. `claude agents --json` (observed fields: pid, cwd, kind, startedAt, sessionId, name, status; a running `claude -p`
//    is listed too).
//
// A session is OPEN when the listing has it with a live process, CLOSED when the listing worked and does not have it
// (or its process is gone), and UNKNOWN when the listing failed or could not be made: the Studio writes only to CLOSED
// sessions (and to new ones it starts).
import { readdir, stat } from 'node:fs/promises';
import path from 'node:path';
import { isUuid } from '../../shared/claude.js';
import { head } from './transcript.js';

export interface AgentEntry {
  readonly session: string;
  readonly pid: number | null;
  readonly cwd: string | null;
  readonly name: string | null;
}

export type OpenState = { readonly kind: 'open'; readonly pid: number } | { readonly kind: 'closed' } | { readonly kind: 'unknown' };

export interface SessionInfo {
  readonly session: string;
  /** Transcript file, when there is one. */
  readonly transcript: string | null;
  readonly state: OpenState;
  readonly name: string | null;
  readonly title: string | null;
  /** Transcript modification time (ms since the epoch). */
  readonly modified: number | null;
}

export interface Discovery {
  /** Newest transcript first; sessions listed without a transcript last. */
  readonly sessions: SessionInfo[];
  readonly warnings: string[];
}

/** The transcript folder name of a working folder (documented rule: non-alphanumerics become '-'). */
export function projectDirName(cwd: string): string {
  return cwd.replace(/[^A-Za-z0-9]/g, '-');
}

/** Same folder? Separators and a trailing separator are ignored; ASCII case is ignored on Windows. */
export function samePath(a: string, b: string): boolean {
  const n = (s: string) => {
    const t = s.replace(/\\/g, '/').replace(/\/+$/, '');
    return process.platform === 'win32' ? t.toLowerCase() : t;
  };
  return n(a) === n(b);
}

/** Parses `claude agents --json`; rows without a string `sessionId` are skipped. Throws on a non-array answer. */
export function parseAgents(json: string): AgentEntry[] {
  let v: unknown;
  try {
    v = JSON.parse(json);
  } catch (e) {
    throw new Error(`not JSON: ${(e as Error).message}`);
  }
  if (!Array.isArray(v)) throw new Error('not a JSON array');
  const s = (r: Record<string, unknown>, k: string) => (typeof r[k] === 'string' ? (r[k] as string) : null);
  const out: AgentEntry[] = [];
  for (const x of v) {
    if (typeof x !== 'object' || x === null) continue;
    const r = x as Record<string, unknown>;
    const session = s(r, 'sessionId');
    if (session === null) continue;
    const pid = r['pid'];
    out.push({ session, pid: Number.isSafeInteger(pid) && (pid as number) > 0 ? (pid as number) : null, cwd: s(r, 'cwd'), name: s(r, 'name') });
  }
  return out;
}

/** Is process `pid` alive? (`kill(pid, 0)`: no signal is sent; EPERM means it exists under another user.) */
export function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === 'EPERM';
  }
}

/** A transcript's head, kept per file (the first lines do not change once written). */
export type HeadCache = Map<string, { cwd: string | null; title: string | null }>;

/**
 * Combines the two sources. `listing` is the parsed `claude agents --json`, or why it failed (or could not be made);
 * `alive` tells whether a pid is a live process; `folders` are the forms of the project folder (as opened, and its real
 * path) whose transcript folders are read.
 */
export async function classify(
  folders: readonly string[],
  claudeHome: string,
  listing: { ok: true; agents: AgentEntry[] } | { ok: false; reason: string },
  alive: (pid: number) => boolean,
  heads: HeadCache = new Map(),
): Promise<Discovery> {
  const warnings: string[] = [];
  const files = new Map<string, { file: string; modified: number | null; title: string | null }>();
  const mine = (c: string) => folders.some((f) => samePath(c, f));
  const dirs = [...new Set(folders.map(projectDirName))].map((d) => path.join(claudeHome, 'projects', d));
  for (const dir of dirs) {
    let names: string[];
    try {
      names = await readdir(dir);
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== 'ENOENT') warnings.push(`${dir}: ${(e as Error).message}`);
      continue;
    }
    for (const name of names) {
      if (!name.endsWith('.jsonl')) continue;
      const id = name.slice(0, -'.jsonl'.length);
      if (!isUuid(id) || files.has(id)) continue;
      const file = path.join(dir, name);
      try {
        const st = await stat(file);
        if (!st.isFile()) continue;
        let h = heads.get(file);
        if (!h || (h.cwd === null && h.title === null)) {
          h = await head(file);
          heads.set(file, h);
        }
        if (h.cwd !== null && !mine(h.cwd)) continue; // another folder whose name maps to the same transcript folder
        files.set(id, { file, modified: st.mtimeMs, title: h.title });
      } catch (e) {
        warnings.push(`${file}: ${(e as Error).message}`);
      }
    }
  }
  const agents = listing.ok ? listing.agents.filter((a) => a.cwd !== null && mine(a.cwd)) : null;
  if (!listing.ok) warnings.push(`claude agents --json: ${listing.reason}`);
  const stateOf = (id: string): { state: OpenState; name: string | null } => {
    if (agents === null) return { state: { kind: 'unknown' }, name: null };
    // any cwd counts for the open state: the session is the unit of the one-writer rule
    const all = listing.ok ? listing.agents.filter((a) => a.session === id) : [];
    const live = all.find((a) => a.pid !== null && alive(a.pid));
    const name = agents.find((a) => a.session === id)?.name ?? live?.name ?? null;
    return live ? { state: { kind: 'open', pid: live.pid! }, name } : { state: { kind: 'closed' }, name };
  };
  const sessions: SessionInfo[] = [...files.entries()]
    .sort((a, b) => (b[1].modified ?? 0) - (a[1].modified ?? 0))
    .map(([id, f]) => ({ session: id, transcript: f.file, modified: f.modified, title: f.title, ...stateOf(id) }));
  for (const a of agents ?? []) {
    if (files.has(a.session) || !isUuid(a.session) || sessions.some((s) => s.session === a.session)) continue;
    const st = stateOf(a.session);
    if (st.state.kind !== 'closed') sessions.push({ session: a.session, transcript: null, modified: null, title: null, ...st });
  }
  return { sessions, warnings };
}
