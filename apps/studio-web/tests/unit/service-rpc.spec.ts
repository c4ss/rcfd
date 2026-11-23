// F-803: the item of this spec, by the commit that added it (afee363a, 2026-11-16).
// Unit tests of the rcfd-service/1 framing and request layer (src/main/service/rpc.ts), against a fake service in this
// process and a fake stdio service in a child Node process. No Electron, no rcfd binary.
import { expect, test } from '@playwright/test';
import { spawn } from 'node:child_process';
import { PassThrough } from 'node:stream';
import { LineSplitter, RpcConnection, RpcError } from '../../src/main/service/rpc.js';
import { FakeService, until } from '../support/fake-service.js';

test.describe('LineSplitter', () => {
  test('splits at newlines across chunk borders, keeps a UTF-8 character cut in two, skips blank lines and CR', () => {
    const lines: string[] = [];
    const s = new LineSplitter((l) => lines.push(l));
    const text = Buffer.from('{"a":1}\n{"b":"Ω→é"}\r\n\n{"c":', 'utf8');
    // cut inside the two-byte "Ω" and inside the three-byte "→"
    const cuts = [3, 13, 14, 16, text.length];
    let at = 0;
    for (const c of cuts) {
      expect(s.push(text.subarray(at, c))).toBe(true);
      at = c;
    }
    s.push(Buffer.from('3}\n', 'utf8'));
    expect(lines).toEqual(['{"a":1}', '{"b":"Ω→é"}', '{"c":3}']);
  });

  test('many lines in one chunk, and a line built from many chunks', () => {
    const lines: string[] = [];
    const s = new LineSplitter((l) => lines.push(l));
    s.push(Buffer.from('1\n2\n3\n'));
    for (const ch of '45678') s.push(Buffer.from(ch));
    s.push(Buffer.from('\n'));
    expect(lines).toEqual(['1', '2', '3', '45678']);
  });

  test('reports a pending line longer than the limit', () => {
    const s = new LineSplitter(() => undefined, 10);
    expect(s.push(Buffer.from('12345'))).toBe(true);
    expect(s.push(Buffer.from('678901'))).toBe(false);
  });
});

test.describe('RpcConnection over TCP', () => {
  let fake: FakeService;
  let c: RpcConnection;
  test.beforeEach(async () => {
    fake = await FakeService.start();
    c = await RpcConnection.connectTcp('127.0.0.1', fake.port);
  });
  test.afterEach(async () => {
    c.close();
    await fake.stop();
  });

  test('a request is one JSON-RPC 2.0 line with an increasing id, and resolves with its result', async () => {
    const a = await c.request('service.info');
    const b = await c.request('settings.get', { path: 'mesh.nx' });
    expect((a as { protocol: string }).protocol).toBe('rcfd-service/1');
    expect(b).toEqual({ echo: { path: 'mesh.nx' } });
    expect(fake.received.map((r) => [r.id, r.method])).toEqual([
      [1, 'service.info'],
      [2, 'settings.get'],
    ]);
  });

  test('replies are matched by id, not by order', async () => {
    const slow = c.request('run.step', { delay_ms: 150 });
    const fast = c.request('settings.get', { k: 1 });
    const order: string[] = [];
    await Promise.all([slow.then(() => order.push('slow')), fast.then(() => order.push('fast'))]);
    expect(order).toEqual(['fast', 'slow']);
  });

  test('a service error becomes an RpcError with its code, message and data', async () => {
    const e = await c.request('settings.validate').catch((x: unknown) => x);
    expect(e).toBeInstanceOf(RpcError);
    expect((e as RpcError).code).toBe(1003);
    expect((e as RpcError).toServiceError()).toEqual({
      code: 1003,
      message: 'settings failed validation',
      data: { diagnostics: [{ path: 'mesh', line: 1, message: 'x' }] },
    });
    const unknown = (await c.request('no.such').catch((x: unknown) => x)) as RpcError;
    expect(unknown.code).toBe(-32601);
  });

  test('a request that outlives its time limit fails with timeout; the late reply is dropped', async () => {
    const e = (await c.request('run.step', { delay_ms: 300 }, 50).catch((x: unknown) => x)) as RpcError;
    expect(e.code).toBe('timeout');
    await until(() => c.stray === 1, 2000, 'the late reply');
    // the connection still works
    expect(await c.request('settings.get', { k: 2 })).toEqual({ echo: { k: 2 } });
    expect(c.pendingCount).toBe(0);
  });

  test('when the service goes away, pending requests fail with disconnected and new ones fail at once', async () => {
    let closed = '';
    c.close();
    c = await RpcConnection.connectTcp('127.0.0.1', fake.port, { onClose: (r) => (closed = r) });
    const p = c.request('run.step', { delay_ms: 10_000 });
    await until(() => fake.received.some((r) => r.method === 'run.step'), 2000, 'the request to arrive');
    fake.dropConnections();
    const e = (await p.catch((x: unknown) => x)) as RpcError;
    expect(e.code).toBe('disconnected');
    expect(closed).toContain('closed the connection');
    expect(c.closed).toBe(true);
    const later = (await c.request('service.info').catch((x: unknown) => x)) as RpcError;
    expect(later.code).toBe('disconnected');
  });

  test('a reply with id null is not paired with any request; a line that is not JSON ends the connection', async () => {
    fake.broadcast('{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}\n');
    await until(() => c.stray === 1, 2000, 'the stray reply');
    expect(await c.request('settings.get', { k: 3 })).toEqual({ echo: { k: 3 } });
    let closed = '';
    c.close();
    c = await RpcConnection.connectTcp('127.0.0.1', fake.port, { onClose: (r) => (closed = r) });
    await until(() => fake.sockets.size === 1, 2000, 'the new connection');
    fake.broadcast('this is not json\n');
    await until(() => c.closed, 2000, 'the close');
    expect(closed).toContain('not JSON');
  });

  test('parameters that are not JSON are refused before they are sent', async () => {
    const cyclic: Record<string, unknown> = {};
    cyclic['self'] = cyclic;
    const e = (await c.request('settings.get', cyclic).catch((x: unknown) => x)) as RpcError;
    expect(e.code).toBe('bad_request');
    expect(fake.methods()).toEqual([]);
  });

  test('connecting to a port nobody listens on fails with disconnected', async () => {
    const dead = await FakeService.start();
    const port = dead.port;
    await dead.stop();
    const e = (await RpcConnection.connectTcp('127.0.0.1', port).catch((x: unknown) => x)) as RpcError;
    expect(e.code).toBe('disconnected');
  });
});

test.describe('RpcConnection over streams (stdio)', () => {
  test('in-memory streams: a request written to one stream, its reply read from the other', async () => {
    const toService = new PassThrough();
    const fromService = new PassThrough();
    const c = new RpcConnection(fromService, toService);
    toService.on('data', (d: Buffer) => {
      const req = JSON.parse(d.toString('utf8'));
      // the reply in three pieces
      const line = JSON.stringify({ jsonrpc: '2.0', id: req.id, result: { ok: req.method } }) + '\n';
      fromService.write(line.slice(0, 5));
      fromService.write(line.slice(5, 9));
      fromService.write(line.slice(9));
    });
    expect(await c.request('service.info')).toEqual({ ok: 'service.info' });
    c.close();
  });

  test('a child process speaking the protocol on its standard streams, as `rcfd serve --stdio` does', async () => {
    const script = `
      let buf = '';
      process.stdin.on('data', (d) => {
        buf += d;
        for (let nl = buf.indexOf('\\n'); nl >= 0; nl = buf.indexOf('\\n')) {
          const req = JSON.parse(buf.slice(0, nl));
          buf = buf.slice(nl + 1);
          process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: req.id, result: { protocol: 'rcfd-service/1', method: req.method } }) + '\\n');
        }
      });`;
    const child = spawn(process.execPath, ['-e', script], { stdio: ['pipe', 'pipe', 'inherit'] });
    const c = new RpcConnection(child.stdout, child.stdin, {}, () => child.stdin.end());
    const replies = await Promise.all([c.request('service.info'), c.request('run.status', { session: 's1' })]);
    expect(replies).toEqual([
      { protocol: 'rcfd-service/1', method: 'service.info' },
      { protocol: 'rcfd-service/1', method: 'run.status' },
    ]);
    c.close();
    await new Promise((r) => child.once('exit', r));
  });
});
