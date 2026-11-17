// Where the engine's project server and the recent list come from, from the Studio's environment. Pure: the unit tests
// call it with plain objects and a fake file check.
//
//   RCFD_STUDIO_MCP=<path of rcfd-mcp>   the engine's project server (rcfd-mcp --project <folder> --read-only)
//   RCFD_STUDIO_RCFD=<path of rcfd>      (F-803) when RCFD_STUDIO_MCP is not set: the rcfd-mcp beside that rcfd
//   RCFD_STUDIO_RECENT=<file>            where the recent list is kept (default: recent-projects.json in the user data)
//
// Nothing is looked for on PATH: the Studio starts only what it was told to (F-805 decision D3, README "The project home"). Where a packaged
// Studio finds its engine is F-820's.
import path from 'node:path';
import type { EngineStatus } from '../../shared/project.js';

export const MCP_EXE = process.platform === 'win32' ? 'rcfd-mcp.exe' : 'rcfd-mcp';

const HOW = 'build it with: cargo build --release -p rcfd-mcp';

export function engineConfig(
  env: Readonly<Record<string, string | undefined>>,
  isFile: (p: string) => boolean,
  exeName: string = MCP_EXE,
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
  return {
    configured: false,
    reason: `The Studio was not told where the engine's project server is. Set RCFD_STUDIO_MCP to the path of ${exeName} (${HOW}) and start the Studio again.`,
  };
}

export function recentFile(env: Readonly<Record<string, string | undefined>>, userData: string): string {
  return env['RCFD_STUDIO_RECENT']?.trim() || path.join(userData, 'recent-projects.json');
}
