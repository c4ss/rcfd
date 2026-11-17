// What a save of the case editor writes, from the session's journal (`journal.get`, docs/service-protocol.md): its
// entries are the create entry and every accepted settings change, `{op: "set", path, value}` or `{op: "remove", path}`,
// from any writer of the session (the Studio, Claude Code, `rcfd call`). Pure, so the unit tests cover it.
//
// The engine's edit of a case file (`edit_case`) sets settings by path and cannot remove one, so:
// - a setting set since the last save is written with the value the session holds now (`settings.get`);
// - a named entry removed since then drops the settings set under it before the removal (they are gone);
// - a removal of an entry the file does not hold (added and removed in this session) writes nothing; one of an entry the
//   file holds cannot be saved in place (`cannot`), and the editor says so.

export interface JournalEntry {
  readonly op: string;
  readonly path?: string;
  readonly source?: { readonly path?: unknown; readonly hash?: unknown } | null;
}

export interface Pending {
  readonly count: number;
  /** Setting paths to write, in the order they were first set (after the latest removal of an entry above them). */
  readonly paths: readonly string[];
  /** Entries removed since the save, as removed. */
  readonly removals: readonly string[];
}

const under = (p: string, entry: string): boolean => p === entry || p.startsWith(`${entry}.`);

/** The journal's entries, checked for shape: a list of objects with a string `op`. */
export function journalEntries(reply: unknown): JournalEntry[] {
  const entries = (reply as { entries?: unknown } | null)?.entries;
  if (!Array.isArray(entries)) throw new Error('journal.get: the reply has no list of entries');
  return entries.map((e, i) => {
    if (typeof e !== 'object' || e === null || typeof (e as { op?: unknown }).op !== 'string') {
      throw new Error(`journal.get: entry ${i} has no op`);
    }
    return e as JournalEntry;
  });
}

/** The changes after the first `from` entries of the journal. */
export function pendingChanges(entries: readonly JournalEntry[], from: number): Pending {
  const paths: string[] = [];
  const removals: string[] = [];
  let count = 0;
  for (const e of entries.slice(from)) {
    const p = typeof e.path === 'string' ? e.path : null;
    if (p === null) continue;
    if (e.op === 'set') {
      count++;
      if (!paths.includes(p)) paths.push(p);
    } else if (e.op === 'remove') {
      count++;
      for (let i = paths.length - 1; i >= 0; i--) if (under(paths[i]!, p)) paths.splice(i, 1);
      if (!removals.includes(p)) removals.push(p);
    }
  }
  return { count, paths, removals };
}

/**
 * The removals a save cannot write: those of entries the file holds (`inFile` answers from a session made from the
 * file). An entry the file holds stays in it whatever is set under it again, because edit_case only sets.
 */
export async function realRemovals(removals: readonly string[], inFile: (path: string) => Promise<boolean>): Promise<string[]> {
  const out: string[] = [];
  for (const r of removals) if (await inFile(r)) out.push(r);
  return out;
}
