// Main process of rcfd Studio: the hardened shell of docs/studio-guardrails.md G10 (F-801).
//
// - Every renderer is sandboxed (app.enableSandbox and the window's own settings), context-isolated, with Node
//   integration off and web security on.
// - The renderer is served from app://studio/ by a protocol handler that only reads the built renderer directory.
//   Each response carries the Content Security Policy header (and src/renderer/index.html repeats it in a <meta>).
// - No remote content: requests to http(s), ws(s), ftp and file are cancelled at the network layer; navigation away
//   from app://studio/ and every window.open are refused; <webview> cannot be attached; permission requests and checks
//   are denied.
// - IPC: one ipcMain.handle per channel of the allow-list in src/shared/ipc.ts, and only for senders that are our own
//   pages.
//
// Service and file I/O will live in this process (F-803 onwards), never in the renderer.
import { app, BrowserWindow, ipcMain, protocol, session, type IpcMainInvokeEvent, type WebContents } from 'electron';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { CHANNELS, type AppInfo, type Channel, type IpcContract } from '../shared/ipc.js';
import {
  APP_ENTRY,
  APP_SCHEME,
  CSP_HEADER,
  contentType,
  isAppUrl,
  isBlockedRequest,
  resolveAppFile,
} from './security.js';

const beside = (rel: string): string => fileURLToPath(new URL(rel, import.meta.url));
const RENDERER_DIR = beside('../renderer/');
const PRELOAD = beside('../preload/index.cjs');

/** P10 measurement only (scripts/measure-cold-start.mjs): write the ready time to this file, then quit. */
const STARTUP_REPORT = process.env['RCFD_STUDIO_STARTUP_REPORT'] ?? '';

// Before ready: the scheme must be privileged to behave like a secure web origin (fetch, CSP 'self', module scripts).
protocol.registerSchemesAsPrivileged([
  { scheme: APP_SCHEME, privileges: { standard: true, secure: true, supportFetchAPI: true } },
]);
app.enableSandbox();

// Every web contents, whoever creates it: no navigation away from the app, no new windows, no <webview>.
app.on('web-contents-created', (_event, contents: WebContents) => {
  const refuseForeign = (e: { preventDefault(): void }, url: string): void => {
    if (!isAppUrl(url)) e.preventDefault();
  };
  contents.on('will-navigate', (e) => refuseForeign(e, e.url));
  contents.on('will-frame-navigate', (e) => refuseForeign(e, e.url));
  contents.on('will-redirect', (e) => refuseForeign(e, e.url));
  contents.on('will-attach-webview', (e) => e.preventDefault());
  contents.setWindowOpenHandler(() => ({ action: 'deny' }));
});

async function serveApp(request: Request): Promise<Response> {
  const file = resolveAppFile(RENDERER_DIR, request.url);
  if (file === null) return new Response('not found', { status: 404, headers: { 'content-type': 'text/plain' } });
  try {
    const body = await readFile(file);
    return new Response(new Uint8Array(body), {
      status: 200,
      headers: {
        'content-type': contentType(file),
        'content-security-policy': CSP_HEADER,
        'x-content-type-options': 'nosniff',
      },
    });
  } catch {
    return new Response('not found', { status: 404, headers: { 'content-type': 'text/plain' } });
  }
}

function hardenSession(): void {
  const ses = session.defaultSession;
  ses.protocol.handle(APP_SCHEME, serveApp);
  ses.webRequest.onBeforeRequest((details, callback) => callback({ cancel: isBlockedRequest(details.url) }));
  ses.webRequest.onHeadersReceived((details, callback) => {
    const headers: Record<string, string[]> = {};
    for (const [k, v] of Object.entries(details.responseHeaders ?? {})) {
      if (k.toLowerCase() !== 'content-security-policy') headers[k] = v;
    }
    headers['Content-Security-Policy'] = [CSP_HEADER];
    callback({ responseHeaders: headers });
  });
  ses.setPermissionRequestHandler((_wc, _permission, callback) => callback(false));
  ses.setPermissionCheckHandler(() => false);
  ses.setDevicePermissionHandler(() => false);
}

let readyReported = false;

/** Refuses a call that carries arguments: the renderer is not trusted to send the shapes it is typed to send. */
function noArgs(channel: Channel, args: unknown[]): void {
  if (args.length !== 0) throw new Error(`${channel}: takes no arguments`);
}

type Handler<C extends Channel> = (args: unknown[]) => IpcContract[C]['reply'] | Promise<IpcContract[C]['reply']>;

/** One handler per channel of the allow-list. Each checks its own arguments, which arrive from the renderer untyped. */
const handlers: { [C in Channel]: Handler<C> } = {
  'app.info': (args) => {
    noArgs('app.info', args);
    return appInfo();
  },
  'app.ready': async (args) => {
    noArgs('app.ready', args);
    if (readyReported) return;
    readyReported = true;
    const readyUnixMs = Date.now();
    if (STARTUP_REPORT) {
      await writeFile(STARTUP_REPORT, JSON.stringify({ readyUnixMs, electron: process.versions.electron }));
      setTimeout(() => app.quit(), 50);
    }
  },
};

function appInfo(): AppInfo {
  return {
    name: 'rcfd Studio',
    version: app.getVersion(),
    electron: process.versions.electron,
    chromium: process.versions.chrome,
    node: process.versions.node,
    platform: process.platform,
    arch: process.arch,
    packaged: app.isPackaged,
  };
}

/** Only our own pages may call: the sending frame must still be on app://studio/. */
function trusted(event: IpcMainInvokeEvent): boolean {
  const frame = event.senderFrame;
  return frame !== null && isAppUrl(frame.url);
}

function registerIpc(): void {
  for (const channel of CHANNELS) {
    const handler: (args: unknown[]) => unknown = handlers[channel];
    ipcMain.handle(channel, (event, ...args: unknown[]) => {
      if (!trusted(event)) throw new Error(`${channel}: refused, the sender is not an rcfd Studio page`);
      return handler(args);
    });
  }
}

function openWindow(): void {
  const win = new BrowserWindow({
    width: 1280,
    height: 800,
    show: false,
    webPreferences: {
      preload: PRELOAD,
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      nodeIntegrationInWorker: false,
      nodeIntegrationInSubFrames: false,
      webSecurity: true,
      allowRunningInsecureContent: false,
      webviewTag: false,
      experimentalFeatures: false,
      navigateOnDragDrop: false,
      safeDialogs: true,
    },
  });
  win.once('ready-to-show', () => win.show());
  void win.loadURL(APP_ENTRY);
}

void app.whenReady().then(() => {
  hardenSession();
  registerIpc();
  openWindow();
});
app.on('window-all-closed', () => app.quit());
