// Reduced motion for the motion JavaScript makes (F-816, G8; DESIGN.md Motion: off under prefers-reduced-motion).
// CSS motion is switched off for the whole window by base.css; what moves from a timer or a frame loop asks here.
import { useEffect, useState } from 'react';

const QUERY = '(prefers-reduced-motion: reduce)';

/** Whether the user asked the system for reduced motion, now. */
export function prefersReducedMotion(): boolean {
  return typeof matchMedia === 'function' && matchMedia(QUERY).matches;
}

/** Whether the user asks for reduced motion, kept up to date when the setting changes. */
export function usePrefersReducedMotion(): boolean {
  const [reduce, setReduce] = useState(prefersReducedMotion);
  useEffect(() => {
    if (typeof matchMedia !== 'function') return;
    const m = matchMedia(QUERY);
    const on = () => setReduce(m.matches);
    on();
    m.addEventListener('change', on);
    return () => m.removeEventListener('change', on);
  }, []);
  return reduce;
}

/**
 * Play under reduced motion (F-816 decision D3): one stop at most every this many ms, below three changes a second, so
 * the field does not flash as it steps; without the preference Play goes as fast as the fields are drawn.
 */
export const REDUCED_PLAY_MS = 500;
