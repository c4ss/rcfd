// G7 as amended 2026-11-17 (the maintainer: "the runner should close with studio by default if no external (existing)
// runner env var is specified"; for active runs: ask), in the packaged Studio against the real engine (rcfd-mcp,
// rcfd-runner, rcfd). Each test has its own copy of the quick-start project, and long runs (never converging) so a run
// is still going at the close.
//
// - A run the Studio started, the window closed: the panel names the run; Escape is Cancel; Stop runs and close cancels
//   it through the engine (its records stay) and the runner is gone.
// - Keep them running and close: the runner and its run go on; a new Studio adopts the runner (does not own it) and its
//   close leaves it alone, with no question.
// - A runner the Studio started that holds no run: gone after the close, with no question; the Studio's own rcfd serve
//   is gone too.
// - A runner started from the CLI before the Studio opened the project: no question about it, still alive after.
// - Q-RL-2 (the maintainer, 2026-11-17): the runner records who started it in .rcfd/runner.json. A runner the Studio
//   started says "studio" with the Studio's pid; one the CLI started while the Studio has the project open says "cli" and
//   is not the Studio's (before Q-RL-2 the looks made it the Studio's).
// - Changes not saved and a run going: one panel asks both.
// - --keep-runner, and the setting "Keep the runner going after the Studio closes": no question, the runner stays.
//
// Every process a test starts is stopped by it (the runner cleanly, through its CLI; whatever is left, killed).
import { expect, test, type Page } from '@playwright/test';
import { mkdtemp, rm } from 'node:fs/promises';
import { existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import type { RunActionResult } from '../../src/shared/project.js';
import type { RunnerPlan } from '../../src/shared/runner-lifetime.js';
import { launchStudio, serviceState, type Studio } from '../support/studio.js';
import { alive, appRoot, killIfRcfd, killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { until } from '../support/fake-service.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'runner-lifetime');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.describe.configure({ mode: 'serial', timeout: 180_000 });

/** Never met: the run goes on until it is stopped. */
const LONG = ['solver.max_iterations=1000000', 'solver.tolerance_u=1e-14', 'solver.tolerance_p=1e-14'];
const RE100 = 'cases/cavity-re100.toml';

let root = '';
/** What the tests started, for the clean-up: runners (by project), Studios, and rcfd serve processes. */
const projects: string[] = [];
const studios: Studio[] = [];
const services: number[] = [];

const pidOf = (s: Studio): number => s.app.process().pid!;

async function newProject(tag: string): Promise<{ folder: string; runner: Runner }> {
  const dir = await mkdtemp(path.join(root, `${tag}-`));
  const folder = await copyExample('quick-start', dir);
  projects.push(folder);
  return { folder, runner: new Runner(runnerExe!, folder) };
}

async function launch(folder: string, args: readonly string[] = [], env: Record<string, string> = {}): Promise<Studio> {
  const s = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(root, 'recent.json'), RCFD_STUDIO_PREFERENCES: path.join(root, 'prefs.json'), ...env }, args);
  studios.push(s);
  await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await stubDialog(s.app, folder);
  await s.page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(s.page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  await expect(s.page.locator(`tr[data-path="${RE100}"]`)).toBeVisible({ timeout: 30_000 });
  // the Studio's own rcfd serve (started with the first project), for the clean-up
  const st = await until(async () => {
    const x = await serviceState(s.page);
    return x.pid !== null ? x : null;
  }, 30_000, 'the Studio’s own service');
  services.push(st.pid!);
  return s;
}

/** A long run started from the window's run control (start_run through the engine; the runner starts if none is alive). */
async function studioRun(page: Page, runner: Runner): Promise<string> {
  const r = (await page.evaluate((set) => window.studioRuns.dispatch('project.queue_case', { case: 'cases/cavity-re100.toml', set, store: 'none' }), LONG)) as RunActionResult;
  expect(r.ok, JSON.stringify(r)).toBe(true);
  const id = r.request!.run!;
  await until(() => (runner.status(id).state === 'running' && runner.status(id).step > 0 ? true : null), 60_000, `run ${id} to run`);
  return id;
}

const plan = async (page: Page): Promise<RunnerPlan> => {
  const r = await page.evaluate(() => window.studioProject.runnerPlan());
  if (!r.ok) throw new Error(r.error.message);
  return r.result;
};

/** The window's own close, as its close button and Alt+F4 do (a round trip to the page, which may ask). */
const closeWindow = (s: Studio) => s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]?.close());

/**
 * Waits for the Studio's process to end and returns Playwright's close, to await once what the Studio left running is
 * stopped (on Windows a runner the Studio started holds Playwright's pipes to it, ADR 0146, so close() waits for it).
 */
async function ended(s: Studio): Promise<{ closed: Promise<void> }> {
  const pid = pidOf(s);
  await until(() => (!alive(pid) ? true : null), 60_000, 'the Studio process to end');
  studios.splice(studios.indexOf(s), 1);
  return { closed: s.app.close().catch(() => undefined) };
}

const panel = (page: Page) => page.getByRole('alertdialog');

/** The project's .rcfd/runner.json (Q-RL-2: started_by and studio_pid). */
const record = (folder: string): Record<string, unknown> => JSON.parse(readFileSync(path.join(folder, '.rcfd', 'runner.json'), 'utf8')) as Record<string, unknown>;

test.beforeAll(async () => {
  root = await mkdtemp(path.join(tmpdir(), 'rcfd-runner-lifetime-'));
});

test.afterAll(async () => {
  for (const s of studios.splice(0)) {
    killPid(pidOf(s));
    await until(() => (!alive(pidOf(s)) ? true : null), 15_000, 'a killed Studio to end').catch(() => undefined);
  }
  for (const f of projects) {
    const r = new Runner(runnerExe!, f);
    let active: string | null = null;
    try {
      for (const line of r.cli('list').split(/\r?\n/)) {
        const j = JSON.parse(line) as { id: string; status: { state: string } };
        if (['queued', 'running', 'paused'].includes(j.status.state)) {
          r.cli('cancel', j.id);
          active = j.id;
        }
      }
    } catch {
      // no list
    }
    await r.cleanUp(active).catch(() => undefined);
    for (const p of mcpPids(f)) killPid(p);
  }
  for (const pid of services) killIfRcfd(pid);
  await rm(root, { recursive: true, force: true }).catch(() => undefined);
});

test('Stop runs and close: the window’s close names the Studio’s runs; Escape keeps them; a running and a paused run are cancelled and the runner gone', async () => {
  const { folder, runner } = await newProject('stop');
  const s = await launch(folder);
  const { page } = s;
  expect(runner.pid()).toBeNull();
  const id = await studioRun(page, runner);
  const runnerPid = runner.pid()!;
  // a second run, paused (the maintainer found a paused rcfd run still holding target/release)
  const held = await studioRun(page, runner);
  const paused = (await page.evaluate((run) => window.studioRuns.dispatch('project.pause_run', { run }), held)) as RunActionResult;
  expect(paused.ok, JSON.stringify(paused)).toBe(true);
  await until(() => (runner.status(held).state === 'paused' ? true : null), 30_000, `run ${held} to pause`);
  const solvers = [runner.status(id).pid!, runner.status(held).pid!];
  const p = await plan(page);
  expect(p).toMatchObject({ owned: true, kept: null });
  expect(p.runs).toHaveLength(2);
  expect(p.runs).toEqual(expect.arrayContaining([expect.objectContaining({ id, case: RE100, state: 'running' }), expect.objectContaining({ id: held, state: 'paused' })]));

  await closeWindow(s);
  const ask = page.getByRole('alertdialog', { name: 'Close rcfd Studio with 2 runs going?' });
  await expect(ask).toBeVisible();
  const listed = ask.getByRole('list', { name: 'Runs going' });
  await expect(listed.locator(`[data-close-run="${id}"]`)).toHaveText(new RegExp(`^${id}: ${RE100}, running, step \\d+$`));
  await expect(listed.locator(`[data-close-run="${held}"]`)).toHaveText(new RegExp(`^${held}: ${RE100}, paused, step \\d+$`));
  await expect(ask.getByRole('button')).toHaveText(['Stop runs and close', 'Keep them running and close', 'Cancel']);
  // Cancel is the default: the focus is on it, and Escape answers it
  await expect(ask.getByRole('button', { name: 'Cancel' })).toBeFocused();
  await page.mouse.move(1439, 899);
  await page.screenshot({ path: path.join(SHOTS, '01-close-asks.png'), scale: 'css' });
  await page.keyboard.press('Escape');
  await expect(panel(page)).toHaveCount(0);
  expect(alive(pidOf(s))).toBe(true);
  expect(runner.status(id).state).toBe('running');

  await closeWindow(s);
  await expect(ask).toBeVisible();
  await ask.getByRole('button', { name: 'Stop runs and close' }).click();
  const { closed } = await ended(s);
  await closed;
  // both runs were cancelled through the engine (cancel_run: a line in the project journal; their records stay), the
  // solvers ended and the runner shut down cleanly
  for (const r of [id, held]) {
    expect(runner.status(r).state).toBe('cancelled');
    expect(existsSync(path.join(folder, 'runs', r, 'run.json'))).toBe(true);
  }
  for (const p of solvers) expect(alive(p)).toBe(false);
  expect(alive(runnerPid)).toBe(false);
  expect(runner.pid()).toBeNull();
  expect(readFileSync(path.join(folder, '.rcfd', 'edits.jsonl'), 'utf8')).toContain('Stopped when the Studio closed');
});

test('Keep them running and close: the runner and its run go on; a new Studio adopts the runner and leaves it at its close', async () => {
  const { folder, runner } = await newProject('keep');
  const s = await launch(folder);
  const id = await studioRun(s.page, runner);
  const runnerPid = runner.pid()!;
  await closeWindow(s);
  const ask = s.page.getByRole('alertdialog', { name: 'Close rcfd Studio with 1 run going?' });
  await expect(ask).toBeVisible();
  await ask.getByRole('button', { name: 'Keep them running and close' }).click();
  const { closed } = await ended(s);
  expect(alive(runnerPid)).toBe(true);
  const atClose = runner.status(id);
  expect(atClose.state).toBe('running');
  await until(() => (runner.status(id).step > atClose.step + 10 ? true : null), 30_000, 'the run to go on after the Studio closed');

  // the next Studio finds the runner alive at the open: adopted, not owned; its close asks nothing and stops nothing
  const again = await launch(folder);
  expect(await plan(again.page)).toEqual({ owned: false, kept: null, runs: [] });
  await closeWindow(again);
  const { closed: closedAgain } = await ended(again);
  expect(alive(runnerPid)).toBe(true);
  expect(runner.status(id).state).toBe('running');

  await runner.cleanUp(id);
  expect(alive(runnerPid)).toBe(false);
  await Promise.all([closed, closedAgain]);
});

test('a runner the Studio started that holds no run: gone after the close, with no question; the Studio’s own service too', async () => {
  const { folder, runner } = await newProject('idle');
  const s = await launch(folder);
  const service = (await serviceState(s.page)).pid!;
  const r = (await s.page.evaluate(() => window.studioRuns.dispatch('project.start_runner', {}))) as RunActionResult;
  expect(r.ok, JSON.stringify(r)).toBe(true);
  const runnerPid = await until(async () => runner.pid(), 15_000, 'the runner to hold the project');
  // Q-RL-2: the record says this Studio started it (its main process's pid, as the main process knows it)
  const mainPid = await s.app.evaluate(() => process.pid);
  expect(record(folder)).toMatchObject({ pid: runnerPid, started_by: 'studio', studio_pid: mainPid });
  expect(await plan(s.page)).toEqual({ owned: true, kept: null, runs: [] });
  // the test-mode quit asks nothing either (playwright.config.ts: RCFD_TEST_HEADLESS); here the window's own close
  await closeWindow(s);
  // no question: a panel would hold the window open, and the Studio ends at once (ended() waits for its process)
  await (await ended(s)).closed;
  expect(alive(runnerPid)).toBe(false);
  expect(runner.pid()).toBeNull();
  await until(() => (!alive(service) ? true : null), 10_000, 'the Studio’s own rcfd serve to end');
});

test('a runner started from the CLI before the Studio opened the project: no question about it, and it is still alive after', async () => {
  const { folder, runner } = await newProject('cli');
  runner.ensure();
  const runnerPid = runner.pid()!;
  const id = runner.submit(RE100, LONG);
  await until(() => (runner.status(id).state === 'running' ? true : null), 60_000, 'the CLI run to run');
  expect(record(folder)).toMatchObject({ pid: runnerPid, started_by: 'cli' });
  expect(record(folder)['studio_pid']).toBeUndefined();
  const s = await launch(folder);
  expect(await plan(s.page)).toEqual({ owned: false, kept: null, runs: [] });
  await closeWindow(s);
  // no question: a panel would hold the window open, and the Studio ends at once (ended() waits for its process)
  const { closed } = await ended(s);
  expect(alive(runnerPid)).toBe(true);
  expect(runner.status(id).state).toBe('running');
  await runner.cleanUp(id);
  await closed;
});

test('Q-RL-2: a runner the CLI starts while the Studio has the project open is not the Studio’s; it is left at the close', async () => {
  const { folder, runner } = await newProject('cli-open');
  const s = await launch(folder);
  expect(runner.pid()).toBeNull();
  runner.ensure();
  const runnerPid = runner.pid()!;
  expect(record(folder)).toMatchObject({ pid: runnerPid, started_by: 'cli' });
  // looks around it: before Q-RL-2, a runner that came up while the project was open with none before was owned
  await until(async () => ((await plan(s.page)).owned === false ? true : null), 5_000, 'the plan to say not owned');
  await new Promise((r) => setTimeout(r, 1500));
  expect(await plan(s.page)).toEqual({ owned: false, kept: null, runs: [] });
  await closeWindow(s);
  const { closed } = await ended(s);
  expect(alive(runnerPid)).toBe(true);
  await runner.cleanUp(null);
  await closed;
});

test('changes not saved and a run going: one panel asks both; Close and lose them with Stop them ends the run', async () => {
  const { folder, runner } = await newProject('merged');
  const s = await launch(folder);
  const { page } = s;
  const id = await studioRun(page, runner);
  const runnerPid = runner.pid()!;
  // an edit of a case, not saved
  await page.evaluate(() => (location.hash = ''));
  await page.getByRole('button', { name: `Edit ${RE100}`, exact: true }).click();
  await expect(page.getByRole('heading', { level: 2, name: RE100, exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('physics.nu');
  const nu = page.locator('.st-key[data-path="physics.nu"]').getByRole('textbox');
  await nu.fill('0.02');
  await nu.press('Enter');
  await expect(page.getByRole('list', { name: 'Settings a save writes' })).toContainText('physics.nu');

  await closeWindow(s);
  const ask = page.getByRole('alertdialog', { name: 'Close rcfd Studio with 1 change not saved and 1 run going?' });
  await expect(ask).toBeVisible();
  await expect(ask.getByRole('list', { name: 'Changes not saved' })).toHaveText(`${RE100}: 1 change (physics.nu)`);
  await expect(ask.getByRole('list', { name: 'Runs going' })).toContainText(id);
  await expect(ask.getByRole('button', { name: /^(Save and close|Close and lose them|Cancel)$/ })).toHaveText(['Save and close', 'Close and lose them', 'Cancel']);
  await expect(ask.getByRole('button', { name: 'Cancel' })).toBeFocused();
  // the runs' answer: Keep them running until Stop them is chosen
  const toggle = ask.getByRole('radiogroup', { name: 'The runs' });
  await expect(toggle.getByRole('radio', { name: 'Keep them running' })).toBeChecked();
  await toggle.getByRole('radio', { name: 'Stop them' }).click();
  await page.mouse.move(1439, 899);
  await page.screenshot({ path: path.join(SHOTS, '02-close-asks-both.png'), scale: 'css' });
  await ask.getByRole('button', { name: 'Close and lose them' }).click();
  await (await ended(s)).closed;
  expect(runner.status(id).state).toBe('cancelled');
  expect(alive(runnerPid)).toBe(false);
});

test('--keep-runner, then the setting: no question at the close, and the runner the Studio started goes on', async () => {
  const { folder, runner } = await newProject('switch');
  const s = await launch(folder, ['--keep-runner']);
  const id = await studioRun(s.page, runner);
  const runnerPid = runner.pid()!;
  expect(await plan(s.page)).toEqual({ owned: true, kept: 'switch', runs: [] });
  await closeWindow(s);
  // no question: a panel would hold the window open, and the Studio ends at once (ended() waits for its process)
  const { closed } = await ended(s);
  expect(alive(runnerPid)).toBe(true);
  await runner.cleanUp(id);
  await closed;

  // the setting, on the Studio's settings page (an icon toggle, no word at rest, G22); kept in studio-preferences.json
  const second = await newProject('setting');
  const t = await launch(second.folder, [], { RCFD_STUDIO_PREFERENCES: path.join(root, 'prefs-setting.json') });
  await t.page.keyboard.press('Control+,');
  const keep = t.page.getByRole('button', { name: 'Keep the runner going after the Studio closes' });
  await expect(keep).toHaveAttribute('aria-pressed', 'false');
  await keep.click();
  await expect(keep).toHaveAttribute('aria-pressed', 'true');
  await t.page.getByRole('button', { name: 'Close settings' }).click();
  const run2 = await studioRun(t.page, second.runner);
  const runner2 = second.runner.pid()!;
  expect(await plan(t.page)).toEqual({ owned: true, kept: 'setting', runs: [] });
  await closeWindow(t);
  // no question: a panel would hold the window open, and the Studio ends at once (ended() waits for its process)
  const { closed: closed2 } = await ended(t);
  expect(alive(runner2)).toBe(true);
  await second.runner.cleanUp(run2);
  await closed2;
  // the setting is in this test's own preferences file (RCFD_STUDIO_PREFERENCES): the other tests' Studios do not see it
});

test('Close project asks the same way: Stop runs and close ends the run and the runner, and the home has no project open', async () => {
  const { folder, runner } = await newProject('project');
  const s = await launch(folder);
  const { page } = s;
  const id = await studioRun(page, runner);
  const runnerPid = runner.pid()!;
  await page.evaluate(() => (location.hash = ''));
  await page.getByRole('button', { name: 'Close project' }).click();
  const ask = page.getByRole('alertdialog', { name: 'Close quick-start with 1 run going?' });
  await expect(ask).toBeVisible();
  await expect(ask.getByRole('button', { name: 'Cancel' })).toBeFocused();
  await ask.getByRole('button', { name: 'Stop runs and close' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'No project open' })).toBeVisible({ timeout: 60_000 });
  await expect(panel(page)).toHaveCount(0);
  expect(runner.status(id).state).toBe('cancelled');
  await until(() => (!alive(runnerPid) ? true : null), 10_000, 'the runner to end');
  // the Studio itself goes on; its quit (test mode, nothing to ask) stops its own service
  expect(alive(pidOf(s))).toBe(true);
  studios.splice(studios.indexOf(s), 1);
  await s.app.close();
});
