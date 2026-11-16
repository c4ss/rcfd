// The Studio's one mechanism for long operations (F-832, guardrail G21): every operation that may take long (opening a
// project, opening a run, loading a field, checking or saving a case, checking a mesh) begins a work here and ends it
// when it is done, failed or cancelled. A work that is still going after PROGRESS_AFTER_MS is shown, by WorkProgress
// in its own surface (`scope`), as progress with a Cancel; a shorter one is never shown (G22: the normal state carries
// no words). Cancel aborts the work's AbortSignal, which the operation hands through its door (the data door's tag
// cancel, the case and project ports' cancel requests), and runs the work's own `onCancel` (dropping a result, going
// back to the run selected before).
//
// No DOM: timers and AbortController only, so tests/unit/work.spec.ts drives it in Node.
import { PROGRESS_AFTER_MS, type WorkKind } from '../../shared/work.js';

export interface WorkSpec {
  readonly kind: WorkKind;
  /** The surface that shows it (WorkProgress `scope`). */
  readonly scope: string;
  /** The accessible name of its progress: what is being done, to what ("Opening run r-20261116T000000-1e05"). */
  readonly label: string;
  /** The few visible words while it is shown ("Opening run"); G22: brief. */
  readonly word: string;
  /** Called once when the user cancels, after the signal aborted. */
  readonly onCancel?: () => void;
}

export interface Work extends WorkSpec {
  readonly id: number;
  readonly startedAt: number;
  readonly signal: AbortSignal;
  /** Cancels the work (the user's Cancel). */
  cancel(): void;
}

export interface WorkHandle {
  readonly id: number;
  readonly signal: AbortSignal;
  /** True once cancelled by the user (end() after a cancel does nothing more). */
  readonly cancelled: boolean;
  /** The operation is over (done, failed or cancelled): it is no longer shown. Idempotent. */
  end(): void;
  /** Cancels it as the user's Cancel does. */
  cancel(): void;
}

export interface TrackerOptions {
  readonly afterMs?: number;
  readonly now?: () => number;
  readonly setTimer?: (fn: () => void, ms: number) => unknown;
  readonly clearTimer?: (t: unknown) => void;
}

interface Entry {
  readonly work: Work;
  readonly controller: AbortController;
  shown: boolean;
  ended: boolean;
  cancelled: boolean;
  timer: unknown;
}

export class WorkTracker {
  private readonly afterMs: number;
  private readonly now: () => number;
  private readonly setTimer: (fn: () => void, ms: number) => unknown;
  private readonly clearTimer: (t: unknown) => void;
  private readonly entries = new Map<number, Entry>();
  private readonly listeners = new Set<() => void>();
  private nextId = 1;
  private shownList: readonly Work[] = [];

  constructor(options: TrackerOptions = {}) {
    this.afterMs = options.afterMs ?? PROGRESS_AFTER_MS;
    this.now = options.now ?? (() => Date.now());
    this.setTimer = options.setTimer ?? ((fn, ms) => setTimeout(fn, ms));
    this.clearTimer = options.clearTimer ?? ((t) => clearTimeout(t as ReturnType<typeof setTimeout>));
  }

  /** Begins a work; it is shown once it has lasted `afterMs`. */
  begin(spec: WorkSpec): WorkHandle {
    const id = this.nextId++;
    const controller = new AbortController();
    const entry: Entry = {
      work: { ...spec, id, startedAt: this.now(), signal: controller.signal, cancel: () => this.cancel(id) },
      controller,
      shown: false,
      ended: false,
      cancelled: false,
      timer: null,
    };
    this.entries.set(id, entry);
    entry.timer = this.setTimer(() => {
      entry.timer = null;
      if (entry.ended) return;
      entry.shown = true;
      this.changed();
    }, this.afterMs);
    return {
      id,
      signal: controller.signal,
      get cancelled() {
        return entry.cancelled;
      },
      end: () => this.end(id),
      cancel: () => this.cancel(id),
    };
  }

  /** The user's Cancel: aborts the signal, runs `onCancel`, and ends the work. */
  cancel(id: number): void {
    const e = this.entries.get(id);
    if (!e || e.ended) return;
    e.cancelled = true;
    e.controller.abort();
    try {
      e.work.onCancel?.();
    } finally {
      this.end(id);
    }
  }

  end(id: number): void {
    const e = this.entries.get(id);
    if (!e || e.ended) return;
    e.ended = true;
    if (e.timer !== null) this.clearTimer(e.timer);
    this.entries.delete(id);
    if (e.shown) this.changed();
  }

  /** The works shown now (lasted `afterMs` and not ended), oldest first; the same array until something changes. */
  shown(): readonly Work[] {
    return this.shownList;
  }

  /** Every work going now, shown or not (for tests and diagnostics). */
  going(): readonly Work[] {
    return [...this.entries.values()].map((e) => e.work);
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private changed(): void {
    this.shownList = [...this.entries.values()].filter((e) => e.shown && !e.ended).map((e) => e.work);
    for (const l of [...this.listeners]) {
      try {
        l();
      } catch {
        // a listener's failure is its own
      }
    }
  }
}

/** The Studio's one tracker. */
export const work = new WorkTracker();

/** Begins a work on the Studio's tracker. */
export const beginWork = (spec: WorkSpec): WorkHandle => work.begin(spec);

/** Rejects (as an AbortError) once `signal` aborts; resolves with `p` otherwise. Used to drop a result on cancel. */
export function abortable<T>(p: Promise<T>, signal: AbortSignal | undefined): Promise<T> {
  if (!signal) return p;
  if (signal.aborted) return Promise.reject(abortError());
  return new Promise<T>((resolve, reject) => {
    const onAbort = () => reject(abortError());
    signal.addEventListener('abort', onAbort, { once: true });
    p.then(
      (v) => {
        signal.removeEventListener('abort', onAbort);
        resolve(v);
      },
      (e: unknown) => {
        signal.removeEventListener('abort', onAbort);
        reject(e);
      },
    );
  });
}

export function abortError(): Error {
  const e = new Error('cancelled');
  e.name = 'AbortError';
  return e;
}

export const isAbort = (e: unknown): boolean => e instanceof Error && e.name === 'AbortError';

/** `onCancel` for a door: the preload's cancel function is called when `signal` aborts (at once if it already has). */
export function cancelHook(signal: AbortSignal | undefined): { onCancel?: (cancel: () => void) => void } {
  if (!signal) return {};
  return {
    onCancel: (cancel) => {
      if (signal.aborted) cancel();
      else signal.addEventListener('abort', () => cancel(), { once: true });
    },
  };
}
