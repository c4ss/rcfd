// F-892, ADR 0789 (T2): what the global teardown ends after a run (tests/support/leftovers.ts). (a) the choice, as a
// pure function over a list of processes and the run's registry; (b) with the real `rcfd serve` where it is built: one
// started with its journal folder under a registered state folder, and no service.json, is ended and named; one whose
// state folder is not registered is left alone.
import { expect, test } from '@playwright/test';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { chooseLeftovers, noteLaunch, readRegistry, recordPids, sweepLeftovers, type ProcessRow, type SweepContext } from '../support/leftovers.js';
import { alive, findRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { until } from '../support/fake-service.js';

test.describe('(a) the choice', () => {
  const W = 'win32' as const;
  const studioExe = 'E:\\co\\apps\\studio-web\\out\\packaged\\rcfd-studio.exe';
  const bin = 'E:\\co\\apps\\studio-web\\out\\packaged\\resources\\bin';
  const state = 'C:\\Users\\u\\AppData\\Local\\Temp\\rcfd-studio-state-AbC123';
  const serve = (dir: string) => `"E:\\t\\release\\rcfd.exe" serve --addr 127.0.0.1:0 --ui --journal-dir ${dir}\\journals`;
  const row = (pid: number, name: string, exe: string, cmd: string): ProcessRow => ({ pid, ppid: 1, name, exe, cmd });
  const ctx = (over: Partial<SweepContext> = {}): SweepContext => ({
    studioExe,
    packagedBin: bin,
    launches: [{ dir: state, studioPid: 99, spec: 'project.spec.ts' }],
    recordPids: new Map(),
    platform: W,
    ...over,
  });

  test('a packaged Studio of this checkout: its main process, not its helpers, not another checkout’s', () => {
    const procs = [
      // launched through a shell on Windows: the registry has the shell's pid, the Studio's parent
      { pid: 100, ppid: 99, name: 'rcfd-studio.exe', exe: studioExe, cmd: `"${studioExe}" --inspect=0` },
      row(101, 'rcfd-studio.exe', studioExe, `"${studioExe}" --type=renderer --lang=en`),
      row(102, 'rcfd-studio.exe', 'E:\\other\\apps\\studio-web\\out\\packaged\\rcfd-studio.exe', 'x'),
      row(103, 'rcfd-studio.exe', 'C:\\Program Files\\rcfd Studio\\rcfd-studio.exe', 'x'),
    ];
    expect(chooseLeftovers(procs, ctx())).toEqual([{ pid: 100, kind: 'studio', cmd: `"${studioExe}" --inspect=0`, rule: 'packaged-studio', spec: 'project.spec.ts' }]);
    expect(chooseLeftovers(procs, ctx({ studioExe: null }))).toEqual([]);
    // the path is compared without regard to case and slashes on Windows
    expect(chooseLeftovers(procs, ctx({ studioExe: studioExe.toUpperCase().replaceAll('\\', '/') })).map((l) => l.pid)).toEqual([100]);
  });

  test('rcfd serve by the state folder in its command line, by the service record, and by the packaged engine folder', () => {
    const other = 'C:\\Users\\u\\AppData\\Local\\Temp\\rcfd-studio-state-ZzZ999';
    const procs = [
      row(200, 'rcfd.exe', 'E:\\t\\release\\rcfd.exe', serve(state)),
      row(201, 'rcfd.exe', 'E:\\t\\release\\rcfd.exe', serve(other)),
      row(202, 'rcfd.exe', 'E:\\t\\release\\rcfd.exe', '"E:\\t\\release\\rcfd.exe" serve --addr 127.0.0.1:0 --ui --root D:\\cases'),
      row(203, 'rcfd.exe', `${bin}\\rcfd.exe`, `"${bin}\\rcfd.exe" serve --addr 127.0.0.1:0 --ui --journal-dir C:\\Users\\u\\AppData\\Roaming\\rcfd-studio\\service\\journals`),
      row(204, 'rcfd.exe', 'E:\\other\\apps\\studio-web\\out\\packaged\\resources\\bin\\rcfd.exe', serve(other)),
    ];
    const chosen = chooseLeftovers(procs, ctx({ recordPids: new Map([[202, state]]) }));
    expect(chosen.map((l) => [l.pid, l.kind, l.rule, l.spec])).toEqual([
      [200, 'service', 'state-folder', 'project.spec.ts'],
      [202, 'service', 'service-record', 'project.spec.ts'],
      [203, 'service', 'packaged-engine', null],
    ]);
    // a folder given with forward slashes and another case is the same folder
    expect(chooseLeftovers(procs, ctx({ launches: [{ dir: state.toLowerCase().replaceAll('\\', '/') }], packagedBin: null })).map((l) => l.pid)).toEqual([200]);
    // nothing registered, no packaged copy: nothing is chosen
    expect(chooseLeftovers(procs, ctx({ launches: [], packagedBin: null, studioExe: null }))).toEqual([]);
  });

  test('never a process of another name, an rcfd that is not serving, a runner or a project server', () => {
    const procs = [
      row(300, 'node.exe', 'C:\\node\\node.exe', serve(state)),
      row(301, 'rcfd.exe', 'E:\\t\\release\\rcfd.exe', `"E:\\t\\release\\rcfd.exe" run ${state}\\case.toml`),
      row(302, 'rcfd-runner.exe', `${bin}\\rcfd-runner.exe`, `"${bin}\\rcfd-runner.exe" serve --project ${state}`),
      row(303, 'rcfd-mcp.exe', `${bin}\\rcfd-mcp.exe`, `"${bin}\\rcfd-mcp.exe" --project ${state}`),
      row(304, 'chrome.exe', studioExe, 'x'),
    ];
    // 300 and 301 have a pid the service record names: a pid can be reused by another program
    expect(chooseLeftovers(procs, ctx({ recordPids: new Map([[300, state], [301, state], [302, state]]) }))).toEqual([]);
  });

  test('on Linux: the names without .exe, the paths as they are', () => {
    const procs = [
      row(400, 'rcfd-studio', '/co/apps/studio-web/out/packaged/rcfd-studio', '/co/apps/studio-web/out/packaged/rcfd-studio --no-sandbox'),
      row(401, 'rcfd', '/t/release/rcfd', '/t/release/rcfd serve --addr 127.0.0.1:0 --ui --journal-dir /tmp/rcfd-studio-state-a1/journals'),
      row(402, 'rcfd', '/t/release/rcfd', '/t/release/rcfd serve --addr 127.0.0.1:0 --ui --journal-dir /tmp/RCFD-studio-state-a1/journals'),
    ];
    const c: SweepContext = { studioExe: '/co/apps/studio-web/out/packaged/rcfd-studio', packagedBin: null, launches: [{ dir: '/tmp/rcfd-studio-state-a1' }], recordPids: new Map(), platform: 'linux' };
    expect(chooseLeftovers(procs, c).map((l) => l.pid)).toEqual([400, 401]);
  });

  test('the registry: one line per launch, a line cut short is passed over; the pids of the service records', () => {
    const dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f892-reg-'));
    try {
      const file = path.join(dir, 'registry.jsonl');
      const a = path.join(dir, 'state-a');
      const b = path.join(dir, 'user-data');
      noteLaunch({ dir: a, spec: 'a.spec.ts' }, file);
      noteLaunch({ dir: a, studioPid: 7, spec: 'a.spec.ts' }, file);
      noteLaunch({ dir: b }, file);
      writeFileSync(file, '{"dir": "cut sho', { flag: 'a' });
      expect(readRegistry(file)).toEqual([{ dir: a, spec: 'a.spec.ts' }, { dir: a, studioPid: 7, spec: 'a.spec.ts' }, { dir: b }]);
      expect(readRegistry(path.join(dir, 'none.jsonl'))).toEqual([]);
      expect(readRegistry(undefined)).toEqual([]);
      noteLaunch({ dir: a }, undefined); // no registry: nothing happens
      mkdirSync(a, { recursive: true });
      mkdirSync(path.join(b, 'service'), { recursive: true });
      writeFileSync(path.join(a, 'service.json'), JSON.stringify({ pid: 11 }));
      writeFileSync(path.join(b, 'service', 'service.json'), JSON.stringify({ pid: 12 }));
      expect([...recordPids(readRegistry(file))]).toEqual([[11, a], [12, b]]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

test.describe('(b) with the real rcfd serve', () => {
  const rcfd = findRcfd();
  test.skip(!rcfd, NO_RCFD);

  test('a service under a registered state folder, with no service.json, is ended and named; another is left alone', async () => {
    const dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f892-sweep-'));
    const registered = path.join(dir, 'rcfd-studio-state-registered');
    const foreign = path.join(dir, 'rcfd-studio-state-foreign');
    const started: RunningRcfd[] = [];
    try {
      for (const d of [registered, foreign]) {
        mkdirSync(path.join(d, 'journals'), { recursive: true });
        started.push(await startRcfd(rcfd!, 0, ['--ui', '--journal-dir', path.join(d, 'journals')]));
      }
      const [mine, theirs] = started as [RunningRcfd, RunningRcfd];
      const file = path.join(dir, 'registry.jsonl');
      noteLaunch({ dir: registered, spec: 'some.spec.ts' }, file);
      const lines: string[] = [];
      const ended = sweepLeftovers({ registryFile: file, studioExe: null, packagedBin: null, log: (l) => lines.push(l) });
      expect(ended.map((l) => [l.pid, l.kind, l.rule, l.spec])).toEqual([[mine.pid, 'service', 'state-folder', 'some.spec.ts']]);
      expect(lines).toHaveLength(1);
      expect(lines[0]).toContain(`pid ${mine.pid}`);
      expect(lines[0]).toContain('some.spec.ts');
      await until(() => !alive(mine.pid), 5000, 'the registered service to be gone');
      expect(alive(theirs.pid)).toBe(true);
      // nothing more to end
      expect(sweepLeftovers({ registryFile: file, studioExe: null, packagedBin: null, log: (l) => lines.push(l) })).toEqual([]);
    } finally {
      for (const s of started) await s.stop();
      rmSync(dir, { recursive: true, force: true, maxRetries: 5 });
    }
  });
});
