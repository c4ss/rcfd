// Comparing two runs (F-815) in the packaged Studio, against the real engine: the project server (rcfd-mcp, read-only)
// and the project's runner (rcfd-runner). A copy of the quick-start project (its runs store f32 fields), plus two short
// runs started here that store f64 fields (output.store_precision = "f64"), on the one cavity mesh.
//
// - Side by side: both views read the same snapshot time (the same iteration for steady runs), in one colour range that
//   is exactly the union of the two views' ranges, with one legend; the scrubber moves both.
// - A stop where the compared run has no snapshot: its view says so in words.
// - The difference field: equal, bit for bit, to an independent f64 computation from the stored bytes (and the decode
//   agrees with the engine's field_summary); drawn as f32, the readout the f32 of that value.
// - Different meshes: the difference is refused, saying why.
// - The residual plot overlays both runs; compare_runs' metrics; the keyboard; the palette's Compare runs.
// Screenshots at 1440 by 900 go to test-results-visual/f815/ for the maintainer's view-and-report (G13).
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { appRoot, killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { until } from '../support/fake-service.js';
import { bits64, decodeField64, snapshotDirs } from '../support/rts-decode.js';
import { Store } from '../../src/main/data/tsstore.js';
import { ProjectServer } from '../../src/main/project/server.js';
import { f32Text, tickLabel } from '../../src/renderer/field/colormap.js';
import { shownRange } from '../../src/renderer/field/surface.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f815');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.describe.configure({ mode: 'serial' });

/** quick-start: the lid-driven cavities at Re 100 and Re 400 share their 32 x 32 mesh; the heated cavity has 40 x 40. */
const RE100 = 'r-20261028T152641-f8c2';
const RE400 = 'r-20261028T152641-97f1';
const HEATED = 'r-20261028T152641-3900';
/** Short runs of the two cavities that store f64 fields: 20 iterations, a snapshot every 5. */
const F64 = ['output.store_precision=f64', 'solver.max_iterations=20'];

let dir = '';
let quick = '';
let runner: Runner | null = null;
let f64a = '';
let f64b = '';
let studio: Studio | null = null;

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const scrubber = (page: Page): Locator => page.getByRole('slider', { name: /^Time of / });
const fieldsOf = (run: string): string => path.join(quick, 'runs', run, 'fields');
const view = (page: Page, v: 'a' | 'b' | 'difference'): Locator => page.locator(`.cmp-view[data-view="${v}"]`);
const frame = (page: Page, v: 'a' | 'b' | 'difference'): Locator => view(page, v).locator('.fv-frame');

async function selectRun(page: Page, id: string): Promise<void> {
  await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${id}"]`).click();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', id);
}

/** Turns comparing on (when off) and chooses the compared run, by pointer. */
async function compareWith(page: Page, other: string, mode: 'Side by side' | 'Difference' = 'Side by side'): Promise<void> {
  const toggle = page.getByRole('button', { name: 'Compare runs', exact: true });
  if ((await toggle.getAttribute('aria-pressed')) !== 'true') await toggle.click();
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: /Compared run/ }).click();
  await page.locator('.ui-popover').getByRole('option', { name: new RegExp(other) }).click();
  await expect(page.locator('.cmp-view[data-view="b"], .cmp-view[data-view="difference"], .cmp-refusal').first()).toBeVisible();
  await page.getByRole('radio', { name: mode, exact: true }).click();
  await expect(page.getByRole('radio', { name: mode, exact: true })).toBeChecked();
}

async function stopAt(page: Page, index: number): Promise<void> {
  await scrubber(page).focus();
  await page.keyboard.press('Home');
  for (let i = 0; i < index; i++) await page.keyboard.press('ArrowRight');
  await expect(scrubber(page)).toHaveAttribute('aria-valuenow', String(index));
}

/** Waits until both views show the stop: A at the workspace's snapshot, B at the comparison's, in the one range. */
async function settledSide(page: Page): Promise<{ a: number; b: number; step: number }> {
  return until(
    async () => {
      const want = await page.locator('.ws').getAttribute('data-snapshot');
      const wantB = await page.locator('.cmp').getAttribute('data-b-snapshot');
      const a = frame(page, 'a');
      const b = frame(page, 'b');
      const ok =
        want !== null &&
        wantB !== null &&
        (await a.getAttribute('data-snapshot')) === want &&
        (await b.getAttribute('data-snapshot')) === wantB &&
        (await a.getAttribute('data-ready')) !== null &&
        (await b.getAttribute('data-ready')) !== null &&
        (await a.getAttribute('data-range')) === (await page.locator('.cmp').getAttribute('data-range')) &&
        (await b.getAttribute('data-range')) === (await page.locator('.cmp').getAttribute('data-range'));
      return ok ? { a: Number(want), b: Number(wantB), step: Number(await page.locator('.ws').getAttribute('data-step')) } : null;
    },
    20_000,
    'both views to show the stop in one range',
  );
}

/** The F-808 range of a stored field of a snapshot: min and max of its values over the cells that own surface triangles. */
function ownRange(dirOf: string, snapshot: number, field: string, values?: Float32Array): { lo: number; hi: number } {
  const s = new Store(dirOf);
  try {
    const cells = new Uint32Array(s.surface().cells);
    return shownRange(cells, values ?? new Float32Array(s.field(snapshot, field)))!;
  } finally {
    s.close();
  }
}

async function residualLines(run: string): Promise<Map<number, { residuals: Record<string, number> }>> {
  const text = await readFile(path.join(quick, 'runs', run, 'residuals.jsonl'), 'utf8');
  const out = new Map<number, { residuals: Record<string, number> }>();
  for (const line of text.split('\n')) {
    if (line.trim() === '') continue;
    const j = JSON.parse(line) as { step: number; residuals: Record<string, number> };
    out.set(j.step, j);
  }
  return out;
}

test.beforeAll(async () => {
  test.setTimeout(240_000);
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f815-'));
  quick = await copyExample('quick-start', dir);
  await mkdir(SHOTS, { recursive: true });
  runner = new Runner(runnerExe!, quick);
  runner.ensure();
  f64a = runner.submit('cases/cavity-re100.toml', F64);
  f64b = runner.submit('cases/cavity-re400.toml', F64);
  for (const id of [f64a, f64b]) await until(() => (['finished', 'failed', 'cancelled'].includes(runner!.status(id).state) ? true : null), 120_000, `the f64 run ${id}`);
  for (const id of [f64a, f64b]) expect(runner.status(id).state).toBe('finished');
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => studio!.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await stubDialog(studio.app, quick);
  await studio.page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(studio.page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  await studio.page.getByRole('tab', { name: 'Workspace' }).click();
  await expect(studio.page.getByRole('listbox', { name: 'Runs of the project' })).toBeVisible({ timeout: 20_000 });
  await expect(studio.page.locator(`.ws-item-case[data-run="${f64b}"]`)).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  if (studio) {
    await studio.app.close().catch(() => undefined);
    studio = null;
  }
  await runner?.cleanUp(null);
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('side by side: both views read the same snapshot time, in one colour range with one legend; the scrubber moves both', async () => {
  const { page } = studio!;
  await selectRun(page, RE100);
  // at rest the comparison adds no visible words: its controls are pictograms (G22)
  const toggle = page.getByRole('button', { name: 'Compare runs', exact: true });
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  expect((await toggle.innerText()).trim()).toBe('');
  await expect(page.locator('.cmp')).toHaveCount(0);
  await compareWith(page, RE400);
  // the only visible words of the controls are the compared run's name (data); the rest are pictograms
  await expect(page.getByRole('button', { name: /Compared run/ })).toHaveText(`cavity-re400.toml · ${RE400}`);
  for (const [role, name] of [['button', 'Compare runs'], ['radio', 'Side by side'], ['radio', 'Difference']] as const) {
    expect((await page.locator('.cmp-controls').getByRole(role, { name, exact: true }).innerText()).trim(), name).toBe('');
  }
  await stopAt(page, 7);
  const { a, b, step } = await settledSide(page);
  const sa = new Store(fieldsOf(RE100));
  const sb = new Store(fieldsOf(RE400));
  try {
    // the same snapshot time: steady runs, so the same iteration; each view's snapshot is its store's at that step
    expect(sa.snapshots[a]!.step).toBe(step);
    expect(sb.snapshots[b]!.step).toBe(step);
    await expect(frame(page, 'a')).toHaveAttribute('data-step', String(step));
    await expect(frame(page, 'b')).toHaveAttribute('data-step', String(step));
    await expect(view(page, 'a').locator('.fv-caption')).toContainText(`${RE100} · p`);
    await expect(view(page, 'a').locator('.fv-caption')).toContainText(`· step ${step} ·`);
    await expect(view(page, 'b').locator('.fv-caption')).toContainText(`${RE400} · p`);
    await expect(view(page, 'b').locator('.fv-caption')).toContainText(`· step ${step} ·`);
  } finally {
    sa.close();
    sb.close();
  }
  // one range: exactly the union of the two views' F-808 ranges, drawn by both and shown by the one legend
  const ra = ownRange(fieldsOf(RE100), a, 'p');
  const rb = ownRange(fieldsOf(RE400), b, 'p');
  const lo = Math.min(ra.lo, rb.lo);
  const hi = Math.max(ra.hi, rb.hi);
  expect([ra.lo, ra.hi]).not.toEqual([rb.lo, rb.hi]);
  await expect(frame(page, 'a')).toHaveAttribute('data-range', `${lo} ${hi}`);
  await expect(frame(page, 'b')).toHaveAttribute('data-range', `${lo} ${hi}`);
  await expect(page.locator('.cmp .fv-legend')).toHaveCount(1);
  const unitStore = new Store(fieldsOf(RE100));
  const unit = unitStore.fields(0).find((f) => f.name === 'p')!.unit;
  unitStore.close();
  const title = unit ? `p, ${unit}` : 'p';
  await expect(page.locator('.cmp-legend').getByRole('group', { name: `Colour map viridis, ${title}, from ${tickLabel(lo)} to ${tickLabel(hi)}` })).toHaveCount(1);
  await expect(view(page, 'a').locator('.fv-caption')).toContainText('viridis, one range for both runs');
  await shot(page, '01-side-by-side');
  // the scrubber drives both
  await scrubber(page).focus();
  await page.keyboard.press('ArrowRight');
  const next = await settledSide(page);
  expect(next.step).toBeGreaterThan(step);
  await expect(frame(page, 'b')).toHaveAttribute('data-step', String(next.step));
  // ADR 0801 Addendum 22: each view has its own camera (orthographic, the 2D cavities face on); one moves alone
  const ca = frame(page, 'a').locator('canvas');
  const cb = frame(page, 'b').locator('canvas');
  for (const c of [ca, cb]) {
    await expect(c).toHaveAttribute('data-camera', 'orthographic');
    await expect(c).toHaveAttribute('data-camera-moved', 'false');
  }
  const eyeB = await cb.getAttribute('data-camera-eye');
  const box = (await ca.boundingBox())!;
  await page.mouse.move(Math.round(box.x + box.width / 2), Math.round(box.y + box.height / 2));
  await page.mouse.wheel(0, -240);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(Math.round(box.x + box.width / 2) + 30, Math.round(box.y + box.height / 2), { steps: 3 });
  await page.mouse.up({ button: 'middle' });
  await expect(ca).toHaveAttribute('data-camera-moved', 'true');
  await expect(cb).toHaveAttribute('data-camera-moved', 'false');
  expect(await cb.getAttribute('data-camera-eye')).toBe(eyeB);
  await page.mouse.move(5, 5);
  await shot(page, '01b-side-by-side-one-view-moved');
  await view(page, 'a').getByRole('button', { name: 'Reset view' }).click();
  await expect(ca).toHaveAttribute('data-camera-moved', 'false');
});

test('a stop where the compared run has no snapshot: its view says so in words, and so does the difference', async () => {
  const { page } = studio!;
  await selectRun(page, RE400);
  await compareWith(page, RE100);
  await scrubber(page).focus();
  await page.keyboard.press('End');
  const step = Number(await page.locator('.ws').getAttribute('data-step'));
  const sb = new Store(fieldsOf(RE100));
  const last = sb.snapshots[sb.snapshots.length - 1]!.step;
  sb.close();
  expect(step).toBeGreaterThan(last);
  const refusal = page.getByRole('status').filter({ hasText: `No snapshot at it ${step}` });
  await expect(refusal).toBeVisible({ timeout: 20_000 });
  await expect(page.locator('.cmp-refusal[data-view="b"]')).toHaveText(`No snapshot at it ${step}`);
  await expect(frame(page, 'a')).toHaveAttribute('data-ready', '');
  await shot(page, '02-no-snapshot-at-that-time');
  await page.getByRole('radio', { name: 'Difference', exact: true }).click();
  await expect(page.locator('.cmp-refusal[data-view="difference"]')).toHaveText(`No snapshot at it ${step}`);
  await page.getByRole('radio', { name: 'Side by side', exact: true }).click();
});

test('different meshes: no difference field, and the refusal says why; side by side still shows both', async () => {
  const { page } = studio!;
  await selectRun(page, RE100);
  await compareWith(page, HEATED, 'Difference');
  await stopAt(page, 7);
  const refusal = page.getByRole('status').filter({ hasText: 'No difference: the meshes differ: 1024 and 1600 cells' });
  await expect(refusal).toBeVisible({ timeout: 20_000 });
  await expect(page.locator('.cmp-view[data-view="difference"]')).toHaveCount(0);
  await shot(page, '03-different-meshes-refused');
  await page.getByRole('radio', { name: 'Side by side', exact: true }).click();
  const { step } = await settledSide(page);
  await expect(frame(page, 'b')).toHaveAttribute('data-step', String(step));
});

test('the difference field equals an independent f64 computation from the stored f64 bytes, bit for bit', async () => {
  const { page } = studio!;
  expect(snapshotDirs(fieldsOf(f64a))[0]!.fields.every((f) => f.precision === 'f64')).toBe(true);
  await selectRun(page, f64a);
  await compareWith(page, f64b, 'Difference');
  await stopAt(page, 1);
  const diff = frame(page, 'difference');
  await until(async () => ((await diff.getAttribute('data-ready')) !== null && (await diff.getAttribute('data-snapshot')) === '1' ? true : null), 20_000, 'the difference');
  const sa = snapshotDirs(fieldsOf(f64a));
  const sb = snapshotDirs(fieldsOf(f64b));
  const j = sb.findIndex((s) => s.step === sa[1]!.step);
  expect(j).toBeGreaterThanOrEqual(0);
  await expect(page.locator('.cmp')).toHaveAttribute('data-b-snapshot', String(j));
  // the independent computation: both fields decoded from the stored bytes, subtracted in f64
  const va = decodeField64(fieldsOf(f64a), 1, 'p');
  const vb = decodeField64(fieldsOf(f64b), j, 'p');
  const want = [...vb].map((x, c) => x - va[c]!);
  expect(want.some((x) => x !== Math.fround(x))).toBe(true); // f64 values the f32 path would not hold
  // the decode agrees with the engine's own reader (field_summary of the same step, in f64)
  const server = await ProjectServer.start(mcp!, quick);
  try {
    const sum = (await server.call('field_summary' as never, { run: f64a, field: 'p', step: sa[1]!.step })) as { min: number; max: number };
    expect([sum.min, sum.max]).toEqual([Math.min(...va), Math.max(...va)]);
  } finally {
    await server.close();
  }
  // what the Studio computed, through the real path (preload, data port, the reader's worker), every value
  const got = await page.evaluate(
    async ([a, b, jj]) => {
      const r = await window.studioCompare.difference({ route: 'local', path: a }, 1, { route: 'local', path: b }, jj, 'p');
      if (!r.ok) return r.error.message;
      const dv = new DataView(r.result.values.buffer);
      const out: string[] = [];
      for (let i = 0; i < r.result.values.length; i++) out.push(dv.getBigUint64(8 * i, true).toString(16).padStart(16, '0'));
      return out;
    },
    [fieldsOf(f64a), fieldsOf(f64b), j] as const,
  );
  expect(got).toEqual(want.map(bits64));
  // drawn as f32 with the F-808 range of the drawn values; the readout is the f32 of the cell's difference
  const drawn = Float32Array.from(want);
  const r = ownRange(fieldsOf(f64a), 1, 'p', drawn);
  await expect(diff).toHaveAttribute('data-range', `${r.lo} ${r.hi}`);
  await diff.focus();
  await page.keyboard.press('+');
  const out = view(page, 'difference').locator('.fv-readout-value');
  await expect(out).toHaveAttribute('data-cell', /\d+/);
  const cell = Number(await out.getAttribute('data-cell'));
  await expect(out).toHaveText(f32Text(Math.fround(want[cell]!)));
  await expect(view(page, 'difference').locator('.fv-legend-title')).toHaveText(/^Δp/);
  await expect(view(page, 'difference').locator('.fv-caption')).toContainText(`${f64b} − ${f64a} · Δp`);
  await shot(page, '04-difference-f64');
});

test('the residual plot overlays both runs; the compared run’s band keeps its fill; compare_runs gives the metrics', async () => {
  const { page } = studio!;
  const drawn = async (): Promise<number> => Number(await page.locator('.tp-resid-plot .plot-frame').getAttribute('data-drawn'));
  const toggle = page.getByRole('button', { name: 'Compare runs', exact: true });
  const off = async () => {
    if ((await toggle.getAttribute('aria-pressed')) === 'true') await toggle.click();
    await expect(page.locator('.cmp')).toHaveCount(0);
  };
  // each run alone first
  await selectRun(page, RE400);
  await off();
  await expect.poll(drawn).toBeGreaterThan(0);
  const alone400 = await drawn();
  await selectRun(page, RE100);
  await off();
  await expect.poll(drawn).toBeGreaterThan(0);
  const alone100 = await drawn();
  await compareWith(page, RE400);
  await expect.poll(drawn).toBe(alone100 + alone400);
  await stopAt(page, 7);
  const step = Number(await page.locator('.ws').getAttribute('data-step'));
  const line = (await residualLines(RE400)).get(step)!;
  const list = page.getByRole('list', { name: 'Residuals of cavity-re400.toml at the line' });
  for (const [shown, eq] of Object.entries({ continuity: 'continuity', res_u: 'u', res_p: 'p' })) {
    const li = list.locator(`li[data-name="${shown}"]`);
    await expect(li).toHaveAttribute('data-step', String(step));
    expect(Number(await li.getAttribute('data-value'))).toBe(line.residuals[eq]);
  }
  const band = page.locator(`.tp-band-row[data-run="${RE400}"]`);
  await expect(band).toHaveClass(/tp-band-row--compared/);
  await expect(band.locator('.tp-band')).not.toHaveClass(/tp-band--faded/);
  // the engine's metrics of both and its difference (compare_runs)
  const metrics = (run: string) => readFile(path.join(quick, 'runs', run, 'metrics.json'), 'utf8').then((t) => (JSON.parse(t) as { reports: Record<string, number> }).reports);
  const ma = await metrics(RE100);
  const mb = await metrics(RE400);
  const row = page.getByRole('table', { name: /^Metrics of cavity-re100.toml and cavity-re400.toml/ }).locator('tr[data-metric="lid_drag"]');
  await expect(row).toHaveAttribute('data-a', String(ma['lid_drag']));
  await expect(row).toHaveAttribute('data-b', String(mb['lid_drag']));
  await expect(row).toHaveAttribute('data-difference', String(mb['lid_drag']! - ma['lid_drag']!));
  await shot(page, '05-residuals-overlaid');
});

test('the keyboard: the compare toggle, the compared run and the view from the keys; the scrubber moves both', async () => {
  const { page } = studio!;
  await selectRun(page, RE100);
  const toggle = page.getByRole('button', { name: 'Compare runs', exact: true });
  if ((await toggle.getAttribute('aria-pressed')) === 'true') await toggle.click();
  await page.mouse.move(5, 5);
  await toggle.focus();
  await page.keyboard.press('Space');
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  expect(await toggle.evaluate((e) => e.matches(':focus-visible'))).toBe(true);
  // the compared run: Tab to its select, open it with the keys and choose the Re 400 cavity
  await page.keyboard.press('Tab');
  const select = page.getByRole('button', { name: /Compared run/ });
  await expect(select).toBeFocused();
  await page.keyboard.press('Enter');
  const options = page.locator('.ui-popover').getByRole('option');
  await expect(options.first()).toBeVisible();
  const names = await options.allInnerTexts();
  const k = names.findIndex((n) => n.includes(RE400));
  expect(k).toBeGreaterThanOrEqual(0);
  // the listbox opens on the selected option; go to the top, then down to the wanted one
  await page.keyboard.press('Home');
  for (let i = 0; i < k; i++) await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect(select).toContainText(RE400);
  // the closed listbox gives the focus back to its Select before the next key
  await expect(page.locator('.ui-popover')).toHaveCount(0);
  await expect(select).toBeFocused();
  // the view: Tab into the group, the arrow keys move between its buttons, Space chooses
  await page.keyboard.press('Tab');
  await expect(page.getByRole('radio', { name: 'Side by side', exact: true })).toBeFocused();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('radio', { name: 'Difference', exact: true })).toBeFocused();
  await page.keyboard.press('Space');
  await expect(page.locator('.cmp')).toHaveAttribute('data-mode', 'difference');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('Space');
  await expect(page.locator('.cmp')).toHaveAttribute('data-mode', 'side');
  // the scrubber moves both views
  await stopAt(page, 3);
  const first = await settledSide(page);
  await page.keyboard.press('ArrowRight');
  const second = await settledSide(page);
  expect(second.a).toBe(first.a + 1);
  expect(second.b).toBe(first.b + 1);
  await shot(page, '06-keyboard');
});

test('the palette’s Compare runs asks which run, refuses the selected one, and opens the comparison', async () => {
  const { page } = studio!;
  await selectRun(page, RE100);
  const toggle = page.getByRole('button', { name: 'Compare runs', exact: true });
  if ((await toggle.getAttribute('aria-pressed')) === 'true') await toggle.click();
  await page.getByRole('tab', { name: 'Home' }).click();
  await page.keyboard.press('Control+K');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  await expect(dialog).toBeVisible();
  await page.keyboard.type('Compare runs');
  await expect(dialog.getByRole('option', { name: /^Compare runs/ })).toBeVisible();
  await page.keyboard.press('Enter');
  const runs = dialog.getByRole('listbox', { name: 'Choose a run' });
  await expect(runs).toBeVisible();
  await expect(runs.getByRole('option').first()).toContainText(RE100);
  await expect(runs.getByRole('option').first()).toContainText('Unavailable: This is the selected run');
  const wanted = runs.getByRole('option').filter({ hasText: RE400 });
  const n = await runs.getByRole('option').count();
  let k = 0;
  for (let i = 0; i < n; i++) if ((await runs.getByRole('option').nth(i).innerText()).includes(RE400)) k = i;
  // nothing is focused in the list until the first ArrowDown
  for (let i = 0; i <= k; i++) await page.keyboard.press('ArrowDown');
  await expect(wanted).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(dialog).toBeHidden();
  await expect(page.getByRole('tab', { name: 'Workspace' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('button', { name: 'Compare runs', exact: true })).toHaveAttribute('aria-pressed', 'true', { timeout: 20_000 });
  // the Re 100 run opens on its last snapshot (it 121), which the Re 400 run did not store: said in words
  await expect(page.locator('.cmp-refusal[data-view="b"]')).toHaveText(/^No snapshot at it \d+$/);
  await stopAt(page, 7);
  await expect(view(page, 'b')).toHaveAttribute('data-run', RE400);
  await settledSide(page);
  await shot(page, '07-from-the-palette');
});
