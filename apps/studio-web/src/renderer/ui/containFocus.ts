// Tab kept inside an alert dialog (F-816, G8). The Studio's decision panel and a run's confirmation strip are alert
// dialogs that take the focus when they open and give it back when they close; while open, Tab and Shift+Tab go round
// their own controls (WAI-ARIA's dialog pattern), so Escape, which only they answer, is always in reach.
import type { KeyboardEvent } from 'react';

const TABBABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** onKeyDown of the dialog's root: Tab past the last control goes to the first, Shift+Tab before the first to the last. */
export function containTab(e: KeyboardEvent<HTMLElement>): void {
  if (e.key !== 'Tab' || e.altKey || e.ctrlKey || e.metaKey) return;
  const root = e.currentTarget;
  const list = Array.from(root.querySelectorAll<HTMLElement>(TABBABLE)).filter((el) => el.checkVisibility?.() ?? true);
  if (list.length === 0) {
    e.preventDefault();
    return;
  }
  const first = list[0]!;
  const last = list[list.length - 1]!;
  const at = document.activeElement as HTMLElement | null;
  const inside = at !== null && list.includes(at);
  if (e.shiftKey ? at === first || !inside : at === last || !inside) {
    e.preventDefault();
    (e.shiftKey ? last : first).focus();
  }
}
