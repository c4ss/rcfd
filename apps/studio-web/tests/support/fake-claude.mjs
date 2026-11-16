#!/usr/bin/env node
// A fake `claude` for the Claude Code panel's tests (F-814). It never talks to a model or the network, and the tests
// never start a real claude (RCFD_STUDIO_CLAUDE names this script; the Studio runs it on its own runtime).
//
// Every start appends one JSON line {argv, cwd, pid, stdin} to $FAKE_CLAUDE_LOG (when set), so a test can read the exact
// argument list the Studio passed.
//
//   claude agents --json   prints the rows of $FAKE_CLAUDE_AGENTS (a JSON array file; none: []) and one row for each
//                          running `-p` of this fake (as the real CLI lists a running `claude -p`).
//                          $FAKE_CLAUDE_AGENTS_FAIL=1: exits 2 with a message, as a broken listing would.
//   claude -p ...          reads one stream-json user line from stdin, then replays the canned stream of
//                          $FAKE_CLAUDE_STREAM (default claude/reply.stream.jsonl beside this file), one line every
//                          $FAKE_CLAUDE_DELAY_MS (default 5), with {{SESSION}}, {{MODE}} and {{CWD}} replaced; waits
//                          $FAKE_CLAUDE_HOLD_MS (default 0) before the last line. {{MODE}} is the --permission-mode
//                          passed, or $FAKE_CLAUDE_MODE when set (to test the abort on a mode other than dontAsk).
//                          With CLAUDE_CONFIG_DIR set, it appends the turn (the user line and the reply text) to the
//                          session's transcript there, as the real CLI does; without it nothing is written.
import { appendFileSync, mkdirSync, readFileSync, rmSync, writeFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const env = process.env;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const running = env.FAKE_CLAUDE_RUNNING || '';

async function readStdin() {
  if (!argv.includes('-p')) return '';
  let s = '';
  process.stdin.setEncoding('utf8');
  for await (const chunk of process.stdin) s += chunk;
  return s;
}

const stdin = await readStdin();
if (env.FAKE_CLAUDE_LOG) appendFileSync(env.FAKE_CLAUDE_LOG, JSON.stringify({ argv, cwd: process.cwd(), pid: process.pid, stdin }) + '\n');

if (argv[0] === 'agents') {
  if (env.FAKE_CLAUDE_AGENTS_FAIL === '1') {
    process.stderr.write('agents: listing failed (fake)\n');
    process.exit(2);
  }
  const rows = env.FAKE_CLAUDE_AGENTS ? JSON.parse(readFileSync(env.FAKE_CLAUDE_AGENTS, 'utf8')) : [];
  if (running) {
    try {
      for (const f of readdirSync(running)) rows.push(JSON.parse(readFileSync(path.join(running, f), 'utf8')));
    } catch {
      // none running
    }
  }
  process.stdout.write(JSON.stringify(rows));
  process.exit(0);
}

if (!argv.includes('-p')) {
  process.stderr.write(`fake claude: unknown use ${JSON.stringify(argv)}\n`);
  process.exit(64);
}

const after = (flag) => {
  const i = argv.indexOf(flag);
  return i >= 0 ? argv[i + 1] : undefined;
};
const session = after('--resume') ?? after('--session-id') ?? '00000000-0000-4000-8000-000000000999';
const mode = env.FAKE_CLAUDE_MODE ?? after('--permission-mode') ?? 'default';
let prompt = '';
try {
  prompt = JSON.parse(stdin.split('\n')[0]).message.content;
} catch {
  prompt = '';
}

let me = '';
if (running) {
  mkdirSync(running, { recursive: true });
  me = path.join(running, `${process.pid}.json`);
  writeFileSync(me, JSON.stringify({ pid: process.pid, cwd: process.cwd(), kind: 'interactive', sessionId: session, name: 'fake-p', status: 'busy' }));
}
const cleanup = () => {
  if (me) rmSync(me, { force: true });
};
process.on('exit', cleanup);

const streamFile = env.FAKE_CLAUDE_STREAM || path.join(here, 'claude', 'reply.stream.jsonl');
const json = (s) => JSON.stringify(s).slice(1, -1);
const lines = readFileSync(streamFile, 'utf8')
  .split('\n')
  .filter((l) => l.trim() !== '')
  .map((l) => l.replaceAll('{{SESSION}}', session).replaceAll('{{MODE}}', mode).replaceAll('{{CWD}}', json(process.cwd())));
const delay = Number(env.FAKE_CLAUDE_DELAY_MS ?? 5);
const hold = Number(env.FAKE_CLAUDE_HOLD_MS ?? 0);
let reply = '';
for (const [i, line] of lines.entries()) {
  if (i === lines.length - 1 && hold > 0) await sleep(hold);
  process.stdout.write(line + '\n');
  try {
    const v = JSON.parse(line);
    if (v.type === 'stream_event' && v.event?.delta?.type === 'text_delta') reply += v.event.delta.text;
  } catch {
    // a malformed line is replayed as it is
  }
  await sleep(delay);
}

if (env.CLAUDE_CONFIG_DIR) {
  const dir = path.join(env.CLAUDE_CONFIG_DIR, 'projects', process.cwd().replace(/[^A-Za-z0-9]/g, '-'));
  mkdirSync(dir, { recursive: true });
  const t = new Date().toISOString();
  const base = { cwd: process.cwd(), sessionId: session, timestamp: t };
  appendFileSync(
    path.join(dir, `${session}.jsonl`),
    [
      { type: 'queue-operation', operation: 'enqueue', sessionId: session, timestamp: t },
      { ...base, type: 'user', message: { role: 'user', content: prompt } },
      { ...base, type: 'assistant', message: { role: 'assistant', content: [{ type: 'text', text: reply }] } },
    ]
      .map((x) => JSON.stringify(x) + '\n')
      .join(''),
  );
}
cleanup();
process.exit(0);
