// The residual and monitor plots (F-811) in the packaged Studio against the real `rcfd serve`, on the development
// route #/dev/plots: a session of the quick-start cavity read once and then by increments while it runs live, every
// plot range holding every value drawn (checked against a monitors.get of the test's own), the drawing on three.js,
// runs without series said in words with their pictogram, names and keyboard reach, no Content Security Policy
// violation, and screenshots at 1440 by 900 in test-results-visual/f811/ for the maintainer's view-and-report (G13).
//
// Needs the rcfd binary (tests/support/rcfd.ts); without it the tests are skipped with that reason.
import { expect, test, type Page } from '@playwright/test';
import { cpSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { boxToml, findRcfd, killIfRcfd, NO_RCFD, startRcfd, appRoot, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, untilState, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
test.skip(rcfd === null, NO_RCFD);
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f811');
let service: RunningRcfd;
let studio: Studio;
let page: Page;
let scratch: string;
const consoleErrors: string[] = [];

test.beforeAll(async () => {
  scratch = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f811-'));
  // the service's sandbox holds a copy of the quick-start cases (the example project is never written to)
  cpSync(path.join(appRoot, '..', '..', 'example-projects', 'quick-start', 'cases'), path.join(scratch, 'root', 'cases'), { recursive: true });
  service = await startRcfd(rcfd!, 0, ['--root', path.join(scratch, 'root')]);
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`, RCFD_STUDIO_STATE_DIR: path.join(scratch, 'state') });
  page = studio.page;
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await untilState(page, (s) => s.status === 'connected');
  await page.evaluate(() => {
    const w = window as unknown as { __csp: string[] };
    w.__csp = [];
    document.addEventListener('securitypolicyviolation', (e) => w.__csp.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
  mkdirSync(SHOTS, { recursive: true });
});

test.afterAll(async () => {
  await studio?.app.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  if (scratch) rmSync(scratch, { recursive: true, force: true });
});

/** A service call from the test (the test is not the renderer's code; the static check covers src/renderer only). */
async function call<T>(method: string, params: Record<string, unknown>, timeoutMs = 120_000): Promise<T> {
  const r = await page.evaluate(
    async ([m, p, t]) => {
      const reply = await window.studioService.call(m as never, p as Record<string, unknown>, { timeoutMs: t as number });
      // monitors.get arrives with Float64Arrays (src/shared/monitors.ts): plain arrays for the trip to the test
      if (reply.ok && m === 'monitors.get') {
        const doc = reply.result as { series: Record<string, { x: ArrayLike<number>; y: ArrayLike<number> }> };
        for (const s of Object.values(doc.series)) {
          s.x = Array.from(s.x);
          s.y = Array.from(s.y);
        }
      }
      return reply;
    },
    [method, params, timeoutMs] as const,
  );
  if (!r.ok) throw new Error(`${method}: ${r.error.message}`);
  return r.result as T;
}

async function open(session: string) {
  // through the empty view, so that the plots start again from nothing
  await page.evaluate(() => {
    location.hash = '';
  });
  await expect(page.getByRole('heading', { name: 'No project open' })).toBeVisible();
  await page.evaluate((s) => {
    location.hash = `#/dev/plots?session=${s}`;
  }, session);
  await expect(page.getByRole('heading', { name: 'Plots', level: 1 })).toBeVisible();
  await expect(page.locator('.plots-dev-panel')).toHaveAttribute('data-run', new RegExp(`/${session}$`));
}

interface Series {
  unit: string;
  x: ArrayLike<number>;
  y: ArrayLike<number>;
}

/** What a plot must draw of `series`: the drawable samples and the range in plot space that holds them. */
function expected(series: Series[], log: boolean) {
  let drawn = 0;
  let skipped = 0;
  let xlo = Infinity;
  let xhi = -Infinity;
  let ylo = Infinity;
  let yhi = -Infinity;
  for (const s of series) {
    for (let i = 0; i < s.x.length; i++) {
      const x = s.x[i]!;
      const y = s.y[i]!;
      const py = log ? (y > 0 ? Math.log10(y) : Number.NaN) : y;
      if (!Number.isFinite(x) || !Number.isFinite(py)) {
        skipped++;
        continue;
      }
      drawn++;
      xlo = Math.min(xlo, x);
      xhi = Math.max(xhi, x);
      ylo = Math.min(ylo, py);
      yhi = Math.max(yhi, py);
    }
  }
  return { drawn, skipped, xlo, xhi, ylo, yhi };
}

/** Checks a plot frame against a fresh monitors.get: counts equal, and its range holds every value drawn. */
async function checkFrame(frame: ReturnType<Page['locator']>, series: Series[], log: boolean) {
  const e = expected(series, log);
  await expect(frame).toHaveAttribute('data-drawn', String(e.drawn));
  await expect(frame).toHaveAttribute('data-skipped', String(e.skipped));
  const [xlo, xhi, ylo, yhi] = (await frame.getAttribute('data-range'))!.split(' ').map(Number) as [number, number, number, number];
  expect(xlo <= e.xlo && e.xhi <= xhi, `x [${e.xlo}, ${e.xhi}] in [${xlo}, ${xhi}]`).toBe(true);
  expect(ylo <= e.ylo && e.yhi <= yhi, `y [${e.ylo}, ${e.yhi}] in [${ylo}, ${yhi}]`).toBe(true);
  return e;
}

const residualNames = (all: Record<string, Series>) => Object.keys(all).filter((n) => n === 'continuity' || n.startsWith('res_'));

test('a run read once, then by increments while it runs; every range holds every value drawn', async () => {
  const { session } = await call<{ session: string }>('session.create', { path: 'cases/cavity-re400.toml' });
  // The 32 x 32 cavity converges in about a second on an idle machine, which is too soon for five increments; this run
  // goes on until the test stops it: the smallest tolerances the schema allows, which this mesh does not reach in the
  // test's time, and no iteration cap within it.
  await call('settings.set', { session, path: 'solver.max_iterations', value: 1_000_000 });
  await call('settings.set', { session, path: 'solver.tolerance_u', value: 1e-14 });
  await call('settings.set', { session, path: 'solver.tolerance_p', value: 1e-14 });
  await call('run.initialize', { session });
  await call('run.step', { session, count: 60 });
  await open(session);
  const residuals = page.getByRole('region', { name: `Residuals · Session ${session}` });
  const monitor = page.getByRole('region', { name: `Monitor · Session ${session}` });
  const rFrame = residuals.locator('.plot-frame');
  const mFrame = monitor.locator('.plot-frame');
  await expect(rFrame).toHaveAttribute('data-backend', /^(WebGPU|WebGL2)$/);
  await expect(mFrame).toHaveAttribute('data-backend', /^(WebGPU|WebGL2)$/);
  let all = (await call<{ series: Record<string, Series> }>('monitors.get', { session })).series;
  await checkFrame(rFrame, residualNames(all).map((n) => all[n]!), true);
  await expect(monitor).toHaveAttribute('data-series', 'lid_drag');
  await checkFrame(mFrame, [all['lid_drag']!], false);
  await expect(residuals.getByRole('list', { name: 'Residual series' }).getByRole('listitem')).toHaveCount(residualNames(all).length);
  await page.screenshot({ path: path.join(SHOTS, 'plots-stepped-1440x900.png'), scale: 'css' });

  // live: the run's events wake the feed, which reads increments only
  await call('run.start', { session });
  const stats = page.getByTestId('feed-stats');
  await expect.poll(async () => Number(await stats.getAttribute('data-increments')), { timeout: 30_000 }).toBeGreaterThanOrEqual(5);
  await page.screenshot({ path: path.join(SHOTS, 'plots-live-1440x900.png'), scale: 'css' });
  await call('run.stop', { session });
  await expect.poll(async () => (await call<{ state: string }>('run.status', { session })).state).not.toBe('running');
  // the last events of the run bring the last samples; then the feed holds exactly what the service holds
  all = (await call<{ series: Record<string, Series> }>('monitors.get', { session })).series;
  const want = expected(residualNames(all).map((n) => all[n]!), true);
  await expect(rFrame).toHaveAttribute('data-drawn', String(want.drawn), { timeout: 15_000 });
  await checkFrame(rFrame, residualNames(all).map((n) => all[n]!), true);
  await checkFrame(mFrame, [all['lid_drag']!], false);
  expect(Number(await stats.getAttribute('data-full'))).toBe(1);
  expect(Number(await stats.getAttribute('data-resyncs'))).toBe(0);
  expect(all['res_u']!.x.length).toBeGreaterThan(60);
  await page.screenshot({ path: path.join(SHOTS, 'plots-stopped-1440x900.png'), scale: 'css' });
});

test('a run without series says so in words with its pictogram, never with another run’s lines', async () => {
  // defined: no simulation, so no histories at all
  const { session: defined } = await call<{ session: string }>('session.create', { toml: boxToml([4, 4, 4]) });
  await open(defined);
  const residuals = page.getByRole('region', { name: `Residuals · Session ${defined}` });
  const monitor = page.getByRole('region', { name: `Monitor · Session ${defined}` });
  await expect(residuals.getByRole('status')).toContainText(`No residuals: session ${defined} is defined and has no simulation yet.`);
  await expect(residuals.locator('svg.ui-picto[data-state="queued"]')).toHaveCount(1);
  await expect(monitor.getByRole('status')).toContainText('No monitor values');
  await expect(page.locator('.plot-frame')).toHaveCount(0);
  await page.screenshot({ path: path.join(SHOTS, 'plots-no-simulation-1440x900.png'), scale: 'css' });

  // residuals but no report: the monitor says so; the residual plot shows this run's lines only
  await call('run.initialize', { session: defined });
  await call('run.step', { session: defined, count: 12 });
  await open(defined);
  await expect(residuals.locator('.plot-frame')).toHaveAttribute('data-backend', /^(WebGPU|WebGL2)$/);
  const all = (await call<{ series: Record<string, Series> }>('monitors.get', { session: defined })).series;
  expect(Object.keys(all).filter((n) => !residualNames(all).includes(n))).toEqual([]);
  await checkFrame(residuals.locator('.plot-frame'), residualNames(all).map((n) => all[n]!), true);
  await expect(monitor.getByRole('status')).toContainText(`No monitor values: session ${defined} is ready and not running, and it records no series besides its residuals.`);
  await expect(monitor.locator('svg.ui-picto')).toHaveCount(1);
  await expect(monitor.locator('.plot-frame')).toHaveCount(0);
  await page.screenshot({ path: path.join(SHOTS, 'plots-no-monitor-1440x900.png'), scale: 'css' });

  // an unknown session: words, no lines
  await open('s999');
  await expect(page.getByRole('region', { name: 'Residuals · Session s999' }).getByRole('status')).toContainText("No residuals for session s999. The service says: no session 's999'");
  await expect(page.locator('.plot-frame')).toHaveCount(0);
});

test('names and keyboard: the plots are named regions, the controls are reached by Tab with focus visible', async () => {
  const { session } = await call<{ session: string }>('session.create', { path: 'cases/cavity-re100.toml' });
  await call('run.initialize', { session });
  await call('run.step', { session, count: 5 });
  await open(session);
  await expect(page.getByRole('region', { name: `Residuals · Session ${session}` })).toBeVisible();
  await expect(page.getByRole('region', { name: `Monitor · Session ${session}` })).toBeVisible();
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  const reached: string[] = [];
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press('Tab');
    const name = ((await page.locator(':focus').ariaSnapshot()).split('\n')[0] ?? '').trim();
    const outline = await page.evaluate(() => {
      const s = getComputedStyle(document.activeElement!);
      return `${s.outlineStyle} ${s.outlineWidth}`;
    });
    reached.push(name);
    expect(outline, `focus visible on ${name}`).toMatch(/solid 2px/);
  }
  for (const name of ['Session', 'Case file', 'New session', 'Initialize', 'Step 20', 'Start', 'Stop']) expect(reached.join(' | ')).toContain(name);
});

test('F-828: the session’s numbers open their provenance: the monitor readout, a residual’s last value, the iteration', async () => {
  const { session } = await call<{ session: string }>('session.create', { path: 'cases/cavity-re100.toml' });
  await call('run.initialize', { session });
  await call('run.step', { session, count: 8 });
  await open(session);
  const all = (await call<{ series: Record<string, Series & { unit: string }> }>('monitors.get', { session })).series;
  const drag = all['lid_drag']!;
  const monitor = page.getByRole('region', { name: `Monitor · Session ${session}` });
  const value = monitor.locator('.plot-readout-value').getByRole('button', { name: /^Provenance of lid_drag: / });
  await expect(value).toHaveCount(1);
  // Enter on the number
  await value.focus();
  await page.keyboard.press('Enter');
  const d = page.getByRole('dialog', { name: 'Provenance of lid_drag' });
  await expect(d).toBeVisible();
  const fact = (k: string) => d.locator(`[data-fact="${k}"] dd`);
  await expect(fact('Session')).toHaveText(session);
  await expect(fact('Value')).toContainText(String(drag.y[drag.y.length - 1]));
  await expect(fact('Iteration')).toHaveText(String(drag.x[drag.x.length - 1]));
  await expect(fact('Unit')).toHaveText(drag.unit === '' ? 'none, empty in monitors.get (the series’ unit)' : `${drag.unit}, monitors.get (the series’ unit)`);
  await expect(fact('Read by')).toHaveText('monitors.get of rcfd serve (rcfd-service/1), the latest sample');
  await expect(fact('Engine')).toHaveText(/^rcfd \S+ \(service\.info\)$/);
  await page.screenshot({ path: path.join(SHOTS, 'plots-provenance-1440x900.png'), scale: 'css' });
  await page.keyboard.press('Escape');
  await expect(value).toBeFocused();
  // a residual's last value: click; the engine's unit as monitors.get gives it
  const residuals = page.getByRole('region', { name: `Residuals · Session ${session}` });
  await residuals.getByRole('button', { name: /^Provenance of res_u: / }).click();
  const r = page.getByRole('dialog', { name: 'Provenance of res_u' });
  await expect(r.locator('[data-fact="Unit"] dd')).toHaveText(all['res_u']!.unit === '' ? 'none, empty in monitors.get (the series’ unit)' : `${all['res_u']!.unit}, monitors.get (the series’ unit)`);
  await page.keyboard.press('Escape');
  // the iteration of run.status: Ctrl+I on the focused number
  const iteration = page.locator('.plots-dev-state').getByRole('button', { name: /^Provenance of iteration: \d+$/ });
  await iteration.focus();
  await page.keyboard.press('Control+i');
  const it = page.getByRole('dialog', { name: 'Provenance of iteration' });
  await expect(it.locator('[data-fact="Read by"] dd')).toHaveText('run.status of rcfd serve (rcfd-service/1) (simulation.iteration)');
  await page.keyboard.press('Escape');
});

test('no Content Security Policy violation and no console error', async () => {
  expect(await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp)).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
