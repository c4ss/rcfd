// P3 of ADR 0350 measured through the product shell for the plots (F-811, docs/studio-guardrails.md G9), by the method of
// ADR 0350 A1.4 and ADR 0353 (bench/electron check `p3`): a 4 x 4 x 4 lid-driven box with one force report
// (`[reports.lid_drag]`), stepped 1e5 times; `monitors.get {names}` of the first four series by name, request to parsed
// object in the renderer, 20 repeats. Limit: median <= 100 ms. Beside it, recorded and not judged:
//
// - P3 main: the same request in the Studio's main process alone (ServiceClient, the reply parsed there), 20 repeats;
// - I1: `monitors.get {names, since}` returning the last 100 samples of each series (ADR 0353 I1), 20 warm-up, 200 timed;
// - the plots on #/dev/plots opened on that session: the feed's first whole fetch, and the first draw of the residual
//   and monitor plots (three.js), the canvas's own timings (request to drawn, render call);
// - the renderer comparison of ADR 0801 addendum 3: four polylines of 1e5 points as one SVG, time from insertion to the
//   next frame, and the same after an append of 100 points (an SVG path is rebuilt whole), against the three.js plot's
//   increment, which uploads only its new segments.
//
//   npm run build && node scripts/package-dir.mjs
//   node scripts/measure-plots.mjs [--rcfd=<rcfd executable>] [--out=<file.json>] [--smoke]
//
// A heavy record (house rule 7): run by hand, once, and recorded in ADR 0801 addendum 3. Not part of npm test. --smoke
// steps 300 times to check the script.
import { _electron as electron } from '@playwright/test';
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3) ?? '';
const smoke = process.argv.includes('--smoke');
const exe = process.platform === 'win32' ? 'rcfd.exe' : 'rcfd';
const rcfd = arg('rcfd') || process.env['RCFD_STUDIO_RCFD'] || [path.join(root, '..', '..', 'target', 'release', exe)].find((p) => existsSync(p));
if (!rcfd || !existsSync(rcfd)) {
  console.error('usage: node scripts/measure-plots.mjs --rcfd=<rcfd executable> [--out=<file.json>] [--smoke]');
  process.exit(2);
}
const Q = smoke ? { steps: 300, repeats: 3, iWarm: 3, iTimed: 10 } : { steps: 100_000, repeats: 20, iWarm: 20, iTimed: 200 };

const TOML = `title = "F-811 P3 lid-driven box"
[mesh]
generator = "box_3d"
nx = 4
ny = 4
nz = 4
lx = 1.0
ly = 1.0
lz = 1.0
[physics]
nu = 0.01
${['xmin', 'xmax', 'ymin', 'zmin', 'zmax'].map((p) => `[boundaries.${p}]\nvelocity = "fixed"\n`).join('')}[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 1000000
tolerance_u = 1e-14
tolerance_p = 1e-14
[reports.lid_drag]
type = "force"
patch = "ymax"
component = "viscous"
`;

function cpuTimes() {
  let busy = 0;
  let total = 0;
  for (const c of os.cpus()) {
    const t = c.times;
    busy += t.user + t.nice + t.sys + t.irq;
    total += t.user + t.nice + t.sys + t.irq + t.idle;
  }
  return { busy, total };
}

function builders() {
  if (process.platform !== 'win32') return null;
  try {
    const list = execFileSync('tasklist', ['/fo', 'csv', '/nh'], { encoding: 'utf8' });
    return list.split('\n').filter((l) => /^"(cargo|rustc|link)\.exe"/i.test(l)).length;
  } catch {
    return null;
  }
}

async function load() {
  const a = cpuTimes();
  await new Promise((r) => setTimeout(r, 1000));
  const b = cpuTimes();
  return { at: new Date().toISOString(), cpuBusyFraction: (b.busy - a.busy) / Math.max(1, b.total - a.total), cargoRustcLinkProcesses: builders(), freeMemGB: os.freemem() / 2 ** 30 };
}

const stats = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  const pct = (p) => s[Math.max(0, Math.ceil(p * s.length) - 1)];
  return { n: s.length, median: pct(0.5), p99: pct(0.99), min: s[0], max: s[s.length - 1] };
};

const scratch = mkdtempSync(path.join(os.tmpdir(), 'rcfd-p3-'));
mkdirSync(path.join(scratch, 'root'));
const loadBefore = await load();
console.log(`P3 through the product shell${smoke ? ' (smoke)' : ''}; load ${loadBefore.cpuBusyFraction.toFixed(2)}, ${loadBefore.cargoRustcLinkProcesses} cargo/rustc/link`);

const server = spawn(rcfd, ['serve', '--addr', '127.0.0.1:0', '--root', path.join(scratch, 'root')], { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
const port = await new Promise((resolve, reject) => {
  let text = '';
  const onData = (d) => {
    text += d.toString('utf8');
    const m = /listening on 127\.0\.0\.1:(\d+)/.exec(text);
    if (m) resolve(Number(m[1]));
  };
  server.stdout.on('data', onData);
  server.stderr.on('data', onData);
  server.once('error', reject);
  server.once('exit', (c) => reject(new Error(`rcfd serve ended with ${c}`)));
});

const { executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8'));
const env = {};
for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_')) env[k] = v;
const app = await electron.launch({ executablePath: executable, args: [], env: { ...env, RCFD_STUDIO_SERVICE: `127.0.0.1:${port}`, RCFD_STUDIO_BENCH: '1', RCFD_STUDIO_STATE_DIR: path.join(scratch, 'state') } });
let result;
try {
  const page = await app.firstWindow();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
  await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].setContentSize(1440, 900));
  await page.waitForFunction(async () => (await window.studioService.state()).status === 'connected', null, { timeout: 15_000 });
  const versions = await page.evaluate(() => window.studio.app.info());

  // ---- the session: built and stepped in the main process (not timed)
  const t0 = Date.now();
  const setup = await app.evaluate(async (_e, q) => {
    const svc = globalThis.rcfdStudioBench.service;
    const { session } = await svc.call('session.create', { toml: q.toml }, 120_000);
    const st = await svc.call('run.step', { session, count: q.steps }, 3_600_000);
    return { session, state: st.state, iteration: st.iteration };
  }, { toml: TOML, steps: Q.steps });
  const stepS = (Date.now() - t0) / 1000;
  const loadDuring = await load();
  console.log(`session ${setup.session} stepped ${Q.steps} in ${stepS.toFixed(1)} s (${setup.state})`);

  // ---- P3 in the renderer: request to parsed object (the reply arrives packed: Float64Arrays, src/shared/monitors.ts)
  const p3 = await page.evaluate(async (q) => {
    const all = await window.studioService.call('monitors.get', { session: q.session }, { timeoutMs: 120_000 });
    if (!all.ok) throw new Error(all.error.message);
    const names = Object.keys(all.result.series).sort().slice(0, 4);
    const samples = names.map((n) => all.result.series[n].x.length);
    const ms = [];
    let parsed = 0;
    let kinds = '';
    for (let i = 0; i < q.repeats; i++) {
      const t = performance.now();
      const r = await window.studioService.call('monitors.get', { session: q.session, names }, { timeoutMs: 120_000 });
      ms.push(performance.now() - t);
      if (!r.ok) throw new Error(r.error.message);
      parsed = Object.keys(r.result.series).length;
      kinds = Object.values(r.result.series).map((s) => s.x.constructor.name).join(',');
    }
    // I1: the last 100 samples of each series
    const xs = all.result.series[names[0]].x;
    const since = xs[xs.length - 101];
    const ims = [];
    let counts = [];
    for (let i = 0; i < q.iWarm + q.iTimed; i++) {
      const t = performance.now();
      const r = await window.studioService.call('monitors.get', { session: q.session, names, since });
      const dt = performance.now() - t;
      if (!r.ok) throw new Error(r.error.message);
      if (i >= q.iWarm) ims.push(dt);
      counts = names.map((n) => r.result.series[n].x.length);
    }
    return { ms, names, samples, parsed, kinds, since, ims, counts };
  }, { session: setup.session, repeats: Q.repeats, iWarm: Q.iWarm, iTimed: Q.iTimed });

  // ---- P3 in the main process alone (recorded)
  const p3main = await app.evaluate(async (_e, q) => {
    const svc = globalThis.rcfdStudioBench.service;
    const ms = [];
    for (let i = 0; i < q.repeats; i++) {
      const t = performance.now();
      await svc.call('monitors.get', { session: q.session, names: q.names }, 120_000);
      ms.push(performance.now() - t);
    }
    return ms;
  }, { session: setup.session, names: p3.names, repeats: Q.repeats });

  // ---- the plots on the dev route: first whole fetch and first draw
  const tOpen = Date.now();
  await page.evaluate((s) => {
    location.hash = `#/dev/plots?session=${s}`;
  }, setup.session);
  await page.waitForFunction(() => {
    const c = [...document.querySelectorAll('.plot-canvas')];
    return c.length === 2 && c.every((e) => e.dataset.drawMs !== undefined && Number(e.dataset.segments) > 0);
  }, null, { timeout: 120_000 });
  const openToDrawnMs = Date.now() - tOpen;
  const plots = await page.evaluate(() => ({
    feed: { ...document.querySelector('[data-testid="feed-stats"]').dataset },
    frames: [...document.querySelectorAll('.plot-frame')].map((f) => ({ ...f.dataset, ...f.querySelector('.plot-canvas').dataset })),
  }));
  await page.screenshot({ path: path.join(scratch, 'plots-1e5.png') });

  // ---- the renderer comparison: SVG polylines of the same four series, whole and after an append of 100 points
  const svg = await page.evaluate(async (q) => {
    const r = await window.studioService.call('monitors.get', { session: q.session, names: q.names }, { timeoutMs: 120_000 });
    const series = Object.values(r.result.series);
    const frame = () => new Promise((res) => requestAnimationFrame(() => requestAnimationFrame(() => res(undefined))));
    const NS = 'http://www.w3.org/2000/svg';
    const host = document.createElementNS(NS, 'svg');
    host.setAttribute('width', '1000');
    host.setAttribute('height', '150');
    host.setAttribute('viewBox', '0 0 1000 150');
    host.style.position = 'fixed';
    host.style.left = '0';
    host.style.top = '0';
    const build = (s, extra) => {
      const n = s.x.length;
      let lo = Infinity;
      let hi = -Infinity;
      for (let i = 0; i < n; i++) {
        const v = s.y[i];
        if (Number.isFinite(v)) {
          lo = Math.min(lo, v);
          hi = Math.max(hi, v);
        }
      }
      const parts = [];
      const total = n + extra;
      for (let i = 0; i < total; i++) {
        const k = Math.min(i, n - 1);
        const v = s.y[k];
        if (!Number.isFinite(v)) continue;
        parts.push(`${parts.length ? 'L' : 'M'}${((i / total) * 1000).toFixed(3)} ${(150 - ((v - lo) / (hi - lo || 1)) * 150).toFixed(3)}`);
      }
      return parts.join('');
    };
    const paths = series.map(() => {
      const p = document.createElementNS(NS, 'path');
      p.setAttribute('fill', 'none');
      p.setAttribute('stroke', 'black');
      host.append(p);
      return p;
    });
    await frame();
    let t = performance.now();
    series.forEach((s, i) => paths[i].setAttribute('d', build(s, 0)));
    document.body.append(host);
    await frame();
    const wholeMs = performance.now() - t;
    const appendMs = [];
    for (let k = 1; k <= 5; k++) {
      t = performance.now();
      series.forEach((s, i) => paths[i].setAttribute('d', build(s, 100 * k)));
      await frame();
      appendMs.push(performance.now() - t);
    }
    host.remove();
    // the empty frame: two animation frames with nothing to do, the floor of both numbers above
    t = performance.now();
    await frame();
    const emptyMs = performance.now() - t;
    return { wholeMs, appendMs, emptyMs, points: series.reduce((a, s) => a + s.x.length, 0) };
  }, { session: setup.session, names: p3.names });

  // ---- the three.js plot's increment: 100 more steps, the feed's poll and the draw that follows
  await app.evaluate(async (_e, q) => globalThis.rcfdStudioBench.service.call('run.step', { session: q.session, count: 100 }, 600_000), { session: setup.session });
  await page.waitForFunction((n) => Number(document.querySelector('[data-testid="feed-stats"]').dataset.increments) >= 1 && document.querySelector('.plot--monitor .plot-frame')?.dataset.drawn === String(n), p3.samples[0] + 100, { timeout: 60_000 }).catch(() => undefined);
  await page.waitForTimeout(500);
  const afterIncrement = await page.evaluate(() => ({
    feed: { ...document.querySelector('[data-testid="feed-stats"]').dataset },
    stats: document.querySelector('[data-testid="feed-stats"]').textContent,
    frames: [...document.querySelectorAll('.plot-frame')].map((f) => ({ ...f.dataset, ...f.querySelector('.plot-canvas').dataset })),
  }));

  const loadAfter = await load();
  result = {
    smoke,
    versions,
    rcfd,
    setup: { ...setup, steps: Q.steps, stepSeconds: stepS },
    p3: { ms: stats(p3.ms), raw: p3.ms, names: p3.names, samples: p3.samples, seriesParsed: p3.parsed, arrays: p3.kinds, limitMs: 100, met: stats(p3.ms).median <= 100 },
    p3main: { ms: stats(p3main), raw: p3main },
    i1: { ms: stats(p3.ims), since: p3.since, countsReturned: p3.counts, limitMs: 2 },
    plots: { openToDrawnMs, ...plots },
    svg,
    afterIncrement,
    load: { before: loadBefore, during: loadDuring, after: loadAfter },
    screenshot: path.join(scratch, 'plots-1e5.png'),
  };
} finally {
  await app.close();
  server.kill();
}
const text = JSON.stringify(result, null, 2);
console.log(text);
const out = arg('out');
if (out) writeFileSync(out, text);
if (!out) rmSync(scratch, { recursive: true, force: true });
