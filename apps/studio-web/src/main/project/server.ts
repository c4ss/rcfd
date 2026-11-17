// The engine's project server for one folder: `rcfd-mcp --project <folder> --read-only`, spoken to over its standard
// streams (MCP: newline-delimited JSON-RPC 2.0, docs/mcp-tools.md). The tools are the queries of the project workspace
// (crates/rcfd-workspace, ADR 0173): the same code Claude Code's project tools run, so the Studio and Claude Code see one
// folder the same way. `--read-only` makes the server refuse every tool that writes; the home only reads.
// F-807: run control starts a second server for the same folder without `--read-only` (`{ readOnly: false }`), and
// calls only the run-control tools on it (src/main/project/control.ts); the reading server stays read-only.
//
// No Electron import: the unit tests drive it with a stand-in executable.
import { spawn, type ChildProcess } from 'node:child_process';
import { RpcConnection, RpcError } from '../service/rpc.js';
import type { BudgetsWriteTool, ProjectTool } from '../../shared/project.js';
import type { RunControlTool } from '../../shared/actions/runs.js';

/** Time limits (ms). */
export const START_TIMEOUT_MS = 15_000;
export const CALL_TIMEOUT_MS = 30_000;
/** Bytes of the server's standard error kept for messages. */
const STDERR_KEEP = 4096;

/** The MCP protocol version the Studio asks for; rcfd-mcp answers with the version it was asked for. */
export const MCP_PROTOCOL_VERSION = '2025-03-26';

export type StartFailureKind = 'not_a_project' | 'engine';

export class StartFailure extends Error {
  constructor(
    readonly kind: StartFailureKind,
    message: string,
    readonly detail: string | null,
  ) {
    super(message);
  }
}

/**
 * How the server is started. `readOnly` (default true) passes `--read-only`; only the case editor (F-812,
 * src/main/case/server.ts) starts one without it, and calls the few tools of its own allow-list on it.
 */
export interface StartOptions {
  readonly readOnly?: boolean;
}

export interface ServerInfo {
  readonly name: string;
  readonly version: string;
}

export interface ProjectServerLike {
  readonly info: ServerInfo;
  readonly exited: boolean;
  call(tool: ProjectTool | RunControlTool | BudgetsWriteTool, args?: Record<string, unknown>): Promise<unknown>;
  /** Called once when the server process ends (with its exit code and the end of its standard error). */
  onExit(listener: (why: string) => void): void;
  close(): Promise<void>;
}

const tail = (s: string): string => s.trim().split(/\r?\n/).slice(-6).join('\n');

/** A tool's result, or its error as an RpcError with the engine's code (1004 refused path, 1202 not found, ...). */
export function toolResult(tool: string, result: unknown): unknown {
  const r = (result ?? {}) as { isError?: unknown; structuredContent?: unknown };
  if (r.isError === true) {
    const e = ((r.structuredContent as { error?: unknown } | undefined)?.error ?? {}) as { code?: unknown; message?: unknown; data?: unknown };
    throw new RpcError(typeof e.code === 'number' ? e.code : 'protocol', typeof e.message === 'string' ? e.message : `${tool} failed`, e.data);
  }
  if (r.structuredContent === undefined) throw new RpcError('protocol', `${tool}: a result without structuredContent`);
  return r.structuredContent;
}

export class ProjectServer implements ProjectServerLike {
  private stderr = '';
  private exitListeners: ((why: string) => void)[] = [];
  private exitWhy: string | null = null;

  private constructor(
    private readonly child: ChildProcess,
    private readonly rpc: RpcConnection,
    readonly info: ServerInfo,
  ) {}

  get exited(): boolean {
    return this.exitWhy !== null;
  }

  /**
   * Starts the server for `folder` and completes the MCP handshake. Rejects with a StartFailure: `not_a_project` when
   * the engine refuses the folder (no rcfd-project.json, or one it cannot read), `engine` for anything else.
   */
  static start(exe: string, folder: string, timeoutMs = START_TIMEOUT_MS, options: StartOptions = {}): Promise<ProjectServer> {
    return new Promise((resolve, reject) => {
      let child: ChildProcess;
      const args = options.readOnly === false ? ['--project', folder] : ['--project', folder, '--read-only'];
      try {
        child = spawn(exe, args, { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
      } catch (e) {
        reject(new StartFailure('engine', `The engine's project server ${exe} could not be started: ${String(e)}`, null));
        return;
      }
      let stderr = '';
      let settled = false;
      const fail = (f: StartFailure) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        rpc.close('start failed');
        if (child.exitCode === null) child.kill();
        reject(f);
      };
      child.stderr!.on('data', (d: Buffer) => {
        stderr = (stderr + d.toString('utf8')).slice(-STDERR_KEEP);
      });
      child.once('error', (e) => fail(new StartFailure('engine', `The engine's project server ${exe} could not be started: ${e.message}`, null)));
      child.once('exit', (code) => {
        // rcfd-mcp exits 2 with "error: --project <dir>: <dir> is not an rcfd project folder: ..." (crates/rcfd-mcp/src/main.rs)
        const text = tail(stderr);
        if (/is not an rcfd project folder/.test(text)) fail(new StartFailure('not_a_project', 'not an rcfd project', text));
        else fail(new StartFailure('engine', `The engine's project server ended at start (exit code ${code ?? 'none'}).`, text || null));
      });
      const rpc = new RpcConnection(child.stdout!, child.stdin!, { timeoutMs: CALL_TIMEOUT_MS }, () => undefined);
      const timer = setTimeout(
        () => fail(new StartFailure('engine', `The engine's project server did not answer within ${timeoutMs / 1000} s.`, tail(stderr) || null)),
        timeoutMs,
      );
      rpc
        .request('initialize', { protocolVersion: MCP_PROTOCOL_VERSION, capabilities: {}, clientInfo: { name: 'rcfd-studio', version: '0' } }, timeoutMs)
        .then((r) => {
          if (settled) return;
          const si = ((r ?? {}) as { serverInfo?: { name?: unknown; version?: unknown } }).serverInfo ?? {};
          const info = { name: typeof si.name === 'string' ? si.name : 'rcfd', version: typeof si.version === 'string' ? si.version : '?' };
          settled = true;
          clearTimeout(timer);
          child.removeAllListeners('exit');
          child.removeAllListeners('error');
          const server = new ProjectServer(child, rpc, info);
          server.stderr = stderr;
          child.stderr!.removeAllListeners('data');
          child.stderr!.on('data', (d: Buffer) => {
            server.stderr = (server.stderr + d.toString('utf8')).slice(-STDERR_KEEP);
          });
          child.once('exit', (code, signal) => server.ended(`exit code ${code ?? signal ?? 'none'}`));
          child.once('error', (e) => server.ended(e.message));
          // the MCP handshake's notification; rcfd-mcp answers no notification
          child.stdin!.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
          resolve(server);
        })
        .catch((e: unknown) => {
          // the exit handler names the cause when the process ended; this covers a reply that is not MCP
          if (child.exitCode === null && !(e instanceof RpcError && e.code === 'disconnected')) {
            fail(new StartFailure('engine', `The engine's project server did not complete the MCP handshake: ${e instanceof Error ? e.message : String(e)}`, null));
          }
        });
    });
  }

  private ended(why: string): void {
    if (this.exitWhy !== null) return;
    const err = tail(this.stderr);
    this.exitWhy = err ? `${why}: ${err}` : why;
    this.rpc.close(`the project server stopped (${this.exitWhy})`);
    for (const l of this.exitListeners.splice(0)) l(this.exitWhy);
  }

  onExit(listener: (why: string) => void): void {
    if (this.exitWhy !== null) listener(this.exitWhy);
    else this.exitListeners.push(listener);
  }

  async call(tool: ProjectTool | RunControlTool | BudgetsWriteTool, args: Record<string, unknown> = {}): Promise<unknown> {
    return this.callTool(tool, args);
  }

  /** Any tool by name; the caller's allow-list decides which (the case editor's, src/main/case/server.ts). */
  async callTool(tool: string, args: Record<string, unknown> = {}, timeoutMs?: number): Promise<unknown> {
    const r = await this.rpc.request('tools/call', { name: tool, arguments: args }, timeoutMs);
    return toolResult(tool, r);
  }

  /** The server's `tools/list`: each tool's name, description and input schema, as the engine declares them. */
  async listTools(): Promise<readonly unknown[]> {
    const r = (await this.rpc.request('tools/list', {})) as { tools?: unknown } | null;
    return Array.isArray(r?.tools) ? r.tools : [];
  }

  /** Ends the server: its input is closed (it exits at end of input), and it is killed if it is still there after 2 s. */
  close(): Promise<void> {
    if (this.exitWhy !== null) return Promise.resolve();
    return new Promise((resolve) => {
      const done = () => {
        clearTimeout(kill);
        resolve();
      };
      this.child.once('exit', done);
      const kill = setTimeout(() => {
        this.child.kill();
        setTimeout(resolve, 500);
      }, 2000);
      this.exitListeners = [];
      this.child.stdin!.end();
    });
  }
}
