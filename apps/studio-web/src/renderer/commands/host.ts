// Where the command palette finds the commands a surface runs itself (F-831). A command that needs a surface's own state
// (going to Runs needs the open project's view, saving needs the case editor's session) is run by that surface: while
// it is mounted it registers a handler for the command's id, with what makes it unavailable now and why (G19). The
// palette asks `commandHandler(id)`; with none registered the command is unavailable, and the palette says what to
// open first.
//
//   useCommandHandlers({
//     'case.save': { unavailable: () => (canSave ? null : reason), run: () => save() },
//   });
//
// Surfaces also publish small facts the palette uses to offer a choice first, never to fill one in (the run selected
// in Runs): `useCommandFact('selectedRun', run)`.
import { useEffect, useRef } from 'react';

export interface CommandHandler {
  /** Why it cannot run now, with the way out; null when it can. */
  unavailable?(): string | null;
  /** Runs it with the values the palette asked for. */
  run(args: Readonly<Record<string, unknown>>): void | Promise<unknown>;
}

/** Handlers by command id; the latest registered wins (a surface mounted over another). */
const handlers = new Map<string, CommandHandler[]>();
const waiting = new Map<string, ((h: CommandHandler) => void)[]>();
const facts = new Map<string, unknown>();
const requests = new Set<(id: string) => void>();

/** Asks the palette to start a command, as its shortcut would (a button that opens the palette). */
export function requestCommand(id: string): void {
  for (const l of requests) l(id);
}

/** Follows `requestCommand`; returns the function that stops following (the palette's host). */
export function onCommandRequest(listener: (id: string) => void): () => void {
  requests.add(listener);
  return () => requests.delete(listener);
}

export function registerCommand(id: string, h: CommandHandler): () => void {
  const list = handlers.get(id) ?? [];
  list.push(h);
  handlers.set(id, list);
  const w = waiting.get(id);
  if (w) {
    waiting.delete(id);
    for (const f of w) f(h);
  }
  return () => {
    const l = handlers.get(id);
    if (!l) return;
    const i = l.lastIndexOf(h);
    if (i >= 0) l.splice(i, 1);
    if (l.length === 0) handlers.delete(id);
  };
}

/** The handler a mounted surface registered for `id`, or null. */
export function commandHandler(id: string): CommandHandler | null {
  const l = handlers.get(id);
  return l && l.length > 0 ? l[l.length - 1]! : null;
}

/**
 * Resolves with the handler of `id` once a surface registers one (now, when one is), or null after `timeoutMs`: the
 * palette goes to the project home first and then asks Runs to show itself.
 */
export function whenRegistered(id: string, timeoutMs = 5000): Promise<CommandHandler | null> {
  const now = commandHandler(id);
  if (now) return Promise.resolve(now);
  return new Promise((resolve) => {
    let done = false;
    const f = (h: CommandHandler) => {
      if (done) return;
      done = true;
      resolve(h);
    };
    const list = waiting.get(id) ?? [];
    list.push(f);
    waiting.set(id, list);
    setTimeout(() => {
      if (done) return;
      done = true;
      const l = waiting.get(id);
      if (l) {
        const i = l.indexOf(f);
        if (i >= 0) l.splice(i, 1);
        if (l.length === 0) waiting.delete(id);
      }
      resolve(null);
    }, timeoutMs);
  });
}

/** Registers the handlers while the calling component is mounted; each call uses the latest closures. */
export function useCommandHandlers(map: Readonly<Record<string, CommandHandler>>): void {
  const ref = useRef(map);
  ref.current = map;
  const ids = Object.keys(map).sort().join('\n');
  useEffect(() => {
    const offs = ids
      .split('\n')
      .filter(Boolean)
      .map((id) =>
        registerCommand(id, {
          unavailable: () => ref.current[id]?.unavailable?.() ?? null,
          run: (args) => ref.current[id]?.run(args),
        }),
      );
    return () => {
      for (const off of offs) off();
    };
  }, [ids]);
}

/** A fact a surface knows while it is mounted (the run selected in Runs), for the palette to offer first. */
export function commandFact<T>(key: string): T | undefined {
  return facts.get(key) as T | undefined;
}

export function useCommandFact(key: string, value: unknown): void {
  useEffect(() => {
    facts.set(key, value);
    return () => {
      if (facts.get(key) === value) facts.delete(key);
    };
  }, [key, value]);
}
