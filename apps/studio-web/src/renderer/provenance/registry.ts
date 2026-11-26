// Which number "Show provenance" (Ctrl+I, the command palette) opens (F-828). Every provenance trigger on screen
// registers itself here while it is mounted; the command takes, in order:
//
//   1. the trigger that has the focus;
//   2. the trigger inside the focused element (a row of the runs table, whose number is not a tab stop of its own);
//   3. the triggers an element names with `data-provenance="<key> <key>..."` (the time panel's scrubber names the snapshot
//      counter, the field view its readout, its domain, its scale bar and its legend's ticks), when the focus is in it;
//   4. the members (ADR 0795: the passive numbers of a dense row, `member`) inside the focused element;
//   5. the trigger under the pointer, or the one the element under the pointer names (the field view names its readout);
//   6. the run selected in Runs or the workspace: its own provenance (the record and the reproduce command).
//
// ADR 0795: 1 to 4 can be several numbers (a legend's five ticks have no tab stop each). Ctrl+I opens the first; pressed
// again while one of them is open it opens the next, and comes round to the first. A number alone stays open.
//
// While the command palette is open, "the focus" is the element that had it before the palette took it.
//
// The palette commands `provenance.show` and `provenance.copy_command` (src/shared/commands/studio.ts) are run here, as
// registered handlers (../commands/host.ts), so the palette needs no provenance code of its own.
import { commandFact, registerCommand } from '../commands/host.js';
import { globalCommandFor } from '../../shared/commands/shortcuts.js';
import { copyText, runProvenance } from './door.js';

export interface TriggerEntry {
  readonly el: () => HTMLElement | null;
  /** The key an element can name with data-provenance. */
  readonly key: string | null;
  readonly open: () => void;
  /** ADR 0795: a passive number of a dense row or a row of ticks: it comes after what its row opens first. */
  readonly member?: boolean;
}

const triggers = new Set<TriggerEntry>();
let hovered: TriggerEntry | null = null;
/** Opens the selected run's own provenance; set by the run-level trigger that is mounted (the selected run's id). */
const runOpeners = new Map<string, () => void>();
/** The number whose popover is open, how to close it, and the numbers Ctrl+I steps through from it (ADR 0795). */
let opened: { entry: TriggerEntry; close: () => void } | null = null;
let group: readonly TriggerEntry[] = [];
/**
 * F-988, ADR 0813: the popover open when Show provenance's key was last pressed. The key's request runs a few frames
 * later (the palette's afterFocusReturns); if that popover has closed in between (Escape), the key was about it, and is
 * spent.
 */
let pressedOn: { readonly at: { entry: TriggerEntry; close: () => void } | null } | null = null;

export function addTrigger(t: TriggerEntry): () => void {
  listen();
  triggers.add(t);
  return () => {
    triggers.delete(t);
    if (hovered === t) hovered = null;
  };
}

export function setHovered(t: TriggerEntry | null, leaving?: TriggerEntry): void {
  if (t === null && leaving !== undefined && hovered !== leaving) return;
  hovered = t;
}

export function addRunOpener(run: string, open: () => void): () => void {
  runOpeners.set(run, open);
  return () => {
    if (runOpeners.get(run) === open) runOpeners.delete(run);
  };
}

const live = (): TriggerEntry[] => [...triggers].filter((t) => t.el()?.isConnected);

/** The element last under the pointer (the field view while it is pointed at: its readout is the number shown). */
let pointed: Element | null = null;
/** The element that had the focus before the command palette took it: the palette's Show provenance is about it. */
let lastFocus: HTMLElement | null = null;
const PALETTE = '.cp-dialog';
/** The provenance popover: its focus is not the number's. */
const POPOVER = '.pv-host, .pv-pop';
let listening = false;
function listen(): void {
  if (listening || typeof document === 'undefined') return;
  listening = true;
  document.addEventListener('pointerover', (e) => (pointed = e.target instanceof Element ? e.target : null), true);
  document.addEventListener('pointerout', (e) => {
    if (e.relatedTarget === null) pointed = null;
  }, true);
  // on the window in the capture phase, as PaletteHost's: its stopPropagation would hide the key from the document
  window.addEventListener('keydown', (e) => {
    if (!e.repeat && !e.isComposing && globalCommandFor(e) === 'provenance.show') pressedOn = { at: opened };
  }, true);
  document.addEventListener('focusin', (e) => {
    if (e.target instanceof HTMLElement && !e.target.closest(`${PALETTE}, ${POPOVER}`)) lastFocus = e.target;
  }, true);
}

/** The keys a `data-provenance` names: one key as it is written (it may hold spaces), else a list of them. */
const keysOf = (all: readonly TriggerEntry[], text: string): readonly string[] => (all.some((x) => x.key === text) ? [text] : text.split(/\s+/).filter((k) => k !== ''));

/** The triggers the nearest `data-provenance` at or above `el` names, in its order. */
function named(all: readonly TriggerEntry[], el: Element | null): TriggerEntry[] {
  const text = el?.closest<HTMLElement>('[data-provenance]')?.dataset['provenance'];
  if (!text) return [];
  return keysOf(all, text).flatMap((k) => all.find((x) => x.key === k) ?? []);
}

const inDocumentOrder = (a: TriggerEntry, b: TriggerEntry): number => {
  const x = a.el();
  const y = b.el();
  return x && y && x !== y && (x.compareDocumentPosition(y) & 4) !== 0 ? -1 : 1;
};

/** The numbers of a focused element, in the order Ctrl+I steps through them (1 to 4 above). */
export function numbersOf(all: readonly TriggerEntry[], active: Element): TriggerEntry[] {
  const own = all.find((t) => t.el() === active);
  if (own) return [own];
  const inside = all.filter((t) => {
    const e = t.el();
    return e !== null && active.contains(e);
  });
  return [...new Set([...inside.filter((t) => !t.member), ...named(all, active), ...inside.filter((t) => t.member).sort(inDocumentOrder)])];
}

/** The numbers "Show provenance" opens now, the first of them first; none when there is none. */
function targets(): readonly TriggerEntry[] {
  const all = live();
  const focused = document.activeElement;
  // while the palette is open its search field has the focus: the number meant is the one focused before it opened
  const active = focused instanceof HTMLElement && focused.closest(PALETTE) ? (lastFocus?.isConnected ? lastFocus : null) : focused;
  if (active instanceof HTMLElement && active !== document.body) {
    const of = numbersOf(all, active);
    if (of.length > 0) return of;
  }
  if (hovered && all.includes(hovered)) return [hovered];
  return named(all, pointed?.isConnected ? pointed : null);
}

/** The trigger "Show provenance" opens now, or null. */
export const target = (): TriggerEntry | null => targets()[0] ?? null;

/** ADR 0795: a popover opened; Ctrl+I steps on from it through the numbers of the element that has the focus now. */
export function setOpened(entry: TriggerEntry, close: () => void): void {
  opened = { entry, close };
  if (group.includes(entry)) return;
  const a = document.activeElement;
  const of = a instanceof HTMLElement && a !== document.body ? numbersOf(live(), a) : [];
  group = of.includes(entry) ? of : [entry];
}

export function clearOpened(entry: TriggerEntry): void {
  if (opened?.entry === entry) opened = null;
}

const FOCUSABLE = 'button, input, [tabindex="0"]';

/**
 * ADR 0795: a click on a passive number (a member) opens it, with the focus on the element that names it, which takes the
 * focus back when the popover closes. Called by the surface's own click handler; a click in a button or a grid's row is
 * theirs (the segment is chosen, the row selected): there the number opens by Ctrl+I with the pointer on it.
 */
export function openMember(e: { readonly target: EventTarget; readonly currentTarget: Element; isDefaultPrevented(): boolean; preventDefault(): void }): void {
  if (e.isDefaultPrevented() || !(e.target instanceof Element)) return;
  const el = e.target.closest('[data-provenance-member]');
  if (!el || !e.currentTarget.contains(el) || el.closest('button, [role="row"]')) return;
  const all = live();
  const entry = all.find((t) => t.el() === el);
  if (!entry) return;
  const names = Array.from(e.currentTarget.querySelectorAll<HTMLElement>('[data-provenance]')).find((x) => entry.key !== null && keysOf(all, x.dataset['provenance']!).includes(entry.key));
  const host = names ? (names.tabIndex >= 0 || names.matches('button') ? names : names.querySelector<HTMLElement>(FOCUSABLE)) : null;
  host?.focus({ preventScroll: true });
  e.preventDefault();
  entry.open();
}

const selectedRun = (): string | null => commandFact<string | null>('selectedRun') ?? null;

/** Opens the trigger drawn by `el` (a host's own click on a passive number); false when none is. */
export function openTrigger(el: Element): boolean {
  const t = live().find((x) => x.el() === el);
  if (t) t.open();
  return t !== undefined;
}

/** Why "Show provenance" cannot open anything now, or null. */
export function showUnavailable(): string | null {
  if (opened || target()) return null;
  const run = selectedRun();
  if (run !== null && runOpeners.has(run)) return null;
  return 'No number to show: focus or point at a number (in Runs, the Workspace, a field view, the settings or the plots), or select a run, then Show provenance (Ctrl+I).';
}

export function showNow(): boolean {
  // F-988, ADR 0813: a key pressed while a popover was open, that popover closed since: nothing opens
  const pressed = pressedOn;
  pressedOn = null;
  if (pressed !== null && pressed.at !== null && pressed.at !== opened) return true;
  if (opened) {
    // ADR 0795: the next number of the same element; the only one stays open
    const at = opened.entry;
    const all = live();
    const rest = group.filter((t) => t === at || all.includes(t));
    const next = rest[(rest.indexOf(at) + 1) % rest.length];
    if (next && next !== at) {
      opened.close();
      next.open();
    }
    return true;
  }
  const of = targets();
  const t = of[0];
  if (t) {
    group = of;
    t.open();
    return true;
  }
  const run = selectedRun();
  const open = run === null ? undefined : runOpeners.get(run);
  if (open) {
    open();
    return true;
  }
  return false;
}

/** Copies the reproduce command of a run; the words of the outcome (the gaps go with it). */
export async function copyCommandOf(run: string): Promise<{ ok: boolean; lines: string[] }> {
  const r = await runProvenance(run);
  if (!r.ok) return { ok: false, lines: [`The provenance of ${run} could not be read: ${r.error.message}`] };
  const rep = r.result.reproduce;
  if (!rep.text) return { ok: false, lines: [`No command reproduces ${run}:`, ...rep.gaps] };
  const c = await copyText(rep.text);
  if (!c.ok) return { ok: false, lines: [`The command could not be copied: ${c.error.message}`, rep.text] };
  return { ok: true, lines: [rep.text, ...rep.gaps.map((g) => `Not reproduced exactly: ${g}`)] };
}

let registered = false;

/** Registers the palette's provenance commands once per page (they stay for the page's life). */
export function registerProvenanceCommands(): void {
  if (registered) return;
  registered = true;
  registerCommand('provenance.show', { unavailable: showUnavailable, run: () => void showNow() });
  registerCommand('provenance.copy_command', {
    unavailable: () => null,
    run: async (args) => {
      const run = typeof args['run'] === 'string' ? args['run'] : selectedRun();
      return run === null ? { ok: false, lines: ['Choose a run first.'] } : copyCommandOf(run);
    },
  });
}
