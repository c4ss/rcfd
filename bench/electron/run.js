// @ts-check
'use strict';
// Launcher of the Electron performance spike (ADR 0350 amendment 1, F-447). Run once by hand:
//
//   node run.js --data=<folder of f447_bench gen> --server=<f447_bench.exe> --out=<results folder> [--stages=A,B,C,P10]
//
// Stage A: P1 to P7, P12 with NC2, P11 and P8 with vertical sync. Stage B: P9 in a process of its own. Stage C: P8
// without a frame-rate limit. Stage P10: the spike packaged by hand (Electron's dist folder with the app in
// resources/app) and started ten times, from spawn to the run list on screen.

const { spawn, execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

/** @param {string} name */
function arg(name) {
  const hit = process.argv.find((a) => a.startsWith(`--${name}=`));
  return hit ? hit.slice(name.length + 3) : '';
}

const dataDir = path.resolve(arg('data'));
const server = path.resolve(arg('server'));
const outDir = path.resolve(arg('out'));
const stages = (arg('stages') || 'A,B,C,P10').split(',');
const electron = path.join(__dirname, 'node_modules', 'electron', 'dist', 'electron.exe');
fs.mkdirSync(outDir, { recursive: true });

function cpu() {
  let busy = 0;
  let total = 0;
  for (const c of os.cpus()) {
    busy += c.times.user + c.times.sys + c.times.irq;
    total += c.times.user + c.times.sys + c.times.irq + c.times.idle;
  }
  return { busy, total };
}

/** The machine's load over two seconds and the building processes of other agents (read only). */
async function load() {
  const a = cpu();
  await new Promise((r) => setTimeout(r, 2000));
  const b = cpu();
  let builders = -1;
  try {
    const list = execFileSync('tasklist', ['/FO', 'CSV', '/NH'], { encoding: 'utf8' });
    builders = list.split('\n').filter((l) => /^"(cargo|rustc|link)\.exe"/i.test(l)).length;
  } catch {
    // tasklist is a convenience; the load fraction is the record
  }
  return {
    at: new Date().toISOString(),
    cpuBusyFraction: (b.busy - a.busy) / Math.max(1, b.total - a.total),
    cargoRustcLinkProcesses: builders,
    freeMemGB: os.freemem() / 2 ** 30,
  };
}

/**
 * @param {string} exe
 * @param {string[]} args
 * @param {string} log
 * @returns {Promise<number>}
 */
function run(exe, args, log) {
  return new Promise((resolve) => {
    const out = fs.openSync(log, 'w');
    const child = spawn(exe, args, { stdio: ['ignore', out, out] });
    child.on('exit', (code) => {
      fs.closeSync(out);
      resolve(code ?? -1);
    });
  });
}

/**
 * @param {string} name
 * @param {string} checks
 * @param {string[]} extra
 */
async function stage(name, checks, extra) {
  const out = path.join(outDir, `${name}.json`);
  if (fs.existsSync(out)) fs.rmSync(out);
  const before = await load();
  console.log(`stage ${name}: ${checks} (load ${before.cpuBusyFraction.toFixed(2)})`);
  const code = await run(
    electron,
    [__dirname, `--data=${dataDir}`, `--server=${server}`, `--out=${out}`, `--checks=${checks}`, ...extra],
    path.join(outDir, `${name}.log`),
  );
  const result = fs.existsSync(out) ? JSON.parse(fs.readFileSync(out, 'utf8')) : { error: `no result file (exit code ${code})` };
  return { exitCode: code, loadBefore: before, loadAfter: await load(), result };
}

async function p10() {
  const packaged = path.join(outDir, 'packaged');
  fs.rmSync(packaged, { recursive: true, force: true });
  fs.cpSync(path.join(__dirname, 'node_modules', 'electron', 'dist'), packaged, { recursive: true });
  const appDir = path.join(packaged, 'resources', 'app');
  fs.mkdirSync(appDir, { recursive: true });
  for (const f of ['package.json', 'main.js', 'preload.js', 'io', 'renderer']) {
    fs.cpSync(path.join(__dirname, f), path.join(appDir, f), { recursive: true });
  }
  const exe = path.join(packaged, 'electron.exe');
  const before = await load();
  const ms = [];
  for (let i = 0; i < 10; i++) {
    const out = path.join(outDir, `start-${i}.json`);
    if (fs.existsSync(out)) fs.rmSync(out);
    const t0 = Date.now();
    const code = await run(exe, ['--start', `--data=${dataDir}`, `--out=${out}`], path.join(outDir, `start-${i}.log`));
    if (!fs.existsSync(out)) throw new Error(`start ${i}: no result file (exit code ${code})`);
    const r = JSON.parse(fs.readFileSync(out, 'utf8'));
    ms.push(r.readyUnixMs - t0);
    console.log(`start ${i}: ${ms[i]} ms to ${r.runs} runs listed`);
    await new Promise((r2) => setTimeout(r2, 1000));
  }
  const s = [...ms].sort((a, b) => a - b);
  return { ms, first: ms[0], p50: s[Math.ceil(0.5 * s.length) - 1], max: s[s.length - 1], loadBefore: before, loadAfter: await load() };
}

(async () => {
  /** @type {Record<string, any>} */
  const all = {
    machine: { cpu: os.cpus()[0].model, logical: os.cpus().length, totalMemGB: os.totalmem() / 2 ** 30, platform: `${os.platform()} ${os.release()}` },
    node: process.version,
    started: new Date().toISOString(),
  };
  if (stages.includes('A')) all.A = await stage('A', 'p1,p2,p3,p4,p5,p6,p7,p12,p11,p8', []);
  if (stages.includes('B')) all.B = await stage('B', 'p9', []);
  if (stages.includes('C')) all.C = await stage('C', 'p8', ['--uncapped']);
  if (stages.includes('P10')) {
    try {
      all.P10 = await p10();
    } catch (e) {
      all.P10 = { error: String(e) };
    }
  }
  all.finished = new Date().toISOString();
  const file = path.join(outDir, `results-${stages.join('-')}.json`);
  fs.writeFileSync(file, JSON.stringify(all, null, 1));
  console.log(`written ${file}`);
})();
