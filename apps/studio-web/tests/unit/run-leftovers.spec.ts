// F-893, ADR 0790 (T1): the rcfd run solvers a test leaves behind. (a) the choice of the global teardown's third rule
// (tests/support/leftovers.ts), as a pure function: a solver under a registered project folder; (b) with the real engine
// where it is built: a solver whose runner was killed (its parent gone) is ended by the sweep only when its project is
// registered; (c) Runner.endAll(), told no run id, ends the solver of a run nobody noted, and the runner.
import { expect, test } from '@playwright/test';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { chooseLeftovers, noteLaunch, noteProject, readRegistry, sweepLeftovers, type ProcessRow, type SweepContext } from '../support/leftovers.js';
import { copyExample } from '../support/project.js';
import { alive, killIfRcfd, killPid } from '../support/rcfd.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { until } from '../support/fake-service.js';

/** The overrides of run-control.spec.ts: never met, the run goes on until it is stopped. */
const LONG = ['solver.max_iterations=1000000', 'solver.tolerance_u=1e-14', 'solver.tolerance_p=1e-14'];

test.describe('(a) the choice', () => {
  const W = 'win32' as const;
  const project = 'C:\\Users\\u\\AppData\\Local\\Temp\\rcfd-f807-AbC123\\quick-start';
  const other = 'C:\\Users\\u\\AppData\\Local\\Temp\\rcfd-f807-ZzZ999\\quick-start';
  const rcfdExe = 'E:\\t\\release\\rcfd.exe';
  const solver = (dir: string) =>
    `"${rcfdExe}" run ${dir}\\runs\\r-1\\case.snapshot --case-dir ${dir}\\cases --set solver.max_iterations=1000000 --stop-file ${dir}\\runs\\r-1\\cancel --pause-file ${dir}\\runs\\r-1\\pause`;
  const row = (pid: number, name: string, exe: string, cmd: string): ProcessRow => ({ pid, ppid: 1, name, exe, cmd });
  const ctx = (over: Partial<SweepContext> = {}): SweepContext => ({
    studioExe: null,
    packagedBin: null,
    launches: [{ dir: project, project: true, spec: 'run-control.spec.ts' }],
    recordPids: new Map(),
    platform: W,
    ...over,
  });

  test('an rcfd run under a registered project folder is chosen; one under another folder is not', () => {
    const procs = [row(500, 'rcfd.exe', rcfdExe, solver(project)), row(501, 'rcfd.exe', rcfdExe, solver(other))];
    expect(chooseLeftovers(procs, ctx()).map((l) => [l.pid, l.kind, l.rule, l.spec])).toEqual([[500, 'solver', 'project-folder', 'run-control.spec.ts']]);
    // the folder without regard to case and slashes on Windows
    expect(chooseLeftovers(procs, ctx({ launches: [{ dir: project.toUpperCase().replaceAll('\\', '/'), project: true }] })).map((l) => l.pid)).toEqual([500]);
    // nothing registered: nothing is chosen
    expect(chooseLeftovers(procs, ctx({ launches: [] }))).toEqual([]);
    // a folder whose name only starts like the project's is another folder
    expect(chooseLeftovers([row(502, 'rcfd.exe', rcfdExe, solver(`${project}-2`))], ctx())).toEqual([]);
  });

  test('never a runner, a project server, another program, or an rcfd whose command is not run', () => {
    const procs = [
      row(600, 'rcfd-runner.exe', 'E:\\t\\release\\rcfd-runner.exe', `"E:\\t\\release\\rcfd-runner.exe" serve --project ${project}`),
      row(601, 'rcfd-mcp.exe', 'E:\\t\\release\\rcfd-mcp.exe', `"E:\\t\\release\\rcfd-mcp.exe" --project ${project}`),
      row(602, 'node.exe', 'C:\\node\\node.exe', solver(project)),
      row(603, 'rcfd.exe', rcfdExe, `"${rcfdExe}" check ${project}\\cases\\cavity-re100.toml --json`),
    ];
    expect(chooseLeftovers(procs, ctx())).toEqual([]);
  });

  test('a project line takes no part in the rules of F-892: no serve by its folder, no packaged rules', () => {
    const serve = row(700, 'rcfd.exe', rcfdExe, `"${rcfdExe}" serve --addr 127.0.0.1:0 --ui --root ${project}`);
    const bin = 'E:\\co\\apps\\studio-web\\out\\packaged\\resources\\bin';
    const packaged = row(701, 'rcfd.exe', `${bin}\\rcfd.exe`, `"${bin}\\rcfd.exe" serve --addr 127.0.0.1:0`);
    expect(chooseLeftovers([serve, packaged], ctx())).toEqual([]);
    // sweepLeftovers turns the packaged rules on only for a run that launched a Studio: a registry of project lines alone
    const dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f893-reg-'));
    try {
      const file = path.join(dir, 'registry.jsonl');
      noteProject(project, 'run-control.spec.ts', file);
      noteProject(other, null, file);
      noteLaunch({ dir: 'C:\\state' }, file);
      expect(readRegistry(file)).toEqual([{ dir: project, project: true, spec: 'run-control.spec.ts' }, { dir: other, project: true }, { dir: 'C:\\state' }]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test('on Linux: the name without .exe, the paths as they are', () => {
    const c: SweepContext = { studioExe: null, packagedBin: null, launches: [{ dir: '/tmp/rcfd-f807-a1/quick-start', project: true }], recordPids: new Map(), platform: 'linux' };
    const procs = [
      row(800, 'rcfd', '/t/release/rcfd', '/t/release/rcfd run /tmp/rcfd-f807-a1/quick-start/runs/r-1/case.snapshot --case-dir /tmp/rcfd-f807-a1/quick-start/cases'),
      row(801, 'rcfd', '/t/release/rcfd', '/t/release/rcfd run /tmp/RCFD-f807-a1/quick-start/runs/r-1/case.snapshot'),
    ];
    expect(chooseLeftovers(procs, c).map((l) => l.pid)).toEqual([800]);
  });
});

test.describe('with the real engine', () => {
  const runnerExe = findRunner();
  test.skip(!runnerExe, NO_RUNNER);

  /** A runner on a fresh copy of quick-start with one LONG run running; returns the run's id and its solver's pid. */
  async function longRun(dir: string): Promise<{ runner: Runner; id: string; solver: number; quick: string }> {
    const quick = await copyExample('quick-start', dir);
    const runner = new Runner(runnerExe!, quick);
    runner.ensure();
    const id = runner.submit('cases/cavity-re100.toml', LONG);
    const solver = await until(
      () => {
        try {
          const st = runner.status(id);
          return st.state === 'running' && st.pid !== null && st.step > 0 ? st.pid : null;
        } catch {
          return null;
        }
      },
      30_000,
      'the run to be running',
    );
    return { runner, id, solver, quick };
  }

  test('(b) a solver whose runner was killed: the sweep ends it only when its project folder is registered', async () => {
    test.setTimeout(90_000);
    const dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f893-b-'));
    let solver: number | null = null;
    try {
      const r = await longRun(dir);
      solver = r.solver;
      const runnerPid = r.runner.pid()!;
      killPid(runnerPid);
      await until(() => !alive(runnerPid), 10_000, 'the runner to end');
      // the solver goes on without its parent (a tree is not ended with its parent on Windows, and a runner's end
      // leaves its solvers by design, G7)
      expect(alive(solver)).toBe(true);
      const file = path.join(dir, 'registry.jsonl');
      const lines: string[] = [];
      noteLaunch({ dir: path.join(dir, 'some-state-folder') }, file);
      expect(sweepLeftovers({ registryFile: file, studioExe: null, packagedBin: null, log: (l) => lines.push(l) }).filter((l) => l.kind === 'solver')).toEqual([]);
      expect(alive(solver)).toBe(true);
      noteProject(r.quick, 'run-leftovers.spec.ts', file);
      const ended = sweepLeftovers({ registryFile: file, studioExe: null, packagedBin: null, log: (l) => lines.push(l) });
      expect(ended.map((l) => [l.pid, l.kind, l.rule, l.spec])).toEqual([[solver, 'solver', 'project-folder', 'run-leftovers.spec.ts']]);
      expect(lines.filter((l) => l.startsWith('leftover rcfd run ended after the run'))).toHaveLength(1);
      expect(lines.at(-1)).toContain(`pid ${solver}`);
      await until(() => !alive(solver!), 10_000, 'the solver to be gone');
    } finally {
      if (solver !== null) killIfRcfd(solver);
      rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
    }
  });

  test('(c) endAll(), told no run id, ends the solver of the run and the runner', async () => {
    test.setTimeout(90_000);
    const dir = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f893-c-'));
    let solver: number | null = null;
    let runnerPid: number | null = null;
    try {
      const r = await longRun(dir);
      solver = r.solver;
      runnerPid = r.runner.pid();
      const killed = await r.runner.endAll();
      // the cancel ended it: no solver had to be ended by its pid
      expect(killed).toEqual([]);
      await until(() => !alive(solver!), 10_000, 'the solver to be gone');
      expect(r.runner.pid()).toBeNull();
      if (runnerPid !== null) await until(() => !alive(runnerPid!), 10_000, 'the runner to be gone');
    } finally {
      if (solver !== null) killIfRcfd(solver);
      if (runnerPid !== null && alive(runnerPid)) killPid(runnerPid);
      rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
    }
  });
});
