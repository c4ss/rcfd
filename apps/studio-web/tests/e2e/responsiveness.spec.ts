// F-832 (G21, G9): no engine or data work on the renderer's main thread. In the packaged Studio at 1440 x 900, with a
// PerformanceObserver on `longtask` and `long-animation-frame` installed before the measurement, the test opens a
// project, opens a run of 1e5 cells in the run workspace, scrubs through its 20 snapshots (one stop at a time, each
// drawn, then a held key), changes the field, and loads fields of the same store by the other bulk route (R1, the
// service's data.* methods of `f447_bench serve`) and by R2 on the field viewport's own page. It fails on any task over
// LONG_TASK_LIMIT_MS and any frame over LONG_FRAME_LIMIT_MS (src/shared/work.ts; registered in README.md and ADR 0801,
// addendum F-832, before this test was written). The entries, their phases and the machine's load go to
// test-results-visual/f832/long-tasks.json.
//
// A probe first: a deliberate 130 ms task in the page must be recorded, so the recorder cannot pass by seeing nothing.
//
// The limits are wall-clock times, and on a machine whose processors are all busy with other work the renderer's thread
// is sometimes not scheduled for tens of milliseconds, which a frame then counts as its own. So the whole scenario is
// one pass in a fresh Studio window, and up to PASSES passes are made: a task the Studio's own code makes is over the
// limit in every pass and fails the test; one the machine's other load makes does not repeat. Every pass is reported
// (test-results-visual/f832/long-tasks.json) with the machine's load before and after it.
//
// A long animation frame is held against LONG_FRAME_LIMIT_MS when the renderer's thread held it up: its
// blockingDuration is over 0 (a task in it over 50 ms, or its rendering after one). A frame whose tasks were all short
// and whose rendering simply came late (blockingDuration 0: the thread was free for input all along, and the wait was
// the GPU process's or the system's; on this loaded machine one such frame lasted 13.5 s with no script in it) is
// reported as `notHeld`, not failed.
//
// Needs f447_bench (tests/support/f447.ts) and rcfd-mcp (tests/support/project.ts); skipped, with the reason, without.
import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { LONG_FRAME_LIMIT_MS, LONG_TASK_LIMIT_MS } from '../../src/shared/work.js';
import { findBench, generate, NO_BENCH, startBenchServer, type BenchServer } from '../support/f447.js';
import { benchProject, benchRunId, entries, installRecorder, machineLoad, phase, type LongEntry } from '../support/longtasks.js';
import { findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';
import { appRoot, killPid } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';
import { until } from '../support/fake-service.js';

const bench = findBench();
const mcp = findMcp();
test.skip(bench === null, NO_BENCH);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f832');
/** The box of f447_bench: 1e5 cells in npm test; RCFD_F832_SIZE=1e6 by hand (a heavy record: its generation takes minutes). */
const SIZE = process.env['RCFD_F832_SIZE'] === '1e6' ? '1e6' : '1e5';
const RUN = benchRunId(SIZE);
const OTHER = benchRunId('tiny');
/** Passes at most; the test passes on the first pass with nothing over a limit. */
const PASSES = 3;

let root = '';
let server: BenchServer | null = null;

test.beforeAll(async () => {
  test.setTimeout(SIZE === '1e6' ? 1_500_000 : 600_000);
  mkdirSync(SHOTS, { recursive: true });
  root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f832-rt-'));
  await generate(bench!, root, ['tiny', SIZE], SIZE === '1e6' ? 1_200_000 : 480_000);
  benchProject(root, ['tiny', SIZE]);
  server = await startBenchServer(bench!, root);
});

test.afterAll(async () => {
  await server?.stop().catch(() => undefined);
  for (const p of mcpPids(path.join(root, 'project'))) killPid(p);
  if (root) rmSync(root, { recursive: true, force: true, maxRetries: 5 });
});

/** Waits until the workspace's field shows the scrubber's snapshot, drawn. */
async function settled(page: Page, what: string): Promise<number> {
  const r = await until(
    async () =>
      page.evaluate(() => {
        const want = document.querySelector('.ws')?.getAttribute('data-snapshot');
        const frame = document.querySelector('.fv-frame');
        return want != null && frame?.getAttribute('data-snapshot') === want && frame.hasAttribute('data-ready') ? { snapshot: Number(want) } : null;
      }),
    60_000,
    what,
  );
  return r.snapshot;
}

/** Waits until the field viewport's page shows `snapshot` of field `field`, drawn. */
async function devDrawn(page: Page, snapshot: number, field: string): Promise<void> {
  await until(
    () =>
      page.evaluate(
        ([s, f]) => {
          const frame = document.querySelector('.fv-frame');
          return frame?.getAttribute('data-snapshot') === s && frame.hasAttribute('data-ready') && (document.querySelector('.fv-caption')?.textContent ?? '').includes(` · ${f}`) ? true : null;
        },
        [String(snapshot), field] as const,
      ),
    60_000,
    `snapshot ${snapshot} of ${field} drawn`,
  );
}

const key = (page: Page, k: string) =>
  page.evaluate((name) => {
    const s = document.querySelector('.tp-scrubber[role=slider]') as HTMLElement;
    s.focus();
    s.dispatchEvent(new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true }));
  }, k);

interface PassReport {
  readonly pass: number;
  readonly loadBefore: Awaited<ReturnType<typeof machineLoad>>;
  readonly loadAfter: Awaited<ReturnType<typeof machineLoad>>;
  readonly backend: string | null;
  readonly byPhase: Record<string, { longtasks: number; maxTaskMs: number; frames: number; maxFrameMs: number }>;
  readonly probe: readonly number[];
  readonly over: readonly LongEntry[];
  /** Long frames over the limit that the renderer's thread did not hold up (blockingDuration 0). */
  readonly notHeld: readonly LongEntry[];
  readonly entries: readonly LongEntry[];
}

/** One pass of the whole scenario in a fresh Studio window. */
async function onePass(n: number): Promise<PassReport> {
  const studio: Studio = await launchStudio({
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_SERVICE: `127.0.0.1:${server!.rpc}`,
    RCFD_STUDIO_DATA_ROOTS: root,
    RCFD_STUDIO_RECENT: path.join(root, `recent-${n}.json`),
    RCFD_STUDIO_STATE_DIR: path.join(root, `state-${n}`),
  });
  try {
    const { page, app } = studio;
    await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
    await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
    const loadBefore = await machineLoad();
    const supported = await installRecorder(page);
    expect(supported.sort()).toEqual(['long-animation-frame', 'longtask']);

    // the probe: a deliberate task over the limit is seen (and kept out of the verdict by its phase)
    await phase(page, 'probe');
    // (a task of the page's own: a script run by the test's evaluate is not reported as a long task)
    await page.evaluate(
      () =>
        new Promise<void>((done) => {
          setTimeout(() => {
            const t = performance.now();
            while (performance.now() - t < 130) {
              // busy
            }
            setTimeout(done, 200);
          }, 0);
        }),
    );

    // opening the project (the engine's project server), then the run workspace
    await phase(page, 'project.open');
    await stubDialog(app, path.join(root, 'project'));
    await page.getByRole('button', { name: 'Open project folder…' }).click();
    await page.getByRole('tab', { name: 'Workspace' }).waitFor({ timeout: 30_000 });
    await page.evaluate(() => new Promise((r) => setTimeout(r, 300)));
    await phase(page, 'workspace.tab');
    await page.getByRole('tab', { name: 'Workspace' }).click({ timeout: 30_000 });
    await page.getByRole('listbox', { name: 'Runs of the project' }).waitFor({ timeout: 30_000 });

    // first the small run (the workspace may already have selected one), then, measured, opening the big one
    await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${OTHER}"]`).click();
    await expect(page.locator('.ws')).toHaveAttribute('data-run', OTHER, { timeout: 30_000 });
    await settled(page, 'the small run drawn');
    await page.evaluate(() => new Promise((r) => setTimeout(r, 300)));

    // opening the run of SIZE cells: its store, surface, newest field and histories
    await phase(page, 'run.open');
    await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${RUN}"]`).click();
    await expect(page.locator('.ws')).toHaveAttribute('data-run', RUN, { timeout: 30_000 });
    expect(await settled(page, 'the newest snapshot of the run drawn')).toBe(19);
    await page.waitForFunction(() => document.querySelector('.tp-resid-values li[data-name="continuity"][data-step]') !== null, null, { timeout: 60_000 });

    // scrubbing: one stop at a time, each drawn (R2), as P5 does
    await phase(page, 'scrub.step');
    await key(page, 'Home');
    expect(await settled(page, 'snapshot 0')).toBe(0);
    for (let i = 1; i < 20; i++) {
      await key(page, 'ArrowRight');
      expect(await settled(page, `snapshot ${i}`)).toBe(i);
    }
    // a held key: 19 stops in quick succession, loads superseded and cancelled on the way
    await phase(page, 'scrub.held');
    await page.evaluate(async () => {
      const s = document.querySelector('.tp-scrubber[role=slider]') as HTMLElement;
      s.focus();
      for (let i = 0; i < 19; i++) {
        s.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true, cancelable: true }));
        await new Promise((r) => setTimeout(r, 33));
      }
    });
    expect(await settled(page, 'snapshot 0 after the held key')).toBe(0);

    // another field of the same run (R2)
    await phase(page, 'field.local');
    await page.locator('.ws-field-select button').click();
    await page.getByRole('option', { name: /^ux/ }).click();
    await until(() => page.evaluate(() => ((document.querySelector('.fv-caption')?.textContent ?? '').includes(' · ux') && document.querySelector('.fv-frame[data-ready]') ? true : null)), 60_000, 'ux drawn');
    await page.locator('.ws-field-select button').click();
    await page.getByRole('option', { name: /^p/ }).click();
    await until(() => page.evaluate(() => ((document.querySelector('.fv-caption')?.textContent ?? '').includes(' · p') && document.querySelector('.fv-frame[data-ready]') ? true : null)), 60_000, 'p drawn');

    // the other bulk route: the same store through the service's data.* methods (R1), on the viewport's page
    await phase(page, 'field.service');
    const servicePath = `project/runs/${RUN}/fields`;
    const dev = (route: string, p: string, field: string, snap: number) =>
      page.evaluate(([r, pp, f, s]) => (location.hash = `#/dev/field?route=${r}&path=${encodeURIComponent(pp!)}&field=${f}&snapshot=${s}`), [route, p, field, String(snap)] as const);
    await dev('service', servicePath, 'p', 19);
    await devDrawn(page, 19, 'p');
    for (let i = 0; i < 20; i += 3) {
      await dev('service', servicePath, 'p', i);
      await devDrawn(page, i, 'p');
    }
    await dev('service', servicePath, 'uy', 18);
    await devDrawn(page, 18, 'uy');

    // R2 on the same page
    await phase(page, 'field.local.dev');
    const localPath = path.join(root, 'project', 'runs', RUN, 'fields');
    await dev('local', localPath, 'uz', 10);
    await devDrawn(page, 10, 'uz');
    for (let i = 11; i < 20; i += 2) {
      await dev('local', localPath, 'uz', i);
      await devDrawn(page, i, 'uz');
    }
    await phase(page, 'end');
    const all = await entries(page);
    const loadAfter = await machineLoad();
    const measured = all.filter((e) => e.phase !== 'probe' && e.phase !== 'before');
    const probe = all.filter((e) => e.phase === 'probe' && e.type === 'longtask').map((e) => e.duration);
    const held = (e: LongEntry) => e.blocking === null || e.blocking > 0;
    const over = measured.filter((e) => (e.type === 'longtask' ? e.duration > LONG_TASK_LIMIT_MS : e.duration > LONG_FRAME_LIMIT_MS && held(e)));
    const notHeld = measured.filter((e) => e.type === 'long-animation-frame' && e.duration > LONG_FRAME_LIMIT_MS && !held(e));
    const byPhase: PassReport['byPhase'] = {};
    for (const e of measured) {
      const ph = (byPhase[e.phase!] ??= { longtasks: 0, maxTaskMs: 0, frames: 0, maxFrameMs: 0 });
      if (e.type === 'longtask') {
        ph.longtasks++;
        ph.maxTaskMs = Math.max(ph.maxTaskMs, e.duration);
      } else {
        ph.frames++;
        ph.maxFrameMs = Math.max(ph.maxFrameMs, e.duration);
      }
    }
    const backend = await page.evaluate(() => document.querySelector('.fv-backend')?.textContent ?? null);
    return { pass: n, loadBefore, loadAfter, backend, byPhase, probe, over, notHeld, entries: measured };
  } finally {
    await studio.app.close().catch(() => undefined);
    for (const p of mcpPids(path.join(root, 'project'))) killPid(p);
  }
}

const fmt = (e: LongEntry) =>
  `${e.type} ${e.duration.toFixed(0)} ms in ${e.phase}${e.blocking !== null ? ` (blocking ${e.blocking.toFixed(0)} ms)` : ''}${e.renderAt !== null ? ` (render at ${e.renderAt.toFixed(0)}, style and layout at ${e.styleAt?.toFixed(0)} ms)` : ''}${
    e.scripts[0] ? ` (${e.scripts[0].invoker} ${e.scripts[0].source.split('/').pop()}:${e.scripts[0].fn} ${e.scripts[0].duration.toFixed(0)} ms)` : ''
  }`;

test('no task over LONG_TASK_LIMIT_MS and no frame over LONG_FRAME_LIMIT_MS while a run opens, fields load (R2 and R1) and the line scrubs', async () => {
  test.setTimeout(PASSES * (SIZE === '1e6' ? 600_000 : 240_000));
  const passes: PassReport[] = [];
  for (let n = 1; n <= PASSES; n++) {
    const r = await onePass(n);
    passes.push(r);
    console.log(
      `F-832 pass ${n}: load ${r.loadBefore.cpuBusy.toFixed(2)} -> ${r.loadAfter.cpuBusy.toFixed(2)} busy (${r.loadBefore.builders} build, node and electron processes); ` +
        Object.entries(r.byPhase)
          .map(([k, v]) => `${k}: ${v.longtasks} tasks (max ${v.maxTaskMs.toFixed(0)} ms), ${v.frames} long frames (max ${v.maxFrameMs.toFixed(0)} ms)`)
          .join('; ') +
        `; probe ${r.probe.map((d) => d.toFixed(0)).join(', ')} ms; over the limits: ${r.over.length ? r.over.map(fmt).join('; ') : 'none'}` +
        `; long frames not held up by the renderer's thread: ${r.notHeld.length ? r.notHeld.map(fmt).join('; ') : 'none'}`,
    );
    expect(r.probe.length, 'the recorder saw the deliberate 130 ms task').toBeGreaterThan(0);
    if (r.over.length === 0) break;
  }
  const report = {
    limits: { LONG_TASK_LIMIT_MS, LONG_FRAME_LIMIT_MS },
    store: { size: SIZE, run: RUN },
    machine: { cpu: os.cpus()[0]?.model, logical: os.cpus().length, totalMemGB: os.totalmem() / 2 ** 30 },
    passes,
  };
  writeFileSync(path.join(SHOTS, 'long-tasks.json'), JSON.stringify(report, null, 1) + '\n');
  const clean = passes.find((p) => p.over.length === 0);
  expect(clean ? [] : passes.map((p) => `pass ${p.pass}: ${p.over.map(fmt).join('; ')}`), `tasks over ${LONG_TASK_LIMIT_MS} ms or frames over ${LONG_FRAME_LIMIT_MS} ms in every pass`).toEqual([]);
});
