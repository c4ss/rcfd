// The globals the preload script adds to the page: the shell's IPC allow-list (src/shared/ipc.ts) and the solver
// service (src/shared/service.ts).
import type { StudioBridge } from '../shared/ipc.js';
import type { ServiceBridge } from '../shared/service.js';

declare global {
  interface Window {
    readonly studio: StudioBridge;
    readonly studioService: ServiceBridge;
  }
}
