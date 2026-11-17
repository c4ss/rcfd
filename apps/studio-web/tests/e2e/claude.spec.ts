// The Claude Code panel (F-814; G11, G10, G8, G12) in the packaged Studio, against the engine's project server
// (rcfd-mcp, for the project) and the fake claude of tests/support/fake-claude.mjs (never a real claude: the program is
// named by RCFD_STUDIO_CLAUDE and CLAUDE_CONFIG_DIR is a temporary folder).
//
// - the project's sessions are listed: one open in another process (mirrored read-only, no message field), one closed;
// - the closed one is resumed and typed into from the keyboard; the reply streams in; the spawned argument list has the
//   allow-list and no approve-everything flag; a second message while the first is answered is refused (one writer);
// - the run id and the files in the reply are reference chips that open the run in Runs, the case file in the case
//   editor (F-812) and the note on Home;
// - the bridge is window.studioClaude with exactly its functions; the Studio writes nothing into the project folder.
// Screenshots at 1440 by 900 go to test-results-visual/f814/ for the maintainer's view-and-report (G13).
import { expect, test, type Page } from '@playwright/test';
import { mkdir, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { appRoot, findRcfd, killIfRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { copyExample, findMcp, NO_MCP, snapshot, stubDialog } from '../support/project.js';
import { APPROVE_EVERYTHING, claudeFixture, FAKE_CLAUDE, sampleTranscript, type ClaudeFixture } from '../support/claude.js';
import { until } from '../support/fake-service.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f814');
const mcp = findMcp();
test.skip(!mcp, NO_MCP);
// the case editor a case file opens in (F-812) makes its session on `rcfd serve`
const rcfd = findRcfd();
test.skip(!rcfd, NO_RCFD);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

const S1 = '00000000-0000-4000-8000-000000000001';
const S2 = '00000000-0000-4000-8000-000000000002';
const RUN = 'r-20261028T152641-3900';

let fx: ClaudeFixture;
let quick = '';
let before: Record<string, string> = {};
let studio: Studio | null = null;
let service: RunningRcfd | null = null;
const consoleErrors: string[] = [];

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const panel = (page: Page) => page.getByRole('complementary', { name: 'Claude Code' });
const sessionButton = (page: Page, name: RegExp) => panel(page).getByRole('button', { name });

test.beforeAll(async () => {
  await mkdir(SHOTS, { recursive: true });
  fx = await claudeFixture('e2e');
  const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f814-project-'));
  quick = await copyExample('quick-start', dir);
  // a closed session (its transcript, no process) and one open in another process (this test's own, alive)
  await fx.transcript(quick, S1, sampleTranscript(quick, S1));
  await fx.transcript(quick, S2, [
    { type: 'user', cwd: quick, sessionId: S2, timestamp: '2026-11-16T10:00:00.000Z', message: { role: 'user', content: 'Plot the residuals of the heated cavity' } },
    { type: 'assistant', cwd: quick, sessionId: S2, timestamp: '2026-11-16T10:00:04.000Z', message: { role: 'assistant', content: [{ type: 'text', text: 'Working on it in the terminal.' }] } },
  ]);
  await fx.setAgents([{ pid: process.pid, cwd: quick, sessionId: S2, name: 'terminal-chat' }]);
  before = await snapshot(quick);
  service = await startRcfd(rcfd!);
  studio = await launchStudio({
    RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`,
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_RECENT: path.join(fx.root, 'recent.json'),
    RCFD_STUDIO_CLAUDE: FAKE_CLAUDE,
    RCFD_STUDIO_CLAUDE_STATE: fx.state,
    ...fx.env({ FAKE_CLAUDE_DELAY_MS: '40', FAKE_CLAUDE_HOLD_MS: '1500' }),
  });
  const { app, page } = studio;
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  await page.evaluate(() => {
    const w = window as unknown as { __csp: string[] };
    w.__csp = [];
    document.addEventListener('securitypolicyviolation', (e) => w.__csp.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
  await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await stubDialog(app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  await studio?.app.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
});

test('the panel lists the project’s sessions: one open elsewhere (read-only), one closed', async () => {
  const page = studio!.page;
  await expect(panel(page)).toBeVisible();
  await expect(panel(page).getByRole('heading', { name: 'Claude Code' })).toBeVisible();
  // the first listings start the CLI cold (an Electron-as-Node process for the fake), which is slow on a loaded machine
  await expect(sessionButton(page, /^terminal-chat, open elsewhere, read-only, process \d+$/)).toBeVisible({ timeout: 45_000 });
  await expect(sessionButton(page, /^Which runs converged\?, closed$/)).toBeVisible();
  await expect(panel(page).getByRole('heading', { name: 'Sessions 2' })).toBeVisible();
  await shot(page, '01-sessions');
});

test('the bridge is window.studioClaude with exactly its functions, frozen; the page cannot name a program or flag', async () => {
  const page = studio!.page;
  const shape = await page.evaluate(() => {
    const c = window.studioClaude as unknown as Record<string, unknown>;
    return { keys: Object.keys(c).sort(), frozen: Object.isFrozen(c), fns: Object.values(c).every((v) => typeof v === 'function') };
  });
  expect(shape).toEqual({ keys: ['cancel', 'confirm', 'offState', 'onState', 'refresh', 'select', 'send', 'state'], frozen: true, fns: true });
  const odd = await page.evaluate(() => (window.studioClaude.send as unknown as (s: unknown, t: unknown) => Promise<{ ok: boolean; error?: { code: string } }>)('--dangerously-skip-permissions', 'hi'));
  expect(odd).toMatchObject({ ok: false, error: { code: 'bad_request' } });
  expect(await fx.chats()).toEqual([]);
});

test('mirror: the open session is shown read-only from the keyboard, with no message field', async () => {
  const page = studio!.page;
  const open = sessionButton(page, /^terminal-chat, open elsewhere/);
  await open.focus();
  await expect(open).toBeFocused();
  expect(await open.evaluate((e) => getComputedStyle(e).outlineStyle)).not.toBe('');
  await page.keyboard.press('Enter');
  await expect(open).toHaveAttribute('aria-current', 'true');
  await expect(panel(page).getByRole('log', { name: /Transcript of terminal-chat/ })).toContainText('Working on it in the terminal.');
  await expect(panel(page).getByRole('note')).toContainText(/open in process \d+; it is mirrored read-only/);
  await expect(panel(page).getByRole('textbox')).toHaveCount(0);
  const refused = await page.evaluate((s) => window.studioClaude.send(s, 'hello'), S2);
  expect(refused).toMatchObject({ ok: false, error: { code: 'read_only' } });
  expect(await fx.chats()).toEqual([]);
  await shot(page, '02-mirror-read-only');
});

test('resume: the closed session is typed into from the keyboard; the reply streams in; one writer; the argv is conservative', async () => {
  const page = studio!.page;
  const closed = sessionButton(page, /^Which runs converged\?, closed$/);
  await closed.focus();
  await page.keyboard.press('Enter');
  await expect(closed).toHaveAttribute('aria-current', 'true');
  const field = panel(page).getByRole('textbox', { name: /^Message to Claude Code, Which runs converged\?/ });
  await expect(field).toBeVisible();
  // Tab order: the session list, then on to the message field (the composer is reachable without the mouse)
  await field.focus();
  await page.keyboard.type('Which run converged, and where is its case?');
  await page.keyboard.press('Enter');
  await expect(panel(page).getByText('Claude Code is answering')).toBeVisible({ timeout: 30_000 });
  await expect(panel(page).getByRole('button', { name: 'Stop' })).toBeVisible();
  // one writer: a second message while the first is answered is refused by the main process
  const second = await page.evaluate((s) => window.studioClaude.send(s, 'again'), S1);
  expect(second).toMatchObject({ ok: false, error: { code: 'busy' } });
  const state = await page.evaluate(() => window.studioClaude.state());
  expect(state.project?.writer?.session).toBe(S1);
  await shot(page, '03-answering');
  await expect(panel(page).getByText(/^Answered/)).toBeVisible({ timeout: 30_000 });
  const chats = await fx.chats();
  expect(chats).toHaveLength(1);
  const argv = chats[0]!.argv;
  expect(argv.slice(-2)).toEqual(['--resume', S1]);
  expect(argv.slice(argv.indexOf('--permission-mode'), argv.indexOf('--permission-mode') + 4)).toEqual(['--permission-mode', 'dontAsk', '--permission-prompts', 'none']);
  expect(argv.filter((a) => a.startsWith('mcp__rcfd_project__'))).toHaveLength(15);
  expect(argv.filter((a) => APPROVE_EVERYTHING.some((re) => re.test(a)))).toEqual([]);
  expect(path.resolve(chats[0]!.cwd).toLowerCase()).toBe(path.resolve(quick).toLowerCase());
  // the reply and its reference chips
  const log = panel(page).getByRole('log');
  await expect(log).toContainText('converged in 412 iterations');
  await expect(log.locator('b', { hasText: RUN }).first()).toBeVisible();
  const refs = panel(page).getByRole('group', { name: 'References in this reply' }).last();
  await expect(refs.getByRole('button')).toHaveText([RUN, 'cases/cavity-re100.toml', 'notes/README.md']);
  await shot(page, '04-reply-with-references');
});

test('a run id in the reply opens the run in Runs; a note opens on the project home, a case file in the case editor (keyboard)', async () => {
  const page = studio!.page;
  const refs = () => panel(page).getByRole('group', { name: 'References in this reply' }).last();
  await refs().getByRole('button', { name: `Open run ${RUN} in Runs` }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('tab', { name: 'Runs' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('row', { selected: true })).toContainText(RUN);
  await shot(page, '05-run-opened-from-reply');
  await refs().getByRole('button', { name: 'Show notes/README.md on the project home' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('tab', { name: 'Home' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('textbox', { name: 'Filter by name' })).toHaveValue('notes/README.md');
  await expect(page.locator('main').getByText('notes/README.md').first()).toBeVisible();
  await expect(refs().getByRole('button', { name: 'Show cases/cavity-re100.toml on the project home' })).toHaveCount(0);
  await refs().getByRole('button', { name: 'Edit cases/cavity-re100.toml in the case editor' }).focus();
  await page.keyboard.press('Enter');
  await expect.poll(() => page.evaluate(() => location.hash), { timeout: 20_000 }).toMatch(/^#\/case\/s\d+$/);
  await expect(page.getByText('cases/cavity-re100.toml').first()).toBeVisible();
  await shot(page, '05b-case-opened-from-reply');
  // back to the project home: the panel and its transcript are as they were
  await page.evaluate(() => (location.hash = ''));
  await expect(refs().getByRole('button')).toHaveText([RUN, 'cases/cavity-re100.toml', 'notes/README.md'], { timeout: 20_000 });
});

test('tool approvals: the allow-list is shown from the keyboard, everything else denied', async () => {
  const page = studio!.page;
  const disclose = panel(page).getByRole('button', { name: '15 allowed, everything else denied' });
  await disclose.focus();
  await page.keyboard.press('Enter');
  await expect(disclose).toHaveAttribute('aria-expanded', 'true');
  await expect(panel(page).getByRole('list', { name: 'Allowed tools' }).getByRole('listitem')).toHaveCount(15);
  await expect(panel(page).getByText(/never asked and never approved/)).toBeVisible();
  await shot(page, '06-approvals');
  await page.keyboard.press('Enter');
});

test('a new chat starts a session with an id chosen by the Studio; the panel hides and shows from the keyboard', async () => {
  const page = studio!.page;
  await sessionButton(page, /^New chat$/).click();
  const field = panel(page).getByRole('textbox', { name: 'Message to Claude Code, a new session' });
  await field.fill('Hello');
  await panel(page).getByRole('button', { name: 'Send' }).press('Enter');
  const chats = await until(async () => ((await fx.chats()).length === 2 ? fx.chats() : null), 30_000, 'the second chat');
  await expect
    .poll(() => page.evaluate(() => window.studioClaude.state().then((s) => s.project?.writer === null && s.project?.turn?.status)), { timeout: 30_000 })
    .toBe('finished');
  await expect(panel(page).getByText(/^Answered/)).toBeVisible();
  expect(chats[1]!.argv.at(-2)).toBe('--session-id');
  await expect(panel(page).getByRole('heading', { name: 'Sessions 3' })).toBeVisible({ timeout: 30_000 });
  const hide = panel(page).getByRole('button', { name: 'Hide Claude Code' });
  await hide.focus();
  await page.keyboard.press('Enter');
  await expect(panel(page)).toHaveCount(0);
  const show = page.getByRole('button', { name: 'Show Claude Code' });
  await show.focus();
  await page.keyboard.press('Enter');
  await expect(panel(page)).toBeVisible();
});

test('the Studio wrote nothing into the project folder (G12), and no console error or CSP violation', async () => {
  const page = studio!.page;
  // the fake writes transcripts only under CLAUDE_CONFIG_DIR; the Studio's own files are in its state folder
  await until(async () => JSON.stringify(await snapshot(quick)) === JSON.stringify(before), 5000, 'the folder unchanged');
  expect(await snapshot(quick)).toEqual(before);
  expect(await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp)).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
