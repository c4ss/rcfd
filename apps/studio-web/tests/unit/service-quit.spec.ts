// F-892, ADR 0789 (T1): the service client's shutdown ends the service it is starting. The real ServiceClient and the
// real spawning function of src/main/service/spawn.ts (startProcess), with a Node process standing in for `rcfd serve`:
// one that prints the listening line only after a wait (W1: spawned, not yet listening), and one that listens at once
// and never answers (W2: listening, not yet connected). A Studio that quit in either window left the service running,
// detached, with nothing that knew its pid; on Windows Playwright's close() then waited for ever on the pipes the
// service had inherited, the test ran into its time-out, and the process stayed after `npm test`.
import { expect, test } from '@playwright/test';
import { spawn, type ChildProcess } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { ServiceClient } from '../../src/main/service/client.js';
import { processAlive, startProcess, writeRecord, type StartHooks, type StartedService } from '../../src/main/service/spawn.js';
import { FakeService, until } from '../support/fake-service.js';

const fast = { connectTimeoutMs: 1000, backoff: () => 50 };

/** Prints the listening line of a port nobody listens on, after `waitMs`, and stays. */
const slow = (waitMs: number): string =>
  `setTimeout(() => console.log('rcfd service listening on 127.0.0.1:9'), ${waitMs}); setInterval(() => {}, 1000);`;
/** Listens at once, prints its address, accepts connections and never answers. */
const SILENT =
  "const s = require('node:net').createServer(() => {}); s.listen(0, '127.0.0.1', () => console.log('rcfd service listening on 127.0.0.1:' + s.address().port));";

/** A start function for the client: startProcess itself, with `node -e <script>` as the command; every child is kept. */
function standIn(script: string, children: ChildProcess[]) {
  return (_rcfd: string, _root: string | null, stateDir: string, timeoutMs?: number, hooks: StartHooks = {}): Promise<StartedService> =>
    startProcess(process.execPath, ['-e', script], stateDir, timeoutMs, {
      ...hooks,
      onSpawned: (c) => {
        children.push(c);
        hooks.onSpawned?.(c);
      },
    });
}

const config = (stateDir: string) => ({ mode: 'spawn', rcfd: path.join(stateDir, 'stand-in'), root: null, stateDir, start: 'launch' }) as const;
const gone = (c: ChildProcess) => c.pid !== undefined && !processAlive(c.pid);

async function withState(f: (stateDir: string, children: ChildProcess[]) => Promise<void>): Promise<void> {
  const stateDir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f892-'));
  const children: ChildProcess[] = [];
  try {
    await f(stateDir, children);
  } finally {
    for (const c of children) c.kill();
    await until(() => children.every((c) => c.exitCode !== null || c.signalCode !== null || gone(c)), 5000, 'the stand-ins to end').catch(() => undefined);
    rmSync(stateDir, { recursive: true, force: true, maxRetries: 5 });
  }
}

test('(a) W1: shutdown while the service has not reported its address ends the process that was spawned', async () => {
  await withState(async (stateDir, children) => {
    const client = new ServiceClient(config(stateDir), { ...fast, startService: standIn(slow(8000), children) });
    client.start();
    await until(() => children.length === 1, 5000, 'the stand-in to be spawned');
    expect(client.state).toMatchObject({ status: 'starting', pid: null });
    expect(processAlive(children[0]!.pid!)).toBe(true);
    expect(await client.shutdown({ keepWithSessions: false })).toEqual({ leftRunning: false });
    await until(() => gone(children[0]!), 5000, 'the stand-in to be gone');
    expect(existsSync(path.join(stateDir, 'service.json'))).toBe(false);
  });
});

for (const keepWithSessions of [false, true]) {
  test(`(b) W2: shutdown once the service listens and before the client is connected ends it (keepWithSessions ${keepWithSessions})`, async () => {
    await withState(async (stateDir, children) => {
      const client = new ServiceClient(config(stateDir), { ...fast, startService: standIn(SILENT, children) });
      client.start();
      await until(() => client.state.pid !== null && existsSync(path.join(stateDir, 'service.json')), 10_000, 'the pid and the record');
      expect(client.state.status).not.toBe('connected');
      expect(client.state).toMatchObject({ pid: children[0]!.pid, adopted: false });
      expect(await client.shutdown({ keepWithSessions })).toEqual({ leftRunning: false });
      await until(() => gone(children[0]!), 5000, 'the stand-in to be gone');
      expect(existsSync(path.join(stateDir, 'service.json'))).toBe(false);
      expect(children).toHaveLength(1);
    });
  });
}

test('(c) shutdown in the same turn as start: nothing is spawned, and no stand-in is alive 2 s later', async () => {
  await withState(async (stateDir, children) => {
    const client = new ServiceClient(config(stateDir), { ...fast, startService: standIn(slow(8000), children) });
    client.start();
    const left = client.shutdown({ keepWithSessions: false });
    expect(await left).toEqual({ leftRunning: false });
    await new Promise((r) => setTimeout(r, 2000));
    expect(children.filter((c) => !gone(c))).toHaveLength(0);
    expect(children).toHaveLength(0);
  });
});

test('(d) G7 kept: a service found running that the client is not yet connected to is left running', async () => {
  const stateDir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f892-'));
  // a process that stands for the service an earlier start left (the fake answers on its own port)
  const holder = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 60000)'], { stdio: 'ignore' });
  const fake = await FakeService.start();
  try {
    const cfg = config(stateDir);
    await writeRecord(stateDir, { host: '127.0.0.1', port: fake.port, pid: holder.pid!, rcfd: cfg.rcfd, startedAt: new Date().toISOString() });
    const client = new ServiceClient(cfg, { ...fast, startService: () => Promise.reject(new Error('nothing is started in this test')) });
    let left: Promise<{ leftRunning: boolean }> | null = null;
    let statusThen = '';
    // in the turn the client adopts it, before its connections are made
    client.onState((s) => {
      if (left === null && s.adopted && s.pid === holder.pid) {
        statusThen = s.status;
        left = client.shutdown({ keepWithSessions: false });
      }
    });
    client.start();
    await until(() => left !== null, 5000, 'the service to be adopted');
    expect(statusThen).not.toBe('connected');
    expect(await left!).toEqual({ leftRunning: true });
    await new Promise((r) => setTimeout(r, 500));
    expect(processAlive(holder.pid!)).toBe(true);
    expect(existsSync(path.join(stateDir, 'service.json'))).toBe(true);
  } finally {
    holder.kill();
    await fake.stop();
    rmSync(stateDir, { recursive: true, force: true, maxRetries: 5 });
  }
});
