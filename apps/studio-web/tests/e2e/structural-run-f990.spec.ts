// F-990 (ADR 0840 E1; F-977's E8 of ADR 0817): a structural run in a project, shown in the open Studio from the rcfd-mcp
// binary. The packaged Studio has the f990 project open: a copy of quick-start with cases/cantilever-hex8.toml, a byte
// copy of crates/rcfd-api/tests/solid_cases/cantilever_hex8.toml. The test is the MCP client: it spawns
// `rcfd-mcp --project <folder>`, runs the cantilever (start_run, wait_run) and calls show_view with that run and no layout.
// Within 1.6 s (4 periods of the feed poll, FEED_POLL_MS 400 ms: ADR 0188 C3-St) the canvas shows the structural layout's
// panels in order (case, the stored u_magnitude and von_mises, the report's displacement_max_magnitude and von_mises_max,
// the run's table) and its title; then each panel holds its view, each monitor the report's value.
//
// Needs rcfd-mcp, with rcfd-runner, rcfd and rcfd-solid-run beside it (tests/support/project.ts); without rcfd-mcp the
// tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { copyFileSync, readFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import readline from 'node:readline';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { copyExample, findMcp, mcpPids, NO_MCP, repoRoot, scratch, stubDialog } from '../support/project.js';
import { killPid } from '../support/rcfd.js';
import { endProjectRunner } from '../support/runner.js';
import { FEED_POLL_MS } from '../../src/shared/project.js';

const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial' });

/** ADR 0840 E1: 4 periods of the Studio's feed poll (ADR 0805 D5), as ADR 0817 E1. */
const LIMIT_MS = 4 * FEED_POLL_MS;
const CASE = 'cases/cantilever-hex8.toml';

let dir = '';
let project = '';
let studio: Studio;
let page: Page;
let client: McpClient | null = null;
let run = '';

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

type Want = { kind: string; field?: string; monitor?: string };

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

/** Whether every panel of view `id` holds its view, each monitor the report's value of `values`. */
function drawnInPage(a: { id: string; run: string; kase: string; values: Record<string, string> }): boolean {
  const root = document.querySelector(`[data-canvas-view="${a.id}"]`);
  if (!root) return false;
  return Array.from(root.querySelectorAll('[data-canvas-panel]')).every((p) => {
    switch (p.getAttribute('data-canvas-kind')) {
      case 'field':
        return !!p.querySelector('section.fv .fv-caption') && !p.querySelector('.fv-status');
      case 'monitor': {
        const name = p.getAttribute('data-canvas-monitor') ?? '';
        return !!p.querySelector('.plot-frame') && p.querySelector('.cv-series')?.getAttribute('data-last-value') === a.values[name];
      }
      case 'table':
        return !!p.querySelector(`tr[data-run="${a.run}"]`);
      case 'case':
        return !!p.querySelector(`tr[data-path="${a.kase}"]`);
      default:
        return false;
    }
  });
}

test.beforeAll(async () => {
  dir = await scratch('structural-f990');
  project = await copyExample('quick-start', dir);
  copyFileSync(path.join(repoRoot, 'crates', 'rcfd-api', 'tests', 'solid_cases', 'cantilever_hex8.toml'), path.join(project, CASE));
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await stubDialog(studio.app, project);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  client = await McpClient.start(mcp!, project);
});

test.afterAll(async () => {
  try {
    client?.close();
    await studio?.app.close();
    for (const p of mcpPids(project)) killPid(p);
  } finally {
    // ADR 0828: the runner start_run started is shut down (and killed only if still alive) before the folder is removed
    await endProjectRunner(project);
    await rm(dir, { recursive: true, force: true }).catch(() => undefined);
  }
});

test('E1 (F-977 E8): a structural run shown from its run in the open Studio', async () => {
  const t0 = Date.now();
  const started = await client!.ok('start_run', { case: CASE, reason: 'F-990 e2e: the cantilever' });
  run = started['run'] as string;
  let state = '';
  for (let i = 0; i < 10; i++) {
    const w = await client!.ok('wait_run', { run, timeout_s: 60 });
    state = w['state'] as string;
    if (w['terminal'] === true) break;
  }
  console.log(`E1: ${run} ${state} ${Date.now() - t0} ms after start_run`);
  expect(state).toBe('finished');
  const report = JSON.parse(readFileSync(path.join(project, 'runs', run, 'output', 'cantilever_hex8.json'), 'utf8')) as {
    displacement: { max_magnitude: number };
    stress: { von_mises_max: number };
  };
  const values = { displacement_max_magnitude: String(report.displacement.max_magnitude), von_mises_max: String(report.stress.von_mises_max) };
  const want: Want[] = [
    { kind: 'case' },
    { kind: 'field', field: 'u_magnitude' },
    { kind: 'field', field: 'von_mises' },
    { kind: 'monitor', monitor: 'displacement_max_magnitude' },
    { kind: 'monitor', monitor: 'von_mises_max' },
    { kind: 'table' },
  ];
  const r = await client!.ok('show_view', { view: { version: 1, run }, reason: 'F-990 e2e E1' });
  const t1 = Date.now();
  expect(r['studios']).toBe(1);
  const id = r['id'] as string;
  console.log(`E1: ${run} -> ${String(r['category'])} (${String(r['rule'])}), ${String(r['panels'])} panels`);
  expect(r['category']).toBe('structural');
  await page.waitForFunction(shownInPage, { id, want, title: 'Structural: ' }, { polling: 10, timeout: 20_000 });
  const ms = Date.now() - t1;
  console.log(`E1: shown with its ${want.length} registered panels ${ms} ms after the tool's result (limit ${LIMIT_MS} ms)`);
  await page.waitForFunction(drawnInPage, { id, run, kase: CASE, values }, { polling: 50, timeout: 30_000 });
  console.log(`E1: every panel holds its view, the monitors ${JSON.stringify(values)}, ${Date.now() - t1} ms after the tool's result`);
  expect(ms).toBeLessThanOrEqual(LIMIT_MS);
});
