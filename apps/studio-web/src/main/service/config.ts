// Which service the Studio talks to. Pure: the unit tests call it with plain objects.
//
//   --service <host:port>                attach to a service someone else started (loopback only, G10); wins over all
//   RCFD_STUDIO_SERVICE=127.0.0.1:7878   the same, as a variable
//   RCFD_STUDIO_RCFD=<path to rcfd>      start `rcfd serve --addr 127.0.0.1:0 --ui` at launch and own it
//   RCFD_STUDIO_SERVICE_ROOT=<dir>       with a started service: passed as `--root` (the service's sandbox)
//   RCFD_STUDIO_STATE_DIR=<dir>          where a started service's address and log are kept (default: the user data dir)
//
// No attach: the engine chosen by src/main/engine/locate.ts (ADR 0801 Addendum 21) gives the rcfd to start.
// --engine-dir <folder> or the default folder (<resources>/bin when packaged, the checkout's target/release with
// `electron .`): that folder's rcfd, started only when the first project is opened (`start: 'project'`), so the launch
// costs nothing more (P10); a service an earlier start left running is still found again at launch (G7). The variables:
// RCFD_STUDIO_RCFD as above; RCFD_STUDIO_MCP alone: the rcfd beside that rcfd-mcp, started with the first project (one
// folder gives the whole engine; a named engine is never mixed with the default one). --no-engine, or a folder without
// the whole engine: no service (state `off`, with the reason). Never from PATH. The attach is where a service on another machine (cloud, cluster) will plug in later.

import path from 'node:path';
import { buildCommand, exe, missingReason, partPath, type EngineChoice } from '../engine/locate.js';

export type ServiceConfig =
  | { readonly mode: 'off'; readonly reason: string }
  | { readonly mode: 'attach'; readonly host: string; readonly port: number }
  | {
      readonly mode: 'spawn';
      readonly rcfd: string;
      readonly root: string | null;
      readonly stateDir: string;
      /** `launch`: started with the Studio (RCFD_STUDIO_RCFD); `project`: when the first project opens (an engine folder's rcfd). */
      readonly start: 'launch' | 'project';
    }
  | { readonly mode: 'invalid'; readonly reason: string };

const LOOPBACK = new Set(['127.0.0.1', 'localhost', '::1', '[::1]']);

/** Parses `host:port` (or `[::1]:port`); only loopback hosts are accepted. */
export function parseLoopbackAddress(text: string): { host: string; port: number } | { error: string } {
  const m = /^(\[[^\]]+\]|[^:]+):(\d{1,5})$/.exec(text.trim());
  if (!m) return { error: `'${text}' is not host:port` };
  const host = m[1]!;
  const port = Number(m[2]);
  if (!(port >= 1 && port <= 65535)) return { error: `port ${m[2]} is out of range` };
  if (!LOOPBACK.has(host.toLowerCase())) {
    return { error: `'${host}' is not a loopback address: the Studio only talks to a service on this machine` };
  }
  return { host: host === '[::1]' ? '::1' : host, port };
}

export function serviceConfig(
  env: Readonly<Record<string, string | undefined>>,
  defaultStateDir: string,
  choice: EngineChoice,
  attachSwitch: string | null = null,
  isFile: (p: string) => boolean = () => false,
): ServiceConfig {
  if (attachSwitch) {
    const a = parseLoopbackAddress(attachSwitch);
    if ('error' in a) return { mode: 'invalid', reason: `--service: ${a.error}` };
    return { mode: 'attach', host: a.host, port: a.port };
  }
  const attach = env['RCFD_STUDIO_SERVICE']?.trim() ?? '';
  const rcfd = choice.from === 'variables' ? (env['RCFD_STUDIO_RCFD']?.trim() ?? '') : '';
  if (attach && rcfd) {
    return { mode: 'invalid', reason: 'RCFD_STUDIO_SERVICE and RCFD_STUDIO_RCFD are both set: give one of them' };
  }
  if (attach) {
    const a = parseLoopbackAddress(attach);
    if ('error' in a) return { mode: 'invalid', reason: `RCFD_STUDIO_SERVICE: ${a.error}` };
    return { mode: 'attach', host: a.host, port: a.port };
  }
  const root = env['RCFD_STUDIO_SERVICE_ROOT']?.trim() || null;
  const stateDir = env['RCFD_STUDIO_STATE_DIR']?.trim() || defaultStateDir;
  if (choice.from === 'variables') {
    if (rcfd) return { mode: 'spawn', rcfd, root, stateDir, start: 'launch' };
    const mcp = env['RCFD_STUDIO_MCP']?.trim() ?? '';
    const beside = path.join(path.dirname(mcp), exe('rcfd'));
    if (isFile(beside)) return { mode: 'spawn', rcfd: beside, root, stateDir, start: 'project' };
    return {
      mode: 'off',
      reason: `RCFD_STUDIO_MCP names ${mcp}, but there is no ${exe('rcfd')} beside it (build it with: ${buildCommand([{ name: 'rcfd', pkg: 'rcfd-cli' }])}).`,
    };
  }
  const own = partPath(choice, 'rcfd');
  if (own) return { mode: 'spawn', rcfd: own, root, stateDir, start: 'project' };
  return { mode: 'off', reason: missingReason(choice) ?? 'No engine.' };
}

/** Delay before reconnection attempt `attempt` (0 = the first retry): 100 ms, doubling, at most 5 s. */
export function backoffMs(attempt: number): number {
  return Math.min(5000, 100 * 2 ** Math.max(0, Math.min(attempt, 16)));
}
