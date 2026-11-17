// The command palette and the keyboard shortcuts (F-831; G20, G8) in the packaged Studio, from the keyboard alone, against
// the fake service (tests/support/fake-service.ts) and the engine's project server (rcfd-mcp) on a copy of the
// quick-start project:
//
// - Ctrl+K opens the palette on its search field; an unavailable command is listed with its reason; Escape closes it and
//   the focus is back where it was;
// - Ctrl+O opens a project folder; Ctrl+1 and Ctrl+2 go to Home and Runs;
// - every command is reached with the arrow keys, Home and End go to the ends;
// - an action is found by its name and run (List sessions); New session asks its optional parameters; Show run status
//   asks for the session, chosen from the service's list;
// - F5 and Ctrl+. ask which case and which run, from the project's lists, and say why a finished run cannot be stopped;
// - the "Keyboard shortcuts" listing holds the whole table and is reached from the palette and by Ctrl+/;
// - no shortcut takes a key of the running application menu, and F5 does not reload the window.
// Screenshots at 1440 by 900 go to test-results-visual/f831/ for the maintainer's view-and-report (G13).
import { expect, test, type Page } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { FakeService } from '../support/fake-service.js';
import { launchStudio, untilState, type Studio } from '../support/studio.js';
import { appRoot } from '../support/rcfd.js';
import { copyExample, findMcp, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { fromAccelerator, paletteModel, SHORTCUTS } from '../../src/shared/commands/index.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f831');
const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

let fake: FakeService;
let studio: Studio;
let page: Page;
let quick = '';
const consoleErrors: string[] = [];
const model = paletteModel();

const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const dialog = () => page.getByRole('dialog', { name: 'Command palette' });
const search = () => dialog().getByLabel('Find a command by its name or its id');

/** The data-key of the option the search field's aria-activedescendant names (the virtually focused one). */
const active = (): Promise<string | null> =>
  page.evaluate(() => {
    const input = document.activeElement as HTMLElement | null;
    const id = input?.getAttribute('aria-activedescendant');
    return id ? (document.getElementById(id)?.getAttribute('data-key') ?? null) : null;
  });

/** What has the focus: its role or tag and its accessible name, enough to say it is the same element again. */
const focusedName = (): Promise<string> =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return 'body';
    return `${el.getAttribute('role') ?? el.tagName.toLowerCase()}:${el.getAttribute('aria-label') ?? el.textContent?.trim() ?? ''}`;
  });

async function closePalette(): Promise<void> {
  await page.keyboard.press('Escape');
  await expect(dialog()).toBeHidden();
}

test.beforeAll(async () => {
  await mkdir(SHOTS, { recursive: true });
  const dir = await scratch('f831');
  quick = await copyExample('quick-start', dir);
  fake = await FakeService.start();
  studio = await launchStudio({
    RCFD_STUDIO_SERVICE: `127.0.0.1:${fake.port}`,
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_RECENT: path.join(dir, 'recent.json'),
  });
  page = studio.page;
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  await page.evaluate(() => {
    const w = window as unknown as { __csp: string[] };
    w.__csp = [];
    document.addEventListener('securitypolicyviolation', (e) => w.__csp.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await untilState(page, (s) => s.status === 'connected', 15_000, 'connected to the fake service');
});

test.afterAll(async () => {
  await studio?.app.close();
  await fake?.stop();
});

test('Ctrl+K opens the palette on its search field; an unavailable command keeps its place and says why; Escape gives the focus back', async () => {
  const open = page.getByRole('button', { name: 'Open project folder…' });
  await open.focus();
  const before = await focusedName();
  await page.keyboard.press('Control+K');
  await expect(dialog()).toBeVisible();
  await expect(search()).toBeFocused();
  // every command is listed, none hidden
  await expect(dialog().getByRole('option')).toHaveCount(model.length);
  await shot('palette-no-project');

  await page.keyboard.type('Go to Runs');
  const runs = dialog().getByRole('option', { name: 'Go to Runs' });
  await expect(runs).toBeVisible();
  await expect(runs).toContainText('Unavailable: No project is open');
  await expect.poll(active).toBe('go.runs');
  await page.keyboard.press('Enter');
  await expect(dialog().getByRole('heading', { name: 'Go to Runs' })).toBeVisible();
  await expect(dialog()).toContainText('Unavailable. No project is open: open a project folder first');
  await expect(dialog().getByRole('button', { name: 'Back to commands' })).toBeFocused();
  await shot('palette-unavailable');

  await closePalette();
  expect(await focusedName()).toBe(before);
  await expect(open).toBeFocused();
});

test('Ctrl+O opens a project folder from the keyboard', async () => {
  await stubDialog(studio.app, quick);
  await page.keyboard.press('Control+O');
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
});

test('every command is reached with the arrow keys; Home and End go to the first and the last', async () => {
  await page.getByRole('tab', { name: 'Home' }).focus();
  await page.keyboard.press('Control+K');
  await expect(search()).toBeFocused();
  const options = dialog().getByRole('option');
  await expect(options).toHaveCount(model.length);
  const keys = await options.evaluateAll((els) => els.map((e) => e.getAttribute('data-key')));
  expect(keys).toEqual(model.map((e) => e.id));

  // the active descendant follows a key after a short delay (React Aria queues it), so each step waits for the change
  const visited: (string | null)[] = [];
  let prev: string | null = null;
  for (let i = 0; i < model.length; i++) {
    await page.keyboard.press('ArrowDown');
    await expect.poll(active).not.toBe(prev);
    prev = await active();
    visited.push(prev);
  }
  expect(visited).toEqual(model.map((e) => e.id));
  await page.keyboard.press('ArrowUp');
  await expect.poll(active).toBe(model[model.length - 2]!.id);
  await page.keyboard.press('Home');
  await expect.poll(active).toBe(model[0]!.id);
  await page.keyboard.press('End');
  await expect.poll(active).toBe(model[model.length - 1]!.id);
  await page.keyboard.press('Home');
  await expect.poll(active).toBe(model[0]!.id);
  await shot('palette-project');
  await closePalette();
  await expect(page.getByRole('tab', { name: 'Home' })).toBeFocused();
});

test('an action is found by its name and run; its reply and its CLI and Claude Code equivalents are shown', async () => {
  const home = page.getByRole('tab', { name: 'Home' });
  await home.focus();
  await page.keyboard.press('Control+K');
  await page.keyboard.type('list sessions');
  await expect.poll(active).toBe('session.list');
  await page.keyboard.press('Enter');
  await expect(dialog().getByRole('heading', { name: 'List sessions: done' })).toBeVisible();
  await expect(dialog().getByLabel('The engine’s reply')).toContainText('"sessions"');
  await expect(dialog().getByRole('list', { name: 'The same request elsewhere' })).toContainText("rcfd call --addr 127.0.0.1:");
  await expect(dialog().getByRole('button', { name: 'Done' })).toBeFocused();
  expect(fake.methods()).toContain('session.list');
  await shot('palette-result');
  await closePalette();
  await expect(home).toBeFocused();

  // New session: its parameters are all optional; left empty, the engine's defaults apply (nothing is made up)
  await page.keyboard.press('Control+K');
  await page.keyboard.type('New session');
  await expect.poll(active).toBe('session.create');
  await page.keyboard.press('Enter');
  const form = dialog().getByRole('form', { name: 'Parameters of New session' });
  await expect(form).toBeVisible();
  await expect(form.getByLabel('toml (optional)')).toBeFocused();
  await shot('palette-form');
  const created = fake.received.length;
  await page.keyboard.press('Enter');
  await expect(dialog().getByRole('heading', { name: 'New session: done' })).toBeVisible();
  const call = fake.received.slice(created).find((r) => r.method === 'session.create');
  expect(call?.params ?? {}).toEqual({});

  // Show run status asks for the session, chosen from session.list
  await page.keyboard.press('Control+K');
  await page.keyboard.type('run.status');
  await expect.poll(active).toBe('run.status');
  await page.keyboard.press('Enter');
  const choose = dialog().getByRole('listbox', { name: 'Choose a session' });
  await expect(choose).toBeVisible();
  await expect(choose.getByRole('option')).toHaveCount(1);
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect(dialog().getByRole('heading', { name: 'Show run status: done' })).toBeVisible();
  expect(fake.received.filter((r) => r.method === 'run.status').at(-1)?.params).toEqual({ session: 's1' });
  await closePalette();
});

test('F5 and Ctrl+. ask which case and which run, from the project’s lists; a finished run cannot be stopped, and the palette says why', async () => {
  await page.keyboard.press('F5');
  const cases = dialog().getByRole('listbox', { name: 'Choose a case file' });
  await expect(cases).toBeVisible();
  await expect(cases.getByRole('option')).toHaveText([
    /cases\/cavity-re100\.toml/,
    /cases\/cavity-re400\.toml/,
    /cases\/heated-cavity-ra1e3\.toml/,
    /cases\/heated-cavity-ra1e4\.toml/,
  ]);
  await shot('palette-pick-case');
  await closePalette();

  await page.keyboard.press('Control+.');
  const runs = dialog().getByRole('listbox', { name: 'Choose a run' });
  await expect(runs).toBeVisible();
  await expect(runs.getByRole('option')).toHaveCount(4);
  await expect(runs.getByRole('option').first()).toContainText('Unavailable: The run has already finished');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect(dialog()).toContainText('Unavailable. The run has already finished: there is nothing to stop.');
  await closePalette();
});

test('Ctrl+1 and Ctrl+2 go to Home and Runs; the palette goes to Case settings and back', async () => {
  await page.keyboard.press('Control+2');
  await expect(page.getByRole('tab', { name: 'Runs' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('tab', { name: 'Runs' })).toBeFocused();
  await page.keyboard.press('Control+1');
  await expect(page.getByRole('tab', { name: 'Home' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('tab', { name: 'Home' })).toBeFocused();

  await page.keyboard.press('Control+K');
  await page.keyboard.type('case settings');
  await expect.poll(active).toBe('go.settings');
  await page.keyboard.press('Enter');
  await expect.poll(() => page.evaluate(() => location.hash)).toBe('#/settings');
  await page.keyboard.press('Control+2');
  await expect(page.getByRole('tab', { name: 'Runs' })).toHaveAttribute('aria-selected', 'true', { timeout: 10_000 });
  await page.keyboard.press('Control+1');
  await expect(page.getByRole('tab', { name: 'Home' })).toHaveAttribute('aria-selected', 'true');
});

test('the keyboard shortcuts listing holds the whole table, from the palette and by Ctrl+/', async () => {
  await page.keyboard.press('Control+K');
  await page.keyboard.type('keyboard shortcuts');
  await expect.poll(active).toBe('help.shortcuts');
  await page.keyboard.press('Enter');
  const listing = dialog().getByRole('region', { name: 'Keyboard shortcuts by where they work' });
  await expect(listing).toBeVisible();
  await expect(listing.getByRole('row')).toHaveCount(SHORTCUTS.length);
  await expect(listing.getByRole('row').filter({ hasText: 'kept: F-829' })).toHaveCount(3);
  await expect(listing.getByRole('row').filter({ hasText: 'kept: F-810' })).toHaveCount(6);
  await shot('palette-shortcuts');
  await closePalette();
  await page.keyboard.press('Control+/');
  await expect(listing).toBeVisible();
  await page.keyboard.press('Tab');
  await closePalette();
});

test('Ctrl+Shift+P opens the palette too, and F5 does not reload the window', async () => {
  await page.evaluate(() => void ((window as unknown as { __mark: number }).__mark = 831));
  await page.keyboard.press('Control+Shift+P');
  await expect(search()).toBeFocused();
  await closePalette();
  await page.keyboard.press('F5');
  await expect(dialog()).toBeVisible();
  await closePalette();
  expect(await page.evaluate(() => (window as unknown as { __mark?: number }).__mark)).toBe(831);
});

/** The default accelerators of Electron's menu roles (menu-item-roles), on Windows and Linux. */
const ROLE_KEYS: Readonly<Record<string, string | null>> = {
  undo: 'CommandOrControl+Z',
  redo: process.platform === 'win32' ? 'Control+Y' : 'Shift+CommandOrControl+Z',
  cut: 'CommandOrControl+X',
  copy: 'CommandOrControl+C',
  paste: 'CommandOrControl+V',
  pasteandmatchstyle: 'Shift+CommandOrControl+V',
  selectall: 'CommandOrControl+A',
  delete: null,
  reload: 'CmdOrCtrl+R',
  forcereload: 'Shift+CmdOrCtrl+R',
  toggledevtools: 'Ctrl+Shift+I',
  resetzoom: 'CommandOrControl+0',
  zoomin: 'CommandOrControl+Plus',
  zoomout: 'CommandOrControl+-',
  togglefullscreen: 'F11',
  minimize: 'CommandOrControl+M',
  close: 'CommandOrControl+W',
  quit: process.platform === 'win32' ? null : 'CommandOrControl+Q',
  about: null,
  zoom: null,
  front: null,
  window: null,
  help: null,
  filemenu: null,
  editmenu: null,
  viewmenu: null,
  windowmenu: null,
  appmenu: null,
  togglespellchecker: null,
};

test('no shortcut of the Studio takes a key of the running application menu', async () => {
  const items = await studio.app.evaluate(({ Menu }) => {
    const out: { role: string | null; accelerator: string | null; label: string }[] = [];
    type M = { readonly items: readonly { role?: unknown; accelerator?: unknown; label: string; submenu?: M | null }[] };
    const walk = (m: M | null) => {
      for (const it of m?.items ?? []) {
        out.push({ role: it.role ? String(it.role) : null, accelerator: it.accelerator ? String(it.accelerator) : null, label: it.label });
        walk(it.submenu ?? null);
      }
    };
    walk(Menu.getApplicationMenu() as unknown as M | null);
    return out;
  });
  expect(items.length, 'the Studio keeps Electron’s default menu (ADR 0801)').toBeGreaterThan(5);
  const unknown = items.filter((i) => i.role && i.accelerator === null && !(i.role.toLowerCase() in ROLE_KEYS)).map((i) => i.role);
  expect(unknown, 'a menu role this test does not know: add its default accelerator to ROLE_KEYS').toEqual([]);
  const chords = [
    ...new Set(
      items
        .map((i) => i.accelerator ?? (i.role ? ROLE_KEYS[i.role.toLowerCase()] : null) ?? null)
        .filter((a): a is string => a !== null)
        .map(fromAccelerator),
    ),
  ];
  expect(chords.length).toBeGreaterThan(8);
  const reserved = new Set(SHORTCUTS.filter((s) => s.reserved && s.scope === 'global').map((s) => s.keys));
  expect(chords.filter((c) => !reserved.has(c)), 'menu keys the table does not reserve').toEqual([]);
  const ours = SHORTCUTS.filter((s) => !s.reserved).map((s) => s.keys);
  expect(chords.filter((c) => ours.includes(c))).toEqual([]);
});

test('no Content Security Policy violation and no console error', async () => {
  expect(await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp)).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
