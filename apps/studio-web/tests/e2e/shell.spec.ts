// The hardened shell (docs/studio-guardrails.md G10), asserted on the packaged app (out/packaged, built by
// scripts/package-dir.mjs) driven by Playwright's _electron.launch. One app for the file; every test leaves it on the
// empty project view.
import { _electron as electron, expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { APP_ENTRY, CSP_HEADER, CSP_META } from '../../src/main/security.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const ALLOWED_CHANNELS = ['app.info', 'app.ready'];

let app: ElectronApplication;
let page: Page;

test.beforeAll(async () => {
  const { executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8')) as { executable: string };
  app = await electron.launch({ executablePath: executable, args: [] });
  page = await app.firstWindow();
  await expect(page.getByRole('heading', { name: 'No project open' })).toBeVisible();
});

test.afterAll(async () => {
  await app?.close();
});

/** The URL the main process sees for the one window, and how many windows there are. */
const mainView = () =>
  app.evaluate(({ BrowserWindow }) => {
    const all = BrowserWindow.getAllWindows();
    return { windows: all.length, url: all[0]?.webContents.getURL() ?? '' };
  });

test('the page is the app at app://studio/, packaged, on the pinned Electron', async () => {
  expect(page.url()).toBe(APP_ENTRY);
  const info = await page.evaluate(() => window.studio.app.info());
  expect(info.name).toBe('rcfd Studio');
  expect(info.electron).toBe('45.3.3');
  expect(info.packaged).toBe(true);
});

test('the window is sandboxed and context-isolated, with Node integration off and web security on', async () => {
  const prefs = await app.evaluate(({ app: a, BrowserWindow }) => {
    const wc = BrowserWindow.getAllWindows()[0]!.webContents;
    // Not in Electron's public types, but the settings the renderer was actually created with (not the ones asked for).
    const p = (wc as unknown as { getLastWebPreferences(): Record<string, unknown> | null }).getLastWebPreferences();
    const metric = a.getAppMetrics().find((m) => m.pid === wc.getOSProcessId());
    return {
      sandbox: p?.sandbox,
      contextIsolation: p?.contextIsolation,
      nodeIntegration: p?.nodeIntegration,
      nodeIntegrationInWorker: p?.nodeIntegrationInWorker,
      nodeIntegrationInSubFrames: p?.nodeIntegrationInSubFrames,
      webSecurity: p?.webSecurity,
      allowRunningInsecureContent: p?.allowRunningInsecureContent,
      webviewTag: p?.webviewTag,
      experimentalFeatures: p?.experimentalFeatures,
      // ProcessMetric.sandboxed is reported on Windows and macOS only.
      osSandboxed: process.platform === 'linux' ? true : metric?.sandboxed,
      rendererType: metric?.type,
    };
  });
  expect(prefs).toEqual({
    sandbox: true,
    contextIsolation: true,
    nodeIntegration: false,
    nodeIntegrationInWorker: false,
    nodeIntegrationInSubFrames: false,
    webSecurity: true,
    allowRunningInsecureContent: false,
    webviewTag: false,
    experimentalFeatures: false,
    osSandboxed: true,
    rendererType: 'Tab',
  });
});

test('the page has no Node: no require, process, module, Buffer or ipcRenderer', async () => {
  const globals = await page.evaluate(() => {
    const w = window as unknown as Record<string, unknown>;
    return ['require', 'process', 'module', 'exports', 'Buffer', 'global', 'ipcRenderer', 'electron'].map(
      (k) => `${k}:${typeof w[k]}`,
    );
  });
  expect(globals).toEqual([
    'require:undefined',
    'process:undefined',
    'module:undefined',
    'exports:undefined',
    'Buffer:undefined',
    'global:undefined',
    'ipcRenderer:undefined',
    'electron:undefined',
  ]);
});

test('the bridge is the allow-list: window.studio.app.{info, ready} and nothing else', async () => {
  const shape = await page.evaluate(() => {
    const s = window.studio as unknown as Record<string, Record<string, unknown>>;
    const app = s['app'] ?? {};
    return {
      top: Object.keys(s).sort(),
      app: Object.keys(app).sort(),
      types: Object.values(app).map((v) => typeof v),
      // A context-isolated bridge is a frozen copy: the page cannot add to it or replace a function.
      frozen: Object.isFrozen(s) && Object.isFrozen(app),
    };
  });
  expect(shape).toEqual({ top: ['app'], app: ['info', 'ready'], types: ['function', 'function'], frozen: true });
});

test('the main process handles exactly the allow-listed channels and refuses foreign senders', async () => {
  const ipc = await app.evaluate(async ({ ipcMain }) => {
    // Electron keeps ipcMain.handle's handlers in a private map; there is no public way to list them.
    const handlers = (ipcMain as unknown as { _invokeHandlers?: Map<string, (...a: unknown[]) => unknown> })._invokeHandlers;
    // Electron registers its own handlers there too, named in capitals (IMPORT_SHARED_TEXTURE_..._RENDERER_TO_MAIN);
    // they are reachable only through ipcRenderer, which the page does not have.
    const names = handlers ? [...handlers.keys()].filter((n) => !/^[A-Z0-9_]+$/.test(n)).sort() : null;
    const call = async (channel: string, url: string, args: unknown[]) => {
      try {
        await handlers?.get(channel)?.({ senderFrame: { url } }, ...args);
        return 'ok';
      } catch (e) {
        return String(e);
      }
    };
    return {
      names,
      // ipcMain.on listeners: none of ours (Electron adds an 'error' listener of its own).
      listeners: ipcMain.eventNames().map(String).filter((n) => n !== 'error'),
      own: await call('app.info', 'app://studio/index.html', []),
      foreign: await call('app.info', 'https://example.com/', []),
      otherHost: await call('app.info', 'app://other/index.html', []),
      withArgs: await call('app.info', 'app://studio/index.html', ['x']),
    };
  });
  expect(ipc.names).toEqual(ALLOWED_CHANNELS);
  expect(ipc.listeners).toEqual([]);
  expect(ipc.own).toBe('ok');
  expect(ipc.foreign).toContain('refused');
  expect(ipc.otherHost).toContain('refused');
  expect(ipc.withArgs).toContain('takes no arguments');
});

test('the Content Security Policy is sent as a header and repeated in a meta element', async () => {
  const csp = await page.evaluate(async () => {
    const res = await fetch(location.href);
    const meta = document.querySelector('meta[http-equiv="Content-Security-Policy"]');
    return { header: res.headers.get('content-security-policy'), meta: meta?.getAttribute('content') ?? null };
  });
  expect(csp.header).toBe(CSP_HEADER);
  expect(csp.meta).toBe(CSP_META);
});

test('the policy blocks inline script, eval and Function', async () => {
  const r = await page.evaluate(async () => {
    const seen: string[] = [];
    const onViolation = (e: SecurityPolicyViolationEvent) => seen.push(e.effectiveDirective);
    document.addEventListener('securitypolicyviolation', onViolation);
    const w = window as unknown as Record<string, unknown>;
    const s = document.createElement('script');
    s.textContent = 'window.__inlineRan = true;';
    document.head.appendChild(s);
    // CDP lets the code of page.evaluate itself call eval (Runtime.evaluate's allowUnsafeEvalBlockedByCSP), so the
    // eval checks run in a later task, as page code would.
    const later = <T,>(f: () => T): Promise<T> => new Promise((done) => setTimeout(() => done(f()), 0));
    const blocked = (f: () => unknown) => () => {
      try {
        f();
        return false;
      } catch (e) {
        return e instanceof EvalError;
      }
    };
    const evalBlocked = await later(blocked(() => (0, eval)('1')));
    const functionBlocked = await later(blocked(() => new Function('return 1')()));
    // A string handed to setTimeout is compiled like eval.
    (setTimeout as unknown as (code: string, ms: number) => number)('window.__timerStringRan = true', 0);
    await new Promise((done) => setTimeout(done, 200));
    const timerStringRan = w['__timerStringRan'] === true;
    document.removeEventListener('securitypolicyviolation', onViolation);
    s.remove();
    return {
      inlineRan: w['__inlineRan'] === true,
      evalBlocked,
      functionBlocked,
      timerStringRan,
      seen: [...new Set(seen)].sort(),
    };
  });
  expect(r).toEqual({
    inlineRan: false,
    evalBlocked: true,
    functionBlocked: true,
    timerStringRan: false,
    seen: expect.arrayContaining(['script-src', 'script-src-elem']),
  });
});

test('the policy blocks remote fetch, images, styles and frames', async () => {
  const r = await page.evaluate(async () => {
    const seen: string[] = [];
    const onViolation = (e: SecurityPolicyViolationEvent) => seen.push(e.effectiveDirective);
    document.addEventListener('securitypolicyviolation', onViolation);
    let fetchError = '';
    try {
      await fetch('https://example.com/');
    } catch (e) {
      fetchError = String(e);
    }
    const img = document.createElement('img');
    img.src = 'https://example.com/x.png';
    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = 'https://example.com/x.css';
    const frame = document.createElement('iframe');
    frame.src = 'https://example.com/';
    document.body.append(img, link, frame);
    await new Promise((done) => setTimeout(done, 300));
    img.remove();
    link.remove();
    frame.remove();
    document.removeEventListener('securitypolicyviolation', onViolation);
    return { fetchError, seen: [...new Set(seen)].sort() };
  });
  expect(r.fetchError).toContain('TypeError');
  expect(r.seen).toEqual(expect.arrayContaining(['connect-src', 'img-src', 'style-src-elem', 'frame-src']));
  expect(await mainView()).toEqual({ windows: 1, url: APP_ENTRY });
});

test('navigation away from the app is refused: remote, file and foreign app:// URLs', async () => {
  for (const url of ['https://example.com/', 'file:///C:/Windows/win.ini', 'app://other/index.html']) {
    await page.evaluate((u) => {
      location.href = u;
    }, url);
    await page.waitForTimeout(300);
    expect(page.url(), url).toBe(APP_ENTRY);
  }
  // A followed link is a navigation too (clicked from the page: Playwright's click would wait for the navigation
  // that never comes).
  await page.evaluate(() => {
    const a = document.createElement('a');
    a.href = 'https://example.com/';
    a.textContent = 'remote';
    document.body.append(a);
    a.click();
    a.remove();
  });
  await page.waitForTimeout(300);
  expect(page.url()).toBe(APP_ENTRY);
  expect(await mainView()).toEqual({ windows: 1, url: APP_ENTRY });
  // Read from the page directly: after a refused navigation Playwright's locators wait for one that never finishes.
  expect(await page.evaluate(() => document.querySelector('h1')?.textContent)).toBe('No project open');
});

test('window.open is denied, for remote and for own URLs', async () => {
  const opened = await page.evaluate(() =>
    ['https://example.com/', 'app://studio/index.html', 'about:blank'].map((u) => window.open(u) === null),
  );
  expect(opened).toEqual([true, true, true]);
  await page.waitForTimeout(300);
  expect(app.windows()).toHaveLength(1);
  expect(await mainView()).toEqual({ windows: 1, url: APP_ENTRY });
});

test('permission requests and checks are denied', async () => {
  const r = await page.evaluate(async () => {
    const notification = await Notification.requestPermission();
    const states: string[] = [];
    for (const name of ['geolocation', 'notifications', 'camera', 'microphone'] as const) {
      try {
        states.push(`${name}:${(await navigator.permissions.query({ name: name as PermissionName })).state}`);
      } catch (e) {
        states.push(`${name}:${String(e)}`);
      }
    }
    const geolocation = await new Promise<number>((done) =>
      navigator.geolocation.getCurrentPosition(
        () => done(0),
        (e) => done(e.code),
        { timeout: 5000 },
      ),
    );
    return { notification, states, geolocation };
  });
  expect(r.notification).toBe('denied');
  expect(r.states).toEqual(['geolocation:denied', 'notifications:denied', 'camera:denied', 'microphone:denied']);
  expect(r.geolocation).toBe(1); // GeolocationPositionError.PERMISSION_DENIED
});

test('the network layer cancels remote loads, even ones the main process starts', async () => {
  const r = await app.evaluate(async ({ BrowserWindow }) => {
    const probe = new BrowserWindow({ show: false, webPreferences: { sandbox: true, contextIsolation: true } });
    const results: string[] = [];
    for (const url of ['https://example.com/', 'http://127.0.0.1:9/', 'file:///C:/Windows/win.ini']) {
      try {
        await probe.loadURL(url);
        results.push(`${url} loaded`);
      } catch (e) {
        results.push(`${url} ${String((e as { code?: string }).code ?? e)}`);
      }
    }
    probe.destroy();
    return results;
  });
  expect(r).toEqual([
    'https://example.com/ ERR_BLOCKED_BY_CLIENT',
    'http://127.0.0.1:9/ ERR_BLOCKED_BY_CLIENT',
    'file:///C:/Windows/win.ini ERR_BLOCKED_BY_CLIENT',
  ]);
});
