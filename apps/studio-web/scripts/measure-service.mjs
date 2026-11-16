// P1 and P2 of ADR 0350 measured again through the product shell (F-803, docs/studio-guardrails.md G9), by the method of
// ADR 0350 A1.4, with the Studio's own service client in place of the spike's:
//
// - P1: round trip of `service.info` and `run.status` (a ready session of the 1e5-cell box) from Node, in the Studio's
//   main process through its ServiceClient (the control connection); 50 warm-up calls, then 1000 timed calls of each.
//   Limit: p50 <= 0.5 ms, p99 <= 2 ms. The same calls from the renderer through window.studioService (two more IPC
//   hops) are recorded beside it, not judged.
// - P2: latency from the solver's `iteration` event to the renderer, the session running (`run.start`): the Studio's
//   events.poll loop in the main process, the service port, the page's subscription. Limit: p99 <= 20 ms. The event log
//   carries no time stamp, so the service is the spike's server (crates/rcfd-api/examples/f447_bench.rs `serve`): the
//   same Service and serve_tcp as `rcfd serve`, with `data.*` on and an in-process long-poll that writes `seq kind
//   unix_ns` of every event to a file. The renderer's clock is joined to the service's by 200 `data.info` round trips
//   through the Studio (the one with the smallest round trip; its error is at most half of it), before and after the
//   sample. Sample: the first 300 iteration events or what 120 s give.
//
//   npm run build && node scripts/package-dir.mjs
//   node scripts/measure-service.mjs --bench-server=<path of f447_bench(.exe)> [--out=<file.json>] [--smoke]
//
// f447_bench is built by `cargo build --release -p rcfd-api --example f447_bench`. A heavy record (house rule 7): run by
// hand, once, and recorded in ADR 0801 addendum 2. Not part of npm test. --smoke runs a small version to check the script.
import { _electron as electron } from '@playwright/test';
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3) ?? '';
const smoke = process.argv.includes('--smoke');
const benchServer = arg('bench-server');
if (!benchServer) {
  console.error('usage: node scripts/measure-service.mjs --bench-server=<f447_bench executable> [--out=<file.json>] [--smoke]');
  process.exit(2);
}
const Q = smoke ? { calls: 20, warm: 5, box: [10, 10, 10], events: 8, joins: 20, maxS: 30 } : { calls: 1000, warm: 50, box: [50, 50, 40], events: 300, joins: 200, maxS: 120 };

function boxToml(n) {
  const walls = ['xmin', 'xmax', 'ymin', 'zmin', 'zmax'].map((p) => `[boundaries.${p}]\nvelocity = "fixed"\n`).join('');
  return `title = "F-803 lid-driven box"
[mesh]
generator = "box_3d"
nx = ${n[0]}
ny = ${n[1]}
nz = ${n[2]}
lx = 1.0
ly = 1.0
lz = 1.0
[physics]
nu = 0.01
${walls}[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 1000000
tolerance_u = 1e-14
tolerance_p = 1e-14
`;
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

const scratch = mkdtempSync(path.join(os.tmpdir(), 'rcfd-p1p2-'));
const loadBefore = await load();
console.log(`P1/P2 through the product shell${smoke ? ' (smoke)' : ''}; load ${loadBefore.cpuBusyFraction.toFixed(2)}, ${loadBefore.cargoRustcLinkProcesses} cargo/rustc/link`);

// the spike's server, confined to the scratch folder; it prints {rpc, data, events, root} and ends when stdin closes
const server = spawn(benchServer, ['serve', scratch], { stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
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
  env: { ...env, RCFD_STUDIO_SERVICE: `127.0.0.1:${ports.rpc}`, RCFD_STUDIO_BENCH: '1', RCFD_STUDIO_STATE_DIR: path.join(scratch, 'state') },
});
let result;
try {
  const page = await app.firstWindow();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
  await page.waitForFunction(async () => (await window.studioService.state()).status === 'connected', null, { timeout: 15_000 });
  const versions = await page.evaluate(() => window.studio.app.info());
  const service = await page.evaluate(() => window.studioService.state());

  // ---- P1 in the main process, through the Studio's ServiceClient
  const p1main = await app.evaluate(async (_electron, q) => {
    const svc = globalThis.rcfdStudioBench.service;
    const { session } = await svc.call('session.create', { toml: q.toml }, 120_000);
    await svc.call('run.initialize', { session }, 600_000);
    const out = {};
    for (const [method, params] of [
      ['service.info', {}],
      ['run.status', { session }],
    ]) {
      for (let i = 0; i < q.warm; i++) await svc.call(method, params);
      const ms = [];
      for (let i = 0; i < q.calls; i++) {
        const t = performance.now();
        await svc.call(method, params);
        ms.push(performance.now() - t);
      }
      out[method] = ms;
    }
    return { session, ms: out };
  }, { toml: boxToml(Q.box), warm: Q.warm, calls: Q.calls });
  const session = p1main.session;

  // ---- P1 from the renderer (recorded): the same calls through window.studioService
  const p1renderer = await page.evaluate(async (q) => {
    const out = {};
    for (const [method, params] of [
      ['service.info', {}],
      ['run.status', { session: q.session }],
    ]) {
      for (let i = 0; i < q.warm; i++) await window.studioService.call(method, params);
      const ms = [];
      for (let i = 0; i < q.calls; i++) {
        const t = performance.now();
        const r = await window.studioService.call(method, params);
        ms.push(performance.now() - t);
        if (!r.ok) throw new Error(r.error.message);
      }
      out[method] = ms;
    }
    return out;
  }, { session, warm: Q.warm, calls: Q.calls });

  // ---- P2 in the renderer
  const p2 = await page.evaluate(async (q) => {
    const join = async () => {
      let best = { rtt: Infinity, offset: 0 };
      for (let i = 0; i < q.joins; i++) {
        const t0 = performance.now();
        const r = await window.studioService.call('data.info');
        const t1 = performance.now();
        if (!r.ok) throw new Error(`data.info: ${r.error.message}`);
        if (t1 - t0 < best.rtt) best = { rtt: t1 - t0, offset: r.result.unix_ns / 1e6 - (t0 + t1) / 2 };
      }
      return best;
    };
    const before = await join();
    const received = [];
    let missed = 0;
    const sub = await window.studioService.subscribe(
      (b) => {
        const t = performance.now();
        missed += b.missed;
        for (const e of b.events) if (e.kind === 'iteration') received.push([e.seq, t]);
      },
      { session: q.session },
    );
    if (!sub.ok) throw new Error(sub.error.message);
    const start = await window.studioService.call('run.start', { session: q.session, max_steps: 1_000_000 });
    if (!start.ok) throw new Error(`run.start: ${start.error.message}`);
    const t0 = performance.now();
    while (received.length < q.events && performance.now() - t0 < q.maxS * 1000) await new Promise((r) => setTimeout(r, 50));
    const seconds = (performance.now() - t0) / 1000;
    await window.studioService.call('run.stop', { session: q.session });
    await window.studioService.unsubscribe(sub.result);
    const after = await join();
    return { before, after, received: received.slice(0, q.events), seconds, missed };
  }, { session, events: Q.events, joins: Q.joins, maxS: Q.maxS });

  // the server's stamps: seq kind unix_ns
  await new Promise((r) => setTimeout(r, 1500)); // the server's observer flushes once per poll
  const stamps = new Map();
  for (const line of readFileSync(ports.events, 'utf8').split('\n')) {
    const p = line.trim().split(' ');
    if (p.length !== 3) continue;
    stamps.set(Number(p[0]), Number(BigInt(p[2]) / 1000n) / 1000);
  }
  const lat = [];
  let noStamp = 0;
  for (const [seq, t] of p2.received) {
    const s = stamps.get(seq);
    if (s === undefined) noStamp++;
    else lat.push(t + p2.before.offset - s);
  }

  const P1 = {
    limit: { p50: 0.5, p99: 2 },
    main: { 'service.info': summary(p1main.ms['service.info']), 'run.status': summary(p1main.ms['run.status']) },
    renderer: { 'service.info': summary(p1renderer['service.info']), 'run.status': summary(p1renderer['run.status']) },
  };
  P1.met = Object.values(P1.main).every((s) => s.p50 <= 0.5 && s.p99 <= 2);
  const P2 = {
    limit: { p99: 20 },
    latencyMs: lat.length ? summary(lat) : null,
    events: p2.received.length,
    seconds: p2.seconds,
    missed: p2.missed,
    noStamp,
    clock: { before: p2.before, after: p2.after, driftMs: p2.after.offset - p2.before.offset },
  };
  P2.met = P2.latencyMs !== null && P2.events >= (smoke ? 1 : 50) && P2.latencyMs.p99 <= 20;
  result = {
    checks: 'P1, P2 (ADR 0350) through the product shell (F-803)',
    smoke,
    machine: { cpu: os.cpus()[0]?.model, logical: os.cpus().length, totalMemGB: os.totalmem() / 2 ** 30, platform: `${os.platform()} ${os.release()}` },
    launcherNode: process.version,
    versions,
    service,
    box: Q.box,
    P1,
    P2,
    loadBefore,
  };
} finally {
  await app.close();
  server.stdin.end();
  await new Promise((r) => (server.exitCode !== null ? r() : server.once('exit', r)));
}
result.loadAfter = await load();
rmSync(scratch, { recursive: true, force: true });

const f = (s) => `p50 ${s.p50.toFixed(3)}, p99 ${s.p99.toFixed(3)}, max ${s.max.toFixed(3)} ms (n ${s.n})`;
console.log(`P1 main     service.info ${f(result.P1.main['service.info'])}`);
console.log(`P1 main     run.status   ${f(result.P1.main['run.status'])}`);
console.log(`P1 renderer service.info ${f(result.P1.renderer['service.info'])}`);
console.log(`P1 renderer run.status   ${f(result.P1.renderer['run.status'])}`);
console.log(`P1 (p50 <= 0.5, p99 <= 2 ms, main): ${result.P1.met ? 'met' : 'NOT met'}`);
if (result.P2.latencyMs) console.log(`P2 ${f(result.P2.latencyMs)}, min ${result.P2.latencyMs.min.toFixed(3)}; ${result.P2.events} events in ${result.P2.seconds.toFixed(1)} s`);
console.log(`P2 clock: rtt ${result.P2.clock.before.rtt.toFixed(3)} / ${result.P2.clock.after.rtt.toFixed(3)} ms, drift ${result.P2.clock.driftMs.toFixed(3)} ms; missed ${result.P2.missed}, no stamp ${result.P2.noStamp}`);
console.log(`P2 (p99 <= 20 ms): ${result.P2.met ? 'met' : 'NOT met'}`);
console.log(`load after ${result.loadAfter.cpuBusyFraction.toFixed(2)}, ${result.loadAfter.cargoRustcLinkProcesses} cargo/rustc/link`);
const out = arg('out');
if (out) writeFileSync(out, JSON.stringify(result, null, 1) + '\n');
