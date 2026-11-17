// Which service the Studio talks to, from its environment. Pure: the unit tests call it with plain objects.
//
//   RCFD_STUDIO_SERVICE=127.0.0.1:7878   attach to a service someone else started (loopback only, G10)
//   RCFD_STUDIO_RCFD=<path to rcfd>      start `rcfd serve --addr 127.0.0.1:0 --ui` and own it
//   RCFD_STUDIO_SERVICE_ROOT=<dir>       with RCFD_STUDIO_RCFD: passed as `--root` (the service's sandbox)
//   RCFD_STUDIO_STATE_DIR=<dir>          where a started service's address and log are kept (default: the user data dir)
//
// Neither RCFD_STUDIO_SERVICE nor RCFD_STUDIO_RCFD: an installed Studio starts the rcfd it ships with
// (`<resources>/bin`, F-820, ADR 0801 Addendum 19), and only when the first project is opened (`start: 'project'`), so
// its launch costs nothing more (P10); a service an earlier start left running is still found again at launch (G7).
// Without a shipped rcfd either: no service (state `off`). Never from PATH.

export type ServiceConfig =
  | { readonly mode: 'off' }
  | { readonly mode: 'attach'; readonly host: string; readonly port: number }
  | {
      readonly mode: 'spawn';
      readonly rcfd: string;
      readonly root: string | null;
      readonly stateDir: string;
      /** `launch`: started with the Studio (RCFD_STUDIO_RCFD); `project`: when the first project opens (the shipped rcfd). */
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

/** `bundledRcfd`: the rcfd an installed Studio ships with, when it is there (src/main/project/config.ts bundledBin). */
export function serviceConfig(
  env: Readonly<Record<string, string | undefined>>,
  defaultStateDir: string,
  bundledRcfd: string | null = null,
): ServiceConfig {
  const attach = env['RCFD_STUDIO_SERVICE']?.trim() ?? '';
  const rcfd = env['RCFD_STUDIO_RCFD']?.trim() ?? '';
  if (attach && rcfd) {
    return { mode: 'invalid', reason: 'RCFD_STUDIO_SERVICE and RCFD_STUDIO_RCFD are both set: give one of them' };
  }
  if (attach) {
    const a = parseLoopbackAddress(attach);
    if ('error' in a) return { mode: 'invalid', reason: `RCFD_STUDIO_SERVICE: ${a.error}` };
    return { mode: 'attach', host: a.host, port: a.port };
  }
  if (rcfd) {
    const root = env['RCFD_STUDIO_SERVICE_ROOT']?.trim() || null;
    const stateDir = env['RCFD_STUDIO_STATE_DIR']?.trim() || defaultStateDir;
    return { mode: 'spawn', rcfd, root, stateDir, start: 'launch' };
  }
  if (bundledRcfd) {
    const root = env['RCFD_STUDIO_SERVICE_ROOT']?.trim() || null;
    const stateDir = env['RCFD_STUDIO_STATE_DIR']?.trim() || defaultStateDir;
    return { mode: 'spawn', rcfd: bundledRcfd, root, stateDir, start: 'project' };
  }
  return { mode: 'off' };
}

/** Delay before reconnection attempt `attempt` (0 = the first retry): 100 ms, doubling, at most 5 s. */
export function backoffMs(attempt: number): number {
  return Math.min(5000, 100 * 2 ** Math.max(0, Math.min(attempt, 16)));
}
