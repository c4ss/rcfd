// One open project folder: its engine project server, the lists of the project home as the engine answers them, and the
// live link. The main process asks the engine's change feed (`read_feed`, ADR 0188) every FEED_POLL_MS what changed in
// the folder, from anyone (Claude Code, the CLI, an editor, the runner); when anything did, it reads the lists again.
// Nothing is cached beyond the last answers, and nothing is written to the folder (G12).
//
// No Electron import: the unit tests drive it with a fake server.
import {
  FEED_POLL_MS,
  MAX_CHANGES,
  type CaseEntry,
  type ChangeEntry,
  type Listing,
  type MeshEntry,
  type NoteEntry,
  type ProjectInfo,
  type ProjectView,
  type RunEntry,
} from '../../shared/project.js';
import type { Names } from './listing.js';
import type { ProjectServerLike } from './server.js';

/** Pages the Studio reads of a paged list (100 entries each): the home shows at most 2000 runs and 2000 meshes. */
export const MAX_PAGES = 20;
const PAGE = 100;

export interface SessionDeps {
  startServer(folder: string): Promise<ProjectServerLike>;
  listFiles(root: string, top: string): Promise<Names>;
  now(): string;
  pollMs?: number;
}

const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));
const str = (v: unknown, d = ''): string => (typeof v === 'string' ? v : d);
const num = (v: unknown, d = 0): number => (typeof v === 'number' && Number.isFinite(v) ? v : d);
const norm = (p: string): string =>
  p
    .replace(/\\/g, '/')
    .split('/')
    .filter((c) => c !== '' && c !== '.')
    .join('/');

const empty = <T,>(error: string | null = null): Listing<T> => ({ items: [], total: 0, error });

/** One feed entry of `read_feed` as the home keeps it. */
export function changeEntry(e: unknown, seen: string): ChangeEntry {
  const o = (e ?? {}) as Record<string, unknown>;
  return {
    n: num(o['n']),
    source: str(o['source'], 'journal'),
    kind: str(o['kind']),
    text: str(o['text'], str(o['kind'])),
    path: typeof o['path'] === 'string' ? o['path'] : null,
    run: typeof o['run'] === 'string' ? o['run'] : null,
    t: typeof o['t'] === 'string' ? o['t'] : null,
    seen,
  };
}

export interface Lists {
  readonly info: ProjectInfo | null;
  readonly infoError: string | null;
  readonly cases: Listing<CaseEntry>;
  readonly meshes: Listing<MeshEntry>;
  readonly runs: Listing<RunEntry>;
  readonly notes: Listing<NoteEntry>;
  readonly skippedRuns: readonly { readonly path: string; readonly message: string }[];
}

/** Reads every list of the home from the engine (and the case and note names, listing.ts). */
export async function readLists(server: ProjectServerLike, root: string, listFiles: SessionDeps['listFiles']): Promise<Lists> {
  let info: ProjectInfo | null = null;
  let infoError: string | null = null;
  try {
    info = (await server.call('project_info')) as ProjectInfo;
  } catch (e) {
    infoError = message(e);
  }

  let runs: Listing<RunEntry>;
  const skippedRuns: { path: string; message: string }[] = [];
  try {
    const items: RunEntry[] = [];
    let total = 0;
    let offset: number | null = 0;
    for (let page = 0; page < MAX_PAGES && offset !== null; page++) {
      const r = (await server.call('list_runs', { offset, limit: PAGE })) as Record<string, unknown>;
      total = num(r['total']);
      for (const x of (r['runs'] as unknown[] | undefined) ?? []) {
        const o = x as Record<string, unknown>;
        items.push({ id: str(o['id']), case: str(o['case']), created: str(o['created']), state: str(o['state']), step: num(o['step']), time: num(o['time']) });
      }
      if (page === 0) for (const s of (r['skipped'] as unknown[] | undefined) ?? []) skippedRuns.push({ path: str((s as Record<string, unknown>)['path']), message: str((s as Record<string, unknown>)['message']) });
      offset = typeof r['next_offset'] === 'number' ? r['next_offset'] : null;
    }
    runs = { items, total, error: null };
  } catch (e) {
    runs = empty(message(e));
  }

  let meshes: Listing<MeshEntry>;
  const unread = new Map<string, string>();
  try {
    const items: MeshEntry[] = [];
    let total = 0;
    let offset: number | null = 0;
    for (let page = 0; page < MAX_PAGES && offset !== null; page++) {
      const r = (await server.call('list_meshes', { offset, limit: PAGE })) as Record<string, unknown>;
      total = num(r['total']);
      for (const x of (r['meshes'] as unknown[] | undefined) ?? []) {
        const o = x as Record<string, unknown>;
        items.push({
          path: str(o['path']),
          kind: str(o['kind'], 'file'),
          bytes: num(o['bytes']),
          files: num(o['files'], 1),
          used_by: ((o['used_by'] as unknown[] | undefined) ?? []).map((c) => str(c)),
        });
      }
      if (page === 0) for (const u of (r['unread'] as unknown[] | undefined) ?? []) unread.set(norm(str((u as Record<string, unknown>)['path'])), str((u as Record<string, unknown>)['message']));
      offset = typeof r['next_offset'] === 'number' ? r['next_offset'] : null;
    }
    meshes = { items, total, error: null };
  } catch (e) {
    meshes = empty(message(e));
  }

  let cases: Listing<CaseEntry>;
  try {
    const names = await listFiles(root, 'cases');
    const runsOf = new Map<string, number>();
    for (const r of runs.items) runsOf.set(norm(r.case), (runsOf.get(norm(r.case)) ?? 0) + 1);
    const meshesOf = new Map<string, string[]>();
    for (const m of meshes.items) for (const c of m.used_by) meshesOf.set(norm(c), [...(meshesOf.get(norm(c)) ?? []), m.path]);
    cases = {
      items: names.names.map((p) => ({ path: p, runs: runsOf.get(p) ?? 0, meshes: meshesOf.get(p) ?? [], unread: unread.get(p) ?? null })),
      total: names.total,
      error: null,
    };
  } catch (e) {
    cases = empty(message(e));
  }

  let notes: Listing<NoteEntry>;
  try {
    const names = await listFiles(root, 'notes');
    notes = { items: names.names.map((p) => ({ path: p })), total: names.total, error: null };
  } catch (e) {
    notes = empty(message(e));
  }

  return { info, infoError, cases, meshes, runs, notes, skippedRuns };
}

export class ProjectSession {
  private server: ProjectServerLike;
  private cursor = 0;
  private timer: NodeJS.Timeout | null = null;
  private refreshing: Promise<void> | null = null;
  private again = false;
  private closed = false;
  private readonly pollMs: number;
  view: ProjectView;

  private constructor(
    readonly folder: string,
    private readonly enginePath: string,
    private readonly deps: SessionDeps,
    private readonly onChange: (view: ProjectView) => void,
    server: ProjectServerLike,
    lists: Lists,
    cursor: number,
  ) {
    this.server = server;
    this.cursor = cursor;
    this.pollMs = deps.pollMs ?? FEED_POLL_MS;
    const now = deps.now();
    this.view = {
      folder,
      engine: { ...server.info, path: enginePath },
      ...lists,
      changes: [],
      openedAt: now,
      readAt: now,
      liveness: { live: true, since: now },
    };
  }

  /** Starts the folder's project server, reads the feed's cursor and then the lists, and starts following the feed. */
  static async open(folder: string, enginePath: string, deps: SessionDeps, onChange: (view: ProjectView) => void): Promise<ProjectSession> {
    const server = await deps.startServer(folder);
    try {
      // the cursor first: whatever changes after it is in the feed, so no change between the two reads is missed
      const feed = (await server.call('read_feed')) as { cursor?: unknown };
      const lists = await readLists(server, folder, deps.listFiles);
      const s = new ProjectSession(folder, enginePath, deps, onChange, server, lists, num(feed.cursor));
      s.watch(server);
      s.schedule();
      return s;
    } catch (e) {
      await server.close();
      throw e;
    }
  }

  private watch(server: ProjectServerLike): void {
    server.onExit((why) => {
      if (this.closed || server !== this.server) return;
      this.stopTimer();
      this.set({ liveness: { live: false, since: this.deps.now(), reason: `The engine's project server stopped (${why}).`, serverStopped: true } });
    });
  }

  private set(patch: Partial<ProjectView>): void {
    this.view = { ...this.view, ...patch };
    this.onChange(this.view);
  }

  private stopTimer(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
  }

  private schedule(): void {
    this.stopTimer();
    if (this.closed || this.server.exited) return;
    this.timer = setTimeout(() => void this.poll(), this.pollMs);
  }

  /** One read of the feed; a refresh of the lists when it holds anything. */
  private async poll(): Promise<void> {
    if (this.closed || this.server.exited) return;
    const server = this.server;
    try {
      const fresh: ChangeEntry[] = [];
      let dropped = 0;
      for (let i = 0; i < 10; i++) {
        const r = (await server.call('read_feed', { since: this.cursor, limit: 200 })) as Record<string, unknown>;
        const seen = this.deps.now();
        for (const e of (r['entries'] as unknown[] | undefined) ?? []) fresh.push(changeEntry(e, seen));
        dropped += num(r['dropped']);
        // `next` is the number of the last entry returned (null when none); `cursor` the feed's latest
        const next = r['next'];
        const latest = num(r['cursor'], this.cursor);
        if (typeof next === 'number') {
          this.cursor = next;
          if (latest > next) continue;
        } else {
          this.cursor = Math.max(this.cursor, latest);
        }
        break;
      }
      if (server !== this.server || this.closed) return;
      if (!this.view.liveness.live) this.set({ liveness: { live: true, since: this.deps.now() } });
      if (fresh.length > 0 || dropped > 0) {
        this.set({ changes: [...fresh.reverse(), ...this.view.changes].slice(0, MAX_CHANGES) });
        await this.refresh();
      }
    } catch (e) {
      if (server === this.server && !this.closed && !server.exited) {
        this.set({ liveness: { live: false, since: this.deps.now(), reason: `The engine's change feed could not be read: ${message(e)}`, serverStopped: false } });
      }
    } finally {
      if (server === this.server) this.schedule();
    }
  }

  /** Reads the lists again (coalesced: a request during a read makes one more read after it). */
  refresh(): Promise<void> {
    if (this.refreshing) {
      this.again = true;
      return this.refreshing;
    }
    this.refreshing = (async () => {
      try {
        do {
          this.again = false;
          if (this.closed || this.server.exited) break;
          const lists = await readLists(this.server, this.folder, this.deps.listFiles);
          if (this.closed) break;
          this.set({ ...lists, readAt: this.deps.now() });
        } while (this.again);
      } finally {
        this.refreshing = null;
      }
    })();
    return this.refreshing;
  }

  get serverStopped(): boolean {
    return this.server.exited;
  }

  /** After the server stopped: a new one, a new cursor, the lists read again, and the feed followed again. */
  async reconnect(): Promise<void> {
    if (this.closed || !this.server.exited) return this.refresh();
    const server = await this.deps.startServer(this.folder);
    if (this.closed) {
      await server.close();
      return;
    }
    try {
      const feed = (await server.call('read_feed')) as { cursor?: unknown };
      const lists = await readLists(server, this.folder, this.deps.listFiles);
      this.server = server;
      this.cursor = num(feed.cursor);
      this.watch(server);
      const now = this.deps.now();
      this.set({ ...lists, engine: { ...server.info, path: this.enginePath }, readAt: now, liveness: { live: true, since: now } });
      this.schedule();
    } catch (e) {
      await server.close();
      throw e;
    }
  }

  async close(): Promise<void> {
    this.closed = true;
    this.stopTimer();
    await this.server.close();
  }
}
