// Help from the engine (F-830, ADR 0730): the error catalogue of the real engine (`rcfd help errors`, the same text as
// the service's help.errors) gives the Studio a recovery line for every JSON-RPC code it can show, and the glossary of
// `rcfd help glossary` holds every term the first path names. Needs the rcfd binary; skipped, with the reason, without it.
import { expect, test } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { findRcfd, NO_RCFD } from '../support/rcfd.js';
import { recoveryOf, termOf, type CatalogueEntry, type GlossaryTerm } from '../../src/renderer/help/engine.js';
import { TERMS } from '../../src/renderer/first/words.js';

const rcfd = findRcfd();

function help(what: string): unknown {
  const r = spawnSync(rcfd!, ['help', what], { encoding: 'utf8', timeout: 60_000 });
  expect(r.status, r.stderr).toBe(0);
  return JSON.parse(r.stdout);
}

test('every JSON-RPC code of the catalogue gives its recovery line; exit codes and unknown codes give none', () => {
  test.skip(rcfd === null, NO_RCFD);
  const list = (help('errors') as { errors: CatalogueEntry[] }).errors;
  const rpc = list.filter((e) => ['service', 'workspace', 'mcp'].includes(e.surface));
  expect(rpc.length).toBeGreaterThan(10);
  for (const e of rpc) expect(recoveryOf(list, e.code), `${e.surface} ${e.code}`).toBe(e.recovery);
  expect(recoveryOf(list, 424242)).toBeNull();
  expect(recoveryOf(list, 'changed')).toBeNull();
  expect(recoveryOf(null, -32602)).toBeNull();
  // an exit code is not a JSON-RPC code: rcfd's exit 3 is not looked up as one
  expect(list.some((e) => e.surface === 'cli_exit' && e.code === 3)).toBe(true);
  expect(recoveryOf(list, 3)).toBeNull();
});

test('every precise term of the first path is a term of the engine’s glossary', () => {
  test.skip(rcfd === null, NO_RCFD);
  const terms = (help('glossary') as { terms: GlossaryTerm[] }).terms;
  for (const [step, list] of Object.entries(TERMS)) {
    for (const t of list) {
      const row = termOf(terms, t.term);
      expect(row, `${step}: ${t.term}`).not.toBeNull();
      expect(row!.meaning.length).toBeGreaterThan(t.plain.length);
    }
  }
});
