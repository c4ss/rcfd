// Preload script of the Studio window: the IPC allow-list of docs/studio-guardrails.md G10, exposed to the page through
// contextBridge. The page never gets ipcRenderer, a generic invoke, a MessagePort or any Node object.
//
// - `window.studio`: the shell's channels of src/shared/ipc.ts (one function per channel, F-801).
// - `window.studioService`: the solver service (F-803, src/shared/service.ts). The main process hands this script a
//   MessagePort on SERVICE_PORT_CHANNEL after each load of the page; the port stays here. Each function below turns its
//   arguments into one request on that port. The checks are the main process's (src/main/service/bridge.ts): this side
//   forwards, so a page cannot get round them by calling a function differently.
// - `window.studioProject`: the project folder (F-805, src/shared/project.ts), over a second port on
//   PROJECT_PORT_CHANNEL in the same way; the checks are src/main/project/bridge.ts's.
//
// CommonJS (.cts, emitted as .cjs) because a sandboxed preload cannot be an ES module, and a single file because a
// sandboxed preload can only require 'electron' and a few built-ins: the shared modules are imported for their types
// only, and the two constants below are typed by theirs, so a change there fails to compile here.
import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron';
import type { AppInfo, Channel, StudioBridge } from '../shared/ipc.js';
import type {
  DEFAULT_CALL_TIMEOUT_MS as DefaultTimeout,
  PortMessage,
  PortRequest,
  SERVICE_PORT_CHANNEL as PortChannel,
  ServiceBridge,
  ServiceEventBatch,
  ServiceInfo,
  ServiceReply,
  ServiceState,
} from '../shared/service.js';
import type {
  PROJECT_PORT_CHANNEL as ProjectChannel,
  ProjectBridge,
  ProjectPortMessage,
  ProjectReply,
  ProjectRequest,
  ProjectState,
} from '../shared/project.js';

const invoke = <T,>(channel: Channel): Promise<T> => ipcRenderer.invoke(channel) as Promise<T>;

const bridge: StudioBridge = {
  app: {
    info: () => invoke<AppInfo>('app.info'),
    ready: () => invoke<void>('app.ready'),
  },
};

contextBridge.exposeInMainWorld('studio', bridge);

// ---- the service port

const SERVICE_PORT_CHANNEL: typeof PortChannel = 'studio.service-port';
const DEFAULT_CALL_TIMEOUT_MS: typeof DefaultTimeout = 60_000;
/** Extra time this side waits beyond a call's own limit before it gives up (main answers a timed-out call itself). */
const GRACE_MS = 5_000;

type Port = IpcRendererEvent['ports'][number];
let port: Port | null = null;
let nextId = 1;
let nextSubscription = 1;
let nextStateListener = 1;
const queued: PortRequest[] = [];
const pending = new Map<number, { resolve: (r: ServiceReply) => void; timer: ReturnType<typeof setTimeout> }>();
const eventListeners = new Map<number, (b: ServiceEventBatch) => void>();
const stateListeners = new Map<number, (s: ServiceState) => void>();

const settle = (id: number, reply: ServiceReply): void => {
  const p = pending.get(id);
  if (!p) return;
  pending.delete(id);
  clearTimeout(p.timer);
  p.resolve(reply);
};

const post = (req: PortRequest): void => {
  try {
    (port as Port).postMessage(req);
  } catch (e) {
    // a value the structured clone refuses (a function, a DOM node): never reaches main
    settle(req.id, { ok: false, error: { code: 'bad_request', message: `the request cannot be sent: ${String(e)}` } });
  }
};

const send = <T,>(req: PortRequest): Promise<ServiceReply<T>> =>
  new Promise((resolve) => {
    // a call: its own limit; the other operations are answered by main at once
    const own = req.op === 'call' ? (typeof req.timeoutMs === 'number' ? req.timeoutMs : DEFAULT_CALL_TIMEOUT_MS) : 0;
    const limit = own + GRACE_MS;
    const timer = setTimeout(() => {
      settle(req.id, {
        ok: false,
        error: port
          ? { code: 'timeout', message: `no answer from the Studio's main process within ${limit} ms` }
          : { code: 'no_port', message: 'the Studio has not connected this window to the service' },
      });
    }, limit);
    pending.set(req.id, { resolve: resolve as (r: ServiceReply) => void, timer });
    if (port) post(req);
    else queued.push(req);
  });

const onPortMessage = (m: PortMessage): void => {
  switch (m?.type) {
    case 'reply':
      settle(m.id, m.reply);
      return;
    case 'state':
      for (const l of stateListeners.values()) {
        try {
          l(m.state);
        } catch {
          // the page's listener failed; others still run
        }
      }
      return;
    case 'events': {
      const l = eventListeners.get(m.subscription);
      if (!l) return;
      try {
        l({ subscription: m.subscription, instance: m.instance, events: m.events, missed: m.missed });
      } catch {
        // the page's listener failed
      }
      return;
    }
  }
};

ipcRenderer.on(SERVICE_PORT_CHANNEL, (event) => {
  const p = event.ports[0];
  if (!p) return;
  port?.close();
  port = p;
  // the DOM MessagePort of the renderer (typed here by the Node environment of this file)
  p.onmessage = (e: { data: unknown }) => onPortMessage(e.data as PortMessage);
  for (const req of queued.splice(0)) post(req);
});

const service: ServiceBridge = {
  info: () => send<ServiceInfo>({ op: 'call', id: nextId++, method: 'service.info' }),
  call: (method, params, options) =>
    send({
      op: 'call',
      id: nextId++,
      method,
      ...(params !== undefined ? { params } : {}),
      ...(options?.timeoutMs !== undefined ? { timeoutMs: options.timeoutMs } : {}),
    }),
  state: async () => {
    const r = await send<ServiceState>({ op: 'state', id: nextId++ });
    if (r.ok) return r.result;
    throw new Error(r.error.message);
  },
  onState: (listener) => {
    const id = nextStateListener++;
    if (typeof listener === 'function') stateListeners.set(id, listener);
    return id;
  },
  offState: (id) => {
    stateListeners.delete(id);
  },
  subscribe: async (listener, options) => {
    if (typeof listener !== 'function') return { ok: false, error: { code: 'bad_request', message: 'subscribe: the listener must be a function' } };
    const subscription = nextSubscription++;
    eventListeners.set(subscription, listener);
    const session = options?.session;
    const r = await send<number>({ op: 'subscribe', id: nextId++, subscription, ...(session !== undefined ? { session } : {}) });
    if (!r.ok) eventListeners.delete(subscription);
    return r;
  },
  unsubscribe: (subscription) => {
    eventListeners.delete(subscription);
    return send<null>({ op: 'unsubscribe', id: nextId++, subscription });
  },
};

contextBridge.exposeInMainWorld('studioService', service);

// ---- the project port (F-805): the same pattern as the service port, its own ids and listeners

const PROJECT_PORT_CHANNEL: typeof ProjectChannel = 'studio.project-port';
/** The folder dialog waits for the user, so `pick` has no time limit of its own; the others are answered at once. */
const PROJECT_TIMEOUT_MS = 120_000;

let projectPort: Port | null = null;
let nextProjectId = 1;
let nextProjectListener = 1;
const projectQueued: ProjectRequest[] = [];
const projectPending = new Map<number, { resolve: (r: ProjectReply) => void; timer: ReturnType<typeof setTimeout> | null }>();
const projectListeners = new Map<number, (s: ProjectState) => void>();

const projectSettle = (id: number, reply: ProjectReply): void => {
  const p = projectPending.get(id);
  if (!p) return;
  projectPending.delete(id);
  if (p.timer) clearTimeout(p.timer);
  p.resolve(reply);
};

const projectPost = (req: ProjectRequest): void => {
  try {
    (projectPort as Port).postMessage(req);
  } catch (e) {
    projectSettle(req.id, { ok: false, error: { code: 'bad_request', message: `the request cannot be sent: ${String(e)}` } });
  }
};

const projectSend = <T,>(req: ProjectRequest): Promise<ProjectReply<T>> =>
  new Promise((resolve) => {
    const timer =
      req.op === 'pick'
        ? null
        : setTimeout(() => {
            projectSettle(req.id, {
              ok: false,
              error: projectPort
                ? { code: 'timeout', message: `no answer from the Studio's main process within ${PROJECT_TIMEOUT_MS} ms` }
                : { code: 'no_port', message: 'the Studio has not connected this window to the project home' },
            });
          }, PROJECT_TIMEOUT_MS);
    projectPending.set(req.id, { resolve: resolve as (r: ProjectReply) => void, timer });
    if (projectPort) projectPost(req);
    else projectQueued.push(req);
  });

const onProjectMessage = (m: ProjectPortMessage): void => {
  switch (m?.type) {
    case 'reply':
      projectSettle(m.id, m.reply);
      return;
    case 'state':
      for (const l of projectListeners.values()) {
        try {
          l(m.state);
        } catch {
          // the page's listener failed; others still run
        }
      }
      return;
  }
};

ipcRenderer.on(PROJECT_PORT_CHANNEL, (event) => {
  const p = event.ports[0];
  if (!p) return;
  projectPort?.close();
  projectPort = p;
  p.onmessage = (e: { data: unknown }) => onProjectMessage(e.data as ProjectPortMessage);
  for (const req of projectQueued.splice(0)) projectPost(req);
});

const project: ProjectBridge = {
  state: async () => {
    const r = await projectSend<ProjectState>({ op: 'state', id: nextProjectId++ });
    if (r.ok) return r.result;
    throw new Error(r.error.message);
  },
  onState: (listener) => {
    const id = nextProjectListener++;
    if (typeof listener === 'function') projectListeners.set(id, listener);
    return id;
  },
  offState: (id) => {
    projectListeners.delete(id);
  },
  pick: () => projectSend({ op: 'pick', id: nextProjectId++ }),
  open: (path) => projectSend({ op: 'open', id: nextProjectId++, path }),
  forget: (path) => projectSend({ op: 'forget', id: nextProjectId++, path }),
  close: () => projectSend({ op: 'close', id: nextProjectId++ }),
  refresh: () => projectSend({ op: 'refresh', id: nextProjectId++ }),
  dismiss: () => projectSend({ op: 'dismiss', id: nextProjectId++ }),
};

contextBridge.exposeInMainWorld('studioProject', project);
