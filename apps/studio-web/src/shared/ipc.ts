// The IPC allow-list of the Studio (docs/studio-guardrails.md G10): every channel the renderer can reach, with its
// arguments and reply. The main process registers a handler for exactly these channels and the preload script exposes
// one function per channel through contextBridge; nothing else crosses the bridge. Shared by all three processes, so
// it uses neither Node nor DOM types. F-803 adds the service client's channels here.

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

/** Channel name -> argument tuple and reply. The only channels there are. */
export interface IpcContract {
  /** The shell's name and versions. */
  'app.info': { args: []; reply: AppInfo };
  /**
   * The renderer reports that the empty project view is in the document and one animation frame has passed. The main
   * process records the time; it writes it to a file only when RCFD_STUDIO_STARTUP_REPORT is set (the P10 measurement).
   */
  'app.ready': { args: []; reply: void };
}

export type Channel = keyof IpcContract;

/** The allow-list at run time. `satisfies` keeps it equal to the keys of IpcContract in both directions. */
export const CHANNELS = ['app.info', 'app.ready'] as const satisfies readonly Channel[];
type Missing = Exclude<Channel, (typeof CHANNELS)[number]>;
const complete: Missing extends never ? true : Missing = true;
void complete;

/** What the preload script puts on `window.studio`. */
export interface StudioBridge {
  readonly app: {
    info(): Promise<AppInfo>;
    ready(): Promise<void>;
  };
}
