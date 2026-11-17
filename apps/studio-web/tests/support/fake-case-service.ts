// A stand-in for `rcfd serve` for the settings surface (F-825): newline-delimited JSON-RPC 2.0 over TCP with the
// methods the surface calls (service.info, settings.schema, session.create/list/close (close: F-829), run.status,
// settings.get/set/remove/validate) and a long-polled event log. Its case schema is a test schema with the shapes the engine's
// Schema::to_json_schema writes (crates/rcfd-core/src/settings.rs), and a test may change it while the Studio runs: that
// is how the tests show a key added to the schema appearing with no Studio code change.
//
// It is not the engine: it checks nothing. A test tells it which change to refuse and with which words, and which
// problems settings.validate reports; the Studio must show those words as they are.
import net from 'node:net';

export type Json = null | boolean | number | string | Json[] | { [k: string]: Json };
type Obj = { [k: string]: Json };

const isObj = (v: unknown): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v);

/** An object node as the engine writes it: properties, required in declaration order, no other members. */
export function object(fields: [string, Obj][], extra: Obj = {}): Obj {
  const optional = new Set(fields.filter(([, f]) => f['x-rcfd-optional'] === true).map(([k]) => k));
  return {
    type: 'object',
    properties: Object.fromEntries([...fields].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))),
    required: fields.map(([k]) => k).filter((k) => !optional.has(k)),
    additionalProperties: false,
    ...extra,
  };
}

export function named(entry: Obj, description: string): Obj {
  return { type: 'object', additionalProperties: entry, 'x-rcfd-named-objects': true, description };
}

/**
 * The test schema: a top-level string, a section with ten own keys of every leaf kind (so two sit one step away), a
 * nested object, an optional object, a list, and a named collection.
 */
export function testSchema(): Obj {
  const solver = object(
    [
      ['algorithm', { type: 'string', enum: ['simple', 'simplec', 'piso'], default: 'simplec', description: 'Pressure-velocity coupling of the test solver' }],
      ['alpha_u', { type: 'number', default: 0.9, minimum: 0.05, maximum: 1, description: 'Velocity under-relaxation factor' }],
      ['max_iterations', { type: 'integer', default: 500, minimum: 1, description: 'Iteration limit' }],
      ['dt', { type: 'number', default: 0.01, minimum: 0, 'x-rcfd-si-dimension': 's', 'x-rcfd-dimension-exponents': [0, 0, 1, 0, 0, 0, 0], description: 'Time step' }],
      ['adaptive', { type: 'boolean', default: false, description: 'Adapt the time step' }],
      ['note', { type: 'string', default: '', description: 'A free note' }],
      ['corrections', { type: 'integer', default: 0, minimum: 0, maximum: 5, description: 'Non-orthogonal correctors' }],
      ['direction', { type: 'array', items: { type: 'number', default: 0 }, description: 'Flow direction (empty list = +x)' }],
      ['tolerance', { type: 'number', default: 1e-6, minimum: 0, description: 'Residual tolerance' }],
      ['scheme', { type: 'string', enum: ['upwind', 'central'], default: 'upwind', description: 'Convection scheme' }],
      ['limits', object([['enabled', { type: 'boolean', default: false, description: 'Limit the velocity' }], ['max', { type: 'number', default: 100, description: 'Velocity limit', 'x-rcfd-si-dimension': 'm s^-1', 'x-rcfd-dimension-exponents': [1, 0, -1, 0, 0, 0, 0] }]], { description: 'Solution limits' })],
      ['rhie_chow', object([['enabled', { type: 'boolean', default: false, description: 'Consistent face interpolation' }]], { description: 'Rhie-Chow options', 'x-rcfd-optional': true })],
    ],
    { description: 'Numerical method and run control' },
  );
  const boundaries = named(
    object([
      ['kind', { type: 'string', enum: ['wall', 'inlet', 'outlet'], default: 'wall', description: 'Boundary kind' }],
      ['velocity', { type: 'array', items: { type: 'number', default: 0, 'x-rcfd-si-dimension': 'm s^-1' }, description: 'Velocity of the patch' }],
    ]),
    'Boundary conditions by patch name',
  );
  return {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    ...object([
      ['title', { type: 'string', default: '', description: 'Case title' }],
      ['solver', solver],
      ['boundaries', boundaries],
    ]),
  };
}

/** Adds a field to the object at a dotted path of a schema (`` for the root), as a later engine would. */
export function addKey(schema: Obj, at: string, name: string, node: Obj): Obj {
  const copy = JSON.parse(JSON.stringify(schema)) as Obj;
  let o: Obj = copy;
  for (const part of at === '' ? [] : at.split('.')) o = (o['properties'] as Obj)[part] as Obj;
  (o['properties'] as Obj)[name] = node;
  if (node['x-rcfd-optional'] !== true) (o['required'] as Json[]).push(name);
  return copy;
}

/** The defaults of a schema node, as the engine's Schema::default_value. */
export function defaults(node: Obj): Json {
  if (isObj(node['properties'])) {
    const out: Obj = {};
    for (const k of (node['required'] as string[]) ?? []) out[k] = defaults((node['properties'] as Obj)[k] as Obj);
    return out;
  }
  if (isObj(node['additionalProperties'])) return {};
  if (node['type'] === 'array') return [];
  return (node['default'] ?? null) as Json;
}

function getAt(tree: Json, path: string): Json | undefined {
  let v: Json | undefined = tree;
  for (const p of path.split('.')) {
    if (!isObj(v) || !(p in v)) return undefined;
    v = v[p];
  }
  return v;
}

function setAt(tree: Obj, path: string, value: Json): void {
  const parts = path.split('.');
  let o = tree;
  for (const p of parts.slice(0, -1)) {
    if (!isObj(o[p])) o[p] = {};
    o = o[p] as Obj;
  }
  o[parts[parts.length - 1]!] = value;
}

interface Session {
  state: string;
  tree: Obj;
}

interface Waiter {
  after: number;
  done: () => void;
}

export class FakeCaseService {
  schema: Obj = testSchema();
  readonly received: { method: string; params: Obj }[] = [];
  /** settings.set of these paths is refused with code 1003 and this message as its diagnostic. */
  readonly refusals = new Map<string, string>();
  /** What settings.validate reports. */
  diagnostics: { path: string; line: number | null; message: string }[] = [];
  readonly sessions = new Map<string, Session>();
  private log: { seq: number; session: string; kind: string; data: Json }[] = [];
  private waiters = new Set<Waiter>();
  private sockets = new Set<net.Socket>();
  private next = 1;

  private constructor(
    private readonly server: net.Server,
    readonly port: number,
  ) {}

  static async start(): Promise<FakeCaseService> {
    const server = net.createServer();
    await new Promise<void>((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', () => resolve());
    });
    const fake = new FakeCaseService(server, (server.address() as net.AddressInfo).port);
    server.on('connection', (s) => fake.accept(s));
    return fake;
  }

  get address(): string {
    return `127.0.0.1:${this.port}`;
  }

  /** The settings.set calls received, as [path, value]. */
  sets(): [string, Json][] {
    return this.received.filter((r) => r.method === 'settings.set').map((r) => [String(r.params['path']), r.params['value'] as Json]);
  }

  /** A session made outside the Studio, with its tree from the schema's defaults (and these values set). */
  makeSession(values: Record<string, Json> = {}): string {
    const id = `s${this.next++}`;
    const tree = defaults(this.schema['properties'] ? this.schema : {}) as Obj;
    for (const [p, v] of Object.entries(values)) setAt(tree, p, v);
    this.sessions.set(id, { state: 'defined', tree });
    this.emit(id, 'created', {});
    return id;
  }

  /** A change made by another writer (Claude Code, the CLI): the tree changes and setting_changed is emitted. */
  externalSet(session: string, path: string, value: Json): void {
    setAt(this.sessions.get(session)!.tree, path, value);
    this.emit(session, 'setting_changed', { path, live: false });
  }

  setState(session: string, state: string): void {
    this.sessions.get(session)!.state = state;
    this.emit(session, state === 'running' ? 'started' : 'stopped', {});
  }

  emit(session: string, kind: string, data: Json): void {
    this.log.push({ seq: this.log.length + 1, session, kind, data });
    for (const w of [...this.waiters]) w.done();
  }

  async stop(): Promise<void> {
    for (const w of [...this.waiters]) w.done();
    for (const s of this.sockets) s.destroy();
    await new Promise<void>((resolve) => this.server.close(() => resolve()));
  }

  private accept(s: net.Socket): void {
    this.sockets.add(s);
    s.on('close', () => this.sockets.delete(s));
    s.on('error', () => undefined);
    let buf = '';
    s.on('data', (chunk) => {
      buf += chunk.toString('utf8');
      for (let nl = buf.indexOf('\n'); nl >= 0; nl = buf.indexOf('\n')) {
        const line = buf.slice(0, nl);
        buf = buf.slice(nl + 1);
        if (line.trim()) this.handle(s, line);
      }
    });
  }

  private reply(s: net.Socket, id: Json, body: { result?: Json; error?: Json }): void {
    if (!s.destroyed) s.write(JSON.stringify({ jsonrpc: '2.0', id, ...body }) + '\n');
  }

  private handle(s: net.Socket, line: string): void {
    const req = JSON.parse(line) as { id: Json; method: string; params?: Obj };
    const p = req.params ?? {};
    const ok = (result: Json) => this.reply(s, req.id, { result });
    const fail = (code: number, message: string, data?: Json) => this.reply(s, req.id, { error: { code, message, ...(data !== undefined ? { data } : {}) } });
    if (req.method !== 'events.poll') this.received.push({ method: req.method, params: p });
    const sess = typeof p['session'] === 'string' ? this.sessions.get(p['session']) : undefined;
    const needSession = () => {
      if (!sess) fail(1001, `unknown session '${String(p['session'])}'`);
      return sess;
    };
    switch (req.method) {
      case 'service.info':
        return ok({ name: 'fake-case', version: '0.0.0', protocol: 'rcfd-service/1', protocol_version: '1.1', methods: ['service.info', 'settings.schema'] });
      case 'settings.schema':
        return ok({ case: this.schema, monitors: { type: 'object', properties: {}, required: [], additionalProperties: false } });
      case 'session.create': {
        const id = this.makeSession();
        return ok({ session: id });
      }
      case 'session.close': {
        // F-829 (undo of a new session, Close session): the session and its tree are gone
        if (!needSession()) return;
        this.sessions.delete(String(p['session']));
        this.emit(String(p['session']), 'closed', {});
        return ok({ closed: String(p['session']) });
      }
      case 'session.list':
        return ok({ sessions: [...this.sessions].map(([session, x]) => ({ session, state: x.state })) });
      case 'run.status': {
        const x = needSession();
        return x ? ok({ session: String(p['session']), state: x.state, iteration: 0 }) : undefined;
      }
      case 'settings.get': {
        const x = needSession();
        if (!x) return;
        const path = typeof p['path'] === 'string' && p['path'] !== '' ? p['path'] : null;
        return ok({ value: path ? (getAt(x.tree, path) ?? null) : x.tree });
      }
      case 'settings.set': {
        const x = needSession();
        if (!x) return;
        const path = String(p['path']);
        if (x.state === 'running' || x.state === 'paused') return fail(1002, 'stop the run before changing settings');
        const refusal = this.refusals.get(path);
        if (refusal !== undefined) return fail(1003, `validation failed: ${path}: ${refusal}`, { diagnostics: [{ path, line: null, message: refusal }] });
        setAt(x.tree, path, p['value'] as Json);
        this.emit(String(p['session']), 'setting_changed', { path, live: false });
        return ok({ path, value: p['value'] as Json });
      }
      case 'settings.remove': {
        const x = needSession();
        if (!x) return;
        const path = String(p['path']);
        const i = path.lastIndexOf('.');
        const parent = getAt(x.tree, path.slice(0, i));
        if (!isObj(parent) || !(path.slice(i + 1) in parent)) return fail(1003, 'validation failed', { diagnostics: [{ path, line: null, message: `no entry ${path}` }] });
        delete parent[path.slice(i + 1)];
        this.emit(String(p['session']), 'setting_changed', { path });
        return ok({ removed: path });
      }
      case 'settings.validate': {
        if (!needSession()) return;
        return ok({ ok: this.diagnostics.length === 0, diagnostics: this.diagnostics });
      }
      case 'events.poll': {
        const after = Number(p['after'] ?? 0);
        const timeout = Math.min(2000, Number(p['timeout_ms'] ?? 0));
        const answer = () => {
          const events = this.log.filter((e) => e.seq > after);
          ok({ events, next: events.length ? events[events.length - 1]!.seq : Math.max(after, this.log.length) });
        };
        if (this.log.some((e) => e.seq > after) || timeout === 0) return answer();
        const w: Waiter = {
          after,
          done: () => {
            clearTimeout(t);
            this.waiters.delete(w);
            answer();
          },
        };
        const t = setTimeout(() => w.done(), timeout);
        this.waiters.add(w);
        return;
      }
      default:
        return fail(-32601, `unknown method '${req.method}'`);
    }
  }
}
