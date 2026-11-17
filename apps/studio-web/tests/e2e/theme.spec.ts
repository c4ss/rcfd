// F-818, the theme setting in the packaged Studio: Light by default (DESIGN.md's tokens, the light native theme and a
// white window), the header's theme control and the palette's Theme commands switch the tokens at once with no reload,
// the native theme and the window background follow, the setting persists across a restart (and a dark start opens on
// the dark ground), and System follows the emulated colour scheme. The setting is kept in a scratch file
// (RCFD_STUDIO_PREFERENCES), never in the user data the other tests share.
import { _electron as electron, expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { themes, type ThemeName } from '../../src/renderer/design/tokens.js';
import { appRoot } from '../support/rcfd.js';

test.describe.configure({ mode: 'serial' });

let dir = '';
let prefs = '';
let app: ElectronApplication;
let page: Page;

async function launch(): Promise<void> {
  const { executable } = JSON.parse(readFileSync(path.join(appRoot, 'out', 'packaged.json'), 'utf8')) as { executable: string };
  const env: Record<string, string> = {};
  for (const [k, v] of Object.entries(process.env)) if (v !== undefined && !k.startsWith('RCFD_STUDIO_')) env[k] = v;
  app = await electron.launch({
    executablePath: executable,
    args: [],
    env: { ...env, RCFD_STUDIO_PREFERENCES: prefs, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') },
  });
  page = await app.firstWindow();
  await expect(page.getByRole('heading', { name: 'No project open' })).toBeVisible();
}

/** A hex colour in six lower-case digits (the built CSS writes #ffffff as #fff). */
const hex6 = (v: string): string => {
  const h = v.trim().toLowerCase();
  return /^#[0-9a-f]{3}$/.test(h) ? `#${[...h.slice(1)].map((c) => c + c).join('')}` : h;
};

/** What the window draws: data-theme, two tokens and the body's ground as computed. */
const drawn = async () => {
  const d = await page.evaluate(() => {
    const cs = getComputedStyle(document.documentElement);
    return {
      theme: document.documentElement.dataset['theme'] ?? null,
      white: cs.getPropertyValue('--color-white').trim(),
      ink: cs.getPropertyValue('--color-ink').trim(),
      ground: getComputedStyle(document.body).backgroundColor,
    };
  });
  return { ...d, white: hex6(d.white), ink: hex6(d.ink) };
};

const rgb = (hex: string): string => {
  const n = parseInt(hex.slice(1), 16);
  return `rgb(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255})`;
};

const expected = (t: ThemeName) => ({ theme: t, white: themes[t].white, ink: themes[t].ink, ground: rgb(themes[t].white) });

/** The main process's side: nativeTheme.themeSource and the window's background colour. */
const native = () =>
  app.evaluate(({ BrowserWindow, nativeTheme }) => ({
    source: nativeTheme.themeSource,
    background: BrowserWindow.getAllWindows()[0]!.getBackgroundColor().toLowerCase(),
  }));

const palette = () => page.getByRole('dialog', { name: 'Command palette' });

async function byPalette(title: string): Promise<void> {
  await page.keyboard.press('Control+K');
  await expect(palette()).toBeVisible();
  await page.keyboard.type(title);
  await expect(palette().getByRole('option').first()).toContainText(title);
  await page.keyboard.press('Enter');
  await expect(palette()).toHaveCount(0);
}

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f818-'));
  prefs = path.join(dir, 'studio-preferences.json');
  await launch();
});

test.afterAll(async () => {
  await app?.close();
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('Light is the default: DESIGN.md’s tokens, the light native theme and a white window', async () => {
  await expect.poll(drawn).toEqual(expected('light'));
  expect(await native()).toEqual({ source: 'light', background: themes.light.white });
  expect(await page.evaluate(() => window.studio.theme.get())).toMatchObject({ setting: 'light' });
});

test('the header’s theme control switches the tokens at once, with no reload; the native theme and the window follow', async () => {
  await page.evaluate(() => ((window as unknown as { __noReload: number }).__noReload = 818));
  const control = page.getByRole('button', { name: /Theme/ });
  await expect(control).toHaveAccessibleName(/Light/);
  await control.click();
  await page.getByRole('option', { name: 'Dark', exact: true }).click();
  await expect.poll(drawn).toEqual(expected('dark'));
  await expect(control).toHaveAccessibleName(/Dark/);
  await expect.poll(native).toEqual({ source: 'dark', background: themes.dark.white });
  expect(await page.evaluate(() => (window as unknown as { __noReload?: number }).__noReload)).toBe(818);
  // kept by the main process in the scratch file
  await expect.poll(async () => JSON.parse(await readFile(prefs, 'utf8')).theme).toBe('dark');
});

test('the palette’s Theme commands switch it too: High contrast', async () => {
  await byPalette('Theme: High contrast');
  await expect.poll(drawn).toEqual(expected('contrast'));
  await expect.poll(native).toEqual({ source: 'light', background: themes.contrast.white });
  expect(await page.evaluate(() => (window as unknown as { __noReload?: number }).__noReload)).toBe(818);
});

test('the setting persists across a restart', async () => {
  await app.close();
  await launch();
  await expect.poll(drawn).toEqual(expected('contrast'));
  expect(await native()).toEqual({ source: 'light', background: themes.contrast.white });
  await expect(page.getByRole('button', { name: /Theme/ })).toHaveAccessibleName(/High contrast/);
});

test('a dark start: the window opens on the dark ground, and the page is dark before and after the setting is read', async () => {
  await byPalette('Theme: Dark');
  await expect.poll(drawn).toEqual(expected('dark'));
  await app.close();
  await launch();
  expect(await native()).toEqual({ source: 'dark', background: themes.dark.white });
  // tokens.css draws the dark theme for a dark system before data-theme is set (nativeTheme makes the page's scheme
  // dark). Playwright emulates a light scheme on every page it drives; without its emulation the scheme is Electron's.
  await page.emulateMedia({ colorScheme: null });
  expect(await page.evaluate(() => matchMedia('(prefers-color-scheme: dark)').matches)).toBe(true);
  const before = await page.evaluate(() => {
    const t = document.documentElement.dataset['theme'];
    delete document.documentElement.dataset['theme'];
    const v = getComputedStyle(document.documentElement).getPropertyValue('--color-white').trim();
    if (t) document.documentElement.dataset['theme'] = t;
    return v;
  });
  expect(hex6(before)).toBe(themes.dark.white);
  await expect.poll(drawn).toEqual(expected('dark'));
});

test('System follows the colour scheme: dark when it is dark, light when it is light', async () => {
  await byPalette('Theme: System');
  await expect.poll(async () => (await native()).source).toBe('system');
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect.poll(drawn).toEqual(expected('dark'));
  await page.emulateMedia({ colorScheme: 'light' });
  await expect.poll(drawn).toEqual(expected('light'));
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect.poll(drawn).toEqual(expected('dark'));
  // an explicit setting does not follow the scheme
  await byPalette('Theme: Light');
  await expect.poll(drawn).toEqual(expected('light'));
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
  expect(await drawn()).toEqual(expected('light'));
  await page.emulateMedia({ colorScheme: null });
  await expect.poll(async () => JSON.parse(await readFile(prefs, 'utf8')).theme).toBe('light');
});

test('the main process refuses a value that is not a setting (G10: the renderer is not trusted)', async () => {
  const r = await page.evaluate(() =>
    window.studio.theme.set('sepia' as never).then(
      () => 'taken',
      (e: unknown) => String(e),
    ),
  );
  expect(r).toContain('not a theme setting');
  expect(await page.evaluate(() => window.studio.theme.get())).toMatchObject({ setting: 'light' });
});
