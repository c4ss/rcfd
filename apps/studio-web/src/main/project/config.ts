// Where the engine's project server and the recent list come from. Pure: the unit tests call it with plain objects and
// a fake file check.
//
// The engine is chosen by src/main/engine/locate.ts (ADR 0801 Addendum 21, "Engine found without setup"):
// --no-engine, then --engine-dir <folder>, then the variables below, then the default folder (<resources>/bin when
// packaged, the checkout's target/release when run with `electron .`). A switch wins over a variable, a variable over
// the default folder. Nothing is looked for on PATH (F-805 decision D3).
//
//   RCFD_STUDIO_MCP=<path of rcfd-mcp>   the engine's project server (rcfd-mcp --project <folder> --read-only)
//   RCFD_STUDIO_RCFD=<path of rcfd>      (F-803) when RCFD_STUDIO_MCP is not set: the rcfd-mcp beside that rcfd
//   RCFD_STUDIO_RECENT=<file>            where the recent list is kept (default: recent-projects.json in the user data)
import path from 'node:path';
import type { EngineStatus } from '../../shared/project.js';
import { buildCommand, exe, missingReason, type EngineChoice } from '../engine/locate.js';

export const MCP_EXE = exe('rcfd-mcp');
export const RCFD_EXE = exe('rcfd');

const HOW = `build it with: ${buildCommand([{ name: 'rcfd-mcp', pkg: 'rcfd-mcp' }])}`;

export function engineConfig(
  env: Readonly<Record<string, string | undefined>>,
  isFile: (p: string) => boolean,
  choice: EngineChoice,
  exeName: string = MCP_EXE,
): EngineStatus {
  if (choice.from !== 'variables') {
    const reason = missingReason(choice);
    if (reason !== null || !('dir' in choice)) return { configured: false, reason: reason ?? 'No engine.' };
    return { configured: true, path: path.join(choice.dir, exeName), from: choice.from };
  }
  const mcp = env['RCFD_STUDIO_MCP']?.trim() ?? '';
  if (mcp) {
    if (!isFile(mcp)) return { configured: false, reason: `RCFD_STUDIO_MCP names ${mcp}, which is not a file (${HOW}).` };
    return { configured: true, path: mcp, from: 'RCFD_STUDIO_MCP' };
  }
  const rcfd = env['RCFD_STUDIO_RCFD']?.trim() ?? '';
  const beside = path.join(path.dirname(rcfd), exeName);
  if (rcfd && isFile(beside)) return { configured: true, path: beside, from: 'RCFD_STUDIO_RCFD' };
  return {
    configured: false,
    reason: `RCFD_STUDIO_RCFD names ${rcfd}, but there is no ${exeName} beside it (${HOW}), and RCFD_STUDIO_MCP is not set.`,
  };
}

export function recentFile(env: Readonly<Record<string, string | undefined>>, userData: string): string {
  return env['RCFD_STUDIO_RECENT']?.trim() || path.join(userData, 'recent-projects.json');
}
