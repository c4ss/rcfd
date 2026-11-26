// F-816 (G8): the keyboard and assistive-technology pass over every built surface of the packaged Studio, against the
// real engine (rcfd serve, rcfd-mcp, the fake claude of Use with Claude Code), at 1440 by 900. Every surface is reached
// and operated with the keyboard alone (Tab, Shift+Tab, the arrow keys, Enter, Space, Escape and the Studio's
// shortcuts); the folder dialog is the only thing stubbed, as the other e2e tests do. On each surface:
//
// - the Tab walk goes round the whole window and comes back to where it started (no trap); Shift+Tab goes round the
//   same stops in the opposite order;
// - every stop shows a focus indicator on the screen (an outline, shadow or border it has only while focused; no colour
//   or contrast rule, by the maintainer's decision of 2026-11-16);
// - every interactive element of the surface is a Tab stop or a member of a composite that is one, and the arrow keys
//   reach every member of each composite;
// - no element has a click or pointer handler that no key reaches;
// - every node of Chromium's accessibility tree with an interactive role has a name, and every Tab stop has a role;
// - a dialog (the command palette, the provenance popover, a select's list, the decision panel) keeps the focus inside
//   while open, closes on Escape and gives the focus back to what opened it.
//
// The reduced-motion and colour-alone checks are the last tests of the file. Screenshots of focus states go to
// test-results-visual/f816/, with report.json (every stop and finding) for the maintainer's view-and-report (G13).
//
// Needs rcfd and rcfd-mcp (tests/support/rcfd.ts, tests/support/project.ts); without them the tests are skipped.
import { expect, test, type CDPSession, type Locator, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import path from 'node:path';
import { arrowWalk, cycleOf, framesDuring, installA11y, motionOf, pointerOnly, reach, rolesOf, tabWalk, unnamed, NO_ROLE, type Stop, type Walk } from '../support/a11y.js';
import { axeCheck, EXCLUDED, OFF, type AxeReport } from '../support/axe.js';
import { claudeCodeFake } from '../support/claude-code.js';
import { copyExample, emptyProject, findMcp, mcpPids, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, killPid, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
const mcp = findMcp();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 300_000 });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f816');
const RE100 = 'r-20261028T152641-f8c2';
const RE400 = 'r-20261028T152641-97f1';
/** Motion shorter than this is not motion (a 0 s transition still reports 0). */
const MOTION_MS = 50;
/** A closed session (a transcript, no process) and one open in another process (this test's own, alive). */

let dir = '';
let quick = '';
let empty = '';
let service: RunningRcfd;
let studio: Studio;
let page: Page;
let cdp: CDPSession;

interface SurfaceReport {
  stops: number;
  closed: boolean;
  loop: string[] | null;
  reverseMatches: boolean;
  noIndicator: string[];
  unreached: string[];
  pointerOnly: string[];
  composites: { composite: string; members: number; reached: number; missed: string[]; restored: boolean; start: string; end: string | null; trace?: string[] }[];
  unnamed: { role: string; html: string }[];
  noRole: string[];
  shots: string[];
  /** How the stops draw their focus indicator, counted. */
  indicators: Record<string, number>;
}
const report: Record<string, SurfaceReport | Record<string, unknown>> = {};
/** axe-core on each surface (F-816, the maintainer's word of 2026-11-17): serious and critical violations fail. */
const axeReport: Record<string, AxeReport> = {};
const checkedComposites = new Set<string>();

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  dir = await scratch('f816');
  quick = await copyExample('quick-start', dir);
  empty = await emptyProject(dir, 'An empty project');
  service = await startRcfd(rcfd!);
  studio = await launchStudio({
    RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`,
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_RECENT: path.join(dir, 'recent.json'),
    ...claudeCodeFake(dir).env,
  });
  page = studio.page;
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  cdp = await studio.app.context().newCDPSession(page);
  await installA11y(page);
  // the pointer rests on the footer, off every control: a hover is not a focus
  await page.mouse.move(1439, 899);
});

test.afterAll(async () => {
  writeFileSync(path.join(SHOTS, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  writeFileSync(path.join(SHOTS, 'axe.json'), JSON.stringify({ off: OFF, excluded: EXCLUDED, surfaces: axeReport }, null, 2) + '\n');
  await studio?.app.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  for (const f of [quick, empty]) for (const p of mcpPids(f)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

// ---- keyboard-only navigation -----------------------------------------------------------------------------------------

const hasFocus = (l: Locator) => l.evaluate((el) => el === document.activeElement || el.contains(document.activeElement)).catch(() => false);

/** Tab until `target` has the focus (or holds it). */
async function tabTo(target: Locator, max = 400, key = 'Tab'): Promise<void> {
  await expect(target).toBeVisible({ timeout: 30_000 });
  for (let i = 0; i < max; i++) {
    if (await hasFocus(target)) return;
    await page.keyboard.press(key);
  }
  throw new Error(`${key} never reached ${target}`);
}

/** Arrow keys until `target` has the focus (in a composite that already holds it), from the composite's first member. */
async function arrowTo(target: Locator, key: 'ArrowDown' | 'ArrowRight', max = 40): Promise<void> {
  const back = key === 'ArrowDown' ? 'ArrowUp' : 'ArrowLeft';
  for (let i = 0, last = ''; i < max; i++) {
    if (await hasFocus(target)) return;
    const now = await page.evaluate(() => document.activeElement?.outerHTML.slice(0, 200) ?? '');
    if (now === last) break;
    last = now;
    await page.keyboard.press(back);
  }
  for (let i = 0; i < max; i++) {
    if (await hasFocus(target)) return;
    await page.keyboard.press(key);
  }
  throw new Error(`${key} never reached ${target}`);
}

// ---- one surface ------------------------------------------------------------------------------------------------------

const seenNoIndicator = new Set<string>();
const seenUnnamed = new Set<string>();

/**
 * The whole check of one surface: the Tab walk both ways, focus indicators, reach, pointer-only targets, composites,
 * names and roles. `root` is the surface's root element; the walk covers the whole window (the header band, the
 * footer), the reach is held for the surface.
 */
async function surface(id: string, root: string, opts: { exclude?: string[]; modal?: boolean; composites?: boolean } = {}): Promise<SurfaceReport> {
  await page.mouse.move(1439, 899);
  await expect(page.locator(root).first()).toBeVisible();
  // axe-core on the whole window as this surface shows it, before the walk moves the focus
  const ax = await axeCheck(page);
  axeReport[id] = ax;
  expect.soft(ax.failing, `${id}: axe-core violations of impact serious or critical`).toEqual([]);
  const shots: string[] = [];
  const shotRoles = new Set<string>();
  const forward = await tabWalk(page, {
    onStop: async (s: Stop) => {
      // a few focus states of the surface, one per role, for the view-and-report
      if (shots.length >= 6 || shotRoles.has(s.role)) return;
      const inRoot = await page.evaluate((r) => {
        const a = document.activeElement;
        return a !== null && a.closest(r) !== null;
      }, root);
      if (!inRoot) return;
      shotRoles.add(s.role);
      const file = `${id}-${String(shots.length + 1).padStart(2, '0')}-${s.role}.png`;
      await page.screenshot({ path: path.join(SHOTS, file), scale: 'css' });
      shots.push(file);
    },
  });
  const backward: Walk = await tabWalk(page, { reverse: true });
  const f = cycleOf(forward.stops.map((s) => s.key));
  const b = cycleOf([...backward.stops.map((s) => s.key)].reverse());
  const reverseMatches = forward.closed && backward.closed && JSON.stringify(f) === JSON.stringify(b);
  const noIndicator = [...forward.stops, ...backward.stops]
    .filter((s) => s.indicator && !s.indicator.ok)
    .map((s) => `${s.desc}: ${s.indicator!.how}${s.indicator!.host ? ` on ${s.indicator!.host} (off screen)` : ''}`)
    .filter((d) => !seenNoIndicator.has(d));
  for (const d of noIndicator) seenNoIndicator.add(d);
  const r = await reach(page, forward, root, opts.exclude ?? []);
  expect(r.found, `${id}: ${root} is on the page`).toBe(true);
  const ptr = await pointerOnly(page, root);
  // names and roles, before the arrow keys change what is shown (a tab list switches its panel)
  const un = (await unnamed(cdp, root)).filter((u) => !seenUnnamed.has(u.html));
  for (const u of un) seenUnnamed.add(u.html);
  const roles = await rolesOf(cdp, forward);
  const noRole = roles.filter((x) => NO_ROLE.has(x.role)).map((x) => `${x.desc}: role "${x.role}"`);
  // composites: the arrow keys reach every member (each composite once in the file)
  const composites: SurfaceReport['composites'] = [];
  if (opts.composites !== false) {
    const byComp = new Map<number, Stop>();
    for (const s of forward.stops) if (s.composite !== null && !byComp.has(s.composite)) byComp.set(s.composite, s);
    for (const s of byComp.values()) {
      const inRoot = await page.evaluate(
        ({ i, r }) => (window as unknown as { __a11y: { els: Element[] } }).__a11y.els[i]!.closest(r) !== null,
        { i: s.id, r: root },
      );
      const name = s.desc.replace(/"[^"]*"$/, '') + (s.compositeRole ?? '');
      if (!inRoot || checkedComposites.has(name + root)) continue;
      checkedComposites.add(name + root);
      // the composite's own Tab stop (its roving member) takes the focus, as Tab gave it before
      const ok = await page.evaluate((i) => {
        const el = (window as unknown as { __a11y: { els: Element[] } }).__a11y.els[i] as HTMLElement;
        if (!el.isConnected) return false;
        el.focus();
        return document.activeElement === el;
      }, s.id);
      if (!ok) continue;
      composites.push(await arrowWalk(page, s));
    }
  }
  const rep: SurfaceReport = {
    stops: forward.stops.length,
    closed: forward.closed,
    loop: forward.loop,
    reverseMatches,
    noIndicator,
    unreached: r.unreached,
    pointerOnly: ptr,
    composites,
    unnamed: un,
    noRole: [...new Set(noRole)],
    shots,
    indicators: forward.stops.reduce<Record<string, number>>((m, s) => {
      const k = s.indicator?.how ?? 'none';
      m[k] = (m[k] ?? 0) + 1;
      return m;
    }, {}),
  };
  report[id] = rep;
  expect.soft(forward.closed, `${id}: Tab comes back to the first stop (a loop is a trap: ${forward.loop?.join(' → ')})`).toBe(true);
  if (opts.modal) expect.soft(forward.stops.every((s) => s.inModal), `${id}: the focus stays in the dialog`).toBe(true);
  else expect.soft(forward.stops.some((s) => s.inModal), `${id}: no dialog holds the focus`).toBe(false);
  expect.soft(reverseMatches, `${id}: Shift+Tab goes round the same stops backwards`).toBe(true);
  expect.soft(noIndicator, `${id}: stops without a focus indicator on the screen`).toEqual([]);
  expect.soft(r.unreached, `${id}: interactive elements no key reaches`).toEqual([]);
  expect.soft(ptr, `${id}: click or pointer handlers no key reaches`).toEqual([]);
  for (const c of composites) {
    expect.soft(c.missed, `${id}: members of ${c.composite} the arrow keys do not reach`).toEqual([]);
    expect.soft(c.restored, `${id}: the arrow keys bring the focus back to where it was in ${c.composite} (${c.start} → ${c.end}; ${c.trace?.join(', ')})`).toBe(true);
  }
  expect.soft(un, `${id}: interactive nodes with no accessible name`).toEqual([]);
  expect.soft(rep.noRole, `${id}: Tab stops with no role`).toEqual([]);
  return rep;
}

/**
 * A dialog opened by `open` (keys pressed on the focused opener): the walk stays inside it; Escape closes it and the
 * focus goes back to the opener.
 */
async function dialog(id: string, opener: Locator, open: () => Promise<void>, dialogRoot: string, opts: { composites?: boolean } = {}): Promise<void> {
  await expect(opener).toBeFocused();
  await open();
  await expect(page.locator(dialogRoot).first()).toBeVisible({ timeout: 20_000 });
  await surface(id, dialogRoot, { modal: true, composites: opts.composites });
  await page.keyboard.press('Escape');
  await expect(page.locator(dialogRoot)).toHaveCount(0);
  await expect.soft(opener, `${id}: Escape gives the focus back to what opened it`).toBeFocused();
}

// ---- the surfaces, reached and operated from the keyboard -------------------------------------------------------------

test('the project home with no project open, and the first path’s four stations', async () => {
  const open = page.getByRole('button', { name: 'Open project folder…' });
  await expect(open).toBeEnabled();
  await surface('home-empty', '.ph-page');
});

test('the Studio’s settings, from the header’s Settings icon by keys: the theme by the arrow keys, Close settings back', async () => {
  const icon = page.locator('.ph-top').getByRole('button', { name: 'Settings', exact: true });
  await tabTo(icon);
  await page.keyboard.press('Enter');
  const group = page.getByRole('radiogroup', { name: 'Theme' });
  await expect(group).toBeVisible();
  await surface('studio-settings', '.ss-page');
  const close = page.getByRole('button', { name: 'Close settings' });
  await tabTo(close);
  await page.keyboard.press('Enter');
  await expect(page.getByRole('button', { name: 'Open project folder…' })).toBeEnabled();
});

test('the project home of an open project, and Use with Claude Code opened from the keyboard', async () => {
  await stubDialog(studio.app, quick);
  await tabTo(page.getByRole('button', { name: 'Open project folder…' }));
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  await expect(page.locator('.ph-group[data-group="runs"] tr[data-run]').first()).toBeVisible({ timeout: 20_000 });
  await surface('home-open', '.ph-page');
  // ADR 0801 Addendum 23: the registration panel under the band, from its icon by keys; Escape closes it
  const icon = page.getByRole('button', { name: 'Use with Claude Code' });
  await tabTo(icon);
  await page.keyboard.press('Enter');
  const panel = page.getByRole('region', { name: 'Use with Claude Code' });
  await expect(panel.getByText('Not registered with Claude Code for this folder.')).toBeVisible({ timeout: 20_000 });
  await surface('claude-code', '.ph-claude');
  await tabTo(panel.getByRole('button', { name: 'Copy' }));
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
});

test('the Runs view, a run selected from the keyboard, and the provenance popover of its step count', async () => {
  await page.keyboard.press('Control+2');
  const grid = page.getByRole('grid', { name: /^Runs of / });
  await expect(grid).toBeVisible({ timeout: 20_000 });
  await surface('runs', '.rv', { exclude: ['.rv-run'] });
  // select RE100: into the grid, down its rows, Space
  await tabTo(grid);
  await arrowTo(grid.locator(`tr[data-run="${RE100}"], [role=row][data-key="${RE100}"]`).first(), 'ArrowDown');
  await page.keyboard.press('Space');
  const aside = page.locator(`aside.rv-run[data-run="${RE100}"]`);
  await expect(aside).toBeVisible();
  await expect(aside.locator('.rv-history, .rv-empty-line').first()).toBeVisible({ timeout: 20_000 });
  await surface('runs-selected', '.rv-run', { exclude: ['.pv-pop'] });
  const trigger = aside.getByRole('button', { name: /^Provenance of (iterations|steps): / }).first();
  await tabTo(trigger);
  await dialog('provenance', trigger, () => page.keyboard.press('Enter'), '.pv-pop');
});

test('the run workspace on a converged run, the field view’s keys, its derived-picture controls and compare mode', async () => {
  // the project tabs, from the keyboard: Workspace is the third
  const tabs = page.getByRole('tablist', { name: 'Project views' });
  await tabTo(tabs.getByRole('tab', { selected: true }));
  await arrowTo(tabs.getByRole('tab', { name: 'Workspace' }), 'ArrowRight');
  const list = page.getByRole('listbox', { name: 'Runs of the project' });
  await expect(list).toBeVisible({ timeout: 20_000 });
  await tabTo(list.getByRole('option').first());
  await arrowTo(list.locator(`[data-run="${RE100}"]`).first().locator('xpath=ancestor-or-self::*[@role="option"][1]'), 'ArrowDown');
  await expect(page.locator('.ws')).toHaveAttribute('data-run', RE100, { timeout: 20_000 });
  await expect(page.locator('.fv-frame[data-ready]')).toHaveCount(1, { timeout: 30_000 });
  await expect(page.locator('.fv-derived')).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(`Run workspace: ${RE100}`);
  await surface('workspace', '.ws', { exclude: ['.fv-derived', '.wk'] });
  await surface('derived-controls', '.fv-derived');

  // the run bands' names (F-816): one Tab stop, the arrows between them, Enter selects
  const names = page.getByRole('toolbar', { name: /^Run bands of / });
  await tabTo(names);
  await arrowTo(names.locator(`[data-band="${RE400}"]`), 'ArrowDown');
  await page.screenshot({ path: path.join(SHOTS, 'workspace-band-name-focus.png'), scale: 'css' });
  await page.keyboard.press('Enter');
  await expect(page.locator('.ws')).toHaveAttribute('data-run', RE400, { timeout: 20_000 });
  await expect(names.locator(`[data-band="${RE400}"]`)).toHaveAttribute('aria-current', 'true');
  await arrowTo(names.locator(`[data-band="${RE100}"]`), 'ArrowDown');
  await page.keyboard.press('Space');
  await expect(page.locator('.ws')).toHaveAttribute('data-run', RE100, { timeout: 20_000 });
  await expect(page.locator('.fv-frame[data-ready]')).toHaveCount(1, { timeout: 30_000 });

  // the select of the field: a list in a popover
  const fieldSelect = page.locator('.ws-field-select button').first();
  await tabTo(fieldSelect);
  await dialog('select-popover', fieldSelect, () => page.keyboard.press('Enter'), '.ui-popover');

  // compare: the toggle, the compared run from its select, the mode from its radio group
  const toggle = page.getByRole('button', { name: 'Compare runs', exact: true });
  await tabTo(toggle);
  await page.keyboard.press('Space');
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  const other = page.getByRole('button', { name: /Compared run/ });
  await tabTo(other);
  await page.keyboard.press('Enter');
  await expect(page.locator('.ui-popover').getByRole('option').first()).toBeFocused();
  await arrowTo(page.locator('.ui-popover').getByRole('option', { name: new RegExp(RE400) }), 'ArrowDown');
  await page.keyboard.press('Enter');
  await tabTo(page.getByRole('radio').first());
  await arrowTo(page.getByRole('radio', { name: 'Side by side', exact: true }), 'ArrowRight');
  // a snapshot time both runs stored: the eighth stop, with the scrubber's keys
  await tabTo(page.getByRole('slider', { name: /^Time of / }));
  await page.keyboard.press('Home');
  for (let i = 0; i < 7; i++) await page.keyboard.press('ArrowRight');
  await expect(page.locator('.cmp-view[data-view="b"] .fv-frame[data-ready]')).toHaveCount(1, { timeout: 30_000 });
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(`Compare: ${RE100} with ${RE400}`);
  await surface('compare', '.ws', { exclude: ['.fv-derived', '.wk'] });
  await tabTo(toggle);
  await page.keyboard.press('Space');
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
});

test('the command palette, from Ctrl+K: inside it, its options by the arrow keys, Escape back', async () => {
  const before = page.getByRole('button', { name: 'Compare runs', exact: true });
  await tabTo(before);
  await dialog('palette', before, () => page.keyboard.press('Control+K'), '.cp-dialog');
});

test('the case editor on a case file, and its undo controls', async () => {
  await page.keyboard.press('Control+1');
  const edit = page.getByRole('button', { name: 'Edit cases/cavity-re100.toml', exact: true });
  await tabTo(edit);
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 2, name: 'cases/cavity-re100.toml', exact: true })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Case editor: cavity-re100.toml');
  await surface('case-editor', '.st-page', { exclude: ['.hi-controls', '.wk'] });
  await surface('undo', '.hi-controls', { exclude: ['.hi-panel'] });
});

test('case settings from the palette, a session from the engine’s defaults, and the decision Close session asks for', async () => {
  await page.keyboard.press('Control+K');
  await page.keyboard.type('Go to case settings');
  await page.keyboard.press('Enter');
  const fresh = page.getByRole('button', { name: 'New session from defaults' });
  await tabTo(fresh);
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible({ timeout: 20_000 });
  await surface('settings', '.st-page', { exclude: ['.hi-controls', '.hi-panel'] });
  const close = page.getByRole('button', { name: 'Close session' });
  await tabTo(close);
  await page.keyboard.press('Enter');
  const panel = page.getByRole('alertdialog', { name: /^Close session / });
  await expect(panel).toBeVisible();
  // the panel takes the focus (the safe choice); Tab stays in it; Escape keeps the session and gives the focus back
  await expect(panel.getByRole('button', { name: 'Keep the session' })).toBeFocused();
  await surface('decision', '.hi-panel', { modal: true });
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect.soft(close, 'decision: Escape gives the focus back to Close session').toBeFocused();
});

// ---- reduced motion ---------------------------------------------------------------------------------------------------

test('reduced motion: no CSS animation or transition runs, Play steps at most twice a second, the camera moves at once', async () => {
  // ADR 0808 Addendum 5 (S1): the step the retired first-path case gave this case, without its surface: the home, and
  // the empty project opened from it
  await page.keyboard.press('Control+1');
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  await stubDialog(studio.app, empty);
  await tabTo(page.getByRole('button', { name: 'Open project…' }));
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 1, name: 'An empty project' })).toBeVisible({ timeout: 30_000 });
  // reopen the quick start so the run bands are drawn again
  await stubDialog(studio.app, quick);
  await page.keyboard.press('Control+O');
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });

  // the check sees motion when there is some: without the preference the run bands grow in and switches slide
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.keyboard.press('Control+2');
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible({ timeout: 20_000 });
  const tabs = page.getByRole('tablist', { name: 'Project views' });
  await tabTo(tabs.getByRole('tab', { selected: true }));
  await arrowTo(tabs.getByRole('tab', { name: 'Workspace' }), 'ArrowRight');
  await expect(page.locator('.tp-seg').first()).toBeVisible({ timeout: 30_000 });
  const motionOn = await motionOf(page, MOTION_MS);
  expect(motionOn.declared.some((d) => d.includes('tp-grow')), 'without reduced motion the bands grow in (the check sees it)').toBe(true);

  await page.emulateMedia({ reducedMotion: 'reduce' });
  expect(await page.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches)).toBe(true);
  const motionOff: Record<string, unknown> = {};
  // every view, at rest and while its parts move
  for (const [view, key] of [
    ['workspace', null],
    ['home', 'Control+1'],
    ['runs', 'Control+2'],
  ] as const) {
    if (key) await page.keyboard.press(key);
    await page.waitForTimeout(300);
    const m = await motionOf(page, MOTION_MS);
    motionOff[view] = m;
    expect.soft(m.declared, `${view}: CSS motion declared under reduced motion`).toEqual([]);
    expect.soft(m.running, `${view}: animations running under reduced motion`).toEqual([]);
  }

  // Play (a JavaScript animation of the time line): at most one stop per 500 ms under reduced motion
  await tabTo(tabs.getByRole('tab', { selected: true }));
  await arrowTo(tabs.getByRole('tab', { name: 'Workspace' }), 'ArrowRight');
  const list = page.getByRole('listbox', { name: 'Runs of the project' });
  await expect(list.locator(`[data-run="${RE100}"]`).first()).toBeVisible({ timeout: 20_000 });
  await expect(page.locator('.ws')).toHaveAttribute('data-run', /r-/, { timeout: 20_000 });
  const slider = page.getByRole('slider', { name: /^Time of / });
  await tabTo(slider);
  await page.keyboard.press('Home');
  const play = page.getByRole('button', { name: 'Play', exact: true });
  await tabTo(play);
  const times: number[] = await (async () => {
    await page.evaluate(() => {
      const w = window as unknown as { __stops: number[]; __obs?: MutationObserver };
      w.__stops = [];
      const ws = document.querySelector('.ws')!;
      w.__obs = new MutationObserver(() => w.__stops.push(performance.now()));
      w.__obs.observe(ws, { attributes: true, attributeFilter: ['data-index'] });
    });
    await page.keyboard.press('Enter');
    await page.waitForTimeout(2600);
    await page.keyboard.press('Enter'); // Pause
    return page.evaluate(() => {
      const w = window as unknown as { __stops: number[]; __obs?: MutationObserver };
      w.__obs?.disconnect();
      return w.__stops;
    });
  })();
  const gaps = times.slice(1).map((t, i) => t - times[i]!);
  motionOff['play'] = { stops: times.length, gapsMs: gaps.map((g) => Math.round(g)) };
  expect(times.length, 'Play moved the line').toBeGreaterThan(1);
  expect.soft(Math.min(...gaps), 'Play under reduced motion: at least 450 ms between stops').toBeGreaterThanOrEqual(450);

  // the camera: one key moves it at once (no easing, no damping): the picture 150 ms after the key is the picture
  // 900 ms after it, and differs from the one before the key
  const frame = page.locator('.fv-frame').first();
  await tabTo(frame);
  const box = (await frame.boundingBox())!;
  const clip = { x: box.x + 2, y: box.y + 2, width: box.width - 4, height: box.height - 4 };
  const before = await page.screenshot({ clip });
  const frames = await framesDuring(page, 150, () => page.keyboard.press('ArrowLeft'));
  const soon = await page.screenshot({ clip });
  await page.waitForTimeout(750);
  const later = await page.screenshot({ clip });
  motionOff['camera'] = { moved: !soon.equals(before), settledAt150ms: soon.equals(later), animationFramesAskedIn150ms: frames };
  expect(soon.equals(before), 'the orbit key moved the camera').toBe(false);
  expect.soft(soon.equals(later), 'the camera is where it ends 150 ms after the key (no easing)').toBe(true);
  report['reduced-motion'] = { on: motionOn.declared.slice(0, 20), off: motionOff };
  await page.emulateMedia({ reducedMotion: null });
});

// ---- state not carried by colour alone --------------------------------------------------------------------------------

test('state is never carried by colour alone: each state colour has a pictogram, a shape, a position or a word', async () => {
  // the run workspace of the quick start (left open by the reduced-motion test)
  await expect(page.locator('.tp-band-row').first()).toBeVisible({ timeout: 20_000 });
  const live = await page.evaluate(() => {
    const out: { state: string; where: string; carrier: string; ok: boolean }[] = [];
    const named = (e: Element) => (e.getAttribute('aria-label') ?? '').trim() !== '';
    // run state: a pictogram of its own shape, and the word (shown, or the name of the role img)
    for (const m of Array.from(document.querySelectorAll('.ui-state'))) {
      const svg = m.querySelector('svg[data-state]');
      const word = m.querySelector('.ui-state-word')?.textContent?.trim() ?? '';
      const ok = svg !== null && (word !== '' || (m.getAttribute('role') === 'img' && named(m)));
      const where = m.parentElement?.className.split(' ')[0] ?? '';
      out.push({ state: `run state ${svg?.getAttribute('data-state') ?? '?'}`, where, carrier: svg ? (word ? 'pictogram + word' : 'pictogram + name') : 'none', ok });
    }
    // band ends: the end's pictogram, the state word as its name; the selected band: bold name and aria-current
    for (const r of Array.from(document.querySelectorAll('.tp-band-row'))) {
      const end = r.querySelector('.tp-end');
      const ok =
        end !== null && end.getAttribute('role') === 'img' && named(end) && (r.getAttribute('data-state') === 'queued' || end.querySelector('svg[data-state]') !== null);
      out.push({ state: `band end ${r.getAttribute('data-state')}`, where: 'time panel', carrier: 'pictogram + name, at the end of the band', ok });
      if (r.classList.contains('tp-band-row--selected')) {
        const n = r.querySelector('.tp-band-name')!;
        const bold = Number(getComputedStyle(n).fontWeight) >= 600;
        out.push({ state: 'band selected', where: 'time panel', carrier: 'bold name + aria-current', ok: bold && n.getAttribute('aria-current') === 'true' });
      }
    }
    // the shapes of the run-state pictograms differ from state to state
    const shapes = new Map<string, string>();
    for (const svg of Array.from(document.querySelectorAll('svg.ui-picto[data-state]'))) shapes.set(svg.getAttribute('data-state')!, svg.innerHTML);
    out.push({
      state: 'run states',
      where: 'pictograms',
      carrier: `${shapes.size} states shown, ${new Set(shapes.values()).size} shapes`,
      ok: new Set(shapes.values()).size === shapes.size,
    });
    return out;
  });

  // the cues that live in the stylesheets, read from elements made for the check (and removed at once): each state's
  // look differs by something that is not a hue
  const css = await page.evaluate(() => {
    const host = document.createElement('div');
    host.style.cssText = 'position:fixed;left:0;top:0;width:400px;opacity:0;pointer-events:none';
    host.innerHTML = [
      '<div class="ui-switch" id="sw-off"><span class="ui-switch-track"><span class="ui-switch-knob"></span></span></div>',
      '<div class="ui-switch" data-selected="true" id="sw-on"><span class="ui-switch-track"><span class="ui-switch-knob"></span></span></div>',
      '<button class="ui-button" id="b-on">x</button><button class="ui-button" data-disabled="true" id="b-off">x</button>',
      '<div class="ui-tabs--sub"><div class="ui-tab" id="t-off">x</div><div class="ui-tab" data-selected="true" id="t-on">x</div></div>',
      '<div class="ui-tabs--project"><div class="ui-tab" id="pt-off">x</div><div class="ui-tab" data-selected="true" id="pt-on">x</div></div>',
      '<div class="ui-option" id="o-off">x</div><div class="ui-option" data-selected="true" id="o-on">x</div>',
      '<table><tr class="ui-row"><td class="ui-cell" id="r-off">x</td></tr><tr class="ui-row" data-selected="true"><td class="ui-cell" id="r-on">x</td></tr></table>',
      '<div class="ws-item" id="w-off">x</div><div class="ws-item" data-selected="true" id="w-on">x</div>',
      '<div class="st-index-item" id="i-off">x</div><div class="st-index-item" data-selected="true" id="i-on">x</div>',
      '<div class="tp-track" style="position:relative;height:20px"><span class="tp-seg tp-seg--iterating" id="g-it" style="left:0;width:40%"></span><span class="tp-seg tp-seg--live" id="g-live" style="left:50%;width:40%"></span></div>',
      '<span class="ph-live" id="l-on"><span class="ph-live-mark"></span></span>',
    ].join('');
    document.body.append(host);
    const cs = (id: string, pseudo?: string) => getComputedStyle(document.getElementById(id)!, pseudo ?? null);
    const knob = (id: string) => getComputedStyle(document.querySelector(`#${id} .ui-switch-knob`)!).transform;
    const w = (id: string) => Number(cs(id).fontWeight);
    const out = [
      { state: 'switch on', carrier: 'knob position', ok: knob('sw-on') !== knob('sw-off') },
      { state: 'unavailable control', carrier: 'dashed border', ok: cs('b-off').borderTopStyle === 'dashed' && cs('b-on').borderTopStyle !== 'dashed' },
      { state: 'selected sub-tab', carrier: 'bold', ok: w('t-on') > w('t-off') },
      { state: 'selected project tab', carrier: 'bold, a white tab open to the page', ok: w('pt-on') > w('pt-off') },
      { state: 'selected option', carrier: 'bold + check mark', ok: w('o-on') > w('o-off') },
      { state: 'selected table row', carrier: 'ink box', ok: cs('r-on').boxShadow !== 'none' && cs('r-off').boxShadow === 'none' },
      { state: 'selected run in the workspace list', carrier: 'ink box', ok: cs('w-on').outlineStyle !== 'none' && cs('w-off').outlineStyle === 'none' },
      { state: 'selected settings section', carrier: 'bold + ink box', ok: w('i-on') > w('i-off') && cs('i-on').boxShadow !== 'none' },
      {
        state: 'live band segment',
        carrier: 'an ink edge at its growing end + "live" in the end’s description and tooltip',
        ok: cs('g-live', '::after').content !== 'none' && parseFloat(cs('g-live', '::after').width) > 0 && cs('g-it', '::after').content === 'none',
      },
      {
        state: 'project folder live',
        carrier: 'an ink square; Not live is the failed pictogram and words',
        ok: parseFloat(getComputedStyle(document.querySelector('#l-on .ph-live-mark')!).width) > 0,
      },
    ];
    host.remove();
    return out;
  });
  const all = [...live, ...css.map((c) => ({ ...c, where: 'stylesheet' }))];
  report['colour'] = { checks: all };
  expect(live.length, 'states were found on the page').toBeGreaterThan(5);
  expect(all.filter((c) => !c.ok)).toEqual([]);
});

test('every surface was walked', () => {
  const walked = Object.keys(report).filter((k) => k !== 'reduced-motion' && k !== 'colour');
  expect(walked.sort()).toEqual(
    [
      'home-empty',
      'home-open',
      'claude-code',
      'runs',
      'runs-selected',
      'provenance',
      'workspace',
      'derived-controls',
      'select-popover',
      'compare',
      'palette',
      'case-editor',
      'undo',
      'settings',
      'decision',
      'studio-settings',
    ].sort(),
  );
  // axe-core ran on every walked surface: home, case editor, runs, run workspace, compare, provenance, the palette and
  // the decision panel among them
  expect(Object.keys(axeReport).sort()).toEqual(walked.sort());
  // F-816 (the maintainer, 2026-11-17): a readers-only h1 on the run workspace, compare, the case editor and undo
  for (const id of ['workspace', 'compare', 'case-editor', 'undo']) {
    expect(axeReport[id]!.other.map((f) => f.rule), id).not.toContain('page-has-heading-one');
  }
});
