// The derived pictures of the field viewport in the packaged Studio (F-809; guardrails G3, G6, G8, G13, G22), at
// #/dev/field, on real stores read by route R2: the f447 boxes made afresh by `f447_bench gen` (3D, the Rust reader's
// reference bytes) and a copy of the quick-start cavity's store (2D).
//
// - G6 and G22: on the 2D case the cut plane and the iso-surface are disabled, each with its reason in its accessible
//   name and in the tooltip its keyboard focus shows; no reason stands on the screen; on the 3D box all four apply;
// - ADR 0738: every derived picture is the engine's (`rcfd post derive`, run by main; the worktree's rcfd by
//   RCFD_STUDIO_RCFD): the frame names the cut it shows (data-cut) once the engine's cut surface is drawn;
// - T2, pixels: with the cut plane on, at grid points away from edges, every coloured pixel is the table colour of the
//   stored value of the cell the readout names, the readout's point is on the kept side, and each point whose unclipped
//   hit was on the cut-away side now reads a different triangle on the kept side (or the ground);
// - T3, pixels: the arrows change only pixels to deep ink; their number is the recomputation's (derived.ts on the
//   surface the page got and the Rust reader's components), which is the engine's count;
// - the engine's iso-surface of the 1e5 box and streamlines of the box and the 2D cavity: drawn (data-iso,
//   data-streamlines are the engine's counts), the level and the seeds set from the keyboard;
// - keyboard: the controls are reached by Tab, toggled by Space, the cut plane's axis, position and side are set from
//   the keyboard; focus is visible;
// - screenshots at 1440 x 900 in test-results-visual/f809/ for the maintainer's view-and-report.
//
// Needs `f447_bench` (tests/support/f447.ts); without it these tests are skipped and say why.
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { cpSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { colourOf, colourRange, lutBytes, lutIndex, lutPlace, type ColourRange } from '../../src/renderer/field/colormap.js';
import { meshLayers, vectorGlyphs, type Cut } from '../../src/renderer/field/derived.js';
import { findBench, generate, NO_BENCH, refBytes, storeDir } from '../support/f447.js';
import { decodePng, pixel, uniformAround, type Rgba } from '../support/png.js';
import { repoRoot } from '../support/project.js';
import { appRoot, findRcfd } from '../support/rcfd.js';
import { launchStudio } from '../support/studio.js';

const bench = findBench();
test.skip(bench === null, NO_BENCH);
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f809');
const SNAPSHOT = 19;
const GROUND: [number, number, number] = [0xee, 0xf1, 0xf4]; // silver-wash, the viewport
const INK: [number, number, number] = [0x1d, 0x3a, 0x5f]; // deep ink, the arrows
const lut = lutBytes();

let root = '';
let quick = '';
let app: ElectronApplication;
let page: Page;
const consoleErrors: string[] = [];

interface Shown {
  values: Float32Array;
  cells: Uint32Array;
  points: Float32Array;
  triangles: Uint32Array;
  range: ColourRange;
  nCells: number;
}

async function surfaceOf(store: string, values: Float32Array, nCells: number): Promise<Shown> {
  const s = await page.evaluate(async (p) => {
    const r = await window.studioData.surface({ route: 'local', path: p });
    if (!r.ok) throw new Error(r.error.message);
    return { cells: [...r.result.cells], triangles: [...r.result.triangles], points: [...r.result.points] };
  }, store);
  const cells = new Uint32Array(s.cells);
  let lo = Infinity;
  let hi = -Infinity;
  for (const c of cells) {
    lo = Math.min(lo, values[c]!);
    hi = Math.max(hi, values[c]!);
  }
  return { values, cells, points: new Float32Array(s.points), triangles: new Uint32Array(s.triangles), range: colourRange(lo, hi), nCells };
}

const refField = (size: string, f: string): Float32Array => {
  const b = refBytes(root, size, SNAPSHOT, f);
  return new Float32Array(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
};

test.beforeAll(async () => {
  test.setTimeout(400_000);
  root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f809-e2e-'));
  await generate(bench!, root, ['tiny', '1e5'], 360_000);
  // the quick-start cavity's store (2D), copied: the example project is never read in place
  const runs = path.join(repoRoot, 'example-projects', 'quick-start', 'runs');
  const run = readdirSync(runs).sort()[0]!;
  quick = path.join(root, 'quick-start', run, 'fields');
  cpSync(path.join(runs, run, 'fields'), quick, { recursive: true });
  // ADR 0738: the engine that makes the derived pictures (the worktree's, as the tests find it)
  const rcfd = findRcfd();
  ({ app, page } = await launchStudio({ RCFD_STUDIO_DATA_ROOTS: root, ...(rcfd ? { RCFD_STUDIO_RCFD: rcfd } : {}) }));
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await page.evaluate(() => {
    const w = window as unknown as { __csp: string[] };
    w.__csp = [];
    document.addEventListener('securitypolicyviolation', (e) => w.__csp.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
  mkdirSync(SHOTS, { recursive: true });
});

test.afterAll(async () => {
  await app?.close();
  if (root) rmSync(root, { recursive: true, force: true });
});

async function show(store: string, field: string, snapshot: number): Promise<void> {
  await page.evaluate(() => (location.hash = '#/dev/field'));
  await expect(page.locator('.fd-empty')).toBeVisible();
  await page.evaluate(
    ([p, f, snap]) => (location.hash = `#/dev/field?route=local&path=${encodeURIComponent(p!)}&field=${f}&snapshot=${snap}`),
    [store, field, String(snapshot)],
  );
  await page.waitForFunction(() => document.querySelector('.fv-frame[data-ready]') !== null, undefined, { timeout: 60_000 });
  await settle();
}

const settle = () => page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
const frameBox = async () => (await page.locator('.fv-frame').boundingBox())!;
const group = () => page.getByRole('group', { name: 'Derived pictures' });

async function shot(name?: string): Promise<{ img: Rgba; dpr: number }> {
  await settle();
  const buf = await page.screenshot();
  if (name) writeFileSync(path.join(SHOTS, `${name}.png`), buf);
  return { img: decodePng(buf), dpr: await page.evaluate(() => devicePixelRatio) };
}

interface Readout {
  cell: number;
  triangle: number;
  point: [number, number, number];
}

async function hoverAt(x: number, y: number): Promise<Readout | null> {
  await page.mouse.move(x, y);
  // the readout of this point: the viewport names the pointer position its readout was taken at
  await page.waitForFunction((at) => document.querySelector('.fv-readout')?.getAttribute('data-pointer') === at, `${x},${y}`, { timeout: 10_000 });
  return page.evaluate(() => {
    const o = document.querySelector('.fv-readout-value');
    if (!o) return null;
    const pt = (o.getAttribute('data-point') ?? '').split(' ').map(Number) as [number, number, number];
    return { cell: Number(o.getAttribute('data-cell')), triangle: Number(o.getAttribute('data-triangle')), point: pt };
  });
}

function allowedEntries(v: number, r: ColourRange): number[] {
  const k = lutIndex(v, r);
  const frac = lutPlace(v, r) + 0.5 - Math.floor(lutPlace(v, r) + 0.5);
  if (frac < 1e-3) return [k, k - 1].filter((x) => x >= 0);
  if (frac > 1 - 1e-3) return [k, k + 1].filter((x) => x < 256);
  return [k];
}

const isTable = (got: number[], v: number, r: ColourRange) => allowedEntries(v, r).some((k) => [lut[4 * k], lut[4 * k + 1], lut[4 * k + 2]].every((c, q) => c === got[q]));

/** The cut plane as the page holds it, read back from its panel's controls. */
async function cutNow(): Promise<Cut> {
  const axis = (await page.getByRole('group', { name: 'Cut plane', exact: true }).getByRole('radio', { checked: true }).textContent()) ?? '';
  const at = Number(await page.getByRole('slider', { name: /^Plane at [xyz], m$/ }).inputValue());
  const label = (await page.getByRole('group', { name: 'Cut plane', exact: true }).getByRole('button').last().getAttribute('aria-label')) ?? '';
  return { axis: (['x', 'y', 'z'].indexOf(axis.trim()) as 0 | 1 | 2), at: Math.fround(at), keep: label.includes('now below') ? 'below' : 'above' };
}

/** ADR 0738: waits until the frame shows the engine's cut surface of the cut the panel holds. */
async function cutDrawn(): Promise<Cut> {
  const c = await cutNow();
  await expect(page.locator('.fv-frame')).toHaveAttribute('data-cut', `${c.axis} ${c.at} ${c.keep}`, { timeout: 30_000 });
  await settle();
  return c;
}

// ---- G6 and G22: what does not apply, and why --------------------------------------------------------------------------

test('a 2D case: the cut plane and the iso-surface are disabled with the reason in their names and tooltips', async () => {
  await show(quick, 'ux', 0);
  const g = group();
  const cut = g.getByRole('button', { name: /^Cut plane, unavailable: This is a 2D case \(one cell layer, thinnest along z\)/ });
  await expect(cut).toHaveAttribute('aria-disabled', 'true');
  const iso = g.getByRole('button', { name: 'Iso-surface, unavailable: A 2D case (one cell layer) has no volume for an iso-surface.' });
  await expect(iso).toHaveAttribute('aria-disabled', 'true');
  // ADR 0738: the engine's streamlines apply (the store holds ux and uy)
  const stream = g.getByRole('button', { name: 'Streamlines', exact: true });
  await expect(stream).toBeEnabled();
  // the in-plane arrows apply (the store holds ux and uy)
  const vectors = g.getByRole('button', { name: 'Vectors', exact: true });
  await expect(vectors).toBeEnabled();
  await expect(vectors).toHaveAttribute('aria-pressed', 'false');
  // G22: no reason stands on the screen; it is the focus detail (the tooltip), shown on keyboard focus
  await expect(page.getByText(/unavailable/)).toHaveCount(0);
  await page.getByRole('button', { name: 'Mesh grid' }).focus();
  await page.keyboard.press('Tab');
  await expect(cut).toBeFocused();
  await expect(page.getByRole('tooltip')).toContainText('This is a 2D case (one cell layer, thinnest along z)');
  // pressing it does nothing: no panel, no cut
  await page.keyboard.press('Space');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('group', { name: 'Cut plane', exact: true })).toHaveCount(0);
  await shot('2d-cut-unavailable');
  await page.keyboard.press('Tab');
  await expect(vectors).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(iso).toBeFocused();
  await expect(page.getByRole('tooltip')).toContainText('has no volume for an iso-surface');
  // focus is visible on the chip: the product's orange outline
  const outline = await iso.evaluate((e) => getComputedStyle(e).outlineStyle);
  expect(outline).toBe('solid');
});

test('a 3D box: the cut plane, vectors, the iso-surface and streamlines apply (the engine makes them, ADR 0738)', async () => {
  await show(storeDir(root, 'tiny', 'raw'), 'p', SNAPSHOT);
  const g = group();
  for (const name of ['Cut plane', 'Vectors', 'Iso-surface', 'Streamlines']) await expect(g.getByRole('button', { name, exact: true }), name).toBeEnabled();
  await expect(page.getByText(/unavailable/)).toHaveCount(0);
});

// ---- T2: rcfd-render's cut plane, pixel by pixel ------------------------------------------------------------------------

async function checkCut(size: string, n: [number, number], minChanged: number) {
  const store = storeDir(root, size, 'raw');
  await show(store, 'p', SNAPSHOT);
  const values = refField(size, 'p');
  const s = await surfaceOf(store, values, values.length);
  const box = await frameBox();
  // before: the unclipped hits at a grid of points
  const before = new Map<string, Readout | null>();
  const pts: Array<[number, number]> = [];
  for (let i = 1; i < n[0]; i++)
    for (let j = 1; j < n[1]; j++) {
      const x = Math.round(box.x + (box.width * i) / n[0]);
      const y = Math.round(box.y + (box.height * j) / n[1]);
      pts.push([x, y]);
      before.set(`${x},${y}`, await hoverAt(x, y));
    }
  const first = await shot();
  // ADR 0738 P5: a local store names no length unit; the lengths say the engine's documented one until a reply names it
  await expect(page.locator('.fv-domain')).toHaveAttribute('data-length-unit', 'documented');
  // the cut plane on, from the keyboard
  await page.getByRole('button', { name: 'Cut plane', exact: true }).focus();
  await page.keyboard.press('Space');
  await expect(page.getByRole('group', { name: 'Cut plane', exact: true })).toBeVisible();
  const cut = await cutDrawn();
  expect(cut.axis).toBe(0);
  expect(cut.keep).toBe('below');
  // the engine's cut surface names the stored mesh's length unit (length_unit of its reply): the domain and the
  // cut panel's position (its slider's name, read by cutNow) say it
  await expect(page.locator('.fv-domain')).toHaveAttribute('data-length-unit', 'derive');
  await expect(page.locator('.fv-domain')).toHaveText(/ m$/);
  await expect(page.locator('.fv-domain')).toHaveAttribute('title', /the unit the engine names for the stored mesh \(its derived picture, length_unit\)/);
  const after = await shot(`${size}-cut-x`);
  const panel = (await page.getByRole('group', { name: 'Cut plane', exact: true }).boundingBox())!;
  const r = Math.ceil(2 * after.dpr);
  const bad: string[] = [];
  let judged = 0;
  let changed = 0;
  for (const [x, y] of pts) {
    if (x >= panel.x - 4 && x <= panel.x + panel.width + 4 && y >= panel.y - 4 && y <= panel.y + panel.height + 4) continue;
    if (!uniformAround(after.img, x * after.dpr, y * after.dpr, r)) continue;
    const got = pixel(after.img, x * after.dpr, y * after.dpr);
    const h = await hoverAt(x, y);
    const was = before.get(`${x},${y}`) ?? null;
    if (got.every((c, k) => c === GROUND[k])) {
      if (h) bad.push(`${x},${y}: ground but the readout names cell ${h.cell}`);
    } else {
      if (!h) {
        bad.push(`${x},${y}: coloured ${got} but no readout`);
        continue;
      }
      judged++;
      if (!(h.point[cut.axis] <= cut.at)) bad.push(`${x},${y}: the point ${h.point} is on the cut-away side of ${cut.at}`);
      if (!isTable(got, s.values[h.cell]!, s.range)) bad.push(`${x},${y}: cell ${h.cell}: pixel ${got}, table ${colourOf(lut, s.values[h.cell]!, s.range)}`);
    }
    if (was && !(was.point[cut.axis] <= cut.at)) {
      // the unclipped hit is cut away: the pixel now shows something else, on the kept side, or the ground
      changed++;
      if (h && h.triangle === was.triangle) bad.push(`${x},${y}: still triangle ${h.triangle} beyond the plane`);
    }
  }
  expect(bad).toEqual([]);
  expect(judged).toBeGreaterThanOrEqual(20);
  expect(changed).toBeGreaterThanOrEqual(minChanged);
  void first;
  return cut;
}

test('T2: the cut plane of the tiny box follows rcfd-render’s rule, pixel by pixel (WebGPU)', async () => {
  test.setTimeout(240_000);
  await checkCut('tiny', [28, 18], 5);
});

test('T2: the cut plane of the 1e5-cell box follows rcfd-render’s rule; axis, position and side from the keyboard', async () => {
  test.setTimeout(300_000);
  const cut = await checkCut('1e5', [36, 24], 5);
  // the position from the keyboard: Page Up moves the plane, the shown position is the plane's
  const slider = page.getByRole('slider', { name: /^Plane at [xyz], m$/ });
  await slider.focus();
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  const moved = await cutDrawn();
  expect(moved.at).toBeLessThan(cut.at);
  // the side kept
  const flip = page.getByRole('group', { name: 'Cut plane', exact: true }).getByRole('button', { name: /Keep the side above the plane/ });
  await flip.focus();
  await page.keyboard.press('Enter');
  expect((await cutDrawn()).keep).toBe('above');
  // the axis: z
  const axisZ = page.getByRole('radiogroup', { name: 'Cut plane normal axis' }).getByRole('radio', { name: 'z' });
  await axisZ.focus();
  await page.keyboard.press('Space');
  expect((await cutDrawn()).axis).toBe(2);
  await shot('1e5-cut-z-above');
  // a readout on the kept side of the new plane
  const box = await frameBox();
  const now = await cutDrawn();
  let read = 0;
  for (const [fx, fy] of [
    [0.5, 0.5],
    [0.6, 0.55],
    [0.45, 0.6],
  ] as const) {
    const h = await hoverAt(Math.round(box.x + box.width * fx), Math.round(box.y + box.height * fy));
    if (h) {
      read++;
      expect(h.point[2]).toBeGreaterThanOrEqual(now.at);
    }
  }
  expect(read).toBeGreaterThan(0);
  // off again: the panel goes, the picture is whole
  await page.getByRole('button', { name: 'Cut plane', exact: true }).click();
  await expect(page.getByRole('group', { name: 'Cut plane', exact: true })).toHaveCount(0);
});

// ---- T3: the arrows --------------------------------------------------------------------------------------------------

async function checkArrows(store: string, field: string, snapshot: number, comps: (Float32Array | null)[], nCells: number, name: string) {
  await show(store, field, snapshot);
  const values = comps.find((c) => c) as Float32Array;
  const s = await surfaceOf(store, values, nCells);
  const layers = meshLayers(s, nCells);
  const expected = vectorGlyphs(s, comps, layers, nCells);
  const off = await shot();
  const vectors = group().getByRole('button', { name: 'Vectors', exact: true });
  await vectors.focus();
  await page.keyboard.press('Space');
  await expect(vectors).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('.fv-frame')).toHaveAttribute('data-arrows', String(expected.cells.length), { timeout: 30_000 });
  await page.mouse.move(0, 0);
  const on = await shot(name);
  // only deep ink is added
  const box = await frameBox();
  let added = 0;
  const other: string[] = [];
  for (let y = Math.ceil(box.y * on.dpr) + 1; y < Math.floor((box.y + box.height) * on.dpr) - 1; y++) {
    for (let x = Math.ceil(box.x * on.dpr) + 1; x < Math.floor((box.x + box.width) * on.dpr) - 1; x++) {
      const p = pixel(on.img, x, y);
      const q = pixel(off.img, x, y);
      if (p[0] === q[0] && p[1] === q[1] && p[2] === q[2]) continue;
      if (p.every((c, k) => c === INK[k])) added++;
      else if (other.length < 10) other.push(`${x},${y}: ${p} (was ${q})`);
    }
  }
  expect(other).toEqual([]);
  expect(added).toBeGreaterThan(200);
  // off again restores every pixel
  await page.keyboard.press('Space');
  await expect(vectors).toHaveAttribute('aria-pressed', 'false');
  const back = await shot();
  let diff = 0;
  for (let y = Math.ceil(box.y * on.dpr); y < Math.floor((box.y + box.height) * on.dpr); y++)
    for (let x = Math.ceil(box.x * on.dpr); x < Math.floor((box.x + box.width) * on.dpr); x++) {
      const p = pixel(back.img, x, y);
      const q = pixel(off.img, x, y);
      if (p[0] !== q[0] || p[1] !== q[1] || p[2] !== q[2]) diff++;
    }
  expect(diff).toBe(0);
}

test('T3: the arrows of the 1e5-cell box are the recomputation’s and change pixels to deep ink only', async () => {
  test.setTimeout(240_000);
  await checkArrows(storeDir(root, '1e5', 'raw'), 'p', SNAPSHOT, [refField('1e5', 'ux'), refField('1e5', 'uy'), refField('1e5', 'uz')], refField('1e5', 'p').length, '1e5-vectors');
});

test('T3: the in-plane arrows of the 2D quick-start cavity, deep ink only', async () => {
  test.setTimeout(240_000);
  // the components as the page reads them (route R2 is bit for bit the Rust reader's, F-804)
  const read = (f: string) =>
    page.evaluate(
      async ([p, name]) => {
        const info = await window.studioData.open({ route: 'local', path: p! });
        if (!info.ok) throw new Error(info.error.message);
        const last = info.result.snapshots.length - 1;
        const r = await window.studioData.field({ route: 'local', path: p! }, last, name!);
        if (!r.ok) throw new Error(r.error.message);
        return { last, values: [...r.result.values] };
      },
      [quick, f],
    );
  const ux = await read('ux');
  const uy = await read('uy');
  await checkArrows(quick, 'ux', ux.last, [new Float32Array(ux.values), new Float32Array(uy.values), null], ux.values.length, '2d-vectors');
});

test('the 3D box with the cut plane and the arrows together (view-and-report), no CSP violation, no console error', async () => {
  await show(storeDir(root, '1e5', 'raw'), 'p', SNAPSHOT);
  await group().getByRole('button', { name: 'Cut plane', exact: true }).click();
  await group().getByRole('button', { name: 'Vectors', exact: true }).click();
  await expect(page.locator('.fv-frame')).toHaveAttribute('data-arrows', /\d+/, { timeout: 30_000 });
  await cutDrawn();
  await page.mouse.move(0, 0);
  await shot('1e5-cut-and-vectors');
  expect(await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp)).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

// ---- ADR 0738: the engine's iso-surface and streamlines ----------------------------------------------------------------

test('the engine’s iso-surface of the 1e5-cell box: drawn in the table’s colour of its level, the level from the keyboard', async () => {
  test.setTimeout(240_000);
  await show(storeDir(root, '1e5', 'raw'), 'p', SNAPSHOT);
  const before = await shot();
  const iso = group().getByRole('button', { name: 'Iso-surface', exact: true });
  await iso.focus();
  await page.keyboard.press('Space');
  await expect(iso).toHaveAttribute('aria-pressed', 'true');
  const panel = page.getByRole('group', { name: 'Iso-surface', exact: true });
  await expect(panel).toBeVisible();
  const frame = page.locator('.fv-frame');
  await expect(frame).toHaveAttribute('data-iso', /^[1-9]\d*$/, { timeout: 30_000 });
  const n0 = Number(await frame.getAttribute('data-iso'));
  await page.mouse.move(0, 0);
  const on = await shot('1e5-iso');
  // the level: the panel's slider, and the picture's colour is the table's colour of it (one colour, flat)
  const slider = panel.getByRole('slider');
  const level = Math.fround(Number(await slider.inputValue()));
  const range = (await frame.getAttribute('data-range'))!.split(' ').map(Number) as [number, number];
  const want = colourOf(lut, level, colourRange(range[0], range[1]));
  const box = await frameBox();
  let seen = 0;
  let changed = 0;
  for (let y = Math.ceil(box.y * on.dpr) + 2; y < Math.floor((box.y + box.height) * on.dpr) - 2; y += 3)
    for (let x = Math.ceil(box.x * on.dpr) + 2; x < Math.floor((box.x + box.width) * on.dpr) - 2; x += 3) {
      const p = pixel(on.img, x, y);
      const q = pixel(before.img, x, y);
      if (p[0] !== q[0] || p[1] !== q[1] || p[2] !== q[2]) changed++;
      if (p.every((c, k) => c === want[k])) seen++;
    }
  expect(changed, 'the iso-surface stands in for the surface').toBeGreaterThan(100);
  expect(seen, 'pixels in the table’s colour of the level').toBeGreaterThan(100);
  // the level from the keyboard: a new surface from the engine
  await slider.focus();
  for (let i = 0; i < 20; i++) await page.keyboard.press('ArrowRight');
  await expect.poll(async () => Number(await frame.getAttribute('data-iso')), { timeout: 30_000 }).not.toBe(n0);
  // off: the surface is back, pixel for pixel
  await iso.click();
  await expect(panel).toHaveCount(0);
  await page.mouse.move(0, 0);
  const back = await shot();
  let diff = 0;
  for (let y = Math.ceil(box.y * on.dpr); y < Math.floor((box.y + box.height) * on.dpr); y++)
    for (let x = Math.ceil(box.x * on.dpr); x < Math.floor((box.x + box.width) * on.dpr); x++) {
      const p = pixel(back.img, x, y);
      const q = pixel(before.img, x, y);
      if (p[0] !== q[0] || p[1] !== q[1] || p[2] !== q[2]) diff++;
    }
  expect(diff).toBe(0);
});

async function checkStreamlines(store: string, field: string, snapshot: number, name: string) {
  await show(store, field, snapshot);
  const off = await shot();
  const stream = group().getByRole('button', { name: 'Streamlines', exact: true });
  await stream.focus();
  await page.keyboard.press('Space');
  const frame = page.locator('.fv-frame');
  await expect(frame).toHaveAttribute('data-streamlines', /^[1-9]\d*$/, { timeout: 60_000 });
  const lines = Number(await frame.getAttribute('data-streamlines'));
  // the seeds from the keyboard: more seeds, more lines (at most two per seed, one each way)
  const seeds = page.getByRole('group', { name: 'Streamlines', exact: true }).getByRole('slider', { name: 'Seeds' });
  await seeds.focus();
  for (let i = 0; i < 5; i++) await page.keyboard.press('ArrowRight');
  const n = Number(await seeds.inputValue());
  await expect.poll(async () => Number(await frame.getAttribute('data-streamlines')), { timeout: 60_000 }).toBeGreaterThan(lines);
  expect(Number(await frame.getAttribute('data-streamlines'))).toBeLessThanOrEqual(2 * n);
  await settle();
  await page.mouse.move(0, 0);
  const on = await shot(name);
  // only deep ink is added, over the surface
  const box = await frameBox();
  let added = 0;
  const other: string[] = [];
  for (let y = Math.ceil(box.y * on.dpr) + 1; y < Math.floor((box.y + box.height) * on.dpr) - 1; y++)
    for (let x = Math.ceil(box.x * on.dpr) + 1; x < Math.floor((box.x + box.width) * on.dpr) - 1; x++) {
      const p = pixel(on.img, x, y);
      const q = pixel(off.img, x, y);
      if (p[0] === q[0] && p[1] === q[1] && p[2] === q[2]) continue;
      if (p.every((c, k) => c === INK[k])) added++;
      else if (other.length < 10) other.push(`${x},${y}: ${p} (was ${q})`);
    }
  // the panel is drawn over the frame too: its pixels are not the lines'
  const panel = (await page.getByRole('group', { name: 'Streamlines', exact: true }).boundingBox())!;
  const outsidePanel = other.filter((o) => {
    const [x, y] = o.split(':')[0]!.split(',').map(Number) as [number, number];
    return !(x >= (panel.x - 4) * on.dpr && x <= (panel.x + panel.width + 4) * on.dpr && y >= (panel.y - 4) * on.dpr && y <= (panel.y + panel.height + 4) * on.dpr);
  });
  expect(outsidePanel).toEqual([]);
  expect(added).toBeGreaterThan(100);
  await stream.click();
  await expect(frame).not.toHaveAttribute('data-streamlines', /.*/);
}

test('the engine’s streamlines of the 1e5-cell box (3D): deep ink over the surface, the seeds from the keyboard', async () => {
  test.setTimeout(240_000);
  await checkStreamlines(storeDir(root, '1e5', 'raw'), 'p', SNAPSHOT, '1e5-streamlines');
});

test('the engine’s streamlines of the 2D quick-start cavity, in its mid-plane', async () => {
  test.setTimeout(240_000);
  const last = await page.evaluate(async (p) => {
    const info = await window.studioData.open({ route: 'local', path: p });
    if (!info.ok) throw new Error(info.error.message);
    return info.result.snapshots.length - 1;
  }, quick);
  await checkStreamlines(quick, 'ux', last, '2d-streamlines');
  expect(consoleErrors).toEqual([]);
});
