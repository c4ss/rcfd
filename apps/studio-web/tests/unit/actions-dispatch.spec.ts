// Unit tests of the action dispatcher (F-827, src/shared/actions/dispatcher.ts) against the fake service, through the
// same path the renderer's calls take (the main process's client, the port bridge and its allow-list checks): dispatch,
// the log, refused actions never reaching the service, the JSON and shell exports, and dispatch + record + replay as a
// round trip onto a second service.
import { expect, test } from '@playwright/test';
import {
  ActionLog,
  Dispatcher,
  MAX_LOGGED_READS,
  scriptProblem,
  SCRIPT_FORMAT,
  toShell,
  type ActionScript,
  type LogEntry,
} from '../../src/shared/actions/index.js';
import { portTransport } from '../support/action-transport.js';
import { FakeService } from '../support/fake-service.js';

const fixedClock = () => new Date('2026-11-16T12:00:00.000Z');

/** The requests the fake received from the dispatcher (not the client's own service.info and long polls). */
const sent = (fake: FakeService, from: number) =>
  fake.received
    .slice(from)
    .filter((r) => r.method !== 'events.poll')
    .map((r) => ({ method: r.method, params: r.params }));

async function rig() {
  const fake = await FakeService.start();
  const t = await portTransport(fake.port);
  const from = fake.received.length;
  return { fake, t, from, close: async () => (await t.close(), await fake.stop()) };
}

test.describe('dispatch', () => {
  test('makes the engine call, returns its reply as a value and logs it', async () => {
    const { fake, t, from, close } = await rig();
    try {
      const d = new Dispatcher(t.transport, { now: fixedClock });
      const created = await d.dispatch('session.create', { toml: 'title = "x"' });
      expect(created).toMatchObject({ ok: true, result: { session: 's1' } });
      const got = await d.dispatch('settings.get', { session: 's1', path: 'physics.nu' });
      expect(got).toMatchObject({ ok: true, result: { echo: { session: 's1', path: 'physics.nu' } } });
      const invalid = await d.dispatch('settings.validate', { session: 's1' });
      expect(invalid).toMatchObject({ ok: false, error: { code: 1003, message: 'settings failed validation' } });
      const unknown = await d.dispatch('run.pause', { session: 's1' });
      expect(unknown).toMatchObject({ ok: false, error: { code: -32601 } });
      expect(sent(fake, from)).toEqual([
        { method: 'session.create', params: { toml: 'title = "x"' } },
        { method: 'settings.get', params: { session: 's1', path: 'physics.nu' } },
        { method: 'settings.validate', params: { session: 's1' } },
        { method: 'run.pause', params: { session: 's1' } },
      ]);
      const log = d.log.entries().map(({ ms, ...e }) => (expect(ms).toBeGreaterThanOrEqual(0), e));
      const at = '2026-11-16T12:00:00.000Z';
      expect(log).toEqual([
        { seq: 1, action: 'session.create', params: { toml: 'title = "x"' }, effect: 'change', origin: 'ui', at, sent: true, ok: true, session: 's1', inverse: [{ action: 'session.close', params: { session: 's1' } }] },
        { seq: 2, action: 'settings.get', params: { session: 's1', path: 'physics.nu' }, effect: 'read', origin: 'ui', at, sent: true, ok: true },
        { seq: 3, action: 'settings.validate', params: { session: 's1' }, effect: 'read', origin: 'ui', at, sent: true, ok: false, error: invalid.ok ? null : invalid.error },
        { seq: 4, action: 'run.pause', params: { session: 's1' }, effect: 'change', origin: 'ui', at, sent: true, ok: false, error: { code: -32601, message: "unknown method 'run.pause'" } },
      ]);
    } finally {
      await close();
    }
  });

  test('an unknown action, bad parameters or an unavailable action never reach the service, and are logged as refused', async () => {
    const { fake, t, from, close } = await rig();
    try {
      let status = 'connected';
      const d = new Dispatcher(t.transport, { context: () => ({ service: { status: status as 'connected', lastError: null } }) });
      const r1 = await d.dispatchCall({ action: 'run.wait', params: { session: 's1' } });
      // @ts-expect-error count is a number
      const r2 = await d.dispatch('run.step', { session: 's1', count: 'two' });
      const r3 = await d.dispatch('run.start', { session: 's1' }, { context: { session: { id: 's1', state: 'running' } } });
      status = 'disconnected';
      const r4 = await d.dispatch('service.info', {});
      const r5 = await d.dispatchCall({ action: 'session.create', params: { toml: 'a', path: 'b' } });
      expect([r1, r2, r3, r4, r5].map((r) => (r.ok ? null : r.error))).toEqual([
        { code: 'unknown_action', message: "'run.wait' is not a registered action" },
        { code: 'bad_request', message: 'run.step: count must be a number' },
        { code: 'unavailable', message: 'A run is in progress in this session; stop it first.' },
        { code: 'unavailable', message: 'The rcfd service is not connected (disconnected).' },
        // the parameters are checked first
        { code: 'bad_request', message: 'session.create: give at most one of toml, settings and path (got toml, path)' },
      ]);
      status = 'connected';
      const r6 = await d.dispatchCall({ action: 'settings.get', params: { session: 's1', path: 7 } });
      expect(r6.ok ? null : r6.error).toEqual({ code: 'bad_request', message: 'settings.get: path must be a string' });
      expect(d.log.entries().every((e) => !e.sent && !e.ok)).toBe(true);
      expect(d.log.entries()).toHaveLength(6);
      // a moment for anything that might still be on its way
      await new Promise((r) => setTimeout(r, 100));
      expect(sent(fake, from)).toEqual([]);
    } finally {
      await close();
    }
  });

  test('an action with a before-read (settings.set) reads first, without logging the read', async () => {
    const { fake, t, from, close } = await rig();
    try {
      const d = new Dispatcher(t.transport);
      await d.dispatch('session.create', {});
      // the fake answers settings.set with "unknown method": the read still went first
      const r = await d.dispatch('settings.set', { session: 's1', path: 'physics.nu', value: 0.02 });
      expect(r.ok).toBe(false);
      expect(sent(fake, from).map((x) => x.method)).toEqual(['session.create', 'settings.get', 'settings.set']);
      expect(d.log.entries().map((e) => e.action)).toEqual(['session.create', 'settings.set']);
    } finally {
      await close();
    }
  });

  test('the log keeps every change and a ring of the latest reads; listeners see each entry', () => {
    const log = new ActionLog();
    const seen: number[] = [];
    const off = log.onEntry((e) => seen.push(e.seq));
    const entry = (seq: number, effect: 'read' | 'change'): LogEntry => ({ seq, action: effect === 'read' ? 'run.status' : 'run.start', params: { session: 's1' }, effect, origin: 'ui', at: '', ms: 0, sent: true, ok: true });
    log.add(entry(1, 'change'));
    for (let i = 0; i < MAX_LOGGED_READS + 10; i++) log.add(entry(2 + i, 'read'));
    log.add(entry(MAX_LOGGED_READS + 12, 'change'));
    off();
    log.add(entry(MAX_LOGGED_READS + 13, 'change'));
    const kept = log.entries();
    expect(kept.filter((e) => e.effect === 'change').map((e) => e.seq)).toEqual([1, MAX_LOGGED_READS + 12, MAX_LOGGED_READS + 13]);
    expect(kept.filter((e) => e.effect === 'read')).toHaveLength(MAX_LOGGED_READS);
    expect(log.droppedReads).toBe(10);
    expect(kept.map((e) => e.seq)).toEqual([...kept.map((e) => e.seq)].sort((a, b) => a - b));
    expect(seen).toHaveLength(MAX_LOGGED_READS + 12);
  });
});

test.describe('record and replay', () => {
  test('dispatch, record, export as JSON and replay onto another service: the same requests, sessions mapped', async () => {
    const one = await rig();
    let script: ActionScript;
    let recorded: { method: string; params: unknown }[];
    try {
      const d = new Dispatcher(one.t.transport, { now: fixedClock });
      await d.dispatch('session.create', { toml: 'title = "a"' });
      await d.dispatch('session.create', { settings: { physics: { nu: 0.01 } } });
      await d.dispatch('run.step', { session: 's1', count: 2 });
      await d.dispatch('settings.get', { session: 's2', path: 'physics' });
      await d.dispatch('run.status', { session: 's1' });
      await d.dispatch('run.step', { session: 's2' });
      await d.dispatch('run.pause', { session: 's1' }); // the fake has no run.pause: fails, left out of the script
      recorded = sent(one.fake, one.from);
      script = JSON.parse(JSON.stringify(d.exportScript({ includeReads: true, address: one.t.address, exported: 'then' })));
      expect(script.format).toBe(SCRIPT_FORMAT);
      expect(scriptProblem(script)).toBeNull();
      expect(script.steps.map((s) => [s.action, s.session ?? null])).toEqual([
        ['session.create', 's1'],
        ['session.create', 's2'],
        ['run.step', null],
        ['settings.get', null],
        ['run.status', null],
        ['run.step', null],
      ]);
      expect(script.steps[2]!.cli).toEqual(['rcfd', 'call', '--addr', one.t.address, 'run.step', '{"session":"s1","count":2}']);
      expect(d.exportScript({ address: one.t.address, exported: 'then' }).steps.map((s) => s.action)).toEqual(['session.create', 'session.create', 'run.step', 'run.step']);
    } finally {
      await one.close();
    }

    const two = await rig();
    try {
      // a session that exists before the replay: the replayed ones get other ids (s2, s3)
      await two.t.client.call('session.create', {});
      const from = two.fake.received.length;
      const d = new Dispatcher(two.t.transport);
      const r = await d.replay(script);
      expect(r).toEqual({ ok: true, steps: script.steps.map((s) => ({ action: s.action, ok: true })), sessions: { s1: 's2', s2: 's3' } });
      const map = (v: unknown) => JSON.parse(JSON.stringify(v).replaceAll('"s2"', '"s3"').replaceAll('"s1"', '"s2"'));
      expect(sent(two.fake, from)).toEqual(map(recorded.slice(0, 6)));
      expect(d.log.entries().every((e) => e.origin === 'replay')).toBe(true);
      // the replay's own log exports to the same script, sessions mapped
      const again = d.exportScript({ includeReads: true, address: one.t.address, exported: 'then' });
      expect(again.steps.map(({ action, params }) => ({ action, params }))).toEqual(map(script.steps.map(({ action, params }) => ({ action, params }))));
    } finally {
      await two.close();
    }
  });

  test('a replay stops at the first failing step unless told otherwise; a bad script runs nothing', async () => {
    const { fake, t, from, close } = await rig();
    try {
      const d = new Dispatcher(t.transport);
      const script: ActionScript = {
        format: SCRIPT_FORMAT,
        version: 1,
        exported: 'x',
        address: t.address,
        steps: [
          { action: 'session.create', params: {}, session: 's9', cli: [], mcp: null },
          { action: 'run.resume', params: { session: 's9' }, cli: [], mcp: null },
          { action: 'run.status', params: { session: 's9' }, cli: [], mcp: null },
        ],
      };
      const stopped = await d.replay(script);
      expect(stopped.ok).toBe(false);
      expect(stopped.steps.map((s) => s.ok)).toEqual([true, false]);
      expect(stopped.steps[1]!.error).toEqual({ code: -32601, message: "unknown method 'run.resume'" });
      const all = await d.replay(script, { stopOnError: false });
      expect(all.steps.map((s) => s.ok)).toEqual([true, false, true]);
      expect(all.sessions).toEqual({ s9: 's2' });
      expect(sent(fake, from).map((x) => x.params)).toEqual([{}, { session: 's1' }, {}, { session: 's2' }, { session: 's2' }]);
      for (const [bad, why] of [
        [null, 'a script is a JSON object'],
        [{ ...script, format: 'other' }, `format must be '${SCRIPT_FORMAT}'`],
        [{ ...script, version: 2 }, 'version 2 is not supported (this Studio reads 1)'],
        [{ ...script, steps: [{ action: 'run.wait', params: {} }] }, "step 1: unknown action 'run.wait'"],
        [{ ...script, steps: [{ action: 'run.status', params: [] }] }, 'step 1: params must be an object'],
      ] as const) {
        expect(await d.replay(bad)).toEqual({ ok: false, steps: [], sessions: {}, problem: why });
      }
    } finally {
      await close();
    }
  });
});

test.describe('the shell export', () => {
  test('one `rcfd call` per action, sessions in shell variables, quotes escaped', () => {
    const script: ActionScript = {
      format: SCRIPT_FORMAT,
      version: 1,
      exported: '2026-11-16T12:00:00.000Z',
      address: '127.0.0.1:7878',
      steps: [
        { action: 'session.create', params: { toml: "title = 'it''s'\n[mesh]" }, session: 's4', cli: [], mcp: null },
        { action: 'settings.set', params: { session: 's4', path: 'physics.nu', value: 0.02 }, cli: [], mcp: null },
        { action: 'run.step', params: { count: 2, session: 's4' }, cli: [], mcp: null },
        { action: 'run.status', params: { session: 's0' }, cli: [], mcp: null },
      ],
    };
    expect(toShell(script)).toBe(
      [
        '#!/bin/sh',
        '# rcfd Studio actions (rcfd-studio-actions 1), exported 2026-11-16T12:00:00.000Z.',
        '# Each line is one Studio action as the CLI sends it; run it against `rcfd serve` (set RCFD_ADDR to its address).',
        'set -e',
        'ADDR="${RCFD_ADDR:-127.0.0.1:7878}"',
        '# 1. New session (session.create)',
        `S1=$(rcfd call --addr "$ADDR" session.create '{"toml":"title = '\\''it'\\'''\\''s'\\''\\n[mesh]"}' | sed -n 's/.*"session":"\\([^"]*\\)".*/\\1/p')`,
        'test -n "$S1"',
        '# 2. Change a setting (settings.set)',
        `rcfd call --addr "$ADDR" settings.set '{"session":"'"$S1"'","path":"physics.nu","value":0.02}'`,
        '# 3. Take steps (run.step)',
        `rcfd call --addr "$ADDR" run.step '{"count":2,"session":"'"$S1"'"}'`,
        '# 4. Show run status (run.status)',
        `rcfd call --addr "$ADDR" run.status '{"session":"s0"}'`,
        '',
      ].join('\n'),
    );
  });
});
