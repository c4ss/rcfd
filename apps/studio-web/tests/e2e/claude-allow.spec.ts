// F-814, "Allow this from now on" (G11, G18, G10; the maintainer's approval of 2026-11-17), in the packaged Studio
// against the engine's project server (rcfd-mcp) and the fake claude of tests/support/fake-claude.mjs, whose canned
// reply has one denied call (Bash, `echo hello > made_by_bash.txt`).
//
// - the denied call offers its narrowest rule; a confirm names the exact rule, keeping it denied is the default;
// - on confirm the main process writes .rcfd/claude-allow.json (the Studio's first list, then the rule), the folder stays
//   confirmed, and the next chat passes the rule to claude;
// - the palette's Undo the last allowed rule takes it out again; an undo over another writer's change is refused, with
//   Undo anyway; a rule that was not suggested, a stale base or an approve-everything rule is refused by the main process;
// - axe (serious or critical) finds nothing on the denied call's entry or on the confirm.
// Screenshots at 1440 by 900 go to test-results-visual/f814/.
import { expect, test, type Page } from '@playwright/test';
import { mkdir, mkdtemp, readdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { appRoot, killPid } from '../support/rcfd.js';
import { copyExample, findMcp, mcpPids, NO_MCP, stubDialog } from '../support/project.js';
import { APPROVE_EVERYTHING, claudeFixture, FAKE_CLAUDE, type ClaudeFixture } from '../support/claude.js';
import { until } from '../support/fake-service.js';
import { axeCheck } from '../support/axe.js';
import { AllowList } from '../../src/main/claude/allow.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f814');
const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

const RULE = 'Bash(echo hello > made_by_bash.txt)';
const FIRST = AllowList.initial().values;

let fx: ClaudeFixture;
let quick = '';
let studio: Studio | null = null;
let page: Page;
const consoleErrors: string[] = [];

const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const panel = () => page.getByRole('complementary', { name: 'Claude Code' });
const allowFile = () => path.join(quick, '.rcfd', 'claude-allow.json');
const fileRules = async (): Promise<string[] | null> => {
  try {
    return (JSON.parse(await readFile(allowFile(), 'utf8')) as { rules: string[] }).rules;
  } catch {
    return null;
  }
};
const allowButton = () => panel().getByRole('button', { name: `Allow this from now on: ${RULE}` }).last();
const confirmPanel = () => page.getByRole('alertdialog', { name: `Allow ${RULE} from now on?` });
const state = () => page.evaluate(() => window.studioClaude.state());

async function chat(text: string): Promise<void> {
  const before = (await fx.chats()).length;
  const field = panel().getByRole('textbox', { name: /^Message to Claude Code/ });
  await field.fill(text);
  await field.press('Enter');
  await until(async () => ((await fx.chats()).length > before ? true : null), 30_000, 'the chat to start');
  await expect.poll(() => state().then((s) => s.project?.writer === null && s.project?.turn?.status), { timeout: 30_000 }).toBe('finished');
}

async function palette(command: string): Promise<void> {
  await page.keyboard.press('Control+K');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  const search = dialog.getByLabel('Find a command by its name or its id');
  await expect(search).toBeFocused();
  await search.fill(command);
  await expect(dialog.getByRole('option', { name: new RegExp(`^${command}`) })).toBeVisible();
  await page.keyboard.press('Enter');
}

test.beforeAll(async () => {
  await mkdir(SHOTS, { recursive: true });
  fx = await claudeFixture('allow');
  const dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f814-allow-'));
  quick = await copyExample('quick-start', dir);
  studio = await launchStudio({
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_RECENT: path.join(fx.root, 'recent.json'),
    RCFD_STUDIO_CLAUDE: FAKE_CLAUDE,
    RCFD_STUDIO_CLAUDE_STATE: fx.state,
    ...fx.env(),
  });
  page = studio.page;
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  await expect(panel()).toBeVisible();
});

test.afterAll(async () => {
  await studio?.app.close();
  for (const p of mcpPids(quick)) killPid(p);
});

test('at rest nothing is offered; a denied call offers its rule, and the confirm names it, keeping it denied by default', async () => {
  await expect(panel().getByRole('button', { name: /^Allow this from now on/ })).toHaveCount(0);
  expect(await fileRules()).toBeNull();
  await chat('Write a file');
  // the denial stays after the transcript replaced the stream
  await expect(allowButton()).toBeVisible({ timeout: 20_000 });
  await expect(panel().locator('[data-kind="denied"]').last()).toContainText(`The allow-list rule ${RULE} would allow it.`);
  // axe on what this item adds: the denied call's entry, then the confirm (the session list's own findings are not this
  // item's; see ADR 0801)
  const denied = await axeCheck(page, '.cc-event--denied');
  expect(denied.failing).toEqual([]);
  await allowButton().focus();
  await page.keyboard.press('Enter');
  await expect(confirmPanel()).toBeVisible();
  await expect(confirmPanel()).toContainText(`The rule ${RULE} is added to .rcfd/claude-allow.json of this project.`);
  await expect(confirmPanel().getByRole('button')).toHaveText(['Keep it denied', 'Allow from now on']);
  expect(await page.evaluate(() => document.activeElement?.textContent)).toBe('Keep it denied');
  const asking = await axeCheck(page, '.hi-panel');
  expect(asking.failing).toEqual([]);
  await page.mouse.move(1439, 899);
  await shot('10-allow-confirm');
  // Escape keeps it denied: nothing written, the focus back on the button
  await page.keyboard.press('Escape');
  await expect(confirmPanel()).toHaveCount(0);
  await expect(allowButton()).toBeFocused();
  expect(await fileRules()).toBeNull();
});

test('on confirm the rule is written to the project’s allow-list file, the folder stays confirmed, and the next chat passes it', async () => {
  await page.keyboard.press('Enter');
  await expect(confirmPanel()).toBeVisible();
  await page.keyboard.press('Tab');
  await expect(confirmPanel().getByRole('button', { name: 'Allow from now on' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(confirmPanel()).toHaveCount(0);
  await expect.poll(fileRules, { timeout: 10_000 }).toEqual([...FIRST, RULE]);
  const text = await readFile(allowFile(), 'utf8');
  expect(JSON.parse(text)).toEqual({ version: 1, rules: [...FIRST, RULE] });
  // no stray file beside it (write, then rename)
  const beside = await readdir(path.join(quick, '.rcfd'));
  expect(beside).toContain('claude-allow.json');
  expect(beside.filter((f) => f.startsWith('claude-allow.json') && f !== 'claude-allow.json')).toEqual([]);
  const s = await state();
  expect(s.project?.allow).toMatchObject({ source: 'file', undo: RULE, error: null });
  expect(s.project?.trust).toEqual({ state: 'confirmed', files: 1 });
  await expect(panel().getByText('Confirm the Claude Code files of this folder')).toHaveCount(0);
  await expect(panel().locator('[data-kind="denied"]').last()).toContainText(`The allow-list rule ${RULE} allows it from now on.`);
  await expect(panel().getByRole('button', { name: `Undo: take ${RULE} out of the allow-list` })).toBeVisible();
  await expect(panel().getByRole('button', { name: `${FIRST.length + 1} allowed, everything else denied` })).toBeVisible();
  await page.mouse.move(1439, 899);
  await shot('11-allowed');
  await chat('Again');
  const chats = await fx.chats();
  const argv = chats.at(-1)!.argv;
  expect(argv.slice(argv.indexOf('--allowedTools') + 1, argv.indexOf('--session-id') >= 0 ? argv.indexOf('--session-id') : argv.indexOf('--resume'))).toEqual([...FIRST, RULE]);
  expect(argv.filter((a) => APPROVE_EVERYTHING.some((re) => re.test(a)))).toEqual([]);
});

test('the palette’s Undo the last allowed rule takes it out again', async () => {
  await palette('Undo the last allowed rule');
  const dialog = page.getByRole('dialog', { name: 'Command palette' });
  await expect(dialog.getByRole('heading', { name: 'Undo the last allowed rule: done' })).toBeVisible();
  await expect(dialog).toContainText(`${RULE} is out of the allow-list again.`);
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  expect(await fileRules()).toEqual(FIRST);
  await expect.poll(() => state().then((s) => s.project?.allow.undo ?? null)).toBeNull();
  await expect(panel().getByRole('button', { name: `${FIRST.length} allowed, everything else denied` })).toBeVisible();
  await expect(allowButton()).toBeVisible();
  // nothing left to undo: the palette says why
  await page.keyboard.press('Control+K');
  const search = dialog.getByLabel('Find a command by its name or its id');
  await search.fill('Undo the last allowed rule');
  await expect(dialog.getByRole('option', { name: /^Undo the last allowed rule/ })).toContainText('Unavailable: This Studio has added no rule');
  await page.keyboard.press('Escape');
});

test('another writer’s change is never overwritten silently: undo is refused, with Undo anyway', async () => {
  await allowButton().click();
  await confirmPanel().getByRole('button', { name: 'Allow from now on' }).click();
  await expect.poll(fileRules, { timeout: 10_000 }).toEqual([...FIRST, RULE]);
  const theirs = `${JSON.stringify({ version: 1, rules: [...FIRST, RULE, 'Bash(git status)'] }, null, 2)}\n`;
  await writeFile(allowFile(), theirs);
  await panel().getByRole('button', { name: `Undo: take ${RULE} out of the allow-list` }).click();
  await expect(panel().getByText(/^Not done: \.rcfd\/claude-allow\.json was changed by another writer after/)).toBeVisible();
  expect(await readFile(allowFile(), 'utf8')).toBe(theirs);
  await page.mouse.move(1439, 899);
  await shot('12-undo-conflict');
  await panel().getByRole('button', { name: 'Undo anyway' }).click();
  await expect.poll(fileRules, { timeout: 10_000 }).toEqual(FIRST);
  // the other writer's change was not the Studio's: the folder's control files are asked about again before a chat
  await expect(panel().getByText('Confirm the Claude Code files of this folder')).toBeVisible();
});

test('the main process refuses a rule it did not suggest, approve-everything rules and a malformed request', async () => {
  const s = await state();
  const base = s.project!.allow.hash;
  for (const rule of ['Bash', 'Bash(*)', 'Edit(.rcfd/claude-allow.json)', 'Bash(git status)', '--dangerously-skip-permissions']) {
    const r = await page.evaluate(([x, b]) => window.studioClaude.allow(x!, b ?? null), [rule, base] as const);
    expect(r, rule).toMatchObject({ ok: false, error: { code: 'bad_request' } });
  }
  const odd = await page.evaluate(() => (window.studioClaude.allow as unknown as (r: unknown, b: unknown) => Promise<{ ok: boolean; error?: { code: string } }>)(['Bash'], 'x'));
  expect(odd).toMatchObject({ ok: false, error: { code: 'bad_request' } });
  expect(await fileRules()).toEqual(FIRST);
  expect(consoleErrors).toEqual([]);
});
