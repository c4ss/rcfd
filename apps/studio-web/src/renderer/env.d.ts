// The globals the preload script adds to the page: the shell's IPC allow-list (src/shared/ipc.ts), the solver
// service (src/shared/service.ts), the project folder (src/shared/project.ts, F-805), bulk field data
// (src/shared/data.ts, F-804), the case editor's case files (src/shared/case.ts, F-812) and a runner run's histories
// (src/shared/run.ts, F-810).
import type { StudioBridge } from '../shared/ipc.js';
import type { ProjectBridge } from '../shared/project.js';
import type { ServiceBridge } from '../shared/service.js';
import type { DataBridge } from '../shared/data.js';
import type { CaseBridge } from '../shared/case.js';
import type { RunBridge } from '../shared/run.js';

declare global {
  interface Window {
    readonly studio: StudioBridge;
    readonly studioService: ServiceBridge;
    readonly studioProject: ProjectBridge;
    readonly studioData: DataBridge;
    readonly studioCase: CaseBridge;
    readonly studioRun: RunBridge;
  }
}
