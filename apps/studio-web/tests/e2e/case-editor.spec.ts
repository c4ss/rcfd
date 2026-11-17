// The case editor (F-812) in the packaged Studio, against the real engine: `rcfd serve` (the sessions, the schema, the
// checks) and `rcfd-mcp --project` (the hash, the save through edit_case, new_case). A copy of the quick-start project:
// a case opened from the project home, a key changed and saved, and the file read back with the CLI's own reading
// (`rcfd run <case> --print-settings`, `rcfd validate`); a file changed outside between open and save shows the
// conflict with both values and nothing is overwritten, then each choice does what it says; a new case from the
// engine's template, a duplicate, the checks, closing with unsaved changes; an invalid case file names the key, the
// problem and the fix; the keyboard path with visible focus. Screenshots at 1440 by 900 to test-results-visual/f812/.
//
// Needs rcfd and rcfd-mcp (tests/support/rcfd.ts, tests/support/project.ts); without them the tests are skipped.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { SAVE_REASON } from '../../src/shared/case.js';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { copyExample, findMcp, NO_MCP, scratch, stubDialog } from '../support/project.js';
import { appRoot, findRcfd, killIfRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
const mcp = findMcp();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f812');
const ORANGE = 'rgb(240, 138, 36)';

let dir = '';
let quick = '';
let service: RunningRcfd;
let rpc: RpcConnection;
let studio: Studio;
let page: Page;
const consoleErrors: string[] = [];

const file = (rel: string) => path.join(quick, ...rel.split('/'));
const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });

/** The CLI's own reading of a case file: the resolved settings `rcfd run --print-settings` prints. */
function cliSettings(rel: string): Record<string, unknown> {
  return JSON.parse(execFileSync(rcfd!, ['run', file(rel), '--print-settings'], { encoding: 'utf8' })) as Record<string, unknown>;
}
const at = (tree: unknown, p: string): unknown => p.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown> | undefined)?.[k], tree);
const cliValidate = (rel: string) => spawnSync(rcfd!, ['validate', file(rel)], { encoding: 'utf8' });

const sessionOfHash = async (): Promise<string> => {
  await expect.poll(() => page.evaluate(() => location.hash), { timeout: 20_000 }).toMatch(/^#\/case\/s\d+$/);
  return decodeURIComponent((await page.evaluate(() => location.hash)).replace('#/case/', ''));
};

const row = (p: string): Locator => page.locator(`.st-key[data-path="${p}"]`);

/** Finds a key with the surface's search and commits a typed value with Enter (as F-825's controls do). */
async function setByKeyboard(p: string, value: string): Promise<void> {
  const find = page.getByRole('textbox', { name: 'Find a setting' });
  await find.fill(p);
  const field = row(p).getByRole('textbox');
  await field.fill(value);
  await field.press('Enter');
  await expect(page.getByRole('list', { name: 'Settings a save writes' })).toContainText(p);
  await find.fill('');
}

async function expectFocusRing(): Promise<void> {
  const r = await page.evaluate(() => {
    const a = document.activeElement as HTMLElement;
    const host: Element = a.closest('[data-focus-visible]') ?? a;
    const cs = getComputedStyle(host);
    return { outline: `${cs.outlineStyle} ${cs.outlineWidth} ${cs.outlineColor}`, visible: a.matches(':focus-visible') || a.closest('[data-focus-visible]') !== null };
  });
  expect(r.visible).toBe(true);
  expect(r.outline).toBe(`solid 2px ${ORANGE}`);
}

async function openFromHome(rel: string): Promise<string> {
  await page.evaluate(() => (location.hash = ''));
  const edit = page.getByRole('button', { name: `Edit ${rel}`, exact: true });
  await expect(edit).toBeVisible({ timeout: 20_000 });
  await edit.click();
  const s = await sessionOfHash();
  await expect(page.getByRole('heading', { level: 2, name: rel, exact: true })).toBeVisible({ timeout: 20_000 });
  return s;
}

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  dir = await scratch('f812');
  quick = await copyExample('quick-start', dir);
  service = await startRcfd(rcfd!);
  rpc = await RpcConnection.connectTcp('127.0.0.1', service.port);
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`, RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  page = studio.page;
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await page.evaluate(() => {
    const w = window as unknown as { __csp: string[] };
    w.__csp = [];
    document.addEventListener('securitypolicyviolation', (e) => w.__csp.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.locator('tr[data-path="cases/cavity-re100.toml"]')).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  await studio?.app.close();
  rpc?.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('window.studioCase is the case port: its functions only, frozen, refusing a path outside cases/', async () => {
  const shape = await page.evaluate(() => {
    const c = window.studioCase as unknown as Record<string, unknown>;
    return { keys: Object.keys(c).sort(), fns: Object.values(c).every((v) => typeof v === 'function'), frozen: Object.isFrozen(c) };
  });
  expect(shape).toEqual({
    // restated for ADR 0737: pickFile and attachFile, a case file outside a project (tests/e2e/case-file-adr0737.spec.ts)
    keys: ['attach', 'attachFile', 'check', 'conflict', 'create', 'duplicate', 'forget', 'pickFile', 'prepare', 'save', 'sources', 'state', 'status'],
    fns: true,
    frozen: true,
  });
  const outside = await page.evaluate(() => window.studioCase.prepare('rcfd-project.json'));
  expect(outside).toMatchObject({ ok: false, error: { code: 'bad_request' } });
  const up = await page.evaluate(() => window.studioCase.prepare('cases/../rcfd-project.json'));
  expect(up).toMatchObject({ ok: false, error: { code: 'bad_request' } });
  const odd = await page.evaluate(() => (window.studioCase.save as unknown as (a: unknown, b: unknown) => Promise<unknown>)('s1', 'force'));
  expect(odd).toMatchObject({ ok: false, error: { code: 'bad_request' } });
});

test('a case opens from the project home into a session the engine made from the file', async () => {
  const before = await readFile(file('cases/cavity-re100.toml'), 'utf8');
  const s = await openFromHome('cases/cavity-re100.toml');
  const journal = (await rpc.request('journal.get', { session: s })) as { entries: { op: string; source?: { path?: string } }[] };
  expect(journal.entries[0]!.op).toBe('create');
  expect(path.resolve(journal.entries[0]!.source!.path!).toLowerCase()).toBe(file('cases/cavity-re100.toml').toLowerCase());
  // the values the surface shows are the session's, which the engine read from the file
  expect(((await rpc.request('settings.get', { session: s, path: 'physics.nu' })) as { value: unknown }).value).toBe(at(cliSettings('cases/cavity-re100.toml'), 'physics.nu'));
  await expect(page.locator('.ce-tab')).toHaveText('cavity-re100.toml');
  await expect(page.locator('.st-top-right .ce-word')).toHaveText('Saved');
  // learner first (PRODUCT Principle 5): the keys the file sets away from the defaults come first
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible();
  await expect(row('mesh.nx').getByRole('textbox')).toHaveValue(String(at(cliSettings('cases/cavity-re100.toml'), 'mesh.nx')));
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  await shot('01-opened');
  // opening reads, it does not write
  expect(await readFile(file('cases/cavity-re100.toml'), 'utf8')).toBe(before);
});

test('round trip: a key changed and saved here is the value the CLI reads from the file', async () => {
  const rel = 'cases/cavity-re100.toml';
  const s = await sessionOfHash();
  const before = await readFile(file(rel), 'utf8');
  expect(at(cliSettings(rel), 'physics.nu')).toBe(0.01);
  await setByKeyboard('physics.nu', '0.02');
  await expect(page.locator('.st-top-right .ce-word')).toHaveText('1 change not saved');
  await shot('02-unsaved');
  await page.keyboard.press('Control+s');
  await expect(page.getByText(`Saved 1 setting to ${rel}`)).toBeVisible({ timeout: 20_000 });
  await expect(page.locator('.st-top-right .ce-word')).toHaveText('Saved');
  await shot('03-saved');
  // the CLI's reading of the same file
  const cli = cliSettings(rel);
  const session = ((await rpc.request('settings.get', { session: s, path: 'physics.nu' })) as { value: unknown }).value;
  expect(at(cli, 'physics.nu')).toBe(0.02);
  expect(at(cli, 'physics.nu')).toBe(session);
  expect(cliValidate(rel).status).toBe(0);
  // in place: only that line changed, its comment kept; the journal names the Studio's save
  const after = await readFile(file(rel), 'utf8');
  const changed = after.split('\n').filter((l, i) => l !== before.split('\n')[i]);
  expect(changed).toEqual(['nu = 0.02   # Re = 1 x 1 / 0.01 = 100']);
  const journal = (await readFile(path.join(quick, '.rcfd', 'edits.jsonl'), 'utf8')).trim().split('\n').map((l) => JSON.parse(l) as Record<string, unknown>);
  const last = journal[journal.length - 1]!;
  expect(last).toMatchObject({ kind: 'edit', who: 'edit_case', path: rel, reason: SAVE_REASON, changes: [{ path: 'physics.nu', old: 0.01, new: 0.02 }] });
  // a second key, in a section the file holds, by the keyboard alone, then the save button
  await setByKeyboard('solver.max_iterations', '1500');
  const save = page.getByRole('button', { name: 'Save', exact: true });
  await save.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText(`Saved 1 setting to ${rel}`)).toBeVisible({ timeout: 20_000 });
  // polled: the first save's notice has the same words, so it can be seen before the second save has written
  await expect.poll(() => at(cliSettings(rel), 'solver.max_iterations'), { timeout: 20_000 }).toBe(1500);
});

test('conflict: the file changed outside between open and save; both values shown, nothing overwritten', async () => {
  const rel = 'cases/cavity-re100.toml';
  const s = await sessionOfHash();
  await setByKeyboard('solver.max_iterations', '1200');
  // another writer (an editor) changes the same key and adds a line
  const theirs = (await readFile(file(rel), 'utf8')).replace('max_iterations = 1500', 'max_iterations = 3000').replace('[solver]', '# tuned by hand\n[solver]');
  await writeFile(file(rel), theirs);
  // the feed names the file: the editor says so before any save
  await expect(page.locator('.st-top-right .ce-word')).toHaveText('Changed outside', { timeout: 10_000 });
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  const notice = page.getByRole('alert').filter({ hasText: 'changed outside the Studio' });
  await expect(notice.getByRole('heading', { name: `Not saved: ${rel} changed outside the Studio` })).toBeVisible({ timeout: 20_000 });
  // focus moved to the notice, so a keyboard user is there
  expect(await page.evaluate(() => document.activeElement?.textContent)).toBe(`Not saved: ${rel} changed outside the Studio`);
  const tableRow = notice.locator('tbody tr').filter({ hasText: 'solver.max_iterations' });
  await expect(tableRow.locator('td').nth(0)).toHaveText('1200');
  await expect(tableRow.locator('td').nth(1)).toHaveText('3000');
  // nothing overwritten
  expect(await readFile(file(rel), 'utf8')).toBe(theirs);
  await shot('04-conflict');
  // choice: keep editing changes nothing
  await notice.getByRole('button', { name: 'Keep editing' }).click();
  expect(await readFile(file(rel), 'utf8')).toBe(theirs);
  // choice: write mine into the file as it is now: my key, their other line kept, and the editor opens the result
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await notice.getByRole('button', { name: 'Write my changes into the file as it is now' }).click();
  await expect.poll(() => page.evaluate(() => location.hash), { timeout: 20_000 }).not.toBe(`#/case/${s}`);
  const s2 = await sessionOfHash();
  expect(s2).not.toBe(s);
  await expect(page.locator('.st-top-right .ce-word')).toHaveText('Saved', { timeout: 20_000 });
  const merged = await readFile(file(rel), 'utf8');
  expect(merged).toContain('# tuned by hand');
  expect(at(cliSettings(rel), 'solver.max_iterations')).toBe(1200);
  const list = (await rpc.request('session.list')) as { sessions: { session: string }[] };
  expect(list.sessions.map((x) => x.session)).not.toContain(s);
  // the other choice: load the file as it is now, dropping mine
  await setByKeyboard('solver.max_iterations', '900');
  const again = merged.replace('max_iterations = 1200', 'max_iterations = 4000');
  await writeFile(file(rel), again);
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await notice.getByRole('button', { name: /^Load the file as it is now, dropping my 1 change$/ }).click();
  await expect.poll(() => page.evaluate(() => location.hash), { timeout: 20_000 }).not.toBe(`#/case/${s2}`);
  const s3 = await sessionOfHash();
  expect(s3).not.toBe(s2);
  expect(((await rpc.request('settings.get', { session: s3, path: 'solver.max_iterations' })) as { value: unknown }).value).toBe(4000);
  expect(await readFile(file(rel), 'utf8')).toBe(again);
});

test('an invalid case file names the key, the problem and the fix', async () => {
  const rel = 'cases/broken.toml';
  await writeFile(file(rel), (await readFile(file('cases/cavity-re400.toml'), 'utf8')).replace(/\[physics\]\nnu = [^\n]*/, '[physics]\nnu = "fast"'));
  await page.evaluate(() => (location.hash = ''));
  await expect(page.locator(`tr[data-path="${rel}"]`)).toBeVisible({ timeout: 10_000 });
  await page.getByRole('button', { name: `Edit ${rel}`, exact: true }).click();
  await expect(page.getByRole('heading', { name: `Failed to open ${rel}` })).toBeVisible({ timeout: 20_000 });
  const problems = page.getByRole('list', { name: 'Problems found by the engine' });
  await expect(problems).toContainText('physics.nu');
  await expect(page.getByText('Fix the file in a text editor, or ask Claude Code, then try again')).toBeVisible();
  await shot('05-invalid-file');
  await rm(file(rel));
});

test('a new case from the engine’s template, a duplicate, the checks, and closing with unsaved changes asks first', async () => {
  await page.evaluate(() => (location.hash = ''));
  await page.getByRole('button', { name: 'New case', exact: true }).click();
  await expect(page.getByRole('heading', { name: /^New case in / })).toBeVisible();
  // the engine's own list: the template is the first choice (learner first), the shipped examples one step away
  const tools = await (async () => {
    const { ProjectServer } = await import('../../src/main/project/server.js');
    const srv = await ProjectServer.start(mcp!, quick);
    try {
      return (await srv.listTools()) as { name: string; inputSchema: { properties: Record<string, { enum?: string[] }> } }[];
    } finally {
      await srv.close();
    }
  })();
  const templates = tools.find((t) => t.name === 'new_case')!.inputSchema.properties['template']!.enum!;
  await expect(page.getByRole('button', { name: new RegExp(`^${templates[0]}`) })).toBeVisible();
  await page.getByRole('textbox', { name: 'Case name' }).fill('learner-first');
  await shot('06-new-case');
  await page.getByRole('button', { name: 'Make the case' }).click();
  const s = await sessionOfHash();
  await expect(page.getByRole('heading', { level: 2, name: 'cases/learner-first.toml', exact: true })).toBeVisible({ timeout: 20_000 });
  expect(cliValidate('cases/learner-first.toml').status).toBe(0);
  // duplicate: the saved file byte for byte
  await page.getByRole('textbox', { name: 'Duplicate as' }).fill('learner-copy');
  await page.getByRole('button', { name: 'Duplicate', exact: true }).click();
  await expect(page.getByText(/^Made cases\/learner-copy\.toml/)).toBeVisible({ timeout: 20_000 });
  expect(await readFile(file('cases/learner-copy.toml'), 'utf8')).toBe(await readFile(file('cases/learner-first.toml'), 'utf8'));
  // the checks: the session's mesh, and the saved file as the CLI's check-only launch sees it
  await page.getByRole('button', { name: 'Check the mesh' }).click();
  await expect(page.getByText(/^The mesh builds: \d+ cells/)).toBeVisible({ timeout: 30_000 });
  await page.getByRole('button', { name: 'Check the saved file' }).click();
  await expect(page.getByText('The saved file checks out (exit code 0, ok)')).toBeVisible({ timeout: 30_000 });
  expect(spawnSync(rcfd!, ['run', file('cases/learner-first.toml'), '--check-only', '--quiet'], { encoding: 'utf8' }).status).toBe(0);
  await shot('07-checks');
  // closing with a change not saved: the editor names what is lost, and keeping is the default
  await setByKeyboard('physics.nu', '0.05');
  await page.getByRole('button', { name: 'Close the session' }).click();
  // F-837: the decision panel, with Save and close beside losing them
  const confirm = page.getByRole('alertdialog', { name: 'Close cases/learner-first.toml with 1 change not saved?' });
  await expect(confirm).toContainText('physics.nu');
  await expect(confirm.getByRole('button', { name: 'Save and close' })).toBeVisible();
  expect(await page.evaluate(() => document.activeElement?.textContent)).toBe('Keep editing');
  await shot('08-close-asks');
  await page.keyboard.press('Enter');
  await expect(confirm).toHaveCount(0);
  // by the keyboard, as a keyboard user leaves it (the next test starts from the keyboard): Shift+Tab from Keep editing
  await page.getByRole('button', { name: 'Close the session' }).focus();
  await page.keyboard.press('Enter');
  await expect(confirm).toBeVisible();
  await page.keyboard.press('Shift+Tab');
  await expect(confirm.getByRole('button', { name: 'Close and lose them' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  const list = (await rpc.request('session.list')) as { sessions: { session: string }[] };
  expect(list.sessions.map((x) => x.session)).not.toContain(s);
  expect(at(cliSettings('cases/learner-first.toml'), 'physics.nu')).not.toBe(0.05);
});

test('the keyboard alone: open from the home, change a key, save with Ctrl+S; focus is always visible', async () => {
  const rel = 'cases/cavity-re400.toml';
  await page.evaluate(() => (location.hash = ''));
  const edit = page.getByRole('button', { name: `Edit ${rel}`, exact: true });
  await expect(edit).toBeVisible({ timeout: 10_000 });
  await edit.focus();
  await expectFocusRing();
  await page.keyboard.press('Enter');
  await sessionOfHash();
  await expect(page.getByRole('heading', { level: 2, name: rel, exact: true })).toBeVisible({ timeout: 20_000 });
  const find = page.getByRole('textbox', { name: 'Find a setting' });
  await find.focus();
  await page.keyboard.type('solver.max_iterations');
  const field = row('solver.max_iterations').getByRole('textbox');
  await page.keyboard.press('Tab');
  // Tab moves on through the page to the key's field
  for (let i = 0; i < 20 && !(await field.evaluate((el) => el === document.activeElement)); i++) await page.keyboard.press('Tab');
  await expect(field).toBeFocused();
  await expectFocusRing();
  await page.keyboard.press('Control+a');
  await page.keyboard.type('2500');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('list', { name: 'Settings a save writes' })).toContainText('solver.max_iterations');
  await page.keyboard.press('Control+s');
  await expect(page.getByText(`Saved 1 setting to ${rel}`)).toBeVisible({ timeout: 20_000 });
  expect(at(cliSettings(rel), 'solver.max_iterations')).toBe(2500);
  const save = page.getByRole('button', { name: 'Save', exact: true });
  await find.focus();
  for (let i = 0; i < 400 && !(await page.getByRole('button', { name: 'Check the mesh' }).evaluate((el) => el === document.activeElement)); i++) await page.keyboard.press('Tab');
  await expectFocusRing();
  // the shortcut is named beside the button (G20)
  await expect(save.locator('xpath=following-sibling::span[1]')).toHaveText('Nothing to save. Ctrl+S saves.');
  await shot('09-keyboard');
});

test('no Content Security Policy violation and no console error', async () => {
  const csp = await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp);
  expect(csp).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
