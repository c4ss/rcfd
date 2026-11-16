// P10 of docs/studio-guardrails.md G9 (cold start to the project view, limit 2.5 s), by the method of ADR 0350 §4: the
// app packaged by hand (scripts/package-dir.mjs), the time from spawn to the renderer's report that the empty project
// view is in the document and one animation frame has passed (the app.ready channel; the main process writes the time
// to RCFD_STUDIO_STARTUP_REPORT and quits). 10 starts one second apart, judged on the median; the first start and the
// maximum reported, with the machine's load before and after. A true cold start (after a reboot) is not taken.
//
//   npm run build && node scripts/package-dir.mjs && node scripts/measure-cold-start.mjs [--out=<file.json>]
//
// A heavy record (house rule 7): run by hand, once, and recorded in ADR 0801 addendum 1. Not part of npm test.
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outArg = process.argv.find((a) => a.startsWith('--out='))?.slice(6) ?? '';
const STARTS = 10;

const { executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8'));

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

function start(report) {
  return new Promise((resolve) => {
    const t0 = Date.now();
    const child = spawn(executable, [], {
      stdio: 'ignore',
      env: { ...process.env, RCFD_STUDIO_STARTUP_REPORT: report },
    });
    child.on('exit', (code) => resolve({ t0, code }));
  });
}

const scratch = mkdtempSync(path.join(os.tmpdir(), 'rcfd-p10-'));
const loadBefore = await load();
console.log(`P10: ${STARTS} starts of ${executable} (load ${loadBefore.cpuBusyFraction.toFixed(2)})`);
const ms = [];
for (let i = 0; i < STARTS; i++) {
  const report = path.join(scratch, `start-${i}.json`);
  const { t0, code } = await start(report);
  if (!existsSync(report)) throw new Error(`start ${i}: no report (exit code ${code})`);
  const r = JSON.parse(readFileSync(report, 'utf8'));
  ms.push(r.readyUnixMs - t0);
  console.log(`start ${i}: ${ms[i]} ms`);
  await new Promise((r2) => setTimeout(r2, 1000));
}
const loadAfter = await load();
rmSync(scratch, { recursive: true, force: true });

const s = [...ms].sort((a, b) => a - b);
const result = {
  check: 'P10',
  limitMs: 2500,
  machine: { cpu: os.cpus()[0]?.model, logical: os.cpus().length, totalMemGB: os.totalmem() / 2 ** 30, platform: `${os.platform()} ${os.release()}` },
  launcherNode: process.version,
  executable,
  ms,
  first: ms[0],
  p50: s[Math.ceil(0.5 * s.length) - 1],
  max: s[s.length - 1],
  loadBefore,
  loadAfter,
};
result.met = result.p50 <= result.limitMs;
console.log(`P10: median ${result.p50} ms, first ${result.first} ms, max ${result.max} ms; limit 2500 ms: ${result.met ? 'met' : 'NOT met'}`);
if (outArg) writeFileSync(outArg, JSON.stringify(result, null, 1) + '\n');
