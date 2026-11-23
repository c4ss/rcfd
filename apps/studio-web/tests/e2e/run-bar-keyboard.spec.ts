// F-888 (ADR 0786): a choice in a select by keyboard, then Tab, where the animation frames come slowly.
//
// The quick-run test of F-807 (run-control.spec.ts:192) received "inactive" on the Run button on the lead's machine: the
// Tab after the choice of the case was sent before the frame in which the closed listbox gives the focus back to the
// select, so it started from the body. Here every requestAnimationFrame callback of the page is delayed by 300 ms, which
// makes that order certain on any machine, and the shared step (tests/support/keyboard.ts, chooseByKeyboard) must still
// leave the focus on the select before the next key.
//
// - The primitives gallery, no engine needed (this one runs on CI, whose packaged Studio has no engine): Tab from the
//   sample select reaches the same control after a choice as before it.
// - The run bar of the Runs view on a copy of the quick-start project (skipped without the engine): the tab order of
//   run-control.spec.ts:192, the case, Run, Queue…, Start the runner, the filter, and the focus ring on Run. No run and
//   no runner is started.
// The mutation beside this file (run-bar-keyboard.spec.mutation.patch) removes the wait of the shared step; both tests
// must then fail.
import { expect, test } from '@playwright/test';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { chooseByKeyboard, delayFrames, restoreFrames } from '../support/keyboard.js';
import { copyExample, findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';
import { killPid } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

/** Later than the Tab by a factor of more than 10, and inside the 5 s of the wait by the same (ADR 0786). */
const FRAME_DELAY_MS = 300;

let studio: Studio | null = null;
let dir = '';
let quick = '';

test.afterEach(async () => {
  if (studio) {
    console.log('F-888: the Studio window had the system’s focus:', await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]?.isFocused() ?? null).catch(() => null));
    await studio.app.close().catch(() => undefined);
    studio = null;
  }
  if (quick) for (const p of mcpPids(quick)) killPid(p);
  if (dir) await rm(dir, { recursive: true, force: true }).catch(() => undefined);
  dir = '';
  quick = '';
});

test('the gallery’s select: after a choice by keyboard under slow frames, Tab goes on from the select', async () => {
  studio = await launchStudio({});
  const { page } = studio;
  await page.evaluate(() => {
    location.hash = '#/dev/primitives';
  });
  await expect(page.getByRole('heading', { name: 'Primitives gallery', level: 1 })).toBeVisible();
  const select = page.getByRole('button', { name: /Sample state filter/ });
  // where Tab goes from the select when no listbox was opened
  await select.focus();
  await expect(select).toBeFocused();
  await page.keyboard.press('Tab');
  const next = await page.evaluate(() => {
    const a = document.activeElement as HTMLElement;
    a.setAttribute('data-f888-next', '');
    return a.tagName;
  });
  expect(next).not.toBe('BODY');
  const after = page.locator('[data-f888-next]');
  await expect(after).toBeFocused();
  await expect(select).not.toBeFocused();

  await delayFrames(page, FRAME_DELAY_MS);
  await chooseByKeyboard(select, 'Running');
  await page.keyboard.press('Tab');
  await expect(after).toBeFocused();
  await restoreFrames(page);
});

test('the run bar: the case chosen by keyboard under slow frames, then Tab to Run, Queue…, Start the runner and the filter', async () => {
  const mcp = findMcp();
  test.skip(!mcp, NO_MCP);
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f888-'));
  quick = await copyExample('quick-start', dir);
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  const { page } = studio;
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible();
  const bar = page.getByRole('region', { name: 'Start a run' });
  await expect(bar).toBeVisible();

  await delayFrames(page, FRAME_DELAY_MS);
  const caseSelect = bar.getByRole('button', { name: /Case to run/ });
  await chooseByKeyboard(caseSelect, 'cavity-re100.toml');
  await page.keyboard.press('Tab');
  const run = bar.getByRole('button', { name: 'Run cavity-re100.toml' });
  await expect(run).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(bar.getByRole('button', { name: 'Queue…' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(bar.getByRole('button', { name: 'Start the runner' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('textbox', { name: 'Filter runs by case or run id' })).toBeFocused();
  // the focus ring is visible on the control (DESIGN.md: 2px orange outline)
  await run.focus();
  expect(await run.evaluate((e) => getComputedStyle(e).outlineStyle)).not.toBe('none');
  await restoreFrames(page);
});
