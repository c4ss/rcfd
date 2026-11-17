// The theme setting of the Studio (F-818): Light (DESIGN.md, the default), Dark, High contrast, or System, which follows
// the operating system (nativeTheme in the main process, prefers-color-scheme in the page). A preference of this Studio
// kept in its user data beside the recent list, never project state (G12); it reaches the page only through the IPC
// allow-list (src/shared/ipc.ts, G10). Shared by all three processes: no Node or DOM types.

/** The values of the setting, in the order the control lists them. */
export const THEME_SETTINGS = ['light', 'dark', 'contrast', 'system'] as const;
export type ThemeSetting = (typeof THEME_SETTINGS)[number];

/** The default: DESIGN.md's light theme, unchanged. */
export const DEFAULT_THEME: ThemeSetting = 'light';

/** The words of each value (the control's options, the palette's choices, the accessible names). */
export const THEME_LABEL: Readonly<Record<ThemeSetting, string>> = {
  light: 'Light',
  dark: 'Dark',
  contrast: 'High contrast',
  system: 'System',
};

export const isThemeSetting = (v: unknown): v is ThemeSetting => typeof v === 'string' && (THEME_SETTINGS as readonly string[]).includes(v);

/** What `theme.get` and `theme.set` return: the setting, and whether the system asks for dark colours now. */
export interface ThemeState {
  readonly setting: ThemeSetting;
  readonly systemDark: boolean;
}

/** The theme a setting draws, given whether the system is dark. */
export const resolveTheme = (setting: ThemeSetting, systemDark: boolean): 'light' | 'dark' | 'contrast' =>
  setting === 'system' ? (systemDark ? 'dark' : 'light') : setting;

/**
 * nativeTheme.themeSource for each setting: the window's own parts (title bar, menus, dialogs) and the page's
 * prefers-color-scheme follow it. High contrast is a light theme.
 */
export const NATIVE_THEME_SOURCE: Readonly<Record<ThemeSetting, 'light' | 'dark' | 'system'>> = {
  light: 'light',
  dark: 'dark',
  contrast: 'light',
  system: 'system',
};

/**
 * The window's background colour per theme (BrowserWindow backgroundColor), so the window is the theme's ground
 * before the page paints: no white flash on a dark start. The ground of each theme of src/renderer/design/tokens.ts
 * (`themes[name].white`); tests/unit/theme.spec.ts keeps the two equal.
 */
export const THEME_GROUND: Readonly<Record<'light' | 'dark' | 'contrast', string>> = {
  light: '#ffffff',
  dark: '#0f1c2c',
  contrast: '#ffffff',
};
