// Unit tests of the service port's checks (src/main/service/bridge.ts) and of the allow-list (src/shared/service.ts),
// with Node's MessageChannel standing in for Electron's and a recording client standing in for the service.
import { expect, test } from '@playwright/test';
import { MessageChannel, type MessagePort } from 'node:worker_threads';
import {
  SERVICE_CONTROL_METHODS,
  SERVICE_METHODS,
  SERVICE_READ_METHODS,
  type PortMessage,
  type ServiceState,
} from '../../src/shared/service.js';
import { jsonProblem, parseRequest, PortBridge, type BridgeClient, type PortLike } from '../../src/main/service/bridge.js';
import type { ClientEventBatch } from '../../src/main/service/client.js';
import { RpcError } from '../../src/main/service/rpc.js';
import { until } from '../support/fake-service.js';

test.describe('the allow-list', () => {
  test('is the read methods and the control methods, without duplicates or the excluded ones', () => {
    expect(SERVICE_METHODS).toEqual([...SERVICE_READ_METHODS, ...SERVICE_CONTROL_METHODS]);
    expect(new Set(SERVICE_METHODS).size).toBe(SERVICE_METHODS.length);
    const excluded = ['events.poll', 'run.wait', 'workflow.run', 'run.headless', 'journal.replay', 'field.export_vtk', 'report.export', 'render.png', 'field.slice', 'data.field', 'cosim.open'];
    for (const m of excluded) expect(SERVICE_METHODS as readonly string[], m).not.toContain(m);
    expect(SERVICE_METHODS.some((m) => m.startsWith('design.') || m.startsWith('workflow.') || m.startsWith('cosim.'))).toBe(false);
  });
});

test.describe('parseRequest', () => {
  test('accepts each operation in its exact shape', () => {
    expect(parseRequest({ op: 'call', id: 1, method: 'service.info' })).toEqual({ ok: true, request: { op: 'call', id: 1, method: 'service.info' } });
    expect(parseRequest({ op: 'call', id: 2, method: 'settings.set', params: { path: 'a', value: [1, null, 'x', { b: true }] }, timeoutMs: 500 })).toEqual({
      ok: true,
      request: { op: 'call', id: 2, method: 'settings.set', params: { path: 'a', value: [1, null, 'x', { b: true }] }, timeoutMs: 500 },
    });
    expect(parseRequest({ op: 'state', id: 3 })).toEqual({ ok: true, request: { op: 'state', id: 3 } });
    expect(parseRequest({ op: 'subscribe', id: 4, subscription: 1, session: 's1' })).toEqual({
      ok: true,
      request: { op: 'subscribe', id: 4, subscription: 1, session: 's1' },
    });
    expect(parseRequest({ op: 'unsubscribe', id: 5, subscription: 1 })).toEqual({ ok: true, request: { op: 'unsubscribe', id: 5, subscription: 1 } });
    for (const m of SERVICE_METHODS) expect(parseRequest({ op: 'call', id: 1, method: m }).ok, m).toBe(true);
  });

  test('refuses every method that is not on the list, by its exact name', () => {
    const refused = [
      'events.poll',
      'run.wait',
      'design.run_study',
      'field.export_vtk',
      'cosim.open',
      'data.field',
      'no.such',
      'service.info ',
      ' service.info',
      'SERVICE.INFO',
      'service.Info',
      'service.info\n',
      '__proto__',
      'constructor',
      'toString',
      'hasOwnProperty',
      '',
    ];
    for (const method of refused) {
      const r = parseRequest({ op: 'call', id: 7, method });
      expect(r, JSON.stringify(method)).toEqual({ ok: false, id: 7, error: { code: 'not_allowed', message: expect.stringContaining('not on the') } });
    }
  });

  test('refuses malformed requests, with the id when there is one', () => {
    const cases: [unknown, number | null][] = [
      [null, null],
      ['call', null],
      [[{ op: 'call', id: 1, method: 'service.info' }], null],
      [{ op: 'call', method: 'service.info' }, null],
      [{ op: 'call', id: -1, method: 'service.info' }, null],
      [{ op: 'call', id: 1.5, method: 'service.info' }, null],
      [{ op: 'call', id: 1, method: 7 }, 1],
      [{ op: 'call', id: 1, method: ['service.info'] }, 1],
      [{ op: 'call', id: 1, method: 'service.info', params: [] }, 1],
      [{ op: 'call', id: 1, method: 'service.info', params: 'x' }, 1],
      [{ op: 'call', id: 1, method: 'service.info', params: null }, 1],
      [{ op: 'call', id: 1, method: 'service.info', params: { a: Number.NaN } }, 1],
      [{ op: 'call', id: 1, method: 'service.info', params: { a: new Map() } }, 1],
      [{ op: 'call', id: 1, method: 'service.info', params: { a: new Date(0) } }, 1],
      [{ op: 'call', id: 1, method: 'service.info', params: { a: new Uint8Array(2) } }, 1],
      [{ op: 'call', id: 1, method: 'service.info', params: { a: 1n } }, 1],
      [{ op: 'call', id: 1, method: 'service.info', params: { a: undefined } }, 1],
      [{ op: 'call', id: 1, method: 'service.info', timeoutMs: 0 }, 1],
      [{ op: 'call', id: 1, method: 'service.info', timeoutMs: 600_001 }, 1],
      [{ op: 'call', id: 1, method: 'service.info', timeoutMs: '5' }, 1],
      [{ op: 'call', id: 1, method: 'service.info', extra: 1 }, 1],
      [{ op: 'state', id: 1, method: 'service.info' }, 1],
      [{ op: 'subscribe', id: 1 }, 1],
      [{ op: 'subscribe', id: 1, subscription: 'a' }, 1],
      [{ op: 'subscribe', id: 1, subscription: 1, session: 5 }, 1],
      [{ op: 'unsubscribe', id: 1, subscription: 1, session: 's' }, 1],
      [{ op: 'poll', id: 1 }, 1],
      [{ op: 'rpc', id: 1, method: 'events.poll' }, 1],
      [{ id: 1, method: 'service.info' }, 1],
    ];
    for (const [raw, id] of cases) {
      const r = parseRequest(raw);
      expect(r.ok, String(JSON.stringify(raw, (_k, v) => (typeof v === 'bigint' ? `${v}n` : v)))).toBe(false);
      if (!r.ok) {
        expect(r.id).toBe(id);
        expect(r.error.code).toBe('bad_request');
      }
    }
    const holes = [1, , 3];
    expect(jsonProblem({ holes })).toContain('hole');
    let deep: unknown = 1;
    for (let i = 0; i < 70; i++) deep = [deep];
    expect(jsonProblem({ deep })).toContain('deeper');
    expect(jsonProblem(Object.assign(Object.create(null), { a: 1 }))).toBeNull();
  });
});

/** A recording client: answers calls from a table, and lets the test push states and events. */
class RecordingClient implements BridgeClient {
  readonly calls: { method: string; params: unknown; timeoutMs: number | undefined }[] = [];
  state: ServiceState = {
    status: 'connected', mode: 'attach', address: '127.0.0.1:1', pid: null, instance: 1, adopted: false,
    version: '0', protocolVersion: '1.1', attempts: 0, retryInMs: null, lastError: null,
  };
  private readonly stateL = new Set<(s: ServiceState) => void>();
  private readonly eventL = new Set<(b: ClientEventBatch) => void>();
  async call(method: string, params?: Record<string, unknown>, timeoutMs?: number): Promise<unknown> {
    this.calls.push({ method, params, timeoutMs });
    if (method === 'settings.validate') throw new RpcError(1003, 'settings failed validation', { diagnostics: [] });
    return { method, params };
  }
  onState(l: (s: ServiceState) => void) {
    this.stateL.add(l);
    return () => this.stateL.delete(l);
  }
  onEvents(l: (b: ClientEventBatch) => void) {
    this.eventL.add(l);
    return () => this.eventL.delete(l);
  }
  push(b: ClientEventBatch) {
    for (const l of this.eventL) l(b);
  }
  setState(s: Partial<ServiceState>) {
    this.state = { ...this.state, ...s };
    for (const l of this.stateL) l(this.state);
  }
  get listeners() {
    return this.stateL.size + this.eventL.size;
  }
}

const adapt = (p: MessagePort): PortLike => ({
  postMessage: (m) => p.postMessage(m),
  onMessage: (l) => {
    p.on('message', l);
  },
  onClose: (l) => {
    p.on('close', l);
  },
  close: () => p.close(),
});

function rig() {
  const { port1, port2 } = new MessageChannel();
  const client = new RecordingClient();
  const bridge = new PortBridge(adapt(port1), client);
  const got: PortMessage[] = [];
  port2.on('message', (m: PortMessage) => got.push(m));
  const replyTo = (id: number) => until(() => got.find((m) => m.type === 'reply' && m.id === id), 2000, `reply ${id}`) as Promise<PortMessage & { type: 'reply' }>;
  return { port2, client, bridge, got, replyTo };
}

test.describe('PortBridge', () => {
  test('sends the state at once, forwards allowed calls with their time limit, and returns errors as values', async () => {
    const { port2, client, got, replyTo, bridge } = rig();
    await until(() => got.find((m) => m.type === 'state'), 2000, 'the first state');
    port2.postMessage({ op: 'call', id: 1, method: 'run.status', params: { session: 's1' } });
    port2.postMessage({ op: 'call', id: 2, method: 'settings.validate', params: { session: 's1' }, timeoutMs: 1234 });
    expect((await replyTo(1)).reply).toEqual({ ok: true, result: { method: 'run.status', params: { session: 's1' } } });
    expect((await replyTo(2)).reply).toEqual({ ok: false, error: { code: 1003, message: 'settings failed validation', data: { diagnostics: [] } } });
    expect(client.calls).toEqual([
      { method: 'run.status', params: { session: 's1' }, timeoutMs: 60_000 },
      { method: 'settings.validate', params: { session: 's1' }, timeoutMs: 1234 },
    ]);
    bridge.dispose();
    port2.close();
  });

  test('refused requests never reach the client', async () => {
    const { port2, client, replyTo, bridge } = rig();
    port2.postMessage({ op: 'call', id: 1, method: 'events.poll', params: { after: 0 } });
    port2.postMessage({ op: 'call', id: 2, method: 'run.wait' });
    port2.postMessage({ op: 'call', id: 3, method: 'service.info', params: [1] });
    port2.postMessage('garbage');
    port2.postMessage({ op: 'call', id: 4, method: 'service.info' });
    expect((await replyTo(1)).reply).toMatchObject({ ok: false, error: { code: 'not_allowed' } });
    expect((await replyTo(2)).reply).toMatchObject({ ok: false, error: { code: 'not_allowed' } });
    expect((await replyTo(3)).reply).toMatchObject({ ok: false, error: { code: 'bad_request' } });
    await replyTo(4);
    expect(client.calls.map((c) => c.method)).toEqual(['service.info']);
    expect(bridge.refused).toBe(4);
    bridge.dispose();
    port2.close();
  });

  test('subscriptions get the events of their session, missed counts, and nothing after unsubscribing', async () => {
    const { port2, client, got, replyTo, bridge } = rig();
    port2.postMessage({ op: 'subscribe', id: 1, subscription: 10 });
    port2.postMessage({ op: 'subscribe', id: 2, subscription: 11, session: 'b' });
    expect((await replyTo(1)).reply).toEqual({ ok: true, result: 10 });
    await replyTo(2);
    const ev = (seq: number, session: string) => ({ seq, session, kind: 'iteration', data: {} });
    client.push({ instance: 1, events: [ev(1, 'a'), ev(2, 'b')], missed: 0 });
    client.push({ instance: 1, events: [ev(3, 'a')], missed: 2 });
    await until(() => got.filter((m) => m.type === 'events').length === 4, 2000, 'four batches');
    const batches = got.filter((m): m is PortMessage & { type: 'events' } => m.type === 'events');
    expect(batches.map((b) => [b.subscription, b.events.map((e) => e.seq), b.missed])).toEqual([
      [10, [1, 2], 0],
      [11, [2], 0],
      [10, [3], 2],
      [11, [], 2],
    ]);
    port2.postMessage({ op: 'unsubscribe', id: 3, subscription: 10 });
    await replyTo(3);
    client.push({ instance: 2, events: [ev(1, 'b')], missed: 0 });
    await until(() => got.filter((m) => m.type === 'events').length === 5, 2000, 'one more batch');
    expect(got.at(-1)).toMatchObject({ type: 'events', subscription: 11, instance: 2 });
    client.setState({ status: 'disconnected' });
    await until(() => got.some((m) => m.type === 'state' && m.state.status === 'disconnected'), 2000, 'the state change');
    bridge.dispose();
    expect(client.listeners).toBe(0);
    port2.close();
  });

  test('a closed port disposes the bridge', async () => {
    const { port2, client } = rig();
    expect(client.listeners).toBe(2);
    port2.close();
    await until(() => client.listeners === 0, 2000, 'the listeners to go');
  });
});
