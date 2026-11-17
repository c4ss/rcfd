// The renderer reaches the solver service only through the allow-list (docs/studio-guardrails.md G10, F-803): the
// packaged Studio attached to a fake service that records every line it receives, so a refused call can be shown never
// to have arrived. The real service is in service.spec.ts.
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { SERVICE_METHODS } from '../../src/shared/service.js';
import { FakeService, until } from '../support/fake-service.js';
import { launchStudio, untilState } from '../support/studio.js';

let fake: FakeService;
let app: ElectronApplication;
let page: Page;

test.beforeAll(async () => {
  fake = await FakeService.start();
  ({ app, page } = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${fake.port}` }));
  await untilState(page, (s) => s.status === 'connected', 15_000, 'connected');
});

test.afterAll(async () => {
  await app?.close();
  await fake?.stop();
});

test('the service bridge is window.studioService with exactly its seven functions, frozen', async () => {
  const shape = await page.evaluate(() => {
    const s = window.studioService as unknown as Record<string, unknown>;
    return { keys: Object.keys(s).sort(), types: [...new Set(Object.values(s).map((v) => typeof v))], frozen: Object.isFrozen(s) };
  });
  expect(shape).toEqual({
    keys: ['call', 'info', 'offState', 'onState', 'state', 'subscribe', 'unsubscribe'],
    types: ['function'],
    frozen: true,
  });
});

test('only the main process is connected to the service: two connections, control and events', async () => {
  expect(fake.connections).toBe(2);
  const s = await untilState(page, (x) => x.status === 'connected');
  expect(s).toMatchObject({ mode: 'attach', address: `127.0.0.1:${fake.port}`, instance: 1, pid: null });
});

test('an allowed call reaches the service and its reply comes back', async () => {
  const r = await page.evaluate(() => window.studioService.call('settings.get', { path: 'mesh' }));
  expect(r).toEqual({ ok: true, result: { echo: { path: 'mesh' } } });
  const info = await page.evaluate(() => window.studioService.info());
  expect(info.ok && info.result.protocol).toBe('rcfd-service/1');
  const err = await page.evaluate(() => window.studioService.call('settings.validate', {}));
  expect(err).toEqual({ ok: false, error: { code: 1003, message: 'settings failed validation', data: { diagnostics: [{ path: 'mesh', line: 1, message: 'x' }] } } });
});

test('a method outside the allow-list is refused in the main process and never reaches the service', async () => {
  const before = fake.received.length;
  const refused = [
    'events.poll',
    'run.wait',
    'run.headless',
    'workflow.run',
    'design.run_study',
    'journal.replay',
    'field.export_vtk',
    'report.export',
    'render.png',
    'field.slice',
    'data.field',
    'cosim.open',
    'no.such.method',
    'service.info ',
    'SERVICE.INFO',
    '__proto__',
    'constructor',
    'toString',
    '',
  ];
  const replies = await page.evaluate(async (methods) => {
    const call = window.studioService.call as unknown as (m: unknown, p?: unknown, o?: unknown) => Promise<{ ok: boolean; error?: { code: unknown } }>;
    const out: unknown[] = [];
    for (const m of methods) out.push(await call(m, {}));
    // not a string at all
    for (const m of [7, null, ['service.info'], { method: 'service.info' }]) out.push(await call(m, {}));
    return out;
  }, refused);
  expect(replies.slice(0, refused.length)).toEqual(refused.map(() => ({ ok: false, error: { code: 'not_allowed', message: expect.any(String) } })));
  expect(replies.slice(refused.length)).toEqual([0, 1, 2, 3].map(() => ({ ok: false, error: { code: 'bad_request', message: expect.any(String) } })));
  await page.waitForTimeout(200);
  const arrived = fake.received.slice(before).map((r) => r.method).filter((m) => m !== 'events.poll');
  expect(arrived).toEqual([]);
  // the client's own long-polls carry on; none of them came from the page (they all have the client's shape)
  const polls = fake.received.slice(before).filter((r) => r.method === 'events.poll');
  for (const p of polls) expect(Object.keys(p.params as object).sort()).toEqual(['after', 'limit', 'timeout_ms']);
});

test('malformed arguments are refused and never reach the service', async () => {
  const before = fake.received.filter((r) => r.method !== 'events.poll').length;
  const replies = await page.evaluate(async () => {
    const call = window.studioService.call as unknown as (m: unknown, p?: unknown, o?: unknown) => Promise<{ ok: boolean; error?: { code: unknown } }>;
    return [
      await call('run.status', [1, 2]),
      await call('run.status', 'session=s1'),
      await call('run.status', { session: () => 1 }),
      await call('run.status', { when: new Date(0) }),
      await call('run.status', { n: Number.NaN }),
      await call('run.status', { session: 's1' }, { timeoutMs: 0 }),
      await call('run.status', { session: 's1' }, { timeoutMs: 'soon' }),
    ];
  });
  for (const r of replies) expect(r).toMatchObject({ ok: false, error: { code: 'bad_request' } });
  await page.waitForTimeout(200);
  expect(fake.received.filter((r) => r.method !== 'events.poll').length).toBe(before);
});

test('the page cannot open a connection of its own to the service', async () => {
  const connections = fake.connections;
  const r = await page.evaluate(async (port) => {
    const tried: string[] = [];
    try {
      await fetch(`http://127.0.0.1:${port}/`, { method: 'POST', body: '{"jsonrpc":"2.0","id":1,"method":"service.info"}\n' });
      tried.push('fetch: answered');
    } catch (e) {
      tried.push(`fetch: ${String(e).slice(0, 40)}`);
    }
    try {
      const ws = new WebSocket(`ws://127.0.0.1:${port}/`);
      await new Promise((done) => {
        ws.onerror = done;
        ws.onclose = done;
        setTimeout(done, 1000);
      });
      tried.push(`websocket: ${ws.readyState}`);
    } catch (e) {
      tried.push(`websocket: ${String(e).slice(0, 40)}`);
    }
    const img = document.createElement('img');
    img.src = `http://127.0.0.1:${port}/x.png`;
    document.body.append(img);
    try {
      new EventSource(`http://127.0.0.1:${port}/`);
    } catch (e) {
      tried.push(`eventsource: ${String(e).slice(0, 40)}`);
    }
    navigator.sendBeacon?.(`http://127.0.0.1:${port}/`, '{}');
    await new Promise((done) => setTimeout(done, 500));
    img.remove();
    return tried;
  }, fake.port);
  expect(r[0]).toContain('TypeError');
  expect(r[1]).toBe('websocket: 3'); // CLOSED, never opened
  await page.waitForTimeout(300);
  expect(fake.connections).toBe(connections);
  expect(fake.foreign).toEqual([]);
});

test('the port stays in the preload script: the page never sees it, and a reload gets a new one', async () => {
  // a listener in the page's world from before its own scripts run, across a reload
  await page.addInitScript(() => {
    const w = window as unknown as { __portsSeen: number };
    w.__portsSeen = 0;
    window.addEventListener('message', (e) => (w.__portsSeen += 1 + e.ports.length), true);
  });
  await page.reload();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
  const seen = await page.evaluate(async () => {
    const r = await window.studioService.call('settings.get', { after: 'reload' });
    await new Promise((done) => setTimeout(done, 200));
    return { ports: (window as unknown as { __portsSeen: number }).__portsSeen, r, studio: Object.keys(window.studio).sort() };
  });
  expect(seen).toEqual({ ports: 0, r: { ok: true, result: { echo: { after: 'reload' } } }, studio: ['app', 'runner', 'theme'] });
  // the reload did not touch the main process's connections (G7)
  expect(fake.connections).toBe(2);
  const s = await untilState(page, (x) => x.status === 'connected');
  expect(s.instance).toBe(1);
});

test('the shell\'s ipcMain channels are still app.info and app.ready only: the service uses none', async () => {
  const names = await app.evaluate(({ ipcMain }) => {
    const handlers = (ipcMain as unknown as { _invokeHandlers?: Map<string, unknown> })._invokeHandlers;
    return {
      handlers: handlers ? [...handlers.keys()].filter((n) => !/^[A-Z0-9_]+$/.test(n)).sort() : null,
      listeners: ipcMain.eventNames().map(String).filter((n) => n !== 'error'),
    };
  });
  // F-818 and F-837: the shell's channels are app.info, app.ready, the close answer and the theme's two
  expect(names).toEqual({ handlers: ['app.closeAnswer', 'app.info', 'app.ready', 'runner.keep.get', 'runner.keep.set', 'theme.get', 'theme.set'], listeners: [] });
});

test('events reach a subscriber, filtered by session when asked', async () => {
  const subs = await page.evaluate(async () => {
    const w = window as unknown as { __all: unknown[]; __s2: unknown[] };
    w.__all = [];
    w.__s2 = [];
    const a = await window.studioService.subscribe((b) => w.__all.push(...b.events));
    const b = await window.studioService.subscribe((x) => w.__s2.push(...x.events), { session: 's2' });
    return [a.ok, b.ok];
  });
  expect(subs).toEqual([true, true]);
  fake.emit('s1', 'iteration', { index: 1 });
  fake.emit('s2', 'iteration', { index: 2 });
  const got = await until(
    () =>
      page.evaluate(() => {
        const w = window as unknown as { __all: { session: string }[]; __s2: { session: string }[] };
        return w.__all.length >= 2 ? { all: w.__all.map((e) => e.session), s2: w.__s2.map((e) => e.session) } : null;
      }),
    10_000,
    'the events in the page',
  );
  expect(got).toEqual({ all: ['s1', 's2'], s2: ['s2'] });
});

test('every allow-listed method is forwarded unchanged (positive control of the list)', async () => {
  const before = fake.received.length;
  await page.evaluate(async (methods) => {
    for (const m of methods) await window.studioService.call(m as never, { probe: m });
  }, [...SERVICE_METHODS]);
  const arrived = fake.received.slice(before).filter((r) => r.method !== 'events.poll');
  expect(arrived.map((r) => r.method)).toEqual([...SERVICE_METHODS]);
  expect(arrived.every((r) => (r.params as { probe: string }).probe === r.method)).toBe(true);
});
