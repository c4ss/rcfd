// Preload script of the Studio window: the IPC allow-list of docs/studio-guardrails.md G10, exposed to the page as
// `window.studio` through contextBridge. One function per channel of src/shared/ipc.ts; the page never gets
// ipcRenderer, a generic invoke or any Node object.
//
// CommonJS (.cts, emitted as .cjs) because a sandboxed preload cannot be an ES module, and a single file because a
// sandboxed preload can only require 'electron' and a few built-ins: the shared module is imported for its types only.
import { contextBridge, ipcRenderer } from 'electron';
import type { AppInfo, Channel, StudioBridge } from '../shared/ipc.js';

const invoke = <T,>(channel: Channel): Promise<T> => ipcRenderer.invoke(channel) as Promise<T>;

const bridge: StudioBridge = {
  app: {
    info: () => invoke<AppInfo>('app.info'),
    ready: () => invoke<void>('app.ready'),
  },
};

contextBridge.exposeInMainWorld('studio', bridge);
