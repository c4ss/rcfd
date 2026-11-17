// Run sites in the packaged Studio (F-848, ADR 0739 phase 1, test S11), against the real engine and the fake remote of
// ADR 0739's tests: sites of kind `exec` (RCFD_TEST_EXEC_SITES=1), whose agent `rcfd-runner site --stdio --work <dir>`
// is spawned directly instead of through ssh, a second folder on this machine being the remote.
//
// - Start on a site: the Where choice of the run bar (shown because the sites file lists sites), Run on the confirmed
//   site `fake`; the run's row has the site badge and its link state (connected, then done), its run.json names the
//   site, its residuals.jsonl is the site's byte for byte, and the workspace reads the copied-back files unchanged: the
//   residual readout is the solver's line, the field is drawn from the downloaded store.
// - Pause, resume and stop of a remote run through the existing controls: each reaches the site's run and the local
//   status follows.
// - The remembered choice: after a restart the run bar's Where is the last site used, kept in the Studio's user data
//   (site-choices.json beside the recent list), and the project folder holds no site.
// - A confirm prompt: the unconfirmed site `fresh` asks in the decision panel (host key, the engines); Keep it here
//   starts nothing and sends nothing; Confirm and run records the yes in the sites file and starts the run there.
// - The Sites section of the Studio settings: the sites with their state, Check, and Open the sites file (the system's
//   editor, stubbed here).
// - No sites file: no Where choice, no Sites section; the remote runs keep their badge (P7), the local runs have none.
//
// Screenshots at 1440 by 900 go to test-results-visual/f848/.
import { expect, test, type Page } from '@playwright/test';
import { existsSync, readdirSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { launchStudio, type Studio } from '../support/studio.js';
import { appRoot, killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, projectState, stubDialog } from '../support/project.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';
import { until } from '../support/fake-service.js';
import { countWords } from '../support/word-count.js';
import { WORD_BUDGETS } from '../../src/shared/word-budget.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f848');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.describe.configure({ mode: 'serial' });

/** Never met: the run goes on until it is stopped. */
const LONG = ['solver.max_iterations=1000000', 'solver.tolerance_u=1e-14', 'solver.tolerance_p=1e-14'];

let dir = '';
let quick = '';
let work = '';
let sitesFile = '';
let recentFile = '';
let runner: Runner | null = null;
let studio: Studio | null = null;
const started: string[] = [];
const envBefore: Record<string, string | undefined> = {};

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const tomlPath = (p: string) => `'${p}'`;

async function launch(env: Record<string, string> = { RCFD_SITES_FILE: sitesFile, RCFD_TEST_EXEC_SITES: '1' }): Promise<Studio> {
  const s = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: recentFile, ...env });
  await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => s.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  return s;
}

async function openProject(s: Studio): Promise<void> {
  await stubDialog(s.app, quick);
  await s.page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(s.page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
  await s.page.getByRole('tab', { name: 'Runs' }).click();
  await expect(s.page.getByRole('grid', { name: /^Runs of / })).toBeVisible();
}

async function close(s: Studio | null): Promise<void> {
  if (!s) return;
  await s.app.close().catch(() => undefined);
  for (const p of mcpPids(quick)) killPid(p);
}

const bar = (page: Page) => page.getByRole('region', { name: 'Start a run' });
const where = (page: Page) => bar(page).getByRole('button', { name: /Where to run/ });
const badge = (page: Page, id: string) => page.locator(`.rv-table [data-run="${id}"]`).locator('xpath=ancestor::*[@role="row"]').locator('.st-badge');
const linkOf = (page: Page, id: string) => badge(page, id).getAttribute('data-link');
const stateOf = (page: Page, id: string) => page.locator(`.rv-case[data-run="${id}"]`).getAttribute('data-state');

async function chooseWhere(page: Page, w: string): Promise<void> {
  await where(page).click();
  await page.getByRole('option', { name: w, exact: true }).click();
  await expect(where(page)).toContainText(w);
}

async function chooseCase(page: Page, c: string): Promise<void> {
  const sel = bar(page).getByRole('button', { name: /Case to run/ });
  await sel.click();
  await page.getByRole('option', { name: c }).click();
  await expect(sel).toContainText(c);
}

/** The newest run the window started (its request's run). */
const newestRun = async (page: Page): Promise<string> =>
  until(async () => (await projectState(page)).project?.runRequests?.find((q) => q.outcome === 'accepted')?.run ?? null, 60_000, 'a started run');

/** The mirror of the project on the fake site (<work>/projects/<key>, the key from the project's name and path). */
const mirror = (): string | null => {
  const p = path.join(work, 'projects');
  if (!existsSync(p)) return null;
  // one project goes to the fake site in these tests
  const d = readdirSync(p, { withFileTypes: true }).find((e) => e.isDirectory())?.name;
  return d ? path.join(p, d) : null;
};

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f848-'));
  quick = await copyExample('quick-start', dir);
  work = path.join(dir, 'site-work');
  sitesFile = path.join(dir, 'sites.toml');
  recentFile = path.join(dir, 'recent.json');
  await mkdir(SHOTS, { recursive: true });
  const bin = path.dirname(runnerExe!);
  await writeFile(
    sitesFile,
    `# the fake remote of the Studio's tests (ADR 0739)\n[site.fake]\nkind = "exec"\nengine = ${tomlPath(bin)}\nwork = ${tomlPath(work)}\nmax_runs = 2\nconfirmed = "exec"\n\n` +
      `[site.fresh]\nkind = "exec"\nengine = ${tomlPath(bin)}\nwork = ${tomlPath(work)}\n`,
  );
  // the runner is started from the test (as run-control.spec.ts does: one the window starts would inherit Playwright's
  // pipes on Windows, ADR 0146), with the sites file and the exec sites in its environment
  for (const k of ['RCFD_SITES_FILE', 'RCFD_TEST_EXEC_SITES']) envBefore[k] = process.env[k];
  process.env['RCFD_SITES_FILE'] = sitesFile;
  process.env['RCFD_TEST_EXEC_SITES'] = '1';
  runner = new Runner(runnerExe!, quick);
  runner.ensure();
});

test.afterAll(async () => {
  await close(studio);
  studio = null;
  for (const id of started) {
    try {
      const st = runner!.status(id);
      if (!['finished', 'failed', 'cancelled'].includes(st.state)) runner!.cli('cancel', id);
    } catch {
      // gone
    }
  }
  await runner?.cleanUp(started.at(-1) ?? null);
  // the site's runner over the mirror (started by the agent on the "remote"): stopped cleanly
  const m = mirror();
  if (m) {
    try {
      execFileSync(runnerExe!, ['shutdown', '--project', m], { stdio: 'ignore', windowsHide: true, timeout: 60_000 });
    } catch {
      // not running
    }
  }
  for (const [k, v] of Object.entries(envBefore)) {
    if (v === undefined) delete process.env[k];
    else process.env[k] = v;
  }
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('start on a site: the Where choice, the site badge and its link state, and the workspace reads the copied-back run', async () => {
  test.setTimeout(180_000);
  studio = await launch();
  const { page } = studio;
  await openProject(studio);
  await expect(where(page)).toBeVisible();
  // local is the default
  await expect(where(page)).toContainText('local');
  await chooseCase(page, 'cavity-re100.toml');
  await chooseWhere(page, 'fake');
  await shot(page, '01-where');
  // G22 with a sites file: the Where choice adds no chrome word to the Runs view (its value is data); reported, not held
  const runsWords = await countWords(page, { root: '.rv', exclude: ['.rv-run'] });
  console.log('F-848 runs words with sites:', runsWords.count, 'budget', WORD_BUDGETS['runs'], JSON.stringify(runsWords.words));
  await bar(page).getByRole('button', { name: 'Run cavity-re100.toml on fake' }).click();
  const id = await newestRun(page);
  started.push(id);
  // the record names the site; the panel shows the CLI line it ran, with the store plan's settings
  const rec = JSON.parse(await readFile(path.join(quick, 'runs', id, 'run.json'), 'utf8')) as { site?: string; settings?: { set?: string[] } };
  expect(rec.site).toBe('fake');
  // the case stores its fields itself, so the store plan adds no setting (the plan's settings are covered by tests/unit)
  expect(rec.settings?.set ?? []).toEqual([]);
  await expect(page.locator('aside.rv-run .rc-equiv')).toContainText(`rcfd-runner start cases/cavity-re100.toml`);
  await expect(page.locator('aside.rv-run .rc-equiv')).toContainText('--site fake --project');
  // the badge: the site's name and the link state, as the local runner writes it
  await expect(badge(page, id)).toHaveAttribute('data-site', 'fake', { timeout: 30_000 });
  const seen = new Set<string>();
  await until(
    async () => {
      const l = await linkOf(page, id);
      if (l) seen.add(l);
      return l === 'done' ? true : null;
    },
    120_000,
    'the link to be done',
  );
  expect([...seen].some((l) => l === 'connected' || l === 'sending' || l === 'copying')).toBe(true);
  await expect.poll(() => stateOf(page, id), { timeout: 30_000 }).toBe('finished');
  await expect(badge(page, id).locator('.st-badge-link')).toHaveText('done');
  await expect(page.locator('aside.rv-run .st-badge')).toHaveAttribute('data-site', 'fake');
  await shot(page, '02-remote-done');
  // the copies are the site's bytes; the outputs and the fields came back (Q4)
  const m = mirror();
  expect(m).not.toBeNull();
  expect(await readFile(path.join(quick, 'runs', id, 'residuals.jsonl'))).toEqual(await readFile(path.join(m!, 'runs', id, 'residuals.jsonl')));
  expect(existsSync(path.join(quick, 'runs', id, 'site.json'))).toBe(true);
  expect(existsSync(path.join(quick, 'runs', id, 'fields'))).toBe(true);

  // the workspace: the badge, the residual readout from the copied residuals.jsonl, the field from the downloaded store
  await page.getByRole('tab', { name: 'Workspace' }).click();
  const list = page.getByRole('listbox', { name: 'Runs of the project' });
  await expect(list).toBeVisible({ timeout: 20_000 });
  await list.locator(`[data-run="${id}"]`).click();
  await expect(page.locator('.ws')).toHaveAttribute('data-run', id);
  await expect(list.locator('.st-badge[data-site="fake"]')).toHaveAttribute('data-link', 'done');
  await until(
    async () => {
      const want = await page.locator('.ws').getAttribute('data-snapshot');
      const got = await page.locator('.fv-frame').getAttribute('data-snapshot');
      return want !== null && want === got && (await page.locator('.fv-frame').getAttribute('data-ready')) !== null ? true : null;
    },
    30_000,
    'the field of the remote run to be drawn',
  );
  const step = Number(await page.locator('.ws').getAttribute('data-step'));
  const lines = (await readFile(path.join(quick, 'runs', id, 'residuals.jsonl'), 'utf8')).split('\n').filter((l) => l.trim());
  const line = lines.map((l) => JSON.parse(l) as { step: number; residuals: Record<string, number> }).find((l) => l.step === step);
  expect(line).toBeTruthy();
  const cont = page.locator('.tp-resid-values li[data-name="continuity"]');
  await expect(cont).toHaveAttribute('data-step', String(step));
  expect(Number(await cont.getAttribute('data-value'))).toBe(line!.residuals['continuity']);
  await shot(page, '03-remote-workspace');
  await page.getByRole('tab', { name: 'Runs' }).click();
});

test('pause, resume and stop of a remote run through the existing controls', async () => {
  test.setTimeout(240_000);
  const { page } = studio!;
  await expect(where(page)).toContainText('fake');
  await chooseCase(page, 'cavity-re400.toml');
  await bar(page).getByRole('button', { name: 'Queue…' }).click();
  await page.getByRole('textbox', { name: 'Settings for this run' }).fill(LONG.join('\n'));
  const before = new Set((await projectState(page)).project!.runRequests?.map((q) => q.seq));
  await page.getByRole('button', { name: 'Add to the queue' }).click();
  const id = await until(
    async () => (await projectState(page)).project?.runRequests?.find((q) => !before.has(q.seq) && q.outcome === 'accepted')?.run ?? null,
    60_000,
    'the queued remote run',
  );
  started.push(id);
  await page.locator(`.rv-case[data-run="${id}"]`).click();
  const aside = page.locator('aside.rv-run');
  await expect(aside).toHaveAttribute('data-run', id);
  await expect.poll(() => stateOf(page, id), { timeout: 90_000 }).toBe('running');
  await expect.poll(() => linkOf(page, id), { timeout: 30_000 }).toBe('connected');
  const m = mirror()!;
  const siteState = () => JSON.parse(execFileSync(runnerExe!, ['status', id, '--project', m], { encoding: 'utf8', windowsHide: true })).state as string;

  await aside.getByRole('button', { name: 'Pause' }).click();
  await expect.poll(() => stateOf(page, id), { timeout: 60_000 }).toBe('paused');
  expect(siteState()).toBe('paused');
  await shot(page, '04-remote-paused');
  await aside.getByRole('button', { name: 'Resume' }).click();
  await expect.poll(() => stateOf(page, id), { timeout: 60_000 }).toBe('running');
  expect(siteState()).toBe('running');

  await aside.getByRole('button', { name: 'Stop…' }).click();
  await page.getByRole('button', { name: 'Stop the run' }).click();
  await expect.poll(() => stateOf(page, id), { timeout: 90_000 }).toBe('cancelled');
  expect(siteState()).toBe('cancelled');
  await expect.poll(() => linkOf(page, id), { timeout: 60_000 }).toBe('done');
  const log = (await readFile(path.join(quick, 'runs', id, 'site.jsonl'), 'utf8')).split('\n').filter((l) => l.trim()).map((l) => JSON.parse(l) as { kind: string; what?: string });
  expect(log.filter((e) => e.kind === 'request_sent').map((e) => e.what)).toEqual(['pause', 'resume', 'cancel']);
  await shot(page, '05-remote-stopped');
});

test('the remembered choice: the last site of the project after a restart, kept in the Studio’s user data only', async () => {
  test.setTimeout(120_000);
  await close(studio);
  studio = await launch();
  const { page } = studio;
  await openProject(studio);
  await expect(where(page)).toContainText('fake');
  const choices = JSON.parse(await readFile(path.join(dir, 'site-choices.json'), 'utf8')) as { format: string; projects: Record<string, string> };
  expect(choices.format).toBe('rcfd-studio-site-choices');
  expect(Object.values(choices.projects)).toEqual(['fake']);
  // nothing machine-specific in the project: no site file and no host in it (Q8)
  const manifest = await readFile(path.join(quick, 'rcfd-project.json'), 'utf8');
  expect(manifest).not.toContain('fake');
  expect(existsSync(path.join(quick, 'site-choices.json'))).toBe(false);
  expect(existsSync(path.join(quick, '.rcfd', 'site-choices.json'))).toBe(false);
});

test('a confirm prompt: an unconfirmed site asks first; no starts nothing, yes records it and starts the run there', async () => {
  test.setTimeout(180_000);
  const { page } = studio!;
  await chooseCase(page, 'cavity-re100.toml');
  await chooseWhere(page, 'fresh');
  const runsBefore = (await projectState(page)).project!.runs.items.length;
  await bar(page).getByRole('button', { name: 'Run cavity-re100.toml on fresh' }).click();
  const panel = page.getByRole('alertdialog', { name: 'Confirm site fresh?' });
  await expect(panel).toBeVisible({ timeout: 30_000 });
  await expect(panel).toContainText('cases/cavity-re100.toml');
  await expect(panel).toContainText('Host key');
  await expect(panel).toContainText('exec');
  await expect(panel.getByRole('button', { name: 'Keep it here' })).toBeFocused();
  await shot(page, '06-confirm');
  await panel.getByRole('button', { name: 'Keep it here' }).click();
  await expect(panel).toBeHidden();
  expect((await readFile(sitesFile, 'utf8')).match(/confirmed/g)?.length).toBe(1);
  expect((await projectState(page)).project!.runs.items.length).toBe(runsBefore);

  await bar(page).getByRole('button', { name: 'Run cavity-re100.toml on fresh' }).click();
  await expect(panel).toBeVisible({ timeout: 30_000 });
  const seqBefore = (await projectState(page)).project!.runRequests?.[0]?.seq ?? 0;
  await panel.getByRole('button', { name: 'Confirm and run' }).click();
  await expect(panel).toBeHidden({ timeout: 30_000 });
  const id = await until(
    async () => (await projectState(page)).project?.runRequests?.find((q) => q.seq > seqBefore && q.outcome === 'accepted')?.run ?? null,
    60_000,
    'the run on fresh',
  );
  started.push(id);
  const text = await readFile(sitesFile, 'utf8');
  expect(text).toMatch(/\[site\.fresh\]\nconfirmed = "exec"/);
  expect(text.startsWith('# the fake remote')).toBe(true);
  await expect(badge(page, id)).toHaveAttribute('data-site', 'fresh', { timeout: 30_000 });
  await expect.poll(() => linkOf(page, id), { timeout: 120_000 }).toBe('done');
});

test('the Sites section of the Studio settings: the sites, Check, and Open the sites file', async () => {
  const { app, page } = studio!;
  await app.evaluate(({ shell }) => {
    const w = globalThis as unknown as { __opened?: string[] };
    w.__opened = [];
    (shell as unknown as { openPath: (p: string) => Promise<string> }).openPath = async (p: string) => {
      w.__opened!.push(p);
      return '';
    };
  });
  await page.keyboard.press('Control+,');
  await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible();
  const section = page.getByRole('region', { name: 'Sites' });
  await expect(section).toBeVisible();
  await expect(section.locator('.ss-site[data-site="fake"]')).toHaveAttribute('data-confirmed', 'true');
  await expect(section.locator('.ss-site[data-site="fresh"]')).toHaveAttribute('data-confirmed', 'true');
  await section.getByRole('button', { name: 'Check fake' }).click();
  await expect(section.locator('.ss-site[data-site="fake"] .ss-site-check')).toHaveAttribute('data-admitted', 'true', { timeout: 30_000 });
  await section.getByRole('button', { name: 'Open the sites file' }).click();
  await expect.poll(() => app.evaluate(() => (globalThis as unknown as { __opened?: string[] }).__opened)).toEqual([sitesFile]);
  await shot(page, '07-sites-section');
  const ssWords = await countWords(page, { root: '.ss-page' });
  console.log('F-848 studio-settings words with sites:', ssWords.count, 'budget', WORD_BUDGETS['studio-settings'], JSON.stringify(ssWords.words));
  await page.getByRole('button', { name: 'Close settings' }).click();
});

test('no sites file: no Where choice, no Sites section, the Studio as before', async () => {
  test.setTimeout(120_000);
  await close(studio);
  studio = await launch({ RCFD_SITES_FILE: path.join(dir, 'no-sites.toml') });
  const { page } = studio;
  await openProject(studio);
  await expect(bar(page).getByRole('button', { name: /Run cavity/ })).toBeVisible();
  await expect(where(page)).toHaveCount(0);
  // the remote runs keep their site's name (ADR 0739 P7); the local runs have no badge
  await expect(page.locator('.rv-table .st-badge[data-site="fake"]').first()).toBeVisible({ timeout: 20_000 });
  const local = (await projectState(page)).project!.runs.items.filter((r) => !started.includes(r.id)).map((r) => r.id);
  expect(local.length).toBeGreaterThan(0);
  for (const id of local) await expect(badge(page, id)).toHaveCount(0);
  await page.keyboard.press('Control+,');
  await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Sites' })).toHaveCount(0);
  await shot(page, '08-no-sites');
  await close(studio);
  studio = null;
});
