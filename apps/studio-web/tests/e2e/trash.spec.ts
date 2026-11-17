// The project trash (ADR 0736, F-829) in the packaged Studio, against the real engine (rcfd-mcp --project): on a copy of
// the quick-start project, by the window's own controls.
// - Move a finished run to the trash: the decision panel names what moves, the engine moves runs/<id>/ into
//   .rcfd/trash/, the home lists it in Trash; Undo (the F-829 history step) puts it back byte for byte.
// - Move a run's results: fields/ and output/ go, the run's record stays; Empty trash, behind the decision panel, has the
//   engine remove them for good.
// - Move a case file, then Restore it from the Trash list.
// The Studio itself moves and removes nothing (every change is the engine's journal line, who: a trash tool).
import { expect, test, type Page } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { copyExample, findMcp, NO_MCP, scratch, snapshot, stubDialog } from '../support/project.js';

const mcp = findMcp();
test.skip(!mcp, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

/** The quick-start run of cases/cavity-re100.toml (finished, with stored fields) and the one of cavity-re400.toml. */
const RE100 = 'r-20261028T152641-f8c2';
const RE400 = 'r-20261028T152641-97f1';

let dir = '';
let quick = '';
let studio: Studio | null = null;

const subtree = (snap: Record<string, string>, prefix: string) => Object.fromEntries(Object.entries(snap).filter(([k]) => k.startsWith(prefix)));
const journal = () =>
  readFileSync(path.join(quick, '.rcfd', 'edits.jsonl'), 'utf8')
    .split('\n')
    .filter((l) => l.trim() !== '')
    .map((l) => JSON.parse(l) as Record<string, unknown>);
const trashGroup = (page: Page) => page.locator('[data-group="trash"]');

async function confirm(page: Page, title: RegExp, choice: string): Promise<void> {
  const panel = page.getByRole('alertdialog', { name: title });
  await expect(panel).toBeVisible();
  await panel.getByRole('button', { name: choice }).click();
  await expect(panel).toBeHidden();
}

test.beforeAll(async () => {
  dir = await scratch('trash');
  quick = await copyExample('quick-start', dir);
  studio = await launchStudio({ RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: path.join(dir, 'recent.json') });
  await stubDialog(studio.app, quick);
  await studio.page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(studio.page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  await studio?.app.close();
  if (dir) await rm(dir, { recursive: true, force: true });
});

test('a finished run moves to the trash, confirmed in words, and Undo puts it back byte for byte', async () => {
  const page = studio!.page;
  const before = subtree(await snapshot(quick), `runs/${RE100}`);
  expect(Object.keys(before).length).toBeGreaterThan(3);
  // at rest there is no Trash list
  await expect(trashGroup(page)).toHaveCount(0);
  await page.getByRole('button', { name: `Move run ${RE100} to the trash` }).click();
  const panel = page.getByRole('alertdialog', { name: `Move run ${RE100} to the trash?` });
  await expect(panel).toContainText(`runs/${RE100}/ moves`);
  // the safe choice has the focus
  await expect(panel.getByRole('button', { name: 'Keep it' })).toBeFocused();
  await confirm(page, /Move run .* to the trash\?/, 'Move to the trash');
  await expect.poll(() => existsSync(path.join(quick, 'runs', RE100))).toBe(false);
  await expect(trashGroup(page)).toContainText(`runs/${RE100}`, { timeout: 10_000 });
  const moved = journal().at(-1)!;
  expect([moved['kind'], moved['who'], moved['path']]).toEqual(['trashed', 'trash_run', `runs/${RE100}`]);

  await page.getByRole('button', { name: `Undo: move run ${RE100} to the trash` }).click();
  await expect.poll(() => existsSync(path.join(quick, 'runs', RE100))).toBe(true);
  expect(subtree(await snapshot(quick), `runs/${RE100}`)).toEqual(before);
  await expect(trashGroup(page)).toHaveCount(0, { timeout: 10_000 });
  expect(journal().at(-1)!['kind']).toBe('restored');
});

test('a run’s results move to the trash; Empty trash removes them for good, behind the decision panel', async () => {
  const page = studio!.page;
  const run = path.join(quick, 'runs', RE400);
  const parts = ['fields', 'output'].filter((p) => existsSync(path.join(run, p)));
  expect(parts.length).toBeGreaterThan(0);
  await page.getByRole('button', { name: `Move the results of run ${RE400} to the trash` }).click();
  await confirm(page, /Move the results of run .* to the trash\?/, 'Move to the trash');
  await expect.poll(() => parts.some((p) => existsSync(path.join(run, p)))).toBe(false);
  expect(existsSync(path.join(run, 'run.json'))).toBe(true);
  await expect(trashGroup(page)).toContainText(parts.join(', '), { timeout: 10_000 });

  await trashGroup(page).getByRole('button', { name: 'Empty trash' }).click();
  const panel = page.getByRole('alertdialog', { name: /in the trash for good\?/ });
  await expect(panel).toContainText('This cannot be undone');
  await expect(panel.getByRole('button', { name: 'Keep the trash' })).toBeFocused();
  await confirm(page, /in the trash for good\?/, 'Remove for good');
  await expect(trashGroup(page)).toHaveCount(0, { timeout: 10_000 });
  expect(journal().at(-1)!['kind']).toBe('trash_emptied');
  expect(parts.some((p) => existsSync(path.join(run, p)))).toBe(false);
});

test('a case file moves to the trash and Restore in the Trash list puts it back', async () => {
  const page = studio!.page;
  const file = path.join(quick, 'cases', 'cavity-re400.toml');
  const text = readFileSync(file, 'utf8');
  await page.getByRole('button', { name: 'Move cases/cavity-re400.toml to the trash' }).click();
  await confirm(page, /Move cases\/cavity-re400\.toml to the trash\?/, 'Move to the trash');
  await expect.poll(() => existsSync(file)).toBe(false);
  await trashGroup(page).getByRole('button', { name: 'Restore cases/cavity-re400.toml' }).click();
  await expect.poll(() => existsSync(file)).toBe(true);
  expect(readFileSync(file, 'utf8')).toBe(text);
  await expect(trashGroup(page)).toHaveCount(0, { timeout: 10_000 });
});
