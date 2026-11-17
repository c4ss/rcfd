// The case editor's project server (F-812): `rcfd-mcp --project <folder>` started without `--read-only`, because saving
// and making cases are the engine's write tools (`edit_case`, `new_case`). The main process sends it only the tools of
// CASE_TOOLS (src/shared/case.ts); this wrapper refuses any other name before it reaches the server, so the server's
// other write and run tools (write_note, import_mesh, start_run, ...) are never called from here. The renderer never
// names a tool: it asks the case port for operations (src/main/case/bridge.ts).
//
// No Electron import.
import { CASE_TOOLS, type CaseTool } from '../../shared/case.js';

/** What the wrapper needs of src/main/project/server.ts's ProjectServer. */
export interface ToolServerLike {
  readonly exited: boolean;
  callTool(tool: string, args?: Record<string, unknown>, timeoutMs?: number): Promise<unknown>;
  listTools(): Promise<readonly unknown[]>;
  close(): Promise<void>;
}

/** Time limit of one tool call (ms); a check builds the mesh. */
export const CASE_CALL_TIMEOUT_MS = 120_000;

export class CaseServer {
  /** Tool calls refused by the allow-list (for tests). */
  refused = 0;

  constructor(private readonly server: ToolServerLike) {}

  get exited(): boolean {
    return this.server.exited;
  }

  call(tool: CaseTool, args: Record<string, unknown>): Promise<unknown> {
    if (!(CASE_TOOLS as readonly string[]).includes(tool)) {
      this.refused++;
      return Promise.reject(new Error(`the case editor does not call ${String(tool)} (not on its allow-list)`));
    }
    return this.server.callTool(tool, args, CASE_CALL_TIMEOUT_MS);
  }

  listTools(): Promise<readonly unknown[]> {
    return this.server.listTools();
  }

  close(): Promise<void> {
    return this.server.close();
  }
}
