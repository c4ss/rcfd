// The IPC allow-list of the Studio (docs/studio-guardrails.md G10): every channel the renderer can reach, with its
// arguments and reply. The main process registers a handler for exactly these channels and the preload script exposes
// one function per channel through contextBridge; nothing else crosses the bridge. Shared by all three processes, so
// it uses neither Node nor DOM types. The solver service has its own way in, a MessagePort with its own allow-list:
// src/shared/service.ts (F-803).
import type { ThemeSetting, ThemeState } from './theme.js';

/** What `app.info` returns: the shell's identity and the versions it runs on. */
export interface AppInfo {
  readonly name: string;
  readonly version: string;
  readonly electron: string;
  readonly chromium: string;
  readonly node: string;
  readonly platform: string;
  readonly arch: string;
  /** True when started from a packaged copy (not `electron .`). */
  readonly packaged: boolean;
}

/** The setting "Keep the runner going after the Studio closes", and the --keep-runner switch. */
export interface KeepRunnerState {
  readonly setting: boolean;
  readonly switch: boolean;
}

/** Channel name -> argument tuple and reply. The only channels there are. */
export interface IpcContract {
  /** The shell's name and versions. */
  'app.info': { args: []; reply: AppInfo };
  /**
   * The renderer reports that the project home is in the document and one animation frame has passed. The main
   * process records the time; it writes it to a file only when RCFD_STUDIO_STARTUP_REPORT is set (the P10 measurement).
   */
  'app.ready': { args: []; reply: void };
  /**
   * F-837: the page's answer to a close request (CLOSE_REQUEST_CHANNEL): `asking` at once (the page is alive and checks
   * for unsaved case changes), then `close` (nothing unsaved, or saved, or discarded) or `cancel` (the user keeps
   * editing). The request is the number the main process sent.
   */
  'app.closeAnswer': { args: [request: number, answer: CloseAnswer]; reply: void };
  /**
   * G7 as amended 2026-11-17: the setting "Keep the runner going after the Studio closes" (off by default), and whether
   * the --keep-runner switch is on (it keeps the runner whatever the setting).
   */
  'runner.keep.get': { args: []; reply: KeepRunnerState };
  /** Sets the setting and keeps it in the Studio's user data (studio-preferences.json). */
  'runner.keep.set': { args: [keep: boolean]; reply: KeepRunnerState };
  /** F-818: the theme setting (src/shared/theme.ts) and whether the system is dark now. */
  'theme.get': { args: []; reply: ThemeState };
  /** F-818: sets the theme, keeps it in the Studio's user data, and sets the window's native theme and background. */
  'theme.set': { args: [setting: ThemeSetting]; reply: ThemeState };
}

/**
 * F-837: the answers to a close request. G7 as amended 2026-11-17: `stop-runs` and `keep-runs` close as `close` does,
 * and say what to do with the active runs of a runner the Studio owns (src/shared/runner-lifetime.ts); `close` keeps
 * such a runner (the page did not ask about it) and shuts down one with no active run.
 */
export const CLOSE_ANSWERS = ['asking', 'close', 'cancel', 'stop-runs', 'keep-runs'] as const;
export type CloseAnswer = (typeof CLOSE_ANSWERS)[number];

/**
 * F-837: main to page, the window is about to close (its close button, Alt+F4, or a quit): the argument is the
 * request's number, answered on `app.closeAnswer`. The page has no ipcRenderer; the preload script hands the request to
 * the one listener set by `app.onCloseRequest`.
 */
export const CLOSE_REQUEST_CHANNEL = 'studio.close-request';

export type Channel = keyof IpcContract;

/** The allow-list at run time. `satisfies` keeps it equal to the keys of IpcContract in both directions. */
export const CHANNELS = ['app.info', 'app.ready', 'app.closeAnswer', 'runner.keep.get', 'runner.keep.set', 'theme.get', 'theme.set'] as const satisfies readonly Channel[];
type Missing = Exclude<Channel, (typeof CHANNELS)[number]>;
const complete: Missing extends never ? true : Missing = true;
void complete;

/** What the preload script puts on `window.studio`. */
export interface StudioBridge {
  readonly app: {
    info(): Promise<AppInfo>;
    ready(): Promise<void>;
    /** F-837: the one listener of close requests (a second call replaces the first; null removes it). */
    onCloseRequest(listener: ((request: number) => void) | null): void;
    closeAnswer(request: number, answer: CloseAnswer): Promise<void>;
  };
  /** G7 as amended 2026-11-17: the setting that keeps the runner going after the Studio closes. */
  readonly runner: {
    getKeep(): Promise<KeepRunnerState>;
    setKeep(keep: boolean): Promise<KeepRunnerState>;
  };
  /** F-818: the theme setting. */
  readonly theme: {
    get(): Promise<ThemeState>;
    set(setting: ThemeSetting): Promise<ThemeState>;
  };
}
