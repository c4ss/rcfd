// F-978 (ADR 0826): the process view of a case on Claude's canvas, driven from the rcfd-mcp binary. The packaged Studio
// has a copy of the quick-start project open; the test is the MCP client Claude Code would be: it spawns
// `rcfd-mcp --project <folder>`, builds a case from new to a finished run (new_case, a mesh edit, a set-up edit,
// start_run and wait_run, a set-up edit after the run), and shows the case's process.
//
// - E1: within 1.6 s of show_view's result the list is shown; its journal steps' data-seq equal, in order, the seq of
//   the lines of .rcfd/edits.jsonl that concern the case (read here by rules (a) to (c) of ADR 0826 D2); its steps equal
//   read_view's process steps one for one; its times never decrease; the residuals step's values are read_residuals'
//   last points.
// - E2: each step picked in turn opens the view of its state: as many panels as its `opens`, each of its kind with that
//   kind's existing view in it.
// - E3: one more edit_case with a reason: within 1.6 s a new last step `setup` with that line's seq and reason.
//
// Each time limit is 4 periods of the Studio's feed poll (FEED_POLL_MS 400 ms): ADR 0188 C3-St, as ADR 0826 registers it.
// Needs rcfd-mcp, with rcfd-runner and rcfd beside it (tests/support/project.ts); without rcfd-mcp the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import readline from 'node:readline';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { copyExample, findMcp, mcpPids, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { killPid } from '../support/rcfd.js';
import { FEED_POLL_MS } from '../../src/shared/project.js';

const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial' });

/** ADR 0826: 4 periods of the Studio's feed poll. */
const LIMIT_MS = 4 * FEED_POLL_MS;
const CASE = 'cases/process-cavity.toml';
const VIEW = { version: 1, title: 'Process of the cavity', panels: [{ kind: 'process', case: CASE }] };

type Json = Record<string, unknown>;
interface Step {
  i: number;
  step: string;
  phase: string;
  source: string;
  run?: string;
  seq?: number;
  t: string | null;
  text: string;
  reason?: string;
  last?: Record<string, [number, number | null]>;
  opens: { panels: Json[] } | null;
}

let dir = '';
let quick = '';
let studio: Studio;
let page: Page;
let client: McpClient | null = null;
let run = '';
let steps: Step[] = [];

/** A minimal MCP client over stdio (as in canvas-f979-a1.spec.ts): the part of Claude Code this test stands in for. */
class McpClient {
  private n = 0;
  private readonly waiting = new Map<number, (m: Json) => void>();
  private constructor(readonly child: ChildProcessWithoutNullStreams) {
    readline.createInterface({ input: child.stdout }).on('line', (l) => {
      const m = JSON.parse(l) as Json;
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
  request(method: string, params: unknown): Promise<Json> {
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
  async ok(name: string, args: Json): Promise<Json> {
    const m = await this.request('tools/call', { name, arguments: args });
    const r = m['result'] as { isError?: boolean; structuredContent?: Json } | undefined;
    if (!r || r.isError) throw new Error(`${name}: ${JSON.stringify(m).slice(0, 800)}`);
    return r.structuredContent ?? {};
  }
  close(): void {
    this.child.stdin.end();
    if (this.child.pid) killPid(this.child.pid);
  }
}

async function edit(set: Json, reason: string): Promise<void> {
  const hash = (await client!.ok('read_project_file', { path: CASE }))['hash'];
  await client!.ok('edit_case', { path: CASE, base_hash: hash, set, reason });
}

/** The journal, read here: the lines of the case by rules (a) to (c) of ADR 0826 D2. */
function journalSeqs(): number[] {
  const text = readFileSync(path.join(quick, '.rcfd', 'edits.jsonl'), 'utf8');
  const lines = text.split('\n').flatMap((l) => {
    try {
      return [JSON.parse(l) as Json];
    } catch {
      return [];
    }
  });
  const runs = new Set(lines.filter((l) => l['kind'] === 'run_submitted' && l['case'] === CASE).map((l) => l['run'] as string));
  return lines.filter((l) => l['path'] === CASE || (l['kind'] === 'run_submitted' && l['case'] === CASE) || (typeof l['run'] === 'string' && runs.has(l['run']))).map((l) => l['seq'] as number);
}

/** A time as this test compares it: the text before the fraction, then the fraction as nanoseconds. */
function tKey(t: string): [string, number] {
  const body = t.replace(/Z$/, '');
  const [s, f = ''] = body.split('.');
  return [s!, Number(f.padEnd(9, '0'))];
}

/** The steps as the page lists them. */
function listed(p: Page): Promise<{ i: number; kind: string; phase: string; source: string; run: string | null; seq: number | null; t: string | null; text: string; facts: string[] }[]> {
  return p.evaluate(() =>
    [...document.querySelectorAll('[data-canvas-kind="process"] li[data-step]')].map((li) => ({
      i: Number(li.getAttribute('data-step')),
      kind: li.getAttribute('data-step-kind') ?? '',
      phase: li.getAttribute('data-phase') ?? '',
      source: li.getAttribute('data-source') ?? '',
      run: li.getAttribute('data-run'),
      seq: li.getAttribute('data-seq') === null ? null : Number(li.getAttribute('data-seq')),
      t: li.getAttribute('data-t'),
      text: li.querySelector('button')?.textContent ?? '',
      facts: [...li.querySelectorAll('.cv-step-fact')].map((f) => f.textContent ?? ''),
    })),
  );
}

test.beforeAll(async () => {
  dir = await scratch('canvas-f978');
  quick = await copyExample('quick-start', dir);
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

test('E1: a case built from new to a finished run lists every journaled step in order, none missing', async () => {
  test.setTimeout(240_000);
  await client!.ok('new_case', { name: 'process-cavity', copy: 'cases/cavity-re100.toml', reason: 'F-978 e2e: new' });
  await edit({ 'mesh.nx': 16, 'mesh.ny': 16 }, 'F-978 e2e: mesh');
  await edit({ 'physics.nu': 0.02 }, 'F-978 e2e: set-up');
  run = (await client!.ok('start_run', { case: CASE, max_steps: 40, reason: 'F-978 e2e: run' }))['run'] as string;
  let end: Json = {};
  for (let i = 0; i < 10; i++) {
    end = await client!.ok('wait_run', { run, timeout_s: 60 });
    if (end['terminal'] === true) break;
  }
  await edit({ 'solver.max_iterations': 500 }, 'F-978 e2e: after the run');
  const shown = await client!.ok('show_view', { view: VIEW, reason: 'F-978 e2e' });
  const t0 = Date.now();
  expect(shown['studios']).toBe(1);
  const read = await client!.ok('read_view', {});
  steps = ((read['process'] as Json[])[0]!['steps'] as Step[]) ?? [];
  const lastSeq = [...steps].reverse().find((s) => s.seq !== undefined)?.seq;
  await page.waitForFunction(
    (a) => {
      const ol = document.querySelector('[data-canvas-kind="process"] ol.cv-process');
      const items = ol?.querySelectorAll('li[data-step]') ?? [];
      return ol?.getAttribute('data-steps') === String(a.n) && items.length === a.n && items[a.n - 1]?.getAttribute('data-seq') === String(a.seq);
    },
    { n: steps.length, seq: lastSeq },
    { polling: 10, timeout: 30_000 },
  );
  const ms = Date.now() - t0;
  const dom = await listed(page);
  const want = journalSeqs();
  const domSeqs = dom.filter((s) => s.source === 'journal').map((s) => s.seq);
  const same = dom.length === steps.length && dom.every((d, k) => {
    const s = steps[k]!;
    return d.i === s.i && d.kind === s.step && d.phase === s.phase && d.run === (s.run ?? null) && d.seq === (s.seq ?? null) && d.text === `${s.phase} ${s.text}`;
  });
  const keys = dom.map((s) => tKey(s.t ?? ''));
  const ordered = keys.every((k, j) => j === 0 || keys[j - 1]![0] < k[0] || (keys[j - 1]![0] === k[0] && keys[j - 1]![1] <= k[1]));
  // the residuals step against read_residuals' last points, as the page shows them
  const res = (await client!.ok('read_residuals', { run }))['series'] as Record<string, { x: number[]; y: (number | null)[] }>;
  const residuals = dom.find((s) => s.kind === 'residuals');
  const names = Object.keys(res).filter((k) => k.startsWith('residuals.') || k.startsWith('monitors.'));
  const equal = names.filter((k) => {
    const s = res[k]!;
    return residuals?.facts.includes(`${k} ${String(s.y[s.y.length - 1])} at step ${s.x[s.x.length - 1]}`);
  });
  console.log(
    `E1: ${dom.length} steps shown ${ms} ms after show_view's result (limit ${LIMIT_MS} ms); kinds ${dom.map((s) => s.kind).join(', ')}; journal seq [${domSeqs.join(', ')}] against [${want.join(', ')}]; equal to read_view ${same}; ordered ${ordered}; residuals ${equal.length} of ${names.length} equal; run ${run} ${String(end['state'])}`,
  );
  expect(want.length).toBeGreaterThan(0);
  expect(domSeqs).toEqual(want);
  expect(same).toBe(true);
  expect(ordered).toBe(true);
  expect(names.length).toBeGreaterThan(0);
  expect(equal.length).toBe(names.length);
  expect(ms).toBeLessThanOrEqual(LIMIT_MS);
});

test('E2: each step opens the view of its state', async () => {
  test.setTimeout(300_000);
  const outcome: string[] = [];
  let held = 0;
  for (const s of steps) {
    if (!s.opens) {
      outcome.push(`${s.i} ${s.step}: no view`);
      continue;
    }
    await page.locator(`[data-canvas-kind="process"] li[data-step="${s.i}"] > button`).click();
    const ok = await page
      .waitForFunction(
        (a) => {
          const v = document.querySelector(`[data-opened-step="${a.i}"]`);
          if (!v || v.getAttribute('data-panels') !== String(a.panels.length)) return false;
          return a.panels.every((p, j) => {
            const e = v.querySelector(`[data-opened-panel="${j}"]`);
            if (!e || e.getAttribute('data-canvas-kind') !== p['kind']) return false;
            switch (p['kind']) {
              case 'case':
                return !!e.querySelector(`tr[data-path="${String(p['case'])}"]`);
              case 'mesh':
                return !!e.querySelector('.cv-mesh .mp');
              case 'table':
                return !!e.querySelector(`tr[data-run="${String((p['runs'] as string[])[0])}"]`);
              case 'run':
                return (e.querySelector('h1')?.textContent ?? '') === `Run workspace: ${String(p['run'])}`;
              case 'field':
                return !!e.querySelector('section.fv');
              case 'plot':
              case 'monitor':
                return !!e.querySelector('.plot-frame');
              default:
                return false;
            }
          });
        },
        { i: s.i, panels: s.opens.panels },
        { polling: 50, timeout: 30_000 },
      )
      .then(() => true)
      .catch(() => false);
    if (ok) held++;
    outcome.push(`${s.i} ${s.step}: ${s.opens.panels.map((p) => p['kind']).join('+')} ${ok ? 'opened' : 'NOT opened'}`);
  }
  console.log(`E2: ${held} of ${steps.length} steps opened their view: ${outcome.join('; ')}`);
  expect(steps.every((s) => s.opens !== null)).toBe(true);
  expect(held).toBe(steps.length);
});

test('E3: a further edit by the client is a new last step within 1.6 s', async () => {
  const reason = 'F-978 e2e: one more set-up';
  const n = await page.locator('[data-canvas-kind="process"] li[data-step]').count();
  await edit({ 'solver.max_iterations': 600 }, reason);
  const t0 = Date.now();
  const seqs = journalSeqs();
  const seq = seqs[seqs.length - 1]!;
  await page.waitForFunction(
    (a) => {
      const items = document.querySelectorAll('[data-canvas-kind="process"] li[data-step]');
      const last = items[items.length - 1];
      return items.length === a.n + 1 && last?.getAttribute('data-step-kind') === 'setup' && last.getAttribute('data-seq') === String(a.seq) && (last.textContent ?? '').includes(a.reason);
    },
    { n, seq, reason },
    { polling: 10, timeout: 30_000 },
  );
  const ms = Date.now() - t0;
  console.log(`E3: step ${n} (seq ${seq}) shown ${ms} ms after edit_case's result (limit ${LIMIT_MS} ms)`);
  expect(ms).toBeLessThanOrEqual(LIMIT_MS);
});
