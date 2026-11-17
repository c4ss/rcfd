// The theme setting in the main process (F-818): kept in the Studio's user data (studio-preferences.json, or the file
// RCFD_STUDIO_PREFERENCES names), applied to the native parts (nativeTheme.themeSource) and to each window's
// background colour, and handed to the page through the theme.get and theme.set channels of the IPC allow-list (G10).
// A preference of this Studio, never project state (G12). Pure parts are exported for the unit tests; the Electron
// parts are passed in.
import { readFileSync } from 'node:fs';
import { mkdir, rename, writeFile } from 'node:fs/promises';
import path from 'node:path';
import {
  DEFAULT_THEME,
  isThemeSetting,
  NATIVE_THEME_SOURCE,
  resolveTheme,
  THEME_GROUND,
  type ThemeSetting,
  type ThemeState,
} from '../shared/theme.js';

const FORMAT = 'rcfd-studio-preferences';

/** Where the setting is kept: RCFD_STUDIO_PREFERENCES, or studio-preferences.json in the user data. */
export function preferencesFile(env: Readonly<Record<string, string | undefined>>, userData: string): string {
  return env['RCFD_STUDIO_PREFERENCES']?.trim() || path.join(userData, 'studio-preferences.json');
}

/** Reads the setting from the file's text; anything unexpected is the default (Light). */
export function parsePreferences(text: string): ThemeSetting {
  try {
    const v = JSON.parse(text) as { format?: unknown; theme?: unknown };
    return v?.format === FORMAT && isThemeSetting(v.theme) ? v.theme : DEFAULT_THEME;
  } catch {
    return DEFAULT_THEME;
  }
}

export const formatPreferences = (theme: ThemeSetting): string => JSON.stringify({ format: FORMAT, version: 1, theme }, null, 2) + '\n';

/** The window background colour of a setting, given whether the system is dark. */
export const backgroundOf = (setting: ThemeSetting, systemDark: boolean): string => THEME_GROUND[resolveTheme(setting, systemDark)];

/** The parts of Electron the setting is applied to (nativeTheme and the open windows). */
export interface ThemeTargets {
  setSource(source: 'light' | 'dark' | 'system'): void;
  systemDark(): boolean;
  setBackground(colour: string): void;
}

export class ThemePreference {
  private current: ThemeSetting;
  private writing: Promise<void> = Promise.resolve();

  /** Reads the file once, synchronously, before the first window, so the window opens in its theme. */
  constructor(private readonly file: string) {
    let text = '';
    try {
      text = readFileSync(file, 'utf8');
    } catch {
      // no file yet: the default
    }
    this.current = text ? parsePreferences(text) : DEFAULT_THEME;
  }

  get setting(): ThemeSetting {
    return this.current;
  }

  state(targets: ThemeTargets): ThemeState {
    return { setting: this.current, systemDark: targets.systemDark() };
  }

  /** Sets the native theme source (before the window opens, and on every change). */
  apply(targets: ThemeTargets): void {
    targets.setSource(NATIVE_THEME_SOURCE[this.current]);
    targets.setBackground(backgroundOf(this.current, targets.systemDark()));
  }

  /** Changes the setting, applies it at once and writes the file (written in order; a failed write keeps the change). */
  async set(setting: unknown, targets: ThemeTargets): Promise<ThemeState> {
    if (!isThemeSetting(setting)) throw new Error(`theme.set: not a theme setting: ${JSON.stringify(setting)}`);
    this.current = setting;
    this.apply(targets);
    const text = formatPreferences(setting);
    this.writing = this.writing.catch(() => undefined).then(async () => {
      await mkdir(path.dirname(this.file), { recursive: true });
      const tmp = `${this.file}.${process.pid}.tmp`;
      await writeFile(tmp, text);
      await rename(tmp, this.file);
    });
    await this.writing.catch(() => undefined);
    return this.state(targets);
  }
}
