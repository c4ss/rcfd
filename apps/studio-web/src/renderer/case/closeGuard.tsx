// F-837: save or discard before a close (the egui Studio asked on exit; ADR 0801 Addendum 13). A close that would end
// a case-editor session holding changes not saved asks first, with the F-829 decision panel: the panel names each
// file and its changes (what a close without saving loses) and offers Save and close, Close and lose them, and Keep
// editing (the safe choice, and Escape). The closes that ask:
//
// - the case editor's Close the session (the button and the palette's case.close): that session;
// - the project home's Close project (the button and the palette's project.close): the sessions of that project;
// - the window (its close button, Alt+F4, the app's quit): every session, through the main process's close request
//   (src/main/index.ts: the page answers `asking` at once, then `close` or `cancel`).
//
// Nothing asks when nothing is unsaved. Save writes each file through the engine as the editor's Save does; a save the
// engine refuses or that meets another writer's change keeps the panel open with the engine's words, and nothing is
// closed. Close and lose them closes the session (Close the session) or the project's sessions (Close project), their
// changes with them; on the window's close it closes nothing more than the quit did before (G7: the Studio's quit
// stops no session or run on the service). The panel takes the focus and holds Tab (DecisionPanel), and Keep editing
// gives the focus back where it was, a text field included.
import { lazy, Suspense, useEffect, useState } from 'react';
import type { CaseBinding, SaveOutcome } from '../../shared/case.js';
import { caseDoor } from './door.js';
import { projectActions } from '../project/door.js';

const DecisionPanel = lazy(() => import('../history/DecisionPanel.js').then((m) => ({ default: m.DecisionPanel })));

/** What closes: one session of the case editor, the open project, or the window (the Studio). */
export type CloseScope = { readonly kind: 'session'; readonly session: string } | { readonly kind: 'project' } | { readonly kind: 'window' };

/** How the close may go on: nothing was unsaved, the changes were saved or discarded; or the user kept editing. */
export type CloseOutcome = 'clean' | 'saved' | 'discarded' | 'cancel';

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
  /** The project's name for the title (project scope). */
  readonly project: string | null;
  readonly resolve: (o: CloseOutcome) => void;
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

/**
 * Asks before a close of `scope` when it would lose changes not saved; resolves when the close may go on (or not).
 * A second request while one is shown cancels the first.
 */
export async function confirmClose(scope: CloseScope): Promise<CloseOutcome> {
  const before = await commitTyping();
  const { unsaved, project } = await unsavedCases(scope);
  if (unsaved.length === 0) return 'clean';
  current?.resolve('cancel');
  return new Promise<CloseOutcome>((resolve) => {
    show({ scope, unsaved, project, before, resolve });
  });
}

/** Close project (the home's button and the palette's project.close), asking first; true when it closed. */
export async function closeProject(): Promise<boolean> {
  if ((await confirmClose({ kind: 'project' })) === 'cancel') return false;
  projectActions.close();
  return true;
}

/** The window's close requests (src/main/index.ts), answered through the shell's bridge. */
function listenForWindowClose(): () => void {
  const app = window.studio?.app;
  if (!app?.onCloseRequest) return () => undefined;
  app.onCloseRequest((request) => {
    void app.closeAnswer(request, 'asking').catch(() => undefined);
    void confirmClose({ kind: 'window' })
      .catch((): CloseOutcome => 'clean')
      .then((o) => app.closeAnswer(request, o === 'cancel' ? 'cancel' : 'close'))
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
  const changes = plural(n, 'change', 'changes');
  if (a.scope.kind === 'session') return `Close ${a.unsaved[0]!.path} with ${changes} not saved?`;
  if (a.scope.kind === 'project') return `Close ${a.project ?? 'the project'} with ${changes} not saved?`;
  return `Close rcfd Studio with ${changes} not saved?`;
}

/** The panel, mounted once beside every route (main.tsx); it loads on the first close that asks. */
export function CloseGuardHost() {
  const [ask, setAsk] = useState<Ask | null>(current);
  const [busy, setBusy] = useState<'save' | 'lose' | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  useEffect(() => {
    const l = (a: Ask | null) => {
      setAsk(a);
      setBusy(null);
      setFailure(null);
    };
    listeners.add(l);
    const off = listenForWindowClose();
    return () => {
      listeners.delete(l);
      off();
    };
  }, []);
  if (ask === null) return null;

  const finish = (o: CloseOutcome) => {
    if (current !== ask) return;
    show(null);
    ask.resolve(o);
    // the panel gave the focus back to what had it when it opened; a field the close blurred gets it again
    if (o === 'cancel' && ask.before?.isConnected) {
      const field = ask.before;
      setTimeout(() => requestAnimationFrame(() => field.focus()), 0);
    }
  };
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
    finish('saved');
  };
  const lose = async () => {
    setBusy('lose');
    // Close project: the sessions close with their changes, as Close the session does; Close the session closes its own
    // session after this; the window: nothing is closed here, the quit path stays as G7 holds it (sessions and runs on
    // the service are not stopped by the Studio's quit), and the editor's bindings end with the Studio
    if (ask.scope.kind === 'project') await discard(ask.unsaved);
    finish('discarded');
  };

  return (
    <Suspense fallback={null}>
      <DecisionPanel
        title={failure ?? titleOf(ask)}
        failed={failure !== null}
        onCancel={() => finish('cancel')}
        choices={[
          { label: 'Save and close', onPress: () => void save(), isDisabled: busy !== null },
          { label: 'Close and lose them', onPress: () => void lose(), isDisabled: busy !== null },
          { label: 'Keep editing', onPress: () => finish('cancel'), safe: true },
        ]}
      >
        <p>{busy === 'save' ? 'Saving through the engine.' : busy === 'lose' ? 'Closing the sessions.' : 'Closing without saving loses them:'}</p>
        <ul className="hi-losses" aria-label="Changes not saved">
          {ask.unsaved.map((u) => (
            <li key={u.session} data-unsaved-session={u.session}>
              {u.path}: {plural(u.count, 'change', 'changes')}
              {u.paths.length > 0 ? ` (${u.paths.slice(0, 4).join(', ')}${u.paths.length > 4 ? ', …' : ''})` : ''}
            </li>
          ))}
        </ul>
      </DecisionPanel>
    </Suspense>
  );
}
