// F-828, ADR 0795: the numbers that had no provenance, in the packaged Studio against the real engine: the residuals of
// the runs table (R), the compared run's residuals (C), the legend's ticks (L1 to L5), the domain's size (D), the scale bar
// (S), the runner's budgets (B1, B2) and the counts of the home (H1 to H8). The rows are the registration's: KR, KC, KL, KD,
// KS, KB, KH, KF (each number opened by the pointer and by the keyboard, its facts against the engine's own answer, asked
// by a second rcfd-mcp of the test's own, or against the stored bytes), KE (the fact Engine, in each), SC (a scan of the
// views for a number with no provenance), KT (the Tab stops), KK (the focus) and KQ (the look at rest).
import { expect, test, type Locator, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { mkdir, mkdtemp, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { until } from '../support/fake-service.js';
import { decodeField64, snapshotDirs } from '../support/rts-decode.js';
import { ProjectServer } from '../../src/main/project/server.js';

const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
// in file order in one window; a test that fails does not stop the others (a new window opens the project again)
test.describe.configure({ mode: 'default', timeout: 180_000 });

/** quick-start: the two lid-driven cavities, on one mesh, with stored fields. */
const RE100 = 'r-20261028T152641-f8c2';
const RE400 = 'r-20261028T152641-97f1';
/** Tab stops of the unchanged Studio (the registration, finding 4). */
const TABS = { home: 32, runs: 32, budgets: 41, workspace: 40, compare: 39 };
/** Ticks of a legend. */
const FIVE = 5;

let dir = '';
let quick = '';
let aside = '';
let studio: Studio | null = null;
const runners: Runner[] = [];
const folders: string[] = [];

type Json = Record<string, unknown>;
type Facts = Record<string, string>;

/** An answer of the engine outside the Studio: a server of the test's own on `folder`. */
async function ask(folder: string, tool: string, args: Json = {}, readOnly = true): Promise<Json> {
  const s = await ProjectServer.start(mcp!, folder, undefined, { readOnly });
  try {
    return (await s.callTool(tool, args, 60_000)) as Json;
  } finally {
    await s.close();
  }
}

/** The project server's name and version, as it says them. */
async function serverLine(folder: string): Promise<string> {
  const s = await ProjectServer.start(mcp!, folder);
  try {
    return `${s.info.name} ${s.info.version}`;
  } finally {
    await s.close();
  }
}

const solverOf = (run: string): string => (JSON.parse(readFileSync(path.join(quick, 'runs', run, 'run.json'), 'utf8')) as { solver_version: string }).solver_version;
const bitsOf = (v: number): bigint => new BigInt64Array(new Float64Array([v]).buffer)[0]!;

async function open(folder: string, name: RegExp): Promise<void> {
  const { app, page } = studio!;
  const close = page.getByRole('button', { name: 'Close project' });
  if (await close.isVisible()) await close.click();
  await stubDialog(app, folder);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name })).toBeVisible({ timeout: 30_000 });
}

const popover = (page: Page): Locator => page.locator('.pv-pop [role="dialog"], .pv-pop[role="dialog"]').first();

/** The facts of the popover that is open, once the run's record is read. */
async function read(page: Page): Promise<Facts> {
  const d = popover(page);
  await expect(d).toBeVisible();
  await expect(d.locator('[data-fact="Engine"] dd')).not.toHaveText('reading…');
  await expect(d.locator('[data-fact="Unit"] dd')).not.toHaveText('reading…');
  const facts = await d.locator('[data-fact]').evaluateAll((rows) => Object.fromEntries(rows.map((r) => [r.getAttribute('data-fact')!, r.querySelector('dd')!.textContent ?? ''])));
  return { ...facts, name: ((await d.getAttribute('aria-label')) ?? '').replace(/^Provenance of /, '') };
}

async function shut(page: Page): Promise<void> {
  await page.keyboard.press('Escape');
  await expect(page.locator('.pv-pop')).toHaveCount(0);
}

async function unfocus(page: Page): Promise<void> {
  await page.mouse.move(2, 890);
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
}

/** By the pointer: a click on the number. */
async function byClick(page: Page, t: Locator): Promise<Facts> {
  await unfocus(page);
  await expect(t).toHaveCount(1);
  await t.click();
  await expect(t).toHaveAttribute('data-provenance-open', '');
  const f = await read(page);
  await shut(page);
  return f;
}

/** By the pointer where a click is the row's or the segment's: the pointer on the number and Ctrl+I. */
async function byHover(page: Page, t: Locator): Promise<Facts> {
  await unfocus(page);
  await expect(t).toHaveCount(1);
  await t.hover();
  await page.keyboard.press('Control+i');
  await expect(t).toHaveAttribute('data-provenance-open', '');
  const f = await read(page);
  await shut(page);
  return f;
}

/** By the keyboard: the focus on `host`, Ctrl+I until `t` is open (at most 12 times); Escape gives the focus back. */
async function byKeys(page: Page, host: Locator, t: Locator): Promise<Facts> {
  await unfocus(page);
  await host.focus();
  await expect(host).toBeFocused();
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press('Control+i');
    await expect(popover(page)).toBeVisible();
    await page.evaluate(() => new Promise((d) => requestAnimationFrame(() => requestAnimationFrame(d))));
    if ((await t.getAttribute('data-provenance-open')) !== null) break;
  }
  await expect(t).toHaveAttribute('data-provenance-open', '');
  const f = await read(page);
  await shut(page);
  await expect(host).toBeFocused();
  return f;
}

const exactOf = (f: Facts): number => Number(/\(([^)]*)\)$/.exec(f['Value'] ?? '')?.[1] ?? f['Value']);

const runRow = (page: Page, id: string): Locator => page.getByRole('row').filter({ has: page.locator(`[data-run="${id}"]`) });
const residual = (page: Page, id: string, eq: string): Locator => runRow(page, id).locator(`.rv-res[data-residual="${eq}"] [data-provenance-member]`);
const frame = (page: Page): Locator => page.locator('.ws .fv-frame').first();
const ticksOf = (root: Locator): Locator => root.locator('.fv-legend-ticks [data-provenance-member]');
const scrubber = (page: Page): Locator => page.getByRole('slider', { name: /^Time of / });

async function toWorkspace(page: Page, run: string): Promise<void> {
  await page.getByRole('tab', { name: 'Workspace' }).click();
  await page.getByRole('listbox', { name: 'Runs of the project' }).locator(`[data-run="${run}"]`).click();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', run);
  await expect(page.locator('.ws')).toHaveAttribute('data-snapshot', /^\d+$/, { timeout: 30_000 });
  await expect(frame(page)).toHaveAttribute('data-ready', '', { timeout: 30_000 });
}

async function stopAt(page: Page, index: number): Promise<void> {
  await scrubber(page).focus();
  await page.keyboard.press('Home');
  for (let i = 0; i < index; i++) await page.keyboard.press('ArrowRight');
  await expect(scrubber(page)).toHaveAttribute('aria-valuenow', String(index));
}

async function compareSide(page: Page): Promise<void> {
  const toggle = page.getByRole('button', { name: 'Compare runs', exact: true });
  if ((await toggle.getAttribute('aria-pressed')) !== 'true') await toggle.click();
  await page.getByRole('button', { name: /Compared run/ }).click();
  await page.locator('.ui-popover').getByRole('option', { name: new RegExp(RE400) }).click();
  await page.getByRole('radio', { name: 'Side by side', exact: true }).click();
  await until(
    async () => {
      const r = await page.locator('.cmp').getAttribute('data-range');
      const f = await page.locator('.cmp .fv-frame').evaluateAll((es) => es.map((e) => [e.getAttribute('data-range'), e.getAttribute('data-ready'), e.getAttribute('data-snapshot')]));
      const want = [await page.locator('.cmp').getAttribute('data-a-snapshot'), await page.locator('.cmp').getAttribute('data-b-snapshot')];
      return r !== null && f.length === 2 && f.every((x, i) => x[0] === r && x[1] !== null && x[2] === want[i]) ? true : null;
    },
    30_000,
    'both views in one range',
  );
}

/** The Tab stops of the page: Tab from its start until the first element comes again. */
async function tabs(page: Page): Promise<number> {
  const seen: string[] = [];
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  for (let i = 0; i < 80; i++) {
    await page.keyboard.press('Tab');
    const d = await page.evaluate(() => {
      const a = document.activeElement as HTMLElement;
      return `${a.tagName}.${a.className}|${a.getAttribute('role') ?? ''}|${(a.getAttribute('aria-label') ?? a.textContent ?? '').trim().slice(0, 50)}`;
    });
    if (d === seen[0]) break;
    seen.push(d);
  }
  return seen.length;
}

/** The exempt texts of the scan (the registration's table), each with its reason; `.x` and below. */
const EXEMPT: readonly (readonly [selector: string, reason: string])[] = [
  ['.ph-ptab, .ph-name, .ph-name-cell, .ph-fact, .ph-recent, .rv-case, .rv-id, .rv-run-title, .ws-item-case, .ws-field-note, .tp-band-name, .tp-resid-title, .tp-resid-compared, .fv-caption > b, .cmp-metrics th, .rc-case, .cmp-other, .ph-designs', 'a name, an id, a path or a file name'],
  ['.ph-fact, .rv-budgets-file', 'a folder or a file name'],
  ['time, .ph-lane, .ph-changed, .ph-table--runs td:nth-child(5), [data-group="trash"] td:nth-child(3), .rv-num-word, .ph-read', 'a time of day or a date'],
  ['.rv-why, .rv-history, .rv-log, .rv-warnings, .ph-notice, .cmp-refusal, .ph-issues', 'the engine’s text'],
  ['.ui-description', 'the words of a rule, not a value (a whole number from 1)'],
  ['.ph-table td:nth-child(4):not(.ph-num)', 'names of cases (a mesh used by)'],
  // numbers outside the brief's list, reported by the registration's "Open"
  ['.cmp-metrics td', 'open: compare_runs'],
  ['.tp-end, .tp-flag', 'open: band ends and the scrubber’s flag'],
  ['.plot-xtick, .plot-ytick', 'open: tick labels of the plots'],
  ['.ph-table--runs td.ph-num', 'open: the home’s Step and Time'],
  ['[data-group="meshes"] td.ph-num, [data-group="trash"] td.ph-num, [data-group="trash"] .ph-group-what', 'open: sizes in kB'],
  ['.rv-state', 'open: the Runs filter’s counts'],
];

interface Scan {
  readonly bare: readonly string[];
  readonly names: readonly string[];
}

/** SC: every visible text with a digit (and number field with a value) that is in no trigger and not exempt. */
const scan = (page: Page, root: string): Promise<Scan> =>
  page.evaluate(
    ([sel, exempt]) => {
      const el = document.querySelector(sel as string)!;
      const bare: string[] = [];
      const names = new Set<string>();
      const shown = (p: Element): boolean => {
        if (!p.checkVisibility({ visibilityProperty: true, opacityProperty: true } as CheckVisibilityOptions)) return false;
        for (let a: Element | null = p; a; a = a.parentElement) {
          const r = a.getBoundingClientRect();
          const cs = getComputedStyle(a);
          if ((cs.overflow !== 'visible' || cs.clipPath !== 'none') && (r.width < 2 || r.height < 2 || cs.clipPath === 'inset(50%)')) return false;
        }
        return true;
      };
      const say = (p: Element, t: string) => bare.push(`${t.trim().slice(0, 60)} < ${p.tagName.toLowerCase()}.${String(p.className)} < ${p.parentElement?.className ?? ''}`);
      const w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let n = w.nextNode(); n; n = w.nextNode()) {
        const t = n.textContent ?? '';
        const p = n.parentElement!;
        const trigger = p.closest('[data-provenance-trigger], [data-provenance-member]');
        if (trigger) {
          names.add(trigger.getAttribute('data-provenance-name') ?? trigger.getAttribute('aria-label') ?? '');
          continue;
        }
        if (!/\d/.test(t) || !shown(p) || p.closest(exempt as string)) continue;
        say(p, t);
      }
      for (const i of Array.from(el.querySelectorAll<HTMLInputElement>('input')))
        if (i.type !== 'radio' && /\d/.test(i.value) && shown(i) && !i.closest('[data-provenance]') && !i.closest(exempt as string)) say(i, i.value);
      return { bare, names: [...names] };
    },
    [root, EXEMPT.map((x) => x[0]).join(', ')],
  );

/** KQ: a trigger at rest is its text. */
const quiet = (page: Page, root: string): Promise<readonly string[]> =>
  page.evaluate(
    (sel) =>
      Array.from(document.querySelectorAll<HTMLElement>(`${sel} [data-provenance-member]`)).flatMap((e) => {
        const c = getComputedStyle(e);
        const p = getComputedStyle(e.parentElement!);
        const ok =
          c.textDecorationLine === 'none' && c.borderTopWidth === '0px' && c.backgroundColor === 'rgba(0, 0, 0, 0)' && c.paddingLeft === '0px' && c.color === p.color && c.fontSize === p.fontSize && c.fontWeight === p.fontWeight && e.tabIndex < 0;
        return ok ? [] : [`${e.textContent} ${c.textDecorationLine} ${c.borderTopWidth} ${c.backgroundColor} ${c.paddingLeft} ${c.color} ${p.color} ${c.fontSize} ${p.fontSize}`];
      }),
    root,
  );

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f828b-'));
  quick = await copyExample('quick-start', dir);
  await mkdir(path.join(dir, 'untouched'));
  aside = await copyExample('quick-start', path.join(dir, 'untouched'));
  folders.push(quick, aside);
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => studio!.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await open(quick, /Quick start/);
});

test.afterAll(async () => {
  await studio?.app.close().catch(() => undefined);
  for (const r of runners) await r.cleanUp(null);
  for (const f of folders) for (const p of mcpPids(f)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('SC KT KQ: no number of the views without provenance; the Tab stops; the look at rest', async () => {
  test.skip(true, 'ADR 0818 amendment 3: the record of the scan that counts the renderer fallback label as a number; see the corrected test after it');
  const { page } = studio!;
  const bare: Record<string, readonly string[]> = {};
  const names: Record<string, readonly string[]> = {};
  const loud: Record<string, readonly string[]> = {};
  const count: Record<string, number> = {};
  const at = async (k: string, root: string) => {
    const s = await scan(page, root);
    bare[k] = s.bare;
    names[k] = s.names;
    await page.mouse.move(2, 890);
    loud[k] = await quiet(page, root);
  };
  await page.waitForTimeout(1500);
  await at('home', 'body');
  count['home'] = await tabs(page);
  await page.getByRole('textbox', { name: 'Filter by name' }).fill('cavity');
  await at('home filtered', '.ph-home');
  await page.getByRole('textbox', { name: 'Filter by name' }).fill('');
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible();
  await page.waitForTimeout(1500);
  await at('runs', '.ph-runs');
  count['runs'] = await tabs(page);
  await page.getByRole('button', { name: /^Runner budgets/ }).click();
  await expect(page.getByRole('textbox', { name: 'max_runs' })).toBeVisible();
  await page.waitForTimeout(500);
  count['budgets'] = await tabs(page);
  await page.locator(`.rv-case[data-run="${RE100}"]`).click();
  await page.waitForTimeout(800);
  await at('runs, budgets, a run selected', '.ph-runs');
  await page.getByRole('button', { name: /^Runner budgets/ }).click();
  await toWorkspace(page, RE100);
  await page.waitForTimeout(1500);
  await at('workspace', '.ws');
  count['workspace'] = await tabs(page);
  await page.getByRole('button', { name: 'Compare runs', exact: true }).click();
  await page.waitForTimeout(4000);
  count['compare'] = await tabs(page);
  await stopAt(page, 7);
  await compareSide(page);
  await at('compare', '.ws');
  await page.getByRole('radio', { name: 'Difference', exact: true }).click();
  await expect(page.locator('.cmp--difference .fv-frame')).toHaveAttribute('data-ready', '', { timeout: 30_000 });
  await at('difference', '.ws');
  await page.getByRole('button', { name: 'Compare runs', exact: true }).click();
  console.log(`SC bare ${JSON.stringify(bare)}`);
  console.log(`KT ${JSON.stringify(count)}`);
  expect(Object.entries(bare).filter(([, v]) => v.length > 0)).toEqual([]);
  // no group left out of the scan
  const has = (k: string, re: RegExp) => expect(names[k]!.some((n) => re.test(n)), `${k}: ${re} in ${names[k]!.join(' | ')}`).toBe(true);
  has('home', /^Provenance of entries: 10$/);
  for (const k of ['cases: 4', 'meshes: 1', 'runs: 4', 'notes: 1', 'designs: \\d+', 'runs of cases/cavity-re100\\.toml: 1']) has('home', new RegExp(`^Provenance of ${k}$`));
  has('home filtered', /^Provenance of cases shown: 4$/);
  has('runs', /^Provenance of residual of u: /);
  has('runs', /^Provenance of residual of p: /);
  has('runs, budgets, a run selected', /^Provenance of max_runs: not set$/);
  for (const k of [/^Provenance of legend of /, /^Provenance of domain: /, /^Provenance of scale bar: /]) for (const v of ['workspace', 'compare', 'difference']) has(v, k);
  expect(names['compare']!.length).toBeGreaterThan(names['workspace']!.length);
  expect(Object.entries(loud).filter(([, v]) => v.length > 0)).toEqual([]);
  expect(count).toEqual(TABS);
});

test('SC KT KQ (ADR 0818 amendment 3, the renderer’s fallback label is not a number of a view): no number of the views without provenance; the Tab stops; the look at rest', async () => {
  // ADR 0818 amendment 3: the label of the WebGL 2 fallback (FieldViewport.tsx), shown only without a WebGPU adapter, names the renderer
  const FALLBACK_LABEL = /^Drawn by WebGL 2 \(fallback: [^)]*\) < span\.fv-backend /;
  const { page } = studio!;
  const bare: Record<string, readonly string[]> = {};
  const names: Record<string, readonly string[]> = {};
  const loud: Record<string, readonly string[]> = {};
  const count: Record<string, number> = {};
  const at = async (k: string, root: string) => {
    const s = await scan(page, root);
    bare[k] = s.bare.filter((b) => !FALLBACK_LABEL.test(b));
    names[k] = s.names;
    await page.mouse.move(2, 890);
    loud[k] = await quiet(page, root);
  };
  await page.waitForTimeout(1500);
  await at('home', 'body');
  count['home'] = await tabs(page);
  await page.getByRole('textbox', { name: 'Filter by name' }).fill('cavity');
  await at('home filtered', '.ph-home');
  await page.getByRole('textbox', { name: 'Filter by name' }).fill('');
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible();
  await page.waitForTimeout(1500);
  await at('runs', '.ph-runs');
  count['runs'] = await tabs(page);
  await page.getByRole('button', { name: /^Runner budgets/ }).click();
  await expect(page.getByRole('textbox', { name: 'max_runs' })).toBeVisible();
  await page.waitForTimeout(500);
  count['budgets'] = await tabs(page);
  await page.locator(`.rv-case[data-run="${RE100}"]`).click();
  await page.waitForTimeout(800);
  await at('runs, budgets, a run selected', '.ph-runs');
  await page.getByRole('button', { name: /^Runner budgets/ }).click();
  await toWorkspace(page, RE100);
  await page.waitForTimeout(1500);
  await at('workspace', '.ws');
  count['workspace'] = await tabs(page);
  await page.getByRole('button', { name: 'Compare runs', exact: true }).click();
  await page.waitForTimeout(4000);
  count['compare'] = await tabs(page);
  await stopAt(page, 7);
  await compareSide(page);
  await at('compare', '.ws');
  await page.getByRole('radio', { name: 'Difference', exact: true }).click();
  await expect(page.locator('.cmp--difference .fv-frame')).toHaveAttribute('data-ready', '', { timeout: 30_000 });
  await at('difference', '.ws');
  await page.getByRole('button', { name: 'Compare runs', exact: true }).click();
  console.log(`SC bare ${JSON.stringify(bare)}`);
  console.log(`KT ${JSON.stringify(count)}`);
  expect(Object.entries(bare).filter(([, v]) => v.length > 0)).toEqual([]);
  // no group left out of the scan
  const has = (k: string, re: RegExp) => expect(names[k]!.some((n) => re.test(n)), `${k}: ${re} in ${names[k]!.join(' | ')}`).toBe(true);
  has('home', /^Provenance of entries: 10$/);
  for (const k of ['cases: 4', 'meshes: 1', 'runs: 4', 'notes: 1', 'designs: \\d+', 'runs of cases/cavity-re100\\.toml: 1']) has('home', new RegExp(`^Provenance of ${k}$`));
  has('home filtered', /^Provenance of cases shown: 4$/);
  has('runs', /^Provenance of residual of u: /);
  has('runs', /^Provenance of residual of p: /);
  has('runs, budgets, a run selected', /^Provenance of max_runs: not set$/);
  for (const k of [/^Provenance of legend of /, /^Provenance of domain: /, /^Provenance of scale bar: /]) for (const v of ['workspace', 'compare', 'difference']) has(v, k);
  expect(names['compare']!.length).toBeGreaterThan(names['workspace']!.length);
  expect(Object.entries(loud).filter(([, v]) => v.length > 0)).toEqual([]);
  expect(count).toEqual(TABS);
});

test('KR KK: the runs table’s residuals are run_status’s of their own run; the row and the cell open them', async () => {
  const { page } = studio!;
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible();
  for (const id of [RE100, RE400]) {
    const st = await ask(quick, 'run_status', { run: id });
    const last = (st['last_residual'] as { residuals: Record<string, number> }).residuals;
    const units = (JSON.parse(readFileSync(path.join(quick, 'runs', id, 'output', 'monitors.json'), 'utf8')) as { series: Record<string, { unit: string }> }).series;
    for (const eq of ['u', 'p']) {
      const t = residual(page, id, eq);
      const cell = runRow(page, id).getByRole('gridcell').filter({ has: page.locator(`[data-residual="${eq}"]`) });
      const unit = units[`res_${eq}`]!.unit;
      for (const f of [await byHover(page, t), await byKeys(page, cell, t), await byKeys(page, runRow(page, id), t)]) {
        expect(f.name).toBe(`residual of ${eq}`);
        expect(bitsOf(exactOf(f))).toBe(bitsOf(last[eq]!));
        expect(f['Run']).toBe(id);
        expect(f['File']).toBe(`runs/${id}/residuals.jsonl`);
        expect(f['Unit']).toBe(unit === '' ? `none, empty in runs/${id}/output/monitors.json` : `${unit}, runs/${id}/output/monitors.json`);
        expect(f['Snapshot']).toMatch(/^none: /);
        expect(f['Read by']).toContain('run_status');
        expect(f['Rounded']).toContain('3 significant figures');
        expect(f['Engine']).toBe(solverOf(id));
      }
    }
  }
  // KK: the row: its own number first, u, p, and round again; a lone number stays; Escape gives the row the focus
  const row = runRow(page, RE100);
  await unfocus(page);
  await row.focus();
  const seen: string[] = [];
  for (let i = 0; i < 4; i++) {
    await page.keyboard.press('Control+i');
    await page.evaluate(() => new Promise((d) => requestAnimationFrame(() => requestAnimationFrame(d))));
    seen.push((await read(page)).name!);
  }
  expect(seen).toEqual(['iterations', 'residual of u', 'residual of p', 'iterations']);
  await shut(page);
  await expect(row).toBeFocused();
  const cell = row.getByRole('gridcell').filter({ has: page.locator('[data-residual="u"]') });
  await cell.focus();
  for (let i = 0; i < 2; i++) {
    await page.keyboard.press('Control+i');
    expect((await read(page)).name).toBe('residual of u');
  }
  await expect(page.locator('.pv-pop')).toHaveCount(1);
  await shut(page);
  await expect(cell).toBeFocused();
  // a click in the cell selects the row, as before, and opens nothing
  await residual(page, RE100, 'u').click();
  await expect(page.locator('aside.rv-run')).toHaveAttribute('data-run', RE100);
  await expect(page.locator('.pv-pop')).toHaveCount(0);
});

test('KL KD KS: the legend’s ticks in each range mode, the domain’s size and the scale bar of one field view', async () => {
  const { page } = studio!;
  await toWorkspace(page, RE100);
  await stopAt(page, 7);
  await expect(frame(page)).toHaveAttribute('data-snapshot', (await page.locator('.ws').getAttribute('data-snapshot'))!, { timeout: 20_000 });
  const view = page.locator('.ws section.fv').first();
  const snapshot = Number(await frame(page).getAttribute('data-snapshot'));
  const dirs = snapshotDirs(path.join(quick, 'runs', RE100, 'fields'));
  const first = await byClick(page, ticksOf(view).first());
  const field = first.name!.replace(/^legend of /, '');
  const info = dirs[snapshot]!.fields.find((f) => f.name === field)!;
  const unit = `${info.unit}, runs/${RE100}/fields/store.rts (the field’s unit)`;
  // the engine's own answers, on the untouched copy (a derived picture writes its file)
  const whole = await ask(aside, 'derive_picture', { run: RE100, kind: 'cut_surface', field, snapshot });
  const { min: lowest, max: highest } = whole['bounds'] as { min: number[]; max: number[] };
  const slice = await ask(aside, 'derive_picture', { run: RE100, kind: 'slice', field, snapshot, axis: 'z', at: 0.5 * (lowest[2]! + highest[2]!) });
  const range = slice['range'] as [number, number];
  const bounds = slice['bounds'] as { min: number[]; max: number[] };

  const ticks = async (kind: RegExp, snap: string | RegExp, lo: number, hi: number) => {
    await expect(ticksOf(view)).toHaveCount(FIVE);
    for (let i = 0; i < FIVE; i++) {
      const t = ticksOf(view).nth(i);
      for (const f of [await byClick(page, t), await byKeys(page, frame(page), t)]) {
        expect(f['Value']).toContain((await t.textContent())!);
        if (i === 0) expect(Math.fround(exactOf(f))).toBe(Math.fround(lo));
        if (i === FIVE - 1) expect(Math.fround(exactOf(f))).toBe(Math.fround(hi));
        expect(exactOf(f)).toBe(i === FIVE - 1 ? Number(f['Range']!.split(' to ')[1]!.split(',')[0]) : Number(f['Range']!.split(' to ')[0]) + ((Number(f['Range']!.split(' to ')[1]!.split(',')[0]) - Number(f['Range']!.split(' to ')[0])) * i) / (FIVE - 1));
        expect(f['Unit']).toBe(unit);
        expect(f['Run']).toBe(RE100);
        expect(f['File']).toBe(`runs/${RE100}/fields/store.rts`);
        expect(f['Snapshot']).toMatch(snap);
        expect(f['Range']).toMatch(kind);
        expect(f['Tick']).toMatch(new RegExp(`^${i + 1} of 5`));
        expect(f['Rounded']).toContain('5 significant figures');
        expect(f['Engine']).toBe(solverOf(RE100));
      }
    }
  };
  const step = dirs[snapshot]!.step;
  // L1
  await ticks(/least and the greatest of the values shown of this snapshot/, new RegExp(`^${snapshot + 1} of ${dirs.length}, step ${step}`), range[0], range[1]);
  // D
  const domain = view.locator('.fv-domain [data-provenance-member]');
  const extents = [0, 1].map((k) => Math.fround(bounds.max[k]!) - Math.fround(bounds.min[k]!));
  for (const f of [await byClick(page, domain), await byKeys(page, frame(page), domain)]) {
    expect(f.name).toBe('domain');
    const exact = /\(([^)]*)\)$/.exec(f['Value']!)?.[1] ?? f['Value']!;
    expect(exact.split(' × ').map((x) => Math.fround(Number(x)))).toEqual(extents.map(Math.fround));
    expect(f['Unit']).toBe(`${String(slice['length_unit'])}, the engine’s documented unit of a stored mesh (ADR 0738); not recorded in store.rts`);
    expect(f['Run']).toBe(RE100);
    expect(f['Snapshot']).toBe('none: one mesh for every snapshot');
    expect(f['File']).toBe(`runs/${RE100}/fields/store.rts`);
    expect(f['Counted']).toMatch(/by the Studio: .*points along x, y \(2D/);
    expect(f['Rounded']).toContain('4 significant figures');
    expect(f['Engine']).toBe(solverOf(RE100));
  }
  // S
  const bar = view.locator('.fv-scalebar [data-provenance-member]');
  const length = Number(await view.locator('.fv-scalebar').getAttribute('data-length'));
  for (const f of [await byClick(page, bar), await byKeys(page, frame(page), bar)]) {
    expect(f.name).toBe('scale bar');
    expect(exactOf(f)).toBe(length);
    expect(await bar.textContent()).toBe(`${length} ${String(slice['length_unit'])}`);
    expect(f['Unit']).toMatch(new RegExp(`^${String(slice['length_unit'])}, .*not recorded in store\\.rts$`));
    expect(f['Run']).toBe(RE100);
    expect(f['File']).toMatch(/^not recorded: /);
    expect(f['Read by']).toMatch(/^not recorded: chosen by the Studio/);
    expect(f['Chosen']).toContain('1, 2 or 5 times a power of ten');
    expect(f['Engine']).toBe(solverOf(RE100));
  }
  // L2: the whole run
  await page.getByRole('button', { name: 'Colour range of the whole run' }).click();
  await expect(view).toContainText(`range of the whole run (${dirs.length} snapshots)`, { timeout: 60_000 });
  let lo = Infinity;
  let hi = -Infinity;
  for (let i = 0; i < dirs.length; i++)
    for (const v of decodeField64(path.join(quick, 'runs', RE100, 'fields'), i, field)) {
      lo = Math.min(lo, v);
      hi = Math.max(hi, v);
    }
  await ticks(new RegExp(`over ${dirs.length} snapshots`), new RegExp(`^none: the range is over ${dirs.length} snapshots`), lo, hi);
  // L3: typed
  await page.getByRole('button', { name: 'Fixed colour range' }).click();
  for (const [name, v] of [
    [/^Bottom of the fixed range/, '0'],
    [/^Top of the fixed range/, '0.1'],
  ] as const) {
    await page.getByRole('textbox', { name }).fill(v);
    await page.getByRole('textbox', { name }).press('Tab');
  }
  await expect(view).toContainText('viridis, a fixed range');
  await ticks(/typed in this window, not recorded by the engine/, /^none: a typed range/, 0, 0.1);
  await page.getByRole('button', { name: 'Colour range of the shown snapshot' }).click();
});

test('KC KL: comparing: the compared run’s residuals, the one legend of both views, each view’s own numbers, the difference’s legend', async () => {
  const { page } = studio!;
  await toWorkspace(page, RE100);
  await stopAt(page, 7);
  await compareSide(page);
  const step = Number(await page.locator('.ws').getAttribute('data-step'));
  // C
  const answer = (await ask(quick, 'read_residuals', { run: RE400, since_step: step, max_points: 2 })) as { series: Record<string, unknown> };
  const list = page.locator(`.tp-resid-compare[data-run="${RE400}"] li[data-value]`);
  const band = page.locator(`.tp-band-name[data-band="${RE400}"]`);
  const units = (JSON.parse(readFileSync(path.join(quick, 'runs', RE400, 'output', 'monitors.json'), 'utf8')) as { series: Record<string, { unit: string }> }).series;
  expect(await list.count()).toBeGreaterThan(1);
  for (let i = 0; i < (await list.count()); i++) {
    const li = list.nth(i);
    const n = (await li.getAttribute('data-name'))!;
    const source = `residuals.${n.replace(/^res_/, '')}`;
    const { x: xs, y: ys } = answer.series[source] as { x: number[]; y: number[] };
    expect(xs[0]).toBe(step);
    const t = li.locator('[data-provenance-member]');
    for (const f of [await byClick(page, t), await byKeys(page, band, t)]) {
      expect(f.name).toBe(n);
      expect(bitsOf(exactOf(f))).toBe(bitsOf(ys[0]!));
      expect(f['Run']).toBe(RE400);
      expect(f['File']).toBe(`runs/${RE400}/residuals.jsonl`);
      const u = units[n]!.unit;
      expect(f['Unit']).toBe(u === '' ? `none, empty in runs/${RE400}/output/monitors.json` : `${u}, runs/${RE400}/output/monitors.json`);
      expect(f['Snapshot']).toMatch(/^none: a residual sample of the compared run/);
      expect(f['Step']).toBe(String(step));
      expect(f['Read by']).toContain('read_residuals');
      expect(f['Engine']).toBe(solverOf(RE400));
    }
  }
  // L4
  const a = Number(await page.locator('.cmp').getAttribute('data-a-snapshot'));
  const b = Number(await page.locator('.cmp').getAttribute('data-b-snapshot'));
  const legend = page.locator('.cmp-legend');
  const name = (await byClick(page, ticksOf(legend).first())).name!;
  const field = name.replace(/^legend of /, '');
  const va = decodeField64(path.join(quick, 'runs', RE100, 'fields'), a, field);
  const vb = decodeField64(path.join(quick, 'runs', RE400, 'fields'), b, field);
  const all = [...va, ...vb];
  const unit = snapshotDirs(path.join(quick, 'runs', RE100, 'fields'))[a]!.fields.find((f) => f.name === field)!.unit;
  const frameA = page.locator('.cmp-view[data-view="a"] .fv-frame');
  const frameB = page.locator('.cmp-view[data-view="b"] .fv-frame');
  for (const i of [0, 2, FIVE - 1]) {
    const t = ticksOf(legend).nth(i);
    for (const f of [await byClick(page, t), await byKeys(page, frameA, t), await byKeys(page, frameB, t)]) {
      if (i === 0) expect(Math.fround(exactOf(f))).toBe(Math.fround(Math.min(...all)));
      if (i === FIVE - 1) expect(Math.fround(exactOf(f))).toBe(Math.fround(Math.max(...all)));
      expect(f['Unit']).toBe(`${unit}, runs/${RE100}/fields/store.rts (the field’s unit)`);
      expect(f['Run']).toBe(RE100);
      expect(f['Snapshot']).toMatch(new RegExp(`^${a + 1} of `));
      expect(f['Range']).toContain('one range of both runs');
      expect(f['Compared run']).toMatch(new RegExp(`^${RE400}, snapshot ${b + 1} of \\d+, runs/${RE400}/fields/store\\.rts$`));
      expect(f['Engine']).toBe(solverOf(RE100));
    }
  }
  // each view's own domain, scale bar and readout (the readout's key was one for both views)
  for (const [v, id] of [
    ['a', RE100],
    ['b', RE400],
  ] as const) {
    const root = page.locator(`.cmp-view[data-view="${v}"]`);
    const host = root.locator('.fv-frame');
    for (const what of ['.fv-domain', '.fv-scalebar']) {
      const t = root.locator(`${what} [data-provenance-member]`);
      expect((await byClick(page, t))['Run']).toBe(id);
      expect((await byKeys(page, host, t))['Run']).toBe(id);
    }
    await unfocus(page);
    await host.focus();
    await page.keyboard.press('ArrowLeft');
    await expect(root.locator('.fv-readout-value')).toBeVisible();
    await page.keyboard.press('Control+i');
    const f = await read(page);
    expect(f['Run']).toBe(id);
    expect(f['Cell']).toBe(await root.locator('.fv-readout-value').getAttribute('data-cell'));
    await shut(page);
  }
  // L5
  await page.getByRole('radio', { name: 'Difference', exact: true }).click();
  const diff = page.locator('.cmp--difference .fv-frame');
  await expect(diff).toHaveAttribute('data-ready', '', { timeout: 30_000 });
  const d = va.map((x, i) => Math.fround(vb[i]! - x));
  const dl = page.locator('.cmp--difference');
  for (const i of [0, FIVE - 1]) {
    const t = ticksOf(dl).nth(i);
    for (const f of [await byClick(page, t), await byKeys(page, diff, t)]) {
      expect(f.name).toBe(`legend of Δ${field}`);
      expect(Math.fround(exactOf(f))).toBe(i === 0 ? Math.min(...d) : Math.max(...d));
      expect(f['Run']).toBe(RE400);
      expect(f['Snapshot']).toMatch(new RegExp(`^${b + 1} of `));
      expect(f['Unit']).toBe(`${unit}, runs/${RE400}/fields/store.rts (the field’s unit)`);
      expect(f['Range']).toContain('subtracted by the Studio');
      expect(f['Baseline run']).toMatch(new RegExp(`^${RE100}, snapshot ${a + 1} of \\d+, runs/${RE100}/fields/store\\.rts$`));
      expect(f['Engine']).toBe(solverOf(RE400));
    }
  }
  await page.getByRole('button', { name: 'Compare runs', exact: true }).click();
});

test('KB: the runner’s budgets, in the file and in use', async () => {
  const { page } = studio!;
  await mkdir(path.join(dir, 'budgets'));
  const folder = await copyExample('quick-start', path.join(dir, 'budgets'));
  folders.push(folder);
  const r = new Runner(runnerExe!, folder);
  runners.push(r);
  await mkdir(path.join(folder, '.rcfd'), { recursive: true });
  await writeFile(path.join(folder, '.rcfd', 'runner-config.json'), '{"max_runs": 2, "cpu_threads": 3}\n');
  r.ensure();
  await until(() => (r.pid() !== null ? true : null), 20_000, 'the runner’s record');
  // another value after the start: the file and the runner's own differ
  await writeFile(path.join(folder, '.rcfd', 'runner-config.json'), '{"max_runs": 4, "cpu_threads": 3}\n');
  await open(folder, /Quick start/);
  await page.getByRole('tab', { name: 'Runs' }).click();
  await page.getByRole('button', { name: /^Runner budgets/ }).click();
  const panel = page.getByRole('region', { name: 'Runner budgets' });
  const answer = await ask(folder, 'runner_config');
  const hash = String(answer['hash']);
  const record = JSON.parse(readFileSync(path.join(folder, '.rcfd', 'runner.json'), 'utf8')) as { config: Record<string, number>; config_source: string; started?: string };
  const values = answer['values'] as Record<string, number>;
  expect(values['max_runs']).toBe(4);
  expect(record.config['max_runs']).toBe(2);
  const server = await serverLine(folder);
  const field = (k: string) => panel.getByRole('textbox', { name: k });
  const of = (k: string) => panel.locator('.rv-budget').filter({ has: page.getByRole('textbox', { name: k }) });
  const name = (k: string) => of(k).locator('.ui-label [data-provenance-member]');
  const inUse = (k: string) => of(k).locator('.ui-description [data-provenance-member]');
  for (const k of ['max_runs', 'cpu_threads']) {
    for (const f of [await byClick(page, name(k)), await byKeys(page, field(k), name(k))]) {
      expect(f.name).toBe(k);
      expect(f['Value']).toBe(String(values[k]));
      expect(f['Unit']).toBe('not recorded: runner_config names no unit of its keys');
      expect(f['Run']).toMatch(/^none: /);
      expect(f['Snapshot']).toMatch(/^none: /);
      expect(f['Project']).toBe(folder);
      expect(f['File']).toBe('.rcfd/runner-config.json');
      expect(f['Read by']).toBe(`runner_config (rcfd-mcp, read-only), values.${k}`);
      expect(f['Hash']).toBe(hash);
      expect(f['Typed']).toBeUndefined();
      expect(f['Engine']).toBe(server);
    }
  }
  for (const k of ['max_runs', 'cpu_threads', 'mem_base_mb']) {
    for (const f of [await byClick(page, inUse(k)), await byKeys(page, field(k), inUse(k))]) {
      expect(f.name).toBe(`${k} in use`);
      expect(Number(f['Value'])).toBe(record.config[k]);
      expect(f['Unit']).toBe('not recorded: runner_config names no unit of its keys');
      expect(f['File']).toBe('.rcfd/runner.json');
      expect(f['Read by']).toBe(`runner_config (rcfd-mcp, read-only): the running runner’s record, config.${k}`);
      expect(f['Config source']).toBe(record.config_source);
      expect(f['Runner started']).not.toBe('');
      expect(f['Engine']).toBe(server);
    }
  }
  // an empty key, and a typed value
  const empty = await byClick(page, name('poll_ms'));
  expect(empty['Value']).toBe('not set');
  expect(empty['Default']).toMatch(/^not recorded in the file: /);
  await field('max_runs').fill('7');
  await field('max_runs').press('Tab');
  const typed = await byClick(page, name('max_runs'));
  expect(typed['Value']).toBe('7');
  expect(typed['Typed']).toBe('not written: the file holds 4');
  await page.getByRole('button', { name: 'Close', exact: true }).click();
});

test('KH: the counts of the home against the engine’s answers and the folder', async () => {
  test.setTimeout(400_000);
  const { page } = studio!;
  await mkdir(path.join(dir, 'home'));
  const folder = await copyExample('quick-start', path.join(dir, 'home'));
  folders.push(folder);
  const at = (...p: string[]) => path.join(folder, ...p);
  await mkdir(at('meshes', 'of', 'constant', 'polyMesh'), { recursive: true });
  for (const f of ['points', 'faces', 'owner', 'neighbour', 'boundary']) await writeFile(at('meshes', 'of', 'constant', 'polyMesh', f), 'x\n');
  await mkdir(at('notes'), { recursive: true });
  await Promise.all(Array.from({ length: 5001 }, (_, i) => writeFile(at('notes', `n${String(i).padStart(4, '0')}.md`), 'n\n')));
  for (let i = 0; i < 53; i++) await mkdir(at('runs', `not-a-run-${i}`));
  await mkdir(at('designs', 'd1'), { recursive: true });
  for (let i = 0; i < 52; i++) await writeFile(at('cases', `t${i}.toml`), 'x = 1\n');
  const w = await ProjectServer.start(mcp!, folder, undefined, { readOnly: false });
  for (let i = 0; i < 52; i++) {
    const h = (await w.callTool('read_project_file', { path: `cases/t${i}.toml`, limit: 1 }, 30_000)) as { hash: string };
    await w.callTool('trash_case', { path: `cases/t${i}.toml`, base_hash: h.hash }, 30_000);
  }
  await w.close();

  const info = (await ask(folder, 'project_info')) as { counts: Record<string, number>; issues: unknown[]; issues_total: number };
  const runs = (await ask(folder, 'list_runs', { limit: 100 })) as { total: number; runs: { id: string; case: string }[] };
  const meshes = (await ask(folder, 'list_meshes', { limit: 100 })) as { total: number; meshes: { path: string; files: number }[] };
  const trash = (await ask(folder, 'list_trash', { limit: 50 })) as { total: number; items: unknown[] };
  const server = await serverLine(folder);
  const files = async (top: string): Promise<number> => (await readdir(at(top), { recursive: true, withFileTypes: true })).filter((e) => e.isFile()).length;
  const cases = await files('cases');
  const notes = await files('notes');
  const designs = (await readdir(at('designs'))).length;
  expect(info.counts['cases']).toBe(cases);
  expect(info.counts['notes']).toBe(notes);
  expect(info.counts['meshes']).not.toBe(meshes.total);
  expect(info.issues_total).toBeGreaterThan(info.issues.length);
  expect(trash.total).toBeGreaterThan(50);
  expect(notes).toBeGreaterThan(5000);

  await open(folder, /Quick start/);
  const seg = (label: string) => page.getByRole('radio', { name: new RegExp(`^${label} \\d+$`) });
  const group = (id: string) => page.locator(`section[aria-labelledby="ph-g-${id}"]`);
  const head = (id: string) => group(id).locator('.ph-group-count [data-provenance-member]');
  const check = (f: Facts, o: { name: string; value: number; file: string | RegExp; readBy: string | RegExp; counted: string | RegExp; engine?: number | null }) => {
    expect(f.name).toBe(o.name);
    expect(f['Value']).toBe(String(o.value));
    expect(f['Unit']).toBe('none: a count');
    expect(f['Run']).toMatch(/^none: /);
    expect(f['Snapshot']).toMatch(/^none: /);
    expect(f['Project']).toBe(folder);
    expect(f['File']).toMatch(o.file);
    expect(f['Read by']).toMatch(o.readBy);
    expect(f['Counted']).toMatch(o.counted);
    if (o.engine != null) expect(f['Engine’s count']).toMatch(new RegExp(`^${o.engine}, counts\\.\\w+ of project_info$`));
    expect(Number.isNaN(Date.parse(f['Read at']!))).toBe(false);
    expect(f['Engine']).toBe(server);
  };
  await expect(head('notes')).toHaveText(String(notes), { timeout: 30_000 });
  // H1
  const total = cases + meshes.total + runs.total + notes;
  const all = seg('All').locator('[data-provenance-member]');
  for (const f of [await byHover(page, all), await byKeys(page, seg('All'), all)])
    check(f, { name: 'entries', value: total, file: /cases\/, meshes\/, runs\/, notes\//, readBy: /totals of the four kinds/, counted: `by the Studio: the sum of cases ${cases}, meshes ${meshes.total}, runs ${runs.total} and notes ${notes}` });
  // H2
  const kinds = [
    { id: 'cases', label: 'Cases', value: cases, readBy: /the Studio, from the names/, counted: /by the Studio: every file below cases\// },
    { id: 'meshes', label: 'Meshes', value: meshes.total, readBy: /^list_meshes .* total$/, counted: /by the engine/ },
    { id: 'runs', label: 'Runs', value: runs.total, readBy: /^list_runs .* total$/, counted: /by the engine/ },
    { id: 'notes', label: 'Notes', value: notes, readBy: /the Studio, from the names/, counted: /by the Studio: every file below notes\// },
  ];
  for (const k of kinds) {
    const own = seg(k.label).locator('[data-provenance-member]');
    const o = { name: k.id, value: k.value, file: new RegExp(`^${k.id}/$`), readBy: k.readBy, counted: k.counted, engine: info.counts[k.id]! };
    for (const f of [await byHover(page, own), await byKeys(page, seg(k.label), own), await byClick(page, head(k.id)), await byKeys(page, seg(k.label), head(k.id))]) check(f, o);
  }
  // H3
  for (const f of [await byClick(page, head('designs')), await byKeys(page, seg('All'), head('designs'))])
    check(f, { name: 'designs', value: designs, file: /^designs\/$/, readBy: /the Studio, from the names in designs\//, counted: /by the Studio: every file and folder directly in designs\//, engine: info.counts['designs']! });
  for (const f of [await byClick(page, head('issues')), await byKeys(page, seg('All'), head('issues'))])
    check(f, { name: 'issues', value: info.issues_total, file: /the project folder/, readBy: /^project_info .* issues_total$/, counted: /by the engine/ });
  for (const f of [await byClick(page, head('trash')), await byKeys(page, seg('All'), head('trash'))])
    check(f, { name: 'items in the trash', value: trash.total, file: /^\.rcfd\/trash\/$/, readBy: /^list_trash .* total$/, counted: /by the engine/ });
  // H5
  const engine = group('meshes').locator('.ph-group-what [data-provenance-member]');
  for (const f of [await byClick(page, engine), await byKeys(page, seg('Meshes'), engine)])
    check(f, { name: 'meshes counted by the engine', value: info.counts['meshes']!, file: /^meshes\/$/, readBy: /^project_info .* counts\.meshes$/, counted: /^by the engine$/ });
  // H6
  const first = group('notes').locator('.ph-more [data-provenance-member]');
  for (const f of [await byClick(page, first.first()), await byKeys(page, seg('Notes'), first.first())])
    check(f, { name: 'notes listed', value: 5000, file: /^notes\/$/, readBy: /the Studio, from the names/, counted: /by the Studio: the entries of the list it read/ });
  for (const f of [await byClick(page, first.nth(1)), await byKeys(page, seg('Notes'), first.nth(1))])
    check(f, { name: 'notes', value: notes, file: /^notes\/$/, readBy: /the Studio, from the names/, counted: /by the Studio: every file below notes\//, engine: info.counts['notes']! });
  const more = page.locator('.ph-issues .ph-more [data-provenance-member]');
  for (const f of [await byClick(page, more), await byKeys(page, seg('All'), more)])
    check(f, { name: 'issues not listed', value: info.issues_total - info.issues.length, file: /the project folder/, readBy: /issues_total and issues$/, counted: `by the Studio: ${info.issues_total} in all less the ${info.issues.length} listed` });
  const older = group('trash').locator('.ph-more [data-provenance-member]');
  for (const f of [await byClick(page, older), await byKeys(page, seg('All'), older)])
    check(f, { name: 'trash items not listed', value: trash.total - trash.items.length, file: /^\.rcfd\/trash\/$/, readBy: /total and items$/, counted: `by the Studio: ${trash.total} in all less the ${trash.items.length} listed` });
  // H7
  for (const c of ['cases/cavity-re100.toml', 'cases/heated-cavity-ra1e3.toml']) {
    const row = page.locator(`tr[data-path="${c}"]`);
    const t = row.locator('td.ph-num [data-provenance-member]');
    for (const f of [await byClick(page, t), await byKeys(page, row.getByRole('button', { name: `Edit ${c}` }), t)])
      check(f, { name: `runs of ${c}`, value: runs.runs.filter((r) => r.case.replace(/\\/g, '/') === c).length, file: /^runs\/$/, readBy: /^list_runs .*case of each run$/, counted: `by the Studio: the runs of the list whose case is ${c}` });
  }
  // H8
  const mesh = meshes.meshes.find((m) => m.files > 1)!;
  const many = page.locator(`tr[data-path="${mesh.path}"] [data-provenance-member]`);
  for (const f of [await byClick(page, many), await byKeys(page, seg('Meshes'), many)])
    check(f, { name: `files of ${mesh.path}`, value: mesh.files, file: new RegExp(`^${mesh.path}$`), readBy: /^list_meshes .* files$/, counted: /by the engine/ });
  // H4
  await page.getByRole('textbox', { name: 'Filter by name' }).fill('cavity');
  const shown = group('cases').locator('.ph-group-what [data-provenance-member]');
  for (const f of [await byClick(page, shown), await byKeys(page, seg('Cases'), shown)])
    check(f, { name: 'cases shown', value: (await readdir(at('cases'))).filter((n) => n.includes('cavity')).length, file: /^cases\/$/, readBy: /the Studio, from the names/, counted: 'by the Studio: the listed entries whose text holds “cavity”' });
  // SC on this project too: nothing of it without provenance
  expect((await scan(page, '.ph-home')).bare).toEqual([]);
  await page.getByRole('textbox', { name: 'Filter by name' }).fill('');
  // by the pointer a segment is still chosen
  await seg('Runs').locator('[data-provenance-member]').click();
  await expect(seg('Runs')).toBeChecked();
  await expect(page.locator('.pv-pop')).toHaveCount(0);
});
