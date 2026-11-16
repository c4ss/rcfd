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
// - The solver service (F-803): this process holds the connections (src/main/service/); after each load of an app page
//   the page's preload script gets one MessagePort on SERVICE_PORT_CHANNEL, and every message on it is checked against
//   the service allow-list of src/shared/service.ts (src/main/service/bridge.ts). The renderer has no other way to it.
// - The project folder (F-805): the same way, a second MessagePort on PROJECT_PORT_CHANNEL (src/main/project/), checked
//   against src/shared/project.ts. The folder is read through the engine's project server (rcfd-mcp, read-only).
//
// Service and file I/O live in this process, never in the renderer.
import {
  app,
  BrowserWindow,
  dialog,
  ipcMain,
  MessageChannelMain,
  protocol,
  session,
  type IpcMainInvokeEvent,
  type MessagePortMain,
  type WebContents,
} from 'electron';
import { statSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { CHANNELS, type AppInfo, type Channel, type IpcContract } from '../shared/ipc.js';
import { SERVICE_PORT_CHANNEL } from '../shared/service.js';
import { PROJECT_PORT_CHANNEL } from '../shared/project.js';
import { ProjectPortBridge, type ProjectPortLike } from './project/bridge.js';
import { engineConfig, recentFile } from './project/config.js';
import { ProjectHome } from './project/home.js';
import { RecentStore } from './project/recent.js';
import { PortBridge, type PortLike } from './service/bridge.js';
import { ServiceClient } from './service/client.js';
import { serviceConfig } from './service/config.js';
import { DataPorts } from './data/port.js';
import { DataRoots } from './data/roots.js';
import { StudioData } from './data/studio-data.js';
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

/** The solver service (src/main/service/config.ts names the environment variables that choose it). */
const service = new ServiceClient(serviceConfig(process.env, path.join(app.getPath('userData'), 'service')));
/** Bulk field data (F-804): R2 reads local stores inside the data roots, R1 goes through the service (src/main/data/). */
const data = new StudioData(service, DataRoots.fromEnv(process.env));
const dataPorts = new DataPorts(data);
/** P1 and P2 measurement only (scripts/measure-service.mjs): the client, reachable from Playwright's main-process evaluate. */
if (process.env['RCFD_STUDIO_BENCH'] === '1') (globalThis as { rcfdStudioBench?: unknown }).rcfdStudioBench = { service };

/** The project home (F-805): the engine's project server from the environment (src/main/project/config.ts). */
const projects = new ProjectHome({
  engine: engineConfig(process.env, (p) => {
    try {
      return statSync(p).isFile();
    } catch {
      return false;
    }
  }),
  recent: new RecentStore(recentFile(process.env, app.getPath('userData'))),
  pickFolder: async () => {
    const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    const options = { title: 'Open project folder', properties: ['openDirectory' as const] };
    const r = win ? await dialog.showOpenDialog(win, options) : await dialog.showOpenDialog(options);
    return r.canceled ? null : (r.filePaths[0] ?? null);
  },
});

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

/** The service port of each web contents: replaced on every load, closed when the contents go away. */
const servicePorts = new Map<number, PortBridge>();

const adaptPort = (p: MessagePortMain): PortLike => ({
  postMessage: (m) => p.postMessage(m),
  onMessage: (listener) => {
    p.on('message', (e) => listener(e.data));
  },
  onClose: (listener) => {
    p.on('close', listener);
  },
  close: () => p.close(),
});

/** The project port of each web contents (F-805), like the service port. */
const projectPorts = new Map<number, ProjectPortBridge>();

const adaptProjectPort = (p: MessagePortMain): ProjectPortLike => ({
  postMessage: (m) => p.postMessage(m),
  onMessage: (listener) => {
    p.on('message', (e) => listener(e.data));
  },
  onClose: (listener) => {
    p.on('close', listener);
  },
  close: () => p.close(),
});

function offerProjectPort(contents: WebContents): void {
  projectPorts.get(contents.id)?.dispose();
  projectPorts.delete(contents.id);
  if (contents.isDestroyed() || !isAppUrl(contents.getURL())) return;
  const { port1, port2 } = new MessageChannelMain();
  projectPorts.set(contents.id, new ProjectPortBridge(adaptProjectPort(port1), projects));
  port1.start();
  contents.postMessage(PROJECT_PORT_CHANNEL, null, [port2]);
}

/** After a load of one of our own pages: a new port for its preload script, and the old one closed. */
function offerServicePort(contents: WebContents): void {
  servicePorts.get(contents.id)?.dispose();
  servicePorts.delete(contents.id);
  if (contents.isDestroyed() || !isAppUrl(contents.getURL())) return;
  const { port1, port2 } = new MessageChannelMain();
  servicePorts.set(contents.id, new PortBridge(adaptPort(port1), service));
  port1.start();
  contents.postMessage(SERVICE_PORT_CHANNEL, null, [port2]);
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
  const contents = win.webContents;
  const id = contents.id;
  // dom-ready: the preload script has run and listens for the port; it queues the page's calls until then
  contents.on('dom-ready', () => {
    offerServicePort(contents);
    offerProjectPort(contents);
    dataPorts.offer(contents);
  });
  contents.once('destroyed', () => {
    dataPorts.drop(id);
    servicePorts.get(id)?.dispose();
    servicePorts.delete(id);
    projectPorts.get(id)?.dispose();
    projectPorts.delete(id);
  });
  win.once('ready-to-show', () => win.show());
  void win.loadURL(APP_ENTRY);
}

void app.whenReady().then(() => {
  hardenSession();
  registerIpc();
  service.start();
  void projects.init();
  openWindow();
});
app.on('window-all-closed', () => app.quit());

// Quitting: the client stops; a service the Studio started is stopped too unless it holds sessions (G7, client.ts).
let serviceStopped = false;
app.on('before-quit', (e) => {
  if (serviceStopped) return;
  e.preventDefault();
  serviceStopped = true;
  const atMost = new Promise((done) => setTimeout(done, 3000));
  // the project server of an open project ends with the Studio: it only reads, and holds no run (G7)
  void Promise.race([Promise.all([service.shutdown(), projects.shutdown()]), atMost])
    .catch(() => undefined)
    .finally(() => app.quit());
});
