// The globals the preload script adds to the page: the shell's IPC allow-list (src/shared/ipc.ts), the solver
// service (src/shared/service.ts) and the project folder (src/shared/project.ts, F-805).
import type { StudioBridge } from '../shared/ipc.js';
import type { ProjectBridge } from '../shared/project.js';
import type { ServiceBridge } from '../shared/service.js';

declare global {
  interface Window {
    readonly studio: StudioBridge;
    readonly studioService: ServiceBridge;
    readonly studioProject: ProjectBridge;
  }
}
