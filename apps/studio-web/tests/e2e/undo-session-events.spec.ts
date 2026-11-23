// F-852 (ADR 0781): the session Redo makes again is shown although its `created` event reaches the page late.
//
// The cause of the time-out of undo.spec.ts:268 on Windows in CI: the settings page learned of the session Redo made
// only by the service's event `created`, and it replaced its events listener when the session in the hash changed. An
// event batch the main process had posted to the old subscription arrived after the page had dropped that listener and
// was lost; the page stayed on "Session … is not on the service" and the next test waited 30 s for the section list.
//
// The test makes that lateness certain instead of leaving it to the machine: every `events` message of the main process
// is posted DELAY_MS late (the other messages at once), through Playwright's evaluation in Electron's main process. No
// source file knows of it. Against the fake service (tests/support/fake-case-service.ts).
import { expect, test, type Locator } from '@playwright/test';
import { FakeCaseService } from '../support/fake-case-service.js';
import { launchStudio, type Studio } from '../support/studio.js';

/** ADR 0781: about ten renders of the page, a seventeenth of the 5 s an expectation waits. */
const DELAY_MS = 300;

let fake: FakeCaseService;
let studio: Studio;

test.beforeAll(async () => {
  fake = await FakeCaseService.start();
  studio = await launchStudio({ RCFD_STUDIO_SERVICE: fake.address });
});

test.afterAll(async () => {
  await studio?.app.close();
  await fake?.stop();
});

/** In the main process: every message of type `events` leaves a message port `ms` late; the count is kept. */
async function delayEvents(ms: number): Promise<void> {
  await studio.app.evaluate(({ MessageChannelMain }, delay) => {
    const g = globalThis as unknown as { __f852Delayed?: number };
    g.__f852Delayed = 0;
    const channel = new MessageChannelMain();
    const proto = Object.getPrototypeOf(channel.port1) as { postMessage: (...a: unknown[]) => void };
    channel.port1.close();
    channel.port2.close();
    const post = proto.postMessage;
    // the arguments are passed on as they came: the port refuses a transfer list that is not an array, undefined too
    proto.postMessage = function (this: unknown, ...a: unknown[]): void {
      const m = a[0];
      if (typeof m === 'object' && m !== null && (m as { type?: unknown }).type === 'events') {
        g.__f852Delayed = (g.__f852Delayed ?? 0) + 1;
        setTimeout(() => {
          try {
            post.apply(this, a);
          } catch {
            // the port closed meanwhile
          }
        }, delay);
        return;
      }
      post.apply(this, a);
    };
  }, ms);
}

test('Redo of "new session": the session made again is shown when its created event reaches the page late', async () => {
  const page = studio.page;
  const group = (): Locator => page.getByRole('group', { name: /^Undo and redo/ });
  const undoButton = (): Locator => group().getByRole('button').first();
  const redoButton = (): Locator => group().getByRole('button').nth(1);
  const settingsShown = (): Locator => page.getByRole('heading', { level: 2, name: 'Set in this case' });

  await page.evaluate(() => (location.hash = '#/settings'));
  await page.getByRole('button', { name: 'New session from defaults' }).click();
  await expect(settingsShown()).toBeVisible();
  const first = [...fake.sessions.keys()][0]!;

  // undo of the new session closes it; Redo then offers to make it again
  await expect(undoButton()).toHaveText(`Undo: new session ${first}`);
  await undoButton().click();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Undo and lose it' }).click();
  await expect.poll(() => fake.sessions.has(first)).toBe(false);
  await expect(page.getByRole('heading', { name: `Session ${first} is not on the service` })).toBeVisible();
  await expect(redoButton()).toHaveText(`Redo: new session ${first}`);

  await delayEvents(DELAY_MS);
  await redoButton().click();
  await expect.poll(() => fake.sessions.size).toBe(1);
  const made = [...fake.sessions.keys()][0]!;
  expect(made).not.toBe(first);
  await expect.poll(() => page.evaluate(() => location.hash)).toBe(`#/settings/${made}`);

  // the page shows the session made again: its settings and its sections
  await expect(settingsShown()).toBeVisible();
  await expect(page.getByRole('listbox', { name: 'Sections of the case' }).getByRole('option', { name: /^solver\b/ })).toBeVisible();
  // it read the service's sessions after the session was made, and the event was one the hook held back
  const calls = fake.received.map((r) => r.method);
  expect(calls.lastIndexOf('session.list')).toBeGreaterThan(calls.lastIndexOf('session.create'));
  const delayed = await studio.app.evaluate(() => (globalThis as unknown as { __f852Delayed?: number }).__f852Delayed ?? 0);
  expect(delayed).toBeGreaterThan(0);
});
