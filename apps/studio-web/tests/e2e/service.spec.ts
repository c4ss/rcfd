// The service client against the real `rcfd serve` (F-803): service.info and an events.poll event from the renderer,
// reconnection after the service is killed and started again (attached to it, and started by the Studio itself), and
// G7: a reload does not touch the service, and a service the Studio started outlives the Studio while it holds sessions.
//
// Needs the rcfd binary (tests/support/rcfd.ts says where it is looked for); without it these tests are skipped with
// that reason.
import { expect, test } from '@playwright/test';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { SERVICE_METHODS } from '../../src/shared/service.js';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { spawn } from 'node:child_process';
import { until } from '../support/fake-service.js';
import { alive, boxToml, findRcfd, killIfRcfd, killPid, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, pageEvents, pageStates, quitLeavingService, recordInPage, serviceState, untilState, type Studio } from '../support/studio.js';
import { emptyProject, projectState, stubDialog } from '../support/project.js';

const rcfd = findRcfd();
test.skip(rcfd === null, NO_RCFD);

/** Processes to kill whatever happens: services started by the tests or by the Studio under test. */
const leftovers = new Set<number>();
const scratch: string[] = [];
test.afterAll(() => {
  for (const pid of leftovers) killIfRcfd(pid);
  for (const d of scratch) rmSync(d, { recursive: true, force: true });
});

const tempDir = () => {
  const d = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f803-'));
  scratch.push(d);
  return d;
};

test.describe('rcfd serve without the Studio', () => {
  test('the allow-list names methods the service has (data.info only when data.* is on, help.* only with --ui)', async () => {
    const s = await startRcfd(rcfd!);
    leftovers.add(s.pid);
    const c = await RpcConnection.connectTcp('127.0.0.1', s.port);
    const info = (await c.request('service.info')) as { protocol: string; methods: string[] };
    c.close();
    await s.stop();
    expect(info.protocol).toBe('rcfd-service/1');
    // restated for ADR 0737: case.facts and mesh.refine are --ui methods too
    expect(SERVICE_METHODS.filter((m) => !info.methods.includes(m))).toEqual(['data.info', 'help.glossary', 'help.errors', 'case.facts', 'mesh.refine']);
    // ADR 0730: the switch the Studio's spawn passes adds the help methods and the ui key
    const u = await startRcfd(rcfd!, 0, ['--ui']);
    leftovers.add(u.pid);
    const cu = await RpcConnection.connectTcp('127.0.0.1', u.port);
    const withUi = (await cu.request('service.info')) as { ui?: string; methods: string[] };
    cu.close();
    await u.stop();
    expect(withUi.ui).toBe('rcfd-ui/1');
    expect(SERVICE_METHODS.filter((m) => !withUi.methods.includes(m))).toEqual(['data.info']);
  });

  test('the client over stdio: `rcfd serve --stdio` answers service.info and run.status', async () => {
    const child = spawn(rcfd!, ['serve', '--stdio'], { stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
    leftovers.add(child.pid!);
    const c = new RpcConnection(child.stdout, child.stdin, {}, () => child.stdin.end());
    const info = (await c.request('service.info')) as { protocol: string };
    const { session } = (await c.request('session.create', { toml: boxToml([4, 4, 4]) })) as { session: string };
    const status = (await c.request('run.status', { session })) as { state: string };
    c.close();
    await new Promise((r) => child.once('exit', r));
    expect(info.protocol).toBe('rcfd-service/1');
    expect(status.state).toBe('defined');
  });
});

test.describe('attached to a running service', () => {
  let svc: RunningRcfd;
  let studio: Studio;

  test.beforeAll(async () => {
    svc = await startRcfd(rcfd!);
    leftovers.add(svc.pid);
    studio = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${svc.port}` });
    await untilState(studio.page, (s) => s.status === 'connected', 15_000, 'connected');
    await recordInPage(studio.page);
  });

  test.afterAll(async () => {
    await studio?.app.close();
    await svc?.stop();
  });

  test('service.info: a round trip from the renderer to rcfd serve and back', async () => {
    const info = await studio.page.evaluate(() => window.studioService.info());
    expect(info.ok).toBe(true);
    if (!info.ok) return;
    expect(info.result).toMatchObject({ name: 'rcfd', protocol: 'rcfd-service/1', protocol_version: '1.1' });
    const state = await studio.page.evaluate(() => window.studioService.state());
    expect(state).toMatchObject({ status: 'connected', mode: 'attach', address: `127.0.0.1:${svc.port}`, instance: 1, version: info.result.version });
  });

  test('events.poll: the events of a session reach the renderer', async () => {
    const page = studio.page;
    const created = await page.evaluate((toml) => window.studioService.call('session.create', { toml }), boxToml([4, 4, 4]));
    expect(created.ok).toBe(true);
    const session = (created as { ok: true; result: { session: string } }).result.session;
    expect((await page.evaluate((s) => window.studioService.call('run.initialize', { session: s }), session)).ok).toBe(true);
    expect((await page.evaluate((s) => window.studioService.call('run.step', { session: s, count: 3 }), session)).ok).toBe(true);
    const kinds = await until(async () => {
      const k = (await pageEvents(page)).filter((e) => e.session === session).map((e) => e.kind);
      return k.filter((x) => x === 'iteration').length === 3 ? k : null;
    }, 15_000, 'the events of the session');
    expect(kinds).toEqual(['created', 'initialized', 'iteration', 'iteration', 'iteration']);
    const seqs = (await pageEvents(page)).map((e) => e.seq);
    expect(seqs).toEqual([...seqs].sort((a, b) => a - b));
  });

  test('a call outside the allow-list is refused before the service: run.wait would block the connection', async () => {
    const r = await studio.page.evaluate(() => (window.studioService.call as (m: string, p: object) => Promise<unknown>)('run.wait', { session: 's1', timeout_ms: 60_000 }));
    expect(r).toMatchObject({ ok: false, error: { code: 'not_allowed' } });
  });

  test('the service is killed and started again on the same port: the Studio reconnects to the new instance', async () => {
    const page = studio.page;
    const port = svc.port;
    await svc.stop();
    await untilState(page, (s) => s.status === 'disconnected', 10_000, 'disconnected');
    const down = await page.evaluate(() => window.studioService.call('service.info'));
    expect(down).toMatchObject({ ok: false, error: { code: 'disconnected' } });
    svc = await until(() => startRcfd(rcfd!, port).catch(() => null), 10_000, 'rcfd on the same port');
    leftovers.add(svc.pid);
    const s = await untilState(page, (x) => x.status === 'connected' && x.instance === 2, 15_000, 'reconnected');
    expect(s.address).toBe(`127.0.0.1:${port}`);
    const statuses = (await pageStates(page)).map((x) => x.status);
    expect(statuses).toContain('disconnected');
    expect(statuses.at(-1)).toBe('connected');
    expect((await page.evaluate(() => window.studioService.info())).ok).toBe(true);
    // the new instance numbers its events from 1 again; the Studio's cursor followed
    const created = await page.evaluate((toml) => window.studioService.call('session.create', { toml }), boxToml([4, 4, 4]));
    const session = (created as { ok: true; result: { session: string } }).result.session;
    const e = await until(async () => (await pageEvents(page)).find((x) => x.session === session && x.instance === 2), 10_000, 'the created event');
    expect(e).toMatchObject({ kind: 'created', seq: 1, instance: 2 });
  });
});

test.describe('a service the Studio starts itself', () => {
  test('starts it, and when it is killed starts a new one and reconnects', async () => {
    const stateDir = tempDir();
    const studio = await launchStudio({ RCFD_STUDIO_RCFD: rcfd!, RCFD_STUDIO_STATE_DIR: stateDir });
    try {
      const first = await untilState(studio.page, (s) => s.status === 'connected', 20_000, 'connected');
      expect(first).toMatchObject({ mode: 'spawn', instance: 1, adopted: false });
      expect(first.pid).not.toBeNull();
      leftovers.add(first.pid!);
      expect(first.address).toMatch(/^127\.0\.0\.1:\d+$/);
      const record = JSON.parse(readFileSync(path.join(stateDir, 'service.json'), 'utf8'));
      expect(record).toMatchObject({ pid: first.pid, host: '127.0.0.1', rcfd });
      await recordInPage(studio.page);
      killPid(first.pid!);
      const second = await untilState(studio.page, (s) => s.status === 'connected' && s.instance === 2, 20_000, 'a new service');
      leftovers.add(second.pid!);
      expect(second.pid).not.toBe(first.pid);
      expect(alive(first.pid!)).toBe(false);
      const created = await studio.page.evaluate((toml) => window.studioService.call('session.create', { toml }), boxToml([4, 4, 4]));
      expect(created.ok).toBe(true);
      const e = await until(async () => (await pageEvents(studio.page)).find((x) => x.kind === 'created' && x.instance === 2), 10_000, 'the created event');
      expect(e.seq).toBe(1);
      // it holds a session now, so it outlives the Studio (next test); here it is stopped by hand
      const { closed } = await quitLeavingService(studio);
      expect(alive(second.pid!)).toBe(true);
      killPid(second.pid!);
      await closed;
    } catch (e) {
      await studio.app.close().catch(() => undefined);
      throw e;
    }
  });

  test('runs outlive the window (G7): a reload keeps the service; with a session it outlives the Studio and is found again', async () => {
    const stateDir = tempDir();
    let studio = await launchStudio({ RCFD_STUDIO_RCFD: rcfd!, RCFD_STUDIO_STATE_DIR: stateDir });
    const first = await untilState(studio.page, (s) => s.status === 'connected', 20_000, 'connected');
    leftovers.add(first.pid!);
    // a run that is still running seconds later: 16^3 cells, tolerances below what the solver reaches
    const created = await studio.page.evaluate((toml) => window.studioService.call('session.create', { toml }), boxToml([16, 16, 16]));
    const session = (created as { ok: true; result: { session: string } }).result.session;
    expect((await studio.page.evaluate((s) => window.studioService.call('run.start', { session: s, max_steps: 1_000_000 }), session)).ok).toBe(true);

    await studio.page.reload();
    await studio.page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
    const afterReload = await untilState(studio.page, (s) => s.status === 'connected');
    expect(afterReload).toMatchObject({ pid: first.pid, instance: 1 });
    const running = await studio.page.evaluate((s) => window.studioService.call('run.status', { session: s }), session);
    expect(running).toMatchObject({ ok: true, result: { state: 'running' } });

    const firstQuit = await quitLeavingService(studio);
    expect(alive(first.pid!)).toBe(true);
    expect(existsSync(path.join(stateDir, 'service.json'))).toBe(true);

    studio = await launchStudio({ RCFD_STUDIO_RCFD: rcfd!, RCFD_STUDIO_STATE_DIR: stateDir });
    const again = await untilState(studio.page, (s) => s.status === 'connected', 20_000, 'connected again');
    expect(again).toMatchObject({ pid: first.pid, adopted: true, instance: 1, address: first.address });
    const status = await studio.page.evaluate((s) => window.studioService.call('run.status', { session: s }), session);
    expect(status).toMatchObject({ ok: true, result: { state: 'running' } });
    expect((await studio.page.evaluate((s) => window.studioService.call('run.stop', { session: s }), session)).ok).toBe(true);
    // still a session (stopped): still left running
    const secondQuit = await quitLeavingService(studio);
    expect(alive(first.pid!)).toBe(true);
    killPid(first.pid!);
    await Promise.all([firstQuit.closed, secondQuit.closed]);
  });

  test('without sessions, the service the Studio started stops when the Studio quits', async () => {
    const stateDir = tempDir();
    const studio = await launchStudio({ RCFD_STUDIO_RCFD: rcfd!, RCFD_STUDIO_STATE_DIR: stateDir });
    const s = await untilState(studio.page, (x) => x.status === 'connected', 20_000, 'connected');
    leftovers.add(s.pid!);
    await studio.app.close();
    await until(() => !alive(s.pid!), 10_000, 'the service to stop');
    expect(existsSync(path.join(stateDir, 'service.json'))).toBe(false);
  });
});

// F-820 (ADR 0801 Addendum 19): an installed Studio, no variable set, uses the engine it ships with in <resources>/bin.
// out/packaged has no bin folder, so RCFD_TEST_BUNDLED_BIN (honoured in test mode only) names the folder of the built
// engine in its place. Its service waits for the first project instead of starting at launch, and G7 still holds.
test.describe('an installed Studio with its shipped engine', () => {
  const bin = rcfd ? path.dirname(rcfd) : '';
  const mcpExe = process.platform === 'win32' ? 'rcfd-mcp.exe' : 'rcfd-mcp';
  test('rcfd serve starts with the first project, not at launch; with a session it outlives the Studio and is found again at launch (G7)', async () => {
    test.skip(!existsSync(path.join(bin, mcpExe)), `no ${mcpExe} beside ${rcfd}`);
    const stateDir = tempDir();
    const folder = await emptyProject(tempDir(), 'F-820 bundled');
    const env = { RCFD_TEST_BUNDLED_BIN: bin, RCFD_STUDIO_STATE_DIR: stateDir, RCFD_STUDIO_RECENT: path.join(stateDir, 'recent.json') };
    let studio = await launchStudio(env);
    try {
      // the engine from the shipped folder; the service waits, nothing is started at launch
      expect((await projectState(studio.page)).engine).toEqual({ configured: true, path: path.join(bin, mcpExe), from: 'bundled' });
      await studio.page.waitForTimeout(1000);
      expect(await serviceState(studio.page)).toMatchObject({ status: 'waiting', mode: 'spawn', pid: null });
      expect(existsSync(path.join(stateDir, 'service.json'))).toBe(false);
      // the first project starts it
      await stubDialog(studio.app, folder);
      await studio.page.evaluate(() => window.studioProject.pick());
      await until(async () => (await projectState(studio.page)).project, 30_000, 'the project to open');
      const first = await untilState(studio.page, (s) => s.status === 'connected', 20_000, 'connected');
      leftovers.add(first.pid!);
      expect(first).toMatchObject({ mode: 'spawn', adopted: false, instance: 1 });
      const record = JSON.parse(readFileSync(path.join(stateDir, 'service.json'), 'utf8'));
      expect(record).toMatchObject({ pid: first.pid, rcfd: path.join(bin, path.basename(rcfd!)) });
      const created = await studio.page.evaluate((toml) => window.studioService.call('session.create', { toml }), boxToml([4, 4, 4]));
      const session = (created as { ok: true; result: { session: string } }).result.session;
      const firstQuit = await quitLeavingService(studio);
      expect(alive(first.pid!)).toBe(true);

      // the next start finds it again at launch, before any project is opened
      studio = await launchStudio(env);
      const again = await untilState(studio.page, (s) => s.status === 'connected', 20_000, 'found again at launch');
      expect(again).toMatchObject({ pid: first.pid, adopted: true, instance: 1, address: first.address });
      expect((await projectState(studio.page)).project).toBeNull();
      const listed = await studio.page.evaluate(() => window.studioService.call('session.list', {}));
      expect(JSON.stringify(listed)).toContain(session);
      const secondQuit = await quitLeavingService(studio);
      expect(alive(first.pid!)).toBe(true);
      killPid(first.pid!);
      await Promise.all([firstQuit.closed, secondQuit.closed]);
    } catch (e) {
      await studio.app.close().catch(() => undefined);
      throw e;
    }
  });
});
