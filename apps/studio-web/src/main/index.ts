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
// - The case editor (F-812): a third MessagePort on CASE_PORT_CHANNEL (src/main/case/), checked against
//   src/shared/case.ts. Saving and making cases go through a project server started without --read-only, on which this
//   process calls only the tools of CASE_TOOLS.
// - The run workspace (F-810): a MessagePort on RUN_PORT_CHANNEL (src/main/run/), checked against
//   src/shared/run.ts; a run's histories are read by the open project's read-only project server. Opening a project
//   makes its folder a data root, so the workspace reads its runs' stores (runs/<id>/fields) through route local.
// - Claude Code (F-814): a MessagePort on CLAUDE_PORT_CHANNEL (src/main/claude/), checked against src/shared/claude.ts;
//   claude is started only here, with the project folder as its working folder.
// - Provenance (F-828): a MessagePort on PROVENANCE_PORT_CHANNEL (src/main/provenance/), checked against
//   src/shared/provenance.ts; a run's record is read by the open project's read-only project server, and Copy writes the
//   system clipboard here (the page has no clipboard permission).
// - The theme (F-818): theme.get and theme.set of the allow-list; the setting is kept in the user data
//   (src/main/theme.ts), and nativeTheme.themeSource and each window's background colour follow it, so a dark start
//   shows no white ground before the page paints.
//
// Service and file I/O live in this process, never in the renderer.
import {
  app,
  BrowserWindow,
  clipboard,
  dialog,
  ipcMain,
  MessageChannelMain,
  nativeTheme,
  protocol,
  session,
  shell,
  type IpcMainInvokeEvent,
  type MessagePortMain,
  type WebContents,
} from 'electron';
import { statSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { CHANNELS, CLOSE_ANSWERS, CLOSE_REQUEST_CHANNEL, type AppInfo, type Channel, type CloseAnswer, type IpcContract } from '../shared/ipc.js';
import { backgroundOf, preferencesFile, ThemePreference, type ThemeTargets } from './theme.js';
import { SERVICE_PORT_CHANNEL } from '../shared/service.js';
import { PROJECT_PORT_CHANNEL } from '../shared/project.js';
import { CASE_PORT_CHANNEL } from '../shared/case.js';
import { RUN_PORT_CHANNEL } from '../shared/run.js';
import { RunPortBridge, type RunPortLike } from './run/bridge.js';
import { RunnerBudgetsEngine } from './run/extras.js';
import { engineSwitches, projectArgument } from './project/argv.js';
import { PROVENANCE_PORT_CHANNEL } from '../shared/provenance.js';
import { ProvenancePortBridge, type ProvenancePortLike } from './provenance/bridge.js';
import { CasePortBridge, type CasePortLike } from './case/bridge.js';
import { CaseEditor } from './case/editor.js';
import { ProjectServer } from './project/server.js';
import { ProjectPortBridge, type ProjectPortLike } from './project/bridge.js';
import { engineConfig, recentFile } from './project/config.js';
import { chooseEngine, defaultEngine } from './engine/locate.js';
import { ProjectHome } from './project/home.js';
import { RecentStore } from './project/recent.js';
import { PortBridge, type PortLike } from './service/bridge.js';
import { ServiceClient } from './service/client.js';
import { serviceConfig } from './service/config.js';
import { DataPorts } from './data/port.js';
import { DataRoots } from './data/roots.js';
import { StudioData } from './data/studio-data.js';
import { claudeConfig } from './claude/config.js';
import { ClaudePanel } from './claude/panel.js';
import { ClaudePorts } from './claude/port.js';
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

const isFile = (p: string): boolean => {
  try {
    return statSync(p).isFile();
  } catch {
    return false;
  }
};
/**
 * The engine, found without setup (ADR 0801 Addendum 21, src/main/engine/locate.ts): --no-engine, then --engine-dir,
 * then the variables, then <resources>/bin when packaged or the checkout's target/release with `electron .`. Never PATH.
 */
const switches = engineSwitches(process.argv, process.defaultApp === true, process.cwd());
for (const p of switches.problems) console.error(`rcfd Studio: ${p}`);
const engineChoice = chooseEngine(process.env, switches, defaultEngine(process.env, app.isPackaged, process.resourcesPath, app.getAppPath()), isFile);

/** The solver service (src/main/service/config.ts names the switches and variables that choose it). */
const serviceSetup = serviceConfig(process.env, path.join(app.getPath('userData'), 'service'), engineChoice, switches.service, isFile);
const service = new ServiceClient(serviceSetup);
/** Bulk field data (F-804): R2 reads local stores inside the data roots, R1 goes through the service (src/main/data/). */
const roots = DataRoots.fromEnv(process.env);
const data = new StudioData(service, roots);
const dataPorts = new DataPorts(data);
/** P1 and P2 measurement only (scripts/measure-service.mjs): the client, reachable from Playwright's main-process evaluate. */
if (process.env['RCFD_STUDIO_BENCH'] === '1') (globalThis as { rcfdStudioBench?: unknown }).rcfdStudioBench = { service };

const engine = engineConfig(process.env, isFile, engineChoice);

/** F-818: the theme setting, read before the first window opens. */
const theme = new ThemePreference(preferencesFile(process.env, app.getPath('userData')));
const themeTargets: ThemeTargets = {
  setSource: (source) => {
    nativeTheme.themeSource = source;
  },
  systemDark: () => nativeTheme.shouldUseDarkColors,
  setBackground: (colour) => {
    for (const w of BrowserWindow.getAllWindows()) if (!w.isDestroyed()) w.setBackgroundColor(colour);
  },
};

/** The project home (F-805): the engine's project server (src/main/project/config.ts). */
const projects = new ProjectHome({
  engine,
  recent: new RecentStore(recentFile(process.env, app.getPath('userData'))),
  pickFolder: async () => {
    const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    const options = { title: 'Open project folder', properties: ['openDirectory' as const] };
    const r = win ? await dialog.showOpenDialog(win, options) : await dialog.showOpenDialog(options);
    return r.canceled ? null : (r.filePaths[0] ?? null);
  },
  // F-835: the folder of a new project; the dialog can make a new folder (the engine then makes the project in it)
  pickNewFolder: async () => {
    const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    const options = {
      title: 'New project: choose or make its folder',
      buttonLabel: 'Make the project here',
      properties: ['openDirectory' as const, 'createDirectory' as const, 'promptToCreate' as const],
    };
    const r = win ? await dialog.showOpenDialog(win, options) : await dialog.showOpenDialog(options);
    return r.canceled ? null : (r.filePaths[0] ?? null);
  },
});

/** The case editor (F-812): sessions bound to case files of the open project, saved through the engine's edit_case. */
const caseEditor = new CaseEditor({
  project: () => {
    const p = projects.state.project;
    return p ? { folder: p.folder } : null;
  },
  engine: projects.state.engine,
  service,
  startServer: (exe, folder) => ProjectServer.start(exe, folder, undefined, { readOnly: false }),
});
projects.onState(() => caseEditor.projectChanged());
// F-820: an installed Studio's own service starts when a project is first opened (start() runs once)
projects.onState((s) => {
  if (s.project) service.start();
});
// F-810: the open project's folder is a data root (roots.ts), so its runs' stores are read by route local
projects.onState((s) => {
  if (s.project) {
    try {
      roots.allow(s.project.folder);
    } catch {
      // not an absolute folder: route local refuses it with the reason
    }
  }
});
/** The Claude Code panel (F-814): claude from RCFD_STUDIO_CLAUDE (src/main/claude/config.ts), for the open project. */
const claude = new ClaudePanel({
  config: claudeConfig(process.env, app.getPath('userData'), isFile, process.execPath),
  rcfdMcp: engine.configured ? engine.path : null,
});
const claudePorts = new ClaudePorts(claude);
projects.onState((s) => void claude.setProject(s.project?.folder ?? null));

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

type Handler<C extends Channel> = (args: unknown[], sender: WebContents) => IpcContract[C]['reply'] | Promise<IpcContract[C]['reply']>;

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
      setTimeout(() => quitWithoutAsking(), 50);
    }
  },
  'theme.get': (args) => {
    noArgs('theme.get', args);
    return theme.state(themeTargets);
  },
  'theme.set': (args) => {
    if (args.length !== 1) throw new Error('theme.set: takes one argument, the setting');
    return theme.set(args[0], themeTargets);
  },
  'app.closeAnswer': (args, sender) => {
    const [request, answer] = args;
    if (args.length !== 2 || !Number.isSafeInteger(request) || !(CLOSE_ANSWERS as readonly unknown[]).includes(answer)) {
      throw new Error('app.closeAnswer: takes a request number and one of asking, close, cancel');
    }
    closeAnswered(sender, request as number, answer as CloseAnswer);
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
    const handler: (args: unknown[], sender: WebContents) => unknown = handlers[channel];
    ipcMain.handle(channel, (event, ...args: unknown[]) => {
      if (!trusted(event)) throw new Error(`${channel}: refused, the sender is not an rcfd Studio page`);
      return handler(args, event.sender);
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

/** The case port of each web contents (F-812), like the project port. */
const casePorts = new Map<number, CasePortBridge>();

const adaptCasePort = (p: MessagePortMain): CasePortLike => ({
  postMessage: (m) => p.postMessage(m),
  onMessage: (listener) => {
    p.on('message', (e) => listener(e.data));
  },
  onClose: (listener) => {
    p.on('close', listener);
  },
  close: () => p.close(),
});

function offerCasePort(contents: WebContents): void {
  casePorts.get(contents.id)?.dispose();
  casePorts.delete(contents.id);
  if (contents.isDestroyed() || !isAppUrl(contents.getURL())) return;
  const { port1, port2 } = new MessageChannelMain();
  casePorts.set(contents.id, new CasePortBridge(adaptCasePort(port1), caseEditor));
  port1.start();
  contents.postMessage(CASE_PORT_CHANNEL, null, [port2]);
}

/** F-837: the runner's budgets of the open project through the engine (ADR 0733), with this process's last write (for undo). */
const runnerBudgets = new RunnerBudgetsEngine({
  read: () => projects.query('runner_config', {}),
  write: (args) => projects.setRunnerConfig(args),
});

/** The run port of each web contents (F-810), like the case port. */
const runPorts = new Map<number, RunPortBridge>();

const adaptRunPort = (p: MessagePortMain): RunPortLike => ({
  postMessage: (m) => p.postMessage(m),
  onMessage: (listener) => {
    p.on('message', (e) => listener(e.data));
  },
  onClose: (listener) => {
    p.on('close', listener);
  },
  close: () => p.close(),
});

function offerRunPort(contents: WebContents): void {
  runPorts.get(contents.id)?.dispose();
  runPorts.delete(contents.id);
  if (contents.isDestroyed() || !isAppUrl(contents.getURL())) return;
  const { port1, port2 } = new MessageChannelMain();
  runPorts.set(
    contents.id,
    new RunPortBridge(adaptRunPort(port1), {
      query: () => (projects.state.project ? (tool, args) => projects.query(tool, args) : null),
      now: () => new Date().toISOString(),
      // F-837: the runner's budgets of the open folder, its run's report in the system's program, Save PNG of the page
      extras: {
        folder: () => projects.state.project?.folder ?? null,
        budgets: runnerBudgets,
        openPath: (file) => shell.openPath(file),
        png: {
          capture: async (rect) => {
            const image = await contents.capturePage(rect);
            const size = image.getSize();
            return { png: new Uint8Array(image.toPNG()), width: size.width, height: size.height };
          },
          savePath: async (name) => {
            const win = BrowserWindow.fromWebContents(contents) ?? undefined;
            const options = {
              title: 'Save the field view as PNG',
              defaultPath: path.join(app.getPath('pictures'), name),
              filters: [{ name: 'PNG image', extensions: ['png'] }],
              properties: ['createDirectory' as const, 'showOverwriteConfirmation' as const],
            };
            const r = win ? await dialog.showSaveDialog(win, options) : await dialog.showSaveDialog(options);
            return r.canceled || !r.filePath ? null : r.filePath;
          },
        },
      },
    }),
  );
  port1.start();
  contents.postMessage(RUN_PORT_CHANNEL, null, [port2]);
}

/** The provenance port of each web contents (F-828), like the run port. */
const provenancePorts = new Map<number, ProvenancePortBridge>();

const adaptProvenancePort = (p: MessagePortMain): ProvenancePortLike => ({
  postMessage: (m) => p.postMessage(m),
  onMessage: (listener) => {
    p.on('message', (e) => listener(e.data));
  },
  onClose: (listener) => {
    p.on('close', listener);
  },
  close: () => p.close(),
});

function offerProvenancePort(contents: WebContents): void {
  provenancePorts.get(contents.id)?.dispose();
  provenancePorts.delete(contents.id);
  if (contents.isDestroyed() || !isAppUrl(contents.getURL())) return;
  const { port1, port2 } = new MessageChannelMain();
  provenancePorts.set(
    contents.id,
    new ProvenancePortBridge(adaptProvenancePort(port1), {
      project: () => {
        const v = projects.state.project;
        return v ? { query: (tool, args) => projects.query(tool, args), folder: v.folder, engine: { name: v.engine.name, version: v.engine.version } } : null;
      },
      copy: (text) => clipboard.writeText(text),
      now: () => new Date().toISOString(),
    }),
  );
  port1.start();
  contents.postMessage(PROVENANCE_PORT_CHANNEL, null, [port2]);
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

/**
 * Test runs only: RCFD_TEST_HEADLESS=1 (set by playwright.config.ts). Electron has no headless mode, a hidden window never
 * paints (screenshots wait for it) and an off-screen one gets other sizes, so the window is shown in place but fully
 * transparent (not on Linux, where a transparent window is not painted: there the tests run under xvfb), passing mouse
 * input through, without a taskbar button and without taking the focus; background throttling is off so its frames and
 * timers run as in a window in front. Unset, the window and its settings are as before.
 */
const HEADLESS = process.env.RCFD_TEST_HEADLESS === '1';

/** The window background of the theme now (nativeTheme says whether the system is dark, for System). */
function themeBackground(): string {
  // the same rule as ThemePreference.apply: resolve System by the system's colours
  return backgroundOf(theme.setting, nativeTheme.shouldUseDarkColors);
}

function openWindow(): void {
  const win = new BrowserWindow({
    width: 1280,
    height: 800,
    show: false,
    // F-818: the theme's ground, so the window is never white before a dark page paints
    backgroundColor: themeBackground(),
    // opacity 0 only off Linux: Chromium on X11 takes a fully transparent window for a hidden one and stops painting it
    // (screenshots time out, frames drop to one a second); Linux test runs use a virtual display (xvfb) instead
    ...(HEADLESS ? { skipTaskbar: true, ...(process.platform === 'linux' ? {} : { opacity: 0 }), focusable: true } : {}),
    webPreferences: {
      ...(HEADLESS ? { backgroundThrottling: false } : {}),
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
    offerCasePort(contents);
    offerRunPort(contents);
    offerProvenancePort(contents);
    dataPorts.offer(contents);
    claudePorts.offer(contents);
  });
  contents.once('destroyed', () => {
    dataPorts.drop(id);
    claudePorts.drop(id);
    servicePorts.get(id)?.dispose();
    servicePorts.delete(id);
    projectPorts.get(id)?.dispose();
    projectPorts.delete(id);
    casePorts.get(id)?.dispose();
    casePorts.delete(id);
    runPorts.get(id)?.dispose();
    runPorts.delete(id);
    provenancePorts.get(id)?.dispose();
    provenancePorts.delete(id);
  });
  // F-837: a close of the window (its close button, Alt+F4, the system's Close) asks the page first, which asks the user
  // when a case has unsaved changes; the quit below asks the same way before anything is stopped
  win.on('close', (e) => {
    if (closeAllowed.has(win)) return;
    e.preventDefault();
    void askToClose(win).then((ok) => {
      if (!ok || win.isDestroyed()) return;
      closeAllowed.add(win);
      win.close();
    });
  });
  win.once('ready-to-show', () => {
    if (!HEADLESS) return win.show();
    win.setIgnoreMouseEvents(true);
    win.showInactive();
    // a window that never has the system focus fires no focus events; the page is told it has the focus, as headless
    // Chromium does
    try {
      contents.debugger.attach('1.3');
      void contents.debugger.sendCommand('Emulation.setFocusEmulationEnabled', { enabled: true }).catch(() => undefined);
    } catch {
      // another debugger client is attached already: the page runs without focus emulation
    }
  });
  void win.loadURL(APP_ENTRY);
}

void app.whenReady().then(() => {
  hardenSession();
  registerIpc();
  // F-818: the native parts in the theme before the window opens; System follows the system's changes
  theme.apply(themeTargets);
  nativeTheme.on('updated', () => {
    if (theme.setting === 'system') themeTargets.setBackground(themeBackground());
  });
  // F-820: the engine's own rcfd starts with the first project; at launch only a service left running is found again (G7)
  if (serviceSetup.mode === 'spawn' && serviceSetup.start === 'project') void service.findRunning();
  else service.start();
  // F-837: `rcfd-studio <PROJECT_DIR>` opens that folder (after the recent list is read, so it goes to its head)
  const asked = projectArgument(process.argv, process.defaultApp === true, process.cwd());
  void projects.init().then(() => (asked ? projects.open(asked) : undefined));
  openWindow();
});
app.on('window-all-closed', () => app.quit());

// ---- F-837: save or discard before closing (the page asks: src/renderer/case/closeGuard.tsx)
//
// A request goes to the page on CLOSE_REQUEST_CHANNEL; the page answers `asking` at once, then `close` or `cancel` once
// the user has chosen (with no unsaved change, `close` straight after). A page that does not say `asking` within
// CLOSE_ACK_MS (hung, crashed, not loaded) cannot hold the window: it closes. A page that crashes or goes away while
// asking lets the window close; a page that loads again ends the question as a cancel, so a second close asks again.
//
// Test mode (RCFD_TEST_HEADLESS=1) quits without asking: Playwright's close() is app.quit(), and a test that leaves a
// change unsaved must not hang its teardown. A close of the window itself (BrowserWindow.close(), what the close button
// and Alt+F4 do) still asks there, so the tests can answer it. The P10 measurement's quit does not ask either.
const CLOSE_ACK_MS = 3000;
/** Windows the page let close (or that close as the app quits): their next close event goes through. */
const closeAllowed = new WeakSet<BrowserWindow>();
/** A quit that does not ask the windows (test mode, the P10 measurement); a window's own close still asks. */
let quitWithoutAskingSet = HEADLESS;
let closeSeq = 0;
/** Requests waiting for the page's answer, by number: the page that was asked, and how to settle the request. */
const closeRequests = new Map<number, { contents: WebContents; ack: () => void; done: (ok: boolean) => void }>();
/** The request in flight per window, shared by its close event and the quit. */
const closeAsks = new Map<BrowserWindow, Promise<boolean>>();

function askToClose(win: BrowserWindow): Promise<boolean> {
  const asked = closeAsks.get(win);
  if (asked) return asked;
  const contents = win.webContents;
  if (win.isDestroyed() || contents.isDestroyed() || contents.isCrashed()) return Promise.resolve(true);
  const n = ++closeSeq;
  const p = new Promise<boolean>((resolve) => {
    let settled = false;
    const gone = (): void => done(true);
    const reloaded = (): void => done(false);
    const timer = setTimeout(() => done(true), CLOSE_ACK_MS);
    function done(ok: boolean): void {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      closeRequests.delete(n);
      closeAsks.delete(win);
      if (!contents.isDestroyed()) {
        contents.off('render-process-gone', gone);
        contents.off('did-navigate', reloaded);
        contents.off('destroyed', gone);
      }
      resolve(ok);
    }
    contents.once('render-process-gone', gone);
    contents.once('did-navigate', reloaded);
    contents.once('destroyed', gone);
    closeRequests.set(n, { contents, ack: () => clearTimeout(timer), done });
    contents.send(CLOSE_REQUEST_CHANNEL, n);
  });
  closeAsks.set(win, p);
  return p;
}

function closeAnswered(sender: WebContents, n: number, answer: CloseAnswer): void {
  const r = closeRequests.get(n);
  if (!r || r.contents !== sender) return;
  if (answer === 'asking') r.ack();
  else r.done(answer === 'close');
}

function quitWithoutAsking(): void {
  quitWithoutAskingSet = true;
  app.quit();
}

// Quitting: every window is asked first (F-837), then the client stops; a service the Studio started is stopped too
// unless it holds sessions (G7, client.ts). A window that cancels cancels the quit, and nothing is stopped.
let serviceStopped = false;
let quitAsking = false;
app.on('before-quit', (e) => {
  if (serviceStopped) return;
  e.preventDefault();
  if (quitAsking) return;
  quitAsking = true;
  const windows = BrowserWindow.getAllWindows().filter((w) => !w.isDestroyed());
  const asks = quitWithoutAskingSet ? [] : windows.filter((w) => !closeAllowed.has(w)).map((w) => askToClose(w));
  void Promise.all(asks).then((answers) => {
    quitAsking = false;
    if (!answers.every(Boolean)) return;
    for (const w of windows) closeAllowed.add(w);
    serviceStopped = true;
    const atMost = new Promise((done) => setTimeout(done, 3000));
    // the project server of an open project ends with the Studio: it only reads, and holds no run (G7)
    void Promise.race([Promise.all([service.shutdown(), projects.shutdown(), caseEditor.shutdown(), claude.shutdown()]), atMost])
      .catch(() => undefined)
      .finally(() => app.quit());
  });
});
