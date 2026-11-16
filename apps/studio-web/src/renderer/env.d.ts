// The globals the preload script adds to the page: the shell's IPC allow-list (src/shared/ipc.ts), the solver
// service (src/shared/service.ts), the project folder (src/shared/project.ts, F-805; run control, F-807) and bulk field data
// (src/shared/data.ts, F-804).
import type { StudioBridge } from '../shared/ipc.js';
import type { ProjectBridge, RunsBridge } from '../shared/project.js';
import type { ServiceBridge } from '../shared/service.js';
import type { DataBridge } from '../shared/data.js';

declare global {
  interface Window {
    readonly studio: StudioBridge;
    readonly studioService: ServiceBridge;
    readonly studioProject: ProjectBridge;
    /** F-807: run control over the project port. */
    readonly studioRuns: RunsBridge;
    readonly studioData: DataBridge;
  }
}
