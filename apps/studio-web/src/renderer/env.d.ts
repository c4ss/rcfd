// The globals the preload script adds to the page: the shell's IPC allow-list (src/shared/ipc.ts), the solver
// service (src/shared/service.ts) and bulk field data (src/shared/data.ts).
import type { StudioBridge } from '../shared/ipc.js';
import type { ServiceBridge } from '../shared/service.js';
import type { DataBridge } from '../shared/data.js';

declare global {
  interface Window {
    readonly studio: StudioBridge;
    readonly studioService: ServiceBridge;
    readonly studioData: DataBridge;
  }
}
