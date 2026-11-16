// The case settings surface (F-825) against the real `rcfd serve`: a session made from the engine's defaults in the
// Studio; the engine's own problems from the check, each leading to its key; an entry the check asks for added from
// there; the engine's words for a refused value; a value with a unit converted by the engine, not the Studio; and, with
// one entry in every named collection, every case key of settings.schema (the F-824 inventory) with exactly one control
// in the window. Screenshots at 1440 by 900 to test-results-visual/f825/ (real-*.png) for the view-and-report.
//
// Needs the rcfd binary (tests/support/rcfd.ts); without it the tests are skipped and say why.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { RpcConnection } from '../../src/main/service/rpc.js';
import { caseModel, entrySeed, type CaseModel, type SettingNode } from '../../src/renderer/settings/schema.js';
import { schemaLeaves } from '../support/capabilities.js';
import { appRoot, findRcfd, killIfRcfd, NO_RCFD, startRcfd, type RunningRcfd } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

const rcfd = findRcfd();
test.skip(rcfd === null, NO_RCFD);
test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f825');

let service: RunningRcfd;
let rpc: RpcConnection;
let studio: Studio;
let page: Page;
let session: string;
let schemaReply: { case: Record<string, unknown> };
let model: CaseModel;
const consoleErrors: string[] = [];

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  service = await startRcfd(rcfd!);
  rpc = await RpcConnection.connectTcp('127.0.0.1', service.port);
  schemaReply = (await rpc.request('settings.schema')) as { case: Record<string, unknown> };
  model = caseModel(schemaReply);
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: `127.0.0.1:${service.port}` });
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
    location.hash = '#/settings';
  });
});

test.afterAll(async () => {
  await studio?.app.close();
  rpc?.close();
  await service?.stop();
  if (service) killIfRcfd(service.pid);
});

const row = (p: string): Locator => page.locator(`.st-key[data-path="${p}"]`);
const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `real-${name}.png`), scale: 'css' });

async function openSection(name: string): Promise<void> {
  await page.getByRole('listbox', { name: 'Sections of the case' }).getByRole('option', { name: new RegExp(`^${name}\\b`) }).click();
  await expect(page.getByRole('heading', { level: 2, name, exact: true })).toBeVisible();
}

test('a session from the engine’s defaults; its sections in the engine’s order', async () => {
  await page.getByRole('button', { name: 'New session from defaults' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible();
  session = decodeURIComponent((await page.evaluate(() => location.hash)).replace('#/settings/', ''));
  const list = (await rpc.request('session.list')) as { sessions: { session: string }[] };
  expect(list.sessions.map((s) => s.session)).toContain(session);
  const names = await page.getByRole('listbox', { name: 'Sections of the case' }).getByRole('option').locator('.st-index-name').allTextContents();
  const order = (schemaReply.case['required'] as string[]).filter((k) => model.root.children.some((c) => c.name === k && c.kind !== 'leaf'));
  expect(names.slice(0, 2)).toEqual(['Set in this case', 'case']);
  expect(names.slice(2, 2 + order.length)).toEqual(order);
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  await shot('set-in-this-case');
  await openSection('solver');
  await expect(page.locator('.st-key')).toHaveCount(8);
  await shot('solver');
  await openSection('mesh');
  await shot('mesh');
});

test('the check shows the engine’s problems; one leads to the entry it asks for, added from there', async () => {
  const direct = (await rpc.request('settings.validate', { session })) as { diagnostics: { path: string; message: string }[] };
  expect(direct.diagnostics.length).toBeGreaterThan(0);
  await page.getByRole('button', { name: 'Check the case' }).click();
  const problems = page.getByRole('list', { name: 'Problems found by the engine' });
  await expect(problems.locator('li')).toHaveCount(direct.diagnostics.length);
  for (const d of direct.diagnostics) await expect(problems).toContainText(d.message);
  await shot('check');
  const lid = direct.diagnostics.find((d) => d.path === 'boundaries.lid');
  expect(lid?.message).toBe("no boundary condition for mesh patch 'lid'");
  await page.getByRole('button', { name: 'Go to boundaries.lid' }).click();
  const name = page.getByRole('textbox', { name: 'Name of a new entry of boundaries' });
  await expect(name).toHaveValue('lid');
  await page.getByRole('button', { name: 'Add entry' }).click();
  await expect(page.getByRole('button', { name: /^lid/ })).toHaveAttribute('aria-expanded', 'true');
  const tree = (await rpc.request('settings.get', { session, path: 'boundaries' })) as { value: Record<string, unknown> };
  expect(Object.keys(tree.value)).toEqual(['lid']);
  await shot('boundaries-entry');
});

test('a refused value shows the engine’s words; a value with a unit is converted by the engine', async () => {
  // the engine's words for the same value, asked directly on a session of its own
  const other = ((await rpc.request('session.create')) as { session: string }).session;
  const words = await rpc.request('settings.set', { session: other, path: 'solver.alpha_u', value: 2 }).then(
    () => null,
    (e: { data?: { diagnostics?: { message: string }[] } }) => e.data?.diagnostics?.[0]?.message ?? null,
  );
  await rpc.request('session.close', { session: other });
  expect(words).toContain('solver.alpha_u');
  await openSection('solver');
  const alpha = row('solver.alpha_u').getByRole('textbox');
  await alpha.fill('2');
  await alpha.press('Enter');
  await expect(row('solver.alpha_u').locator('.st-key-error')).toHaveText(`Change failed. ${words}`);
  await shot('refused');
  await alpha.press('Escape');
  await expect(alpha).toHaveValue('0.9');
  // a dimensioned number: "36 km/h" is sent as text and the engine stores 10 (m/s)
  const speed = model.leaves.find((l) => l.control === 'number' && l.siDimension === 'm s^-1' && !l.pattern.includes('*'));
  expect(speed, 'a number in m/s outside a collection').toBeTruthy();
  await page.getByRole('textbox', { name: 'Find a setting' }).fill(speed!.pattern);
  const field = row(speed!.pattern).getByRole('textbox');
  await field.fill('36 km/h');
  await field.press('Enter');
  await expect(field).toHaveValue('10');
  expect(((await rpc.request('settings.get', { session, path: speed!.pattern })) as { value: unknown }).value).toBe(10);
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('');
});

test('every case key of settings.schema gets exactly one control in the window', async () => {
  test.setTimeout(240_000);
  // one entry in every named collection (made through the engine, as another writer would: it shows live)
  const seeds: { path: string; value: unknown }[] = [];
  const walk = (n: SettingNode, at: string) => {
    if (n.kind === 'group') for (const c of n.children) walk(c, at === '' ? c.name : `${at}.${c.name}`);
    else if (n.kind === 'collection') {
      const entry = `${at}.probe`;
      const seed = entrySeed(n.entry, entry);
      if (seed) seeds.push(seed);
      walk(n.entry, entry);
    }
  };
  walk(model.root, '');
  for (const s of seeds) await rpc.request('settings.set', { session, path: s.path, value: s.value });
  const options = page.getByRole('listbox', { name: 'Sections of the case' }).getByRole('option');
  await options.first().click();
  await expect(page.locator('.st-filled')).toHaveCount(seeds.length);
  await page.getByRole('switch', { name: 'Show every key' }).focus();
  await page.keyboard.press('Space');
  const n = await options.count();
  const seen = new Map<string, number>();
  for (let i = 1; i < n; i++) {
    await options.nth(i).click();
    await expect(page.locator('#st-view-title')).toHaveText((await options.nth(i).locator('.st-index-name').textContent())!);
    const rows = await page.$$eval('.st-key', (els) =>
      els.map((e) => ({
        pattern: e.getAttribute('data-pattern')!,
        controls: e.querySelectorAll('.ui-input, [role="switch"], .ui-select-button').length,
      })),
    );
    for (const r of rows) {
      expect(r.controls, r.pattern).toBe(1);
      seen.set(r.pattern, (seen.get(r.pattern) ?? 0) + 1);
    }
  }
  const keys = schemaLeaves(schemaReply.case).map((l) => l.path);
  expect(keys.length).toBeGreaterThan(900);
  expect(keys.filter((k) => !seen.has(k))).toEqual([]);
  expect([...seen.keys()].filter((k) => !keys.includes(k))).toEqual([]);
  // the columns scroll on their own; the page itself stays one window high (header band and footer in view)
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(900);
  await openSection('turbulence');
  await shot('turbulence-every-key');
  await page.getByRole('switch', { name: 'Show every key' }).focus();
  await page.keyboard.press('Space');
});

test('no Content Security Policy violation and no console error', async () => {
  const csp = await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp);
  expect(csp).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
