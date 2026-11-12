// @ts-check
'use strict';
// Clients of the rcfd solver service for the spike of ADR 0350 (F-447): the JSON-RPC line protocol
// (docs/service-protocol.md) and the data connection of `rcfd-data/1` (route R1).

const net = require('node:net');

/** One connection of newline-delimited JSON-RPC 2.0; requests are answered in order. */
class RpcClient {
  /** @param {number} port */
  constructor(port) {
    this.port = port;
    /** @type {net.Socket | null} */
    this.socket = null;
    /** @type {Buffer[]} */
    this.parts = [];
    /** @type {{resolve: (line: string) => void, reject: (e: Error) => void}[]} */
    this.waiting = [];
    this.nextId = 1;
  }

  /** @returns {Promise<void>} */
  connect() {
    return new Promise((resolve, reject) => {
      const s = net.connect({ host: '127.0.0.1', port: this.port }, () => resolve());
      s.setNoDelay(true);
      s.on('error', (e) => {
        reject(e);
        for (const w of this.waiting.splice(0)) w.reject(e);
      });
      s.on('data', (chunk) => this.onData(chunk));
      this.socket = s;
    });
  }

  /** @param {Buffer} chunk */
  onData(chunk) {
    let start = 0;
    for (;;) {
      const nl = chunk.indexOf(10, start);
      if (nl < 0) break;
      this.parts.push(chunk.subarray(start, nl));
      const line = this.parts.length === 1 ? this.parts[0].toString('utf8') : Buffer.concat(this.parts).toString('utf8');
      this.parts = [];
      start = nl + 1;
      const w = this.waiting.shift();
      if (w) w.resolve(line);
    }
    if (start < chunk.length) this.parts.push(chunk.subarray(start));
  }

  /**
   * The reply line of one request, unparsed.
   * @param {string} method
   * @param {unknown} [params]
   * @returns {Promise<string>}
   */
  raw(method, params) {
    const id = this.nextId++;
    const text = JSON.stringify({ jsonrpc: '2.0', id, method, params: params ?? {} }) + '\n';
    return new Promise((resolve, reject) => {
      this.waiting.push({ resolve, reject });
      /** @type {net.Socket} */ (this.socket).write(text);
    });
  }

  /**
   * @param {string} method
   * @param {unknown} [params]
   * @returns {Promise<any>}
   */
  async call(method, params) {
    const reply = JSON.parse(await this.raw(method, params));
    if (reply.error) throw new Error(`${method}: ${reply.error.code} ${reply.error.message}`);
    return reply.result;
  }

  close() {
    if (this.socket) this.socket.destroy();
  }
}

/** The data connection: a ticket in, the sections' bytes out (each `u64` length, then the bytes). */
class DataClient {
  /** @param {number} port */
  constructor(port) {
    this.port = port;
    /** @type {net.Socket | null} */
    this.socket = null;
    /** @type {{sections: number, out: ArrayBuffer[], resolve: (b: ArrayBuffer[]) => void, reject: (e: Error) => void}[]} */
    this.jobs = [];
    this.head = Buffer.alloc(8);
    this.headFill = 0;
    /** @type {Uint8Array | null} */
    this.body = null;
    this.bodyFill = 0;
    /** Test hook of mutation M2 (never set by the bench). */
    this.dropTail = 0;
  }

  /** @returns {Promise<void>} */
  connect() {
    return new Promise((resolve, reject) => {
      const s = net.connect({ host: '127.0.0.1', port: this.port }, () => resolve());
      s.setNoDelay(true);
      s.on('error', (e) => {
        reject(e);
        for (const j of this.jobs.splice(0)) j.reject(e);
      });
      s.on('data', (chunk) => this.onData(chunk));
      this.socket = s;
    });
  }

  /** @param {Buffer} chunk */
  onData(chunk) {
    let pos = 0;
    while (pos < chunk.length) {
      const job = this.jobs[0];
      if (!job) return;
      if (!this.body) {
        const n = Math.min(8 - this.headFill, chunk.length - pos);
        chunk.copy(this.head, this.headFill, pos, pos + n);
        this.headFill += n;
        pos += n;
        if (this.headFill < 8) return;
        this.headFill = 0;
        const lo = this.head.readUInt32LE(0);
        const hi = this.head.readUInt32LE(4);
        if (lo === 0xffffffff && hi === 0xffffffff) {
          this.jobs.shift();
          job.reject(new Error('unknown data ticket'));
          return;
        }
        this.body = new Uint8Array(hi * 4294967296 + lo);
        this.bodyFill = 0;
      }
      const body = this.body;
      const n = Math.min(body.length - this.bodyFill, chunk.length - pos);
      body.set(chunk.subarray(pos, pos + n), this.bodyFill);
      this.bodyFill += n;
      pos += n;
      if (this.bodyFill === body.length) {
        job.out.push(/** @type {ArrayBuffer} */ (body.buffer));
        this.body = null;
        if (job.out.length === job.sections) {
          this.jobs.shift();
          job.resolve(job.out);
        }
      }
    }
  }

  /**
   * @param {number} ticket
   * @param {number} sections
   * @returns {Promise<ArrayBuffer[]>}
   */
  redeem(ticket, sections) {
    const t = Buffer.alloc(8);
    t.writeUInt32LE(ticket % 4294967296, 0);
    t.writeUInt32LE(Math.floor(ticket / 4294967296), 4);
    return new Promise((resolve, reject) => {
      this.jobs.push({ sections, out: [], resolve, reject });
      /** @type {net.Socket} */ (this.socket).write(t);
    });
  }

  close() {
    if (this.socket) this.socket.destroy();
  }
}

/**
 * Route R1: one field of one snapshot as `f32` bytes.
 * @param {RpcClient} rpc
 * @param {DataClient} data
 * @param {string} path
 * @param {number} snapshot
 * @param {string} field
 * @returns {Promise<ArrayBuffer>}
 */
async function r1Field(rpc, data, path, snapshot, field) {
  const r = await rpc.call('data.field', { path, snapshot, field });
  const [values] = await data.redeem(r.ticket, 1);
  return values;
}

/**
 * Route R1: the boundary surface.
 * @param {RpcClient} rpc
 * @param {DataClient} data
 * @param {string} path
 * @returns {Promise<{points: ArrayBuffer, triangles: ArrayBuffer, cells: ArrayBuffer}>}
 */
async function r1Surface(rpc, data, path) {
  const r = await rpc.call('data.surface', { path });
  const [points, triangles, cells] = await data.redeem(r.ticket, 3);
  return { points, triangles, cells };
}

module.exports = { RpcClient, DataClient, r1Field, r1Surface };
