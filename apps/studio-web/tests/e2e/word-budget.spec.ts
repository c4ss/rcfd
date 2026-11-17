// F-833 (G22, The Quiet Rule): every built surface at rest, in the packaged Studio at 1440 by 900 against the real
// engine (rcfd serve for the sessions and the schema, rcfd-mcp for the project), with its visible chrome words counted from the DOM (tests/support/word-count.ts, by the definition in
// src/shared/word-budget.ts) and held to the surface's budget there. Each surface is screenshotted to
// test-results-visual/f833/<phase>/ (RCFD_WORD_PHASE, default "after") and the counts, with every counted word, are
// written to counts.json beside them, for the maintainer's view-and-report (G13).
//
// The Studio settings are counted with no sites file (studio-settings), then with a sites file of two ssh sites
// (studio-settings-sites, the Sites section alone: its heading plus one line per site; the maintainer's answer of
// 2026-11-17, ADR 0801 Q-RL-1). The file is removed again before the project surfaces, which a site would change.
//
// Needs rcfd and rcfd-mcp (tests/support/rcfd.ts, tests/support/project.ts); without them the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import path from 'node:path';
import { budgetOf, WORD_BUDGETS, type SurfaceId } from '../../src/shared/word-budget.js';
import { copyExample, emptyProject, findMcp, mcpPids, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, killPid, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';
import { countWords, type SurfaceRoot, type WordCount } from '../support/word-count.js';

const rcfd = findRcfd();
const mcp = findMcp();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

const PHASE = process.env['RCFD_WORD_PHASE'] || 'after';
const SHOTS = path.join(appRoot, 'test-results-visual', 'f833', PHASE);
/** The quick-start run of cases/cavity-re100.toml (converged, 24 stored snapshots) and the one of cavity-re400.toml. */
const RE100 = 'r-20261028T152641-f8c2';
const RE400 = 'r-20261028T152641-97f1';
/** Long enough for the opening's progress row (shown after PROGRESS_AFTER_MS, 300 ms) to be photographed. */
const OPEN_DELAY_MS = 2500;

let dir = '';
let sitesFile = '';
let quick = '';
let empty = '';
let service: RunningRcfd;
let studio: Studio;
let page: Page;
const counts: Record<string, WordCount & { budget: number }> = {};

/** Every surface's root (and the roots nested in it that are counted on their own). */
const ROOTS: Record<SurfaceId, SurfaceRoot> = {
  'home-empty': { root: '.ph-page', exclude: ['.fp', '.wk'] },
  'first-path-intro': { root: '.fp--intro' },
  'home-open': { root: '.ph-page', exclude: ['.fp', '.wk'] },
  'first-path': { root: '.fp--open' },
  runs: { root: '.rv', exclude: ['.rv-run'] },
  'runs-selected': { root: '.rv-run', exclude: ['.pv-pop'] },
  workspace: { root: '.ws', exclude: ['.fv-derived', '.wk'] },
  compare: { root: '.ws', exclude: ['.fv-derived', '.wk', '.ws-runs', '.tp'] },
  'derived-controls': { root: '.fv-derived' },
  provenance: { root: '.pv-pop' },
  palette: { root: '.cp-dialog' },
  settings: { root: '.st-page', exclude: ['.hi-controls', '.hi-panel'] },
  'case-editor': { root: '.st-page', exclude: ['.hi-controls', '.wk'] },
  undo: { root: '.hi-controls', exclude: ['.hi-panel'] },
  decision: { root: '.hi-panel' },
  progress: { root: '.wk' },
  'studio-settings': { root: '.ss-page', exclude: ['.ss-sites'] },
  'studio-settings-sites': { root: '.ss-sites' },
};

/** Counts the surface and screenshots the window (the budgets are held by the last test, once every surface is counted). */
async function atRest(id: SurfaceId, items = 0): Promise<void> {
  // the focus ring and a hover tooltip are not the surface at rest
  await page.mouse.move(1439, 899);
  await page.screenshot({ path: path.join(SHOTS, `${id}.png`), scale: 'css' });
  const c = await countWords(page, ROOTS[id]);
  counts[id] = { ...c, budget: budgetOf(WORD_BUDGETS[id], items) };
  expect(c.found, `${id}: ${ROOTS[id].root} is on the page`).toBe(true);
}

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  dir = await scratch('f833');
  quick = await copyExample('quick-start', dir);
  empty = await emptyProject(dir, 'An empty project');
  // absent until the Sites section is counted (the Studio looks for it each time a page asks)
  sitesFile = path.join(dir, 'sites.toml');
  service = await startRcfd(rcfd!);
  studio = await launchStudio({
    RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`,
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_RECENT: path.join(dir, 'recent.json'),
    RCFD_SITES_FILE: sitesFile,
    RCFD_STUDIO_TEST_DELAY: JSON.stringify({ projectOpen: OPEN_DELAY_MS }),
  });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
});

test.afterAll(async () => {
  writeFileSync(path.join(SHOTS, 'counts.json'), JSON.stringify(counts, null, 2) + '\n');
  await studio?.app.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  for (const f of [quick, empty]) for (const p of mcpPids(f)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('the project home with no project open, and the first path’s four stations', async () => {
  await expect(page.getByRole('button', { name: 'Open project folder…' })).toBeEnabled();
  await atRest('home-empty');
  await atRest('first-path-intro');
  // F-818: the Studio's settings page, from Ctrl+, and back
  await page.keyboard.press('Control+,');
  await expect(page.getByRole('radiogroup', { name: 'Theme' })).toBeVisible();
  await atRest('studio-settings');
  await page.getByRole('button', { name: 'Close settings' }).click();
  await expect(page.getByRole('button', { name: 'Open project folder…' })).toBeEnabled();
  // the Sites section, with two sites listed (no site is reached: ssh sites are only listed here)
  const site = (name: string) => `[site.${name}]
kind = "ssh"
host = "me@${name}.example.org"
engine = "/opt/rcfd/bin"
work = "/scratch/me/rcfd"
`;
  writeFileSync(sitesFile, `${site('lab')}
${site('cluster')}`);
  await page.keyboard.press('Control+,');
  const sites = page.getByRole('region', { name: 'Sites' });
  await expect(sites.locator('.ss-site')).toHaveCount(2, { timeout: 20_000 });
  await atRest('studio-settings-sites', 2);
  await page.getByRole('button', { name: 'Close settings' }).click();
  await expect(page.getByRole('button', { name: 'Open project folder…' })).toBeEnabled();
  rmSync(sitesFile);
});

test('the progress row of a project opening that lasts', async () => {
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('progressbar', { name: /^Opening project / })).toBeVisible({ timeout: 10_000 });
  await atRest('progress');
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
});

test('the project home of an open project', async () => {
  await expect(page.locator('.ph-group[data-group="runs"] tr[data-run]').first()).toBeVisible({ timeout: 20_000 });
  // ADR 0801 Addendum 23: Use with Claude Code and Open Claude Code here are icons in the band, with no word at rest
  await expect(page.getByRole('button', { name: 'Use with Claude Code' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open Claude Code here' })).toBeVisible();
  await atRest('home-open');
});

test('the Runs view, then a run selected, then the provenance of its step count', async () => {
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible({ timeout: 20_000 });
  await atRest('runs');
  await page.locator(`.rv-case[data-run="${RE100}"]`).click();
  const aside = page.locator(`aside.rv-run[data-run="${RE100}"]`);
  await expect(aside).toBeVisible();
  await expect(aside.locator('.rv-history, .rv-empty-line').first()).toBeVisible({ timeout: 20_000 });
  await atRest('runs-selected');
  await aside.getByRole('button', { name: /^Provenance of (iterations|steps): / }).first().click();
  await expect(page.getByRole('dialog', { name: /^Provenance of / })).toBeVisible();
  await expect(page.locator('.pv-cmd')).not.toBeEmpty({ timeout: 20_000 });
  await atRest('provenance');
  await page.keyboard.press('Escape');
  await expect(page.locator('.pv-pop')).toHaveCount(0);
});

test('the run workspace on a converged run, its derived-picture controls, and compare mode', async () => {
  await page.getByRole('tab', { name: 'Workspace' }).click();
  const list = page.getByRole('listbox', { name: 'Runs of the project' });
  await expect(list).toBeVisible({ timeout: 20_000 });
  await list.locator(`[data-run="${RE100}"]`).click();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', RE100);
  await expect(page.locator('.fv-frame[data-ready]')).toHaveCount(1, { timeout: 30_000 });
  await expect(page.locator('.fv-derived')).toBeVisible({ timeout: 20_000 });
  await atRest('workspace');
  await atRest('derived-controls');
  const toggle = page.getByRole('button', { name: 'Compare runs', exact: true });
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: /Compared run/ }).click();
  await page.locator('.ui-popover').getByRole('option', { name: new RegExp(RE400) }).click();
  await page.getByRole('radio', { name: 'Side by side', exact: true }).click();
  // a snapshot time both runs stored (as compare.spec.ts does): the eighth stop
  await page.getByRole('slider', { name: /^Time of / }).focus();
  await page.keyboard.press('Home');
  for (let i = 0; i < 7; i++) await page.keyboard.press('ArrowRight');
  await expect(page.locator('.cmp-view[data-view="b"] .fv-frame[data-ready]')).toHaveCount(1, { timeout: 30_000 });
  await atRest('compare');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
});

test('the command palette, opened with nothing typed', async () => {
  await page.keyboard.press('Control+K');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('option').first()).toBeVisible();
  await atRest('palette');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

test('the case editor on a case file, nothing changed, and its undo controls', async () => {
  await page.getByRole('tab', { name: 'Home' }).click();
  const edit = page.getByRole('button', { name: 'Edit cases/cavity-re100.toml', exact: true });
  await expect(edit).toBeVisible({ timeout: 20_000 });
  await edit.click();
  await expect(page.getByRole('heading', { level: 2, name: 'cases/cavity-re100.toml', exact: true })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible({ timeout: 20_000 });
  await atRest('case-editor');
  await atRest('undo');
});

test('the settings of a session from the engine’s defaults, and the decision Close session asks for', async () => {
  await page.evaluate(() => (location.hash = '#/settings'));
  await page.getByRole('button', { name: 'New session from defaults' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible({ timeout: 20_000 });
  await atRest('settings');
  await page.getByRole('button', { name: 'Close session' }).click();
  const panel = page.getByRole('alertdialog', { name: /^Close session / });
  await expect(panel).toBeVisible();
  await atRest('decision');
  await panel.getByRole('button', { name: 'Keep the session' }).click();
  await expect(panel).toHaveCount(0);
});

test('the first path on a project that holds nothing yet', async () => {
  await page.evaluate(() => (location.hash = ''));
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  await stubDialog(studio.app, empty);
  await page.getByRole('button', { name: 'Open project…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'An empty project' })).toBeVisible({ timeout: 30_000 });
  await expect(page.locator('.fp--open .fp-primary button')).toBeEnabled({ timeout: 20_000 });
  await atRest('first-path');
});

test('every surface at rest is within its budget of visible chrome words', () => {
  expect(Object.keys(counts).sort(), 'every surface of the table was counted').toEqual(Object.keys(WORD_BUDGETS).sort());
  const over = Object.entries(counts)
    .filter(([, c]) => c.count > c.budget)
    .map(([id, c]) => `${id}: ${c.count} words (budget ${c.budget}): ${c.words.map((w) => w.word).join(' ')}`);
  expect(over, 'surfaces past their budget').toEqual([]);
});
