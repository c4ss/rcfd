// Unit tests of the action registry (F-827, src/shared/actions/): every action has an engine equivalent (G16), the
// allow-list has no method without an action, the parameter schemas and their types, the availability rules, and the
// CLI and MCP equivalents. The engine side is read from the engine's own sources (the method list of service.info, the
// CLI's `call` command, the MCP server's tools); tests/e2e/actions-rcfd.spec.ts checks the method list against the real
// `rcfd serve` as well.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import {
  ACTIONS,
  actionParamsProblem,
  availability,
  describeAction,
  equivalents,
  getAction,
  methodsWithoutAction,
  paramsProblem,
  toJsonSchema,
  type ActionParams,
  type RegisteredAction,
} from '../../src/shared/actions/index.js';
import { SERVICE_CONTROL_METHODS, SERVICE_METHODS, SERVICE_READ_METHODS } from '../../src/shared/service.js';
import { appRoot } from '../support/rcfd.js';

const repo = path.resolve(appRoot, '..', '..');
const rust = (rel: string) => readFileSync(path.join(repo, rel), 'utf8');

/** The strings of a `const NAME: ... = [ ... ];` list in a Rust source. */
function rustList(src: string, name: string): string[] {
  const m = new RegExp(`const ${name}: [^=]*= &?\\[([\\s\\S]*?)\\];`).exec(src);
  if (!m) throw new Error(`no const ${name}`);
  return [...m[1]!.matchAll(/"([^"]+)"/g)].map((x) => x[1]!);
}

/** What the engine offers, from its sources. */
function engine() {
  const methods = new Set([...rustList(rust('crates/rcfd-api/src/service.rs'), 'METHODS'), ...rustList(rust('crates/rcfd-api/src/data_service.rs'), 'DATA_METHODS')]);
  const cli = rust('crates/rcfd-cli/src/main.rs');
  const hasCall = cli.includes('"call" => cmd_call(rest)') && /\n {4}call \[--addr <host:port>\] <method> \[<json-params>\]/.test(cli);
  const mcp = rust('crates/rcfd-mcp/src/lib.rs');
  const dispatchAt = mcp.indexOf('fn dispatch(');
  /** The service methods the MCP tool's arm calls, or null when the server has no such tool. */
  const mcpCalls = (tool: string): string[] | null => {
    if (!mcp.includes(`name: "${tool}",`)) return null;
    const at = mcp.indexOf(`\n            "${tool}" =>`, dispatchAt);
    if (at < 0) return null;
    const rest = mcp.slice(at + 1);
    const next = rest.slice(1).search(/\n {12}"[a-z_]+" =>/);
    const arm = next < 0 ? rest : rest.slice(0, next + 1);
    return [...arm.matchAll(/svc\.call\("([^"]+)"/g)].map((x) => x[1]!);
  };
  return { methods, hasCall, mcpCalls };
}

/** Why an action has no engine equivalent (G16), or an empty list. */
function engineProblems(a: RegisteredAction, e: ReturnType<typeof engine>): string[] {
  const out: string[] = [];
  if (!(SERVICE_METHODS as readonly string[]).includes(a.engine.service)) out.push(`${a.id}: ${a.engine.service} is not on the service allow-list`);
  if (!e.methods.has(a.engine.service)) out.push(`${a.id}: the service has no method ${a.engine.service}`);
  if (!e.hasCall) out.push(`${a.id}: the CLI has no \`rcfd call\``);
  if (a.mcp.tool !== null) {
    const calls = e.mcpCalls(a.mcp.tool);
    if (calls === null) out.push(`${a.id}: the MCP server has no tool ${a.mcp.tool}`);
    else if (!calls.includes(a.engine.service)) out.push(`${a.id}: MCP tool ${a.mcp.tool} does not call ${a.engine.service} (it calls ${calls.join(', ')})`);
  } else if (!a.mcp.note) out.push(`${a.id}: no MCP tool and no note saying so`);
  return out;
}

test.describe('the registry', () => {
  test('every action has an engine equivalent: an allow-listed service method, `rcfd call`, and the MCP tool it names (G16)', () => {
    const e = engine();
    expect(e.methods.size).toBeGreaterThan(40);
    expect(ACTIONS.flatMap((a) => engineProblems(a, e))).toEqual([]);
  });

  test('the G16 check fails on an action without an engine equivalent', () => {
    const e = engine();
    const base = getAction('run.start')!;
    const noMethod = { ...base, id: 'x.nothing', engine: { service: 'run.wait' } } as unknown as RegisteredAction;
    const wrongTool = { ...base, id: 'x.tool', mcp: { tool: 'check_mesh' } } as RegisteredAction;
    const noTool = { ...base, id: 'x.mcp', mcp: { tool: 'no_such_tool' } } as RegisteredAction;
    expect(engineProblems(noMethod, e)).toEqual(['x.nothing: run.wait is not on the service allow-list']);
    expect(engineProblems(wrongTool, e)).toEqual(['x.tool: MCP tool check_mesh does not call run.start (it calls mesh.check)']);
    expect(engineProblems(noTool, e)).toEqual(['x.mcp: the MCP server has no tool no_such_tool']);
    expect(engineProblems({ ...base, id: 'x.gone', engine: { service: 'no.such' } } as unknown as RegisteredAction, { ...e, methods: new Set() })).toHaveLength(3);
  });

  test('every method of the allow-list is called by an action, and each action is the one entry of its id', () => {
    expect(methodsWithoutAction()).toEqual([]);
    expect(new Set(ACTIONS.map((a) => a.id)).size).toBe(ACTIONS.length);
    expect(ACTIONS.map((a) => a.engine.service).sort()).toEqual([...SERVICE_METHODS].sort());
  });

  test('ids, titles and descriptions are there for the palette and help (G19, G20); the effect follows the method', () => {
    for (const a of ACTIONS) {
      expect(a.id, a.id).toMatch(/^[a-z]+(\.[a-z_]+)+$/);
      expect(a.title.length, a.id).toBeGreaterThan(3);
      expect(a.title, a.id).toMatch(/^[A-Z]/);
      expect(a.title.endsWith('.'), a.id).toBe(false);
      expect(a.description.length, a.id).toBeGreaterThan(20);
      expect(a.description.endsWith('.'), a.id).toBe(true);
      for (const [k, s] of Object.entries(a.params)) expect(s.description.length, `${a.id}.${k}`).toBeGreaterThan(5);
      expect(a.effect, a.id).toBe((SERVICE_CONTROL_METHODS as readonly string[]).includes(a.engine.service) ? 'change' : 'read');
      if (a.effect === 'read') expect((SERVICE_READ_METHODS as readonly string[]).includes(a.engine.service)).toBe(true);
      const d = describeAction(a);
      expect(JSON.parse(JSON.stringify(d))).toEqual(d);
    }
    expect(new Set(ACTIONS.map((a) => a.title)).size).toBe(ACTIONS.length);
  });

  test('the actions that F-829 can undo, and how', () => {
    expect(ACTIONS.filter((a) => a.undo).map((a) => a.id)).toEqual(['session.create', 'session.close', 'settings.set', 'settings.remove']);
    const create = getAction('session.create')!;
    expect(create.undo!.inverse({} as never, { session: 's7' }, null)).toEqual([{ action: 'session.close', params: { session: 's7' } }]);
    const set = getAction('settings.set')!;
    const p = { session: 's1', path: 'physics.nu', value: 0.02 };
    expect(set.undo!.before!(p as never)).toEqual({ action: 'settings.get', params: { session: 's1', path: 'physics.nu' } });
    expect(set.undo!.inverse(p as never, {}, { ok: true, result: { value: 0.01 } })).toEqual([{ action: 'settings.set', params: { session: 's1', path: 'physics.nu', value: 0.01 } }]);
    const remove = getAction('settings.remove')!;
    const entry = { velocity: 'fixed', velocity_value: [1, 0, 0], profile: { kind: 'parabolic', peak: 2 }, extra: {} };
    expect(remove.undo!.inverse({ session: 's1', path: 'boundaries.lid' } as never, {}, { ok: true, result: { value: entry } })).toEqual(
      (
        [
          ['boundaries.lid.velocity', 'fixed'],
          ['boundaries.lid.velocity_value', [1, 0, 0]],
          ['boundaries.lid.profile.kind', 'parabolic'],
          ['boundaries.lid.profile.peak', 2],
          ['boundaries.lid.extra', {}],
        ] as const
      ).map(([path, value]) => ({ action: 'settings.set', params: { session: 's1', path, value } })),
    );
    expect(set.undo!.inverse(p as never, {}, { ok: false, error: { code: -32602, message: 'no such setting' } })).toBeNull();
    expect(getAction('session.close')!.undo!.inverse({} as never, {}, null)).toBeNull();
  });
});

test.describe('parameters', () => {
  test('the schema check: unknown, missing, and wrong kinds', () => {
    const step = getAction('run.step')!;
    expect(actionParamsProblem(step, { session: 's1' })).toBeNull();
    expect(actionParamsProblem(step, { session: 's1', count: 3 })).toBeNull();
    expect(actionParamsProblem(step, {})).toBe("missing parameter 'session': The session (run.status, session.list).");
    expect(actionParamsProblem(step, { session: '' })).toBe('session must be a session id (a non-empty string)');
    expect(actionParamsProblem(step, { session: 's1', count: 0 })).toBe('count must be at least 1');
    expect(actionParamsProblem(step, { session: 's1', count: 1.5 })).toBe('count must be an integer');
    expect(actionParamsProblem(step, { session: 's1', count: '2' })).toBe('count must be a number');
    expect(actionParamsProblem(step, { session: 's1', steps: 2 })).toBe("unknown parameter 'steps' (expected session, count)");
    expect(actionParamsProblem(getAction('service.info')!, { x: 1 })).toBe("unknown parameter 'x' (this action takes none)");
    expect(actionParamsProblem(step, [])).toBe('the parameters must be an object');
    const set = getAction('settings.set')!;
    expect(actionParamsProblem(set, { session: 's1', path: 'a', value: { b: [1, null, 'x'] } })).toBeNull();
    expect(actionParamsProblem(set, { session: 's1', path: 'a', value: Number.NaN })).toBe('value is not a finite number');
    expect(actionParamsProblem(set, { session: 's1', path: 'a', value: new Date() })).toBe('value is not a JSON value');
    expect(actionParamsProblem(getAction('monitors.get')!, { session: 's', names: ['a', 2] })).toBe('names must be a list of strings');
  });

  test("session.create's own check: at most one source, base_dir only with text", () => {
    const c = getAction('session.create')!;
    expect(actionParamsProblem(c, {})).toBeNull();
    expect(actionParamsProblem(c, { toml: 'x', base_dir: 'cases' })).toBeNull();
    expect(actionParamsProblem(c, { toml: 'x', path: 'a.toml' })).toBe('give at most one of toml, settings and path (got toml, path)');
    expect(actionParamsProblem(c, { path: 'a.toml', base_dir: 'b' })).toBe('base_dir goes with toml or settings; a case file resolves against its own folder');
    expect(actionParamsProblem(c, { settings: 'x' })).toBe('settings must be an object');
  });

  test('the JSON Schema of the palette and the inventory', () => {
    expect(toJsonSchema(getAction('run.step')!.params)).toEqual({
      type: 'object',
      properties: {
        session: { description: 'The session (run.status, session.list).', type: 'string', minLength: 1, 'x-rcfd-kind': 'session' },
        count: { description: 'How many steps (default 1; the service caps it).', type: 'integer', minimum: 1 },
      },
      required: ['session'],
      additionalProperties: false,
    });
    expect(toJsonSchema({})).toEqual({ type: 'object', properties: {}, required: [], additionalProperties: false });
    expect(paramsProblem({ k: { type: 'string', enum: ['a', 'b'], description: 'one of two' } }, { k: 'c' })).toBe('k must be one of a, b');
  });

  test('the parameter types follow the schema (checked by tsc)', () => {
    const ok: ActionParams<'run.step'> = { session: 's1', count: 2 };
    const minimal: ActionParams<'run.step'> = { session: 's1' };
    // @ts-expect-error count is a number
    const badCount: ActionParams<'run.step'> = { session: 's1', count: '2' };
    // @ts-expect-error session is required
    const noSession: ActionParams<'run.step'> = { count: 2 };
    // @ts-expect-error no such parameter
    const extra: ActionParams<'run.pause'> = { session: 's1', count: 2 };
    const set: ActionParams<'settings.set'> = { session: 's1', path: 'a', value: { b: [1, 'x', null] } };
    expect([ok, minimal, badCount, noSession, extra, set]).toHaveLength(6);
  });
});

test.describe('availability', () => {
  const connected = { status: 'connected', lastError: null } as const;
  const at = (state: string | undefined) => ({ service: connected, session: { id: 's1', ...(state ? { state } : {}) } });
  const av = (id: string, ctx: Parameters<typeof availability>[1], params: Record<string, unknown> = { session: 's1' }) => availability(getAction(id)!, ctx, params);

  test('no service or no connection: nothing is available, and the reason says how to get one (G19)', () => {
    for (const a of ACTIONS) {
      expect(availability(a, { service: { status: 'off', lastError: null } }, { session: 's1' })).toEqual({
        available: false,
        reason: 'No rcfd service is configured: start the Studio with RCFD_STUDIO_SERVICE (attach) or RCFD_STUDIO_RCFD (start one).',
      });
    }
    expect(av('run.status', { service: { status: 'disconnected', lastError: 'connection refused' } })).toEqual({
      available: false,
      reason: 'The rcfd service is not connected (disconnected: connection refused).',
    });
  });

  test('a session action without a session', () => {
    expect(av('run.start', { service: connected }, {})).toEqual({ available: false, reason: 'No session is open; create or choose a session first.' });
    expect(av('run.start', { service: connected }, { session: 's1' })).toEqual({ available: true });
    expect(av('session.list', { service: connected }, {})).toEqual({ available: true });
    expect(av('session.create', {}, {})).toEqual({ available: true });
  });

  test('run states: unavailable only where the service refuses or nothing would happen; unknown state is available (G2)', () => {
    const table: Record<string, Record<string, boolean>> = {
      'run.start': { defined: true, ready: true, running: false, paused: false, finished: true, stopped: true, failed: true },
      'run.step': { defined: true, ready: true, running: false, paused: false, finished: true },
      'run.initialize': { defined: true, running: false, paused: false, failed: true },
      'run.pause': { defined: false, ready: false, running: true, paused: false },
      'run.resume': { ready: false, running: false, paused: true },
      'run.stop': { defined: false, ready: false, running: true, paused: true, finished: false },
      'settings.set': { defined: true, ready: true, running: false, paused: true },
      'settings.remove': { defined: true, running: false, paused: false },
      'monitors.get': { defined: false, ready: true, running: true, finished: true },
      'field.stats': { defined: false, ready: true },
      'reports.get': { defined: false, paused: true },
      'run.status': { defined: true, running: true, failed: true },
    };
    for (const [id, states] of Object.entries(table)) {
      for (const [state, want] of Object.entries(states)) expect(av(id, at(state)).available, `${id} in ${state}`).toBe(want);
      expect(av(id, at(undefined)).available, `${id} in an unknown state`).toBe(true);
    }
    expect(av('run.start', at('running'))).toEqual({ available: false, reason: 'A run is in progress in this session; stop it first.' });
    expect(av('monitors.get', at('defined'))).toEqual({ available: false, reason: 'The session has no built simulation yet; initialize or start the run first.' });
    // the context's state is its own session's: another session's state is unknown
    expect(av('run.start', at('running'), { session: 's2' })).toEqual({ available: true });
    expect(av('run.start', at('running'), {})).toEqual({ available: false, reason: 'A run is in progress in this session; stop it first.' });
  });
});

test.describe('equivalents (G16, G17)', () => {
  test('the CLI command is `rcfd call` with the same method and parameters', () => {
    const a = getAction('settings.set')!;
    const p = { session: 's1', path: 'physics.nu', value: 0.02 };
    expect(equivalents(a, p)).toEqual({
      service: { method: 'settings.set', params: p },
      cli: ['rcfd', 'call', '--addr', '127.0.0.1:7878', 'settings.set', '{"session":"s1","path":"physics.nu","value":0.02}'],
      mcp: { tool: 'set_setting', arguments: p },
      mcpNote: null,
    });
    expect(equivalents(getAction('run.pause')!, { session: 's2' }, '127.0.0.1:5000')).toEqual({
      service: { method: 'run.pause', params: { session: 's2' } },
      cli: ['rcfd', 'call', '--addr', '127.0.0.1:5000', 'run.pause', '{"session":"s2"}'],
      mcp: null,
      mcpNote: 'No session tool; in project mode pause_run pauses a project run.',
    });
  });

  test('MCP arguments where the tool differs', () => {
    expect(equivalents(getAction('run.start')!, { session: 's1', max_steps: 5 }).mcp).toEqual({ tool: 'run', arguments: { session: 's1', max_steps: 5, wait_seconds: 0 } });
    expect(equivalents(getAction('session.create')!, { toml: 'x' }).mcp).toEqual({ tool: 'create_case', arguments: { toml: 'x' } });
    const withBase = equivalents(getAction('session.create')!, { toml: 'x', base_dir: 'cases' });
    expect(withBase.mcp).toBeNull();
    expect(withBase.mcpNote).toContain('create_case cannot express these parameters');
  });
});
