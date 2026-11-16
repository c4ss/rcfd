// The case editor of the main process (F-812; src/shared/case.ts says how a case is opened, saved and created). It
// binds service sessions to case files of the open project and writes them back through the engine's project server;
// it never reads or writes the case text itself (G2), and it never writes over a file another writer changed since it
// was read (G18): the engine's `edit_case` compares the hash under the project's edit lock.
//
// No Electron import: the unit tests drive it with a fake service and a fake project server.
import path from 'node:path';
import {
  DUPLICATE_REASON,
  NEW_REASON,
  SAVE_REASON,
  type CaseBinding,
  type CaseDoorState,
  type CaseError,
  type CaseSources,
  type CaseStatus,
  type ConflictRow,
  type ConflictView,
  type CreatedCase,
  type Diagnostic,
  type FileCheck,
  type FileNow,
  type NewCaseSource,
  type PreparedCase,
  type SaveOutcome,
} from '../../shared/case.js';
import type { EngineStatus } from '../../shared/project.js';
import { RpcError } from '../service/rpc.js';
import { journalEntries, pendingChanges, realRemovals, type JournalEntry } from './plan.js';
import { CaseServer, type ToolServerLike } from './server.js';

/** Engine error codes the editor tells apart (docs/mcp-tools.md, docs/service-protocol.md). */
export const CODE = { UNKNOWN_SESSION: 1001, INVALID: 1003, REFUSED: 1004, CONFLICT: 1200, NOT_FOUND: 1202 } as const;

/** A request the editor refuses before or after asking the engine; bridge.ts turns it into the reply's error. */
export class CaseRefusal extends Error {
  constructor(
    readonly code: CaseError['code'],
    message: string,
    readonly data?: unknown,
  ) {
    super(message);
  }
}

export interface ServiceLike {
  call(method: string, params?: Record<string, unknown>, timeoutMs?: number): Promise<unknown>;
}

export interface EditorDeps {
  /** The open project's folder, or null. */
  project(): { readonly folder: string } | null;
  /** Where the engine's project server is (src/main/project/config.ts). */
  readonly engine: EngineStatus;
  readonly service: ServiceLike;
  /** Starts the project server for `folder` without `--read-only` (the default: ProjectServer.start). */
  startServer(exe: string, folder: string): Promise<ToolServerLike>;
  now?(): string;
}

interface Bound extends CaseBinding {
  /** Journal entries of the session at the open or the last save: the changes after them are unsaved. */
  readonly savedIndex: number;
}

const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));
const codeOf = (e: unknown): RpcError['code'] | null => (e instanceof RpcError ? e.code : null);

/** True when two absolute paths name the same file (letter case ignored on Windows, as the file system does). */
export function samePath(a: string, b: string, platform: NodeJS.Platform = process.platform): boolean {
  const ra = path.resolve(a);
  const rb = path.resolve(b);
  return platform === 'win32' ? ra.toLowerCase() === rb.toLowerCase() : ra === rb;
}

/**
 * A case file's project-relative path, `/`-separated: `cases/<...>.toml` or `.json`, with no empty, `.` or `..` part;
 * null for anything else. The engine checks the path again (no links, inside the project).
 */
export function caseRelPath(p: string): string | null {
  const parts = p.replace(/\\/g, '/').split('/');
  if (parts.length < 2 || parts[0] !== 'cases') return null;
  if (parts.some((x) => x === '' || x === '.' || x === '..' || x.includes(':'))) return null;
  if (!/\.(toml|json)$/i.test(parts[parts.length - 1]!)) return null;
  return parts.join('/');
}

function diagnostics(data: unknown): Diagnostic[] {
  const d = (data as { diagnostics?: unknown } | null)?.diagnostics;
  if (!Array.isArray(d)) return [];
  return d.map((x: { path?: unknown; line?: unknown; message?: unknown }) => ({
    path: typeof x?.path === 'string' ? x.path : '',
    line: typeof x?.line === 'number' ? x.line : null,
    message: typeof x?.message === 'string' ? x.message : String(x?.message ?? ''),
  }));
}

/** An engine failure as the port's error: the engine's code and message unchanged. */
function engineError(e: unknown): CaseRefusal {
  if (e instanceof CaseRefusal) return e;
  if (e instanceof RpcError) return new CaseRefusal(typeof e.code === 'number' ? e.code : e.code === 'timeout' ? 'timeout' : 'engine', e.message, e.data);
  return new CaseRefusal('engine', message(e));
}

export class CaseEditor {
  private readonly bindings = new Map<string, Bound>();
  private server: { folder: string; started: Promise<CaseServer> } | null = null;
  private sourcesOf: { server: Promise<CaseServer>; sources: CaseSources } | null = null;
  /** One operation at a time per session (a save and a status must not interleave). */
  private readonly queues = new Map<string, Promise<unknown>>();
  private readonly now: () => string;

  constructor(private readonly deps: EditorDeps) {
    this.now = deps.now ?? (() => new Date().toISOString());
  }

  state(): CaseDoorState {
    const p = this.deps.project();
    return {
      project: p ? { folder: p.folder } : null,
      engine: this.deps.engine.configured ? { available: true } : { available: false, reason: this.deps.engine.reason },
      bindings: [...this.bindings.values()].map(view),
    };
  }

  /** Called when the open project may have changed: the project server of a folder no longer open is stopped. */
  projectChanged(): void {
    const p = this.deps.project();
    if (this.server && (!p || !samePath(p.folder, this.server.folder))) void this.stopServer();
  }

  async shutdown(): Promise<void> {
    await this.stopServer();
  }

  private async stopServer(): Promise<void> {
    const s = this.server;
    this.server = null;
    this.sourcesOf = null;
    if (!s) return;
    try {
      await (await s.started).close();
    } catch {
      // a server that never started has nothing to stop
    }
  }

  /** The open project's folder, or a refusal that says what to do. */
  private openFolder(): string {
    const p = this.deps.project();
    if (!p) throw new CaseRefusal('no_project', 'No project is open. Open the project folder from the project home first.');
    return p.folder;
  }

  /** The project server of the open project, started on first use (one per open project). */
  private async caseServer(): Promise<CaseServer> {
    const folder = this.openFolder();
    const engine = this.deps.engine;
    if (!engine.configured) throw new CaseRefusal('not_configured', engine.reason);
    if (this.server && samePath(this.server.folder, folder)) {
      try {
        const s = await this.server.started;
        if (!s.exited) return s;
      } catch {
        // start again below
      }
    }
    await this.stopServer();
    const started = this.deps.startServer(engine.path, folder).then((s) => new CaseServer(s));
    this.server = { folder, started };
    try {
      return await started;
    } catch (e) {
      if (this.server?.started === started) this.server = null;
      throw new CaseRefusal('engine', `The engine's project server could not be started for ${folder}: ${message(e)}`);
    }
  }

  private serial<T>(key: string, fn: () => Promise<T>): Promise<T> {
    const before = this.queues.get(key) ?? Promise.resolve();
    const run = before.then(fn, fn);
    const tail = run.catch(() => undefined);
    this.queues.set(key, tail);
    void tail.then(() => {
      if (this.queues.get(key) === tail) this.queues.delete(key);
    });
    return run;
  }

  private rel(p: string): string {
    const rel = caseRelPath(p);
    if (rel === null) throw new CaseRefusal('bad_request', `${p}: a case file is cases/<name>.toml or cases/<name>.json inside the project`);
    return rel;
  }

  /** The file's hash as the engine reads it now (`read_project_file`, one byte of text). */
  private async hashOf(server: CaseServer, rel: string): Promise<{ path: string; hash: string; size: number }> {
    const r = (await server.call('read_project_file', { path: rel, limit: 1 })) as { path?: unknown; hash?: unknown; size?: unknown };
    if (typeof r?.hash !== 'string') throw new CaseRefusal('engine', `read_project_file ${rel}: the reply has no hash`);
    return { path: typeof r.path === 'string' ? r.path : rel, hash: r.hash, size: typeof r.size === 'number' ? r.size : 0 };
  }

  private async journal(session: string): Promise<JournalEntry[]> {
    try {
      return journalEntries(await this.deps.service.call('journal.get', { session }));
    } catch (e) {
      if (codeOf(e) === CODE.UNKNOWN_SESSION) {
        this.bindings.delete(session);
        throw new CaseRefusal('not_bound', `Session ${session} is no longer on the service (it was closed, or the service started again). Open the case again from the project home.`);
      }
      throw engineError(e);
    }
  }

  private bound(session: string): Bound {
    const b = this.bindings.get(session);
    if (!b) throw new CaseRefusal('not_bound', `Session ${session} is not open on a case file. Open a case from the project home.`);
    return b;
  }

  /** The binding's journal, checked to be the session made from its file (a restarted service reuses session ids). */
  private async boundJournal(session: string): Promise<{ b: Bound; entries: JournalEntry[] }> {
    const b = this.bound(session);
    const entries = await this.journal(session);
    if (!madeFrom(entries, b.file)) {
      this.bindings.delete(session);
      throw new CaseRefusal('not_bound', `Session ${session} on the service now is not the one opened from ${b.path} (the service started again). Open the case again.`);
    }
    return { b, entries };
  }

  // ---- operations of the port

  prepare(p: string): Promise<PreparedCase> {
    return (async () => {
      const rel = this.rel(p);
      const folder = this.openFolder();
      const server = await this.caseServer();
      try {
        const h = await this.hashOf(server, rel);
        return { path: h.path, file: path.join(folder, ...h.path.split('/')), hash: h.hash, size: h.size };
      } catch (e) {
        throw engineError(e);
      }
    })();
  }

  attach(session: string, p: string, hash: string): Promise<CaseBinding> {
    return this.serial(session, async () => {
      const rel = this.rel(p);
      const folder = this.openFolder();
      const file = path.join(folder, ...rel.split('/'));
      const entries = await this.journal(session);
      if (!madeFrom(entries, file)) {
        throw new CaseRefusal('not_from_file', `Session ${session} was not made from ${rel} (its journal names another source).`);
      }
      const server = await this.caseServer();
      let now: string;
      try {
        now = (await this.hashOf(server, rel)).hash;
      } catch (e) {
        throw engineError(e);
      }
      if (now !== hash) {
        throw new CaseRefusal('changed', `${rel} changed while it was being opened (now ${now}). Open it again to read it as it is now.`, { current_hash: now });
      }
      const b: Bound = { session, folder, path: rel, file, hash, openedAt: this.now(), savedAt: null, savedIndex: entries.length };
      this.bindings.set(session, b);
      return view(b);
    });
  }

  forget(session: string): void {
    this.bindings.delete(session);
  }

  status(session: string): Promise<CaseStatus> {
    return this.serial(session, async () => {
      const { b, entries } = await this.boundJournal(session);
      const pending = pendingChanges(entries, b.savedIndex);
      const p = this.deps.project();
      const projectOpen = p !== null && samePath(p.folder, b.folder);
      let file: FileNow;
      if (!projectOpen) {
        file = { state: 'unknown', message: `The project ${b.folder} is not open; open it again to read or save ${b.path}.` };
      } else {
        try {
          const h = await this.hashOf(await this.caseServer(), b.path);
          file = { state: h.hash === b.hash ? 'same' : 'changed', hash: h.hash };
        } catch (e) {
          const r = engineError(e);
          file = r.code === CODE.NOT_FOUND ? { state: 'missing', message: r.message } : { state: 'unknown', message: r.message };
        }
      }
      return { binding: view(b), pending, file, projectOpen };
    });
  }

  /** A session of the file as it is now, for reading its values; closed by `done`. */
  private async fileSession<T>(file: string, use: (s: string | null, error: string | null) => Promise<T>): Promise<T> {
    let tmp: string | null = null;
    let error: string | null = null;
    try {
      const r = (await this.deps.service.call('session.create', { path: file })) as { session?: unknown };
      tmp = typeof r?.session === 'string' ? r.session : null;
    } catch (e) {
      const d = e instanceof RpcError ? diagnostics(e.data) : [];
      error = d.length ? d.map((x) => `${x.path}: ${x.message}`).join('; ') : message(e);
    }
    try {
      return await use(tmp, error);
    } finally {
      if (tmp !== null) await this.deps.service.call('session.close', { session: tmp }).catch(() => undefined);
    }
  }

  private async valueIn(session: string, p: string): Promise<{ found: true; value: unknown } | { found: false }> {
    try {
      const r = (await this.deps.service.call('settings.get', { session, path: p })) as { value?: unknown };
      return { found: true, value: r?.value ?? null };
    } catch (e) {
      if (codeOf(e) === CODE.INVALID || codeOf(e) === -32602) return { found: false };
      throw engineError(e);
    }
  }

  conflict(session: string): Promise<ConflictView> {
    return this.serial(session, async () => {
      const { b, entries } = await this.boundJournal(session);
      this.requireBindingProject(b);
      const server = await this.caseServer();
      let current: string | null;
      try {
        current = (await this.hashOf(server, b.path)).hash;
      } catch (e) {
        const r = engineError(e);
        if (r.code !== CODE.NOT_FOUND) throw r;
        current = null;
      }
      if (current === b.hash) return { changed: false };
      const pending = pendingChanges(entries, b.savedIndex);
      const mine = async (p: string) => (await this.valueIn(session, p)) as { found: boolean; value?: unknown };
      if (current === null) {
        const rows: ConflictRow[] = [];
        for (const p of pending.paths) rows.push({ path: p, mine: (await mine(p)).value ?? null, file: null, inFile: false });
        return { changed: true, currentHash: null, rows, removals: pending.removals, fileError: `${b.path} is no longer in the project.` };
      }
      return this.fileSession(b.file, async (tmp, error) => {
        const rows: ConflictRow[] = [];
        for (const p of pending.paths) {
          const m = await mine(p);
          const f = tmp === null ? { found: false as const } : await this.valueIn(tmp, p);
          rows.push({ path: p, mine: m.value ?? null, file: f.found ? f.value : null, inFile: f.found });
        }
        return { changed: true, currentHash: current, rows, removals: pending.removals, fileError: error === null ? null : `The engine does not read ${b.path} as a case now: ${error}` };
      });
    });
  }

  private requireBindingProject(b: Bound): void {
    const folder = this.openFolder();
    if (!samePath(folder, b.folder)) {
      throw new CaseRefusal('no_project', `${b.path} belongs to the project ${b.folder}, which is not open. Open that project again to save it.`);
    }
  }

  save(session: string, onto: 'opened' | 'current'): Promise<SaveOutcome> {
    return this.serial(session, async () => {
      const { b, entries } = await this.boundJournal(session);
      this.requireBindingProject(b);
      const server = await this.caseServer();
      const pending = pendingChanges(entries, b.savedIndex);
      if (pending.count === 0) return { kind: 'nothing' };
      let base = b.hash;
      if (onto === 'current') {
        try {
          base = (await this.hashOf(server, b.path)).hash;
        } catch (e) {
          throw engineError(e);
        }
      }
      if (pending.removals.length > 0) {
        const real = await this.fileSession(b.file, (tmp, error) => {
          if (tmp === null) throw new CaseRefusal('engine', `The engine does not read ${b.path} as a case now: ${error ?? 'no session'}`);
          return realRemovals(pending.removals, async (p) => (await this.valueIn(tmp, p)).found);
        });
        if (real.length > 0) {
          return {
            kind: 'cannot',
            removals: real,
            message: `${b.path} holds ${real.join(', ')}, which this session removed. The engine's edit of a case file (edit_case) sets settings but cannot remove an entry, so nothing was written. Remove the entry's table from the file in a text editor (or ask Claude Code), then open the case again; or add the entry back here and save.`,
          };
        }
      }
      if (pending.paths.length === 0) {
        // only entries added and removed again in this session: the file already holds what the session holds
        this.bindings.set(session, { ...b, savedIndex: entries.length, savedAt: this.now() });
        return { kind: 'nothing' };
      }
      const set: Record<string, unknown> = {};
      for (const p of pending.paths) {
        const v = await this.valueIn(session, p);
        if (!v.found) throw new CaseRefusal('engine', `The session holds no setting ${p} any more; open the case again.`);
        set[p] = v.value;
      }
      let r: { path?: unknown; old_hash?: unknown; new_hash?: unknown; form?: unknown; values?: unknown; mesh?: { ok?: unknown } };
      try {
        r = (await server.call('edit_case', { path: b.path, base_hash: base, set, reason: SAVE_REASON })) as typeof r;
      } catch (e) {
        const x = engineError(e);
        if (x.code === CODE.CONFLICT) {
          const cur = (x.data as { current_hash?: unknown } | undefined)?.current_hash;
          return { kind: 'conflict', currentHash: typeof cur === 'string' ? cur : null, message: x.message };
        }
        if (x.code === CODE.INVALID) return { kind: 'refused', message: x.message, diagnostics: diagnostics(x.data) };
        throw x;
      }
      const newHash = typeof r.new_hash === 'string' ? r.new_hash : '';
      this.bindings.set(session, { ...b, hash: newHash, savedIndex: entries.length, savedAt: this.now() });
      return {
        kind: 'saved',
        path: typeof r.path === 'string' ? r.path : b.path,
        oldHash: typeof r.old_hash === 'string' ? r.old_hash : base,
        newHash,
        form: typeof r.form === 'string' ? r.form : '',
        values: (r.values ?? {}) as Record<string, unknown>,
        mesh: r.mesh && typeof r.mesh === 'object' ? { ok: r.mesh.ok === true } : null,
        reopen: base !== b.hash,
      };
    });
  }

  /** The engine's templates and shipped examples: the enumerations of `new_case` in the server's `tools/list`. */
  async sources(): Promise<CaseSources> {
    const server = await this.caseServer();
    const started = this.server?.started;
    if (this.sourcesOf && started && this.sourcesOf.server === started) return this.sourcesOf.sources;
    let tools: readonly unknown[];
    try {
      tools = await server.listTools();
    } catch (e) {
      throw engineError(e);
    }
    const nc = tools.find((t) => (t as { name?: unknown })?.name === 'new_case') as { inputSchema?: { properties?: Record<string, { enum?: unknown; description?: unknown }> } } | undefined;
    const props = nc?.inputSchema?.properties ?? {};
    const list = (k: string) => (Array.isArray(props[k]?.enum) ? (props[k]!.enum as unknown[]).filter((x): x is string => typeof x === 'string') : []);
    const help = (k: string) => (typeof props[k]?.description === 'string' ? (props[k]!.description as string) : '');
    const sources: CaseSources = {
      templates: list('template'),
      examples: list('example'),
      templateHelp: help('template'),
      exampleHelp: help('example'),
      nameHelp: help('name'),
    };
    if (started) this.sourcesOf = { server: started, sources };
    return sources;
  }

  async create(name: string, source: NewCaseSource): Promise<CreatedCase> {
    const s = await this.sources();
    const offered = source.kind === 'template' ? s.templates : s.examples;
    if (!offered.includes(source.name)) {
      throw new CaseRefusal('bad_request', `${source.name} is not a ${source.kind === 'template' ? 'template' : 'shipped example'} the engine offers.`);
    }
    return this.newCase({ name, [source.kind]: source.name, reason: NEW_REASON });
  }

  async duplicate(p: string, name: string): Promise<CreatedCase> {
    const rel = this.rel(p);
    return this.newCase({ name, copy: rel, reason: DUPLICATE_REASON });
  }

  private async newCase(args: Record<string, unknown>): Promise<CreatedCase> {
    const server = await this.caseServer();
    try {
      const r = (await server.call('new_case', args)) as { path?: unknown; hash?: unknown; from?: unknown; data_files?: unknown };
      const files = Array.isArray(r?.data_files) ? (r.data_files as { path?: unknown }[]).map((f) => String(f?.path ?? '')).filter((x) => x !== '') : [];
      return { path: String(r?.path ?? ''), hash: String(r?.hash ?? ''), from: String(r?.from ?? ''), dataFiles: files };
    } catch (e) {
      throw engineError(e);
    }
  }

  async check(p: string): Promise<FileCheck> {
    const rel = this.rel(p);
    const server = await this.caseServer();
    let r: { ok?: unknown; exit_code?: unknown; event?: { diagnostics?: unknown; cells?: unknown; mesh?: { errors?: unknown; warnings?: unknown } } };
    try {
      r = (await server.call('check_case_headless', { path: rel })) as typeof r;
    } catch (e) {
      throw engineError(e);
    }
    const ev = r?.event ?? {};
    const strings = (v: unknown) => (Array.isArray(v) ? v.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))) : []);
    return {
      path: rel,
      ok: r?.ok === true,
      exitCode: typeof r?.exit_code === 'number' ? r.exit_code : null,
      diagnostics: diagnostics(ev),
      cells: typeof ev.cells === 'number' ? ev.cells : null,
      meshErrors: strings(ev.mesh?.errors),
      meshWarnings: strings(ev.mesh?.warnings),
    };
  }
}

/** True when the journal's create entry names `file` as the session's source. */
function madeFrom(entries: readonly JournalEntry[], file: string): boolean {
  const first = entries[0];
  const src = first?.op === 'create' ? first.source?.path : undefined;
  return typeof src === 'string' && samePath(src, file);
}

const view = (b: Bound): CaseBinding => ({
  session: b.session,
  folder: b.folder,
  path: b.path,
  file: b.file,
  hash: b.hash,
  openedAt: b.openedAt,
  savedAt: b.savedAt,
});
