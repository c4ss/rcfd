// The project home (F-805) in the packaged Studio, against the real engine project server (rcfd-mcp, built by
// `cargo build --release -p rcfd-mcp`): a project folder opened through the main process's folder dialog (stubbed here),
// its cases, meshes, runs and notes listed as the engine lists them, edits made outside the Studio shown within 2 s
// without a reload, the empty and failure states, the port's refusals, and the folder left exactly as it was (G12).
// Screenshots at 1440 by 900 go to test-results-visual/f805/ for the maintainer's view-and-report (G13).
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { mkdir, readFile, rm, unlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { appRoot } from '../support/rcfd.js';
import {
  copyExample,
  copyRun,
  emptyProject,
  engineAnswers,
  findMcp,
  mcpPids,
  NO_MCP,
  projectState,
  scratch,
  snapshot,
  stubDialog,
} from '../support/project.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f805');
const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial' });

let dir = '';
let quick = '';
let recentFile = '';
let studio: Studio | null = null;

const sized = async (s: Studio): Promise<Studio> => {
  await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => s.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  return s;
};

const launch = async (env: Record<string, string>): Promise<Studio> => sized(await launchStudio(env));
const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });

/** Opens `folder` through the (stubbed) folder dialog and waits for its name as the page's h1. */
async function openByDialog(app: ElectronApplication, page: Page, folder: string, name: RegExp | string): Promise<void> {
  await stubDialog(app, folder);
  await page.getByRole('button', { name: /^Open project( folder)?…$/ }).first().click();
  await expect(page.getByRole('heading', { level: 1, name })).toBeVisible({ timeout: 20_000 });
}

test.beforeAll(async () => {
  dir = await scratch('e2e');
  quick = await copyExample('quick-start', dir);
  recentFile = path.join(dir, 'recent.json');
  await mkdir(SHOTS, { recursive: true });
});

test.afterAll(async () => {
  await studio?.app.close();
  studio = null;
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('first start: no project open, the next step, and an empty recent list', async () => {
  studio = await launch({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: recentFile });
  const { page } = studio;
  expect(await page.evaluate(() => document.querySelector('h1')?.textContent)).toBe('No project open');
  await expect(page.getByRole('button', { name: 'Open project folder…' })).toBeEnabled();
  await expect(page.getByRole('note', { name: /^No recent projects\./ })).toBeVisible();
  const s = await projectState(page);
  expect(s).toMatchObject({ engine: { configured: true, path: mcp, from: 'RCFD_STUDIO_MCP' }, recent: [], opening: null, failure: null, project: null });
  await shot(page, '01-no-project');
});

test('the bridge is window.studioProject with exactly its functions, and refuses a path that is not in the recent list', async () => {
  const { page } = studio!;
  const shape = await page.evaluate(() => {
    const p = window.studioProject as unknown as Record<string, unknown>;
    return { keys: Object.keys(p).sort(), fns: Object.values(p).every((v) => typeof v === 'function'), frozen: Object.isFrozen(p) };
  });
  expect(shape).toEqual({ keys: ['close', 'dismiss', 'forget', 'offState', 'onState', 'open', 'pick', 'refresh', 'state'], fns: true, frozen: true });
  const r = await page.evaluate((f) => window.studioProject.open(f), quick);
  expect(r).toEqual({ ok: false, error: { code: 'not_recent', message: expect.stringContaining('recent list') } });
  const odd = await page.evaluate(() => (window.studioProject.open as unknown as (p: unknown) => Promise<unknown>)({ toString: () => 'C:\\' }));
  expect(odd).toMatchObject({ ok: false, error: { code: 'bad_request' } });
  expect((await projectState(page)).project).toBeNull();
  // the shell's own channels are unchanged
  const names = await studio!.app.evaluate(({ ipcMain }) => {
    const handlers = (ipcMain as unknown as { _invokeHandlers?: Map<string, unknown> })._invokeHandlers;
    return {
      handlers: handlers ? [...handlers.keys()].filter((n) => !/^[A-Z0-9_]+$/.test(n)).sort() : null,
      listeners: ipcMain.eventNames().map(String).filter((n) => n !== 'error'),
    };
  });
  // F-818 and F-837: the shell's channels are app.info, app.ready, the close answer and the theme's two
  expect(names).toEqual({ handlers: ['app.closeAnswer', 'app.info', 'app.ready', 'theme.get', 'theme.set'], listeners: [] });
});

test('opening a project lists its cases, meshes, runs and notes as the engine lists them, and writes nothing to it', async () => {
  const { app, page } = studio!;
  const before = await snapshot(quick);
  const engine = await engineAnswers(mcp!, quick);
  await openByDialog(app, page, quick, engine.info.name);
  const s = await projectState(page);
  const v = s.project!;
  expect(v.folder).toBe(quick);
  expect(v.liveness.live).toBe(true);
  expect(v.info?.name).toBe(engine.info.name);
  // runs and meshes: the engine's own lists, in its order
  expect(v.runs.items.map((r) => [r.id, r.state])).toEqual(engine.runs.runs.map((r) => [r.id, r.state]));
  expect(v.runs.total).toBe(engine.runs.total);
  expect(v.meshes.items.map((m) => m.path)).toEqual(engine.meshes.meshes.map((m) => m.path));
  // cases and notes: the names agree with the engine's counts
  expect(v.cases.total).toBe(engine.info.counts['cases']);
  expect(v.notes.total).toBe(engine.info.counts['notes']);
  expect(v.cases.items.map((c) => c.path)).toEqual([
    'cases/cavity-re100.toml',
    'cases/cavity-re400.toml',
    'cases/heated-cavity-ra1e3.toml',
    'cases/heated-cavity-ra1e4.toml',
  ]);
  // and the page shows them: one row per entry, the counts in the kind toggle
  for (const r of engine.runs.runs) await expect(page.locator(`tr[data-run="${r.id}"]`)).toBeVisible();
  for (const c of v.cases.items) await expect(page.locator(`tr[data-path="${c.path}"]`)).toBeVisible();
  await expect(page.getByRole('radio', { name: `Runs ${engine.runs.total}` }).or(page.getByRole('button', { name: `Runs ${engine.runs.total}` }))).toBeVisible();
  await expect(page.getByRole('status', { name: 'Live', exact: true })).toBeVisible();
  await shot(page, '02-open-quick-start');
  // the recent list holds the folder and when, nothing else
  const recent = JSON.parse(await readFile(recentFile, 'utf8')) as { projects: Record<string, unknown>[] };
  expect(recent.projects).toEqual([{ path: quick, opened: expect.any(String) }]);
  await page.waitForTimeout(1000);
  expect(await snapshot(quick)).toEqual(before);
});

test('the kind toggle and the filter narrow the index without asking the engine again', async () => {
  const { page } = studio!;
  const readAt = (await projectState(page)).project!.readAt;
  await page.getByRole('radio', { name: /^Runs \d+$/ }).or(page.getByRole('button', { name: /^Runs \d+$/ })).click();
  await expect(page.locator('section[data-group="cases"]')).toHaveCount(0);
  await expect(page.locator('section[data-group="runs"]')).toBeVisible();
  await page.getByRole('radio', { name: /^All \d+$/ }).or(page.getByRole('button', { name: /^All \d+$/ })).click();
  await page.getByRole('textbox', { name: 'Filter by name' }).fill('heated');
  await expect(page.locator('tr[data-path="cases/cavity-re100.toml"]')).toHaveCount(0);
  await expect(page.locator('tr[data-path="cases/heated-cavity-ra1e3.toml"]')).toBeVisible();
  await page.getByRole('textbox', { name: 'Filter by name' }).fill('no-such-entry');
  await expect(page.getByText('No entry matches “no-such-entry”.')).toBeVisible();
  await page.getByRole('button', { name: 'Clear the filter' }).click();
  await expect(page.locator('tr[data-path="cases/cavity-re100.toml"]')).toBeVisible();
  expect((await projectState(page)).project!.readAt).toBe(readAt);
});

test('edits made outside the Studio show within 2 s, without a reload', async () => {
  const { page } = studio!;
  await page.evaluate(() => ((window as unknown as { __noReload: boolean }).__noReload = true));
  const within2s = async (what: string, edit: () => Promise<void>, shown: () => Promise<void>) => {
    const t0 = Date.now();
    await edit();
    await shown();
    const ms = Date.now() - t0;
    test.info().annotations.push({ type: 'latency', description: `${what}: ${ms} ms` });
    expect(ms, what).toBeLessThan(2000);
  };
  const row = (p: string) => page.locator(`tr[data-path="${p}"]`);
  const opts = { timeout: 2000 };

  // a new case (as Claude Code's new_case or an editor would write it)
  const text = await readFile(path.join(quick, 'cases', 'cavity-re100.toml'), 'utf8');
  await within2s('new case', () => writeFile(path.join(quick, 'cases', 'cavity-re1000.toml'), text.replace(/100\b/, '1000')), () => expect(row('cases/cavity-re1000.toml')).toBeVisible(opts));
  await expect(row('cases/cavity-re1000.toml').getByText(/^changed /)).toBeVisible();
  // a new note
  await within2s('new note', () => writeFile(path.join(quick, 'notes', 'from-claude.md'), '# A note\n'), () => expect(row('notes/from-claude.md')).toBeVisible(opts));
  // a deleted note
  await within2s('deleted note', () => unlink(path.join(quick, 'notes', 'README.md')), () => expect(row('notes/README.md')).toHaveCount(0, opts));
  // a new run folder (as the runner writes it)
  const first = (await projectState(page)).project!.runs.items[0]!.id;
  const id = 'r-20261116T120000-f805';
  await within2s('new run', () => copyRun(quick, first, id), () => expect(page.locator(`tr[data-run="${id}"]`)).toBeVisible(opts));
  // the lane holds the engine's own sentences
  await expect(page.locator('.ph-changes li').filter({ hasText: 'cases/cavity-re1000.toml' }).first()).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { __noReload?: boolean }).__noReload)).toBe(true);
  await shot(page, '03-external-edits');
  // what the Studio shows is what the engine says now
  const engine = await engineAnswers(mcp!, quick);
  const v = (await projectState(page)).project!;
  expect(v.runs.items.map((r) => r.id)).toEqual(engine.runs.runs.map((r) => r.id));
  expect(v.cases.total).toBe(engine.info.counts['cases']);
  expect(v.notes.total).toBe(engine.info.counts['notes']);
});

test('when the project server stops, the home says so and Reconnect brings the link back', async () => {
  const { page } = studio!;
  const pids = mcpPids(quick);
  expect(pids.length).toBeGreaterThan(0);
  for (const pid of pids) process.kill(pid);
  await expect(page.getByText('Live link failed: the project server stopped')).toBeVisible({ timeout: 10_000 });
  await expect(page.getByRole('status').filter({ hasText: 'Not live' })).toBeVisible();
  await shot(page, '04-server-stopped');
  await page.getByRole('alert').getByRole('button', { name: 'Reconnect' }).click();
  await expect(page.getByRole('status', { name: 'Live', exact: true })).toBeVisible({ timeout: 20_000 });
  await writeFile(path.join(quick, 'notes', 'after-reconnect.md'), 'x\n');
  await expect(page.locator('tr[data-path="notes/after-reconnect.md"]')).toBeVisible({ timeout: 2000 });
});

test('a folder that is not a project: the failure, what to do, and the open project stays open', async () => {
  const { app, page } = studio!;
  const plain = path.join(dir, 'just-a-folder');
  await mkdir(plain, { recursive: true });
  await stubDialog(app, plain);
  await page.getByRole('button', { name: 'Open project…' }).click();
  const alert = page.getByRole('alert').filter({ hasText: 'Failed to open: not an rcfd project' });
  await expect(alert).toBeVisible({ timeout: 20_000 });
  await expect(alert).toContainText('rcfd-project.json');
  await expect(alert.getByText('The engine said')).toBeVisible();
  expect((await projectState(page)).project?.folder).toBe(quick);
  await shot(page, '05-not-a-project');
  await alert.getByRole('button', { name: 'Dismiss' }).click();
  await expect(alert).toHaveCount(0);
  // a cancelled dialog changes nothing
  await stubDialog(app, null);
  await page.getByRole('button', { name: 'Open project…' }).click();
  await page.waitForTimeout(300);
  expect((await projectState(page)).project?.folder).toBe(quick);
});

test('an empty project: every group says what is missing and the next step', async () => {
  const { app, page } = studio!;
  const empty = await emptyProject(dir);
  await openByDialog(app, page, empty, 'Empty project');
  await expect(page.getByRole('note', { name: /^No cases yet\./ })).toBeVisible();
  await expect(page.getByRole('note', { name: /^No mesh files\./ })).toBeVisible();
  await expect(page.getByRole('note', { name: /^No runs yet\./ })).toBeVisible();
  await expect(page.getByRole('note', { name: /^No notes\./ })).toBeVisible();
  await expect(page.getByRole('note', { name: /^Nothing has changed since the project was opened\./ })).toBeVisible();
  await shot(page, '06-empty-project');
  // the first case appears without a reload
  await writeFile(path.join(empty, 'cases', 'first.toml'), 'title = "first"\n');
  await expect(page.locator('tr[data-path="cases/first.toml"]')).toBeVisible({ timeout: 2000 });
  // the project open before was closed: its server ended
  await expect.poll(() => mcpPids(quick).length, { timeout: 10_000 }).toBe(0);
});

test('close: back to no project open, with the recent list; a folder that went away says so and can be forgotten', async () => {
  const { page } = studio!;
  await page.getByRole('button', { name: 'Close project' }).click();
  expect(await page.evaluate(() => document.querySelector('h1')?.textContent)).toBe('No project open');
  const recent = page.getByRole('region', { name: 'Recent projects' }).or(page.locator('section.ph-recent'));
  await expect(recent.getByText('empty-project', { exact: true })).toBeVisible();
  await expect(recent.getByText('quick-start', { exact: true })).toBeVisible();
  await shot(page, '07-recent');
  await rm(path.join(dir, 'empty-project'), { recursive: true, force: true });
  await studio!.app.close();
  studio = await launch({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: recentFile });
  const p2 = studio.page;
  const s = await projectState(p2);
  expect(s.project).toBeNull();
  expect(s.recent.map((r) => [path.basename(r.path), r.exists])).toEqual([
    ['empty-project', false],
    ['quick-start', true],
  ]);
  await expect(p2.locator('.ph-recent-row--gone').getByText('Not found')).toBeVisible();
  await shot(p2, '08-recent-folder-gone');
  await p2.getByRole('button', { name: 'Forget empty-project' }).click();
  await expect(p2.locator('.ph-recent-row--gone')).toHaveCount(0);
  const file = JSON.parse(await readFile(recentFile, 'utf8')) as { projects: { path: string }[] };
  expect(file.projects.map((r) => path.basename(r.path))).toEqual(['quick-start']);
  // opening from the recent list
  await p2.getByRole('button', { name: 'Open quick-start' }).click();
  await expect(p2.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  await studio.app.close();
  studio = null;
  // the project server ends with the Studio
  await expect.poll(() => mcpPids(quick).length, { timeout: 10_000 }).toBe(0);
});

test('without the engine: opening is unavailable, and the window says what to set', async () => {
  studio = await launch({ RCFD_STUDIO_RECENT: recentFile });
  const { page } = studio;
  await expect(page.getByText('Projects cannot be opened yet')).toBeVisible();
  await expect(page.getByText(/Set RCFD_STUDIO_MCP to the path of rcfd-mcp/)).toBeVisible();
  // the button is the Unavailable chip: it explains, and does nothing
  await expect(page.getByRole('button', { name: 'Open project folder…' })).toHaveAttribute('aria-disabled', 'true');
  await page.getByRole('button', { name: 'Open project folder…' }).click({ force: true });
  await page.waitForTimeout(300);
  expect((await projectState(page)).failure).toBeNull();
  const s = await projectState(page);
  expect(s.engine.configured).toBe(false);
  await shot(page, '09-not-configured');
  await studio.app.close();
  studio = null;
});
