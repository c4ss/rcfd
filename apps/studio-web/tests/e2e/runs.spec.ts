// The run list and run state (F-806) in the packaged Studio, against the real engine: the project server (rcfd-mcp)
// and the project's runner (rcfd-runner), which owns every run. A copy of the quick-start project; runs started from the
// runner's CLI, as a user or Claude Code would start them (the Studio starts none until F-807).
//
// - The runs table lists the runs as the engine lists them, with the state pictogram and word, the result and the time
//   in words where the engine has no simulated time (a steady run's solver reports 0); sort, filter and selection from
//   the keyboard; selection fades the other rows by colour and never a failure.
// - G7: a long run started from the CLI; the Studio is closed during it, then killed during it; after each restart the
//   run has gone on (its step, from the runner's own status.json, is past where it was) and the table shows its true
//   state, live; pause, resume and cancel from the CLI show in the table as the runner writes them.
// Screenshots at 1440 by 900 go to test-results-visual/f806/ for the maintainer's view-and-report (G13).
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { alive, appRoot, killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, projectState, stubDialog } from '../support/project.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { until } from '../support/fake-service.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f806');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.describe.configure({ mode: 'serial' });

/** Never met: the run goes on until the test cancels it (about 55 iterations a second on the maintainer's machine). */
const LONG = ['solver.max_iterations=1000000', 'solver.tolerance_u=1e-14', 'solver.tolerance_p=1e-14'];
const INK_2 = 'rgb(74, 99, 130)';
const FAIL = 'rgb(155, 28, 36)';

let dir = '';
let quick = '';
let recentFile = '';
let runner: Runner | null = null;
let longRun: string | null = null;
let failedRun = '';
let studio: Studio | null = null;

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });

async function launch(): Promise<Studio> {
  const s = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: recentFile });
  await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => s.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  return s;
}

/** Opens the quick-start copy from the recent list (after the first open) or the stubbed folder dialog, then Runs. */
async function openRuns(s: Studio, from: 'dialog' | 'recent'): Promise<void> {
  if (from === 'dialog') {
    await stubDialog(s.app, quick);
    await s.page.getByRole('button', { name: 'Open project folder…' }).click();
  } else {
    await s.page.getByRole('button', { name: 'Open quick-start' }).click();
  }
  await expect(s.page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  await s.page.getByRole('tab', { name: 'Runs' }).click();
  await expect(s.page.getByRole('grid', { name: /^Runs of / })).toBeVisible();
}

const rowOf = (page: Page, id: string): Locator => page.getByRole('row').filter({ has: page.locator(`[data-run="${id}"]`) });
const shownRunnerState = (page: Page, id: string) => page.locator(`.rv-case[data-run="${id}"]`).getAttribute('data-state');
const shownStep = async (page: Page, id: string): Promise<number> => Number(await rowOf(page, id).locator('.rv-head').getAttribute('data-step'));

/** Kills the Studio's main process (no quit, no clean-up), as a crash would, and the project servers it leaves. */
async function killStudio(s: Studio): Promise<void> {
  const pid = s.app.process().pid!;
  killPid(pid);
  await until(() => !alive(pid), 15_000, 'the killed Studio to end');
  for (const p of mcpPids(quick)) killPid(p);
}

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f806-'));
  quick = await copyExample('quick-start', dir);
  recentFile = path.join(dir, 'recent.json');
  await mkdir(SHOTS, { recursive: true });
  runner = new Runner(runnerExe!, quick);
  runner.ensure();
  // a real failure: the runner's pre-launch check refuses the case (mesh.nx must be at least 1)
  failedRun = runner.submit('cases/cavity-re100.toml', ['mesh.nx=0']);
  await until(() => (runner!.status(failedRun).state === 'failed' ? true : null), 60_000, 'the failed run');
});

test.afterAll(async () => {
  if (studio) {
    await killStudio(studio).catch(() => undefined);
    studio = null;
  }
  await runner?.cleanUp(longRun);
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('the runs table: case, id, state pictogram and word, result; a steady run has no time, in words; a failure is red', async () => {
  studio = await launch();
  const { page } = studio;
  await openRuns(studio, 'dialog');
  const v = (await projectState(page)).project!;
  // every run the engine lists is a row, with the runner's state
  expect(v.runs.items).toHaveLength(5);
  for (const r of v.runs.items) {
    await expect(rowOf(page, r.id)).toBeVisible();
    expect(await shownRunnerState(page, r.id)).toBe(r.state);
  }
  // the details arrive from run_status: the quick-start runs converged, so their state is Converged
  await expect.poll(async () => Object.keys((await projectState(page)).project!.runDetails).length, { timeout: 15_000 }).toBe(5);
  const converged = v.runs.items.filter((r) => r.state === 'finished');
  for (const r of converged) {
    const row = rowOf(page, r.id);
    await expect(row.getByText('Converged', { exact: true })).toBeVisible();
    await expect(row.locator('svg[data-state="converged"]')).toBeVisible();
    // G3: a steady run's solver reports time 0; it has no simulated time, and the result says iterations, not "t = 0"
    await expect(row.locator('.rv-head')).toHaveText(`${r.step} iterations`);
  }
  // the failed run keeps its fail-wash ground and red word, with the runner's reason
  const failed = rowOf(page, failedRun);
  await expect(failed.getByText('Failed', { exact: true })).toBeVisible();
  await expect(failed.locator('.rv-note')).toContainText('pre-launch check failed');
  expect(await failed.locator('.ui-state-word').evaluate((e) => getComputedStyle(e).color)).toBe(FAIL);
  // the home's run list says the same, and no longer shows "0" for a steady run's time (the F-805 view)
  await page.getByRole('tab', { name: 'Home' }).click();
  for (const r of converged) await expect(page.locator(`tr[data-run="${r.id}"] td`).nth(2)).toHaveText('none, steady');
  await expect(page.locator(`tr[data-run="${failedRun}"] td`).nth(2)).toHaveText('none');
  await page.getByRole('tab', { name: 'Runs' }).click();
  await shot(page, '01-runs-table');
});

test('sort and filter by keyboard; selection fades the other rows by colour and never a failure; Escape clears it', async () => {
  const { page } = studio!;
  const grid = page.getByRole('grid', { name: /^Runs of / });
  const ids = async () => grid.locator('.rv-case').evaluateAll((es) => es.map((e) => e.getAttribute('data-run') ?? ''));
  // focus moves by Tab from the filter field to the state select and into the table
  await page.getByRole('textbox', { name: 'Filter runs by case or run id' }).focus();
  await page.keyboard.type('heated');
  await expect.poll(async () => (await ids()).length).toBe(2);
  await expect(page.getByText(/2 of 5 shown/)).toBeVisible();
  await page.keyboard.press('Control+A');
  await page.keyboard.press('Backspace');
  await expect.poll(async () => (await ids()).length).toBe(5);
  // the state filter, from the keyboard: open, pick Failed
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: /Run state/ })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('listbox')).toBeVisible();
  await page.keyboard.type('Fa');
  await expect(page.getByRole('option', { name: /^Failed, 1$/ })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect.poll(ids).toEqual([failedRun]);
  await page.getByRole('button', { name: /Run state/ }).click();
  await page.getByRole('option', { name: /^All states/ }).click();
  await expect.poll(async () => (await ids()).length).toBe(5);
  // sort: the column heads are reached with the arrow keys and sort with Enter
  await page.getByRole('button', { name: /Run state/ }).focus();
  await page.keyboard.press('Tab');
  const focusedRole = await page.evaluate(() => document.activeElement?.getAttribute('role'));
  expect(['row', 'gridcell', 'rowheader', 'columnheader']).toContain(focusedRole);
  const stateHead = grid.getByRole('columnheader', { name: /State/ });
  await stateHead.focus();
  await page.keyboard.press('Enter');
  await expect(stateHead).toHaveAttribute('aria-sort', 'ascending');
  // running, paused, queued, failed first: the failed run heads the list of ended runs
  expect((await ids())[0]).toBe(failedRun);
  await page.keyboard.press('Enter');
  await expect(stateHead).toHaveAttribute('aria-sort', 'descending');
  expect((await ids())[4]).toBe(failedRun);
  const resultHead = grid.getByRole('columnheader', { name: /Result/ });
  await resultHead.focus();
  await page.keyboard.press('Enter');
  await expect(resultHead).toHaveAttribute('aria-sort', 'ascending');
  const steps = await grid.locator('.rv-head').evaluateAll((es) => es.map((e) => Number(e.getAttribute('data-step'))));
  expect(steps).toEqual([...steps].sort((a, b) => a - b));
  // select the second row with the keyboard: down from the head
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  // from a cell, Left moves to the cells before it and then to the row itself, which Space selects
  for (let i = 0; i < 3; i++) await page.keyboard.press('ArrowLeft');
  expect(await page.evaluate(() => document.activeElement?.getAttribute('role'))).toBe('row');
  await page.keyboard.press('Space');
  const selectedRow = grid.locator('[role="row"][aria-selected="true"]');
  await expect(selectedRow).toHaveCount(1);
  const selectedId = await selectedRow.locator('.rv-case').getAttribute('data-run');
  expect(selectedId).not.toBe(failedRun);
  // every other row fades: its text turns ink-2 and its pictogram half-filled; the failure keeps its red word and full mark
  for (const id of await ids()) {
    const row = rowOf(page, id);
    const caseColour = await row.locator('.rv-case').evaluate((e) => getComputedStyle(e).color);
    const pictoOpacity = await row.locator('svg.ui-picto').evaluate((e) => getComputedStyle(e).opacity);
    if (id === selectedId) {
      expect(caseColour).not.toBe(INK_2);
      expect(pictoOpacity).toBe('1');
    } else if (id === failedRun) {
      expect(await row.locator('.ui-state-word').evaluate((e) => getComputedStyle(e).color)).toBe(FAIL);
      expect(await row.getAttribute('class')).not.toContain('ui-row--faded');
      expect(pictoOpacity).toBe('1');
    } else {
      expect(caseColour).toBe(INK_2);
      expect(pictoOpacity).toBe('0.5');
    }
    // text is never faded by opacity (the Fade by Colour Rule)
    expect(await row.locator('.rv-case').evaluate((e) => getComputedStyle(e).opacity)).toBe('1');
  }
  // the selected run's numbers, why it is in its state and what the runner recorded, beside the table
  await expect(page.locator(`aside.rv-run[data-run="${selectedId}"]`)).toBeVisible();
  await expect(page.locator('aside.rv-run').getByText('State changes the runner recorded')).toBeVisible();
  await expect(page.locator('aside.rv-run dl').getByText('Time, s', { exact: true })).toBeVisible();
  await expect(page.locator('aside.rv-run dl')).toContainText('none, steady');
  await shot(page, '02-selected-faded');
  // select the failed run: nothing about it fades, and the others do
  await rowOf(page, failedRun).click();
  await expect(rowOf(page, failedRun)).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('aside.rv-run .rv-why--failed')).toContainText('pre-launch check failed');
  await shot(page, '03-failed-selected');
  // Escape clears the selection: no row fades
  await page.keyboard.press('Escape');
  await expect(grid.locator('[role="row"][aria-selected="true"]')).toHaveCount(0);
  await expect(grid.locator('.ui-row--faded')).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'No run selected' })).toBeVisible();
});

test('G7: a run goes on when the Studio is closed and when it is killed; on each restart its true state is shown, live', async () => {
  test.setTimeout(240_000);
  const r = runner!;
  const { page } = studio!;
  longRun = r.submit('cases/cavity-re100.toml', LONG);
  // the new run appears by itself (the feed), then runs, its step rising in the table as the runner writes it
  await expect(rowOf(page, longRun)).toBeVisible({ timeout: 5000 });
  await expect.poll(() => shownRunnerState(page, longRun!), { timeout: 30_000 }).toBe('running');
  await expect(rowOf(page, longRun).getByText('Running', { exact: true })).toBeVisible();
  const a = await shownStep(page, longRun);
  await expect.poll(() => shownStep(page, longRun!), { timeout: 10_000 }).toBeGreaterThan(a);
  await rowOf(page, longRun).click();
  await expect(page.locator('aside.rv-run')).toContainText('running');
  await shot(page, '04-running-live');

  // 1. close the Studio normally during the run
  const solverPid = r.status(longRun).pid;
  expect(solverPid).not.toBeNull();
  await studio!.app.close();
  studio = null;
  const atClose = r.status(longRun);
  expect(atClose.state).toBe('running');
  await until(() => (r.status(longRun!).step > atClose.step + 20 ? true : null), 30_000, 'the run to go on after the Studio closed');
  expect(alive(solverPid!)).toBe(true);

  // restart: the true state, read from the runner's record, and the step goes on rising
  studio = await launch();
  await openRuns(studio, 'recent');
  const p2 = studio.page;
  await expect(rowOf(p2, longRun)).toBeVisible();
  await expect.poll(() => shownRunnerState(p2, longRun!), { timeout: 10_000 }).toBe(r.status(longRun).state);
  await expect(rowOf(p2, longRun).getByText('Running', { exact: true })).toBeVisible();
  // the step shown is the runner's, between what status.json said just before and just after reading the page
  await expect
    .poll(
      async () => {
        const before = r.status(longRun!).step;
        const shown = await shownStep(p2, longRun!);
        const after = r.status(longRun!).step;
        return shown > atClose.step && shown >= before - 120 && shown <= after;
      },
      { timeout: 10_000 },
    )
    .toBe(true);
  await shot(p2, '05-after-close-restart');

  // 2. kill the Studio during the run (no quit: as a crash)
  await killStudio(studio);
  studio = null;
  const atKill = r.status(longRun);
  expect(atKill.state).toBe('running');
  await until(() => (r.status(longRun!).step > atKill.step + 20 ? true : null), 30_000, 'the run to go on after the Studio was killed');
  expect(r.status(longRun).pid).toBe(solverPid);
  expect(alive(solverPid!)).toBe(true);

  studio = await launch();
  await openRuns(studio, 'recent');
  const p3 = studio.page;
  await expect.poll(() => shownRunnerState(p3, longRun!), { timeout: 10_000 }).toBe('running');
  await expect.poll(() => shownStep(p3, longRun!), { timeout: 10_000 }).toBeGreaterThan(atKill.step);
  const b = await shownStep(p3, longRun);
  await expect.poll(() => shownStep(p3, longRun!), { timeout: 10_000 }).toBeGreaterThan(b);
  await rowOf(p3, longRun).click();
  await shot(p3, '06-after-kill-restart');

  // 3. the runner's other states, from its CLI, show in the table as it writes them: paused, running, cancelled
  r.cli('pause', longRun);
  await expect.poll(() => shownRunnerState(p3, longRun!), { timeout: 20_000 }).toBe('paused');
  await expect(rowOf(p3, longRun).getByText('Paused', { exact: true })).toBeVisible();
  await expect(rowOf(p3, longRun).locator('svg[data-state="paused"]')).toBeVisible();
  expect(r.status(longRun).state).toBe('paused');
  await shot(p3, '07-paused');
  r.cli('resume', longRun);
  await expect.poll(() => shownRunnerState(p3, longRun!), { timeout: 20_000 }).toBe('running');
  r.cli('cancel', longRun);
  await expect.poll(() => shownRunnerState(p3, longRun!), { timeout: 30_000 }).toBe('cancelled');
  const end = r.status(longRun);
  expect(end.state).toBe('cancelled');
  const row = rowOf(p3, longRun);
  await expect(row.getByText('Cancelled', { exact: true })).toBeVisible();
  await expect(row.locator('svg[data-state="cancelled"]')).toBeVisible();
  await expect(row.locator('.rv-head')).toHaveText(`Stopped at iteration ${end.step}`);
  await expect(row.locator('.rv-note')).toContainText('cancelled on request');
  await expect(p3.locator('aside.rv-run')).toContainText('running to cancelled');
  await shot(p3, '08-cancelled');
});
