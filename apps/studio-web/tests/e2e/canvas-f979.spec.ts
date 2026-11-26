// F-979 (ADR 0807): Claude's canvas, live, driven from the rcfd-mcp binary. The packaged Studio has a copy of the
// quick-start project open (its reading server is the project's canvas); the test is the MCP client Claude Code would be:
// it spawns `rcfd-mcp --project <folder>`, shows a view of a mesh, two settings and two monitors, then refines the mesh,
// edits a setting with a reason and runs a short case, and reads each change in the open Studio.
//
// - E0: the view is shown: the mesh of cases/cavity-re100.toml drawn (1,024 cells), its physics.nu 0.01 and that of
//   cavity-re400 0.0025, and the lid_drag monitor of the newest run of the case and of a fixed run, each equal to the
//   engine's last point (read_residuals).
// - E1: refine_case_mesh with a reason: within 1.6 s the mesh is drawn again (another mesh hash, 4,096 cells) and its
//   change line names edit_case, the client mcp with its pid, and the reason.
// - E2: edit_case of physics.nu with a reason: within 1.6 s the setting shows 0.005 with its change (old and new, actor,
//   reason); the other case's setting and the mesh are unchanged.
// - E3: start_run with max_steps 300: (a) within 1.6 s the monitor follows the new run; (b) it shows a step of the run
//   while it runs; (c) within 1.6 s of the client reading the run's end, its final step and value equal the engine's,
//   with the runner and the reason of its last state change; the fixed run's monitor is unchanged.
//
// Every limit is 4 periods of the Studio's feed poll (FEED_POLL_MS 400 ms): ADR 0188 C3-St, as ADR 0807 D4 registers it.
// Needs rcfd-mcp, with rcfd-runner and rcfd beside it (tests/support/project.ts); without rcfd-mcp the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import readline from 'node:readline';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { copyExample, findMcp, mcpPids, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { killPid } from '../support/rcfd.js';
import { FEED_POLL_MS } from '../../src/shared/project.js';

const mcp = findMcp();
test.skip(!mcp, NO_MCP);
// ADR 0807 amendment 1: the record of the first set-up, whose refine_case_mesh call left out `factor` (error -32602);
// canvas-f979-a1.spec.ts is the corrected test.
test.skip(true, 'ADR 0807 amendment 1: the defective set-up of E1 (no factor); see canvas-f979-a1.spec.ts');
test.describe.configure({ mode: 'serial' });

/** ADR 0807 D4: 4 periods of the Studio's feed poll. */
const LIMIT_MS = 4 * FEED_POLL_MS;
const RE100 = 'cases/cavity-re100.toml';
const RE400 = 'cases/cavity-re400.toml';
const FIXED = 'r-20261028T152641-f8c2';
const VIEW = {
  version: 1,
  title: 'Live cavity',
  layout: { columns: 2 },
  panels: [
    { kind: 'mesh', case: RE100 },
    { kind: 'setting', case: RE100, setting: 'physics.nu' },
    { kind: 'monitor', case: RE100, monitor: 'lid_drag' },
    { kind: 'setting', case: RE400, setting: 'physics.nu' },
    { kind: 'monitor', run: FIXED, monitor: 'lid_drag' },
  ],
};

let dir = '';
let quick = '';
let studio: Studio;
let page: Page;
let client: McpClient | null = null;
let pid = 0;
let meshHash = '';
let meshBuild = '';
let fixedLast = '';
const refineReason = 'F-979 e2e: refine the cavity x2';

/** A minimal MCP client over stdio (as in canvas-f976.spec.ts): the part of Claude Code this test stands in for. */
class McpClient {
  private n = 0;
  private readonly waiting = new Map<number, (m: Record<string, unknown>) => void>();
  private constructor(readonly child: ChildProcessWithoutNullStreams) {
    readline.createInterface({ input: child.stdout }).on('line', (l) => {
      const m = JSON.parse(l) as Record<string, unknown>;
      const w = this.waiting.get(m['id'] as number);
      if (w) {
        this.waiting.delete(m['id'] as number);
        w(m);
      }
    });
  }
  static async start(exe: string, folder: string): Promise<McpClient> {
    const c = new McpClient(spawn(exe, ['--project', folder], { cwd: folder, windowsHide: true }));
    await c.request('initialize', { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'claude-code-stand-in', version: '0' } });
    c.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' })}\n`);
    return c;
  }
  request(method: string, params: unknown): Promise<Record<string, unknown>> {
    const id = ++this.n;
    return new Promise((resolve, reject) => {
      const t = setTimeout(() => reject(new Error(`${method}: no answer in 90 s`)), 90_000);
      this.waiting.set(id, (m) => {
        clearTimeout(t);
        resolve(m);
      });
      this.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
    });
  }
  /** The tool's structured result; an error result fails the test with the engine's words. */
  async ok(name: string, args: Record<string, unknown>): Promise<Record<string, unknown>> {
    const m = await this.request('tools/call', { name, arguments: args });
    const r = m['result'] as { isError?: boolean; structuredContent?: Record<string, unknown> } | undefined;
    if (!r || r.isError) throw new Error(`${name}: ${JSON.stringify(m).slice(0, 800)}`);
    return r.structuredContent ?? {};
  }
  close(): void {
    this.child.stdin.end();
    if (this.child.pid) killPid(this.child.pid);
  }
}

/** The engine's last point of a run's monitor, as read_residuals gives it. */
async function lastPoint(run: string, monitor: string): Promise<{ step: number; value: string }> {
  const r = await client!.ok('read_residuals', { run });
  const s = (r['series'] as Record<string, { x: number[]; y: (number | null)[] }>)[`monitors.${monitor}`]!;
  return { step: s.x[s.x.length - 1]!, value: String(s.y[s.y.length - 1] ?? NaN) };
}

const panel = (i: number) => `[data-canvas-panel="${i}"]`;

/** E0: the view in the page (run in the page). */
function shownInPage(a: { fixed: string; last: string }): boolean {
  const q = (s: string) => document.querySelector(s);
  const root = q('[data-canvas-view]');
  if (!root || root.getAttribute('data-panels') !== '5') return false;
  const mp = q('[data-canvas-panel="0"] .mp');
  const facts = q('[data-canvas-panel="0"] .mp-facts')?.textContent ?? '';
  const s1 = q('[data-canvas-panel="1"] [data-setting]');
  const s3 = q('[data-canvas-panel="3"] [data-setting]');
  const m2 = q('[data-canvas-panel="2"] .cv-series');
  const m4 = q('[data-canvas-panel="4"] .cv-series');
  return (
    mp?.getAttribute('data-drawn') === 'true' &&
    facts.startsWith('1,024 cells') &&
    s1?.getAttribute('data-value') === '0.01' &&
    s3?.getAttribute('data-value') === '0.0025' &&
    m2?.getAttribute('data-run') === a.fixed &&
    m2?.getAttribute('data-last-value') === a.last &&
    m4?.getAttribute('data-last-value') === a.last
  );
}

test.beforeAll(async () => {
  dir = await scratch('canvas-f979');
  quick = await copyExample('quick-start', dir);
  writeFileSync(path.join(quick, 'meshes', 'box.msh'), '$MeshFormat\n2.2 0 8\n$EndMeshFormat\n');
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  client = await McpClient.start(mcp!, quick);
  pid = client.child.pid!;
});

test.afterAll(async () => {
  client?.close();
  await studio?.app.close();
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('E0: the live view is shown with the mesh drawn, the settings and the monitors as the engine gives them', async () => {
  const fixed = await lastPoint(FIXED, 'lid_drag');
  fixedLast = fixed.value;
  const r = await client!.ok('show_view', { view: VIEW, reason: 'F-979 e2e' });
  const t0 = Date.now();
  expect(r['studios']).toBe(1);
  await page.waitForFunction(shownInPage, { fixed: FIXED, last: fixed.value }, { polling: 10, timeout: 30_000 });
  console.log(`E0: view ${String(r['id'])} shown with its mesh drawn ${Date.now() - t0} ms after the tool's result (recorded)`);
  meshHash = (await page.locator(`${panel(0)} .mp`).getAttribute('data-mesh-hash')) ?? '';
  meshBuild = (await page.locator(`${panel(0)} .cv-mesh`).getAttribute('data-mesh-build')) ?? '';
  expect(meshHash).not.toBe('');
});

test('E1: a mesh refined by an MCP tool is drawn again within 1.6 s, with its actor and reason', async () => {
  const hash = (await client!.ok('read_project_file', { path: RE100 }))['hash'];
  const r = await client!.ok('refine_case_mesh', { case: RE100, base_hash: hash, reason: refineReason });
  const t0 = Date.now();
  const cells = r['cells_after'] as number;
  expect(cells).toBe(4096);
  await page.waitForFunction(
    (a) => {
      const p = document.querySelector('[data-canvas-panel="0"]');
      const mp = p?.querySelector('.mp');
      const line = p?.querySelector('.cv-change');
      return (
        !!mp &&
        !!mp.getAttribute('data-mesh-hash') &&
        mp.getAttribute('data-mesh-hash') !== a.old &&
        mp.getAttribute('data-drawn') === 'true' &&
        (p?.querySelector('.mp-facts')?.textContent ?? '').startsWith(`${a.cells.toLocaleString('en-US')} cells`) &&
        line?.getAttribute('data-who') === 'edit_case' &&
        line?.getAttribute('data-actor') === 'mcp' &&
        line?.getAttribute('data-pid') === String(a.pid) &&
        line?.getAttribute('data-reason') === a.reason
      );
    },
    { old: meshHash, cells, pid, reason: refineReason },
    { polling: 10, timeout: 30_000 },
  );
  const ms = Date.now() - t0;
  console.log(`E1: the refined mesh (${cells} cells) drawn ${ms} ms after the tool's result (limit ${LIMIT_MS} ms)`);
  meshHash = (await page.locator(`${panel(0)} .mp`).getAttribute('data-mesh-hash')) ?? '';
  meshBuild = (await page.locator(`${panel(0)} .cv-mesh`).getAttribute('data-mesh-build')) ?? '';
  expect(ms).toBeLessThanOrEqual(LIMIT_MS);
});

test('E2: a setting edited by an MCP client is updated within 1.6 s with its change; the others are not', async () => {
  const reason = 'F-979 e2e: Re 200';
  const hash = (await client!.ok('read_project_file', { path: RE100 }))['hash'];
  await client!.ok('edit_case', { path: RE100, base_hash: hash, set: { 'physics.nu': 0.005 }, reason });
  const t0 = Date.now();
  await page.waitForFunction(
    (a) => {
      const p = document.querySelector('[data-canvas-panel="1"]');
      const line = p?.querySelector('.cv-change');
      return (
        p?.querySelector('[data-setting]')?.getAttribute('data-value') === '0.005' &&
        line?.getAttribute('data-who') === 'edit_case' &&
        line?.getAttribute('data-actor') === 'mcp' &&
        line?.getAttribute('data-pid') === String(a.pid) &&
        line?.getAttribute('data-reason') === a.reason &&
        line?.getAttribute('data-old') === '0.01' &&
        line?.getAttribute('data-new') === '0.005'
      );
    },
    { pid, reason },
    { polling: 10, timeout: 30_000 },
  );
  const ms = Date.now() - t0;
  // the controls, at the same moment
  const other = await page.locator(`${panel(3)} [data-setting]`).getAttribute('data-value');
  const otherLines = await page.locator(`${panel(3)} .cv-change`).count();
  const build = await page.locator(`${panel(0)} .cv-mesh`).getAttribute('data-mesh-build');
  const meshReason = await page.locator(`${panel(0)} .cv-change`).getAttribute('data-reason');
  console.log(`E2: physics.nu 0.005 shown ${ms} ms after the tool's result (limit ${LIMIT_MS} ms); controls: re400 ${other}, ${otherLines} change line(s); mesh build ${build} (was ${meshBuild}), its reason ${meshReason}`);
  expect(other).toBe('0.0025');
  expect(otherLines).toBe(0);
  expect(build).toBe(meshBuild);
  expect(meshReason).toBe(refineReason);
  expect(ms).toBeLessThanOrEqual(LIMIT_MS);
});

test('E3: a short run started by an MCP client is followed by the monitor, and its end is shown within 1.6 s', async () => {
  const reason = 'F-979 e2e: a short run';
  const cursor = (await client!.ok('read_feed', {}))['cursor'] as number;
  const started = await client!.ok('start_run', { case: RE100, max_steps: 300, reason });
  const t0 = Date.now();
  const run = started['run'] as string;
  // (a) the monitor follows the new run
  await page.waitForFunction((id) => document.querySelector('[data-canvas-panel="2"] .cv-series')?.getAttribute('data-run') === id, run, { polling: 10, timeout: 30_000 });
  const followMs = Date.now() - t0;
  console.log(`E3a: the monitor follows ${run} ${followMs} ms after start_run's result (limit ${LIMIT_MS} ms)`);
  // (b) the steps the monitor shows while the run runs, sampled every 50 ms until the client reads the run's end
  const seen: { step: number | null; state: string | null; at: number }[] = [];
  let sampling = true;
  const sampler = (async () => {
    while (sampling) {
      const s = await page
        .evaluate(() => {
          const e = document.querySelector('[data-canvas-panel="2"] .cv-series');
          const step = e?.getAttribute('data-last-step');
          return { step: step ? Number(step) : null, state: e?.getAttribute('data-state') ?? null };
        })
        .catch(() => ({ step: null, state: null }));
      seen.push({ ...s, at: Date.now() });
      await new Promise((r) => setTimeout(r, 50));
    }
  })();
  let state = '';
  let tEnd = 0;
  let endMs = 0;
  let last = { step: 0, value: '' };
  let endReason: string | null = null;
  try {
    for (let i = 0; i < 10; i++) {
      const w = await client!.ok('wait_run', { run, timeout_s: 60 });
      state = w['state'] as string;
      if (w['terminal'] === true) break;
    }
    tEnd = Date.now();
    last = await lastPoint(run, 'lid_drag');
    // the run's state changes as the client's own feed has them, until it holds the last (within the measured time)
    const states: Record<string, unknown>[] = [];
    let since = cursor;
    for (let i = 0; i < 50 && !states.some((e) => e['to'] === state); i++) {
      const feed = await client!.ok('read_feed', { since, limit: 200 });
      const entries = (feed['entries'] as Record<string, unknown>[]) ?? [];
      states.push(...entries.filter((e) => e['kind'] === 'run_state' && e['run'] === run));
      if (typeof feed['next'] === 'number') since = feed['next'];
      else await new Promise((r) => setTimeout(r, 100));
    }
    endReason = (states[states.length - 1]?.['reason'] as string | undefined) ?? null;
    // (c) the end, from the client's reading of it
    await page.waitForFunction(
      (a) => {
        const e = document.querySelector('[data-canvas-panel="2"] .cv-series');
        const line = document.querySelector('[data-canvas-panel="2"] .cv-change');
        return (
          e?.getAttribute('data-state') === a.state &&
          e?.getAttribute('data-last-step') === String(a.step) &&
          e?.getAttribute('data-last-value') === a.value &&
          line?.getAttribute('data-actor') === 'runner' &&
          line?.getAttribute('data-reason') === a.reason
        );
      },
      { state, step: last.step, value: last.value, reason: endReason },
      { polling: 10, timeout: 30_000 },
    );
    endMs = Date.now() - tEnd;
  } finally {
    sampling = false;
    await sampler;
  }
  const during = seen.filter((s) => s.at < tEnd).map((s) => s.step);
  const between = [...new Set(during.filter((s): s is number => s !== null && s > 0 && s < last.step))];
  const fixedNow = await page.locator(`${panel(4)} .cv-series`).getAttribute('data-last-value');
  console.log(
    `E3b: steps shown while ${run} ran (final ${last.step}): ${between.length} [${between.join(', ')}]; E3c: ${state} at step ${last.step}, lid_drag ${last.value}, shown ${endMs} ms after the client read the end (limit ${LIMIT_MS} ms), reason ${endReason}; fixed run ${fixedNow} (was ${fixedLast})`,
  );
  expect(followMs).toBeLessThanOrEqual(LIMIT_MS);
  expect(between.length).toBeGreaterThanOrEqual(1);
  expect(fixedNow).toBe(fixedLast);
  expect(endMs).toBeLessThanOrEqual(LIMIT_MS);
});
