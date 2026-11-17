// The theme of the page (F-818): <html data-theme> names the theme drawn (light, dark or contrast), and
// design/tokens.css sets that theme's values of the DESIGN.md tokens under it, so a change applies at once, with no
// reload. The setting (Light, Dark, High contrast, System) is the main process's (src/main/theme.ts, through the theme
// channels of the IPC allow-list, G10); System follows the page's prefers-color-scheme, which the main process's
// nativeTheme drives. Until the main process answers, data-theme is not set and tokens.css follows prefers-color-scheme,
// so a dark start is dark from its first frame.
//
// The three.js canvases (field/FieldCanvas.tsx, plots/linePlot.ts) take their colours from `themeHex` and redraw on
// `onThemeChange`; the field's colour map is not a token and never changes.
import { useSyncExternalStore } from 'react';
import { themes, type PaletteName, type ThemeName } from '../design/tokens.js';
import { DEFAULT_THEME, resolveTheme, type ThemeSetting, type ThemeState } from '../../shared/theme.js';

const DARK_QUERY = '(prefers-color-scheme: dark)';

let setting: ThemeSetting = DEFAULT_THEME;
let known = false;
const listeners = new Set<() => void>();

const media = (): MediaQueryList | null => (typeof matchMedia === 'function' ? matchMedia(DARK_QUERY) : null);
const systemDark = (): boolean => media()?.matches ?? false;

/** The theme drawn now: data-theme once set, otherwise what tokens.css draws without it. */
export function currentTheme(): ThemeName {
  const t = document.documentElement.dataset['theme'];
  if (t === 'light' || t === 'dark' || t === 'contrast') return t;
  return systemDark() ? 'dark' : 'light';
}

/** A palette colour's value in the theme drawn now (for three.js materials and clear colours). */
export const themeHex = (name: PaletteName): string => themes[currentTheme()][name];

function draw(): void {
  if (!known) return;
  const t = resolveTheme(setting, systemDark());
  if (document.documentElement.dataset['theme'] !== t) document.documentElement.dataset['theme'] = t;
}

function adopt(s: ThemeState): void {
  setting = s.setting;
  known = true;
  draw();
  for (const l of listeners) l();
}

/** Asks the main process for the setting and draws it; System follows the system's changes. Once, from main.tsx. */
export function initTheme(): void {
  media()?.addEventListener('change', () => {
    if (setting === 'system') draw();
  });
  void window.studio?.theme
    ?.get()
    .then(adopt)
    .catch(() => adopt({ setting: DEFAULT_THEME, systemDark: systemDark() }));
}

/** Changes the setting: drawn at once, then kept by the main process (which sets the native theme). */
export async function setTheme(next: ThemeSetting): Promise<void> {
  adopt({ setting: next, systemDark: systemDark() });
  try {
    adopt(await window.studio.theme.set(next));
  } catch {
    // the main process refused or is gone: the window shows what it holds
    await window.studio.theme
      .get()
      .then(adopt)
      .catch(() => undefined);
  }
}

/** The setting, for the control and the palette (re-rendered when it changes). */
export function useThemeSetting(): ThemeSetting {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => setting,
  );
}

/** Calls `cb` with the theme drawn whenever it changes (data-theme, or the system's colours before it is set). */
export function onThemeChange(cb: (theme: ThemeName) => void): () => void {
  let last = currentTheme();
  const check = () => {
    const t = currentTheme();
    if (t !== last) {
      last = t;
      cb(t);
    }
  };
  const mo = new MutationObserver(check);
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  const mq = media();
  mq?.addEventListener('change', check);
  return () => {
    mo.disconnect();
    mq?.removeEventListener('change', check);
  };
}
