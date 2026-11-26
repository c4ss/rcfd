// F-988, ADR 0813: KK2, a Ctrl+I pressed while the provenance popover of a Runs grid cell (or row) is open, and Escape
// pressed before that key's request has run, leave the popover closed and the focus where it was. The request of a
// Ctrl+I runs two animation frames and a task after the key (afterFocusReturns of src/renderer/commands/CommandPalette.tsx);
// the check waits ten frames and 200 ms, then reads the state once.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';

const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'default', timeout: 120_000 });

/** quick-start: the lid-driven cavity at Re 100, with its residuals. */
const RE100 = 'r-20261028T152641-f8c2';

let dir = '';
let quick = '';
let studio: Studio | null = null;

const runRow = (page: Page, id: string): Locator => page.getByRole('row').filter({ has: page.locator(`[data-run="${id}"]`) });
const popover = (page: Page): Locator => page.locator('.pv-pop [role="dialog"], .pv-pop[role="dialog"]').first();

/** The name of the popover that is open, once the run's record is read. */
async function openName(page: Page): Promise<string> {
  const d = popover(page);
  await expect(d).toBeVisible();
  await expect(d.locator('[data-fact="Engine"] dd')).not.toHaveText('reading…');
  return ((await d.getAttribute('aria-label')) ?? '').replace(/^Provenance of /, '');
}

/** Ten animation frames, then 200 ms: five times the frames of a Ctrl+I's deferral, and its task. */
const settle = (page: Page): Promise<void> =>
  page.evaluate(
    () =>
      new Promise<void>((done) => {
        let n = 0;
        const f = () => (++n >= 10 ? setTimeout(done, 200) : requestAnimationFrame(f));
        requestAnimationFrame(f);
      }),
  );

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f988-'));
  quick = await copyExample('quick-start', dir);
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  const { app, page } = studio;
  await stubDialog(app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
});

test.afterAll(async () => {
  await studio?.app.close().catch(() => undefined);
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('KK2: Ctrl+I then Escape at once, on a Runs grid cell and on its row: the popover stays closed, the focus stays', async () => {
  const { page } = studio!;
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible();
  const row = runRow(page, RE100);
  const cell = row.getByRole('gridcell').filter({ has: page.locator('[data-residual="u"]') });
  for (const [host, first] of [
    [cell, 'residual of u'],
    [row, 'iterations'],
  ] as const) {
    await page.mouse.move(2, 890);
    await host.focus();
    await expect(host).toBeFocused();
    await page.keyboard.press('Control+i');
    expect(await openName(page)).toBe(first);
    await page.keyboard.press('Control+i');
    await page.keyboard.press('Escape');
    await settle(page);
    // read once: a popover the late key opened again would be here now
    expect(await page.locator('.pv-pop').count(), `${first}: a popover open after Escape`).toBe(0);
    expect(await host.evaluate((e) => e === document.activeElement), `${first}: the focus back on its host`).toBe(true);
  }
});
