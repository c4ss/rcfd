// One connection of rcfd-service/1 (docs/service-protocol.md): JSON-RPC 2.0, one JSON object per line, over TCP or
// over the standard streams of `rcfd serve --stdio`. Requests carry increasing integer ids and are matched to replies
// by id; each has a time limit. Everything is asynchronous: nothing here waits on the main process's thread.
//
// No Electron import: the unit tests run this file in Node against a fake service.
import net from 'node:net';
import type { Readable, Writable } from 'node:stream';
import type { ServiceError } from '../../shared/service.js';

/** Longest reply line accepted. A service that sends more without a newline is cut off as broken. */
export const MAX_LINE_BYTES = 512 * 1024 * 1024;

/** A failed request: a JSON-RPC error of the service, or one of the client's own codes. */
export class RpcError extends Error {
  readonly code: ServiceError['code'];
  readonly data: unknown;
  constructor(code: ServiceError['code'], message: string, data?: unknown) {
    super(message);
    this.name = 'RpcError';
    this.code = code;
    this.data = data;
  }
  toServiceError(): ServiceError {
    return this.data === undefined
      ? { code: this.code, message: this.message }
      : { code: this.code, message: this.message, data: this.data };
  }
}

/**
 * Splits a byte stream into lines at "\n". Bytes are kept as buffers until a newline arrives, so a UTF-8 character cut
 * between two chunks is decoded whole (the byte 0x0A never occurs inside a multi-byte character).
 */
export class LineSplitter {
  private parts: Buffer[] = [];
  private size = 0;
  constructor(
    private readonly onLine: (line: string) => void,
    private readonly maxBytes = MAX_LINE_BYTES,
  ) {}

  /** Feeds one chunk; returns false when the pending line grew past the limit (the caller closes the connection). */
  push(chunk: Buffer): boolean {
    let start = 0;
    for (;;) {
      const nl = chunk.indexOf(10, start);
      if (nl < 0) break;
      const piece = chunk.subarray(start, nl);
      let line: string;
      if (this.parts.length === 0) line = piece.toString('utf8');
      else {
        this.parts.push(piece);
        line = Buffer.concat(this.parts, this.size + piece.length).toString('utf8');
        this.parts = [];
        this.size = 0;
      }
      start = nl + 1;
      // a tolerant reader: a carriage return before the newline and blank lines are not replies
      const trimmed = line.endsWith('\r') ? line.slice(0, -1) : line;
      if (trimmed.trim() !== '') this.onLine(trimmed);
    }
    if (start < chunk.length) {
      const rest = chunk.subarray(start);
      this.parts.push(rest);
      this.size += rest.length;
      if (this.size > this.maxBytes) return false;
    }
    return true;
  }
}

interface Pending {
  readonly method: string;
  readonly resolve: (v: unknown) => void;
  readonly reject: (e: RpcError) => void;
  readonly timer: NodeJS.Timeout;
}

export interface RpcConnectionOptions {
  /** Default time limit of a request in milliseconds. */
  readonly timeoutMs?: number;
  /** Called once when the connection ends, for whatever reason (the pending requests are rejected first). */
  readonly onClose?: (reason: string) => void;
}

/**
 * A request/response connection over a readable and a writable stream. The service answers one connection's requests
 * in the order they were sent, but replies are matched by id all the same; a reply whose request already timed out is
 * dropped.
 */
export class RpcConnection {
  private nextId = 1;
  private readonly pending = new Map<number, Pending>();
  private closedReason: string | null = null;
  private readonly splitter: LineSplitter;
  private readonly defaultTimeoutMs: number;
  private readonly onCloseCb: ((reason: string) => void) | undefined;
  /** Lines that were not a reply to a pending request (late replies after a timeout, or noise). */
  stray = 0;

  constructor(
    input: Readable,
    private readonly output: Writable,
    options: RpcConnectionOptions = {},
    private readonly destroy: () => void = () => {
      input.destroy();
      output.destroy();
    },
  ) {
    this.defaultTimeoutMs = options.timeoutMs ?? 60_000;
    this.onCloseCb = options.onClose;
    this.splitter = new LineSplitter((line) => this.onLine(line));
    input.on('data', (chunk: Buffer) => {
      if (!this.splitter.push(chunk)) this.close(`a reply line longer than ${MAX_LINE_BYTES} bytes`);
    });
    input.on('end', () => this.close('the service closed the connection'));
    input.on('close', () => this.close('the service closed the connection'));
    input.on('error', (e: Error) => this.close(`connection error: ${e.message}`));
    output.on('error', (e: Error) => this.close(`connection error: ${e.message}`));
  }

  /** Connects to `host:port` over TCP (Nagle off: requests are whole lines, sent at once). */
  static connectTcp(host: string, port: number, options: RpcConnectionOptions & { connectTimeoutMs?: number } = {}): Promise<RpcConnection> {
    return new Promise((resolve, reject) => {
      const socket = net.connect({ host, port });
      socket.setNoDelay(true);
      const timer = setTimeout(() => {
        socket.destroy();
        reject(new RpcError('disconnected', `no connection to ${host}:${port} within ${options.connectTimeoutMs ?? 3000} ms`));
      }, options.connectTimeoutMs ?? 3000);
      const fail = (e: Error) => {
        clearTimeout(timer);
        reject(new RpcError('disconnected', `cannot connect to ${host}:${port}: ${e.message}`));
      };
      socket.once('error', fail);
      socket.once('connect', () => {
        clearTimeout(timer);
        socket.off('error', fail);
        resolve(new RpcConnection(socket, socket, options, () => socket.destroy()));
      });
    });
  }

  get closed(): boolean {
    return this.closedReason !== null;
  }

  get pendingCount(): number {
    return this.pending.size;
  }

  /** Sends one request and resolves with its `result`; rejects with an RpcError (service error, timeout, closed). */
  request(method: string, params: unknown = {}, timeoutMs = this.defaultTimeoutMs): Promise<unknown> {
    if (this.closedReason !== null) return Promise.reject(new RpcError('disconnected', this.closedReason));
    const id = this.nextId++;
    let text: string;
    try {
      text = JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n';
    } catch (e) {
      return Promise.reject(new RpcError('bad_request', `${method}: parameters are not JSON: ${String(e)}`));
    }
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        if (this.pending.delete(id)) reject(new RpcError('timeout', `${method}: no reply within ${timeoutMs} ms`));
      }, timeoutMs);
      this.pending.set(id, { method, resolve, reject, timer });
      this.output.write(text);
    });
  }

  /** Ends the connection; pending requests are rejected with `reason`. Idempotent. */
  close(reason = 'closed by the client'): void {
    if (this.closedReason !== null) return;
    this.closedReason = reason;
    for (const [id, p] of this.pending) {
      clearTimeout(p.timer);
      this.pending.delete(id);
      p.reject(new RpcError('disconnected', `${p.method}: ${reason}`));
    }
    this.destroy();
    this.onCloseCb?.(reason);
  }

  private onLine(line: string): void {
    let msg: unknown;
    try {
      msg = JSON.parse(line);
    } catch {
      this.close('the service sent a line that is not JSON');
      return;
    }
    if (typeof msg !== 'object' || msg === null || Array.isArray(msg)) {
      this.close('the service sent a reply that is not a JSON-RPC object');
      return;
    }
    const m = msg as { id?: unknown; result?: unknown; error?: unknown };
    if (typeof m.id !== 'number') {
      // A reply with id null answers a line the service could not read (-32700). This client only sends valid lines,
      // so it cannot be paired with a request; it is counted, not guessed at.
      this.stray++;
      return;
    }
    const p = this.pending.get(m.id);
    if (!p) {
      this.stray++;
      return;
    }
    this.pending.delete(m.id);
    clearTimeout(p.timer);
    if (m.error !== undefined) {
      const e = (typeof m.error === 'object' && m.error !== null ? m.error : {}) as { code?: unknown; message?: unknown; data?: unknown };
      const code = typeof e.code === 'number' ? e.code : 'protocol';
      const message = typeof e.message === 'string' ? e.message : 'error without a message';
      p.reject(new RpcError(code, message, e.data));
    } else if ('result' in m) {
      p.resolve(m.result);
    } else {
      p.reject(new RpcError('protocol', `${p.method}: a reply with neither result nor error`));
    }
  }
}
