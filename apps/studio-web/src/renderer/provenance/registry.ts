// Which number "Show provenance" (Ctrl+I, the command palette) opens (F-828). Every provenance trigger on screen
// registers itself here while it is mounted; the command takes, in order:
//
//   1. the trigger that has the focus;
//   2. the trigger inside the focused element (a row of the runs table, whose number is not a tab stop of its own);
//   3. the trigger an element names with `data-provenance="<key>"` (the time panel's scrubber names the snapshot counter,
//      the field view its readout), when the focus is in that element;
//   4. the trigger under the pointer, or the one the element under the pointer names (the field view names its readout);
//   5. the run selected in Runs or the workspace: its own provenance (the record and the reproduce command).
//
// The palette commands `provenance.show` and `provenance.copy_command` (src/shared/commands/studio.ts) are run here, as
// registered handlers (../commands/host.ts), so the palette needs no provenance code of its own.
import { commandFact, registerCommand } from '../commands/host.js';
import { copyText, runProvenance } from './door.js';

export interface TriggerEntry {
  readonly el: () => HTMLElement | null;
  /** The key an element can name with data-provenance. */
  readonly key: string | null;
  readonly open: () => void;
}

const triggers = new Set<TriggerEntry>();
let hovered: TriggerEntry | null = null;
/** Opens the selected run's own provenance; set by the run-level trigger that is mounted (the selected run's id). */
const runOpeners = new Map<string, () => void>();

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
let listening = false;
function listen(): void {
  if (listening || typeof document === 'undefined') return;
  listening = true;
  document.addEventListener('pointerover', (e) => (pointed = e.target instanceof Element ? e.target : null), true);
  document.addEventListener('pointerout', (e) => {
    if (e.relatedTarget === null) pointed = null;
  }, true);
}

const byKey = (all: readonly TriggerEntry[], el: Element | null): TriggerEntry | null => {
  const named = el?.closest<HTMLElement>('[data-provenance]')?.dataset['provenance'];
  return named ? (all.find((x) => x.key === named) ?? null) : null;
};

/** The trigger "Show provenance" opens now, or null. */
export function target(): TriggerEntry | null {
  const all = live();
  const active = document.activeElement;
  if (active instanceof HTMLElement && active !== document.body) {
    const own = all.find((t) => t.el() === active);
    if (own) return own;
    const inside = all.find((t) => {
      const e = t.el();
      return e !== null && active.contains(e);
    });
    if (inside) return inside;
    const named = byKey(all, active);
    if (named) return named;
  }
  if (hovered && all.includes(hovered)) return hovered;
  return byKey(all, pointed?.isConnected ? pointed : null);
}

const selectedRun = (): string | null => commandFact<string | null>('selectedRun') ?? null;

/** Why "Show provenance" cannot open anything now, or null. */
export function showUnavailable(): string | null {
  if (target()) return null;
  const run = selectedRun();
  if (run !== null && runOpeners.has(run)) return null;
  return 'No number to show: focus or point at a number of Runs or the Workspace (or select a run), then Show provenance (Ctrl+I).';
}

export function showNow(): boolean {
  const t = target();
  if (t) {
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
