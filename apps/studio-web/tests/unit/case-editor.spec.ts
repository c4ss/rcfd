// Unit tests of the case editor's main-process side (F-812, src/main/case/) against a fake service and a fake project
// server: what a save writes (the settings set since the last save, with the session's values, compared against the
// hash the editor read), the conflict and refusal outcomes, the removals the engine's edit cannot write, the binding
// checks, the allow-list of tools, and the port's message checks. Plus the static rules of src/renderer/case/: no table
// of case keys (G2), tokens-only CSS, and one door to window.studioCase.
import { expect, test } from '@playwright/test';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { CASE_TOOLS, SAVE_REASON } from '../../src/shared/case.js';
import { parseCaseRequest } from '../../src/main/case/bridge.js';
import { CaseEditor, caseRelPath, samePath } from '../../src/main/case/editor.js';
import { pendingChanges } from '../../src/main/case/plan.js';
import { CaseServer, type ToolServerLike } from '../../src/main/case/server.js';
import { RpcError } from '../../src/main/service/rpc.js';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import type { Inventory } from '../support/capabilities.js';
import { appRoot } from '../support/rcfd.js';

const FOLDER = path.resolve('/projects/demo');
const abs = (rel: string) => path.join(FOLDER, ...rel.split('/'));

type Values = Record<string, unknown>;

/** The project folder as the fake project server sees it: each file's hash and the values the engine reads from it. */
class FakeFolder {
  files = new Map<string, { hash: string; values: Values }>();
  calls: { tool: string; args: Record<string, unknown> }[] = [];
  n = 0;
  put(rel: string, values: Values): string {
    const hash = `h${++this.n}`;
    this.files.set(rel, { hash, values: { ...values } });
    return hash;
  }
}

class FakeToolServer implements ToolServerLike {
  exited = false;
  constructor(private readonly folder: FakeFolder) {}
  async callTool(tool: string, args: Record<string, unknown> = {}): Promise<unknown> {
    this.folder.calls.push({ tool, args });
    const f = this.folder.files.get(String(args['path'] ?? ''));
    switch (tool) {
      case 'read_project_file':
        if (!f) throw new RpcError(1202, `${String(args['path'])}: no such file`);
        return { path: args['path'], hash: f.hash, size: 10, text: 'x', offset: 0, next_offset: 1 };
      case 'edit_case': {
        if (!f) throw new RpcError(1202, 'no such file');
        if (args['base_hash'] !== f.hash) throw new RpcError(1200, `${String(args['path'])}: changed since it was read (now ${f.hash}); read it again`, { current_hash: f.hash });
        const set = args['set'] as Values;
        if (Object.values(set).includes('refuse')) throw new RpcError(1003, 'refused', { diagnostics: [{ path: Object.keys(set)[0], line: null, message: 'not a value' }] });
        const hash = this.folder.put(String(args['path']), { ...f.values, ...set });
        return { path: args['path'], written: true, valid: true, form: 'minimal', old_hash: f.hash, new_hash: hash, values: set };
      }
      case 'new_case': {
        const rel = `cases/${String(args['name'])}.toml`;
        if (this.folder.files.has(rel)) throw new RpcError(1004, `${rel} is taken`);
        const from = args['copy'] ? this.folder.files.get(String(args['copy']))!.values : {};
        const hash = this.folder.put(rel, from);
        return { path: rel, hash, bytes: 1, from: args['template'] ? `template ${String(args['template'])}` : `copy ${String(args['copy'])}`, data_files: [] };
      }
      case 'check_case_headless':
        return { ok: true, exit_code: 0, event: { diagnostics: [], cells: 1024, mesh: { errors: [], warnings: [] } } };
      default:
        throw new Error(`the fake has no ${tool}`);
    }
  }
  async listTools(): Promise<readonly unknown[]> {
    return [{ name: 'new_case', inputSchema: { properties: { name: { description: 'a name' }, template: { enum: ['lid_driven_cavity'], description: 't' }, example: { enum: ['sod/sod.toml'], description: 'e' } } } }];
  }
  async close(): Promise<void> {
    this.exited = true;
  }
}

/** A fake service: sessions with a journal and their current values; a session made from a file reads the fake folder. */
class FakeService {
  sessions = new Map<string, { journal: Record<string, unknown>[]; values: Values }>();
  n = 0;
  constructor(private readonly folder: FakeFolder) {}
  make(file: string, values?: Values): string {
    const id = `s${++this.n}`;
    const rel = path.relative(FOLDER, file).split(path.sep).join('/');
    this.sessions.set(id, { journal: [{ op: 'create', source: { path: file } }], values: { ...(values ?? this.folder.files.get(rel)?.values ?? {}) } });
    return id;
  }
  set(s: string, p: string, v: unknown): void {
    const x = this.sessions.get(s)!;
    x.values[p] = v;
    x.journal.push({ op: 'set', path: p, value: v });
  }
  remove(s: string, p: string): void {
    const x = this.sessions.get(s)!;
    for (const k of Object.keys(x.values)) if (k === p || k.startsWith(`${p}.`)) delete x.values[k];
    x.journal.push({ op: 'remove', path: p });
  }
  async call(method: string, params: Record<string, unknown> = {}): Promise<unknown> {
    const s = this.sessions.get(String(params['session']));
    switch (method) {
      case 'journal.get':
        if (!s) throw new RpcError(1001, 'unknown session');
        return { format: 'rcfd-journal', version: 1, entries: s.journal, case_hash: 'x' };
      case 'settings.get': {
        if (!s) throw new RpcError(1001, 'unknown session');
        const p = String(params['path']);
        const hit = Object.keys(s.values).find((k) => k === p || k.startsWith(`${p}.`));
        if (hit === undefined) throw new RpcError(1003, `settings path not found: ${p}`);
        return { value: s.values[p] ?? {} };
      }
      case 'session.create':
        return { session: this.make(String(params['path'])) };
      case 'session.close':
        this.sessions.delete(String(params['session']));
        return {};
      default:
        throw new Error(`the fake has no ${method}`);
    }
  }
}

function setup(open = true) {
  const folder = new FakeFolder();
  const service = new FakeService(folder);
  let project: { folder: string } | null = open ? { folder: FOLDER } : null;
  const started: string[] = [];
  const editor = new CaseEditor({
    project: () => project,
    engine: { configured: true, path: '/bin/rcfd-mcp', from: 'RCFD_STUDIO_MCP' },
    service,
    startServer: async (_exe, f) => {
      started.push(f);
      return new FakeToolServer(folder);
    },
    now: () => '2026-11-16T12:00:00.000Z',
  });
  return { folder, service, editor, started, close: () => (project = null) };
}

/** Opens `rel` as the renderer does: prepare, a session made from the file, attach. */
async function open(t: ReturnType<typeof setup>, rel: string): Promise<string> {
  const p = await t.editor.prepare(rel);
  const s = t.service.make(p.file);
  await t.editor.attach(s, p.path, p.hash);
  return s;
}

test.describe('paths and plans', () => {
  test('a case path is cases/<...>.toml or .json, without empty, dot or dot-dot parts', () => {
    expect(caseRelPath('cases/a.toml')).toBe('cases/a.toml');
    expect(caseRelPath('cases\\sub\\b.JSON')).toBe('cases/sub/b.JSON');
    for (const bad of ['a.toml', 'cases/', 'cases/a.txt', 'cases/../rcfd-project.json', '/cases/a.toml', 'cases//a.toml', 'cases/./a.toml', 'meshes/a.toml', 'cases/C:a.toml']) {
      expect(caseRelPath(bad), bad).toBeNull();
    }
    expect(samePath('C:\\P\\cases\\a.toml', 'c:/p/cases/a.toml', 'win32')).toBe(true);
  });

  test('the pending changes: set paths in first-set order, removals dropping the sets under them', () => {
    const j = [
      { op: 'create' },
      { op: 'set', path: 'physics.nu' },
      { op: 'set', path: 'boundaries.inlet.velocity' },
      { op: 'set', path: 'physics.nu' },
      { op: 'set', path: 'boundaries.inlet.velocity_value' },
      { op: 'remove', path: 'boundaries.inlet' },
      { op: 'set', path: 'solver.max_iterations' },
    ];
    expect(pendingChanges(j, 1)).toEqual({ count: 6, paths: ['physics.nu', 'solver.max_iterations'], removals: ['boundaries.inlet'] });
    expect(pendingChanges(j, 7)).toEqual({ count: 0, paths: [], removals: [] });
    expect(pendingChanges(j, 4)).toEqual({ count: 3, paths: ['solver.max_iterations'], removals: ['boundaries.inlet'] });
  });
});

test.describe('the case port', () => {
  test('refuses anything but exactly the fields of an operation', () => {
    expect(parseCaseRequest({ op: 'save', id: 1, session: 's1', onto: 'opened' })).toEqual({ ok: true, request: { op: 'save', id: 1, session: 's1', onto: 'opened' } });
    expect(parseCaseRequest({ op: 'create', id: 2, name: 'a', source: { kind: 'template', name: 'lid_driven_cavity' } }).ok).toBe(true);
    const refused = [
      'x',
      { op: 'state' },
      { op: 'state', id: 1, extra: 1 },
      { op: 'write', id: 1 },
      { op: 'save', id: 1, session: 's1', onto: 'force' },
      { op: 'save', id: 1, session: 's 1', onto: 'opened' },
      { op: 'prepare', id: 1, path: '' },
      { op: 'prepare', id: 1, path: 'x'.repeat(5000) },
      { op: 'attach', id: 1, session: 's1', path: 'cases/a.toml' },
      { op: 'create', id: 1, name: 'a', source: { kind: 'toml', name: 'x' } },
      { op: 'create', id: 1, name: 'a'.repeat(65), source: { kind: 'template', name: 'x' } },
      { op: 'duplicate', id: 1, path: 'cases/a.toml' },
    ];
    for (const r of refused) expect(parseCaseRequest(r).ok, JSON.stringify(r).slice(0, 80)).toBe(false);
  });

  test('the project server is called only with the tools of the allow-list', async () => {
    const folder = new FakeFolder();
    const server = new CaseServer(new FakeToolServer(folder));
    await expect(server.call('write_note' as never, { path: 'notes/a.md', text: 'x' })).rejects.toThrow(/allow-list/);
    await expect(server.call('start_run' as never, {})).rejects.toThrow(/allow-list/);
    expect(server.refused).toBe(2);
    expect(folder.calls).toEqual([]);
    expect([...CASE_TOOLS]).toEqual(['read_project_file', 'edit_case', 'new_case', 'check_case_headless']);
  });
});

test.describe('the editor', () => {
  test('attach binds only a session made from that file, and only while the file is as read', async () => {
    const t = setup();
    t.folder.put('cases/a.toml', { 'physics.nu': 0.01 });
    t.folder.put('cases/b.toml', {});
    const p = await t.editor.prepare('cases/a.toml');
    expect(p).toEqual({ path: 'cases/a.toml', file: abs('cases/a.toml'), hash: 'h1', size: 10 });
    const other = t.service.make(abs('cases/b.toml'));
    await expect(t.editor.attach(other, 'cases/a.toml', p.hash)).rejects.toMatchObject({ code: 'not_from_file' });
    const s = t.service.make(p.file);
    t.folder.put('cases/a.toml', { 'physics.nu': 0.5 }); // another writer, between prepare and attach
    await expect(t.editor.attach(s, 'cases/a.toml', p.hash)).rejects.toMatchObject({ code: 'changed', data: { current_hash: 'h3' } });
    expect(t.editor.state().bindings).toEqual([]);
    await expect(t.editor.prepare('rcfd-project.json')).rejects.toMatchObject({ code: 'bad_request' });
  });

  test('save writes the settings changed since the last save, with the session’s values, against the hash read', async () => {
    const t = setup();
    t.folder.put('cases/a.toml', { 'physics.nu': 0.01, 'solver.max_iterations': 2000 });
    const s = await open(t, 'cases/a.toml');
    expect((await t.editor.status(s)).pending).toEqual({ count: 0, paths: [], removals: [] });
    expect(await t.editor.save(s, 'opened')).toEqual({ kind: 'nothing' });
    t.service.set(s, 'physics.nu', 0.03);
    t.service.set(s, 'physics.nu', 0.02);
    t.service.set(s, 'boundaries.inlet.velocity', 'fixed');
    const st = await t.editor.status(s);
    expect(st.pending).toEqual({ count: 3, paths: ['physics.nu', 'boundaries.inlet.velocity'], removals: [] });
    expect(st.file).toEqual({ state: 'same', hash: 'h1' });
    t.folder.calls = [];
    const saved = await t.editor.save(s, 'opened');
    expect(saved).toMatchObject({ kind: 'saved', path: 'cases/a.toml', oldHash: 'h1', newHash: 'h2', form: 'minimal', reopen: false });
    const edit = t.folder.calls.find((c) => c.tool === 'edit_case')!;
    expect(edit.args).toEqual({ path: 'cases/a.toml', base_hash: 'h1', set: { 'physics.nu': 0.02, 'boundaries.inlet.velocity': 'fixed' }, reason: SAVE_REASON });
    expect(t.folder.calls.filter((c) => c.tool === 'edit_case' || c.tool === 'new_case')).toHaveLength(1);
    // saved: nothing pending, the next save compares the new hash and writes only what changed after
    const after = await t.editor.status(s);
    expect(after.pending.count).toBe(0);
    expect(after.binding).toMatchObject({ hash: 'h2', savedAt: '2026-11-16T12:00:00.000Z' });
    t.service.set(s, 'solver.max_iterations', 1500);
    await t.editor.save(s, 'opened');
    expect(t.folder.calls.filter((c) => c.tool === 'edit_case').pop()!.args).toMatchObject({ base_hash: 'h2', set: { 'solver.max_iterations': 1500 } });
  });

  test('a file changed by another writer is a conflict: nothing written, both values shown, then each choice', async () => {
    const t = setup();
    t.folder.put('cases/a.toml', { 'solver.max_iterations': 2000 });
    const s = await open(t, 'cases/a.toml');
    t.service.set(s, 'solver.max_iterations', 1200);
    t.folder.put('cases/a.toml', { 'solver.max_iterations': 3000, title: 'by hand' });
    expect((await t.editor.status(s)).file).toEqual({ state: 'changed', hash: 'h2' });
    const r = await t.editor.save(s, 'opened');
    expect(r).toMatchObject({ kind: 'conflict', currentHash: 'h2' });
    expect(t.folder.files.get('cases/a.toml')!.values).toEqual({ 'solver.max_iterations': 3000, title: 'by hand' });
    const view = await t.editor.conflict(s);
    expect(view).toEqual({ changed: true, currentHash: 'h2', rows: [{ path: 'solver.max_iterations', mine: 1200, file: 3000, inFile: true }], removals: [], fileError: null });
    // the session made to read the file is closed again
    expect([...t.service.sessions.keys()]).toEqual([s]);
    // the user's choice: write mine onto the file as it is now (their other change stays); the editor reopens it
    const onto = await t.editor.save(s, 'current');
    expect(onto).toMatchObject({ kind: 'saved', oldHash: 'h2', reopen: true });
    expect(t.folder.files.get('cases/a.toml')!.values).toEqual({ 'solver.max_iterations': 1200, title: 'by hand' });
  });

  test('a removed entry the file holds cannot be saved in place; one added and removed in the session writes nothing', async () => {
    const t = setup();
    t.folder.put('cases/a.toml', { 'boundaries.lid.velocity': 'fixed', 'physics.nu': 0.01 });
    const s = await open(t, 'cases/a.toml');
    t.service.set(s, 'physics.nu', 0.02);
    t.service.remove(s, 'boundaries.lid');
    const r = await t.editor.save(s, 'opened');
    expect(r).toMatchObject({ kind: 'cannot', removals: ['boundaries.lid'] });
    expect(t.folder.calls.some((c) => c.tool === 'edit_case')).toBe(false);
    expect(t.folder.files.get('cases/a.toml')!.hash).toBe('h1');

    const u = setup();
    u.folder.put('cases/b.toml', { 'physics.nu': 0.01 });
    const s2 = await open(u, 'cases/b.toml');
    u.service.set(s2, 'boundaries.inlet.velocity', 'fixed');
    u.service.remove(s2, 'boundaries.inlet');
    expect(await u.editor.save(s2, 'opened')).toEqual({ kind: 'nothing' });
    expect((await u.editor.status(s2)).pending.count).toBe(0);
    u.service.set(s2, 'physics.nu', 0.5);
    u.service.set(s2, 'boundaries.inlet.velocity', 'fixed');
    u.service.remove(s2, 'boundaries.inlet');
    expect(await u.editor.save(s2, 'opened')).toMatchObject({ kind: 'saved' });
    expect(u.folder.calls.filter((c) => c.tool === 'edit_case').pop()!.args['set']).toEqual({ 'physics.nu': 0.5 });
  });

  test('a refused edit returns the engine’s located diagnostics; nothing changes', async () => {
    const t = setup();
    t.folder.put('cases/a.toml', { title: 'x' });
    const s = await open(t, 'cases/a.toml');
    t.service.set(s, 'title', 'refuse');
    expect(await t.editor.save(s, 'opened')).toEqual({ kind: 'refused', message: 'refused', diagnostics: [{ path: 'title', line: null, message: 'not a value' }] });
    expect((await t.editor.status(s)).pending.count).toBe(1);
  });

  test('no project, another project, a closed session: refused with what to do', async () => {
    const t = setup();
    t.folder.put('cases/a.toml', {});
    const s = await open(t, 'cases/a.toml');
    t.service.set(s, 'title', 'y');
    t.close();
    t.editor.projectChanged();
    await expect(t.editor.save(s, 'opened')).rejects.toMatchObject({ code: 'no_project' });
    const st = await t.editor.status(s);
    expect(st.projectOpen).toBe(false);
    expect(st.file.state).toBe('unknown');
    await expect(t.editor.prepare('cases/a.toml')).rejects.toMatchObject({ code: 'no_project' });
    const u = setup();
    u.folder.put('cases/a.toml', {});
    const s2 = await open(u, 'cases/a.toml');
    u.service.sessions.delete(s2);
    await expect(u.editor.status(s2)).rejects.toMatchObject({ code: 'not_bound' });
    expect(u.editor.state().bindings).toEqual([]);
    await expect(u.editor.status('s99')).rejects.toMatchObject({ code: 'not_bound' });
  });

  test('new case only from a template or example the engine offers; duplicate copies; one project server per project', async () => {
    const t = setup();
    t.folder.put('cases/a.toml', { title: 'a' });
    expect(await t.editor.sources()).toEqual({ templates: ['lid_driven_cavity'], examples: ['sod/sod.toml'], templateHelp: 't', exampleHelp: 'e', nameHelp: 'a name' });
    await expect(t.editor.create('x', { kind: 'template', name: 'my_own' })).rejects.toMatchObject({ code: 'bad_request' });
    expect(await t.editor.create('first', { kind: 'template', name: 'lid_driven_cavity' })).toEqual({ path: 'cases/first.toml', hash: 'h2', from: 'template lid_driven_cavity', dataFiles: [] });
    expect(t.folder.calls.find((c) => c.tool === 'new_case')!.args).toMatchObject({ name: 'first', template: 'lid_driven_cavity' });
    expect(await t.editor.duplicate('cases/a.toml', 'copy')).toMatchObject({ path: 'cases/copy.toml', from: 'copy cases/a.toml' });
    await expect(t.editor.duplicate('cases/a.toml', 'copy')).rejects.toMatchObject({ code: 1004 });
    expect(await t.editor.check('cases/a.toml')).toEqual({ path: 'cases/a.toml', ok: true, exitCode: 0, diagnostics: [], cells: 1024, meshErrors: [], meshWarnings: [] });
    expect(t.started).toEqual([FOLDER]);
  });
});

// ---- static rules of src/renderer/case

const caseDir = path.join(appRoot, 'src', 'renderer', 'case');
const literals = (text: string): string[] => {
  const code = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');
  return [...code.matchAll(/'((?:[^'\\\n]|\\.)*)'|"((?:[^"\\\n]|\\.)*)"/g)].map((m) => m[1] ?? m[2] ?? '');
};

test('the case editor holds no table of case keys: sections and keys come from the engine (G2)', () => {
  const inv = JSON.parse(readFileSync(path.join(appRoot, 'capabilities', 'inventory.json'), 'utf8')) as Inventory;
  const keys = inv.capabilities.filter((c) => c.kind === 'case_key').map((c) => c.id.slice('case:'.length));
  const sections = new Set(keys.map((k) => k.split('.')[0]!));
  const paths = new Set(keys.filter((k) => k.includes('.')));
  const files = [
    ...readdirSync(caseDir).filter((f) => /\.(ts|tsx)$/.test(f)).map((f) => path.join(caseDir, f)),
    ...readdirSync(path.join(appRoot, 'src', 'main', 'case')).map((f) => path.join(appRoot, 'src', 'main', 'case', f)),
  ];
  expect(files.length).toBeGreaterThanOrEqual(7);
  // the port's operation names are words of its protocol (src/shared/case.ts), not case sections
  const ops = new Set(['sources']);
  const named: string[] = [];
  for (const f of files) {
    for (const s of literals(readFileSync(f, 'utf8'))) {
      if ((sections.has(s) || paths.has(s)) && !(path.basename(f) === 'bridge.ts' && ops.has(s))) named.push(`${path.basename(f)}: '${s}'`);
    }
  }
  expect(named).toEqual([]);
});

test('case.css uses only defined tokens, no colour literals, square corners and no drop shadow', () => {
  const css = readFileSync(path.join(caseDir, 'case.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(30);
  expect(used.filter((u) => !defined.has(u))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  expect(css.match(/border-radius|box-shadow/g) ?? []).toEqual([]);
});

test('only case/door.ts of the renderer names window.studioCase', () => {
  const renderer = path.join(appRoot, 'src', 'renderer');
  const walk = (d: string): string[] =>
    readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(path.join(d, e.name)) : /\.(ts|tsx)$/.test(e.name) && !e.name.endsWith('.d.ts') ? [path.join(d, e.name)] : []));
  const code = (t: string) => t.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');
  const naming = walk(renderer)
    .filter((f) => /studioCase/.test(code(readFileSync(f, 'utf8'))))
    .map((f) => path.relative(renderer, f).split(path.sep).join('/'));
  expect(naming).toEqual(['case/door.ts']);
});
