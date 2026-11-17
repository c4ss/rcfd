// Unit tests of the data port's checks (src/main/data/bridge.ts), the data roots (roots.ts) and the cancellable queue
// (queue.ts), with Node's MessageChannel standing in for Electron's and a scripted backend standing in for the readers.
// The readers themselves are tested against the Rust reader in data-routes.spec.ts.
import { expect, test } from '@playwright/test';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { MessageChannel, type MessagePort } from 'node:worker_threads';
import { DataPortBridge, parseDataRequest, type DataBackend, type DataPortLike } from '../../src/main/data/bridge.js';
import { DataFailure, SerialQueue, type Job } from '../../src/main/data/queue.js';
import { splitAddress } from '../../src/main/data/remote.js';
import { DataRoots, isInside, pathProblem } from '../../src/main/data/roots.js';
import { MAX_DATA_PENDING, type DataPortMessage, type DataRequest, type DataWireResult } from '../../src/shared/data.js';
import { until } from '../support/fake-service.js';

const abs = process.platform === 'win32' ? 'C:\\runs\\a\\fields' : '/runs/a/fields';
const local = { route: 'local', path: abs };

test.describe('parseDataRequest', () => {
  test('accepts each operation in its exact shape', () => {
    expect(parseDataRequest({ op: 'open', id: 1, store: local })).toEqual({ ok: true, request: { op: 'open', id: 1, store: local } });
    expect(parseDataRequest({ op: 'field', id: 2, store: { route: 'service', path: 'runs/a/fields' }, snapshot: 19, field: 'p', tag: 'scrub', timeoutMs: 500 })).toEqual({
      ok: true,
      request: { op: 'field', id: 2, store: { route: 'service', path: 'runs/a/fields' }, snapshot: 19, field: 'p', tag: 'scrub', timeoutMs: 500 },
    });
    expect(parseDataRequest({ op: 'surface', id: 3, store: local, tag: 7 })).toEqual({ ok: true, request: { op: 'surface', id: 3, store: local, tag: 7 } });
    expect(parseDataRequest({ op: 'cancel', id: 4, tag: 'scrub' })).toEqual({ ok: true, request: { op: 'cancel', id: 4, tag: 'scrub' } });
  });

  test('refuses everything else, with the id when there is one', () => {
    const refused: unknown[] = [
      null,
      7,
      'open',
      [],
      { op: 'open', store: local },
      { op: 'open', id: -1, store: local },
      { op: 'open', id: 1.5, store: local },
      { op: 'read', id: 1, store: local },
      { op: 'open', id: 1 },
      { op: 'open', id: 1, store: local, extra: 1 },
      { op: 'open', id: 1, store: { route: 'file', path: abs } },
      { op: 'open', id: 1, store: { route: 'local' } },
      { op: 'open', id: 1, store: { route: 'local', path: abs, x: 1 } },
      { op: 'open', id: 1, store: { route: 'local', path: 7 } },
      { op: 'open', id: 1, store: { route: 'local', path: '' } },
      { op: 'open', id: 1, store: { route: 'local', path: 'a'.repeat(4097) } },
      { op: 'open', id: 1, store: { route: 'local', path: `${abs}\0x` } },
      { op: 'open', id: 1, store: Object.assign(Object.create({ route: 'local' }), { path: abs }) },
      { op: 'field', id: 1, store: local, snapshot: -1, field: 'p' },
      { op: 'field', id: 1, store: local, snapshot: 1.5, field: 'p' },
      { op: 'field', id: 1, store: local, snapshot: '1', field: 'p' },
      { op: 'field', id: 1, store: local, snapshot: 1, field: '' },
      { op: 'field', id: 1, store: local, snapshot: 1, field: 'x'.repeat(257) },
      { op: 'field', id: 1, store: local, snapshot: 1 },
      { op: 'surface', id: 1, store: local, timeoutMs: 0 },
      { op: 'surface', id: 1, store: local, timeoutMs: 600_001 },
      { op: 'surface', id: 1, store: local, tag: 'x'.repeat(65) },
      { op: 'surface', id: 1, store: local, tag: { a: 1 } },
      { op: 'cancel', id: 1 },
      { op: 'cancel', id: 1, tag: -3 },
      { op: 'cancel', id: 1, tag: 'a', store: local },
    ];
    for (const r of refused) {
      const p = parseDataRequest(r);
      expect(p.ok, JSON.stringify(r)).toBe(false);
      if (!p.ok) expect(p.error.code).toBe('bad_request');
    }
    const p = parseDataRequest({ op: 'field', id: 9, store: local, snapshot: -1, field: 'p' });
    expect(!p.ok && p.id).toBe(9);
  });
});

test('F-819b: surfaceField has the exact shape of field, and the same refusals', () => {
  expect(parseDataRequest({ op: 'surfaceField', id: 5, store: local, snapshot: 3, field: 'p', tag: 'scrub', timeoutMs: 500 })).toEqual({
    ok: true,
    request: { op: 'surfaceField', id: 5, store: local, snapshot: 3, field: 'p', tag: 'scrub', timeoutMs: 500 },
  });
  for (const r of [
    { op: 'surfaceField', id: 1, store: local, snapshot: -1, field: 'p' },
    { op: 'surfaceField', id: 1, store: local, snapshot: 1.5, field: 'p' },
    { op: 'surfaceField', id: 1, store: local, snapshot: 1, field: '' },
    { op: 'surfaceField', id: 1, store: local, snapshot: 1, field: 'x'.repeat(257) },
    { op: 'surfaceField', id: 1, store: local, snapshot: 1 },
    { op: 'surfaceField', id: 1, store: local, snapshot: 1, field: 'p', cells: [0] },
    { op: 'surfaceField', id: 1, store: { route: 'file', path: abs }, snapshot: 1, field: 'p' },
  ]) {
    const p = parseDataRequest(r);
    expect(p.ok, JSON.stringify(r)).toBe(false);
    if (!p.ok) expect([p.id, p.error.code]).toEqual([1, 'bad_request']);
  }
});

test.describe('data roots', () => {
  let dir = '';
  test.beforeAll(() => {
    dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f804-roots-'));
    mkdirSync(path.join(dir, 'project', 'runs', 'a', 'fields'), { recursive: true });
    mkdirSync(path.join(dir, 'elsewhere', 'fields'), { recursive: true });
    writeFileSync(path.join(dir, 'elsewhere', 'fields', 'store.rts'), 'x');
  });
  test.afterAll(() => rmSync(dir, { recursive: true, force: true }));

  test('a path is refused before the file system is asked: relative, with .., device paths', () => {
    expect(pathProblem('runs/a/fields')).toMatch(/absolute/);
    expect(pathProblem(path.join(dir, 'project', '..', 'elsewhere'))).toBeNull(); // path.join resolves the '..'
    expect(pathProblem(`${dir}${path.sep}project${path.sep}..${path.sep}elsewhere`)).toMatch(/'\.\.'/);
    expect(pathProblem('')).toMatch(/empty/);
    if (process.platform === 'win32') expect(pathProblem('\\\\?\\C:\\x')).toMatch(/device/);
    expect(isInside('/a/b', '/a/b')).toBe(true);
    expect(isInside('/a/b', '/a/b/c')).toBe(true);
    expect(isInside('/a/b', '/a/bc')).toBe(false);
  });

  test('only folders inside a root are read, also through a link that points out of it', async () => {
    const roots = DataRoots.fromEnv({ RCFD_STUDIO_DATA_ROOTS: path.join(dir, 'project') });
    const inside = await roots.resolve(path.join(dir, 'project', 'runs', 'a', 'fields'));
    expect(inside.ok).toBe(true);
    expect((await roots.resolve(path.join(dir, 'elsewhere', 'fields'))).ok).toBe(false);
    expect((await roots.resolve(path.join(dir, 'project', 'runs', 'missing'))).ok).toBe(false);
    // a junction (Windows) or symbolic link inside the root that leads outside it
    const link = path.join(dir, 'project', 'runs', 'escape');
    symlinkSync(path.join(dir, 'elsewhere'), link, process.platform === 'win32' ? 'junction' : 'dir');
    const out = await roots.resolve(path.join(link, 'fields'));
    expect(out).toEqual({ ok: false, message: expect.stringMatching(/outside/) });
    // no roots at all: nothing local is read
    expect((await DataRoots.fromEnv({}).resolve(path.join(dir, 'project'))).ok).toBe(false);
    // F-805 adds a project folder as a root
    const later = DataRoots.fromEnv({});
    later.allow(path.join(dir, 'elsewhere'));
    expect((await later.resolve(path.join(dir, 'elsewhere', 'fields'))).ok).toBe(true);
    expect(() => later.allow('relative/dir')).toThrow(/absolute/);
  });

  test('several roots by the platform delimiter; relative entries are ignored', () => {
    const r = DataRoots.fromEnv({ RCFD_STUDIO_DATA_ROOTS: [path.join(dir, 'a'), 'relative', path.join(dir, 'b')].join(path.delimiter) });
    expect(r.list).toEqual([path.join(dir, 'a'), path.join(dir, 'b')]);
  });

  test('the service address splits into host and port', () => {
    expect(splitAddress('127.0.0.1:7878')).toEqual({ host: '127.0.0.1', port: 7878 });
    expect(splitAddress('::1:7878')).toEqual({ host: '::1', port: 7878 });
    expect(splitAddress('localhost')).toBeNull();
  });
});

test.describe('SerialQueue', () => {
  test('runs one task at a time in order; a waiting task is cancelled without running, a running one is dropped', async () => {
    const q = new SerialQueue();
    const ran: string[] = [];
    let release!: () => void;
    const a = q.push(() => new Promise<string>((r) => (release = () => r('a'))).then((v) => (ran.push(v), v)));
    const b = q.push(async () => (ran.push('b'), 'b'));
    const c = q.push(async () => (ran.push('c'), 'c'));
    b.cancel();
    a.cancel();
    await expect(a.promise).rejects.toMatchObject({ code: 'cancelled' });
    await expect(b.promise).rejects.toMatchObject({ code: 'cancelled' });
    release();
    await expect(c.promise).resolves.toBe('c');
    expect(ran).toEqual(['a', 'c']);
    const d = q.push(() => {
      throw new DataFailure('store', 'broken');
    });
    await expect(d.promise).rejects.toMatchObject({ code: 'store', message: 'broken' });
    await expect(q.push(async () => 1).promise).resolves.toBe(1);
  });
});

// ---- the port: a scripted backend whose jobs the test settles by hand

class Scripted implements DataBackend {
  readonly runs: { request: Exclude<DataRequest, { op: 'cancel' }>; timeoutMs: number; resolve: (r: DataWireResult) => void; reject: (e: unknown) => void; cancelled: boolean }[] = [];
  run(request: Exclude<DataRequest, { op: 'cancel' }>, timeoutMs: number): Job<DataWireResult> {
    let entry!: Scripted['runs'][number];
    const promise = new Promise<DataWireResult>((resolve, reject) => {
      entry = { request, timeoutMs, resolve, reject, cancelled: false };
    });
    this.runs.push(entry);
    return {
      promise,
      cancel: () => {
        entry.cancelled = true;
      },
    };
  }
}

function harness() {
  const { port1, port2 } = new MessageChannel();
  const wrap = (p: MessagePort): DataPortLike => ({
    postMessage: (m) => p.postMessage(m),
    onMessage: (l) => p.on('message', l),
    onClose: (l) => p.on('close', l),
    close: () => p.close(),
  });
  const backend = new Scripted();
  const bridge = new DataPortBridge(wrap(port1), backend);
  const got: DataPortMessage[] = [];
  port2.on('message', (m: DataPortMessage) => got.push(m));
  const reply = (id: number) => until(async () => got.find((m) => m.id === id) ?? null, 5000, `reply ${id}`);
  return { port2, bridge, backend, got, reply, close: () => port2.close() };
}

test.describe('DataPortBridge', () => {
  test('a request reaches the backend checked; its result goes back with the ArrayBuffer intact', async () => {
    const h = harness();
    h.port2.postMessage({ op: 'field', id: 1, store: local, snapshot: 3, field: 'p' });
    await until(async () => h.backend.runs[0] ?? null, 5000, 'the backend call');
    expect(h.backend.runs[0]!.request).toEqual({ op: 'field', id: 1, store: local, snapshot: 3, field: 'p' });
    expect(h.backend.runs[0]!.timeoutMs).toBe(60_000);
    const values = new Float32Array([1, -0, Number.NaN, 3.5]).buffer;
    h.backend.runs[0]!.resolve({ kind: 'field', snapshot: 3, field: 'p', values });
    const m = await h.reply(1);
    expect(m.reply.ok).toBe(true);
    if (m.reply.ok && m.reply.result.kind === 'field') {
      expect([...new Uint32Array(m.reply.result.values)]).toEqual([...new Uint32Array(new Float32Array([1, -0, Number.NaN, 3.5]).buffer)]);
    }
    h.close();
  });

  test('a malformed request is refused and never reaches the backend', async () => {
    const h = harness();
    h.port2.postMessage({ op: 'field', id: 5, store: { route: 'file', path: abs }, snapshot: 0, field: 'p' });
    h.port2.postMessage({ op: 'open' });
    const m = await h.reply(5);
    expect(m.reply).toEqual({ ok: false, error: { code: 'bad_request', message: expect.stringMatching(/route/) } });
    await until(async () => (h.bridge.refused === 2 ? true : null), 5000, 'two refusals');
    expect(h.backend.runs).toEqual([]);
    h.close();
  });

  test('cancel(tag) answers every waiting request with that tag at once and cancels its job; a late result is dropped', async () => {
    const h = harness();
    for (let id = 1; id <= 3; id++) h.port2.postMessage({ op: 'field', id, store: local, snapshot: id, field: 'p', tag: 'scrub' });
    h.port2.postMessage({ op: 'field', id: 4, store: local, snapshot: 4, field: 'p', tag: 'other' });
    await until(async () => (h.backend.runs.length === 4 ? true : null), 5000, 'four calls');
    h.port2.postMessage({ op: 'cancel', id: 10, tag: 'scrub' });
    const c = await h.reply(10);
    expect(c.reply).toEqual({ ok: true, result: { kind: 'cancel', cancelled: 3 } });
    for (const id of [1, 2, 3]) expect((await h.reply(id)).reply).toMatchObject({ ok: false, error: { code: 'cancelled' } });
    expect(h.backend.runs.map((r) => r.cancelled)).toEqual([true, true, true, false]);
    h.backend.runs[0]!.resolve({ kind: 'field', snapshot: 1, field: 'p', values: new ArrayBuffer(4) });
    h.backend.runs[3]!.resolve({ kind: 'field', snapshot: 4, field: 'p', values: new ArrayBuffer(4) });
    expect((await h.reply(4)).reply.ok).toBe(true);
    expect(h.got.filter((m) => m.id === 1)).toHaveLength(1);
    h.close();
  });

  test('a request past its time limit answers timeout and cancels its job; errors keep their codes', async () => {
    const h = harness();
    h.port2.postMessage({ op: 'surface', id: 1, store: local, timeoutMs: 50 });
    h.port2.postMessage({ op: 'open', id: 2, store: { route: 'service', path: 'x' } });
    expect((await h.reply(1)).reply).toMatchObject({ ok: false, error: { code: 'timeout' } });
    expect(h.backend.runs[0]!.cancelled).toBe(true);
    await until(async () => h.backend.runs[1] ?? null, 5000, 'the second call');
    h.backend.runs[1]!.reject(new DataFailure(1004, "path 'x' is outside the sandbox"));
    expect((await h.reply(2)).reply).toEqual({ ok: false, error: { code: 1004, message: "path 'x' is outside the sandbox" } });
    h.close();
  });

  test('at most MAX_DATA_PENDING requests wait per window; a duplicate id is refused; closing the port cancels them all', async () => {
    const h = harness();
    for (let id = 1; id <= MAX_DATA_PENDING + 1; id++) h.port2.postMessage({ op: 'open', id, store: local });
    const over = await h.reply(MAX_DATA_PENDING + 1);
    expect(over.reply).toMatchObject({ ok: false, error: { code: 'bad_request', message: expect.stringMatching(/at most/) } });
    h.port2.postMessage({ op: 'open', id: 1, store: local });
    await until(async () => (h.got.filter((m) => m.id === 1).length === 1 ? true : null), 5000, 'the duplicate refused');
    expect(h.backend.runs).toHaveLength(MAX_DATA_PENDING);
    h.bridge.dispose();
    expect(h.backend.runs.every((r) => r.cancelled)).toBe(true);
    h.close();
  });
});
