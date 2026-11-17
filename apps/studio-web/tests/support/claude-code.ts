// Test support of "Use with Claude Code" and "Open Claude Code here" (ADR 0801 Addendum 23): the fake claude
// (./fake-claude-mcp.mjs) with its call log and registrations in a scratch folder, and the launch record the Studio
// writes in test mode instead of opening a terminal (src/main/claudecode/terminal.ts).
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { appRoot } from './rcfd.js';

export const FAKE_CLAUDE_MCP = path.join(appRoot, 'tests', 'support', 'fake-claude-mcp.mjs');

export interface ClaudeCodeFake {
  /** The Studio's variables: the fake claude, its log and state, and the launch record. */
  readonly env: Record<string, string>;
  /** Each call of the fake: its argv and working folder. */
  calls(): { argv: string[]; cwd: string }[];
  /** Each terminal the Studio would have opened: its argv and working folder. */
  launches(): { argv: string[]; cwd: string }[];
}

const lines = <T,>(file: string): T[] =>
  existsSync(file)
    ? readFileSync(file, 'utf8')
        .split('\n')
        .filter((l) => l.trim() !== '')
        .map((l) => JSON.parse(l) as T)
    : [];

/** The fake claude, keeping its files in `dir`. */
export function claudeCodeFake(dir: string, extra: Record<string, string> = {}): ClaudeCodeFake {
  const log = path.join(dir, 'fake-claude.jsonl');
  const state = path.join(dir, 'fake-claude-state.json');
  const record = path.join(dir, 'launches.jsonl');
  return {
    env: { RCFD_STUDIO_CLAUDE: FAKE_CLAUDE_MCP, FAKE_CLAUDE_LOG: log, FAKE_CLAUDE_STATE: state, RCFD_STUDIO_LAUNCH_RECORD: record, ...extra },
    calls: () => lines(log),
    launches: () => lines(record),
  };
}
