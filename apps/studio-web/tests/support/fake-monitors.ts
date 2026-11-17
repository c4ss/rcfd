// A stand-in for `rcfd serve` that holds monitor histories (F-811 tests): newline-delimited JSON-RPC 2.0 over TCP with
// `service.info`, an empty `events.poll`, `run.status` and `monitors.get` with the semantics of the service
// (crates/rcfd-api/src/service.rs and rcfd-post MonitorStore::to_json_filtered, ADR 0353): `names` (an array; entries
// that are not strings are skipped, names that are not series ignored; not an array: all series) and `since` (a number:
// each series' suffix after partition_point(x <= since); not a number: ignored), an unknown session answered with
// 1001, a session without a simulation with 1002, non-finite numbers written as null. It is not the service; the tests
// against the real binary are in tests/e2e.
import net from 'node:net';

export interface FakeSeries {
  unit: string;
  x: number[];
  y: number[];
}

interface FakeSession {
  /** null: no simulation yet (state defined). */
  store: Map<string, FakeSeries> | null;
  xLabel: string;
  /** Delay of each monitors.get reply, ms (to let replies of two runs cross). */
  delayMs: number;
}

export class FakeMonitorService {
  readonly received: { method: string; params: Record<string, unknown> }[] = [];
  private readonly sessions = new Map<string, FakeSession>();
  private readonly sockets = new Set<net.Socket>();

  private constructor(
    private readonly server: net.Server,
    readonly port: number,
  ) {}

  static async start(): Promise<FakeMonitorService> {
    const server = net.createServer();
    await new Promise<void>((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', () => resolve());
    });
    const fake = new FakeMonitorService(server, (server.address() as net.AddressInfo).port);
    server.on('connection', (s) => fake.accept(s));
    return fake;
  }

  /** Adds a session; `simulation: false` is a session in state defined (monitors.get answers 1002). */
  addSession(id: string, opts: { simulation?: boolean; xLabel?: string; delayMs?: number } = {}): void {
    this.sessions.set(id, { store: opts.simulation === false ? null : new Map(), xLabel: opts.xLabel ?? 'iteration', delayMs: opts.delayMs ?? 0 });
  }

  /** Gives a defined session its simulation (empty histories). */
  build(id: string): void {
    this.sessions.get(id)!.store = new Map();
  }

  setDelay(id: string, ms: number): void {
    this.sessions.get(id)!.delayMs = ms;
  }

  /** Appends one sample, as MonitorStore::push (the unit is kept from the first sample). */
  push(id: string, name: string, unit: string, x: number, y: number): void {
    const store = this.sessions.get(id)!.store!;
    let s = store.get(name);
    if (!s) {
      s = { unit, x: [], y: [] };
      store.set(name, s);
    }
    s.x.push(x);
    s.y.push(y);
  }

  /** The histories now (a copy). */
  histories(id: string): Map<string, FakeSeries> {
    const out = new Map<string, FakeSeries>();
    for (const [k, v] of this.sessions.get(id)!.store ?? []) out.set(k, { unit: v.unit, x: [...v.x], y: [...v.y] });
    return out;
  }

  /** Replaces the histories (the simulation was built again). */
  reset(id: string): void {
    this.sessions.get(id)!.store = new Map();
  }

  /** The monitors.get requests received, oldest first. */
  monitorRequests(): Record<string, unknown>[] {
    return this.received.filter((r) => r.method === 'monitors.get').map((r) => r.params);
  }

  async stop(): Promise<void> {
    for (const s of this.sockets) s.destroy();
    await new Promise<void>((resolve) => this.server.close(() => resolve()));
  }

  private accept(s: net.Socket): void {
    this.sockets.add(s);
    s.setNoDelay(true);
    s.on('close', () => this.sockets.delete(s));
    s.on('error', () => undefined);
    let buf = '';
    s.on('data', (chunk) => {
      buf += chunk.toString('utf8');
      for (let nl = buf.indexOf('\n'); nl >= 0; nl = buf.indexOf('\n')) {
        const line = buf.slice(0, nl);
        buf = buf.slice(nl + 1);
        if (line.trim()) void this.handle(s, line);
      }
    });
  }

  private reply(s: net.Socket, id: unknown, body: { result?: unknown; error?: unknown }): void {
    if (!s.destroyed) s.write(JSON.stringify({ jsonrpc: '2.0', id, ...body }) + '\n');
  }

  private async handle(s: net.Socket, line: string): Promise<void> {
    const req = JSON.parse(line) as { id?: unknown; method?: string; params?: Record<string, unknown> };
    const p = req.params ?? {};
    const method = String(req.method);
    this.received.push({ method, params: p });
    switch (method) {
      case 'service.info':
        return this.reply(s, req.id, { result: { name: 'fake-monitors', version: '0.0.0', protocol: 'rcfd-service/1', protocol_version: '1.1', methods: ['service.info', 'monitors.get'] } });
      case 'events.poll': {
        const after = Number(p['after'] ?? 0);
        const timeout = Math.min(2000, Number(p['timeout_ms'] ?? 0));
        if (timeout > 0) await new Promise((r) => setTimeout(r, timeout));
        return this.reply(s, req.id, { result: { events: [], next: after } });
      }
      case 'run.status': {
        const sess = this.sessions.get(String(p['session']));
        if (!sess) return this.reply(s, req.id, { error: { code: 1001, message: `unknown session '${String(p['session'])}'` } });
        return this.reply(s, req.id, { result: { session: p['session'], state: sess.store ? 'ready' : 'defined' } });
      }
      case 'monitors.get': {
        const sess = this.sessions.get(String(p['session']));
        if (!sess) return this.reply(s, req.id, { error: { code: 1001, message: `unknown session '${String(p['session'])}'` } });
        if (!sess.store) return this.reply(s, req.id, { error: { code: 1002, message: 'no simulation yet; call run.initialize or run.start' } });
        // the reply is built now (the state of the store when the request is handled), then delayed
        const result = monitorsDocument(sess.store, sess.xLabel, p['names'], p['since']);
        const text = JSON.stringify({ jsonrpc: '2.0', id: req.id, result }) + '\n';
        if (sess.delayMs > 0) await new Promise((r) => setTimeout(r, sess.delayMs));
        if (!s.destroyed) s.write(text);
        return;
      }
      default:
        return this.reply(s, req.id, { error: { code: -32601, message: `unknown method '${method}'` } });
    }
  }
}

/** Rust's slice::partition_point for a predicate `x <= after`: the first index where it is false (binary search). */
export function partitionPoint(xs: readonly number[], after: number): number {
  let lo = 0;
  let hi = xs.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (xs[mid]! <= after) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** The rcfd-monitors 1 document of `store` restricted by `names` and `since` (MonitorStore::to_json_filtered). */
export function monitorsDocument(store: Map<string, FakeSeries>, xLabel: string, names: unknown, since: unknown): unknown {
  const wanted = Array.isArray(names) ? new Set(names.filter((n): n is string => typeof n === 'string')) : null;
  const after = typeof since === 'number' ? since : null;
  const series: Record<string, { unit: string; x: number[]; y: number[] }> = {};
  for (const name of [...store.keys()].sort()) {
    if (wanted && !wanted.has(name)) continue;
    const s = store.get(name)!;
    const from = after === null ? 0 : partitionPoint(s.x, after);
    series[name] = { unit: s.unit, x: s.x.slice(from), y: s.y.slice(from) };
  }
  return { format: 'rcfd-monitors', version: 1, x_label: xLabel, series };
}
