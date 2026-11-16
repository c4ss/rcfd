// P5 and P6 of ADR 0350 measured through the run workspace (F-810, docs/studio-guardrails.md G9): scrubbing 20
// consecutive snapshots of the 1e6-cell box (P5: at least 10 snapshots per second, no frame over 150 ms; 10 passes) and of
// the 4e6-cell box (P6: at least 3 per second, recorded, not blocking; 5 passes), in the packaged Studio, through the
// whole product path: a key on the scrubber (the slider of src/renderer/workspace/TimePanel.tsx) -> the workspace's stop
// -> the field viewport's read of the snapshot through the data door -> the GPU write -> the draw -> the GPU done -> the
// next animation frame (the viewport's onDrawn, performance mark `rcfd:scrub:drawn`). As ADR 0350: one warm-up pass, then
// each pass requests snapshot 1 (Home) and the 19 after it (ArrowRight), the next request as soon as the one before is
// drawn; rate = requests / total time, frame = request to next request. The residual cursor, the monitor readout and the
// counter move in the same render.
//
// Also recorded (not a limit of ADR 0350): the live latency of Follow live, from a snapshot's commit to the store of a
// running run (seen by polling store.rts with the Studio's reader) to that snapshot drawn in the workspace, on a copy of
// example-projects/quick-start with a run that does not converge (a snapshot every 5 iterations).
//
//   npm run build && node scripts/package-dir.mjs
//   f447_bench gen <folder> tiny 1e6 4e6        (crates/rcfd-api/examples/f447_bench.rs; once, by hand; 4e6 takes ~20 min)
//   npm run bench:scrub -- --data=<folder> [--out=<file.json>] [--smoke] [--no-live]
//
// The script adds what the engine's run list needs beside the generated stores (rcfd-project.json; for each box a folder
// named as a run id, runs/r-20261116T000000-<size>: runs/box-<size>-raw moved there and linked back (a junction), with
// run.json, status.json and a residuals.jsonl of one line per stored step), so that <folder>/project is a project the
// Studio opens. rcfd-mcp and rcfd-runner are found as the tests find them (RCFD_MCP_BIN, CARGO_TARGET_DIR, target/release).
// A heavy record (house rule 7): run by hand, once, and recorded in ADR 0801, addendum F-810. Not part of npm test.
// --smoke uses the 1e5 box for P5 and the tiny box for P6, one pass each, to check the script.
import { _electron as electron } from '@playwright/test';
import { execFileSync, spawnSync } from 'node:child_process';
import { cpSync, existsSync, lstatSync, mkdtempSync, readFileSync, renameSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Store } from '../src/main/data/tsstore.ts';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repo = path.resolve(root, '..', '..');
const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3) ?? '';
const smoke = process.argv.includes('--smoke');
const noLive = process.argv.includes('--no-live');
const data = arg('data') ? path.resolve(arg('data')) : '';
if (!data) {
  console.error('usage: node --experimental-strip-types scripts/measure-scrub.mjs --data=<folder of f447_bench gen> [--out=<file.json>] [--smoke] [--no-live]');
  process.exit(2);
}
const exe = (n) => (process.platform === 'win32' ? `${n}.exe` : n);
function findBin(name, envName) {
  const fromEnv = process.env[envName];
  if (fromEnv && existsSync(fromEnv)) return fromEnv;
  for (const dir of [process.env.CARGO_TARGET_DIR && path.join(process.env.CARGO_TARGET_DIR, 'release'), path.join(repo, 'target', 'release')]) {
    if (dir && existsSync(path.join(dir, exe(name)))) return path.join(dir, exe(name));
  }
  return null;
}
const mcp = findBin('rcfd-mcp', 'RCFD_MCP_BIN');
const runnerExe = process.env.RCFD_RUNNER_BIN ?? (mcp ? path.join(path.dirname(mcp), exe('rcfd-runner')) : null);
if (!mcp) {
  console.error('rcfd-mcp not found: cargo build --release -p rcfd-mcp, or set RCFD_MCP_BIN');
  process.exit(2);
}
const CHECKS = smoke
  ? [
      { name: 'P5', size: '1e5', passes: 1, rate: 10, frameMs: 150, blocking: true },
      { name: 'P6', size: 'tiny', passes: 1, rate: 3, frameMs: null, blocking: false },
    ]
  : [
      { name: 'P5', size: '1e6', passes: 10, rate: 10, frameMs: 150, blocking: true },
      { name: 'P6', size: '4e6', passes: 5, rate: 3, frameMs: null, blocking: false },
    ];
const project = path.join(data, 'project');
// the engine lists only folders named as run ids (r-<date>T<time>-<hex>), and not through a link: each generated box is
// moved to such a name (a rename), and its old name left as a link to it for scripts/measure-field.mjs
const RUN_IDS = { tiny: 'r-20261116T000000-0001', '1e5': 'r-20261116T000000-1e05', '1e6': 'r-20261116T000000-1e06', '4e6': 'r-20261116T000000-4e06' };
const runId = (size) => RUN_IDS[size];
const boxDir = (size) => path.join(project, 'runs', `box-${size}-raw`);
for (const c of CHECKS) {
  if (!existsSync(path.join(boxDir(c.size), 'fields', 'store.rts'))) {
    console.error(`no ${c.size} raw store in ${data}: run f447_bench gen ${data} tiny ${c.size}`);
    process.exit(2);
  }
}

// ---- the bench project: what the run list needs beside the stores
const NOW = new Date().toISOString();
if (!existsSync(path.join(project, 'rcfd-project.json'))) {
  writeFileSync(path.join(project, 'rcfd-project.json'), JSON.stringify({ schema_version: 1, name: 'Scrub bench (f447_bench boxes)', created: NOW, solver_version_created: 'rcfd 0.1.0' }, null, 2) + '\n');
}
for (const c of CHECKS) {
  const dir = path.join(project, 'runs', runId(c.size));
  const isLink = (p) => {
    try {
      return lstatSync(p).isSymbolicLink();
    } catch {
      return false;
    }
  };
  if (isLink(dir)) unlinkSync(dir);
  if (!existsSync(dir)) {
    renameSync(boxDir(c.size), dir);
    symlinkSync(dir, boxDir(c.size), 'junction');
  }
  const index = JSON.parse(readFileSync(path.join(dir, 'fields', 'index.json'), 'utf8'));
  const steps = index.snapshots.map((s) => s.step);
  const last = steps[steps.length - 1];
  writeFileSync(path.join(dir, 'run.json'), JSON.stringify({ id: runId(c.size), case: `cases/box-${c.size}.toml`, case_snapshot_hash: 'fnv64:0000000000000000', solver_version: 'rcfd 0.1.0', settings: { mem_mb: null, priority: 0, set: [], threads: 1 }, created: NOW }, null, 2) + '\n');
  writeFileSync(path.join(dir, 'status.json'), JSON.stringify({ state: 'finished', step: last, time: 0, pid: null, updated: NOW, reason: 'step limit (f447_bench gen)', exit_code: 0 }, null, 2) + '\n');
  // a residual and a monitor at every stored step, so the cursor and the readout move with the field
  writeFileSync(path.join(dir, 'residuals.jsonl'), steps.map((s) => JSON.stringify({ monitors: { probe: s / 1000 }, residuals: { continuity: 1 / s, p: 2 / s, u: 3 / s }, step: s, time: 0 })).join('\n') + '\n');
}

// ---- machine and load (as scripts/measure-field.mjs)
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
const pct = (s, p) => s[Math.max(0, Math.ceil(p * s.length) - 1)];
const summary = (xs) => {
  const s = sorted(xs);
  return { n: s.length, p50: pct(s, 0.5), p99: pct(s, 0.99), max: s[s.length - 1], min: s[0] };
};

const { executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8'));
const baseEnv = {};
for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_')) baseEnv[k] = v;
const OCCLUSION = '--disable-features=CalculateNativeWinOcclusion';
const state = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f810-bench-'));

async function openProject(folder) {
  const app = await electron.launch({
    executablePath: executable,
    args: [OCCLUSION],
    env: { ...baseEnv, RCFD_STUDIO_MCP: mcp, RCFD_STUDIO_RECENT: path.join(state, 'recent.json'), RCFD_STUDIO_STATE_DIR: state },
  });
  const page = await app.firstWindow();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
  await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].setContentSize(1440, 900));
  await app.evaluate(({ dialog }, f) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [f] });
  }, folder);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await page.getByRole('tab', { name: 'Workspace' }).click({ timeout: 30_000 });
  await page.getByRole('listbox', { name: 'Runs of the project' }).waitFor({ timeout: 30_000 });
  // every rcfd:scrub:drawn mark resolves the waiter of its snapshot, at once (no observer delay)
  await page.evaluate(() => {
    const mark = performance.mark.bind(performance);
    window.__scrub = { waiters: new Map(), drawn: [] };
    performance.mark = (name, opts) => {
      const m = mark(name, opts);
      if (name === 'rcfd:scrub:drawn') {
        window.__scrub.drawn.push({ at: performance.timeOrigin + m.startTime, snapshot: m.detail?.snapshot, run: m.detail?.run });
        const w = window.__scrub.waiters.get(m.detail?.snapshot);
        if (w) {
          window.__scrub.waiters.delete(m.detail?.snapshot);
          w(m.startTime);
        }
      }
      return m;
    };
  });
  return { app, page };
}

async function selectRun(page, id) {
  await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${id}"]`).click({ timeout: 30_000 });
  await page.waitForFunction((r) => document.querySelector('.ws')?.getAttribute('data-run') === r, id);
  // this run's newest snapshot drawn (its own mark, not the previous run's picture): the store open, the surface built
  await page.waitForFunction(
    (r) => {
      const ws = document.querySelector('.ws');
      return window.__scrub.drawn.some((d) => d.run === r && String(d.snapshot) === ws?.getAttribute('data-snapshot'));
    },
    id,
    { timeout: 300_000 },
  );
  // and its residuals read (the cursor names a step), so the timed passes start on a workspace that has everything
  await page.waitForFunction(() => document.querySelector('.tp-resid-values li[data-name="continuity"][data-step]') !== null, null, { timeout: 60_000 });
}

/** One pass: Home, then ArrowRight 19 times, each as soon as the one before is drawn. Times in ms. */
function pass(page, n) {
  return page.evaluate(async (count) => {
    const slider = document.querySelector('.tp-scrubber[role=slider]');
    slider.focus();
    const frames = [];
    const errors = [];
    const t0 = performance.now();
    for (let i = 0; i < count; i++) {
      const want = i;
      const done = new Promise((res) => window.__scrub.waiters.set(want, res));
      const timeout = new Promise((res) => setTimeout(() => res(null), 30_000));
      const start = performance.now();
      slider.dispatchEvent(new KeyboardEvent('keydown', { key: i === 0 ? 'Home' : 'ArrowRight', bubbles: true, cancelable: true }));
      const at = await Promise.race([done, timeout]);
      if (at === null) errors.push(`snapshot ${want} was not drawn in 30 s`);
      frames.push(performance.now() - start);
      // the same stop everywhere: the slider, the counter and the residual cursor name the drawn snapshot's step
      const step = document.querySelector('.ws')?.getAttribute('data-step');
      const cur = document.querySelector('.tp-resid-values li[data-name="continuity"]')?.getAttribute('data-step');
      if (document.querySelector('.ws')?.getAttribute('data-snapshot') !== String(want) || cur !== step) errors.push(`snapshot ${want}: workspace ${document.querySelector('.ws')?.getAttribute('data-snapshot')}, cursor step ${cur} vs ${step}`);
    }
    return { totalMs: performance.now() - t0, frames, errors };
  }, n);
}

const loadBefore = await load();
console.log(`P5, P6 through the run workspace${smoke ? ' (smoke)' : ''}; load ${loadBefore.cpuBusyFraction.toFixed(2)}, ${loadBefore.cargoRustcLinkProcesses} cargo/rustc/link`);
const result = { checks: 'P5, P6 (ADR 0350) through the run workspace (F-810)', smoke, loadBefore, machine: { cpu: os.cpus()[0]?.model, logical: os.cpus().length, totalMemGB: os.totalmem() / 2 ** 30, platform: `${os.platform()} ${os.release()}` }, launcherNode: process.version };
try {
  const { app, page } = await openProject(project);
  try {
    result.versions = await page.evaluate(() => window.studio.app.info());
    result.gpu = await page.evaluate(async () => {
      const a = await navigator.gpu?.requestAdapter();
      const i = a?.info ?? {};
      return { vendor: i.vendor, architecture: i.architecture, description: i.description };
    });
    result.window = await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().map((w) => w.getContentBounds()));
    result.scrub = [];
    for (const c of CHECKS) {
      await selectRun(page, runId(c.size));
      result.backend = await page.evaluate(() => document.querySelector('.fv-backend')?.textContent ?? null);
      const n = await page.evaluate(() => Number(document.querySelector('.tp-scrubber')?.getAttribute('aria-valuemax')) + 1);
      if (n !== 20) throw new Error(`${c.size}: ${n} snapshots, not 20`);
      const warm = await pass(page, n);
      const passes = [];
      for (let p = 0; p < c.passes; p++) passes.push(await pass(page, n));
      const frames = passes.flatMap((p) => p.frames);
      const total = passes.reduce((a, p) => a + p.totalMs, 0);
      const rate = (1000 * frames.length) / total;
      const s = summary(frames);
      const errors = [...warm.errors, ...passes.flatMap((p) => p.errors)];
      const met = rate >= c.rate && (c.frameMs === null || s.max <= c.frameMs) && errors.length === 0;
      const line = { check: c.name, size: c.size, passes: c.passes, requests: frames.length, snapshotsPerS: rate, frameMs: s, warmUpMs: warm.totalMs, passTotalsMs: passes.map((p) => p.totalMs), framesByPass: passes.map((p) => p.frames.map((f) => Math.round(f * 10) / 10)), errors, met, blocking: c.blocking };
      result.scrub.push(line);
      console.log(
        `${c.name} ${c.size}: ${rate.toFixed(1)} snapshots per s over ${frames.length} requests (${c.passes} passes), frame p50 ${s.p50.toFixed(1)}, p99 ${s.p99.toFixed(1)}, max ${s.max.toFixed(1)} ms; warm-up pass ${warm.totalMs.toFixed(0)} ms; ${met ? 'met' : 'NOT met'} (>= ${c.rate} per s${c.frameMs ? `, no frame > ${c.frameMs} ms` : ''}${c.blocking ? '' : '; recorded, not blocking'})${errors.length ? `; ${errors.length} errors: ${errors.slice(0, 3).join('; ')}` : ''}`,
      );
    }
  } finally {
    await app.close();
  }

  // ---- live latency of Follow live (recorded)
  if (!noLive && runnerExe && existsSync(runnerExe)) {
    const quick = path.join(state, 'quick-start');
    cpSync(path.join(repo, 'example-projects', 'quick-start'), quick, { recursive: true });
    const cli = (...a) => execFileSync(runnerExe, [...a, '--project', quick], { cwd: quick, encoding: 'utf8', windowsHide: true }).trim();
    const ensure = spawnSync(runnerExe, ['ensure', '--project', quick, '--quiet'], { cwd: quick, stdio: 'ignore', windowsHide: true });
    if (ensure.status !== 0) throw new Error(`rcfd-runner ensure ended with ${ensure.status}`);
    let run = null;
    const { app, page } = await openProject(quick);
    try {
      run = cli('start', 'cases/cavity-re100.toml', '--no-runner', '--set', 'solver.max_iterations=1000000', '--set', 'solver.tolerance_u=1e-14', '--set', 'solver.tolerance_p=1e-14');
      const fields = path.join(quick, 'runs', run, 'fields');
      // the commit time of every snapshot, seen by the Studio's reader of store.rts (scanned index of a live store)
      const committed = new Map();
      let store = null;
      const poll = setInterval(() => {
        try {
          if (!store && existsSync(path.join(fields, 'store.rts'))) store = new Store(fields);
          if (!store) return;
          store.refresh();
          const now = Date.now();
          for (let i = 0; i < store.snapshots.length; i++) if (!committed.has(i)) committed.set(i, now);
        } catch {
          // the header or mesh not written yet
          store?.close();
          store = null;
        }
      }, 2);
      await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${run}"]`).click({ timeout: 60_000 });
      await page.waitForFunction(() => document.querySelector('button.tp-follow')?.getAttribute('aria-pressed') === 'true', null, { timeout: 60_000 });
      await new Promise((r) => setTimeout(r, smoke ? 5_000 : 30_000));
      clearInterval(poll);
      store?.close();
      const drawn = await page.evaluate((r) => window.__scrub.drawn.filter((d) => d.run === r), run);
      const lat = [];
      const behind = [];
      const commits = [...committed.values()];
      for (const d of drawn) {
        const c = committed.get(d.snapshot);
        if (c !== undefined && d.at >= c) lat.push(d.at - c);
        // how many newer snapshots were already in the store when this one was drawn
        behind.push(commits.filter((t) => t <= d.at).length - 1 - d.snapshot);
      }
      // between draws: the picture's age, from the commit of the newest snapshot to the draw that showed one at least as new
      const gaps = drawn.slice(1).map((d, i) => d.at - drawn[i].at);
      const shown = new Set(drawn.map((d) => d.snapshot)).size;
      const f0 = (x) => (x === undefined ? '-' : x.toFixed(0));
      result.live = {
        run,
        windowS: smoke ? 5 : 30,
        snapshotsCommitted: committed.size,
        snapshotsDrawn: shown,
        commitToDrawnMs: lat.length ? summary(lat) : null,
        snapshotsBehindAtDraw: behind.length ? summary(behind) : null,
        msBetweenDraws: gaps.length ? summary(gaps) : null,
        note: 'commit seen by polling store.rts every 2 ms with the Studio reader; drawn = the viewport done and the next frame. Follow live moves to the newest snapshot the workspace knows of; it opens the running store again 250 ms after the last open answered (and when the run list, read every second, reports a new step), and skips snapshots written while one is read and drawn',
      };
      console.log(
        `live (Follow live, recorded): ${committed.size} snapshots committed in ${result.live.windowS} s, ${shown} drawn, a draw every ${f0(result.live.msBetweenDraws?.p50)} ms (p50); commit to drawn p50 ${f0(result.live.commitToDrawnMs?.p50)}, max ${f0(result.live.commitToDrawnMs?.max)} ms; snapshots behind the store at the draw p50 ${f0(result.live.snapshotsBehindAtDraw?.p50)}, max ${f0(result.live.snapshotsBehindAtDraw?.max)}`,
      );
    } finally {
      await app.close();
      try {
        if (run) cli('cancel', run);
      } catch {
        // already over
      }
      for (let i = 0; i < 60 && run; i++) {
        const st = JSON.parse(readFileSync(path.join(quick, 'runs', run, 'status.json'), 'utf8')).state;
        if (['finished', 'failed', 'cancelled'].includes(st)) break;
        await new Promise((r) => setTimeout(r, 500));
      }
      try {
        cli('shutdown');
      } catch {
        // not running
      }
    }
  }
} finally {
  result.loadAfter = await load();
  try {
    rmSync(state, { recursive: true, force: true, maxRetries: 5 });
  } catch {
    // a process may still hold a file; the folder is in the temporary directory
  }
}
console.log(`backend ${result.backend}; GPU ${JSON.stringify(result.gpu)}; load after ${result.loadAfter.cpuBusyFraction.toFixed(2)}, ${result.loadAfter.cargoRustcLinkProcesses} cargo/rustc/link`);
const out = arg('out');
if (out) writeFileSync(out, JSON.stringify(result, null, 1) + '\n');
