// ADR 0737 (F-837) in the packaged Studio, against the real engine (`rcfd serve --ui`): a case file outside any project
// opened with Open a case file (the main process's dialog, stubbed to answer the file), the estimate line from the
// engine's case.facts, Refine x2 (the doubled counts as unsaved changes), the save by the service's case.save (the
// changed lines, the comment kept, no lock file left), a file changed outside shown as a conflict with nothing written,
// an undo of Refine x2 after another writer set a count shown as
// the conflict panel (ADR 0736 P2: the counts it left are checked and sent as base_value), and a mesh-file case refused
// with the engine's reason. Screenshots at 1440 by 900 to test-results-visual/adr0737/.
//
// Needs rcfd (tests/support/rcfd.ts); without it the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { readFile, readdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
test.skip(rcfd === null, NO_RCFD);
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'adr0737');

const PISO = `title = "piso cavity" # a comment the save keeps
[mesh]
generator = "cavity_2d"
nx = 8
ny = 8
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = 0.01
end_time = 1.0
`;

let dir = '';
let file = '';
let service: RunningRcfd;
let studio: Studio;
let page: Page;

const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });

async function openFile(f: string): Promise<string> {
  await stubDialog(studio.app, f);
  await page.evaluate(() => (location.hash = '#/case/file'));
  await expect.poll(() => page.evaluate(() => location.hash), { timeout: 20_000 }).toMatch(/^#\/case\/s\d+$/);
  await expect(page.getByRole('heading', { level: 2, name: f, exact: true })).toBeVisible({ timeout: 20_000 });
  return decodeURIComponent((await page.evaluate(() => location.hash)).replace('#/case/', ''));
}

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  dir = await scratch('adr0737');
  file = path.join(dir, 'piso.toml');
  await writeFile(file, PISO);
  service = await startRcfd(rcfd!, 0, ['--ui']);
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
});

test.afterAll(async () => {
  await studio?.app.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('a case file outside a project opens with no project, and the estimate line is the engine’s', async () => {
  await openFile(file);
  // the case's own dt (it is in the text): Courant 1 m/s * 0.01 s / 0.125 m; memory 64 MB + 64 cells * 4 KB, rounded up
  const line = page.locator('.ce-estimate p[data-words="data"]');
  await expect(line).toHaveText('64 cells · h 0.125 m · dt 0.01 s · 100 steps · Courant 0.08 · 65 MB', { timeout: 20_000 });
  await expect(line).toHaveAttribute('title', /with the runner's default budgets \(no project\)/);
  // a file outside a project has no project-only actions
  await expect(page.getByRole('button', { name: 'Check the saved file' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Duplicate' })).toHaveCount(0);
  await shot('01-opened-estimate');
});

test('Refine x2 doubles the counts as unsaved changes; the estimate follows; Save writes the lines and keeps the comment', async () => {
  await page.getByRole('button', { name: 'Refine the mesh x2', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Refined x2' })).toContainText('64 → 256 cells');
  await expect(page.getByRole('list', { name: 'Settings a save writes' })).toContainText('mesh.nx');
  await expect(page.locator('.ce-estimate p[data-words="data"]')).toContainText('256 cells');
  await shot('02-refined');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText('Saved 2 settings to')).toBeVisible({ timeout: 20_000 });
  const text = await readFile(file, 'utf8');
  expect(text).toContain('nx = 16\nny = 16\n');
  expect(text).toContain('# a comment the save keeps');
  expect((await readdir(dir)).filter((f) => f.endsWith('.rcfd-lock'))).toEqual([]);
  await shot('03-saved');
});

test('a file changed outside is a conflict: both sides shown, nothing written', async () => {
  const outside = (await readFile(file, 'utf8')).replace('nu = 0.01', 'nu = 0.02');
  await writeFile(file, outside);
  await page.getByRole('button', { name: 'Refine the mesh x2', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Refined x2' })).toContainText('256 → 1024 cells');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('heading', { name: `Not saved: ${file} changed outside the Studio` })).toBeVisible({ timeout: 20_000 });
  expect(await readFile(file, 'utf8')).toBe(outside);
  await shot('04-conflict');
  await page.getByRole('button', { name: 'Write my changes into the file as it is now' }).click();
  await expect.poll(async () => readFile(file, 'utf8'), { timeout: 20_000 }).toContain('nx = 32');
  expect(await readFile(file, 'utf8')).toContain('nu = 0.02');
});

test('undo of Refine x2 checks the counts it left: another writer’s count is a conflict with three choices', async () => {
  // a refine (nx, ny 32 -> 64) from the session the write left; another writer (a second connection) sets ny afterwards
  const session = decodeURIComponent((await page.evaluate(() => location.hash)).replace('#/case/', ''));
  const rpc = await RpcConnection.connectTcp('127.0.0.1', service.port);
  try {
    const counts = async () => Promise.all(['mesh.nx', 'mesh.ny'].map(async (p) => ((await rpc.request('settings.get', { session, path: p })) as { value: unknown }).value));
    expect(await counts()).toEqual([32, 32]);
    await page.getByRole('button', { name: 'Refine the mesh x2', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: 'Refined x2' })).toContainText('1024 → 4096 cells');
    const undo = page.getByRole('group', { name: /^Undo and redo/ }).getByRole('button').first();
    await expect(undo).toHaveText('Undo: refine the mesh x2');
    await rpc.request('settings.set', { session, path: 'mesh.ny', value: 70 });
    await undo.click();
    const d = page.getByRole('alertdialog');
    await expect(d.getByRole('heading')).toHaveText('Not undone: changed outside the Studio');
    await expect(d.getByRole('row', { name: /mesh\.ny/ })).toContainText('64');
    await expect(d.getByRole('row', { name: /mesh\.ny/ })).toContainText('70');
    await expect(d.getByRole('row', { name: /mesh\.nx/ })).toHaveCount(0);
    await expect(d.getByRole('button')).toHaveText(['Undo anyway, replacing the value now', 'Keep the value now and drop this step', 'Cancel']);
    await shot('05-refine-undo-conflict');
    await d.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByRole('alertdialog')).toHaveCount(0);
    expect(await counts()).toEqual([64, 70]);
    // undo anyway: both counts back to what the refine found
    await undo.click();
    await page.getByRole('alertdialog').getByRole('button', { name: 'Undo anyway, replacing the value now' }).click();
    await expect.poll(counts, { timeout: 20_000 }).toEqual([32, 32]);
    await expect(undo).toHaveText('Nothing to undo');
  } finally {
    rpc.close();
  }
});

test('Refine x2 of a mesh-file case is refused with the engine’s reason', async () => {
  const f = path.join(dir, 'file-mesh.toml');
  await writeFile(f, PISO.replace('generator = "cavity_2d"\nnx = 8\nny = 8\n', 'source = "file"\nfile = "m.msh"\n'));
  const opened = await (async () => {
    await stubDialog(studio.app, f);
    await page.evaluate(() => (location.hash = '#/case/file'));
    await expect.poll(() => page.evaluate(() => location.hash), { timeout: 20_000 }).not.toBe('#/case/file');
    return page.evaluate(() => location.hash);
  })();
  if (!/^#\/case\/s\d+$/.test(opened)) {
    // the engine refuses the session itself (the mesh file is not there): the open says why
    await expect(page.getByRole('heading', { name: 'Failed to open the case file' })).toBeVisible();
    return;
  }
  await page.getByRole('button', { name: 'Refine the mesh x2', exact: true }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Refine x2 refused' })).toContainText('mesh file');
  await shot('06-refused');
});
