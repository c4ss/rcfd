// Preload script of the Studio window: the IPC allow-list of docs/studio-guardrails.md G10, exposed to the page through
// contextBridge. The page never gets ipcRenderer, a generic invoke, a MessagePort or any Node object.
//
// - `window.studio`: the shell's channels of src/shared/ipc.ts (one function per channel, F-801).
// - `window.studioData`: bulk field data (F-804, src/shared/data.ts), on a second port; see the end of this file.
// - `window.studioService`: the solver service (F-803, src/shared/service.ts). The main process hands this script a
//   MessagePort on SERVICE_PORT_CHANNEL after each load of the page; the port stays here. Each function below turns its
//   arguments into one request on that port. The checks are the main process's (src/main/service/bridge.ts): this side
//   forwards, so a page cannot get round them by calling a function differently.
// - `window.studioProject`: the project folder (F-805, src/shared/project.ts), over a second port on
//   PROJECT_PORT_CHANNEL in the same way; the checks are src/main/project/bridge.ts's.
// - `window.studioCase`: the case editor's case files (F-812, src/shared/case.ts), on a port of its own on
//   CASE_PORT_CHANNEL; the checks are src/main/case/bridge.ts's. See the end of this file.
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
import type {
  DATA_PORT_CHANNEL as DataPortChannel,
  DataBridge,
  DataOptions,
  DataPortMessage,
  DataReply,
  DataRequest,
  DataWireResult,
  DEFAULT_DATA_TIMEOUT_MS as DefaultDataTimeout,
} from '../shared/data.js';
import type { CASE_PORT_CHANNEL as CaseChannel, CaseBridge, CasePortMessage, CaseReply, CaseRequest } from '../shared/case.js';

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

// ---- the data port (F-804, src/shared/data.ts)
//
// `window.studioData`: bulk field data by route R2 (`local`) or R1 (`service`). The main process hands this script a
// second MessagePort on DATA_PORT_CHANNEL after each load; it stays here like the service port. Values arrive as
// ArrayBuffers (one copy across the process boundary) and are handed to the page as typed arrays (contextBridge copies
// them once more inside this process). As with the service, the checks are the main process's (src/main/data/bridge.ts).

const DATA_PORT_CHANNEL: typeof DataPortChannel = 'studio.data-port';
const DEFAULT_DATA_TIMEOUT_MS: typeof DefaultDataTimeout = 60_000;

let dataPort: Port | null = null;
let nextDataId = 1;
const dataQueued: DataRequest[] = [];
const dataPending = new Map<number, { resolve: (r: DataReply<DataWireResult>) => void; timer: ReturnType<typeof setTimeout> }>();

const settleData = (id: number, reply: DataReply<DataWireResult>): void => {
  const p = dataPending.get(id);
  if (!p) return;
  dataPending.delete(id);
  clearTimeout(p.timer);
  p.resolve(reply);
};

const postData = (req: DataRequest): void => {
  try {
    (dataPort as Port).postMessage(req);
  } catch (e) {
    settleData(req.id, { ok: false, error: { code: 'bad_request', message: `the request cannot be sent: ${String(e)}` } });
  }
};

const sendData = (req: DataRequest): Promise<DataReply<DataWireResult>> =>
  new Promise((resolve) => {
    const own = req.op === 'cancel' ? 0 : typeof req.timeoutMs === 'number' ? req.timeoutMs : DEFAULT_DATA_TIMEOUT_MS;
    const limit = own + GRACE_MS;
    const timer = setTimeout(() => {
      settleData(req.id, {
        ok: false,
        error: dataPort
          ? { code: 'timeout', message: `no answer from the Studio's main process within ${limit} ms` }
          : { code: 'no_port', message: 'the Studio has not connected this window to its data' },
      });
    }, limit);
    dataPending.set(req.id, { resolve, timer });
    if (dataPort) postData(req);
    else dataQueued.push(req);
  });

ipcRenderer.on(DATA_PORT_CHANNEL, (event) => {
  const p = event.ports[0];
  if (!p) return;
  dataPort?.close();
  dataPort = p;
  p.onmessage = (e: { data: unknown }) => {
    const m = e.data as DataPortMessage;
    if (m?.type === 'reply') settleData(m.id, m.reply);
  };
  for (const req of dataQueued.splice(0)) postData(req);
});

/** The parts of a request every operation shares; arguments are passed on as given (main checks them). */
const dataCommon = (options: DataOptions | undefined): { tag?: string | number; timeoutMs?: number } => ({
  ...(options?.tag !== undefined ? { tag: options.tag } : {}),
  ...(options?.timeoutMs !== undefined ? { timeoutMs: options.timeoutMs } : {}),
});

const unexpected = (kind: string): DataReply<never> => ({ ok: false, error: { code: 'internal', message: `unexpected reply '${kind}'` } });

const data: DataBridge = {
  open: async (store, options) => {
    const r = await sendData({ op: 'open', id: nextDataId++, store, ...dataCommon(options) });
    if (!r.ok) return r;
    return r.result.kind === 'open' ? { ok: true, result: r.result.info } : unexpected(r.result.kind);
  },
  field: async (store, snapshot, field, options) => {
    const r = await sendData({ op: 'field', id: nextDataId++, store, snapshot, field, ...dataCommon(options) });
    if (!r.ok) return r;
    const x = r.result;
    if (x.kind !== 'field') return unexpected(x.kind);
    return { ok: true, result: { snapshot: x.snapshot, field: x.field, values: new Float32Array(x.values) } };
  },
  surface: async (store, options) => {
    const r = await sendData({ op: 'surface', id: nextDataId++, store, ...dataCommon(options) });
    if (!r.ok) return r;
    const x = r.result;
    if (x.kind !== 'surface') return unexpected(x.kind);
    return { ok: true, result: { points: new Float32Array(x.points), triangles: new Uint32Array(x.triangles), cells: new Uint32Array(x.cells) } };
  },
  cancel: async (tag) => {
    const r = await sendData({ op: 'cancel', id: nextDataId++, tag });
    if (!r.ok) return r;
    return r.result.kind === 'cancel' ? { ok: true, result: r.result.cancelled } : unexpected(r.result.kind);
  },
};

contextBridge.exposeInMainWorld('studioData', data);

// ---- the case port (F-812, src/shared/case.ts)
//
// `window.studioCase`: requests and replies only (no pushed state). A save or a check asks the engine to validate the
// case and may build its mesh, so a reply may take a while; the main process gives each tool call its own limit.

const CASE_PORT_CHANNEL: typeof CaseChannel = 'studio.case-port';
const CASE_TIMEOUT_MS = 300_000;

let casePort: Port | null = null;
let nextCaseId = 1;
const caseQueued: CaseRequest[] = [];
const casePending = new Map<number, { resolve: (r: CaseReply) => void; timer: ReturnType<typeof setTimeout> }>();

const caseSettle = (id: number, reply: CaseReply): void => {
  const p = casePending.get(id);
  if (!p) return;
  casePending.delete(id);
  clearTimeout(p.timer);
  p.resolve(reply);
};

const casePost = (req: CaseRequest): void => {
  try {
    (casePort as Port).postMessage(req);
  } catch (e) {
    caseSettle(req.id, { ok: false, error: { code: 'bad_request', message: `the request cannot be sent: ${String(e)}` } });
  }
};

const caseSend = <T,>(req: CaseRequest): Promise<CaseReply<T>> =>
  new Promise((resolve) => {
    const timer = setTimeout(() => {
      caseSettle(req.id, {
        ok: false,
        error: casePort
          ? { code: 'timeout', message: `no answer from the Studio's main process within ${CASE_TIMEOUT_MS} ms` }
          : { code: 'no_port', message: 'the Studio has not connected this window to the case editor' },
      });
    }, CASE_TIMEOUT_MS);
    casePending.set(req.id, { resolve: resolve as (r: CaseReply) => void, timer });
    if (casePort) casePost(req);
    else caseQueued.push(req);
  });

ipcRenderer.on(CASE_PORT_CHANNEL, (event) => {
  const p = event.ports[0];
  if (!p) return;
  casePort?.close();
  casePort = p;
  p.onmessage = (e: { data: unknown }) => {
    const m = e.data as CasePortMessage;
    if (m?.type === 'reply') caseSettle(m.id, m.reply);
  };
  for (const req of caseQueued.splice(0)) casePost(req);
});

const caseFiles: CaseBridge = {
  state: () => caseSend({ op: 'state', id: nextCaseId++ }),
  sources: () => caseSend({ op: 'sources', id: nextCaseId++ }),
  prepare: (path) => caseSend({ op: 'prepare', id: nextCaseId++, path }),
  attach: (session, path, hash) => caseSend({ op: 'attach', id: nextCaseId++, session, path, hash }),
  status: (session) => caseSend({ op: 'status', id: nextCaseId++, session }),
  conflict: (session) => caseSend({ op: 'conflict', id: nextCaseId++, session }),
  save: (session, onto) => caseSend({ op: 'save', id: nextCaseId++, session, onto }),
  create: (name, source) => caseSend({ op: 'create', id: nextCaseId++, name, source }),
  duplicate: (path, name) => caseSend({ op: 'duplicate', id: nextCaseId++, path, name }),
  check: (path) => caseSend({ op: 'check', id: nextCaseId++, path }),
  forget: (session) => caseSend({ op: 'forget', id: nextCaseId++, session }),
};

contextBridge.exposeInMainWorld('studioCase', caseFiles);
