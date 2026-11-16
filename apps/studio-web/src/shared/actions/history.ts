// Undo and redo of the Studio's edits (F-829, guardrail G18), built on the action log and the registry's undo hooks
// (F-827 D7). Every change action that succeeds from the UI and has an undo hook becomes a step of its session's
// history: settings.set, settings.remove and session.create (a session made from the page). Undo dispatches the step's
// inverse through the dispatcher (origin `undo`), redo dispatches the step's own call again (origin `redo`), so both are
// logged, replayable and the engine's (G2, G16).
//
// Three rules keep undo safe:
// - Scope: one history per session (the session the edit changed, or the session session.create made). A session that
//   closes takes its history with it; a restarted service clears every history (its sessions are gone).
// - Conflict: before undoing (or redoing) a step the settings it left are read again; a value another writer (Claude
//   Code, the CLI, a second window) changed since is a conflict, shown with the values on both sides and a choice,
//   never overwritten silently (G18). The service has no compare-and-set, so the read and the write are two calls; the
//   gap between them is a few milliseconds (reported as a missing engine entry point).
// - Loss: undoing session.create closes the session, which cannot be undone in turn, so it is confirmed in words that
//   name what is lost (loss.ts); so is an undo or redo of a settings change on a session that holds results, which the
//   change discards.
//
// Shared by the renderer, the main process and the tests: no Node or DOM types.
import type { JsonObject, JsonValue } from './params.js';
import type { ActionReply, Dispatcher } from './dispatcher.js';
import { getAction, type ActionCall } from './registry.js';
import { lossOf, NO_INVERSE, statusFacts, type SessionFacts } from './loss.js';

/** What a surface may say about a step it records (dispatch's `undo` option in the renderer). */
export interface RecordOptions {
  /**
   * The actions that undo this one, instead of the registry's inverse. The settings surface gives one when it makes a
   * named entry from its first key (the registry's inverse of that settings.set is none: the key did not exist before):
   * removing the entry it made.
   */
  readonly inverse?: readonly ActionCall[];
  /** The step's title instead of the one made from the action (for example "add boundaries.inlet"). */
  readonly title?: string;
}

/** A setting a step expects to find before it is undone or redone: the value it left; null for none (absent). */
export interface Expectation {
  readonly path: string;
  readonly value: JsonValue;
}

/** One setting of a conflict: what the step left, what the session holds now, and what undo or redo would set. */
export interface ConflictRow {
  readonly path: string;
  /** The value this step left (null: absent). */
  readonly expected: JsonValue;
  /** The value now, set by another writer (null: absent). */
  readonly now: JsonValue;
}

export type Direction = 'undo' | 'redo';

export type HistoryOutcome =
  | { readonly kind: 'nothing'; readonly direction: Direction }
  | { readonly kind: 'busy'; readonly direction: Direction }
  /** The step cannot be undone this way; `reason` says why (the registry's note). */
  | { readonly kind: 'unavailable'; readonly direction: Direction; readonly title: string; readonly reason: string }
  /** Something would be lost: ask, then call again with `confirmed: true`. */
  | { readonly kind: 'confirm'; readonly direction: Direction; readonly title: string; readonly scope: string; readonly loss: string }
  /** Another writer changed what the step left: show the rows, then `force: true` or `drop`. */
  | { readonly kind: 'conflict'; readonly direction: Direction; readonly title: string; readonly scope: string; readonly rows: readonly ConflictRow[] }
  /** A call was refused; `done` calls of the step had gone through first. The step stays where it was. */
  | { readonly kind: 'failed'; readonly direction: Direction; readonly title: string; readonly message: string; readonly done: number }
  | { readonly kind: 'done'; readonly direction: Direction; readonly title: string; readonly scope: string };

export interface HistoryOptions {
  /** Undo or redo even though another writer changed the settings the step touches (the user chose to). */
  readonly force?: boolean;
  /** The user confirmed the loss the last outcome named. */
  readonly confirmed?: boolean;
}

/** What a control shows for one direction of a scope. */
export interface StepView {
  /** "set solver.alpha_u": the button reads "Undo: set solver.alpha_u". */
  readonly title: string;
  readonly available: boolean;
  /** Why it is unavailable. */
  readonly reason: string | null;
}

export interface HistoryView {
  readonly scope: string;
  readonly undo: StepView | null;
  readonly redo: StepView | null;
  readonly undoDepth: number;
  readonly redoDepth: number;
  /** Titles of the steps that can be undone, oldest first (for the words of a close). */
  readonly titles: readonly string[];
}

export type HistoryEvent = { readonly kind: 'changed' } | { readonly kind: 'renamed'; readonly from: string; readonly to: string };

interface Step {
  readonly id: number;
  readonly action: string;
  title: string;
  /** The call that redo makes again. */
  forward: ActionCall;
  /** The calls that undo it, in order; null when it cannot be undone. */
  backward: ActionCall[] | null;
  /** `backward` came from the surface (RecordOptions.inverse): a redo keeps it. */
  readonly fixed: boolean;
  /** Why there is no inverse. */
  readonly note: string | null;
  /** What the step left (checked before undo). */
  after: Expectation[];
  /** What undoing it left (checked before redo). */
  before: Expectation[];
}

interface Stacks {
  undo: Step[];
  redo: Step[];
}

/** Steps kept per session (oldest dropped first). */
export const MAX_STEPS = 200;

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

/** JSON with sorted keys: equal values give equal text. */
export function canonical(v: unknown): string {
  if (v === undefined || v === null) return 'null';
  if (Array.isArray(v)) return `[${v.map(canonical).join(',')}]`;
  if (isObject(v)) {
    return `{${Object.keys(v)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canonical(v[k])}`)
      .join(',')}}`;
  }
  return JSON.stringify(v);
}

/** The step's title from its call ("set solver.alpha_u", "remove boundaries.lid", "new session s3"). */
export function stepTitle(call: ActionCall, made?: string): string {
  const path = typeof call.params['path'] === 'string' ? call.params['path'] : '';
  switch (call.action) {
    case 'settings.set':
      return `set ${path}`;
    case 'settings.remove':
      return `remove ${path}`;
    case 'session.create':
      return `new session ${made ?? ''}${path ? ` from ${path}` : ''}`.trim();
    default:
      return getAction(call.action)?.title.toLowerCase() ?? call.action;
  }
}

/** The settings a successful call left, as the engine reported them. */
function left(call: ActionCall, result: unknown): Expectation[] {
  const path = typeof call.params['path'] === 'string' ? call.params['path'] : null;
  if (path === null) return [];
  if (call.action === 'settings.set') return [{ path, value: isObject(result) && 'value' in result ? ((result['value'] ?? null) as JsonValue) : null }];
  if (call.action === 'settings.remove') return [{ path, value: null }];
  return [];
}

const sessionOfCall = (c: ActionCall): string | null => (typeof c.params['session'] === 'string' ? c.params['session'] : null);

export class History {
  private readonly scopes = new Map<string, Stacks>();
  private readonly listeners = new Set<(e: HistoryEvent) => void>();
  private nextId = 1;
  private running = false;

  constructor(private readonly dispatcher: Pick<Dispatcher, 'dispatchCall'>) {}

  /**
   * Records a dispatched action: a step when it is an edit from the UI with an undo hook that succeeded. A closed
   * session's history goes with it. Anything else is ignored (reads, failures, replays, undo and redo themselves).
   */
  record(reply: ActionReply, options: RecordOptions = {}): void {
    const e = reply.entry;
    if (!reply.ok || e.effect !== 'change' || e.origin !== 'ui') return;
    if (e.action === 'session.close') {
      const s = sessionOfCall(e);
      if (s) this.forget(s);
      return;
    }
    const a = getAction(e.action);
    if (!a?.undo) return;
    const scope = e.action === 'session.create' ? (e.session ?? null) : sessionOfCall(e);
    if (!scope) return;
    const call: ActionCall = { action: e.action, params: e.params };
    const backward = options.inverse ? [...options.inverse] : e.inverse ? [...e.inverse] : null;
    const step: Step = {
      id: this.nextId++,
      action: e.action,
      title: options.title ?? stepTitle(call, e.session),
      forward: call,
      backward,
      fixed: options.inverse !== undefined,
      note: backward ? null : (a.undo.note ?? 'The engine gives no way back for this change.'),
      after: left(call, reply.result),
      before: [],
    };
    const st = this.stacks(scope);
    st.undo.push(step);
    if (st.undo.length > MAX_STEPS) st.undo.shift();
    st.redo = [];
    this.emit({ kind: 'changed' });
  }

  /** What the controls of a scope show. */
  view(scope: string): HistoryView {
    const st = this.scopes.get(scope);
    const top = (s: Step | undefined, dir: Direction): StepView | null =>
      s ? { title: s.title, available: dir === 'redo' || s.backward !== null, reason: dir === 'undo' && s.backward === null ? s.note : null } : null;
    return {
      scope,
      undo: top(st?.undo.at(-1), 'undo'),
      redo: top(st?.redo.at(-1), 'redo'),
      undoDepth: st?.undo.length ?? 0,
      redoDepth: st?.redo.length ?? 0,
      titles: (st?.undo ?? []).map((s) => s.title),
    };
  }

  /** Calls `listener` after every change; returns the function that stops it. */
  subscribe(listener: (e: HistoryEvent) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  undo(scope: string, options: HistoryOptions = {}): Promise<HistoryOutcome> {
    return this.move(scope, 'undo', options);
  }

  redo(scope: string, options: HistoryOptions = {}): Promise<HistoryOutcome> {
    return this.move(scope, 'redo', options);
  }

  /**
   * Drops the next step of a direction without running it: the user keeps what another writer set (a conflict's
   * "keep theirs"). The steps beyond it stay.
   */
  drop(scope: string, direction: Direction): void {
    const st = this.scopes.get(scope);
    const list = direction === 'undo' ? st?.undo : st?.redo;
    if (list?.length) {
      list.pop();
      this.emit({ kind: 'changed' });
    }
  }

  /** Forgets a scope (its session closed). */
  forget(scope: string): void {
    if (this.scopes.delete(scope)) this.emit({ kind: 'changed' });
  }

  /** Forgets every scope (the service restarted: its sessions are gone). */
  clear(): void {
    if (this.scopes.size) {
      this.scopes.clear();
      this.emit({ kind: 'changed' });
    }
  }

  private stacks(scope: string): Stacks {
    let st = this.scopes.get(scope);
    if (!st) {
      st = { undo: [], redo: [] };
      this.scopes.set(scope, st);
    }
    return st;
  }

  private emit(e: HistoryEvent): void {
    for (const l of this.listeners) {
      try {
        l(e);
      } catch {
        // a listener's failure is its own
      }
    }
  }

  private async move(scope: string, dir: Direction, options: HistoryOptions): Promise<HistoryOutcome> {
    if (this.running) return { kind: 'busy', direction: dir };
    const st = this.scopes.get(scope);
    const step = dir === 'undo' ? st?.undo.at(-1) : st?.redo.at(-1);
    if (!st || !step) return { kind: 'nothing', direction: dir };
    const calls = dir === 'undo' ? step.backward : [step.forward];
    if (!calls) return { kind: 'unavailable', direction: dir, title: step.title, reason: step.note ?? 'This step cannot be undone.' };
    this.running = true;
    try {
      // 1. what the step (or its undo) left must still be there
      if (!options.force) {
        const rows = await this.changedSince(scope, dir === 'undo' ? step.after : step.before);
        if (rows === 'gone') return { kind: 'failed', direction: dir, title: step.title, message: `Session ${scope} is no longer on the service.`, done: 0 };
        if (rows.length) return { kind: 'conflict', direction: dir, title: step.title, scope, rows };
      }
      // 2. what would be lost
      if (!options.confirmed) {
        const loss = await this.lossOf(scope, step, calls);
        if (loss) return { kind: 'confirm', direction: dir, title: step.title, scope, loss };
      }
      // 3. the calls, through the dispatcher
      const replies: ActionReply[] = [];
      for (const c of calls) {
        const r = await this.dispatcher.dispatchCall(c, { origin: dir });
        if (!r.ok) return { kind: 'failed', direction: dir, title: step.title, message: r.error.message, done: replies.length };
        replies.push(r);
      }
      let target = scope;
      if (dir === 'undo') {
        step.before = replies.flatMap((r, i) => left(calls[i]!, r.ok ? r.result : null));
        st.undo.pop();
        st.redo.push(step);
      } else {
        const r = replies[0]!;
        step.after = left(step.forward, r.ok ? r.result : null);
        if (!step.fixed) step.backward = r.entry.inverse ? [...r.entry.inverse] : step.backward;
        st.redo.pop();
        st.undo.push(step);
        // a session made again has a new id: the scope follows it
        const made = r.entry.session;
        if (step.action === 'session.create' && made && made !== scope) {
          target = made;
          this.follow(scope, made);
          step.title = stepTitle(step.forward, made);
          step.backward = r.entry.inverse ? [...r.entry.inverse] : null;
        }
      }
      this.emit({ kind: 'changed' });
      if (target !== scope) this.emit({ kind: 'renamed', from: scope, to: target });
      return { kind: 'done', direction: dir, title: step.title, scope: target };
    } finally {
      this.running = false;
    }
  }

  /** The settings of `expect` whose value now differs, or 'gone' when the session is not on the service. */
  private async changedSince(scope: string, expect: readonly Expectation[]): Promise<ConflictRow[] | 'gone'> {
    const rows: ConflictRow[] = [];
    for (const x of expect) {
      const r = await this.dispatcher.dispatchCall({ action: 'settings.get', params: { session: scope, path: x.path } }, { origin: 'undo' });
      if (!r.ok && r.error.code === 1001) return 'gone';
      // a path the engine does not find is absent (it answers -32602), like a value of null
      const now = r.ok && isObject(r.result) ? ((r.result['value'] ?? null) as JsonValue) : null;
      if (canonical(now) !== canonical(x.value)) rows.push({ path: x.path, expected: x.value, now });
    }
    return rows;
  }

  /** What running `calls` (a step's undo or redo) on the session would lose, in words, or null. */
  private async lossOf(scope: string, step: Step, calls: readonly ActionCall[]): Promise<string | null> {
    const status = await this.dispatcher.dispatchCall({ action: 'run.status', params: { session: scope } }, { origin: 'undo' });
    const facts: SessionFacts = status.ok ? statusFacts(status.result) : {};
    for (const c of calls) {
      if (c.action === 'session.close') {
        // undoing session.create: the edits made here were undone before it (they wait on the redo side)
        const words = NO_INVERSE['session.close'](c.params as JsonObject, { ...facts, edits: [] });
        return step.action === 'session.create' ? `${words} Redo makes a new session the same way (${stepTitle(step.forward)}), then offers the undone changes again.` : words;
      }
      const words = lossOf(c.action, c.params as JsonObject, facts);
      if (words) return words;
    }
    return null;
  }

  /** Moves a scope to a new session id, its steps' session parameters with it. */
  private follow(from: string, to: string): void {
    const st = this.scopes.get(from);
    if (!st) return;
    this.scopes.delete(from);
    const map = (c: ActionCall): ActionCall => (c.params['session'] === from ? { action: c.action, params: { ...c.params, session: to } } : c);
    for (const s of [...st.undo, ...st.redo]) {
      s.forward = map(s.forward);
      if (s.backward) s.backward = s.backward.map(map);
    }
    const existing = this.scopes.get(to);
    this.scopes.set(to, existing ? { undo: [...existing.undo, ...st.undo], redo: st.redo } : st);
  }
}
