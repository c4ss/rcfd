// ADR 0808 (F-981): the project home's ways to a project folder that stay when the learner's first path (F-813) was
// retired (the maintainer's answer of 2026-11-26, ADR 0803), carried over from tests/e2e/first-path.spec.ts with their
// steps unchanged, in the packaged Studio against the real engine (rcfd-mcp, rcfd-runner and rcfd beside rcfd-mcp):
//
// - No project open: the disclaimer in the footer (G6), as the first case of the old file checked it.
// - An empty folder: the engine refuses it as no project; the page says what is missing (the project file) and the next
//   step, Make a project here, which runs the engine's `rcfd project init` (F-835, ADR 0731) and opens the new project.
//   Nothing in this file writes rcfd-project.json: the engine does (G4, G12).
// - New project… from the command palette in a folder that holds other files: they stay, and the project opens.
//
// The Studio is launched in beforeAll (the old file's first case did it); the `rcfd serve --ui` the old file started
// served only the first path's glossary terms and is not started. Screenshots go to test-results-visual/f813/ as before.
import { expect, test, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { launchStudio, type Studio } from '../support/studio.js';
import { alive, appRoot, killPid } from '../support/rcfd.js';
import { findMcp, mcpPids, NO_MCP, snapshot, stubDialog } from '../support/project.js';
import { until } from '../support/fake-service.js';
import { findRunner, NO_RUNNER, Runner } from '../support/runner.js';

const SHOTS = path.join(appRoot, 'test-results-visual', 'f813');
const mcp = findMcp();
const runnerExe = findRunner();
test.skip(!mcp, NO_MCP);
test.skip(!runnerExe, NO_RUNNER);
test.skip(!!mcp && !existsSync(rcfdExe()), 'rcfd was not found beside rcfd-mcp: build it with `cargo build --release -p rcfd-cli`');
test.describe.configure({ mode: 'serial' });

let dir = '';
let recentFile = '';
let studio: Studio | null = null;
const folders: string[] = [];

const shot = (page: Page, name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });

async function launch(): Promise<Studio> {
  const env: Record<string, string> = { RCFD_STUDIO_MCP: mcp!, RCFD_STUDIO_RECENT: recentFile };
  const s = await launchStudio(env);
  await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => s.page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  return s;
}

/** The rcfd beside the engine's project server: the one the Studio runs for `rcfd project init` (F-835). */
function rcfdExe(): string {
  return path.join(path.dirname(mcp!), process.platform === 'win32' ? 'rcfd.exe' : 'rcfd');
}
/** What rcfd_project's Project::create makes (ADR 0731 P1): the manifest and the six standard folders. */
const MADE = ['.rcfd/', 'cases/', 'designs/', 'meshes/', 'notes/', 'rcfd-project.json', 'runs/'];

/** The project file the engine wrote: named after the folder, with the version `rcfd --version` reports (ADR 0731 Q6). */
async function expectEngineManifest(folder: string): Promise<void> {
  const m = JSON.parse(await readFile(path.join(folder, 'rcfd-project.json'), 'utf8')) as Record<string, unknown>;
  expect(m['name']).toBe(path.basename(folder));
  expect(m['schema_version']).toBe(1);
  expect(m['solver_version_created']).toBe(execFileSync(rcfdExe(), ['--version'], { encoding: 'utf8' }).trim());
}

test.beforeAll(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'rcfd-f981-'));
  recentFile = path.join(dir, 'recent.json');
  await mkdir(SHOTS, { recursive: true });
  studio = await launch();
});

/** Ends the Studio this file started, and the project servers it left (as the F-806/F-807 walks do). */
async function killStudio(s: Studio): Promise<void> {
  const pid = s.app.process().pid!;
  killPid(pid);
  await until(() => !alive(pid), 15_000, 'the killed Studio to end');
  for (const f of folders) for (const p of mcpPids(f)) killPid(p);
}

test.afterAll(async () => {
  if (studio) await killStudio(studio).catch(() => undefined);
  studio = null;
  for (const f of folders) await new Runner(runnerExe!, f).cleanUp(null).catch(() => undefined);
  await rm(dir, { recursive: true, force: true }).catch(() => undefined);
});

test('no project open: the disclaimer stays in the footer (G6)', async () => {
  const { page } = studio!;
  expect(await page.evaluate(() => document.querySelector('h1')?.textContent)).toBe('No project open');
  await expect(page.locator('footer')).toContainText('Not validated for engineering decisions or for safety-critical use. No warranty.');
});

test('an empty folder: what is missing and the next step; Make a project here runs the engine’s command and opens the project', async () => {
  const { app, page } = studio!;
  const empty = path.join(dir, 'my-first-project');
  await mkdir(empty);
  folders.push(empty);
  await stubDialog(app, empty);
  await page.getByRole('button', { name: 'Open project folder…' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Failed to open: not an rcfd project' })).toBeVisible({ timeout: 20_000 });
  const why = page.getByRole('region', { name: 'This folder is not a project yet' });
  await expect(why).toBeVisible();
  // an exception, so in words, short: what is missing and the next step
  await expect(why.locator('.fp-missing')).toHaveText('Empty folder: no rcfd-project.json.');
  await expect(why.locator('.fp-next')).toHaveText('Next: make it a project here.');
  // F-835: the next step is the engine's command; what it does is its description (G22), after the alert's Dismiss
  const make = why.getByRole('button', { name: 'Make a project here' });
  await expect(make).toBeEnabled();
  await expect(make).not.toHaveAttribute('aria-disabled', 'true');
  await expect(make).toHaveAccessibleDescription(/The engine makes the project file .* \(rcfd project init\)/);
  await page.getByRole('alert').getByRole('button', { name: 'Dismiss' }).focus();
  await page.keyboard.press('Tab');
  await expect(make).toBeFocused();
  await expect(why).toHaveAttribute('data-folder-state', 'watching');
  await why.getByRole('button', { name: 'Explain what a project needs' }).click();
  const about = why.getByRole('region', { name: 'About what a project needs' });
  await expect(about).toContainText(`rcfd project init ${empty}`);
  await expect(about).toContainText('Claude Code makes it with create_project');
  await expect(about).toContainText('the Studio checks the folder every second and opens it as soon as the project file is there');
  expect(await readdir(empty)).toEqual([]);
  await shot(page, '02-empty-folder');
  // the engine makes the project (rcfd project init), and it opens
  await make.click();
  await expect(page.getByRole('heading', { level: 1, name: 'my-first-project' })).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole('alert').filter({ hasText: /Failed to (open|make)/ })).toHaveCount(0);
  await expectEngineManifest(empty);
  // what the engine made and nothing else: opening wrote nothing (G12)
  expect(Object.keys(await snapshot(empty)).sort()).toEqual(MADE);
});

test('New project… from the command palette, in a folder that holds other files: they stay, and the project opens', async () => {
  const { app, page } = studio!;
  await page.getByRole('button', { name: 'Close project' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'No project open' })).toBeVisible();
  const third = path.join(dir, 'palette-project');
  await mkdir(third);
  await writeFile(path.join(third, 'my-notes.txt'), 'mine\n');
  folders.push(third);
  await stubDialog(app, third);
  await page.keyboard.press('Control+K');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await expect(palette).toBeVisible();
  await page.keyboard.type('New project');
  await expect(palette.getByRole('option', { name: /^New project…/ })).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 1, name: 'palette-project' })).toBeVisible({ timeout: 20_000 });
  await expectEngineManifest(third);
  // the engine keeps what was there (ADR 0731 Q3 (a))
  expect(await readFile(path.join(third, 'my-notes.txt'), 'utf8')).toBe('mine\n');
  expect(Object.keys(await snapshot(third)).sort()).toEqual([...MADE, 'my-notes.txt'].sort());
});
