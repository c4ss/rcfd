// Preload script of the Studio window: the IPC allow-list of docs/studio-guardrails.md G10, exposed to the page through
// contextBridge. The page never gets ipcRenderer, a generic invoke, a MessagePort or any Node object.
//
// - `window.studio`: the shell's channels of src/shared/ipc.ts (one function per channel, F-801; the theme, F-818).
// - `window.studioData`: bulk field data (F-804, src/shared/data.ts), on a second port; see the end of this file.
// - `window.studioService`: the solver service (F-803, src/shared/service.ts). The main process hands this script a
//   MessagePort on SERVICE_PORT_CHANNEL after each load of the page; the port stays here. Each function below turns its
//   arguments into one request on that port. The checks are the main process's (src/main/service/bridge.ts): this side
//   forwards, so a page cannot get round them by calling a function differently.
// - `window.studioProject`: the project folder (F-805, src/shared/project.ts), over a second port on
//   PROJECT_PORT_CHANNEL in the same way; the checks are src/main/project/bridge.ts's.
// - `window.studioCase`: the case editor's case files (F-812, src/shared/case.ts), on a port of its own on
//   CASE_PORT_CHANNEL; the checks are src/main/case/bridge.ts's. See the end of this file.
// - `window.studioRunData`: a runner run's residual and monitor histories (F-810, src/shared/run.ts), on a port of its own on
//   RUN_PORT_CHANNEL; the checks are src/main/run/bridge.ts's. See the end of this file.
// - `window.studioClaude`: the Claude Code panel (F-814, src/shared/claude.ts), over a port on CLAUDE_PORT_CHANNEL in
//   the same way; the checks are src/main/claude/bridge.ts's.
// - `window.studioProvenance`: a run's provenance and the clipboard of Copy (F-828, src/shared/provenance.ts), on a port of
//   its own on PROVENANCE_PORT_CHANNEL; the checks are src/main/provenance/bridge.ts's. See the end of this file.
//
// CommonJS (.cts, emitted as .cjs) because a sandboxed preload cannot be an ES module, and a single file because a
// sandboxed preload can only require 'electron' and a few built-ins: the shared modules are imported for their types
// only, and the two constants below are typed by theirs, so a change there fails to compile here.
import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron';
import type { AppInfo, Channel, CloseAnswer, CLOSE_REQUEST_CHANNEL as CloseChannel, StudioBridge } from '../shared/ipc.js';
import type { ThemeSetting, ThemeState } from '../shared/theme.js';
import type {
  CLAUDE_PORT_CHANNEL as ClaudeChannel,
  ClaudeBridge,
  ClaudePortMessage,
  ClaudeReply,
  ClaudeRequest,
  ClaudeState,
} from '../shared/claude.js';
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
  RunActionResult,
  RunsBridge,
} from '../shared/project.js';
import type {
  DATA_PORT_CHANNEL as DataPortChannel,
  CompareBridge,
  DataBridge,
  DataOptions,
  DataPortMessage,
  DataReply,
  DataRequest,
  DataWireResult,
  DEFAULT_DATA_TIMEOUT_MS as DefaultDataTimeout,
  SurfaceValuesBridge,
} from '../shared/data.js';
import type { CASE_PORT_CHANNEL as CaseChannel, CaseBridge, CasePortMessage, CaseReply, CaseRequest } from '../shared/case.js';
import type { RUN_PORT_CHANNEL as RunChannel, RunBridge, RunPortMessage, RunReply, RunRequest } from '../shared/run.js';
import type {
  PROVENANCE_PORT_CHANNEL as ProvenanceChannel,
  ProvenanceBridge,
  ProvenancePortMessage,
  ProvenanceReply,
  ProvenanceRequest,
} from '../shared/provenance.js';
import type { CancelOptions } from '../shared/work.js';

/**
 * F-832 (G21): hands the page the function that cancels one request, through the `onCancel` it passed (an AbortSignal
 * cannot cross contextBridge; functions cross as proxies). Anything else in `options` is ignored.
 */
const offerCancel = (options: CancelOptions | undefined, cancel: () => void): void => {
  const hook = options && typeof options === 'object' ? options.onCancel : undefined;
  if (typeof hook !== 'function') return;
  try {
    hook(() => {
      cancel();
    });
  } catch {
    // the page's hook failed; the request goes on as if it had passed none
  }
};

const invoke = <T,>(channel: Channel, ...args: unknown[]): Promise<T> => ipcRenderer.invoke(channel, ...args) as Promise<T>;

// F-837: the window's close requests (src/main/index.ts) go to the one listener the page set; with none, the request
// is not answered and the main process closes after its wait for an answer
const CLOSE_REQUEST_CHANNEL: typeof CloseChannel = 'studio.close-request';
let closeListener: ((request: number) => void) | null = null;
ipcRenderer.on(CLOSE_REQUEST_CHANNEL, (_event, request: unknown) => {
  if (typeof request !== 'number' || closeListener === null) return;
  try {
    closeListener(request);
  } catch {
    // the page's listener failed: the main process's wait for an answer decides
  }
});

const bridge: StudioBridge = {
  app: {
    info: () => invoke<AppInfo>('app.info'),
    ready: () => invoke<void>('app.ready'),
    onCloseRequest: (listener) => {
      closeListener = typeof listener === 'function' ? listener : null;
    },
    closeAnswer: (request: number, answer: CloseAnswer) => invoke<void>('app.closeAnswer', request, answer),
  },
  // F-818: the theme setting; the main process checks the value (src/main/theme.ts)
  theme: {
    get: () => invoke<ThemeState>('theme.get'),
    set: (setting: ThemeSetting) => invoke<ThemeState>('theme.set', setting),
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
/** The folder dialog waits for the user, so `pick` and `create` (F-835) have no time limit of their own; the others are answered at once. */
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
      req.op === 'pick' || req.op === 'create'
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
  pick: (options) => {
    offerCancel(options, () => void projectSend({ op: 'cancel', id: nextProjectId++ }));
    return projectSend({ op: 'pick', id: nextProjectId++ });
  },
  open: (path, options) => {
    offerCancel(options, () => void projectSend({ op: 'cancel', id: nextProjectId++ }));
    return projectSend({ op: 'open', id: nextProjectId++, path });
  },
  forget: (path) => projectSend({ op: 'forget', id: nextProjectId++, path }),
  close: () => projectSend({ op: 'close', id: nextProjectId++ }),
  refresh: () => projectSend({ op: 'refresh', id: nextProjectId++ }),
  dismiss: () => projectSend({ op: 'dismiss', id: nextProjectId++ }),
  // F-835: the engine makes the project (rcfd project init), then it opens; main checks `where` again
  create: (where, options) => {
    offerCancel(options, () => void projectSend({ op: 'cancel', id: nextProjectId++ }));
    return projectSend({ op: 'create', id: nextProjectId++, where });
  },
};

contextBridge.exposeInMainWorld('studioProject', project);

// `window.studioRuns` (F-807): run-control actions over the same project port (op `action`), checked again in main
// (src/main/project/bridge.ts, control.ts). A port that cannot answer gives a failure in words, never a rejection.
const runs: RunsBridge = {
  dispatch: async (action, params = {}) => {
    const r = await projectSend<RunActionResult>({ op: 'action', id: nextProjectId++, action, params });
    if (r.ok) return r.result;
    return {
      ok: false,
      request: null,
      failure: {
        code: r.error.code === 'bad_request' ? 'bad_request' : 'disconnected',
        problem: `The request did not reach the Studio's main process: ${r.error.message}`,
        recovery: r.error.code === 'bad_request' ? 'Nothing was sent; the request had the wrong shape.' : 'Reload the window (Ctrl+R); runs are not affected.',
        engine: null,
      },
    };
  },
};

contextBridge.exposeInMainWorld('studioRuns', runs);

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

// F-815: the difference of two stores' field, f64 (src/main/data/worker.ts), over the same data port
const compare: CompareBridge = {
  difference: async (store, snapshot, other, otherSnapshot, field, options) => {
    const r = await sendData({ op: 'difference', id: nextDataId++, store, snapshot, other, otherSnapshot, field, ...dataCommon(options) });
    if (!r.ok) return r;
    const x = r.result;
    if (x.kind !== 'difference') return unexpected(x.kind);
    return { ok: true, result: { snapshot: x.snapshot, otherSnapshot: x.otherSnapshot, field: x.field, values: new Float64Array(x.values) } };
  },
};

contextBridge.exposeInMainWorld('studioCompare', compare);

// F-819b: a field's values on the store's surface only, one per triangle (src/main/data/worker.ts), over the same data port
const surfaceValues: SurfaceValuesBridge = {
  field: async (store, snapshot, field, options) => {
    const r = await sendData({ op: 'surfaceField', id: nextDataId++, store, snapshot, field, ...dataCommon(options) });
    if (!r.ok) return r;
    const x = r.result;
    if (x.kind !== 'surfaceField') return unexpected(x.kind);
    return { ok: true, result: { snapshot: x.snapshot, field: x.field, meshHash: x.meshHash, values: new Float32Array(x.values) } };
  },
};

contextBridge.exposeInMainWorld('studioSurfaceValues', surfaceValues);

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
  save: (session, onto, options) => {
    const id = nextCaseId++;
    offerCancel(options, () => void caseSend({ op: 'cancel', id: nextCaseId++, target: id }));
    return caseSend({ op: 'save', id, session, onto });
  },
  create: (name, source) => caseSend({ op: 'create', id: nextCaseId++, name, source }),
  duplicate: (path, name) => caseSend({ op: 'duplicate', id: nextCaseId++, path, name }),
  check: (path, options) => {
    const id = nextCaseId++;
    offerCancel(options, () => void caseSend({ op: 'cancel', id: nextCaseId++, target: id }));
    return caseSend({ op: 'check', id, path });
  },
  forget: (session) => caseSend({ op: 'forget', id: nextCaseId++, session }),
  pickFile: () => caseSend({ op: 'pickFile', id: nextCaseId++ }),
  attachFile: (session, path) => caseSend({ op: 'attachFile', id: nextCaseId++, session, path }),
};

contextBridge.exposeInMainWorld('studioCase', caseFiles);

// ---- the Claude port (F-814, src/shared/claude.ts): the same pattern as the project port, its own ids and listeners

const CLAUDE_PORT_CHANNEL: typeof ClaudeChannel = 'studio.claude-port';
/** A send waits for the session check and the start of claude; the others are answered at once. */
const CLAUDE_TIMEOUT_MS = 60_000;

let claudePort: Port | null = null;
let nextClaudeId = 1;
let nextClaudeListener = 1;
const claudeQueued: ClaudeRequest[] = [];
const claudePending = new Map<number, { resolve: (r: ClaudeReply) => void; timer: ReturnType<typeof setTimeout> }>();
const claudeListeners = new Map<number, (s: ClaudeState) => void>();

const claudeSettle = (id: number, reply: ClaudeReply): void => {
  const p = claudePending.get(id);
  if (!p) return;
  claudePending.delete(id);
  clearTimeout(p.timer);
  p.resolve(reply);
};

const claudePost = (req: ClaudeRequest): void => {
  try {
    (claudePort as Port).postMessage(req);
  } catch (e) {
    claudeSettle(req.id, { ok: false, error: { code: 'bad_request', message: `the request cannot be sent: ${String(e)}` } });
  }
};

const claudeSend = <T,>(req: ClaudeRequest): Promise<ClaudeReply<T>> =>
  new Promise((resolve) => {
    const timer = setTimeout(() => {
      claudeSettle(req.id, {
        ok: false,
        error: claudePort
          ? { code: 'timeout', message: `no answer from the Studio's main process within ${CLAUDE_TIMEOUT_MS} ms` }
          : { code: 'no_port', message: 'the Studio has not connected this window to Claude Code' },
      });
    }, CLAUDE_TIMEOUT_MS);
    claudePending.set(req.id, { resolve: resolve as (r: ClaudeReply) => void, timer });
    if (claudePort) claudePost(req);
    else claudeQueued.push(req);
  });

ipcRenderer.on(CLAUDE_PORT_CHANNEL, (event) => {
  const p = event.ports[0];
  if (!p) return;
  claudePort?.close();
  claudePort = p;
  p.onmessage = (e: { data: unknown }) => {
    const m = e.data as ClaudePortMessage;
    if (m?.type === 'reply') claudeSettle(m.id, m.reply);
    else if (m?.type === 'state') {
      for (const l of claudeListeners.values()) {
        try {
          l(m.state);
        } catch {
          // the page's listener failed; others still run
        }
      }
    }
  };
  for (const req of claudeQueued.splice(0)) claudePost(req);
});

const claude: ClaudeBridge = {
  state: async () => {
    const r = await claudeSend<ClaudeState>({ op: 'state', id: nextClaudeId++ });
    if (r.ok) return r.result;
    throw new Error(r.error.message);
  },
  onState: (listener) => {
    const id = nextClaudeListener++;
    if (typeof listener === 'function') claudeListeners.set(id, listener);
    return id;
  },
  offState: (id) => {
    claudeListeners.delete(id);
  },
  select: (session) => claudeSend({ op: 'select', id: nextClaudeId++, session }),
  send: (session, text) => claudeSend({ op: 'send', id: nextClaudeId++, session, text }),
  cancel: () => claudeSend({ op: 'cancel', id: nextClaudeId++ }),
  confirm: (token) => claudeSend({ op: 'confirm', id: nextClaudeId++, token }),
  refresh: () => claudeSend({ op: 'refresh', id: nextClaudeId++ }),
  allow: (rule, base) => claudeSend({ op: 'allow', id: nextClaudeId++, rule, base }),
  undoAllow: (force) => claudeSend({ op: 'undoAllow', id: nextClaudeId++, force }),
};

contextBridge.exposeInMainWorld('studioClaude', claude);

// ---- the run port (F-810, src/shared/run.ts)
//
// `window.studioRunData`: requests and replies only, like the case port. The main process asks the open project's read-only
// project server, which reads the run's residuals.jsonl on every call.

const RUN_PORT_CHANNEL: typeof RunChannel = 'studio.run-port';
const RUN_TIMEOUT_MS = 60_000;

let runPort: Port | null = null;
let nextRunId = 1;
const runQueued: RunRequest[] = [];
const runPending = new Map<number, { resolve: (r: RunReply) => void; timer: ReturnType<typeof setTimeout> }>();

const runSettle = (id: number, reply: RunReply): void => {
  const p = runPending.get(id);
  if (!p) return;
  runPending.delete(id);
  clearTimeout(p.timer);
  p.resolve(reply);
};

const runPost = (req: RunRequest): void => {
  try {
    (runPort as Port).postMessage(req);
  } catch (e) {
    runSettle(req.id, { ok: false, error: { code: 'bad_request', message: `the request cannot be sent: ${String(e)}` } });
  }
};

const runSend = <T,>(req: RunRequest, waitMs = RUN_TIMEOUT_MS): Promise<RunReply<T>> =>
  new Promise((resolve) => {
    const timer = setTimeout(() => {
      runSettle(req.id, {
        ok: false,
        error: runPort
          ? { code: 'timeout', message: `no answer from the Studio's main process within ${waitMs} ms` }
          : { code: 'no_port', message: 'the Studio has not connected this window to the run histories' },
      });
    }, waitMs);
    runPending.set(req.id, { resolve: resolve as (r: RunReply) => void, timer });
    if (runPort) runPost(req);
    else runQueued.push(req);
  });

ipcRenderer.on(RUN_PORT_CHANNEL, (event) => {
  const p = event.ports[0];
  if (!p) return;
  runPort?.close();
  runPort = p;
  p.onmessage = (e: { data: unknown }) => {
    const m = e.data as RunPortMessage;
    if (m?.type === 'reply') runSettle(m.id, m.reply);
  };
  for (const req of runQueued.splice(0)) runPost(req);
});

const runHistories: RunBridge = {
  series: (run) => runSend({ op: 'series', id: nextRunId++, run }),
  at: (run, step) => runSend({ op: 'at', id: nextRunId++, run, step }),
  compare: (run, other) => runSend({ op: 'compare', id: nextRunId++, run, other }),
  // F-837
  log: (run) => runSend({ op: 'log', id: nextRunId++, run }),
  openReport: (run) => runSend({ op: 'openReport', id: nextRunId++, run }),
  budgets: () => runSend({ op: 'budgets', id: nextRunId++ }),
  setBudgets: (values, base, replace) => runSend({ op: 'setBudgets', id: nextRunId++, values, base, replace }),
  undoBudgets: (force) => runSend({ op: 'undoBudgets', id: nextRunId++, force }),
  // the save dialog waits for the user: an hour
  savePng: (rect, name) => runSend({ op: 'savePng', id: nextRunId++, rect, name }, 3_600_000),
};

contextBridge.exposeInMainWorld('studioRunData', runHistories);

// ---- the provenance port (F-828, src/shared/provenance.ts)
//
// `window.studioProvenance`: requests and replies only, like the run port. The main process reads a run's record through
// the open project's read-only project server, and writes the clipboard for Copy.

const PROVENANCE_PORT_CHANNEL: typeof ProvenanceChannel = 'studio.provenance-port';
const PROVENANCE_TIMEOUT_MS = 60_000;

let provenancePort: Port | null = null;
let nextProvenanceId = 1;
const provenanceQueued: ProvenanceRequest[] = [];
const provenancePending = new Map<number, { resolve: (r: ProvenanceReply) => void; timer: ReturnType<typeof setTimeout> }>();

const provenanceSettle = (id: number, reply: ProvenanceReply): void => {
  const p = provenancePending.get(id);
  if (!p) return;
  provenancePending.delete(id);
  clearTimeout(p.timer);
  p.resolve(reply);
};

const provenancePost = (req: ProvenanceRequest): void => {
  try {
    (provenancePort as Port).postMessage(req);
  } catch (e) {
    provenanceSettle(req.id, { ok: false, error: { code: 'bad_request', message: `the request cannot be sent: ${String(e)}` } });
  }
};

const provenanceSend = <T,>(req: ProvenanceRequest): Promise<ProvenanceReply<T>> =>
  new Promise((resolve) => {
    const timer = setTimeout(() => {
      provenanceSettle(req.id, {
        ok: false,
        error: provenancePort
          ? { code: 'timeout', message: `no answer from the Studio's main process within ${PROVENANCE_TIMEOUT_MS} ms` }
          : { code: 'no_port', message: 'the Studio has not connected this window to the provenance port' },
      });
    }, PROVENANCE_TIMEOUT_MS);
    provenancePending.set(req.id, { resolve: resolve as (r: ProvenanceReply) => void, timer });
    if (provenancePort) provenancePost(req);
    else provenanceQueued.push(req);
  });

ipcRenderer.on(PROVENANCE_PORT_CHANNEL, (event) => {
  const p = event.ports[0];
  if (!p) return;
  provenancePort?.close();
  provenancePort = p;
  p.onmessage = (e: { data: unknown }) => {
    const m = e.data as ProvenancePortMessage;
    if (m?.type === 'reply') provenanceSettle(m.id, m.reply);
  };
  for (const req of provenanceQueued.splice(0)) provenancePost(req);
});

const provenance: ProvenanceBridge = {
  run: (run) => provenanceSend({ op: 'run', id: nextProvenanceId++, run }),
  copy: (text) => provenanceSend({ op: 'copy', id: nextProvenanceId++, text }),
};

contextBridge.exposeInMainWorld('studioProvenance', provenance);
