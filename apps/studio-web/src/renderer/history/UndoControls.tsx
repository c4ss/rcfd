// Undo and redo in the header band (F-829, G18, G8, G20): two ghost buttons that name the step ("Undo: set
// solver.alpha_u", "Redo: remove boundaries.lid"), Ctrl+Z and Ctrl+Shift+Z (Ctrl+Y too) from anywhere on the page, and
// the decisions undo will not take alone: a loss confirmed in words (undoing a new session closes it), a conflict with
// another writer shown with both values and three choices, a refusal in the engine's words.
//
// One history per session (src/shared/actions/history.ts, through ../actions): the page passes the session it shows.
// Inside a text field that holds typing not yet sent, Ctrl+Z stays the field's own undo of that typing; once the value
// is sent (Enter, or focus leaves) or dropped (Escape), it undoes the last edit.
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import type { HistoryOptions, HistoryOutcome, HistoryView } from '../../shared/actions/history.js';
import { history } from '../actions/index.js';
import { Button, Unavailable } from '../ui/index.js';
import { formatValue } from '../settings/schema.js';
import { DecisionPanel } from './DecisionPanel.js';
import './history.css';

export interface UndoControlsProps {
  /** The session whose edits the controls undo; null: none (the controls say so). */
  readonly scope: string | null;
  /** What the scope is, for the group's name (default `session <scope>`; ADR 0736: the project home's trash steps). */
  readonly label?: string;
  /** Redo made a session again under a new id (redo of "new session"): the page follows it. */
  readonly onRenamed?: (from: string, to: string) => void;
}

type Prompt = { id: number; outcome: Extract<HistoryOutcome, { kind: 'confirm' | 'conflict' | 'failed' }>; options: HistoryOptions };

const EMPTY: HistoryView = { scope: '', undo: null, redo: null, undoDepth: 0, redoDepth: 0, titles: [] };

const shown = (v: unknown) => (v === null || v === undefined ? 'not set' : formatValue(v as never) || 'empty');

/** A text field holding typing not yet sent: its Ctrl+Z is the field's. */
function isEditing(el: Element | null, dirty: Element | null): boolean {
  return el !== null && el === dirty && el.matches('input, textarea');
}

export function UndoControls({ scope, onRenamed, label }: UndoControlsProps) {
  const [view, setView] = useState<HistoryView>(EMPTY);
  const [prompt, setPrompt] = useState<Prompt | null>(null);
  const [announce, setAnnounce] = useState('');
  const renamed = useRef(onRenamed);
  renamed.current = onRenamed;

  useEffect(() => {
    const h = history();
    const read = () => setView(scope ? h.view(scope) : EMPTY);
    read();
    return h.subscribe((e) => {
      if (e.kind === 'renamed' && e.from === scope) renamed.current?.(e.from, e.to);
      read();
    });
  }, [scope]);

  useEffect(() => setPrompt(null), [scope]);

  const run = useCallback(
    async (dir: 'undo' | 'redo', options: HistoryOptions = {}) => {
      if (!scope) return;
      const h = history();
      const o = dir === 'undo' ? await h.undo(scope, options) : await h.redo(scope, options);
      const Dir = dir === 'undo' ? 'Undone' : 'Redone';
      switch (o.kind) {
        case 'done':
          setPrompt(null);
          setAnnounce(`${Dir}: ${o.title}.`);
          return;
        case 'confirm':
        case 'conflict':
        case 'failed':
          // a new panel each time (a conflict answered may lead to a loss to confirm): focus starts again
          setPrompt((p) => ({ id: (p?.id ?? 0) + 1, outcome: o, options }));
          return;
        case 'unavailable':
          setPrompt(null);
          setAnnounce(`Cannot undo ${o.title}: ${o.reason}`);
          return;
        case 'nothing':
          setAnnounce(dir === 'undo' ? 'Nothing to undo.' : 'Nothing to redo.');
          return;
        case 'busy':
          return;
      }
    },
    [scope],
  );

  // Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y (Cmd on macOS) from anywhere on the page
  const runRef = useRef(run);
  runRef.current = run;
  const promptOpen = useRef(false);
  promptOpen.current = prompt !== null;
  useEffect(() => {
    let dirty: Element | null = null;
    const onInput = (e: Event) => {
      dirty = e.target instanceof Element ? e.target : null;
    };
    const onFocusOut = (e: FocusEvent) => {
      if (e.target === dirty) dirty = null;
    };
    const onKey = (e: KeyboardEvent) => {
      if ((e.key === 'Enter' || e.key === 'Escape') && e.target === dirty) {
        dirty = null;
        return;
      }
      if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
      const k = e.key.toLowerCase();
      const dir = k === 'z' ? (e.shiftKey ? 'redo' : 'undo') : k === 'y' && !e.shiftKey ? 'redo' : null;
      if (!dir || promptOpen.current || isEditing(document.activeElement, dirty)) return;
      e.preventDefault();
      void runRef.current(dir);
    };
    // capture: React Aria's fields stop the propagation of the keys they handle
    window.addEventListener('input', onInput, true);
    window.addEventListener('focusout', onFocusOut, true);
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('input', onInput, true);
      window.removeEventListener('focusout', onFocusOut, true);
      window.removeEventListener('keydown', onKey, true);
    };
  }, []);

  const undoKeys = useId();
  const redoKeys = useId();
  const undoLabel = view.undo ? `Undo: ${view.undo.title}` : 'Nothing to undo';
  const redoLabel = view.redo ? `Redo: ${view.redo.title}` : 'Nothing to redo';

  return (
    <div className="hi-controls" role="group" aria-label={scope ? `Undo and redo, ${label ?? `session ${scope}`}` : 'Undo and redo'}>
      {view.undo && !view.undo.available ? (
        <Unavailable label={`Cannot undo: ${view.undo.title}`} reason={view.undo.reason ?? 'This step cannot be undone.'} />
      ) : (
        // G22: nothing to undo is the normal state: the arrow alone, the words its name and tooltip
        <Button
          variant="ghost"
          className="hi-button"
          icon={view.undo ? undefined : 'back'}
          onPress={() => void run('undo')}
          isDisabled={!view.undo}
          aria-describedby={undoKeys}
          aria-label={undoLabel}
        >
          <span className={view.undo ? 'hi-label' : 'hi-label sr-only'} title={view.undo ? undefined : undoLabel}>
            {undoLabel}
          </span>
        </Button>
      )}
      <Button
        variant="ghost"
        className="hi-button"
        icon={view.redo ? undefined : 'forward'}
        onPress={() => void run('redo')}
        isDisabled={!view.redo}
        aria-describedby={redoKeys}
        aria-label={redoLabel}
      >
        <span className={view.redo ? 'hi-label' : 'hi-label sr-only'} title={view.redo ? undefined : redoLabel}>
          {redoLabel}
        </span>
      </Button>
      <span id={undoKeys} className="sr-only">
        Ctrl+Z
      </span>
      <span id={redoKeys} className="sr-only">
        Ctrl+Shift+Z or Ctrl+Y
      </span>
      {prompt && <PromptView key={prompt.id} prompt={prompt} scope={scope ?? ''} run={run} close={() => setPrompt(null)} />}
      <div className="sr-only" aria-live="polite">
        {announce}
      </div>
    </div>
  );
}

function PromptView({ prompt, scope, run, close }: { prompt: Prompt; scope: string; run: (dir: 'undo' | 'redo', o?: HistoryOptions) => Promise<void>; close: () => void }) {
  const o = prompt.outcome;
  const dir = o.direction;
  const Verb = dir === 'undo' ? 'Undo' : 'Redo';
  if (o.kind === 'failed') {
    return (
      <DecisionPanel title={`Failed to ${dir}: ${o.title}`} failed onCancel={close} choices={[{ label: 'Dismiss', onPress: close, safe: true }]}>
        <p>The engine answered: {o.message}</p>
        <p>{o.done > 0 ? `${o.done} of its calls had gone through; ${dir} again to finish it.` : 'Nothing was changed; the step is still there.'}</p>
      </DecisionPanel>
    );
  }
  if (o.kind === 'confirm') {
    return (
      <DecisionPanel
        title={`${Verb}: ${o.title}?`}
        onCancel={close}
        choices={[
          { label: 'Keep everything as it is', onPress: close, safe: true },
          { label: `${Verb} and lose it`, onPress: () => void run(dir, { ...prompt.options, confirmed: true }) },
        ]}
      >
        <p>{o.loss}</p>
      </DecisionPanel>
    );
  }
  return (
    <DecisionPanel
      title={`Not ${dir === 'undo' ? 'undone' : 'redone'}: changed outside the Studio`}
      onCancel={close}
      choices={[
        { label: `${Verb} anyway, replacing the value now`, onPress: () => void run(dir, { ...prompt.options, force: true }) },
        {
          label: 'Keep the value now and drop this step',
          onPress: () => {
            history().drop(scope, dir);
            close();
          },
        },
        { label: 'Cancel', onPress: close },
      ]}
    >
      <p>
        Another writer (Claude Code, the CLI or another window) changed what “{o.title}” left in session {scope}, so nothing was
        changed. Choose what to keep:
      </p>
      <table className="hi-table">
        <thead>
          <tr>
            <th scope="col">Setting</th>
            <th scope="col">{dir === 'undo' ? 'This step left' : 'Undo left'}</th>
            <th scope="col">Now</th>
          </tr>
        </thead>
        <tbody>
          {o.rows.map((r) => (
            <tr key={r.path}>
              <th scope="row">{r.path}</th>
              <td>{shown(r.expected)}</td>
              <td>{shown(r.now)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </DecisionPanel>
  );
}
