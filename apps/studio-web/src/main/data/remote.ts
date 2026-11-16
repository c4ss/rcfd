// Route R1 in the main process (F-804): the opt-in `data.*` methods of the connected service (`rcfd-data/1`, protocol
// 1.2; docs/service-protocol.md, Bulk data methods). The call goes on the service client's call connection; the bytes
// come over one more TCP connection to the service's data port: write the ticket as 8 bytes little-endian, read per
// section the byte length as u64 little-endian and then the values. Everything is asynchronous (G21).
//
// One request at a time: the service keeps at most 8 tickets and drops the oldest, so a ticket is redeemed before the
// next call. The data port is asked for (`data.info`) once per service instance; a restart of the service (a new
// ServiceState.instance) or a closed data connection starts over. The bytes are checked against the sections the call
// announced (name, dtype, count, byte length) before they are handed on.
import net from 'node:net';
import type { DataWireResult, FieldInfo, StoreInfo } from '../../shared/data.js';
import type { ServiceState } from '../../shared/service.js';
import { DataFailure, SerialQueue, type Job } from './queue.js';

/** The part of ServiceClient this route uses. */
export interface RemoteService {
  readonly state: Pick<ServiceState, 'address' | 'instance'>;
  call(method: string, params?: Record<string, unknown>, timeoutMs?: number): Promise<unknown>;
}

/** `u64::MAX`: the service's answer to an unknown ticket (it closes the connection after it). */
const UNKNOWN_TICKET = 0xffffffff;

interface Section {
  readonly name: string;
  readonly dtype: string;
  readonly count: number;
  readonly bytes: number;
}

/** host:port as ServiceState.address writes it (an IPv6 host is not bracketed there). */
export function splitAddress(a: string): { host: string; port: number } | null {
  const i = a.lastIndexOf(':');
  if (i <= 0) return null;
  const host = a.slice(0, i).replace(/^\[|\]$/g, '');
  const port = Number(a.slice(i + 1));
  return Number.isInteger(port) && port > 0 && port < 65536 ? { host, port } : null;
}

const toFailure = (e: unknown): DataFailure => {
  if (e instanceof DataFailure) return e;
  const x = e as { code?: unknown; message?: unknown; data?: unknown };
  const code = typeof x?.code === 'number' || typeof x?.code === 'string' ? (x.code as DataFailure['code']) : 'protocol';
  // the service client's own codes are a subset of the data codes ('disconnected', 'timeout', 'not_configured', 'protocol')
  const known = ['disconnected', 'timeout', 'not_configured', 'protocol', 'bad_request'];
  const c = typeof code === 'number' || known.includes(code) ? code : 'protocol';
  return new DataFailure(c, typeof x?.message === 'string' ? x.message : String(e), x?.data);
};

/** One TCP connection to the data port; one redemption at a time (the queue in RemoteData sees to it). */
export class DataConnection {
  private chunks: Buffer[] = [];
  private have = 0;
  private wanted: { n: number; resolve: (b: Buffer) => void; reject: (e: DataFailure) => void } | null = null;
  private closedWith: DataFailure | null = null;

  private constructor(private readonly socket: net.Socket) {
    socket.on('data', (d: Buffer) => {
      this.chunks.push(d);
      this.have += d.length;
      this.drain();
    });
    const end = (why: string) => {
      if (!this.closedWith) this.closedWith = new DataFailure('disconnected', `the service's data connection ${why}`);
      const w = this.wanted;
      this.wanted = null;
      w?.reject(this.closedWith);
    };
    socket.on('error', (e) => end(`failed: ${e.message}`));
    socket.on('close', () => end('closed'));
  }

  static connect(host: string, port: number, timeoutMs: number): Promise<DataConnection> {
    return new Promise((resolve, reject) => {
      const s = net.connect({ host, port });
      const timer = setTimeout(() => {
        s.destroy();
        reject(new DataFailure('disconnected', `no data connection to ${host}:${port} within ${timeoutMs} ms`));
      }, timeoutMs);
      s.once('connect', () => {
        clearTimeout(timer);
        s.setNoDelay(true);
        s.removeAllListeners('error');
        resolve(new DataConnection(s));
      });
      s.once('error', (e) => {
        clearTimeout(timer);
        reject(new DataFailure('disconnected', `no data connection to ${host}:${port}: ${e.message}`));
      });
    });
  }

  get open(): boolean {
    return this.closedWith === null && !this.socket.destroyed;
  }

  close(): void {
    this.closedWith ??= new DataFailure('disconnected', "the service's data connection was closed");
    this.socket.destroy();
  }

  /** Redeems `ticket` and reads the sections, checking each byte length against the announced one. */
  async redeem(ticket: number, sections: readonly Section[], timeoutMs: number): Promise<ArrayBuffer[]> {
    if (!this.open) throw this.closedWith ?? new DataFailure('disconnected', "the service's data connection is closed");
    // past the limit the connection is out of step: close it (the next request opens a new one)
    const timer = setTimeout(() => {
      this.closedWith = new DataFailure('timeout', `the data of ticket ${ticket} did not arrive within ${timeoutMs} ms`);
      this.close();
    }, timeoutMs);
    try {
      return await this.read(ticket, sections);
    } finally {
      clearTimeout(timer);
    }
  }

  private async read(ticket: number, sections: readonly Section[]): Promise<ArrayBuffer[]> {
    const t = Buffer.alloc(8);
    t.writeUInt32LE(ticket % 4294967296, 0);
    t.writeUInt32LE(Math.floor(ticket / 4294967296), 4);
    this.socket.write(t);
    const out: ArrayBuffer[] = [];
    for (const s of sections) {
      const len = await this.take(8);
      const lo = len.readUInt32LE(0);
      const hi = len.readUInt32LE(4);
      if (lo === UNKNOWN_TICKET && hi === UNKNOWN_TICKET) {
        this.close();
        throw new DataFailure('protocol', `the service no longer holds ticket ${ticket} (it keeps at most 8)`);
      }
      const n = hi * 4294967296 + lo;
      if (n !== s.bytes) {
        this.close();
        throw new DataFailure('protocol', `section '${s.name}' has ${n} bytes, the call announced ${s.bytes}`);
      }
      const body = await this.take(n);
      // a buffer of its own, starting at the data (the slab of a small Buffer is shared)
      const ab = new ArrayBuffer(n);
      new Uint8Array(ab).set(body);
      out.push(ab);
    }
    return out;
  }

  private take(n: number): Promise<Buffer> {
    if (this.closedWith) return Promise.reject(this.closedWith);
    return new Promise((resolve, reject) => {
      this.wanted = { n, resolve, reject };
      this.drain();
    });
  }

  private drain(): void {
    const w = this.wanted;
    if (!w || this.have < w.n) return;
    let buf: Buffer;
    if (this.chunks.length === 1 && this.chunks[0]!.length === w.n) {
      buf = this.chunks[0]!;
      this.chunks = [];
    } else {
      const all = this.chunks.length === 1 ? this.chunks[0]! : Buffer.concat(this.chunks, this.have);
      buf = all.subarray(0, w.n);
      const rest = all.subarray(w.n);
      this.chunks = rest.length > 0 ? [rest] : [];
    }
    this.have -= w.n;
    this.wanted = null;
    w.resolve(buf);
  }
}

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const isCount = (v: unknown): v is number => Number.isSafeInteger(v) && (v as number) >= 0;

/** The `{ticket, sections}` reply of `data.field` or `data.surface`, checked against the sections expected. */
function ticketReply(r: unknown, expect: readonly { name: string; dtype: string; width: number }[]): { ticket: number; sections: Section[] } {
  if (!isObj(r) || !isCount(r['ticket']) || !Array.isArray(r['sections'])) throw new DataFailure('protocol', 'the reply is not {ticket, sections}');
  const list = r['sections'] as unknown[];
  if (list.length !== expect.length) throw new DataFailure('protocol', `expected ${expect.length} sections, the service announced ${list.length}`);
  const sections = list.map((s, i): Section => {
    const e = expect[i]!;
    if (!isObj(s) || s['name'] !== e.name || s['dtype'] !== e.dtype || !isCount(s['count']) || !isCount(s['bytes'])) {
      throw new DataFailure('protocol', `section ${i} is not '${e.name}' of ${e.dtype}`);
    }
    if (s['bytes'] !== e.width * (s['count'] as number)) throw new DataFailure('protocol', `section '${e.name}' has a byte length that does not match its count`);
    return { name: e.name, dtype: e.dtype, count: s['count'] as number, bytes: s['bytes'] as number };
  });
  return { ticket: r['ticket'] as number, sections };
}

function storeInfo(r: unknown, path: string): StoreInfo {
  if (!isObj(r) || !isCount(r['n_cells']) || !isCount(r['n_points']) || typeof r['mesh_hash'] !== 'string' || !Array.isArray(r['snapshots']) || !Array.isArray(r['fields'])) {
    throw new DataFailure('protocol', 'the reply of data.open does not have its members');
  }
  const snapshots = (r['snapshots'] as unknown[]).map((s) => {
    if (!isObj(s) || !isCount(s['step']) || typeof s['time'] !== 'number') throw new DataFailure('protocol', 'a snapshot of data.open is not {step, time}');
    return { step: s['step'] as number, time: s['time'] as number };
  });
  const fields = (r['fields'] as unknown[]).map((f): FieldInfo => {
    if (!isObj(f) || typeof f['name'] !== 'string' || typeof f['unit'] !== 'string' || !isCount(f['n'])) throw new DataFailure('protocol', 'a field of data.open is not {name, unit, n, ...}');
    return {
      name: f['name'] as string,
      unit: f['unit'] as string,
      location: f['location'] as FieldInfo['location'],
      n: f['n'] as number,
      precision: f['precision'] as FieldInfo['precision'],
      codec: f['codec'] as FieldInfo['codec'],
    };
  });
  return { route: 'service', path, nCells: r['n_cells'] as number, nPoints: r['n_points'] as number, meshHash: r['mesh_hash'] as string, snapshots, fields, index: null };
}

export class RemoteData {
  private readonly queue = new SerialQueue();
  private conn: DataConnection | null = null;
  private connInstance = -1;
  private closed = false;

  constructor(
    private readonly service: RemoteService,
    private readonly connectTimeoutMs = 3000,
  ) {}

  open(path: string, timeoutMs: number): Job<DataWireResult> {
    return this.queue.push(async () => {
      const r = await this.call('data.open', { path }, timeoutMs);
      return { kind: 'open', info: storeInfo(r, path) };
    });
  }

  field(path: string, snapshot: number, field: string, timeoutMs: number): Job<DataWireResult> {
    return this.queue.push(async () => {
      const c = await this.connection(timeoutMs);
      const r = ticketReply(await this.call('data.field', { path, snapshot, field }, timeoutMs), [{ name: 'values', dtype: 'f32', width: 4 }]);
      const [values] = await c.redeem(r.ticket, r.sections, timeoutMs);
      return { kind: 'field', snapshot, field, values: values! };
    });
  }

  surface(path: string, timeoutMs: number): Job<DataWireResult> {
    return this.queue.push(async () => {
      const c = await this.connection(timeoutMs);
      const r = ticketReply(await this.call('data.surface', { path }, timeoutMs), [
        { name: 'points', dtype: 'f32', width: 4 },
        { name: 'triangles', dtype: 'u32', width: 4 },
        { name: 'cells', dtype: 'u32', width: 4 },
      ]);
      const [points, triangles, cells] = await c.redeem(r.ticket, r.sections, timeoutMs);
      return { kind: 'surface', points: points!, triangles: triangles!, cells: cells! };
    });
  }

  close(): void {
    this.closed = true;
    this.conn?.close();
    this.conn = null;
  }

  private async call(method: string, params: Record<string, unknown>, timeoutMs: number): Promise<unknown> {
    try {
      return await this.service.call(method, params, timeoutMs);
    } catch (e) {
      const f = toFailure(e);
      if (f.code === -32601) throw new DataFailure('no_data', `the service has no data.* methods (${f.message}); rcfd serve does not switch them on (ADR 0350 QA4)`);
      throw f;
    }
  }

  /** The data connection of the current service instance (asks `data.info` for the port the first time). */
  private async connection(timeoutMs: number): Promise<DataConnection> {
    if (this.closed) throw new DataFailure('disconnected', 'the data route is closed');
    const st = this.service.state;
    if (this.conn && this.conn.open && this.connInstance === st.instance) return this.conn;
    this.conn?.close();
    this.conn = null;
    const info = await this.call('data.info', {}, timeoutMs);
    if (!isObj(info) || info['protocol'] !== 'rcfd-data/1' || !isCount(info['port'])) throw new DataFailure('protocol', 'data.info is not rcfd-data/1');
    const a = st.address ? splitAddress(st.address) : null;
    if (!a) throw new DataFailure('disconnected', 'the service has no address');
    this.conn = await DataConnection.connect(a.host, info['port'] as number, this.connectTimeoutMs);
    this.connInstance = this.service.state.instance;
    return this.conn;
  }
}
