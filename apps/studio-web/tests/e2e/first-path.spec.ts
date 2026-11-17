// A learner's first path (F-813) in the packaged Studio, against the real engine (rcfd-mcp, rcfd-runner and the solver
// they run, built by `cargo build --release -p rcfd-mcp -p rcfd-runner -p rcfd-cli`): from an empty temporary folder to
// the first honest result of the quick-start example (the engine's template lid_driven_cavity, the case of
// docs/tutorials/quick-start-cli.md).
//
// - No project open: the four steps of the path and what the first one needs.
// - An empty folder: the engine refuses it as no project; the page says what is missing (the project file, which the
//   Studio cannot make: the engine has no command for it) and the next step, and opens the folder by itself once the file
//   is there. The test stands in for the missing engine command and writes what rcfd_project's Project::create writes.
// - An empty project: Make the example (the engine's new_case), Run the example (start_run with the engine's start plan,
//   through run control), and the result: the engine's numbers equal to the run's own records, the stored field drawn,
//   the limits and the disclaimer visible (G6), the run opened in the run workspace (the result link, F-810).
// - The same walk by keyboard only, focus following each new step.
// - The precise terms behind Explain are the engine's glossary rows (F-830, `help.glossary`): the walk attaches the Studio
//   to an `rcfd serve --ui` it starts (with the rcfd beside rcfd-mcp) and stops at the end.
// Screenshots at 1440 by 900 go to test-results-visual/f813/ for the maintainer's view-and-report (G13).
import { expect, test, type Locator, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { mkdir, mkdtemp, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { alive, appRoot, killIfRcfd, killPid, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { findMcp, mcpPids, NO_MCP, projectState, snapshot, stubDialog } from '../support/project.js';
import { until } from '../support/fake-service.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { FIRST_CASE } from '../../src/shared/actions/first.js';
import { DISCLAIMER } from '../../src/renderer/first/words.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f813');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.describe.configure({ mode: 'serial' });

let dir = '';
let recentFile = '';
let studio: Studio | null = null;
let service: RunningRcfd | null = null;
const folders: string[] = [];

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });

async function launch(): Promise<Studio> {
  const env: Record<string, string> = { RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: recentFile };
  if (service) env['RCFD_STUDIO_SERVICE'] = `127.0.0.1:${service.port}`;
  const s = await launchStudio(env);
  await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => s.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  return s;
}

/**
 * Stands in for the engine command that does not exist yet (F-813 report: a project-creation entry point): writes what
 * rcfd_project::Project::create writes (the manifest of schema 1 and the standard folders), with the version `rcfd
 * --version` reports. Not the Studio: the Studio writes nothing to a project folder (G2, G12).
 */
async function makeProjectFileOutside(folder: string, name: string): Promise<void> {
  for (const d of ['cases', 'meshes', 'runs', 'designs', 'notes', '.rcfd']) await mkdir(path.join(folder, d), { recursive: true });
  const rcfd = path.join(path.dirname(mcp!), process.platform === 'win32' ? 'rcfd.exe' : 'rcfd');
  const version = existsSync(rcfd) ? execFileSync(rcfd, ['--version'], { encoding: 'utf8' }).trim() : 'rcfd 0.1.0';
  await writeFile(
    path.join(folder, 'rcfd-project.json'),
    JSON.stringify({ schema_version: 1, name, created: new Date().toISOString(), solver_version_created: version }, null, 2) + '\n',
  );
}

const path_ = (page: Page): Locator => page.locator('section.fp--open');
/** A step of the path, by role and accessible name (its name is for readers; the station strip shows it). */
const stepGroup = (page: Page, name: string): Locator => path_(page).getByRole('group', { name, exact: true });
/** The Quiet Rule (G22): at rest, the path shows no standing sentence; its words open under Explain. */
const quietAtRest = async (page: Page) => expect(path_(page).locator('.fp-body p:visible')).toHaveCount(0);

/** Presses Tab until the focused element is named `name` (its aria-label or its text); fails after 80 presses. */
async function tabTo(page: Page, name: string | RegExp): Promise<void> {
  for (let i = 0; i < 80; i++) {
    await page.keyboard.press('Tab');
    const now = await page.evaluate(() => {
      const a = document.activeElement as HTMLElement | null;
      return a ? (a.getAttribute('aria-label') ?? a.textContent ?? '').trim() : '';
    });
    if (typeof name === 'string' ? now === name : name.test(now)) return;
  }
  throw new Error(`Tab never reached ${String(name)}`);
}


test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f813-'));
  recentFile = path.join(dir, 'recent.json');
  await mkdir(SHOTS, { recursive: true });
  // the solver service the Explain terms come from (help.glossary, rcfd serve --ui)
  const rcfd = path.join(path.dirname(mcp!), process.platform === 'win32' ? 'rcfd.exe' : 'rcfd');
  if (existsSync(rcfd)) service = await startRcfd(rcfd, 0, ['--ui']);
});

/** Ends the Studio this file started, and the project servers it left (as the F-806/F-807 walks do). */
async function killStudio(s: Studio): Promise<void> {
  const pid = s.app.process().pid!;
  killPid(pid);
  await until(() => !alive(pid), 15_000, 'the killed Studio to end');
  for (const f of folders) for (const p of mcpPids(f)) killPid(p);
}

test.afterAll(async () => {
  if (studio) await killStudio(studio).catch(() => undefined);
  studio = null;
  // the runners the path started (they outlive the window, G7): stopped cleanly, then the folders removed
  for (const f of folders) await new Runner(runnerExe!, f).cleanUp(null).catch(() => undefined);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
  if (service) {
    await service.stop().catch(() => undefined);
    killIfRcfd(service.pid);
  }
  service = null;
});

test('no project open: the four steps of the path, their explanation one step away, and the disclaimer', async () => {
  studio = await launch();
  const { page } = studio;
  expect(await page.evaluate(() => document.querySelector('h1')?.textContent)).toBe('No project open');
  const intro = page.getByRole('region', { name: 'A first result in four steps' });
  await expect(intro).toBeVisible();
  await expect(intro.getByRole('list', { name: 'The steps of the first path' }).getByRole('listitem')).toHaveText([
    /Make the example, to come/,
    /Run it, to come/,
    /Watch it work, to come/,
    /See the result, to come/,
  ]);
  // G22: nothing to read at rest; the explanation is one step away
  await expect(intro.locator('p:visible')).toHaveCount(0);
  await shot(page, '01-no-project');
  await intro.getByRole('button', { name: 'Explain the first steps' }).click();
  const about = intro.getByRole('region', { name: 'About the first steps' });
  await expect(about).toContainText('an empty folder needs rcfd’s project file first');
  await expect(about).toContainText('example-projects/quick-start');
  await intro.getByRole('button', { name: 'Hide the first steps' }).click();
  await expect(about).toHaveCount(0);
  await expect(page.locator('footer')).toContainText('Not validated for engineering decisions or for safety-critical use. No warranty.');
});

test('an empty folder: what is missing and the next step; it opens by itself once the project file is there', async () => {
  const { app, page } = studio!;
  const empty = path.join(dir, 'my-first-project');
  await mkdir(empty);
  folders.push(empty);
  await stubDialog(app, empty);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Failed to open: not an rcfd project' })).toBeVisible({ timeout: 20_000 });
  const why = page.getByRole('region', { name: 'This folder is not a project yet' });
  await expect(why).toBeVisible();
  // an exception, so in words, short: what is missing and the next step
  await expect(why.locator('.fp-missing')).toHaveText('Empty folder: no rcfd-project.json.');
  await expect(why.locator('.fp-next')).toHaveText('Next: make it with Claude Code or the older rcfd-studio; the folder then opens by itself.');
  const unavailable = why.getByRole('button', { name: 'Make a project here' });
  await expect(unavailable).toHaveAttribute('aria-disabled', 'true');
  // its reason is its tooltip, on keyboard focus (the alert's Dismiss comes just before it)
  await page.getByRole('alert').getByRole('button', { name: 'Dismiss' }).focus();
  await page.keyboard.press('Tab');
  await expect(unavailable).toBeFocused();
  await expect(page.getByRole('tooltip')).toContainText('The engine has no command that makes a new project yet');
  await expect(why).toHaveAttribute('data-folder-state', 'watching');
  await why.getByRole('button', { name: 'Explain what a project needs' }).click();
  const about = why.getByRole('region', { name: 'About what a project needs' });
  await expect(about).toContainText('the Studio checks the folder every second and opens it as soon as the project file is there');
  await expect(about).toContainText('Project > New project in the older rcfd-studio app');
  expect(await readdir(empty)).toEqual([]);
  await shot(page, '02-empty-folder');
  // what the missing engine command would do, done outside the Studio
  await makeProjectFileOutside(empty, 'My first project');
  const before = await snapshot(empty);
  await expect(page.getByRole('heading', { level: 1, name: 'My first project' })).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole('alert').filter({ hasText: 'Failed to open' })).toHaveCount(0);
  // opening wrote nothing (G12)
  expect(await snapshot(empty)).toEqual(before);
});

test('an empty project: one action, the example made by the engine; the explanation, full control and precise terms one step away', async () => {
  const { page } = studio!;
  const folder = folders[0]!;
  await expect(path_(page)).toBeVisible();
  await expect(path_(page).getByRole('heading', { level: 2, name: 'Your first result' })).toBeVisible();
  await expect(path_(page).locator('[aria-current="step"]')).toContainText('Make the example');
  await expect(stepGroup(page, 'Make the example')).toBeVisible();
  await quietAtRest(page);
  // the F-805 empty states of the groups are still there, each once
  await expect(page.getByRole('note', { name: /^No cases yet\./ })).toBeVisible();
  await expect(page.getByRole('note', { name: /^No runs yet\./ })).toBeVisible();
  await shot(page, '03-empty-project');
  await path_(page).getByRole('button', { name: 'Explain this step' }).click();
  const about = path_(page).getByRole('region', { name: 'About this step' });
  await expect(about).toContainText('a square box of fluid whose top, the lid, slides sideways');
  await expect(about).toContainText('New case in the header band');
  await expect(about.getByRole('term').filter({ hasText: 'a square box whose lid slides' })).toBeVisible();
  // the precise term is the engine's glossary row, with its source (F-830, G19)
  await expect(about).toContainText('Template: A ready-made case the engine can write into a project');
  await expect(about).toContainText('docs/glossary.md, Template');
  await shot(page, '03b-make-explained');
  await path_(page).getByRole('button', { name: 'Make the example', exact: true }).click();
  await expect(page.locator(`tr[data-path="${FIRST_CASE}"]`)).toBeVisible({ timeout: 20_000 });
  await expect(stepGroup(page, 'Run it')).toBeVisible();
  await quietAtRest(page);
  // the engine wrote the template's case, nothing else
  const text = readFileSync(path.join(folder, ...FIRST_CASE.split('/')), 'utf8');
  expect(text).toContain('title = "lid-driven cavity Re=100"');
  expect(text).toContain('[reports.lid_drag]');
  expect(await readdir(path.join(folder, 'runs'))).toEqual([]);
  await shot(page, '04-run-it');
});

test('run it, and the first honest result: the engine’s numbers, the stored picture, the limits and the disclaimer; the run in the workspace', async () => {
  const { page } = studio!;
  const folder = folders[0]!;
  await path_(page).getByRole('button', { name: 'Run the example' }).click();
  await expect(stepGroup(page, 'See the result')).toBeVisible({ timeout: 90_000 });
  const runs = await readdir(path.join(folder, 'runs'));
  expect(runs).toHaveLength(1);
  const id = runs[0]!;
  const status = JSON.parse(readFileSync(path.join(folder, 'runs', id, 'status.json'), 'utf8')) as { state: string; step: number; reason: string };
  const metrics = JSON.parse(readFileSync(path.join(folder, 'runs', id, 'metrics.json'), 'utf8')) as { reports: Record<string, number> };
  expect(status.state).toBe('finished');
  const p = path_(page);
  // the state is its pictogram; the plain word, the precise word and the outcome are its accessible name (G8, G22)
  const mark = p.getByRole('img', { name: /^Done, settled \(converged/ });
  await expect(mark).toBeVisible({ timeout: 10_000 });
  await expect(mark).toHaveAccessibleName(new RegExp(`after ${status.step} rounds of calculation`));
  const grid = p.locator('dl.fp-grid');
  await expect(grid).toHaveAccessibleName(`Numbers of ${id}`);
  await expect(grid).toContainText(String(status.step));
  const drag = grid.locator('dd[title]').first();
  expect(Number(await drag.getAttribute('title'))).toBe(metrics.reports['lid_drag']);
  // the picture: the stored field of the last snapshot, drawn
  await expect(p.locator('.fv-frame[data-ready]')).toBeVisible({ timeout: 30_000 });
  await expect(p.getByText(/^Drawn by /)).toBeVisible({ timeout: 30_000 });
  await expect(p.locator('.fv-caption')).toContainText(id);
  // the learner's caption (G22, PRODUCT Principle 5): plain words at rest; the stored name, snapshot, step and time
  // are said to readers and behind Explain; the round opens its provenance like every number of the result
  const caption = p.locator('.fv-caption');
  await expect(caption).toHaveAttribute('data-caption', 'learner');
  await expect(caption).toContainText('Velocity to the right');
  const seen = await caption.evaluate((e) => {
    const out: string[] = [];
    const walk = (n: Node) => {
      if (n.nodeType === Node.TEXT_NODE) {
        const el = n.parentElement!;
        if (!el.closest('.sr-only')) out.push(n.textContent ?? '');
      } else n.childNodes.forEach(walk);
    };
    walk(e);
    return out.join('');
  });
  for (const precise of [/snapshot/, /step/, /t =/, /triangles/, /ux/]) expect(seen).not.toMatch(precise);
  expect(seen).toMatch(/round \d+/);
  const round = caption.getByRole('button', { name: /^Provenance of step: \d+$/ });
  await round.click();
  const rp = page.getByRole('dialog', { name: 'Provenance of step' });
  await expect(rp.locator('[data-fact="File"] dd')).toHaveText(`runs/${id}/fields/store.rts`);
  await page.keyboard.press('Escape');
  // the result's numbers open their provenance (F-828), as the same numbers do in Runs
  await expect(grid.getByRole('button', { name: /^Provenance of lid_drag: / })).toHaveCount(1);
  await expect(grid.getByRole('button', { name: /^Provenance of (iterations|steps): / })).toHaveCount(1);
  await expect(p.getByLabel('What the picture shows')).toBeVisible();
  expect(existsSync(path.join(folder, 'runs', id, 'fields', 'store.rts'))).toBe(true);
  // honest about limits (G6): the limits and the disclaimer stay visible on the result, and the footer too
  const limits = p.getByRole('note', { name: 'What this result is not' });
  await expect(limits).toContainText('Not a validation');
  await expect(limits).toContainText(DISCLAIMER);
  await expect(page.locator('footer')).toContainText('Not validated for engineering decisions');
  // at rest the only sentences in the path are the limits (G22)
  await expect(p.locator('.fp-step-text p:visible')).toHaveCount(3);
  await expect(p.locator('.fp-step-text p:visible').first()).toHaveClass(/fp-state/);
  // the page itself never scrolls: the path's body does
  expect(await page.evaluate(() => [document.documentElement.scrollHeight - innerHeight, document.documentElement.scrollWidth - innerWidth])).toEqual([0, 0]);
  await shot(page, '05-first-result');
  // one step away: the engine's words and the same run from the command line, with the plan's entries (G17, G19)
  await p.getByRole('button', { name: 'Explain this result' }).click();
  const about = p.getByRole('region', { name: 'About this result' });
  await expect(about).toContainText(status.reason);
  await expect(about.locator('.fp-command')).toContainText(`rcfd-runner start ${FIRST_CASE} --set output.store_interval=`);
  await expect(about).toContainText('Runs: every number of the run');
  await expect(about).toContainText('The run workspace: every stored picture of the run');
  // the precise terms of the caption, one step away (G19)
  await expect(about).toContainText("Snapshot: One stored state of the fields in a run's field store");
  await shot(page, '05b-result-explained');
  // the result link: the run in the run workspace (F-810)
  await p.getByRole('button', { name: 'Open the run in the workspace' }).click();
  await expect(page.getByRole('tab', { name: 'Workspace', selected: true })).toBeVisible();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', id, { timeout: 20_000 });
  await expect(page.locator('.ws .fv-frame[data-ready]')).toBeVisible({ timeout: 30_000 });
  await shot(page, '06-the-run-in-the-workspace');
  await page.getByRole('tab', { name: 'Home' }).click();
  expect((await projectState(page)).project?.runs.items.map((r) => r.id)).toEqual([id]);
});

test('the same walk by keyboard only, the focus following each new step', async () => {
  const { app, page } = studio!;
  await page.getByRole('button', { name: 'Close project' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'No project open' })).toBeVisible();
  const second = path.join(dir, 'keyboard-project');
  await mkdir(second);
  folders.push(second);
  await makeProjectFileOutside(second, 'Keyboard project');
  await stubDialog(app, second);
  await page.locator('body').focus();
  await tabTo(page, 'Open project folder…');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 1, name: 'Keyboard project' })).toBeVisible({ timeout: 20_000 });
  await tabTo(page, 'Make the example');
  await page.keyboard.press('Enter');
  // the focus goes on to the next step's action
  await expect(path_(page).getByRole('button', { name: 'Run the example' })).toBeFocused({ timeout: 20_000 });
  await page.keyboard.press('Tab');
  await expect(path_(page).getByRole('button', { name: 'Explain this step' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(path_(page).getByRole('region', { name: 'About this step' })).toBeVisible();
  await page.keyboard.press('Shift+Tab');
  await expect(path_(page).getByRole('button', { name: 'Run the example' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(path_(page).getByRole('button', { name: 'Open the run in the workspace' })).toBeFocused({ timeout: 90_000 });
  await expect(path_(page).locator('.fv-frame[data-ready]')).toBeVisible({ timeout: 30_000 });
  await expect(path_(page).getByText(/^Drawn by /)).toBeVisible({ timeout: 30_000 });
  await shot(page, '07-keyboard-result');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('tab', { name: 'Workspace', selected: true })).toBeVisible();
  await expect(page.locator('.ws[data-run]')).toBeVisible({ timeout: 20_000 });
});
