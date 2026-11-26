// F-976 (ADR 0805): Claude's canvas, driven from the rcfd-mcp binary. The packaged Studio has a copy of the quick-start
// project open (its reading server is the project's canvas, `--canvas`); the test is the MCP client Claude Code would be:
// it spawns `rcfd-mcp --project <folder>` and calls show_view.
//
// - E1: a view of the seven kinds is shown in the open Studio within 1.6 s of the tool's result (4 periods of the feed
//   poll, FEED_POLL_MS 400 ms: ADR 0188 C3-St), each panel holding its kind's existing view.
// - E2: an invalid view is refused with the key and the reason, and 1.6 s later the canvas still shows the first view.
// - E3: the canvas is drawn in the Studio's theme: the same font, ink and ground as the open project's view, and its
//   style sheet names tokens only.
//
// Needs rcfd-mcp (tests/support/project.ts); without it the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import readline from 'node:readline';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { copyExample, findMcp, mcpPids, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { appRoot, killPid } from '../support/rcfd.js';
import { FEED_POLL_MS } from '../../src/shared/project.js';

const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial' });

/** ADR 0805 D5: 4 periods of the Studio's feed poll. */
const LIMIT_MS = 4 * FEED_POLL_MS;
const RUN = 'r-20261028T152641-3900';
const OTHER = 'r-20261028T152641-5cc1';
const VIEW = {
  version: 1,
  title: 'Heated cavity',
  layout: { columns: 2 },
  panels: [
    { kind: 'case', case: 'cases/cavity-re100.toml' },
    { kind: 'run', run: RUN },
    { kind: 'field', run: RUN, field: 'T' },
    { kind: 'mesh', mesh: 'meshes/box.msh' },
    { kind: 'plot', run: RUN },
    { kind: 'monitor', run: RUN, monitor: 'nu_hot' },
    { kind: 'table', runs: [RUN, OTHER] },
  ],
};

let dir = '';
let quick = '';
let studio: Studio;
let page: Page;
let client: McpClient | null = null;
let firstId = '';

/** A minimal MCP client over stdio (as in claude-code.spec.ts): the part of Claude Code this test stands in for. */
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
  /** The tool's result: `{isError, structuredContent}`. */
  async call(name: string, args: Record<string, unknown>): Promise<{ isError: boolean; content: Record<string, unknown> }> {
    const m = await this.request('tools/call', { name, arguments: args });
    const r = m['result'] as { isError?: boolean; structuredContent?: Record<string, unknown> } | undefined;
    if (!r) throw new Error(`${name}: ${JSON.stringify(m).slice(0, 500)}`);
    return { isError: r.isError === true, content: r.structuredContent ?? {} };
  }
  close(): void {
    this.child.stdin.end();
    if (this.child.pid) killPid(this.child.pid);
  }
}

/** Whether the canvas shows view `id` with every panel's existing view in the document. */
function shownInPage(a: { id: string; run: string; other: string }): boolean {
  const root = document.querySelector(`[data-canvas-view="${a.id}"]`);
  if (!root || root.getAttribute('data-panels') !== '7') return false;
  const p = (k: string) => root.querySelector(`[data-canvas-kind="${k}"]`);
  return (
    !!p('case')?.querySelector('tr[data-path="cases/cavity-re100.toml"]') &&
    !!p('mesh')?.querySelector('tr[data-path="meshes/box.msh"]') &&
    !!p('table')?.querySelector(`tr[data-run="${a.run}"]`) &&
    !!p('table')?.querySelector(`tr[data-run="${a.other}"]`) &&
    (p('run')?.querySelector('h1')?.textContent ?? '') === `Run workspace: ${a.run}` &&
    !!p('field')?.querySelector('section.fv') &&
    !!p('plot')?.querySelector('.plot-frame') &&
    !!p('monitor')?.querySelector('.plot-frame')
  );
}

test.beforeAll(async () => {
  dir = await scratch('canvas-f976');
  quick = await copyExample('quick-start', dir);
  // a mesh entry of meshes/ (list_meshes lists the file; nothing reads it)
  writeFileSync(path.join(quick, 'meshes', 'box.msh'), '$MeshFormat\n2.2 0 8\n$EndMeshFormat\n');
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  client = await McpClient.start(mcp!, quick);
});

test.afterAll(async () => {
  client?.close();
  await studio?.app.close();
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('E1: show_view from the rcfd-mcp binary is shown by the open Studio with its existing views within 1.6 s', async () => {
  const r = await client!.call('show_view', { view: VIEW, reason: 'F-976 e2e' });
  const t0 = Date.now();
  expect(r.isError, JSON.stringify(r.content)).toBe(false);
  expect(r.content['studios']).toBe(1);
  firstId = r.content['id'] as string;
  await page.waitForFunction(shownInPage, { id: firstId, run: RUN, other: OTHER }, { polling: 10, timeout: 20_000 });
  const ms = Date.now() - t0;
  console.log(`E1: view ${firstId} shown with its 7 panels ${ms} ms after the tool's result (limit ${LIMIT_MS} ms)`);
  await expect(page.getByRole('tab', { name: 'Canvas' })).toHaveAttribute('aria-selected', 'true');
  expect(ms).toBeLessThanOrEqual(LIMIT_MS);
});

test('E2: an invalid view is refused with the key and the reason, and the shown view is unchanged', async () => {
  const bad = { ...VIEW, panels: [{ kind: 'chart' }, ...VIEW.panels.slice(1)] };
  const r = await client!.call('show_view', { view: bad });
  expect(r.isError).toBe(true);
  const e = r.content['error'] as { code: number; message: string; data: { errors: { key: string; reason: string }[] } };
  console.log(`E2: ${JSON.stringify(e)}`);
  expect(e.code).toBe(1003);
  expect(e.data.errors[0]!.key).toBe('panels[0].kind');
  expect(e.data.errors[0]!.reason).toContain("'chart' is not a kind");
  await page.waitForTimeout(LIMIT_MS);
  expect(await page.evaluate(shownInPage, { id: firstId, run: RUN, other: OTHER })).toBe(true);
  expect(await page.locator('[data-canvas-view]').getAttribute('data-canvas-view')).toBe(firstId);
});

test('E3: the canvas is drawn in the Studio theme', async () => {
  const style = (sel: string) =>
    page.locator(sel).first().evaluate((el) => {
      const s = getComputedStyle(el);
      return { font: s.fontFamily, color: s.color, ground: s.backgroundColor };
    });
  const canvas = await style('.cv');
  const home = await style('.ph-open');
  console.log(`E3: canvas ${JSON.stringify(canvas)}, open project ${JSON.stringify(home)}`);
  expect(canvas).toEqual(home);
  const css = readFileSync(path.join(appRoot, 'src', 'renderer', 'canvas', 'canvas.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi)).toBeNull();
});
