// Unit tests of run sites in the Studio (F-848, ADR 0739 phase 1): the engine's answers as the Studio reads them (rcfd
// site list --json, rcfd site check), the Where choice and its store in the user data, the link state of a remote run
// from its folder, the action that starts a run on a site (its engine entry point and CLI line, G16; the store plan
// from start_run's dry run; nothing sent when the engine refuses), the project port's `sites` operation, and that the
// Studio does nothing while the user has no sites file. The real engine (target/release) answers the check of an exec
// site when it is built.
import { expect, test } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import {
  LOCAL,
  NO_SITES,
  SITE_RUN_ACTIONS,
  getSiteRunAction,
  isSiteName,
  parseSiteCheck,
  parseSiteList,
  runSiteOf,
  whereOf,
  type RunSiteFiles,
} from '../../src/shared/sites.js';
import { runAvailability, runEquivalents } from '../../src/shared/actions/runs.js';
import type { ProjectView } from '../../src/shared/project.js';
import { RunControl, type ControlDeps } from '../../src/main/project/control.js';
import { parseProjectRequest } from '../../src/main/project/bridge.js';
import { readRunSite, recordedSite, RunSites, SiteChoices, Sites, sitesFilePath } from '../../src/main/project/sites.js';
import type { ProjectServerLike } from '../../src/main/project/server.js';
import { requestWords } from '../../src/renderer/runs/control-model.js';
import { appRoot } from '../support/rcfd.js';
import { findMcp } from '../support/project.js';

const PROJECT = '/p/quick';
const exe = (n: string) => (process.platform === 'win32' ? `${n}.exe` : n);

const view = (cases: string[] = ['cases/a.toml']): ProjectView =>
  ({
    folder: PROJECT,
    engine: { name: 'rcfd', version: '0', path: '/e/rcfd-mcp' },
    info: null,
    infoError: null,
    cases: { items: cases.map((p) => ({ path: p, runs: 0, meshes: [], unread: null })), total: cases.length, error: null },
    meshes: { items: [], total: 0, error: null },
    runs: { items: [], total: 0, error: null },
    notes: { items: [], total: 0, error: null },
    skippedRuns: [],
    runDetails: {},
    runDetailErrors: {},
    changes: [],
    openedAt: 't',
    readAt: 't',
    liveness: { live: true, since: 't' },
  }) as ProjectView;

test.describe('the engine’s answers', () => {
  test('rcfd site list --json: sites with valid names, local is never a site', () => {
    const list = parseSiteList({
      file: 'f',
      sites: [
        { name: 'lab', kind: 'ssh', host: 'me@hpc', engine: '/opt/rcfd', work: '/scratch', assistant: true, confirmed: true, accept_engines: ['rcfd 1 (x)', 3] },
        { name: 'local', kind: 'exec', engine: 'e', work: 'w' },
        { name: 'bad name', kind: 'ssh' },
        { name: 'fake', kind: 'exec', host: null, engine: 'e', work: 'w', confirmed: false },
      ],
    });
    expect(list.map((s) => s.name)).toEqual(['lab', 'fake']);
    expect(list[0]).toEqual({ name: 'lab', kind: 'ssh', host: 'me@hpc', engine: '/opt/rcfd', work: '/scratch', assistant: true, confirmed: true, acceptEngines: ['rcfd 1 (x)'] });
    expect(list[1]!.confirmed).toBe(false);
    expect(parseSiteList(null)).toEqual([]);
    expect(isSiteName('a-b_9')).toBe(true);
    expect(isSiteName('-x')).toBe(true);
    expect(isSiteName('a b')).toBe(false);
    expect(isSiteName('x'.repeat(65))).toBe(false);
  });

  test('rcfd site check: the report, a confirmed site and another engine', () => {
    const c = parseSiteCheck({
      site: 'fake',
      host: null,
      kind: 'exec',
      fingerprint: 'exec',
      fingerprint_error: null,
      engine: 'rcfd 0.1.0 (zzz)',
      local_engine: 'rcfd 0.1.0 (unknown)',
      same_engine: false,
      protocol_ok: true,
      confirmed: 'exec',
      admitted: false,
      reason: 'site fake runs the engine rcfd 0.1.0 (zzz)',
    });
    expect(c).toMatchObject({ site: 'fake', fingerprint: 'exec', sameEngine: false, protocolOk: true, confirmed: true, admitted: false, reason: 'site fake runs the engine rcfd 0.1.0 (zzz)' });
    expect(parseSiteCheck({ confirmed: null }).confirmed).toBe(false);
  });

  test('the Where of the run bar: the remembered site while it is listed, else local', () => {
    const sites = parseSiteList({ sites: [{ name: 'lab', kind: 'ssh' }] });
    expect(whereOf({ ...NO_SITES, exists: true, sites, choice: 'lab' })).toBe('lab');
    expect(whereOf({ ...NO_SITES, exists: true, sites, choice: 'gone' })).toBe(LOCAL);
    expect(whereOf(NO_SITES)).toBe(LOCAL);
  });
});

test.describe('the sites file and the remembered choice', () => {
  test('the sites file is the engine’s: RCFD_SITES_FILE, else the user’s config folder', () => {
    expect(sitesFilePath({ RCFD_SITES_FILE: 'X:\\s.toml', APPDATA: 'C:\\A' }, 'win32')).toBe('X:\\s.toml');
    expect(sitesFilePath({ APPDATA: 'C:\\Users\\me\\AppData\\Roaming' }, 'win32')).toBe('C:\\Users\\me\\AppData\\Roaming\\rcfd\\sites.toml');
    expect(sitesFilePath({ XDG_CONFIG_HOME: '/x', HOME: '/h' }, 'linux')).toBe('/x/rcfd/sites.toml');
    expect(sitesFilePath({ HOME: '/h' }, 'darwin')).toBe('/h/.config/rcfd/sites.toml');
    expect(sitesFilePath({}, 'linux')).toBeNull();
    expect(sitesFilePath({ RCFD_SITES_FILE: '' }, 'win32')).toBeNull();
  });

  test('the choice per project in the user data: site names only, local writes nothing that was not there', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-sites-unit-'));
    try {
      const file = path.join(dir, 'site-choices.json');
      const c = new SiteChoices(file);
      expect(await c.get('/p/a')).toBe(LOCAL);
      await c.set('/p/a', LOCAL);
      expect(existsSync(file)).toBe(false);
      await c.set('/p/a', 'lab');
      await c.set('/p/b', 'fake');
      expect(await c.get('/p/a')).toBe('lab');
      expect(await c.get(process.platform === 'win32' ? '/P/A/' : '/p/a/')).toBe('lab');
      const stored = JSON.parse(await readFile(file, 'utf8'));
      expect(stored.format).toBe('rcfd-studio-site-choices');
      expect(Object.values(stored.projects).sort()).toEqual(['fake', 'lab']);
      await c.set('/p/a', LOCAL);
      expect(await c.get('/p/a')).toBe(LOCAL);
      expect(await c.get('/p/b')).toBe('fake');
      await c.set('/p/b', 'not a name');
      expect(await c.get('/p/b')).toBe('fake');
      await writeFile(file, '{"format":"other","projects":{"x":"lab"}}');
      expect(await c.get('x')).toBe(LOCAL);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  test('no sites file: list runs no program, check and open refuse, and the session reads no run folder', async () => {
    const env = { RCFD_SITES_FILE: path.join(tmpdir(), 'rcfd-no-such-sites-file.toml') };
    // an rcfd that cannot run: any spawn would answer with an error, not NO_SITES
    const s = new Sites({ rcfd: path.join(tmpdir(), 'no-rcfd'), runner: path.join(tmpdir(), 'no-runner'), choices: null, env });
    expect(await s.list('/p')).toEqual({ ...NO_SITES, file: env.RCFD_SITES_FILE });
    expect(await s.check('lab', false)).toEqual({ ok: false, message: 'There is no sites file, so there is no site to check.' });
    expect(await s.open()).toBe('There is no sites file to open.');
    const reads: string[] = [];
    const rs = new RunSites('/p', () => false, {
      recordedSite: async (_f, id) => (reads.push(id), null),
      readRunSite: async () => {
        throw new Error('read');
      },
    });
    expect(await rs.update(['r-1', 'r-2'])).toBeNull();
    expect(reads).toEqual([]);
  });
});

test.describe('the link state of a remote run', () => {
  const files = (over: Partial<RunSiteFiles>): RunSiteFiles => ({ site: 'lab', siteJson: null, events: [], endedOnSite: false, endedHere: false, ...over });
  const sent = { submitted: true, link: 'connected', last_contact: '2026-11-17T10:00:00Z' };

  test('sending, connected, retrying, copying back, done, refused', () => {
    expect(runSiteOf(files({})).link).toBe('sending');
    expect(runSiteOf(files({ events: [{ kind: 'connected' }, { kind: 'uploaded' }] })).link).toBe('sending');
    const up = runSiteOf(files({ siteJson: sent, events: [{ kind: 'connected' }, { kind: 'submitted' }] }));
    expect(up).toEqual({ site: 'lab', link: 'connected', message: null, lastContact: '2026-11-17T10:00:00Z' });
    const lost = runSiteOf(files({ siteJson: sent, events: [{ kind: 'connected' }, { kind: 'lost', message: 'agent exited' }] }));
    expect(lost).toMatchObject({ link: 'retrying', message: 'agent exited' });
    expect(runSiteOf(files({ siteJson: sent, events: [{ kind: 'lost' }, { kind: 'connected' }, { kind: 'request_sent' }] })).link).toBe('connected');
    expect(runSiteOf(files({ siteJson: sent, events: [{ kind: 'unconfigured', message: 'site lab is not configured' }] })).link).toBe('retrying');
    expect(runSiteOf(files({ siteJson: sent, endedOnSite: true })).link).toBe('copying');
    expect(runSiteOf(files({ siteJson: sent, endedOnSite: true, endedHere: true })).link).toBe('connected');
    expect(runSiteOf(files({ siteJson: { ...sent, link: 'done', done: true }, events: [{ kind: 'downloaded' }] })).link).toBe('done');
    expect(runSiteOf(files({ events: [{ kind: 'refused', message: 'site lab speaks rcfd-site/2' }], endedHere: true }))).toMatchObject({ link: 'refused', message: 'site lab speaks rcfd-site/2' });
  });

  test('read from a run folder: run.json site, site.json, site.jsonl and the copied events', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-sites-run-'));
    try {
      const run = path.join(dir, 'runs', 'r-1');
      await mkdir(run, { recursive: true });
      await writeFile(path.join(run, 'run.json'), JSON.stringify({ id: 'r-1', site: 'fake' }));
      await mkdir(path.join(dir, 'runs', 'r-2'), { recursive: true });
      await writeFile(path.join(dir, 'runs', 'r-2', 'run.json'), JSON.stringify({ id: 'r-2' }));
      expect(await recordedSite(dir, 'r-1')).toBe('fake');
      expect(await recordedSite(dir, 'r-2')).toBeNull();
      expect(await recordedSite(dir, 'r-3')).toBeNull();
      expect((await readRunSite(dir, 'r-1', 'fake')).link).toBe('sending');
      await writeFile(path.join(run, 'site.json'), JSON.stringify({ submitted: true, link: 'connected' }));
      await writeFile(path.join(run, 'site.jsonl'), '{"kind":"connected"}\n{"kind":"submitted"}\n{"kind":"lo');
      await writeFile(path.join(run, 'status.json'), JSON.stringify({ state: 'running' }));
      expect((await readRunSite(dir, 'r-1', 'fake')).link).toBe('connected');
      await writeFile(path.join(run, 'events.jsonl'), '{"kind":"state","from":"running","to":"finished"}\n');
      expect((await readRunSite(dir, 'r-1', 'fake')).link).toBe('copying');
      await writeFile(path.join(run, 'status.json'), JSON.stringify({ state: 'finished' }));
      await writeFile(path.join(run, 'site.json'), JSON.stringify({ submitted: true, link: 'done', done: true }));
      const rs = new RunSites(dir, () => true);
      expect(await rs.update(['r-1', 'r-2'])).toEqual({ 'r-1': { site: 'fake', link: 'done', message: null, lastContact: null } });
      expect(await rs.update(['r-1', 'r-2'])).toBeNull();
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

// ---- the action

/** The inventory's capabilities (generated from the binaries, F-824). */
const inventory = new Set(
  (JSON.parse(readFileSync(path.join(appRoot, 'capabilities', 'inventory.json'), 'utf8')) as { capabilities: { id: string }[] }).capabilities.map((c) => c.id),
);

test.describe('the action that starts a run on a site (G16)', () => {
  test('a runner command and its options are the engine’s; no MCP equivalent from the Studio', () => {
    expect(SITE_RUN_ACTIONS.map((a) => a.id)).toEqual(['site.run_case']);
    const a = getSiteRunAction('site.run_case')!;
    expect(a.engine).toEqual({ kind: 'runner', command: 'start' });
    const eq = runEquivalents(a, { case: 'cases/a.toml', site: 'lab', threads: 4, set: ['x=1'] }, PROJECT);
    expect(eq.mcp).toBeNull();
    expect(eq.mcpNote).toContain('assistant = true');
    expect(eq.cli.argv).toEqual(['rcfd-runner', 'start', 'cases/a.toml', '--threads', '4', '--set', 'x=1', '--site', 'lab', '--project', PROJECT]);
    expect(inventory.has('cli:rcfd-runner start')).toBe(true);
    for (const w of eq.cli.argv.filter((x) => x.startsWith('--') && x !== '--project')) expect(inventory.has(`cli:rcfd-runner start ${w}`), w).toBe(true);
    expect(inventory.has('cli:rcfd-runner start --no-runner')).toBe(true);
    expect(inventory.has('cli:rcfd site check --confirm')).toBe(true);
    expect(inventory.has('cli:rcfd site list --json')).toBe(true);
  });

  test('availability: a case of the project, a site name, settings written path=value', () => {
    const a = getSiteRunAction('site.run_case')!;
    const ctx = { project: true, cases: ['cases/a.toml'] };
    expect(runAvailability(a, ctx, { case: 'cases/a.toml', site: 'lab' })).toEqual({ available: true });
    expect(runAvailability(a, ctx, { case: 'cases/b.toml', site: 'lab' }).available).toBe(false);
    expect(runAvailability(a, ctx, { case: 'cases/a.toml', site: 'local' }).available).toBe(false);
    expect(runAvailability(a, ctx, { case: 'cases/a.toml', site: 'lab', set: ['oops'] }).available).toBe(false);
    expect(runAvailability(a, { project: false }, {}).available).toBe(false);
  });

  type Call = { tool: string; args: Record<string, unknown> | undefined };
  const control = (calls: Call[], starts: (readonly string[])[], answer: { ok: true; run: string } | { ok: false; message: string }, ensured: string[]) => {
    const server: ProjectServerLike = {
      info: { name: 'rcfd', version: '0' },
      exited: false,
      async call(tool, args) {
        calls.push({ tool, args });
        if (tool === 'start_run' && args?.['dry_run'] === true) return { dry_run: true, set: ['output.store.every=5', ...((args['set'] as string[] | undefined) ?? [])] };
        throw new Error(`asked ${tool}`);
      },
      onExit() {},
      async close() {},
    };
    const deps: ControlDeps = {
      startControlServer: async () => server,
      ensureRunner: async (f) => (ensured.push(f), { ok: true }),
      startOnSite: async (_f, argv) => (starts.push(argv), answer),
      now: () => '2026-11-17T12:00:00Z',
    };
    return new RunControl(PROJECT, deps);
  };

  test('the store plan from start_run’s dry run, then rcfd-runner start --site, then the runner', async () => {
    const calls: Call[] = [];
    const starts: (readonly string[])[] = [];
    const ensured: string[] = [];
    const rc = control(calls, starts, { ok: true, run: 'r-20261117T120000-abcd' }, ensured);
    const r = await rc.dispatch('site.run_case', { case: 'cases/a.toml', site: 'fake', set: ['solver.max_iterations=5'] }, view());
    expect(r.ok).toBe(true);
    expect(calls).toEqual([{ tool: 'start_run', args: { case: 'cases/a.toml', store: 'plan', dry_run: true, set: ['solver.max_iterations=5'] } }]);
    expect(starts).toEqual([['start', 'cases/a.toml', '--set', 'output.store.every=5', '--set', 'solver.max_iterations=5', '--site', 'fake']]);
    expect(ensured).toEqual([PROJECT]);
    expect(r.request!.run).toBe('r-20261117T120000-abcd');
    expect(r.request!.mcp).toBeNull();
    expect(r.request!.cli.argv).toEqual(['rcfd-runner', 'start', 'cases/a.toml', '--set', 'output.store.every=5', '--set', 'solver.max_iterations=5', '--site', 'fake', '--project', PROJECT]);
    expect(requestWords(r.request!)).toContain('Started on a site from the Studio');
  });

  test('a refusal of the engine: nothing more is started, the words name the site and the way out', async () => {
    const calls: Call[] = [];
    const starts: (readonly string[])[] = [];
    const ensured: string[] = [];
    const rc = control(calls, starts, { ok: false, message: 'site fake is not confirmed: run `rcfd site check fake --confirm` once' }, ensured);
    const r = await rc.dispatch('site.run_case', { case: 'cases/a.toml', site: 'fake' }, view());
    expect(r.ok).toBe(false);
    if (r.ok) return;
    expect(r.failure.problem).toBe('The engine refused to start a run of cases/a.toml on site fake: site fake is not confirmed: run `rcfd site check fake --confirm` once');
    expect(r.failure.recovery).toContain('Nothing was sent to the site');
    expect(ensured).toEqual([]);
    // unavailable before the engine: a case that is not the project's
    const u = await rc.dispatch('site.run_case', { case: 'cases/b.toml', site: 'fake' }, view());
    expect(u.ok).toBe(false);
    expect(starts.length).toBe(1);
  });

  test('the project port’s sites operation: exactly what, and a site name for check and confirm', () => {
    const ok = (raw: unknown) => parseProjectRequest(raw);
    expect(ok({ op: 'sites', id: 1, what: 'list' })).toEqual({ ok: true, request: { op: 'sites', id: 1, what: 'list' } });
    expect(ok({ op: 'sites', id: 1, what: 'open' }).ok).toBe(true);
    expect(ok({ op: 'sites', id: 1, what: 'check', name: 'fake' })).toEqual({ ok: true, request: { op: 'sites', id: 1, what: 'check', name: 'fake' } });
    const refused = (raw: unknown) => {
      const p = parseProjectRequest(raw);
      return p.ok ? null : p.error.message;
    };
    expect(refused({ op: 'sites', id: 1, what: 'edit' })).toContain('what must be one of list, check, confirm, open');
    expect(refused({ op: 'sites', id: 1, what: 'check' })).toBe('sites: check takes exactly what and a site name');
    expect(refused({ op: 'sites', id: 1, what: 'confirm', name: '-oProxyCommand=x y' })).toContain('the name must be a site name');
    expect(refused({ op: 'sites', id: 1, what: 'confirm', name: 'local' })).toContain('the name must be a site name');
    expect(refused({ op: 'sites', id: 1, what: 'list', name: 'x' })).toBe('sites: list takes exactly what');
  });
});

// ---- the real engine on an exec site (the fake remote of ADR 0739's tests)

const mcp = findMcp();
const bin = mcp ? path.dirname(mcp) : null;
const engineThere = bin !== null && existsSync(path.join(bin, exe('rcfd'))) && existsSync(path.join(bin, exe('rcfd-runner')));

test.describe('with the engine', () => {
  test.skip(!engineThere, 'rcfd and rcfd-runner are not built beside rcfd-mcp');

  test('list, check and confirm an exec site through the engine', async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-sites-engine-'));
    const prev = process.env['RCFD_TEST_EXEC_SITES'];
    process.env['RCFD_TEST_EXEC_SITES'] = '1';
    try {
      const file = path.join(dir, 'sites.toml');
      await writeFile(file, `# test sites\n[site.fake]\nkind = "exec"\nengine = '${bin}'\nwork = '${path.join(dir, 'work')}'\n`);
      const s = new Sites({ rcfd: path.join(bin!, exe('rcfd')), runner: path.join(bin!, exe('rcfd-runner')), choices: null, env: { RCFD_SITES_FILE: file } });
      // the engine reads RCFD_SITES_FILE from its environment, the one this process gives it
      process.env['RCFD_SITES_FILE'] = file;
      const l = await s.list(null);
      expect(l.error).toBeNull();
      expect(l.sites.map((x) => [x.name, x.confirmed])).toEqual([['fake', false]]);
      const c = await s.check('fake', false);
      expect(c.ok && c.check).toMatchObject({ site: 'fake', admitted: false, confirmed: false, fingerprint: 'exec', sameEngine: true });
      const y = await s.check('fake', true);
      expect(y.ok && y.check.admitted).toBe(true);
      const text = await readFile(file, 'utf8');
      expect(text.startsWith('# test sites\n[site.fake]\nconfirmed = "exec"\n')).toBe(true);
      expect((await s.list(null)).sites[0]!.confirmed).toBe(true);
      const n = await s.check('nope', false);
      expect(n.ok).toBe(false);
      if (!n.ok) expect(n.message).toContain("site 'nope' is not configured");
    } finally {
      if (prev === undefined) delete process.env['RCFD_TEST_EXEC_SITES'];
      else process.env['RCFD_TEST_EXEC_SITES'] = prev;
      delete process.env['RCFD_SITES_FILE'];
      await rm(dir, { recursive: true, force: true });
    }
  });
});
