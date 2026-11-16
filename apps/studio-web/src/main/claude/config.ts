// Where the Claude Code panel's `claude` program, Claude Code's home and the panel's own per-user files come from, from
// the Studio's environment. Pure: the unit tests call it with plain objects and a fake file check.
//
//   RCFD_STUDIO_CLAUDE=<path of claude>     the Claude Code CLI. A path ending in .mjs, .cjs or .js is a Node script
//                                           (a wrapper, or the tests' fake claude); it runs on the Studio's own
//                                           runtime (ELECTRON_RUN_AS_NODE=1).
//   CLAUDE_CONFIG_DIR=<dir>                 Claude Code's home (else ~/.claude): its transcripts are under projects/.
//   RCFD_STUDIO_CLAUDE_STATE=<dir>          the panel's per-user files: the folder confirmations (claude-trust.json)
//                                           and the app-owned MCP configs (mcp/); default <user data>/claude.
//
// Nothing is looked for on PATH (F-814 decision D2, as F-805 D3 for rcfd-mcp): the Studio starts only the program it was
// told to. Where a packaged Studio finds claude is F-820's.
import path from 'node:path';
import type { ClaudeProgram } from '../../shared/claude.js';

export interface ClaudeConfig {
  readonly program: ClaudeProgram;
  /** What is spawned, and the arguments that come before claude's own (the script of a Node script program). */
  readonly command: string | null;
  readonly prefix: readonly string[];
  /** Added to the child's environment. */
  readonly env: Readonly<Record<string, string>>;
  /** Claude Code's home; null when neither CLAUDE_CONFIG_DIR nor a home folder is known. */
  readonly home: string | null;
  /** The panel's per-user folder (never inside a project). */
  readonly stateDir: string;
}

const EXE = process.platform === 'win32' ? 'claude.exe' : 'claude';

export function claudeConfig(
  env: Readonly<Record<string, string | undefined>>,
  userData: string,
  isFile: (p: string) => boolean,
  nodeRuntime: string,
): ClaudeConfig {
  const user = (process.platform === 'win32' ? env['USERPROFILE'] : env['HOME'])?.trim() ?? '';
  const home = env['CLAUDE_CONFIG_DIR']?.trim() || (user ? path.join(user, '.claude') : null);
  const stateDir = env['RCFD_STUDIO_CLAUDE_STATE']?.trim() || path.join(userData, 'claude');
  const base = { home, stateDir, env: {} as Record<string, string> };
  const p = env['RCFD_STUDIO_CLAUDE']?.trim() ?? '';
  if (!p) {
    return {
      ...base,
      program: {
        configured: false,
        reason: `The Studio was not told where Claude Code is. Set RCFD_STUDIO_CLAUDE to the path of ${EXE} (the Claude Code CLI, code.claude.com/docs) and start the Studio again. The Studio does not look for it on PATH. Until then the project's sessions are listed from their transcripts and shown read-only.`,
      },
      command: null,
      prefix: [],
    };
  }
  if (!isFile(p)) {
    return { ...base, program: { configured: false, reason: `RCFD_STUDIO_CLAUDE names ${p}, which is not a file.` }, command: null, prefix: [] };
  }
  const program: ClaudeProgram = { configured: true, path: p, from: 'RCFD_STUDIO_CLAUDE' };
  if (/\.(mjs|cjs|js)$/i.test(p)) return { ...base, program, command: nodeRuntime, prefix: [p], env: { ELECTRON_RUN_AS_NODE: '1' } };
  return { ...base, program, command: p, prefix: [] };
}
