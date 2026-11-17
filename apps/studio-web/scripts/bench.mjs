// F-817: the performance regression bench of the product shell (docs/studio-guardrails.md G9). One entry point over the
// measurement scripts the roadmap items wrote; it repeats P2, P4, P5, P7, P8 and P10 against the packaged Studio and
// prints each against its limit, one line each:
//
//   P<n> <what> <value> <unit> limit <limit> met|MISSED
//
// Lines marked "(recorded)" are printed for comparison and are not judged. The machine is shared with other jobs, so the
// machine and its load (processor busy fraction, the processes that use the processor, free memory) are taken before
// and after the run and around each case, and a case run on a busy machine says so beside its numbers. A miss is not
// retried.
//
//   npm run build && npm run package:dir
//   npm run bench [-- --heavy] [--data=<folder>] [--bench-server=<f447_bench>] [--only=P2,P4,...] [--out=<file.json>] [--no-gen]
//
// The light set (default, a few minutes) and --heavy (once, by hand: house rule 7):
//
// | P   | script                  | light                                      | --heavy adds                                    |
// |-----|-------------------------|--------------------------------------------|-------------------------------------------------|
// | P10 | measure-cold-start.mjs  | 10 starts of the packaged copy             | (the same)                                      |
// | P2  | measure-service.mjs     | 100 iteration events of the 1e5 box (+P1)  | 300 events (ADR 0350's sample)                  |
// | P4  | measure-data.mjs        | 1e6, 200 repeats per route and codec       | P12 over the 1e6 and 4e6 boxes (recorded)       |
// | P5  | measure-scrub.mjs       | 1e6, 10 passes                             | P6 at 4e6 and Follow live (recorded)            |
// | P7  | measure-field.mjs       | 1e6, 5 repeats per route (R2, and R1)      | (the same)                                      |
// | P8  | measure-field.mjs       | 1e6 triangles, vertical sync and uncapped  | (the same)                                      |
// | F819| measure-large.mjs       | -                                          | P7, P5, P8 on 4e6 and FAN-01, once (recorded)   |
//
// The data sets are the boxes of crates/rcfd-api/examples/f447_bench.rs (`gen`): tiny and 1e6 for the light set, 4e6 as
// well for --heavy. They are kept in --data, else $RCFD_BENCH_DATA, else <temp>/rcfd-studio-bench-data, and the sizes
// that are missing are generated first (about 3 min for 1e6, 20 min for 4e6, not counted in any number) unless
// --no-gen. f447_bench is found as the tests find it (RCFD_F447_BENCH, $CARGO_TARGET_DIR/release/examples, then
// target/release/examples here or above); rcfd-mcp likewise (RCFD_MCP_BIN, $CARGO_TARGET_DIR/release, target/release).
//
// Judging, as the records of ADR 0801 did: P4 and P7 are met by route R1 or R2 (ADR 0350 §3), on the raw store for P4;
// P8's frame rate is the better of the two runs (F-808: vertical sync on a fast display already gives more than 60 per
// s), its p99 frame is the vertically synced run's. Exit code 0 when every judged line is met, 1 when one is missed, 2
// when a case could not run. The results (every line, every script's own JSON, the load) go to --out, else
// out/bench/bench-<time>.json. Not part of npm test or CI (F-823 decides that).
//
// The Studio runs in the test mode of playwright.config.ts (RCFD_TEST_HEADLESS=1, src/main/index.ts: the window shown
// in place but fully transparent, click-through, unfocused, off the taskbar, background throttling off), so that a run
// opens no visible window; RCFD_TEST_HEADLESS=0 in the environment measures with the normal window. The mode is printed
// and kept in the results.
//
// The Chromium profile (F-817b; ADR 0800 §9, Q5, the maintainer's answer of 2026-11-16): every case but P10 runs with a
// scratch profile. The bench makes a fresh folder for the case in the temporary directory and passes it to the script
// as --profile-root (scripts/profile.mjs), where each launch of the Studio gets an empty profile of its own
// (`--user-data-dir=<folder>/launch-<n>`); the folder is removed when the case has ended. P10 keeps the user's own
// profile (%APPDATA%\rcfd Studio on Windows), since it measures the user's own start. The profile of each case is
// printed and kept in the results.
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const arg = (name) => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3) ?? '';
const heavy = process.argv.includes('--heavy');
const noGen = process.argv.includes('--no-gen');
const only = arg('only') ? new Set(arg('only').toUpperCase().split(',')) : null;
const BUSY = 0.5; // a processor busy fraction at or above this is reported as a loaded machine

// ---- the binaries and the data
const exe = (n) => (process.platform === 'win32' ? `${n}.exe` : n);
function findUp(rel) {
  for (let dir = root; ; dir = path.dirname(dir)) {
    const p = path.join(dir, rel);
    if (existsSync(p)) return p;
    if (path.dirname(dir) === dir) return null;
  }
}
function findBin(name, envName, sub) {
  const fromEnv = process.env[envName];
  if (fromEnv && existsSync(fromEnv)) return fromEnv;
  const target = process.env.CARGO_TARGET_DIR;
  if (target && existsSync(path.join(target, 'release', ...sub, exe(name)))) return path.join(target, 'release', ...sub, exe(name));
  return findUp(path.join('target', 'release', ...sub, exe(name)));
}
const benchServer = arg('bench-server') || findBin('f447_bench', 'RCFD_F447_BENCH', ['examples']);
const mcp = findBin('rcfd-mcp', 'RCFD_MCP_BIN', []);
const data = path.resolve(arg('data') || process.env.RCFD_BENCH_DATA || path.join(os.tmpdir(), 'rcfd-studio-bench-data'));
const packagedJson = path.join(root, 'out', 'packaged.json');
if (!existsSync(packagedJson)) {
  console.error('no packaged copy: run npm run build && npm run package:dir first');
  process.exit(2);
}
const { executable } = JSON.parse(readFileSync(packagedJson, 'utf8'));
if (!benchServer) {
  console.error('f447_bench not found: cargo build --release -p rcfd-api --example f447_bench, or set RCFD_F447_BENCH or pass --bench-server');
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
/** The processes that used the processor over about 2 s, summed by name (cores = processor seconds per second). */
function topProcesses() {
  try {
    if (process.platform === 'win32') {
      const ps = [
        '$a=@{}; Get-Process | ForEach-Object { if ($_.CPU -ne $null) { $a[$_.Id]=$_.CPU } }; Start-Sleep -Milliseconds 2000;',
        '$r = Get-Process | Where-Object { $_.CPU -ne $null -and $a.ContainsKey($_.Id) } | ForEach-Object { [pscustomobject]@{ n=$_.ProcessName; d=($_.CPU - $a[$_.Id]) } };',
        '$r | Group-Object n | ForEach-Object { [pscustomobject]@{ name=$_.Name; count=$_.Count; cores=[math]::Round((($_.Group | Measure-Object d -Sum).Sum)/2,2) } } | Where-Object { $_.cores -gt 0.05 } | Sort-Object cores -Descending | Select-Object -First 12 | ConvertTo-Json -Compress',
      ].join(' ');
      const out = execFileSync('powershell', ['-NoProfile', '-NonInteractive', '-Command', ps], { encoding: 'utf8', windowsHide: true, timeout: 30_000 }).trim();
      if (!out) return [];
      const j = JSON.parse(out);
      return Array.isArray(j) ? j : [j];
    }
    const out = execFileSync('ps', ['-eo', 'comm,pcpu', '--sort=-pcpu'], { encoding: 'utf8', timeout: 10_000 });
    return out
      .split('\n')
      .slice(1, 13)
      .map((l) => l.trim().split(/\s+/))
      .filter((p) => p.length >= 2)
      .map(([name, pcpu]) => ({ name, count: 1, cores: Number(pcpu) / 100 }));
  } catch {
    return null;
  }
}
async function load(withProcesses) {
  const a = cpuTimes();
  const procs = withProcesses ? topProcesses() : null;
  if (!withProcesses) await new Promise((r) => setTimeout(r, 1000));
  const b = cpuTimes();
  const builders = procs ? procs.filter((p) => /^(cargo|rustc|link)$/i.test(p.name)).reduce((n, p) => n + p.count, 0) : null;
  return { at: new Date().toISOString(), cpuBusyFraction: (b.busy - a.busy) / Math.max(1, b.total - a.total), freeMemGB: os.freemem() / 2 ** 30, busyBuilders: builders, processes: procs };
}
const procText = (l) => (l.processes ? l.processes.slice(0, 6).map((p) => `${p.name}${p.count > 1 ? ` x${p.count}` : ''} ${p.cores.toFixed(1)}`).join(', ') : 'not taken');
const machine = {
  cpu: os.cpus()[0]?.model,
  logical: os.cpus().length,
  totalMemGB: Math.round((10 * os.totalmem()) / 2 ** 30) / 10,
  platform: `${os.platform()} ${os.release()}`,
};

// ---- the cases
const CASES = [
  { ids: ['P10'], script: 'measure-cold-start.mjs', args: [], profile: 'own' },
  { ids: ['P2'], script: 'measure-service.mjs', args: [`--bench-server=${benchServer}`, ...(heavy ? [] : ['--events=100'])] },
  { ids: ['P4'], sizes: heavy ? ['1e6', '4e6'] : ['1e6'], script: 'measure-data.mjs', args: [`--bench-server=${benchServer}`, `--data=${data}`, ...(heavy ? ['--p12'] : [])] },
  { ids: ['P5'], sizes: heavy ? ['1e6', '4e6'] : ['1e6'], script: 'measure-scrub.mjs', strip: true, args: [`--data=${data}`, ...(heavy ? [] : ['--no-p6', '--no-live'])], needs: mcp ? null : 'rcfd-mcp not found (cargo build --release -p rcfd-mcp, or RCFD_MCP_BIN)' },
  { ids: ['P7', 'P8'], sizes: ['tiny', '1e6'], script: 'measure-field.mjs', args: [`--data=${data}`, `--bench-server=${benchServer}`] },
  // F-819 (--heavy only): P7, P5 and P8 on the 4e6 box and the FAN-01 meshes (scripts/measure-large.mjs), one repeat, recorded
  ...(heavy ? [{ ids: ['F819'], sizes: ['tiny', '4e6'], script: 'measure-large.mjs', args: [`--data=${data}`, '--repeats=1', ...(noGen ? ['--no-gen'] : [])], needs: mcp ? null : 'rcfd-mcp not found (cargo build --release -p rcfd-mcp, or RCFD_MCP_BIN)' }] : []),
].filter((c) => !only || c.ids.some((id) => only.has(id)));

// ---- the data sets
const sizes = ['tiny', '1e6', '4e6'].filter((s) => CASES.some((c) => c.sizes?.includes(s)));
const have = (s) =>
  ['raw', 'deflate'].every((c) => existsSync(path.join(data, 'project', 'runs', `box-${s}-${c}`, 'fields', 'store.rts'))) && existsSync(path.join(data, 'ref', s, '19-uz.f32'));
const missing = sizes.filter((s) => !have(s));
const gen = { sizes: missing, seconds: null };
if (missing.length) {
  if (noGen) {
    console.error(`missing data sets ${missing.join(', ')} in ${data} (--no-gen): run ${benchServer} gen ${data} ${missing.join(' ')}`);
    process.exit(2);
  }
  mkdirSync(data, { recursive: true });
  console.log(`generating the data sets ${missing.join(', ')} in ${data} (f447_bench gen; not timed as part of any P)`);
  const t = Date.now();
  await new Promise((resolve, reject) => {
    const child = spawn(benchServer, ['gen', data, ...missing], { stdio: ['ignore', 'inherit', 'inherit'], windowsHide: true });
    child.once('error', reject);
    child.once('exit', (c) => (c === 0 ? resolve() : reject(new Error(`f447_bench gen ended with ${c}`))));
  });
  gen.seconds = (Date.now() - t) / 1000;
}

const scratch = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f817-'));
const headless = (process.env.RCFD_TEST_HEADLESS ?? '1') === '1';
const childEnv = { ...process.env, RCFD_TEST_HEADLESS: headless ? '1' : '0', ...(mcp ? { RCFD_MCP_BIN: mcp } : {}) };
const modeText = headless ? 'test mode RCFD_TEST_HEADLESS=1 (window transparent, opacity 0, unfocused; background throttling off)' : 'normal window (RCFD_TEST_HEADLESS=0)';

/** Q5: a fresh profile folder for a case (removed by `removeProfile` when it has ended), or null for the user's own. */
const scratchProfile = (c) => (c.profile === 'own' ? null : mkdtempSync(path.join(os.tmpdir(), `rcfd-f817-profile-${c.ids.join('-')}-`)));
function removeProfile(dir) {
  try {
    // the Studio's helper processes can hold a file for a moment after the script has ended
    rmSync(dir, { recursive: true, force: true, maxRetries: 20, retryDelay: 250 });
  } catch (e) {
    console.log(`    the profile folder ${dir} could not be removed: ${e.message}`);
  }
  return !existsSync(dir);
}
const profileText = (dir) => (dir ? `scratch, a fresh --user-data-dir per launch under ${dir}, removed after the case` : "the user's own (Q5: P10 measures the user's own start)");

function runCase(c, outFile, profileDir) {
  return new Promise((resolve) => {
    const nodeArgs = [...(c.strip ? ['--experimental-strip-types', '--no-warnings'] : []), path.join(root, 'scripts', c.script), ...c.args, ...(profileDir ? [`--profile-root=${profileDir}`] : []), `--out=${outFile}`];
    const child = spawn(process.execPath, nodeArgs, { cwd: root, env: childEnv, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
    const tail = [];
    let firstError = null;
    const echo = (d) => {
      for (const line of d.toString('utf8').split(/\r?\n/)) {
        if (!line) continue;
        console.log(`    | ${line}`);
        if (firstError === null && /^\S*Error\b|exceeded|^no .* store|not found/.test(line.trim())) firstError = line.trim().slice(0, 200);
        tail.push(line);
        if (tail.length > 20) tail.shift();
      }
    };
    child.stdout.on('data', echo);
    child.stderr.on('data', echo);
    child.once('error', (e) => resolve({ code: -1, tail: [String(e)], firstError: String(e) }));
    child.once('exit', (code) => resolve({ code, tail, firstError }));
  });
}

const lines = [];
const busyFmt = (v) => (typeof v === 'number' ? v.toFixed(2) : 'n/a');
const fmt = (v) =>
  v === null || v === undefined || Number.isNaN(v) ? 'n/a' : Number.isInteger(v) ? String(v) : Math.abs(v) >= 100 ? v.toFixed(0) : Math.abs(v) >= 10 ? v.toFixed(1) : v.toFixed(2);
/** One line of the bench. op is '<=' or '>='; judged lines decide the exit code. */
function line(id, what, value, unit, op, limit, { judged = true, note = '' } = {}) {
  const met = value !== null && value !== undefined && (op === '<=' ? value <= limit : value >= limit);
  lines.push({ id, what, value, unit, limit: `${op}${limit}`, met, judged, note });
}

function linesFor(c, r) {
  if (c.ids[0] === 'F819') {
    // its own limits and verdict are in its output; in the bench every line is recorded
    for (const l of r.lines ?? []) line(l.id, `${l.what} (F-819)`, l.value, l.unit, l.op, l.limit, { judged: false, note: l.note });
  } else if (c.ids[0] === 'P10') {
    line('P10', 'cold start to the project view, median of 10', r.p50, 'ms', '<=', 2500, { note: `first ${r.first}, max ${r.max}` });
  } else if (c.ids[0] === 'P2') {
    const l = r.P2.latencyMs;
    line('P2', `solver event to renderer, p99 of ${r.P2.events} events`, l?.p99 ?? null, 'ms', '<=', 20, { note: l ? `p50 ${fmt(l.p50)}, max ${fmt(l.max)}; clock drift ${fmt(r.P2.clock.driftMs)} ms; missed ${r.P2.missed}` : 'no latency' });
    if (r.P2.events < 50) line('P2', 'sample size, iteration events received in 120 s', r.P2.events, 'events', '>=', 50);
    const m = r.P1.main;
    line('P1', 'service.info round trip, main process, p99', m['service.info'].p99, 'ms', '<=', 2, { judged: false });
    line('P1', 'run.status round trip, main process, p99', m['run.status'].p99, 'ms', '<=', 2, { judged: false });
    // F-807's P2 per run action, written by npm test (tests/e2e/run-control.spec.ts), when there is one
    const f807 = path.join(root, 'test-results-visual', 'f807', 'p2.json');
    if (existsSync(f807)) {
      try {
        const per = JSON.parse(readFileSync(f807, 'utf8')).filter((e) => typeof e.receivedMs === 'number');
        const worst = per.reduce((a, e) => (e.receivedMs > a.receivedMs ? e : a), per[0]);
        if (worst) line('P2', 'engine answer to renderer per run action (F-807, worst)', worst.receivedMs, 'ms', '<=', 20, { judged: false, note: `${worst.action}; ${per.length} actions; npm test of ${statSync(f807).mtime.toISOString()}` });
      } catch {
        // a partial file of an interrupted test run
      }
    }
  } else if (c.ids[0] === 'P4') {
    const raw = r.P4.filter((l) => l.codec === 'raw');
    // ADR 0350 §3: R1 or R2, on the raw store; the judged route is one that meets, else the one with the lower p99
    const best = raw.find((l) => l.met) ?? [...raw].sort((a, b) => a.ms.p99 - b.ms.p99)[0];
    line('P4', `1e6 scalar to GPU buffer, ${best.route} raw, p50`, best.ms.p50, 'ms', '<=', 30, { note: `R1 or R2; max ${fmt(best.ms.max)}` });
    line('P4', `1e6 scalar to GPU buffer, ${best.route} raw, p99`, best.ms.p99, 'ms', '<=', 60);
    for (const l of r.P4.filter((x) => x !== best)) {
      line('P4', `1e6 scalar to GPU buffer, ${l.route} ${l.codec}, p50`, l.ms.p50, 'ms', '<=', 30, { judged: false });
      line('P4', `1e6 scalar to GPU buffer, ${l.route} ${l.codec}, p99`, l.ms.p99, 'ms', '<=', 60, { judged: false });
    }
    if (r.P12) {
      const bad = r.P12.filter((x) => !x.notRun).reduce((n, x) => n + x.differingFields, 0);
      line('P12', `fields equal to the Rust reader (${r.P12.filter((x) => !x.notRun).length} size, route, codec lines)`, bad, 'differing fields', '<=', 0, { judged: false });
    }
  } else if (c.ids[0] === 'P5') {
    for (const s of r.scrub) {
      if (s.check === 'P5') {
        line('P5', `scrubbing 1e6, ${s.requests} requests`, s.snapshotsPerS, 'snapshots/s', '>=', 10, { note: `frame p50 ${fmt(s.frameMs.p50)}, p99 ${fmt(s.frameMs.p99)}` });
        line('P5', 'scrubbing 1e6, longest frame', s.frameMs.max, 'ms', '<=', 150);
        if (s.errors.length) line('P5', `scrubbing 1e6, stops that disagreed (${s.errors[0]})`, s.errors.length, 'stops', '<=', 0);
      } else {
        line(s.check, `scrubbing ${s.size}, ${s.requests} requests`, s.snapshotsPerS, 'snapshots/s', '>=', 3, { judged: false, note: `longest frame ${fmt(s.frameMs.max)} ms` });
      }
    }
    if (r.live?.commitToDrawnMs) line('P5', 'Follow live, commit to drawn, p50 (no limit in ADR 0350)', r.live.commitToDrawnMs.p50, 'ms', '<=', Infinity, { judged: false, note: `max ${fmt(r.live.commitToDrawnMs.max)}; ${r.live.snapshotsCommitted} committed, ${r.live.snapshotsDrawn} drawn` });
  } else {
    // P7: R1 or R2, on the median of 5
    const best = [...r.P7].sort((a, b) => a.ms.p50 - b.ms.p50)[0];
    line('P7', `first picture of the 1e6 surface, ${best.route}, median of ${best.all.length}`, best.ms.p50, 'ms', '<=', 1500, { note: `R1 or R2; first ${fmt(best.first)}` });
    for (const l of r.P7.filter((x) => x !== best)) line('P7', `first picture of the 1e6 surface, ${l.route}, median of ${l.all.length}`, l.ms.p50, 'ms', '<=', 1500, { judged: false, note: `first ${fmt(l.first)}` });
    const sync = r.P8.find((l) => !l.uncapped);
    const free = r.P8.find((l) => l.uncapped);
    const fast = [sync, free].filter(Boolean).sort((a, b) => b.fps - a.fps)[0];
    line('P8', `1e6 triangles orbiting, frame rate (${fast.uncapped ? 'no rate limit' : 'vertical sync'}, the better run)`, fast.fps, 'frames/s', '>=', 60, { note: `display ${fmt(sync?.displayHz)} Hz` });
    if (sync) line('P8', '1e6 triangles orbiting, p99 frame (vertical sync)', sync.frameMs.p99, 'ms', '<=', 25, { note: `p50 ${fmt(sync.frameMs.p50)}, max ${fmt(sync.frameMs.max)}` });
    const other = fast === sync ? free : sync;
    if (other) line('P8', `1e6 triangles orbiting, frame rate (${other.uncapped ? 'no rate limit' : 'vertical sync'})`, other.fps, 'frames/s', '>=', 60, { judged: false });
  }
}

const started = new Date();
console.log(`F-817 bench, ${heavy ? 'heavy (--heavy)' : 'light'} set, ${started.toISOString()}`);
console.log(`machine: ${machine.cpu} (${machine.logical} logical), ${machine.totalMemGB} GB, ${machine.platform}; launcher Node ${process.version}`);
console.log(`window: ${modeText}`);
console.log(`packaged copy: ${executable} (${existsSync(executable) ? `packaged ${statSync(packagedJson).mtime.toISOString()}` : 'missing'})`);
console.log(`data: ${data}${gen.seconds !== null ? ` (generated ${missing.join(', ')} in ${gen.seconds.toFixed(0)} s)` : ''}; f447_bench ${benchServer}; rcfd-mcp ${mcp ?? 'not found'}`);
const loadBefore = await load(true);
console.log(`load before: processor busy ${loadBefore.cpuBusyFraction.toFixed(2)}, ${loadBefore.freeMemGB.toFixed(1)} GB free; busiest processes (cores): ${procText(loadBefore)}`);

const cases = [];
for (const c of CASES) {
  const entry = { ids: c.ids, script: c.script, args: c.args, startedAt: new Date().toISOString() };
  cases.push(entry);
  if (c.needs) {
    entry.notRun = c.needs;
    console.log(`\n-- ${c.ids.join(', ')}: not run: ${c.needs}`);
    continue;
  }
  console.log(`\n-- ${c.ids.join(', ')}: ${c.script} ${c.args.map((a) => a.replace(/=.*[\\/]/, '=…/')).join(' ')}`);
  const outFile = path.join(scratch, `${c.ids.join('-')}.json`);
  const profileDir = scratchProfile(c);
  console.log(`   profile: ${profileText(profileDir)}`);
  const t = Date.now();
  const loadCase = await load(false);
  const { code, tail, firstError } = await runCase(c, outFile, profileDir);
  entry.wallS = (Date.now() - t) / 1000;
  entry.exitCode = code;
  entry.profile = { mode: profileDir ? 'scratch' : 'own', root: profileDir, removed: profileDir ? removeProfile(profileDir) : null };
  const from = lines.length;
  if (existsSync(outFile)) {
    entry.result = JSON.parse(readFileSync(outFile, 'utf8'));
    if (entry.result.profile?.launches !== undefined) entry.profile.launches = entry.result.profile.launches;
    try {
      linesFor(c, entry.result);
    } catch (e) {
      entry.notRun = `the result could not be read: ${e.message}`;
    }
  } else {
    entry.notRun = `${c.script} ended with ${code} and wrote no result: ${firstError ?? tail.slice(-3).join(' / ')}`;
  }
  // each script takes its own load; when it wrote nothing, the bench's own samples around it stand in
  const lb = entry.result?.loadBefore?.cpuBusyFraction ?? loadCase.cpuBusyFraction;
  const la = entry.result?.loadAfter?.cpuBusyFraction ?? (await load(false)).cpuBusyFraction;
  entry.load = { before: lb ?? null, after: la ?? null };
  const busy = (lb ?? 0) >= BUSY || (la ?? 0) >= BUSY;
  for (const l of lines.slice(from)) {
    l.load = entry.load;
    l.loaded = busy;
  }
  const profileDone = entry.profile.mode === 'own' ? "the user's own profile" : `scratch profile, ${entry.profile.launches ?? '?'} launch${entry.profile.launches === 1 ? '' : 'es'}, folder ${entry.profile.removed ? 'removed' : 'NOT removed'}`;
  console.log(`-- ${c.ids.join(', ')}: ${entry.notRun ? `NOT RUN (${entry.notRun})` : 'done'} in ${entry.wallS.toFixed(0)} s; processor busy ${busyFmt(lb)} before, ${busyFmt(la)} after; ${profileDone}`);
}
const loadAfter = await load(true);
rmSync(scratch, { recursive: true, force: true });

// ---- the summary
const text = (l) =>
  `${l.id} ${l.what} ${fmt(l.value)} ${l.unit} limit ${l.limit.replace('Infinity', 'none')} ${l.met ? 'met' : 'MISSED'}${l.judged ? '' : ' (recorded)'}${l.note ? `; ${l.note}` : ''}${l.loaded ? `; LOADED MACHINE (processor busy ${busyFmt(l.load.before)} before, ${busyFmt(l.load.after)} after)` : ''}`;
console.log(`\n==== F-817 bench (${heavy ? 'heavy' : 'light'}), ${started.toISOString()} to ${new Date().toISOString()}`);
console.log(`machine: ${machine.cpu} (${machine.logical} logical), ${machine.totalMemGB} GB, ${machine.platform}`);
console.log(`window: ${modeText}`);
const profileOf = (mode) => cases.filter((c) => c.profile?.mode === mode).map((c) => c.ids.join(', ')).join('; ') || 'none';
console.log(`profile: scratch for ${profileOf('scratch')}; the user's own for ${profileOf('own')}`);
console.log(`load before: processor busy ${loadBefore.cpuBusyFraction.toFixed(2)}; ${procText(loadBefore)}`);
console.log(`load after:  processor busy ${loadAfter.cpuBusyFraction.toFixed(2)}; ${procText(loadAfter)}`);
for (const l of lines) console.log(text(l));
const notRun = cases.filter((c) => c.notRun);
for (const c of notRun) console.log(`${c.ids.join(', ')} NOT RUN: ${c.notRun}`);
const judged = lines.filter((l) => l.judged);
const missed = judged.filter((l) => !l.met);
const loaded = loadBefore.cpuBusyFraction >= BUSY || loadAfter.cpuBusyFraction >= BUSY || lines.some((l) => l.loaded);
const verdict = missed.length ? 'MISSED' : notRun.length ? 'INCOMPLETE' : 'met';
console.log(
  `verdict: ${verdict}: ${judged.length - missed.length} of ${judged.length} judged lines met${missed.length ? `; missed: ${[...new Set(missed.map((l) => l.id))].join(', ')}` : ''}${notRun.length ? `; not run: ${notRun.map((c) => c.ids.join(', ')).join('; ')}` : ''}${loaded ? '; the machine was loaded during the run (see above), the numbers are not retried' : ''}`,
);

const outFile = arg('out') ? path.resolve(arg('out')) : path.join(root, 'out', 'bench', `bench-${started.toISOString().replace(/[:.]/g, '-')}.json`);
mkdirSync(path.dirname(outFile), { recursive: true });
const results = {
  bench: 'F-817 (docs/studio-guardrails.md G9)',
  mode: heavy ? 'heavy' : 'light',
  started: started.toISOString(),
  ended: new Date().toISOString(),
  machine,
  launcherNode: process.version,
  window: { headless, RCFD_TEST_HEADLESS: headless ? '1' : '0', note: modeText },
  executable,
  data,
  generated: gen,
  benchServer,
  mcp,
  loadBefore,
  loadAfter,
  lines: lines.map((l) => ({ ...l, limit: l.limit.replace('Infinity', 'none'), text: text(l) })),
  verdict,
  cases,
};
writeFileSync(outFile, JSON.stringify(results, null, 1) + '\n');
console.log(`results: ${outFile}`);
process.exit(missed.length ? 1 : notRun.length ? 2 : 0);
