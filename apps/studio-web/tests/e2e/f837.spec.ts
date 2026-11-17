// F-837 in the packaged Studio at 1440 by 900, against the real engine (rcfd serve for the case editor, rcfd-mcp for the
// project, rcfd-runner for the run started again, the fake claude for the panel): what the removed egui Studio offered
// beside its run list, field view and project view, rebuilt on the engine (the runner's budgets through ADR 0733's
// runner_config and set_runner_config). One test per capability;
// screenshots go to test-results-visual/f837/.
//
// The quick-start copy gets, as fixtures: a solver warning at the end of the Re 100 run's events.jsonl (the shape the
// solver writes, {"event":"warning","message":...}), a data file in cases/ and a folder in designs/.
import { _electron as electron, expect, test, type Page } from '@playwright/test';
import { appendFile, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { claudeFixture, FAKE_CLAUDE, sampleTranscript, type ClaudeFixture } from '../support/claude.js';
import { copyExample, findMcp, mcpPids, NO_MCP, projectState, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, killPid, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { launchStudio, type Studio } from '../support/studio.js';
import { decodePng } from '../support/png.js';
import { launchArgs } from '../support/launch.js';

const rcfd = findRcfd();
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.skip(runnerExe === null, NO_RUNNER);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f837');
const RE100 = 'r-20261028T152641-f8c2';
const CASE = 'cases/cavity-re100.toml';
const S1 = '00000000-0000-4000-8000-000000000001';

let dir = '';
let quick = '';
let fx: ClaudeFixture;
let service: RunningRcfd;
let studio: Studio;
let page: Page;
let runner: Runner | null = null;
let rerun: string | null = null;

const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const aside = () => page.locator(`aside.rv-run[data-run="${RE100}"]`);

async function toRuns(): Promise<void> {
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible({ timeout: 20_000 });
}

async function selectRe100(): Promise<void> {
  await toRuns();
  await page.locator(`.rv-case[data-run="${RE100}"]`).click();
  await expect(aside()).toBeVisible();
}

async function toWorkspace(): Promise<void> {
  await page.getByRole('tab', { name: 'Workspace' }).click();
  const list = page.getByRole('listbox', { name: 'Runs of the project' });
  await expect(list).toBeVisible({ timeout: 20_000 });
  await list.locator(`[data-run="${RE100}"]`).click();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', RE100);
  await expect(page.locator('.fv-frame[data-ready]')).toHaveCount(1, { timeout: 30_000 });
}

test.beforeAll(async () => {
  await mkdir(SHOTS, { recursive: true });
  dir = await scratch('f837');
  quick = await copyExample('quick-start', dir);
  await appendFile(
    path.join(quick, 'runs', RE100, 'events.jsonl'),
    JSON.stringify({ event: 'solver', kind: 'solver', solver: { event: 'warning', format: 'rcfd-status', message: 'Courant number 3.2 above 1 at iteration 40', seq: 200, version: 1 }, t: '2026-10-28T15:26:42.700000000Z' }) + '\n',
  );
  await writeFile(path.join(quick, 'cases', 'lid-profile.csv'), 'x,u\n0,0\n1,1\n');
  await mkdir(path.join(quick, 'designs', 'lid-shapes'), { recursive: true });
  await writeFile(path.join(quick, 'designs', 'lid-shapes', 'flat.json'), '{}\n');
  fx = await claudeFixture('f837');
  await fx.transcript(quick, S1, sampleTranscript(quick, S1));
  service = await startRcfd(rcfd!);
  studio = await launchStudio({
    RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`,
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_RECENT: path.join(dir, 'recent.json'),
    RCFD_STUDIO_CLAUDE: FAKE_CLAUDE,
    RCFD_STUDIO_CLAUDE_STATE: fx.state,
    ...fx.env({ FAKE_CLAUDE_DELAY_MS: '20' }),
  });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
});

test.afterAll(async () => {
  await studio?.app.close();
  if (runner) await runner.cleanUp(rerun);
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
  await rm(fx.root, { recursive: true, force: true }).catch(() => undefined);
});

test('the project home: designs/ and its entries, a data file of cases/ apart from the case files (no Edit or Run)', async () => {
  const designs = page.locator('.ph-group[data-group="designs"]');
  await expect(designs).toBeVisible({ timeout: 20_000 });
  await expect(designs.getByRole('heading', { level: 2 })).toHaveText(/designs\/\s*2/);
  await expect(designs.locator('li .ph-name-cell')).toHaveText(['lid-shapes/', 'variants.ledger.jsonl']);
  const data = page.getByRole('region', { name: /^Data files in cases\// });
  await expect(data.locator('li[data-data-file]')).toHaveCount(1);
  await expect(data).toContainText('lid-profile.csv');
  await expect(page.getByRole('button', { name: 'Edit cases/lid-profile.csv' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: `Edit ${CASE}`, exact: true })).toBeVisible();
  await page.mouse.move(1439, 899);
  await shot('01-home-designs-data-files');
});

test('a case file with unsaved changes in the case editor is marked on the home, and the mark goes with a save', async () => {
  const file = path.join(quick, 'cases', 'cavity-re100.toml');
  await page.getByRole('button', { name: `Edit ${CASE}`, exact: true }).click();
  await expect(page.getByRole('heading', { level: 2, name: CASE, exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('physics.nu');
  const nu = page.locator('.st-key[data-path="physics.nu"]').getByRole('textbox');
  await nu.fill('0.02');
  await nu.press('Enter');
  await expect(page.locator('.st-key[data-path="physics.nu"]').getByRole('textbox')).toHaveValue('0.02');
  await expect.poll(() => page.evaluate(() => location.hash)).toMatch(/^#\/case\/s\d+$/);
  const session = decodeURIComponent((await page.evaluate(() => location.hash)).replace('#/case/', ''));
  // the project home (the case editor is a page of its own, with no tabs)
  await page.evaluate(() => (location.hash = ''));
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  const mark = page.locator(`.ph-unsaved[data-unsaved="${CASE}"]`);
  await expect(mark).toBeVisible({ timeout: 15_000 });
  await expect(mark).toHaveAttribute('data-session', session);
  await expect(page.getByRole('link', { name: `Unsaved changes in the case editor (session ${session}): open them to save or discard` })).toHaveCount(1);
  await expect(page.locator('.ph-unsaved')).toHaveCount(1);
  await page.mouse.move(1439, 899);
  await shot('02-home-unsaved-mark');
  // the file is untouched until the editor saves it
  expect(readFileSync(file, 'utf8')).toContain('nu = 0.01');
  // the mark leads back to the session that holds the change (Edit would open the file in a new one), by the keyboard
  await mark.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 2, name: CASE, exact: true })).toBeVisible({ timeout: 20_000 });
  expect(await page.evaluate(() => location.hash)).toBe(`#/case/${session}`);
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('physics.nu');
  await expect(page.locator('.st-key[data-path="physics.nu"]').getByRole('textbox')).toHaveValue('0.02');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect.poll(() => readFileSync(file, 'utf8'), { timeout: 20_000 }).toMatch(/nu = 0\.02/);
  await page.evaluate(() => (location.hash = ''));
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  await expect(page.locator('.ph-unsaved')).toHaveCount(0, { timeout: 15_000 });
});

test('the run list shows each run’s latest residuals of u and p, from the last line of residuals.jsonl', async () => {
  await toRuns();
  await expect(page.getByRole('columnheader', { name: 'Last residual of u' })).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole('columnheader', { name: 'Last residual of p' })).toBeVisible();
  const lines = readFileSync(path.join(quick, 'runs', RE100, 'residuals.jsonl'), 'utf8').trim().split('\n');
  const last = JSON.parse(lines.at(-1)!) as { residuals: Record<string, number> };
  const row = page.getByRole('row').filter({ has: page.locator(`[data-run="${RE100}"]`) });
  const u = row.locator('.rv-res[data-residual="u"]');
  await expect(u).toHaveAttribute('data-value', String(last.residuals['u']), { timeout: 20_000 });
  await expect(u).toHaveText(last.residuals['u']!.toExponential(2));
  await expect(row.locator('.rv-res[data-residual="p"]')).toHaveAttribute('data-value', String(last.residuals['p']));
  await page.mouse.move(1439, 899);
  await shot('03-runs-residual-columns');
});

test('the selected run: its warning at rest; its log’s end and events, the warning named, from the keyboard', async () => {
  await selectRe100();
  const warnings = aside().getByRole('list', { name: `Warnings of ${RE100}` });
  await expect(warnings).toContainText('warning: Courant number 3.2 above 1 at iteration 40', { timeout: 20_000 });
  const toggle = aside().getByRole('button', { name: `Log and events of ${RE100}` });
  await toggle.focus();
  await page.keyboard.press('Enter');
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  const tail = aside().getByRole('region', { name: `The end of runs/${RE100}/log.txt` });
  await expect(tail).toContainText('finished after 121 iterations/steps', { timeout: 20_000 });
  const log = readFileSync(path.join(quick, 'runs', RE100, 'log.txt'), 'utf8').trimEnd().split(/\r?\n/);
  expect((await tail.textContent())!.trimEnd().split('\n').at(-1)).toBe(log.at(-1));
  const warned = aside().locator('.rv-event--warning');
  await expect(warned).toHaveCount(1);
  await expect(warned).toHaveAttribute('data-warning', 'true');
  await expect(warned).toContainText('warning: Courant number 3.2');
  await expect(aside().locator('.rv-event').first()).toContainText('state');
  await shot('04-run-log-events');
  await page.keyboard.press('Enter');
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await expect(tail).toHaveCount(0);
});

test('the selected run’s report opens in the system’s program for HTML', async () => {
  await studio.app.evaluate(({ shell }) => {
    const g = globalThis as unknown as { __opened: string[] };
    g.__opened = [];
    (shell as unknown as { openPath: unknown }).openPath = async (p: string) => (g.__opened.push(p), '');
  });
  const open = aside().getByRole('button', { name: `Open the report (runs/${RE100}/output/report.html)` });
  await expect(open).toBeVisible({ timeout: 20_000 });
  await open.click();
  await expect.poll(() => studio.app.evaluate(() => (globalThis as unknown as { __opened: string[] }).__opened)).toEqual([path.join(quick, 'runs', RE100, 'output', 'report.html')]);
});

test('the runner’s budgets: read and written by the runner (ADR 0733), refused over another writer’s change, undone by values', async () => {
  await toRuns();
  const file = path.join(quick, '.rcfd', 'runner-config.json');
  expect(existsSync(file)).toBe(false);
  await page.getByRole('button', { name: 'Runner budgets (.rcfd/runner-config.json)' }).click();
  const panel = page.getByRole('region', { name: 'Runner budgets' });
  await expect(panel).toContainText('no file: the runner uses its defaults');
  await panel.getByRole('textbox', { name: 'max_runs' }).fill('2');
  await panel.getByRole('textbox', { name: 'max_runs' }).press('Tab');
  await panel.getByRole('button', { name: 'Write', exact: true }).click();
  await expect(panel.getByRole('status')).toContainText('Written to .rcfd/runner-config.json; applies at the next runner start.');
  expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({ max_runs: 2 });
  // written by the runner's set_runner_config: one journal line of that command (the Studio writes nothing into .rcfd/)
  const edits = (await readFile(path.join(quick, '.rcfd', 'edits.jsonl'), 'utf8'))
    .split('\n')
    .filter((l) => l.trim() !== '')
    .map((l) => JSON.parse(l) as Record<string, unknown>)
    .filter((l) => l['path'] === '.rcfd/runner-config.json');
  expect(edits.map((l) => l['who'])).toEqual(['set_runner_config']);
  await shot('05-runner-budgets-written');
  // another writer since: the Studio does not write over it
  await writeFile(file, '{"max_runs": 3}\n');
  await panel.getByRole('textbox', { name: 'cpu_threads' }).fill('2');
  await panel.getByRole('textbox', { name: 'cpu_threads' }).press('Tab');
  await panel.getByRole('button', { name: 'Write', exact: true }).click();
  await expect(panel.getByRole('alert')).toContainText('was changed by another writer since it was read: nothing was written');
  expect(await readFile(file, 'utf8')).toBe('{"max_runs": 3}\n');
  await panel.getByRole('button', { name: 'Read again' }).click();
  await expect(panel.getByRole('textbox', { name: 'max_runs' })).toHaveValue('3');
  // undo of the write over the other writer's change asks first
  await panel.getByRole('button', { name: 'Undo the write' }).click();
  await expect(panel.getByRole('alert')).toContainText('was changed by another writer after this window wrote it');
  await panel.getByRole('button', { name: 'Keep it' }).click();
  expect(await readFile(file, 'utf8')).toBe('{"max_runs": 3}\n');
  await panel.getByRole('button', { name: 'Undo the write' }).click();
  await panel.getByRole('button', { name: 'Undo anyway' }).click();
  await expect(panel.getByRole('status')).toContainText('is as it was');
  // no file before the write: undone to no key set (the runner's defaults); the Studio removes no file (house rule)
  expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({});
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  // a file the runner cannot read (ADR 0733 Q4): refused as it is, replaced whole on purpose
  await writeFile(file, '{"gpu": 1}\n');
  await page.getByRole('button', { name: 'Runner budgets (.rcfd/runner-config.json)' }).click();
  await expect(panel.getByRole('alert')).toContainText('The runner would refuse this file: unknown field `gpu`');
  await panel.getByRole('textbox', { name: 'max_runs' }).fill('2');
  await panel.getByRole('textbox', { name: 'max_runs' }).press('Tab');
  await panel.getByRole('button', { name: 'Replace the file', exact: true }).click();
  await expect(panel.getByRole('status')).toContainText('Written to .rcfd/runner-config.json');
  expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({ max_runs: 2 });
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
});

test('Run again with fields stored: start_run with store plan, from the selected run', async () => {
  runner = new Runner(runnerExe!, quick);
  runner.ensure();
  await selectRe100();
  const again = aside().getByRole('button', { name: 'Run cavity-re100.toml again with fields stored' });
  await again.click();
  await expect.poll(async () => (await projectState(page)).project?.runRequests?.[0]?.action, { timeout: 20_000 }).toBe('project.queue_case');
  const req = (await projectState(page)).project!.runRequests![0]!;
  expect(req.outcome).toBe('accepted');
  expect(req.mcp).toEqual({ tool: 'start_run', arguments: { case: CASE, store: 'plan' } });
  rerun = req.run;
  expect(rerun).toMatch(/^r-/);
  await shot('06-run-again-fields-stored');
});

test('the field view: whole-run and fixed colour ranges; the snapshot’s range again', async () => {
  await toWorkspace();
  const fv = page.locator('section.fv').first();
  await expect(fv).toHaveAttribute('data-range-mode', 'snapshot');
  await page.getByRole('button', { name: 'Colour range of the whole run' }).click();
  await expect(fv).toHaveAttribute('data-range-mode', 'run');
  await expect(page.locator('.fv-info')).toContainText(/range of the whole run \(\d+ snapshots\)/, { timeout: 30_000 });
  await page.mouse.move(1439, 899);
  await shot('07-range-whole-run');
  await page.getByRole('button', { name: 'Fixed colour range' }).click();
  const lo = page.getByRole('textbox', { name: /^Bottom of the fixed range/ });
  const hi = page.getByRole('textbox', { name: /^Top of the fixed range/ });
  await expect(lo).toBeVisible();
  await lo.fill('0');
  await lo.press('Tab');
  await hi.fill('0.1');
  await hi.press('Tab');
  await expect(page.locator('.fv-info')).toContainText('viridis, a fixed range');
  await expect(page.locator('[data-outside]')).toHaveCount(1);
  expect(Number(await page.locator('[data-outside]').getAttribute('data-outside'))).toBeGreaterThan(0);
  await page.mouse.move(1439, 899);
  await shot('08-range-fixed');
  await page.getByRole('button', { name: 'Colour range of the shown snapshot' }).click();
  await expect(fv).toHaveAttribute('data-range-mode', 'snapshot');
  await expect(page.locator('[data-outside]')).toHaveCount(0);
});

test('the field view face on: the 2D plane straight on, a scale bar in metres, the domain in the caption', async () => {
  await expect(page.locator('.fv-domain')).toHaveAttribute('data-domain', '1 × 1');
  // ADR 0801 Addendum 22: a 2D case opens face on, orthographic along its thin axis, so its scale bar is there at once
  const canvas = page.locator('.fv-frame canvas').first();
  await expect(canvas).toHaveAttribute('data-camera', 'orthographic');
  await expect(canvas).toHaveAttribute('data-camera-forward', '0.00000 0.00000 -1.00000');
  const bar = page.locator('.fv-scalebar');
  await expect(bar).toBeVisible();
  const length = Number(await bar.getAttribute('data-length'));
  const m = length / 10 ** Math.floor(Math.log10(length) + 1e-12);
  expect([1, 2, 5].some((x) => Math.abs(m - x) < 1e-9)).toBe(true);
  await expect(bar).toContainText(/m$/);
  // ADR 0738 P5: a local store names no length unit, so the bar says the engine's documented store unit, and why
  await expect(bar).toHaveAttribute('data-length-unit', 'documented');
  await expect(bar).toHaveAttribute('title', /the engine stores the mesh in metres, after mesh\.scale and \[mesh\.transform\]/);
  const zoom0 = Number(await canvas.getAttribute('data-camera-zoom'));
  // zoomed in: Face on only turns the view (it is face on already), the zoom stays; Reset view frames the domain again
  await page.locator('.fv-frame').first().focus();
  for (let i = 0; i < 3; i++) await page.keyboard.press('+');
  const zoomed = Number(await canvas.getAttribute('data-camera-zoom'));
  expect(zoomed).toBeLessThan(zoom0 * 0.8);
  await page.getByRole('button', { name: 'View face on (along z)' }).click();
  expect(Number(await canvas.getAttribute('data-camera-zoom'))).toBe(zoomed);
  await expect(bar).toBeVisible();
  await page.mouse.move(1439, 899);
  await shot('09-face-on-scale-bar');
  await page.getByRole('button', { name: 'Reset view' }).click();
  await expect(canvas).toHaveAttribute('data-camera-moved', 'false');
  expect(Number(await canvas.getAttribute('data-camera-zoom'))).toBe(zoom0);
  await expect(bar).toBeVisible();
});

test('Save PNG: the view with its legend and caption, to the file the save dialog names', async () => {
  const to = path.join(dir, 'saved-view.png');
  await studio.app.evaluate(({ dialog }, f) => {
    const g = globalThis as unknown as { __saveAsked: string[] };
    g.__saveAsked = [];
    (dialog as unknown as { showSaveDialog: unknown }).showSaveDialog = async (...a: unknown[]) => {
      const o = a.at(-1) as { defaultPath?: string };
      g.__saveAsked.push((o.defaultPath ?? '').split(/[\\/]/).pop() ?? '');
      return { canceled: false, filePath: f };
    };
  }, to);
  await page.getByRole('button', { name: 'Save the view as PNG…' }).click();
  await expect(page.locator('[data-saved]')).toContainText('saved-view.png', { timeout: 20_000 });
  const asked = await studio.app.evaluate(() => (globalThis as unknown as { __saveAsked: string[] }).__saveAsked);
  expect(asked[0]).toMatch(new RegExp(`^${RE100}-.+\\.png$`));
  const img = decodePng(new Uint8Array(await readFile(to)));
  const frame = (await page.locator('.fv-frame').boundingBox())!;
  expect(img.width).toBeGreaterThanOrEqual(Math.floor(frame.width));
  expect(img.height).toBeGreaterThan(Math.floor(frame.height));
  // a picture, not a blank: more than a handful of distinct colours
  const seen = new Set<number>();
  for (let i = 0; i < img.data.length; i += 4 * 97) seen.add((img.data[i]! << 16) | (img.data[i + 1]! << 8) | img.data[i + 2]!);
  expect(seen.size).toBeGreaterThan(20);
  await shot('10-saved-png');
});

test('the monitor on a log scale, from the palette; values not above zero are counted, not drawn', async () => {
  const toggle = page.getByRole('button', { name: 'Monitor on a log scale' });
  await expect(toggle).toBeVisible({ timeout: 20_000 });
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Control+K');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  const search = dialog.getByLabel('Find a command by its name or its id');
  await expect(search).toBeFocused();
  await search.fill('Monitor on a log scale');
  await expect(dialog.getByRole('option')).toHaveCount(1);
  await expect(dialog.getByRole('option', { name: /^Monitor on a log scale/ })).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(dialog).toHaveCount(0);
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  // the quick-start's lid drag is negative at every step: none drawn, and the count is said
  const skipped = page.locator('.tp-monitor-skipped');
  await expect(skipped).toBeVisible();
  expect(Number(await skipped.getAttribute('data-skipped'))).toBeGreaterThan(0);
  await page.mouse.move(1439, 899);
  await shot('11-monitor-log');
  await toggle.click();
  await expect(skipped).toHaveCount(0);
});

test('the palette lists the F-837 commands; one that needs a run says where', async () => {
  await page.getByRole('tab', { name: 'Home' }).click();
  await page.keyboard.press('Control+K');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  const search = dialog.getByLabel('Find a command by its name or its id');
  await expect(search).toBeFocused();
  for (const t of ['Show the run log', 'Open the run report', 'Runner budgets', 'Save the field view as PNG', 'Set the colour range', 'View the field face on', 'Monitor on a log scale']) {
    await search.fill(t);
    await expect(dialog.getByRole('option').first()).toHaveAccessibleName(new RegExp(`^${t}`));
  }
  await search.fill('Show the run log');
  await expect(dialog.getByRole('option', { name: /^Show the run log/ })).toContainText('Open Runs (Ctrl+2) and select a run first.');
  await shot('12-palette');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

test('the Claude Code panel: a finished turn shows its model and the session’s cost', async () => {
  const panel = page.getByRole('complementary', { name: 'Claude Code' });
  const closed = panel.getByRole('button', { name: /^Which runs converged\?, closed, \d{4}-\d\d-\d\d \d\d:\d\d$/ });
  await expect(closed).toBeVisible({ timeout: 45_000 });
  await closed.click();
  const field = panel.getByRole('textbox', { name: /^Message to Claude Code, Which runs converged\?/ });
  await field.fill('Which run converged?');
  await field.press('Enter');
  await expect(panel.getByText(/^Answered/)).toBeVisible({ timeout: 30_000 });
  const meta = panel.locator('[data-turn-meta]').last();
  await expect(meta.locator('[data-model]')).toHaveAttribute('data-model', 'claude-haiku-5-5');
  await expect(meta.locator('[data-session-cost]')).toHaveAttribute('data-session-cost', '0.0031');
  await expect(meta).toContainText('0.0031 USD');
  await page.mouse.move(1439, 899);
  await shot('13-claude-model-cost');
});

test('a project folder given on the command line opens at start', async () => {
  const { executable } = JSON.parse(readFileSync(path.join(appRoot, 'out', 'packaged.json'), 'utf8')) as { executable: string };
  const env: Record<string, string> = {};
  for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_')) env[k] = v;
  const app = await electron.launch({
    executablePath: executable,
    ...launchArgs([quick]),
    env: {
      ...env,
      RCFD_STUDIO_MCP: mcp!,
      RCFD_STUDIO_RECENT: path.join(dir, 'recent-argv.json'),
      // the rcfd beside rcfd-mcp starts with the project (ADR 0801 Addendum 21): its record stays in the test's folder
      RCFD_STUDIO_STATE_DIR: path.join(dir, 'service-argv'),
    },
  });
  try {
    const p = await app.firstWindow();
    await expect(p.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
    const st = await p.evaluate(() => window.studioProject.state());
    expect(path.resolve(st.project!.folder).toLowerCase()).toBe(path.resolve(quick).toLowerCase());
    await p.screenshot({ path: path.join(SHOTS, '14-project-argument.png'), scale: 'css' });
  } finally {
    await app.close();
  }
});
