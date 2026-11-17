// F-837: save or discard before a close (the egui Studio asked on exit; ADR 0801 Addendum 13). A close that would end
// a case-editor session holding changes not saved asks first, with the F-829 decision panel: the panel names each
// file and its changes (what a close without saving loses) and offers Save and close, Close and lose them, and Keep
// editing (the safe choice, and Escape). The closes that ask:
//
// - the case editor's Close the session (the button and the palette's case.close): that session;
// - the project home's Close project (the button and the palette's project.close): the sessions of that project;
// - the window (its close button, Alt+F4, the app's quit): every session, through the main process's close request
//   (src/main/index.ts: the page answers `asking` at once, then `close`, `stop-runs`, `keep-runs` or `cancel`).
//
// Nothing asks when nothing is unsaved. Save writes each file through the engine as the editor's Save does; a save the
// engine refuses or that meets another writer's change keeps the panel open with the engine's words, and nothing is
// closed. Close and lose them closes the session (Close the session) or the project's sessions (Close project), their
// changes with them; on the window's close the editor's bindings end with the Studio. The panel takes the focus and
// holds Tab (DecisionPanel), and Keep editing gives the focus back where it was, a text field included.
//
// G7 as amended 2026-11-17 (src/shared/runner-lifetime.ts): Close project and the window's close also ask when the
// project's runner is the Studio's own and holds runs queued, running or paused. The panel names each run (case, state,
// step or time) and offers Stop runs and close (each run is cancelled through the engine, its records stay; then the
// runner is shut down), Keep them running and close (the runner stays; the next start adopts it), and Cancel (the safe
// choice, and Escape). With case changes not saved as well, one panel asks both: the runs' answer is a toggle in it
// (Keep them running, until Stop them is chosen), and the choices are Save and close, Close and lose them, and Cancel.
import { lazy, Suspense, useEffect, useState } from 'react';
import type { CaseBinding, SaveOutcome } from '../../shared/case.js';
import { closeTitle, NO_PLAN, runLine, runnerNeedsAsk, type ActiveRun } from '../../shared/runner-lifetime.js';
import { caseDoor } from './door.js';
import { closeProjectWith, runnerPlan } from '../project/door.js';
import { SegmentedToggle } from '../ui/index.js';

const DecisionPanel = lazy(() => import('../history/DecisionPanel.js').then((m) => ({ default: m.DecisionPanel })));

/** What closes: one session of the case editor, the open project, or the window (the Studio). */
export type CloseScope = { readonly kind: 'session'; readonly session: string } | { readonly kind: 'project' } | { readonly kind: 'window' };

/** How the close may go on: nothing was unsaved, the changes were saved or discarded; or the user kept editing. */
export type CloseOutcome = 'clean' | 'saved' | 'discarded' | 'cancel';

/** The runs' answer: stop them, keep them running, or not asked (no active run of a runner the Studio owns). */
export type RunsAnswer = 'stop' | 'keep' | null;

export interface CloseDecision {
  readonly outcome: CloseOutcome;
  readonly runs: RunsAnswer;
  /** Hides the panel when it was kept up while the close goes on (Stop runs and close: "Stopping the runs."). */
  readonly done: () => void;
}

/** A session holding changes not saved. */
export interface UnsavedCase {
  readonly session: string;
  readonly path: string;
  readonly count: number;
  readonly paths: readonly string[];
}

interface Ask {
  readonly scope: CloseScope;
  readonly unsaved: readonly UnsavedCase[];
  /** The active runs of the project's runner the Studio owns (project and window scopes). */
  readonly runs: readonly ActiveRun[];
  /** The project's name for the title (project scope). */
  readonly project: string | null;
  readonly resolve: (o: CloseOutcome, runs: RunsAnswer, keepShown: boolean) => void;
  /** Where the focus was before the panel took it (a text field the close blurred, to commit its typing). */
  readonly before: HTMLElement | null;
}

let current: Ask | null = null;
const listeners = new Set<(a: Ask | null) => void>();
const show = (a: Ask | null): void => {
  current = a;
  for (const l of listeners) l(a);
};

const plural = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`;
const baseName = (p: string): string => p.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? p;

/** A value typed but not committed (Enter or leaving the field commits it) is committed first, as Save does. */
async function commitTyping(): Promise<HTMLElement | null> {
  const active = document.activeElement as HTMLElement | null;
  if (active && active.closest('.st-key') && active.matches('input, textarea')) {
    active.blur();
    await new Promise((r) => setTimeout(r, 150));
    return active;
  }
  return null;
}

/** The sessions of the scope that hold changes not saved, oldest first (the counts Save writes). */
export async function unsavedCases(scope: CloseScope): Promise<{ unsaved: UnsavedCase[]; project: string | null }> {
  const s = await caseDoor.state();
  if (!s.ok) return { unsaved: [], project: null };
  const folder = s.result.project?.folder ?? null;
  const inScope = (b: CaseBinding): boolean =>
    scope.kind === 'window' ? true : scope.kind === 'session' ? b.session === scope.session : folder !== null && b.folder === folder;
  const out: UnsavedCase[] = [];
  for (const b of [...s.result.bindings].filter(inScope).sort((x, y) => x.openedAt.localeCompare(y.openedAt))) {
    const st = await caseDoor.status(b.session);
    if (st.ok && st.result.pending.count > 0) out.push({ session: b.session, path: b.path, count: st.result.pending.count, paths: st.result.pending.paths });
  }
  return { unsaved: out, project: folder === null ? null : baseName(folder) };
}

const nothing = (): void => undefined;

/**
 * Asks before a close of `scope` when it would lose changes not saved, or (project and window) when the runner the
 * Studio owns holds active runs; resolves when the close may go on (or not), with the runs' answer. A second request
 * while one is shown cancels the first.
 */
export async function confirmCloseDecision(scope: CloseScope): Promise<CloseDecision> {
  const before = await commitTyping();
  const [{ unsaved, project }, plan] = await Promise.all([unsavedCases(scope), scope.kind === 'session' ? Promise.resolve(NO_PLAN) : runnerPlan()]);
  const runs = runnerNeedsAsk(plan) ? plan.runs : [];
  if (unsaved.length === 0 && runs.length === 0) return { outcome: 'clean', runs: null, done: nothing };
  current?.resolve('cancel', null, false);
  return new Promise<CloseDecision>((resolve) => {
    const ask: Ask = {
      scope,
      unsaved,
      runs,
      project: project ?? null,
      before,
      resolve: (outcome, r, keepShown) => {
        if (!keepShown && current === ask) show(null);
        resolve({ outcome, runs: r, done: () => (current === ask ? show(null) : undefined) });
      },
    };
    show(ask);
  });
}

/** Asks before a close of `scope` (see confirmCloseDecision); the outcome only. */
export async function confirmClose(scope: CloseScope): Promise<CloseOutcome> {
  return (await confirmCloseDecision(scope)).outcome;
}

/** Close project (the home's button and the palette's project.close), asking first; true when it closed. */
export async function closeProject(): Promise<boolean> {
  const d = await confirmCloseDecision({ kind: 'project' });
  if (d.outcome === 'cancel') return false;
  try {
    await closeProjectWith(d.runs);
  } finally {
    d.done();
  }
  return true;
}

/** The window's close requests (src/main/index.ts), answered through the shell's bridge. */
function listenForWindowClose(): () => void {
  const app = window.studio?.app;
  if (!app?.onCloseRequest) return () => undefined;
  app.onCloseRequest((request) => {
    void app.closeAnswer(request, 'asking').catch(() => undefined);
    void confirmCloseDecision({ kind: 'window' })
      .catch((): CloseDecision => ({ outcome: 'clean', runs: null, done: nothing }))
      .then((d) => {
        const answer = d.outcome === 'cancel' ? 'cancel' : d.runs === 'stop' ? 'stop-runs' : d.runs === 'keep' ? 'keep-runs' : 'close';
        return app.closeAnswer(request, answer);
      })
      .catch(() => undefined);
  });
  return () => app.onCloseRequest(null);
}

/** One save of the close: the editor's Save of that session onto the file as it was opened. */
async function saveOne(u: UnsavedCase): Promise<string | null> {
  const r = await caseDoor.save(u.session, 'opened');
  if (!r.ok) return `Failed to save ${u.path}: ${r.error.message}`;
  const o: SaveOutcome = r.result;
  if (o.kind === 'saved' || o.kind === 'nothing') return null;
  if (o.kind === 'conflict') return `Not saved: ${u.path} changed outside the Studio. Open it in the case editor to choose what to keep.`;
  return `Not saved: ${u.path}: ${o.message}`;
}

/** Closes the sessions whose changes the user let go (the service's session.close, as Close the session). */
async function discard(unsaved: readonly UnsavedCase[]): Promise<void> {
  const { dispatch } = await import('../actions/index.js');
  const atMost = (ms: number) => new Promise((r) => setTimeout(r, ms));
  for (const u of unsaved) {
    await Promise.race([dispatch('session.close', { session: u.session }), atMost(5000)]).catch(() => undefined);
    await caseDoor.forget(u.session);
  }
}

function titleOf(a: Ask): string {
  const n = a.unsaved.reduce((t, u) => t + u.count, 0);
  if (a.scope.kind === 'session') return `Close ${a.unsaved[0]!.path} with ${plural(n, 'change', 'changes')} not saved?`;
  return closeTitle(a.scope.kind === 'project' ? (a.project ?? 'the project') : 'rcfd Studio', n, a.runs.length);
}

const RUNS_OPTIONS = [
  { id: 'keep', label: 'Keep them running' },
  { id: 'stop', label: 'Stop them' },
] as const;

/** The panel, mounted once beside every route (main.tsx); it loads on the first close that asks. */
export function CloseGuardHost() {
  const [ask, setAsk] = useState<Ask | null>(current);
  const [busy, setBusy] = useState<'save' | 'lose' | 'stop' | 'keep' | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  // the runs' answer when the panel asks about changes as well (the toggle); the non-destructive one until chosen
  const [runsChoice, setRunsChoice] = useState<'keep' | 'stop'>('keep');
  useEffect(() => {
    const l = (a: Ask | null) => {
      setAsk(a);
      setBusy(null);
      setFailure(null);
      setRunsChoice('keep');
    };
    listeners.add(l);
    const off = listenForWindowClose();
    return () => {
      listeners.delete(l);
      off();
    };
  }, []);
  if (ask === null) return null;

  const hasRuns = ask.runs.length > 0;
  const hasUnsaved = ask.unsaved.length > 0;
  const finish = (o: CloseOutcome, runs: RunsAnswer = null) => {
    if (current !== ask) return;
    // a stop waits for the runs to end: the panel stays up, saying so, until the close is done
    const keepShown = runs === 'stop';
    if (keepShown) setBusy('stop');
    ask.resolve(o, runs, keepShown);
    // the panel gave the focus back to what had it when it opened; a field the close blurred gets it again
    if (o === 'cancel' && ask.before?.isConnected) {
      const field = ask.before;
      setTimeout(() => requestAnimationFrame(() => field.focus()), 0);
    }
  };
  const runsAnswer = (): RunsAnswer => (hasRuns ? runsChoice : null);
  const save = async () => {
    setBusy('save');
    setFailure(null);
    for (const u of ask.unsaved) {
      const failed = await saveOne(u);
      if (failed !== null) {
        if (current !== ask) return;
        setBusy(null);
        setFailure(failed);
        // the pressed choice was disabled while saving: the focus goes to the title, inside the panel, so Escape answers
        requestAnimationFrame(() => document.querySelector<HTMLElement>('.hi-panel-title')?.focus());
        return;
      }
    }
    finish('saved', runsAnswer());
  };
  const lose = async () => {
    setBusy('lose');
    // Close project: the sessions close with their changes, as Close the session does; Close the session closes its own
    // session after this; the window: nothing is closed here, and the editor's bindings end with the Studio
    if (ask.scope.kind === 'project') await discard(ask.unsaved);
    finish('discarded', runsAnswer());
  };

  const choices = hasUnsaved
    ? [
        { label: 'Save and close', onPress: () => void save(), isDisabled: busy !== null },
        { label: 'Close and lose them', onPress: () => void lose(), isDisabled: busy !== null },
        { label: hasRuns ? 'Cancel' : 'Keep editing', onPress: () => finish('cancel'), safe: true, isDisabled: busy === 'stop' },
      ]
    : [
        { label: 'Stop runs and close', onPress: () => finish('clean', 'stop'), isDisabled: busy !== null },
        { label: 'Keep them running and close', onPress: () => finish('clean', 'keep'), isDisabled: busy !== null },
        { label: 'Cancel', onPress: () => finish('cancel'), safe: true, isDisabled: busy === 'stop' },
      ];
  const said =
    busy === 'save'
      ? 'Saving through the engine.'
      : busy === 'lose'
        ? 'Closing the sessions.'
        : busy === 'stop'
          ? 'Stopping the runs through the engine; then the runner is shut down.'
          : hasUnsaved
            ? 'Closing without saving loses them:'
            : 'The runner the Studio started holds these runs:';

  return (
    <Suspense fallback={null}>
      <DecisionPanel title={failure ?? titleOf(ask)} failed={failure !== null} onCancel={() => (busy === 'stop' ? undefined : finish('cancel'))} choices={choices}>
        <p>{said}</p>
        {hasUnsaved && (
          <ul className="hi-losses" aria-label="Changes not saved">
            {ask.unsaved.map((u) => (
              <li key={u.session} data-unsaved-session={u.session}>
                {u.path}: {plural(u.count, 'change', 'changes')}
                {u.paths.length > 0 ? ` (${u.paths.slice(0, 4).join(', ')}${u.paths.length > 4 ? ', …' : ''})` : ''}
              </li>
            ))}
          </ul>
        )}
        {hasRuns && hasUnsaved && <p>The runner the Studio started holds these runs:</p>}
        {hasRuns && (
          <ul className="hi-losses" aria-label="Runs going">
            {ask.runs.map((r) => (
              <li key={r.id} data-close-run={r.id} data-state={r.state}>
                {runLine(r)}
              </li>
            ))}
          </ul>
        )}
        {hasRuns && (
          <p>Stopped runs keep their records; a run with a restart file can go on later with --continue-from.</p>
        )}
        {hasRuns && hasUnsaved && (
          <SegmentedToggle
            aria-label="The runs"
            options={RUNS_OPTIONS}
            selected={runsChoice}
            onChange={(id) => setRunsChoice(id === 'stop' ? 'stop' : 'keep')}
          />
        )}
      </DecisionPanel>
    </Suspense>
  );
}
