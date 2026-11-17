// ADR 0737 (F-837) in the Studio, without the engine: a case file outside a project bound with the hash the service's
// journal recorded and saved by the service's `case.save` (the changed lines, the hash compared; 1007 is the conflict,
// never an overwrite), only files the open dialog gave; the case port's two new operations; the #/case/file route; the
// estimate line read from a `case.facts` reply (the Studio computes no part of the run-time rule); the new methods on
// the allow-list, with registry actions. The engine side is crates/rcfd-api/tests/case_facts_adr0737.rs; the packaged
// Studio against the real engine is tests/e2e/case-file-adr0737.spec.ts.
import { expect, test } from '@playwright/test';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { CASE_ACTIONS, CASE_OPS } from '../../src/shared/case.js';
import { SERVICE_CONTROL_METHODS, SERVICE_READ_METHODS } from '../../src/shared/service.js';
import { getAction } from '../../src/shared/actions/registry.js';
import { parseCaseRequest } from '../../src/main/case/bridge.js';
import { CaseEditor } from '../../src/main/case/editor.js';
import { RpcError } from '../../src/main/service/rpc.js';
import { caseHash, caseRoute } from '../../src/renderer/case/hash.js';
import { estimateOf, estimateParts, estimateWords, short } from '../../src/renderer/case/estimateModel.js';
import { appRoot } from '../support/rcfd.js';

const FILE = path.resolve('/elsewhere/cavity.toml');

/** A service with one file on disk (its hash changes with each write) and the methods the editor calls. */
class FakeService {
  disk = { hash: 'fnv64:1', values: { 'mesh.nx': 8 } as Record<string, unknown> };
  sessions = new Map<string, { journal: Record<string, unknown>[]; values: Record<string, unknown> }>();
  saves: Record<string, unknown>[] = [];
  n = 0;
  async call(method: string, params: Record<string, unknown> = {}): Promise<unknown> {
    const s = this.sessions.get(String(params['session']));
    switch (method) {
      case 'session.create': {
        const id = `s${++this.n}`;
        this.sessions.set(id, { journal: [{ op: 'create', source: { kind: 'path', path: String(params['path']), hash: this.disk.hash } }], values: { ...this.disk.values } });
        return { session: id };
      }
      case 'session.close':
        this.sessions.delete(String(params['session']));
        return {};
      case 'journal.get':
        if (!s) throw new RpcError(1001, 'unknown session');
        return { entries: s.journal };
      case 'settings.set':
        if (!s) throw new RpcError(1001, 'unknown session');
        this.set(String(params['session']), String(params['path']), params['value']);
        return {};
      case 'settings.get': {
        if (!s) throw new RpcError(1001, 'unknown session');
        const p = String(params['path']);
        if (!(p in s.values)) throw new RpcError(1003, `settings path not found: ${p}`);
        return { value: s.values[p] };
      }
      case 'case.save': {
        this.saves.push(params);
        if (params['base_hash'] !== this.disk.hash) throw new RpcError(1007, 'the file changed since it was read', { current_hash: this.disk.hash });
        const old = this.disk.hash;
        this.disk = { hash: `fnv64:${Number(old.slice(6)) + 1}`, values: { ...s!.values } };
        return { path: params['path'], old_hash: old, new_hash: this.disk.hash, form: params['form'] };
      }
      default:
        throw new Error(`the fake has no ${method}`);
    }
  }
  set(session: string, p: string, v: unknown): void {
    const x = this.sessions.get(session)!;
    x.values[p] = v;
    x.journal.push({ op: 'set', path: p, value: v });
  }
}

function setup(picked: string | null = FILE) {
  const service = new FakeService();
  const editor = new CaseEditor({
    project: () => null,
    engine: { configured: false, reason: 'no project server in this test' } as never,
    service,
    startServer: () => Promise.reject(new Error('no project server: a file outside a project never asks one')),
    pickFile: async () => picked,
    now: () => '2026-11-17T10:00:00.000Z',
  });
  return { service, editor };
}

async function open(service: FakeService, editor: CaseEditor) {
  const f = await editor.pickFile();
  const { session } = (await service.call('session.create', { path: f!.file })) as { session: string };
  return { session, binding: await editor.attachFile(session, f!.file) };
}

test.describe('a case file outside a project (ADR 0737)', () => {
  test('bound with the journal hash, without a project or a project server', async () => {
    const { service, editor } = setup();
    const { session, binding } = await open(service, editor);
    expect(binding).toMatchObject({ session, path: FILE, file: FILE, folder: path.dirname(FILE), hash: 'fnv64:1', outside: true });
    const st = await editor.status(session);
    expect(st.projectOpen).toBe(true);
    expect(st.file).toEqual({ state: 'same', hash: 'fnv64:1' });
    expect(st.pending.count).toBe(0);
    // the status read the hash from a session of its own, closed again
    expect([...service.sessions.keys()]).toEqual([session]);
  });

  test('a save is case.save {form: lines} with the hash read; then the new hash is the base', async () => {
    const { service, editor } = setup();
    const { session } = await open(service, editor);
    expect(await editor.save(session, 'opened')).toEqual({ kind: 'nothing' });
    service.set(session, 'mesh.nx', 16);
    const o = await editor.save(session, 'opened');
    expect(o).toMatchObject({ kind: 'saved', path: FILE, oldHash: 'fnv64:1', newHash: 'fnv64:2', form: 'minimal', values: { 'mesh.nx': 16 }, reopen: false });
    expect(service.saves).toEqual([{ session, path: FILE, base_hash: 'fnv64:1', form: 'lines' }]);
    expect((await editor.status(session)).pending.count).toBe(0);
  });

  test('a file changed outside is a conflict (1007), shown with both sides; nothing is written until the choice', async () => {
    const { service, editor } = setup();
    const { session } = await open(service, editor);
    service.set(session, 'mesh.nx', 16);
    service.disk = { hash: 'fnv64:7', values: { 'mesh.nx': 12, 'physics.nu': 0.02 } };
    expect((await editor.status(session)).file).toEqual({ state: 'changed', hash: 'fnv64:7' });
    const o = await editor.save(session, 'opened');
    expect(o).toEqual({ kind: 'conflict', currentHash: 'fnv64:7', message: 'the file changed since it was read' });
    expect(service.disk.values['mesh.nx']).toBe(12);
    const c = await editor.conflict(session);
    expect(c).toMatchObject({ changed: true, currentHash: 'fnv64:7', rows: [{ path: 'mesh.nx', mine: 16, file: 12, inFile: true }] });
    const mine = await editor.save(session, 'current');
    expect(mine).toMatchObject({ kind: 'saved', oldHash: 'fnv64:7', reopen: true });
    // the setting listed is written; the other writer's other change stays (a session of the file now, saved)
    expect(service.disk.values).toEqual({ 'mesh.nx': 16, 'physics.nu': 0.02 });
    expect([...service.sessions.keys()]).toEqual([session]);
  });

  test('only a file the open dialog gave is bound; a cancelled dialog is null', async () => {
    const { service, editor } = setup();
    const { session } = (await service.call('session.create', { path: FILE })) as { session: string };
    await expect(editor.attachFile(session, FILE)).rejects.toMatchObject({ code: 'bad_request' });
    const other = path.resolve('/elsewhere/other.toml');
    await editor.pickFile();
    await expect(editor.attachFile(session, other)).rejects.toMatchObject({ code: 'bad_request' });
    expect(await setup(null).editor.pickFile()).toBeNull();
    await expect(setup(path.resolve('/elsewhere/notes.txt')).editor.pickFile()).rejects.toMatchObject({ code: 'bad_request' });
  });

  test('the case port takes pickFile {} and attachFile {path, session}, exactly', () => {
    expect(CASE_OPS).toContain('pickFile');
    expect(CASE_OPS).toContain('attachFile');
    expect(parseCaseRequest({ id: 1, op: 'pickFile' }).ok).toBe(true);
    expect(parseCaseRequest({ id: 1, op: 'pickFile', path: FILE }).ok).toBe(false);
    expect(parseCaseRequest({ id: 2, op: 'attachFile', session: 's1', path: FILE }).ok).toBe(true);
    expect(parseCaseRequest({ id: 2, op: 'attachFile', session: 's1' }).ok).toBe(false);
  });

  test('the route #/case/file and the palette action', () => {
    expect(caseHash.file).toBe('#/case/file');
    expect(caseRoute('#/case/file')).toEqual({ kind: 'file' });
    expect(CASE_ACTIONS.find((a) => a.id === 'case.open_file')?.engine).toContain('case.save {session, path, base_hash, form: lines}');
  });
});

test.describe('the estimate line and Refine x2 (ADR 0737)', () => {
  const transient = {
    cells: 64,
    h_min: 0.125,
    u0: 1,
    u0_from: 'boundary lid',
    run_time: { dt0: 0.0625, steps_estimate: 16, courant_estimate: 0.5, warning: null, entries: ['solver.end_time=1'] },
    memory: { mb: 65, rule: 'mem_base_mb + cells * mem_kb_per_cell / 1024', budgets: 'runner_defaults' },
  };

  test('read from case.facts: numbers with units; the words say the runner defaults when there is no project', () => {
    const e = estimateOf(transient);
    expect(estimateParts(e)).toEqual(['64 cells', 'h 0.125 m', 'dt 0.0625 s', '16 steps', 'Courant 0.5', '65 MB']);
    expect(estimateWords(e)).toContain("with the runner's default budgets (no project)");
    const steady = estimateOf({ cells: 400, h_min: 0.05, run_time: { refused: 'a steady run has no time step' }, memory: { mb: 66, budgets: 'project' } });
    expect(estimateParts(steady)).toEqual(['400 cells', 'h 0.05 m', '66 MB']);
    expect(estimateWords(steady)).toContain('no time step: a steady run has no time step');
    expect(estimateWords(steady)).toContain("with the project's runner budgets");
    expect([short(1.23456e-7), short(0.000123456), short(12345.678)]).toEqual(['1.23e-7', '0.0001235', '12350']);
  });

  test('the Studio holds no copy of the run-time rule: no Courant target, no cell-step seconds, no step count of its own', () => {
    const src = path.join(appRoot, 'src');
    const files: string[] = [];
    const walk = (d: string) => {
      for (const e of readdirSync(d, { withFileTypes: true })) {
        if (e.isDirectory()) walk(path.join(d, e.name));
        else if (/\.(ts|tsx|cts)$/.test(e.name)) files.push(path.join(d, e.name));
      }
    };
    walk(src);
    for (const f of files) {
      const t = readFileSync(f, 'utf8');
      // ADR 0153's constants, by value and by name
      expect(t, f).not.toMatch(/4\.7e-6|COURANT_TARGET|CELL_STEP_SECONDS|MIN_STEPS/);
    }
  });

  test('case.facts is a read method and mesh.refine a control method, each an action of the registry; case.save is not on the list', () => {
    expect(SERVICE_READ_METHODS).toContain('case.facts');
    expect(SERVICE_CONTROL_METHODS).toContain('mesh.refine');
    expect([...SERVICE_READ_METHODS, ...SERVICE_CONTROL_METHODS]).not.toContain('case.save');
    expect(getAction('case.facts')?.engine.service).toBe('case.facts');
    expect(getAction('mesh.refine')?.engine.service).toBe('mesh.refine');
  });
});
