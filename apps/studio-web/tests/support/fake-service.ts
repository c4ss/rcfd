// A stand-in for `rcfd serve` in tests: newline-delimited JSON-RPC 2.0 over TCP with the few methods the client needs,
// an event log with `events.poll` long-polls, and a record of every line it received. It is not the service: the
// replies only have the shapes docs/service-protocol.md gives, and the tests against the real binary are in tests/e2e.
import net from 'node:net';

export interface FakeEvent {
  seq: number;
  session: string;
  kind: string;
  data: unknown;
}

export interface FakeOptions {
  port?: number;
  protocol?: string;
  /** Event log capacity (oldest dropped first), as ServiceConfig.event_capacity. */
  capacity?: number;
  /** First sequence number minus one: a restarted service starts at 0. */
  seqStart?: number;
}

interface Waiter {
  after: number;
  session: string | undefined;
  limit: number;
  done: (events: FakeEvent[], next: number) => void;
}

export class FakeService {
  readonly received: { method: string; params: unknown; id: unknown }[] = [];
  readonly sockets = new Set<net.Socket>();
  connections = 0;
  /** Lines that were not JSON-RPC (for example an HTTP request). */
  readonly foreign: string[] = [];
  private seq: number;
  private log: FakeEvent[] = [];
  private waiters = new Set<Waiter>();
  private sessions = new Map<string, string>();
  private nextSession = 1;
  private constructor(
    private readonly server: net.Server,
    readonly port: number,
    private readonly opts: FakeOptions,
  ) {
    this.seq = opts.seqStart ?? 0;
  }

  static async start(opts: FakeOptions = {}): Promise<FakeService> {
    const server = net.createServer();
    await new Promise<void>((resolve, reject) => {
      server.once('error', reject);
      server.listen(opts.port ?? 0, '127.0.0.1', () => resolve());
    });
    const port = (server.address() as net.AddressInfo).port;
    const fake = new FakeService(server, port, opts);
    server.on('connection', (s) => fake.accept(s));
    return fake;
  }

  get head(): number {
    return this.seq;
  }

  methods(): string[] {
    return this.received.map((r) => r.method);
  }

  emit(session: string, kind: string, data: unknown = {}): FakeEvent {
    const e = { seq: ++this.seq, session, kind, data };
    this.log.push(e);
    const cap = this.opts.capacity ?? 20_000;
    while (this.log.length > cap) this.log.shift();
    for (const w of [...this.waiters]) {
      const found = this.find(w.after, w.session, w.limit);
      if (found.length) {
        this.waiters.delete(w);
        w.done(found, found[found.length - 1]!.seq);
      }
    }
    return e;
  }

  /** Sends a raw line on every connection (for malformed replies). */
  broadcast(line: string): void {
    for (const s of this.sockets) s.write(line);
  }

  /** Drops every connection but keeps listening (a network hiccup, not a restart). */
  dropConnections(): void {
    for (const s of this.sockets) s.destroy();
  }

  /** Stops listening and drops every connection (the service is gone). */
  async stop(): Promise<void> {
    for (const w of this.waiters) w.done([], this.seq);
    this.waiters.clear();
    for (const s of this.sockets) s.destroy();
    await new Promise<void>((resolve) => this.server.close(() => resolve()));
  }

  private find(after: number, session: string | undefined, limit: number): FakeEvent[] {
    return this.log.filter((e) => e.seq > after && (session === undefined || e.session === session)).slice(0, limit);
  }

  private accept(s: net.Socket): void {
    this.connections++;
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
    let req: { id?: unknown; method?: unknown; params?: Record<string, unknown> };
    try {
      req = JSON.parse(line);
    } catch {
      this.foreign.push(line);
      this.reply(s, null, { error: { code: -32700, message: 'parse error' } });
      return;
    }
    const method = String(req.method);
    const p = req.params ?? {};
    this.received.push({ method, params: p, id: req.id });
    const id = req.id;
    switch (method) {
      case 'service.info':
        return this.reply(s, id, {
          result: { name: 'fake', version: '0.0.0', protocol: this.opts.protocol ?? 'rcfd-service/1', protocol_version: '1.1', methods: ['service.info'] },
        });
      case 'session.list':
        return this.reply(s, id, { result: { sessions: [...this.sessions].map(([session, state]) => ({ session, state })) } });
      case 'session.create': {
        const sid = `s${this.nextSession++}`;
        this.sessions.set(sid, 'defined');
        this.emit(sid, 'created');
        return this.reply(s, id, { result: { session: sid } });
      }
      case 'run.status':
        return this.reply(s, id, { result: { session: p['session'], state: this.sessions.get(String(p['session'])) ?? 'unknown' } });
      case 'settings.get':
        return this.reply(s, id, { result: { echo: p } });
      case 'settings.validate':
        return this.reply(s, id, { error: { code: 1003, message: 'settings failed validation', data: { diagnostics: [{ path: 'mesh', line: 1, message: 'x' }] } } });
      case 'run.step': {
        // answers after `delay_ms` (an out-of-order reply when a faster request follows)
        const ms = Number(p['delay_ms'] ?? 0);
        await new Promise((r) => setTimeout(r, ms));
        return this.reply(s, id, { result: { stepped: true, delay_ms: ms } });
      }
      case 'events.poll': {
        const after = Number(p['after'] ?? 0);
        const limit = Math.max(1, Math.min(5000, Number(p['limit'] ?? 500)));
        const session = typeof p['session'] === 'string' ? p['session'] : undefined;
        const timeout = Math.min(60_000, Number(p['timeout_ms'] ?? 0));
        const found = this.find(after, session, limit);
        if (found.length || timeout === 0) {
          return this.reply(s, id, { result: { events: found, next: found.length ? found[found.length - 1]!.seq : Math.max(this.seq, after) } });
        }
        const w: Waiter = {
          after,
          session,
          limit,
          done: (events, next) => {
            clearTimeout(t);
            this.reply(s, id, { result: { events, next: events.length ? next : Math.max(this.seq, after) } });
          },
        };
        const t = setTimeout(() => {
          this.waiters.delete(w);
          w.done([], this.seq);
        }, timeout);
        this.waiters.add(w);
        s.once('close', () => {
          clearTimeout(t);
          this.waiters.delete(w);
        });
        return;
      }
      default:
        return this.reply(s, id, { error: { code: -32601, message: `unknown method '${method}'` } });
    }
  }
}

/** Waits until `f` returns a truthy value, polling every 20 ms. */
export async function until<T>(f: () => T | Promise<T>, timeoutMs = 10_000, what = 'condition'): Promise<NonNullable<T>> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const v = await f();
    if (v) return v as NonNullable<T>;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 20));
  }
}
