// The project home of the main process: the engine's configuration, the recent list, the folder being opened, the last
// failed open and the open project's session. The renderer reaches it only through the project port (bridge.ts), and
// gets the whole state after every change.
//
// No Electron import: the folder dialog is passed in, so the unit tests drive the home with fakes.
import { existsSync } from 'node:fs';
import { readdir, stat } from 'node:fs/promises';
import path from 'node:path';
import { FIRST_WATCH_MS, type EngineStatus, type OpenFailure, type PickResult, type ProjectState, type ProjectTool, type ProjectView, type RunActionResult } from '../../shared/project.js';
import { ensureRunnerCli, type ControlDeps } from './control.js';
import { filesUnder, type Names } from './listing.js';
import { sameFolder, withExistence, withOpened, without, type StoredRecent } from './recent.js';
import { ProjectServer, START_TIMEOUT_MS, StartFailure, type ProjectServerLike } from './server.js';
import { ProjectSession } from './session.js';
import { testDelay } from '../work/delay.js';

export interface RecentLike {
  load(): Promise<StoredRecent[]>;
  save(list: readonly StoredRecent[]): Promise<void>;
}

export interface HomeDeps {
  readonly engine: EngineStatus;
  readonly recent: RecentLike;
  /** Shows the folder dialog; null when cancelled. */
  pickFolder(): Promise<string | null>;
  startServer?(exe: string, folder: string): Promise<ProjectServerLike>;
  listFiles?(root: string, top: string): Promise<Names>;
  isDirectory?(p: string): Promise<boolean>;
  now?(): string;
  pollMs?: number;
  /** F-807: the run-control server (default: rcfd-mcp --project <folder>, not read-only, the engine's path). */
  startControlServer?(exe: string, folder: string): Promise<ProjectServerLike>;
  /** F-807: `rcfd-runner ensure` (default: the rcfd-runner beside the engine's project server). */
  ensureRunner?(folder: string): ReturnType<ControlDeps['ensureRunner']>;
  /** F-813: how often a folder that is not a project is checked for its project file (default FIRST_WATCH_MS; 0: never). */
  firstWatchMs?: number;
}

/** F-813: the project file whose appearance turns a folder into a project (rcfd_project's MANIFEST). */
export const PROJECT_FILE = 'rcfd-project.json';

/** F-813: entries at the top of `folder` (names only, nothing read), or null when it cannot be listed. */
export async function countEntries(folder: string): Promise<number | null> {
  try {
    return (await readdir(folder)).length;
  } catch {
    return null;
  }
}

export const RUNNER_EXE = process.platform === 'win32' ? 'rcfd-runner.exe' : 'rcfd-runner';

/** A request the home refuses (bridge.ts turns it into the reply's error). */
export class HomeRefusal extends Error {
  constructor(
    readonly code: 'not_recent',
    message: string,
  ) {
    super(message);
  }
}

const isDir = async (p: string): Promise<boolean> => {
  try {
    return (await stat(p)).isDirectory();
  } catch {
    return false;
  }
};

export class ProjectHome {
  private stored: StoredRecent[] = [];
  private session: ProjectSession | null = null;
  private opening: string | null = null;
  private failure: OpenFailure | null = null;
  private recentView: ProjectState['recent'] = [];
  private listeners = new Set<(s: ProjectState) => void>();
  private openSeq = 0;
  private readonly now: () => string;
  /** F-813: the timer that checks a not-a-project folder for its project file, and that folder. */
  private watch: { folder: string; timer: ReturnType<typeof setInterval> } | null = null;

  constructor(private readonly deps: HomeDeps) {
    this.now = deps.now ?? (() => new Date().toISOString());
  }

  get state(): ProjectState {
    return {
      engine: this.deps.engine,
      recent: this.recentView,
      opening: this.opening,
      failure: this.failure,
      project: this.session?.view ?? null,
    };
  }

  onState(listener: (s: ProjectState) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private emit(): void {
    const s = this.state;
    for (const l of this.listeners) {
      try {
        l(s);
      } catch {
        // a listener's failure is its own
      }
    }
  }

  async init(): Promise<void> {
    this.stored = await this.deps.recent.load();
    await this.updateRecentView();
    this.emit();
  }

  private async updateRecentView(): Promise<void> {
    this.recentView = await withExistence(this.stored);
  }

  /**
   * Applies one change to the recent list as the file holds it now, not as it was read at start: another Studio window
   * may have written it since, and its entries are kept (F-829, G18: no silent overwrite of another writer's change).
   */
  private async setRecent(change: (list: StoredRecent[]) => StoredRecent[]): Promise<void> {
    let latest = this.stored;
    try {
      latest = await this.deps.recent.load();
    } catch {
      // unreadable: the list as read before
    }
    const list = change(latest);
    this.stored = list;
    try {
      await this.deps.recent.save(list);
    } catch {
      // the list is a convenience: a folder that cannot be written does not stop the Studio
    }
    await this.updateRecentView();
  }

  async pick(): Promise<PickResult> {
    const folder = await this.deps.pickFolder();
    if (folder !== null) await this.open(folder);
    return { folder };
  }

  /** Opens a folder of the recent list; the renderer cannot name any other path. */
  async openRecent(path: string): Promise<void> {
    const r = this.stored.find((x) => sameFolder(x.path, path));
    if (!r) throw new HomeRefusal('not_recent', 'only a folder of the recent list can be opened by path; choose others with the folder dialog');
    await this.open(r.path);
  }

  private fail(f: OpenFailure): void {
    this.stopWatch();
    this.failure = f;
  }

  /**
   * F-813: a folder the engine refused as no project. Says what it holds (a count of names) and, when it has no project
   * file at all, checks every FIRST_WATCH_MS whether one has appeared (made by Claude Code, the CLI, another program or
   * by hand: the Studio cannot make one, the engine has no command for it) and then opens the folder. Nothing is written.
   */
  private async lookAt(folder: string, seq: number): Promise<void> {
    const entries = await countEntries(folder);
    const ms = this.deps.firstWatchMs ?? FIRST_WATCH_MS;
    const watching = ms > 0 && !existsSync(path.join(folder, PROJECT_FILE));
    const f = this.failure as OpenFailure | null;
    if (seq !== this.openSeq || f === null || f.folder !== folder) return;
    this.failure = { ...f, folderState: { entries, watching } };
    if (!watching) return;
    const timer = setInterval(() => {
      if (!existsSync(path.join(folder, PROJECT_FILE))) return;
      this.stopWatch();
      if (this.opening === null && (this.failure as OpenFailure | null)?.folder === folder) void this.open(folder);
    }, ms);
    timer.unref?.();
    this.watch = { folder, timer };
  }

  private stopWatch(): void {
    if (this.watch) clearInterval(this.watch.timer);
    this.watch = null;
  }

  /** Opens `folder`; the project open before stays open until the new one is open, and stays open if it fails. */
  async open(folder: string): Promise<void> {
    const seq = ++this.openSeq;
    const engine = this.deps.engine;
    if (!engine.configured) {
      this.fail({ kind: 'not_configured', folder, message: engine.reason, detail: null });
      this.emit();
      return;
    }
    if (!(await (this.deps.isDirectory ?? isDir)(folder))) {
      this.fail({
        kind: 'not_found',
        folder,
        message: `The folder ${folder} is not there. It may have been moved, renamed or deleted: open it where it is now, or forget it.`,
        detail: null,
      });
      await this.updateRecentView();
      this.emit();
      return;
    }
    this.opening = folder;
    this.stopWatch();
    this.failure = null;
    this.emit();
    // F-832: the tests' delay before the engine is asked (none outside them); a cancel during it starts nothing
    await testDelay('projectOpen');
    if (seq !== this.openSeq) return;
    const start = this.deps.startServer ?? ((exe: string, f: string) => ProjectServer.start(exe, f));
    let session: ProjectSession | null = null;
    try {
      session = await ProjectSession.open(
        folder,
        engine.path,
        {
          startServer: (f) => start(engine.path, f),
          listFiles: this.deps.listFiles ?? filesUnder,
          now: this.now,
          ...(this.deps.pollMs ? { pollMs: this.deps.pollMs } : {}),
          control: this.controlDeps(engine.path),
        },
        (view) => this.onView(session, view),
      );
    } catch (e) {
      if (seq === this.openSeq) {
        this.opening = null;
        const f = openFailure(folder, e);
        this.fail(f);
        if (f.kind === 'not_a_project') await this.lookAt(folder, seq);
        this.emit();
      }
      return;
    }
    if (seq !== this.openSeq) {
      // a later open overtook this one
      await session.close();
      return;
    }
    const before = this.session;
    this.session = session;
    this.opening = null;
    this.failure = null;
    await this.setRecent((l) => withOpened(l, folder, this.now()));
    this.emit();
    if (before) await before.close();
  }

  /**
   * F-832 (G21): cancels the opening that is going. The project open before stays open and the recent list is not
   * touched; the opening, overtaken, closes its session (and so its project server) when the engine answers. The
   * engine has no way to stop a project server's start half way, so that one finishes and is then stopped.
   */
  cancelOpen(): boolean {
    if (this.opening === null) return false;
    this.openSeq++;
    this.opening = null;
    this.emit();
    return true;
  }

  /** F-807: how run control reaches the engine for a project read by the project server at `exe`. */
  private controlDeps(exe: string): ControlDeps {
    const startControl = this.deps.startControlServer ?? ((e: string, f: string) => ProjectServer.start(e, f, START_TIMEOUT_MS, { readOnly: false }));
    // the runner beside the project server, as the engine's run tools find it (docs/mcp-tools.md); never from PATH (D3)
    const runner = path.join(path.dirname(exe), RUNNER_EXE);
    const ensure =
      this.deps.ensureRunner ??
      ((folder: string) =>
        existsSync(runner)
          ? ensureRunnerCli(runner, folder)
          : Promise.resolve({ ok: false as const, message: `there is no ${RUNNER_EXE} beside ${exe}; build it with cargo build --release -p rcfd-runner` }));
    return { startControlServer: (f) => startControl(exe, f), ensureRunner: ensure, now: this.now };
  }

  /** F-807: a run-control action on the open project. */
  async action(id: string, params: Readonly<Record<string, unknown>>): Promise<RunActionResult> {
    const s = this.session;
    if (!s) {
      return {
        ok: false,
        request: null,
        failure: { code: 'no_project', problem: 'No project is open, so there is no run to control.', recovery: 'Open a project folder first (Open project folder…).', engine: null },
      };
    }
    return s.dispatch(id, params);
  }

  private onView(session: ProjectSession | null, _view: ProjectView): void {
    if (session !== null && session === this.session) this.emit();
  }

  async forget(path: string): Promise<void> {
    await this.setRecent((l) => without(l, path));
    if (this.failure && sameFolder(this.failure.folder, path)) {
      this.stopWatch();
      this.failure = null;
    }
    this.emit();
  }

  async close(): Promise<void> {
    const s = this.session;
    this.session = null;
    this.emit();
    if (s) await s.close();
  }

  /** Reads the open project again; when its server stopped, starts a new one. */
  async refresh(): Promise<void> {
    const s = this.session;
    if (!s) return;
    try {
      if (s.serverStopped) await s.reconnect();
      else await s.refresh();
    } catch (e) {
      this.fail(openFailure(s.folder, e));
      this.emit();
    }
  }

  /** F-810: one read-only query of the open project's server (the run workspace); refused when no project is open. */
  query(tool: ProjectTool, args: Record<string, unknown>): Promise<unknown> {
    const s = this.session;
    if (!s) return Promise.reject(new Error('no project is open'));
    return s.query(tool, args);
  }

  dismiss(): void {
    this.stopWatch();
    this.failure = null;
    this.emit();
  }

  async shutdown(): Promise<void> {
    this.stopWatch();
    this.openSeq++;
    const s = this.session;
    this.session = null;
    if (s) await s.close();
  }
}

/** A failed open in words: the problem and the next step (G6, G19). */
export function openFailure(folder: string, e: unknown): OpenFailure {
  if (e instanceof StartFailure && e.kind === 'not_a_project') {
    return {
      kind: 'not_a_project',
      folder,
      message: `${folder} is not an rcfd project: the engine found no readable rcfd-project.json in it. Choose the project's top folder, the one that holds rcfd-project.json.`,
      detail: e.detail,
    };
  }
  if (e instanceof StartFailure) return { kind: 'engine', folder, message: e.message, detail: e.detail };
  return { kind: 'engine', folder, message: `The engine could not read ${folder}: ${e instanceof Error ? e.message : String(e)}`, detail: null };
}
