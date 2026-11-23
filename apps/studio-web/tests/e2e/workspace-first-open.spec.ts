// F-890 (ADR 0787; G21): the first opening of the workspace does not wait on the main thread for the GPU process.
//
// The task of 400 to 440 ms that failed responsiveness.spec.ts on a machine with free processors was the field legend's
// first use of a 2D canvas: Chromium then creates the renderer's shared GPU context by a synchronous call to the GPU
// process, and the main thread waits for the rest of whatever task the GPU process's main thread is running, which at
// that moment is the Studio's own WebGPU work. How long that is depends on the machine's load, so this test makes the
// state itself (tests/support/gpu-hold.ts): it holds the GPU process's main thread for at least HOLD_MS with a pipeline
// of its own, started in the task that clicks the Workspace tab, and checks that the legend's bar arrived while the hold
// was running. With the context created ahead (src/renderer/ui/canvasContext.ts) nothing waits; without it the legend's
// effect is one task as long as the rest of the hold.
//
// It holds LONG_TASK_LIMIT_MS (src/shared/work.ts, F-832) in the phase from the click to the small run drawn. It does
// not hold LONG_FRAME_LIMIT_MS: while the test itself keeps the GPU process busy, frames wait for the GPU by the test's
// doing (responsiveness.spec.ts holds that limit, unchanged). One pass: the state is made here and does not depend on
// the machine's load. A probe first, as in responsiveness.spec.ts: a deliberate task of 130 ms must be recorded.
//
// Needs f447_bench (tests/support/f447.ts) and rcfd-mcp (tests/support/project.ts); skipped, with the reason, without.
import { expect, test } from '@playwright/test';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { LONG_TASK_LIMIT_MS } from '../../src/shared/work.js';
import { findBench, generate, NO_BENCH, startBenchServer, type BenchServer } from '../support/f447.js';
import { until } from '../support/fake-service.js';
import { armGpuHold, gpuHold, startGpuHoldAndClick } from '../support/gpu-hold.js';
import { benchProject, benchRunId, entries, installRecorder, machineLoad, phase } from '../support/longtasks.js';
import { findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';
import { appRoot, killPid } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const bench = findBench();
const mcp = findMcp();
test.skip(bench === null, NO_BENCH);
test.skip(mcp === null, NO_MCP);

const OUT = path.join(appRoot, 'test-results-visual', 'f890');
/** The GPU process's main thread is held at least this long (ms): the legend arrives within a second of the click. */
const HOLD_MS = 2000;
const SMALL = benchRunId('tiny');

let root = '';
let server: BenchServer | null = null;

test.beforeAll(async () => {
  test.setTimeout(600_000);
  mkdirSync(OUT, { recursive: true });
  root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f890-rt-'));
  await generate(bench!, root, ['tiny', '1e5'], 480_000);
  benchProject(root, ['tiny', '1e5']);
  server = await startBenchServer(bench!, root);
});

test.afterAll(async () => {
  await server?.stop().catch(() => undefined);
  for (const p of mcpPids(path.join(root, 'project'))) killPid(p);
  if (root) rmSync(root, { recursive: true, force: true, maxRetries: 5 });
});

test('no task over LONG_TASK_LIMIT_MS when the workspace first opens while the GPU process is busy', async () => {
  test.setTimeout(300_000);
  const studio: Studio = await launchStudio({
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_SERVICE: `127.0.0.1:${server!.rpc}`,
    RCFD_STUDIO_DATA_ROOTS: root,
    RCFD_STUDIO_RECENT: path.join(root, 'recent.json'),
    RCFD_STUDIO_STATE_DIR: path.join(root, 'state'),
  });
  try {
    const { page, app } = studio;
    await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
    await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
    const loadBefore = await machineLoad();
    expect(await installRecorder(page)).toContain('longtask');

    // the probe: a deliberate task over the limit is seen (a task of the page's own, not of the test's evaluate)
    await phase(page, 'probe');
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

    await phase(page, 'project.open');
    await stubDialog(app, path.join(root, 'project'));
    await page.getByRole('button', { name: 'Open project folder…' }).click();
    await page.getByRole('tab', { name: 'Workspace' }).waitFor({ timeout: 30_000 });

    // the hold: calibrated here, then started in the task that clicks the tab
    await phase(page, 'hold.calibration');
    const armed = await armGpuHold(page, 'canvas[data-legend-bar]', HOLD_MS);
    await page.evaluate(() => new Promise((r) => setTimeout(r, 300)));
    await phase(page, 'workspace.tab');
    await startGpuHoldAndClick(page, 'tab', 'Workspace');
    const hold = await until(
      async () => {
        const h = await gpuHold(page);
        return h.heldMs !== null ? h : null;
      },
      120_000,
      'the hold of the GPU process to end',
    );

    // then, as responsiveness.spec.ts: the small run, drawn
    await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${SMALL}"]`).click({ timeout: 30_000 });
    await expect(page.locator('.ws')).toHaveAttribute('data-run', SMALL, { timeout: 30_000 });
    await until(
      async () =>
        page.evaluate(() => {
          const want = document.querySelector('.ws')?.getAttribute('data-snapshot');
          const frame = document.querySelector('.fv-frame');
          return want != null && frame?.getAttribute('data-snapshot') === want && frame.hasAttribute('data-ready') ? true : null;
        }),
      60_000,
      'the small run drawn',
    );
    await page.evaluate(() => new Promise((r) => setTimeout(r, 300)));
    await phase(page, 'end');

    const all = await entries(page);
    const loadAfter = await machineLoad();
    const probe = all.filter((e) => e.phase === 'probe' && e.type === 'longtask').map((e) => e.duration);
    const tasks = all.filter((e) => e.phase === 'workspace.tab' && e.type === 'longtask');
    const over = tasks.filter((e) => e.duration > LONG_TASK_LIMIT_MS);
    const report = { limit: LONG_TASK_LIMIT_MS, holdMs: HOLD_MS, armed, hold, loadBefore, loadAfter, probe, tasks, entries: all.filter((e) => e.phase === 'workspace.tab') };
    writeFileSync(path.join(OUT, 'workspace-first-open.json'), JSON.stringify(report, null, 1) + '\n');
    console.log(
      `F-890: load ${loadBefore.cpuBusy.toFixed(2)} -> ${loadAfter.cpuBusy.toFixed(2)} busy; the GPU process held ${hold.heldMs!.toFixed(0)} ms from ${hold.startedAt!.toFixed(0)} ` +
        `(a shader of ${armed.statements} statements, ${armed.calibratedMs.toFixed(0)} ms in the calibration); the legend's bar added at ${hold.seenAt?.toFixed(0) ?? 'no time'}; ` +
        `tasks of workspace.tab: ${tasks.map((e) => `${e.duration.toFixed(0)} ms at ${e.start.toFixed(0)}`).join(', ') || 'none over 50 ms'}; probe ${probe.map((d) => d.toFixed(0)).join(', ')} ms`,
    );

    expect(probe.length, 'the recorder saw the deliberate 130 ms task').toBeGreaterThan(0);
    // the state was made: the hold was long enough, and the legend's bar arrived while it ran
    expect(hold.heldMs!, 'the GPU process was held at least HOLD_MS').toBeGreaterThanOrEqual(HOLD_MS);
    expect(hold.seenAt, 'the legend’s bar was added').not.toBeNull();
    expect(hold.seenAt!, 'the legend’s bar was added after the hold started').toBeGreaterThan(hold.startedAt!);
    expect(hold.seenAt!, 'the legend’s bar was added while the GPU process was held').toBeLessThan(hold.startedAt! + hold.heldMs!);
    expect(
      over.map((e) => `longtask ${e.duration.toFixed(0)} ms at ${e.start.toFixed(0)} in workspace.tab`),
      `tasks over ${LONG_TASK_LIMIT_MS} ms while the workspace first opened with the GPU process held`,
    ).toEqual([]);
  } finally {
    await studio.app.close().catch(() => undefined);
    for (const p of mcpPids(path.join(root, 'project'))) killPid(p);
  }
});
