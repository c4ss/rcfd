// The renderer's way to the solver service (F-803): the allow-list of service methods, the messages that cross the
// service port, and the bridge the preload script puts on `window.studioService`. Shared by all three processes, so it
// uses neither Node nor DOM types.
//
// How the renderer reaches the service (docs/studio-guardrails.md G2, G10): never directly. The main process holds the
// connections to `rcfd-service/1` (src/main/service/); after each load of an app page it hands the preload script one
// MessagePort on the channel SERVICE_PORT_CHANNEL. The preload keeps the port to itself and exposes a few functions;
// the main process checks every message that arrives on the port against the shapes below and SERVICE_METHODS before
// anything reaches the service. The shell's ipcMain channels (src/shared/ipc.ts) are not used for this.

/** The protocol identifier `service.info` returns (docs/service-protocol.md). */
export const SERVICE_PROTOCOL = 'rcfd-service/1';

/** The one IPC channel main uses to hand the preload script the service port (main -> renderer only). */
export const SERVICE_PORT_CHANNEL = 'studio.service-port';

/**
 * Methods that only read. `data.info` (protocol 1.2, opt-in) returns the service's clock and data port; a service
 * without `data.*` answers it with -32601. `help.glossary` and `help.errors` (ADR 0730, opt-in with `rcfd serve --ui`)
 * return the engine's glossary and error catalogue, and `case.facts` (ADR 0737, the same switch) a case's facts and run
 * estimate; a service without --ui answers them with -32601. `mesh.preview` (ADR
 * 0732, also --ui) builds the session's mesh without solving and writes it as a mesh-only store under the caller's
 * folder: it changes no session or run, so it is listed here, and the Studio names `.rcfd/previews/` of the project.
 * `mesh.refine` (ADR 0737, --ui) is a control method. The service's `case.save` is not on the list: the case editor's
 * main process calls it (src/main/case/editor.ts).
 */
export const SERVICE_READ_METHODS = [
  'service.info',
  'settings.schema',
  'session.list',
  'settings.get',
  'settings.validate',
  'mesh.check',
  'run.status',
  'monitors.get',
  'reports.get',
  'field.stats',
  'log.tail',
  'journal.get',
  'panel.snapshot',
  'data.info',
  'help.glossary',
  'help.errors',
  'mesh.preview',
  'case.facts',
] as const;

/** Methods that change a session or its run. */
export const SERVICE_CONTROL_METHODS = [
  'session.create',
  'session.close',
  'settings.set',
  'settings.remove',
  'run.initialize',
  'run.start',
  'run.pause',
  'run.resume',
  'run.stop',
  'run.step',
  'mesh.refine',
] as const;

/**
 * Every method the renderer may call through `service.call`. Not on it, on purpose:
 * - `events.poll`: the main process owns the long-poll; the renderer subscribes (`subscribe`).
 * - `run.wait`, `workflow.run`, `run.headless`, `design.*`, `journal.replay`: they hold the connection until a run or a
 *   study ends, and every call of the window shares one ordered connection.
 * - `field.slice`, `field.grid`, `field.sample`, `render.*`, `data.*` other than `data.info`: bulk data (F-804).
 * - `report.export`, `field.export_vtk`, `workflow.save`, `workflow.load`, `status.read`, `run.check`, `mesh.*` other than
 *   `mesh.check` and `mesh.preview`, `particles.report`, `workflow.*`, `cosim.*`: file output, other subsystems or co-simulation; each item
 *   that needs one adds it here with a test.
 * The Studio's named actions (G16, G20) will sit on top of this list; it is not the action layer.
 */
export const SERVICE_METHODS = [...SERVICE_READ_METHODS, ...SERVICE_CONTROL_METHODS] as const;

export type ServiceMethod = (typeof SERVICE_METHODS)[number];

/** Default time a call may take before the main process gives up on it (the call still runs in the service). */
export const DEFAULT_CALL_TIMEOUT_MS = 60_000;
/** Bounds of the `timeoutMs` a caller may ask for. */
export const MIN_CALL_TIMEOUT_MS = 1;
export const MAX_CALL_TIMEOUT_MS = 600_000;

/** Errors raised by the client itself rather than by the service (whose errors keep their JSON-RPC codes). */
export type ClientErrorCode =
  | 'not_allowed' // the method is not on SERVICE_METHODS
  | 'bad_request' // the message or its parameters have the wrong shape
  | 'not_configured' // the Studio was not told which service to use
  | 'disconnected' // no connection right now (see ServiceState)
  | 'timeout' // no reply within the call's time limit
  | 'protocol' // the service sent something that is not rcfd-service/1
  | 'no_port'; // the preload script has no service port (main never sent one)

export interface ServiceError {
  /** A JSON-RPC error code from the service (docs/service-protocol.md, Errors), or a client error code. */
  readonly code: number | ClientErrorCode;
  readonly message: string;
  /** `error.data` of the service's reply, when it gave one (for example the diagnostics of 1003). */
  readonly data?: unknown;
}

/** Every reply is a value, never a thrown error: errors carry their code across the bridge unchanged. */
export type ServiceReply<T = unknown> =
  | { readonly ok: true; readonly result: T }
  | { readonly ok: false; readonly error: ServiceError };

/** One entry of the service's event log, as `events.poll` returns it. */
export interface ServiceEvent {
  readonly seq: number;
  readonly session: string;
  readonly kind: string;
  readonly data: unknown;
}

/** What a subscriber receives: the events of one `events.poll` reply that match its filter. */
export interface ServiceEventBatch {
  readonly subscription: number;
  /** The service instance the events come from (ServiceState.instance). */
  readonly instance: number;
  readonly events: readonly ServiceEvent[];
  /**
   * Events of the service's log that were never seen: dropped from its bounded log before they were polled (a long
   * disconnection). The renderer should read the current state again when this is not 0.
   */
  readonly missed: number;
}

/** The members of `service.info` the Studio relies on; the reply carries more. */
export interface ServiceInfo {
  readonly name: string;
  readonly version: string;
  readonly protocol: string;
  readonly protocol_version: string;
  readonly methods: readonly string[];
  readonly [member: string]: unknown;
}

export type ServiceStatus =
  | 'off' // no service configured
  | 'waiting' // an installed Studio's own service, started when the first project is opened (F-820)
  | 'starting' // the Studio is starting its own service process
  | 'connecting'
  | 'connected'
  | 'disconnected' // lost; a new attempt follows after retryInMs
  | 'stopped'; // the Studio is quitting

export interface ServiceState {
  readonly status: ServiceStatus;
  /** `attach`: a service someone else started; `spawn`: the Studio starts and owns `rcfd serve`. */
  readonly mode: 'off' | 'attach' | 'spawn';
  /** `host:port` of the service, when known. */
  readonly address: string | null;
  /** Process id of a service the Studio started (or found again from an earlier start), else null. */
  readonly pid: number | null;
  /**
   * Counts the service instances this Studio has talked to: 1 for the first, one more each time a reconnection finds a
   * different instance (the service was restarted). The renderer reads its state again when it changes.
   */
  readonly instance: number;
  /** True when the Studio found a service it had started before it last quit, and attached to it. */
  readonly adopted: boolean;
  /** The service's version and protocol version, from `service.info` at connection. */
  readonly version: string | null;
  readonly protocolVersion: string | null;
  /** Connection attempts since the last success. */
  readonly attempts: number;
  /** Delay before the next attempt (status `disconnected`), else null. */
  readonly retryInMs: number | null;
  /** Why the last attempt or connection failed, in words. */
  readonly lastError: string | null;
}

// ---- Messages on the service port (preload <-> main). The main process validates every request (src/main/service/bridge.ts).

export interface CallOptions {
  /** Time limit of this call in milliseconds (MIN_CALL_TIMEOUT_MS to MAX_CALL_TIMEOUT_MS; DEFAULT_CALL_TIMEOUT_MS). */
  readonly timeoutMs?: number;
}

/** Preload -> main. `id` pairs a request with its reply. */
export type PortRequest =
  | { readonly op: 'call'; readonly id: number; readonly method: ServiceMethod; readonly params?: Record<string, unknown>; readonly timeoutMs?: number }
  | { readonly op: 'state'; readonly id: number }
  | { readonly op: 'subscribe'; readonly id: number; readonly subscription: number; readonly session?: string }
  | { readonly op: 'unsubscribe'; readonly id: number; readonly subscription: number };

/** Main -> preload. */
export type PortMessage =
  | { readonly type: 'reply'; readonly id: number; readonly reply: ServiceReply }
  | { readonly type: 'state'; readonly state: ServiceState }
  | ({ readonly type: 'events' } & ServiceEventBatch);

/** What the preload script puts on `window.studioService`. Each function's arguments are checked again in main. */
export interface ServiceBridge {
  /** `service.info` of the connected service. */
  info(): Promise<ServiceReply<ServiceInfo>>;
  /** One request of the allow-list. Resolves with the service's result or an error; never rejects. */
  call(method: ServiceMethod, params?: Record<string, unknown>, options?: CallOptions): Promise<ServiceReply>;
  /** The connection's current state. */
  state(): Promise<ServiceState>;
  /** Calls `listener` with every later state change; returns an id for `offState`. */
  onState(listener: (state: ServiceState) => void): number;
  offState(id: number): void;
  /**
   * Delivers the service's events from now on, optionally of one session only; resolves with the subscription id once
   * the main process has it, so events of a call made after that are not missed.
   */
  subscribe(listener: (batch: ServiceEventBatch) => void, options?: { session?: string }): Promise<ServiceReply<number>>;
  unsubscribe(subscription: number): Promise<ServiceReply<null>>;
}
