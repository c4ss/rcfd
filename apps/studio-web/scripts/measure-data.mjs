// P4 of ADR 0350 measured again through the product shell (F-804, docs/studio-guardrails.md G9), by the method of ADR
// 0350 A1.4 with the Studio's own data path in place of the spike's, and P12 over the larger data sets (G3):
//
// - P4: one snapshot of the 1e6-cell scalar `p` (4 MB of f32), from the renderer's request (`window.studioData.field`)
//   to `queue.writeBuffer` into an existing GPU storage buffer followed by `queue.onSubmittedWorkDone()`; snapshot i mod
//   20 for repeat i; 20 warm-up and 200 timed repeats per route and store codec (R1 raw, R1 deflated, R2 raw, R2
//   deflated). Limit: p50 <= 30 ms, p99 <= 60 ms; a route meets P4 on the raw store (what the run path writes), the
//   deflated line is reported with its own met or not met. Beside it (G21, recorded, not judged): the longest gap of a
//   1 ms interval timer in the main process during each line (its idle gap is measured first), and the renderer's long
//   tasks (PerformanceObserver 'longtask', over 50 ms).
// - P12 (--p12): every field of every snapshot of the 1e6 box (four fields) and `p` of the 4e6 box (as ADR 0350 did), by
//   both routes and both codecs: SHA-256 of the values the page received (crypto.subtle, in the renderer) against
//   SHA-256 of the Rust reader's reference bytes (ref/<size>/<snapshot>-<field>.f32).
//
//   npm run build && node scripts/package-dir.mjs
//   f447_bench gen <folder> 1e6 4e6                  (crates/rcfd-api/examples/f447_bench.rs; once, by hand)
//   node scripts/measure-data.mjs --bench-server=<f447_bench(.exe)> --data=<folder> [--p12] [--out=<file.json>] [--smoke]
//
// A heavy record (house rule 7): run by hand, once, and recorded in ADR 0801 addendum 3. Not part of npm test. --smoke
// runs a small version (the 1e5 box, if present, else whatever size there is) to check the script.
import { _electron as electron } from '@playwright/test';
import { execFileSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3) ?? '';
const smoke = process.argv.includes('--smoke');
const withP12 = process.argv.includes('--p12');
const benchServer = arg('bench-server');
const data = arg('data') ? path.resolve(arg('data')) : '';
if (!benchServer || !data) {
  console.error('usage: node scripts/measure-data.mjs --bench-server=<f447_bench executable> --data=<folder of f447_bench gen> [--p12] [--out=<file.json>] [--smoke]');
  process.exit(2);
}
const P4_SIZE = smoke ? '1e5' : '1e6';
const Q = smoke ? { warm: 3, timed: 20 } : { warm: 20, timed: 200 };
const FIELDS = ['p', 'ux', 'uy', 'uz'];
const SNAPSHOTS = 20;
const storeDir = (size, codec) => path.join(data, 'project', 'runs', `box-${size}-${codec}`, 'fields');
const servicePath = (size, codec) => `project/runs/box-${size}-${codec}/fields`;
for (const codec of ['raw', 'deflate']) {
  if (!existsSync(path.join(storeDir(P4_SIZE, codec), 'store.rts'))) {
    console.error(`no ${P4_SIZE} ${codec} store in ${data}: run f447_bench gen ${data} ${P4_SIZE}`);
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
  return {
    at: new Date().toISOString(),
    cpuBusyFraction: (b.busy - a.busy) / Math.max(1, b.total - a.total),
    cargoRustcLinkProcesses: builders(),
    freeMemGB: os.freemem() / 2 ** 30,
  };
}

const sorted = (xs) => [...xs].sort((a, b) => a - b);
const pct = (s, p) => s[Math.ceil(p * s.length) - 1];
const summary = (xs) => {
  const s = sorted(xs);
  return { n: s.length, p50: pct(s, 0.5), p99: pct(s, 0.99), max: s[s.length - 1], min: s[0] };
};

const loadBefore = await load();
console.log(`P4${withP12 ? ', P12' : ''} through the product shell${smoke ? ' (smoke)' : ''}; load ${loadBefore.cpuBusyFraction.toFixed(2)}, ${loadBefore.cargoRustcLinkProcesses} cargo/rustc/link`);

// the spike's server (data.* on), confined to the data folder; it prints {rpc, data, events, root} and ends when stdin closes
const server = spawn(benchServer, ['serve', data], { stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
const ports = await new Promise((resolve, reject) => {
  let text = '';
  server.stdout.on('data', (d) => {
    text += d.toString('utf8');
    const nl = text.indexOf('\n');
    if (nl >= 0) resolve(JSON.parse(text.slice(0, nl)));
  });
  server.once('error', reject);
  server.once('exit', (c) => reject(new Error(`f447_bench serve ended with ${c}`)));
});

const { executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8'));
const env = {};
for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_')) env[k] = v;
const app = await electron.launch({
  executablePath: executable,
  args: [],
  env: { ...env, RCFD_STUDIO_SERVICE: `127.0.0.1:${ports.rpc}`, RCFD_STUDIO_DATA_ROOTS: data, RCFD_STUDIO_STATE_DIR: path.join(os.tmpdir(), 'rcfd-f804-state') },
});

/** In the main process: a 1 ms interval timer recording its longest gap (how long the event loop was held). */
const lagStart = () =>
  app.evaluate(() => {
    const g = globalThis;
    if (g.__lagTimer) clearInterval(g.__lagTimer);
    g.__lag = { max: 0, ticks: 0, last: performance.now() };
    g.__lagTimer = setInterval(() => {
      const t = performance.now();
      g.__lag.max = Math.max(g.__lag.max, t - g.__lag.last);
      g.__lag.ticks++;
      g.__lag.last = t;
    }, 1);
  });
const lagStop = () =>
  app.evaluate(() => {
    const g = globalThis;
    clearInterval(g.__lagTimer);
    g.__lagTimer = null;
    return { maxGapMs: g.__lag.max, ticks: g.__lag.ticks };
  });

let result;
try {
  const page = await app.firstWindow();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
  await page.waitForFunction(async () => (await window.studioService.state()).status === 'connected', null, { timeout: 15_000 });
  const versions = await page.evaluate(() => window.studio.app.info());
  const window_ = await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().map((w) => ({ content: w.getContentBounds(), visible: w.isVisible() })));

  // ---- the GPU device and one storage buffer of the field's size, made before any timing
  const gpu = await page.evaluate(async (bytes) => {
    if (!navigator.gpu) return { error: 'navigator.gpu is missing' };
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
    if (!adapter) return { error: 'no WebGPU adapter' };
    const device = await adapter.requestDevice();
    const buffer = device.createBuffer({ size: bytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    globalThis.__p4 = { device, buffer };
    const i = adapter.info ?? {};
    return { vendor: i.vendor, architecture: i.architecture, device: i.device, description: i.description };
  }, 4 * (smoke ? 100_000 : 1_000_000));
  if (gpu.error) throw new Error(`P4 not run: ${gpu.error}`);

  // ---- idle gap of the main process's timer (what the timer gives with nothing to do)
  await lagStart();
  await new Promise((r) => setTimeout(r, 3000));
  const idleLag = await lagStop();

  // ---- P4 per route and codec
  const lines = [];
  for (const [route, codec] of [
    ['service', 'raw'],
    ['service', 'deflate'],
    ['local', 'raw'],
    ['local', 'deflate'],
  ]) {
    const store = { route, path: route === 'local' ? storeDir(P4_SIZE, codec) : servicePath(P4_SIZE, codec) };
    await lagStart();
    const r = await page.evaluate(
      async ({ store, q }) => {
        const { device, buffer } = globalThis.__p4;
        const long = [];
        const obs = new PerformanceObserver((list) => {
          for (const e of list.getEntries()) long.push(e.duration);
        });
        obs.observe({ entryTypes: ['longtask'] });
        const once = async (i) => {
          const t = performance.now();
          const f = await window.studioData.field(store, i % 20, 'p');
          if (!f.ok) throw new Error(`${f.error.code}: ${f.error.message}`);
          device.queue.writeBuffer(buffer, 0, f.result.values);
          await device.queue.onSubmittedWorkDone();
          return performance.now() - t;
        };
        for (let i = 0; i < q.warm; i++) await once(i);
        const longWarm = long.length;
        const ms = [];
        for (let i = 0; i < q.timed; i++) ms.push(await once(i));
        await new Promise((r) => setTimeout(r, 100));
        obs.disconnect();
        const timedLong = long.slice(longWarm);
        return { ms, longTasks: { n: timedLong.length, maxMs: timedLong.length ? Math.max(...timedLong) : 0 } };
      },
      { store, q: Q },
    );
    const lag = await lagStop();
    const s = summary(r.ms);
    const line = { route: route === 'local' ? 'R2' : 'R1', codec, ms: s, met: s.p50 <= 30 && s.p99 <= 60, mainLoopMaxGapMs: lag.maxGapMs, rendererLongTasks: r.longTasks };
    lines.push(line);
    console.log(`P4 ${line.route} ${codec.padEnd(7)} p50 ${s.p50.toFixed(1)}, p99 ${s.p99.toFixed(1)}, max ${s.max.toFixed(1)} ms (n ${s.n}) ${line.met ? 'met' : 'NOT met'}; main timer max gap ${lag.maxGapMs.toFixed(1)} ms; renderer long tasks ${r.longTasks.n} (max ${r.longTasks.maxMs.toFixed(1)} ms)`);
  }

  // ---- P12 over the large data sets (by SHA-256 in the renderer)
  let p12 = null;
  if (withP12) {
    p12 = [];
    const plan = [
      { size: smoke ? '1e5' : '1e6', fields: FIELDS },
      ...(smoke ? [] : [{ size: '4e6', fields: ['p'] }]),
    ];
    for (const { size, fields } of plan) {
      if (!existsSync(path.join(storeDir(size, 'raw'), 'store.rts'))) {
        p12.push({ size, notRun: 'no store' });
        continue;
      }
      const want = [];
      for (let i = 0; i < SNAPSHOTS; i++) for (const f of fields) want.push(createHash('sha256').update(readFileSync(path.join(data, 'ref', size, `${i}-${f}.f32`))).digest('hex'));
      for (const [route, codec] of [
        ['local', 'raw'],
        ['local', 'deflate'],
        ['service', 'raw'],
        ['service', 'deflate'],
      ]) {
        const store = { route, path: route === 'local' ? storeDir(size, codec) : servicePath(size, codec) };
        const got = await page.evaluate(
          async ({ store, fields, snapshots }) => {
            const hex = (b) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, '0')).join('');
            const out = [];
            let values = 0;
            for (let i = 0; i < snapshots; i++) {
              for (const f of fields) {
                const r = await window.studioData.field(store, i, f, { timeoutMs: 600_000 });
                if (!r.ok) throw new Error(`${i}-${f}: ${r.error.code} ${r.error.message}`);
                values += r.result.values.length;
                out.push(hex(await crypto.subtle.digest('SHA-256', r.result.values)));
              }
            }
            return { out, values };
          },
          { store, fields, snapshots: SNAPSHOTS },
        );
        const differ = got.out.filter((h, k) => h !== want[k]).length;
        const line = { size, route: route === 'local' ? 'R2' : 'R1', codec, fields: got.out.length, values: got.values, differingFields: differ };
        p12.push(line);
        console.log(`P12 ${size} ${line.route} ${codec.padEnd(7)} ${line.fields} fields, ${line.values} values, ${differ} differing`);
      }
    }
  }

  result = {
    checks: `P4${withP12 ? ', P12' : ''} (ADR 0350) through the product shell (F-804)`,
    smoke,
    machine: { cpu: os.cpus()[0]?.model, logical: os.cpus().length, totalMemGB: os.totalmem() / 2 ** 30, platform: `${os.platform()} ${os.release()}` },
    launcherNode: process.version,
    versions,
    window: window_,
    gpu,
    size: P4_SIZE,
    repeats: Q,
    idleMainTimerMaxGapMs: idleLag.maxGapMs,
    P4: lines,
    // ADR 0350 §3: "R1 or R2"; a route meets P4 on the raw store
    P4met: lines.some((l) => l.codec === 'raw' && l.met),
    P4routesMet: lines.filter((l) => l.codec === 'raw' && l.met).map((l) => l.route),
    P12: p12,
    loadBefore,
  };
} finally {
  await app.close();
  server.stdin.end();
  await new Promise((r) => (server.exitCode !== null ? r() : server.once('exit', r)));
}
result.loadAfter = await load();
console.log(`idle main timer max gap ${result.idleMainTimerMaxGapMs.toFixed(1)} ms; GPU ${JSON.stringify(result.gpu)}`);
console.log(`P4 (R1 or R2 on the raw store: p50 <= 30, p99 <= 60 ms): ${result.P4met ? `met by ${result.P4routesMet.join(' and ')}` : 'NOT met'}`);
console.log(`load after ${result.loadAfter.cpuBusyFraction.toFixed(2)}, ${result.loadAfter.cargoRustcLinkProcesses} cargo/rustc/link`);
const out = arg('out');
if (out) writeFileSync(out, JSON.stringify(result, null, 1) + '\n');
