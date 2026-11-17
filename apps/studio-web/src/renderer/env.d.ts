// The globals the preload script adds to the page: the shell's IPC allow-list (src/shared/ipc.ts), the solver
// service (src/shared/service.ts), the project folder (src/shared/project.ts, F-805; run control, F-807), bulk field
// data (src/shared/data.ts, F-804), the case editor's case files (src/shared/case.ts, F-812), a runner run's histories
// (src/shared/run.ts, F-810), Claude Code (src/shared/claude.ts, F-814) and provenance (src/shared/provenance.ts, F-828).
import type { StudioBridge } from '../shared/ipc.js';
import type { ProjectBridge, RunsBridge } from '../shared/project.js';
import type { ServiceBridge } from '../shared/service.js';
import type { CompareBridge, DataBridge, MeshPreviewBridge, SurfaceValuesBridge } from '../shared/data.js';
import type { CaseBridge } from '../shared/case.js';
import type { RunBridge } from '../shared/run.js';
import type { ClaudeBridge } from '../shared/claude.js';
import type { ProvenanceBridge } from '../shared/provenance.js';

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
    /** F-819b: a field's values on the store's surface only (one per triangle), over the data port. */
    readonly studioSurfaceValues: SurfaceValuesBridge;
    /** F-836: the mesh preview's surface with the patch of each triangle, over the data port. */
    readonly studioMeshPreview: MeshPreviewBridge;
    readonly studioCase: CaseBridge;
    /** F-810: a runner run's residual and monitor histories, read only (not run control: that is studioRuns). */
    readonly studioRunData: RunBridge;
    readonly studioClaude: ClaudeBridge;
    /** F-828: a run's provenance and the clipboard of Copy. */
    readonly studioProvenance: ProvenanceBridge;
  }
}
