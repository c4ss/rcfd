// The primitives of F-802 in the packaged app, on the primitives gallery (#/dev/primitives): for each primitive, focus
// is visible, it works from the keyboard alone, and its role and name are present (G8). The rendered page is also
// checked for Content Security Policy violations and console errors, and screenshots at 1440 by 900 are written to test-results-visual/f802/ for the maintainer's view-and-report (G13).
import { _electron as electron, expect, test, type ElectronApplication, type Locator, type Page } from '@playwright/test';
import { mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const SHOTS = path.join(root, 'test-results-visual', 'f802');
const ORANGE = 'rgb(240, 138, 36)';

let app: ElectronApplication;
let page: Page;
const consoleErrors: string[] = [];

test.beforeAll(async () => {
  const { executable } = JSON.parse(readFileSync(path.join(root, 'out', 'packaged.json'), 'utf8')) as { executable: string };
  app = await electron.launch({ executablePath: executable, args: [] });
  page = await app.firstWindow();
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text());
  });
  await expect(page.getByRole('heading', { name: 'No project open' })).toBeVisible();
  await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0]!.setContentSize(1440, 900));
  await expect.poll(() => page.evaluate(() => [innerWidth, innerHeight])).toEqual([1440, 900]);
  await page.evaluate(() => {
    const w = window as unknown as { __csp: string[] };
    w.__csp = [];
    document.addEventListener('securitypolicyviolation', (e) => w.__csp.push(`${e.effectiveDirective} ${e.blockedURI}`));
    location.hash = '#/dev/primitives';
  });
  await expect(page.getByRole('heading', { name: 'Primitives gallery', level: 1 })).toBeVisible();
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
});

test.afterAll(async () => {
  await app?.close();
});

// ---- helpers ---------------------------------------------------------------------------------------------------

/** Moves focus with Tab alone, from the top of the page, until `target` has it (or contains it). */
async function tabTo(target: Locator, max = 150): Promise<void> {
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  for (let i = 0; i < max; i++) {
    await page.keyboard.press('Tab');
    if (await target.evaluate((el) => el === document.activeElement || el.contains(document.activeElement))) return;
  }
  throw new Error('Tab never reached the target');
}

/** The computed focus indicator of the focused element: where it is drawn, its outline and its box shadow. */
const focusRing = () =>
  page.evaluate(() => {
    const a = document.activeElement as HTMLElement;
    let host: Element = a.closest('[data-focus-visible]') ?? a;
    const row = host.matches('.ui-row');
    if (host.matches('.ui-switch')) host = host.querySelector('.ui-switch-track')!;
    if (row) host = host.querySelector('.ui-cell')!;
    if (a.closest('.ui-number-group')) host = a.closest('.ui-number-group')!;
    const cs = getComputedStyle(host);
    return {
      host: host.className,
      row,
      outline: `${cs.outlineStyle} ${cs.outlineWidth} ${cs.outlineColor}`,
      shadow: cs.boxShadow,
      visible: a.matches(':focus-visible') || a.closest('[data-focus-visible]') !== null,
    };
  });

/**
 * The focus of tokens.ts (DESIGN.md): a 2px orange outline, outset or inset. A table row has no box of its own, so its
 * cells draw the 2px orange line as inset shadows.
 */
async function expectFocusRing(): Promise<void> {
  const r = await focusRing();
  expect(r.visible, r.host).toBe(true);
  if (r.row) expect(r.shadow, r.host).toContain(`${ORANGE} 0px 2px 0px 0px inset`);
  else expect(r.outline, r.host).toBe(`solid 2px ${ORANGE}`);
}

const readout = (id: string) => page.getByTestId(id);

/** The first line of the focused element's accessibility snapshot: `- role "name"`. */
const focusedAria = async (): Promise<string> => ((await page.locator(':focus').ariaSnapshot()).split('\n')[0] ?? '').trim();

// ---- view-and-report ---------------------------------------------------------------------------------------------

test('screenshots of the gallery at 1440 by 900 for the view-and-report', async () => {
  mkdirSync(SHOTS, { recursive: true });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: path.join(SHOTS, 'gallery-1440x900.png'), scale: 'css' });
  // The whole page as 1440 by 900 views (the header band and the disclaimer footer are sticky, as in the product, so a
  // single full-page capture would draw them mid-page). Each view overlaps the last by the header and footer.
  const { total, step } = await page.evaluate(() => ({
    total: document.documentElement.scrollHeight,
    step: innerHeight - 48 - 3 - 24 - 12,
  }));
  for (let i = 0, y = 0; y < total - 900 + step && i < 12; i++, y += step) {
    await page.evaluate((top) => window.scrollTo(0, top), y);
    await page.screenshot({ path: path.join(SHOTS, `gallery-1440x900-page-${String(i + 1).padStart(2, '0')}.png`), scale: 'css' });
  }
  await page.evaluate(() => window.scrollTo(0, 0));

  // Keyboard focus on a selected segment and the unavailable option's tooltip.
  await tabTo(page.getByRole('button', { name: 'Sample four' }));
  await expect(page.getByRole('tooltip')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, 'gallery-focus-and-tooltip.png'), scale: 'css' });

  // The select open, with the state pictograms in its options.
  await tabTo(page.getByRole('button', { name: /Sample state filter/ }));
  await page.keyboard.press('Enter');
  await expect(page.getByRole('listbox')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, 'gallery-select-open.png'), scale: 'css' });
  await page.keyboard.press('Escape');
  await expect(page.getByRole('listbox')).toHaveCount(0);

  // The table and pictograms, scrolled into view.
  await page.getByRole('heading', { name: 'Table' }).scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(SHOTS, 'gallery-table-and-pictograms.png'), scale: 'css' });
  await page.evaluate(() => window.scrollTo(0, 0));
});

// ---- each primitive from the keyboard ---------------------------------------------------------------------------

test('buttons: focus visible, Enter and Space press them, names and roles present', async () => {
  const def = page.getByRole('button', { name: 'Default', exact: true });
  await tabTo(def);
  await expectFocusRing();
  await page.keyboard.press('Enter');
  await expect(readout('buttons-readout')).toHaveText('1 press; last: Default');
  await page.keyboard.press('Space');
  await expect(readout('buttons-readout')).toHaveText('2 presses; last: Default');

  for (const name of ['Primary', 'With icon', 'Chip', 'Ghost']) {
    await tabTo(page.getByRole('button', { name, exact: true }));
    await expectFocusRing();
    await page.keyboard.press('Enter');
    await expect(readout('buttons-readout')).toContainText(`last: ${name}`);
  }

  // An icon-only button is named by its label, which its tooltip shows on keyboard focus.
  const iconOnly = page.getByRole('button', { name: 'Icon only' });
  await tabTo(iconOnly);
  await expectFocusRing();
  await expect(page.getByRole('tooltip', { name: 'Icon only' })).toBeVisible();
  await page.keyboard.press('Space');
  await expect(readout('buttons-readout')).toContainText('last: Icon only');

  // A disabled button is out of the tab order.
  await expect(page.getByRole('button', { name: 'Disabled', exact: true })).toBeDisabled();
});

test('segmented toggle: one radio group, arrows move, Space selects; the unavailable option explains itself', async () => {
  const group = page.getByRole('radiogroup', { name: 'Sample view' });
  await expect(group.getByRole('radio')).toHaveCount(3);
  await tabTo(group);
  await expectFocusRing();
  expect(await focusedAria()).toBe('- radio "Sample one" [checked]');
  await page.keyboard.press('ArrowRight');
  expect(await focusedAria()).toBe('- radio "Sample two"');
  await expectFocusRing();
  await page.keyboard.press('Space');
  await expect(group.getByRole('radio', { name: 'Sample two' })).toBeChecked();
  await expect(group.getByRole('radio', { name: 'Sample one' })).not.toBeChecked();
  await expect(readout('segmented-readout')).toContainText('Selected: Sample two');
  // One selection always stays: pressing the selected item again keeps it.
  await page.keyboard.press('Space');
  await expect(group.getByRole('radio', { name: 'Sample two' })).toBeChecked();

  // The unavailable option: focusable, announced as a disabled button, its reason in a tooltip that describes it.
  const four = page.getByRole('button', { name: 'Sample four' });
  await page.keyboard.press('Tab');
  await expect(four).toBeFocused();
  await expectFocusRing();
  await expect(four).toHaveAttribute('aria-disabled', 'true');
  await expect(page.getByRole('tooltip')).toContainText('Unavailable here');
  await expect(four).toHaveAccessibleDescription(/Unavailable here/);
  await page.keyboard.press('Enter');
  await expect(readout('segmented-readout')).toContainText('Selected: Sample two');

  // A single toggle button: Space turns it on and off.
  const toggle = page.getByRole('button', { name: 'Sample toggle' });
  await page.keyboard.press('Tab');
  await expect(toggle).toBeFocused();
  await expectFocusRing();
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Space');
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await expect(readout('segmented-readout')).toContainText('toggle on');
});

test('text and number fields: labelled, described, typed into and stepped from the keyboard', async () => {
  const text = page.getByRole('textbox', { name: 'Sample text' });
  await tabTo(text);
  await expectFocusRing();
  await expect(text).toHaveAccessibleDescription(/Help in place/);
  await page.keyboard.type('abc');
  await expect(readout('fields-readout')).toContainText('Text: abc');
  const caret = await text.evaluate((el) => getComputedStyle(el).caretColor);
  expect(caret).toBe(ORANGE); // DESIGN.md: orange caret

  const num = page.getByRole('textbox', { name: 'Sample length, m' });
  await tabTo(num);
  await expectFocusRing();
  await expect(num).toHaveValue('2.50');
  await page.keyboard.press('ArrowUp');
  await expect(num).toHaveValue('3.00');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(num).toHaveValue('2.00');
  await page.keyboard.press('End');
  await expect(num).toHaveValue('10.00');
  await page.keyboard.press('Home');
  await expect(num).toHaveValue('0.00');
  await expect(readout('fields-readout')).toContainText('length 0.00');
  // The step buttons are named, and kept out of the tab order (the arrows do their work).
  await expect(page.getByRole('button', { name: /^Increase/ })).toHaveAttribute('tabindex', '-1');
  await expect(page.getByRole('button', { name: /^Decrease/ })).toHaveAttribute('tabindex', '-1');

  const invalid = page.getByRole('textbox', { name: 'Sample invalid' });
  await expect(invalid).toHaveAttribute('aria-invalid', 'true');
  await expect(invalid).toHaveAccessibleDescription(/name the problem and how to recover/);
});

test('select: opens from the keyboard, arrows choose, Enter selects, Escape closes, focus returns', async () => {
  const trigger = page.getByRole('button', { name: /Sample state filter/ });
  await tabTo(trigger);
  await expectFocusRing();
  await expect(trigger).toHaveAttribute('aria-haspopup', 'listbox');
  await page.keyboard.press('Enter');
  const list = page.getByRole('listbox');
  await expect(list).toBeVisible();
  await expect(list.getByRole('option')).toHaveCount(6);
  expect(await focusedAria()).toContain('option "All states"');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expectFocusRing();
  expect(await focusedAria()).toContain('option "Running"');
  await page.keyboard.press('Enter');
  await expect(list).toHaveCount(0);
  await expect(trigger).toBeFocused();
  await expect(readout('select-readout')).toHaveText('Filter: Running');

  // Escape closes without changing the value.
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('listbox')).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('listbox')).toHaveCount(0);
  await expect(trigger).toBeFocused();
  await expect(readout('select-readout')).toHaveText('Filter: Running');

  // On the header band; a disabled option is announced and skipped.
  const band = page.getByRole('button', { name: /Sample select on the header band/ });
  await tabTo(band);
  await expectFocusRing();
  await page.keyboard.press('Space');
  await expect(page.getByRole('option', { name: 'Sample C (unavailable)' })).toHaveAttribute('aria-disabled', 'true');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  expect(await focusedAria()).toContain('option "Sample B"');
  await page.keyboard.press('Escape');
});

test('switch: a named switch, Space toggles it, focus drawn on the track; a disabled one is out of reach', async () => {
  const sw = page.getByRole('switch', { name: 'Sample switch', exact: true });
  await tabTo(sw);
  await expectFocusRing();
  expect((await focusRing()).host).toContain('ui-switch-track');
  await expect(sw).not.toBeChecked();
  await page.keyboard.press('Space');
  await expect(sw).toBeChecked();
  await expect(readout('switch-readout')).toHaveText('Switch on');
  await page.keyboard.press('Space');
  await expect(sw).not.toBeChecked();
  await expect(page.getByRole('switch', { name: 'Sample switch, disabled' })).toBeDisabled();
});

test('slider: a named slider, arrows step, Home and End go to the limits, the value is shown', async () => {
  const slider = page.getByRole('slider', { name: 'Sample position, %' });
  await tabTo(slider);
  await expectFocusRing();
  expect((await focusRing()).host).toContain('ui-slider-thumb');
  await expect(slider).toHaveValue('40');
  await page.keyboard.press('ArrowRight');
  await expect(slider).toHaveValue('41');
  await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('ArrowLeft');
  await expect(slider).toHaveValue('39');
  await page.keyboard.press('End');
  await expect(slider).toHaveValue('100');
  await page.keyboard.press('Home');
  await expect(slider).toHaveValue('0');
  await expect(readout('slider-readout')).toHaveText('Position 0');
  await expect(page.locator('.ui-slider-output')).toHaveText('0');
});

test('tabs: sub-tabs and project tabs, arrows select, Home and End, panels named by their tab', async () => {
  const list = page.getByRole('tablist', { name: 'Sample sub-tabs' });
  await tabTo(list);
  await expectFocusRing();
  expect(await focusedAria()).toBe('- tab "Sample tab one" [selected]');
  await page.keyboard.press('ArrowRight');
  await expect(list.getByRole('tab', { name: 'Sample tab two' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('tabpanel', { name: 'Sample tab two' })).toHaveText('The second sample panel.');
  await expectFocusRing();
  await page.keyboard.press('End');
  await expect(list.getByRole('tab', { name: 'Sample tab three' })).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('Home');
  await expect(list.getByRole('tab', { name: 'Sample tab one' })).toHaveAttribute('aria-selected', 'true');
  await expect(readout('tabs-readout')).toHaveText('Tab: Sample tab one');

  const projects = page.getByRole('tablist', { name: 'Sample project tabs' });
  await tabTo(projects);
  await expectFocusRing();
  await page.keyboard.press('ArrowRight');
  await expect(projects.getByRole('tab', { name: 'sample-project-b' })).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('ArrowLeft');
  await expect(projects.getByRole('tab', { name: 'sample-project-a' })).toHaveAttribute('aria-selected', 'true');
});

test('table: one tab stop, arrows move rows, Space selects, Enter on a column head sorts', async () => {
  const grid = page.getByRole('grid', { name: 'Sample rows' });
  await expect(grid.getByRole('columnheader')).toHaveCount(3);
  await expect(grid.getByRole('row')).toHaveCount(6);
  await tabTo(grid);
  await expectFocusRing();
  const focusedRowName = async () => (await focusedAria()).replace(/^- row "([^"]*)".*$/, '$1');
  await page.keyboard.press('ArrowDown');
  await expectFocusRing();
  const second = await focusedRowName();
  await page.keyboard.press('Space');
  await expect(readout('table-readout')).toContainText(`Selected: ${second.split(' ').slice(0, 3).join(' ')}`);
  await expect(grid.getByRole('row', { selected: true })).toHaveCount(1);

  // Up to the column heads, then sort by name, descending.
  for (let i = 0; i < 8 && !(await focusedAria()).startsWith('- columnheader'); i++) await page.keyboard.press('ArrowUp');
  expect(await focusedAria()).toContain('columnheader "Name"');
  await expectFocusRing();
  const name = grid.getByRole('columnheader', { name: 'Name' });
  await expect(name).toHaveAttribute('aria-sort', 'ascending');
  await page.keyboard.press('Enter');
  await expect(name).toHaveAttribute('aria-sort', 'descending');
  await expect(readout('table-readout')).toContainText('sorted by name, descending');
  await expect(grid.getByRole('rowheader').first()).toHaveText('Sample row E');

  // Every state cell is a pictogram plus its word; the failed row keeps its ground and red word.
  const failed = grid.getByRole('row', { name: /Sample row E/ });
  await expect(failed).toContainText('Failed');
  const ground = await failed.locator('td, th').first().evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(ground).toBe('rgb(247, 228, 229)');
});

test('state pictograms and icons: decorative beside their word, never focusable, deep colours on light grounds', async () => {
  const marks = page.locator('.gx-pictos .ui-state');
  await expect(marks).toHaveCount(5 + 5 + 5 + 1 + 5);
  const info = await page.evaluate(() =>
    [...document.querySelectorAll('.gx-pictos .ui-state')].map((s) => {
      const svg = s.querySelector('svg')!;
      return {
        state: svg.getAttribute('data-state'),
        word: s.textContent,
        hidden: svg.getAttribute('aria-hidden'),
        focusable: svg.getAttribute('focusable'),
        tabIndex: (svg as unknown as HTMLElement).tabIndex,
        stroke: getComputedStyle(svg).color,
        opacity: getComputedStyle(svg).opacity,
        faded: svg.classList.contains('ui-picto--faded'),
      };
    }),
  );
  const word: Record<string, string> = { queued: 'Queued', running: 'Running', converged: 'Converged', finished: 'Finished', failed: 'Failed' };
  for (const i of info) {
    expect(i.word).toBe(word[i.state!]);
    expect(i.hidden).toBe('true');
    expect(i.focusable).toBe('false');
    expect(i.tabIndex).toBe(-1);
  }
  const find = (state: string, faded: boolean) => info.find((i) => i.state === state && i.faded === faded);
  expect(find('running', false)?.stroke).toBe('rgb(179, 90, 0)'); // orange-deep (design.json, State Pictogram)
  expect(find('converged', false)?.stroke).toBe('rgb(47, 125, 59)'); // meadow-deep
  expect(find('failed', false)?.stroke).toBe('rgb(155, 28, 36)');
  expect(find('running', false)?.opacity).toBe('1');
  expect(find('running', true)?.stroke).toBe('rgb(179, 90, 0)'); // faded: same colour, half-filled
  expect(find('running', true)?.opacity).toBe('0.5');
  expect(info.filter((i) => i.state === 'failed').every((i) => !i.faded)).toBe(true); // a failure never fades

  const icons = page.locator('.gx-icons svg');
  expect(await icons.count()).toBeGreaterThan(10);
  expect(await icons.evaluateAll((els) => els.every((e) => e.getAttribute('aria-hidden') === 'true'))).toBe(true);
});

// ---- the page as a whole -----------------------------------------------------------------------------------------

test('Tab reaches every control once, each with a role, a name and a visible focus ring, with no trap', async () => {
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  const seen: string[] = [];
  for (let i = 0; i < 120; i++) {
    await page.keyboard.press('Tab');
    const id = await page.evaluate(() => {
      const w = window as unknown as { __walk?: WeakMap<Element, number>; __n?: number };
      w.__walk ??= new WeakMap();
      w.__n ??= 0;
      const a = document.activeElement;
      if (!a || a === document.body) return -1;
      if (w.__walk.has(a)) return w.__walk.get(a)!;
      w.__walk.set(a, w.__n);
      return w.__n++;
    });
    if (id === -1) continue;
    const aria = await focusedAria();
    if (seen.includes(aria) && id < seen.length) break;
    expect(aria, `stop ${seen.length}`).toMatch(/^- [a-z]+ "[^"]+"/);
    await expectFocusRing();
    seen.push(aria);
  }
  // The header's project tabs and ghost button, then every primitive section in order.
  const roles = seen.map((s) => s.replace(/^- ([a-z]+) .*$/, '$1'));
  for (const r of ['tab', 'button', 'radio', 'textbox', 'switch', 'slider']) expect(roles, r).toContain(r);
  // The table is one tab stop; it returns focus to the row, cell or column head focused last.
  expect(roles.filter((r) => ['row', 'gridcell', 'rowheader', 'columnheader'].includes(r)), seen.join(' | ')).toHaveLength(1);
  expect(seen.length).toBeGreaterThanOrEqual(20);
  expect(seen.length).toBeLessThan(60);
});

test('the gallery raised no Content Security Policy violation and no console error', async () => {
  const csp = await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp);
  expect(csp).toEqual([]);
  expect(consoleErrors).toEqual([]);
  // The pressable rule React Aria would inject is the stylesheet's, through the marker.
  expect(await page.evaluate(() => document.querySelectorAll('style').length)).toBe(0);
  expect(await page.evaluate(() => document.getElementById('react-aria-pressable-style')?.tagName)).toBe('META');
});

test('the gallery is a route of the one page: leaving the hash returns to the project view', async () => {
  await page.evaluate(() => {
    location.hash = '';
  });
  await expect(page.getByRole('heading', { name: 'No project open' })).toBeVisible();
  expect(page.url()).toMatch(/^app:\/\/studio\/index\.html#?$/);
});
