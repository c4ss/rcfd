// The settings surface with the engine's UI metadata (F-826, ADR 0730) against the real `rcfd serve --ui`: keys named
// by the engine's labels, the advanced keys one step away behind Show all (G15), and a key's focus detail with the
// engine's help line and a usual range with its source. Screenshots at 1440 by 900 to test-results-visual/f826/ for the
// maintainer's view-and-report.
//
// Needs the rcfd binary (tests/support/rcfd.ts); without it the tests are skipped and say why.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { appRoot, findRcfd, killIfRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
test.skip(rcfd === null, NO_RCFD);
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f826');

let service: RunningRcfd;
let studio: Studio;
let page: Page;

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  service = await startRcfd(rcfd!, 0, ['--ui']);
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}` });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await page.evaluate(() => {
    location.hash = '#/settings';
  });
  await page.getByRole('button', { name: 'New session from defaults' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible();
});

test.afterAll(async () => {
  await studio?.app.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
});

const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const row = (p: string): Locator => page.locator(`.st-key[data-path="${p}"]`);

async function openSection(name: string): Promise<void> {
  await page.getByRole('listbox', { name: 'Sections of the case' }).getByRole('option', { name: new RegExp(`^${name}\\b`) }).click();
  await expect(page.getByRole('heading', { level: 2, name, exact: true })).toBeVisible();
}

test('solver: common keys by the engine’s labels at rest, the advanced ones behind Show all', async () => {
  await openSection('solver');
  const alpha = row('solver.alpha_u');
  await expect(alpha).toBeVisible();
  // the engine's label names the row, not the key's last segment
  await expect(alpha.locator('.st-key-name')).not.toHaveText('alpha_u');
  const showAll = page.getByRole('button', { name: /^Show all: \d+ advanced keys? of solver$/ });
  await expect(showAll).toBeVisible();
  await shot('01-solver-at-rest');
  await showAll.click();
  await expect(page.getByRole('button', { name: /^\d+ advanced keys? of solver$/ })).toHaveAttribute('aria-expanded', 'true');
  await shot('02-solver-show-all');
});

test('a key’s focus detail: the engine’s help line and the docs heading', async () => {
  await openSection('solver');
  await row('solver.alpha_u').getByRole('textbox').focus();
  await expect(row('solver.alpha_u').locator('.st-key-help')).toBeVisible();
  await shot('03-solver-focus-detail');
});

test('a usual range with its source (turbulence.nutilde_ratio)', async () => {
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('turbulence.nutilde_ratio');
  const r = row('turbulence.nutilde_ratio');
  await expect(r).toBeVisible();
  await r.getByRole('textbox').focus();
  await expect(r.locator('.st-key-usual')).toContainText('Spalart and C. L. Rumsey');
  await shot('04-usual-range-with-source');
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('');
});
