// Undo, redo, losses and conflicts in the packaged Studio (F-829, G18, G8) against the fake service
// (tests/support/fake-case-service.ts), on the case settings page:
// - the header band's Undo and Redo name the step ("Undo: set solver.alpha_u") and take every edit back and forth;
// - Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y do the same from the keyboard, and inside a field holding typing not yet sent
//   Ctrl+Z stays the field's own;
// - a value another writer changed is shown as a conflict with both values and named choices, and nothing is sent
//   until the user chooses, on undo and on a typed value alike;
// - undoing the new session and Close session, which cannot be undone, ask first in words that name what is lost, with
//   keeping the default;
// - focus is visible and the choices are reached with the keyboard; no CSP violation or console error.
// Screenshots at 1440 by 900 to test-results-visual/f829/ (G13). The history against the real engine:
// tests/e2e/undo-rcfd.spec.ts.
import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { FakeCaseService } from '../support/fake-case-service.js';
import { appRoot } from '../support/rcfd.js';
import { launchStudio, type Studio } from '../support/studio.js';

test.describe.configure({ mode: 'serial' });

const SHOTS = path.join(appRoot, 'test-results-visual', 'f829');
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
  await page.evaluate(() => {
    const w = window as unknown as { __csp: string[] };
    w.__csp = [];
    document.addEventListener('securitypolicyviolation', (e) => w.__csp.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
});

test.afterAll(async () => {
  await studio?.app.close();
  await fake?.stop();
});

const row = (p: string): Locator => page.locator(`.st-key[data-path="${p}"]`);
const field = (p: string): Locator => row(p).getByRole('textbox');
const undoButton = (): Locator => page.getByRole('group', { name: /^Undo and redo/ }).getByRole('button').first();
const redoButton = (): Locator => page.getByRole('group', { name: /^Undo and redo/ }).getByRole('button').nth(1);
const dialog = (): Locator => page.getByRole('alertdialog');
const shot = (name: string) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), scale: 'css' });
const value = (p: string): unknown => {
  let v: unknown = fake.sessions.get(session)?.tree;
  for (const k of p.split('.')) v = typeof v === 'object' && v !== null ? (v as Record<string, unknown>)[k] : undefined;
  return v;
};

async function openSolver(): Promise<void> {
  await page.getByRole('listbox', { name: 'Sections of the case' }).getByRole('option', { name: /^solver\b/ }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'solver', exact: true })).toBeVisible();
}

async function type(p: string, text: string): Promise<void> {
  await field(p).fill(text);
  await field(p).press('Enter');
}

/** The ring of the element the decision panel focused itself (however it was opened). */
async function expectPanelRing(): Promise<void> {
  const outline = await page.evaluate(() => {
    const cs = getComputedStyle(document.activeElement as HTMLElement);
    return `${cs.outlineStyle} ${cs.outlineWidth} ${cs.outlineColor}`;
  });
  expect(outline).toBe(`solid 2px ${ORANGE}`);
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

test('a new session and each edit become steps; Undo and Redo name them and take them back and forth', async () => {
  await page.evaluate(() => (location.hash = '#/settings'));
  await page.getByRole('button', { name: 'New session from defaults' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible();
  session = [...fake.sessions.keys()][0]!;
  await expect(undoButton()).toHaveText(`Undo: new session ${session}`);
  await expect(redoButton()).toHaveText('Nothing to redo');
  await expect(redoButton()).toBeDisabled();
  await openSolver();
  await type('solver.alpha_u', '0.5');
  await expect.poll(() => value('solver.alpha_u')).toBe(0.5);
  await expect(undoButton()).toHaveText('Undo: set solver.alpha_u');
  await expect(undoButton()).toHaveAccessibleDescription('Ctrl+Z');
  await expect(redoButton()).toHaveAccessibleDescription('Ctrl+Shift+Z or Ctrl+Y');
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  await shot('undo-named-step');
  await undoButton().click();
  await expect.poll(() => value('solver.alpha_u')).toBe(0.9);
  await expect(field('solver.alpha_u')).toHaveValue('0.9');
  await expect(redoButton()).toHaveText('Redo: set solver.alpha_u');
  await redoButton().click();
  await expect.poll(() => value('solver.alpha_u')).toBe(0.5);
  await expect(field('solver.alpha_u')).toHaveValue('0.5');
  // a switch and a select are steps too
  await row('solver.adaptive').getByRole('switch').focus();
  await page.keyboard.press('Space');
  await expect.poll(() => value('solver.adaptive')).toBe(true);
  await expect(undoButton()).toHaveText('Undo: set solver.adaptive');
  await undoButton().click();
  await expect.poll(() => value('solver.adaptive')).toBe(false);
  await expect(row('solver.adaptive').getByRole('switch')).not.toBeChecked();
});

test('Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y undo and redo; inside a field with unsent typing Ctrl+Z is the field’s', async () => {
  await type('solver.max_iterations', '700');
  await expect.poll(() => value('solver.max_iterations')).toBe(700);
  // Enter sent the value: focus is still in the field, and Ctrl+Z now undoes the edit
  await expect(field('solver.max_iterations')).toBeFocused();
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(() => value('solver.max_iterations')).toBe(500);
  await expect(field('solver.max_iterations')).toHaveValue('500');
  await page.keyboard.press('ControlOrMeta+Shift+z');
  await expect.poll(() => value('solver.max_iterations')).toBe(700);
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(() => value('solver.max_iterations')).toBe(500);
  await page.keyboard.press('ControlOrMeta+y');
  await expect.poll(() => value('solver.max_iterations')).toBe(700);
  // typing not yet sent: Ctrl+Z belongs to the field, nothing goes to the engine
  const sent = fake.sets().length;
  await field('solver.max_iterations').focus();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.type('12');
  await page.keyboard.press('ControlOrMeta+z');
  await expect(undoButton()).toHaveText('Undo: set solver.max_iterations');
  expect(fake.sets().length).toBe(sent);
  expect(value('solver.max_iterations')).toBe(700);
  await page.keyboard.press('Escape');
  await expect(field('solver.max_iterations')).toHaveValue('700');
  // from the body as well
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(() => value('solver.max_iterations')).toBe(500);
  await page.keyboard.press('ControlOrMeta+Shift+z');
  await expect.poll(() => value('solver.max_iterations')).toBe(700);
});

test('undo of a value another writer changed: a conflict with both values and three choices, nothing sent', async () => {
  fake.externalSet(session, 'solver.max_iterations', 40);
  await expect(field('solver.max_iterations')).toHaveValue('40');
  const sent = fake.sets().length;
  await undoButton().click();
  const d = dialog();
  await expect(d).toBeVisible();
  await expect(d.getByRole('heading')).toHaveText('Not undone: changed outside the Studio');
  await expect(d).toContainText(`changed what “set solver.max_iterations” left in session ${session}, so nothing was changed`);
  await expect(d.getByRole('row', { name: /solver\.max_iterations/ })).toContainText('700');
  await expect(d.getByRole('row', { name: /solver\.max_iterations/ })).toContainText('40');
  // no choice is preselected: focus is on the title
  await expect(d.getByRole('heading')).toBeFocused();
  await expectPanelRing();
  for (const name of ['Undo anyway, replacing the value now', 'Keep the value now and drop this step', 'Cancel']) await expect(d.getByRole('button', { name })).toBeVisible();
  await shot('undo-conflict');
  expect(fake.sets().length).toBe(sent);
  // Cancel keeps the step; Escape too
  await d.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog()).toHaveCount(0);
  await expect(undoButton()).toHaveText('Undo: set solver.max_iterations');
  await undoButton().click();
  await expect(dialog()).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog()).toHaveCount(0);
  // keep theirs: the step goes, the other writer's 40 stays
  await undoButton().click();
  await dialog().getByRole('button', { name: 'Keep the value now and drop this step' }).click();
  await expect(dialog()).toHaveCount(0);
  expect(value('solver.max_iterations')).toBe(40);
  expect(fake.sets().length).toBe(sent);
  await expect(undoButton()).not.toHaveText('Undo: set solver.max_iterations');
  // undo anyway, from the keyboard
  await type('solver.corrections', '3');
  await expect.poll(() => value('solver.corrections')).toBe(3);
  fake.externalSet(session, 'solver.corrections', 5);
  await expect(field('solver.corrections')).toHaveValue('5');
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press('ControlOrMeta+z');
  await expect(dialog().getByRole('heading')).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(dialog().getByRole('button', { name: 'Undo anyway, replacing the value now' })).toBeFocused();
  await expectFocusRing();
  await page.keyboard.press('Enter');
  await expect.poll(() => value('solver.corrections')).toBe(0);
  await expect(dialog()).toHaveCount(0);
});

test('a typed value overtaken by another writer is not sent: the field shows both and asks which to keep', async () => {
  const sent = fake.sets().length;
  await field('solver.alpha_u').fill('0.3');
  fake.externalSet(session, 'solver.alpha_u', 0.7);
  await field('solver.alpha_u').press('Enter');
  const box = row('solver.alpha_u').getByRole('alert');
  await expect(box).toContainText('Not sent: solver.alpha_u changed outside the Studio while you typed. It is now 0.7 (it was 0.5 when you began).');
  expect(fake.sets().length).toBe(sent);
  await expect(field('solver.alpha_u')).toHaveValue('0.3');
  await shot('typed-value-overtaken');
  await box.getByRole('button', { name: 'Keep 0.7' }).click();
  await expect(field('solver.alpha_u')).toHaveValue('0.7');
  expect(fake.sets().length).toBe(sent);
  // again, and set mine
  await field('solver.alpha_u').fill('0.3');
  fake.externalSet(session, 'solver.alpha_u', 0.6);
  await field('solver.alpha_u').press('Enter');
  await row('solver.alpha_u').getByRole('alert').getByRole('button', { name: 'Set mine: 0.3' }).click();
  await expect.poll(() => value('solver.alpha_u')).toBe(0.3);
  await expect(undoButton()).toHaveText('Undo: set solver.alpha_u');
});

test('undoing the new session asks first and names what is lost; keeping is the default; redo makes it again', async () => {
  // undo down to the new session: "set solver.alpha_u" to 0.3 (back to the other writer's 0.6), then the first one, whose
  // 0.5 the other writer had changed (a conflict: undo anyway)
  await undoButton().click();
  await expect.poll(() => value('solver.alpha_u')).toBe(0.6);
  await undoButton().click();
  await dialog().getByRole('button', { name: 'Undo anyway, replacing the value now' }).click();
  await expect.poll(() => value('solver.alpha_u')).toBe(0.9);
  await expect(undoButton()).toHaveText(`Undo: new session ${session}`);
  // from the keyboard: the panel takes focus, on the safe choice, with the focus ring
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press('ControlOrMeta+z');
  const d = dialog();
  await expect(d.getByRole('heading')).toHaveText(`Undo: new session ${session}?`);
  await expect(d).toContainText(`Session ${session} is removed from the service. Lost: its settings, which exist only in the service.`);
  await expect(d).toContainText('Closing cannot be undone');
  await expect(d.getByRole('button', { name: 'Keep everything as it is' })).toBeFocused();
  await expectPanelRing();
  await shot('undo-new-session-confirm');
  await page.keyboard.press('Enter');
  await expect(dialog()).toHaveCount(0);
  expect(fake.sessions.has(session)).toBe(true);
  await undoButton().click();
  await dialog().getByRole('button', { name: 'Undo and lose it' }).click();
  await expect.poll(() => fake.sessions.has(session)).toBe(false);
  await expect(page.getByRole('heading', { name: `Session ${session} is not on the service` })).toBeVisible();
  await expect(redoButton()).toHaveText(`Redo: new session ${session}`);
  await redoButton().click();
  await expect.poll(() => fake.sessions.size).toBe(1);
  const made = [...fake.sessions.keys()][0]!;
  expect(made).not.toBe(session);
  await expect.poll(() => page.evaluate(() => location.hash)).toBe(`#/settings/${made}`);
  await expect(undoButton()).toHaveText(`Undo: new session ${made}`);
  await expect(redoButton()).toHaveText(/^Redo: set /);
  session = made;
});

test('Close session cannot be undone: it asks first in words that name what is lost, from the keyboard', async () => {
  await openSolver();
  await type('solver.alpha_u', '0.4');
  await expect.poll(() => value('solver.alpha_u')).toBe(0.4);
  await page.getByRole('button', { name: 'Close session' }).click();
  const d = dialog();
  await expect(d.getByRole('heading')).toHaveText(`Close session ${session}?`);
  await expect(d).toContainText(`Session ${session} is removed from the service. Lost: its settings, which exist only in the service (2 changes made here: set solver.alpha_u, new session ${session}).`);
  await expect(d).toContainText('Closing cannot be undone');
  await expect(d.getByRole('button', { name: 'Keep the session' })).toBeFocused();
  await expectPanelRing();
  await shot('close-session-confirm');
  await page.keyboard.press('Escape');
  await expect(dialog()).toHaveCount(0);
  expect(fake.sessions.has(session)).toBe(true);
  await page.getByRole('button', { name: 'Close session' }).focus();
  await page.keyboard.press('Enter');
  await expect(dialog().getByRole('button', { name: 'Keep the session' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog()).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Close session' })).toBeFocused();
  await page.keyboard.press('Enter');
  // the panel opens once run.status has answered, with focus on keeping
  await expect(dialog().getByRole('button', { name: 'Keep the session' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(dialog().getByRole('button', { name: `Close ${session} and lose it` })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect.poll(() => fake.sessions.has(session)).toBe(false);
  await expect.poll(() => page.evaluate(() => location.hash)).toBe('#/settings');
  await expect(page.getByRole('heading', { name: 'Choose a session' })).toBeVisible();
  // the closed session took its history with it
  await expect(undoButton()).toHaveText('Nothing to undo');
});

test('the undo controls are reached with Tab and show the focus ring (G8)', async () => {
  await page.getByRole('button', { name: 'New session from defaults' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Set in this case' })).toBeVisible();
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press('Tab');
    if (await undoButton().evaluate((el) => el === document.activeElement)) break;
  }
  await expect(undoButton()).toBeFocused();
  await expectFocusRing();
});

test('no Content Security Policy violation and no console error', async () => {
  const csp = await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp);
  expect(csp).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
