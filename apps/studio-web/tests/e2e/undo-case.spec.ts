// Undo and redo in the case editor (F-829 on F-812) in the packaged Studio, against the real engine (`rcfd serve` and
// `rcfd-mcp --project`, a copy of the quick-start project): the history is the session's, opening the file is not a
// step, an undone change leaves the list of what a save writes, the file is never touched by undo (a save is the
// file's own, hash-guarded step), and a value another writer set in the session is a conflict, not overwritten.
// Screenshot at 1440 by 900 to test-results-visual/f829/.
//
// Needs rcfd and rcfd-mcp (tests/support/rcfd.ts, tests/support/project.ts); without them the tests are skipped.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { readFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { copyExample, findMcp, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
const mcp = findMcp();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f829');
const CASE = 'cases/cavity-re100.toml';

let dir = '';
let quick = '';
let service: RunningRcfd;
let rpc: RpcConnection;
let studio: Studio;
let page: Page;
let session = '';
const consoleErrors: string[] = [];

const row = (p: string): Locator => page.locator(`.st-key[data-path="${p}"]`);
const controls = (): Locator => page.getByRole('group', { name: /^Undo and redo/ });
const undoButton = (): Locator => controls().getByRole('button').first();
const redoButton = (): Locator => controls().getByRole('button').nth(1);
const engineValue = async (p: string) => ((await rpc.request('settings.get', { session, path: p })) as { value: unknown }).value;
const saveList = (): Locator => page.getByRole('list', { name: 'Settings a save writes' });

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  dir = await scratch('f829');
  quick = await copyExample('quick-start', dir);
  service = await startRcfd(rcfd!);
  rpc = await RpcConnection.connectTcp('127.0.0.1', service.port);
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`, RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  page = studio.page;
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.locator(`tr[data-path="${CASE}"]`)).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  await studio?.app.close();
  rpc?.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('opening the file is not a step; a change is, and undo and redo take it back and forth in the session only', async () => {
  const file = path.join(quick, 'cases', 'cavity-re100.toml');
  const before = await readFile(file, 'utf8');
  await page.getByRole('button', { name: `Edit ${CASE}`, exact: true }).click();
  await expect.poll(() => page.evaluate(() => location.hash), { timeout: 20_000 }).toMatch(/^#\/case\/s\d+$/);
  session = decodeURIComponent((await page.evaluate(() => location.hash)).replace('#/case/', ''));
  await expect(page.getByRole('heading', { level: 2, name: CASE, exact: true })).toBeVisible({ timeout: 20_000 });
  await expect(undoButton()).toHaveText('Nothing to undo');
  await expect(undoButton()).toBeDisabled();

  const find = page.getByRole('textbox', { name: 'Find a setting' });
  await find.fill('physics.nu');
  const nu = row('physics.nu').getByRole('textbox');
  await nu.fill('0.02');
  await nu.press('Enter');
  await expect(saveList()).toContainText('physics.nu');
  await expect(undoButton()).toHaveText('Undo: set physics.nu');
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  await page.screenshot({ path: path.join(SHOTS, 'case-editor-undo.png'), scale: 'css' });

  await undoButton().click();
  await expect.poll(() => engineValue('physics.nu')).toBe(0.01);
  await expect(nu).toHaveValue('0.01');
  await expect(redoButton()).toHaveText('Redo: set physics.nu');
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press('ControlOrMeta+Shift+z');
  await expect.poll(() => engineValue('physics.nu')).toBe(0.02);
  await expect(nu).toHaveValue('0.02');
  // undo touches the session, never the file: saving is the file's own step
  expect(await readFile(file, 'utf8')).toBe(before);
});

test('a value another writer set in the session is a conflict: shown, not overwritten', async () => {
  await rpc.request('settings.set', { session, path: 'physics.nu', value: 0.03 });
  await expect(row('physics.nu').getByRole('textbox')).toHaveValue('0.03');
  await undoButton().click();
  const d = page.getByRole('alertdialog');
  await expect(d.getByRole('heading')).toHaveText('Not undone: changed outside the Studio');
  await expect(d.getByRole('row', { name: /physics\.nu/ })).toContainText('0.02');
  await expect(d.getByRole('row', { name: /physics\.nu/ })).toContainText('0.03');
  expect(await engineValue('physics.nu')).toBe(0.03);
  await d.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('alertdialog')).toHaveCount(0);
  expect(await engineValue('physics.nu')).toBe(0.03);
});

test('no console error', () => {
  expect(consoleErrors).toEqual([]);
});
