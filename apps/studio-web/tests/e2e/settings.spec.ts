// The case settings surface (F-825) in the packaged Studio against a fake service (tests/support/fake-case-service.ts):
// every key of the schema gets a control made from its schema node; a key and a section added to the schema appear
// with no Studio code change (the same build, the page read again); edits are settings.set actions and the messages of
// a refused change and of the check are the service's words; a change made by another writer shows without a reload;
// a running session makes the surface read-only with the reason; everything works from the keyboard with visible focus
// (G8); no Content Security Policy violation or console error. Screenshots at 1440 by 900 go to
// test-results-visual/f825/ for the maintainer's view-and-report (G13). The same surface against the real engine:
// tests/e2e/settings-rcfd.spec.ts.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { schemaLeaves } from '../support/capabilities.js';
import { addKey, FakeCaseService, testSchema } from '../support/fake-case-service.js';
import { appRoot } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f825');
const ORANGE = 'rgb(240, 138, 36)';

let fake: FakeCaseService;
let studio: Studio;
let page: Page;
let session: string;
const consoleErrors: string[] = [];

test.beforeAll(async () => {
  mkdirSync(SHOTS, { recursive: true });
  fake = await FakeCaseService.start();
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: fake.address });
  page = studio.page;
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  await studio.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await watchCsp();
});

test.afterAll(async () => {
  await studio?.app.close();
  await fake?.stop();
});

async function watchCsp(): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as { __csp: string[] };
    w.__csp = [];
    document.addEventListener('securitypolicyviolation', (e) => w.__csp.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
}

const row = (p: string): Locator => page.locator(`.st-key[data-path="${p}"]`);
const indexItem = (name: string | RegExp): Locator => page.getByRole('listbox', { name: 'Sections of the case' }).getByRole('option', { name });
const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });

/** A switch is a hidden input under its drawn track: focus it and press Space, as a keyboard user would. */
async function flip(sw: Locator): Promise<void> {
  await sw.focus();
  await page.keyboard.press('Space');
}

async function openSection(name: string): Promise<void> {
  await indexItem(new RegExp(`^${name}\\b`)).click();
  await expect(page.getByRole('heading', { level: 2, name, exact: true })).toBeVisible();
}

/** Moves focus with Tab alone, from the top of the page, until `target` has it (or contains it). */
async function tabTo(target: Locator, max = 200): Promise<void> {
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  for (let i = 0; i < max; i++) {
    await page.keyboard.press('Tab');
    if (await target.evaluate((el) => el === document.activeElement || el.contains(document.activeElement))) return;
  }
  throw new Error('Tab never reached the target');
}

/** The focus indicator of the focused element (DESIGN.md: a 2px orange outline, drawn on the switch's track). */
async function expectFocusRing(): Promise<void> {
  const r = await page.evaluate(() => {
    const a = document.activeElement as HTMLElement;
    let host: Element = a.closest('[data-focus-visible]') ?? a;
    if (host.matches('.ui-switch')) host = host.querySelector('.ui-switch-track')!;
    const cs = getComputedStyle(host);
    return { outline: `${cs.outlineStyle} ${cs.outlineWidth} ${cs.outlineColor}`, visible: a.matches(':focus-visible') || a.closest('[data-focus-visible]') !== null };
  });
  expect(r.visible).toBe(true);
  expect(r.outline).toBe(`solid 2px ${ORANGE}`);
}

test('without a session the page offers the service’s sessions and a new one from the engine’s defaults', async () => {
  await page.evaluate(() => (location.hash = '#/settings'));
  await expect(page.getByRole('heading', { level: 1, name: 'Case settings' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Choose a session' })).toBeVisible();
  await shot('choose-session');
  await page.getByRole('button', { name: 'New session from defaults' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible();
  session = [...fake.sessions.keys()][0]!;
  await expect.poll(() => page.evaluate(() => location.hash)).toBe(`#/settings/${session}`);
  expect(fake.received.map((r) => r.method)).toEqual(expect.arrayContaining(['settings.schema', 'session.list', 'session.create', 'settings.get', 'run.status']));
  await expect(page.getByText('Every key has its default.')).toBeVisible();
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  await shot('set-in-this-case-empty');
});

test('every key of the schema gets one control, made from its schema node', async () => {
  // an entry, so the keys of the named collection have controls too (made outside the Studio: it shows live)
  fake.externalSet(session, 'boundaries.lid', { kind: 'wall', velocity: [1, 0, 0] });
  await expect(page.locator('.st-filled', { hasText: 'boundaries' })).toBeVisible();
  await flip(page.getByRole('switch', { name: 'Show every key' }));
  const seen = new Map<string, string>();
  for (const name of ['case', 'solver', 'boundaries']) {
    await openSection(name);
    for (const r of await page.locator('.st-key').all()) {
      const pattern = (await r.getAttribute('data-pattern'))!;
      const control = (await r.getAttribute('data-control'))!;
      const p = (await r.getAttribute('data-path'))!;
      // exactly one control, named by its path
      const switches = r.getByRole('switch');
      const boxes = r.getByRole('textbox');
      const selects = r.locator('.ui-select-button');
      const counts = [await switches.count(), await boxes.count(), await selects.count()];
      expect(counts.reduce((a, b) => a + b), pattern).toBe(1);
      if (control === 'boolean') await expect(switches).toHaveAccessibleName(p);
      else if (control === 'enum') expect(await selects.count()).toBe(1);
      else await expect(boxes).toHaveAccessibleName(new RegExp(`^${p.replaceAll('.', '\\.')}`));
      seen.set(pattern, control);
    }
  }
  expect([...seen.keys()].sort()).toEqual(schemaLeaves(testSchema()).map((l) => l.path).sort());
  expect(Object.fromEntries(seen)).toMatchObject({
    'solver.algorithm': 'enum',
    'solver.adaptive': 'boolean',
    'solver.alpha_u': 'number',
    'solver.max_iterations': 'integer',
    'solver.direction': 'list',
    title: 'string',
    'boundaries.*.velocity': 'list',
  });
  // type, unit, range, enumeration and default, as the schema gives them
  await openSection('solver');
  await expect(row('solver.alpha_u')).toContainText('Default 0.9');
  await expect(row('solver.alpha_u')).toContainText('Range 0.05 to 1');
  await expect(row('solver.alpha_u')).toContainText('Velocity under-relaxation factor');
  await expect(row('solver.dt').locator('.st-key-name')).toHaveText('dt, s');
  await expect(row('solver.algorithm')).toContainText('3 values');
  await shot('solver-every-key');
  await flip(page.getByRole('switch', { name: 'Show every key' }));
});

test('learner-first: the head of a section first, the rest one step away, a set key always shown', async () => {
  await openSection('solver');
  await expect(page.locator('.st-key')).toHaveCount(8);
  const more = page.getByRole('button', { name: 'Show 2 more keys of solver' });
  await expect(more).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByRole('button', { name: /^limits/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /^rhie_chow.*optional, not in the case/ })).toBeVisible();
  await shot('solver-learner-first');
  fake.externalSet(session, 'solver.tolerance', 0.0001);
  await expect(row('solver.tolerance')).toBeVisible();
  await expect(row('solver.tolerance')).toHaveAttribute('data-set', 'true');
  await expect(page.getByRole('button', { name: 'Show 1 more key of solver' })).toBeVisible();
  await expect(indexItem(/^solver/)).toContainText('1 set');
  await more.or(page.getByRole('button', { name: 'Show 1 more key of solver' })).click();
  await expect(row('solver.scheme')).toBeVisible();
});

test('a key and a section added to the schema appear with no Studio code change', async () => {
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('f825');
  await expect(page.getByRole('heading', { level: 2, name: /^0 keys match/ })).toBeVisible();
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('');
  let schema = addKey(fake.schema, 'solver', 'f825_added', {
    type: 'number',
    default: 0.25,
    minimum: 0,
    maximum: 2,
    'x-rcfd-si-dimension': 's',
    description: 'A key added to the schema by the test',
  });
  schema = addKey(schema, '', 'acoustics', {
    type: 'object',
    properties: { model: { type: 'string', enum: ['off', 'fwh'], default: 'off', description: 'Acoustic analogy (added by the test)' } },
    required: ['model'],
    additionalProperties: false,
    description: 'A section added to the schema by the test',
  });
  fake.schema = schema;
  for (const s of fake.sessions.values()) s.tree['acoustics'] = { model: 'off' };
  await page.reload();
  await watchCsp();
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible();
  await expect(indexItem(/^acoustics/)).toBeVisible();
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('f825_added');
  const added = row('solver.f825_added');
  await expect(added).toBeVisible();
  await expect(added.locator('.st-key-name')).toHaveText('solver.f825_added, s');
  await expect(added).toContainText('Default 0.25');
  await expect(added).toContainText('Range 0 to 2');
  await expect(added).toContainText('A key added to the schema by the test');
  await added.getByRole('textbox').fill('1.5');
  await added.getByRole('textbox').press('Enter');
  await expect.poll(() => fake.sets().at(-1)).toEqual(['solver.f825_added', 1.5]);
  await expect(added).toHaveAttribute('data-set', 'true');
  await page.getByRole('textbox', { name: 'Find a setting' }).fill('');
  await openSection('acoustics');
  await expect(row('acoustics.model').locator('.ui-select-button')).toContainText('off');
  await shot('added-section');
});

test('validation messages are the engine’s words, kept until Escape; the Studio refuses nothing itself', async () => {
  const words = "invalid value at 'solver.alpha_u': 2 outside [Some(0.05), Some(1.0)]";
  fake.refusals.set('solver.alpha_u', words);
  await openSection('solver');
  const field = row('solver.alpha_u').getByRole('textbox');
  await field.fill('2');
  await field.press('Enter');
  await expect(row('solver.alpha_u').locator('.st-key-error')).toHaveText(`Change failed. ${words}`);
  await expect(field).toHaveAttribute('aria-invalid', 'true');
  await expect(field).toHaveValue('2');
  await expect(field).toHaveAccessibleDescription(new RegExp(words.replace(/[[\]().']/g, '\\$&')));
  await shot('refused-change');
  await field.press('Escape');
  await expect(field).toHaveValue('0.9');
  await expect(row('solver.alpha_u').locator('.st-key-error')).toHaveCount(0);
  fake.refusals.delete('solver.alpha_u');
  // a value with a unit is sent as text for the engine to convert; a non-number is sent as it is, for the engine to refuse
  const dt = row('solver.dt').getByRole('textbox');
  await dt.fill('10 ms');
  await dt.press('Enter');
  await expect.poll(() => fake.sets().at(-1)).toEqual(['solver.dt', '10 ms']);
  const it = row('solver.max_iterations').getByRole('textbox');
  await it.fill('1.5');
  await it.press('Enter');
  await expect.poll(() => fake.sets().at(-1)).toEqual(['solver.max_iterations', '1.5']);
  await it.fill('700');
  await it.press('Enter');
  await expect.poll(() => fake.sets().at(-1)).toEqual(['solver.max_iterations', 700]);
  // reset to the schema's default
  await row('solver.max_iterations').getByRole('button', { name: 'Reset solver.max_iterations to its default' }).click();
  await expect.poll(() => fake.sets().at(-1)).toEqual(['solver.max_iterations', 500]);
});

test('the check lists the engine’s problems; each leads to its key, and a missing entry can be added from there', async () => {
  fake.diagnostics = [{ path: 'boundaries.walls', line: null, message: "no boundary condition for mesh patch 'walls'" }];
  await page.getByRole('button', { name: 'Check the case' }).click();
  await expect(page.getByText('Check failed: 1 problem')).toBeVisible();
  await expect(page.getByRole('list', { name: 'Problems found by the engine' })).toContainText("no boundary condition for mesh patch 'walls'");
  await shot('check-problems');
  await page.getByRole('button', { name: 'Go to boundaries.walls' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'boundaries', exact: true })).toBeVisible();
  const name = page.getByRole('textbox', { name: 'Name of a new entry of boundaries' });
  await expect(name).toHaveValue('walls');
  await expect(name).toBeFocused();
  await page.keyboard.press('Enter');
  await expect.poll(() => fake.sets().at(-1)).toEqual(['boundaries.walls.kind', 'wall']);
  await expect(page.getByRole('button', { name: /^walls/ })).toHaveAttribute('aria-expanded', 'true');
  await expect(row('boundaries.walls.kind')).toBeVisible();
  await expect(page.locator('.st-cli')).toHaveText(`rcfd call --addr ${fake.address} settings.set '{"session":"${session}","path":"boundaries.walls.kind","value":"wall"}'`);
  await shot('collection-entries');
  fake.diagnostics = [];
  await page.getByRole('button', { name: 'Check the case' }).click();
  await expect(page.locator('.st-verdict')).toHaveText('The engine found no problems.');
});

test('a change made by another writer shows without a reload', async () => {
  await openSection('solver');
  fake.externalSet(session, 'solver.alpha_u', 0.5);
  await expect(row('solver.alpha_u').getByRole('textbox')).toHaveValue('0.5', { timeout: 2000 });
});

test('while the run goes the surface is read-only and says why', async () => {
  fake.setState(session, 'running');
  await expect(page.getByRole('status').filter({ hasText: 'The run is going; stop it before changing settings.' })).toBeVisible();
  await expect(row('solver.alpha_u').getByRole('textbox')).toHaveAttribute('readonly', '');
  await expect(page.locator('.st-facts')).toContainText('running');
  await shot('read-only-running');
  fake.setState(session, 'stopped');
  await expect(page.locator('.st-banner')).toHaveCount(0);
  await expect(row('solver.alpha_u').getByRole('textbox')).not.toHaveAttribute('readonly', '');
});

test('the surface works from the keyboard alone, with visible focus (G8)', async () => {
  await openSection('solver');
  // search
  const find = page.getByRole('textbox', { name: 'Find a setting' });
  await tabTo(find);
  await expectFocusRing();
  await page.keyboard.type('under-relaxation');
  await expect(page.getByRole('heading', { level: 2, name: /^1 key matches/ })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(find).toHaveValue('');
  // show every key, then the section index: arrows select, Home and End go to the ends
  await page.keyboard.press('Tab');
  await expect(page.getByRole('switch', { name: 'Show every key' })).toBeFocused();
  await expectFocusRing();
  await page.keyboard.press('Tab');
  await expectFocusRing();
  await page.keyboard.press('Home');
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('heading', { level: 2, name: 'case', exact: true })).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('heading', { level: 2, name: 'solver', exact: true })).toBeVisible();
  await expectFocusRing();
  // an enumeration: Enter opens it, arrows choose, Enter takes the value
  const algorithm = row('solver.algorithm').locator('.ui-select-button');
  await tabTo(algorithm);
  await expectFocusRing();
  await expect(algorithm).toHaveAccessibleName(/solver\.algorithm/);
  await page.keyboard.press('Enter');
  await expect(page.getByRole('listbox', { name: /solver\.algorithm/ })).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect.poll(() => fake.sets().at(-1)).toEqual(['solver.algorithm', 'piso']);
  // a boolean: Space switches it
  const adaptive = row('solver.adaptive').getByRole('switch');
  await tabTo(adaptive);
  await expectFocusRing();
  await page.keyboard.press('Space');
  await expect.poll(() => fake.sets().at(-1)).toEqual(['solver.adaptive', true]);
  await expect(adaptive).toBeChecked();
  // a number: typed, Enter sends it, Escape goes back to the engine's value
  const corr = row('solver.corrections').getByRole('textbox');
  await tabTo(corr);
  await expectFocusRing();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.type('3');
  await page.keyboard.press('Enter');
  await expect.poll(() => fake.sets().at(-1)).toEqual(['solver.corrections', 3]);
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.type('4');
  await page.keyboard.press('Escape');
  await expect(corr).toHaveValue('3');
  // a disclosure: Enter opens it and Tab goes into what it opened
  const limits = page.getByRole('button', { name: /^limits/ });
  await tabTo(limits);
  await expectFocusRing();
  await expect(limits).toHaveAttribute('aria-expanded', 'false');
  await page.keyboard.press('Enter');
  await expect(limits).toHaveAttribute('aria-expanded', 'true');
  await page.keyboard.press('Tab');
  await expect(row('solver.limits.enabled').getByRole('switch')).toBeFocused();
  await expectFocusRing();
  // removing an entry is confirmed in words, from the keyboard
  await openSection('boundaries');
  const lid = page.getByRole('button', { name: /^lid/ });
  await tabTo(lid);
  await page.keyboard.press('Enter');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Remove entry boundaries.lid' })).toBeFocused();
  await expectFocusRing();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('group', { name: 'Remove boundaries.lid?' })).toContainText('Remove boundaries.lid and its 2 keys from the session?');
  await expect(page.getByRole('button', { name: 'Remove', exact: true })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect.poll(() => fake.received.filter((r) => r.method === 'settings.remove').map((r) => r.params['path'])).toEqual(['boundaries.lid']);
  await expect(page.getByRole('button', { name: /^lid/ })).toHaveCount(0);
});

test('no Content Security Policy violation and no console error', async () => {
  const csp = await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp);
  expect(csp).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
