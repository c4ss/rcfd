// Support for the Claude Code panel's tests (F-814): a temporary Claude Code home, the fake claude
// (tests/support/fake-claude.mjs) and what it recorded. No test starts a real claude: the program is always the fake,
// named by path, and CLAUDE_CONFIG_DIR always points at a temporary folder.
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { appRoot } from './rcfd.js';

export const FAKE_CLAUDE = path.join(appRoot, 'tests', 'support', 'fake-claude.mjs');

export interface FakeStart {
  readonly argv: string[];
  readonly cwd: string;
  readonly pid: number;
  readonly stdin: string;
}

export interface ClaudeFixture {
  readonly root: string;
  /** CLAUDE_CONFIG_DIR. */
  readonly home: string;
  /** RCFD_STUDIO_CLAUDE_STATE. */
  readonly state: string;
  readonly log: string;
  readonly agents: string;
  readonly running: string;
  /** The fake's environment (FAKE_CLAUDE_* and CLAUDE_CONFIG_DIR). */
  env(extra?: Record<string, string>): Record<string, string>;
  /** Every start of the fake, in order. */
  starts(): Promise<FakeStart[]>;
  /** The `-p` starts only (the chat processes). */
  chats(): Promise<FakeStart[]>;
  setAgents(rows: readonly { pid: number; cwd: string; sessionId: string; name?: string }[]): Promise<void>;
  /** Writes a transcript of `session` for `folder`, as Claude Code would. */
  transcript(folder: string, session: string, lines: readonly unknown[]): Promise<string>;
}

export const dirName = (cwd: string) => cwd.replace(/[^A-Za-z0-9]/g, '-');

export async function claudeFixture(tag: string): Promise<ClaudeFixture> {
  const root = await mkdtemp(path.join(tmpdir(), `rcfd-f814-${tag}-`));
  const home = path.join(root, 'claude-home');
  const state = path.join(root, 'studio-state');
  const log = path.join(root, 'fake-claude.log');
  const agents = path.join(root, 'agents.json');
  const running = path.join(root, 'running');
  await mkdir(home, { recursive: true });
  await writeFile(agents, '[]');
  const starts = async (): Promise<FakeStart[]> => {
    try {
      return (await readFile(log, 'utf8'))
        .split('\n')
        .filter((l) => l.trim() !== '')
        .map((l) => JSON.parse(l) as FakeStart);
    } catch {
      return [];
    }
  };
  return {
    root,
    home,
    state,
    log,
    agents,
    running,
    env: (extra = {}) => ({
      CLAUDE_CONFIG_DIR: home,
      FAKE_CLAUDE_LOG: log,
      FAKE_CLAUDE_AGENTS: agents,
      FAKE_CLAUDE_RUNNING: running,
      ...extra,
    }),
    starts,
    chats: async () => (await starts()).filter((s) => s.argv.includes('-p')),
    setAgents: (rows) =>
      writeFile(agents, JSON.stringify(rows.map((r) => ({ kind: 'interactive', status: 'busy', name: r.name ?? 'other', ...r })))),
    transcript: async (folder, session, lines) => {
      const dir = path.join(home, 'projects', dirName(folder));
      await mkdir(dir, { recursive: true });
      const file = path.join(dir, `${session}.jsonl`);
      await writeFile(file, lines.map((l) => JSON.stringify(l) + '\n').join(''));
      return file;
    },
  };
}

/** A transcript of two turns in `folder`, in the shapes observed on 2.1.310. */
export const sampleTranscript = (folder: string, session: string) => [
  { type: 'queue-operation', operation: 'enqueue', sessionId: session, timestamp: '2026-11-16T09:00:00.000Z' },
  { type: 'user', cwd: folder, sessionId: session, timestamp: '2026-11-16T09:00:01.000Z', message: { role: 'user', content: 'Which runs converged?' } },
  {
    type: 'assistant',
    cwd: folder,
    sessionId: session,
    timestamp: '2026-11-16T09:00:05.000Z',
    message: { role: 'assistant', content: [{ type: 'text', text: 'Run **r-20261028T152641-3900** converged; its case is cases/cavity-re100.toml.' }] },
  },
  { type: 'last-prompt', lastPrompt: 'Which runs converged?', sessionId: session },
];

/** Every forbidden permission token: a test asserts none is ever in a spawned argument list. */
export const APPROVE_EVERYTHING = [/dangerously/i, /bypasspermissions/i, /skip-permissions/i, /^--permission-mode=/i, /^acceptEdits$/, /^bypassPermissions$/, /^--settings/i, /^--add-dir/i, /^--permission-prompt-tool/i];
