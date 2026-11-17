// The mesh preview of the case editor (F-836, ADR 0732) in the packaged Studio, against the real engine started with
// --ui as the Studio starts it (`rcfd serve --ui`, `rcfd-mcp --project`): a generator mesh, a mesh file and a
// [mesh.adapt] case are drawn before any run, through `mesh.preview` and the store the engine writes into the
// project's .rcfd/previews/<mesh hash>/; patch visibility; an edit of the session (not saved) draws the new mesh; a mesh
// that cannot be built shows the engine's located message. The case editor's visible chrome words at rest stay within
// its G22 budget with the preview drawn. Screenshots at 1440 by 900 to test-results-visual/f836/.
//
// Needs rcfd and rcfd-mcp (tests/support/rcfd.ts, tests/support/project.ts); without them the tests are skipped.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import path from 'node:path';
import { WORD_BUDGETS } from '../../src/shared/word-budget.js';
import { copyExample, findMcp, mcpPids, NO_MCP, repoRoot, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, killPid, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';
import { countWords } from '../support/word-count.js';

const rcfd = findRcfd();
const mcp = findMcp();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f836');

let dir = '';
let quick = '';
let service: RunningRcfd;
let studio: Studio;
let page: Page;

const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const preview = (): Locator => page.locator('.mp');
const patches = (): Locator => page.getByRole('list', { name: 'Patches shown' });

async function openFromHome(rel: string): Promise<void> {
  await page.evaluate(() => (location.hash = ''));
  const edit = page.getByRole('button', { name: `Edit ${rel}`, exact: true });
  await expect(edit).toBeVisible({ timeout: 20_000 });
  await edit.click();
  await expect(page.getByRole('heading', { level: 2, name: rel, exact: true })).toBeVisible({ timeout: 30_000 });
}

/** The mesh hash on screen once the preview has drawn (and its canvas is there). */
async function drawn(): Promise<string> {
  await expect(preview()).toHaveAttribute('data-mesh-hash', /^[0-9a-f]{16}$/, { timeout: 60_000 });
  await expect(preview().locator('.mp-view canvas')).toBeVisible({ timeout: 30_000 });
  await expect(preview()).toHaveAttribute('aria-busy', 'false', { timeout: 60_000 });
  // the canvas's first picture of these triangles is on screen
  await expect(preview()).toHaveAttribute('data-drawn', 'true', { timeout: 30_000 });
  return (await preview().getAttribute('data-mesh-hash'))!;
}

const triangles = async (): Promise<number> => Number(await preview().getAttribute('data-triangles'));

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  dir = await scratch('f836');
  quick = await copyExample('quick-start', dir);
  // a [mesh.adapt] case, a mesh file, and a mesh file that is not there
  copyFileSync(path.join(repoRoot, 'cases', 'cavity', 'adapt_re100.toml'), path.join(quick, 'cases', 'adaptive.toml'));
  copyFileSync(path.join(repoRoot, 'crates', 'rcfd-io', 'tests', 'data', 'hex_v4.1_ascii.msh'), path.join(quick, 'meshes', 'hex.msh'));
  const fileCase = (file: string) => `title = "A mesh file"\n\n[mesh]\nsource = "file"\nfile = "${file}"\n\n[physics]\nnu = 0.01\n`;
  writeFileSync(path.join(quick, 'cases', 'file-mesh.toml'), fileCase('../meshes/hex.msh'));
  writeFileSync(path.join(quick, 'cases', 'missing-mesh.toml'), fileCase('../meshes/nowhere.msh'));
  service = await startRcfd(rcfd!, 0, ['--ui']);
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`, RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.locator('tr[data-path="cases/cavity-re100.toml"]')).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  await studio?.app.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  if (quick) for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('a generator mesh is drawn before any run, from the store the engine wrote; its words stay within the budget', async () => {
  await openFromHome('cases/cavity-re100.toml');
  const hash = await drawn();
  // the engine's store, named by the mesh hash, in the project's .rcfd/previews
  const store = path.join(quick, '.rcfd', 'previews', hash, 'store.rts');
  expect(existsSync(store)).toBe(true);
  expect(readFileSync(store).subarray(0, 8).toString('latin1')).toBe('RCFDTSS\0');
  await expect(preview().locator('.mp-facts')).toContainText('1,024 cells');
  await expect(patches().getByRole('listitem')).toHaveCount(3);
  await expect(patches()).toContainText('lid');
  await expect(patches()).toContainText('frontAndBack');
  // 32 x 32 hexahedra: every boundary quadrilateral is two triangles (lid 32, walls 96, front and back 2 x 1024)
  expect(await triangles()).toBe(2 * (32 + 96 + 2048));
  // ADR 0801 Addendum 22: the 2D cavity's mesh is drawn orthographic, straight along its thin axis (z), framed
  const canvas = preview().locator('.mp-view canvas');
  await expect(canvas).toHaveAttribute('data-camera', 'orthographic');
  await expect(canvas).toHaveAttribute('data-camera-forward', '0.00000 0.00000 -1.00000');
  await expect(canvas).toHaveAttribute('data-camera-moved', 'false');
  await page.mouse.move(1439, 899);
  await shot('case-editor-preview');
  // it pans with the right button and zooms with the wheel, and never turns
  const box = (await canvas.boundingBox())!;
  const cx = Math.round(box.x + box.width / 2);
  const cy = Math.round(box.y + box.height / 2);
  const eye0 = (await canvas.getAttribute('data-camera-eye'))!;
  const zoom0 = Number(await canvas.getAttribute('data-camera-zoom'));
  await page.mouse.move(cx, cy);
  await page.mouse.down({ button: 'right' });
  await page.mouse.move(cx + 40, cy + 20, { steps: 4 });
  await page.mouse.up({ button: 'right' });
  await expect(canvas).toHaveAttribute('data-camera-moved', 'true');
  await expect(canvas).toHaveAttribute('data-camera-forward', '0.00000 0.00000 -1.00000');
  expect(await canvas.getAttribute('data-camera-eye')).not.toBe(eye0);
  await page.mouse.wheel(0, -200);
  await expect.poll(async () => Number(await canvas.getAttribute('data-camera-zoom'))).toBeLessThan(zoom0);
  await page.keyboard.down('Alt');
  await page.mouse.down();
  await page.mouse.move(cx + 90, cy + 60, { steps: 4 });
  await page.mouse.up();
  await page.keyboard.up('Alt');
  await expect(canvas).toHaveAttribute('data-camera-forward', '0.00000 0.00000 -1.00000');
  await page.mouse.move(1439, 899);
  await shot('case-editor-preview-panned');
  const c = await countWords(page, { root: '.st-page', exclude: ['.hi-controls', '.wk'] });
  const budget = WORD_BUDGETS['case-editor'].budget;
  console.log(`F-836: case editor with the preview drawn, ${c.count} chrome words (budget ${budget}): ${c.words.map((w) => w.word).join(' ')}`);
  expect(c.found).toBe(true);
  expect(c.count, c.words.map((w) => w.word).join(' ')).toBeLessThanOrEqual(budget);
  // hiding the front and back shows the walls and the lid only
  await patches().getByRole('checkbox', { name: /frontAndBack/ }).uncheck();
  await expect.poll(triangles).toBe(2 * (32 + 96));
  await expect(preview()).toHaveAttribute('data-drawn', 'true', { timeout: 30_000 });
  await shot('case-editor-preview-front-hidden');
  await patches().getByRole('checkbox', { name: /frontAndBack/ }).check();
  await expect.poll(triangles).toBe(2 * (32 + 96 + 2048));
});

test('an edit of the session, not saved, draws the new mesh', async () => {
  const before = await drawn();
  const find = page.getByRole('textbox', { name: 'Find a setting' });
  await find.fill('mesh.nx');
  const field = page.locator('.st-key[data-path="mesh.nx"]').getByRole('textbox');
  await field.fill('40');
  await field.press('Enter');
  await expect(page.getByRole('list', { name: 'Settings a save writes' })).toContainText('mesh.nx');
  await find.fill('');
  await expect(preview()).not.toHaveAttribute('data-mesh-hash', before, { timeout: 30_000 });
  await drawn();
  await expect(preview().locator('.mp-facts')).toContainText('1,280 cells');
  // nothing was saved: the file still has nx = 32
  expect(readFileSync(path.join(quick, 'cases', 'cavity-re100.toml'), 'utf8')).toMatch(/nx = 32/);
});

test('a mesh file is drawn with its own patches', async () => {
  await openFromHome('cases/file-mesh.toml');
  await drawn();
  await expect(patches()).toContainText('inlet');
  await expect(patches()).toContainText('outlet');
  await expect(patches()).toContainText('walls');
  expect(await triangles()).toBeGreaterThan(0);
  await page.mouse.move(1439, 899);
  await shot('file-mesh-preview');
});

test('a [mesh.adapt] case shows the mesh the run starts on, and the adapted mesh on request with its cost said first', async () => {
  await openFromHome('cases/adaptive.toml');
  const base = await drawn();
  await expect(preview().locator('.mp-facts')).toContainText('576 cells');
  await expect(preview()).toContainText('the run starts on this mesh and adapts it in its first step');
  const show = page.getByRole('button', { name: 'Show the adapted mesh (3 solves)' });
  await expect(show).toBeVisible();
  await page.mouse.move(1439, 899);
  await shot('adaptive-base');
  await show.click();
  await expect(preview().locator('.mp-facts')).toContainText('2,799 cells', { timeout: 90_000 });
  expect(await drawn()).not.toBe(base);
  await page.mouse.move(1439, 899);
  await expect(preview()).toContainText('the mesh after the run’s first step (3 solves)');
  await shot('adaptive-final');
  await page.getByRole('button', { name: 'Show the starting mesh' }).click();
  await expect(preview().locator('.mp-facts')).toContainText('576 cells', { timeout: 30_000 });
});

test('a mesh that cannot be built shows the engine’s located message', async () => {
  await openFromHome('cases/missing-mesh.toml');
  const alert = preview().getByRole('alert');
  await expect(alert).toContainText('Mesh preview failed', { timeout: 30_000 });
  await expect(alert).toContainText('mesh.file');
  await expect(alert).toContainText('nowhere.msh');
  await shot('missing-mesh');
});
