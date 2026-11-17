// A decision the Studio will not take for the user (F-829, G18): what would be lost or what another writer changed, in
// words, and named choices. A floating panel under the header band (an alertdialog): focus goes to the safe choice when
// there is one (a loss: keeping is the default), else to the title, so no choice is preselected (a conflict); Escape
// is Cancel; focus goes back where it was when the panel closes. Keyboard: Tab moves through the choices and stays in
// the panel while it is open (G8, F-816).
import { useEffect, useId, useRef, type ReactNode } from 'react';
import { Button } from '../ui/index.js';
import { containTab } from '../ui/containFocus.js';
import './history.css';

export interface Choice {
  readonly label: string;
  readonly onPress: () => void;
  /** The safe choice: primary, and focused when the panel opens. */
  readonly safe?: boolean;
  readonly isDisabled?: boolean;
}

export interface DecisionPanelProps {
  readonly title: string;
  readonly children: ReactNode;
  readonly choices: readonly Choice[];
  /** Escape, and the choice that changes nothing. */
  readonly onCancel: () => void;
  /** A failure (Failure Red title, with the word "Failed" in it). */
  readonly failed?: boolean;
}

export function DecisionPanel({ title, children, choices, onCancel, failed }: DecisionPanelProps) {
  const titleId = useId();
  const bodyId = useId();
  const root = useRef<HTMLElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const cancel = useRef(onCancel);
  cancel.current = onCancel;

  useEffect(() => {
    const before = document.activeElement as HTMLElement | null;
    const safe = root.current?.querySelector<HTMLElement>('[data-safe="true"]');
    (safe ?? heading.current)?.focus();
    return () => {
      if (before && before.isConnected) before.focus();
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && root.current?.contains(document.activeElement)) {
        e.preventDefault();
        e.stopPropagation();
        cancel.current();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  return (
    <section
      ref={root}
      className={failed ? 'hi-panel hi-panel--failed' : 'hi-panel'}
      role="alertdialog"
      aria-modal="true"
      onKeyDown={containTab}
      aria-labelledby={titleId}
      aria-describedby={bodyId}
      data-words="exception"
    >
      <h2 id={titleId} ref={heading} tabIndex={-1} className="hi-panel-title">
        {title}
      </h2>
      <div id={bodyId} className="hi-panel-body">
        {children}
      </div>
      <div className="hi-choices">
        {choices.map((c) => (
          <Button key={c.label} variant={c.safe ? 'primary' : 'default'} onPress={c.onPress} isDisabled={c.isDisabled} data-safe={c.safe ? 'true' : undefined}>
            {c.label}
          </Button>
        ))}
      </div>
    </section>
  );
}
