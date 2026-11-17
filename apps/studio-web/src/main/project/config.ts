// Where the engine's project server and the recent list come from, from the Studio's environment. Pure: the unit tests
// call it with plain objects and a fake file check.
//
//   RCFD_STUDIO_MCP=<path of rcfd-mcp>   the engine's project server (rcfd-mcp --project <folder> --read-only)
//   RCFD_STUDIO_RCFD=<path of rcfd>      (F-803) when RCFD_STUDIO_MCP is not set: the rcfd-mcp beside that rcfd
//   RCFD_STUDIO_RECENT=<file>            where the recent list is kept (default: recent-projects.json in the user data)
//
// Neither variable, and the Studio is packaged (an installed copy): the engine shipped with it in <resources>/bin
// (F-820, ADR 0801 Addendum 19, Q-F820-2 answer (a)). The variables always win over it. Nothing is looked for on PATH:
// the Studio starts only what it was told to or what it ships with (F-805 decision D3, README "The project home").
import path from 'node:path';
import type { EngineStatus } from '../../shared/project.js';

export const MCP_EXE = process.platform === 'win32' ? 'rcfd-mcp.exe' : 'rcfd-mcp';
export const RCFD_EXE = process.platform === 'win32' ? 'rcfd.exe' : 'rcfd';

/**
 * F-820: the folder of the engine an installed Studio ships with, `<resources>/bin` (rcfd, rcfd-mcp, rcfd-runner, put
 * there by scripts/package-installers.mjs), or null when the Studio is not packaged (`electron .`). out/packaged
 * (scripts/package-dir.mjs) has no bin folder, so a test build finds nothing there and is told by the variables, as
 * before. Test runs only (RCFD_TEST_HEADLESS=1): RCFD_TEST_BUNDLED_BIN stands in for that folder, so the e2e tests can
 * check the installed behaviour without an installer.
 */
export function bundledBin(env: Readonly<Record<string, string | undefined>>, packaged: boolean, resourcesPath: string): string | null {
  const test = env['RCFD_TEST_HEADLESS'] === '1' ? (env['RCFD_TEST_BUNDLED_BIN']?.trim() ?? '') : '';
  if (test) return test;
  return packaged && resourcesPath ? path.join(resourcesPath, 'bin') : null;
}

const HOW = 'build it with: cargo build --release -p rcfd-mcp';

export function engineConfig(
  env: Readonly<Record<string, string | undefined>>,
  isFile: (p: string) => boolean,
  exeName: string = MCP_EXE,
  bundled: string | null = null,
): EngineStatus {
  const mcp = env['RCFD_STUDIO_MCP']?.trim() ?? '';
  if (mcp) {
    if (!isFile(mcp)) return { configured: false, reason: `RCFD_STUDIO_MCP names ${mcp}, which is not a file (${HOW}).` };
    return { configured: true, path: mcp, from: 'RCFD_STUDIO_MCP' };
  }
  const rcfd = env['RCFD_STUDIO_RCFD']?.trim() ?? '';
  if (rcfd) {
    const beside = path.join(path.dirname(rcfd), exeName);
    if (isFile(beside)) return { configured: true, path: beside, from: 'RCFD_STUDIO_RCFD' };
    return {
      configured: false,
      reason: `RCFD_STUDIO_RCFD names ${rcfd}, but there is no ${exeName} beside it (${HOW}), and RCFD_STUDIO_MCP is not set.`,
    };
  }
  if (bundled) {
    const shipped = path.join(bundled, exeName);
    if (isFile(shipped)) return { configured: true, path: shipped, from: 'bundled' };
  }
  return {
    configured: false,
    reason: `The Studio was not told where the engine's project server is. Set RCFD_STUDIO_MCP to the path of ${exeName} (${HOW}) and start the Studio again.`,
  };
}

export function recentFile(env: Readonly<Record<string, string | undefined>>, userData: string): string {
  return env['RCFD_STUDIO_RECENT']?.trim() || path.join(userData, 'recent-projects.json');
}
