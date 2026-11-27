// F-980 (ADR 0823): a view exported as one self-contained styled HTML page, read against the open Studio. The packaged
// Studio has the f980 project open: a copy of quick-start with meshes/box.msh, the cylinder's case and run of
// wind-tunnel and the fan's case and run of axial-fan. The test is the MCP client (`rcfd-mcp --project`): for each
// registered specification X1 to X5 it calls show_view, waits until every panel of the canvas holds its view, reads the
// Studio's values (ADR 0823 finding 3), calls export_view {} (the view shown last), opens the page in a new window of
// the Studio's Electron in a session of its own where every request other than the page's own file: URL is cancelled
// and counted, and reads the page's values (E1 to E5: equal, numbers within 1e-12 relative; 0 requests). E6: the page's
// canvas has the Studio's font, ink and ground, and export.css names no colour literal.
//
// Needs rcfd-mcp, with rcfd-runner and rcfd beside it (tests/support/project.ts); without rcfd-mcp the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { cp, rm } from 'node:fs/promises';
import readline from 'node:readline';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { copyExample, findMcp, mcpPids, NO_MCP, repoRoot, scratch, stubDialog } from '../support/project.js';
import { killPid } from '../support/rcfd.js';

const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial' });

const RUN = 'r-20261028T152641-3900';
const OTHER = 'r-20261028T152641-5cc1';
const D5CB = 'r-20261028T152128-d5cb';
const FAN = 'r-20261030T223519-0489';
const RE100 = 'cases/cavity-re100.toml';
/** ADR 0823 D3: tolerances.md, "Same discrete problem by two routes", where only rounding separates the two. */
const REL = 1e-12;

/** X1: ADR 0805 V2's view. */
const X1 = {
  version: 1,
  title: 'Cavity',
  layout: { columns: 2 },
  panels: [
    { kind: 'case', case: RE100 },
    { kind: 'run', run: RUN },
    { kind: 'field', run: RUN, field: 'p' },
    { kind: 'mesh', mesh: 'meshes/box.msh' },
    { kind: 'plot', run: RUN },
    { kind: 'monitor', run: RUN, monitor: 'nu_hot' },
    { kind: 'table', runs: [RUN, OTHER] },
  ],
};
/** X2: ADR 0807 V3's view. */
const X2 = {
  ...X1,
  panels: [X1.panels[0], X1.panels[2], X1.panels[3], X1.panels[5], { kind: 'mesh', case: RE100 }, { kind: 'setting', case: RE100, setting: 'physics.nu' }, { kind: 'monitor', case: RE100, monitor: 'lid_drag' }, { kind: 'table', runs: [RUN, OTHER] }],
};

let dir = '';
let project = '';
let studio: Studio;
let page: Page;
let client: McpClient | null = null;

/** A minimal MCP client over stdio (as in canvas-f977.spec.ts). */
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
      const t = setTimeout(() => reject(new Error(`${method}: no answer in 120 s`)), 120_000);
      this.waiting.set(id, (m) => {
        clearTimeout(t);
        resolve(m);
      });
      this.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
    });
  }
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

/** One panel's values (ADR 0823 D3): its kind and title, then the values of its kind. */
type Values = { kind: string; title: string; v: Record<string, string | null> };

/** The open Studio's values of view `id`, or null while a panel does not yet hold its view. */
function studioValues(id: string): Values[] | null {
  const root = document.querySelector(`[data-canvas-view="${id}"]`);
  if (!root) return null;
  const out: Values[] = [];
  for (const p of Array.from(root.querySelectorAll('[data-canvas-panel]'))) {
    const kind = p.getAttribute('data-canvas-kind') ?? '';
    const title = p.querySelector('.cv-panel-title')?.textContent ?? '';
    const v: Record<string, string | null> = {};
    switch (kind) {
      case 'field': {
        const f = p.querySelector('[data-ready][data-range]');
        if (!f) return null;
        const [lo, hi] = (f.getAttribute('data-range') ?? '').split(' ');
        Object.assign(v, { lo: lo ?? null, hi: hi ?? null, snapshot: f.getAttribute('data-snapshot'), step: f.getAttribute('data-step') });
        break;
      }
      case 'mesh': {
        const row = p.querySelector('tr[data-path]');
        if (row) v['path'] = row.getAttribute('data-path');
        else {
          const m = p.querySelector('.mp[data-triangles][data-drawn="true"]');
          if (!m) return null;
          v['triangles'] = m.getAttribute('data-triangles');
        }
        break;
      }
      case 'case': {
        const row = p.querySelector('tr[data-path]');
        if (!row) return null;
        v['path'] = row.getAttribute('data-path');
        break;
      }
      case 'plot': {
        const f = p.querySelector('.plot-frame');
        if (!f || (f.getAttribute('data-drawn') ?? '0') === '0') return null;
        Object.assign(v, { drawn: f.getAttribute('data-drawn'), skipped: f.getAttribute('data-skipped') });
        break;
      }
      case 'monitor': {
        const s = p.querySelector('.cv-series[data-last-value]');
        if (!s) return null;
        Object.assign(v, { step: s.getAttribute('data-last-step'), value: s.getAttribute('data-last-value') });
        break;
      }
      case 'run': {
        const ws = p.querySelector('.ws[data-run]');
        const dd = Array.from(p.querySelectorAll('.ws-grid > div > dd')).map((d) => (d.textContent ?? '').trim());
        if (!ws || dd.length < 2 || dd[1] === '…') return null;
        Object.assign(v, { run: ws.getAttribute('data-run'), step: dd[0] ?? null, snapshots: dd[1] ?? null });
        break;
      }
      case 'table': {
        const rows = Array.from(p.querySelectorAll('tr[data-run]'));
        if (rows.length === 0) return null;
        v['runs'] = rows.map((r) => r.getAttribute('data-run')).join(' ');
        v['steps'] = rows.map((r) => (r.querySelector('td.ph-num')?.textContent ?? '').trim()).join(' ');
        break;
      }
      case 'setting': {
        const s = p.querySelector('.cv-setting[data-value]');
        if (!s) return null;
        v['value'] = s.getAttribute('data-value');
        break;
      }
    }
    out.push({ kind, title, v });
  }
  return out;
}

/** The exported page's values, read the same way from its hooks (ADR 0823 D2). */
function pageValues(): Values[] {
  return Array.from(document.querySelectorAll('.cv-panel')).map((p) => {
    const kind = p.getAttribute('data-kind') ?? '';
    const title = p.querySelector('.cv-panel-title')?.textContent ?? '';
    const a = (sel: string, name: string) => p.querySelector(sel)?.getAttribute(name) ?? null;
    const v: Record<string, string | null> = {};
    switch (kind) {
      case 'field':
        Object.assign(v, { lo: a('.xv-field', 'data-lo'), hi: a('.xv-field', 'data-hi'), snapshot: a('.xv-field', 'data-snapshot'), step: a('.xv-field', 'data-step') });
        break;
      case 'mesh':
        if (p.querySelector('.xv-meshentry')) v['path'] = a('.xv-meshentry', 'data-path');
        else v['triangles'] = a('.xv-mesh', 'data-triangles');
        break;
      case 'case':
        v['path'] = a('.xv-case', 'data-path');
        break;
      case 'plot':
        Object.assign(v, { drawn: a('.xv-series', 'data-drawn'), skipped: a('.xv-series', 'data-skipped') });
        break;
      case 'monitor':
        Object.assign(v, { step: a('.xv-series', 'data-last-step'), value: a('.xv-series', 'data-last-value') });
        break;
      case 'run':
        Object.assign(v, { run: a('.xv-run', 'data-run'), step: a('.xv-run', 'data-step'), snapshots: a('.xv-run', 'data-snapshots') });
        break;
      case 'table': {
        const rows = Array.from(p.querySelectorAll('tr[data-run]'));
        v['runs'] = rows.map((r) => r.getAttribute('data-run')).join(' ');
        v['steps'] = rows.map((r) => r.getAttribute('data-step')).join(' ');
        break;
      }
      case 'setting':
        v['value'] = a('.cv-setting', 'data-value');
        break;
    }
    return { kind, title, v };
  });
}

/** The theme of the canvas root: font, ink and ground. */
function themeOf(sel: string): string[] {
  const el = document.querySelector(sel);
  if (!el) return [];
  const s = getComputedStyle(el);
  return [s.fontFamily, s.color, s.backgroundColor];
}

/** Whether two values agree: numbers within REL relative, anything else exactly. */
function agree(key: string, a: string | null, b: string | null): boolean {
  if (['lo', 'hi', 'value'].includes(key) && a !== null && b !== null) {
    const x = Number(a);
    const y = Number(b);
    return x === y || Math.abs(x - y) <= REL * Math.max(Math.abs(x), Math.abs(y));
  }
  return a === b;
}

/** The exported file opened offline: a new window in a session of its own, every request but the file's cancelled. */
async function openOffline(file: string): Promise<{ page: Page; requests: string[]; close: () => Promise<void> }> {
  const pending = studio.app.waitForEvent('window');
  await studio.app.evaluate(async ({ BrowserWindow, session }, f) => {
    const ses = session.fromPartition('f980-export-offline');
    const g = globalThis as unknown as { f980Requests: string[] };
    g.f980Requests = [];
    ses.webRequest.onBeforeRequest((d, cb) => {
      g.f980Requests.push(d.url);
      cb({ cancel: !d.url.startsWith('file:') });
    });
    const w = new BrowserWindow({ show: false, width: 1440, height: 900, webPreferences: { session: ses } });
    await w.loadFile(f);
  }, file);
  const pg = await pending;
  await pg.waitForLoadState('load');
  const requests = await studio.app.evaluate(() => (globalThis as unknown as { f980Requests: string[] }).f980Requests);
  return { page: pg, requests, close: () => pg.close() };
}

let themes: { studio: string[]; page: string[] } | null = null;

/** E1 to E5: show `view`, read the Studio, export, open offline, compare. */
async function compare(tag: string, view: Record<string, unknown>, panels: number): Promise<void> {
  const shown = await client!.ok('show_view', { view, reason: `F-980 e2e ${tag}` });
  const id = shown['id'] as string;
  const handle = await page.waitForFunction(studioValues, id, { polling: 100, timeout: 60_000 });
  const inStudio = (await handle.jsonValue()) as Values[];
  const t0 = Date.now();
  const r = await client!.ok('export_view', { reason: `F-980 e2e ${tag}` });
  const ms = Date.now() - t0;
  const file = path.join(project, r['path'] as string);
  console.log(`${tag}: exported ${String(r['path'])}, ${String(r['bytes'])} bytes, in ${ms} ms`);
  const off = await openOffline(file);
  const inPage = await off.page.evaluate(pageValues);
  const others = off.requests.filter((u) => u !== off.requests[0] || !u.startsWith('file:'));
  console.log(`${tag}: requests ${JSON.stringify(off.requests)}; other than the page's own: ${others.length}`);
  if (themes === null) themes = { studio: await page.evaluate(themeOf, `[data-canvas-view="${id}"]`), page: await off.page.evaluate(themeOf, '.cv') };
  await off.close();
  const misses: string[] = [];
  expect(inStudio.length).toBe(panels);
  expect(inPage.length).toBe(panels);
  inStudio.forEach((s, i) => {
    const p = inPage[i]!;
    if (s.kind !== p.kind) misses.push(`panel ${i}: kind ${s.kind} against ${p.kind}`);
    if (s.title !== p.title) misses.push(`panel ${i}: title ${s.title} against ${p.title}`);
    for (const [k, sv] of Object.entries(s.v)) {
      const pv = p.v[k] ?? null;
      const ok = agree(k, sv, pv);
      console.log(`${tag} panel ${i} ${s.kind} ${k}: Studio ${String(sv)}, page ${String(pv)}${ok ? '' : ' MISS'}`);
      if (!ok) misses.push(`panel ${i} ${s.kind} ${k}: Studio ${String(sv)}, page ${String(pv)}`);
    }
  });
  console.log(`${tag}: ${misses.length} values differ`);
  expect(misses).toEqual([]);
  expect(others).toEqual([]);
}

test.beforeAll(async () => {
  dir = await scratch('export-f980');
  project = await copyExample('quick-start', dir);
  writeFileSync(path.join(project, 'meshes', 'box.msh'), '$MeshFormat\n2.2 0 8\n$EndMeshFormat\n');
  for (const [p, kase, run] of [
    ['wind-tunnel', 'cylinder-re100.toml', D5CB],
    ['axial-fan', 'fan01-passage-upwind.toml', FAN],
  ] as const) {
    const from = path.join(repoRoot, 'example-projects', p);
    await cp(path.join(from, 'cases', kase), path.join(project, 'cases', kase));
    await cp(path.join(from, 'runs', run), path.join(project, 'runs', run), { recursive: true });
  }
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
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('E1: the seven kinds of ADR 0805 V2', async () => {
  await compare('E1', X1, 7);
});

test('E2: the live forms of ADR 0807 V3', async () => {
  await compare('E2', X2, 8);
});

test('E3: the cylinder run, external aerodynamics', async () => {
  await compare('E3', { version: 1, run: D5CB }, 6);
});

test('E4: the heated cavity run, heat transfer', async () => {
  await compare('E4', { version: 1, run: RUN }, 6);
});

test('E5: the fan run, a 3D store', async () => {
  await compare('E5', { version: 1, run: FAN }, 7);
});

test('E6: the page in the Studio theme, export.css in tokens only', () => {
  console.log(`E6: Studio ${JSON.stringify(themes?.studio)}, page ${JSON.stringify(themes?.page)}`);
  expect(themes).not.toBeNull();
  expect(themes!.page).toEqual(themes!.studio);
  const css = readFileSync(path.join(repoRoot, 'apps', 'studio-web', 'src', 'renderer', 'canvas', 'export.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const literals = css.match(/#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/g) ?? [];
  console.log(`E6: colour literals in export.css: ${literals.length}`);
  expect(literals).toEqual([]);
});
