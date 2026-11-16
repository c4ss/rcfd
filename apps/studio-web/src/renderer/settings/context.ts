// What every part of the settings surface shares: the session's settings tree, the edit state and the operations, all
// of which reach the engine through the action layer (../actions) in SettingsSurface.tsx.
import { createContext, useContext } from 'react';
import type { JsonValue } from '../../shared/actions/params.js';

export interface SettingsContext {
  readonly session: string;
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
  /** Sends settings.set; resolves true when the engine took the value. */
  change(path: string, value: JsonValue): Promise<boolean>;
  clearError(path: string): void;
  /** Makes a named entry of a collection (settings.set of its first key at its default). */
  addEntry(collectionPath: string, name: string): Promise<boolean>;
  /** settings.remove of a named entry. */
  removeEntry(path: string): Promise<boolean>;
  /** A name to put in a collection's new-entry field (from a problem of the check). */
  readonly prefill: ReadonlyMap<string, string>;
  /** Shows a key wherever it sits: its section, every disclosure on its way, then focus on its control. */
  goTo(path: string): void;
}

export const Settings = createContext<SettingsContext | null>(null);

export function useSettings(): SettingsContext {
  const c = useContext(Settings);
  if (!c) throw new Error('useSettings outside the settings surface');
  return c;
}
