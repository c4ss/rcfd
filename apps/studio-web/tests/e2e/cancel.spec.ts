// F-832 (G21): every long operation shows its progress after PROGRESS_AFTER_MS, with a Cancel, and a cancel leaves no
// partial state. In the packaged Studio at 1440 x 900, with RCFD_STUDIO_TEST_DELAY (src/main/work/delay.ts) making
// each operation last long enough to be cancelled on any machine:
//
// - opening a project: Cancel leaves no project open, the recent list as it was, no failure, no project server;
// - opening a run: Cancel goes back to the run selected before, its picture and stop as they were;
// - a field load while the line scrubs: Cancel keeps the picture shown before (the same pixels) and puts the line back
//   on its stop; on the field viewport's own page, likewise;
// - saving a case: Cancel before the engine is asked writes nothing (the file's bytes unchanged) and the changes stay
//   pending; a save not cancelled writes them;
// - checking the saved file: Cancel shows no result; a check not cancelled shows one.
//
// The progress is found by role and name (G13, G22: the words are in the accessible names). Screenshots of the
// progress states at 1440 x 900 go to test-results-visual/f832/.
//
// Needs rcfd and rcfd-mcp (the case editor's part), f447_bench and rcfd-mcp (the run workspace's part); each part is
// skipped, with the reason, without them.
import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { PROGRESS_AFTER_MS } from '../../src/shared/work.js';
import { findBench, generate, NO_BENCH } from '../support/f447.js';
import { benchProject, benchRunId } from '../support/longtasks.js';
import { copyExample, copyRun, findMcp, mcpPids, NO_MCP, projectState, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, killPid, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';
import { until } from '../support/fake-service.js';

const rcfd = findRcfd();
const mcp = findMcp();
const bench = findBench();
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f832');
/** Long enough to see the progress (PROGRESS_AFTER_MS) and cancel it, on a loaded machine too. */
const DELAY_MS = 4000;

async function sized(studio: Studio): Promise<void> {
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => studio.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
}

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });

test.describe('the case editor and the project home', () => {
  test.skip(rcfd === null, NO_RCFD);
  test.skip(mcp === null, NO_MCP);

  let dir = '';
  let quick = '';
  let service: RunningRcfd;
  let studio: Studio;
  let page: Page;
  const REL = 'cases/cavity-re100.toml';
  const file = () => path.join(quick, ...REL.split('/'));

  test.beforeAll(async () => {
    mkdirSync(SHOTS, { recursive: true });
    dir = await scratch('f832');
    quick = await copyExample('quick-start', dir);
    service = await startRcfd(rcfd!);
    studio = await launchStudio({
      RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`,
      RCFD_STUDIO_MCP: mcp!,
      RCFD_STUDIO_RECENT: path.join(dir, 'recent.json'),
      RCFD_STUDIO_TEST_DELAY: JSON.stringify({ projectOpen: DELAY_MS, caseCheck: DELAY_MS, caseSave: DELAY_MS }),
    });
    page = studio.page;
    await sized(studio);
  });

  test.afterAll(async () => {
    await studio?.app.close();
    await service?.stop();
    if (service) killIfRcfd(service.pid);
    for (const p of quick ? mcpPids(quick) : []) killPid(p);
    await rm(dir, { recursive: true, force: true }).catch(() => undefined);
  });

  test('opening a project: progress after PROGRESS_AFTER_MS, and Cancel leaves nothing open and the recent list as it was', async () => {
    await stubDialog(studio.app, quick);
    const recentBefore = await readFile(path.join(dir, 'recent.json'), 'utf8').catch(() => null);
    const t0 = Date.now();
    await page.getByRole('button', { name: 'Open project folder…' }).click();
    const bar = page.getByRole('progressbar', { name: `Opening project ${quick}` });
    await expect(bar).toBeVisible({ timeout: 10_000 });
    expect(Date.now() - t0).toBeGreaterThanOrEqual(PROGRESS_AFTER_MS);
    await expect(page.getByRole('status', { name: 'In progress' })).toContainText('Opening project');
    await shot(page, 'progress-project-open');
    await page.getByRole('button', { name: `Cancel opening project ${quick}` }).click();
    await expect(bar).toBeHidden();
    await expect(page.getByRole('heading', { level: 1, name: 'No project open' })).toBeVisible();
    const s = await projectState(page);
    expect({ project: s.project, opening: s.opening, failure: s.failure }).toEqual({ project: null, opening: null, failure: null });
    expect(await readFile(path.join(dir, 'recent.json'), 'utf8').catch(() => null)).toBe(recentBefore);
    // the opening was cancelled before the engine's project server was started: none runs for the folder
    await page.waitForTimeout(DELAY_MS + 1000);
    expect(mcpPids(quick)).toEqual([]);
    expect((await projectState(page)).project).toBeNull();

    // not cancelled, the same opening opens the project
    await page.getByRole('button', { name: 'Open project folder…' }).click();
    await expect(page.locator(`tr[data-path="${REL}"]`)).toBeVisible({ timeout: 30_000 });
  });

  test('saving a case: Cancel before the engine is asked writes nothing and keeps the changes pending', async () => {
    const before = await readFile(file());
    await page.getByRole('button', { name: `Edit ${REL}`, exact: true }).click();
    await expect(page.getByRole('heading', { level: 2, name: REL, exact: true })).toBeVisible({ timeout: 20_000 });
    const find = page.getByRole('textbox', { name: 'Find a setting' });
    await find.fill('physics.nu');
    const field = page.locator('.st-key[data-path="physics.nu"]').getByRole('textbox');
    await field.fill('0.02');
    await field.press('Enter');
    const pending = page.getByRole('list', { name: 'Settings a save writes' });
    await expect(pending).toContainText('physics.nu');
    await find.fill('');

    await page.getByRole('button', { name: 'Save', exact: true }).click();
    const bar = page.getByRole('progressbar', { name: `Saving ${REL}` });
    await expect(bar).toBeVisible({ timeout: 10_000 });
    await shot(page, 'progress-case-save');
    await page.getByRole('button', { name: `Cancel saving ${REL}` }).click();
    await expect(bar).toBeHidden();
    await page.waitForTimeout(DELAY_MS + 500);
    expect((await readFile(file())).equals(before)).toBe(true);
    await expect(pending).toContainText('physics.nu');
    await expect(page.getByText(`Saved 1 setting to ${REL}`)).toHaveCount(0);

    // not cancelled, the same save writes the change
    await page.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(page.getByText(`Saved 1 setting to ${REL}`)).toBeVisible({ timeout: DELAY_MS + 20_000 });
    expect((await readFile(file(), 'utf8')).includes('nu = 0.02')).toBe(true);
  });

  test('checking the saved file: Cancel shows no result; a check not cancelled shows one', async () => {
    const verdict = page.getByText('The saved file checks out');
    await expect(verdict).toHaveCount(0);
    const check = page.getByRole('button', { name: 'Check the saved file' });
    await check.click();
    const bar = page.getByRole('progressbar', { name: `Checking the saved file ${REL}` });
    await expect(bar).toBeVisible({ timeout: 10_000 });
    await shot(page, 'progress-case-check');
    await page.getByRole('button', { name: `Cancel checking the saved file ${REL}` }).click();
    await expect(bar).toBeHidden();
    await page.waitForTimeout(DELAY_MS + 500);
    await expect(verdict).toHaveCount(0);
    await expect(check).toBeEnabled();

    await check.click();
    await expect(verdict).toBeVisible({ timeout: DELAY_MS + 60_000 });
  });
});

test.describe('the run workspace and the field viewport', () => {
  test.skip(bench === null, NO_BENCH);
  test.skip(mcp === null, NO_MCP);

  const RUN = benchRunId('tiny');
  const OTHER = 'r-20261116T000000-0002';
  let root = '';
  let studio: Studio;
  let page: Page;

  test.beforeAll(async () => {
    test.setTimeout(600_000);
    mkdirSync(SHOTS, { recursive: true });
    root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f832-cancel-'));
    await generate(bench!, root, ['tiny'], 480_000);
    benchProject(root, ['tiny']);
    await copyRun(path.join(root, 'project'), RUN, OTHER);
    studio = await launchStudio({
      RCFD_STUDIO_MCP: mcp!,
      RCFD_STUDIO_DATA_ROOTS: root,
      RCFD_STUDIO_RECENT: path.join(root, 'recent.json'),
      RCFD_STUDIO_STATE_DIR: path.join(root, 'state'),
      RCFD_STUDIO_TEST_DELAY: JSON.stringify({ data: DELAY_MS }),
    });
    page = studio.page;
    await sized(studio);
    await stubDialog(studio.app, path.join(root, 'project'));
    await page.getByRole('button', { name: 'Open project folder…' }).click();
    await page.getByRole('tab', { name: 'Workspace' }).click({ timeout: 30_000 });
    await page.getByRole('listbox', { name: 'Runs of the project' }).waitFor({ timeout: 30_000 });
  });

  test.afterAll(async () => {
    await studio?.app.close().catch(() => undefined);
    for (const p of root ? mcpPids(path.join(root, 'project')) : []) killPid(p);
    if (root) rmSync(root, { recursive: true, force: true, maxRetries: 5 });
  });

  const drawn = (what: string) =>
    until(
      () =>
        page.evaluate(() => {
          const want = document.querySelector('.ws')?.getAttribute('data-snapshot');
          const frame = document.querySelector('.fv-frame');
          return want != null && frame?.getAttribute('data-snapshot') === want && frame.hasAttribute('data-ready') ? { run: document.querySelector('.ws')!.getAttribute('data-run'), snapshot: Number(want) } : null;
        }),
      DELAY_MS * 8 + 60_000,
      what,
    );

  test('opening a run: Cancel goes back to the run selected before, as it was', async () => {
    test.setTimeout(240_000);
    const runs = page.getByRole('listbox', { name: 'Runs of the project' });
    await runs.locator(`[data-run="${RUN}"]`).click();
    await expect(page.locator('.ws')).toHaveAttribute('data-run', RUN, { timeout: 30_000 });
    const shown = await drawn('the first run drawn');
    const canvas = page.locator('.fv-frame canvas').first();
    const picture = await canvas.screenshot();

    await runs.locator(`[data-run="${OTHER}"]`).click();
    const bar = page.getByRole('progressbar', { name: `Opening run ${OTHER}` });
    await expect(bar).toBeVisible({ timeout: 10_000 });
    await shot(page, 'progress-run-open');
    await page.getByRole('button', { name: `Cancel opening run ${OTHER}` }).click();
    await expect(bar).toBeHidden();
    await expect(page.locator('.ws')).toHaveAttribute('data-run', RUN);
    expect(await drawn('the first run again')).toEqual(shown);
    await page.waitForTimeout(DELAY_MS + 500);
    // nothing of the cancelled run arrived later
    await expect(page.locator('.ws')).toHaveAttribute('data-run', RUN);
    expect(await drawn('still the first run')).toEqual(shown);
    expect((await canvas.screenshot()).equals(picture)).toBe(true);
  });

  test('a field load while the line scrubs: Cancel keeps the picture and puts the line back on its stop', async () => {
    test.setTimeout(240_000);
    const shown = await drawn('the run drawn');
    const canvas = page.locator('.fv-frame canvas').first();
    const picture = await canvas.screenshot();
    await page.evaluate(() => {
      const s = document.querySelector('.tp-scrubber[role=slider]') as HTMLElement;
      s.focus();
      s.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true, cancelable: true }));
    });
    await expect(page.locator('.ws')).toHaveAttribute('data-snapshot', String(shown.snapshot - 1));
    const bar = page.getByRole('progressbar', { name: /^Reading p of snapshot / });
    await expect(bar).toBeVisible({ timeout: 10_000 });
    await shot(page, 'progress-field-load');
    await page.getByRole('button', { name: /^Cancel reading p of snapshot / }).click();
    await expect(bar).toBeHidden();
    await expect(page.locator('.ws')).toHaveAttribute('data-snapshot', String(shown.snapshot));
    await page.waitForTimeout(DELAY_MS + 500);
    expect(await drawn('the stop shown before')).toEqual(shown);
    expect((await canvas.screenshot()).equals(picture)).toBe(true);
  });

  test('on the field viewport’s page: Cancel keeps the snapshot shown before', async () => {
    test.setTimeout(240_000);
    const store = path.join(root, 'project', 'runs', RUN, 'fields');
    const go = (snapshot: number) =>
      page.evaluate(([p, s]) => (location.hash = `#/dev/field?route=local&path=${encodeURIComponent(p!)}&field=ux&snapshot=${s}`), [store, String(snapshot)] as const);
    const frameAt = (s: number) =>
      until(() => page.evaluate((want) => (document.querySelector('.fv-frame')?.getAttribute('data-snapshot') === want && document.querySelector('.fv-frame[data-ready]') ? true : null), String(s)), DELAY_MS * 8 + 60_000, `snapshot ${s}`);
    await go(5);
    await frameAt(5);
    await go(9);
    const bar = page.getByRole('progressbar', { name: /^Reading ux of snapshot 10 / });
    await expect(bar).toBeVisible({ timeout: 10_000 });
    await shot(page, 'progress-field-dev');
    await page.getByRole('button', { name: /^Cancel reading ux of snapshot 10 / }).click();
    await expect(bar).toBeHidden();
    await page.waitForTimeout(DELAY_MS + 500);
    await expect(page.locator('.fv-frame')).toHaveAttribute('data-snapshot', '5');
    await expect(page.locator('.fv-frame')).toHaveAttribute('data-ready', /.*/);
  });
});
