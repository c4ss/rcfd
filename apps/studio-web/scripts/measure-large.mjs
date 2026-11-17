// F-819: the Studio on large meshes and on the FAN-01 fan mesh (docs/studio-guardrails.md G9; ADR 0350 §6: "a fan mesh
// with many patches and polyhedral cells may change P7"). The G9 limits of P7, P5 and P8, unchanged, applied to meshes
// larger and less regular than the 1e6 box they were set on:
//
// - first picture (P7): the field viewport of the development page (#/dev/field, route R2 local) opens the store, reads
//   its surface and field `p` of the last snapshot, prepares, draws, waits for the GPU and one frame (the measure
//   `rcfd:field:first-picture` of src/renderer/field/FieldViewport.tsx), as scripts/measure-field.mjs: the viewport is
//   open on the tiny box first and switches store between the opens, 5 opens per mesh, judged on the median.
// - scrubbing (P5): the run workspace of the packaged Studio, as scripts/measure-scrub.mjs: Home, then ArrowRight on the
//   scrubber through every snapshot of the run, each key as soon as the one before is drawn (mark `rcfd:scrub:drawn`);
//   one warm-up pass, then the timed passes; rate = requests / time, frame = request to next request.
// - draw rate while orbiting (P8): a real drag on the viewport (Playwright's mouse: trusted pointer events, so
//   OrbitControls rotates the camera and three draws on demand), held for a fixed time. A draw is an animation frame in
//   which the page submitted GPU work (GPUQueue.submit, counted by a hook of this script); the rate is draws per second,
//   the frame is the time between consecutive draws. Taken in two variants: with the hover readout as the user has it
//   (each pointer move also picks the cell under the cursor on the CPU, FieldCanvas `pickAt`), and with the pick
//   skipped (the canvas's getBoundingClientRect answers an empty rectangle for the measurement, so `pickAt` returns at
//   once and only the draw and React's update remain). Each with vertical sync (the p99 frame is judged there) and with
//   --disable-frame-rate-limit --disable-gpu-vsync (the better rate of the two is judged, as P8 of scripts/bench.mjs).
//   Also recorded: the GPU time of each submit (submit to onSubmittedWorkDone).
//
// The limits are registered in LIMITS below, in git before the first measurement (F-819; commit "F-819 limits"), and
// printed first. Every case runs --repeats times (default 3) and every run is reported; nothing is retried. The
// processor's busy fraction is taken before and after every measurement and printed beside its numbers.
//
// The meshes (none is generated inside the repository):
// - box-4e6 (and box-1e6 with --sizes=1e6,4e6): the lid-driven boxes of crates/rcfd-api/examples/f447_bench.rs (`gen`,
//   20 steps through the run path), in --data (else $RCFD_BENCH_DATA, else <temp>/rcfd-studio-bench-data), prepared as a
//   project the way scripts/measure-scrub.mjs prepares it (run-id folders and the files the run list needs).
// - fan01-F3: the FAN-01 blade passage of example-projects/axial-fan (C-grid, F3 counts, 4 920 cells, 7 patches) with
//   its committed engine run r-20261030T223519-0489 (1500 iterations, 10 snapshots).
// - fan01-F1: the same case on the finest registered member F1 of the ADR 0086 family (38 184 cells, 7 patches), run
//   for 20 iterations with a snapshot of every iteration by the engine (rcfd-runner start ... --set), in --fan (else
//   <data>/fan01, a copy of example-projects/axial-fan). The run is made once when missing (about 80 s under load; not
//   timed, not counted) unless --no-gen. F1's counts doubled do not build: the C-grid's leading-edge fan folds
//   ("mesh.fan.front_gap: the line family is folded at layer 14", the pre-launch check after 75 min, 2026-11-17), so
//   there is no larger FAN-01 mesh to measure.
//
//   npm run build && npm run package:dir
//   node scripts/measure-large.mjs [--data=<folder>] [--fan=<project folder>] [--sizes=4e6]
//        [--fans=F3,F1] [--repeats=3] [--only=first,scrub,orbit] [--seconds=6] [--no-gen] [--smoke]
//        [--profile-root=<folder>] [--out=<file.json>]
//
// --profile-root: a fresh Chromium profile per launch under it (scripts/profile.mjs; ADR 0800 §9 Q5). --smoke:
// fan01-F3 only, one repeat, 2 s drags, to check the script. Exit code 0 when every judged line is met, 1 on a
// miss, 2 when a case could not run. A heavy record (house rule 7): run by hand; `npm run bench -- --heavy` runs it once
// with --repeats=1 and prints its lines as recorded. Not part of npm test or CI (ADR 0800 §9, Q3).
import { _electron as electron } from '@playwright/test';
import { execFileSync, spawnSync } from 'node:child_process';
import { cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, openSync, readFileSync, readSync, closeSync, readdirSync, renameSync, rmSync, statSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { profileArgs, profileRecord } from './profile.mjs';

// ---- the limits, registered before any measurement (G9, ADR 0350 §3; unchanged)
const LIMITS = {
  registered: 'F-819, 2026-11-17, before the first measurement; the G9 limits of docs/studio-guardrails.md unchanged',
  firstPicture: { id: 'P7', what: 'first picture of the surface, median of 5 opens', op: '<=', limit: 1500, unit: 'ms', judged: true },
  scrubRate: { id: 'P5', what: 'scrubbing, snapshots per second', op: '>=', limit: 10, unit: 'snapshots/s', judged: true },
  scrubFrame: { id: 'P5', what: 'scrubbing, longest frame', op: '<=', limit: 150, unit: 'ms', judged: true },
  scrubRate4e6: { id: 'P6', what: 'scrubbing at 4e6 cells, snapshots per second (ADR 0350 P6, recorded)', op: '>=', limit: 3, unit: 'snapshots/s', judged: false },
  drawRate: { id: 'P8', what: 'orbit, draws per second, the better of vertical sync and no rate limit', op: '>=', limit: 60, unit: 'draws/s', judged: true },
  drawFrame: { id: 'P8', what: 'orbit, p99 time between draws, vertical sync', op: '<=', limit: 25, unit: 'ms', judged: true },
};
const limitText = (l) => `${l.id} ${l.what}: ${l.op} ${l.limit} ${l.unit}${l.judged ? '' : ' (recorded, not judged)'}`;
console.log(`F-819 limits (${LIMITS.registered}):`);
for (const [k, l] of Object.entries(LIMITS)) if (k !== 'registered') console.log(`  ${limitText(l)}`);
console.log('  P7, P5 and P8 are judged on every mesh and in every repeat, on the drag with the hover readout and on the drag with it skipped.');

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repo = path.resolve(root, '..', '..');
const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3) ?? '';
const smoke = process.argv.includes('--smoke');
const noGen = process.argv.includes('--no-gen');
const only = arg('only') ? new Set(arg('only').split(',')) : null;
const want = (what) => !only || only.has(what);
const REPEATS = smoke ? 1 : Number(arg('repeats') || 3);
const SECONDS = smoke ? 2 : Number(arg('seconds') || 6);
const WARM_S = 1;
const data = path.resolve(arg('data') || process.env.RCFD_BENCH_DATA || path.join(os.tmpdir(), 'rcfd-studio-bench-data'));
const fanProject = path.resolve(arg('fan') || path.join(data, 'fan01'));
const SIZES = smoke ? [] : (arg('sizes') || '4e6').split(',').filter(Boolean);
const FANS = smoke ? ['F3'] : (arg('fans') || 'F3,F1').split(',').filter(Boolean);

const exe = (n) => (process.platform === 'win32' ? `${n}.exe` : n);
function findBin(name, envName) {
  const fromEnv = process.env[envName];
  if (fromEnv && existsSync(fromEnv)) return fromEnv;
  for (const dir of [process.env.CARGO_TARGET_DIR && path.join(process.env.CARGO_TARGET_DIR, 'release'), path.join(repo, 'target', 'release')]) {
    if (dir && existsSync(path.join(dir, exe(name)))) return path.join(dir, exe(name));
  }
  // a worktree of the repository: the target folder of the main checkout above it
  for (let dir = repo; path.dirname(dir) !== dir; dir = path.dirname(dir)) {
    const p = path.join(dir, 'target', 'release', exe(name));
    if (existsSync(p)) return p;
  }
  return null;
}
const mcp = findBin('rcfd-mcp', 'RCFD_MCP_BIN');
const runnerExe = process.env.RCFD_RUNNER_BIN ?? (mcp ? path.join(path.dirname(mcp), exe('rcfd-runner')) : null);
if (!mcp) {
  console.error('rcfd-mcp not found: cargo build --release -p rcfd-mcp, or set RCFD_MCP_BIN');
  process.exit(2);
}

// ---- machine and load
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
    return execFileSync('tasklist', ['/fo', 'csv', '/nh'], { encoding: 'utf8', windowsHide: true, timeout: 15_000 })
      .split('\n')
      .filter((l) => /^"(cargo|rustc|link)\.exe"/i.test(l)).length;
  } catch {
    return null;
  }
}
// the processor's busy fraction over 1 s around every measurement; the cargo/rustc/link count (tasklist, which took
// minutes on the loaded machine of 2026-11-17) only at the start and the end
async function load(withBuilders = false) {
  const a = cpuTimes();
  await new Promise((r) => setTimeout(r, 1000));
  const b = cpuTimes();
  return { at: new Date().toISOString(), cpuBusyFraction: (b.busy - a.busy) / Math.max(1, b.total - a.total), cargoRustcLinkProcesses: withBuilders ? builders() : null, freeMemGB: os.freemem() / 2 ** 30 };
}
const loadText = (l) => `busy ${l.cpuBusyFraction.toFixed(2)}, ${l.cargoRustcLinkProcesses ?? 'n/a'} cargo/rustc/link, ${l.freeMemGB.toFixed(1)} GB free`;
const sorted = (xs) => [...xs].sort((a, b) => a - b);
const pct = (s, p) => s[Math.max(0, Math.ceil(p * s.length) - 1)];
const summary = (xs) => {
  if (!xs.length) return null;
  const s = sorted(xs);
  return { n: s.length, p50: pct(s, 0.5), p99: pct(s, 0.99), max: s[s.length - 1], min: s[0] };
};
const f1 = (v) => (v === null || v === undefined || Number.isNaN(v) ? 'n/a' : Math.abs(v) >= 100 ? v.toFixed(0) : v.toFixed(1));

// ---- the stored meshes: sizes, faces, patches, faces per cell (read from the MESH frame of store.rts, as
// src/main/data/tsstore.ts lays it out; not timed)
function meshInfo(fields) {
  const fd = openSync(path.join(fields, 'store.rts'), 'r');
  try {
    const rd = (off, n) => {
      const b = Buffer.alloc(n);
      readSync(fd, b, 0, n, off);
      return b;
    };
    const base = 20 + 28;
    const h = rd(base, 32);
    const nCells = Number(h.readBigUInt64LE(8));
    const nPoints = Number(h.readBigUInt64LE(16));
    const jl = Number(h.readBigUInt64LE(24));
    const meta = JSON.parse(rd(base + 32, jl).toString('utf8'));
    const nFaces = meta.n_faces;
    const nNb = meta.n_neighbour;
    const pointsAt = base + 32 + jl;
    const offsetsAt = pointsAt + 24 * nPoints;
    const fpAt = offsetsAt + 8 * (nFaces + 1);
    const ownerAt = fpAt + 8 * meta.n_face_points;
    const word = (b, i) => b.readUInt32LE(8 * i); // low words: every count is below 2^32 (the reader checks it)
    const facesOfCell = new Uint8Array(nCells);
    const pointsPerFace = {};
    let boundaryTriangles = 0;
    const CH = 1 << 20;
    for (let f0 = 0; f0 < nFaces; f0 += CH) {
      const n = Math.min(CH, nFaces - f0);
      const offs = rd(offsetsAt + 8 * f0, 8 * (n + 1));
      const own = rd(ownerAt + 8 * f0, 8 * n);
      const nbN = Math.max(0, Math.min(n, nNb - f0));
      const nb = nbN ? rd(ownerAt + 8 * nFaces + 8 * f0, 8 * nbN) : null;
      for (let i = 0; i < n; i++) {
        const k = word(offs, i + 1) - word(offs, i);
        pointsPerFace[k] = (pointsPerFace[k] ?? 0) + 1;
        if (f0 + i >= nNb) boundaryTriangles += Math.max(0, k - 2);
        const o = word(own, i);
        if (facesOfCell[o] < 255) facesOfCell[o]++;
        if (nb && i < nbN) {
          const c = word(nb, i);
          if (facesOfCell[c] < 255) facesOfCell[c]++;
        }
      }
    }
    const facesPerCell = {};
    for (const c of facesOfCell) facesPerCell[c] = (facesPerCell[c] ?? 0) + 1;
    return {
      cells: nCells,
      points: nPoints,
      faces: nFaces,
      internalFaces: nNb,
      boundaryFaces: nFaces - nNb,
      boundaryTriangles,
      patches: (meta.patches ?? []).map((p) => ({ name: p.name, kind: p.kind, faces: p.size })),
      facesPerCell,
      pointsPerFace,
      storeBytes: statSync(path.join(fields, 'store.rts')).size,
    };
  } finally {
    closeSync(fd);
  }
}

// ---- the box project (as scripts/measure-scrub.mjs prepares it; the folders are made only when missing)
const RUN_IDS = { tiny: 'r-20261116T000000-0001', '1e5': 'r-20261116T000000-1e05', '1e6': 'r-20261116T000000-1e06', '4e6': 'r-20261116T000000-4e06' };
const boxProject = path.join(data, 'project');
const isLink = (p) => {
  try {
    return lstatSync(p).isSymbolicLink();
  } catch {
    return false;
  }
};
function prepareBox(size) {
  const dir = path.join(boxProject, 'runs', RUN_IDS[size]);
  const old = path.join(boxProject, 'runs', `box-${size}-raw`);
  if (!existsSync(path.join(dir, 'fields', 'store.rts'))) {
    if (!existsSync(path.join(old, 'fields', 'store.rts'))) {
      console.error(`no ${size} raw store in ${data}: run f447_bench gen ${data} tiny ${size}`);
      process.exit(2);
    }
    if (isLink(dir)) unlinkSync(dir);
    renameSync(old, dir);
    symlinkSync(dir, old, 'junction');
  }
  const NOW = new Date().toISOString();
  if (!existsSync(path.join(boxProject, 'rcfd-project.json'))) {
    writeFileSync(path.join(boxProject, 'rcfd-project.json'), JSON.stringify({ schema_version: 1, name: 'Scrub bench (f447_bench boxes)', created: NOW, solver_version_created: 'rcfd 0.1.0' }, null, 2) + '\n');
  }
  const steps = JSON.parse(readFileSync(path.join(dir, 'fields', 'index.json'), 'utf8')).snapshots.map((s) => s.step);
  const last = steps[steps.length - 1];
  if (!existsSync(path.join(dir, 'run.json'))) {
    writeFileSync(path.join(dir, 'run.json'), JSON.stringify({ id: RUN_IDS[size], case: `cases/box-${size}.toml`, case_snapshot_hash: 'fnv64:0000000000000000', solver_version: 'rcfd 0.1.0', settings: { mem_mb: null, priority: 0, set: [], threads: 1 }, created: NOW }, null, 2) + '\n');
  }
  if (!existsSync(path.join(dir, 'status.json'))) {
    writeFileSync(path.join(dir, 'status.json'), JSON.stringify({ state: 'finished', step: last, time: 0, pid: null, updated: NOW, reason: 'step limit (f447_bench gen)', exit_code: 0 }, null, 2) + '\n');
  }
  if (!existsSync(path.join(dir, 'residuals.jsonl'))) {
    writeFileSync(path.join(dir, 'residuals.jsonl'), steps.map((s) => JSON.stringify({ monitors: { probe: s / 1000 }, residuals: { continuity: 1 / s, p: 2 / s, u: 3 / s }, step: s, time: 0 })).join('\n') + '\n');
  }
  return { name: `box-${size}`, kind: 'box', project: boxProject, run: RUN_IDS[size], fields: path.join(dir, 'fields'), source: `f447_bench gen ${size} (crates/rcfd-api/examples/f447_bench.rs: lid-driven box, 20 steps through the run path)` };
}

// ---- the FAN-01 project: a copy of example-projects/axial-fan, its committed F3 run, and the engine runs made here
// counts: n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap (crates/rcfd-api/tests/fan01_stall_w43c.rs F1..F3)
const FAN_SPECS = {
  F3: { counts: null, run: 'r-20261030T223519-0489', note: 'F3 counts (6, 12, 6, 8, 3, 10, 5), the committed run of example-projects/axial-fan (1500 iterations, a snapshot every 150)' },
  F1: { counts: [12, 24, 12, 15, 6, 21, 10], note: 'F1 counts (12, 24, 12, 15, 6, 21, 10), the finest member of the ADR 0086 family; 20 iterations, a snapshot of every iteration' },
};
const COUNT_KEYS = ['n_up', 'n_chord', 'n_down', 'n_side', 'n_nose', 'n_span', 'n_gap'];
const FAN_CASE = 'cases/fan01-passage-upwind.toml';
const fanRuns = path.join(fanProject, 'f819-runs.json');
function prepareFan(name) {
  const spec = FAN_SPECS[name];
  if (!spec) throw new Error(`unknown FAN-01 mesh ${name} (${Object.keys(FAN_SPECS).join(', ')})`);
  if (!existsSync(path.join(fanProject, 'rcfd-project.json'))) {
    mkdirSync(path.dirname(fanProject), { recursive: true });
    cpSync(path.join(repo, 'example-projects', 'axial-fan'), fanProject, { recursive: true });
  }
  const known = existsSync(fanRuns) ? JSON.parse(readFileSync(fanRuns, 'utf8')) : {};
  let run = spec.run ?? known[name]?.run ?? null;
  const set = spec.counts ? [...spec.counts.map((c, i) => `mesh.fan.${COUNT_KEYS[i]}=${c}`), 'solver.max_iterations=20', 'output.store_interval=1'] : [];
  if (!run && spec.counts && existsSync(path.join(fanProject, 'runs'))) {
    // a run of these settings made before (an interrupted earlier call): adopted, and waited for when not over
    for (const id of readdirSync(path.join(fanProject, 'runs')).sort()) {
      try {
        const rj = JSON.parse(readFileSync(path.join(fanProject, 'runs', id, 'run.json'), 'utf8'));
        if (rj.case === FAN_CASE && JSON.stringify(rj.settings?.set) === JSON.stringify(set)) run = id;
      } catch {
        // not a run folder
      }
    }
    if (run) {
      const t = Date.now();
      for (;;) {
        const st = JSON.parse(readFileSync(path.join(fanProject, 'runs', run, 'status.json'), 'utf8'));
        if (['finished', 'failed', 'cancelled'].includes(st.state)) break;
        if (Date.now() - t > 7200_000) throw new Error(`the ${name} run ${run} is still ${st.state} after 2 h`);
        Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 5000);
      }
      if (!existsSync(path.join(fanProject, 'runs', run, 'fields', 'store.rts'))) throw new Error(`the ${name} run ${run} has no field store`);
      known[name] = { run, counts: spec.counts, adopted: true, made: new Date().toISOString() };
      writeFileSync(fanRuns, JSON.stringify(known, null, 1) + '\n');
    }
  }
  if (!run || !existsSync(path.join(fanProject, 'runs', run, 'fields', 'store.rts'))) {
    if (spec.run) throw new Error(`the committed FAN-01 run ${spec.run} is missing from ${fanProject}`);
    if (noGen) throw new Error(`no ${name} run in ${fanProject} (--no-gen)`);
    if (!runnerExe || !existsSync(runnerExe)) throw new Error('rcfd-runner not found (next to rcfd-mcp, or RCFD_RUNNER_BIN)');
    console.log(`making the FAN-01 ${name} run in ${fanProject} (rcfd-runner start ${FAN_CASE} ${set.map((x) => `--set ${x}`).join(' ')}; not timed)`);
    const t = Date.now();
    const cli = (...a) => execFileSync(runnerExe, [...a, '--project', fanProject], { cwd: fanProject, encoding: 'utf8', windowsHide: true }).trim();
    const ensure = spawnSync(runnerExe, ['ensure', '--project', fanProject, '--quiet'], { cwd: fanProject, stdio: 'ignore', windowsHide: true });
    if (ensure.status !== 0) throw new Error(`rcfd-runner ensure ended with ${ensure.status}`);
    try {
      run = cli('start', FAN_CASE, ...set.flatMap((x) => ['--set', x]), '--no-runner');
      const w = spawnSync(runnerExe, ['wait', run, '--timeout', '7200', '--project', fanProject], { cwd: fanProject, encoding: 'utf8', windowsHide: true });
      const status = JSON.parse(readFileSync(path.join(fanProject, 'runs', run, 'status.json'), 'utf8'));
      if (w.status !== 0 || !existsSync(path.join(fanProject, 'runs', run, 'fields', 'store.rts'))) throw new Error(`the ${name} run ${run} ended ${status.state}: ${status.reason}`);
    } finally {
      spawnSync(runnerExe, ['shutdown', '--project', fanProject], { cwd: fanProject, stdio: 'ignore', windowsHide: true });
    }
    known[name] = { run, counts: spec.counts, seconds: (Date.now() - t) / 1000, made: new Date().toISOString() };
    writeFileSync(fanRuns, JSON.stringify(known, null, 1) + '\n');
    console.log(`  ${name}: run ${run} in ${known[name].seconds.toFixed(0)} s`);
  }
  return { name: `fan01-${name}`, kind: 'fan', project: fanProject, run, fields: path.join(fanProject, 'runs', run, 'fields'), source: `FAN-01 passage (${FAN_CASE}), ${spec.note}`, made: known[name] ?? null };
}

// ---- the meshes
const tiny = path.join(data, 'project', 'runs', 'box-tiny-raw', 'fields');
if (!existsSync(path.join(tiny, 'store.rts'))) {
  console.error(`no tiny raw store in ${data}: run f447_bench gen ${data} tiny`);
  process.exit(2);
}
const meshes = [];
try {
  for (const s of SIZES) meshes.push(prepareBox(s));
  for (const f of FANS) meshes.push(prepareFan(f));
} catch (e) {
  console.error(`a mesh could not be prepared: ${e.message}`);
  process.exit(2);
}
for (const m of meshes) {
  m.index = JSON.parse(readFileSync(path.join(m.fields, 'index.json'), 'utf8'));
  m.snapshots = m.index.snapshots.length;
  m.info = meshInfo(m.fields);
  const i = m.info;
  console.log(
    `mesh ${m.name}: ${i.cells} cells, ${i.points} points, ${i.faces} faces (${i.boundaryFaces} on the boundary, drawn as ${i.boundaryTriangles} triangles), ${i.patches.length} patches (${i.patches.map((p) => `${p.name} ${p.kind} ${p.faces}`).join(', ')}); faces per cell ${JSON.stringify(i.facesPerCell)}, points per face ${JSON.stringify(i.pointsPerFace)}; ${m.snapshots} snapshots, store ${(i.storeBytes / 2 ** 20).toFixed(0)} MiB; ${m.source}${m.run ? `; run ${m.run}` : ''}`,
  );
}

const { executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8'));
const baseEnv = {};
for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_')) baseEnv[k] = v;
const OCCLUSION = '--disable-features=CalculateNativeWinOcclusion';
const UNCAPPED = ['--disable-frame-rate-limit', '--disable-gpu-vsync'];
const state = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f819-state-'));
const roots = [...new Set([data, fanProject, ...meshes.map((m) => path.dirname(path.dirname(m.fields)))])].join(path.delimiter);

async function launch(extraArgs, size) {
  const app = await electron.launch({
    executablePath: executable,
    args: [OCCLUSION, ...profileArgs(), ...extraArgs],
    env: { ...baseEnv, RCFD_STUDIO_DATA_ROOTS: roots, RCFD_STUDIO_MCP: mcp, RCFD_STUDIO_RECENT: path.join(state, 'recent.json'), RCFD_STUDIO_STATE_DIR: state },
  });
  const page = await app.firstWindow();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open');
  await app.evaluate(({ BrowserWindow }, s) => BrowserWindow.getAllWindows()[0].setContentSize(s[0], s[1]), size);
  return { app, page };
}

// ---- first picture (as scripts/measure-field.mjs)
const fieldHash = (p, snapshot) => `#/dev/field?route=local&path=${encodeURIComponent(p)}&field=p&snapshot=${snapshot}`;
async function firstPicture(page, hash) {
  await page.evaluate(() => performance.clearMeasures('rcfd:field:first-picture'));
  await page.evaluate((h) => (location.hash = h), hash);
  await page.waitForFunction(() => performance.getEntriesByName('rcfd:field:first-picture').length > 0 || document.querySelector('.fv-status[role=alert]') !== null, null, { timeout: 300_000 });
  const r = await page.evaluate(() => ({
    ms: performance.getEntriesByName('rcfd:field:first-picture')[0]?.duration ?? null,
    error: document.querySelector('.fv-status[role=alert]')?.textContent ?? null,
    backend: document.querySelector('.fv-backend')?.getAttribute('data-backend') ?? null,
  }));
  if (r.ms === null) throw new Error(`no picture: ${r.error}`);
  return r;
}
const tinyHash = fieldHash(tiny, 19);

// ---- orbit: a real drag held for SECONDS; draws = animation frames with a GPU submit
async function installHooks(page) {
  await page.evaluate(() => {
    if (window.__f819) return;
    window.__f819 = { submits: 0, gpu: [], recording: false };
    const q = globalThis.GPUQueue?.prototype;
    if (q) {
      const submit = q.submit;
      q.submit = function (...a) {
        const o = window.__f819;
        o.submits++;
        const t = performance.now();
        const r = submit.apply(this, a);
        if (o.recording && o.gpu.length < 20000) this.onSubmittedWorkDone().then(() => o.gpu.push(performance.now() - t));
        return r;
      };
    }
  });
}
async function orbit(page, { pick, seconds }) {
  const canvas = page.locator('.fv-frame canvas').first();
  const b = await canvas.boundingBox();
  if (!b) throw new Error('no canvas');
  // the press lands on the canvas itself: a point of the view not covered by what is drawn over it (the readout of the
  // drag before, the reset button), else OrbitControls never sees it
  const at = await page.evaluate(({ x, y, w, h }) => {
    const c = document.querySelector('.fv-frame canvas');
    for (const [fx, fy] of [[0.5, 0.5], [0.4, 0.6], [0.6, 0.4], [0.3, 0.3], [0.7, 0.7], [0.5, 0.25], [0.5, 0.75]]) {
      if (document.elementFromPoint(x + fx * w, y + fy * h) === c) return [x + fx * w, y + fy * h];
    }
    return null;
  }, { x: b.x, y: b.y, w: b.width, h: b.height });
  if (!at) throw new Error('no point of the view reaches the canvas');
  const [cx, cy] = at;
  // the drag starts with a trusted press (OrbitControls takes the mouse pointer and listens for its moves), then one move
  // of that pointer per animation frame, as Chromium delivers a real mouse's moves (aligned to the frames); Playwright's
  // mouse.move round trips (20 to 30 ms each under load) would make the input, not the Studio, set the rate
  await page.mouse.move(cx, cy);
  await page.mouse.down();
  const r = await page.evaluate(
    async ({ withPick, ms, x0, y0, w, h }) => {
      const o = window.__f819;
      o.frames = [];
      o.gpu = [];
      o.recording = true;
      const c = document.querySelector('.fv-frame canvas');
      // the pick skipped: FieldCanvas pickAt answers null at once for an empty rectangle (OrbitControls does not use it)
      if (!withPick) c.getBoundingClientRect = () => new DOMRect(0, 0, 0, 0);
      let moves = 0;
      const t0 = performance.now();
      await new Promise((done) => {
        const tick = (t) => {
          o.frames.push([t, o.submits]);
          if (t - t0 >= ms) return done();
          moves++;
          const x = x0 + 0.3 * w * Math.sin(moves / 60);
          const y = y0 + 0.1 * h * Math.sin(moves / 157);
          c.dispatchEvent(new PointerEvent('pointermove', { pointerId: 1, pointerType: 'mouse', isPrimary: true, buttons: 1, clientX: x, clientY: y, screenX: x, screenY: y, bubbles: true, cancelable: true, composed: true }));
          requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      });
      o.recording = false;
      delete c.getBoundingClientRect;
      return { frames: o.frames, gpu: o.gpu, webgpu: !!globalThis.GPUQueue, moves };
    },
    { withPick: pick, ms: (seconds + WARM_S) * 1000, x0: cx, y0: cy, w: b.width, h: b.height },
  );
  await page.mouse.up();
  const moves = r.moves;
  // after the warm-up second: the frames whose interval had a submit
  const start = r.frames[0][0] + WARM_S * 1000;
  const draws = [];
  const allFrames = [];
  for (let k = 1; k < r.frames.length; k++) {
    const [t, s] = r.frames[k];
    if (t < start) continue;
    allFrames.push(t - r.frames[k - 1][0]);
    if (s > r.frames[k - 1][1]) draws.push(t);
  }
  if (draws.length < 2) throw new Error(`the drag drew ${draws.length} frames in ${r.frames.length} animation frames and ${r.moves} moves (pressed at ${cx.toFixed(0)}, ${cy.toFixed(0)}): the orbit did not move the view`);
  const between = draws.slice(1).map((t, k) => t - draws[k]);
  const span = draws.length > 1 ? draws[draws.length - 1] - draws[0] : 0;
  return {
    pick,
    seconds,
    moves,
    movesPerS: moves / (seconds + WARM_S),
    draws: draws.length,
    drawsPerS: span > 0 ? (1000 * (draws.length - 1)) / span : 0,
    betweenDrawsMs: summary(between),
    animationFrameMs: summary(allFrames),
    gpuMs: summary(r.gpu),
    webgpu: r.webgpu,
  };
}

// ---- scrub (as scripts/measure-scrub.mjs)
async function openProject(folder) {
  const { app, page } = await launch([], [1440, 900]);
  await app.evaluate(({ dialog }, f) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [f] });
  }, folder);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await page.getByRole('tab', { name: 'Workspace' }).click({ timeout: 60_000 });
  await page.getByRole('listbox', { name: 'Runs of the project' }).waitFor({ timeout: 60_000 });
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
  await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${id}"]`).click({ timeout: 60_000 });
  await page.waitForFunction((r) => document.querySelector('.ws')?.getAttribute('data-run') === r, id);
  await page.waitForFunction(
    (r) => {
      const ws = document.querySelector('.ws');
      return window.__scrub.drawn.some((d) => d.run === r && String(d.snapshot) === ws?.getAttribute('data-snapshot'));
    },
    id,
    { timeout: 600_000 },
  );
  await page.waitForFunction(() => document.querySelector('.tp-resid-values li[data-name="continuity"][data-step]') !== null, null, { timeout: 120_000 });
}
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
      const timeout = new Promise((res) => setTimeout(() => res(null), 60_000));
      const start = performance.now();
      slider.dispatchEvent(new KeyboardEvent('keydown', { key: i === 0 ? 'Home' : 'ArrowRight', bubbles: true, cancelable: true }));
      const at = await Promise.race([done, timeout]);
      if (at === null) errors.push(`snapshot ${want} was not drawn in 60 s`);
      frames.push(performance.now() - start);
      if (document.querySelector('.ws')?.getAttribute('data-snapshot') !== String(want)) errors.push(`snapshot ${want}: workspace shows ${document.querySelector('.ws')?.getAttribute('data-snapshot')}`);
    }
    return { totalMs: performance.now() - t0, frames, errors };
  }, n);
}

// ---- the lines
const lines = [];
function line(l, mesh, repeat, value, { note = '', load: ld = null } = {}) {
  const met = value !== null && value !== undefined && !Number.isNaN(value) && (l.op === '<=' ? value <= l.limit : value >= l.limit);
  const entry = { id: l.id, what: `${mesh}, ${l.what}, run ${repeat}`, mesh, repeat, value, unit: l.unit, op: l.op, limit: l.limit, met, judged: l.judged, note, load: ld };
  lines.push(entry);
  console.log(text(entry));
}
const text = (l) =>
  `${l.id} ${l.what} ${f1(l.value)} ${l.unit} limit ${l.op}${l.limit} ${l.met ? 'met' : 'MISSED'}${l.judged ? '' : ' (recorded)'}${l.note ? `; ${l.note}` : ''}${l.load ? `; LOAD ${l.load}` : ''}`;
const ldPair = (a, b) => `processor busy ${a.cpuBusyFraction.toFixed(2)} before, ${b.cpuBusyFraction.toFixed(2)} after`;

const loadBefore = await load(true);
console.log(`F-819 measurements, ${REPEATS} repeat${REPEATS === 1 ? '' : 's'}, drags of ${SECONDS} s after ${WARM_S} s warm-up${smoke ? ' (smoke)' : ''}; load ${loadText(loadBefore)}`);
console.log(`window: ${(process.env.RCFD_TEST_HEADLESS ?? '') === '1' ? 'test mode RCFD_TEST_HEADLESS=1' : 'normal window'}; profile: ${profileRecord().mode}${profileRecord().root ? ` under ${profileRecord().root}` : ''}`);
const result = {
  item: 'F-819',
  limits: LIMITS,
  started: new Date().toISOString(),
  smoke,
  repeats: REPEATS,
  dragSeconds: SECONDS,
  warmUpSeconds: WARM_S,
  machine: { cpu: os.cpus()[0]?.model, logical: os.cpus().length, totalMemGB: os.totalmem() / 2 ** 30, platform: `${os.platform()} ${os.release()}` },
  launcherNode: process.version,
  window: process.env.RCFD_TEST_HEADLESS ?? '',
  meshes: meshes.map(({ index, ...m }) => m),
  loadBefore,
  runs: [],
};
let failed = null;
try {
  for (let rep = 1; rep <= REPEATS; rep++) {
    // first picture and the vertically synced drags, in one launch on the development page
    if (want('first') || want('orbit')) {
      for (const uncapped of want('orbit') ? [false, true] : [false]) {
        if (uncapped && !want('orbit')) continue;
        console.log(`-- run ${rep}: the development page, ${uncapped ? 'no rate limit' : 'vertical sync'}`);
        const { app, page } = await launch(uncapped ? UNCAPPED : [], [1280, 800]);
        try {
          const versions = await page.evaluate(() => window.studio.app.info());
          await installHooks(page);
          const warm = await firstPicture(page, tinyHash);
          result.runs.push({ repeat: rep, launch: uncapped ? 'dev page, no rate limit' : 'dev page, vertical sync', versions, backend: warm.backend });
          for (const m of meshes) {
            const last = m.snapshots - 1;
            if (!uncapped && want('first')) {
              console.log(`   first picture ${m.name}: 5 opens`);
              const la = await load();
              const ms = [];
              for (let i = 0; i < 5; i++) {
                await new Promise((r) => setTimeout(r, 200));
                ms.push((await firstPicture(page, fieldHash(m.fields, last))).ms);
                await firstPicture(page, tinyHash);
              }
              const lb = await load();
              const s = summary(ms);
              result.runs.push({ repeat: rep, mesh: m.name, check: 'first', ms, loadBefore: la, loadAfter: lb });
              line(LIMITS.firstPicture, m.name, rep, s.p50, { note: `opens ${ms.map((x) => x.toFixed(0)).join(', ')} ms`, load: ldPair(la, lb) });
            }
            if (want('orbit')) {
              console.log(`   orbit ${m.name}: drags of ${SECONDS} s`);
              await firstPicture(page, fieldHash(m.fields, last));
              for (const pick of [true, false]) {
                const la = await load();
                const o = await orbit(page, { pick, seconds: SECONDS });
                const lb = await load();
                result.runs.push({ repeat: rep, mesh: m.name, check: 'orbit', uncapped, ...o, loadBefore: la, loadAfter: lb });
                const tag = `${pick ? 'with the hover readout' : 'pick skipped'}, ${uncapped ? 'no rate limit' : 'vertical sync'}`;
                const note = `${tag}: ${o.draws} draws, between draws p50 ${f1(o.betweenDrawsMs?.p50)}, p99 ${f1(o.betweenDrawsMs?.p99)}, max ${f1(o.betweenDrawsMs?.max)} ms; animation frame p99 ${f1(o.animationFrameMs?.p99)} ms; GPU per submit p50 ${f1(o.gpuMs?.p50)}, p99 ${f1(o.gpuMs?.p99)} ms; ${o.movesPerS.toFixed(0)} pointer moves per s`;
                console.log(`   orbit ${m.name} run ${rep}: ${o.drawsPerS.toFixed(1)} draws/s; ${note}; LOAD ${ldPair(la, lb)}`);
              }
            }
          }
        } finally {
          await app.close();
        }
      }
    }
    // scrubbing, through the run workspace, one launch per project
    if (want('scrub')) {
      for (const proj of [...new Set(meshes.filter((m) => m.project).map((m) => m.project))]) {
        console.log(`-- run ${rep}: scrubbing in the run workspace of ${proj}`);
        const { app, page } = await openProject(proj);
        try {
          for (const m of meshes.filter((x) => x.project === proj)) {
            await selectRun(page, m.run);
            const n = await page.evaluate(() => Number(document.querySelector('.tp-scrubber')?.getAttribute('aria-valuemax')) + 1);
            if (n !== m.snapshots) throw new Error(`${m.name}: the scrubber has ${n} stops, the store ${m.snapshots} snapshots`);
            const la = await load();
            const warm = await pass(page, n);
            const passes = [];
            const count = Math.max(1, Math.round(smoke ? 1 : 200 / n));
            for (let p = 0; p < count; p++) passes.push(await pass(page, n));
            const lb = await load();
            const frames = passes.flatMap((p) => p.frames);
            const total = passes.reduce((a, p) => a + p.totalMs, 0);
            const rate = (1000 * frames.length) / total;
            const s = summary(frames);
            const errors = [...warm.errors, ...passes.flatMap((p) => p.errors)];
            result.runs.push({ repeat: rep, mesh: m.name, check: 'scrub', stops: n, passes: count, requests: frames.length, snapshotsPerS: rate, frameMs: s, warmUpMs: warm.totalMs, framesByPass: passes.map((p) => p.frames.map((f) => Math.round(f * 10) / 10)), errors, loadBefore: la, loadAfter: lb });
            const note = `${frames.length} requests (${count} passes of ${n}), frame p50 ${f1(s.p50)}, p99 ${f1(s.p99)} ms; warm-up pass ${warm.totalMs.toFixed(0)} ms${errors.length ? `; ${errors.length} errors: ${errors.slice(0, 2).join('; ')}` : ''}`;
            line(LIMITS.scrubRate, m.name, rep, rate, { note, load: ldPair(la, lb) });
            line(LIMITS.scrubFrame, m.name, rep, s.max, { load: ldPair(la, lb) });
            if (m.info.cells >= 3e6) line(LIMITS.scrubRate4e6, m.name, rep, rate, { load: ldPair(la, lb) });
          }
        } finally {
          await app.close();
        }
      }
    }
  }
  // P8 lines: per mesh, repeat and pick mode, the better rate of the two launches and the synced run's p99
  for (let rep = 1; rep <= REPEATS; rep++) {
    for (const m of meshes) {
      for (const pick of [true, false]) {
        const o = result.runs.filter((r) => r.check === 'orbit' && r.repeat === rep && r.mesh === m.name && r.pick === pick);
        const sync = o.find((r) => !r.uncapped);
        const free = o.find((r) => r.uncapped);
        if (!sync && !free) continue;
        const best = [sync, free].filter(Boolean).sort((a, b) => b.drawsPerS - a.drawsPerS)[0];
        const tag = pick ? 'with the hover readout' : 'pick skipped';
        const ld = (r) => (r ? ldPair(r.loadBefore, r.loadAfter) : 'n/a');
        line({ ...LIMITS.drawRate, what: `orbit ${tag}, draws per second (the better of vertical sync and no rate limit)` }, m.name, rep, best.drawsPerS, {
          note: `vertical sync ${f1(sync?.drawsPerS)}, no rate limit ${f1(free?.drawsPerS)}; GPU per submit p50 ${f1(best.gpuMs?.p50)} ms`,
          load: `synced ${ld(sync)}; unlimited ${ld(free)}`,
        });
        if (sync) line({ ...LIMITS.drawFrame, what: `orbit ${tag}, p99 time between draws (vertical sync)` }, m.name, rep, sync.betweenDrawsMs?.p99 ?? null, { note: `p50 ${f1(sync.betweenDrawsMs?.p50)}, max ${f1(sync.betweenDrawsMs?.max)} ms`, load: ld(sync) });
      }
    }
  }
} catch (e) {
  failed = e.stack ?? String(e);
  console.error(`F-819: a case could not run: ${failed}`);
} finally {
  result.loadAfter = await load(true);
  try {
    rmSync(state, { recursive: true, force: true, maxRetries: 5 });
  } catch {
    // a helper process may still hold a file; the folder is in the temporary directory
  }
}
const judged = lines.filter((l) => l.judged);
const missed = judged.filter((l) => !l.met);
result.ended = new Date().toISOString();
result.lines = lines.map((l) => ({ ...l, text: text(l) }));
result.failed = failed;
result.profile = profileRecord();
result.verdict = missed.length ? 'MISSED' : failed ? 'INCOMPLETE' : 'met';
console.log(`\n==== F-819, ${result.started} to ${result.ended}; load before ${loadText(loadBefore)}, after ${loadText(result.loadAfter)}`);
for (const l of lines) console.log(text(l));
console.log(`verdict: ${result.verdict}: ${judged.length - missed.length} of ${judged.length} judged lines met${missed.length ? `; missed: ${[...new Set(missed.map((l) => `${l.id} ${l.mesh}`))].join(', ')}` : ''}${failed ? '; a case could not run' : ''}`);
const out = arg('out');
if (out) writeFileSync(out, JSON.stringify(result, null, 1) + '\n');
process.exit(missed.length ? 1 : failed ? 2 : 0);
