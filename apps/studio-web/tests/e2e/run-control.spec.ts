// Run control (F-807) in the packaged Studio, against the real engine: the project server's run tools (rcfd-mcp
// --project, start_run, pause_run, resume_run, cancel_run) and the project's runner (rcfd-runner), which owns every run.
// A copy of the quick-start project whose runner holds one run at a time (.rcfd/runner-config.json max_runs 1), so a
// second run stays queued.
//
// - Each action from the window, by keyboard: quick run from the Runs view and from the project home, queue with
//   per-run settings, pause, resume, stop and cancel (both confirmed in words naming what is lost, G18); each shows in
//   the run list (the request at once, P2 measured from the engine's answer; then the runner's state), and the runner's
//   own status.json agrees.
// - Refusals name the problem and the way out (G19): a control that cannot apply is an unavailable chip with its reason,
//   the main process refuses the same action sent past the page, and the engine's own refusal (a case deleted under the
//   Studio, a run that is not there) comes back in its words with a recovery.
// - No runner alive: a request the runner does not act on is marked after 3 s, and Start the runner (rcfd-runner ensure)
//   lets the runner act on it.
// - G7: a run started from the Studio goes on when the Studio is closed, and the next start shows it.
// Screenshots at 1440 by 900 go to test-results-visual/f807/; the P2 numbers to test-results-visual/f807/p2.json.
// F-893 (ADR 0790): the pause test reads the run list's note as it appeared and the runner's acting from the panel; the
// clean-up ends every run of the project, not only the ones a test noted.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { copyFile, mkdir, mkdtemp, rm, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import type { RunActionResult } from '../../src/shared/project.js';
import { launchStudio, type Studio } from '../support/studio.js';
import { alive, appRoot, killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, projectState, stubDialog } from '../support/project.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { until } from '../support/fake-service.js';
import { chooseByKeyboard } from '../support/keyboard.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f807');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.describe.configure({ mode: 'serial' });

/** Never met: the run goes on until it is stopped. */
const LONG = ['solver.max_iterations=1000000', 'solver.tolerance_u=1e-14', 'solver.tolerance_p=1e-14'];
const P2_MS = 20;
/** The bound on the DOM commit of the run list's note (see measured). */
const SHOWN_MS = 50;

let dir = '';
let quick = '';
let recentFile = '';
let runner: Runner | null = null;
let studio: Studio | null = null;
/** Runs the tests started (the clean-up ends every run of the project that is not over, F-893). */
const started: string[] = [];
/** Engine answer to run list, ms, per action (P2). */
const p2: { action: string; receivedMs: number | null; commitMs: number; frameMs: number }[] = [];

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });

async function launch(): Promise<Studio> {
  const s = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: recentFile });
  await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => s.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  return s;
}

async function openProject(s: Studio, from: 'dialog' | 'recent'): Promise<void> {
  if (from === 'dialog') {
    await stubDialog(s.app, quick);
    await s.page.getByRole('button', { name: 'Open project folder…' }).click();
  } else {
    await s.page.getByRole('button', { name: 'Open quick-start' }).click();
  }
  await expect(s.page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
}

async function toRuns(page: Page): Promise<void> {
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible();
}

const rowOf = (page: Page, id: string): Locator => page.getByRole('row').filter({ has: page.locator(`[data-run="${id}"]`) });
const shownState = (page: Page, id: string) => page.locator(`.rv-case[data-run="${id}"]`).getAttribute('data-state');
const aside = (page: Page) => page.locator('aside.rv-run');

/**
 * In the page: from now on, note when (ms since the epoch, fractional) an element matching `selector` first appears in
 * the document, and when the frame after it starts (the DOM commit and the paint that shows it); and its words then
 * (F-893, ADR 0790: the run list's note of a request goes once the runner has acted on it, which can be before the test
 * looks).
 */
async function watchFor(page: Page, selector: string): Promise<void> {
  await page.evaluate((sel) => {
    const w = window as unknown as { __seen?: { commit: number; frame: number; words: string } | null; __obs?: MutationObserver; __recv?: number | null; __recvId?: number };
    w.__obs?.disconnect();
    w.__seen = null;
    const now = () => performance.timeOrigin + performance.now();
    // when the page's state listener first gets a state holding the request (before React renders it)
    const want = Number(/data-request-seq="(\d+)"/.exec(sel)?.[1] ?? '0');
    w.__recv = null;
    if (w.__recvId !== undefined) window.studioProject.offState(w.__recvId);
    w.__recvId = window.studioProject.onState((st) => {
      if (w.__recv == null && st.project?.runRequests?.some((q) => q.seq === want)) w.__recv = now();
    });
    const check = () => {
      if (w.__seen || !document.querySelector(sel)) return;
      const commit = now();
      w.__seen = { commit, frame: 0, words: document.querySelector(sel)!.textContent ?? '' };
      requestAnimationFrame(() => {
        w.__seen!.frame = now();
      });
      w.__obs?.disconnect();
    };
    w.__obs = new MutationObserver(check);
    w.__obs.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
    check();
  }, selector);
}

/** The time from the main process's engine answer of the newest request to the run list showing it (P2). */
async function measured(page: Page, action: string): Promise<void> {
  const seen = await until(
    () => page.evaluate(() => {
      const s = (window as unknown as { __seen?: { commit: number; frame: number } | null }).__seen;
      return s && s.frame > 0 ? s : null;
    }),
    10_000,
    `the run list to show ${action}`,
  );
  const req = (await projectState(page)).project!.runRequests![0]!;
  expect(req.action).toBe(action);
  const recv = await page.evaluate(() => (window as unknown as { __recv?: number | null }).__recv ?? null);
  const entry = { action, receivedMs: recv === null ? null : recv - req.answeredAt, commitMs: seen.commit - req.answeredAt, frameMs: seen.frame - req.answeredAt };
  p2.push(entry);
  // the engine's answer reaches the renderer within P2 (G9: engine event to renderer), measured once per action. The run
  // list's DOM commit follows after React's render of the Runs view (13 to 19 ms measured on the maintainer's machine,
  // README F-807); it is held to SHOWN_MS here so that a loaded machine does not fail the run, and reported in p2.json.
  expect(entry.receivedMs, `${action}: engine answer to the renderer, ms`).not.toBeNull();
  expect(entry.receivedMs!, `${action}: engine answer to the renderer, ms`).toBeLessThanOrEqual(P2_MS);
  expect(entry.commitMs, `${action}: engine answer to the run list in the DOM, ms`).toBeLessThanOrEqual(SHOWN_MS);
}

/** The words of the element `watchFor` waited for, as they were when it first appeared (F-893). */
const seenWords = (page: Page): Promise<string | null> =>
  page.evaluate(() => (window as unknown as { __seen?: { words: string } | null }).__seen?.words ?? null);

/** Waits in the page for the run list's note of the next request (the one the next action makes). */
async function watchNext(page: Page): Promise<void> {
  const next = ((await projectState(page)).project!.runRequests?.[0]?.seq ?? 0) + 1;
  await watchFor(page, `.rv-table .rc-pending[data-request-seq="${next}"]`);
}

/** The id of the newest request's run (the run a start made). */
const newestRun = async (page: Page): Promise<string> => (await projectState(page)).project!.runRequests![0]!.run!;

async function killStudio(s: Studio): Promise<void> {
  const pid = s.app.process().pid!;
  killPid(pid);
  await until(() => !alive(pid), 15_000, 'the killed Studio to end');
  for (const p of mcpPids(quick)) killPid(p);
}

/** Focuses a control and presses Enter (every control is reached and operated from the keyboard, G8). */
async function press(target: Locator, key = 'Enter'): Promise<void> {
  await target.focus();
  await expect(target).toBeFocused();
  await target.page().keyboard.press(key);
}

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f807-'));
  quick = await copyExample('quick-start', dir);
  recentFile = path.join(dir, 'recent.json');
  await mkdir(SHOTS, { recursive: true });
  await mkdir(path.join(quick, '.rcfd'), { recursive: true });
  await writeFile(path.join(quick, '.rcfd', 'runner-config.json'), JSON.stringify({ max_runs: 1 }) + '\n');
  // a case the refusal test deletes under the Studio
  await copyFile(path.join(quick, 'cases', 'cavity-re100.toml'), path.join(quick, 'cases', 'to-delete.toml'));
  runner = new Runner(runnerExe!, quick);
  // started from the test, not the window: on Windows a runner the Studio starts inherits Playwright's pipes to the
  // Studio (ADR 0146), and closing the Studio would wait for it; the no-runner test at the end starts one from the window
  runner.ensure();
});

test.afterAll(async () => {
  if (studio) {
    await killStudio(studio).catch(() => undefined);
    studio = null;
  }
  // F-893 (ADR 0790): every run of the project that is not over, the ones a test failed before noting in `started`
  // among them, is cancelled and waited for, and a solver still alive is ended by its pid; then the runner is shut down
  // (a runner's shutdown leaves its solvers running, G7)
  const killed = (await runner?.endAll()) ?? [];
  if (killed.length) console.log(`F-893: solver ended by its pid after its cancel: ${killed.join(', ')}`);
  for (const p of mcpPids(quick)) killPid(p);
  if (p2.length) await writeFile(path.join(SHOTS, 'p2.json'), JSON.stringify(p2, null, 2) + '\n').catch(() => undefined);
  console.log('F-807 P2 (engine answer to run list, ms):', JSON.stringify(p2));
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('quick run from the Runs view, by keyboard: the run shows at once, then queued, running and its end, as the runner writes them', async () => {
  test.setTimeout(120_000);
  studio = await launch();
  const { page } = studio;
  await openProject(studio, 'dialog');
  await toRuns(page);
  const bar = page.getByRole('region', { name: 'Start a run' });
  await expect(bar).toBeVisible();
  // Tab order: the case, Run, Queue…, Start the runner, then the filter of F-806
  const caseSelect = bar.getByRole('button', { name: /Case to run/ });
  // F-888 (ADR 0786): the closed listbox gives the focus back to the select in the next animation frame; the shared step
  // returns once the select has it, so the Tab below starts from the select and not from the body
  await chooseByKeyboard(caseSelect, 'cavity-re100.toml');
  await expect(caseSelect).toContainText('cavity-re100.toml');
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
  await shot(page, '01-run-bar');

  const before = new Set((await projectState(page)).project!.runs.items.map((r) => r.id));
  await watchNext(page);
  await page.keyboard.press('Enter');
  await measured(page, 'project.run_case');
  const id = await newestRun(page);
  started.push(id);
  expect(before.has(id)).toBe(false);
  // the new run is selected and followed; the panel says what was asked and how to do the same elsewhere (G16, G17)
  await expect(aside(page)).toHaveAttribute('data-run', id);
  const status = aside(page).locator('.rc-status');
  await expect(status).toContainText('Started from the Studio');
  await expect(aside(page).locator('.rc-equiv')).toContainText('rcfd-runner start cases/cavity-re100.toml --project');
  // restated for ADR 0737: the Studio's Run is start_run's store plan
  await expect(aside(page).locator('.rc-equiv')).toContainText('start_run {"case":"cases/cavity-re100.toml","store":"plan"}');
  // the runner launches it and it ends; the table follows its state, and status.json says the same
  await expect.poll(() => shownState(page, id), { timeout: 60_000 }).toBe('finished');
  expect(runner!.status(id).state).toBe('finished');
  await expect(rowOf(page, id).getByRole('img', { name: 'Converged', exact: true })).toBeVisible();
  await expect(status).toHaveAttribute('data-settled', 'true');
  await shot(page, '02-quick-run-ended');
});

test('Run on a case of the project home: a quick run, followed in the Runs view', async () => {
  test.setTimeout(120_000);
  const { page } = studio!;
  await page.getByRole('tab', { name: 'Home' }).click();
  const run = page.locator('tr[data-path="cases/cavity-re400.toml"]').getByRole('button', { name: 'Run cavity-re400.toml' });
  await expect(run).toBeVisible();
  await shot(page, '03-home-run');
  await press(run);
  // the window switches to Runs with the new run selected
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible({ timeout: 15_000 });
  const id = await newestRun(page);
  started.push(id);
  await expect(aside(page)).toHaveAttribute('data-run', id);
  expect((await projectState(page)).project!.runRequests![0]!.cli.argv).toEqual(['rcfd-runner', 'start', 'cases/cavity-re400.toml', '--project', quick]);
  await expect.poll(() => shownState(page, id), { timeout: 90_000 }).toMatch(/^(finished|failed)$/);
  expect(runner!.status(id).state).toBe((await shownState(page, id))!);
});

let longRun = '';
let queuedRun = '';

test('queue with per-run settings; a second run waits; cancel it (confirmed in words); each shows within P2', async () => {
  test.setTimeout(120_000);
  const { page } = studio!;
  const bar = page.getByRole('region', { name: 'Start a run' });
  await press(bar.getByRole('button', { name: 'Queue…' }));
  const form = page.getByRole('group', { name: /^Queue / });
  await expect(form).toBeVisible();
  // the first field takes the focus; the per-run settings are typed one per line
  await expect(form.getByRole('textbox', { name: 'Priority' })).toBeFocused();
  const settings = form.getByRole('textbox', { name: 'Settings for this run' });
  await settings.focus();
  await page.keyboard.type(LONG.join('\n'));
  await shot(page, '04-queue-form');
  await watchNext(page);
  await press(form.getByRole('button', { name: 'Add to the queue' }));
  await measured(page, 'project.queue_case');
  longRun = await newestRun(page);
  started.push(longRun);
  const req = (await projectState(page)).project!.runRequests![0]!;
  expect(req.mcp).toEqual({ tool: 'start_run', arguments: { case: 'cases/cavity-re100.toml', store: 'plan', set: LONG } });
  expect(req.cli.argv).toEqual(['rcfd-runner', 'start', 'cases/cavity-re100.toml', ...LONG.flatMap((s) => ['--set', s]), '--project', quick]);
  await expect.poll(() => shownState(page, longRun), { timeout: 30_000 }).toBe('running');

  // a second run: the runner holds one run at a time, so it waits in the queue
  await press(bar.getByRole('button', { name: 'Queue…' }));
  await press(page.getByRole('group', { name: /^Queue / }).getByRole('button', { name: 'Add to the queue' }));
  await until(async () => ((await newestRun(page)) !== longRun ? true : null), 10_000, 'the second run');
  queuedRun = await newestRun(page);
  started.push(queuedRun);
  await expect.poll(() => shownState(page, queuedRun), { timeout: 10_000 }).toBe('queued');
  await page.waitForTimeout(1000);
  expect(runner!.status(queuedRun).state).toBe('queued');
  await expect(aside(page)).toHaveAttribute('data-run', queuedRun);
  // a queued run has nothing to stop: Cancel… is offered, and Pause holds it
  const controls = aside(page).locator('.rc-run');
  await expect(controls.getByRole('button', { name: 'Cancel…' })).toBeVisible();
  await expect(controls.getByRole('button', { name: 'Stop…' })).toHaveCount(0);
  await press(controls.getByRole('button', { name: 'Cancel…' }));
  const confirm = page.getByRole('alertdialog');
  await expect(confirm).toContainText(`Cancel queued run ${queuedRun}?`);
  await expect(confirm).toContainText('It has not started, so no result is lost. What is lost: its place in the queue.');
  // the safe choice has the focus
  await expect(confirm.getByRole('button', { name: 'Keep it queued' })).toBeFocused();
  await shot(page, '05-confirm-cancel-queued');
  await watchNext(page);
  await page.keyboard.press('Shift+Tab');
  await expect(confirm.getByRole('button', { name: 'Cancel the run' })).toBeFocused();
  await page.keyboard.press('Enter');
  await measured(page, 'project.cancel_run');
  await expect(confirm).toHaveCount(0);
  await expect.poll(() => shownState(page, queuedRun), { timeout: 10_000 }).toBe('cancelled');
  expect(runner!.status(queuedRun).state).toBe('cancelled');
  await expect(rowOf(page, queuedRun).locator('.rv-note')).toContainText('cancelled before launch');
  await shot(page, '06-cancelled-queued');
});

test('pause and resume by keyboard: the request shows within P2, then the runner holds and releases the solver', async () => {
  test.setTimeout(120_000);
  const { page } = studio!;
  await rowOf(page, longRun).click();
  await expect(aside(page)).toHaveAttribute('data-run', longRun);
  const controls = aside(page).locator('.rc-run');
  await watchNext(page);
  await press(controls.getByRole('button', { name: 'Pause' }));
  await measured(page, 'project.pause_run');
  // F-893 (ADR 0790): the row shows a request only until the runner has acted on it (pendingByRun), and the runner can act
  // before the test looks. The row's words are read as they were when the note appeared; that the runner acted is read
  // from the panel's status line, which keeps the request, and then the row has no note left.
  expect(await seenWords(page)).toMatch(/^Pause requested from the Studio/);
  const pauseSeq = (await projectState(page)).project!.runRequests![0]!.seq;
  const status = aside(page).locator('.rc-status');
  await expect(status).toHaveAttribute('data-request-seq', String(pauseSeq));
  await expect(status).toHaveAttribute('data-settled', 'true', { timeout: 15_000 });
  await expect(status).toContainText('Pause requested from the Studio');
  await expect(status).toContainText('the runner has acted on it');
  await expect(rowOf(page, longRun).locator('.rc-pending')).toHaveCount(0);
  await expect.poll(() => shownState(page, longRun), { timeout: 15_000 }).toBe('paused');
  expect(runner!.status(longRun).state).toBe('paused');
  await expect(controls.getByRole('button', { name: 'Resume' })).toBeVisible();
  await shot(page, '07-paused');
  const pausedAt = runner!.status(longRun).step;
  await page.waitForTimeout(1500);
  expect(runner!.status(longRun).step).toBe(pausedAt);

  await watchNext(page);
  await press(controls.getByRole('button', { name: 'Resume' }), 'Space');
  await measured(page, 'project.resume_run');
  await expect.poll(() => shownState(page, longRun), { timeout: 15_000 }).toBe('running');
  await until(() => (runner!.status(longRun).step > pausedAt ? true : null), 15_000, 'the run to go on after resume');
});

test('G7: a run started from the Studio goes on when the Studio is closed; the next start shows it running', async () => {
  test.setTimeout(120_000);
  await studio!.app.close();
  studio = null;
  const atClose = runner!.status(longRun);
  expect(atClose.state).toBe('running');
  await until(() => (runner!.status(longRun).step > atClose.step + 20 ? true : null), 30_000, 'the run to go on after the Studio closed');
  studio = await launch();
  await openProject(studio, 'recent');
  await toRuns(studio.page);
  await expect.poll(() => shownState(studio!.page, longRun), { timeout: 10_000 }).toBe('running');
});

test('stop, confirmed in words that name what is lost; Escape keeps it running; the solver stops cleanly', async () => {
  test.setTimeout(120_000);
  const { page } = studio!;
  await rowOf(page, longRun).click();
  const controls = aside(page).locator('.rc-run');
  const stop = controls.getByRole('button', { name: 'Stop…' });
  await press(stop);
  const confirm = page.getByRole('alertdialog');
  await expect(confirm).toContainText(`Stop run ${longRun}?`);
  await expect(confirm).toContainText('What is lost: the steps it has not computed yet.');
  await expect(confirm).toContainText(`its results so far stay in runs/${longRun}/`);
  await expect(confirm.getByRole('button', { name: 'Keep it running' })).toBeFocused();
  await shot(page, '08-confirm-stop');
  // Escape keeps the run going, and the focus goes back to Stop…
  await page.keyboard.press('Escape');
  await expect(confirm).toHaveCount(0);
  await expect(stop).toBeFocused();
  expect(runner!.status(longRun).state).toBe('running');
  await page.keyboard.press('Enter');
  await expect(confirm).toBeVisible();
  await watchNext(page);
  await press(confirm.getByRole('button', { name: 'Stop the run' }));
  await measured(page, 'project.stop_run');
  await expect.poll(() => shownState(page, longRun), { timeout: 30_000 }).toBe('cancelled');
  const end = runner!.status(longRun);
  expect(end.state).toBe('cancelled');
  await expect(rowOf(page, longRun).locator('.rv-note')).toContainText('cancelled on request; the solver stopped cleanly');
  // an ended run offers neither pause nor stop: the chips say why
  await expect(controls.getByRole('button', { name: 'Pause' })).toHaveAttribute('aria-disabled', 'true');
  await shot(page, '09-stopped');
});

test('refused actions name the problem and the way out: an unavailable control, the main process, and the engine', async () => {
  test.setTimeout(120_000);
  const { page } = studio!;
  // 1. in the window: Stop on an ended run is an unavailable chip whose reason says why and what to do
  const controls = aside(page).locator('.rc-run');
  const chip = controls.getByRole('button', { name: 'Stop…' });
  await expect(chip).toHaveAttribute('aria-disabled', 'true');
  await chip.focus();
  await expect(page.getByRole('tooltip')).toContainText('The run has already been cancelled: there is nothing to stop. Run its case again for a new run.');
  // 2. the same action sent past the page: the main process refuses it with the same words, and nothing reaches the runner
  const refused = await page.evaluate((run) => window.studioRuns.dispatch('project.stop_run', { run }), longRun);
  expect(refused.ok).toBe(false);
  const f = (refused as Extract<RunActionResult, { ok: false }>).failure;
  expect(f.code).toBe('unavailable');
  expect(f.problem).toBe(`Stop the project run: not available for run ${longRun}.`);
  expect(f.recovery).toContain('Run its case again');
  // 3. the engine's refusal: a run that is not in the project
  const gone = 'r-20990101T000000-dead';
  const engine = await page.evaluate((run) => window.studioRuns.dispatch('project.pause_run', { run }), gone);
  expect(engine.ok).toBe(false);
  const g = (engine as Extract<RunActionResult, { ok: false }>).failure;
  expect(g.code).toBe('not_found');
  expect(g.problem).toContain(`The engine refused to pause run ${gone}:`);
  expect(g.engine?.code).toBe(1202);
  expect(g.recovery).toContain('the run list shows the runs that are there now');
  // a malformed request never reaches the engine
  const bad = await page.evaluate(() => window.studioRuns.dispatch('project.queue_case', { case: 'cases/cavity-re100.toml', threads: 0 }));
  expect(bad.ok).toBe(false);
  expect((bad as Extract<RunActionResult, { ok: false }>).failure.code).toBe('bad_request');
  // 4. in the window, the engine's refusal: the case file is deleted under the Studio and Run is pressed before the list
  //    catches up; the engine says the file is not there and the page says what to do
  const bar = page.getByRole('region', { name: 'Start a run' });
  await bar.getByRole('button', { name: /Case to run/ }).click();
  await page.getByRole('option', { name: 'to-delete.toml' }).click();
  const run = bar.getByRole('button', { name: 'Run to-delete.toml' });
  await run.focus();
  await unlink(path.join(quick, 'cases', 'to-delete.toml'));
  await page.keyboard.press('Enter');
  const alert = bar.getByRole('alert');
  await expect(alert).toBeVisible({ timeout: 10_000 });
  await expect(alert.locator('.rc-problem')).toContainText('The engine refused to start a run of cases/to-delete.toml');
  await expect(alert.locator('.rc-problem')).toContainText('no such case file');
  await expect(alert.locator('.rc-recovery:not([data-recovery-code])')).toContainText('The case file was moved or deleted outside the Studio');
  // ADR 0801 Addendum 21: RCFD_STUDIO_MCP alone now starts the rcfd beside it with the project, so the service's error
  // catalogue (help.errors, F-830) is there, and its recovery line for the engine's code is shown under the Studio's
  await expect(alert.locator('.rc-recovery[data-recovery-code]')).toHaveCount(1);
  await expect(alert.locator('.rc-recovery[data-recovery-code]')).not.toBeEmpty();
  await shot(page, '10-refused');
  await alert.getByRole('button', { name: 'Dismiss' }).click();
  await expect(alert).toHaveCount(0);
});

test('no runner alive: a request the runner does not act on is marked after 3 s; Start the runner lets it act', async () => {
  test.setTimeout(120_000);
  const r = runner!;
  const { page } = studio!;
  r.cli('shutdown');
  await until(() => (r.pid() === null || !alive(r.pid()!) ? true : null), 30_000, 'the runner to stop');
  // a run queued from the CLI with no runner to launch it
  const waiting = r.submit('cases/cavity-re100.toml');
  started.push(waiting);
  await expect(rowOf(page, waiting)).toBeVisible({ timeout: 10_000 });
  await rowOf(page, waiting).click();
  const controls = aside(page).locator('.rc-run');
  await press(controls.getByRole('button', { name: 'Cancel…' }));
  await press(page.getByRole('alertdialog').getByRole('button', { name: 'Cancel the run' }));
  const status = controls.locator('.rc-status');
  await expect(status).toHaveAttribute('data-slow', 'false');
  // the request is accepted (the engine writes it), but no runner takes the run out of the queue
  await expect(status).toHaveAttribute('data-slow', 'true', { timeout: 10_000 });
  await expect(status).toContainText('the runner has not acted on it after 3 s');
  await expect(status).toContainText('Start the runner');
  expect(r.status(waiting).state).toBe('queued');
  await expect(rowOf(page, waiting).locator('.rc-pending')).toHaveAttribute('data-slow', 'true');
  await shot(page, '11-no-runner');
  await press(controls.locator('.rc-slow-actions').getByRole('button', { name: 'Start the runner' }));
  await expect.poll(() => shownState(page, waiting), { timeout: 30_000 }).toBe('cancelled');
  expect(r.status(waiting).state).toBe('cancelled');
  expect(r.pid()).not.toBeNull();
  await expect(aside(page).locator('.rc-status')).toContainText('Runner started');
  await shot(page, '12-runner-started');
});
