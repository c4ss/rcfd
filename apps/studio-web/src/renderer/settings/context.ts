// What every part of the settings surface shares: the session's settings tree, the edit state and the operations, all
// of which reach the engine through the action layer (../actions) in SettingsSurface.tsx.
import { createContext, useContext } from 'react';
import type { JsonValue } from '../../shared/actions/params.js';

export interface SettingsContext {
  readonly session: string;
  /** The schema carries the engine's UI metadata (F-826): the advanced flag decides what sits behind "Show all". */
  readonly ui?: boolean;
  /** The session's settings (settings.get without a path), or null while it loads. */
  readonly tree: unknown;
  /** False while the action layer says settings.set is unavailable (a run is going); then readOnlyReason says why. */
  readonly editable: boolean;
  readonly readOnlyReason: string | null;
  /** The engine's message of the last refused change, by concrete path (or `add:<collection>` for a new entry). */
  readonly errors: ReadonlyMap<string, string>;
  /** Paths with a change in flight. */
  readonly pending: ReadonlySet<string>;
  /** Whether a disclosure (more keys, a nested object, a collection, an entry) is open. */
  isOpen(id: string): boolean;
  toggle(id: string): void;
  /**
   * Sends settings.set with the value last read at the path as its compare-and-set base (ADR 0736 P2; `base` instead
   * when given: the value now, after the user chose to set theirs over it); resolves true when the engine took the value. A value another writer set since is
   * refused by the engine (1007): the row then shows a conflict (`conflicts`).
   */
  change(path: string, value: JsonValue, base?: JsonValue): Promise<boolean>;
  /** ADR 0736 P2: changes the engine refused because another writer changed the value since it was read, by path. */
  readonly conflicts: ReadonlyMap<string, SettingConflict>;
  /** Keeps the other writer's value: forgets the conflict and sends nothing. */
  keepTheirs(path: string): void;
  clearError(path: string): void;
  /** Makes a named entry of a collection (settings.set of its first key at its default). */
  addEntry(collectionPath: string, name: string): Promise<boolean>;
  /** settings.remove of a named entry, with the entry last read as its base (`base` instead when given). */
  removeEntry(path: string, base?: JsonValue): Promise<boolean>;
  /** A name to put in a collection's new-entry field (from a problem of the check). */
  readonly prefill: ReadonlyMap<string, string>;
  /** Shows a key wherever it sits: its section, every disclosure on its way, then focus on its control. */
  goTo(path: string): void;
  /**
   * The engine's value of a setting now (settings.get), null when the case holds none; `ok: false` when it could not be
   * read. A typed value is checked against it before it is sent, so a value another writer set while the user typed is
   * not overwritten silently (F-829, G18).
   */
  current?(path: string): Promise<{ readonly ok: true; readonly value: JsonValue } | { readonly ok: false }>;
}

/** A change the engine refused with 1007 (ADR 0736 P2): the value now (the other writer's) and the value sent. */
export interface SettingConflict {
  readonly now: JsonValue;
  readonly mine: JsonValue;
  /** settings.remove of a named entry (`mine` is then null). */
  readonly remove: boolean;
}

export const Settings = createContext<SettingsContext | null>(null);

export function useSettings(): SettingsContext {
  const c = useContext(Settings);
  if (!c) throw new Error('useSettings outside the settings surface');
  return c;
}
