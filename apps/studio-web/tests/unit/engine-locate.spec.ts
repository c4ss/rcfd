// ADR 0801 Addendum 21, "Engine found without setup": where the Studio finds its engine (src/main/engine/locate.ts),
// the switches a shortcut can carry (src/main/project/argv.ts), and what the project server (src/main/project/config.ts)
// and the service (src/main/service/config.ts) make of it. Pure functions with fake file checks; no Electron.
//
// The order: --no-engine > --engine-dir > RCFD_STUDIO_MCP / RCFD_STUDIO_RCFD > the default folder (<resources>/bin when
// packaged, the checkout's target/release with `electron .`). Attaching: --service > RCFD_STUDIO_SERVICE. Never PATH.
import { expect, test } from '@playwright/test';
import path from 'node:path';
import {
  buildCommand,
  checkoutRelease,
  chooseEngine,
  defaultEngine,
  exe,
  missingParts,
  missingReason,
  type DefaultEngine,
} from '../../src/main/engine/locate.js';
import { engineSwitches, projectArgument } from '../../src/main/project/argv.js';
import { engineConfig } from '../../src/main/project/config.js';
import { serviceConfig } from '../../src/main/service/config.js';

const abs = (p: string) => path.resolve(p);
const APP = abs('/repo/apps/studio-web');
const CHECKOUT = path.join(abs('/repo'), 'target', 'release');
const SHIPPED = path.join(abs('/app/resources'), 'bin');
const ELSEWHERE = abs('/opt/rcfd');

/** A file check that knows the whole engine in each of these folders (and the files listed one by one). */
function files(dirs: readonly string[], extra: readonly string[] = []) {
  const set = new Set(extra);
  for (const d of dirs) for (const n of ['rcfd', 'rcfd-mcp', 'rcfd-runner']) set.add(path.join(d, exe(n)));
  return (p: string) => set.has(p);
}
const NO_SWITCH = { engineDir: null, noEngine: false };
const checkout: DefaultEngine = { dir: CHECKOUT, kind: 'checkout' };
const bundled: DefaultEngine = { dir: SHIPPED, kind: 'bundled' };
const STATE = abs('/state');

test.describe('the default folder', () => {
  test('electron . from the checkout: <repo>/target/release beside the app folder, or $CARGO_TARGET_DIR/release', () => {
    expect(checkoutRelease({}, APP)).toBe(CHECKOUT);
    expect(checkoutRelease({ CARGO_TARGET_DIR: abs('/t') }, APP)).toBe(path.join(abs('/t'), 'release'));
    // a blank variable is not set
    expect(checkoutRelease({ CARGO_TARGET_DIR: '  ' }, APP)).toBe(CHECKOUT);
    expect(defaultEngine({}, false, abs('/electron/resources'), APP)).toEqual({ dir: CHECKOUT, kind: 'checkout' });
  });
  test('packaged (installed, or out/packaged): <resources>/bin, whatever CARGO_TARGET_DIR says', () => {
    expect(defaultEngine({ CARGO_TARGET_DIR: abs('/t') }, true, abs('/app/resources'), abs('/app/resources/app'))).toEqual({ dir: SHIPPED, kind: 'bundled' });
  });
});

test.describe('which engine', () => {
  test('nothing set: the default folder, complete', () => {
    const c = chooseEngine({}, NO_SWITCH, checkout, files([CHECKOUT]));
    expect(c).toEqual({ from: 'checkout', dir: CHECKOUT, missing: [] });
    expect(engineConfig({}, files([CHECKOUT]), c)).toEqual({ configured: true, path: path.join(CHECKOUT, exe('rcfd-mcp')), from: 'checkout' });
    expect(serviceConfig({}, STATE, c)).toEqual({ mode: 'spawn', rcfd: path.join(CHECKOUT, exe('rcfd')), root: null, stateDir: STATE, start: 'project' });
    const b = chooseEngine({}, NO_SWITCH, bundled, files([SHIPPED]));
    expect(engineConfig({}, files([SHIPPED]), b)).toMatchObject({ configured: true, from: 'bundled' });
    expect(serviceConfig({}, STATE, b)).toMatchObject({ mode: 'spawn', rcfd: path.join(SHIPPED, exe('rcfd')), start: 'project' });
  });

  test('the dev checkout without a built part: which file, where, and the one cargo command that builds it', () => {
    const only = files([], [path.join(CHECKOUT, exe('rcfd')), path.join(CHECKOUT, exe('rcfd-runner'))]);
    const c = chooseEngine({}, NO_SWITCH, checkout, only);
    expect(c).toEqual({ from: 'checkout', dir: CHECKOUT, missing: [{ name: 'rcfd-mcp', pkg: 'rcfd-mcp' }] });
    const e = engineConfig({}, only, c);
    expect(e.configured).toBe(false);
    if (!e.configured) {
      expect(e.reason).toBe(
        `The Studio looked for the engine built in this checkout, in ${CHECKOUT}: ${exe('rcfd-mcp')} is not there. Build it with: cargo build --release -p rcfd-mcp`,
      );
    }
    // the service is not started from half an engine, and says the same
    const s = serviceConfig({}, STATE, c);
    expect(s).toEqual({ mode: 'off', reason: e.configured ? '' : e.reason });
    // nothing built: all three, one command
    const none = chooseEngine({}, NO_SWITCH, checkout, files([]));
    expect(missingReason(none)).toContain(`${exe('rcfd-mcp')}, ${exe('rcfd')} and ${exe('rcfd-runner')} are not there`);
    expect(missingReason(none)).toContain('cargo build --release -p rcfd-mcp -p rcfd-cli -p rcfd-runner');
    expect(buildCommand()).toBe('cargo build --release -p rcfd-mcp -p rcfd-cli -p rcfd-runner');
  });

  test('a packaged Studio whose bin folder is incomplete says so, and how to get past it', () => {
    const c = chooseEngine({}, NO_SWITCH, bundled, files([]));
    expect(missingReason(c)).toMatch(/^The engine that comes with the Studio is incomplete: .* not in .*bin\. Install the Studio again, or start it with --engine-dir <folder>\.$/);
  });

  test('--no-engine: no project server and no service the Studio starts; an attach still applies', () => {
    const c = chooseEngine({ RCFD_STUDIO_MCP: abs('/e/rcfd-mcp'), RCFD_STUDIO_RCFD: abs('/e/rcfd') }, { engineDir: ELSEWHERE, noEngine: true }, checkout, files([CHECKOUT, ELSEWHERE]));
    expect(c).toEqual({ from: 'no-engine' });
    expect(engineConfig({}, files([CHECKOUT]), c)).toEqual({ configured: false, reason: 'The Studio was started without an engine (--no-engine).' });
    expect(serviceConfig({ RCFD_STUDIO_RCFD: abs('/e/rcfd') }, STATE, c)).toEqual({ mode: 'off', reason: 'The Studio was started without an engine (--no-engine).' });
    expect(serviceConfig({}, STATE, c, '127.0.0.1:7878')).toEqual({ mode: 'attach', host: '127.0.0.1', port: 7878 });
    expect(serviceConfig({ RCFD_STUDIO_SERVICE: '127.0.0.1:7879' }, STATE, c)).toEqual({ mode: 'attach', host: '127.0.0.1', port: 7879 });
  });

  test('--engine-dir: that folder, over the variables and the default folder; its missing parts are named', () => {
    const has = files([CHECKOUT, ELSEWHERE]);
    const c = chooseEngine({ RCFD_STUDIO_MCP: abs('/e/rcfd-mcp') }, { engineDir: ELSEWHERE, noEngine: false }, checkout, has);
    expect(c).toEqual({ from: '--engine-dir', dir: ELSEWHERE, missing: [] });
    expect(engineConfig({ RCFD_STUDIO_MCP: abs('/e/rcfd-mcp') }, has, c)).toEqual({ configured: true, path: path.join(ELSEWHERE, exe('rcfd-mcp')), from: '--engine-dir' });
    // RCFD_STUDIO_RCFD does not start a service at launch once a switch chose the engine
    expect(serviceConfig({ RCFD_STUDIO_RCFD: abs('/e/rcfd') }, STATE, c)).toEqual({
      mode: 'spawn',
      rcfd: path.join(ELSEWHERE, exe('rcfd')),
      root: null,
      stateDir: STATE,
      start: 'project',
    });
    const half = chooseEngine({}, { engineDir: ELSEWHERE, noEngine: false }, checkout, files([CHECKOUT], [path.join(ELSEWHERE, exe('rcfd-mcp'))]));
    expect(missingParts(ELSEWHERE, files([], [path.join(ELSEWHERE, exe('rcfd-mcp'))])).map((p) => p.name)).toEqual(['rcfd', 'rcfd-runner']);
    expect(engineConfig({}, files([]), half)).toEqual({
      configured: false,
      reason: `--engine-dir names ${ELSEWHERE}, but ${exe('rcfd')} and ${exe('rcfd-runner')} are not there.`,
    });
    expect(serviceConfig({}, STATE, half).mode).toBe('off');
  });

  test('a variable wins over the default folder, and the default folder is not mixed in', () => {
    const has = files([CHECKOUT]);
    for (const env of [{ RCFD_STUDIO_MCP: abs('/e/rcfd-mcp') }, { RCFD_STUDIO_RCFD: abs('/e/rcfd') }]) {
      expect(chooseEngine(env, NO_SWITCH, checkout, has)).toEqual({ from: 'variables' });
    }
    // RCFD_STUDIO_SERVICE alone does not name an engine: the project server is still the default folder's
    expect(chooseEngine({ RCFD_STUDIO_SERVICE: '127.0.0.1:1' }, NO_SWITCH, checkout, has)).toMatchObject({ from: 'checkout' });
  });

  test('RCFD_STUDIO_MCP alone: the rcfd beside it starts with the first project (one folder, the whole engine)', () => {
    const e = abs('/e');
    const env = { RCFD_STUDIO_MCP: path.join(e, exe('rcfd-mcp')) };
    const c = chooseEngine(env, NO_SWITCH, checkout, files([CHECKOUT, e]));
    expect(engineConfig(env, files([e]), c)).toEqual({ configured: true, path: path.join(e, exe('rcfd-mcp')), from: 'RCFD_STUDIO_MCP' });
    expect(serviceConfig(env, STATE, c, null, files([e]))).toEqual({ mode: 'spawn', rcfd: path.join(e, exe('rcfd')), root: null, stateDir: STATE, start: 'project' });
    // no rcfd beside it: no service (the checkout's rcfd is not used), and the reason names the file and the command
    const s = serviceConfig(env, STATE, c, null, files([CHECKOUT], [path.join(e, exe('rcfd-mcp'))]));
    expect(s).toEqual({
      mode: 'off',
      reason: `RCFD_STUDIO_MCP names ${path.join(e, exe('rcfd-mcp'))}, but there is no ${exe('rcfd')} beside it (build it with: cargo build --release -p rcfd-cli).`,
    });
  });

  test('attaching: --service over RCFD_STUDIO_SERVICE, over a started service; loopback only', () => {
    const c = chooseEngine({}, NO_SWITCH, checkout, files([CHECKOUT]));
    expect(serviceConfig({ RCFD_STUDIO_SERVICE: '127.0.0.1:1' }, STATE, c, '[::1]:2')).toEqual({ mode: 'attach', host: '::1', port: 2 });
    expect(serviceConfig({ RCFD_STUDIO_SERVICE: '127.0.0.1:1' }, STATE, c)).toEqual({ mode: 'attach', host: '127.0.0.1', port: 1 });
    // the switch also settles the variables' conflict (a switch wins over a variable)
    expect(serviceConfig({ RCFD_STUDIO_SERVICE: '127.0.0.1:1', RCFD_STUDIO_RCFD: 'x' }, STATE, { from: 'variables' }, '127.0.0.1:3')).toMatchObject({ mode: 'attach', port: 3 });
    expect(serviceConfig({}, STATE, c, '10.0.0.2:7878')).toEqual({ mode: 'invalid', reason: expect.stringMatching(/^--service: .*not a loopback address/) });
  });

  test('never PATH: a PATH that holds the whole engine changes nothing', () => {
    const c = chooseEngine({ PATH: ELSEWHERE }, NO_SWITCH, checkout, files([ELSEWHERE]));
    expect(c).toMatchObject({ from: 'checkout', dir: CHECKOUT });
    expect(engineConfig({ PATH: ELSEWHERE }, files([ELSEWHERE]), c).configured).toBe(false);
  });
});

test.describe('the switches on the command line', () => {
  const cwd = abs('/home/u');
  test('--engine-dir, --no-engine and --service, as two arguments or with =, after the program or the app path', () => {
    expect(engineSwitches(['rcfd-studio.exe', '--engine-dir', 'eng', '--service=127.0.0.1:9'], false, cwd)).toEqual({
      engineDir: path.resolve(cwd, 'eng'),
      noEngine: false,
      service: '127.0.0.1:9',
      keepRunner: false,
      claude: null,
      problems: [],
    });
    expect(engineSwitches(['electron', '.', '--no-engine', '--engine-dir=/x', '--service', 'localhost:7'], true, cwd)).toEqual({
      engineDir: path.resolve(cwd, '/x'),
      noEngine: true,
      service: 'localhost:7',
      keepRunner: false,
      claude: null,
      problems: [],
    });
    // electron . : the app path is not a switch value
    expect(engineSwitches(['electron', '.'], true, cwd)).toEqual({ engineDir: null, noEngine: false, service: null, keepRunner: false, claude: null, problems: [] });
    expect(engineSwitches(['rcfd-studio', '--engine-dir', '--no-engine', '--service='], false, cwd)).toEqual({
      engineDir: null,
      noEngine: true,
      service: null,
      keepRunner: false,
      claude: null,
      problems: ['--engine-dir needs a value', '--service needs a value'],
    });
  });
  test('G7 as amended 2026-11-17: --keep-runner is a switch with no value, and never the project folder', () => {
    expect(engineSwitches(['rcfd-studio.exe', '--keep-runner', 'proj'], false, cwd)).toMatchObject({ keepRunner: true, problems: [] });
    expect(engineSwitches(['electron', '.', '--engine-dir', 'eng', '--keep-runner'], true, cwd)).toMatchObject({ engineDir: path.resolve(cwd, 'eng'), keepRunner: true });
    expect(engineSwitches(['rcfd-studio.exe', '--keep-runners'], false, cwd).keepRunner).toBe(false);
    expect(projectArgument(['rcfd-studio', '--keep-runner', 'proj'], false, cwd)).toBe(path.resolve(cwd, 'proj'));
  });
  test('a switch value is never taken for the project folder (F-837)', () => {
    expect(projectArgument(['rcfd-studio.exe', '--engine-dir', 'C:\\eng', 'C:\\p\\quick'], false, 'C:\\')).toBe(path.resolve('C:\\', 'C:\\p\\quick'));
    expect(projectArgument(['rcfd-studio', '--service', '127.0.0.1:9', '--no-engine'], false, cwd)).toBeNull();
    expect(projectArgument(['electron', '.', '--engine-dir=eng', 'proj'], true, cwd)).toBe(path.resolve(cwd, 'proj'));
  });
});
