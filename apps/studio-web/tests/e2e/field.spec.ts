// The field viewport in the packaged Studio (F-808; guardrails G3, G8, G10, G13, G21), on stores made afresh by the Rust
// example `f447_bench gen` (the tiny box of 120 cells and the 1e5-cell box of ADR 0350), read by route R2 through the
// data door, at #/dev/field:
//
// - drawn by three.js WebGPURenderer, the back end named on screen (WebGL 2 only where this window has no WebGPU, or when
//   the view asks for it); no Content Security Policy violation and no console error;
// - G3, pixels: screenshots of the canvas read back in Node; at a grid of points inside flat triangles, the pixel is
//   exactly the table colour of the stored value of the cell under it (the value from the Rust reader's reference
//   bytes, the cell from the hover readout), within rcfd-render's colour of that value by the stated tolerance; the
//   viewport ground is exactly its token colour (the read-back path converts no colour). Both back ends.
// - G3, values: every corner of every surface triangle of both stores gets the table colour of its stored value, within
//   the stated tolerance of rcfd-render's colour; the legend bar's pixels are the table;
// - the hover readout equals the stored value (bit for bit, and its shortest text);
// - the mesh grid toggle (keyboard), reset view (pointer and keyboard), keyboard orbit with the readout at the centre;
// - ADR 0801 Addendum 22, the camera (Unity's scene view), read through the canvas's data-camera-* attributes: the right
//   button looks about the eye and flies with W A S D, the middle button pans, Alt and the left button orbit about the
//   point under the cursor, the wheel dollies toward it, a double-click sets the pivot there, F frames, O turns face on
//   (zoom and centre kept), Home resets; the plain left button moves nothing (it is the readout's);
// - screenshots at 1440 x 900 in test-results-visual/f808/ for the maintainer's view-and-report.
//
// Needs `f447_bench` (tests/support/f447.ts); without it these tests are skipped and say why. Nothing is timed here.
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { colourOf, colourRange, f32Text, LUT_TOLERANCE_LEVELS, lutBytes, lutIndex, lutPlace, tickLabel, ticks, type ColourRange } from '../../src/renderer/field/colormap.js';
import { gridEdges } from '../../src/renderer/field/surface.js';
import { findBench, generate, NO_BENCH, refBytes, storeDir } from '../support/f447.js';
import { decodePng, pixel, uniformAround, type Rgba } from '../support/png.js';
import { rcfdRenderColour, rcfdRenderViridis } from '../support/rcfd-render-map.js';
import { appRoot } from '../support/rcfd.js';
import { launchStudio } from '../support/studio.js';

const bench = findBench();
test.skip(bench === null, NO_BENCH);
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f808');
const SNAPSHOT = 19;
const GROUND: [number, number, number] = [0xee, 0xf1, 0xf4]; // silver-wash, tokens.ts role viewport
const lut = lutBytes();
// DESIGN.md Field view: the mesh grid in 38% white
const GRID_ALPHA = 0.38;
const map = rcfdRenderViridis();

let root = '';
let app: ElectronApplication;
let page: Page;
const consoleErrors: string[] = [];

interface Shown {
  values: Float32Array;
  cells: Uint32Array;
  points: Float32Array;
  triangles: Uint32Array;
  range: ColourRange;
}
const shown: Record<string, Shown> = {};

test.beforeAll(async () => {
  test.setTimeout(400_000);
  root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f808-e2e-'));
  await generate(bench!, root, ['tiny', '1e5'], 360_000);
  ({ app, page } = await launchStudio({ RCFD_STUDIO_DATA_ROOTS: root }));
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
  for (const size of ['tiny', '1e5']) {
    const store = storeDir(root, size, 'raw');
    const ref = refBytes(root, size, SNAPSHOT, 'p');
    const values = new Float32Array(ref.buffer.slice(ref.byteOffset, ref.byteOffset + ref.byteLength));
    // the surface as the page gets it (data.surface's triangulation, read by R2)
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
    shown[size] = { values, cells, points: new Float32Array(s.points), triangles: new Uint32Array(s.triangles), range: colourRange(lo, hi) };
  }
});

test.afterAll(async () => {
  await app?.close();
  if (root) rmSync(root, { recursive: true, force: true });
});

/** Opens the viewport on a store (a fresh mount) and waits for its first picture. */
async function show(size: string, extra = ''): Promise<void> {
  await page.evaluate(() => (location.hash = '#/dev/field'));
  await expect(page.locator('.fd-empty')).toBeVisible();
  await page.evaluate(
    ([p, x, snap]) => (location.hash = `#/dev/field?route=local&path=${encodeURIComponent(p!)}&field=p&snapshot=${snap}${x}`),
    [storeDir(root, size, 'raw'), extra, String(SNAPSHOT)],
  );
  await page.waitForFunction(() => performance.getEntriesByName('rcfd:field:first-picture').length > 0 && document.querySelector('.fv-frame[data-ready]') !== null, undefined, {
    timeout: 60_000,
  });
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
}

const frameBox = async () => (await page.locator('.fv-frame').boundingBox())!;

async function shot(name?: string): Promise<{ img: Rgba; dpr: number }> {
  const buf = await page.screenshot();
  if (name) writeFileSync(path.join(SHOTS, `${name}.png`), buf);
  return { img: decodePng(buf), dpr: await page.evaluate(() => devicePixelRatio) };
}

/** How many pixels of the frame (the canvas and its overlays) differ between two screenshots. */
async function frameDiff(a: { img: Rgba; dpr: number }, b: { img: Rgba; dpr: number }): Promise<number> {
  const box = await frameBox();
  let n = 0;
  for (let y = Math.ceil(box.y * a.dpr); y < Math.floor((box.y + box.height) * a.dpr); y++) {
    for (let x = Math.ceil(box.x * a.dpr); x < Math.floor((box.x + box.width) * a.dpr); x++) {
      const p = pixel(a.img, x, y);
      const q = pixel(b.img, x, y);
      if (p[0] !== q[0] || p[1] !== q[1] || p[2] !== q[2]) n++;
    }
  }
  return n;
}

/** The readout after the pointer moved to (x, y): the cell and the text, or null off the surface. */
async function hoverAt(x: number, y: number): Promise<{ cell: number; text: string } | null> {
  await page.mouse.move(x, y);
  return page.evaluate(
    () =>
      new Promise<{ cell: number; text: string } | null>((done) =>
        requestAnimationFrame(() => {
          const o = document.querySelector('.fv-readout-value');
          done(o ? { cell: Number(o.getAttribute('data-cell')), text: o.textContent ?? '' } : null);
        }),
      ),
  );
}

/** The entries a value may take: its own, and its neighbour when it sits on a rounding tie (the GPU may fuse t*255+0.5). */
function allowedEntries(v: number, r: ColourRange): number[] {
  const k = lutIndex(v, r);
  const frac = lutPlace(v, r) + 0.5 - Math.floor(lutPlace(v, r) + 0.5);
  if (frac < 1e-3) return [k, k - 1].filter((x) => x >= 0);
  if (frac > 1 - 1e-3) return [k, k + 1].filter((x) => x < 256);
  return [k];
}

/** Samples the frame on a grid; at each point inside a flat triangle, compares the pixel with the hovered cell's value. */
async function checkPixels(size: string, n: [number, number], minSamples: number) {
  const s = shown[size]!;
  const box = await frameBox();
  const { img, dpr } = await shot();
  // control: the ground is exactly its token colour (nothing converts the colours on the way to the screenshot)
  expect(pixel(img, (box.x + 6) * dpr, (box.y + 6) * dpr)).toEqual(GROUND);
  const bad: string[] = [];
  let samples = 0;
  let worstVsRust = 0;
  for (let i = 1; i < n[0]; i++) {
    for (let j = 1; j < n[1]; j++) {
      const x = Math.round(box.x + (box.width * i) / n[0]);
      const y = Math.round(box.y + (box.height * j) / n[1]);
      if (!uniformAround(img, x * dpr, y * dpr, Math.ceil(3 * dpr))) continue;
      const got = pixel(img, x * dpr, y * dpr);
      if (got.every((c, k) => c === GROUND[k])) continue;
      const h = await hoverAt(x, y);
      if (!h) {
        bad.push(`${x},${y}: coloured ${got} but no readout`);
        continue;
      }
      const v = s.values[h.cell]!;
      samples++;
      // the readout is the stored value
      if (h.text !== f32Text(v) || !Object.is(Math.fround(Number(h.text)), v)) bad.push(`${x},${y}: readout ${h.text}, stored ${v}`);
      // the pixel is the table colour of that value
      const ok = allowedEntries(v, s.range).some((k) => [lut[4 * k], lut[4 * k + 1], lut[4 * k + 2]].every((c, q) => c === got[q]));
      if (!ok) bad.push(`${x},${y}: cell ${h.cell} value ${v}: pixel ${got}, table ${colourOf(lut, v, s.range)}`);
      const rust = rcfdRenderColour(v, s.range.lo, s.range.hi, map);
      worstVsRust = Math.max(worstVsRust, ...got.map((c, q) => Math.abs(c - rust[q]!)));
    }
  }
  expect(bad).toEqual([]);
  expect(samples).toBeGreaterThanOrEqual(minSamples);
  expect(worstVsRust).toBeLessThanOrEqual(LUT_TOLERANCE_LEVELS);
  return samples;
}

test('the viewport is drawn by WebGPURenderer (WebGL 2 only without WebGPU), named on screen', async () => {
  await show('tiny');
  const webgpu = await page.evaluate(async () => {
    const gpu = (navigator as unknown as { gpu?: { requestAdapter(): Promise<unknown> } }).gpu;
    return !!gpu && (await gpu.requestAdapter()) !== null;
  });
  const backend = page.locator('.fv-backend');
  if (webgpu) await expect(backend).toHaveText('Drawn by WebGPU');
  else await expect(backend).toHaveText(/^Drawn by WebGL 2 \(fallback: .+\)$/);
  test.info().annotations.push({ type: 'backend', description: (await backend.textContent()) ?? '' });
  await expect(page.locator('.fv-caption')).toContainText('box-tiny-raw');
  await expect(page.locator('.fv-caption')).toContainText(`snapshot ${SNAPSHOT + 1} of 20`);
  await shot('tiny-webgpu');
});

test('G3: every corner of every surface triangle gets the table colour of its stored value, within the tolerance of rcfd-render', () => {
  for (const size of ['tiny', '1e5']) {
    const s = shown[size]!;
    let worst = 0;
    let exact = 0;
    for (const c of s.cells) {
      const v = s.values[c]!;
      const got = colourOf(lut, v, s.range);
      const want = rcfdRenderColour(v, s.range.lo, s.range.hi, map);
      const d = Math.max(...got.map((x, k) => Math.abs(x - want[k]!)));
      worst = Math.max(worst, d);
      if (d === 0) exact++;
    }
    test.info().annotations.push({ type: `corners ${size}`, description: `${3 * s.cells.length} corners, ${3 * exact} exactly rcfd-render's colour, worst ${worst} levels` });
    expect(worst).toBeLessThanOrEqual(LUT_TOLERANCE_LEVELS);
    // the ends of the range are exact
    expect(colourOf(lut, s.range.lo, s.range)).toEqual(rcfdRenderColour(s.range.lo, s.range.lo, s.range.hi, map));
    expect(colourOf(lut, s.range.hi, s.range)).toEqual(rcfdRenderColour(s.range.hi, s.range.lo, s.range.hi, map));
  }
});

test('G3 on the GPU (WebGPU): pixels inside triangles are the table colour of the stored value under them; the readout is that value', async () => {
  const n = await checkPixels('tiny', [36, 24], 40);
  test.info().annotations.push({ type: 'samples', description: `${n} points` });
});

test('the legend bar is the table, its ticks rcfd-studio’s labels of the shown range', async () => {
  const bar = await page.evaluate(() => {
    const c = document.querySelector<HTMLCanvasElement>('canvas[data-legend-bar]')!;
    return [...c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data];
  });
  expect(bar).toEqual([...lut]);
  const r = shown['tiny']!.range;
  await expect(page.locator('.fv-legend-ticks > span')).toHaveText(ticks(r.lo, r.hi).map(tickLabel));
  await expect(page.locator('.fv-legend-title')).toHaveText(/^p(, .+)?$/);
});

test('the mesh grid: a keyboard toggle; its lines are the boundary faces’ edges at 38% white; off again restores every pixel', async () => {
  const s = shown['tiny']!;
  // data.surface's triangles are fans of the faces: the grid of the 6 x 5 x 4 box is its 296 face edges
  expect(gridEdges(s).length / 6).toBe(2 * (6 * 5 + 6 * 4 + 5 * 4) * 2);
  const before = await shot();
  const toggle = page.getByRole('button', { name: 'Mesh grid' });
  await toggle.focus();
  await page.keyboard.press('Space');
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('.fv-note')).toHaveCount(0);
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  const on = await shot('tiny-grid');
  const box = await frameBox();
  let changed = 0;
  let once = 0;
  const notBlend: string[] = [];
  const { dpr } = on;
  for (let y = Math.ceil(box.y * dpr); y < Math.floor((box.y + box.height) * dpr); y++) {
    for (let x = Math.ceil(box.x * dpr); x < Math.floor((box.x + box.width) * dpr); x++) {
      const a = pixel(before.img, x, y);
      const b = pixel(on.img, x, y);
      if (a.every((c, k) => c === b[k])) continue;
      changed++;
      // white at GRID_ALPHA over the surface, once, or a few times where lines cross or meet (alpha compounds)
      const times = [1, 2, 3, 4].find((n) => a.every((c, k) => Math.abs(255 - (255 - c) * (1 - GRID_ALPHA) ** n - b[k]!) <= 1.01));
      if (times === undefined) notBlend.push(`${x},${y}: ${a} -> ${b}`);
      else if (times === 1) once++;
    }
  }
  expect(changed).toBeGreaterThan(500);
  expect(notBlend.slice(0, 10)).toEqual([]);
  // most line pixels are one line over the surface
  expect(once / changed).toBeGreaterThan(0.5);
  await page.keyboard.press('Space');
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  const off = await shot();
  expect(await frameDiff(off, before)).toBe(0);
});

/** ADR 0801 Addendum 22: the camera's state, from the canvas's data attributes. */
interface Cam {
  mode: string;
  eye: number[];
  forward: number[];
  up: number[];
  pivot: number[];
  zoom: number;
  moved: boolean;
}
async function camera(): Promise<Cam> {
  // one frame after the last input, so the attributes are the ones of the picture
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(done)));
  return page.locator('.fv-frame canvas').evaluate((c) => {
    const d = (c as HTMLElement).dataset;
    const v = (s?: string) => (s ?? '').split(' ').map(Number);
    return { mode: d['camera'] ?? '', eye: v(d['cameraEye']), forward: v(d['cameraForward']), up: v(d['cameraUp']), pivot: v(d['cameraPivot']), zoom: Number(d['cameraZoom']), moved: d['cameraMoved'] === 'true' };
  });
}
const vsub = (a: number[], b: number[]) => a.map((x, i) => x - b[i]!);
const vdot = (a: number[], b: number[]) => a.reduce((s, x, i) => s + x * b[i]!, 0);
const vlen = (a: number[]) => Math.hypot(...a);
/** Equal to the attributes' 6 significant digits (relative to the domain's size, about 1). */
function near(a: number[], b: number[], eps = 2e-4): void {
  expect(vlen(vsub(a, b)), `${a} vs ${b}`).toBeLessThan(eps * Math.max(1, vlen(b)));
}
/** Reset view from the keyboard (Home in the focused view). */
async function home(): Promise<Cam> {
  await page.mouse.move(10, 10);
  await page.locator('.fv-frame').focus();
  await page.keyboard.press('Home');
  const c = await camera();
  expect(c.moved).toBe(false);
  return c;
}
/** The point of the surface under (x, y), as the readout names it. */
async function pointAt(x: number, y: number): Promise<number[]> {
  await page.mouse.move(x, y);
  const out = page.locator('.fv-readout-value');
  await expect(page.locator('.fv-readout')).toHaveAttribute('data-pointer', `${x},${y}`);
  await expect(out).toHaveCount(1);
  return ((await out.getAttribute('data-point')) ?? '').split(' ').map(Number);
}

test('reset view: a right-button drag moves the view and shows Reset view; pressing it from the keyboard restores every pixel', async () => {
  await page.mouse.move(10, 10);
  const before = await shot();
  const c0 = await camera();
  expect(c0.mode).toBe('perspective');
  expect(c0.moved).toBe(false);
  const box = await frameBox();
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.move(cx, cy);
  await page.mouse.down({ button: 'right' });
  await page.mouse.move(cx + 140, cy + 40, { steps: 8 });
  await page.mouse.up({ button: 'right' });
  const reset = page.getByRole('button', { name: 'Reset view' });
  await expect(reset).toBeVisible();
  expect((await camera()).moved).toBe(true);
  await page.mouse.move(10, 10);
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  const moved = await shot();
  expect(await frameDiff(moved, before)).toBeGreaterThan(1000);
  await reset.focus();
  await page.keyboard.press('Enter');
  await expect(reset).toHaveCount(0);
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  const after = await shot();
  expect(await frameDiff(after, before)).toBe(0);
  expect(await camera()).toEqual(c0);
});

test('F-819b: no readout is taken while a button is held (a look); at the release it is the stored value under the cursor', async () => {
  const box = await frameBox();
  const cx = Math.round(box.x + box.width / 2);
  const cy = Math.round(box.y + box.height / 2);
  const readout = page.locator('.fv-readout-value');
  const where = page.locator('.fv-readout');
  const h = await hoverAt(cx, cy);
  expect(h).not.toBeNull();
  await expect(where).toHaveAttribute('data-pointer', `${cx},${cy}`);
  await page.mouse.down({ button: 'right' });
  await page.mouse.move(cx + 30, cy + 10, { steps: 6 });
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  // the view turned, and nothing was read out on the way: no value, and the readout's point is still the press's
  await expect(page.getByRole('button', { name: 'Reset view' })).toBeVisible();
  await expect(readout).toHaveCount(0);
  await expect(where).toHaveAttribute('data-pointer', `${cx},${cy}`);
  await page.mouse.up({ button: 'right' });
  await expect(where).toHaveAttribute('data-pointer', `${cx + 30},${cy + 10}`);
  await expect(readout).toHaveCount(1);
  const cell = Number(await readout.getAttribute('data-cell'));
  expect(await readout.textContent()).toBe(f32Text(shown['tiny']!.values[cell]!));
  // as a plain hover there
  expect(await hoverAt(cx + 30, cy + 10)).toEqual({ cell, text: f32Text(shown['tiny']!.values[cell]!) });
  await page.mouse.move(10, 10);
  await page.getByRole('button', { name: 'Reset view' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('button', { name: 'Reset view' })).toHaveCount(0);
});

test('keyboard: the view takes focus, arrow keys orbit, the readout shows the stored value at the centre, Home resets', async () => {
  const frame = page.locator('.fv-frame');
  await page.mouse.move(10, 10);
  await frame.focus();
  await expect(frame).toBeFocused();
  expect(await frame.evaluate((el) => el.matches(':focus-visible'))).toBe(true);
  await expect(frame).toHaveAttribute('aria-label', /Arrow keys orbit/);
  const c0 = await camera();
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowUp');
  await expect(page.getByRole('button', { name: 'Reset view' })).toBeVisible();
  // an orbit about the pivot: it stays, and so does the eye's distance to it
  const c = await camera();
  near(c.pivot, c0.pivot);
  expect(vlen(vsub(c.eye, c.pivot))).toBeCloseTo(vlen(vsub(c0.eye, c0.pivot)), 3);
  await expect(page.locator('.fv-readout-label')).toHaveText(/^p at the centre/);
  const out = page.locator('.fv-readout-value');
  const cell = Number(await out.getAttribute('data-cell'));
  expect(await out.textContent()).toBe(f32Text(shown['tiny']!.values[cell]!));
  // a chord with Ctrl is not the view's (Ctrl+0 is the window's actual size)
  await page.keyboard.press('Control+0');
  await expect(page.getByRole('button', { name: 'Reset view' })).toBeVisible();
  await page.keyboard.press('Home');
  await expect(page.getByRole('button', { name: 'Reset view' })).toHaveCount(0);
});

test('the plain left button moves nothing: a left drag keeps the camera, and the readout is taken where it is let go', async () => {
  const c0 = await home();
  const box = await frameBox();
  const cx = Math.round(box.x + box.width / 2);
  const cy = Math.round(box.y + box.height / 2);
  await page.mouse.move(cx, cy);
  await page.mouse.down();
  await page.mouse.move(cx + 40, cy + 20, { steps: 6 });
  await page.mouse.up();
  expect(await camera()).toEqual(c0);
  await expect(page.locator('.fv-readout')).toHaveAttribute('data-pointer', `${cx + 40},${cy + 20}`);
  await expect(page.getByRole('button', { name: 'Reset view' })).toHaveCount(0);
});

test('Addendum 22: right-drag looks about the eye; W A S D fly while it is held, Shift faster; no pole lock', async () => {
  const c0 = await home();
  const box = await frameBox();
  const cx = Math.round(box.x + box.width / 2);
  const cy = Math.round(box.y + box.height / 2);
  await page.mouse.move(cx, cy);
  await page.mouse.down({ button: 'right' });
  await page.mouse.move(cx + 60, cy, { steps: 4 });
  let c = await camera();
  // the eye stays, the view turned to the right (the new forward leans toward the old right)
  near(c.eye, c0.eye);
  const right0 = [c0.forward[1]! * c0.up[2]! - c0.forward[2]! * c0.up[1]!, c0.forward[2]! * c0.up[0]! - c0.forward[0]! * c0.up[2]!, c0.forward[0]! * c0.up[1]! - c0.forward[1]! * c0.up[0]!];
  expect(vdot(c.forward, right0)).toBeGreaterThan(0.1);
  // fly forward with W, held 400 ms (the canvas takes the fly keys while the button is held)
  const f = c.forward;
  const e0 = c.eye;
  await page.keyboard.down('w');
  await page.waitForTimeout(400);
  await page.keyboard.up('w');
  c = await camera();
  const d = vsub(c.eye, e0);
  expect(vdot(d, f)).toBeGreaterThan(0.9 * vlen(d)); // along the view
  const slow = vlen(d);
  expect(slow).toBeGreaterThan(0);
  await page.keyboard.down('Shift');
  await page.keyboard.down('S');
  await page.waitForTimeout(400);
  await page.keyboard.up('S');
  await page.keyboard.up('Shift');
  const c2 = await camera();
  const back = vsub(c2.eye, c.eye);
  expect(vdot(back, f)).toBeLessThan(0);
  expect(vlen(back), 'Shift flies faster').toBeGreaterThan(1.5 * slow);
  // a long look down goes on past straight down (no clamp at the pole): the view is upside down then
  await page.mouse.move(cx + 60, cy + 600, { steps: 20 });
  const c3 = await camera();
  expect(c3.up[2]).toBeLessThan(0);
  await page.mouse.up({ button: 'right' });
  expect(c3.moved).toBe(true);
  // the keys fly only while the button is held: W now does nothing to the camera
  await page.locator('.fv-frame').focus();
  await page.keyboard.press('w');
  expect((await camera()).eye).toEqual(c3.eye);
  await page.mouse.move(10, 10);
  await shot('camera-after-fly');
});

test('Addendum 22: middle-drag pans (the view direction kept); Alt and the left button orbit about the point under the cursor', async () => {
  let c0 = await home();
  const box = await frameBox();
  const cx = Math.round(box.x + box.width / 2);
  const cy = Math.round(box.y + box.height / 2);
  await page.mouse.move(cx, cy);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(cx + 50, cy - 30, { steps: 5 });
  await page.mouse.up({ button: 'middle' });
  let c = await camera();
  near(c.forward, c0.forward);
  near(vsub(c.eye, c0.eye), vsub(c.pivot, c0.pivot));
  expect(vlen(vsub(c.eye, c0.eye))).toBeGreaterThan(0.05);
  expect(c.moved).toBe(true);

  c0 = await home();
  // a point of the surface off the centre
  const x = Math.round(box.x + box.width * 0.42);
  const y = Math.round(box.y + box.height * 0.55);
  const p = await pointAt(x, y);
  await page.keyboard.down('Alt');
  await page.mouse.down();
  await page.mouse.move(x + 80, y + 30, { steps: 6 });
  await page.mouse.up();
  await page.keyboard.up('Alt');
  c = await camera();
  near(c.pivot, p);
  // the eye kept its distance to that point
  expect(Math.abs(vlen(vsub(c.eye, p)) - vlen(vsub(c0.eye, p)))).toBeLessThan(1e-3 * vlen(vsub(c0.eye, p)));
  expect(vdot(c.forward, c0.forward)).toBeLessThan(0.999);
  await page.mouse.move(10, 10);
  await shot('camera-after-orbit');
});

test('Addendum 22: the wheel dollies toward the point under the cursor; a double-click puts the pivot there', async () => {
  let c0 = await home();
  const box = await frameBox();
  const x = Math.round(box.x + box.width * 0.45);
  const y = Math.round(box.y + box.height * 0.6);
  const p = await pointAt(x, y);
  await page.mouse.wheel(0, -300);
  let c = await camera();
  // toward the point: the eye on the line from the point to the old eye, closer by the wheel's factor
  const was = vsub(c0.eye, p);
  const now = vsub(c.eye, p);
  expect(vlen(now) / vlen(was)).toBeCloseTo(Math.pow(1.0015, -300), 2);
  expect(vdot(now, was) / (vlen(now) * vlen(was))).toBeGreaterThan(0.99999);
  near(c.forward, c0.forward);
  // the same point is still under the cursor (it stayed where it was on screen)
  await page.mouse.move(x + 1, y);
  const q = await pointAt(x, y);
  expect(vlen(vsub(q, p))).toBeLessThan(0.05);
  // and out again
  await page.mouse.wheel(0, 300);
  near((await camera()).eye, c0.eye, 1e-3);

  c0 = await home();
  await page.mouse.dblclick(x, y);
  c = await camera();
  near(c.pivot, p);
  near(c.eye, c0.eye);
  expect(c.moved, 'a new pivot does not move the view').toBe(false);
  // the next orbit turns about it
  await page.locator('.fv-frame').focus();
  await page.keyboard.press('ArrowLeft');
  c = await camera();
  near(c.pivot, p);
  expect(Math.abs(vlen(vsub(c.eye, p)) - vlen(vsub(c0.eye, p)))).toBeLessThan(1e-3);
});

test('Addendum 22: F frames the domain (the orientation kept); O turns face on, keeping the zoom and the centre; Home resets all', async () => {
  const c0 = await home();
  const frame = page.locator('.fv-frame');
  await frame.focus();
  for (const k of ['ArrowLeft', 'ArrowLeft', 'ArrowUp', '+', '+']) await page.keyboard.press(k);
  const box = await frameBox();
  await page.mouse.move(Math.round(box.x + box.width * 0.4), Math.round(box.y + box.height * 0.5));
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(Math.round(box.x + box.width * 0.45), Math.round(box.y + box.height * 0.52), { steps: 3 });
  await page.mouse.up({ button: 'middle' });
  await frame.focus();
  let c = await camera();
  const turned = c.forward;
  await page.keyboard.press('f');
  c = await camera();
  near(c.forward, turned);
  near(c.pivot, c0.pivot);
  expect(c.zoom).toBeCloseTo(c0.zoom, 3);
  // face on: the nearest axis, the centre of the view and its distance kept
  for (const k of ['ArrowLeft', '+']) await page.keyboard.press(k);
  const b = await camera();
  const centre = b.eye.map((e, i) => e + b.forward[i]! * b.zoom);
  await page.keyboard.press('o');
  c = await camera();
  expect(Math.max(...c.forward.map(Math.abs))).toBe(1);
  expect(c.forward.filter((v) => v === 0)).toHaveLength(2);
  expect(c.zoom).toBeCloseTo(b.zoom, 4);
  near(c.eye.map((e, i) => e + c.forward[i]! * c.zoom), centre);
  // the button does the same, from its name
  await page.getByRole('button', { name: 'View face on (the nearest axis)' }).click();
  near((await camera()).eye, c.eye);
  await page.mouse.move(10, 10);
  await shot('camera-face-on-3d');
  // Home: everything back
  await frame.focus();
  await page.keyboard.press('Home');
  expect(await camera()).toEqual(c0);
});

test('the 1e5-cell box: the hover readout equals the stored value at every point of a grid, pixels too; screenshots', async () => {
  test.setTimeout(120_000);
  await show('1e5');
  await shot('box-1e5-webgpu');
  const s = shown['1e5']!;
  const box = await frameBox();
  const bad: string[] = [];
  let hits = 0;
  for (let i = 1; i < 24; i++) {
    for (let j = 1; j < 16; j++) {
      const h = await hoverAt(box.x + (box.width * i) / 24, box.y + (box.height * j) / 16);
      if (!h) continue;
      hits++;
      const v = s.values[h.cell]!;
      if (h.text !== f32Text(v) || !Object.is(Math.fround(Number(h.text)), v)) bad.push(`cell ${h.cell}: ${h.text} vs ${v}`);
    }
  }
  expect(bad).toEqual([]);
  expect(hits).toBeGreaterThan(60);
  // and the pixels of the many small faces of this box against the stored values under them
  const n = await checkPixels('1e5', [64, 40], 100);
  test.info().annotations.push({ type: 'samples 1e5', description: `${hits} readouts, ${n} pixel points` });
  await page.mouse.move(box.x + box.width * 0.5, box.y + box.height * 0.45);
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(done)));
  await shot('box-1e5-hover');
  await page.getByRole('button', { name: 'Mesh grid' }).click();
  await expect(page.locator('.fv-note')).toHaveCount(0);
  await page.mouse.move(10, 10);
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  await shot('box-1e5-grid');
});

test('the WebGL 2 fallback: named on screen, and its pixels are the same table colours of the same stored values', async () => {
  await show('tiny', '&backend=webgl');
  await expect(page.locator('.fv-backend')).toHaveText('Drawn by WebGL 2 (fallback: asked for by the view)');
  const n = await checkPixels('tiny', [36, 24], 40);
  test.info().annotations.push({ type: 'samples', description: `${n} points` });
  await page.mouse.move(10, 10);
  await shot('tiny-webgl');
});

test('no Content Security Policy violation and no console error', async () => {
  expect(await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp)).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
