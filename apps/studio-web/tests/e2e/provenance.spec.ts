// The provenance inspector (F-828) in the packaged Studio, against the real engine: the project server (rcfd-mcp,
// read-only), the project's runner (rcfd-runner) and the solver (rcfd). A copy of the quick-start project.
//
// - G17, bit-identical: a run of cases/cavity-re100.toml started from the window (Runs, Run, by keyboard); its reproduce
//   command copied from its provenance (Copy command, read back from the system clipboard); the command run from a
//   shell with only --project changed to another, empty project; the two run folders compared file by file as the
//   README registers (F-828, "The reproduce test"): the results byte for byte, the records with their times, process
//   ids and own paths left out.
// - Every number-bearing surface (the selected run of Runs, the runs table, the workspace's numbers, the time panel's
//   residuals, monitor readout and snapshot counter, the field readout and the field caption's step and time) opens its
//   provenance (the settings and the plots' numbers of a service session: settings-rcfd.spec.ts, plots.spec.ts): run, snapshot, file, the
//   engine version that made it, the unit exactly as the engine wrote it.
// - The keyboard: Enter on a number, Ctrl+I on the focused number, row, scrubber or field view, Escape gives the focus
//   back; the palette's Show provenance and Copy reproduce command.
// - G22: at rest a number looks as it did (its text, no underline, border or fill); the words are its accessible name.
// Screenshots at 1440 by 900 go to test-results-visual/f828/.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { appRoot, killPid } from '../support/rcfd.js';
import { copyExample, emptyProject, findMcp, mcpPids, NO_MCP, projectState, stubDialog } from '../support/project.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { until } from '../support/fake-service.js';
import { shellLine } from '../../src/shared/provenance.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f828');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.describe.configure({ mode: 'serial' });

/** The quick-start run of cases/cavity-re100.toml with saved fields (24 snapshots, every 5 iterations). */
const CAVITY = 'r-20261028T152641-f8c2';

let dir = '';
let quick = '';
let other = '';
let runnerA: Runner | null = null;
let runnerB: Runner | null = null;
let studio: Studio | null = null;
let runA = '';
let runB = '';
let copied = '';

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const aside = (page: Page) => page.locator('aside.rv-run');
const dialogOf = (page: Page, name: string | RegExp) => page.getByRole('dialog', { name });
const fact = (d: Locator, k: string) => d.locator(`[data-fact="${k}"] dd`);
const clipboard = (s: Studio) => s.app.evaluate(({ clipboard: c }) => c.readText());

async function press(target: Locator, key = 'Enter'): Promise<void> {
  await target.focus();
  await expect(target).toBeFocused();
  await target.page().keyboard.press(key);
}

/** G22: at rest a trigger is its text; no underline, border, fill or padding of its own. */
async function quietAtRest(t: Locator, text: string | RegExp): Promise<void> {
  await expect(t).toHaveText(text);
  const st = await t.evaluate((e) => {
    const c = getComputedStyle(e);
    return { deco: c.textDecorationLine, border: c.borderTopWidth, bg: c.backgroundColor, pad: c.paddingLeft };
  });
  expect(st).toEqual({ deco: 'none', border: '0px', bg: 'rgba(0, 0, 0, 0)', pad: '0px' });
}

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f828-'));
  quick = await copyExample('quick-start', dir);
  other = await emptyProject(dir, 'Reproduce here');
  await mkdir(SHOTS, { recursive: true });
  // started from the test, not the window (ADR 0146: a runner the Studio starts inherits Playwright's pipes)
  runnerA = new Runner(runnerExe!, quick);
  runnerA.ensure();
  runnerB = new Runner(runnerExe!, other);
  runnerB.ensure();
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => studio!.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await stubDialog(studio.app, quick);
  await studio.page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(studio.page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  if (studio) {
    await studio.app.close().catch(() => undefined);
    studio = null;
  }
  await runnerA?.cleanUp(runA || null);
  await runnerB?.cleanUp(runB || null);
  for (const p of [...mcpPids(quick), ...mcpPids(other)]) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('a run started from the window shows its provenance and the command that reproduces it; Copy command copies it', async () => {
  test.setTimeout(120_000);
  const { page } = studio!;
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible();
  const bar = page.getByRole('region', { name: 'Start a run' });
  const caseSelect = bar.getByRole('button', { name: /Case to run/ });
  await press(caseSelect);
  await page.getByRole('option', { name: 'cavity-re100.toml' }).focus();
  await page.keyboard.press('Enter');
  await expect(caseSelect).toContainText('cavity-re100.toml');
  await press(bar.getByRole('button', { name: 'Run cavity-re100.toml' }));
  await until(async () => (await projectState(page)).project?.runRequests?.[0]?.run ?? null, 20_000, 'the run’s id');
  runA = (await projectState(page)).project!.runRequests![0]!.run!;
  await expect(aside(page)).toHaveAttribute('data-run', runA);
  await expect.poll(() => page.locator(`.rv-case[data-run="${runA}"]`).getAttribute('data-state'), { timeout: 90_000 }).toBe('finished');

  // the run's own provenance: on its id in the selected run's header, a button whose words are its name (G22)
  const runTrigger = aside(page).getByRole('button', { name: `Provenance of run ${runA}` });
  await quietAtRest(runTrigger, runA);
  await shot(page, '01-runs-at-rest');
  await press(runTrigger);
  const d = dialogOf(page, `Provenance of run ${runA}`);
  await expect(d).toBeVisible();
  const record = JSON.parse(await readFile(path.join(quick, 'runs', runA, 'run.json'), 'utf8')) as Record<string, unknown>;
  await expect(fact(d, 'Run')).toHaveText(runA);
  await expect(fact(d, 'Case')).toHaveText(String(record['case']));
  await expect(fact(d, 'Engine')).toHaveText(String(record['solver_version']));
  await expect(fact(d, 'Case snapshot')).toHaveText(`runs/${runA}/case.snapshot, ${String(record['case_snapshot_hash'])}`);
  await expect(fact(d, 'Settings')).toHaveText(JSON.stringify(record['settings']));
  const cmd = d.locator('[data-command]');
  // the record's case in the open folder and every recorded setting, in order
  const settings = record['settings'] as { threads: number; priority: number; mem_mb: number | null; set: string[] };
  const want = [
    'rcfd-runner',
    'start',
    path.join(quick, 'cases', 'cavity-re100.toml'),
    '--project',
    quick,
    '--threads',
    String(settings.threads),
    '--priority',
    String(settings.priority),
    ...(settings.mem_mb !== null ? ['--mem-mb', String(settings.mem_mb)] : []),
    ...settings.set.flatMap((x) => ['--set', x]),
  ];
  await expect(cmd).toHaveText(shellLine(want));
  // nothing stands between this command and the same result: no gap is said
  await expect(d.getByRole('list', { name: 'Why the command would not give the same result' })).toHaveCount(0);
  await shot(page, '02-run-provenance');
  await d.getByRole('button', { name: 'Copy command' }).click();
  await expect(d.getByRole('status')).toHaveText('Command copied');
  copied = await clipboard(studio!);
  expect(copied).toBe(await cmd.textContent());
  // Escape closes it, and the focus goes back to the number
  await page.keyboard.press('Escape');
  await expect(d).toHaveCount(0);
  await expect(runTrigger).toBeFocused();
});

test('G17: the copied command, run from a shell in another project, gives the same results bit for bit', async () => {
  test.setTimeout(180_000);
  expect(copied).toMatch(/^rcfd-runner start /);
  // only --project changes: the run goes to the other project
  const line = copied.replace(/--project ('[^']*'|\S+)/, `--project '${other}'`);
  expect(line).not.toBe(copied);
  const env = { ...process.env, PATH: `${path.dirname(runnerExe!)}${path.delimiter}${process.env['PATH'] ?? ''}` };
  const shell = process.platform === 'win32' ? ['powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', line]] : ['sh', ['-c', line]];
  const r = spawnSync(shell[0] as string, shell[1] as string[], { cwd: dir, env, encoding: 'utf8', windowsHide: true, timeout: 60_000 });
  expect(r.status, `${r.stdout}\n${r.stderr}`).toBe(0);
  runB = r.stdout.trim().split(/\r?\n/).at(-1)!.trim();
  expect(runB).toMatch(/^r-\d{8}T\d{6}-[0-9a-f]{4}$/);
  runnerB!.cli('wait', runB, '--timeout', '120');
  const a = path.join(quick, 'runs', runA);
  const b = path.join(other, 'runs', runB);
  const bytes = (p: string) => readFile(p);
  // byte for byte: the results and their inputs
  for (const f of [
    'fields/store.rts',
    'fields/index.json',
    'residuals.jsonl',
    'output/monitors.json',
    'output/monitors.csv',
    'output/report.html',
    'output/restart.rcfd',
    'output/solution.vtu',
    'case.snapshot',
    'check.json',
  ]) {
    const [x, y] = await Promise.all([bytes(path.join(a, f)), bytes(path.join(b, f))]);
    expect(x.length, f).toBeGreaterThan(0);
    expect(Buffer.compare(x, y), `${f} differs`).toBe(0);
  }
  // the records, with times, process ids and own paths left out (README, F-828)
  const json = async (p: string) => JSON.parse(await readFile(p, 'utf8')) as unknown;
  const lines = async (p: string) => (await readFile(p, 'utf8')).split('\n').filter((l) => l.trim() !== '').map((l) => JSON.parse(l) as unknown);
  const DROP = new Set(['t', 'pid', 'updated', 'elapsed_s', 'wall_s', 'launch_seq']);
  const norm = (v: unknown, folder: string, run: string): unknown => {
    if (Array.isArray(v)) return v.map((x) => norm(x, folder, run));
    if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).filter(([k]) => !DROP.has(k)).map(([k, x]) => [k, norm(x, folder, run)]));
    if (typeof v === 'string') return v.split(folder).join('<project>').split(run).join('<run>').replace(/\\/g, '/');
    return v;
  };
  const na = (v: unknown) => norm(v, quick, runA);
  const nb = (v: unknown) => norm(v, other, runB);
  expect(nb(await json(path.join(b, 'metrics.json')))).toEqual(na(await json(path.join(a, 'metrics.json'))));
  expect(nb(await json(path.join(b, 'status.json')))).toEqual(na(await json(path.join(a, 'status.json'))));
  expect(nb(await lines(path.join(b, 'solver-status.jsonl')))).toEqual(na(await lines(path.join(a, 'solver-status.jsonl'))));
  const ra = (await json(path.join(a, 'run.json'))) as Record<string, unknown>;
  const rb = (await json(path.join(b, 'run.json'))) as Record<string, unknown>;
  for (const k of ['case_snapshot_hash', 'solver_version', 'settings']) expect(rb[k], `run.json ${k}`).toEqual(ra[k]);
  const kinds = async (p: string) => ((await lines(p)) as { kind?: string; to?: string }[]).map((e) => `${e.kind}${e.to ? `:${e.to}` : ''}`);
  expect(await kinds(path.join(b, 'events.jsonl'))).toEqual(await kinds(path.join(a, 'events.jsonl')));
});

test('the reproduced run’s record names the first project’s case by its whole path (the runner records a case outside its project absolute)', async () => {
  // the record of the reproduced run names the first project's case by its whole path
  const rb = JSON.parse(await readFile(path.join(other, 'runs', runB, 'run.json'), 'utf8')) as Record<string, unknown>;
  expect(path.resolve(String(rb['case']))).toBe(path.join(quick, 'cases', 'cavity-re100.toml'));
});

test('Runs: the table’s numbers and the selected run’s numbers open their provenance; Ctrl+I on the focused row', async () => {
  const { page } = studio!;
  const steps = aside(page).getByRole('button', { name: /^Provenance of (iterations|steps): / });
  await expect(steps).toHaveCount(1);
  const status = JSON.parse(await readFile(path.join(quick, 'runs', runA, 'status.json'), 'utf8')) as { step: number };
  await quietAtRest(steps, String(status.step));
  await press(steps);
  const d = dialogOf(page, /^Provenance of (iterations|steps)$/);
  await expect(fact(d, 'File')).toHaveText(`runs/${runA}/status.json`);
  await expect(fact(d, 'Unit')).toHaveText(/^none: a count of the solver’s steps/);
  await expect(fact(d, 'Engine')).toHaveText('rcfd 0.1.0');
  await page.keyboard.press('Escape');
  await expect(steps).toBeFocused();
  // the solver time is in seconds by the engine's own key (elapsed_s)
  const solverTime = aside(page).getByRole('button', { name: /^Provenance of solver time: / });
  await press(solverTime);
  await expect(fact(dialogOf(page, 'Provenance of solver time'), 'Unit')).toHaveText(/^s, the key elapsed_s/);
  await page.keyboard.press('Escape');
  // the report values: the unit is the series unit of output/monitors.json, exactly as the engine wrote it
  const reports = aside(page).getByRole('button', { name: /^Provenance of (q_hot|nu_hot|lid_drag)/ });
  if ((await reports.count()) > 0) {
    await press(reports.first());
    await expect(fact(page.getByRole('dialog'), 'Unit')).toHaveText(/monitors\.json/);
    await page.keyboard.press('Escape');
  }
  // the table's numbers are not tab stops (the rows take the arrow keys): Ctrl+I opens the focused row's number
  const row = page.getByRole('row').filter({ has: page.locator(`[data-run="${runA}"]`) });
  const inRow = row.locator('[data-provenance-trigger]');
  await expect(inRow).toHaveCount(1);
  await expect(inRow).toHaveAttribute('data-provenance-name', /^Provenance of (iterations|steps): /);
  await row.focus();
  await page.keyboard.press('Control+i');
  const rd = dialogOf(page, /^Provenance of (iterations|steps)$/);
  await expect(rd).toBeVisible();
  await expect(fact(rd, 'Run')).toHaveText(runA);
  await shot(page, '03-row-ctrl-i');
  await page.keyboard.press('Escape');
  await expect(rd).toHaveCount(0);
  // the popover's Escape is its own: the run stays selected, and the focus goes back to the row
  await expect(aside(page)).toHaveAttribute('data-run', runA);
  await expect(row).toBeFocused();
});

test('the palette: Copy reproduce command asks which run and copies its command; Show provenance opens the selected run', async () => {
  const { page } = studio!;
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await page.getByRole('tab', { name: 'Runs' }).focus();
  await studio!.app.evaluate(({ clipboard: c }) => c.writeText('before'));
  await page.keyboard.press('Control+K');
  await expect(palette).toBeVisible();
  await page.keyboard.type('Copy reproduce command');
  await page.keyboard.press('Enter');
  const choose = palette.getByRole('listbox', { name: 'Choose a run' });
  await expect(choose).toBeVisible();
  const option = choose.getByRole('option').filter({ hasText: runA });
  await expect(option).toHaveCount(1);
  await option.click();
  await expect(palette.getByRole('heading', { name: 'Copy reproduce command: done' })).toBeVisible({ timeout: 10_000 });
  await expect(palette).toContainText(copied);
  expect(await clipboard(studio!)).toBe(copied);
  await shot(page, '04-palette-copy-command');
  await page.keyboard.press('Escape');
  await expect(palette).toHaveCount(0);
  // Show provenance with no number focused or pointed: the selected run's own provenance
  await page.mouse.move(2, 890);
  await page.getByRole('tab', { name: 'Runs' }).focus();
  await page.keyboard.press('Control+i');
  const d = dialogOf(page, `Provenance of run ${runA}`);
  await expect(d).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(d).toHaveCount(0);
});

test('the workspace: residual values, the monitor readout, the counter and the numbers open their provenance', async () => {
  test.setTimeout(90_000);
  const { page } = studio!;
  await page.getByRole('tab', { name: 'Workspace' }).click();
  await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${CAVITY}"]`).click();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', CAVITY);
  // the run's stops must be loaded before the keys move along them (the scrubber ignores keys while it has none)
  await expect(page.locator('.ws')).toHaveAttribute('data-snapshot', /^\d+$/);
  const slider = page.getByRole('slider', { name: /^Time of / });
  await slider.focus();
  await page.keyboard.press('Home');
  for (let i = 0; i < 4; i++) await page.keyboard.press('ArrowRight');
  const step = Number(await page.locator('.ws').getAttribute('data-step'));
  const snapshot = Number(await page.locator('.ws').getAttribute('data-snapshot'));
  // Ctrl+I on the scrubber opens the snapshot counter
  await page.keyboard.press('Control+i');
  const c = dialogOf(page, 'Provenance of snapshot');
  await expect(c).toBeVisible();
  await expect(fact(c, 'File')).toHaveText(`runs/${CAVITY}/fields/store.rts`);
  await expect(fact(c, 'Snapshot')).toHaveText(new RegExp(`^${snapshot + 1} of \\d+, step ${step}`));
  await page.keyboard.press('Escape');
  // the monitor readout: lid_drag with its unit from output/monitors.json, its exact value
  const line = (await readFile(path.join(quick, 'runs', CAVITY, 'residuals.jsonl'), 'utf8'))
    .split('\n')
    .filter((l) => l.trim() !== '')
    .map((l) => JSON.parse(l) as { step: number; monitors: Record<string, number> })
    .find((l) => l.step === step)!;
  const monitor = page.locator('.tp-readout-value').getByRole('button', { name: /^Provenance of lid_drag: .* N$/ });
  await expect(monitor).toHaveCount(1);
  await quietAtRest(monitor, (await page.locator('.tp-readout-value').textContent())!);
  await shot(page, '05-workspace-at-rest');
  await press(monitor);
  const m = dialogOf(page, 'Provenance of lid_drag');
  await expect(fact(m, 'Unit')).toHaveText(`N, runs/${CAVITY}/output/monitors.json`);
  await expect(fact(m, 'Value')).toContainText(String(line.monitors['lid_drag']));
  await expect(fact(m, 'File')).toHaveText(`runs/${CAVITY}/residuals.jsonl`);
  await expect(fact(m, 'Engine')).toHaveText('rcfd 0.1.0');
  await expect(fact(m, 'Snapshot')).toHaveText(new RegExp(`^${snapshot + 1} of \\d+, step ${step}`));
  await shot(page, '06-monitor-provenance');
  await page.keyboard.press('Escape');
  await expect(monitor).toBeFocused();
  // the residual values: the engine's empty unit is said to be empty
  const resid = page.locator('.tp-resid-value').getByRole('button', { name: /^Provenance of / });
  expect(await resid.count()).toBeGreaterThan(0);
  await press(resid.first());
  await expect(fact(page.getByRole('dialog'), 'Unit')).toHaveText(/^none, empty in runs\/.*\/output\/monitors\.json$/);
  await page.keyboard.press('Escape');
  // the workspace's numbers
  const grid = page.locator('.ws-numbers');
  for (const q of [/^Provenance of (iterations|steps): /, /^Provenance of snapshots stored: /, /^Provenance of residual lines: /, /^Provenance of step: /]) {
    await expect(grid.getByRole('button', { name: q })).toHaveCount(1);
  }
});

test('the field readout: Ctrl+I on the field view opens the value at the centre, its unit as store.rts gives it', async () => {
  test.setTimeout(60_000);
  const { page } = studio!;
  const frame = page.locator('.fv-frame');
  await expect(frame).toHaveAttribute('data-ready', '', { timeout: 20_000 });
  await frame.focus();
  await page.keyboard.press('ArrowLeft');
  const out = page.locator('.fv-readout-value');
  await expect(out).toBeVisible();
  const text = (await out.textContent())!;
  await page.keyboard.press('Control+i');
  const d = page.getByRole('dialog', { name: /^Provenance of / });
  await expect(d).toBeVisible();
  await expect(fact(d, 'File')).toHaveText(`runs/${CAVITY}/fields/store.rts`);
  await expect(fact(d, 'Value')).toContainText(text);
  await expect(fact(d, 'Cell')).toHaveText(await out.getAttribute('data-cell') as string);
  await expect(fact(d, 'Unit')).toHaveText(/store\.rts \(the field’s unit\)$/);
  await shot(page, '07-field-provenance');
  await page.keyboard.press('Escape');
  await expect(d).toHaveCount(0);
});

test('the field caption: its step and its time open their provenance (Enter, click), the time’s unit said as not recorded', async () => {
  const { page } = studio!;
  const caption = page.locator('.ws .fv-caption');
  const step = caption.getByRole('button', { name: /^Provenance of step: \d+$/ });
  await expect(step).toHaveCount(1);
  await quietAtRest(step, /^\d+$/);
  const shownStep = (await step.textContent())!;
  await press(step);
  const d = dialogOf(page, 'Provenance of step');
  await expect(fact(d, 'File')).toHaveText(`runs/${CAVITY}/fields/store.rts`);
  await expect(fact(d, 'Snapshot')).toHaveText(new RegExp(`^\\d+ of \\d+, step ${shownStep}`));
  await expect(fact(d, 'Engine')).toHaveText('rcfd 0.1.0');
  await page.keyboard.press('Escape');
  await expect(step).toBeFocused();
  const time = caption.getByRole('button', { name: /^Provenance of time: / });
  await time.click();
  const t = dialogOf(page, 'Provenance of time');
  await expect(fact(t, 'Unit')).toHaveText(`none: not recorded: runs/${CAVITY}/fields/store.rts gives a snapshot’s time without a unit`);
  await shot(page, '08-caption-provenance');
  await page.keyboard.press('Escape');
  await expect(t).toHaveCount(0);
});
