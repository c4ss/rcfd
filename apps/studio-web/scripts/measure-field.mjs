// P7 and P8 of ADR 0350 measured through the product shell (F-808, docs/studio-guardrails.md G9), with the field
// viewport of src/renderer/field (three.js WebGPURenderer, the TSL material, the data door) in place of the spike's
// hand-written WebGPU code:
//
// - P7, first picture of the 1e6-cell surface: from the viewport's open of the store (performance mark
//   `rcfd:field:open`) through `data.open`, the surface and field `p` of snapshot 19, the geometry and the draw, to the
//   first animation frame after the GPU finished it (measure `rcfd:field:first-picture`, src/renderer/field/
//   FieldViewport.tsx). As ADR 0350: the device exists before (the viewport is open on the tiny box first, and switches
//   store between repeats: its renderer stays, the surface's buffers and the material's pipeline are made inside the
//   timed part); 5 repeats per route, judged on the median. The first of them is the first open of the 1e6 store in
//   the launch (of the Studio, and of the bench server for R1), before the reader's worker remembers its mesh check; it
//   is reported separately and judged by scripts/bench.mjs against the same limit (F-819, the maintainer's answer to
//   Q-819b-1 of 2026-11-17). Route R2 (local) always, route R1 (service) when --bench-server is given. Limit: 1.5 s.
// - P8, 1e6 triangles orbiting: #/dev/field?bench=p8, the viewport's canvas on a synthetic 1000 x 500 x 2 surface with a
//   value per triangle, coloured by the same material, one turn per 600 frames; 120 warm-up frames, 1200 timed frame
//   intervals of requestAnimationFrame. Taken twice as ADR 0350: with vertical sync (the p99 frame is judged there,
//   limit 25 ms) and with --disable-frame-rate-limit --disable-gpu-vsync, each frame waiting for the GPU (the frames per
//   second are judged there, limit 60).
//
//   npm run build && node scripts/package-dir.mjs
//   f447_bench gen <folder> tiny 1e6                 (crates/rcfd-api/examples/f447_bench.rs; once, by hand)
//   node scripts/measure-field.mjs --data=<folder> [--bench-server=<f447_bench(.exe)>] [--out=<file.json>] [--smoke]
// --profile-root=<folder>: a fresh Chromium profile per launch under it (scripts/profile.mjs; F-817b, ADR 0800 §9 Q5).
//
// A heavy record (house rule 7): run by hand, once, and recorded in ADR 0801, addendum F-808. Not part of npm test. --smoke
// uses the 1e5 box for P7 and 120 timed frames for P8, to check the script.
import { _electron as electron } from '@playwright/test';
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { profileArgs, profileRecord } from './profile.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3) ?? '';
const smoke = process.argv.includes('--smoke');
const data = arg('data') ? path.resolve(arg('data')) : '';
const benchServer = arg('bench-server');
if (!data) {
  console.error('usage: node scripts/measure-field.mjs --data=<folder of f447_bench gen> [--bench-server=<f447_bench>] [--out=<file.json>] [--smoke]');
  process.exit(2);
}
const SIZE = smoke ? '1e5' : '1e6';
const REPEATS = 5;
const storeDir = (size) => path.join(data, 'project', 'runs', `box-${size}-raw`, 'fields');
const servicePath = (size) => `project/runs/box-${size}-raw/fields`;
for (const size of ['tiny', SIZE]) {
  if (!existsSync(path.join(storeDir(size), 'store.rts'))) {
    console.error(`no ${size} raw store in ${data}: run f447_bench gen ${data} tiny ${SIZE}`);
    process.exit(2);
  }
}

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
    return execFileSync('tasklist', ['/fo', 'csv', '/nh'], { encoding: 'utf8' })
      .split('\n')
      .filter((l) => /^"(cargo|rustc|link)\.exe"/i.test(l)).length;
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
const sorted = (xs) => [...xs].sort((a, b) => a - b);
const pct = (s, p) => s[Math.ceil(p * s.length) - 1];
const summary = (xs) => {
  const s = sorted(xs);
  return { n: s.length, p50: pct(s, 0.5), p99: pct(s, 0.99), max: s[s.length - 1], min: s[0] };
};

const { executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8'));
const baseEnv = {};
for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_')) baseEnv[k] = v;
// as ADR 0350: Chromium's occlusion tracking off, so that a covering window cannot stop the frames
const OCCLUSION = '--disable-features=CalculateNativeWinOcclusion';

async function launch(extraArgs, env) {
  const app = await electron.launch({ chromiumSandbox: true, executablePath: executable, args: [OCCLUSION, ...profileArgs(), ...extraArgs], env: { ...baseEnv, RCFD_STUDIO_DATA_ROOTS: data, RCFD_STUDIO_STATE_DIR: path.join(os.tmpdir(), 'rcfd-f808-state'), ...env } });
  const page = await app.firstWindow();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
  await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].setContentSize(1280, 800));
  return { app, page };
}

const fieldHash = (route, p) => `#/dev/field?route=${route}&path=${encodeURIComponent(p)}&field=p&snapshot=19`;

/** Sets the hash and waits for the viewport's first picture of that store; returns the measure (ms). */
async function firstPicture(page, hash) {
  await page.evaluate(() => {
    performance.clearMeasures('rcfd:field:first-picture');
  });
  await page.evaluate((h) => (location.hash = h), hash);
  await page.waitForFunction(() => performance.getEntriesByName('rcfd:field:first-picture').length > 0 || document.querySelector('.fv-status[role=alert]') !== null, null, {
    timeout: 120_000,
  });
  const r = await page.evaluate(() => ({
    ms: performance.getEntriesByName('rcfd:field:first-picture')[0]?.duration ?? null,
    error: document.querySelector('.fv-status[role=alert]')?.textContent ?? null,
    backend: document.querySelector('.fv-backend')?.textContent ?? null,
  }));
  if (r.ms === null) throw new Error(`no picture: ${r.error}`);
  return r;
}

const loadBefore = await load();
console.log(`P7, P8 through the product shell${smoke ? ' (smoke)' : ''}; load ${loadBefore.cpuBusyFraction.toFixed(2)}, ${loadBefore.cargoRustcLinkProcesses} cargo/rustc/link`);

let server = null;
let ports = null;
if (benchServer) {
  server = spawn(benchServer, ['serve', data], { stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
  ports = await new Promise((resolve, reject) => {
    let text = '';
    server.stdout.on('data', (d) => {
      text += d.toString('utf8');
      const nl = text.indexOf('\n');
      if (nl >= 0) resolve(JSON.parse(text.slice(0, nl)));
    });
    server.once('error', reject);
    server.once('exit', (c) => reject(new Error(`f447_bench serve ended with ${c}`)));
  });
}

const result = { checks: 'P7, P8 (ADR 0350) through the product shell (F-808)', smoke, size: SIZE, loadBefore, machine: { cpu: os.cpus()[0]?.model, logical: os.cpus().length, totalMemGB: os.totalmem() / 2 ** 30, platform: `${os.platform()} ${os.release()}` }, launcherNode: process.version };
try {
  // ---- P7
  {
    const { app, page } = await launch([], ports ? { RCFD_STUDIO_SERVICE: `127.0.0.1:${ports.rpc}` } : {});
    try {
      if (ports) await page.waitForFunction(async () => (await window.studioService.state()).status === 'connected', null, { timeout: 15_000 });
      result.versions = await page.evaluate(() => window.studio.app.info());
      result.window = await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().map((w) => ({ content: w.getContentBounds(), visible: w.isVisible() })));
      result.gpu = await page.evaluate(async () => {
        const a = await navigator.gpu?.requestAdapter();
        const i = a?.info ?? {};
        return { vendor: i.vendor, architecture: i.architecture, description: i.description };
      });
      const warm = await firstPicture(page, fieldHash('local', storeDir('tiny')));
      result.backend = warm.backend;
      result.P7 = [];
      for (const route of ports ? ['local', 'service'] : ['local']) {
        const big = route === 'local' ? storeDir(SIZE) : servicePath(SIZE);
        const small = route === 'local' ? storeDir('tiny') : servicePath('tiny');
        const ms = [];
        for (let i = 0; i < REPEATS; i++) {
          await new Promise((r) => setTimeout(r, 200));
          ms.push((await firstPicture(page, fieldHash(route, big))).ms);
          await firstPicture(page, fieldHash(route, small));
        }
        const s = summary(ms);
        const line = { route: route === 'local' ? 'R2' : 'R1', ms: s, first: ms[0], all: ms, met: s.p50 <= 1500, firstMet: ms[0] <= 1500 };
        result.P7.push(line);
        console.log(`P7 ${line.route} median ${s.p50.toFixed(0)} ms ${line.met ? 'met' : 'NOT met'}, first open of the launch ${ms[0].toFixed(0)} ms ${line.firstMet ? 'met' : 'NOT met'}, all ${ms.map((x) => x.toFixed(0)).join(', ')}`);
      }
      // the viewport mounted cold on the big store: renderer, device and pipeline inside the time too (recorded only)
      await page.evaluate(() => (location.hash = '#/dev/field'));
      await page.waitForSelector('.fd-empty');
      await new Promise((r) => setTimeout(r, 300));
      result.P7coldMount = (await firstPicture(page, fieldHash('local', storeDir(SIZE)))).ms;
      console.log(`P7 cold mount (renderer and device created inside the time; recorded, not judged): ${result.P7coldMount.toFixed(0)} ms`);
    } finally {
      await app.close();
    }
  }

  // ---- P8, with vertical sync and without
  result.P8 = [];
  for (const uncapped of [false, true]) {
    const { app, page } = await launch(uncapped ? ['--disable-frame-rate-limit', '--disable-gpu-vsync'] : [], {});
    try {
      await page.evaluate((h) => (location.hash = h), `#/dev/field?bench=p8${uncapped ? '&uncapped=1' : ''}`);
      const out = await page.waitForSelector('output[data-bench="p8"]', { timeout: 300_000 });
      const r = JSON.parse((await out.textContent()) ?? '{}');
      const frames = smoke ? r.frameMs.slice(0, 120) : r.frameMs;
      const s = summary(frames);
      const total = frames.reduce((a, b) => a + b, 0);
      const fps = (1000 * frames.length) / total;
      const displayHz = 1000 / summary(r.emptyFrameMs).p50;
      const work = r.workMs.length ? summary(smoke ? r.workMs.slice(0, 120) : r.workMs) : null;
      const line = { uncapped, triangles: r.triangles, backend: r.backend, frameMs: s, fps, displayHz, workMs: work, met: uncapped ? fps >= 60 : s.p99 <= 25 };
      result.P8.push(line);
      console.log(`P8 ${uncapped ? 'no rate limit' : 'vertical sync'}: ${r.triangles} triangles, ${fps.toFixed(1)} frames per s, frame p50 ${s.p50.toFixed(2)}, p99 ${s.p99.toFixed(2)}, max ${s.max.toFixed(2)} ms; display ${displayHz.toFixed(1)} Hz; ${line.met ? 'met' : 'NOT met'} (${uncapped ? '>= 60 per s' : 'p99 <= 25 ms'})${work ? `; draw to GPU done p50 ${work.p50.toFixed(2)}, p99 ${work.p99.toFixed(2)} ms` : ''}`);
    } finally {
      await app.close();
    }
  }
} finally {
  if (server) {
    server.stdin.end();
    await new Promise((r) => (server.exitCode !== null ? r() : server.once('exit', r)));
  }
}
result.loadAfter = await load();
console.log(`backend ${result.backend}; GPU ${JSON.stringify(result.gpu)}; load after ${result.loadAfter.cpuBusyFraction.toFixed(2)}, ${result.loadAfter.cargoRustcLinkProcesses} cargo/rustc/link`);
const out = arg('out');
result.profile = profileRecord();
if (out) writeFileSync(out, JSON.stringify(result, null, 1) + '\n');
