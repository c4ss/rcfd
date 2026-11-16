// The security policy of the Studio shell (docs/studio-guardrails.md G10), as plain values and pure functions so that
// the unit tests can check them without starting Electron. src/main/index.ts applies them.
//
// The renderer is served from a custom, privileged scheme (app://studio/...) instead of file://: the protocol handler
// can only reach the built renderer directory, every response carries the Content Security Policy header, and "is this
// one of our own pages" is a comparison of origin, not of file paths.
import path from 'node:path';

export const APP_SCHEME = 'app';
export const APP_HOST = 'studio';
export const APP_ORIGIN = `${APP_SCHEME}://${APP_HOST}`;
export const APP_ENTRY = `${APP_ORIGIN}/index.html`;

/**
 * The Content Security Policy directives. Everything is refused unless named: scripts, styles, images, fonts and
 * fetches only from the app's own origin; no inline script or style element, no eval, no plugins, no frames, no forms,
 * no <base>. A later item that needs more (a worker, a blob: texture, WebAssembly) widens one directive with its own
 * justification and a test.
 */
const DIRECTIVES: ReadonlyArray<readonly [string, string]> = [
  ['default-src', "'none'"],
  ['script-src', "'self'"],
  ['style-src', "'self'"],
  ['img-src', "'self'"],
  ['font-src', "'self'"],
  ['connect-src', "'self'"],
  ['object-src', "'none'"],
  ['base-uri', "'none'"],
  ['form-action', "'none'"],
];

/** Only valid in a header: browsers ignore (and warn about) frame-ancestors in a <meta> element. */
const HEADER_ONLY: ReadonlyArray<readonly [string, string]> = [['frame-ancestors', "'none'"]];

const join = (d: ReadonlyArray<readonly [string, string]>): string => d.map(([k, v]) => `${k} ${v}`).join('; ');

/** The policy sent as the Content-Security-Policy header of every app:// response. */
export const CSP_HEADER = join([...DIRECTIVES, ...HEADER_ONLY]);

/** The same policy without the header-only directives: the <meta http-equiv> of src/renderer/index.html. */
export const CSP_META = join(DIRECTIVES);

/** True when `url` is a page or file of the app itself (app://studio/...). Anything unparsable is not. */
export function isAppUrl(url: string): boolean {
  try {
    const u = new URL(url);
    return u.protocol === `${APP_SCHEME}:` && u.host === APP_HOST;
  } catch {
    return false;
  }
}

/** Schemes that reach the network or the local file system. Requests to them are cancelled (no remote content). */
const BLOCKED_SCHEMES = new Set(['http:', 'https:', 'ws:', 'wss:', 'ftp:', 'file:']);

/** The network-layer rule: cancel requests to the network or to file://, and to any app:// host but our own. */
export function isBlockedRequest(url: string): boolean {
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return true;
  }
  if (u.protocol === `${APP_SCHEME}:`) return u.host !== APP_HOST;
  return BLOCKED_SCHEMES.has(u.protocol);
}

/**
 * The file that an app:// URL names inside `root` (the built renderer directory), or null when the URL is not one of
 * ours or its path, once decoded, would leave `root`.
 */
export function resolveAppFile(root: string, url: string): string | null {
  if (!isAppUrl(url)) return null;
  let rel: string;
  try {
    rel = decodeURIComponent(new URL(url).pathname);
  } catch {
    return null;
  }
  if (rel.includes('\0')) return null;
  if (rel === '/' || rel === '') rel = '/index.html';
  const base = path.resolve(root);
  const file = path.resolve(base, `.${rel}`);
  const inside = path.relative(base, file);
  if (inside === '' || inside.startsWith('..') || path.isAbsolute(inside)) return null;
  return file;
}

const TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.wasm': 'application/wasm',
};

/** The Content-Type of a file of the built renderer, by its extension. */
export function contentType(file: string): string {
  return TYPES[path.extname(file).toLowerCase()] ?? 'application/octet-stream';
}
