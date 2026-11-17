// F-818, the themes: each theme is a value for every DESIGN.md colour, the light theme is DESIGN.md's palette and its
// CSS is the unchanged :root block, each themed colour keeps its hue (so its one job, the Colour Has a Job Rule), the
// field's colour map is not a token, and the setting is parsed, kept and mapped to the native theme as documented.
// No contrast requirement is checked (G8: none is imposed).
import { expect, test } from '@playwright/test';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  cssCustomProperties,
  palette,
  role,
  THEME_NAMES,
  themeColorScheme,
  themeCustomProperties,
  themes,
  tokensCssText,
  type PaletteName,
} from '../../src/renderer/design/tokens.js';
import { NATIVE_THEME_SOURCE, resolveTheme, THEME_GROUND, THEME_LABEL, THEME_SETTINGS } from '../../src/shared/theme.js';
import { backgroundOf, formatPreferences, parsePreferences, preferencesFile, ThemePreference, type ThemeTargets } from '../../src/main/theme.js';
import { STUDIO_COMMANDS } from '../../src/shared/commands/studio.js';
import { globalShortcutOf } from '../../src/shared/commands/shortcuts.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const renderer = path.join(root, 'src', 'renderer');
const read = (...p: string[]): string => readFileSync(path.join(...p), 'utf8').replace(/\r\n/g, '\n');

/** The hue of a colour in degrees, and its saturation (HSL). */
function hsl(hex: string): { h: number; s: number } {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => v / 255) as [number, number, number];
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  const l = (max + min) / 2;
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  let h = 0;
  if (d !== 0) {
    if (max === r) h = 60 * (((g - b) / d) % 6);
    else if (max === g) h = 60 * ((b - r) / d + 2);
    else h = 60 * ((r - g) / d + 4);
  }
  return { h: (h + 360) % 360, s };
}
const hueGap = (a: number, b: number): number => Math.min(Math.abs(a - b), 360 - Math.abs(a - b));

test.describe('the themes of tokens.ts', () => {
  test('light is DESIGN.md’s palette; every theme sets every colour, and no two colours of a theme share a value', () => {
    expect(themes.light).toBe(palette);
    expect([...THEME_NAMES]).toEqual(['light', 'dark', 'contrast']);
    for (const name of THEME_NAMES) {
      const t = themes[name];
      expect(Object.keys(t).sort(), name).toEqual(Object.keys(palette).sort());
      for (const v of Object.values(t)) expect(v, name).toMatch(/^#[0-9a-f]{6}$/);
      expect(new Set(Object.values(t)).size, `${name}: each colour its own value`).toBe(Object.keys(t).length);
    }
  });

  test('each themed colour keeps its hue, so its job (the Colour Has a Job Rule)', () => {
    // the coloured scales: sky, meadow, orange and red stay those hues in every theme (within 25 degrees)
    const chromatic: PaletteName[] = ['sky', 'sky-wash', 'meadow', 'meadow-wash', 'meadow-deep', 'orange', 'orange-deep', 'fail', 'fail-wash'];
    for (const name of THEME_NAMES) {
      for (const c of chromatic) {
        const a = hsl(palette[c]);
        const b = hsl(themes[name][c]);
        expect(hueGap(a.h, b.h), `${name} ${c}: ${palette[c]} -> ${themes[name][c]}`).toBeLessThanOrEqual(25);
      }
      // the neutrals stay ink-tinted: never black, never a pure grey (the No Black, No Grey Text Rule)
      for (const c of ['ink', 'ink-2', 'ink-hover', 'silver', 'silver-wash', 'ink-tint'] as const) {
        expect(themes[name][c], `${name} ${c}`).not.toBe('#000000');
        const { h, s } = hsl(themes[name][c]);
        expect(s, `${name} ${c} is tinted`).toBeGreaterThan(0);
        expect(hueGap(h, hsl(palette.ink).h), `${name} ${c} is tinted from ink`).toBeLessThanOrEqual(25);
      }
    }
  });

  test('the roles are the same in every theme: a theme changes values, never jobs', () => {
    // one role table for all themes; CSS roles refer to the palette by var(), so a theme block sets colours only
    for (const name of THEME_NAMES) {
      const keys = themeCustomProperties(name).map(([k]) => k);
      expect(keys.filter((k) => k.startsWith('--role-')), name).toEqual([]);
      expect(keys.filter((k) => k.startsWith('--color-')).length).toBe(Object.keys(palette).length);
    }
    expect(Object.values(role).every((v) => v in palette)).toBe(true);
  });

  test('tokens.css: the :root block is the light theme unchanged, then one block per other theme and the dark system', () => {
    const css = read(renderer, 'design', 'tokens.css');
    expect(css).toBe(tokensCssText());
    const rootBlock = css.split('\n}\n')[0]!;
    expect([...rootBlock.matchAll(/^\s*(--[a-z0-9-]+):\s*(.+);$/gm)].map((m) => [m[1]!, m[2]!])).toEqual(cssCustomProperties().map(([k, v]) => [k, v]));
    expect(rootBlock).not.toContain('color-scheme');
    expect(css).not.toContain("data-theme='light'");
    const block = (selector: string): string => {
      const i = css.indexOf(`${selector} {`);
      expect(i, selector).toBeGreaterThan(0);
      return css.slice(i, css.indexOf('}', i));
    };
    for (const name of ['dark', 'contrast'] as const) {
      const b = block(`:root[data-theme='${name}']`);
      expect([...b.matchAll(/^\s*(--[a-z0-9-]+):\s*(.+);$/gm)].map((m) => [m[1]!, m[2]!])).toEqual(themeCustomProperties(name).map(([k, v]) => [k, v]));
      expect(b).toContain(`color-scheme: ${themeColorScheme[name]};`);
    }
    const sys = block(':root:not([data-theme])');
    expect(css.lastIndexOf('@media (prefers-color-scheme: dark)')).toBeLessThan(css.indexOf(':root:not([data-theme])'));
    expect([...sys.matchAll(/^\s*(--[a-z0-9-]+):\s*(.+);$/gm)].map((m) => [m[1]!, m[2]!])).toEqual(themeCustomProperties('dark').map(([k, v]) => [k, v]));
  });

  test('the window background of each theme is that theme’s ground', () => {
    for (const name of THEME_NAMES) expect(THEME_GROUND[name], name).toBe(themes[name].white);
  });

  test('the field colour map is not a token: colormap.ts reads no theme, and the canvas overlays keep DESIGN.md’s values', () => {
    const colormap = read(renderer, 'field', 'colormap.ts');
    expect(colormap).not.toMatch(/tokens|theme/);
    const canvas = read(renderer, 'field', 'FieldCanvas.tsx');
    expect(canvas).toContain('raw(palette.white)');
    expect(canvas).toContain('raw(palette.ink)');
    expect(canvas).toContain("raw(themeHex('silver-wash'))");
    expect(canvas).not.toContain("palette['silver-wash']");
  });

  test('the settings page’s stylesheet uses only defined tokens, no colour literals and square corners', () => {
    const css = read(renderer, 'studio-settings', 'studio-settings.css').replace(/\/\*[\s\S]*?\*\//g, '');
    const defined = new Set(cssCustomProperties().map(([k]) => k));
    const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
    expect(used.filter((v) => !defined.has(v))).toEqual([]);
    expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
    expect(css).not.toMatch(/border-radius|box-shadow/);
  });
});

test.describe('the theme setting', () => {
  test('Light, Dark, High contrast, System; Light the default; System follows the system', () => {
    expect([...THEME_SETTINGS]).toEqual(['light', 'dark', 'contrast', 'system']);
    expect(THEME_SETTINGS.map((s) => THEME_LABEL[s])).toEqual(['Light', 'Dark', 'High contrast', 'System']);
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('contrast', true)).toBe('contrast');
    expect(NATIVE_THEME_SOURCE).toEqual({ light: 'light', dark: 'dark', contrast: 'light', system: 'system' });
    expect(backgroundOf('dark', false)).toBe(themes.dark.white);
    expect(backgroundOf('system', true)).toBe(themes.dark.white);
    expect(backgroundOf('light', true)).toBe(themes.light.white);
  });

  test('the file: anything unexpected is Light; a written file reads back', () => {
    expect(parsePreferences('')).toBe('light');
    expect(parsePreferences('{')).toBe('light');
    expect(parsePreferences('{"format":"rcfd-studio-preferences","theme":"sepia"}')).toBe('light');
    expect(parsePreferences('{"format":"other","theme":"dark"}')).toBe('light');
    for (const s of THEME_SETTINGS) expect(parsePreferences(formatPreferences(s))).toBe(s);
    expect(preferencesFile({ RCFD_STUDIO_PREFERENCES: ' C:/x/p.json ' }, 'C:/u')).toBe('C:/x/p.json');
    expect(preferencesFile({}, 'C:/u')).toBe(path.join('C:/u', 'studio-preferences.json'));
  });

  test('set applies the native theme and the background at once, writes the file, and refuses a value that is not a setting', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f818-'));
    try {
      const file = path.join(dir, 'sub', 'prefs.json');
      const seen: string[] = [];
      const targets: ThemeTargets = { setSource: (s) => seen.push(`source ${s}`), systemDark: () => false, setBackground: (c) => seen.push(`bg ${c}`) };
      const p = new ThemePreference(file);
      expect(p.setting).toBe('light');
      expect(await p.set('dark', targets)).toEqual({ setting: 'dark', systemDark: false });
      expect(seen).toEqual(['source dark', `bg ${themes.dark.white}`]);
      expect(parsePreferences(await readFile(file, 'utf8'))).toBe('dark');
      await expect(p.set('sepia', targets)).rejects.toThrow(/not a theme setting/);
      expect(p.setting).toBe('dark');
      expect(new ThemePreference(file).setting).toBe('dark');
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  test('Settings: a palette command on Ctrl+, that opens the Studio’s settings page', () => {
    const c = STUDIO_COMMANDS.find((x) => x.id === 'go.studio_settings')!;
    expect(c.title).toBe('Settings');
    expect(globalShortcutOf('go.studio_settings')?.keys).toBe('Ctrl+,');
    expect(read(renderer, 'studio-settings', 'hash.ts')).toContain("'#/studio-settings'");
  });

  test('the palette has one command per setting, in the View group', () => {
    const ids = STUDIO_COMMANDS.filter((c) => c.id.startsWith('view.theme_'));
    expect(ids.map((c) => c.id)).toEqual(THEME_SETTINGS.map((s) => `view.theme_${s}`));
    expect(ids.map((c) => c.title)).toEqual(THEME_SETTINGS.map((s) => `Theme: ${THEME_LABEL[s]}`));
    expect(new Set(ids.map((c) => c.group))).toEqual(new Set(['View']));
  });
});
