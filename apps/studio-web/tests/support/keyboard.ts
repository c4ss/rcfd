// Keyboard steps the e2e tests share (F-888, ADR 0786).
//
// When a listbox, a menu or a dialog of the component library closes, the focus goes back to the control that opened it
// in the next animation frame, and only if nothing else took the focus meanwhile (react-aria's FocusScope). Until that
// frame the document's active element is the body. A test types faster than a frame: a Tab sent right after the choice
// starts from the body, lands on the first control of the page, and the focus never returns to the select. So a key that
// relies on the returned focus is sent only after the focus is seen there, as a user sees it.
import { expect, type Locator, type Page } from '@playwright/test';

/**
 * Chooses an option of a Select by keyboard: Enter on the select opens its listbox, Enter on the option chooses it and
 * closes the listbox. Returns once the select shows `shows` and has the focus again (G8: the focus returns to the
 * control that opened the listbox), so the next key starts from the select.
 */
export async function chooseByKeyboard(select: Locator, option: string | RegExp, shows: string | RegExp = option): Promise<void> {
  const page = select.page();
  await select.focus();
  await page.keyboard.press('Enter');
  await page.getByRole('option', { name: option }).focus();
  await page.keyboard.press('Enter');
  await expect(select).toContainText(shows);
  await expect(select).toBeFocused();
}

/**
 * In the page: from now on every requestAnimationFrame callback runs `ms` later than its frame, as on a machine whose
 * frames come slowly (a display that is off, a loaded compositor). `restoreFrames` puts the page's own function back.
 */
export async function delayFrames(page: Page, ms: number): Promise<void> {
  await page.evaluate((delay) => {
    const w = window as unknown as { __ownRaf?: typeof requestAnimationFrame };
    const own = w.__ownRaf ?? window.requestAnimationFrame.bind(window);
    w.__ownRaf = own;
    window.requestAnimationFrame = (cb) => own(() => void setTimeout(() => cb(performance.now()), delay));
  }, ms);
}

export async function restoreFrames(page: Page): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as { __ownRaf?: typeof requestAnimationFrame };
    if (w.__ownRaf) window.requestAnimationFrame = w.__ownRaf;
  });
}
