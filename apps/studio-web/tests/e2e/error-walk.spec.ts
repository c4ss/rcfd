// F-830 (G19): a walk through the Studio's real surfaces, against the real engine (`rcfd serve --ui` for the sessions,
// `rcfd-mcp --project` for a copy of the quick-start project), that provokes every error of the engine's error
// catalogue (`help.errors`) a Studio user can reach, and checks that each shows the catalogue's recovery line for its
// code, as the engine wrote it. Another writer (a second connection to the same service, as Claude Code or `rcfd call`
// would be, or a file written beside the Studio) sets up the state where a user's action gets the error.
//
// The catalogue's other rows are listed below, each with why the walk does not provoke it; the test fails when the
// catalogue gains a row that is in neither list, so the walk is kept up with the engine. Screenshots at 1440 by 900 to
// test-results-visual/f830-errors/.
//
// Needs rcfd and rcfd-mcp (tests/support/rcfd.ts, tests/support/project.ts); without them the tests are skipped.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { mkdir, readFile, rm, unlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { copyExample, findMcp, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
const mcp = findMcp();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f830-errors');
const CASE = 'cases/cavity-re100.toml';

interface Entry {
  readonly surface: string;
  readonly code: number;
  readonly name: string;
  readonly recovery: string;
}

/** The JSON-RPC codes this walk provokes through the Studio, and where (each test below names its code). */
const PROVOKED: Readonly<Record<number, string>> = {
  1001: 'the case editor: its session closed by another writer',
  1002: 'the case editor: a setting changed while another writer holds the session’s run paused',
  1003: 'the case editor: a value the engine refuses (a negative viscosity)',
  1004: 'the mesh preview: a mesh over the preview limit',
  1201: 'the case editor: Save while another writer holds the file’s edit lock',
  1202: 'the run bar: Run of a case file deleted under the Studio',
};

/** The catalogue's codes the walk does not provoke, with the reason (kept small and honest). */
const NOT_PROVOKED: Readonly<Record<string, string>> = {
  'service -32700': 'the Studio’s client writes every request as one JSON object per line (src/main/service/rpc.ts)',
  'service -32600': 'the Studio’s client writes every request as JSON-RPC 2.0 with a method',
  'service -32601': 'the Studio calls only the methods it starts the service for (--ui); without --ui help.errors itself is missing, so no line could be shown',
  'service -32602': 'the dispatcher checks every call’s parameters against the action registry before it is sent (F-827)',
  'service 1005': 'the Studio has no co-simulation surface',
  'service 1006': 'the Studio has no co-simulation surface',
  'service 1007': 'answered by the conflict panel and its three choices, not the line (tests/e2e/undo-case.spec.ts, case-file-adr0737.spec.ts)',
  'service 1008': 'answered by the save conflict panel and its choices, not the line (tests/e2e/case-file-adr0737.spec.ts)',
  'service -32000': 'an engine fault, which no user action provokes on purpose',
  'workspace 1200': 'answered by the save conflict panel and its choices, not the line (tests/e2e/case-editor.spec.ts)',
  'mcp 1100': 'the Studio sends write tools only to its control server, whose policy allows them',
  // exit codes: the Studio reads run states and tool replies, not the exit codes of these programs, and the catalogue's
  // lines are looked up for JSON-RPC codes only (recoveryOf, src/renderer/help/engine.ts)
  cli_exit: 'an exit code of rcfd, which the Studio does not run for these',
  runner_exit: 'an exit code of rcfd-runner, which the Studio starts and talks to over its control files',
  mcp_exit: 'an exit code of rcfd-mcp, which the Studio starts with a project it has checked',
  mcp_hook_exit: 'an exit code of the Claude Code hook, which Claude Code runs, not the Studio',
};

let dir = '';
let quick = '';
let service: RunningRcfd;
let rpc: RpcConnection;
let studio: Studio;
let page: Page;
let catalogue: readonly Entry[] = [];

const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const line = (code: number): string => {
  const e = catalogue.find((x) => ['service', 'workspace', 'mcp'].includes(x.surface) && x.code === code);
  if (!e) throw new Error(`the catalogue has no JSON-RPC code ${code}`);
  return e.recovery;
};
const row = (p: string): Locator => page.locator(`.st-key[data-path="${p}"]`);
const sessionOfHash = async (): Promise<string> => decodeURIComponent((await page.evaluate(() => location.hash)).replace('#/case/', ''));

/** Types a value into a setting's field from the find box, then Enter (as a user does). */
async function typeInto(p: string, value: string): Promise<void> {
  const find = page.getByRole('textbox', { name: 'Find a setting' });
  await find.fill(p);
  const field = row(p).getByRole('textbox');
  await field.fill(value);
  await field.press('Enter');
}

async function openCase(rel: string): Promise<string> {
  await page.evaluate(() => (location.hash = ''));
  const edit = page.getByRole('button', { name: `Edit ${rel}`, exact: true });
  await expect(edit).toBeVisible({ timeout: 20_000 });
  await edit.click();
  await expect(page.getByRole('heading', { level: 2, name: rel, exact: true })).toBeVisible({ timeout: 30_000 });
  return sessionOfHash();
}

/** FNV-1a 64 of a string, as `rcfd_core::hash::fnv64` (the name of a file's edit lock, rcfd-project atomic.rs). */
function fnv64(s: string): string {
  let h = 0xcbf29ce484222325n;
  for (const b of new TextEncoder().encode(s)) h = BigInt.asUintN(64, (h ^ BigInt(b)) * 0x100000001b3n);
  return h.toString(16).padStart(16, '0');
}

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  dir = await scratch('f830-errors');
  quick = await copyExample('quick-start', dir);
  await writeFile(path.join(quick, 'cases', 'to-delete.toml'), await readFile(path.join(quick, CASE), 'utf8'));
  service = await startRcfd(rcfd!, 0, ['--ui']);
  rpc = await RpcConnection.connectTcp('127.0.0.1', service.port);
  catalogue = ((await rpc.request('help.errors', {})) as { errors: Entry[] }).errors;
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`, RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.locator(`tr[data-path="${CASE}"]`)).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  await studio?.app.close();
  rpc?.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('every row of the catalogue is provoked below or listed with the reason it is not', () => {
  const unaccounted = catalogue.filter((e) => {
    if (['service', 'workspace', 'mcp'].includes(e.surface)) return !(e.code in PROVOKED) && !(`${e.surface} ${e.code}` in NOT_PROVOKED);
    return !(e.surface in NOT_PROVOKED);
  });
  expect(unaccounted.map((e) => `${e.surface} ${e.code} ${e.name}`)).toEqual([]);
  // a provoked code is one the catalogue has, with a line
  for (const code of Object.keys(PROVOKED).map(Number)) expect(line(code), String(code)).not.toBe('');
  // nothing is in both lists
  expect(Object.keys(PROVOKED).filter((c) => catalogue.some((e) => e.code === Number(c) && `${e.surface} ${c}` in NOT_PROVOKED))).toEqual([]);
});

test('1003 VALIDATION: a value the engine refuses names the problem and the catalogue’s way out', async () => {
  await openCase(CASE);
  await typeInto('physics.nu', '-1');
  const error = row('physics.nu').locator('.st-key-error');
  await expect(error).toContainText('physics.nu', { timeout: 20_000 });
  await expect(error).toContainText(line(1003));
  await shot('1003-validation');
});

test('1002 BAD_STATE: a setting changed while another writer holds the run paused', async () => {
  const session = await sessionOfHash();
  await rpc.request('run.start', { session });
  await rpc.request('run.pause', { session });
  try {
    await typeInto('physics.nu', '0.02');
    const error = row('physics.nu').locator('.st-key-error');
    await expect(error).toContainText('stop the run before changing settings', { timeout: 20_000 });
    await expect(error).toContainText(line(1002));
    await shot('1002-bad-state');
  } finally {
    await rpc.request('run.stop', { session });
  }
});

test('1004 LIMIT: a mesh over the preview limit; Build anyway is offered', async () => {
  await openCase(CASE);
  await typeInto('mesh.nx', '1500');
  await typeInto('mesh.ny', '1500');
  const failed = page.locator('.mp-failed');
  await expect(failed).toContainText('more than the preview limit', { timeout: 60_000 });
  await expect(failed.locator('[data-recovery-code="1004"]')).toHaveText(line(1004));
  await expect(failed.getByRole('button', { name: 'Build anyway' })).toBeVisible();
  await shot('1004-limit');
  await typeInto('mesh.nx', '32');
  await typeInto('mesh.ny', '32');
  await expect(failed).toHaveCount(0, { timeout: 60_000 });
});

test('1201 LOCK_BUSY: Save while another writer holds the file’s edit lock', async () => {
  await typeInto('solver.max_iterations', '1234');
  const lock = path.join(quick, '.rcfd', 'locks', `${fnv64(CASE.toLowerCase())}.lock`);
  await mkdir(path.dirname(lock), { recursive: true });
  // a fresh lock of a live writer (this test's process): the engine waits, then gives up with 1201
  await writeFile(lock, `${process.pid} another-writer ${new Date().toISOString()}\n`);
  try {
    const before = await readFile(path.join(quick, CASE), 'utf8');
    await page.getByRole('button', { name: 'Save', exact: true }).click();
    const notice = page.locator('.ce-notice--failed');
    await expect(notice).toContainText('Failed to save', { timeout: 30_000 });
    await expect(notice.locator('[data-recovery-code="1201"]')).toHaveText(line(1201));
    expect(await readFile(path.join(quick, CASE), 'utf8')).toBe(before);
    await shot('1201-lock-busy');
    await notice.getByRole('button', { name: 'Dismiss' }).click();
  } finally {
    await unlink(lock).catch(() => undefined);
  }
  // the lock gone, the same save goes through
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText(new RegExp(`^Saved \\d+ settings? to ${CASE}$`))).toBeVisible({ timeout: 20_000 });
  expect(await readFile(path.join(quick, CASE), 'utf8')).toMatch(/max_iterations = 1234/);
});

test('1001 NO_SESSION: the case editor’s session closed by another writer', async () => {
  const session = await openCase(CASE);
  await rpc.request('session.close', { session });
  // the service's event makes the editor read the session again; the engine answers it is gone
  const gone = page.getByRole('main', { name: 'The settings could not be read' });
  await expect(gone).toContainText(`no session '${session}'`, { timeout: 20_000 });
  await expect(gone.locator('[data-recovery-code="1001"]')).toHaveText(line(1001));
  await shot('1001-no-session');
});

test('1202 NOT_FOUND: Run of a case file deleted under the Studio', async () => {
  await page.evaluate(() => (location.hash = ''));
  await page.getByRole('tab', { name: 'Runs' }).click();
  const bar = page.getByRole('region', { name: 'Start a run' });
  await bar.getByRole('button', { name: /Case to run/ }).click();
  await page.getByRole('option', { name: 'to-delete.toml' }).click();
  const run = bar.getByRole('button', { name: 'Run to-delete.toml' });
  await run.focus();
  await unlink(path.join(quick, 'cases', 'to-delete.toml'));
  await page.keyboard.press('Enter');
  const alert = bar.getByRole('alert');
  await expect(alert.locator('.rc-problem')).toContainText('no such case file', { timeout: 20_000 });
  await expect(alert.locator('[data-recovery-code="1202"]')).toHaveText(line(1202));
  await shot('1202-not-found');
  await alert.getByRole('button', { name: 'Dismiss' }).click();
});
