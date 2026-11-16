// The globals the preload script adds to the page: the shell's IPC allow-list (src/shared/ipc.ts), the solver
// service (src/shared/service.ts), the project folder (src/shared/project.ts, F-805) and bulk field data
// (src/shared/data.ts, F-804), and Claude Code (src/shared/claude.ts, F-814).
import type { StudioBridge } from '../shared/ipc.js';
import type { ProjectBridge } from '../shared/project.js';
import type { ServiceBridge } from '../shared/service.js';
import type { DataBridge } from '../shared/data.js';
import type { ClaudeBridge } from '../shared/claude.js';

declare global {
  interface Window {
    readonly studio: StudioBridge;
    readonly studioService: ServiceBridge;
    readonly studioProject: ProjectBridge;
    readonly studioData: DataBridge;
    readonly studioClaude: ClaudeBridge;
  }
}
