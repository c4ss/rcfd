// The Quiet Rule (G22, DESIGN.md; F-833): the words a normal state needs are said to readers and in a tooltip, not as
// standing text. Three pieces, used by every surface:
// - Quiet: words for assistive technology only (the .sr-only class); a heading, a caption or a source line whose place
//   on the screen already says it.
// - EmptyMark: an empty state at rest. DESIGN.md's "not yet" hatch, with the sentence as its accessible name (role
//   note) and its tooltip; tests read it by role and name.
// - Dash: a normal "no value" (a steady run's time, a case that builds its own mesh): an en dash, its words as the name.
import type { ReactNode } from 'react';
import { cx } from './cx.js';

/** Words for readers only: kept in the accessibility tree and the text of the element, not shown. */
export function Quiet({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <span className="sr-only" id={id}>
      {children}
    </span>
  );
}

/** An empty state with no visible words: the hatch, the sentence as its name and tooltip (G19 help on the spot). */
export function EmptyMark({ text, className }: { text: string; className?: string }) {
  return (
    <p className={cx('ui-empty', className)} role="note" aria-label={text} title={text} data-empty="">
      <span className="ui-empty-hatch" aria-hidden="true" />
    </p>
  );
}

/** A normal absence of a value: an en dash on screen, the words for readers and in the tooltip. */
export function Dash({ words }: { words: string }) {
  return (
    <span className="ui-dash" title={words}>
      <span aria-hidden="true">–</span>
      <span className="sr-only">{words}</span>
    </span>
  );
}
