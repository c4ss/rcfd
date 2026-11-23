// F-817: the item of this spec, by the commit that added it (853ebddd, 2026-11-17).
// F-817b (G9, power): the Studio draws on demand. At rest (after a key, a scrub, data or an animation has settled) the
// page asks for no animation frames, or only a few; while the user interacts, frames are drawn as before (P7 and P8
// are the bench's, scripts/measure-field.mjs).
//
// The counter wraps the page's requestAnimationFrame and counts the callbacks that run (a request cancelled before its
// frame does not count). The limit is registered before anything is measured. Surfaces at rest: the start page, the
// project home, the Runs view, and the run workspace with a field shown after one orbit key and after one scrub step.
// The counts, the callers of the requests and the limit go to test-results-visual/f817b/idle-frames.json.
//
// Before F-817b three.js's WebGPURenderer ran its own frame loop for ever once it had drawn (three's `Animation`,
// started by `render()`): one loop per field canvas and per plot, at the display's rate.
import { expect, test, type Page } from '@playwright/test';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { appRoot, killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';
import { until } from '../support/fake-service.js';

/** The limit, registered before measuring: animation-frame callbacks per second at rest. */
const LIMIT_PER_S = 5;
/** How long a surface is left to settle after its last input, then how long the frames are counted. */
const SETTLE_MS = 1_000;
const COUNT_MS = 2_000;

const SHOTS = path.join(appRoot, 'test-results-visual', 'f817b');
const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial' });

/** The quick-start run of cases/cavity-re100.toml: converged at iteration 121, 24 snapshots (every 5 iterations). */
const CAVITY = 'r-20261028T152641-f8c2';

let dir = '';
let quick = '';
let studio: Studio | null = null;
const report: { limitPerS: number; settleMs: number; countMs: number; surfaces: Record<string, Counted> } = {
  limitPerS: LIMIT_PER_S,
  settleMs: SETTLE_MS,
  countMs: COUNT_MS,
  surfaces: {},
};

interface Counted {
  /** Callbacks that ran since the last count (or since the counter was put in) up to the counting window: the input's frames. */
  readonly settling: number;
  /** Callbacks that ran in the counting window. */
  readonly ran: number;
  /** Requests made in the counting window. */
  readonly asked: number;
  readonly perS: number;
  /** The requests' callers (the first frame below the counter), most frequent first. */
  readonly callers: readonly [string, number][];
}

/** Wraps requestAnimationFrame once per page load: counts requests and the callbacks that run, with their callers. */
async function installCounter(page: Page): Promise<void> {
  await page.evaluate(() => {
    type W = { __f817b?: { raf: typeof requestAnimationFrame; ran: number; asked: number; callers: Map<string, number> } };
    const w = window as unknown as W;
    if (w.__f817b) return;
    const c = { raf: window.requestAnimationFrame.bind(window), ran: 0, asked: 0, callers: new Map<string, number>() };
    w.__f817b = c;
    window.requestAnimationFrame = (cb: FrameRequestCallback): number => {
      c.asked++;
      const line = (new Error().stack ?? '').split('\n')[2]?.trim() ?? '?';
      c.callers.set(line, (c.callers.get(line) ?? 0) + 1);
      return c.raf((t) => {
        c.ran++;
        cb(t);
      });
    };
  });
}

/** Lets the surface settle, then counts the frames over COUNT_MS. */
async function countAtRest(page: Page, surface: string): Promise<Counted> {
  await installCounter(page);
  await page.waitForTimeout(SETTLE_MS);
  const settling = await page.evaluate(() => {
    const c = (window as unknown as { __f817b: { ran: number; asked: number; callers: Map<string, number> } }).__f817b;
    const ran = c.ran;
    c.ran = 0;
    c.asked = 0;
    c.callers.clear();
    return ran;
  });
  await page.waitForTimeout(COUNT_MS);
  const got = await page.evaluate(() => {
    const c = (window as unknown as { __f817b: { ran: number; asked: number; callers: Map<string, number> } }).__f817b;
    return { ran: c.ran, asked: c.asked, callers: [...c.callers.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8) };
  });
  const counted: Counted = { settling, ...got, perS: got.ran / (COUNT_MS / 1000) };
  report.surfaces[surface] = counted;
  return counted;
}

function judge(c: Counted, surface: string): void {
  expect(c.perS, `${surface}: animation frames per second at rest (limit ${LIMIT_PER_S}); callers ${JSON.stringify(c.callers)}`).toBeLessThanOrEqual(LIMIT_PER_S);
}

/** Waits until the field shows the workspace's snapshot (drawn). */
async function settled(page: Page): Promise<void> {
  const ws = page.locator('.ws');
  const frame = page.locator('.fv-frame').first();
  await until(
    async () => {
      const want = await ws.getAttribute('data-snapshot');
      const got = await frame.getAttribute('data-snapshot');
      const ready = (await frame.getAttribute('data-ready')) !== null;
      return want !== null && want === got && ready ? true : null;
    },
    20_000,
    'the field to show the scrubber’s snapshot',
  );
}

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f817b-'));
  quick = await copyExample('quick-start', dir);
  await mkdir(SHOTS, { recursive: true });
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => studio!.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
});

test.afterAll(async () => {
  await writeFile(path.join(SHOTS, 'idle-frames.json'), JSON.stringify(report, null, 2) + '\n').catch(() => undefined);
  if (studio) {
    await studio.app.close().catch(() => undefined);
    studio = null;
  }
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('at rest the start page, the project home and the Runs view ask for no animation frames', async () => {
  const { page, app } = studio!;
  judge(await countAtRest(page, 'start'), 'the start page');

  await stubDialog(app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  judge(await countAtRest(page, 'home'), 'the project home');

  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible({ timeout: 20_000 });
  judge(await countAtRest(page, 'runs'), 'the Runs view');
});

test('at rest the run workspace with a field shown asks for no animation frames, after an orbit key and after a scrub', async () => {
  const { page } = studio!;
  await page.getByRole('tab', { name: 'Workspace' }).click();
  const runs = page.getByRole('listbox', { name: 'Runs of the project' });
  await expect(runs).toBeVisible({ timeout: 20_000 });
  await runs.locator(`[data-run="${CAVITY}"]`).click();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', CAVITY);
  await settled(page);

  // one orbit key on the field: the camera moves (a frame is drawn), then nothing moves
  const frame = page.locator('.fv-frame').first();
  await frame.focus();
  await installCounter(page);
  await page.keyboard.press('ArrowLeft');
  const orbit = await countAtRest(page, 'workspace after an orbit key');

  // one scrub step: a new snapshot is read and drawn, then nothing moves
  const slider = page.getByRole('slider', { name: /^Time of / });
  await slider.focus();
  await page.keyboard.press('ArrowLeft');
  await settled(page);
  const scrub = await countAtRest(page, 'workspace after a scrub');

  // the counter sees the frames the input drew (so a count of 0 at rest is not a counter that sees nothing)
  expect(orbit.settling, 'the orbit key drew frames').toBeGreaterThan(0);
  expect(scrub.settling, 'the scrub drew frames').toBeGreaterThan(0);
  judge(orbit, 'the workspace after an orbit key');
  judge(scrub, 'the workspace after a scrub');
});
