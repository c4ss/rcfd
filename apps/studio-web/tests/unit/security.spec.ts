// Unit tests of the shell's security policy (src/main/security.ts), without Electron.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  APP_ENTRY,
  CSP_HEADER,
  CSP_META,
  contentType,
  isAppUrl,
  isBlockedRequest,
  resolveAppFile,
} from '../../src/main/security.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

/** The policy of a <meta http-equiv="Content-Security-Policy"> in an HTML file. */
function metaPolicy(file: string): string | undefined {
  const html = readFileSync(file, 'utf8');
  return /<meta\s+http-equiv="Content-Security-Policy"\s+content="([^"]*)"/i.exec(html)?.[1];
}

const directives = (policy: string): Map<string, string> =>
  new Map(
    policy
      .split(';')
      .map((d) => d.trim())
      .filter(Boolean)
      .map((d) => {
        const [k, ...v] = d.split(/\s+/);
        return [k ?? '', v.join(' ')] as const;
      }),
  );

test.describe('Content Security Policy', () => {
  test('is strict: nothing by default, own origin only, no inline or eval, no frames, forms or base', () => {
    const d = directives(CSP_HEADER);
    expect(d.get('default-src')).toBe("'none'");
    for (const k of ['script-src', 'style-src', 'img-src', 'font-src', 'connect-src']) expect(d.get(k)).toBe("'self'");
    for (const k of ['object-src', 'base-uri', 'form-action', 'frame-ancestors']) expect(d.get(k)).toBe("'none'");
    expect(CSP_HEADER).not.toMatch(/unsafe-inline|unsafe-eval|unsafe-hashes|wasm-unsafe-eval|data:|blob:|https?:|\*/);
  });

  test('the meta policy is the header policy without frame-ancestors', () => {
    const header = directives(CSP_HEADER);
    header.delete('frame-ancestors');
    expect(directives(CSP_META)).toEqual(header);
  });

  test('src/renderer/index.html and the built dist/renderer/index.html carry the meta policy', () => {
    expect(metaPolicy(path.join(root, 'src', 'renderer', 'index.html'))).toBe(CSP_META);
    expect(metaPolicy(path.join(root, 'dist', 'renderer', 'index.html'))).toBe(CSP_META);
  });

  test('the built page has no inline script or style and no remote URL', () => {
    const html = readFileSync(path.join(root, 'dist', 'renderer', 'index.html'), 'utf8');
    expect(html).not.toMatch(/<script(?![^>]*\bsrc=)[^>]*>/i);
    expect(html).not.toMatch(/<style[\s>]/i);
    expect(html).not.toMatch(/\sstyle="/i);
    expect(html).not.toMatch(/(src|href)="(https?:)?\/\//i);
  });
});

test.describe('own pages and the network rule', () => {
  test('isAppUrl accepts app://studio/ only', () => {
    expect(isAppUrl(APP_ENTRY)).toBe(true);
    expect(isAppUrl('app://studio/assets/index.js')).toBe(true);
    for (const url of [
      'app://other/index.html',
      'https://studio/index.html',
      'http://localhost/',
      'file:///C:/Windows/win.ini',
      'data:text/html,hi',
      'javascript:alert(1)',
      'about:blank',
      'not a url',
    ]) {
      expect(isAppUrl(url), url).toBe(false);
    }
  });

  test('isBlockedRequest cancels network, file and foreign app:// requests', () => {
    for (const url of [
      'https://example.com/',
      'http://127.0.0.1:8080/',
      'ws://localhost/',
      'wss://example.com/',
      'ftp://example.com/',
      'file:///etc/passwd',
      'app://other/index.html',
      'not a url',
    ]) {
      expect(isBlockedRequest(url), url).toBe(true);
    }
    for (const url of [APP_ENTRY, 'app://studio/assets/x.woff2', 'devtools://devtools/bundled/inspector.html']) {
      expect(isBlockedRequest(url), url).toBe(false);
    }
  });
});

test.describe('the app:// protocol handler', () => {
  const base = path.resolve('/srv/renderer');

  test('maps app://studio/ paths into the renderer directory', () => {
    expect(resolveAppFile(base, APP_ENTRY)).toBe(path.join(base, 'index.html'));
    expect(resolveAppFile(base, 'app://studio/')).toBe(path.join(base, 'index.html'));
    expect(resolveAppFile(base, 'app://studio/assets/a%20b.js')).toBe(path.join(base, 'assets', 'a b.js'));
    // The URL parser removes dot segments (plain or percent-encoded) before the handler sees the path.
    expect(resolveAppFile(base, 'app://studio/../../package.json')).toBe(path.join(base, 'package.json'));
    expect(resolveAppFile(base, 'app://studio/%2e%2e/%2e%2e/package.json')).toBe(path.join(base, 'package.json'));
  });

  test('refuses foreign hosts and every way out of the directory', () => {
    for (const url of [
      'app://other/index.html',
      'https://studio/index.html',
      'app://studio/..%2F..%2Fpackage.json',
      'app://studio/..%5C..%5Cpackage.json',
      'app://studio/assets/%00.js',
      'app://studio/%E0%A4%A',
    ]) {
      expect(resolveAppFile(base, url), url).toBeNull();
    }
  });

  test('gives the content types the page needs', () => {
    expect(contentType('a/index.html')).toBe('text/html; charset=utf-8');
    expect(contentType('a/x.js')).toBe('text/javascript; charset=utf-8');
    expect(contentType('a/x.css')).toBe('text/css; charset=utf-8');
    expect(contentType('a/x.woff2')).toBe('font/woff2');
    expect(contentType('a/x.unknown')).toBe('application/octet-stream');
  });
});
