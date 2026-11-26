// The run workspace (F-810) in the packaged Studio, against the real engine: the project server (rcfd-mcp, read-only)
// and the project's runner (rcfd-runner). A copy of the quick-start project, whose runs are real runner runs with saved
// fields (runs/<id>/fields/store.rts, a snapshot every 5 iterations), plus runs started here from the runner's CLI.
//
// - One snapshot: at a scrub position the field (its stored values, read back with the Studio's reader and summarised
//   by the engine's field_summary), the residual cursor and the monitor readout (the line of residuals.jsonl at that
//   snapshot's step) and the snapshot counter all belong to the same snapshot.
// - The run bands: every run on one axis, each end with its pictogram and word; One Live Thing.
// - Follow live: a running run's line stays on its newest snapshot as the runner writes them, and stops following when
//   moved; Follow live brings it back.
// - The keyboard: the scrubber is a slider (arrow keys, Page Up and Down, Home and End), focus visible.
// Screenshots at 1440 by 900 go to test-results-visual/f810/ for the maintainer's view-and-report (G13).
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { appRoot, killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { until } from '../support/fake-service.js';
import { Store } from '../../src/main/data/tsstore.js';
import { ProjectServer } from '../../src/main/project/server.js';
import { f32Text } from '../../src/renderer/field/colormap.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f810');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.describe.configure({ mode: 'serial' });

/** The quick-start run of cases/cavity-re100.toml: converged at iteration 121, 24 snapshots (every 5 iterations). */
const CAVITY = 'r-20261028T152641-f8c2';
/** Never met: the run goes on until the test cancels it. */
const LONG = ['solver.max_iterations=1000000', 'solver.tolerance_u=1e-14', 'solver.tolerance_p=1e-14'];
const ORANGE = 'rgb(240, 138, 36)';

let dir = '';
let quick = '';
let runner: Runner | null = null;
let failedRun = '';
let longRun: string | null = null;
let studio: Studio | null = null;

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const scrubber = (page: Page): Locator => page.getByRole('slider', { name: /^Time of / });
const attr = async (l: Locator, a: string): Promise<number> => Number(await l.getAttribute(a));

/** The lines of a run's residuals.jsonl, by step, as the solver wrote them. */
async function residualLines(run: string): Promise<Map<number, { residuals: Record<string, number>; monitors: Record<string, number> }>> {
  const text = await readFile(path.join(quick, 'runs', run, 'residuals.jsonl'), 'utf8');
  const out = new Map<number, { residuals: Record<string, number>; monitors: Record<string, number> }>();
  for (const line of text.split('\n')) {
    if (line.trim() === '') continue;
    const j = JSON.parse(line) as { step: number; residuals: Record<string, number>; monitors: Record<string, number> };
    out.set(j.step, j);
  }
  return out;
}

async function openWorkspace(s: Studio): Promise<void> {
  await stubDialog(s.app, quick);
  await s.page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(s.page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  await s.page.getByRole('tab', { name: 'Workspace' }).click();
  await expect(s.page.getByRole('listbox', { name: 'Runs of the project' })).toBeVisible({ timeout: 20_000 });
}

async function selectRun(page: Page, id: string): Promise<void> {
  await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${id}"]`).click();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', id);
}

/** Waits until the field shows the workspace's snapshot (drawn) and returns it. */
async function settled(page: Page): Promise<{ snapshot: number; step: number }> {
  const ws = page.locator('.ws');
  const frame = page.locator('.fv-frame');
  return until(
    async () => {
      const want = await ws.getAttribute('data-snapshot');
      const got = await frame.getAttribute('data-snapshot');
      const ready = (await frame.getAttribute('data-ready')) !== null;
      return want !== null && want === got && ready ? { snapshot: Number(want), step: await attr(ws, 'data-step') } : null;
    },
    20_000,
    'the field to show the scrubber’s snapshot',
  );
}

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f810-'));
  quick = await copyExample('quick-start', dir);
  await mkdir(SHOTS, { recursive: true });
  runner = new Runner(runnerExe!, quick);
  runner.ensure();
  // a real failure: the runner's pre-launch check refuses the case (mesh.nx must be at least 1)
  failedRun = runner.submit('cases/cavity-re100.toml', ['mesh.nx=0']);
  await until(() => (runner!.status(failedRun).state === 'failed' ? true : null), 60_000, 'the failed run');
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => studio!.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await openWorkspace(studio);
});

test.afterAll(async () => {
  if (studio) {
    await studio.app.close().catch(() => undefined);
    studio = null;
  }
  await runner?.cleanUp(longRun);
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('the run bands: every run on one axis, each end with its pictogram and word, the selected one underlined', async () => {
  const { page } = studio!;
  await selectRun(page, CAVITY);
  const rows = page.locator('.tp-band-row');
  await expect(rows).toHaveCount(5);
  // the converged quick-start runs end with the converged pictogram and "it <last iteration>"; the state word is the end's
  // name and tooltip, "converged · it <last iteration>" (G22, F-833)
  const status = JSON.parse(await readFile(path.join(quick, 'runs', CAVITY, 'status.json'), 'utf8')) as { step: number };
  const cavity = page.locator(`.tp-band-row[data-run="${CAVITY}"]`);
  await expect(cavity.locator('.tp-end')).toHaveText(`it ${status.step}`);
  await expect(cavity.locator('.tp-end')).toHaveAccessibleName(`converged · it ${status.step}`);
  await expect(cavity.locator('.tp-end')).toHaveAttribute('title', `converged · it ${status.step}`);
  await expect(cavity.locator('.tp-end svg[data-state="converged"]')).toHaveCount(1);
  await expect(cavity.locator('.tp-seg--converged')).toHaveCount(1);
  await expect(cavity).toHaveClass(/tp-band-row--selected/);
  // a finished run opens on its newest snapshot
  const store = new Store(path.join(quick, 'runs', CAVITY, 'fields'));
  const n = store.snapshots.length;
  store.close();
  await expect(scrubber(page)).toHaveAttribute('aria-valuenow', String(n - 1));
  expect((await settled(page)).snapshot).toBe(n - 1);
  // the failure: red cap, the failed pictogram and its word, never faded
  const failed = page.locator(`.tp-band-row[data-run="${failedRun}"]`);
  await expect(failed.locator('.tp-end')).toHaveText(/^failed · it \d+$/);
  await expect(failed.locator('.tp-end')).toHaveAccessibleName(/^failed · it \d+$/);
  await expect(failed.locator('.tp-end svg[data-state="failed"]')).toHaveCount(1);
  expect(await failed.locator('.tp-seg--failed').evaluate((e) => getComputedStyle(e, '::before').opacity)).toBe('1');
  // the other bands drop to 40% fill
  const other = page.locator('.tp-band-row:not(.tp-band-row--selected) .tp-seg--iterating').first();
  expect(await other.evaluate((e) => getComputedStyle(e, '::before').opacity)).toBe('0.4');
  // one axis: every band's end lies inside it, in order of the runs' last iterations
  const ends = await page.locator('.tp-end').evaluateAll((es) => es.map((e) => e.getAttribute('aria-label') ?? ''));
  for (const e of ends) expect(e).toMatch(/^(converged|finished|failed|running|paused|cancelled|queued) · /);
  // no run is running: no orange
  await expect(page.locator('.tp-seg--live')).toHaveCount(0);
  await shot(page, '01-workspace-converged');
});

test('one snapshot: field values, residual cursor, monitor readout and counter at a scrub position all belong to it', async () => {
  const { page } = studio!;
  await selectRun(page, CAVITY);
  const slider = scrubber(page);
  await slider.focus();
  await page.keyboard.press('Home');
  for (let i = 0; i < 7; i++) await page.keyboard.press('ArrowRight');
  const { snapshot, step } = await settled(page);
  expect(snapshot).toBe(7);
  // the store says which step snapshot 7 is; the counter, the slider and the field name the same
  const store = new Store(path.join(quick, 'runs', CAVITY, 'fields'));
  try {
    expect(store.snapshots[snapshot]!.step).toBe(step);
    await expect(page.locator('.tp-counter')).toHaveText(`snapshot ${snapshot + 1} of ${store.snapshots.length}`);
    await expect(page.locator('.tp-counter')).toHaveAttribute('data-step', String(step));
    await expect(slider).toHaveAttribute('aria-valuenow', String(snapshot));
    await expect(page.locator('.fv-frame')).toHaveAttribute('data-step', String(step));
    await expect(page.locator('.fv-caption')).toContainText(`snapshot ${snapshot + 1} of ${store.snapshots.length} · step ${step}`);
    // the residual cursor: every residual at the line is the solver's value at that step (residuals.jsonl)
    const lines = await residualLines(CAVITY);
    const line = lines.get(step)!;
    expect(line).toBeTruthy();
    const names: Record<string, string> = { continuity: 'continuity', res_u: 'u', res_p: 'p' };
    for (const [shown, eq] of Object.entries(names)) {
      const li = page.locator(`.tp-resid-values li[data-name="${shown}"]`);
      await expect(li).toHaveAttribute('data-step', String(step));
      expect(Number(await li.getAttribute('data-value'))).toBe(line.residuals[eq]);
    }
    await expect(page.locator('.plot-cursor').first()).toHaveAttribute('data-cursor', String(step));
    // the monitor readout: lid_drag at that step, with its unit from output/monitors.json
    const readout = page.locator('.tp-readout-value');
    await expect(readout).toHaveAttribute('data-name', 'lid_drag');
    await expect(readout).toHaveAttribute('data-step', String(step));
    expect(Number(await readout.getAttribute('data-value'))).toBe(line.monitors['lid_drag']);
    await expect(page.locator('.tp-readout small')).toContainText('lid_drag, N · at it');
    // the field: the value under the view's centre is the stored value of snapshot 7 (the Studio's reader of store.rts,
    // bit for bit the Rust reader's, F-804), and it is not the value of the snapshots before and after
    const frame = page.locator('.fv-frame');
    await frame.focus();
    await page.keyboard.press('+');
    const out = page.locator('.fv-readout-value');
    await expect(out).toHaveAttribute('data-cell', /\d+/);
    const cell = Number(await out.getAttribute('data-cell'));
    const field = (i: number) => new Float32Array(store.field(i, 'p'));
    expect(await out.textContent()).toBe(f32Text(field(snapshot)[cell]!));
    expect(field(snapshot - 1)[cell]).not.toBe(field(snapshot)[cell]);
    // and the engine agrees on which snapshot that step is, and on its values (field_summary of the same step)
    const server = await ProjectServer.start(mcp!, quick);
    try {
      const sum = (await server.call('field_summary' as never, { run: CAVITY, field: 'p', step })) as { snapshot: { index: number; step: number }; min: number; max: number };
      expect(sum.snapshot).toMatchObject({ index: snapshot, step });
      const v = field(snapshot);
      let lo = Infinity;
      let hi = -Infinity;
      for (const x of v) {
        lo = Math.min(lo, x);
        hi = Math.max(hi, x);
      }
      expect([sum.min, sum.max]).toEqual([lo, hi]);
    } finally {
      await server.close();
    }
  } finally {
    store.close();
  }
  await shot(page, '02-scrubbed-snapshot-8');
});

test('the keyboard: the scrubber is a slider with arrow keys, Page Up and Down, Home and End; its line turns orange on focus', async () => {
  const { page } = studio!;
  await selectRun(page, CAVITY);
  const slider = scrubber(page);
  await page.mouse.move(5, 5);
  await slider.focus();
  const store = new Store(path.join(quick, 'runs', CAVITY, 'fields'));
  const n = store.snapshots.length;
  const lastStep = store.snapshots[n - 1]!.step;
  store.close();
  await page.keyboard.press('End');
  await expect(slider).toHaveAttribute('aria-valuenow', String(n - 1));
  await expect(slider).toHaveAttribute('aria-valuetext', `snapshot ${n} of ${n}, it ${lastStep}`);
  await page.keyboard.press('Home');
  await expect(slider).toHaveAttribute('aria-valuenow', '0');
  await page.keyboard.press('PageUp');
  await expect(slider).toHaveAttribute('aria-valuenow', '10');
  await page.keyboard.press('ArrowLeft');
  await expect(slider).toHaveAttribute('aria-valuenow', '9');
  await page.keyboard.press('PageDown');
  await expect(slider).toHaveAttribute('aria-valuenow', '0');
  await page.keyboard.press('ArrowLeft');
  await expect(slider).toHaveAttribute('aria-valuenow', '0');
  await page.keyboard.press('ArrowUp');
  await expect(slider).toHaveAttribute('aria-valuenow', '1');
  const { snapshot } = await settled(page);
  expect(snapshot).toBe(1);
  // focus visible: the line is 2px orange while the slider has keyboard focus
  expect(await slider.evaluate((e) => e.matches(':focus-visible'))).toBe(true);
  const line = page.locator('.tp-line');
  expect(await line.evaluate((e) => [getComputedStyle(e).borderLeftColor, getComputedStyle(e).borderLeftWidth])).toEqual([ORANGE, '2px']);
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Previous snapshot' }).or(page.getByRole('button', { name: 'First snapshot' })).first()).toBeVisible();
  // the transport from the keyboard
  await page.getByRole('button', { name: 'Next snapshot' }).focus();
  await page.keyboard.press('Enter');
  await expect(slider).toHaveAttribute('aria-valuenow', '2');
  await page.getByRole('button', { name: 'First snapshot' }).focus();
  await page.keyboard.press('Enter');
  await expect(slider).toHaveAttribute('aria-valuenow', '0');
  // the runs list from the keyboard: Down selects the next run
  await page.getByRole('listbox', { name: 'Runs of the project' }).locator('[role="option"][aria-selected="true"]').focus();
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('.ws')).not.toHaveAttribute('data-run', CAVITY);
  await shot(page, '03-keyboard');
});

test('ADR 0801 Addendum 22: Play steps one stored snapshot a second; the 2D cavity is face on, orthographic, pans and zooms only', async () => {
  test.setTimeout(90_000);
  const { page } = studio!;
  await selectRun(page, CAVITY);
  const slider = scrubber(page);
  await page.mouse.move(5, 5);
  await slider.focus();
  await page.keyboard.press('Home');
  await settled(page);
  // the field view: orthographic, straight along the thin axis, framed; the scale bar from the start
  const canvas = page.locator('.fv-frame canvas');
  await expect(canvas).toHaveAttribute('data-camera', 'orthographic');
  await expect(canvas).toHaveAttribute('data-camera-forward', '0.00000 0.00000 -1.00000');
  await expect(canvas).toHaveAttribute('data-camera-moved', 'false');
  await expect(page.locator('.fv-scalebar')).toBeVisible();
  await expect(page.locator('.fv-frame')).toHaveAttribute('aria-label', /Arrow keys pan/);
  const eye0 = (await canvas.getAttribute('data-camera-eye'))!;
  const zoom0 = Number(await canvas.getAttribute('data-camera-zoom'));
  const box = (await canvas.boundingBox())!;
  const cx = Math.round(box.x + box.width / 2);
  const cy = Math.round(box.y + box.height / 2);
  // the middle and right buttons pan; the wheel zooms toward the cursor; nothing turns the view
  for (const button of ['middle', 'right'] as const) {
    await page.mouse.move(cx, cy);
    await page.mouse.down({ button });
    await page.mouse.move(cx + 50, cy - 20, { steps: 4 });
    await page.mouse.up({ button });
  }
  await expect(canvas).toHaveAttribute('data-camera-moved', 'true');
  expect(await canvas.getAttribute('data-camera-eye')).not.toBe(eye0);
  await page.mouse.move(cx - 100, cy + 40);
  await page.mouse.wheel(0, -300);
  await expect.poll(async () => Number(await canvas.getAttribute('data-camera-zoom'))).toBeLessThan(zoom0 * 0.8);
  await page.locator('.fv-frame').focus();
  await page.keyboard.press('ArrowLeft');
  await expect(canvas).toHaveAttribute('data-camera-forward', '0.00000 0.00000 -1.00000');
  await expect(canvas).toHaveAttribute('data-camera-up', '0.00000 1.00000 0.00000');
  await page.mouse.move(5, 5);
  await shot(page, '04a-2d-panned-zoomed');
  // Face on keeps the zoom; Reset view frames the domain again
  const zoomed = await canvas.getAttribute('data-camera-zoom');
  await page.getByRole('button', { name: 'View face on (along z)' }).click();
  expect(await canvas.getAttribute('data-camera-zoom')).toBe(zoomed);
  await page.getByRole('button', { name: 'Reset view' }).click();
  await expect(canvas).toHaveAttribute('data-camera-moved', 'false');
  expect(await canvas.getAttribute('data-camera-eye')).toBe(eye0);

  // Play: the stop moves once a second
  await slider.focus();
  await page.keyboard.press('Home');
  await settled(page);
  await page.evaluate(() => {
    const w = window as unknown as { __stops: number[]; __obs?: MutationObserver };
    w.__stops = [performance.now()];
    w.__obs = new MutationObserver(() => w.__stops.push(performance.now()));
    w.__obs.observe(document.querySelector('.ws')!, { attributes: true, attributeFilter: ['data-index'] });
  });
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await page.waitForTimeout(4300);
  await page.getByRole('button', { name: 'Pause', exact: true }).click();
  const stops = await page.evaluate(() => {
    const w = window as unknown as { __stops: number[]; __obs?: MutationObserver };
    w.__obs?.disconnect();
    return w.__stops;
  });
  const gaps = stops.slice(1).map((t, i) => t - stops[i]!);
  test.info().annotations.push({ type: 'play gaps (ms)', description: gaps.map((g) => Math.round(g)).join(', ') });
  // the first gap runs from before the click; every stop after it a second (its field drawn well within it)
  expect(stops.length - 1, 'four stops in 4.3 s').toBe(4);
  for (const g of gaps.slice(1)) {
    expect(g).toBeGreaterThanOrEqual(950);
    expect(g).toBeLessThan(1400);
  }
  await expect(slider).toHaveAttribute('aria-valuenow', '4');

  // Q-A22-3: the speed menu beside Play (a pictogram; its name says the speed), remembered per run
  const speedButton = page.locator('.tp-speed');
  await expect(speedButton).toHaveAccessibleName('Play speed 1×, one snapshot a second');
  expect((await speedButton.innerText()).trim(), 'no word at rest').toBe('');
  await speedButton.click();
  const items = page.getByRole('menuitemradio');
  await expect(items).toHaveText(['0.25×', '0.5×', '1×', '2×', '4×']);
  await expect(page.getByRole('menuitemradio', { name: '1×' })).toHaveAttribute('aria-checked', 'true');
  await page.getByRole('menuitemradio', { name: '4×' }).click();
  await expect(speedButton).toHaveAccessibleName('Play speed 4×, 4 snapshots a second');
  await expect(speedButton).toHaveAttribute('data-speed', '4');
  await page.mouse.move(5, 5);
  await shot(page, '04b-play-speed-4x');
  await slider.focus();
  await page.keyboard.press('Home');
  await settled(page);
  await page.evaluate(() => {
    const w = window as unknown as { __stops: number[]; __obs?: MutationObserver };
    w.__stops = [];
    w.__obs = new MutationObserver(() => w.__stops.push(performance.now()));
    w.__obs.observe(document.querySelector('.ws')!, { attributes: true, attributeFilter: ['data-index'] });
  });
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await page.waitForTimeout(2100);
  await page.getByRole('button', { name: 'Pause', exact: true }).click();
  const fast = await page.evaluate(() => {
    const w = window as unknown as { __stops: number[]; __obs?: MutationObserver };
    w.__obs?.disconnect();
    return w.__stops;
  });
  const fastGaps = fast.slice(1).map((t, i) => t - fast[i]!);
  test.info().annotations.push({ type: 'play gaps at 4x (ms)', description: fastGaps.map((g) => Math.round(g)).join(', ') });
  expect(fast.length, 'about eight stops in 2.1 s at 4x').toBeGreaterThanOrEqual(6);
  expect(Math.min(...fastGaps)).toBeGreaterThanOrEqual(230);
  expect([...fastGaps].sort((p, q) => p - q)[Math.floor(fastGaps.length / 2)]!).toBeLessThan(400);
  // another run plays at 1x; back on this one, 4x is remembered
  await selectRun(page, 'r-20261028T152641-97f1');
  await expect(speedButton).toHaveAttribute('data-speed', '1');
  await selectRun(page, CAVITY);
  await expect(speedButton).toHaveAttribute('data-speed', '4');
  await speedButton.click();
  await page.getByRole('menuitemradio', { name: '1×' }).click();
  await expect(speedButton).toHaveAttribute('data-speed', '1');
});

test('Follow live: the line stays on the running run’s newest snapshot; moving it stops following; Follow live brings it back', async () => {
  test.skip(true, 'ADR 0818: the record of the race of reading the line and the count apart; see the corrected test after it');
  test.setTimeout(120_000);
  const { page } = studio!;
  longRun = runner!.submit('cases/cavity-re100.toml', LONG);
  await until(() => (runner!.status(longRun!).state === 'running' && runner!.status(longRun!).step > 30 ? true : null), 60_000, 'the long run to run');
  await expect(page.locator(`.ws-item-case[data-run="${longRun}"]`)).toBeVisible({ timeout: 20_000 });
  await selectRun(page, longRun);
  const follow = page.getByRole('button', { name: 'Follow live' });
  await expect(follow).toHaveAttribute('aria-pressed', 'true');
  // One Live Thing: one orange segment, the running run's
  await expect(page.locator('.tp-seg--live')).toHaveCount(1);
  await expect(page.locator(`.tp-band-row[data-run="${longRun}"] .tp-seg--live`)).toHaveCount(1);
  await expect(page.locator(`.tp-band-row[data-run="${longRun}"] .tp-end`)).toHaveAccessibleName(/^running · it \d+$/);
  await expect(page.locator(`.tp-band-row[data-run="${longRun}"] .tp-end`)).toHaveText(/^it \d+$/);
  const ws = page.locator('.ws');
  const counter = page.locator('.tp-counter');
  // following: the line is on the newest snapshot, and moves on as the runner writes more
  const first = await settled(page);
  // on the newest snapshot: "N of N" (the run may have written more since `first` when the renderer is slow, as with
  // software WebGL on Linux, so N is not pinned to first.snapshot + 1)
  await expect(counter).toHaveText(/^snapshot (\d+) of \1 computed so far$/);
  expect(Number((await counter.textContent())!.match(/of (\d+)/)![1])).toBeGreaterThanOrEqual(first.snapshot + 1);
  await until(async () => ((await attr(ws, 'data-snapshot')) > first.snapshot + 2 ? true : null), 30_000, 'the line to follow new snapshots');
  const later = await settled(page);
  expect(later.snapshot).toBeGreaterThan(first.snapshot);
  const n = Number((await counter.textContent())!.match(/of (\d+)/)![1]);
  expect(later.snapshot).toBe(n - 1);
  await shot(page, '04-follow-live');
  // moving the line stops following: it stays where it was put while the run goes on
  await scrubber(page).focus();
  await page.keyboard.press('ArrowLeft');
  await expect(follow).toHaveAttribute('aria-pressed', 'false');
  const held = await attr(ws, 'data-snapshot');
  await until(async () => (Number((await counter.textContent())!.match(/of (\d+)/)![1]) > held + 3 ? true : null), 30_000, 'more snapshots');
  expect(await attr(ws, 'data-snapshot')).toBe(held);
  // held on the live run, the cursor reads the solver's values at that step, also past the last read of its histories
  const heldStep = await attr(ws, 'data-step');
  const line = (await residualLines(longRun)).get(heldStep)!;
  const cont = page.locator('.tp-resid-values li[data-name="continuity"]');
  await expect(cont).toHaveAttribute('data-step', String(heldStep));
  await expect(cont).toHaveAttribute('data-value', String(line.residuals['continuity']));
  await expect(page.locator('.tp-readout-value')).toHaveAttribute('data-value', String(line.monitors['lid_drag']));
  // Follow live brings it to the newest again
  await follow.click();
  await expect(follow).toHaveAttribute('aria-pressed', 'true');
  await until(
    async () => {
      const m = (await counter.textContent())!.match(/^snapshot (\d+) of (\d+)/);
      return m && m[1] === m[2] ? true : null;
    },
    15_000,
    'the line on the newest snapshot',
  );
  runner!.cli('cancel', longRun);
  await until(() => (runner!.status(longRun!).state === 'cancelled' ? true : null), 30_000, 'the cancel');
  await expect(page.locator(`.tp-band-row[data-run="${longRun}"] .tp-end`)).toHaveText(/^cancelled · it \d+$/, { timeout: 15_000 });
  await expect(page.locator('.tp-seg--live')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Follow live' })).toHaveCount(0);
});

test('Follow live (ADR 0818, the line and the count read together): the line stays on the running run’s newest snapshot; moving it stops following; Follow live brings it back', async () => {
  test.setTimeout(120_000);
  const { page } = studio!;
  longRun = runner!.submit('cases/cavity-re100.toml', LONG);
  await until(() => (runner!.status(longRun!).state === 'running' && runner!.status(longRun!).step > 30 ? true : null), 60_000, 'the long run to run');
  await expect(page.locator(`.ws-item-case[data-run="${longRun}"]`)).toBeVisible({ timeout: 20_000 });
  await selectRun(page, longRun);
  const follow = page.getByRole('button', { name: 'Follow live' });
  await expect(follow).toHaveAttribute('aria-pressed', 'true');
  // One Live Thing: one orange segment, the running run's
  await expect(page.locator('.tp-seg--live')).toHaveCount(1);
  await expect(page.locator(`.tp-band-row[data-run="${longRun}"] .tp-seg--live`)).toHaveCount(1);
  await expect(page.locator(`.tp-band-row[data-run="${longRun}"] .tp-end`)).toHaveAccessibleName(/^running · it \d+$/);
  await expect(page.locator(`.tp-band-row[data-run="${longRun}"] .tp-end`)).toHaveText(/^it \d+$/);
  const ws = page.locator('.ws');
  const counter = page.locator('.tp-counter');
  // following: the line is on the newest snapshot, and moves on as the runner writes more
  const first = await settled(page);
  // on the newest snapshot: "N of N" (the run may have written more since `first` when the renderer is slow, as with
  // software WebGL on Linux, so N is not pinned to first.snapshot + 1)
  await expect(counter).toHaveText(/^snapshot (\d+) of \1 computed so far$/);
  expect(Number((await counter.textContent())!.match(/of (\d+)/)![1])).toBeGreaterThanOrEqual(first.snapshot + 1);
  await until(async () => ((await attr(ws, 'data-snapshot')) > first.snapshot + 2 ? true : null), 30_000, 'the line to follow new snapshots');
  const later = await settled(page);
  expect(later.snapshot).toBeGreaterThan(first.snapshot);
  // ADR 0818: the line and the count in one task of the page (one DOM state), until the line is on the newest; read
  // apart, the run wrote more snapshots between the two reads (the race of the record above)
  await until(
    () =>
      page.evaluate(() => {
        const s = document.querySelector('.ws')?.getAttribute('data-snapshot');
        const m = document.querySelector('.tp-counter')?.textContent?.match(/of (\d+)/);
        return s != null && m != null && Number(s) === Number(m[1]) - 1 ? true : null;
      }),
    15_000,
    'the line on the newest snapshot while following',
  );
  await shot(page, '04-follow-live');
  // moving the line stops following: it stays where it was put while the run goes on
  await scrubber(page).focus();
  await page.keyboard.press('ArrowLeft');
  await expect(follow).toHaveAttribute('aria-pressed', 'false');
  const held = await attr(ws, 'data-snapshot');
  await until(async () => (Number((await counter.textContent())!.match(/of (\d+)/)![1]) > held + 3 ? true : null), 30_000, 'more snapshots');
  expect(await attr(ws, 'data-snapshot')).toBe(held);
  // held on the live run, the cursor reads the solver's values at that step, also past the last read of its histories
  const heldStep = await attr(ws, 'data-step');
  const line = (await residualLines(longRun)).get(heldStep)!;
  const cont = page.locator('.tp-resid-values li[data-name="continuity"]');
  await expect(cont).toHaveAttribute('data-step', String(heldStep));
  await expect(cont).toHaveAttribute('data-value', String(line.residuals['continuity']));
  await expect(page.locator('.tp-readout-value')).toHaveAttribute('data-value', String(line.monitors['lid_drag']));
  // Follow live brings it to the newest again
  await follow.click();
  await expect(follow).toHaveAttribute('aria-pressed', 'true');
  await until(
    async () => {
      const m = (await counter.textContent())!.match(/^snapshot (\d+) of (\d+)/);
      return m && m[1] === m[2] ? true : null;
    },
    15_000,
    'the line on the newest snapshot',
  );
  runner!.cli('cancel', longRun);
  await until(() => (runner!.status(longRun!).state === 'cancelled' ? true : null), 30_000, 'the cancel');
  await expect(page.locator(`.tp-band-row[data-run="${longRun}"] .tp-end`)).toHaveText(/^cancelled · it \d+$/, { timeout: 15_000 });
  await expect(page.locator('.tp-seg--live')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Follow live' })).toHaveCount(0);
});

test('from the Runs view: Open in workspace shows the selected run there', async () => {
  const { page } = studio!;
  await page.getByRole('tab', { name: 'Runs' }).click();
  const grid = page.getByRole('grid', { name: /^Runs of / });
  await grid.locator(`.rv-case[data-run="${CAVITY}"]`).click();
  await page.getByRole('button', { name: 'Open in workspace' }).click();
  await expect(page.getByRole('tab', { name: 'Workspace' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('.ws')).toHaveAttribute('data-run', CAVITY);
  await settled(page);
  await shot(page, '05-open-from-runs');
});
