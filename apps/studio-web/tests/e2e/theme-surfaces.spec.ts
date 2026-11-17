// F-818: the main surfaces in each theme, in the packaged Studio at 1440 by 900 against the real engine (rcfd serve,
// rcfd-mcp, the fake claude for the Claude Code panel), screenshotted to test-results-visual/f818/<theme>/ for the
// maintainer's view-and-report (G13). On the run workspace the three.js canvases are checked too: the field viewport's
// ground and the plots' ground are the theme's tokens, redrawn at once on a change, while the field keeps its colour
// map (the same pixels inside the frame in every theme, except where the ground shows). The theme is chosen through the
// palette's Theme commands; the setting is kept in a scratch file (RCFD_STUDIO_PREFERENCES).
//
// Needs rcfd and rcfd-mcp (tests/support/rcfd.ts, tests/support/project.ts); without them the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import path from 'node:path';
import { themes, type ThemeName } from '../../src/renderer/design/tokens.js';
import { claudeFixture, FAKE_CLAUDE, type ClaudeFixture } from '../support/claude.js';
import { decodePng, pixel, type Rgba } from '../support/png.js';
import { copyExample, findMcp, mcpPids, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, killPid, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
const mcp = findMcp();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 180_000 });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f818');
const THEMES: readonly ThemeName[] = ['light', 'dark', 'contrast'];
const TITLE: Record<ThemeName, string> = { light: 'Theme: Light', dark: 'Theme: Dark', contrast: 'Theme: High contrast' };
/** The quick-start run of cases/cavity-re100.toml (converged, 24 stored snapshots). */
const RE100 = 'r-20261028T152641-f8c2';

let dir = '';
let quick = '';
let fx: ClaudeFixture;
let service: RunningRcfd;
let studio: Studio;
let page: Page;

const frames = () => page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));

/** Chooses the theme through the palette and waits until the page draws it. */
async function useTheme(t: ThemeName): Promise<void> {
  await page.keyboard.press('Control+K');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  await expect(dialog).toBeVisible();
  await page.keyboard.type(TITLE[t]);
  await expect(dialog.getByRole('option').first()).toContainText(TITLE[t]);
  await page.keyboard.press('Enter');
  await expect(dialog).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => document.documentElement.dataset['theme'])).toBe(t);
  await frames();
  await frames();
}

async function shoot(t: ThemeName, name: string): Promise<{ img: Rgba; dpr: number }> {
  // the focus ring and a hover tooltip are not the surface at rest
  await page.mouse.move(1439, 899);
  await frames();
  const buf = await page.screenshot({ path: path.join(SHOTS, t, `${name}.png`), scale: 'css' });
  return { img: decodePng(buf), dpr: 1 };
}

/** The surface in every theme, back to Light at the end. */
async function everyTheme(name: string, check?: (t: ThemeName, shot: { img: Rgba; dpr: number }) => Promise<void>): Promise<void> {
  for (const t of THEMES) {
    await useTheme(t);
    const s = await shoot(t, name);
    if (check) await check(t, s);
  }
  await useTheme('light');
}

const rgb = (hex: string): [number, number, number] => {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};
const near = (p: readonly number[], q: readonly number[], tol: number): boolean => p.every((v, i) => i > 2 || Math.abs(v - q[i]!) <= tol);

/** The most frequent colour inside a box of the screenshot (the ground of a plot, under its lines and labels). */
function modeIn(img: Rgba, box: { x: number; y: number; width: number; height: number }): [number, number, number] {
  const counts = new Map<string, number>();
  for (let y = Math.ceil(box.y) + 2; y < Math.floor(box.y + box.height) - 2; y += 2) {
    for (let x = Math.ceil(box.x) + 2; x < Math.floor(box.x + box.width) - 2; x += 2) {
      const p = pixel(img, x, y);
      const k = `${p[0]},${p[1]},${p[2]}`;
      counts.set(k, (counts.get(k) ?? 0) + 1);
    }
  }
  const top = [...counts.entries()].sort((a, b) => b[1] - a[1])[0]![0];
  return top.split(',').map(Number) as [number, number, number];
}

test.beforeAll(async () => {
  for (const t of THEMES) mkdirSync(path.join(SHOTS, t), { recursive: true });
  dir = await scratch('f818');
  quick = await copyExample('quick-start', dir);
  fx = await claudeFixture('f818');
  service = await startRcfd(rcfd!);
  studio = await launchStudio({
    RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`,
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_RECENT: path.join(dir, 'recent.json'),
    RCFD_STUDIO_PREFERENCES: path.join(dir, 'studio-preferences.json'),
    RCFD_STUDIO_CLAUDE: FAKE_CLAUDE,
    RCFD_STUDIO_CLAUDE_STATE: fx.state,
    ...fx.env(),
  });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
});

test.afterAll(async () => {
  await studio?.app.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
  await rm(fx.root, { recursive: true, force: true }).catch(() => undefined);
});

test('the project home with no project open, and the Studio’s settings page', async () => {
  await expect(page.getByRole('button', { name: 'Open project folder…' })).toBeEnabled();
  await everyTheme('home-empty');
  await page.locator('.ph-top').getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('radiogroup', { name: 'Theme' })).toBeVisible();
  await everyTheme('studio-settings');
  await page.getByRole('button', { name: 'Close settings' }).click();
  await expect(page.getByRole('heading', { name: 'No project open' })).toBeVisible();
});

test('the project home of an open project, with the Claude Code panel beside it', async () => {
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  await expect(page.locator('.ph-group[data-group="runs"] tr[data-run]').first()).toBeVisible({ timeout: 20_000 });
  const panel = page.getByRole('complementary', { name: 'Claude Code' });
  await expect(panel.getByRole('list', { name: 'Claude Code sessions of this project' })).toBeVisible({ timeout: 20_000 });
  await everyTheme('home-open');
});

test('the Runs view with a run selected', async () => {
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible({ timeout: 20_000 });
  await page.locator(`.rv-case[data-run="${RE100}"]`).click();
  const aside = page.locator(`aside.rv-run[data-run="${RE100}"]`);
  await expect(aside).toBeVisible();
  await expect(aside.locator('.rv-history, .rv-empty-line').first()).toBeVisible({ timeout: 20_000 });
  await everyTheme('runs-selected');
});

test('the run workspace: the field viewport and the plots follow the theme, the colour map does not', async () => {
  await page.getByRole('tab', { name: 'Workspace' }).click();
  const list = page.getByRole('listbox', { name: 'Runs of the project' });
  await expect(list).toBeVisible({ timeout: 20_000 });
  await list.locator(`[data-run="${RE100}"]`).click();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', RE100);
  await expect(page.locator('.fv-frame[data-ready]')).toHaveCount(1, { timeout: 30_000 });
  await expect(page.locator('.plot-canvas').first()).toBeVisible({ timeout: 20_000 });
  const frame = (await page.locator('.fv-frame').boundingBox())!;
  const plot = (await page.locator('.plot-canvas').first().boundingBox())!;
  /** The pixels of the field inside the frame (away from its edge), in the light theme, for the colour-map check. */
  let field: Array<readonly number[]> | null = null;
  const inside: Array<[number, number]> = [];
  for (let i = 1; i < 8; i++) for (let j = 1; j < 8; j++) inside.push([Math.round(frame.x + (frame.width * i) / 8), Math.round(frame.y + (frame.height * j) / 8)]);
  const bad: string[] = [];
  await everyTheme('workspace', async (t, { img }) => {
    // the viewport's ground (the frame's corner) is the theme's silver-wash, exactly (FieldCanvas writes it unconverted)
    const g = pixel(img, frame.x + 6, frame.y + 6);
    if (!near(g, rgb(themes[t]['silver-wash']), 0)) bad.push(`${t}: field ground ${g.slice(0, 3)} is not ${themes[t]['silver-wash']}`);
    // the plot's ground is the theme's ground
    const m = modeIn(img, plot);
    if (!near(m, rgb(themes[t].white), 2)) bad.push(`${t}: plot ground ${m} is not ${themes[t].white}`);
    // inside the frame, a pixel that is not the ground is the colour map's, the same in every theme
    const now = inside.map(([x, y]) => pixel(img, x, y));
    if (field === null) field = now;
    else {
      now.forEach((p, k) => {
        const was = field![k]!;
        const ground = near(was, rgb(themes.light['silver-wash']), 0) || near(p, rgb(themes[t]['silver-wash']), 0);
        if (!ground && !near(p, was, 0)) bad.push(`${t}: field pixel ${inside[k]} ${p.slice(0, 3)}, light ${was.slice(0, 3)}`);
      });
    }
  });
  expect(bad).toEqual([]);
});

test('the command palette, opened', async () => {
  for (const t of THEMES) {
    await useTheme(t);
    await page.keyboard.press('Control+K');
    const dialog = page.getByRole('dialog', { name: 'Command palette' });
    await expect(dialog.getByRole('option').first()).toBeVisible();
    await page.screenshot({ path: path.join(SHOTS, t, 'palette.png'), scale: 'css' });
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
  }
  await useTheme('light');
});

test('the case editor on a case file', async () => {
  await page.getByRole('tab', { name: 'Home' }).click();
  const edit = page.getByRole('button', { name: 'Edit cases/cavity-re100.toml', exact: true });
  await expect(edit).toBeVisible({ timeout: 20_000 });
  await edit.click();
  await expect(page.getByRole('heading', { level: 2, name: 'cases/cavity-re100.toml', exact: true })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible({ timeout: 20_000 });
  await everyTheme('case-editor');
});

test('the case settings of a session, with the Settings icon in their header', async () => {
  await page.evaluate(() => (location.hash = '#/settings'));
  await page.getByRole('button', { name: 'New session from defaults' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible({ timeout: 20_000 });
  await expect(page.locator('.st-top').getByRole('button', { name: 'Settings', exact: true })).toBeVisible();
  await everyTheme('settings');
});
