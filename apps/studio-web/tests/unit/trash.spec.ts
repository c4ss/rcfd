// ADR 0736 in the Studio (F-829; G12, G16, G18): the project trash and compare-and-set for the settings writes.
// - the main process: every trash operation goes to the engine's run-control server under the tool TRASH_OPS names,
//   with only the arguments that operation takes; a case is sent with the engine's hash read just before as its base, a
//   run's results with the parts the run holds; the engine's refusal comes back in its words; the trash is read on the
//   read-only server (list_trash) and is in the view; the project port checks the request's shape;
// - the history: a move to the trash is an outside step whose undo restores and whose redo moves again, both through
//   the engine; an undo or redo of a settings write carries the value it expects as its base (base_value), so a value
//   another writer set between the history's read and its write is the engine's 1007, shown as the same conflict;
//   a forced undo sends no base;
// - the service: a settings.set whose base is stale is refused with 1007 and changes nothing (the fake service models
//   the engine's answer; crates/rcfd-api/tests/settings_cas_adr0736.rs holds the engine to it).
import { expect, test } from '@playwright/test';
import { mkdtempSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { TRASH_OPS, TRASH_WRITE_TOOLS, PROJECT_TOOLS, type ProjectTool } from '../../src/shared/project.js';
import { Dispatcher, History, withBase, type ActionCall } from '../../src/shared/actions/index.js';
import { SETTINGS_CONFLICT, equivalents, getAction } from '../../src/shared/actions/registry.js';
import { ProjectSession } from '../../src/main/project/session.js';
import { parseProjectRequest } from '../../src/main/project/bridge.js';
import type { ProjectServerLike } from '../../src/main/project/server.js';
import { RpcError } from '../../src/main/service/rpc.js';
import { FakeCaseService } from '../support/fake-case-service.js';
import { portTransport } from '../support/action-transport.js';

interface Call {
  readonly server: 'read' | 'control';
  readonly tool: string;
  readonly args: Record<string, unknown> | undefined;
}

class Fake implements ProjectServerLike {
  readonly info = { name: 'rcfd', version: '0.0.0-test' };
  exited = false;
  constructor(
    private readonly calls: Call[],
    private readonly kind: 'read' | 'control',
    private readonly answer: (tool: string, args: Record<string, unknown> | undefined) => unknown,
  ) {}
  async call(tool: string, args?: Record<string, unknown>): Promise<unknown> {
    this.calls.push({ server: this.kind, tool, args });
    if (this.kind === 'read') {
      switch (tool as ProjectTool) {
        case 'project_info':
          return { name: 'fake', counts: {}, issues: [], issues_total: 0 };
        case 'list_runs':
          return { runs: [], total: 0, next_offset: null, skipped: [] };
        case 'list_meshes':
          return { meshes: [], total: 0, next_offset: null, unread: [] };
        case 'read_feed':
          return { cursor: 0, entries: [] };
        case 'read_project_file':
          return { path: args?.['path'], hash: 'fnv64:00000000000000aa', size: 3, offset: 0, text: 'a', next_offset: 1 };
        case 'list_trash':
          return {
            items: [{ token: 't-20261117T100000-abcd', kind: 'run', path: 'runs/r-1', trashed: '2026-11-17T10:00:00Z', who: 'trash_run', reason: 'old', hash: null, bytes: 2048 }],
            total: 1,
            bytes: 2048,
            offset: 0,
            next_offset: null,
            skipped: [],
          };
      }
      throw new Error(`the read server was asked ${tool}`);
    }
    return this.answer(tool, args);
  }
  onExit(): void {}
  async close(): Promise<void> {
    this.exited = true;
  }
}

async function openSession(folder: string, answer: (tool: string, args: Record<string, unknown> | undefined) => unknown) {
  const calls: Call[] = [];
  const s = await ProjectSession.open(
    folder,
    '/e/rcfd-mcp',
    {
      startServer: async () => new Fake(calls, 'read', answer),
      listFiles: async () => ({ names: [], total: 0 }),
      now: () => new Date().toISOString(),
      pollMs: 10_000,
      control: { startControlServer: async () => new Fake(calls, 'control', answer), ensureRunner: async () => ({ ok: true }), now: () => new Date().toISOString() },
    },
    () => undefined,
  );
  return { s, calls };
}

test.describe('the project trash in the main process', () => {
  test('the trash tools: write tools on the run-control server, list_trash a read of the read-only server', () => {
    expect([...TRASH_WRITE_TOOLS].sort()).toEqual(Object.values(TRASH_OPS).sort());
    expect(PROJECT_TOOLS).toContain('list_trash');
    expect((PROJECT_TOOLS as readonly string[]).filter((t) => (TRASH_WRITE_TOOLS as readonly string[]).includes(t))).toEqual([]);
  });

  test('each operation: its tool and arguments on the control server; a case with its hash as base; results with the parts there', async () => {
    const folder = mkdtempSync(path.join(tmpdir(), 'rcfd-0736-'));
    try {
      mkdirSync(path.join(folder, 'runs', 'r-1', 'fields'), { recursive: true });
      const { s, calls } = await openSession(folder, (tool) => ({ token: 't-20261117T100000-abcd', tool }));
      try {
        // the trash is in the view, read on the read-only server
        expect(s.view.trash).toMatchObject({ total: 1, bytes: 2048, error: null, items: [{ token: 't-20261117T100000-abcd', kind: 'run', path: 'runs/r-1', reason: 'old', parts: [] }] });
        expect(calls.find((c) => c.tool === 'list_trash')?.server).toBe('read');
        expect(await s.trash('run', { run: 'r-1' })).toMatchObject({ ok: true, result: { token: 't-20261117T100000-abcd' } });
        expect(await s.trash('results', { run: 'r-1' })).toMatchObject({ ok: true });
        expect(await s.trash('case', { path: 'cases/a.toml' })).toMatchObject({ ok: true });
        expect(await s.trash('restore', { token: 't-20261117T100000-abcd' })).toMatchObject({ ok: true });
        expect(await s.trash('empty', { tokens: ['t-20261117T100000-abcd'] })).toMatchObject({ ok: true });
        const control = calls.filter((c) => c.server === 'control').map((c) => [c.tool, c.args]);
        expect(control).toEqual([
          ['trash_run', { run: 'r-1' }],
          // only fields/ is there: output/ is not named (the engine refuses a part the run does not hold)
          ['trash_results', { run: 'r-1', parts: ['fields'] }],
          ['trash_case', { path: 'cases/a.toml', base_hash: 'fnv64:00000000000000aa' }],
          ['restore_trash', { token: 't-20261117T100000-abcd' }],
          ['empty_trash', { tokens: ['t-20261117T100000-abcd'] }],
        ]);
        // the case's hash came from the read-only server, just before
        const read = calls.findIndex((c) => c.tool === 'read_project_file');
        expect(read).toBeGreaterThanOrEqual(0);
        expect(calls[read]!.server).toBe('read');
        // no trash tool ever goes to the read-only server
        expect(calls.filter((c) => c.server === 'read' && (TRASH_WRITE_TOOLS as readonly string[]).includes(c.tool))).toEqual([]);
        // an argument an operation does not take is refused before the engine; a run with no results, in words
        const before = calls.length;
        expect(await s.trash('run', { run: 'r-1', parts: ['fields'] })).toMatchObject({ ok: false, code: 'bad_request' });
        expect(await s.trash('results', { run: '../x' })).toMatchObject({ ok: false, code: -32602 });
        mkdirSync(path.join(folder, 'runs', 'r-2'), { recursive: true });
        const none = await s.trash('results', { run: 'r-2' });
        expect(none).toMatchObject({ ok: false, code: 'nothing' });
        expect(calls.slice(before).filter((c) => c.server === 'control')).toEqual([]);
      } finally {
        await s.close();
      }
    } finally {
      rmSync(folder, { recursive: true, force: true });
    }
  });

  test('the engine’s refusal comes back with its code, message and data (a run another run continues from: 1003)', async () => {
    const folder = mkdtempSync(path.join(tmpdir(), 'rcfd-0736-'));
    try {
      const { s } = await openSession(folder, () => {
        throw new RpcError(1003, 'run r-1 is continued by a run that has not ended', { runs: ['r-2'] });
      });
      try {
        expect(await s.trash('run', { run: 'r-1' })).toEqual({ ok: false, code: 1003, message: 'run r-1 is continued by a run that has not ended', data: { runs: ['r-2'] } });
      } finally {
        await s.close();
      }
    } finally {
      rmSync(folder, { recursive: true, force: true });
    }
  });

  test('the project port takes a trash request of exactly what and params, what one of TRASH_OPS', () => {
    expect(parseProjectRequest({ op: 'trash', id: 1, what: 'run', params: { run: 'r-1' } })).toEqual({ ok: true, request: { op: 'trash', id: 1, what: 'run', params: { run: 'r-1' } } });
    for (const bad of [
      { op: 'trash', id: 1, what: 'delete', params: {} },
      { op: 'trash', id: 1, what: 'toString', params: {} },
      { op: 'trash', id: 1, what: 'run' },
      { op: 'trash', id: 1, what: 'run', params: [] },
      { op: 'trash', id: 1, what: 'run', params: {}, tool: 'empty_trash' },
    ]) {
      expect(parseProjectRequest(bad).ok, JSON.stringify(bad)).toBe(false);
    }
  });
});

test.describe('the trash in the history (F-829)', () => {
  test('an outside step: undo and redo run its own functions; a refusal is a failed outcome and the step stays', async () => {
    const h = new History({ dispatchCall: async () => ({}) as never });
    const ran: string[] = [];
    let refuse = false;
    h.recordStep('project:/p', {
      title: 'move run r-1 to the trash',
      undo: async () => (ran.push('restore'), refuse ? { ok: false, message: 'runs/r-1 is taken' } : { ok: true }),
      redo: async () => (ran.push('trash'), { ok: true }),
    });
    expect(h.view('project:/p')).toMatchObject({ undo: { title: 'move run r-1 to the trash', available: true }, redo: null });
    expect(await h.undo('project:/p')).toEqual({ kind: 'done', direction: 'undo', title: 'move run r-1 to the trash', scope: 'project:/p' });
    expect(await h.redo('project:/p')).toMatchObject({ kind: 'done', direction: 'redo' });
    refuse = true;
    expect(await h.undo('project:/p')).toEqual({ kind: 'failed', direction: 'undo', title: 'move run r-1 to the trash', message: 'runs/r-1 is taken', done: 0 });
    expect(h.view('project:/p').undo?.title).toBe('move run r-1 to the trash');
    expect(ran).toEqual(['restore', 'trash', 'restore']);
  });
});

test.describe('compare-and-set of the settings writes (ADR 0736 P2)', () => {
  test('withBase: a settings write carries the value its step expects; forced, none; other actions as they are', () => {
    const expect_ = [{ path: 'solver.alpha_u', value: 0.5 }];
    const set: ActionCall = { action: 'settings.set', params: { session: 's', path: 'solver.alpha_u', value: 0.9, base_value: 0.1 } };
    expect(withBase(set, expect_, false).params).toEqual({ session: 's', path: 'solver.alpha_u', value: 0.9, base_value: 0.5 });
    expect(withBase(set, expect_, true).params).toEqual({ session: 's', path: 'solver.alpha_u', value: 0.9 });
    expect(withBase(set, [], false).params).toEqual({ session: 's', path: 'solver.alpha_u', value: 0.9 });
    const rm: ActionCall = { action: 'settings.remove', params: { session: 's', path: 'boundaries.lid' } };
    expect(withBase(rm, [{ path: 'boundaries.lid', value: { kind: 'wall' } }], false).params).toEqual({ session: 's', path: 'boundaries.lid', base_value: { kind: 'wall' } });
    const close: ActionCall = { action: 'session.close', params: { session: 's' } };
    expect(withBase(close, expect_, false)).toBe(close);
  });

  test('a stale base is refused with 1007 and the value now; the same call without a base answers as before', async () => {
    const fake = await FakeCaseService.start();
    const t = await portTransport(fake.port);
    try {
      const d = new Dispatcher(t.transport);
      const session = fake.makeSession();
      const stale = await d.dispatchCall({ action: 'settings.set', params: { session, path: 'solver.alpha_u', value: 0.5, base_value: 0.1 } });
      expect(stale.ok).toBe(false);
      expect(!stale.ok && stale.error.code).toBe(SETTINGS_CONFLICT);
      expect(!stale.ok && stale.error.data).toMatchObject({ path: 'solver.alpha_u', current_value: 0.9 });
      expect(fake.sessions.get(session)?.tree['solver']).toMatchObject({ alpha_u: 0.9 });
      const fresh = await d.dispatchCall({ action: 'settings.set', params: { session, path: 'solver.alpha_u', value: 0.5, base_value: 0.9 } });
      const plain = await d.dispatchCall({ action: 'settings.set', params: { session, path: 'solver.alpha_u', value: 0.6 } });
      expect(fresh.ok && fresh.result).toEqual({ path: 'solver.alpha_u', value: 0.5 });
      expect(plain.ok && plain.result).toEqual({ path: 'solver.alpha_u', value: 0.6 });
      // a call with a base has no MCP equivalent (set_setting takes none); one without keeps set_setting
      const set = getAction('settings.set')!;
      expect(equivalents(set, { session, path: 'solver.alpha_u', value: 0.5, base_value: 0.9 }).mcp).toBeNull();
      expect(equivalents(set, { session, path: 'solver.alpha_u', value: 0.6 }).mcp).toEqual({ tool: 'set_setting', arguments: { session, path: 'solver.alpha_u', value: 0.6 } });
    } finally {
      await t.close();
      await fake.stop();
    }
  });

  test('undo: a value another writer set between the history’s read and its write is the engine’s 1007, shown as a conflict', async () => {
    const fake = await FakeCaseService.start();
    const t = await portTransport(fake.port);
    try {
      const d = new Dispatcher(t.transport);
      const session = fake.makeSession();
      // another writer sets the value right after the history read it (the gap the base closes)
      let race = false;
      const h = new History({
        dispatchCall: async (c, o) => {
          const r = await d.dispatchCall(c, o);
          if (race && c.action === 'settings.get') {
            race = false;
            fake.externalSet(session, 'solver.alpha_u', 0.7);
          }
          return r;
        },
      });
      const r = await d.dispatchCall({ action: 'settings.set', params: { session, path: 'solver.alpha_u', value: 0.5 } });
      h.record(r);
      race = true;
      expect(await h.undo(session)).toEqual({
        kind: 'conflict',
        direction: 'undo',
        title: 'set solver.alpha_u',
        scope: session,
        rows: [{ path: 'solver.alpha_u', expected: 0.5, now: 0.7 }],
      });
      expect(fake.sessions.get(session)?.tree['solver']).toMatchObject({ alpha_u: 0.7 });
      // undo anyway: no base, the value is replaced
      expect(await h.undo(session, { force: true })).toMatchObject({ kind: 'done' });
      expect(fake.sessions.get(session)?.tree['solver']).toMatchObject({ alpha_u: 0.9 });
      const sent = fake.received.filter((x) => x.method === 'settings.set').map((x) => x.params['base_value'] ?? 'none');
      expect(sent).toEqual(['none', 0.5, 'none']);
    } finally {
      await t.close();
      await fake.stop();
    }
  });
});
