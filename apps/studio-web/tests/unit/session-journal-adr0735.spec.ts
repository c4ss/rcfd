// Session journals on disk (F-828 E-828c, ADR 0735) from the Studio's side, without a window: the folder the Studio
// starts `rcfd serve` with (`--journal-dir <state dir>/journals`, src/main/service/spawn.ts journalDirFor), a service so
// started keeping a session's journal file there, and a session number's provenance (src/shared/provenance.ts) naming
// the session's case file and the `rcfd run --journal` command when journal.get gives them. The Studio writes no
// journal file itself (D-L1).
//
// The test with the real rcfd needs the binary (tests/support/rcfd.ts says where it is looked for); without it that
// test is skipped with that reason.
import { expect, test } from '@playwright/test';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { journalDirFor, startService } from '../../src/main/service/spawn.js';
import {
  SESSION_FILE_NOT_RECORDED,
  SESSION_NO_REPRODUCE,
  sessionFacts,
  sessionJournal,
  sessionReproduce,
  type SessionNumberSource,
} from '../../src/shared/provenance.js';
import { boxToml, findRcfd, NO_RCFD } from '../support/rcfd.js';

test('the journal folder: <state dir>/journals, left out when a sandbox root does not hold it', () => {
  const state = path.resolve('/studio/state');
  expect(journalDirFor(state, null)).toBe(path.join(state, 'journals'));
  expect(journalDirFor(state, path.resolve('/studio'))).toBe(path.join(state, 'journals'));
  expect(journalDirFor(state, state)).toBe(path.join(state, 'journals'));
  expect(journalDirFor(state, path.resolve('/cases'))).toBeNull();
  expect(journalDirFor(state, path.join(state, 'journals', 'deeper'))).toBeNull();
});

const src: SessionNumberSource = {
  quantity: 'solver.alpha_u',
  shown: '0.7',
  exact: null,
  unit: { kind: 'none', why: 'the case schema names no unit for solver.alpha_u' },
  session: 's2',
  at: { none: 'a setting has no iteration' },
  readBy: 'settings.get (rcfd service)',
};

test('a session number: the case file and the reproduce command when journal.get gives them', () => {
  const file = path.join('/state', 'journals', 's2-20261117T080910.journal.json');
  const j = sessionJournal({
    format: 'rcfd-journal',
    version: 1,
    entries: [{ op: 'create' }, { op: 'set' }],
    case_hash: '00ff',
    source: { kind: 'path', path: '/p/cases/cavity.toml', hash: 'fnv64:0123456789abcdef' },
    file,
  });
  expect(j).toEqual({ entries: 2, caseHash: '00ff', source: { kind: 'path', path: '/p/cases/cavity.toml', hash: 'fnv64:0123456789abcdef' }, file });
  const rows = new Map(sessionFacts(src, null, j));
  expect(rows.get('Case file')).toBe('/p/cases/cavity.toml, hash fnv64:0123456789abcdef (journal.get)');
  expect(rows.get('Journal file')).toBe(`${file} (the service keeps it)`);
  expect(sessionReproduce(j)).toBe(`rcfd run --journal ${file.includes('\\') ? `'${file}'` : file}`);
  // a session made from text, on a service that keeps no journal file
  const text = sessionJournal({ entries: [{}], case_hash: 'aa', source: { kind: 'toml', path: null, hash: null }, file: null });
  expect(new Map(sessionFacts(src, null, text)).get('Case file')).toBe('none: the session was made from case text (journal.get)');
  expect(new Map(sessionFacts(src, null, text)).has('Journal file')).toBe(false);
  expect(sessionReproduce(text)).toBeNull();
  // an engine before ADR 0735: as before
  const old = sessionJournal({ entries: [{}], case_hash: 'aa' });
  expect(old).toEqual({ entries: 1, caseHash: 'aa' });
  expect(new Map(sessionFacts(src, null, old)).get('Case file')).toBe(SESSION_FILE_NOT_RECORDED);
  expect(sessionReproduce(old)).toBeNull();
  expect(sessionReproduce(null)).toBeNull();
  expect(sessionJournal({ entries: 'x' })).toEqual({ error: 'journal.get answered without entries or case_hash' });
  expect(SESSION_NO_REPRODUCE).toMatch(/^Not recorded: /);
});

const rcfd = findRcfd();

test('a service the Studio starts keeps each session’s journal in <state dir>/journals', async () => {
  test.skip(rcfd === null, NO_RCFD);
  const state = mkdtempSync(path.join(os.tmpdir(), 'rcfd-adr0735-'));
  const s = await startService(rcfd!, null, state);
  try {
    const c = await RpcConnection.connectTcp(s.host, s.port);
    const info = (await c.request('service.info')) as { journal_dir?: string; methods: string[] };
    const { session } = (await c.request('session.create', { toml: boxToml([3, 3, 3]) })) as { session: string };
    await c.request('settings.set', { session, path: 'solver.max_iterations', value: 7 });
    const reply = await c.request('journal.get', { session });
    c.close();
    const dir = path.join(state, 'journals');
    expect(info.journal_dir).toBe(dir);
    expect(info.methods.at(-1)).toBe('journal.save');
    const names = readdirSync(dir);
    expect(names).toHaveLength(1);
    expect(names[0]).toMatch(new RegExp(`^${session}-\\d{8}T\\d{6}\\.journal\\.json$`));
    const file = path.join(dir, names[0]!);
    expect((reply as { file: string }).file).toBe(file);
    expect(JSON.parse(readFileSync(file, 'utf8'))).toEqual(reply);
    expect(sessionReproduce(sessionJournal(reply))).toContain(names[0]!);
  } finally {
    s.child.kill();
    await new Promise((r) => (s.child.exitCode !== null ? r(null) : s.child.once('exit', r)));
    if (existsSync(state)) rmSync(state, { recursive: true, force: true });
  }
});
