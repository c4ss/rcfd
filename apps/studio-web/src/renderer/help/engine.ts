// Help from the engine (F-830, ADR 0730; G19): the glossary and the error catalogue, asked of the solver service once per
// service instance (`help.glossary`, `help.errors`, on with `rcfd serve --ui`, which the Studio's spawn passes). The
// Studio keeps no help text of its own for what the engine documents: a term's meaning is the glossary's row, an error's
// way out is the catalogue's recovery line for its code. Without a service, or with one started without --ui, both are
// unavailable and the surfaces say so in place of the text (they never fall back to words of their own).
import { useEffect, useState } from 'react';
import { dispatch, watchServiceState } from '../actions/index.js';

export interface GlossaryTerm {
  readonly term: string;
  readonly meaning: string;
  /** Where the term is used most, as docs/glossary.md writes it (Markdown). */
  readonly page: string;
}

export interface CatalogueEntry {
  /** `service`, `workspace`, `mcp`, `cli_exit`, `runner_exit`, `mcp_exit` or `mcp_hook_exit`. */
  readonly surface: string;
  readonly code: number;
  readonly name: string;
  readonly meaning: string;
  readonly recovery: string;
}

export type Loaded<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly reason: string };

/** The JSON-RPC surfaces: one number space shared by the service, the project tools and the MCP server. */
const RPC_SURFACES = new Set(['service', 'workspace', 'mcp']);

let instance = -1;
let glossary: Promise<Loaded<readonly GlossaryTerm[]>> | null = null;
let errors: Promise<Loaded<readonly CatalogueEntry[]>> | null = null;

function reset(next: number) {
  if (next === instance) return;
  instance = next;
  glossary = null;
  errors = null;
}

const asString = (v: unknown) => (typeof v === 'string' ? v : '');

/** The glossary rows (`help.glossary`), once per service instance. */
export function loadGlossary(): Promise<Loaded<readonly GlossaryTerm[]>> {
  glossary ??= dispatch('help.glossary', {}, { undo: false }).then((r) => {
    if (!r.ok) {
      glossary = null;
      return { ok: false, reason: r.error.message };
    }
    const terms = (r.result as { terms?: unknown }).terms;
    const value = Array.isArray(terms)
      ? terms.map((t: { term?: unknown; meaning?: unknown; page?: unknown }) => ({ term: asString(t.term), meaning: asString(t.meaning), page: asString(t.page) }))
      : [];
    return { ok: true, value };
  });
  return glossary;
}

/** The error catalogue (`help.errors`), once per service instance. */
export function loadErrors(): Promise<Loaded<readonly CatalogueEntry[]>> {
  errors ??= dispatch('help.errors', {}, { undo: false }).then((r) => {
    if (!r.ok) {
      errors = null;
      return { ok: false, reason: r.error.message };
    }
    const list = (r.result as { errors?: unknown }).errors;
    const value = Array.isArray(list)
      ? list.map((e: Record<string, unknown>) => ({
          surface: asString(e['surface']),
          code: typeof e['code'] === 'number' ? e['code'] : NaN,
          name: asString(e['name']),
          meaning: asString(e['meaning']),
          recovery: asString(e['recovery']),
        }))
      : [];
    return { ok: true, value };
  });
  return errors;
}

/** The catalogue's recovery line of a JSON-RPC error code, or null (not a number, not in the catalogue, not loaded). */
export function recoveryOf(list: readonly CatalogueEntry[] | null, code: unknown): string | null {
  if (list === null || typeof code !== 'number') return null;
  return list.find((e) => RPC_SURFACES.has(e.surface) && e.code === code)?.recovery ?? null;
}

/** A glossary row by its term (exact, as docs/glossary.md names it). */
export function termOf(list: readonly GlossaryTerm[] | null, term: string): GlossaryTerm | null {
  return list?.find((t) => t.term === term) ?? null;
}

function useLoaded<T>(load: () => Promise<Loaded<T>>): Loaded<T> | null {
  const [v, setV] = useState<Loaded<T> | null>(null);
  const [seen, setSeen] = useState(-1);
  useEffect(
    () =>
      watchServiceState((s) => {
        if (s.status !== 'connected') {
          // nothing to ask: say so, and keep what an earlier connection gave
          setV((old) => old ?? { ok: false, reason: s.mode === 'off' ? 'the rcfd service is not running' : `the rcfd service is ${s.status}` });
          return;
        }
        reset(s.instance);
        setSeen(s.instance);
      }),
    [],
  );
  useEffect(() => {
    if (seen < 0) return;
    let live = true;
    void load().then((r) => {
      if (live) setV(r);
    });
    return () => {
      live = false;
    };
  }, [seen, load]);
  return v;
}

/** The error catalogue for a surface: null until the service answered. */
export function useErrorCatalogue(): readonly CatalogueEntry[] | null {
  const v = useLoaded(loadErrors);
  return v?.ok ? v.value : null;
}

/** The glossary for a surface: null until the service answered, or why it is unavailable. */
export function useGlossary(): Loaded<readonly GlossaryTerm[]> | null {
  return useLoaded(loadGlossary);
}
