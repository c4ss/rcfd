// ADR 0801 Addendum 23 (F-814 superseded): Claude Code is used in its own terminal, with the engine's project server
// registered with it, not embedded. In the packaged Studio, against the real engine:
//
// - Use with Claude Code: the panel says the folder is not registered, shows the exact command, and Register runs
//   `claude mcp add rcfd-project --scope local -- <rcfd-mcp> --project <folder>` in the folder (the fake claude records
//   each call; the real CLI's answers were checked by hand, see tests/unit/claude-code.spec.ts); a registration of
//   another engine reads "different engine path" and Re-register removes it first.
// - Open Claude Code here: in test mode the terminal is recorded, never opened.
// - Live update: the test is the MCP client Claude Code would be. It spawns `rcfd-mcp --project <folder>` itself and calls
//   new_case, edit_case, start_run and trash_case over stdio; the open Studio shows each change without a reload, and
//   the header's activity line names the latest one with a link. The Studio's own writes never show there.
//
// Needs rcfd-mcp and rcfd-runner (tests/support/project.ts, tests/support/runner.ts); without them the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import readline from 'node:readline';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { claudeCodeFake, type ClaudeCodeFake } from '../support/claude-code.js';
import { copyExample, findMcp, mcpPids, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { appRoot, killPid } from '../support/rcfd.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'claude-code');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.describe.configure({ mode: 'serial' });

let dir = '';
let quick = '';
let fake: ClaudeCodeFake;
let studio: Studio;
let page: Page;
let client: McpClient | null = null;
let run: string | null = null;

const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const norm = (p: string) => path.resolve(p).toLowerCase();

/** The project journal's lines, as the engine wrote them. */
const readJournal = (): { who?: string; actor?: { client?: string } }[] =>
  readFileSync(path.join(quick, '.rcfd', 'edits.jsonl'), 'utf8')
    .split('\n')
    .filter((l) => l.trim() !== '')
    .map((l) => JSON.parse(l) as { who?: string; actor?: { client?: string } });

/** Runs a palette command by its id from the keyboard; a command that reports its outcome is closed with Done. */
async function palette(id: string, reports = false): Promise<void> {
  await page.keyboard.press('Control+K');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  const search = dialog.getByRole('combobox', { name: 'Find a command by its name or its id' });
  await expect(search).toBeFocused();
  await page.keyboard.type(id);
  await expect(dialog.getByRole('option')).toHaveCount(1);
  await page.keyboard.press('Enter');
  if (reports) {
    await expect(dialog.getByRole('status')).toContainText('Opened Claude Code in');
    await dialog.getByRole('button', { name: 'Done' }).click();
  }
  await expect(dialog).toHaveCount(0);
}

/** A minimal MCP client over stdio: the part of Claude Code these tests stand in for. */
class McpClient {
  private n = 0;
  private readonly waiting = new Map<number, (m: Record<string, unknown>) => void>();
  private constructor(readonly child: ChildProcessWithoutNullStreams) {
    readline.createInterface({ input: child.stdout }).on('line', (l) => {
      const m = JSON.parse(l) as Record<string, unknown>;
      const w = this.waiting.get(m['id'] as number);
      if (w) {
        this.waiting.delete(m['id'] as number);
        w(m);
      }
    });
  }
  static async start(exe: string, folder: string): Promise<McpClient> {
    const c = new McpClient(spawn(exe, ['--project', folder], { cwd: folder, windowsHide: true }));
    await c.request('initialize', { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'claude-code-stand-in', version: '0' } });
    c.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' })}\n`);
    return c;
  }
  request(method: string, params: unknown): Promise<Record<string, unknown>> {
    const id = ++this.n;
    return new Promise((resolve, reject) => {
      const t = setTimeout(() => reject(new Error(`${method}: no answer in 60 s`)), 60_000);
      this.waiting.set(id, (m) => {
        clearTimeout(t);
        resolve(m);
      });
      this.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
    });
  }
  async call(name: string, args: Record<string, unknown>): Promise<Record<string, unknown>> {
    const m = await this.request('tools/call', { name, arguments: args });
    const r = m['result'] as { isError?: boolean; structuredContent?: Record<string, unknown>; content?: { text: string }[] } | undefined;
    if (!r || r.isError) throw new Error(`${name}: ${JSON.stringify(m).slice(0, 500)}`);
    return r.structuredContent ?? (JSON.parse(r.content?.[0]?.text ?? '{}') as Record<string, unknown>);
  }
  async hash(file: string): Promise<string> {
    return (await this.call('read_project_file', { path: file, limit: 1 }))['hash'] as string;
  }
  close(): void {
    this.child.stdin.end();
    if (this.child.pid) killPid(this.child.pid);
  }
}

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  dir = await scratch('claude-code');
  quick = await copyExample('quick-start', dir);
  fake = claudeCodeFake(dir);
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json'), ...fake.env });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
});

test.afterAll(async () => {
  client?.close();
  await new Runner(runnerExe!, quick).cleanUp(run);
  await studio?.app.close();
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('Use with Claude Code: not registered, the exact command, and Register runs it in the project folder', async () => {
  // the palette command opens the same panel as the header icon
  await palette('claude_code.register');
  const panel = page.getByRole('region', { name: 'Use with Claude Code' });
  await expect(panel.getByText('Not registered with Claude Code for this folder.')).toBeVisible({ timeout: 20_000 });
  const command = `claude mcp add rcfd-project --scope local -- "${mcp}" --project "${quick}"`;
  await expect(panel.locator('code')).toHaveText(command);
  await expect(panel.getByText('Claude Code asks before each tool.')).toBeVisible();
  await shot('01-not-registered');
  await panel.getByRole('button', { name: 'Register' }).click();
  await expect(panel.getByText('Registered with Claude Code for this folder.', { exact: true })).toBeVisible({ timeout: 20_000 });
  await expect(panel.getByRole('button', { name: 'Register' })).toBeDisabled();
  const add = fake.calls().filter((c) => c.argv[1] === 'add');
  expect(add).toHaveLength(1);
  expect(add[0]!.argv).toEqual(['mcp', 'add', 'rcfd-project', '--scope', 'local', '--', mcp, '--project', quick]);
  expect(norm(add[0]!.cwd)).toBe(norm(quick));
  for (const c of fake.calls()) expect(norm(c.cwd)).toBe(norm(quick));
  await shot('02-registered');
  // Copy puts the command on the clipboard
  await panel.getByRole('button', { name: 'Copy' }).click();
  await expect.poll(() => studio.app.evaluate(({ clipboard }) => clipboard.readText())).toBe(command);
  await panel.getByRole('button', { name: 'Close Use with Claude Code' }).click();
  await expect(panel).toHaveCount(0);
});

test('a registration of another engine path reads so, and Re-register removes it, then adds this one', async () => {
  const state = path.join(dir, 'fake-claude-state.json');
  const s = JSON.parse(readFileSync(state, 'utf8')) as Record<string, Record<string, { command: string; args: string[] }>>;
  for (const k of Object.keys(s)) s[k]!['rcfd-project']!.command = path.join(dir, 'old', 'rcfd-mcp.exe');
  writeFileSync(state, JSON.stringify(s));
  await page.getByRole('button', { name: 'Use with Claude Code' }).click();
  const panel = page.getByRole('region', { name: 'Use with Claude Code' });
  await expect(panel.getByText('Registered with a different engine path.')).toBeVisible({ timeout: 20_000 });
  const before = fake.calls().length;
  await panel.getByRole('button', { name: 'Re-register' }).click();
  await expect(panel.getByText('Registered with Claude Code for this folder.', { exact: true })).toBeVisible({ timeout: 20_000 });
  const done = fake.calls().slice(before).map((c) => c.argv.slice(0, 2).join(' '));
  // Register reads the registration again before it changes anything
  expect(done).toEqual(['mcp get', 'mcp remove', 'mcp add', 'mcp get']);
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
});

test('Open Claude Code here: a terminal in the project folder running claude (recorded in test mode, never opened)', async () => {
  await page.getByRole('button', { name: 'Open Claude Code here' }).click();
  await expect.poll(() => fake.launches().length).toBe(1);
  const l = fake.launches()[0]!;
  expect(norm(l.cwd)).toBe(norm(quick));
  if (process.platform === 'win32') {
    const wt = l.argv[0]!.toLowerCase().endsWith('wt.exe');
    expect(l.argv).toEqual(wt ? [l.argv[0], '-d', quick, fake.env['RCFD_STUDIO_CLAUDE']] : [l.argv[0], '/k', fake.env['RCFD_STUDIO_CLAUDE']]);
  } else {
    expect(l.argv.at(-1)).toBe(fake.env['RCFD_STUDIO_CLAUDE']);
  }
  // the palette command does the same
  await palette('claude_code.open', true);
  await expect.poll(() => fake.launches().length).toBe(2);
});

test('what an MCP client does to the project shows in the open Studio at once, and the activity line names it', async () => {
  const activity = page.locator('.ph-activity');
  // the Studio's own reads and nothing else so far: no line
  await expect(activity).toHaveCount(0);
  client = await McpClient.start(mcp!, quick);
  const cases = page.locator('section[data-group="cases"]');

  await client.call('new_case', { name: 'from-claude', template: 'lid_driven_cavity' });
  await expect(cases).toContainText('from-claude', { timeout: 10_000 });
  await expect(activity).toContainText('Claude Code made cases/from-claude.toml', { timeout: 10_000 });
  await shot('03-activity-made');

  await client.call('edit_case', { path: 'cases/from-claude.toml', base_hash: await client.hash('cases/from-claude.toml'), set: { 'physics.nu': 0.02 } });
  await expect(activity).toContainText('Claude Code changed cases/from-claude.toml', { timeout: 10_000 });

  const started = await client.call('start_run', { case: 'cases/from-claude.toml', max_steps: 5 });
  run = started['run'] as string;
  expect(run).toBeTruthy();
  await expect(page.locator(`.ph-group[data-group="runs"] tr[data-run="${run}"]`)).toBeVisible({ timeout: 20_000 });
  await expect(activity).toContainText(`Claude Code started run ${run}`, { timeout: 10_000 });
  // the link opens the run in the Runs view; the view never moved on its own before that
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible();
  await activity.getByRole('button', { name: run }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible({ timeout: 20_000 });
  await page.getByRole('tab', { name: 'Home' }).click();

  await new Runner(runnerExe!, quick).cleanUp(run);
  await client.call('trash_case', { path: 'cases/from-claude.toml', base_hash: await client.hash('cases/from-claude.toml') });
  await expect(page.locator('[data-group="trash"]')).toContainText('cases/from-claude.toml', { timeout: 10_000 });
  await expect(activity).toContainText('Claude Code moved to the trash cases/from-claude.toml', { timeout: 10_000 });
  await shot('04-activity-trash');

  // the Studio's own write (Restore, journalled as client studio) shows in the lists, never on the activity line
  await page.locator('[data-group="trash"]').getByRole('button', { name: 'Restore cases/from-claude.toml' }).click();
  await expect(cases).toContainText('from-claude', { timeout: 10_000 });
  const journal = readJournal();
  expect(journal.filter((j) => j.who === 'restore_trash').map((j) => j.actor?.client)).toEqual(['studio']);
  expect(journal.filter((j) => j.who === 'new_case').map((j) => j.actor?.client)).toEqual(['mcp']);
  await expect(activity).toContainText('Claude Code moved to the trash cases/from-claude.toml');
});
