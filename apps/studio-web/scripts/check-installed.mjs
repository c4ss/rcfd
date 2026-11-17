// Starts an installed (or unpacked) Studio with none of the RCFD_STUDIO_* variables and checks that it uses the engine
// it ships with (F-820, ADR 0801 Addendum 19): the project server is <resources>/bin's rcfd-mcp, no service runs at
// launch, and opening a project starts <resources>/bin's rcfd serve. The installers workflow runs it after the silent
// install-then-upgrade check (scripts/check-nsis-upgrade.ps1); it can be run by hand on win-unpacked or linux-unpacked.
//
//   node scripts/check-installed.mjs --exe=<path of rcfd-studio(.exe)>
//
// The recent list and the service's state go to a temporary folder (RCFD_STUDIO_RECENT, RCFD_STUDIO_STATE_DIR: they
// choose no engine), and the project is an empty one in a temporary folder; the window is kept hidden
// (RCFD_TEST_HEADLESS=1, which changes nothing about where the engine is found). Exit code 0 when every check holds.
import { _electron as electron } from '@playwright/test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const exeArg = process.argv.find((a) => a.startsWith('--exe='))?.slice(6);
if (!exeArg || !existsSync(exeArg)) {
  console.error(`--exe=<path of the installed rcfd-studio> is required${exeArg ? ` (${exeArg} does not exist)` : ''}`);
  process.exit(2);
}
const exe = path.resolve(exeArg);
const win = process.platform === 'win32';
const bin = path.join(path.dirname(exe), 'resources', 'bin');
const shippedMcp = path.join(bin, win ? 'rcfd-mcp.exe' : 'rcfd-mcp');
const shippedRcfd = path.join(bin, win ? 'rcfd.exe' : 'rcfd');

const scratch = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f820-installed-'));
const project = path.join(scratch, 'project');
for (const d of ['cases', 'meshes', 'runs', 'designs', 'notes']) mkdirSync(path.join(project, d), { recursive: true });
writeFileSync(
  path.join(project, 'rcfd-project.json'),
  JSON.stringify({ schema_version: 1, name: 'Installed check', created: '2026-11-17T09:00:00.000Z', solver_version_created: 'rcfd 0.0.0' }, null, 2) + '\n',
);
const stateDir = path.join(scratch, 'state');

const env = {};
for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_') && !k.startsWith('RCFD_TEST_')) env[k] = v;
Object.assign(env, { RCFD_TEST_HEADLESS: '1', RCFD_STUDIO_STATE_DIR: stateDir, RCFD_STUDIO_RECENT: path.join(scratch, 'recent.json') });

const failures = [];
const check = (ok, what) => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
  if (!ok) failures.push(what);
};
const until = async (f, ms, what) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() > end) throw new Error(`timed out after ${ms} ms waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 100));
  }
};
const same = (a, b) => (win ? path.resolve(a).toLowerCase() === path.resolve(b).toLowerCase() : path.resolve(a) === path.resolve(b));

console.log(`installed Studio: ${exe}`);
check(existsSync(shippedMcp) && existsSync(shippedRcfd), `the engine is shipped in ${bin}`);
const app = await electron.launch({ executablePath: exe, env, ...(process.platform === 'linux' ? { chromiumSandbox: true } : {}) });
let servicePid = null;
try {
  const page = await app.firstWindow();
  await page.waitForFunction(() => document.querySelector('h1')?.textContent === 'No project open', null, { timeout: 30_000 });
  const info = await page.evaluate(() => window.studio.app.info()).catch(() => null);
  if (info) console.log(`app: ${JSON.stringify(info)}`);
  const engine = (await page.evaluate(() => window.studioProject.state())).engine;
  check(engine.configured === true && engine.from === 'bundled' && same(engine.path, shippedMcp), `the project server is the shipped rcfd-mcp (${JSON.stringify(engine)})`);
  await new Promise((r) => setTimeout(r, 1000));
  const before = await page.evaluate(() => window.studioService.state());
  check(before.status === 'waiting' && before.pid === null, `no service at launch, it waits for a project (status ${before.status})`);
  check(!existsSync(path.join(stateDir, 'service.json')), 'nothing was started at launch (no service.json)');

  await app.evaluate(({ dialog }, f) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [f] });
  }, project);
  await page.evaluate(() => window.studioProject.pick());
  const opened = await until(async () => (await page.evaluate(() => window.studioProject.state())).project, 60_000, 'the project to open');
  check(same(opened.folder, project), `the project opened (${opened.folder})`);
  const connected = await until(async () => {
    const s = await page.evaluate(() => window.studioService.state());
    return s.status === 'connected' ? s : null;
  }, 30_000, 'the service to connect');
  servicePid = connected.pid;
  check(connected.mode === 'spawn' && connected.adopted === false, `opening the project started rcfd serve (pid ${connected.pid}, ${connected.address}, rcfd ${connected.version})`);
  const record = JSON.parse(readFileSync(path.join(stateDir, 'service.json'), 'utf8'));
  check(same(record.rcfd, shippedRcfd), `the service is the shipped rcfd (${record.rcfd})`);
} catch (e) {
  failures.push(String(e instanceof Error ? e.message : e));
  console.log(`FAIL ${failures.at(-1)}`);
} finally {
  // no session was made, so the Studio stops the service it started when it quits
  await app.close().catch(() => undefined);
}
if (servicePid !== null) {
  const gone = await until(() => {
    try {
      process.kill(servicePid, 0);
      return false;
    } catch {
      return true;
    }
  }, 15_000, 'the service to stop').catch(() => false);
  check(gone === true, 'the service stopped with the Studio (it held no session)');
  if (gone !== true) {
    try {
      process.kill(servicePid);
    } catch {
      // gone after all
    }
  }
}
rmSync(scratch, { recursive: true, force: true });
console.log(failures.length ? `${failures.length} check(s) failed` : 'every check holds');
process.exit(failures.length ? 1 : 0);
