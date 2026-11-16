// The globals the preload script adds to the page: the shell's IPC allow-list (src/shared/ipc.ts), the solver
// service (src/shared/service.ts), the project folder (src/shared/project.ts, F-805; run control, F-807), bulk field
// data (src/shared/data.ts, F-804), the case editor's case files (src/shared/case.ts, F-812), a runner run's histories
// (src/shared/run.ts, F-810) and Claude Code (src/shared/claude.ts, F-814).
import type { StudioBridge } from '../shared/ipc.js';
import type { ProjectBridge, RunsBridge } from '../shared/project.js';
import type { ServiceBridge } from '../shared/service.js';
import type { CompareBridge, DataBridge } from '../shared/data.js';
import type { CaseBridge } from '../shared/case.js';
import type { RunBridge } from '../shared/run.js';
import type { ClaudeBridge } from '../shared/claude.js';

declare global {
  interface Window {
    readonly studio: StudioBridge;
    readonly studioService: ServiceBridge;
    readonly studioProject: ProjectBridge;
    /** F-807: run control over the project port. */
    readonly studioRuns: RunsBridge;
    readonly studioData: DataBridge;
    /** F-815: the difference of two runs' field, over the data port. */
    readonly studioCompare: CompareBridge;
    readonly studioCase: CaseBridge;
    /** F-810: a runner run's residual and monitor histories, read only (not run control: that is studioRuns). */
    readonly studioRunData: RunBridge;
    readonly studioClaude: ClaudeBridge;
  }
}
