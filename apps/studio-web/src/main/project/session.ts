// One open project folder: its engine project server, the lists of the project home as the engine answers them, and the
// live link. The main process asks the engine's change feed (`read_feed`, ADR 0188) every FEED_POLL_MS what changed in
// the folder, from anyone (Claude Code, the CLI, an editor, the runner); when anything did, it reads the lists again.
// Nothing is cached beyond the last answers, and nothing is written to the folder (G12).
//
// No Electron import: the unit tests drive it with a fake server.
import {
  FEED_POLL_MS,
  MAX_CHANGES,
  RUN_BOOST_MS,
  RUN_FAST_POLL_MS,
  RUN_POLL_MS,
  RUN_REQUESTS_KEPT,
  RUN_SLOW_MS,
  type RunActionResult,
  type RunDetail,
  type CaseEntry,
  type ChangeEntry,
  type Listing,
  type MeshEntry,
  type DesignEntry,
  type NoteEntry,
  type ProjectInfo,
  type ProjectView,
  type RunEntry,
  type ProjectTool,
} from '../../shared/project.js';
import { RunControl, settle, type ControlDeps } from './control.js';
import { entriesOf, type Names } from './listing.js';
import { isActive, readRunDetail, readRuns, stale } from './runs.js';
import type { ProjectServerLike } from './server.js';

/** Pages the Studio reads of a paged list (100 entries each): the home shows at most 2000 runs and 2000 meshes. */
export const MAX_PAGES = 20;
const PAGE = 100;

/** Run details read between two pushes of the view while many are read (the first read of a large project). */
const DETAIL_BATCH = 25;

/** What a pass of `refresh` reads: the run details only, the run list (and details), or every list (and details). */
type ReadKind = 0 | 1 | 2;
const DETAILS: ReadKind = 0;
const RUNS: ReadKind = 1;
const FULL: ReadKind = 2;

export interface SessionDeps {
  startServer(folder: string): Promise<ProjectServerLike>;
  listFiles(root: string, top: string): Promise<Names>;
  now(): string;
  pollMs?: number;
  /** F-806: the progress read of active runs (default RUN_POLL_MS). */
  runPollMs?: number;
  /** F-807: run control (the not-read-only server and the runner's CLI); without it every run-control action is refused. */
  control?: ControlDeps;
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
  readonly designs: Listing<DesignEntry>;
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
  let skippedRuns: readonly { readonly path: string; readonly message: string }[] = [];
  try {
    ({ runs, skippedRuns } = await readRuns(server));
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

  // F-837: designs/, as the engine counts it (no query of the engine lists it; the names only, as listing.ts does)
  let designs: Listing<DesignEntry>;
  try {
    const d = await entriesOf(root, 'designs');
    designs = { items: d.entries, total: d.total, error: null };
  } catch (e) {
    designs = empty(message(e));
  }

  return { info, infoError, cases, meshes, runs, notes, designs, skippedRuns };
}

export class ProjectSession {
  private server: ProjectServerLike;
  private cursor = 0;
  private timer: NodeJS.Timeout | null = null;
  private refreshing: Promise<void> | null = null;
  /** The widest read asked for while a pass runs (null: none). */
  private wanted: ReadKind | null = null;
  private closed = false;
  private readonly pollMs: number;
  private readonly runPollMs: number;
  /** When the run list was last read (ms, Date.now()), for the progress read of active runs (RUN_POLL_MS). */
  private runsReadAt = 0;
  /** F-807: run control, made on the first action. */
  private control: RunControl | null = null;
  /** F-807: until when (Date.now()) the feed is read at RUN_FAST_POLL_MS after a run-control action. */
  private boostUntil = 0;
  private polling = false;
  private slowTimers = new Set<NodeJS.Timeout>();
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
    this.runPollMs = deps.runPollMs ?? RUN_POLL_MS;
    this.runsReadAt = Date.now();
    const now = deps.now();
    this.view = {
      folder,
      engine: { ...server.info, path: enginePath },
      ...lists,
      runDetails: {},
      runDetailErrors: {},
      runRequests: [],
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
      // the runs' details after the first view is out, so a project with many runs opens at once (G21)
      void s.read(DETAILS);
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
    const next = { ...this.view, ...patch };
    // F-807: whether the runner has acted on each run-control request, from the lists just read
    const requests = next.runRequests ?? [];
    const settled = requests.some((r) => !r.settled) ? settle(requests, next, performance.timeOrigin + performance.now()) : requests;
    this.view = settled === requests ? next : { ...next, runRequests: settled };
    this.onChange(this.view);
  }

  private stopTimer(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
  }

  private schedule(): void {
    this.stopTimer();
    if (this.closed || this.server.exited) return;
    this.timer = setTimeout(() => void this.poll(), Date.now() < this.boostUntil ? Math.min(RUN_FAST_POLL_MS, this.pollMs) : this.pollMs);
  }

  /** One read of the feed; a refresh of the lists when it holds anything. */
  private async poll(): Promise<void> {
    if (this.closed || this.server.exited || this.polling) return;
    this.polling = true;
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
      } else if (this.hasActiveRun() && Date.now() - this.runsReadAt >= this.runPollMs) {
        // F-806: the feed reports state changes, not progress; an active run's step is read from its status.json
        await this.read(RUNS);
      }
    } catch (e) {
      if (server === this.server && !this.closed && !server.exited) {
        this.set({ liveness: { live: false, since: this.deps.now(), reason: `The engine's change feed could not be read: ${message(e)}`, serverStopped: false } });
      }
    } finally {
      this.polling = false;
      if (server === this.server) this.schedule();
    }
  }

  /**
   * F-807: one run-control action. The engine's answer is in the view at once (the request, with the run it made or is
   * about), the feed is then read fast for RUN_BOOST_MS so the runner's change shows as soon as it writes it, and the
   * run list is read again now. A request still waiting after RUN_SLOW_MS is marked slow.
   */
  async dispatch(action: string, params: Readonly<Record<string, unknown>>): Promise<RunActionResult> {
    if (!this.deps.control) {
      return {
        ok: false,
        request: null,
        failure: {
          code: 'engine',
          problem: 'Run control is not set up in this Studio: it has no way to reach the engine’s run tools.',
          recovery: 'Start the Studio with RCFD_STUDIO_MCP set to the path of rcfd-mcp, with rcfd-runner and rcfd beside it.',
          engine: null,
        },
      };
    }
    if (this.closed) {
      return { ok: false, request: null, failure: { code: 'no_project', problem: 'The project was closed.', recovery: 'Open the project again.', engine: null } };
    }
    this.control ??= new RunControl(this.folder, this.deps.control);
    const r = await this.control.dispatch(action, params, this.view);
    if (this.closed || !r.request) return r;
    this.set({ runRequests: [r.request, ...(this.view.runRequests ?? [])].slice(0, RUN_REQUESTS_KEPT) });
    if (r.request.outcome === 'accepted') {
      this.boostUntil = Date.now() + RUN_BOOST_MS;
      if (!this.polling) this.schedule();
      void this.read(RUNS);
      const t = setTimeout(() => {
        this.slowTimers.delete(t);
        if (!this.closed) this.set({});
      }, RUN_SLOW_MS + 20);
      this.slowTimers.add(t);
    }
    return r;
  }

  /** Reads the lists again (coalesced: a request during a read makes one more read after it). */
  refresh(): Promise<void> {
    return this.read(FULL);
  }

  /** Whether a listed run is queued, running or paused (as last read, by its detail when that is newer). */
  private hasActiveRun(): boolean {
    return this.view.runs.items.some((r) => isActive(this.view.runDetails[r.id]?.state ?? r.state));
  }

  /**
   * One coalesced read: every list (FULL), the run list (RUNS) or nothing but the run details (DETAILS), then the
   * `run_status` of each run whose state or step changed since it was last read. A request during a pass makes one
   * more pass after it, as wide as the widest request.
   */
  private read(kind: ReadKind): Promise<void> {
    this.wanted = this.wanted === null ? kind : (Math.max(this.wanted, kind) as ReadKind);
    if (this.refreshing) return this.refreshing;
    this.refreshing = (async () => {
      try {
        while (this.wanted !== null) {
          const k = this.wanted;
          this.wanted = null;
          if (this.closed || this.server.exited) break;
          if (k === FULL) {
            const lists = await readLists(this.server, this.folder, this.deps.listFiles);
            if (this.closed) break;
            this.runsReadAt = Date.now();
            this.set({ ...lists, readAt: this.deps.now() });
          } else if (k === RUNS) {
            let read: Awaited<ReturnType<typeof readRuns>> | null = null;
            try {
              read = await readRuns(this.server);
            } catch (e) {
              if (!this.closed) this.set({ runs: { items: [], total: 0, error: message(e) }, readAt: this.deps.now() });
            }
            if (this.closed) break;
            this.runsReadAt = Date.now();
            if (read) this.set({ runs: read.runs, skippedRuns: read.skippedRuns, readAt: this.deps.now() });
          }
          await this.readDetails(k === FULL);
        }
      } finally {
        this.refreshing = null;
      }
    })();
    return this.refreshing;
  }

  /** `run_status` of each listed run whose state or step changed since its last answer; details of runs gone are dropped. */
  private async readDetails(retryErrors: boolean): Promise<void> {
    const items = this.view.runs.items;
    const listed = new Set(items.map((r) => r.id));
    const details: Record<string, RunDetail> = {};
    const errors: Record<string, string> = {};
    for (const [id, d] of Object.entries(this.view.runDetails)) if (listed.has(id)) details[id] = d;
    for (const [id, m] of Object.entries(this.view.runDetailErrors)) if (listed.has(id)) errors[id] = m;
    // a run the engine could not answer for is asked again when its state changes, when it is active, or on a full read
    const todo = items.filter((r) => stale(r, details[r.id]) && (retryErrors || !(r.id in errors) || isActive(r.state)));
    const pruned = Object.keys(details).length !== Object.keys(this.view.runDetails).length || Object.keys(errors).length !== Object.keys(this.view.runDetailErrors).length;
    if (todo.length === 0) {
      if (pruned) this.set({ runDetails: details, runDetailErrors: errors });
      return;
    }
    const server = this.server;
    let n = 0;
    for (const r of todo) {
      if (this.closed || server !== this.server || server.exited) return;
      try {
        details[r.id] = await readRunDetail(server, r.id, this.deps.now);
        delete errors[r.id];
      } catch (e) {
        errors[r.id] = message(e);
      }
      if (++n % DETAIL_BATCH === 0) this.set({ runDetails: { ...details }, runDetailErrors: { ...errors } });
    }
    if (this.closed || server !== this.server) return;
    this.set({ runDetails: details, runDetailErrors: errors });
  }

  /** F-837 (ADR 0733): the runner writes its budgets through the run-control server (made on first use, as for an action). */
  setRunnerConfig(args: Record<string, unknown>): Promise<unknown> {
    if (!this.deps.control) return Promise.reject(new Error('Run control is not set up in this Studio, so the runner’s budgets cannot be written.'));
    if (this.closed) return Promise.reject(new Error('the project was closed'));
    this.control ??= new RunControl(this.folder, this.deps.control);
    return this.control.setRunnerConfig(args);
  }

  /**
   * F-810: one query of this folder's read-only project server, for the run workspace (src/main/run/). Refused when
   * the session is closed or its server stopped.
   */
  query(tool: ProjectTool, args: Record<string, unknown>): Promise<unknown> {
    if (this.closed) return Promise.reject(new Error('the project was closed'));
    if (this.server.exited) return Promise.reject(new Error("the engine's project server stopped; Reconnect starts a new one"));
    return this.server.call(tool, args);
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
      this.runsReadAt = Date.now();
      this.set({ ...lists, engine: { ...server.info, path: this.enginePath }, readAt: now, liveness: { live: true, since: now } });
      this.schedule();
      void this.read(DETAILS);
    } catch (e) {
      await server.close();
      throw e;
    }
  }

  async close(): Promise<void> {
    this.closed = true;
    this.stopTimer();
    for (const t of this.slowTimers) clearTimeout(t);
    this.slowTimers.clear();
    await Promise.all([this.server.close(), this.control?.close()]);
  }
}
