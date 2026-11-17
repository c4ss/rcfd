// The words of Claude Code's registration and of the activity line (ADR 0801 Addendum 23), with no DOM and no React,
// so the unit tests check them (the line itself is ./claudeCode.tsx's).
import type { ChangeEntry, ClaudeCodeStatus } from '../../shared/project.js';

/** The registration in words. */
export function registrationWords(s: ClaudeCodeStatus): string {
  switch (s.registration) {
    case 'registered':
      return 'Registered with Claude Code for this folder.';
    case 'not_registered':
      return 'Not registered with Claude Code for this folder.';
    case 'other':
      return 'Registered with a different engine path.';
    case 'none':
      return 'No project is open.';
    case 'unknown':
      return s.claude.found ? 'Claude Code did not say whether it is registered.' : 'Claude Code was not found.';
  }
}

/** The actor client of the engine's own MCP server, when another program than the Studio runs it (Claude Code). */
export const MCP_CLIENT = 'mcp';

/** The latest change of the feed (newest first) made through an MCP client other than the Studio, or null. */
export function latestByClaude(changes: readonly ChangeEntry[]): ChangeEntry | null {
  return changes.find((c) => c.source === 'journal' && c.actor?.client === MCP_CLIENT) ?? null;
}

/** Where the activity line's link goes. */
export type ActivityTarget = { readonly kind: 'run'; readonly run: string } | { readonly kind: 'case'; readonly path: string } | { readonly kind: 'path'; readonly path: string } | { readonly kind: 'runs' };

/** The words of one change: the verb, and the thing linked. */
export function activityOf(c: ChangeEntry): { readonly verb: string; readonly thing: string; readonly target: ActivityTarget | null } {
  const run = c.run;
  const p = c.path ?? '';
  const isCase = /^cases\/[^/]+\.(toml|json)$/i.test(p);
  const onPath = (verb: string) => ({ verb, thing: p, target: p ? (isCase ? ({ kind: 'case', path: p } as const) : ({ kind: 'path', path: p } as const)) : null });
  const onRun = (verb: string) => ({ verb, thing: run ?? '', target: run ? ({ kind: 'run', run } as const) : null });
  switch (c.who) {
    case 'new_case':
      return onPath('made');
    case 'edit_case':
      return onPath('changed');
    case 'refine_case_mesh':
      return onPath('refined the mesh of');
    case 'write_note':
      return onPath('wrote');
    case 'import_mesh':
      return onPath('brought in');
    case 'start_run':
      return onRun('started run');
    case 'pause_run':
      return onRun('paused run');
    case 'resume_run':
      return onRun('resumed run');
    case 'cancel_run':
      return onRun('cancelled run');
    case 'trash_case':
    case 'trash_run':
    case 'trash_results':
      return { verb: 'moved to the trash', thing: p, target: p ? { kind: 'path', path: p } : null };
    case 'restore_trash':
      return onPath('restored');
    case 'empty_trash':
      return { verb: 'emptied the trash', thing: '', target: null };
    case 'set_runner_config':
      return { verb: 'changed the runner’s budgets', thing: '', target: { kind: 'runs' } };
    default:
      if (run) return onRun(`${c.who ?? c.kind} on run`);
      return onPath(`${c.who ?? c.kind}:`);
  }
}
