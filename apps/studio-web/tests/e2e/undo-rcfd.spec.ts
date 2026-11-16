// Undo and redo against the real `rcfd serve` (F-829, G18): the history of src/shared/actions/history.ts on the
// dispatcher, through the main process's client and port bridge (tests/support/action-transport.ts), as in the Studio.
// Round trips of settings.set, settings.remove, a named entry made from its first key and a new session; a value set
// by another writer (a second connection, as `rcfd call` or Claude Code would) is a conflict and stays; an undo that
// would discard results asks first. No Electron. The same against the fake service: tests/unit/history.spec.ts.
//
// Needs the rcfd binary (tests/support/rcfd.ts); without it the tests are skipped and say why.
import { expect, test } from '@playwright/test';
import { Dispatcher, History, type RecordOptions } from '../../src/shared/actions/index.js';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { portTransport, type PortTransport } from '../support/action-transport.js';
import { boxToml, findRcfd, killIfRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';

const rcfd = findRcfd();
test.skip(rcfd === null, NO_RCFD);

const leftovers = new Set<number>();
test.afterAll(() => {
  for (const pid of leftovers) killIfRcfd(pid);
});

interface Rig {
  s: RunningRcfd;
  t: PortTransport;
  d: Dispatcher;
  h: History;
  /** Another writer: its own connection to the same service. */
  other: RpcConnection;
  ui(action: string, params: Record<string, unknown>, undo?: RecordOptions): Promise<unknown>;
  get(session: string, p: string): Promise<unknown>;
  close(): Promise<void>;
}

async function rig(): Promise<Rig> {
  const s = await startRcfd(rcfd!);
  leftovers.add(s.pid);
  const t = await portTransport(s.port);
  const d = new Dispatcher(t.transport);
  const h = new History(d);
  const other = await RpcConnection.connectTcp('127.0.0.1', s.port);
  return {
    s,
    t,
    d,
    h,
    other,
    ui: async (action, params, undo) => {
      const r = await d.dispatchCall({ action, params });
      h.record(r, undo);
      if (!r.ok) throw new Error(`${action}: ${r.error.message}`);
      return r.result;
    },
    get: async (session, p) => {
      try {
        return ((await other.request('settings.get', { session, path: p })) as { value: unknown }).value;
      } catch (e) {
        return { absent: String(e) };
      }
    },
    close: async () => {
      other.close();
      await t.close();
      await s.stop();
    },
  };
}

test.describe('undo and redo on the real service', () => {
  test('settings.set, settings.remove and a new named entry round trip; the log holds the undo and redo calls', async () => {
    const r = await rig();
    try {
      const { session } = (await r.ui('session.create', { toml: boxToml([3, 3, 3]) })) as { session: string };
      expect(await r.get(session, 'physics.nu')).toBe(0.01);

      await r.ui('settings.set', { session, path: 'physics.nu', value: 0.05 });
      expect(r.h.view(session).undo?.title).toBe('set physics.nu');
      expect(await r.h.undo(session)).toMatchObject({ kind: 'done' });
      expect(await r.get(session, 'physics.nu')).toBe(0.01);
      expect(await r.h.redo(session)).toMatchObject({ kind: 'done' });
      expect(await r.get(session, 'physics.nu')).toBe(0.05);

      const lid = await r.get(session, 'boundaries.ymax');
      await r.ui('settings.remove', { session, path: 'boundaries.ymax' });
      expect(await r.get(session, 'boundaries.ymax')).toMatchObject({ absent: expect.any(String) });
      expect(await r.h.undo(session)).toMatchObject({ kind: 'done' });
      expect(await r.get(session, 'boundaries.ymax')).toEqual(lid);
      expect(await r.h.redo(session)).toMatchObject({ kind: 'done' });
      expect(await r.get(session, 'boundaries.ymax')).toMatchObject({ absent: expect.any(String) });
      expect(await r.h.undo(session)).toMatchObject({ kind: 'done' });

      // a named entry made by its first key: the engine has no old value, so the registry gives no inverse; the
      // settings surface's inverse (remove the entry it made) takes it back
      const plain = await r.d.dispatchCall({ action: 'settings.set', params: { session, path: 'boundaries.probe.velocity', value: 'fixed' } });
      expect(plain.ok && plain.entry.inverse).toBeNull();
      await r.d.dispatchCall({ action: 'settings.remove', params: { session, path: 'boundaries.probe' } });
      await r.ui('settings.set', { session, path: 'boundaries.probe.velocity', value: 'fixed' }, {
        inverse: [{ action: 'settings.remove', params: { session, path: 'boundaries.probe' } }],
        title: 'add boundaries.probe',
      });
      expect(await r.h.undo(session)).toMatchObject({ kind: 'done', title: 'add boundaries.probe' });
      expect(await r.get(session, 'boundaries.probe')).toMatchObject({ absent: expect.any(String) });
      expect(await r.h.redo(session)).toMatchObject({ kind: 'done' });
      expect(await r.get(session, 'boundaries.probe')).toMatchObject({ velocity: 'fixed' });

      const v = await r.d.dispatchCall({ action: 'settings.validate', params: { session } });
      expect(v.ok).toBe(true);
      const changes = r.d.log.entries().filter((e) => e.effect === 'change' && e.origin !== 'ui').map((e) => `${e.origin} ${e.action}`);
      expect(changes.filter((c) => c.startsWith('undo')).length).toBeGreaterThan(3);
      expect(changes.filter((c) => c.startsWith('redo')).length).toBe(3);
    } finally {
      await r.close();
    }
  });

  test('undoing a new session asks first, then closes it; redo makes it again from the same case text', async () => {
    const r = await rig();
    try {
      const { session } = (await r.ui('session.create', { toml: boxToml([3, 3, 3]) })) as { session: string };
      await r.ui('settings.set', { session, path: 'physics.nu', value: 0.02 });
      await r.h.undo(session);
      const ask = await r.h.undo(session);
      expect(ask).toMatchObject({ kind: 'confirm', title: `new session ${session}` });
      expect(ask.kind === 'confirm' && ask.loss).toContain(`Session ${session} is removed from the service.`);
      const listed = (await r.other.request('session.list')) as { sessions: { session: string }[] };
      expect(listed.sessions.map((x) => x.session)).toContain(session);
      expect(await r.h.undo(session, { confirmed: true })).toMatchObject({ kind: 'done' });
      const after = (await r.other.request('session.list')) as { sessions: { session: string }[] };
      expect(after.sessions.map((x) => x.session)).not.toContain(session);
      const again = await r.h.redo(session);
      const made = again.kind === 'done' ? again.scope : '';
      expect(made).not.toBe('');
      expect(made).not.toBe(session);
      expect(await r.get(made, 'physics.nu')).toBe(0.01);
      expect(await r.h.redo(made)).toMatchObject({ kind: 'done' });
      expect(await r.get(made, 'physics.nu')).toBe(0.02);
    } finally {
      await r.close();
    }
  });

  test('another writer’s value is a conflict: nothing is overwritten until the user chooses', async () => {
    const r = await rig();
    try {
      const { session } = (await r.ui('session.create', { toml: boxToml([3, 3, 3]) })) as { session: string };
      await r.ui('settings.set', { session, path: 'physics.nu', value: 0.05 });
      await r.other.request('settings.set', { session, path: 'physics.nu', value: 0.07 });
      expect(await r.h.undo(session)).toMatchObject({ kind: 'conflict', rows: [{ path: 'physics.nu', expected: 0.05, now: 0.07 }] });
      expect(await r.get(session, 'physics.nu')).toBe(0.07);
      expect(await r.h.undo(session, { force: true })).toMatchObject({ kind: 'done' });
      expect(await r.get(session, 'physics.nu')).toBe(0.01);
    } finally {
      await r.close();
    }
  });

  test('an undo that would discard results asks first and names them', async () => {
    const r = await rig();
    try {
      const { session } = (await r.ui('session.create', { toml: boxToml([3, 3, 3]) })) as { session: string };
      await r.ui('settings.set', { session, path: 'physics.nu', value: 0.05 });
      const step = await r.d.dispatchCall({ action: 'run.step', params: { session, count: 2 } });
      expect(step.ok).toBe(true);
      const ask = await r.h.undo(session);
      expect(ask).toMatchObject({ kind: 'confirm' });
      expect(ask.kind === 'confirm' && ask.loss).toMatch(new RegExp(`^A settings change returns session ${session} to defined and discards its built simulation and its results \\(\\w+, iteration 2\\)\\.$`));
      expect(await r.get(session, 'physics.nu')).toBe(0.05);
      expect(await r.h.undo(session, { confirmed: true })).toMatchObject({ kind: 'done' });
      expect(await r.get(session, 'physics.nu')).toBe(0.01);
    } finally {
      await r.close();
    }
  });
});
