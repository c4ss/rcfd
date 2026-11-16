// The case editor's routes (F-812): location hashes, as the rest of the Studio's. No imports, so the project home can
// link here without loading the editor:
//
//   #/case                                the sessions open on case files
//   #/case/new                            a new case from one of the engine's templates or shipped examples
//   #/case/open/<cases/x.toml>[?replaces=<session>]
//                                         open a case file; with `replaces`, that session is closed once the new one
//                                         is open (loading the file again after a conflict or an outside change)
//   #/case/<session>                      the editor on a session bound to a case file

export const CASE_HASH = '#/case';

export type CaseRoute =
  | { readonly kind: 'index' }
  | { readonly kind: 'new' }
  | { readonly kind: 'open'; readonly path: string; readonly replaces: string | null }
  | { readonly kind: 'session'; readonly session: string };

export const caseHash = {
  index: CASE_HASH,
  new: `${CASE_HASH}/new`,
  open: (path: string, replaces?: string): string => `${CASE_HASH}/open/${encodeURIComponent(path)}${replaces ? `?replaces=${encodeURIComponent(replaces)}` : ''}`,
  session: (session: string): string => `${CASE_HASH}/${encodeURIComponent(session)}`,
};

/** The route of a location hash, or null when the hash is not one of the editor's. */
export function caseRoute(hash: string): CaseRoute | null {
  if (hash === CASE_HASH || hash === `${CASE_HASH}/`) return { kind: 'index' };
  if (!hash.startsWith(`${CASE_HASH}/`)) return null;
  const rest = hash.slice(CASE_HASH.length + 1);
  if (rest === 'new') return { kind: 'new' };
  const open = /^open\/([^?]+)(?:\?replaces=(.+))?$/.exec(rest);
  if (open) return { kind: 'open', path: decodeURIComponent(open[1]!), replaces: open[2] ? decodeURIComponent(open[2]) : null };
  if (/^[^/?]+$/.test(rest)) return { kind: 'session', session: decodeURIComponent(rest) };
  return null;
}

/** Goes to a route without leaving a history step for the one it replaces (an open in progress). */
export function replaceHash(hash: string): void {
  history.replaceState(null, '', hash);
  window.dispatchEvent(new HashChangeEvent('hashchange'));
}
