// F-837: save or discard before a close, in the packaged Studio against the real engine (rcfd serve for the case
// editor's sessions, rcfd-mcp for the project). A close that would lose case changes not saved asks with the decision
// panel (src/renderer/case/closeGuard.tsx): Close the session, Close project, and the window's own close (what its close
// button and Alt+F4 do: BrowserWindow.close() in the main process, a round trip to the page). The panel names each file
// and its changes, holds Tab, answers Escape as Keep editing and gives the focus back. A quit in test mode does not ask
// (Playwright's close() must not hang), and a page that does not answer cannot hold its window.
import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { mkdir, rm } from 'node:fs/promises';
import path from 'node:path';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { copyExample, findMcp, mcpPids, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, killPid, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
const mcp = findMcp();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f837');
const RE100 = 'cases/cavity-re100.toml';
const RE400 = 'cases/cavity-re400.toml';

let dir = '';
let quick = '';
let service: RunningRcfd;
let rpc: RpcConnection;
let studio: Studio;
let page: Page;
const extra: Studio[] = [];

const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const file = (rel: string) => readFileSync(path.join(quick, ...rel.split('/')), 'utf8');
const panel = () => page.getByRole('alertdialog');
const sessions = async () => ((await rpc.request('session.list')) as { sessions: { session: string }[] }).sessions.map((s) => s.session);

async function launch(): Promise<Studio> {
  const s = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`, RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => s.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await stubDialog(s.app, quick);
  await s.page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(s.page.locator(`tr[data-path="${RE100}"]`)).toBeVisible({ timeout: 30_000 });
  return s;
}

/** Opens `rel` in the case editor from the home and returns its session. */
async function edit(p: Page, rel: string): Promise<string> {
  await p.evaluate(() => (location.hash = ''));
  await p.getByRole('button', { name: `Edit ${rel}`, exact: true }).click();
  await expect(p.getByRole('heading', { level: 2, name: rel, exact: true })).toBeVisible({ timeout: 30_000 });
  await expect.poll(() => p.evaluate(() => location.hash)).toMatch(/^#\/case\/s\d+$/);
  return decodeURIComponent((await p.evaluate(() => location.hash)).replace('#/case/', ''));
}

/** Types `value` into physics.nu; with `commit`, Enter commits it (else the typing is left in the field). */
async function setNu(p: Page, value: string, commit = true): Promise<void> {
  await p.getByRole('textbox', { name: 'Find a setting' }).fill('physics.nu');
  const nu = p.locator('.st-key[data-path="physics.nu"]').getByRole('textbox');
  await nu.fill(value);
  if (!commit) return;
  await nu.press('Enter');
  await expect(p.getByRole('list', { name: 'Settings a save writes' })).toContainText('physics.nu');
}

/** Tab and Shift+Tab, several times round: the focus stays inside the panel. */
async function expectTabHeld(): Promise<void> {
  for (const key of ['Tab', 'Tab', 'Tab', 'Tab', 'Shift+Tab', 'Shift+Tab', 'Shift+Tab', 'Shift+Tab']) {
    await page.keyboard.press(key);
    expect(await page.evaluate(() => document.activeElement?.closest('[role="alertdialog"]') !== null), `after ${key}`).toBe(true);
  }
}

/** The window's own close, as its close button and Alt+F4 do. */
const closeWindow = (s: Studio) => s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]?.close());

test.beforeAll(async () => {
  await mkdir(SHOTS, { recursive: true });
  dir = await scratch('f837-close');
  quick = await copyExample('quick-start', dir);
  service = await startRcfd(rcfd!);
  rpc = await RpcConnection.connectTcp('127.0.0.1', service.port);
  studio = await launch();
  page = studio.page;
});

test.afterAll(async () => {
  for (const s of [...extra, studio]) await s?.app.close().catch(() => undefined);
  rpc?.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('Close the session with a change not saved: the panel names it, holds Tab, Escape keeps editing, Save and close writes it', async () => {
  const s = await edit(page, RE100);
  await setNu(page, '0.02');
  const close = page.getByRole('button', { name: 'Close the session' });
  await close.focus();
  await page.keyboard.press('Enter');
  const ask = page.getByRole('alertdialog', { name: `Close ${RE100} with 1 change not saved?` });
  await expect(ask).toBeVisible();
  await expect(ask.getByRole('list', { name: 'Changes not saved' })).toHaveText(`${RE100}: 1 change (physics.nu)`);
  await expect(ask.getByRole('button')).toHaveText(['Save and close', 'Close and lose them', 'Keep editing']);
  // keeping is the default: the focus is on Keep editing
  expect(await page.evaluate(() => document.activeElement?.textContent)).toBe('Keep editing');
  await page.mouse.move(1439, 899);
  await shot('20-close-session-asks');
  await expectTabHeld();
  await page.keyboard.press('Escape');
  await expect(panel()).toHaveCount(0);
  await expect(close).toBeFocused();
  expect(file(RE100)).toContain('nu = 0.01');
  // Save and close, by the keyboard: the file holds the change and the session is gone
  await page.keyboard.press('Enter');
  await expect(ask).toBeVisible();
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Shift+Tab');
  await expect(ask.getByRole('button', { name: 'Save and close' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  await expect.poll(() => file(RE100), { timeout: 20_000 }).toMatch(/nu = 0\.02/);
  expect(await sessions()).not.toContain(s);
});

test('the case settings’ own Close session (F-837): unsaved case edits ask with the shared prompt, not the older loss question', async () => {
  const s = await edit(page, RE100);
  await setNu(page, '0.03');
  await page.evaluate((id) => (location.hash = `#/settings/${encodeURIComponent(id)}`), s);
  const close = page.getByRole('button', { name: 'Close session' });
  await expect(close).toBeVisible({ timeout: 30_000 });
  await close.focus();
  await page.keyboard.press('Enter');
  const ask = page.getByRole('alertdialog', { name: `Close ${RE100} with 1 change not saved?` });
  await expect(ask).toBeVisible();
  await expect(ask.getByRole('button')).toHaveText(['Save and close', 'Close and lose them', 'Keep editing']);
  await expect(page.getByRole('alertdialog', { name: /^Close session / })).toHaveCount(0);
  // Keep editing is the default (focused, and Escape): the session stays
  expect(await page.evaluate(() => document.activeElement?.textContent)).toBe('Keep editing');
  await page.mouse.move(1439, 899);
  await shot('22-settings-close-session-asks');
  await page.keyboard.press('Escape');
  await expect(panel()).toHaveCount(0);
  await expect(close).toBeFocused();
  expect(await sessions()).toContain(s);
  // Close and lose them: the session closes, the file is untouched, and no second question follows
  await page.keyboard.press('Enter');
  await expect(ask).toBeVisible();
  await page.keyboard.press('Shift+Tab');
  await expect(ask.getByRole('button', { name: 'Close and lose them' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(panel()).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => location.hash), { timeout: 20_000 }).toBe('#/settings');
  await expect.poll(sessions, { timeout: 20_000 }).not.toContain(s);
  expect(file(RE100)).not.toMatch(/nu = 0\.03/);
  await page.evaluate(() => (location.hash = ''));
  await expect(page.locator(`tr[data-path="${RE100}"]`)).toBeVisible({ timeout: 30_000 });
});

test('Close project with a change not saved: Keep editing keeps the project; Close and lose them closes the session and the project', async () => {
  const s = await edit(page, RE400);
  await setNu(page, '0.005');
  await page.evaluate(() => (location.hash = ''));
  await expect(page.locator(`.ph-unsaved[data-unsaved="${RE400}"]`)).toBeVisible({ timeout: 15_000 });
  await page.getByRole('button', { name: 'Close project' }).click();
  const ask = page.getByRole('alertdialog', { name: /^Close .+ with 1 change not saved\?$/ });
  await expect(ask).toBeVisible();
  await expect(ask).toContainText(`${RE400}: 1 change (physics.nu)`);
  await expect(ask).toContainText('Closing without saving loses them');
  await page.mouse.move(1439, 899);
  await shot('21-close-project-asks');
  await ask.getByRole('button', { name: 'Keep editing' }).click();
  await expect(panel()).toHaveCount(0);
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible();
  expect(await sessions()).toContain(s);
  // lose them: the session closes with its change, the file stays as it was, the project closes
  await page.getByRole('button', { name: 'Close project' }).click();
  await ask.getByRole('button', { name: 'Close and lose them' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'No project open' })).toBeVisible({ timeout: 20_000 });
  await expect.poll(sessions, { timeout: 10_000 }).not.toContain(s);
  expect(file(RE400)).toContain('nu = 0.0025');
  // nothing unsaved: Close project closes at once (the project opens again for the next test)
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.locator(`tr[data-path="${RE100}"]`)).toBeVisible({ timeout: 30_000 });
  await page.getByRole('button', { name: 'Close project' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'No project open' })).toBeVisible({ timeout: 20_000 });
  await expect(panel()).toHaveCount(0);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.locator(`tr[data-path="${RE100}"]`)).toBeVisible({ timeout: 30_000 });
});

test('the window’s own close asks too: typing not yet committed counts, Escape gives the focus back to the field, Save and close writes and closes', async () => {
  await edit(page, RE400);
  await setNu(page, '0.004', false);
  const nu = page.locator('.st-key[data-path="physics.nu"]').getByRole('textbox');
  await expect(nu).toBeFocused();
  await closeWindow(studio);
  const ask = page.getByRole('alertdialog', { name: 'Close rcfd Studio with 1 change not saved?' });
  await expect(ask).toBeVisible({ timeout: 15_000 });
  await expect(ask).toContainText(`${RE400}: 1 change (physics.nu)`);
  await page.mouse.move(1439, 899);
  await shot('22-close-window-asks');
  await expectTabHeld();
  await page.keyboard.press('Escape');
  await expect(panel()).toHaveCount(0);
  await expect(nu).toBeFocused();
  // the window stayed, and a second close asks again
  expect(await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().length)).toBe(1);
  const closed = studio.app.waitForEvent('close', { timeout: 60_000 });
  await closeWindow(studio);
  await expect(ask).toBeVisible({ timeout: 15_000 });
  await ask.getByRole('button', { name: 'Save and close' }).click();
  await closed;
  expect(file(RE400)).toMatch(/nu = 0\.004/);
});

test('nothing unsaved, the window closes at once; a page that does not answer cannot hold it; a quit in test mode does not ask', async () => {
  // nothing unsaved
  const a = await launch();
  extra.push(a);
  const t0 = Date.now();
  const closedA = a.app.waitForEvent('close', { timeout: 30_000 });
  await closeWindow(a);
  await closedA;
  expect(Date.now() - t0).toBeLessThan(15_000);
  // the page's listener removed: no answer, the window closes after the main process's wait
  const b = await launch();
  extra.push(b);
  await b.page.evaluate(() => window.studio.app.onCloseRequest(null));
  const closedB = b.app.waitForEvent('close', { timeout: 30_000 });
  await closeWindow(b);
  await closedB;
  // a change not saved, then Playwright's close(), which is app.quit(): test mode quits without asking
  const c = await launch();
  extra.push(c);
  await edit(c.page, RE100);
  await setNu(c.page, '0.03');
  await Promise.race([c.app.close(), new Promise((_, no) => setTimeout(() => no(new Error('the quit hung')), 30_000))]);
  expect(file(RE100)).toMatch(/nu = 0\.02/);
});
