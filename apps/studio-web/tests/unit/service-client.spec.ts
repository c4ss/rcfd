// F-803: the item of this spec, by the commit that added it (afee363a, 2026-11-16).
// Unit tests of the service client (src/main/service/client.ts, config.ts, spawn.ts) against the fake service: the
// connection state, the events.poll loop, and reconnection after a dropped connection and after a restart.
import { expect, test } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import type { ServiceEvent, ServiceState } from '../../src/shared/service.js';
import { findHead, ServiceClient, type ClientEventBatch } from '../../src/main/service/client.js';
import { backoffMs, parseLoopbackAddress, serviceConfig } from '../../src/main/service/config.js';
import { exe, type EngineChoice } from '../../src/main/engine/locate.js';
import { RpcConnection, RpcError } from '../../src/main/service/rpc.js';
import { parseListening, processAlive, readRecord, removeRecord, writeRecord } from '../../src/main/service/spawn.js';
import { FakeService, until } from '../support/fake-service.js';

/** The engine chosen by the variables (src/main/engine/locate.ts chooseEngine), and a complete engine folder. */
const VARS: EngineChoice = { from: 'variables' };
const SHIPPED: EngineChoice = { from: 'bundled', dir: path.resolve('/res/bin'), missing: [] };
const shippedRcfd = path.join(path.resolve('/res/bin'), exe('rcfd'));

test.describe('configuration', () => {
  test('attach, spawn, off, and the conflicts', () => {
    expect(serviceConfig({}, '/state', { from: 'no-engine' })).toEqual({ mode: 'off', reason: expect.stringContaining('--no-engine') });
    expect(serviceConfig({ RCFD_STUDIO_SERVICE: '127.0.0.1:7878' }, '/state', VARS)).toEqual({ mode: 'attach', host: '127.0.0.1', port: 7878 });
    expect(serviceConfig({ RCFD_STUDIO_SERVICE: '[::1]:9' }, '/state', VARS)).toEqual({ mode: 'attach', host: '::1', port: 9 });
    expect(serviceConfig({ RCFD_STUDIO_RCFD: '/bin/rcfd' }, '/state', VARS)).toEqual({ mode: 'spawn', rcfd: '/bin/rcfd', root: null, stateDir: '/state', start: 'launch' });
    expect(
      serviceConfig({ RCFD_STUDIO_RCFD: '/bin/rcfd', RCFD_STUDIO_SERVICE_ROOT: '/cases', RCFD_STUDIO_STATE_DIR: '/s2' }, '/state', VARS),
    ).toEqual({ mode: 'spawn', rcfd: '/bin/rcfd', root: '/cases', stateDir: '/s2', start: 'launch' });
    expect(serviceConfig({ RCFD_STUDIO_SERVICE: '127.0.0.1:1', RCFD_STUDIO_RCFD: 'x' }, '/s', VARS).mode).toBe('invalid');
  });

  test('F-820: an installed Studio starts its shipped rcfd with the first project; the variables still win; never PATH', () => {
    expect(serviceConfig({}, '/state', SHIPPED)).toEqual({ mode: 'spawn', rcfd: shippedRcfd, root: null, stateDir: '/state', start: 'project' });
    expect(serviceConfig({ RCFD_STUDIO_SERVICE_ROOT: '/cases', RCFD_STUDIO_STATE_DIR: '/s2' }, '/state', SHIPPED)).toEqual({
      mode: 'spawn',
      rcfd: shippedRcfd,
      root: '/cases',
      stateDir: '/s2',
      start: 'project',
    });
    // a variable names the engine: chooseEngine gives `variables`, and the shipped folder is not used
    expect(serviceConfig({ RCFD_STUDIO_RCFD: '/bin/rcfd' }, '/state', VARS)).toMatchObject({ rcfd: '/bin/rcfd', start: 'launch' });
    // an attach replaces the service the Studio would start, whatever the engine
    expect(serviceConfig({ RCFD_STUDIO_SERVICE: '127.0.0.1:7878' }, '/state', SHIPPED)).toEqual({ mode: 'attach', host: '127.0.0.1', port: 7878 });
    // a shipped folder without the whole engine: off, saying what is missing
    const broken: EngineChoice = { from: 'bundled', dir: path.resolve('/res/bin'), missing: [{ name: 'rcfd', pkg: 'rcfd-cli' }] };
    expect(serviceConfig({ PATH: '/bin' }, '/state', broken)).toEqual({ mode: 'off', reason: expect.stringContaining(exe('rcfd')) });
  });

  test('only loopback addresses are accepted (G10)', () => {
    expect(parseLoopbackAddress('localhost:80')).toEqual({ host: 'localhost', port: 80 });
    for (const bad of ['10.0.0.2:7878', '0.0.0.0:7878', 'example.com:7878', '127.0.0.1', '127.0.0.1:0', '127.0.0.1:70000', ':7878']) {
      expect('error' in parseLoopbackAddress(bad), bad).toBe(true);
    }
    const c = serviceConfig({ RCFD_STUDIO_SERVICE: '192.168.1.5:7878' }, '/s', VARS);
    expect(c).toEqual({ mode: 'invalid', reason: expect.stringContaining('not a loopback address') });
  });

  test('back-off: 100 ms doubling to at most 5 s', () => {
    expect([0, 1, 2, 3, 4, 5, 6, 7, 30].map(backoffMs)).toEqual([100, 200, 400, 800, 1600, 3200, 5000, 5000, 5000]);
  });

  test('the address rcfd serve prints', () => {
    expect(parseListening('rcfd service listening on 127.0.0.1:50601\n')).toEqual({ host: '127.0.0.1', port: 50601 });
    expect(parseListening('rcfd service listening on [::1]:9\n')).toEqual({ host: '::1', port: 9 });
    expect(parseListening('error: cannot listen')).toBeNull();
  });

  test('the record of a started service, and whether a process lives', async () => {
    const dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f803-'));
    try {
      expect(await readRecord(dir)).toBeNull();
      const r = { host: '127.0.0.1', port: 5, pid: process.pid, rcfd: 'rcfd', startedAt: 'now' };
      await writeRecord(dir, r);
      expect(await readRecord(dir)).toEqual(r);
      await removeRecord(dir);
      expect(await readRecord(dir)).toBeNull();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
    expect(processAlive(process.pid)).toBe(true);
    expect(processAlive(2 ** 22 + 12345)).toBe(false);
  });
});

test.describe('findHead', () => {
  for (const n of [0, 1, 2, 3, 7, 8, 9, 100, 1000]) {
    test(`finds the newest sequence number of a log of ${n} events`, async () => {
      const fake = await FakeService.start({ capacity: 50 });
      for (let i = 0; i < n; i++) fake.emit('s', 'iteration');
      const c = await RpcConnection.connectTcp('127.0.0.1', fake.port);
      expect(await findHead(c)).toBe(n);
      c.close();
      await fake.stop();
    });
  }
});

/** A client on the fake, with the batches and states it reported. */
function watch(client: ServiceClient) {
  const batches: ClientEventBatch[] = [];
  const states: ServiceState[] = [];
  client.onEvents((b) => batches.push(b));
  client.onState((s) => states.push(s));
  const events = (): ServiceEvent[] => batches.flatMap((b) => b.events);
  return { batches, states, events };
}

const fast = { backoff: () => 30, pollTimeoutMs: 2000, connectTimeoutMs: 1000 };

test.describe('ServiceClient (attach mode, fake service)', () => {
  test('connects, starts at the head of the log, and delivers later events in order', async () => {
    const fake = await FakeService.start();
    fake.emit('old', 'created'); // before the client: not delivered
    const client = new ServiceClient({ mode: 'attach', host: '127.0.0.1', port: fake.port }, fast);
    const w = watch(client);
    client.start();
    await until(() => client.state.status === 'connected', 5000, 'connected');
    expect(client.state).toMatchObject({ mode: 'attach', address: `127.0.0.1:${fake.port}`, instance: 1, version: '0.0.0', protocolVersion: '1.1', attempts: 0 });
    // two connections: control and events
    expect(fake.connections).toBe(2);
    const r = (await client.call('session.create')) as { session: string };
    fake.emit(r.session, 'iteration', { index: 1 });
    fake.emit(r.session, 'iteration', { index: 2 });
    await until(() => w.events().length === 3, 5000, 'three events');
    expect(w.events().map((e) => [e.seq, e.kind])).toEqual([
      [2, 'created'],
      [3, 'iteration'],
      [4, 'iteration'],
    ]);
    expect(w.batches.every((b) => b.instance === 1 && b.missed === 0)).toBe(true);
    await client.shutdown();
    expect(client.state.status).toBe('stopped');
    await fake.stop();
  });

  test('a pending long-poll does not hold up calls', async () => {
    const fake = await FakeService.start();
    const client = new ServiceClient({ mode: 'attach', host: '127.0.0.1', port: fake.port }, { ...fast, pollTimeoutMs: 30_000 });
    client.start();
    await until(() => client.state.status === 'connected', 5000, 'connected');
    await until(() => fake.received.filter((r) => r.method === 'events.poll' && (r.params as { timeout_ms: number }).timeout_ms === 30_000).length === 1, 5000, 'a long poll');
    const t = performance.now();
    for (let i = 0; i < 20; i++) await client.call('settings.get', { i });
    expect(performance.now() - t).toBeLessThan(2000);
    await client.shutdown();
    await fake.stop();
  });

  test('without a connection a call fails at once; without a configuration it says so', async () => {
    const dead = await FakeService.start();
    const port = dead.port;
    await dead.stop();
    const client = new ServiceClient({ mode: 'attach', host: '127.0.0.1', port }, fast);
    const w = watch(client);
    client.start();
    await until(() => client.state.status === 'disconnected', 5000, 'disconnected');
    const t = performance.now();
    const e = (await client.call('service.info').catch((x: unknown) => x)) as RpcError;
    expect(performance.now() - t).toBeLessThan(50);
    expect(e.code).toBe('disconnected');
    expect(client.state.lastError).toContain('cannot connect');
    await until(() => client.state.attempts >= 3, 5000, 'retries');
    expect(w.states.some((s) => s.retryInMs === 30)).toBe(true);
    await client.shutdown();

    const off = new ServiceClient({ mode: 'off', reason: 'The Studio was started without an engine (--no-engine).' });
    off.start();
    expect(off.state.status).toBe('off');
    // the reason is in the state, for the settings page and the actions' availability
    expect(off.state.lastError).toBe('The Studio was started without an engine (--no-engine).');
    const offError = (await off.call('service.info').catch((x: unknown) => x)) as RpcError;
    expect(offError.code).toBe('not_configured');
    expect(offError.message).toBe('No rcfd service: The Studio was started without an engine (--no-engine).');
    const invalid = new ServiceClient({ mode: 'invalid', reason: 'RCFD_STUDIO_SERVICE: nope' });
    expect(invalid.state.lastError).toBe('RCFD_STUDIO_SERVICE: nope');
    expect(((await invalid.call('service.info').catch((x: unknown) => x)) as RpcError).message).toContain('nope');
  });

  test('F-820: waiting for a project; findRunning() connects only to a service an earlier start left, and starts nothing', async () => {
    const stateDir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f820-'));
    // a process that stands for the service an earlier start left (the fake answers on its own port)
    const holder = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 60000)'], { stdio: 'ignore' });
    const fake = await FakeService.start();
    try {
      const cfg = { mode: 'spawn', rcfd: path.join(stateDir, 'no-such-rcfd'), root: null, stateDir, start: 'project' } as const;
      const waiting = new ServiceClient(cfg, fast);
      expect(waiting.state).toMatchObject({ status: 'waiting', mode: 'spawn', pid: null });
      expect(((await waiting.call('service.info').catch((x: unknown) => x)) as RpcError).message).toContain('starts when a project is opened');
      // no record: nothing found, nothing started
      expect(await waiting.findRunning()).toBe(false);
      expect(waiting.state.status).toBe('waiting');
      await waiting.shutdown();

      await writeRecord(stateDir, { host: '127.0.0.1', port: fake.port, pid: holder.pid!, rcfd: cfg.rcfd, startedAt: new Date().toISOString() });
      const client = new ServiceClient(cfg, fast);
      expect(await client.findRunning()).toBe(true);
      await until(() => client.state.status === 'connected', 5000, 'connected to the service left running');
      expect(client.state).toMatchObject({ pid: holder.pid, adopted: true });
      client.start(); // the first project: already connected, nothing more happens
      expect(client.state.status).toBe('connected');
      // it holds a session: left running (G7)
      await client.call('session.create', {});
      expect(await client.shutdown()).toEqual({ leftRunning: true });
    } finally {
      holder.kill();
      await fake.stop();
      rmSync(stateDir, { recursive: true, force: true });
    }
  });

  test('a service of another protocol is refused', async () => {
    const fake = await FakeService.start({ protocol: 'something-else/9' });
    const client = new ServiceClient({ mode: 'attach', host: '127.0.0.1', port: fake.port }, fast);
    client.start();
    await until(() => client.state.status === 'disconnected', 5000, 'disconnected');
    expect(client.state.lastError).toContain('not rcfd-service/1');
    await client.shutdown();
    await fake.stop();
  });

  test('a dropped connection to the same service: reconnects, same instance, events in between delivered', async () => {
    const fake = await FakeService.start();
    const client = new ServiceClient({ mode: 'attach', host: '127.0.0.1', port: fake.port }, fast);
    const w = watch(client);
    client.start();
    await until(() => client.state.status === 'connected', 5000, 'connected');
    fake.emit('a', 'iteration', { index: 1 });
    await until(() => w.events().length === 1, 5000, 'the first event');
    fake.dropConnections();
    fake.emit('a', 'iteration', { index: 2 }); // while disconnected
    await until(() => w.states.some((s) => s.status === 'disconnected'), 5000, 'disconnected');
    await until(() => client.state.status === 'connected', 5000, 'reconnected');
    fake.emit('a', 'iteration', { index: 3 });
    await until(() => w.events().length === 3, 5000, 'all three events');
    expect(w.events().map((e) => (e.data as { index: number }).index)).toEqual([1, 2, 3]);
    expect(client.state.instance).toBe(1);
    await client.shutdown();
    await fake.stop();
  });

  test('a restarted service on the same port: a new instance, its events from its first one', async () => {
    let fake = await FakeService.start();
    const port = fake.port;
    const client = new ServiceClient({ mode: 'attach', host: '127.0.0.1', port }, fast);
    const w = watch(client);
    client.start();
    await until(() => client.state.status === 'connected', 5000, 'connected');
    for (let i = 1; i <= 5; i++) fake.emit('a', 'iteration', { index: i });
    await until(() => w.events().length === 5, 5000, 'five events');
    await fake.stop();
    await until(() => client.state.status === 'disconnected', 5000, 'disconnected');
    // a new service with a shorter log than the cursor
    fake = await FakeService.start({ port });
    fake.emit('b', 'created');
    await until(() => client.state.status === 'connected', 5000, 'reconnected');
    expect(client.state.instance).toBe(2);
    fake.emit('b', 'iteration', { index: 1 });
    await until(() => w.events().length === 7, 5000, 'the new instance events');
    expect(w.events().slice(5).map((e) => [e.seq, e.session, e.kind])).toEqual([
      [1, 'b', 'created'],
      [2, 'b', 'iteration'],
    ]);
    expect(w.batches.at(-1)!.instance).toBe(2);
    await client.shutdown();
    await fake.stop();
  });

  test('a restarted service whose log has grown past the cursor is still told apart by the last event seen', async () => {
    let fake = await FakeService.start();
    const port = fake.port;
    const client = new ServiceClient({ mode: 'attach', host: '127.0.0.1', port }, fast);
    const w = watch(client);
    client.start();
    await until(() => client.state.status === 'connected', 5000, 'connected');
    fake.emit('a', 'iteration', { index: 1 });
    fake.emit('a', 'iteration', { index: 2 });
    await until(() => w.events().length === 2, 5000, 'two events');
    await fake.stop();
    await until(() => client.state.status === 'disconnected', 5000, 'disconnected');
    fake = await FakeService.start({ port });
    for (let i = 0; i < 4; i++) fake.emit('b', 'iteration', { index: 10 + i }); // seq 1..4, past the old cursor 2
    await until(() => client.state.status === 'connected', 5000, 'reconnected');
    await until(() => w.events().length === 6, 5000, 'the new instance events');
    expect(client.state.instance).toBe(2);
    expect(w.events().slice(2).map((e) => e.seq)).toEqual([1, 2, 3, 4]);
    await client.shutdown();
    await fake.stop();
  });

  test('events dropped from the bounded log while disconnected are counted as missed', async () => {
    const fake = await FakeService.start({ capacity: 3 });
    const client = new ServiceClient({ mode: 'attach', host: '127.0.0.1', port: fake.port }, { ...fast, backoff: () => 300 });
    const w = watch(client);
    client.start();
    await until(() => client.state.status === 'connected', 5000, 'connected');
    fake.emit('a', 'iteration', { index: 1 });
    await until(() => w.events().length === 1, 5000, 'the first event');
    fake.dropConnections();
    await until(() => client.state.status === 'disconnected', 5000, 'disconnected');
    for (let i = 2; i <= 10; i++) fake.emit('a', 'iteration', { index: i }); // the log keeps 8, 9, 10
    await until(() => client.state.status === 'connected', 5000, 'reconnected');
    await until(() => w.events().length === 4, 5000, 'the retained events');
    expect(client.state.instance).toBe(1);
    const last = w.batches.at(-1)!;
    expect(last.events.map((e) => e.seq)).toEqual([8, 9, 10]);
    expect(last.missed).toBe(6);
    await client.shutdown();
    await fake.stop();
  });
});
