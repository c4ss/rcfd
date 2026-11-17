// The progress of long operations (F-832, G21; G22: the exception speaks, briefly): the works of one surface that have
// lasted PROGRESS_AFTER_MS, each as an indeterminate bar drawn as DESIGN.md's "not yet" hatch moving (still under
// reduced motion), its few words, and Cancel. Nothing at all is rendered while there is no such work, so a short
// operation never shows a word. The bar's accessible name says what is being done to what; the Cancel button's name
// says what it cancels (tests find both by role and name, G13).
import { useEffect, useRef, useSyncExternalStore } from 'react';
import { Button } from '../ui/index.js';
import { beginWork, work, type Work, type WorkHandle, type WorkSpec } from './work.js';
import './work.css';

/** The works shown now in `scope`. */
export function useShownWork(scope: string): readonly Work[] {
  const all = useSyncExternalStore(
    (l) => work.subscribe(l),
    () => work.shown(),
  );
  return all.filter((w) => w.scope === scope);
}

/**
 * A work that lasts while `active` is true (begun when it turns true, ended when it turns false, the spec's key
 * changes, or the component unmounts). Returns the handle while it lasts.
 */
export function useWorkWhile(active: boolean, spec: WorkSpec, key: string): WorkHandle | null {
  const handle = useRef<WorkHandle | null>(null);
  const specRef = useRef(spec);
  specRef.current = spec;
  useEffect(() => {
    if (!active) return;
    const h = beginWork({ ...specRef.current, onCancel: () => specRef.current.onCancel?.() });
    handle.current = h;
    return () => {
      h.end();
      if (handle.current === h) handle.current = null;
    };
  }, [active, key]);
  return handle.current;
}

const lower = (s: string): string => s.charAt(0).toLowerCase() + s.slice(1);

export function WorkProgress({ scope, placement = 'overlay' }: { scope: string; placement?: 'overlay' | 'inline' | 'band' }) {
  const works = useShownWork(scope);
  if (works.length === 0) return null;
  return (
    <div className={`wk wk--${placement}`} role="status" aria-label="In progress">
      {works.map((w) => (
        <div key={w.id} className="wk-item" data-kind={w.kind}>
          <div className="wk-bar" role="progressbar" aria-label={w.label} />
          <span className="wk-word" aria-hidden="true">
            {w.word}
          </span>
          <Button className="wk-cancel" aria-label={`Cancel ${lower(w.label)}`} onPress={() => w.cancel()}>
            Cancel
          </Button>
        </div>
      ))}
    </div>
  );
}
