// The action layer against the real `rcfd serve` (F-827; G16, G17, G18): every action's method is one the service
// offers; actions recorded on one service and replayed on another, from the JSON script through the dispatcher and from
// the shell script through `rcfd call`, give the same case (journal hash) and the same run state, residuals included;
// the undo hooks of settings.set and settings.remove restore what was there. No Electron: the requests cross the main
// process's client and port bridge as in the Studio (tests/support/action-transport.ts).
//
// Needs the rcfd binary (tests/support/rcfd.ts); without it the tests are skipped and say why. The shell replay also
// needs `sh` and `sed` on PATH; without them that part is not run and the test carries a 'not run' annotation.
import { expect, test } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { ACTIONS, Dispatcher, type ActionScript } from '../../src/shared/actions/index.js';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { portTransport } from '../support/action-transport.js';
import { boxToml, findRcfd, killIfRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';

const rcfd = findRcfd();
test.skip(rcfd === null, NO_RCFD);

const leftovers = new Set<number>();
const scratch: string[] = [];
test.afterAll(() => {
  for (const pid of leftovers) killIfRcfd(pid);
  for (const d of scratch) rmSync(d, { recursive: true, force: true });
});

async function serve(): Promise<RunningRcfd> {
  const s = await startRcfd(rcfd!);
  leftovers.add(s.pid);
  return s;
}

/** journal.get's hash and run.status without the session id: what a replay must reproduce. */
async function outcome(port: number, session: string) {
  const c = await RpcConnection.connectTcp('127.0.0.1', port);
  try {
    const journal = (await c.request('journal.get', { session })) as { case_hash: string; entries: unknown[] };
    const { session: _id, ...status } = (await c.request('run.status', { session })) as Record<string, unknown>;
    return { case_hash: journal.case_hash, entries: journal.entries.length, status };
  } finally {
    c.close();
  }
}

test.describe('actions on the real service', () => {
  test('every action calls a method the service offers (data.info only with data.* on)', async () => {
    const s = await serve();
    const c = await RpcConnection.connectTcp('127.0.0.1', s.port);
    const info = (await c.request('service.info')) as { methods: string[] };
    c.close();
    await s.stop();
    expect(ACTIONS.map((a) => a.engine.service).filter((m) => !info.methods.includes(m))).toEqual(['data.info']);
  });

  test('recorded on one service, replayed on another from JSON and from the shell: the same case and run (G16, G17)', async () => {
    const a = await serve();
    let script: ActionScript;
    let shell: string;
    let want: Awaited<ReturnType<typeof outcome>>;
    {
      const t = await portTransport(a.port);
      try {
        const d = new Dispatcher(t.transport);
        const created = await d.dispatch('session.create', { toml: boxToml([4, 4, 4]) });
        expect(created.ok).toBe(true);
        const session = created.ok ? (created.result as { session: string }).session : '';
        for (const r of [
          await d.dispatch('settings.set', { session, path: 'physics.nu', value: 0.02 }),
          await d.dispatch('settings.set', { session, path: 'solver.convection', value: 'upwind' }),
          await d.dispatch('run.step', { session, count: 3 }),
          await d.dispatch('run.status', { session }),
          await d.dispatch('run.step', { session, count: 2 }),
        ]) {
          expect(r.ok, JSON.stringify(r.ok ? null : r.error)).toBe(true);
        }
        want = await outcome(a.port, session);
        expect(want.status).toMatchObject({ state: 'ready', simulation: { iteration: 5 } });
        script = JSON.parse(JSON.stringify(d.exportScript({ address: t.address })));
        shell = d.exportShell({ address: t.address });
        expect(script.steps.map((s) => s.action)).toEqual(['session.create', 'settings.set', 'settings.set', 'run.step', 'run.step']);
      } finally {
        await t.close();
        await a.stop();
      }
    }

    // the JSON script through a dispatcher, on a service that already holds a session
    const b = await serve();
    try {
      const t = await portTransport(b.port);
      try {
        await t.client.call('session.create', {});
        const r = await new Dispatcher(t.transport).replay(script);
        expect(r.ok, JSON.stringify(r)).toBe(true);
        expect(r.sessions).toEqual({ s1: 's2' });
        expect(await outcome(b.port, 's2')).toEqual(want);
      } finally {
        await t.close();
      }
    } finally {
      await b.stop();
    }

    // the shell script: one `rcfd call` per action
    const sh = spawnSync('sh', ['-c', 'command -v sed >/dev/null && echo ok'], { encoding: 'utf8' });
    if (sh.error !== undefined || sh.stdout.trim() !== 'ok') {
      test.info().annotations.push({ type: 'not run', description: 'no POSIX sh with sed on PATH: the shell replay was not run' });
      return;
    }
    const c = await serve();
    try {
      const dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f827-'));
      scratch.push(dir);
      const file = path.join(dir, 'actions.sh');
      writeFileSync(file, shell);
      const run = spawnSync('sh', [file], {
        encoding: 'utf8',
        env: { ...process.env, RCFD_ADDR: `127.0.0.1:${c.port}`, PATH: `${path.dirname(rcfd!)}${path.delimiter}${process.env['PATH'] ?? ''}` },
        timeout: 120_000,
      });
      expect(run.status, `${run.stdout}\n${run.stderr}`).toBe(0);
      expect(await outcome(c.port, 's1')).toEqual(want);
    } finally {
      await c.stop();
    }
  });

  test('undo hooks: the inverse of settings.set and settings.remove restores the old value; of session.create closes it (G18)', async () => {
    const s = await serve();
    const t = await portTransport(s.port);
    try {
      const d = new Dispatcher(t.transport);
      const created = await d.dispatch('session.create', { toml: boxToml([3, 3, 3]) });
      const session = created.ok ? (created.result as { session: string }).session : '';
      const get = async (p: string) => {
        const r = await d.dispatch('settings.get', { session, path: p });
        return r.ok ? (r.result as { value: unknown }).value : r.error;
      };
      /** Dispatches the inverse of an entry in order (what F-829's undo will do): whether each step succeeded. */
      const undo = async (steps: readonly { action: string; params: object }[] | null | undefined) => {
        const out: boolean[] = [];
        for (const c of steps ?? []) {
          const r = await d.dispatchCall(c, { origin: 'undo' });
          out.push(r.ok);
          if (!r.ok) console.log(JSON.stringify(r.error));
        }
        return out;
      };
      const nu = await get('physics.nu');
      const lid = await get('boundaries.ymax');
      expect(nu).toBe(0.01);
      expect(lid).toMatchObject({ velocity: 'fixed', velocity_value: [1, 0, 0] });

      const set = await d.dispatch('settings.set', { session, path: 'physics.nu', value: 0.05 });
      expect(set.entry.inverse).toEqual([{ action: 'settings.set', params: { session, path: 'physics.nu', value: 0.01 } }]);
      expect(await get('physics.nu')).toBe(0.05);
      expect(await undo(set.entry.inverse)).toEqual([true]);
      expect(await get('physics.nu')).toBe(0.01);

      const removed = await d.dispatch('settings.remove', { session, path: 'boundaries.ymax' });
      expect(removed.ok).toBe(true);
      expect(removed.entry.inverse).toEqual(
        Object.entries(lid as object).map(([k, value]) => ({ action: 'settings.set', params: { session, path: `boundaries.ymax.${k}`, value } })),
      );
      expect(await get('boundaries.ymax')).toMatchObject({ code: -32602 });
      expect(await undo(removed.entry.inverse)).toEqual(Object.keys(lid as object).map(() => true));
      expect(await get('boundaries.ymax')).toEqual(lid);
      const v = await d.dispatch('settings.validate', { session });
      expect(v).toMatchObject({ ok: true, result: { ok: true } });

      expect(await undo(created.entry.inverse)).toEqual([true]);
      expect(d.log.entries().at(-1)).toMatchObject({ action: 'session.close', params: { session }, origin: 'undo', ok: true });
      expect(d.log.entries().filter((e) => e.origin === 'undo').map((e) => e.action)).toEqual([
        'settings.set',
        ...Object.keys(lid as object).map(() => 'settings.set'),
        'session.close',
      ]);
    } finally {
      await t.close();
      await s.stop();
    }
  });
});
