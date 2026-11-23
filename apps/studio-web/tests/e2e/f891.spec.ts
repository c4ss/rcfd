// F-891, ADR 0788 (check E1) in the packaged Studio, against the real engine: the runner-budgets panel and the
// project's change feed. The feed has an entry for every journal line that names .rcfd/runner-config.json, this window's
// own writes among them, and the entry reaches the panel up to a feed poll after the write. The panel looks at the file
// then and acts only if it is not the file it holds; it calls values typed only when a field differs from the file.
//
// The tests make an entry arrive when they choose: they append to the project copy's journal (.rcfd/edits.jsonl) a copy
// of the line of this window's own write (a fixture), wait until the project state has the entry and the panel's
// data-file-change has its number, and only then assert. Nothing here waits for time.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { appendFile, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { copyExample, findMcp, mcpPids, NO_MCP, projectState, scratch, stubDialog } from '../support/project.js';
import { findRcfd, killIfRcfd, killPid, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
const mcp = findMcp();
test.skip(rcfd === null, NO_RCFD);
test.skip(mcp === null, NO_MCP);
test.describe.configure({ mode: 'serial', timeout: 120_000 });

const BUDGETS = '.rcfd/runner-config.json';

let dir = '';
let quick = '';
let file = '';
let service: RunningRcfd;
let studio: Studio;
let page: Page;
let panel: Locator;
/** Feed entries about the budgets file the tests have waited for so far. */
let entries = 0;

const fileEntries = async () => ((await projectState(page)).project?.changes ?? []).filter((c) => c.path === BUDGETS);

/** Waits for one more feed entry about the file, and until the open panel has finished with it. */
async function entryHandled(): Promise<void> {
  entries += 1;
  await expect.poll(async () => (await fileEntries()).length).toBe(entries);
  const newest = (await fileEntries())[0]!.n;
  await expect(panel).toHaveAttribute('data-file-change', String(newest));
}

/** One more journal line about the file: a copy of the line of this window's last write. */
async function journalLine(): Promise<void> {
  const journal = path.join(quick, '.rcfd', 'edits.jsonl');
  const lines = (await readFile(journal, 'utf8')).split('\n').filter((l) => l.trim() !== '');
  const last = lines.filter((l) => (JSON.parse(l) as Record<string, unknown>)['path'] === BUDGETS).at(-1);
  expect(last).toBeDefined();
  await appendFile(journal, `${last}\n`);
}

async function openPanel(): Promise<void> {
  await page.getByRole('button', { name: 'Runner budgets (.rcfd/runner-config.json)' }).click();
  await expect(panel).toBeVisible();
}

async function closePanel(): Promise<void> {
  await panel.focus();
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
}

async function type(key: string, value: string): Promise<void> {
  await panel.getByRole('textbox', { name: key }).fill(value);
  await panel.getByRole('textbox', { name: key }).press('Tab');
}

const write = () => panel.getByRole('button', { name: 'Write', exact: true });

test.beforeAll(async () => {
  dir = await scratch('f891');
  quick = await copyExample('quick-start', dir);
  file = path.join(quick, '.rcfd', 'runner-config.json');
  service = await startRcfd(rcfd!);
  studio = await launchStudio({
    RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}`,
    RCFD_STUDIO_MCP: mcp!,
    RCFD_STUDIO_RECENT: path.join(dir, 'recent.json'),
  });
  page = studio.page;
  panel = page.getByRole('region', { name: 'Runner budgets' });
  await stubDialog(studio.app, quick);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('heading', { level: 1, name: /Quick start/ })).toBeVisible({ timeout: 30_000 });
  await page.getByRole('tab', { name: 'Runs' }).click();
  await expect(page.getByRole('grid', { name: /^Runs of / })).toBeVisible({ timeout: 20_000 });
});

test.afterAll(async () => {
  await studio?.app.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
  for (const p of mcpPids(quick)) killPid(p);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('values typed, then an entry that leaves the file as the panel holds it: no alert, the typed value kept', async () => {
  await openPanel();
  await type('max_runs', '2');
  await write().click();
  await expect(panel.getByRole('status')).toContainText('Written to .rcfd/runner-config.json');
  // the entry of this window's own write: the file is the one the panel holds
  await entryHandled();
  await expect(panel.getByRole('alert')).toHaveCount(0);
  await type('cpu_threads', '2');
  await expect(write()).toBeEnabled();
  await journalLine();
  await entryHandled();
  await expect(panel.getByRole('alert')).toHaveCount(0);
  await expect(panel.getByRole('status')).toContainText('Written to .rcfd/runner-config.json');
  await expect(panel.getByRole('textbox', { name: 'cpu_threads' })).toHaveValue('2');
  await expect(write()).toBeEnabled();
  expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({ max_runs: 2 });
  await closePanel();
});

test('a file the runner would refuse, the panel opened, then an entry: one alert, the refused file’s', async () => {
  await writeFile(file, '{"gpu": 1}\n');
  await openPanel();
  await expect(panel.getByText('The runner would refuse this file: unknown field `gpu`')).toBeVisible();
  await journalLine();
  await entryHandled();
  await expect(panel.getByRole('alert')).toHaveCount(1);
  await expect(panel.getByRole('alert')).toContainText('The runner would refuse this file: unknown field `gpu`');
});

test('a refused file with nothing typed, changed by another writer: read again, and nothing said of typed values', async () => {
  // the panel is open on {"gpu": 1}
  await writeFile(file, '{"gpu": 2, "max_runs": 7}\n');
  await journalLine();
  await entryHandled();
  await expect(panel.getByRole('alert')).toHaveCount(1);
  await expect(panel.getByRole('alert')).toContainText('The runner would refuse this file: unknown field `gpu`');
  await expect(panel.locator('[data-note="conflict"]')).toHaveCount(0);
  // the panel holds the file as it is now: the runner replaces it on that base
  await type('max_runs', '2');
  await panel.getByRole('button', { name: 'Replace the file', exact: true }).click();
  await expect(panel.getByRole('status')).toContainText('Written to .rcfd/runner-config.json');
  expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({ max_runs: 2 });
  await entryHandled();
  await expect(panel.getByRole('alert')).toHaveCount(0);
  await closePanel();
});

test('another writer’s change: taken with nothing typed (a Write then has nothing to refuse); said and refused with values typed', async () => {
  await openPanel();
  await expect(panel.getByRole('textbox', { name: 'max_runs' })).toHaveValue('2');
  // nothing typed: the form shows the other writer's file, and says nothing
  await writeFile(file, '{"max_runs": 3}\n');
  await journalLine();
  await entryHandled();
  await expect(panel.getByRole('textbox', { name: 'max_runs' })).toHaveValue('3');
  await expect(panel.getByRole('alert')).toHaveCount(0);
  await type('cpu_threads', '2');
  await write().click();
  await expect(panel.getByRole('status')).toContainText('Written to .rcfd/runner-config.json');
  expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({ max_runs: 3, cpu_threads: 2 });
  await entryHandled();
  // values typed: they are kept, the panel says so, and the Write is refused
  await type('max_runs', '4');
  await expect(write()).toBeEnabled();
  await writeFile(file, '{"max_runs": 5}\n');
  await journalLine();
  await entryHandled();
  await expect(panel.getByRole('alert')).toContainText('was changed by another writer; the values typed here are kept');
  await expect(panel.getByRole('textbox', { name: 'max_runs' })).toHaveValue('4');
  await write().click();
  await expect(panel.getByRole('alert')).toContainText('was changed by another writer since it was read: nothing was written');
  expect(await readFile(file, 'utf8')).toBe('{"max_runs": 5}\n');
  await closePanel();
});
