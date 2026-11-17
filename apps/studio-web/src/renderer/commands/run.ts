// What the command palette does with each command (F-831): whether it is available now and why not (G19), the values a
// parameter can be chosen from, the confirmation it asks for (G18), and the run itself, each through the surface's own
// door (G16): service actions through the dispatcher (../actions), run control and the project folder through
// ../project/door, Claude Code through ../claude/door, and the commands of a mounted surface through its registered
// handler (./host). Nothing here makes up a value: a session, a case, a run or a folder is chosen from what the engine or
// the main process lists, and an action whose form already exists (New case, Duplicate as, a Claude message) opens it.
import type { ClaudeState } from '../../shared/claude.js';
import type { ProjectState, ProjectView } from '../../shared/project.js';
import type { ServiceState } from '../../shared/service.js';
import type { JsonObject } from '../../shared/actions/params.js';
import { DEFAULT_CLI_ADDRESS, equivalents, getAction, type ActionId } from '../../shared/actions/registry.js';
import { commandLine, getRunAction, runEquivalents, type Confirmation, type RunActionId } from '../../shared/actions/runs.js';
import { CHOICE_KINDS, type PaletteEntry, type PaletteParam } from '../../shared/commands/index.js';
import { availability, dispatch } from '../actions/index.js';
import { projectActions, runAction, runActionAvailability, runActionConfirmation } from '../project/door.js';
import { claudeActions } from '../claude/door.js';
import { caseHash } from '../case/hash.js';
import { closeProject } from '../case/closeGuard.js';
import { commandFact, commandHandler, whenRegistered } from './host.js';
import { setTheme } from '../theme/theme.js';
import type { ThemeSetting } from '../../shared/theme.js';

/** What the palette knows of the window when it asks. */
export interface PaletteEnv {
  readonly project: ProjectState | null;
  readonly claude: ClaudeState | null;
  readonly service: ServiceState | null;
}

export type Values = Readonly<Record<string, unknown>>;

const NO_PROJECT = 'No project is open: open a project folder first (Open project folder…, Ctrl+O).';
const NOT_IN_EDITOR = 'No case file is open in the case editor: open one first (Open a case file, or Edit on a case of Home).';

const view = (env: PaletteEnv): ProjectView | null => env.project?.project ?? null;

const turnRunning = (env: PaletteEnv): boolean => {
  const s = env.claude?.project?.turn?.status;
  return s === 'starting' || s === 'running';
};

/** Why a command cannot run now (before any value is chosen), with the way out; null when it can. */
export function unavailableReason(e: PaletteEntry, env: PaletteEnv): string | null {
  switch (e.source) {
    case 'service': {
      const svc = env.service;
      if (svc && svc.status !== 'connected') {
        const a = availability(e.id as ActionId, {}, { service: svc });
        if (!a.available) return a.reason;
      }
      return null;
    }
    case 'run': {
      const a = runActionAvailability(e.id as RunActionId, {}, view(env));
      return a.available ? null : a.reason;
    }
    case 'case': {
      if (e.id === 'case.new') return null;
      if (e.id === 'case.open') return view(env) ? null : NO_PROJECT;
      const h = commandHandler(e.id);
      return h ? (h.unavailable?.() ?? null) : NOT_IN_EDITOR;
    }
    case 'studio':
      return studioReason(e.id, env);
  }
}

function studioReason(id: string, env: PaletteEnv): string | null {
  const p = env.project;
  switch (id) {
    case 'go.home':
    case 'go.runs':
    case 'go.claude':
    case 'view.toggle_claude':
    case 'project.refresh':
    case 'project.close':
    case 'claude.refresh':
      return view(env) ? null : NO_PROJECT;
    case 'project.open_folder':
    case 'project.new':
      if (p && !p.engine.configured) return p.engine.reason;
      if (p?.opening) return `${p.opening} is being opened; wait for it.`;
      return null;
    case 'project.open_recent':
      if (p && !p.engine.configured) return p.engine.reason;
      return p && p.recent.length === 0 ? 'The recent list is empty: the folders you open appear there, the latest first.' : null;
    case 'project.forget_recent':
      return p && p.recent.length === 0 ? 'The recent list is empty: there is nothing to forget.' : null;
    case 'claude.new_chat':
    case 'claude.select': {
      if (!view(env)) return NO_PROJECT;
      if (env.claude?.project?.writer) return 'A message is being answered; wait for the reply or stop it (Stop the Claude Code reply), then choose another session.';
      return null;
    }
    case 'workspace.compare': {
      const v = view(env);
      if (!v) return NO_PROJECT;
      return v.runs.items.length < 2 ? 'The project has fewer than two runs: there is nothing to compare yet. Run a case again (Run the case, F5).' : null;
    }
    case 'claude.stop':
      if (!view(env)) return NO_PROJECT;
      return turnRunning(env) ? null : 'No Claude Code reply is being written: there is nothing to stop.';
    case 'claude.undo_allow':
      if (!view(env)) return NO_PROJECT;
      return env.claude?.project?.allow.undo
        ? null
        : 'This Studio has added no rule to this project’s allow-list: there is nothing to undo (Allow this from now on, on a denied call).';
    case 'provenance.show':
    case 'provenance.copy_command': {
      // F-828: run by the provenance module's handlers (src/renderer/provenance/registry.ts); a number of a service
      // session (the settings, the plots) needs no project, a run's command does
      if (id === 'provenance.copy_command' && !view(env)) return NO_PROJECT;
      const h = commandHandler(id);
      if (!h && !view(env)) return NO_PROJECT;
      return h ? (h.unavailable?.() ?? null) : 'Open Runs or the Workspace of the project first.';
    }
    // F-837: run by the surface that shows them (runs/RunExtras.tsx, field/FieldViewport.tsx, workspace/TimePanel.tsx)
    case 'runs.log':
    case 'runs.report':
    case 'runs.budgets':
    case 'field.save_png':
    case 'field.range':
    case 'field.face_on':
    case 'plots.monitor_log': {
      if (!view(env)) return NO_PROJECT;
      const h = commandHandler(id);
      return h ? (h.unavailable?.() ?? null) : (F837_WHERE[id] ?? null);
    }
    default:
      return null;
  }
}

/** F-837: where a surface command is run from, when its surface is not shown. */
const F837_WHERE: Readonly<Record<string, string>> = {
  'runs.log': 'Open Runs (Ctrl+2) and select a run first.',
  'runs.report': 'Open Runs (Ctrl+2) and select a run first.',
  'runs.budgets': 'Open Runs (Ctrl+2) first.',
  'field.save_png': 'Open a run in the workspace first.',
  'field.range': 'Open a run in the workspace first.',
  'field.face_on': 'Open a run in the workspace first.',
  'plots.monitor_log': 'Open a run in the workspace first.',
};

// ---- choices

export interface Choice {
  readonly value: string;
  readonly label: string;
  readonly detail: string | null;
  /** Why the command cannot run on this value, with the way out; null when it can. */
  readonly reason: string | null;
  /** A word for a choice offered first ("selected in Runs"). */
  readonly mark: string | null;
}

export interface Choices {
  readonly choices: readonly Choice[];
  /** What to say when there is nothing to choose, and what to do. */
  readonly empty: string;
}

/** Whether the palette asks for this parameter by a list of choices (a session, a case, a run, a folder). */
export const isChoice = (p: PaletteParam): boolean => CHOICE_KINDS.has(p.kind);

/** What a parameter can be chosen from now. */
export async function choicesFor(e: PaletteEntry, p: PaletteParam, values: Values, env: PaletteEnv): Promise<Choices> {
  const v = view(env);
  switch (p.kind) {
    case 'session': {
      const r = await dispatch('session.list', {});
      if (!r.ok) return { choices: [], empty: `The service did not list its sessions: ${r.error.message}` };
      const list = (r.result as { sessions?: { session: string; state: string }[] }).sessions ?? [];
      return {
        choices: list.map((s) => {
          const a = availability(e.id as ActionId, { ...values, [p.name]: s.session }, { session: { id: s.session, state: s.state } });
          return { value: s.session, label: s.session, detail: s.state, reason: a.available ? null : a.reason, mark: null };
        }),
        empty: 'The service holds no session: make one first (New session, or open a case file in the case editor).',
      };
    }
    case 'case': {
      if (!v) return { choices: [], empty: NO_PROJECT };
      return {
        choices: v.cases.items.map((c) => {
          const reason = e.source === 'run' ? runReason(e.id, { ...values, [p.name]: c.path }, v) : null;
          return { value: c.path, label: c.path, detail: c.unread ? `the engine could not read it: ${c.unread}` : `${c.runs} ${c.runs === 1 ? 'run' : 'runs'}`, reason, mark: null };
        }),
        empty: 'The project has no case file in cases/: make one first (New case).',
      };
    }
    case 'run': {
      if (!v) return { choices: [], empty: NO_PROJECT };
      const selected = commandFact<string | null>('selectedRun') ?? null;
      const runs = [...v.runs.items].sort((a, b) => (a.id === selected ? -1 : b.id === selected ? 1 : 0));
      return {
        choices: runs.map((r) => ({
          value: r.id,
          label: r.id,
          detail: `${r.case} · ${r.state}${r.step > 0 ? ` · step ${r.step}` : ''}`,
          // a Studio command on a run applies to every run (F-828's Copy reproduce command), except that Compare runs
          // refuses the selected run (F-815)
          reason: e.source === 'run' ? runReason(e.id, { ...values, [p.name]: r.id }, v) : compareReason(e.id, r.id, selected),
          mark: r.id === selected ? 'selected in Runs' : null,
        })),
        empty: 'The project has no run yet: run a case first (Run the case, F5).',
      };
    }
    case 'recent': {
      const recent = env.project?.recent ?? [];
      return {
        choices: recent.map((r) => ({
          value: r.path,
          label: r.path,
          detail: r.exists ? `opened ${r.opened}` : 'not found',
          reason:
            e.id === 'project.open_recent' && !r.exists ? 'The folder is not there any more: it may have been moved, renamed or deleted. Forget it, or open another.' : null,
          mark: null,
        })),
        empty: 'The recent list is empty: the folders you open appear there, the latest first.',
      };
    }
    case 'claude-session': {
      const cv = env.claude?.project;
      if (!cv) return { choices: [], empty: NO_PROJECT };
      return {
        choices: cv.sessions.map((s) => ({
          value: s.id,
          label: s.name ?? s.title ?? s.id,
          detail: `${s.open}${s.readOnly ? ` · read-only: ${s.readOnly}` : ''}`,
          reason: null,
          mark: s.id === cv.selected ? 'shown now' : null,
        })),
        empty: 'Claude Code has no session in this project yet: start one (New Claude Code chat).',
      };
    }
    default:
      return { choices: [], empty: '' };
  }
}

/** F-815: the run Compare runs compares with must not be the selected run itself. */
function compareReason(id: string, run: string, selected: string | null): string | null {
  return id === 'workspace.compare' && run === selected ? 'This is the selected run, the baseline of the comparison: choose another run to compare with it.' : null;
}

function runReason(id: string, values: Values, v: ProjectView): string | null {
  const a = runActionAvailability(id as RunActionId, values as never, v);
  return a.available ? null : a.reason;
}

/** Why the command cannot run with these values; null when it can (the engine still has the last word, G2). */
export function valuesReason(e: PaletteEntry, values: Values, env: PaletteEnv): string | null {
  if (e.source === 'service') {
    const a = availability(e.id as ActionId, values as never, {});
    return a.available ? null : a.reason;
  }
  if (e.source === 'run') {
    const a = runActionAvailability(e.id as RunActionId, values as never, view(env));
    return a.available ? null : a.reason;
  }
  return null;
}

/** The confirmation the command asks for before it is dispatched (G18), or null. */
export function confirmationFor(e: PaletteEntry, values: Values, env: PaletteEnv): Confirmation | null {
  const v = view(env);
  if (e.source !== 'run' || !v) return null;
  return runActionConfirmation(e.id as RunActionId, values as never, v);
}

// ---- running

export type Outcome =
  /** The palette closes (the focus goes back), then `after` runs: a view change, or a surface's own form. */
  | { readonly kind: 'close'; readonly after: () => void | Promise<unknown> }
  | {
      readonly kind: 'done';
      readonly ok: boolean;
      readonly title: string;
      readonly lines: readonly string[];
      /** The engine's reply, as JSON text (shortened when long). */
      readonly reply: string | null;
      /** The same from the CLI and from Claude Code (G16, G17). */
      readonly equivalents: readonly string[];
      /** A run the command made or is about, to follow in Runs. */
      readonly run: string | null;
    };

const REPLY_CHARS = 6000;

function replyText(v: unknown): string {
  const s = JSON.stringify(v, null, 2) ?? 'null';
  return s.length > REPLY_CHARS ? `${s.slice(0, REPLY_CHARS)}\n… (${s.length - REPLY_CHARS} more characters; the CLI command prints all of it)` : s;
}

/** Goes to the project home (when another page is shown) and runs a command of the open project's views there. */
async function inProject(id: string, args: Values = {}): Promise<void> {
  if (!commandHandler(id)) location.hash = '';
  const h = await whenRegistered(id);
  await h?.run(args);
}

const close = (after: () => void | Promise<unknown>): Outcome => ({ kind: 'close', after });

/** Shows a run in Runs, selected (after a run action, "Show run ... in Runs"). */
export function followRun(run: string): Promise<void> {
  return inProject('go.runs', { run });
}

/** Runs the command with its values. Never throws: a refusal is an outcome in words. */
export async function execute(e: PaletteEntry, values: Values, env: PaletteEnv): Promise<Outcome> {
  try {
    return await executeInner(e, values, env);
  } catch (err) {
    return { kind: 'done', ok: false, title: `Failed: ${e.title}`, lines: [String(err)], reply: null, equivalents: [], run: null };
  }
}

async function executeInner(e: PaletteEntry, values: Values, env: PaletteEnv): Promise<Outcome> {
  switch (e.source) {
    case 'service': {
      const r = await dispatch(e.id as ActionId, values as never);
      const eq = equivalents(getAction(e.id)!, r.entry.params, env.service?.address ?? DEFAULT_CLI_ADDRESS);
      const equivalent = [
        `Command line: ${commandLine(eq.cli)}`,
        eq.mcp ? `Claude Code: ${eq.mcp.tool} ${JSON.stringify(eq.mcp.arguments)}` : `Claude Code: ${eq.mcpNote ?? 'no MCP tool'}`,
      ];
      if (!r.ok) {
        return { kind: 'done', ok: false, title: `Failed: ${e.title}`, lines: [r.error.message], reply: null, equivalents: equivalent, run: null };
      }
      return { kind: 'done', ok: true, title: `${e.title}: done`, lines: [], reply: replyText(r.result), equivalents: equivalent, run: null };
    }
    case 'run': {
      const v = view(env);
      const r = await runAction(e.id as RunActionId, values as never);
      const a = getRunAction(e.id)!;
      const eq = v ? runEquivalents(a, values as JsonObject, v.folder) : null;
      const equivalent = eq
        ? [
            `Command line (in ${eq.cli.cwd}): ${commandLine(eq.cli.argv)}`,
            eq.mcp ? `Claude Code: ${eq.mcp.tool} ${JSON.stringify(eq.mcp.arguments)}` : `Claude Code: ${eq.mcpNote ?? 'no MCP tool'}`,
          ]
        : [];
      if (!r.ok) {
        const lines = [r.failure.problem, r.failure.recovery, ...(r.failure.engine ? [`The engine said: ${r.failure.engine.message}`] : [])];
        return { kind: 'done', ok: false, title: `Failed: ${e.title}`, lines, reply: null, equivalents: equivalent, run: r.request?.run ?? null };
      }
      const run = r.request.run;
      return {
        kind: 'done',
        ok: true,
        title: `${e.title}: asked`,
        lines: [`The runner has the request${run ? ` for run ${run}` : ''}; Runs shows what it does with it.`],
        reply: replyText(r.result),
        equivalents: equivalent,
        run,
      };
    }
    case 'case': {
      if (e.id === 'case.open') return close(() => void (location.hash = caseHash.open(String(values['case']))));
      if (e.id === 'case.new') return close(() => void (location.hash = caseHash.new));
      const h = commandHandler(e.id);
      if (!h) return { kind: 'done', ok: false, title: `Unavailable: ${e.title}`, lines: [NOT_IN_EDITOR], reply: null, equivalents: [], run: null };
      return close(() => h.run(values));
    }
    case 'studio':
      return studio(e, values, env);
  }
}

async function studio(e: PaletteEntry, values: Values, env: PaletteEnv): Promise<Outcome> {
  const failed = (lines: string[]): Outcome => ({ kind: 'done', ok: false, title: `Failed: ${e.title}`, lines, reply: null, equivalents: [], run: null });
  const done = (lines: string[]): Outcome => ({ kind: 'done', ok: true, title: `${e.title}: done`, lines, reply: null, equivalents: [], run: null });
  switch (e.id) {
    case 'go.home':
    case 'go.runs':
    case 'go.claude':
    case 'view.toggle_claude':
      return close(() => inProject(e.id, values));
    case 'workspace.compare':
      return close(() => inProject(e.id, values));
    case 'go.settings':
      return close(() => void (location.hash = '#/settings'));
    case 'go.studio_settings':
      return close(() => void (location.hash = '#/studio-settings'));
    case 'go.case_editor':
      return close(() => void (location.hash = caseHash.index));
    case 'project.open_folder':
      return close(() => projectActions.pick());
    case 'project.new':
      return close(() => projectActions.create());
    case 'project.open_recent':
      return close(() => projectActions.open(String(values['path'])));
    case 'project.forget_recent':
      projectActions.forget(String(values['path']));
      return done([`${String(values['path'])} is off the recent list; the folder is untouched.`]);
    case 'project.refresh':
      return close(() => projectActions.refresh());
    case 'project.close':
      // F-837: asks first when a case of the project has changes not saved (closeGuard.tsx)
      return close(async () => {
        if (await closeProject()) location.hash = '';
      });
    case 'claude.new_chat':
    case 'claude.select': {
      const r = await claudeActions.select(e.id === 'claude.new_chat' ? null : String(values['session']));
      if (!r.ok) return failed([r.error.message]);
      return close(() => inProject('go.claude'));
    }
    case 'claude.stop': {
      const r = await claudeActions.cancel();
      return r.ok ? done(['The reply was stopped.']) : failed([r.error.message]);
    }
    case 'claude.refresh': {
      const r = await claudeActions.refresh();
      return r.ok ? done(['The sessions were listed again; the panel shows them.']) : failed([r.error.message]);
    }
    case 'claude.undo_allow': {
      const rule = env.claude?.project?.allow.undo ?? 'the rule';
      const r = await claudeActions.undoAllow(false);
      return r.ok ? done([`${rule} is out of the allow-list again.`]) : failed([r.error.message]);
    }
    case 'provenance.show': {
      // F-828: the palette closes first, so the focus is back on the number it was opened from
      const h = commandHandler(e.id);
      return h ? close(() => h.run(values)) : failed(['Open Runs or the Workspace of the project first.']);
    }
    case 'runs.log':
    case 'runs.budgets':
    case 'field.range':
    case 'field.face_on':
    case 'plots.monitor_log': {
      // F-837: the palette closes first, so the focus is back on the surface
      const h = commandHandler(e.id);
      return h ? close(() => h.run(values)) : failed([F837_WHERE[e.id] ?? 'Open the surface first.']);
    }
    // F-818: the theme, applied at once (src/renderer/theme/theme.ts); the palette closes on the new colours
    case 'view.theme_light':
    case 'view.theme_dark':
    case 'view.theme_contrast':
    case 'view.theme_system':
      return close(() => void setTheme(e.id.slice('view.theme_'.length) as ThemeSetting));
    case 'runs.report':
    case 'field.save_png': {
      const h = commandHandler(e.id);
      if (!h) return failed([F837_WHERE[e.id] ?? 'Open the surface first.']);
      try {
        const lines = (await h.run(values)) as string[] | undefined;
        return done(lines ?? []);
      } catch (err) {
        return failed([err instanceof Error ? err.message : String(err)]);
      }
    }
    case 'provenance.copy_command': {
      const h = commandHandler(e.id);
      if (!h) return failed(['Open Runs or the Workspace of the project first.']);
      const r = (await h.run(values)) as { ok: boolean; lines: string[] } | undefined;
      return r?.ok ? done(r.lines) : failed(r?.lines ?? ['The command could not be copied.']);
    }
    default:
      return failed([`${e.id} is run by the palette itself.`]);
  }
}
