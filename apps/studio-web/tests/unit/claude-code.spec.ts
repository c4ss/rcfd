// ADR 0801 Addendum 23: Claude Code registered with Claude Code itself and opened in its own terminal. Where `claude` is
// found (src/main/engine/locate.ts, src/main/project/argv.ts), how `claude mcp get` is read and the registration made
// (src/main/claudecode/register.ts, against outputs of the real CLI 2.1.326 and against the fake claude), the terminal
// each platform opens (src/main/claudecode/terminal.ts, never opened here), the project port's `claudeCode` request,
// the activity line's words (src/renderer/project/activity.ts), and the journal actor kept from the change feed.
import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { claudeInstallerPath, locateClaude } from '../../src/main/engine/locate.js';
import { engineSwitches } from '../../src/main/project/argv.js';
import { addArgs, addCommand, ClaudeCode, parseGet, registrationOf, type RunResult } from '../../src/main/claudecode/register.js';
import { commandOf, launchPlan, launchTerminal } from '../../src/main/claudecode/terminal.js';
import { parseProjectRequest } from '../../src/main/project/bridge.js';
import { changeEntry } from '../../src/main/project/session.js';
import { activityOf, latestByClaude, registrationWords } from '../../src/renderer/project/activity.js';
import type { ChangeEntry, ClaudeCodeStatus } from '../../src/shared/project.js';
import { FAKE_CLAUDE_MCP } from '../support/claude-code.js';

const ENGINE = 'E:\\rcfd\\target\\release\\rcfd-mcp.exe';
const FOLDER = 'C:\\Users\\a b\\proj';

// What the real CLI printed (Claude Code 2.1.326, in a scratch folder, 2026-11-17)
const GET_NONE: RunResult = {
  code: 1,
  stdout: 'No MCP server named "rcfd-project". Configured servers: claude.ai Gmail, rea (and 5 more — run `claude mcp list` to see all)\n',
  stderr: '',
};
const getFound = (command: string, args: string): RunResult => ({
  code: 0,
  stdout: `rcfd-project:\n  Scope: Local config (private to you in this project)\n  Status: ✔ Connected\n  Type: stdio\n  Command: ${command}\n  Args: ${args}\n  Environment:\n\nTo remove this server, run: claude mcp remove rcfd-project -s local\n`,
  stderr: '',
});

test.describe('where claude is found', () => {
  const env = { USERPROFILE: 'C:\\Users\\u', HOME: '/home/u' };
  test('RCFD_STUDIO_CLAUDE, then --claude, then the installer’s folder; never PATH', () => {
    const inst = claudeInstallerPath(env, 'win32')!;
    expect(inst).toBe('C:\\Users\\u\\.local\\bin\\claude.exe');
    expect(claudeInstallerPath(env, 'linux')).toBe('/home/u/.local/bin/claude');
    const all = (p: string) => ['C:\\v\\claude.exe', 'C:\\s\\claude.exe', inst, 'C:\\path\\claude.exe'].includes(p);
    expect(locateClaude({ ...env, RCFD_STUDIO_CLAUDE: 'C:\\v\\claude.exe' }, 'C:\\s\\claude.exe', all, 'win32')).toEqual({ found: true, path: 'C:\\v\\claude.exe', from: 'RCFD_STUDIO_CLAUDE' });
    expect(locateClaude(env, 'C:\\s\\claude.exe', all, 'win32')).toEqual({ found: true, path: 'C:\\s\\claude.exe', from: '--claude' });
    expect(locateClaude({ ...env, PATH: 'C:\\path' }, null, all, 'win32')).toEqual({ found: true, path: inst, from: 'installer' });
    // a variable or switch that names no file is not passed over for the next place
    expect(locateClaude({ ...env, RCFD_STUDIO_CLAUDE: 'C:\\gone.exe' }, null, all, 'win32')).toMatchObject({ found: false, reason: expect.stringContaining('RCFD_STUDIO_CLAUDE names C:\\gone.exe') });
    expect(locateClaude(env, 'C:\\gone.exe', all, 'win32')).toMatchObject({ found: false, reason: expect.stringContaining('--claude names') });
    // only on PATH: not found, and said where the installer puts it
    const onPath = (p: string) => p === 'C:\\path\\claude.exe';
    const r = locateClaude({ ...env, PATH: 'C:\\path' }, null, onPath, 'win32');
    expect(r).toMatchObject({ found: false });
    expect(!r.found && r.reason).toContain('was not found where its installer puts it (C:\\Users\\u\\.local\\bin\\claude.exe)');
  });
  test('--claude <path> and --claude=<path> on the Studio’s command line', () => {
    const cwd = path.resolve('/w');
    expect(engineSwitches(['rcfd-studio', '--claude', 'bin/claude'], false, cwd).claude).toBe(path.resolve(cwd, 'bin/claude'));
    expect(engineSwitches(['electron', '.', '--claude=/x/claude'], true, cwd).claude).toBe(path.resolve(cwd, '/x/claude'));
    expect(engineSwitches(['rcfd-studio', '--claude'], false, cwd).problems).toEqual(['--claude needs a value']);
  });
});

test.describe('the registration', () => {
  test('the command shown and the arguments run', () => {
    expect(addCommand(ENGINE, FOLDER)).toBe('claude mcp add rcfd-project --scope local -- "E:\\rcfd\\target\\release\\rcfd-mcp.exe" --project "C:\\Users\\a b\\proj"');
    expect(addArgs(ENGINE, FOLDER)).toEqual(['mcp', 'add', 'rcfd-project', '--scope', 'local', '--', ENGINE, '--project', FOLDER]);
  });
  test('claude mcp get, read: none, this engine, another engine, another scope, an answer not understood', () => {
    expect(parseGet(GET_NONE)).toEqual({ found: false, scope: null, command: null, args: null });
    const here = parseGet(getFound(ENGINE, `--project ${FOLDER}`))!;
    expect(here).toMatchObject({ found: true, command: ENGINE, args: `--project ${FOLDER}` });
    expect(registrationOf(here, ENGINE, FOLDER, 'win32')).toBe('registered');
    // the letter case and a trailing separator of a Windows path do not matter
    expect(registrationOf(here, ENGINE.toLowerCase(), `${FOLDER}\\`, 'win32')).toBe('registered');
    expect(registrationOf(parseGet(getFound('D:\\old\\rcfd-mcp.exe', `--project ${FOLDER}`))!, ENGINE, FOLDER, 'win32')).toBe('other');
    expect(registrationOf(parseGet(getFound(ENGINE, `--project ${FOLDER} --read-only`))!, ENGINE, FOLDER, 'win32')).toBe('other');
    const user = parseGet({ ...getFound(ENGINE, `--project ${FOLDER}`), stdout: getFound(ENGINE, `--project ${FOLDER}`).stdout.replace('Local config (private to you in this project)', 'User config (available in all your projects)') })!;
    expect(registrationOf(user, ENGINE, FOLDER, 'win32')).toBe('not_registered');
    expect(parseGet({ code: 3, stdout: '', stderr: 'boom' })).toBeNull();
  });

  function harness(get: RunResult, opts: { found?: boolean } = {}) {
    const calls: { args: readonly string[]; cwd: string }[] = [];
    let current = get;
    const copied: string[] = [];
    const launched: [string, string][] = [];
    const cc = new ClaudeCode({
      claude: opts.found === false ? { found: false, reason: 'Claude Code was not found where its installer puts it (X).' } : { found: true, path: 'C:\\c\\claude.exe', from: 'installer' },
      engine: () => ENGINE,
      folder: () => FOLDER,
      run: async (_exe, args, cwd) => {
        calls.push({ args, cwd });
        if (args[1] === 'get') return current;
        if (args[1] === 'add') {
          current = getFound(args[6]!, `--project ${args[8]}`);
          return { code: 0, stdout: 'Added', stderr: '' };
        }
        current = GET_NONE;
        return { code: 0, stdout: 'Removed', stderr: '' };
      },
      copy: (t) => void copied.push(t),
      launch: async (c, f) => (launched.push([c, f]), { ok: true, message: 'opened' }),
      now: () => '2026-11-17T00:00:00Z',
    });
    return { cc, calls, copied, launched };
  }

  test('Register adds in the project folder; registered again is left alone; another engine is removed, then added', async () => {
    const a = harness(GET_NONE);
    expect(((await a.cc.do('status')) as ClaudeCodeStatus).registration).toBe('not_registered');
    const s = (await a.cc.do('register')) as ClaudeCodeStatus;
    expect(s.registration).toBe('registered');
    expect(a.calls.map((c) => c.args.slice(0, 2).join(' '))).toEqual(['mcp get', 'mcp get', 'mcp add', 'mcp get']);
    expect(a.calls.every((c) => c.cwd === FOLDER)).toBe(true);
    expect(a.calls[2]!.args).toEqual(addArgs(ENGINE, FOLDER));
    expect(((await a.cc.do('register')) as ClaudeCodeStatus).detail).toBe('Already registered for this folder.');

    const b = harness(getFound('D:\\old\\rcfd-mcp.exe', `--project ${FOLDER}`));
    const before = (await b.cc.do('status')) as ClaudeCodeStatus;
    expect(before.registration).toBe('other');
    expect(before.registeredAs).toBe(`D:\\old\\rcfd-mcp.exe --project ${FOLDER}`);
    expect(registrationWords(before)).toBe('Registered with a different engine path.');
    const after = (await b.cc.do('register')) as ClaudeCodeStatus;
    expect(after.registration).toBe('registered');
    expect(b.calls.map((c) => c.args.join(' '))).toContain('mcp remove rcfd-project --scope local');
  });

  test('without claude: the command to copy and the reason; nothing run; Copy puts the command on the clipboard', async () => {
    const h = harness(GET_NONE, { found: false });
    const s = (await h.cc.do('register')) as ClaudeCodeStatus;
    expect(s.registration).toBe('unknown');
    expect(s.command).toBe(addCommand(ENGINE, FOLDER));
    expect(s.detail).toContain('was not found where its installer puts it');
    expect(registrationWords(s)).toBe('Claude Code was not found.');
    expect(h.calls).toEqual([]);
    await h.cc.do('copy');
    expect(h.copied).toEqual([addCommand(ENGINE, FOLDER)]);
    expect(await h.cc.do('open')).toEqual({ ok: false, message: 'Claude Code was not found where its installer puts it (X).' });
    const g = harness(GET_NONE);
    expect(await g.cc.do('open')).toEqual({ ok: true, message: 'opened' });
    expect(g.launched).toEqual([['C:\\c\\claude.exe', FOLDER]]);
  });

  test('the fake claude answers like the real one, and keeps the registration by folder', () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'rcfd-claude-code-'));
    try {
      const env = { ...process.env, FAKE_CLAUDE_STATE: path.join(dir, 'state.json'), FAKE_CLAUDE_LOG: path.join(dir, 'log.jsonl') };
      const run = (args: string[]): RunResult => {
        try {
          return { code: 0, stdout: execFileSync(process.execPath, [FAKE_CLAUDE_MCP, ...args], { cwd: dir, env, encoding: 'utf8' }), stderr: '' };
        } catch (e) {
          const x = e as { status: number; stdout: string; stderr: string };
          return { code: x.status, stdout: x.stdout, stderr: x.stderr };
        }
      };
      expect(parseGet(run(['mcp', 'get', 'rcfd-project']))).toEqual({ found: false, scope: null, command: null, args: null });
      expect(run(addArgs(ENGINE, FOLDER)).code).toBe(0);
      expect(run(addArgs(ENGINE, FOLDER)).code).toBe(1);
      expect(registrationOf(parseGet(run(['mcp', 'get', 'rcfd-project']))!, ENGINE, FOLDER, 'win32')).toBe('registered');
      expect(run(['mcp', 'remove', 'rcfd-project', '--scope', 'local']).code).toBe(0);
      expect(readFileSync(path.join(dir, 'log.jsonl'), 'utf8').trim().split('\n')).toHaveLength(5);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

test.describe('Open Claude Code here', () => {
  test('Windows Terminal when installed, else a new command window; Linux x-terminal-emulator, else said', () => {
    const env = { LOCALAPPDATA: 'C:\\Users\\u\\AppData\\Local', SystemRoot: 'C:\\Windows' };
    const wt = 'C:\\Users\\u\\AppData\\Local\\Microsoft\\WindowsApps\\wt.exe';
    expect(launchPlan('C:\\c\\claude.exe', FOLDER, env, (p) => p === wt, 'win32')).toEqual({ argv: [wt, '-d', FOLDER, 'C:\\c\\claude.exe'], cwd: FOLDER, what: 'Windows Terminal' });
    expect(launchPlan('C:\\c\\claude.exe', FOLDER, env, () => false, 'win32')).toEqual({ argv: ['C:\\Windows\\System32\\cmd.exe', '/k', 'C:\\c\\claude.exe'], cwd: FOLDER, what: 'a new command window' });
    expect(launchPlan('/h/.local/bin/claude', '/p', {}, (p) => p === '/usr/bin/x-terminal-emulator', 'linux')).toEqual({ argv: ['/usr/bin/x-terminal-emulator', '-e', '/h/.local/bin/claude'], cwd: '/p', what: 'a terminal' });
    const none = launchPlan('/h/.local/bin/claude', '/p', {}, () => false, 'linux');
    expect(none.argv).toBeNull();
    expect(none.argv === null && none.message).toContain('No terminal found');
  });
  test('test mode records the launch and opens nothing', async () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'rcfd-claude-launch-'));
    try {
      const record = path.join(dir, 'launches.jsonl');
      const r = await launchTerminal('C:\\c\\claude.exe', FOLDER, { env: { SystemRoot: 'C:\\Windows' }, isFile: () => false, headless: true, record, platform: 'win32' });
      expect(r.ok).toBe(true);
      expect(JSON.parse(readFileSync(record, 'utf8'))).toEqual({ argv: ['C:\\Windows\\System32\\cmd.exe', '/k', 'C:\\c\\claude.exe'], cwd: FOLDER });
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
  test('a .mjs claude runs on the Studio’s own runtime', () => {
    expect(commandOf('C:\\t\\fake.mjs', ['mcp', 'get', 'x'], 'C:\\e\\electron.exe')).toEqual({ file: 'C:\\e\\electron.exe', args: ['C:\\t\\fake.mjs', 'mcp', 'get', 'x'], node: true });
    expect(commandOf('C:\\c\\claude.exe', ['mcp'], 'C:\\e\\electron.exe')).toEqual({ file: 'C:\\c\\claude.exe', args: ['mcp'], node: false });
  });
});

test.describe('the project port and the feed', () => {
  test('claudeCode takes exactly one of status, register, copy, open', () => {
    for (const what of ['status', 'register', 'copy', 'open'] as const) expect(parseProjectRequest({ op: 'claudeCode', id: 1, what })).toEqual({ ok: true, request: { op: 'claudeCode', id: 1, what } });
    expect(parseProjectRequest({ op: 'claudeCode', id: 1, what: 'run' })).toMatchObject({ ok: false });
    expect(parseProjectRequest({ op: 'claudeCode', id: 1, what: 'status', path: 'x' })).toMatchObject({ ok: false });
  });

  const entry = (o: Record<string, unknown>): ChangeEntry => changeEntry({ n: 1, source: 'journal', kind: 'edit', text: 't', ...o }, '2026-11-17T10:00:00Z');
  test('a feed entry keeps its command and actor; the activity line takes the latest of client mcp, not the Studio’s', () => {
    const mcp = entry({ n: 3, who: 'start_run', kind: 'run_submitted', run: 'r-1', actor: { client: 'mcp', instance: { pid: 42, started: 'x' } } });
    expect(mcp.who).toBe('start_run');
    expect(mcp.actor).toEqual({ client: 'mcp', pid: 42 });
    const studio = entry({ n: 4, who: 'edit_case', path: 'cases/a.toml', actor: { client: 'studio', instance: { pid: 7, started: 'x' } } });
    const run = entry({ n: 5, source: 'run', kind: 'run_state', run: 'r-1' });
    expect(run.actor).toBeNull();
    expect(latestByClaude([run, studio, mcp])).toBe(mcp);
    expect(latestByClaude([run, studio])).toBeNull();
    expect(activityOf(mcp)).toEqual({ verb: 'started run', thing: 'r-1', target: { kind: 'run', run: 'r-1' } });
  });
  test('the words of each write tool', () => {
    const of = (who: string, o: Record<string, unknown> = {}) => activityOf(entry({ who, ...o }));
    expect(of('new_case', { path: 'cases/a.toml' })).toEqual({ verb: 'made', thing: 'cases/a.toml', target: { kind: 'case', path: 'cases/a.toml' } });
    expect(of('edit_case', { path: 'cases/a.toml' }).verb).toBe('changed');
    expect(of('write_note', { path: 'notes/n.md' })).toEqual({ verb: 'wrote', thing: 'notes/n.md', target: { kind: 'path', path: 'notes/n.md' } });
    expect(of('pause_run', { run: 'r-1' }).verb).toBe('paused run');
    expect(of('cancel_run', { run: 'r-1' }).verb).toBe('cancelled run');
    expect(of('trash_case', { path: 'cases/a.toml' })).toEqual({ verb: 'moved to the trash', thing: 'cases/a.toml', target: { kind: 'path', path: 'cases/a.toml' } });
    expect(of('restore_trash', { path: 'cases/a.toml' }).verb).toBe('restored');
    expect(of('set_runner_config', { path: '.rcfd/runner-config.json' })).toEqual({ verb: 'changed the runner’s budgets', thing: '', target: { kind: 'runs' } });
  });
});
