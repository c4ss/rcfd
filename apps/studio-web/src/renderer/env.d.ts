// The one global the preload script adds to the page (the IPC allow-list, src/shared/ipc.ts).
import type { StudioBridge } from '../shared/ipc.js';

declare global {
  interface Window {
    readonly studio: StudioBridge;
  }
}
