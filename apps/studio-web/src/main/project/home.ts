// The project home of the main process: the engine's configuration, the recent list, the folder being opened, the last
// failed open and the open project's session. The renderer reaches it only through the project port (bridge.ts), and
// gets the whole state after every change.
//
// No Electron import: the folder dialog is passed in, so the unit tests drive the home with fakes.
import { stat } from 'node:fs/promises';
import type { EngineStatus, OpenFailure, PickResult, ProjectState, ProjectTool, ProjectView } from '../../shared/project.js';
import { filesUnder, type Names } from './listing.js';
import { sameFolder, withExistence, withOpened, without, type StoredRecent } from './recent.js';
import { ProjectServer, StartFailure, type ProjectServerLike } from './server.js';
import { ProjectSession } from './session.js';

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
}

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

  private async setRecent(list: StoredRecent[]): Promise<void> {
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
    this.failure = f;
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
    this.failure = null;
    this.emit();
    const start = this.deps.startServer ?? ((exe: string, f: string) => ProjectServer.start(exe, f));
    let session: ProjectSession | null = null;
    try {
      session = await ProjectSession.open(
        folder,
        engine.path,
        { startServer: (f) => start(engine.path, f), listFiles: this.deps.listFiles ?? filesUnder, now: this.now, ...(this.deps.pollMs ? { pollMs: this.deps.pollMs } : {}) },
        (view) => this.onView(session, view),
      );
    } catch (e) {
      if (seq === this.openSeq) {
        this.opening = null;
        this.fail(openFailure(folder, e));
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
    await this.setRecent(withOpened(this.stored, folder, this.now()));
    this.emit();
    if (before) await before.close();
  }

  private onView(session: ProjectSession | null, _view: ProjectView): void {
    if (session !== null && session === this.session) this.emit();
  }

  async forget(path: string): Promise<void> {
    await this.setRecent(without(this.stored, path));
    if (this.failure && sameFolder(this.failure.folder, path)) this.failure = null;
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
    this.failure = null;
    this.emit();
  }

  async shutdown(): Promise<void> {
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
