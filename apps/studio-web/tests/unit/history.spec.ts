// Undo, redo, losses and conflicts (F-829, guardrail G18) against the fake service through the main process's client
// and port bridge (tests/support/action-transport.ts), as the renderer's dispatcher reaches it:
// - every edit through the action layer (settings.set, settings.remove, a named entry made from its first key, a new
//   session) undoes and redoes, logged with origins undo and redo;
// - an action without an inverse names what it loses before it runs (the loss table covers every change action);
// - a value another writer changed since the edit is a conflict with both values, and nothing is overwritten until the
//   user chooses;
// - a run, its results or a case leave the project only through the engine's trash (ADR 0736: trash_run, trash_results,
//   trash_case, restored by restore_trash, removed for good only by empty_trash); the test fails when the engine gains
//   another destructive entry point, so the Studio offers it through the engine too (tests/unit/trash.spec.ts);
// - the recent list keeps another window's entries; history.css keeps DESIGN.md's rules.
// The same history against the real `rcfd serve`: tests/e2e/undo-rcfd.spec.ts.
import { expect, test } from '@playwright/test';
import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { ACTIONS, Dispatcher, History, lossOf, NO_INVERSE, isNoInverseAction, canonical, type ActionCall, type RecordOptions } from '../../src/shared/actions/index.js';
import { CASE_TOOLS } from '../../src/shared/case.js';
import { PROJECT_TOOLS, TRASH_WRITE_TOOLS } from '../../src/shared/project.js';
import { ProjectHome } from '../../src/main/project/home.js';
import type { StoredRecent } from '../../src/main/project/recent.js';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import { FakeCaseService } from '../support/fake-case-service.js';
import { portTransport, type PortTransport } from '../support/action-transport.js';
import { appRoot } from '../support/rcfd.js';

interface Rig {
  fake: FakeCaseService;
  t: PortTransport;
  d: Dispatcher;
  h: History;
  /** An edit from the UI: dispatched, then recorded, as src/renderer/actions/index.ts does. */
  ui(action: string, params: Record<string, unknown>, undo?: RecordOptions): Promise<unknown>;
  get(session: string, p: string): unknown;
  close(): Promise<void>;
}

async function rig(): Promise<Rig> {
  const fake = await FakeCaseService.start();
  const t = await portTransport(fake.port);
  const d = new Dispatcher(t.transport);
  const h = new History(d);
  return {
    fake,
    t,
    d,
    h,
    ui: async (action, params, undo) => {
      const r = await d.dispatchCall({ action, params });
      h.record(r, undo);
      return r.ok ? r.result : r.error;
    },
    get: (session, p) => {
      let v: unknown = fake.sessions.get(session)?.tree;
      for (const k of p.split('.')) v = typeof v === 'object' && v !== null ? (v as Record<string, unknown>)[k] : undefined;
      return v;
    },
    close: async () => {
      await t.close();
      await fake.stop();
    },
  };
}

test.describe('undo and redo of every edit through the action layer', () => {
  test('settings.set: undo restores the old value, redo sets it again; both are logged and the titles name the step', async () => {
    const r = await rig();
    try {
      const { session } = (await r.ui('session.create', {})) as { session: string };
      await r.ui('settings.set', { session, path: 'solver.alpha_u', value: 0.5 });
      expect(r.h.view(session)).toMatchObject({ undo: { title: 'set solver.alpha_u', available: true }, redo: null, undoDepth: 2 });
      expect(await r.h.undo(session)).toEqual({ kind: 'done', direction: 'undo', title: 'set solver.alpha_u', scope: session });
      expect(r.get(session, 'solver.alpha_u')).toBe(0.9);
      expect(r.h.view(session)).toMatchObject({ undo: { title: `new session ${session}` }, redo: { title: 'set solver.alpha_u' } });
      expect(await r.h.redo(session)).toMatchObject({ kind: 'done', direction: 'redo' });
      expect(r.get(session, 'solver.alpha_u')).toBe(0.5);
      // undo twice more round trip: the inverse a redo computes is the engine's again
      expect(await r.h.undo(session)).toMatchObject({ kind: 'done' });
      expect(r.get(session, 'solver.alpha_u')).toBe(0.9);
      const origins = r.d.log.entries().filter((e) => e.effect === 'change').map((e) => `${e.origin} ${e.action} ${String(e.params['value'] ?? '')}`);
      expect(origins).toEqual(['ui session.create ', 'ui settings.set 0.5', 'undo settings.set 0.9', 'redo settings.set 0.5', 'undo settings.set 0.9']);
      expect(r.fake.sets()).toEqual([
        ['solver.alpha_u', 0.5],
        ['solver.alpha_u', 0.9],
        ['solver.alpha_u', 0.5],
        ['solver.alpha_u', 0.9],
      ]);
    } finally {
      await r.close();
    }
  });

  test('settings.remove: undo makes the entry again with every value it had; redo removes it again', async () => {
    const r = await rig();
    try {
      const session = r.fake.makeSession({ 'boundaries.lid': { kind: 'inlet', velocity: [1, 0, 0] } });
      await r.ui('settings.remove', { session, path: 'boundaries.lid' });
      expect(r.get(session, 'boundaries.lid')).toBeUndefined();
      expect(r.h.view(session).undo?.title).toBe('remove boundaries.lid');
      expect(await r.h.undo(session)).toMatchObject({ kind: 'done' });
      expect(r.get(session, 'boundaries.lid')).toEqual({ kind: 'inlet', velocity: [1, 0, 0] });
      expect(await r.h.redo(session)).toMatchObject({ kind: 'done' });
      expect(r.get(session, 'boundaries.lid')).toBeUndefined();
    } finally {
      await r.close();
    }
  });

  test('a named entry made from its first key: undo removes the entry the surface made, redo makes it again', async () => {
    const r = await rig();
    try {
      const session = r.fake.makeSession();
      await r.ui('settings.set', { session, path: 'boundaries.walls.kind', value: 'wall' }, {
        inverse: [{ action: 'settings.remove', params: { session, path: 'boundaries.walls' } }],
        title: 'add boundaries.walls',
      });
      expect(r.h.view(session).undo?.title).toBe('add boundaries.walls');
      expect(await r.h.undo(session)).toMatchObject({ kind: 'done' });
      expect(r.get(session, 'boundaries.walls')).toBeUndefined();
      expect(await r.h.redo(session)).toMatchObject({ kind: 'done' });
      expect(r.get(session, 'boundaries.walls')).toEqual({ kind: 'wall' });
      // the surface's inverse is kept across a redo
      expect(await r.h.undo(session)).toMatchObject({ kind: 'done' });
      expect(r.get(session, 'boundaries.walls')).toBeUndefined();
    } finally {
      await r.close();
    }
  });

  test('a new edit clears what could be redone; a closed session takes its history; a restarted service clears all', async () => {
    const r = await rig();
    try {
      const session = r.fake.makeSession();
      await r.ui('settings.set', { session, path: 'solver.alpha_u', value: 0.5 });
      await r.h.undo(session);
      expect(r.h.view(session).redoDepth).toBe(1);
      await r.ui('settings.set', { session, path: 'solver.max_iterations', value: 9 });
      expect(r.h.view(session)).toMatchObject({ redoDepth: 0, undo: { title: 'set solver.max_iterations' } });
      // reads, refusals, replays and the undo's own calls are not steps
      await r.ui('settings.get', { session, path: 'solver.alpha_u' });
      r.fake.refusals.set('solver.dt', 'no');
      await r.ui('settings.set', { session, path: 'solver.dt', value: -1 });
      expect(r.h.view(session).undoDepth).toBe(1);
      const other = r.fake.makeSession();
      await r.ui('settings.set', { session: other, path: 'title', value: 'b' });
      expect(r.h.view(other).undoDepth).toBe(1);
      await r.ui('session.close', { session: other });
      expect(r.h.view(other).undoDepth).toBe(0);
      expect(r.h.view(session).undoDepth).toBe(1);
      r.h.clear();
      expect(r.h.view(session)).toMatchObject({ undo: null, redo: null });
      expect(await r.h.undo(session)).toEqual({ kind: 'nothing', direction: 'undo' });
    } finally {
      await r.close();
    }
  });
});

test.describe('an action without an inverse asks first and names what is lost', () => {
  test('every change action has an inverse or words for what it loses', () => {
    const change = ACTIONS.filter((a) => a.effect === 'change').map((a) => a.id);
    // mesh.refine {apply: true} (ADR 0737) is undone by a settings.set of each count back to its `before` (its reply's edits)
    const invertible = ['session.create', 'settings.set', 'settings.remove', 'mesh.refine'];
    expect(change.filter((id) => !invertible.includes(id) && !isNoInverseAction(id))).toEqual([]);
    expect(Object.keys(NO_INVERSE).sort()).toEqual(change.filter((id) => !invertible.includes(id)).sort());
    // session.close names the session and everything only it holds
    const closeWords = NO_INVERSE['session.close']({ session: 's3' }, { state: 'finished', iteration: 340, edits: ['set solver.alpha_u', 'remove boundaries.lid'] });
    expect(closeWords).toBe(
      'Session s3 is removed from the service. Lost: its settings, which exist only in the service (2 changes made here: remove boundaries.lid, set solver.alpha_u); ' +
        'its built mesh and fields; its results (finished, iteration 340). Closing cannot be undone: the session journal (journal.get) can make a new session with the same settings, not the same results.',
    );
    expect(NO_INVERSE['session.close']({ session: 's1' }, { state: 'defined', iteration: 0 })).toBe(
      'Session s1 is removed from the service. Lost: its settings, which exist only in the service. Closing cannot be undone: the session journal (journal.get) can make a new session with the same settings, not the same results.',
    );
    // the run actions lose results only from an ended run
    expect(lossOf('run.start', { session: 's1' }, { state: 'ready', iteration: 0 })).toBeNull();
    expect(lossOf('run.start', { session: 's1' }, { state: 'finished', iteration: 12 })).toBe(
      "Session s1 starts again from its initial state: its results (finished, iteration 12) are replaced by the new run's.",
    );
    expect(lossOf('run.initialize', { session: 's1' }, { state: 'stopped', iteration: 3 })).toContain('are discarded');
    expect(lossOf('run.stop', { session: 's1' }, { state: 'running', iteration: 3 })).toBeNull();
    expect(lossOf('settings.set', { session: 's1', path: 'solver.alpha_u' }, { state: 'finished', iteration: 3 })).toBe(
      'A settings change returns session s1 to defined and discards its built simulation and its results (finished, iteration 3).',
    );
    expect(lossOf('settings.set', { session: 's1', path: 'turbulence.model' }, { state: 'paused', iteration: 3 })).toBeNull();
  });

  test('undoing a new session closes it: confirmed first in words, then closed; redo makes it again and the history follows', async () => {
    const r = await rig();
    try {
      const { session } = (await r.ui('session.create', {})) as { session: string };
      await r.ui('settings.set', { session, path: 'solver.alpha_u', value: 0.5 });
      await r.h.undo(session);
      const ask = await r.h.undo(session);
      expect(ask).toMatchObject({ kind: 'confirm', direction: 'undo', title: `new session ${session}`, scope: session });
      expect(ask.kind === 'confirm' && ask.loss).toBe(
        `Session ${session} is removed from the service. Lost: its settings, which exist only in the service. Closing cannot be undone: ` +
          'the session journal (journal.get) can make a new session with the same settings, not the same results. Redo makes a new session the same way (new session), then offers the undone changes again.',
      );
      expect(r.fake.sessions.has(session)).toBe(true);
      expect(r.fake.received.some((x) => x.method === 'session.close')).toBe(false);
      expect(await r.h.undo(session, { confirmed: true })).toMatchObject({ kind: 'done' });
      expect(r.fake.sessions.has(session)).toBe(false);
      const renamed: string[] = [];
      r.h.subscribe((e) => e.kind === 'renamed' && renamed.push(`${e.from}->${e.to}`));
      const again = await r.h.redo(session);
      expect(again.kind).toBe('done');
      const made = again.kind === 'done' ? again.scope : '';
      expect(made).not.toBe(session);
      expect(renamed).toEqual([`${session}->${made}`]);
      expect(r.h.view(made)).toMatchObject({ undo: { title: `new session ${made}` }, redo: { title: 'set solver.alpha_u' } });
      // the redo side follows the session to its new id
      expect(await r.h.redo(made)).toMatchObject({ kind: 'done' });
      expect(r.get(made, 'solver.alpha_u')).toBe(0.5);
    } finally {
      await r.close();
    }
  });

  test('undo and redo of a settings change on a session with results ask first: the change discards them', async () => {
    const r = await rig();
    try {
      const session = r.fake.makeSession();
      await r.ui('settings.set', { session, path: 'solver.alpha_u', value: 0.5 });
      r.fake.setState(session, 'finished');
      const ask = await r.h.undo(session);
      expect(ask).toMatchObject({ kind: 'confirm', loss: `A settings change returns session ${session} to defined and discards its built simulation and its results (finished, iteration 0).` });
      expect(r.get(session, 'solver.alpha_u')).toBe(0.5);
      expect(await r.h.undo(session, { confirmed: true })).toMatchObject({ kind: 'done' });
      expect(r.get(session, 'solver.alpha_u')).toBe(0.9);
    } finally {
      await r.close();
    }
  });
});

test.describe('a value another writer changed is never overwritten silently', () => {
  test('undo: a conflict with both values, nothing sent; the user keeps theirs (drop), or undoes anyway (force)', async () => {
    const r = await rig();
    try {
      const session = r.fake.makeSession();
      await r.ui('settings.set', { session, path: 'solver.alpha_u', value: 0.5 });
      await r.ui('settings.set', { session, path: 'solver.max_iterations', value: 9 });
      r.fake.externalSet(session, 'solver.max_iterations', 40);
      const before = r.fake.sets().length;
      expect(await r.h.undo(session)).toEqual({
        kind: 'conflict',
        direction: 'undo',
        title: 'set solver.max_iterations',
        scope: session,
        rows: [{ path: 'solver.max_iterations', expected: 9, now: 40 }],
      });
      expect(r.fake.sets().length).toBe(before);
      expect(r.get(session, 'solver.max_iterations')).toBe(40);
      // keep theirs: the step goes, theirs stays, the next undo is the step before
      r.h.drop(session, 'undo');
      expect(r.h.view(session).undo?.title).toBe('set solver.alpha_u');
      expect(r.get(session, 'solver.max_iterations')).toBe(40);
      // undo anyway
      r.fake.externalSet(session, 'solver.alpha_u', 0.7);
      expect(await r.h.undo(session)).toMatchObject({ kind: 'conflict', rows: [{ path: 'solver.alpha_u', expected: 0.5, now: 0.7 }] });
      expect(await r.h.undo(session, { force: true })).toMatchObject({ kind: 'done' });
      expect(r.get(session, 'solver.alpha_u')).toBe(0.9);
    } finally {
      await r.close();
    }
  });

  test('redo: what undo left is checked the same way; a removed entry made again elsewhere is a conflict', async () => {
    const r = await rig();
    try {
      const session = r.fake.makeSession();
      await r.ui('settings.set', { session, path: 'solver.alpha_u', value: 0.5 });
      await r.h.undo(session);
      r.fake.externalSet(session, 'solver.alpha_u', 0.3);
      expect(await r.h.redo(session)).toMatchObject({ kind: 'conflict', direction: 'redo', rows: [{ path: 'solver.alpha_u', expected: 0.9, now: 0.3 }] });
      expect(r.get(session, 'solver.alpha_u')).toBe(0.3);
      r.h.drop(session, 'redo');
      expect(r.h.view(session).redo).toBeNull();
      // remove, then another writer makes the entry again: undo (which would make it) stops
      r.fake.externalSet(session, 'boundaries.lid', { kind: 'wall', velocity: [] });
      await r.ui('settings.remove', { session, path: 'boundaries.lid' });
      r.fake.externalSet(session, 'boundaries.lid', { kind: 'outlet', velocity: [] });
      const c = await r.h.undo(session);
      expect(c).toMatchObject({ kind: 'conflict', rows: [{ path: 'boundaries.lid', expected: null, now: { kind: 'outlet', velocity: [] } }] });
      expect(r.get(session, 'boundaries.lid')).toEqual({ kind: 'outlet', velocity: [] });
    } finally {
      await r.close();
    }
  });

  test('Refine x2: its undo checks the counts it left and sends them as base_value; another writer’s count is a conflict', async () => {
    const r = await rig();
    try {
      const session = r.fake.makeSession({ 'mesh.nx': 8, 'mesh.ny': 4 });
      await r.ui('mesh.refine', { session, factor: 2, apply: true });
      expect([r.get(session, 'mesh.nx'), r.get(session, 'mesh.ny')]).toEqual([16, 8]);
      // a plain round trip: the undo's writes carry the refined counts as their base
      expect(await r.h.undo(session)).toMatchObject({ kind: 'done', direction: 'undo' });
      expect([r.get(session, 'mesh.nx'), r.get(session, 'mesh.ny')]).toEqual([8, 4]);
      const bases = r.fake.received.filter((x) => x.method === 'settings.set').map((x) => [x.params['path'], x.params['value'], x.params['base_value']]);
      expect(bases).toEqual([
        ['mesh.nx', 8, 16],
        ['mesh.ny', 4, 8],
      ]);
      expect(await r.h.redo(session)).toMatchObject({ kind: 'done', direction: 'redo' });
      expect([r.get(session, 'mesh.nx'), r.get(session, 'mesh.ny')]).toEqual([16, 8]);
      // another writer sets a count after the refine: the undo stops with both values, nothing written
      r.fake.externalSet(session, 'mesh.ny', 12);
      const sets = r.fake.sets().length;
      expect(await r.h.undo(session)).toMatchObject({ kind: 'conflict', direction: 'undo', scope: session, rows: [{ path: 'mesh.ny', expected: 8, now: 12 }] });
      expect(r.fake.sets().length).toBe(sets);
      expect([r.get(session, 'mesh.nx'), r.get(session, 'mesh.ny')]).toEqual([16, 12]);
      // undo anyway: no base, both counts back
      expect(await r.h.undo(session, { force: true })).toMatchObject({ kind: 'done' });
      expect([r.get(session, 'mesh.nx'), r.get(session, 'mesh.ny')]).toEqual([8, 4]);
      expect(r.fake.received.filter((x) => x.method === 'settings.set').slice(-2).map((x) => 'base_value' in x.params)).toEqual([false, false]);
      // a refine that only asked (no apply) leaves nothing to check and is no step
      const s2 = r.fake.makeSession({ 'mesh.nx': 8 });
      await r.ui('mesh.refine', { session: s2, factor: 2 });
      expect(r.get(s2, 'mesh.nx')).toBe(8);
    } finally {
      await r.close();
    }
  });

  test('Refine x2: a count another writer sets between the history’s read and its write is the engine’s 1007, a conflict', async () => {
    const fake = await FakeCaseService.start();
    const t = await portTransport(fake.port);
    try {
      const d = new Dispatcher(t.transport);
      const session = fake.makeSession({ 'mesh.nx': 8, 'mesh.ny': 4 });
      let race = false;
      const h = new History({
        dispatchCall: async (c, o) => {
          const r = await d.dispatchCall(c, o);
          if (race && c.action === 'settings.get' && (c.params as Record<string, unknown>)['path'] === 'mesh.ny') {
            race = false;
            fake.externalSet(session, 'mesh.nx', 20);
          }
          return r;
        },
      });
      h.record(await d.dispatchCall({ action: 'mesh.refine', params: { session, factor: 2, apply: true } }));
      race = true;
      expect(await h.undo(session)).toMatchObject({ kind: 'conflict', direction: 'undo', rows: [{ path: 'mesh.nx', expected: 16, now: 20 }] });
      expect(fake.sessions.get(session)?.tree['mesh']).toMatchObject({ nx: 20, ny: 8 });
    } finally {
      await t.close();
      await fake.stop();
    }
  });

  test('a session that is gone fails in words and keeps the step; values compare as JSON, keys in any order', async () => {
    const r = await rig();
    try {
      const session = r.fake.makeSession();
      await r.ui('settings.set', { session, path: 'solver.alpha_u', value: 0.5 });
      r.fake.sessions.delete(session);
      expect(await r.h.undo(session)).toEqual({ kind: 'failed', direction: 'undo', title: 'set solver.alpha_u', message: `Session ${session} is no longer on the service.`, done: 0 });
      expect(r.h.view(session).undoDepth).toBe(1);
      expect(canonical({ b: [1, { d: 2, c: 3 }], a: null })).toBe(canonical({ a: undefined, b: [1, { c: 3, d: 2 }] }));
    } finally {
      await r.close();
    }
  });
});

test.describe('deletes of runs, results and cases', () => {
  test('the engine deletes a run, a result or a case only through its trash (ADR 0736), and the Studio reaches each of those tools', () => {
    const inv = JSON.parse(readFileSync(path.join(appRoot, 'capabilities', 'inventory.json'), 'utf8')) as { capabilities: { id: string }[] };
    const destructive = inv.capabilities
      .map((c) => c.id)
      .filter((id) => !id.startsWith('case:') && /(delete|remove|trash|purge|prune|discard|clean|archive|rm)/i.test(id));
    // settings.remove removes a named entry of a session (undoable, compare-and-set since ADR 0736) and workflow.delete a
    // task of a session workflow: neither touches a file. The trash tools and their CLI twins (ADR 0736) move to the
    // project trash and back (Undo, Restore) and empty it behind the decision panel. When this list grows, the engine has
    // another delete: offer it (recoverable or confirmed in words).
    expect(destructive.filter((id) => !id.startsWith('cli:rcfd project ')).sort()).toEqual([
      'mcp:empty_trash',
      'mcp:list_trash',
      'mcp:restore_trash',
      'mcp:trash_case',
      'mcp:trash_results',
      'mcp:trash_run',
      'service:settings.remove',
      'service:workflow.delete',
    ]);
    expect([...TRASH_WRITE_TOOLS, ...PROJECT_TOOLS.filter((t) => t === 'list_trash')].sort()).toEqual(
      destructive.filter((id) => id.startsWith('mcp:')).map((id) => id.slice(4)).sort(),
    );
    // the case port's write tools never replace or remove a file (new_case refuses a taken name; edit_case is hash guarded)
    expect([...CASE_TOOLS].sort()).toEqual(['check_case_headless', 'edit_case', 'new_case', 'read_project_file']);
  });

  test('no Studio code removes a file or folder of a project', async () => {
    const files: string[] = [];
    const walk = async (d: string) => {
      for (const e of await readdir(d, { withFileTypes: true })) {
        const p = path.join(d, e.name);
        if (e.isDirectory()) await walk(p);
        else if (/\.(ts|tsx|cts|mts)$/.test(e.name)) files.push(p);
      }
    };
    await walk(path.join(appRoot, 'src'));
    const uses: string[] = [];
    for (const f of files) {
      const code = (await readFile(f, 'utf8')).replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`\\])\/\/.*$/gm, '$1');
      for (const m of code.matchAll(/\b(rm|rmSync|rmdir|rmdirSync|unlink|unlinkSync|rename|renameSync)\s*\(/g)) uses.push(`${path.relative(appRoot, f).split(path.sep).join('/')}: ${m[1]}`);
    }
    // the only removals and renames: the Studio's own record of a service it started (user data), the recent list's
    // write-then-rename (user data), and the Claude Code panel's write-then-rename of its MCP config and its record of
    // confirmed control files (F-814, user data; trust.ts refuses a store inside the project), and the theme
    // setting's write-then-rename (F-818, user data); none of those is in a project folder. The one rename in a project
    // folder: the Claude Code allow-list file's write-then-rename (F-814, "Allow this from now on", the maintainer's
    // word of 2026-11-17), only over a file unchanged since it was read. The field viewport's derive (ADR 0738) removes
    // its own mkdtemp folder under the OS temp directory, never a project folder. The run sites' remembered choice per
    // project is a write-then-rename in user data too (F-848, site-choices.json beside the recent list)
    expect(uses.sort()).toEqual([
      'src/main/claude/allow.ts: rename',
      'src/main/claude/panel.ts: rename',
      'src/main/claude/trust.ts: rename',
      'src/main/data/derive.ts: rm',
      'src/main/project/recent.ts: rename',
      'src/main/project/sites.ts: rename',
      'src/main/service/spawn.ts: rm',
      'src/main/theme.ts: rename',
    ]);
  });
});

test.describe('other write paths', () => {
  test('the recent list is read again before each write, so another window’s entries stay', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f829-recent-'));
    try {
      const store = {
        list: [] as StoredRecent[],
        load: async () => store.list,
        save: async (l: readonly StoredRecent[]) => {
          store.list = [...l];
        },
      };
      const home = new ProjectHome({ engine: { configured: false, reason: 'not needed' }, recent: store, pickFolder: async () => null });
      const a = path.join(dir, 'a');
      const b = path.join(dir, 'b');
      store.list = [{ path: a, opened: '2026-11-16T10:00:00.000Z' }];
      await home.init();
      // another window adds b after this one read the list
      store.list = [{ path: b, opened: '2026-11-16T11:00:00.000Z' }, ...store.list];
      await home.forget(a);
      expect(store.list).toEqual([{ path: b, opened: '2026-11-16T11:00:00.000Z' }]);
      await home.shutdown();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

test('history.css uses only defined tokens, no colour literals, square corners, and only the popover lift', () => {
  const css = readFileSync(path.join(appRoot, 'src', 'renderer', 'history', 'history.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
  expect(used.length).toBeGreaterThan(30);
  expect(used.filter((u) => !defined.has(u))).toEqual([]);
  expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
  expect([...css.matchAll(/border-radius:\s*([^;]+)/g)].map((m) => m[1]!.trim()).filter((r) => r !== '0' && r !== 'var(--radius-none)')).toEqual([]);
  expect([...css.matchAll(/box-shadow:\s*([^;]+)/g)].map((m) => m[1]!.trim())).toEqual(['var(--shadow-popover)']);
});

// a type check of the shapes the tests build
const _call: ActionCall = { action: 'settings.get', params: {} };
void _call;
