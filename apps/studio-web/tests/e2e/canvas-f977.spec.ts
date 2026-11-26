// F-977 (ADR 0817): the layout of results chosen by the category of the case, shown in the open Studio from the rcfd-mcp
// binary. The packaged Studio has the f977 project open: a copy of quick-start with the cylinder's case and run of
// wind-tunnel, the fan's case and run of axial-fan and the still pool of ADR 0049 (`pool_text` of
// crates/rcfd-api/tests/vof_tree.rs). The test is the MCP client: it spawns `rcfd-mcp --project <folder>`, runs the still
// pool (E0) and calls show_view with a run and no layout for each category's example run (E1 to E5) and for two negative
// controls (E6 a case of no known category, E7 a run without its field store). Each view is checked within 1.6 s (4
// periods of the feed poll, FEED_POLL_MS 400 ms: ADR 0188 C3-St) for its panels in order (kind, field, monitor) and its
// title, then for each panel's existing view. E8 (structural) cannot run: no project run of a solid case exists.
//
// Needs rcfd-mcp, with rcfd-runner and rcfd beside it (tests/support/project.ts); without rcfd-mcp the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { cp, rm } from 'node:fs/promises';
import readline from 'node:readline';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { copyExample, copyRun, findMcp, mcpPids, NO_MCP, repoRoot, scratch, stubDialog } from '../support/project.js';
import { killPid } from '../support/rcfd.js';
import { FEED_POLL_MS } from '../../src/shared/project.js';

const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial' });

/** ADR 0817 E1 to E7: 4 periods of the Studio's feed poll (ADR 0805 D5). */
const LIMIT_MS = 4 * FEED_POLL_MS;
const D5CB = 'r-20261028T152128-d5cb';
const FAN = 'r-20261030T223519-0489';
const F8C2 = 'r-20261028T152641-f8c2';
const HEAT = 'r-20261028T152641-3900';
const GENERAL = 'r-20261126T000000-b001';
const NO_FIELDS = 'r-20261126T000000-b002';

/** ADR 0817 (Checks): pool_text("", "") of vof_tree.rs with end_time 0.05 and an [output] block. */
const STILL_POOL = `
[mesh]
generator = "box_2d"
nx = 10
ny = 20
lx = 0.5
ly = 1.0
[physics]
gravity = [0.0, -9.81, 0.0]
[multiphase]
enabled = true
density_1 = 1000.0
density_2 = 1.0
viscosity_1 = 1.0e-3
viscosity_2 = 1.8e-5

[multiphase.initial]
shape = "half_space"
normal = [0.0, 1.0, 0.0]
point = [0.0, 0.5, 0.0]
[solver]
algorithm = "piso"
convection = "upwind"
n_correctors = 3
dt = 1.0e-3
end_time = 0.05

[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[output]
store_interval = 10
store_fields = ["alpha", "ux", "uy", "p"]
`;

let dir = '';
let project = '';
let studio: Studio;
let page: Page;
let client: McpClient | null = null;
let pool = '';

/** A minimal MCP client over stdio (as in canvas-f976.spec.ts). */
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
      const t = setTimeout(() => reject(new Error(`${method}: no answer in 60 s`)), 60_000);
      this.waiting.set(id, (m) => {
        clearTimeout(t);
        resolve(m);
      });
      this.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
    });
  }
  /** The tool's structured result; a tool error throws with its content. */
  async ok(name: string, args: Record<string, unknown>): Promise<Record<string, unknown>> {
    const m = await this.request('tools/call', { name, arguments: args });
    const r = m['result'] as { isError?: boolean; structuredContent?: Record<string, unknown> } | undefined;
    if (!r || r.isError === true) throw new Error(`${name}: ${JSON.stringify(m).slice(0, 800)}`);
    return r.structuredContent ?? {};
  }
  close(): void {
    this.child.stdin.end();
    if (this.child.pid) killPid(this.child.pid);
  }
}

/** One registered panel: its kind and the field or monitor it names. */
type Want = { kind: string; field?: string; monitor?: string };
const f = (field: string): Want => ({ kind: 'field', field });
const m = (monitor: string): Want => ({ kind: 'monitor', monitor });
const k = (kind: string): Want => ({ kind });

/** Whether the canvas shows view `id` with the panels `want` in order and a title starting `title`. */
function shownInPage(a: { id: string; want: Want[]; title: string }): boolean {
  const root = document.querySelector(`[data-canvas-view="${a.id}"]`);
  if (!root || root.getAttribute('data-panels') !== String(a.want.length)) return false;
  if (!(root.querySelector('.cv-title')?.textContent ?? '').startsWith(a.title)) return false;
  const panels = Array.from(root.querySelectorAll('[data-canvas-panel]'));
  return (
    panels.length === a.want.length &&
    panels.every((p, i) => {
      const w = a.want[i]!;
      return p.getAttribute('data-canvas-kind') === w.kind && (p.getAttribute('data-canvas-field') ?? undefined) === w.field && (p.getAttribute('data-canvas-monitor') ?? undefined) === w.monitor;
    })
  );
}

/** Whether every panel of view `id` holds its kind's existing view. */
function drawnInPage(a: { id: string; run: string; kase: string }): boolean {
  const root = document.querySelector(`[data-canvas-view="${a.id}"]`);
  if (!root) return false;
  return Array.from(root.querySelectorAll('[data-canvas-panel]')).every((p) => {
    switch (p.getAttribute('data-canvas-kind')) {
      case 'field':
        return !!p.querySelector('section.fv');
      case 'plot':
      case 'monitor':
        return !!p.querySelector('.plot-frame');
      case 'table':
        return !!p.querySelector(`tr[data-run="${a.run}"]`);
      case 'mesh':
        return !!p.querySelector('[data-drawn="true"]');
      case 'case':
        return !!p.querySelector(`tr[data-path="${a.kase}"]`);
      default:
        return false;
    }
  });
}

/** show_view of `run` with no layout: the registered panels within LIMIT_MS, then every panel's view. */
async function showRun(tag: string, run: string, kase: string, category: string, title: string, want: Want[]): Promise<void> {
  const r = await client!.ok('show_view', { view: { version: 1, run }, reason: `F-977 e2e ${tag}` });
  const t0 = Date.now();
  expect(r['studios']).toBe(1);
  const id = r['id'] as string;
  console.log(`${tag}: ${run} -> ${String(r['category'])} (${String(r['rule'])}), ${String(r['panels'])} panels`);
  expect(r['category']).toBe(category);
  await page.waitForFunction(shownInPage, { id, want, title }, { polling: 10, timeout: 20_000 });
  const ms = Date.now() - t0;
  console.log(`${tag}: shown with its ${want.length} registered panels ${ms} ms after the tool's result (limit ${LIMIT_MS} ms)`);
  await page.waitForFunction(drawnInPage, { id, run, kase }, { polling: 50, timeout: 30_000 });
  console.log(`${tag}: every panel holds its view ${Date.now() - t0} ms after the tool's result`);
  expect(ms).toBeLessThanOrEqual(LIMIT_MS);
}

test.beforeAll(async () => {
  dir = await scratch('canvas-f977');
  project = await copyExample('quick-start', dir);
  for (const [p, kase, run] of [
    ['wind-tunnel', 'cylinder-re100.toml', D5CB],
    ['axial-fan', 'fan01-passage-upwind.toml', FAN],
  ] as const) {
    const from = path.join(repoRoot, 'example-projects', p);
    await cp(path.join(from, 'cases', kase), path.join(project, 'cases', kase));
    await cp(path.join(from, 'runs', run), path.join(project, 'runs', run), { recursive: true });
  }
  writeFileSync(path.join(project, 'cases', 'still-pool.toml'), STILL_POOL);
  // E6: a run of a case of no known category (its snapshot names no boundaries); E7: a run without its field store
  await copyRun(project, F8C2, GENERAL);
  writeFileSync(path.join(project, 'runs', GENERAL, 'case.snapshot'), '[mesh]\ngenerator = "cavity_2d"\n');
  await copyRun(project, HEAT, NO_FIELDS);
  await rm(path.join(project, 'runs', NO_FIELDS, 'fields'), { recursive: true, force: true });
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await stubDialog(studio.app, project);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  client = await McpClient.start(mcp!, project);
});

test.afterAll(async () => {
  client?.close();
  await studio?.app.close();
  for (const p of mcpPids(project)) killPid(p);
  // the runner E0's start_run started (ADR 0807 addendum 1: it outlives the spec otherwise)
  const info = path.join(project, '.rcfd', 'runner.json');
  if (existsSync(info)) {
    const pid = (JSON.parse(readFileSync(info, 'utf8')) as { pid?: number }).pid;
    if (typeof pid === 'number') killPid(pid);
  }
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('E0: the still pool is run by the MCP client', async () => {
  const t0 = Date.now();
  const started = await client!.ok('start_run', { case: 'cases/still-pool.toml', reason: 'F-977 e2e: the multiphase example' });
  pool = started['run'] as string;
  let state = '';
  for (let i = 0; i < 10; i++) {
    const w = await client!.ok('wait_run', { run: pool, timeout_s: 60 });
    state = w['state'] as string;
    if (w['terminal'] === true) break;
  }
  console.log(`E0: ${pool} ${state} ${Date.now() - t0} ms after start_run`);
  expect(state).toBe('finished');
});

test('E1: external aerodynamics, the cylinder at Re 100', async () => {
  await showRun('E1', D5CB, 'cases/cylinder-re100.toml', 'external_aerodynamics', 'External aerodynamics: ', [f('ux'), f('p'), m('cd'), m('cl'), k('plot'), k('table')]);
});

test('E2: internal flow and turbomachinery, the fan passage', async () => {
  await showRun('E2', FAN, 'cases/fan01-passage-upwind.toml', 'internal_flow_and_turbomachinery', 'Internal flow and turbomachinery: ', [k('mesh'), f('ux'), f('p'), m('m_in'), m('m_out'), m('p_in'), k('plot')]);
});

test('E3: internal flow and turbomachinery, the cavity at Re 100', async () => {
  await showRun('E3', F8C2, 'cases/cavity-re100.toml', 'internal_flow_and_turbomachinery', 'Internal flow and turbomachinery: ', [k('mesh'), f('ux'), f('p'), m('lid_drag'), k('plot')]);
});

test('E4: heat transfer, the heated cavity at Ra 1e4', async () => {
  await showRun('E4', HEAT, 'cases/heated-cavity-ra1e4.toml', 'heat_transfer', 'Heat transfer: ', [f('T'), f('ux'), m('q_hot'), m('nu_hot'), k('plot'), k('table')]);
});

test('E5: multiphase, the still pool', async () => {
  await showRun('E5', pool, 'cases/still-pool.toml', 'multiphase', 'Multiphase: ', [f('alpha'), f('ux'), f('p'), k('plot'), k('table')]);
});

test('E6: a case of no known category takes the general layout', async () => {
  await showRun('E6', GENERAL, 'cases/cavity-re100.toml', 'general', 'General: ', [k('case'), k('plot'), k('table')]);
});

test('E7: a run without its field store has no field panel', async () => {
  await showRun('E7', NO_FIELDS, 'cases/heated-cavity-ra1e4.toml', 'heat_transfer', 'Heat transfer: ', [m('q_hot'), m('nu_hot'), k('plot'), k('table')]);
});
