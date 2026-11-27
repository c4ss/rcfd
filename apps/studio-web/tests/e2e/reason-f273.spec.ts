// F-273 (ADR 0837): the reason (why) of a change shown with who and what, read from the feed, in the header's activity
// line and in the process view's rows on Claude's canvas. The packaged Studio has a copy of the quick-start project
// open; the test is the MCP client Claude Code would be: it spawns `rcfd-mcp --project <folder>` and makes
//
//   S1 new_case with a reason, S2 edit_case without one, S3 edit_case with a reason holding markup, S4 start_run with a
//   reason; then show_view of the case's process; then appends to .rcfd/edits.jsonl, with the server's actor, what no
//   tool writes: S5 a reason of 600 code points, S6 a reason that is a number.
//
// A1: after S1 to S4 the activity line shows who (Claude Code, data-actor mcp, data-pid the server's), what (the verb and
// the thing) and why (the reason, or "not given"). A2: every journaled step of the case shows "by mcp (pid N); why: ...".
// A3: S3's markup shows as its characters and makes no element. A4: S5 is cut after 500 code points with an ellipsis
// (ADR 0188 J4). A5: S6 reads "not given" and the view keeps working. Each within L = 4 periods of the Studio's feed poll
// (ADR 0188 C3-St, as ADR 0826 registers it), from the tool's result or the append.
// Needs rcfd-mcp with rcfd-runner beside it (tests/support/project.ts); without rcfd-mcp the tests are skipped.
import { expect, test, type Page } from '@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { appendFileSync, readFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import readline from 'node:readline';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { copyExample, findMcp, mcpPids, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { killPid } from '../support/rcfd.js';
import { endProjectRunner } from '../support/runner.js';
import { FEED_POLL_MS } from '../../src/shared/project.js';

const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial' });

/** ADR 0837: 4 periods of the Studio's feed poll. */
const LIMIT_MS = 4 * FEED_POLL_MS;
const CASE = 'cases/why-cavity.toml';
const VIEW = { version: 1, title: 'Why of the cavity', panels: [{ kind: 'process', case: CASE }] };
const MARKUP = '<b>bold</b> & "q" <i>';
const LONG = 'why '.repeat(150);

type Json = Record<string, unknown>;

let dir = '';
let quick = '';
let studio: Studio;
let page: Page;
let client: McpClient | null = null;
const times: Record<string, number> = {};

/** A minimal MCP client over stdio (as in canvas-f978.spec.ts): the part of Claude Code this test stands in for. */
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

/** The why this test expects of a journal line's reason, by ADR 0837 D2, written here apart from the Studio's code. */
function expectWhy(r: unknown): string {
  if (typeof r !== 'string' || r.trim() === '') return 'not given';
  const p = [...r];
  return p.length > 500 ? `${p.slice(0, 500).join('')}…` : r;
}

function journal(): Json[] {
  return readFileSync(path.join(quick, '.rcfd', 'edits.jsonl'), 'utf8')
    .split('\n')
    .flatMap((l) => {
      try {
        return [JSON.parse(l) as Json];
      } catch {
        return [];
      }
    });
}

/** Waits until the activity line shows who, what and why; the milliseconds since `t0`. */
async function activityShows(t0: number, what: string, why: string, given: boolean): Promise<number> {
  await page.waitForFunction(
    (a) => {
      const line = document.querySelector('.ph-activity');
      const w = line?.querySelector('.ph-activity-why');
      return (
        !!line &&
        (line.textContent ?? '').includes(`Claude Code ${a.what}`) &&
        line.getAttribute('data-actor') === 'mcp' &&
        line.getAttribute('data-pid') === String(a.pid) &&
        (w?.textContent ?? '') === `; why: ${a.why}` &&
        w?.getAttribute('data-why') === (a.given ? 'given' : 'none')
      );
    },
    { what, why, given, pid: client!.child.pid },
    { polling: 10, timeout: 30_000 },
  );
  return Date.now() - t0;
}

async function edit(set: Json, reason?: string): Promise<void> {
  const hash = (await client!.ok('read_project_file', { path: CASE }))['hash'];
  await client!.ok('edit_case', { path: CASE, base_hash: hash, set, ...(reason === undefined ? {} : { reason }) });
}

/** The rows' who-and-why lines of the journaled steps, by seq. */
function rowLines(p: Page): Promise<{ seq: number; text: string; actor: string | null; pid: string | null; why: string | null; cut: string | null; elements: number }[]> {
  return p.evaluate(() =>
    [...document.querySelectorAll('[data-canvas-kind="process"] li[data-step][data-source="journal"]')].map((li) => {
      const c = li.querySelector('.cv-change');
      return {
        seq: Number(li.getAttribute('data-seq')),
        text: c?.textContent ?? '',
        actor: c?.getAttribute('data-actor') ?? null,
        pid: c?.getAttribute('data-pid') ?? null,
        why: c?.getAttribute('data-why') ?? null,
        cut: c?.getAttribute('data-reason-cut') ?? null,
        elements: c?.querySelectorAll('b, i').length ?? -1,
      };
    }),
  );
}

/** Waits until the rows hold `n` journaled steps, each with the who and why of its journal line. */
async function rowsShow(t0: number, n: number): Promise<number> {
  const want = Object.fromEntries(journal().map((l) => [String(l['seq']), `by mcp (pid ${client!.child.pid}); why: ${expectWhy(l['reason'])}`]));
  await page.waitForFunction(
    (a) => {
      const rows = [...document.querySelectorAll('[data-canvas-kind="process"] li[data-step][data-source="journal"]')];
      return rows.length === a.n && rows.every((li) => (li.querySelector('.cv-change')?.textContent ?? '') === a.want[li.getAttribute('data-seq') ?? '']);
    },
    { n, want },
    { polling: 10, timeout: 30_000 },
  );
  return Date.now() - t0;
}

/** Appends a journal line by hand, as the server's actor would write it, with the given reason. */
function appendLine(reason: unknown): number {
  const lines = journal();
  const last = [...lines].reverse().find((l) => (l['actor'] as Json | undefined)?.['client'] === 'mcp')!;
  const seq = Math.max(...lines.map((l) => Number(l['seq'] ?? 0))) + 1;
  const t = new Date().toISOString().replace(/\.(\d{3})Z$/, '.$1000000Z');
  appendFileSync(path.join(quick, '.rcfd', 'edits.jsonl'), `${JSON.stringify({ kind: 'edit', t, seq, path: CASE, who: 'edit_case', actor: last['actor'], reason })}\n`);
  return Date.now();
}

test.beforeAll(async () => {
  dir = await scratch('reason-f273');
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
  try {
    client?.close();
    await studio?.app.close();
    for (const p of mcpPids(quick)) killPid(p);
  } finally {
    // ADR 0828: the runner start_run started (detached, no idle exit), ended before the folder is removed
    await endProjectRunner(quick);
    await rm(dir, { recursive: true, force: true }).catch(() => undefined);
  }
  console.log(`F-273 times (ms, limit ${LIMIT_MS}): ${JSON.stringify(times)}`);
});

test('A1, A3: each change of the MCP client shows on the activity line with who, what and why', async () => {
  test.setTimeout(180_000);
  await client!.ok('new_case', { name: 'why-cavity', copy: 'cases/cavity-re100.toml', reason: 'F-273 e2e: a case to ask why' });
  times['S1'] = await activityShows(Date.now(), `made ${CASE}`, 'F-273 e2e: a case to ask why', true);
  await edit({ 'physics.nu': 0.02 });
  times['S2'] = await activityShows(Date.now(), `changed ${CASE}`, 'not given', false);
  await edit({ 'physics.nu': 0.03 }, MARKUP);
  times['S3'] = await activityShows(Date.now(), `changed ${CASE}`, MARKUP, true);
  expect(await page.locator('.ph-activity b, .ph-activity i').count()).toBe(0);
  const run = (await client!.ok('start_run', { case: CASE, max_steps: 5, reason: 'F-273 e2e: run it' }))['run'] as string;
  times['S4'] = await activityShows(Date.now(), `started run ${run}`, 'F-273 e2e: run it', true);
  for (const k of ['S1', 'S2', 'S3', 'S4']) expect(times[k], k).toBeLessThanOrEqual(LIMIT_MS);
});

test('A2, A3: every journaled step of the case shows who and why in the process view', async () => {
  test.setTimeout(120_000);
  const shown = await client!.ok('show_view', { view: VIEW, reason: 'F-273 e2e' });
  const t0 = Date.now();
  expect(shown['studios']).toBe(1);
  const n = journal().filter((l) => l['path'] === CASE || l['case'] === CASE).length;
  times['A2'] = await rowsShow(t0, n);
  const rows = await rowLines(page);
  expect(rows.length).toBe(n);
  for (const r of rows) {
    expect(r.actor).toBe('mcp');
    expect(r.pid).toBe(String(client!.child.pid));
    expect(r.elements).toBe(0);
  }
  expect(rows.filter((r) => r.why === 'none').length).toBe(1);
  expect(rows.some((r) => r.text.endsWith(`why: ${MARKUP}`))).toBe(true);
  expect(times['A2']).toBeLessThanOrEqual(LIMIT_MS);
});

test('A4, A5: a reason over 500 code points is cut, a reason that is not a string reads "not given"', async () => {
  test.setTimeout(120_000);
  const before = (await rowLines(page)).length;
  let t0 = appendLine(LONG);
  const cut = `${[...LONG].slice(0, 500).join('')}…`;
  times['S5-line'] = await activityShows(t0, `changed ${CASE}`, cut, true);
  times['S5-row'] = await rowsShow(t0, before + 1);
  expect(await page.locator('.ph-activity-why').getAttribute('data-reason-cut')).toBe('true');
  expect((await rowLines(page)).at(-1)!.cut).toBe('true');
  t0 = appendLine(42);
  times['S6-line'] = await activityShows(t0, `changed ${CASE}`, 'not given', false);
  times['S6-row'] = await rowsShow(t0, before + 2);
  expect((await rowLines(page)).at(-1)!.why).toBe('none');
  for (const k of ['S5-line', 'S5-row', 'S6-line', 'S6-row']) expect(times[k], k).toBeLessThanOrEqual(LIMIT_MS);
});
