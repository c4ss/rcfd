// The Studio's client of the solver service (F-803): it attaches to a running `rcfd serve` on loopback or starts its
// own, keeps two connections to it, long-polls the event log on one of them, and reconnects with back-off when the
// connection drops or the service restarts.
//
// - Control connection: every `call`, in the order sent (the service answers a connection's requests one after the
//   other, so order is kept and a slow call delays the ones behind it; the renderer's allow-list leaves out the calls
//   that wait for a run to end).
// - Events connection: nothing but `events.poll {after, timeout_ms}` in a loop, so a waiting poll never holds a call up.
// - After a reconnection the client tells a restarted service from the same one by the event it saw last: the same
//   instance still has that event under its sequence number, a new one has a shorter log or a different event there.
//   A new instance starts the cursor at 0 (all of its events are new to the Studio) and raises `instance`.
// - Runs outlive the window (G7): a reload of the renderer does not touch this client. G7 as amended 2026-11-17: a
//   service this start of the Studio started is stopped when the Studio quits (runs are the runner's, not the
//   service's); with --keep-runner or the Studio setting, and for a service found running (adopted), only if it holds no
//   session, as before; otherwise it keeps running and the next start finds it again through `service.json` (spawn.ts).
// - The shipped rcfd of an installed Studio (config `start: 'project'`, F-820) is started by `start()` when the first
//   project opens; until then the state is `waiting`. `findRunning()` at launch connects only to a service an earlier
//   start left running (G7: its sessions are found again without a project), and starts nothing.
//
// Everything is asynchronous; no call waits on the main process's thread. No Electron import (unit-tested in Node).
import type { ChildProcess } from 'node:child_process';
import {
  SERVICE_PROTOCOL,
  type ServiceEvent,
  type ServiceInfo,
  type ServiceState,
} from '../../shared/service.js';
import { backoffMs, type ServiceConfig } from './config.js';
import { RpcConnection, RpcError } from './rpc.js';
import { processAlive, readRecord, removeRecord, startService, writeRecord } from './spawn.js';

export interface ClientEventBatch {
  readonly instance: number;
  readonly events: readonly ServiceEvent[];
  readonly missed: number;
}

export interface ServiceClientOptions {
  readonly connectTimeoutMs?: number;
  /** `timeout_ms` of each `events.poll` (the service caps it at 60 s). */
  readonly pollTimeoutMs?: number;
  /** `limit` of each `events.poll`. */
  readonly pollLimit?: number;
  /** Retry delay after `attempt` failures in a row. */
  readonly backoff?: (attempt: number) => number;
}

const INITIAL: ServiceState = {
  status: 'off',
  mode: 'off',
  address: null,
  pid: null,
  instance: 0,
  adopted: false,
  version: null,
  protocolVersion: null,
  attempts: 0,
  retryInMs: null,
  lastError: null,
};

interface Link {
  readonly control: RpcConnection;
  readonly events: RpcConnection;
}

export class ServiceClient {
  private st: ServiceState;
  private link: Link | null = null;
  /** Raised on every new link or teardown: loops and callbacks of an older link see a different value and stop. */
  private epoch = 0;
  private stopped = false;
  private started = false;
  private retryTimer: NodeJS.Timeout | null = null;
  private child: ChildProcess | null = null;
  private target: { host: string; port: number } | null = null;
  private everConnected = false;
  private lastPid: number | null = null;
  private cursor = 0;
  private lastSeen: { seq: number; text: string } | null = null;
  private readonly stateListeners = new Set<(s: ServiceState) => void>();
  private readonly eventListeners = new Set<(b: ClientEventBatch) => void>();
  private readonly connectTimeoutMs: number;
  private readonly pollTimeoutMs: number;
  private readonly pollLimit: number;
  private readonly backoff: (attempt: number) => number;

  constructor(
    private readonly config: ServiceConfig,
    options: ServiceClientOptions = {},
  ) {
    this.connectTimeoutMs = options.connectTimeoutMs ?? 3000;
    this.pollTimeoutMs = options.pollTimeoutMs ?? 15_000;
    this.pollLimit = options.pollLimit ?? 500;
    this.backoff = options.backoff ?? backoffMs;
    const mode = config.mode === 'attach' || config.mode === 'spawn' ? config.mode : 'off';
    this.st = {
      ...INITIAL,
      status: config.mode === 'spawn' && config.start === 'project' ? 'waiting' : 'off',
      mode,
      address: config.mode === 'attach' ? `${config.host}:${config.port}` : null,
      lastError: config.mode === 'invalid' || config.mode === 'off' ? config.reason : null,
    };
    if (config.mode === 'attach') this.target = { host: config.host, port: config.port };
  }

  get state(): ServiceState {
    return this.st;
  }

  onState(listener: (s: ServiceState) => void): () => void {
    this.stateListeners.add(listener);
    return () => this.stateListeners.delete(listener);
  }

  onEvents(listener: (b: ClientEventBatch) => void): () => void {
    this.eventListeners.add(listener);
    return () => this.eventListeners.delete(listener);
  }

  /** Starts connecting (and keeps trying). Does nothing without a configured service, or a second time. */
  start(): void {
    if (this.config.mode !== 'attach' && this.config.mode !== 'spawn') return;
    if (this.stopped || this.started) return;
    this.started = true;
    void this.connect();
  }

  /**
   * Spawn mode: connects (as `start()`) only when a service an earlier start of the Studio left running is there and
   * answers; starts no process. True when it was found. Used at launch when the service waits for a project (F-820).
   */
  async findRunning(): Promise<boolean> {
    if (this.config.mode !== 'spawn' || this.stopped || this.started) return false;
    const rec = await readRecord(this.config.stateDir);
    if (!rec || !processAlive(rec.pid) || !(await answers(rec.host, rec.port, this.connectTimeoutMs))) return false;
    this.start();
    return true;
  }

  /** One request on the control connection. Rejects with an RpcError when there is no connection, at once. */
  call(method: string, params: Record<string, unknown> = {}, timeoutMs?: number): Promise<unknown> {
    if (this.config.mode === 'off') return Promise.reject(new RpcError('not_configured', `No rcfd service: ${this.config.reason}`));
    if (this.config.mode === 'invalid') return Promise.reject(new RpcError('not_configured', this.config.reason));
    const link = this.link;
    if (this.st.status === 'waiting') {
      return Promise.reject(new RpcError('disconnected', 'the rcfd service starts when a project is opened'));
    }
    if (!link || this.st.status !== 'connected') {
      return Promise.reject(new RpcError('disconnected', `no connection to the service (${this.st.status}${this.st.lastError ? `: ${this.st.lastError}` : ''})`));
    }
    return link.control.request(method, params, timeoutMs);
  }

  /**
   * Stops the client for good (the Studio is quitting). A service the Studio started is stopped too when it holds no
   * session; with sessions it is left running for the next start (G7). Resolves with whether it was left running.
   *
   * G7 as amended 2026-11-17: `keepWithSessions: false` (the Studio's quit unless --keep-runner or the setting keeps the
   * runner) stops a service this start of the Studio started when none of its sessions has a solve going in the service
   * itself (`running` or `paused`, `run.start` on a session): the project's runs are the runner's, and the case
   * editor's sessions end with the Studio. A service found running (adopted) keeps the rule above: the Studio never
   * stops a service it did not start while it holds someone's session.
   */
  async shutdown(opts: { readonly keepWithSessions?: boolean } = {}): Promise<{ leftRunning: boolean }> {
    this.stopped = true;
    if (this.retryTimer) clearTimeout(this.retryTimer);
    this.retryTimer = null;
    let leftRunning = false;
    const pid = this.st.pid;
    if (this.config.mode === 'spawn' && pid !== null && processAlive(pid)) {
      let keep = true;
      const link = this.link;
      const owned = opts.keepWithSessions === false && !this.st.adopted;
      if (link && this.st.status === 'connected') {
        try {
          const r = (await link.control.request('session.list', {}, 1500)) as { sessions?: unknown };
          keep = owned ? keptForSolves(r.sessions) : !Array.isArray(r.sessions) || r.sessions.length > 0;
        } catch {
          keep = true; // cannot tell: never stop what may hold someone's run (G7, G18)
        }
      }
      if (keep) leftRunning = true;
      else {
        try {
          if (this.child && this.child.pid === pid) this.child.kill();
          else process.kill(pid);
        } catch {
          // already gone
        }
        await removeRecord(this.config.stateDir).catch(() => undefined);
      }
    }
    this.teardown('the Studio is quitting');
    this.setState({ status: 'stopped', retryInMs: null });
    return { leftRunning };
  }

  // ---- connection

  private async connect(): Promise<void> {
    let opened: Link | null = null;
    try {
      const target = await this.resolveTarget();
      this.setState({ status: 'connecting', address: `${target.host}:${target.port}`, retryInMs: null });
      const epoch = ++this.epoch;
      const onClose = (reason: string) => {
        if (epoch === this.epoch) this.lost(reason);
      };
      const opts = { connectTimeoutMs: this.connectTimeoutMs, onClose };
      const both = await Promise.allSettled([
        RpcConnection.connectTcp(target.host, target.port, opts),
        RpcConnection.connectTcp(target.host, target.port, opts),
      ]);
      const failed = both.find((r) => r.status === 'rejected');
      if (failed) {
        this.epoch++;
        for (const r of both) if (r.status === 'fulfilled') r.value.close('connection attempt failed');
        throw failed.reason;
      }
      const [control, events] = both.map((r) => (r as PromiseFulfilledResult<RpcConnection>).value) as [RpcConnection, RpcConnection];
      opened = { control, events };
      if (this.stopped || epoch !== this.epoch) throw new Error('superseded');
      const info = (await control.request('service.info', {}, this.connectTimeoutMs)) as Partial<ServiceInfo>;
      if (info?.protocol !== SERVICE_PROTOCOL) {
        throw new RpcError('protocol', `the service at ${target.host}:${target.port} speaks ${String(info?.protocol)}, not ${SERVICE_PROTOCOL}`);
      }
      const newInstance = await this.resolveCursor(events);
      if (this.stopped || epoch !== this.epoch) throw new Error('superseded');
      this.link = opened;
      this.everConnected = true;
      this.lastPid = this.st.pid;
      this.setState({
        status: 'connected',
        instance: newInstance ? this.st.instance + 1 : this.st.instance,
        version: typeof info.version === 'string' ? info.version : null,
        protocolVersion: typeof info.protocol_version === 'string' ? info.protocol_version : null,
        attempts: 0,
        retryInMs: null,
        lastError: null,
      });
      void this.pollLoop(epoch, events);
    } catch (e) {
      if (opened && this.link !== opened) {
        this.epoch++; // their close callbacks must not report a loss: this attempt reports its own reason below
        opened.control.close('connection attempt failed');
        opened.events.close('connection attempt failed');
      }
      if (!this.stopped) this.scheduleRetry(e instanceof Error ? e.message : String(e));
    }
  }

  /** Where to connect. In spawn mode: our own process if it still runs, else the one an earlier start left, else a new one. */
  private async resolveTarget(): Promise<{ host: string; port: number }> {
    if (this.config.mode === 'attach') return this.target!;
    if (this.config.mode !== 'spawn') throw new Error('no service configured');
    const cfg = this.config;
    if (this.target && this.st.pid !== null && processAlive(this.st.pid)) return this.target;
    if (!this.everConnected && this.st.pid === null) {
      const rec = await readRecord(cfg.stateDir);
      if (rec && processAlive(rec.pid) && (await answers(rec.host, rec.port, this.connectTimeoutMs))) {
        this.target = { host: rec.host, port: rec.port };
        this.setState({ pid: rec.pid, adopted: true });
        return this.target;
      }
      if (rec) await removeRecord(cfg.stateDir);
    }
    this.setState({ status: 'starting', pid: null, adopted: false, retryInMs: null });
    const s = await startService(cfg.rcfd, cfg.root, cfg.stateDir);
    if (this.stopped) {
      // the Studio began to quit while the service started: it holds nothing yet
      s.child.kill();
      throw new Error('the Studio is quitting');
    }
    this.child = s.child;
    s.child.once('exit', () => {
      // the sockets close too; this makes sure the next attempt starts a new process at once
      if (this.child === s.child) this.child = null;
    });
    this.target = { host: s.host, port: s.port };
    this.setState({ pid: s.pid });
    await writeRecord(cfg.stateDir, { host: s.host, port: s.port, pid: s.pid, rcfd: cfg.rcfd, startedAt: new Date().toISOString() });
    return this.target;
  }

  /**
   * Sets the event cursor for a new connection; true when the service is a new instance. First connection: the cursor
   * starts at the head of the log (events from now on). Later: see the file comment.
   */
  private async resolveCursor(events: RpcConnection): Promise<boolean> {
    const head = await findHead(events);
    if (!this.everConnected) {
      this.cursor = head;
      return true;
    }
    let fresh: boolean;
    if (this.config.mode === 'spawn' && this.st.pid !== this.lastPid) fresh = true;
    else if (this.lastSeen) {
      if (head < this.lastSeen.seq) fresh = true;
      else {
        const r = await pollOnce(events, this.lastSeen.seq - 1);
        const e = r.events[0];
        if (e && e.seq === this.lastSeen.seq) fresh = JSON.stringify(e) !== this.lastSeen.text;
        else fresh = false; // that event left the bounded log: same instance, the gap shows up as `missed`
      }
    } else fresh = head < this.cursor;
    if (fresh) {
      this.cursor = 0;
      this.lastSeen = null;
    }
    return fresh;
  }

  private async pollLoop(epoch: number, events: RpcConnection): Promise<void> {
    while (!this.stopped && epoch === this.epoch) {
      let r: { events: ServiceEvent[]; next: number };
      try {
        r = await pollOnce(events, this.cursor, this.pollTimeoutMs, this.pollLimit, this.pollTimeoutMs + 10_000);
      } catch (e) {
        // a poll that outlives its own timeout by 10 s, or a reply of the wrong shape: the connection is not usable
        if (epoch === this.epoch) events.close(`events.poll failed: ${e instanceof Error ? e.message : String(e)}`);
        return;
      }
      if (epoch !== this.epoch) return;
      if (r.events.length > 0) {
        const first = r.events[0]!;
        const last = r.events[r.events.length - 1]!;
        const missed = Math.max(0, first.seq - this.cursor - 1);
        this.cursor = last.seq;
        this.lastSeen = { seq: last.seq, text: JSON.stringify(last) };
        const batch: ClientEventBatch = { instance: this.st.instance, events: r.events, missed };
        for (const l of this.eventListeners) {
          try {
            l(batch);
          } catch {
            // a listener's failure is its own
          }
        }
      } else if (r.next > this.cursor) this.cursor = r.next;
    }
  }

  private lost(reason: string): void {
    if (this.stopped) return;
    this.teardown(reason);
    this.scheduleRetry(reason);
  }

  private teardown(reason: string): void {
    this.epoch++;
    const link = this.link;
    this.link = null;
    link?.control.close(reason);
    link?.events.close(reason);
  }

  private scheduleRetry(reason: string): void {
    if (this.stopped || this.retryTimer) return;
    const delay = this.backoff(this.st.attempts);
    this.setState({ status: 'disconnected', attempts: this.st.attempts + 1, retryInMs: delay, lastError: reason });
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null;
      if (!this.stopped) void this.connect();
    }, delay);
  }

  private setState(patch: Partial<ServiceState>): void {
    this.st = Object.freeze({ ...this.st, ...patch });
    for (const l of this.stateListeners) {
      try {
        l(this.st);
      } catch {
        // a listener's failure is its own
      }
    }
  }
}

/** One `events.poll`, with the reply's shape checked. */
async function pollOnce(
  conn: RpcConnection,
  after: number,
  timeoutMs = 0,
  limit = 1,
  requestTimeoutMs = 10_000,
): Promise<{ events: ServiceEvent[]; next: number }> {
  const r = (await conn.request('events.poll', { after, timeout_ms: timeoutMs, limit }, requestTimeoutMs)) as {
    events?: unknown;
    next?: unknown;
  };
  if (!Array.isArray(r?.events) || typeof r.next !== 'number') throw new RpcError('protocol', 'events.poll: a reply without events and next');
  for (const e of r.events as unknown[]) {
    const ev = e as Partial<ServiceEvent> | null;
    if (typeof ev !== 'object' || ev === null || typeof ev.seq !== 'number' || typeof ev.kind !== 'string') {
      throw new RpcError('protocol', 'events.poll: an event without seq and kind');
    }
  }
  return { events: r.events as ServiceEvent[], next: r.next };
}

/**
 * The sequence number of the newest event in the service's log (0 when empty). `events.poll {after: x}` finds an event
 * exactly when the newest one is above x (the bounded log drops the oldest first), so the head is found by doubling
 * and halving x: about 2 log2(head) instant requests.
 */
export async function findHead(conn: RpcConnection): Promise<number> {
  const above = async (x: number) => (await pollOnce(conn, x)).events.length > 0;
  if (!(await above(0))) return 0;
  let lo = 0; // above(lo) is true
  let hi = 1;
  while (await above(hi)) {
    lo = hi;
    hi *= 2;
    if (hi > Number.MAX_SAFE_INTEGER) throw new RpcError('protocol', 'events.poll: no end to the event log');
  }
  // above(lo) true, above(hi) false: the head is in (lo, hi]
  while (hi - lo > 1) {
    const mid = Math.floor((lo + hi) / 2);
    if (await above(mid)) lo = mid;
    else hi = mid;
  }
  return hi;
}

/** True when a service of this protocol answers at host:port. */
async function answers(host: string, port: number, timeoutMs: number): Promise<boolean> {
  let c: RpcConnection | null = null;
  try {
    c = await RpcConnection.connectTcp(host, port, { connectTimeoutMs: timeoutMs });
    const info = (await c.request('service.info', {}, timeoutMs)) as Partial<ServiceInfo>;
    return info?.protocol === SERVICE_PROTOCOL;
  } catch {
    return false;
  } finally {
    c?.close();
  }
}

/**
 * G7 as amended 2026-11-17: whether a service the Studio started is kept at its quit for a solve going in it: a session
 * whose state is `running` or `paused` (session.list), or a list that cannot be read.
 */
export function keptForSolves(sessions: unknown): boolean {
  if (!Array.isArray(sessions)) return true;
  return sessions.some((x) => {
    const state = typeof x === 'object' && x !== null ? (x as { state?: unknown }).state : undefined;
    return state === 'running' || state === 'paused';
  });
}
