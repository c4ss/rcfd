// Unit tests of the project home's main-process parts (src/main/project/, F-805): where the engine comes from, the
// recent list, the case and note names, the port's checks, and a session and the home driven by a fake project server.
// The home's own stylesheet is checked like the others of tests/unit/tokens.spec.ts.
import { expect, test } from '@playwright/test';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { readdirSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { MessageChannel, type MessagePort } from 'node:worker_threads';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { MAX_CHANGES, type ProjectPortMessage, type ProjectState, type ProjectTool } from '../../src/shared/project.js';
import { parseProjectRequest, ProjectPortBridge, type ProjectPortLike } from '../../src/main/project/bridge.js';
import { bundledBin, engineConfig, recentFile } from '../../src/main/project/config.js';
import { HomeRefusal, ProjectHome, type RecentLike } from '../../src/main/project/home.js';
import { filesUnder, isTempName } from '../../src/main/project/listing.js';
import { formatRecent, MAX_RECENT, parseRecent, RecentStore, sameFolder, withOpened, without, type StoredRecent } from '../../src/main/project/recent.js';
import { StartFailure, toolResult, type ProjectServerLike } from '../../src/main/project/server.js';
import { ProjectSession, readLists } from '../../src/main/project/session.js';
import { RpcError } from '../../src/main/service/rpc.js';
import { until } from '../support/fake-service.js';
import { appRoot } from '../support/rcfd.js';

const abs = (p: string) => path.resolve(p);

test.describe('engineConfig', () => {
  const files = new Set([abs('/e/rcfd-mcp.exe'), path.join(abs('/b'), 'rcfd-mcp.exe')]);
  const isFile = (p: string) => files.has(p);
  test('RCFD_STUDIO_MCP first, then the rcfd-mcp beside RCFD_STUDIO_RCFD, and nothing from PATH', () => {
    expect(engineConfig({ RCFD_STUDIO_MCP: abs('/e/rcfd-mcp.exe'), RCFD_STUDIO_RCFD: abs('/b/rcfd.exe') }, isFile, 'rcfd-mcp.exe')).toEqual({
      configured: true,
      path: abs('/e/rcfd-mcp.exe'),
      from: 'RCFD_STUDIO_MCP',
    });
    expect(engineConfig({ RCFD_STUDIO_RCFD: abs('/b/rcfd.exe') }, isFile, 'rcfd-mcp.exe')).toEqual({
      configured: true,
      path: path.join(abs('/b'), 'rcfd-mcp.exe'),
      from: 'RCFD_STUDIO_RCFD',
    });
    const none = engineConfig({ PATH: abs('/e') }, isFile, 'rcfd-mcp.exe');
    expect(none.configured).toBe(false);
    if (!none.configured) expect(none.reason).toContain('RCFD_STUDIO_MCP');
  });
  test('a name that is not a file is not configured, and says which variable and how to build it', () => {
    const r = engineConfig({ RCFD_STUDIO_MCP: abs('/nope/rcfd-mcp.exe') }, isFile, 'rcfd-mcp.exe');
    expect(r.configured).toBe(false);
    if (!r.configured) expect(r.reason).toMatch(/RCFD_STUDIO_MCP names .* not a file .*cargo build --release -p rcfd-mcp/);
    const b = engineConfig({ RCFD_STUDIO_RCFD: abs('/c/rcfd.exe') }, isFile, 'rcfd-mcp.exe');
    expect(b.configured).toBe(false);
  });
  test('F-820: an installed Studio uses the rcfd-mcp it ships with when no variable names one; never PATH', () => {
    const shipped = new Set([...files, path.join(abs('/res/bin'), 'rcfd-mcp.exe')]);
    const has = (p: string) => shipped.has(p);
    expect(engineConfig({}, has, 'rcfd-mcp.exe', abs('/res/bin'))).toEqual({ configured: true, path: path.join(abs('/res/bin'), 'rcfd-mcp.exe'), from: 'bundled' });
    // the variables win over it
    expect(engineConfig({ RCFD_STUDIO_MCP: abs('/e/rcfd-mcp.exe') }, has, 'rcfd-mcp.exe', abs('/res/bin'))).toMatchObject({ from: 'RCFD_STUDIO_MCP' });
    expect(engineConfig({ RCFD_STUDIO_RCFD: abs('/b/rcfd.exe') }, has, 'rcfd-mcp.exe', abs('/res/bin'))).toMatchObject({ from: 'RCFD_STUDIO_RCFD' });
    // a variable that names nothing is an error, not a reason to fall back
    expect(engineConfig({ RCFD_STUDIO_MCP: abs('/nope/rcfd-mcp.exe') }, has, 'rcfd-mcp.exe', abs('/res/bin')).configured).toBe(false);
    // packaged without a bin folder (out/packaged): not configured, as before
    expect(engineConfig({ PATH: abs('/e') }, has, 'rcfd-mcp.exe', abs('/other/bin')).configured).toBe(false);
  });
  test('F-820: the shipped engine is <resources>/bin when packaged; RCFD_TEST_BUNDLED_BIN only in test mode', () => {
    expect(bundledBin({}, true, abs('/app/resources'))).toBe(path.join(abs('/app/resources'), 'bin'));
    expect(bundledBin({}, false, abs('/app/resources'))).toBeNull();
    expect(bundledBin({ RCFD_TEST_BUNDLED_BIN: abs('/t/bin') }, true, abs('/app/resources'))).toBe(path.join(abs('/app/resources'), 'bin'));
    expect(bundledBin({ RCFD_TEST_HEADLESS: '1', RCFD_TEST_BUNDLED_BIN: abs('/t/bin') }, false, '')).toBe(abs('/t/bin'));
  });
  test('the recent list lives in the user data unless RCFD_STUDIO_RECENT names a file', () => {
    expect(recentFile({}, abs('/u'))).toBe(path.join(abs('/u'), 'recent-projects.json'));
    expect(recentFile({ RCFD_STUDIO_RECENT: abs('/x/r.json') }, abs('/u'))).toBe(abs('/x/r.json'));
  });
});

test.describe('the recent list', () => {
  test('holds paths and times only, latest first, at most MAX_RECENT, one entry per folder', () => {
    let list: StoredRecent[] = [];
    for (let i = 0; i < MAX_RECENT + 3; i++) list = withOpened(list, abs(`/p/${i}`), `2026-11-16T10:00:${String(i).padStart(2, '0')}Z`);
    expect(list).toHaveLength(MAX_RECENT);
    expect(list[0]!.path).toBe(abs(`/p/${MAX_RECENT + 2}`));
    list = withOpened(list, abs('/p/5'), 'later');
    expect(list[0]).toEqual({ path: abs('/p/5'), opened: 'later' });
    expect(list.filter((r) => r.path === abs('/p/5'))).toHaveLength(1);
    expect(without(list, abs('/p/5')).some((r) => r.path === abs('/p/5'))).toBe(false);
    const text = formatRecent(list);
    const parsed = JSON.parse(text) as { format: string; version: number; projects: object[] };
    expect(Object.keys(parsed).sort()).toEqual(['format', 'projects', 'version']);
    for (const p of parsed.projects) expect(Object.keys(p).sort()).toEqual(['opened', 'path']);
    expect(parseRecent(text)).toEqual(list);
  });
  test('a file of another shape, relative paths and junk are dropped rather than trusted', () => {
    expect(parseRecent('not json')).toEqual([]);
    expect(parseRecent(JSON.stringify({ format: 'other', projects: [] }))).toEqual([]);
    expect(
      parseRecent(JSON.stringify({ format: 'rcfd-studio-recent', version: 1, projects: [{ path: 'rel/x', opened: 't' }, { path: abs('/a'), opened: 1 }, 7, { path: abs('/b'), opened: 't' }] })),
    ).toEqual([{ path: abs('/b'), opened: 't' }]);
  });
  test('same folder ignores a trailing separator, and letter case on Windows and macOS', () => {
    expect(sameFolder(abs('/a/B/'), abs('/a/B'), 'linux')).toBe(true);
    expect(sameFolder(abs('/a/b'), abs('/a/B'), 'linux')).toBe(false);
    expect(sameFolder(abs('/a/b'), abs('/a/B'), 'win32')).toBe(true);
  });
  test('the store writes the whole list atomically and reads it back; a missing file is an empty list', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-recent-'));
    try {
      const store = new RecentStore(path.join(dir, 'sub', 'recent.json'));
      expect(await store.load()).toEqual([]);
      const list = withOpened([], abs('/q'), 't1');
      await store.save(list);
      expect(await store.load()).toEqual(list);
      expect(await readFile(path.join(dir, 'sub', 'recent.json'), 'utf8')).toBe(formatRecent(list));
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

test.describe('case and note names (decision D2)', () => {
  test('every file below the folder, recursively, sorted, without the temporary files of an atomic write', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-names-'));
    try {
      await mkdir(path.join(dir, 'cases', 'sub'), { recursive: true });
      await writeFile(path.join(dir, 'cases', 'b.toml'), '');
      await writeFile(path.join(dir, 'cases', 'a.json'), '');
      await writeFile(path.join(dir, 'cases', 'sub', 'c.toml'), '');
      await writeFile(path.join(dir, 'cases', '.b.toml.12.0.tmp'), '');
      expect(await filesUnder(dir, 'cases')).toEqual({ names: ['cases/a.json', 'cases/b.toml', 'cases/sub/c.toml'], total: 3 });
      expect(await filesUnder(dir, 'cases', 2)).toEqual({ names: ['cases/a.json', 'cases/b.toml'], total: 3 });
      expect(await filesUnder(dir, 'notes')).toEqual({ names: [], total: 0 });
      expect(isTempName('.x.tmp')).toBe(true);
      expect(isTempName('x.tmp')).toBe(false);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

test.describe('parseProjectRequest', () => {
  test('accepts each operation in its exact shape', () => {
    for (const op of ['state', 'pick', 'close', 'refresh', 'dismiss'] as const) expect(parseProjectRequest({ op, id: 3 })).toEqual({ ok: true, request: { op, id: 3 } });
    expect(parseProjectRequest({ op: 'open', id: 4, path: abs('/p') })).toEqual({ ok: true, request: { op: 'open', id: 4, path: abs('/p') } });
    expect(parseProjectRequest({ op: 'forget', id: 5, path: abs('/p') })).toEqual({ ok: true, request: { op: 'forget', id: 5, path: abs('/p') } });
    // F-835: create takes exactly where, pick or here; the page names no path
    for (const where of ['pick', 'here'] as const) expect(parseProjectRequest({ op: 'create', id: 6, where })).toEqual({ ok: true, request: { op: 'create', id: 6, where } });
  });
  test('F-835: create is refused in any other shape', () => {
    for (const raw of [
      { op: 'create', id: 6 },
      { op: 'create', id: 6, where: 'there' },
      { op: 'create', id: 6, where: 7 },
      { op: 'create', id: 6, where: 'pick', path: abs('/p') },
      { op: 'create', id: 6, path: abs('/p') },
    ]) {
      const r = parseProjectRequest(raw);
      expect(r.ok, JSON.stringify(raw)).toBe(false);
      if (!r.ok) expect(r.error.code).toBe('bad_request');
    }
  });
  test('refuses anything else, with the id when there is one', () => {
    const refused = (raw: unknown) => {
      const r = parseProjectRequest(raw);
      expect(r.ok, JSON.stringify(raw)).toBe(false);
      return r.ok ? null : r;
    };
    expect(refused(null)!.id).toBeNull();
    expect(refused([1])!.id).toBeNull();
    expect(refused({ op: 'state' })!.id).toBeNull();
    expect(refused({ op: 'state', id: -1 })!.id).toBeNull();
    expect(refused({ op: 'state', id: 1, extra: true })!.id).toBe(1);
    expect(refused({ op: 'open', id: 2 })!.error.code).toBe('bad_request');
    expect(refused({ op: 'open', id: 2, path: 7 })).not.toBeNull();
    expect(refused({ op: 'open', id: 2, path: '' })).not.toBeNull();
    expect(refused({ op: 'open', id: 2, path: 'a\0b' })).not.toBeNull();
    expect(refused({ op: 'open', id: 2, path: 'x'.repeat(4097) })).not.toBeNull();
    expect(refused({ op: 'pick', id: 2, path: abs('/p') })).not.toBeNull();
    expect(refused({ op: 'project_info', id: 2 })!.error.message).toContain('unknown op');
    expect(refused({ op: 'read_project_file', id: 2, path: 'x' })).not.toBeNull();
    expect(refused(Object.assign(Object.create({ inherited: 1 }), { op: 'state', id: 1 }))).not.toBeNull();
  });
});

// ---- a fake project server: lists from an object the test changes, a feed the test appends to

interface FakeWorld {
  runs: { id: string; case: string; state: string }[];
  meshes: { path: string; used_by: string[] }[];
  feed: { n: number; source: string; kind: string; text: string; path?: string; run?: string }[];
  failFeed: boolean;
  failMeshes: boolean;
}

class FakeServer implements ProjectServerLike {
  readonly info = { name: 'rcfd', version: '0.0.0-test' };
  exited = false;
  calls: { tool: ProjectTool; args: Record<string, unknown> | undefined }[] = [];
  private exit: ((why: string) => void)[] = [];
  constructor(private readonly w: FakeWorld) {}
  async call(tool: ProjectTool, args?: Record<string, unknown>): Promise<unknown> {
    this.calls.push({ tool, args });
    if (this.exited) throw new RpcError('disconnected', 'the server stopped');
    const w = this.w;
    switch (tool) {
      case 'project_info':
        return {
          name: 'fake',
          root: '/f',
          schema_version: 1,
          created: '2026-11-16T10:00:00Z',
          solver_version_created: '0.1.0',
          migrated_from: null,
          counts: { cases: 2, meshes: w.meshes.length, designs: 0, notes: 1, runs: w.runs.length, skipped_runs: 0 },
          runs_by_state: {},
          issues: [],
          issues_total: 0,
        };
      case 'list_runs': {
        const off = Number(args?.['offset'] ?? 0);
        const lim = Number(args?.['limit'] ?? 100);
        const page = w.runs.slice(off, off + lim).map((r) => ({ ...r, created: '2026-11-16T10:00:00Z', step: 3, time: 0.5 }));
        return { runs: page, total: w.runs.length, next_offset: off + lim < w.runs.length ? off + lim : null, skipped: [] };
      }
      case 'list_meshes':
        if (w.failMeshes) throw new RpcError(1202, 'meshes/ cannot be read');
        return { meshes: w.meshes.map((m) => ({ ...m, kind: 'file', bytes: 1200, files: 1 })), total: w.meshes.length, next_offset: null, unread: [] };
      case 'read_feed': {
        if (w.failFeed) throw new RpcError(1100, 'feed broken');
        const cursor = w.feed.length ? w.feed[w.feed.length - 1]!.n : 0;
        if (args?.['since'] === undefined) return { cursor, entries: [], total: w.feed.length };
        const since = Number(args['since']);
        const entries = w.feed.filter((e) => e.n > since);
        return { cursor, entries, next: entries.length ? entries[entries.length - 1]!.n : null, dropped: 0 };
      }
    }
  }
  onExit(l: (why: string) => void): void {
    this.exit.push(l);
  }
  die(): void {
    this.exited = true;
    for (const l of this.exit) l('exit code 1');
  }
  async close(): Promise<void> {
    this.exited = true;
  }
}

const world = (): FakeWorld => ({
  runs: [
    { id: 'r1', case: 'cases/a.toml', state: 'finished' },
    { id: 'r2', case: 'cases\\a.toml', state: 'failed' },
  ],
  meshes: [{ path: 'meshes/m.msh', used_by: ['cases/b.toml'] }],
  feed: [],
  failFeed: false,
  failMeshes: false,
});

const names = (cases: string[], notes: string[]) => async (_root: string, top: string) => {
  const list = top === 'cases' ? cases : notes;
  return { names: list, total: list.length };
};

test.describe('readLists and the session', () => {
  test('cases joined with their runs and mesh files; a list the engine cannot read carries its error', async () => {
    const w = world();
    const s = new FakeServer(w);
    const lists = await readLists(s, '/f', names(['cases/a.toml', 'cases/b.toml'], ['notes/n.md']));
    expect(lists.cases.items).toEqual([
      { path: 'cases/a.toml', runs: 2, meshes: [], unread: null },
      { path: 'cases/b.toml', runs: 0, meshes: ['meshes/m.msh'], unread: null },
    ]);
    expect(lists.runs.total).toBe(2);
    expect(lists.notes.items).toEqual([{ path: 'notes/n.md' }]);
    expect(lists.info?.name).toBe('fake');
    w.failMeshes = true;
    const broken = await readLists(s, '/f', names([], []));
    expect(broken.meshes).toEqual({ items: [], total: 0, error: 'meshes/ cannot be read' });
    expect(broken.runs.error).toBeNull();
    // the Studio asks only read tools (ADR 0736: list_trash, the project trash)
    expect(new Set(s.calls.map((c) => c.tool))).toEqual(new Set(['project_info', 'list_runs', 'list_meshes', 'list_trash']));
  });

  test('runs are paged to the end', async () => {
    const w = world();
    w.runs = Array.from({ length: 250 }, (_, i) => ({ id: `r${i}`, case: 'cases/a.toml', state: 'queued' }));
    const s = new FakeServer(w);
    const lists = await readLists(s, '/f', names([], []));
    expect(lists.runs.items).toHaveLength(250);
    expect(s.calls.filter((c) => c.tool === 'list_runs').map((c) => c.args?.['offset'])).toEqual([0, 100, 200]);
  });

  test('a feed entry adds a change and reads the lists again; a broken feed and a stopped server say so', async () => {
    const w = world();
    const server = new FakeServer(w);
    let caseNames = ['cases/a.toml'];
    const views: unknown[] = [];
    const s = await ProjectSession.open(
      '/f',
      '/e/rcfd-mcp',
      { startServer: async () => server, listFiles: async (_r, top) => (top === 'cases' ? { names: caseNames, total: caseNames.length } : { names: [], total: 0 }), now: () => new Date().toISOString(), pollMs: 20 },
      (v) => views.push(v),
    );
    try {
      expect(s.view.cases.total).toBe(1);
      expect(s.view.engine).toEqual({ name: 'rcfd', version: '0.0.0-test', path: '/e/rcfd-mcp' });
      caseNames = ['cases/a.toml', 'cases/b.toml'];
      w.feed.push({ n: 1, source: 'external', kind: 'external', text: 'cases/b.toml changed outside the workspace', path: 'cases/b.toml' });
      await until(async () => (s.view.cases.total === 2 ? true : null), 5000, 'the new case');
      expect(s.view.changes.map((c) => c.text)).toEqual(['cases/b.toml changed outside the workspace']);
      w.feed.push({ n: 2, source: 'run', kind: 'state', text: 'r1 finished', run: 'r1' });
      await until(async () => (s.view.changes.length === 2 ? true : null), 5000, 'the second change');
      expect(s.view.changes[0]!.n).toBe(2);
      for (let n = 3; n < MAX_CHANGES + 20; n++) w.feed.push({ n, source: 'external', kind: 'external', text: `x${n}` });
      await until(async () => (s.view.changes[0]!.n === MAX_CHANGES + 19 ? true : null), 5000, 'the burst');
      expect(s.view.changes).toHaveLength(MAX_CHANGES);
      w.failFeed = true;
      await until(async () => (!s.view.liveness.live ? true : null), 5000, 'not live');
      expect(s.view.liveness).toMatchObject({ live: false, serverStopped: false });
      w.failFeed = false;
      await until(async () => (s.view.liveness.live ? true : null), 5000, 'live again');
      server.die();
      expect(s.view.liveness).toMatchObject({ live: false, serverStopped: true });
      expect(s.serverStopped).toBe(true);
    } finally {
      await s.close();
    }
  });

  test('reconnect starts a new server after the old one stopped', async () => {
    const w = world();
    const servers: FakeServer[] = [];
    const s = await ProjectSession.open(
      '/f',
      '/e/rcfd-mcp',
      {
        startServer: async () => {
          const x = new FakeServer(w);
          servers.push(x);
          return x;
        },
        listFiles: names([], []),
        now: () => new Date().toISOString(),
        pollMs: 20,
      },
      () => undefined,
    );
    try {
      servers[0]!.die();
      await s.reconnect();
      expect(servers).toHaveLength(2);
      expect(s.view.liveness.live).toBe(true);
      w.feed.push({ n: 1, source: 'external', kind: 'external', text: 'later', path: 'notes/n.md' });
      await until(async () => (s.view.changes.length === 1 ? true : null), 5000, 'a change after the reconnect');
    } finally {
      await s.close();
    }
  });
});

test.describe('the home', () => {
  const memoryRecent = (initial: StoredRecent[] = []): RecentLike & { list: StoredRecent[] } => {
    const r = {
      list: initial,
      load: async () => r.list,
      save: async (l: readonly StoredRecent[]) => {
        r.list = [...l];
      },
    };
    return r;
  };
  const engine = { configured: true as const, path: '/e/rcfd-mcp', from: 'RCFD_STUDIO_MCP' as const };

  test('opens only picker results and recent folders; a failed open keeps the project open before', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-home-'));
    try {
      const a = path.join(dir, 'a');
      const b = path.join(dir, 'b');
      await mkdir(a);
      await mkdir(b);
      const recent = memoryRecent();
      let pick: string | null = a;
      const home = new ProjectHome({
        engine,
        recent,
        pickFolder: async () => pick,
        startServer: async (_e, f) => {
          if (f === b) throw new StartFailure('not_a_project', 'no', 'error: --project: not an rcfd project folder');
          return new FakeServer(world());
        },
        listFiles: names([], []),
        pollMs: 50,
      });
      await home.init();
      await expect(home.openRecent(a)).rejects.toBeInstanceOf(HomeRefusal);
      expect(await home.pick()).toEqual({ folder: a });
      expect(home.state.project?.folder).toBe(a);
      expect(recent.list.map((r) => r.path)).toEqual([a]);
      expect(Object.keys(recent.list[0]!).sort()).toEqual(['opened', 'path']);
      pick = b;
      await home.pick();
      expect(home.state.failure).toMatchObject({ kind: 'not_a_project', folder: b, detail: 'error: --project: not an rcfd project folder' });
      expect(home.state.project?.folder).toBe(a);
      expect(recent.list.map((r) => r.path)).toEqual([a]);
      pick = null;
      expect(await home.pick()).toEqual({ folder: null });
      home.dismiss();
      expect(home.state.failure).toBeNull();
      await home.openRecent(a.toUpperCase() === a ? a : process.platform === 'win32' ? a.toUpperCase() : a);
      expect(home.state.project?.folder).toBe(a);
      await home.close();
      expect(home.state.project).toBeNull();
      await rm(a, { recursive: true });
      await home.openRecent(a);
      expect(home.state.failure?.kind).toBe('not_found');
      expect(home.state.recent).toEqual([{ path: a, opened: recent.list[0]!.opened, exists: false }]);
      await home.forget(a);
      expect(home.state.recent).toEqual([]);
      expect(home.state.failure).toBeNull();
      await home.shutdown();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  test('F-835: create runs the engine on the folder chosen or the folder refused, then opens it; a refusal is said with the engine’s words', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-home-create-'));
    try {
      const a = path.join(dir, 'a');
      const b = path.join(dir, 'b');
      const c = path.join(dir, 'c');
      await mkdir(b);
      await mkdir(c);
      const made: string[] = [];
      let refuse: string | null = null;
      let pick: string | null = a;
      const projects = new Set<string>();
      const home = new ProjectHome({
        engine,
        recent: memoryRecent(),
        pickFolder: async () => c,
        pickNewFolder: async () => pick,
        // the fake engine: it records the folder (the Studio itself writes nothing)
        initProject: async (f) => {
          made.push(f);
          if (refuse !== null) return { ok: false, message: refuse };
          await mkdir(f, { recursive: true });
          projects.add(f);
          return { ok: true, result: { root: f, name: path.basename(f) } };
        },
        startServer: async (_e, f) => {
          if (!projects.has(f)) throw new StartFailure('not_a_project', 'no', 'error: --project: not an rcfd project folder');
          return new FakeServer(world());
        },
        listFiles: names([], []),
        pollMs: 50,
        firstWatchMs: 0,
      });
      await home.init();
      // here, with no folder refused as no project: refused, nothing run
      await expect(home.create('here')).rejects.toMatchObject({ code: 'no_folder' });
      expect(made).toEqual([]);
      // pick: a folder that is not there yet; the engine makes it, and the project opens
      expect(await home.create('pick')).toEqual({ folder: a });
      expect(made).toEqual([a]);
      expect(home.state.project?.folder).toBe(a);
      expect(home.state.recent.map((r) => r.path)).toEqual([a]);
      // the dialog cancelled: nothing run
      pick = null;
      expect(await home.create('pick')).toEqual({ folder: null });
      expect(made).toEqual([a]);
      // a folder the engine refused as no project, then Make a project here
      await home.pick();
      expect(home.state.failure).toMatchObject({ kind: 'not_a_project', folder: c });
      expect(home.state.project?.folder).toBe(a);
      expect(await home.create('here')).toEqual({ folder: c });
      expect(made).toEqual([a, c]);
      expect(home.state.failure).toBeNull();
      expect(home.state.project?.folder).toBe(c);
      // the engine refuses: the failure names the folder and carries the engine's message; the open project stays
      refuse = 'error: /b: is a file, not a folder';
      pick = b;
      expect(await home.create('pick')).toEqual({ folder: b });
      expect(home.state.failure).toMatchObject({ kind: 'not_created', folder: b, detail: refuse });
      expect(home.state.failure?.message).toContain(b);
      expect(home.state.opening).toBeNull();
      expect(home.state.project?.folder).toBe(c);
      await home.shutdown();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  test('without an engine, nothing starts and the failure says what to set', async () => {
    let started = 0;
    const home = new ProjectHome({
      engine: { configured: false, reason: 'Set RCFD_STUDIO_MCP.' },
      recent: memoryRecent(),
      pickFolder: async () => tmpdir(),
      startServer: async () => {
        started++;
        return new FakeServer(world());
      },
    });
    await home.init();
    await home.pick();
    expect(started).toBe(0);
    expect(home.state.failure).toMatchObject({ kind: 'not_configured', message: 'Set RCFD_STUDIO_MCP.' });
  });
});

test.describe('the project port', () => {
  test('state goes out at once and on every change; refused requests are answered with their reason', async () => {
    const { port1, port2 } = new MessageChannel();
    const adapt = (p: MessagePort): ProjectPortLike => ({
      postMessage: (m) => p.postMessage(m),
      onMessage: (l) => p.on('message', l),
      onClose: (l) => p.on('close', l),
      close: () => p.close(),
    });
    let listener: ((s: ProjectState) => void) | null = null;
    const state: ProjectState = { engine: { configured: false, reason: 'r' }, recent: [], opening: null, failure: null, project: null };
    const calls: string[] = [];
    const bridge = new ProjectPortBridge(adapt(port1), {
      state,
      onState: (l) => {
        listener = l;
        return () => (listener = null);
      },
      pick: async () => (calls.push('pick'), { folder: null }),
      openRecent: async (p) => {
        calls.push(`open ${p}`);
        throw new HomeRefusal('not_recent', 'only a folder of the recent list');
      },
      forget: async (p) => void calls.push(`forget ${p}`),
      close: async () => void calls.push('close'),
      refresh: async () => void calls.push('refresh'),
      dismiss: () => void calls.push('dismiss'),
    });
    const got: ProjectPortMessage[] = [];
    port2.on('message', (m: ProjectPortMessage) => got.push(m));
    try {
      await until(async () => (got.length >= 1 ? true : null), 2000, 'the first state');
      expect(got[0]).toEqual({ type: 'state', state });
      listener!({ ...state, opening: '/x' });
      port2.postMessage({ op: 'pick', id: 1 });
      port2.postMessage({ op: 'open', id: 2, path: abs('/elsewhere') });
      port2.postMessage({ op: 'read_project_file', id: 3, path: 'rcfd-project.json' });
      port2.postMessage({ id: 'x' });
      port2.postMessage({ op: 'close', id: 4 });
      await until(async () => (got.filter((m) => m.type === 'reply').length === 4 ? true : null), 2000, 'four replies');
      const replies = Object.fromEntries(got.flatMap((m) => (m.type === 'reply' ? [[m.id, m.reply]] : [])));
      expect(replies[1]).toEqual({ ok: true, result: { folder: null } });
      expect(replies[2]).toEqual({ ok: false, error: { code: 'not_recent', message: 'only a folder of the recent list' } });
      expect(replies[3]).toMatchObject({ ok: false, error: { code: 'bad_request' } });
      expect(replies[4]).toEqual({ ok: true, result: null });
      expect(got.filter((m) => m.type === 'state')).toHaveLength(2);
      expect(bridge.refused).toBe(2);
      expect(calls).toEqual(['pick', `open ${abs('/elsewhere')}`, 'close']);
    } finally {
      bridge.dispose();
      port2.close();
    }
  });
});

test.describe('the engine answers', () => {
  test('a tool error keeps the engine code and message', () => {
    expect(toolResult('x', { structuredContent: { a: 1 } })).toEqual({ a: 1 });
    try {
      toolResult('read_feed', { isError: true, structuredContent: { error: { code: 1100, message: 'read-only' } } });
      throw new Error('no throw');
    } catch (e) {
      expect(e).toBeInstanceOf(RpcError);
      expect((e as RpcError).code).toBe(1100);
      expect((e as RpcError).message).toBe('read-only');
    }
  });
});

test('project/project.css uses only defined tokens and no colour literals', () => {
  const css = readFileSync(path.join(appRoot, 'src', 'renderer', 'project', 'project.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(0);
  expect(used.filter((v) => !defined.has(v))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  const radii = [...css.matchAll(/border-radius:\s*([^;]+)/g)].map((m) => m[1]!.trim());
  expect(radii.filter((v) => v !== 'var(--radius-none)' && v !== '0')).toEqual([]);
});

test('only project/door.ts of the renderer names window.studioProject', () => {
  const renderer = path.join(appRoot, 'src', 'renderer');
  const walk = (d: string): string[] =>
    readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(path.join(d, e.name)) : /\.(ts|tsx)$/.test(e.name) && !e.name.endsWith('.d.ts') ? [path.join(d, e.name)] : []));
  const code = (t: string) => t.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');
  const naming = walk(renderer)
    .filter((f) => /studioProject/.test(code(readFileSync(f, 'utf8'))))
    .map((f) => path.relative(renderer, f).split(path.sep).join('/'));
  expect(naming).toEqual(['project/door.ts']);
});
