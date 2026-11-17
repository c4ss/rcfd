// F-816 (G8): the keyboard and assistive-technology checks of the packaged Studio, from the DOM, the keyboard and the
// browser's own accessibility tree (Chromium's, through the DevTools protocol). No checker library (G5): the rules are
// written here, each one stated where it is checked.
//
// - The Tab walk: Tab (or Shift+Tab) pressed until the focus comes back to where it started. Each stop is recorded with
//   the focus indicator it shows: the outline, box shadow or border that the element (or the part that draws its ring:
//   a switch's track, a table row's cells, the scrubber's line) has while focused and not once the focus has moved on.
//   That is a check that an indicator is drawn, not of its colour or contrast (the maintainer removed the contrast
//   requirement on 2026-11-16).
// - Composite widgets (a list box, a grid, a tab list, a radio group) are one Tab stop; their members are reached with
//   the arrow keys (the ARIA pattern React Aria implements). `arrowWalk` checks that.
// - Reachable: every interactive element of a surface is a Tab stop, or a member of a composite that is one.
// - Pointer-only targets: an element with a React click or pointer handler that no key reaches (React keeps an element's
//   props on the element; the check reads them, so a `div` with `onClick` and no keyboard path is found).
// - Names and roles: every node of the accessibility tree with an interactive role has a non-empty accessible name, and
//   every Tab stop has a role that is not generic.
// - Motion: the CSS animations and transitions declared on every element (and its ::before and ::after), and the
//   document's running animations.
import type { CDPSession, Page } from '@playwright/test';

/** One element the walk saw: its identity in the page (`id`) and what the checks need. */
export interface Stop {
  /** Index of the element in the page's list (stable while the element stays in the document). */
  readonly id: number;
  /** The identity the order is compared by: a composite's container for its members, else the element. */
  readonly key: number;
  /** tag.class[role] "name", for messages. */
  readonly desc: string;
  readonly role: string;
  /** The composite container the element is a member of (its id), or null. */
  readonly composite: number | null;
  readonly compositeRole: string | null;
  /** Inside a dialog (role dialog or alertdialog). */
  readonly inModal: boolean;
  /** The focus indicator: how it is drawn and on what, or why none was found. */
  indicator: Indicator | null;
}

export interface Indicator {
  readonly ok: boolean;
  /** outline, box-shadow or border, on which element ("self", "ancestor 1", "descendant") */
  readonly how: string;
  readonly host: string;
  /** The indicator's box is on the screen (non-empty and inside the window). */
  readonly visible: boolean;
}

export interface Walk {
  readonly stops: Stop[];
  /** The walk came back to its first stop. */
  readonly closed: boolean;
  /** Focus came back to an earlier stop that was not the first: the stops of that loop. */
  readonly loop: string[] | null;
  /** Steps where the focus was on the document itself (between the last stop and the first). */
  readonly bodySteps: number;
}

/** Installs the page side of the checks (once per document; a hash change keeps it). */
export async function installA11y(page: Page): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as { __a11y?: unknown };
    if (w.__a11y) return;
    const MEMBER_ROLES = new Set(['option', 'tab', 'radio', 'row', 'gridcell', 'columnheader', 'rowheader', 'menuitem', 'menuitemradio', 'menuitemcheckbox', 'treeitem']);
    const COMPOSITE = '[role=listbox],[role=grid],[role=treegrid],[role=tablist],[role=radiogroup],[role=menu],[role=menubar],[role=tree],[role=toolbar]';
    const INTERACTIVE_ROLES = new Set([
      'button',
      'link',
      'checkbox',
      'radio',
      'switch',
      'tab',
      'option',
      'menuitem',
      'menuitemcheckbox',
      'menuitemradio',
      'slider',
      'spinbutton',
      'textbox',
      'searchbox',
      'combobox',
      'treeitem',
    ]);
    const els: Element[] = [];
    const ids = new WeakMap<Element, number>();
    const cands = new Map<number, Element[]>();
    const id = (el: Element): number => {
      let i = ids.get(el);
      if (i === undefined) {
        i = els.length;
        els.push(el);
        ids.set(el, i);
      }
      return i;
    };
    const roleOf = (el: Element): string => {
      const r = el.getAttribute('role');
      if (r) return r;
      const t = el.tagName.toLowerCase();
      if (t === 'input') {
        const ty = (el as HTMLInputElement).type;
        return ty === 'checkbox' ? 'checkbox' : ty === 'radio' ? 'radio' : ty === 'range' ? 'slider' : ty === 'number' ? 'spinbutton' : 'textbox';
      }
      if (t === 'a' && el.hasAttribute('href')) return 'link';
      if (t === 'textarea') return 'textbox';
      if (t === 'select') return 'combobox';
      return t;
    };
    const desc = (el: Element): string => {
      const cls = typeof el.className === 'string' ? el.className.trim().split(/\s+/).slice(0, 2).join('.') : '';
      const name = el.getAttribute('aria-label') ?? (el.textContent ?? '').trim().replace(/\s+/g, ' ').slice(0, 50);
      return `${el.tagName.toLowerCase()}${cls ? '.' + cls : ''}[${roleOf(el)}] "${name}"`;
    };
    /** An identity that survives a re-render: React Aria's data-key, else the element's id, else its description. */
    const dkey = (el: Element): string => el.getAttribute('data-key') ?? (el.id || desc(el));
    /** The element focus is on for the user: the active element, or the option its aria-activedescendant names. */
    const target = (): Element | null => {
      const a = document.activeElement;
      if (!a || a === document.body || a === document.documentElement) return null;
      const ad = a.getAttribute('aria-activedescendant');
      if (ad) {
        const t = document.getElementById(ad);
        if (t) return t;
      }
      return a;
    };
    const compositeOf = (el: Element): Element | null => {
      const r = roleOf(el);
      if (r === 'button') {
        // a toolbar with a roving Tab stop (its other buttons out of the Tab order) is a composite too
        const t = el.parentElement?.closest('[role=toolbar]') ?? null;
        return t && t.querySelector('button[tabindex="-1"]') ? t : null;
      }
      if (!MEMBER_ROLES.has(r)) return null;
      return el.parentElement?.closest(COMPOSITE) ?? null;
    };
    /** The elements that may draw an element's focus ring: itself, three ancestors and what is inside them. */
    const candidatesOf = (el: Element): Element[] => {
      const out = new Set<Element>([el]);
      for (const d of Array.from(el.querySelectorAll('*')).slice(0, 60)) out.add(d);
      let a: Element | null = el.parentElement;
      for (let k = 0; k < 3 && a && a !== document.body; k++, a = a.parentElement) {
        out.add(a);
        if (k < 2) for (const d of Array.from(a.querySelectorAll('*')).slice(0, 60)) out.add(d);
      }
      return [...out];
    };
    const look = (c: Element, pseudo: string | null): string => {
      const s = getComputedStyle(c, pseudo);
      const out = s.outlineStyle === 'none' || parseFloat(s.outlineWidth) === 0 ? '' : `outline ${s.outlineStyle} ${s.outlineWidth} ${s.outlineColor}`;
      const sh = s.boxShadow === 'none' ? '' : `box-shadow ${s.boxShadow}`;
      const b = ['Top', 'Right', 'Bottom', 'Left']
        .map((side) => {
          const st = s.getPropertyValue(`border-${side.toLowerCase()}-style`);
          const wd = s.getPropertyValue(`border-${side.toLowerCase()}-width`);
          return st === 'none' || parseFloat(wd) === 0 ? '' : `${side} ${st} ${wd} ${s.getPropertyValue(`border-${side.toLowerCase()}-color`)}`;
        })
        .filter(Boolean)
        .join(',');
      return [out, sh, b ? `border ${b}` : ''].join('|');
    };
    const sigOf = (list: Element[]): string[] =>
      list.flatMap((c) => (c.isConnected ? [look(c, null), look(c, '::before'), look(c, '::after')] : ['gone', 'gone', 'gone']));
    const onScreen = (c: Element): boolean => {
      const r = c.getBoundingClientRect();
      const vis = (c as HTMLElement).checkVisibility?.({ visibilityProperty: true } as CheckVisibilityOptions) ?? true;
      return vis && r.width > 0 && r.height > 0 && r.right > 0 && r.bottom > 0 && r.left < innerWidth && r.top < innerHeight;
    };
    const labelOf = (list: Element[], i: number, el: Element): string => {
      const c = list[i]!;
      if (c === el) return 'self';
      if (c.contains(el)) return 'ancestor';
      if (el.contains(c)) return 'descendant';
      return 'near';
    };
    /** The indicator: what changed between the focused and the unfocused look, and is an outline, a shadow or a border. */
    const indicator = (elId: number, focused: string[]) => {
      const el = els[elId]!;
      const list = cands.get(elId) ?? [];
      const now = sigOf(list);
      const kinds: string[] = [];
      // still focused (the only stop of a dialog): the indicator is what it draws now, on itself or what holds it
      if (target() === el) {
        for (let i = 0; i < now.length; i++) {
          const ci = Math.floor(i / 3);
          const c = list[ci]!;
          if (c !== el && !c.contains(el) && !el.contains(c)) continue;
          const [o, sh] = now[i]!.split('|');
          if (o || sh) kinds.push(`${labelOf(list, ci, el)} ${o ? 'outline' : 'box-shadow'} (still focused)|${desc(c)}|${onScreen(c) ? 1 : 0}`);
        }
        return kinds;
      }
      for (let i = 0; i < focused.length; i++) {
        if (focused[i] === now[i] || now[i] === 'gone') continue;
        const [o, s, b] = focused[i]!.split('|');
        const [o0, s0, b0] = (now[i] ?? '||').split('|');
        const ci = Math.floor(i / 3);
        const pseudo = ['', '::before', '::after'][i % 3];
        const host = list[ci]!;
        const where = `${labelOf(list, ci, el)}${pseudo}`;
        if (o && o !== o0) kinds.push(`${where} outline|${desc(host)}|${onScreen(host) ? 1 : 0}`);
        else if (s && s !== s0) kinds.push(`${where} box-shadow|${desc(host)}|${onScreen(host) ? 1 : 0}`);
        else if (b && b !== b0) kinds.push(`${where} border|${desc(host)}|${onScreen(host) ? 1 : 0}`);
      }
      return kinds;
    };
    /** Everything interactive inside `root` that a user could operate (shown, not disabled, not hidden from readers). */
    const interactive = (root: string, exclude: string[]) => {
      const r = document.querySelector(root);
      if (!r) return null;
      const out: { id: number; desc: string; composite: number | null; stepperOf: number | null }[] = [];
      for (const el of Array.from(r.querySelectorAll('*'))) {
        if (exclude.some((s) => el.closest(s) && el.closest(s) !== r && r.contains(el.closest(s)))) continue;
        const native = el.matches('button, a[href], input:not([type=hidden]), select, textarea, summary, [contenteditable=""], [contenteditable=true]');
        const role = el.getAttribute('role');
        const ti = el.getAttribute('tabindex');
        const isIt = native || (role !== null && INTERACTIVE_ROLES.has(role)) || (ti !== null && Number(ti) >= 0) || ((role === 'row' || role === 'gridcell') && ti !== null);
        if (!isIt) continue;
        if (el.closest('[aria-hidden=true], [inert]')) continue;
        if (el.matches(':disabled') || el.getAttribute('aria-disabled') === 'true' || el.closest('[data-disabled]') === el) continue;
        // React Aria's DismissButton: a hidden button at each end of a popover for screen readers' virtual cursor, out of
        // the Tab order on purpose; its key is Escape, which the dialog check presses
        if (el.matches('button[tabindex="-1"][aria-label="Dismiss"]') && el.closest('[role=dialog], [role=alertdialog], .ui-popover, [data-trigger]')) continue;
        // a visually hidden input (a switch's, a radio's) is drawn by its label; it counts when its label is shown
        const shown = onScreen(el) || (el.matches('input') && el.closest('label') !== null && onScreen(el.closest('label')!));
        if (!shown) {
          // in a scroll box, below the fold: still reachable, so counted
          const cs = getComputedStyle(el);
          if (cs.display === 'none' || cs.visibility === 'hidden' || !(el as HTMLElement).checkVisibility?.()) continue;
        }
        const group = el.closest('.ui-number-group');
        const stepper = group && el.matches('button[slot=increment], button[slot=decrement]') ? group.querySelector('input') : null;
        out.push({ id: id(el), desc: desc(el), composite: compositeOf(el) ? id(compositeOf(el)!) : null, stepperOf: stepper ? id(stepper) : null });
      }
      return out;
    };
    /** Elements with a React click or pointer handler that neither they, an ancestor nor a descendant make keyboard reachable. */
    const pointerOnly = (root: string) => {
      const r = document.querySelector(root);
      if (!r) return [];
      const out: string[] = [];
      const focusable = (e: Element) => e.matches('button, a[href], input, select, textarea, [tabindex]') && !e.matches(':disabled');
      for (const el of [r, ...Array.from(r.querySelectorAll('*'))]) {
        const key = Object.keys(el).find((k) => k.startsWith('__reactProps$'));
        if (!key) continue;
        const props = (el as unknown as Record<string, Record<string, unknown>>)[key]!;
        const handlers = ['onClick', 'onDoubleClick', 'onMouseDown', 'onPointerDown', 'onPointerUp'].filter((h) => typeof props[h] === 'function');
        if (handlers.length === 0) continue;
        if (!onScreen(el)) continue;
        // a disabled control does nothing on a click either
        if (el.matches(':disabled') || el.getAttribute('aria-disabled') === 'true') continue;
        let a: Element | null = el;
        let ok = false;
        for (; a; a = a.parentElement) if (focusable(a)) ok = true;
        if (!ok && el.querySelector('button, a[href], input, select, textarea, [tabindex]')) ok = true;
        if (!ok) out.push(`${desc(el)} (${handlers.join(', ')})`);
      }
      return out;
    };
    const members = (compId: number): { key: string; desc: string }[] => {
      const c = els[compId]!;
      const role = roleOf(c);
      const sel = role === 'grid' || role === 'treegrid' ? '[role=row][tabindex]' : role === 'radiogroup' ? 'input[type=radio], [role=radio]' : role === 'toolbar' ? 'button' : '[role=option], [role=tab], [role=menuitem], [role=treeitem], [role=menuitemradio], [role=menuitemcheckbox]';
      return Array.from(c.querySelectorAll(sel))
        .filter((m) => m.parentElement?.closest(COMPOSITE) === c || (role === 'radiogroup' && m.closest(COMPOSITE) === c))
        .filter((m) => !(m as HTMLInputElement).disabled && m.getAttribute('aria-disabled') !== 'true' && !m.closest('[aria-hidden=true]'))
        .filter((m) => (m as HTMLElement).checkVisibility?.() ?? true)
        .map((m) => ({ key: dkey(m), desc: desc(m) }));
    };
    const info = () => {
      const el = target();
      if (!el) return null;
      const i = id(el);
      // taken afresh at each focus: what draws the ring may have been rendered again since the last one
      cands.set(i, candidatesOf(el));
      // a combobox's list (the palette's search field: virtual focus) is the composite the field stands for
      const controlled = el.getAttribute('aria-controls');
      const listOf = controlled && /^(combobox|searchbox|textbox)$/.test(roleOf(el)) ? document.getElementById(controlled.split(' ')[0]!) : null;
      const comp = compositeOf(el) ?? (listOf && listOf.matches(COMPOSITE) ? listOf : null);
      const cid = comp ? id(comp) : null;
      return {
        id: i,
        key: cid ?? i,
        desc: desc(el),
        role: roleOf(el),
        composite: cid,
        compositeRole: comp ? roleOf(comp) : null,
        dkey: dkey(el),
        ckey: comp ? desc(comp) : null,
        inModal: el.closest('[role=dialog], [role=alertdialog]') !== null,
        sig: sigOf(cands.get(i)!),
        selected: el.getAttribute('aria-selected') === 'true' || el.getAttribute('aria-checked') === 'true' || (el as HTMLInputElement).checked === true,
      };
    };
    w.__a11y = { els, id, info, indicator, interactive, pointerOnly, members, desc, roleOf };
  });
}

type PageInfo = Omit<Stop, 'indicator'> & { sig: string[]; selected: boolean; dkey: string; ckey: string | null };

const pageInfo = (page: Page): Promise<PageInfo | null> =>
  page.evaluate(() => (window as unknown as { __a11y: { info(): PageInfo | null } }).__a11y.info());

function toIndicator(kinds: string[]): Indicator {
  if (kinds.length === 0) return { ok: false, how: 'no outline, box shadow or border changes with the focus', host: '', visible: false };
  // prefer one on the screen
  const best = kinds.find((k) => k.endsWith('|1')) ?? kinds[0]!;
  const [how, host, vis] = best.split('|');
  return { ok: vis === '1', how: how!, host: host!, visible: vis === '1' };
}

/** The indicator of `stop` (focused when `focusedSig` was taken), now that the focus has left it. */
async function settle(page: Page, stop: Stop, focusedSig: string[]): Promise<void> {
  const kinds = await page.evaluate(
    ({ id, sig }) => (window as unknown as { __a11y: { indicator(id: number, s: string[]): string[] } }).__a11y.indicator(id, sig),
    { id: stop.id, sig: focusedSig },
  );
  stop.indicator = toIndicator(kinds);
}

/**
 * Presses Tab (or Shift+Tab) until the focus is back on the first stop, or comes back to another one (a loop that is a
 * trap unless it is a dialog's), or `max` presses. `onStop` runs while each stop has the focus (for screenshots).
 */
export async function tabWalk(page: Page, opts: { reverse?: boolean; max?: number; onStop?: (s: Stop) => Promise<void> } = {}): Promise<Walk> {
  await installA11y(page);
  const key = opts.reverse ? 'Shift+Tab' : 'Tab';
  const max = opts.max ?? 600;
  const stops: Stop[] = [];
  const sigs = new Map<number, string[]>();
  let prev: Stop | null = null;
  let bodySteps = 0;
  let bodyRun = 0;
  let closed = false;
  let loop: string[] | null = null;
  for (let i = 0; i < max; i++) {
    await page.keyboard.press(key);
    const cur = await pageInfo(page);
    if (prev) await settle(page, prev, sigs.get(prev.id)!);
    if (cur === null) {
      bodySteps++;
      if (++bodyRun > 3) break;
      prev = null;
      continue;
    }
    bodyRun = 0;
    if (stops.length > 0 && cur.key === stops[0]!.key) {
      closed = true;
      break;
    }
    const seen = stops.findIndex((s) => s.key === cur.key);
    if (seen >= 0) {
      loop = stops.slice(seen).map((s) => s.desc);
      break;
    }
    const { sig, selected: _selected, dkey: _dkey, ckey: _ckey, ...rest } = cur;
    const stop: Stop = { ...rest, indicator: null };
    sigs.set(stop.id, sig);
    stops.push(stop);
    if (opts.onStop) await opts.onStop(stop);
    prev = stop;
  }
  // the last stop's indicator, once the focus has moved on from it
  if (prev && prev.indicator === null) {
    await page.keyboard.press(key);
    await settle(page, prev, sigs.get(prev.id)!);
    await page.keyboard.press(opts.reverse ? 'Tab' : 'Shift+Tab');
  }
  return { stops, closed, loop, bodySteps };
}

/** The keys of a walk as a cycle starting at its smallest key (forward order). */
export function cycleOf(keys: number[]): number[] {
  if (keys.length === 0) return [];
  const m = keys.indexOf(Math.min(...keys));
  return [...keys.slice(m), ...keys.slice(0, m)];
}

export interface Reach {
  readonly found: boolean;
  readonly unreached: string[];
  readonly count: number;
}

/** Whether every interactive element in `root` was a stop of `walk`, or a member of a composite that was. */
export async function reach(page: Page, walk: Walk, root: string, exclude: string[] = []): Promise<Reach> {
  await installA11y(page);
  const list = await page.evaluate(
    ({ root, exclude }) =>
      (
        window as unknown as {
          __a11y: { interactive(r: string, x: string[]): { id: number; desc: string; composite: number | null; stepperOf: number | null }[] | null };
        }
      ).__a11y.interactive(root, exclude),
    { root, exclude },
  );
  if (list === null) return { found: false, unreached: [], count: 0 };
  const ids = new Set(walk.stops.map((s) => s.id));
  const comps = new Set(walk.stops.map((s) => s.composite).filter((c): c is number => c !== null));
  // a composite whose container itself is the stop (a list box that holds the focus with aria-activedescendant)
  for (const s of walk.stops) comps.add(s.id);
  const unreached = list
    .filter((e) => !ids.has(e.id) && !(e.composite !== null && comps.has(e.composite)) && !(e.stepperOf !== null && ids.has(e.stepperOf)))
    .map((e) => e.desc);
  return { found: true, unreached, count: list.length };
}

/** Elements with a click or pointer handler and no keyboard path, inside `root`. */
export async function pointerOnly(page: Page, root: string): Promise<string[]> {
  await installA11y(page);
  return page.evaluate((r) => (window as unknown as { __a11y: { pointerOnly(r: string): string[] } }).__a11y.pointerOnly(r), root);
}

export interface ArrowResult {
  readonly composite: string;
  readonly members: number;
  readonly reached: number;
  readonly missed: string[];
  /** The focus is back on the member it started on (where a selection follows the focus, that member is selected). */
  readonly restored: boolean;
  readonly start: string;
  readonly end: string | null;
  readonly trace?: string[];
}

const HORIZONTAL = new Set(['tablist', 'radiogroup', 'menubar']);

/**
 * From a member of a composite (focused), the arrow keys reach every member: back to the first, then forward to the
 * last, then back to where it started (so a composite whose selection follows the focus, tabs and radio groups, ends as
 * it was).
 */
export async function arrowWalk(page: Page, stop: Stop): Promise<ArrowResult> {
  const lib = (fn: string, arg: number) =>
    page.evaluate(({ fn, arg }) => (window as unknown as { __a11y: Record<string, (a: number) => unknown> }).__a11y[fn]!(arg), { fn, arg });
  const comp = stop.composite!;
  const all = (await lib('members', comp)) as { key: string; desc: string }[];
  const compDesc = (await page.evaluate((c) => {
    const a = (window as unknown as { __a11y: { els: Element[]; desc(e: Element): string } }).__a11y;
    return a.desc(a.els[c]!);
  }, comp)) as string;
  const horizontal = HORIZONTAL.has(stop.compositeRole ?? '') || (await page.evaluate((c) => (window as unknown as { __a11y: { els: Element[] } }).__a11y.els[c]!.getAttribute('aria-orientation') === 'horizontal', comp));
  const back = horizontal ? 'ArrowLeft' : 'ArrowUp';
  const fwd = horizontal ? 'ArrowRight' : 'ArrowDown';
  const start = await pageInfo(page);
  if (!start || start.ckey !== compDesc) return { composite: compDesc, members: all.length, reached: 0, missed: ['the focus is not in the composite'], restored: false, start: start?.desc ?? '', end: null };
  // members are compared by React Aria's data-key: a selection that follows the focus may render them again
  const inComp = (c: PageInfo | null): c is PageInfo => c !== null && c.ckey === compDesc;
  const seen = new Set<string>([start.dkey]);
  const trace: string[] = [`start ${start.dkey}`];
  // a key's effect is read once it shows (a loaded machine renders the next member late); at an end nothing changes
  const press = async (k: string) => {
    const was = (await pageInfo(page))?.dkey;
    await page.keyboard.press(k);
    let c = await pageInfo(page);
    for (let i = 0; i < 10 && c?.dkey === was; i++) {
      await page.waitForTimeout(50);
      c = await pageInfo(page);
    }
    trace.push(`${k} ${c?.dkey ?? 'none'}${c && c.ckey !== compDesc ? ' (outside)' : ''}`);
    return c;
  };
  const limit = Math.min(all.length, 80) + 2;
  const want = all.slice(0, 80);
  const done = () => want.every((m) => seen.has(m.key));
  // back towards the first (a composite that wraps comes round to the start)
  let at = start.dkey;
  for (let i = 0; i < limit && !done(); i++) {
    const c = await press(back);
    if (!inComp(c) || c.dkey === at || c.dkey === start.dkey) break;
    seen.add(c.dkey);
    at = c.dkey;
  }
  // forward to the last
  for (let i = 0; i < limit * 2 && !done(); i++) {
    const c = await press(fwd);
    if (!inComp(c) || c.dkey === at) break;
    seen.add(c.dkey);
    at = c.dkey;
  }
  // back to the start: towards the last first (the walk may have ended before the start), then towards the first
  for (const k of [fwd, back]) {
    let here = (await pageInfo(page))?.dkey ?? '';
    for (let i = 0; i < limit * 2 && here !== start.dkey; i++) {
      const c = await press(k);
      if (!inComp(c) || c.dkey === here) break;
      here = c.dkey;
    }
    if (here === start.dkey) break;
  }
  const end = await pageInfo(page);
  return {
    composite: compDesc,
    members: all.length,
    reached: seen.size,
    missed: want.filter((m) => !seen.has(m.key)).map((m) => m.desc),
    // a field with virtual focus over its list ends anywhere in the list
    restored: end !== null && (end.dkey === start.dkey || (!all.some((m) => m.key === start.dkey) && end.ckey === compDesc)),
    start: start.desc,
    end: end?.desc ?? null,
    trace,
  };
}

// ---- names and roles from the accessibility tree --------------------------------------------------------------------

/** The roles whose nodes must have a non-empty accessible name. */
export const NAMED_ROLES = new Set([
  'button',
  'link',
  'textbox',
  'searchbox',
  'spinbutton',
  'slider',
  'tab',
  'option',
  'checkbox',
  'radio',
  'switch',
  'combobox',
  'menuitem',
  'menuitemcheckbox',
  'menuitemradio',
  'treeitem',
  'listbox',
  'grid',
  'treegrid',
  'tablist',
  'radiogroup',
  'dialog',
  'alertdialog',
  'progressbar',
  'img',
  'tabpanel',
  'meter',
  'tree',
  'menu',
  'toolbar',
]);

/** Roles a Tab stop must not have: the element would be announced as nothing. */
export const NO_ROLE = new Set(['generic', 'none', 'presentation', 'StaticText', 'GenericContainer', 'paragraph', 'Section', 'div', 'span', '']);

interface AXValue {
  value?: unknown;
}
interface AXNode {
  nodeId: string;
  ignored: boolean;
  role?: AXValue;
  name?: AXValue;
  backendDOMNodeId?: number;
}

export interface NameProblem {
  readonly role: string;
  readonly html: string;
}

/** Every node of the page's accessibility tree with a role of NAMED_ROLES and an empty name (inside `root`, if given). */
export async function unnamed(cdp: CDPSession, root?: string): Promise<NameProblem[]> {
  await cdp.send('Accessibility.enable');
  const { nodes } = (await cdp.send('Accessibility.getFullAXTree')) as unknown as { nodes: AXNode[] };
  const out: NameProblem[] = [];
  for (const n of nodes) {
    if (n.ignored) continue;
    const role = String(n.role?.value ?? '');
    if (!NAMED_ROLES.has(role)) continue;
    const name = String(n.name?.value ?? '').trim();
    if (name !== '') continue;
    if (n.backendDOMNodeId === undefined) {
      out.push({ role, html: '(no DOM node)' });
      continue;
    }
    const { object } = (await cdp.send('DOM.resolveNode', { backendNodeId: n.backendDOMNodeId })) as { object: { objectId?: string } };
    if (!object.objectId) continue;
    const r = (await cdp.send('Runtime.callFunctionOn', {
      objectId: object.objectId,
      functionDeclaration: `function (root) {
        if (root && !(this.closest && this.closest(root))) return null;
        if (this.closest && this.closest('[aria-hidden=true]')) return null;
        return this.outerHTML.replace(/\\s+/g, ' ').slice(0, 200);
      }`,
      arguments: [{ value: root ?? null }],
      returnByValue: true,
    })) as { result: { value: string | null } };
    if (r.result.value !== null) out.push({ role, html: r.result.value });
  }
  return out;
}

/** The role and name Chromium gives each Tab stop of `walk` (from its accessibility tree). */
export async function rolesOf(cdp: CDPSession, walk: Walk): Promise<{ desc: string; role: string; name: string }[]> {
  await cdp.send('Accessibility.enable');
  const out: { desc: string; role: string; name: string }[] = [];
  for (const s of walk.stops) {
    const { result } = (await cdp.send('Runtime.evaluate', { expression: `window.__a11y.els[${s.id}]` })) as { result: { objectId?: string } };
    if (!result.objectId) continue;
    const { nodes } = (await cdp.send('Accessibility.getPartialAXTree', { objectId: result.objectId, fetchRelatives: false })) as unknown as { nodes: AXNode[] };
    const n = nodes.find((x) => !x.ignored) ?? nodes[0];
    out.push({ desc: s.desc, role: String(n?.role?.value ?? ''), name: String(n?.name?.value ?? '').trim() });
  }
  return out;
}

// ---- motion -------------------------------------------------------------------------------------------------------

export interface Motion {
  /** Animations and transitions declared longer than the threshold: element, property, duration. */
  readonly declared: string[];
  /** Animations of the document running now (CSS animations, transitions, Web Animations). */
  readonly running: string[];
}

/** The CSS motion of the whole document longer than `thresholdMs`. */
export function motionOf(page: Page, thresholdMs: number): Promise<Motion> {
  return page.evaluate((limit) => {
    const ms = (v: string): number[] => v.split(',').map((x) => (x.trim().endsWith('ms') ? parseFloat(x) : parseFloat(x) * 1000));
    const name = (el: Element) => `${el.tagName.toLowerCase()}${typeof el.className === 'string' && el.className ? '.' + el.className.trim().split(/\s+/).slice(0, 2).join('.') : ''}`;
    const declared: string[] = [];
    for (const el of Array.from(document.querySelectorAll('*'))) {
      for (const pseudo of [null, '::before', '::after']) {
        const s = getComputedStyle(el, pseudo);
        if (s.animationName !== 'none') {
          const d = Math.max(...ms(s.animationDuration));
          if (d > limit) declared.push(`${name(el)}${pseudo ?? ''} animation ${s.animationName} ${d} ms`);
        }
        if (s.transitionProperty !== 'none' && s.transitionProperty !== '') {
          const d = Math.max(...ms(s.transitionDuration)) + Math.max(0, ...ms(s.transitionDelay));
          if (d > limit && Math.max(...ms(s.transitionDuration)) > 0) declared.push(`${name(el)}${pseudo ?? ''} transition ${s.transitionProperty} ${d} ms`);
        }
      }
    }
    const running = document
      .getAnimations()
      .filter((a) => a.playState === 'running')
      .map((a) => {
        const t = a.effect?.getComputedTiming();
        const target = (a.effect as KeyframeEffect | null)?.target;
        return { d: Number(t?.duration ?? 0), what: `${target ? name(target) : '?'} ${(a as CSSAnimation).animationName ?? (a as CSSTransition).transitionProperty ?? a.id} ${t?.duration} ms` };
      })
      .filter((a) => a.d > limit)
      .map((a) => a.what);
    return { declared: [...new Set(declared)], running };
  }, thresholdMs);
}

/** Counts the requestAnimationFrame callbacks the page asks for during `ms` (wraps the function for that time). */
export async function framesDuring(page: Page, ms: number, act: () => Promise<void>): Promise<number> {
  await page.evaluate(() => {
    const w = window as unknown as { __raf?: typeof requestAnimationFrame; __rafCount: number };
    w.__rafCount = 0;
    if (!w.__raf) {
      w.__raf = window.requestAnimationFrame.bind(window);
      window.requestAnimationFrame = (cb: FrameRequestCallback) => {
        w.__rafCount++;
        return w.__raf!(cb);
      };
    }
  });
  await act();
  await page.waitForTimeout(ms);
  return page.evaluate(() => (window as unknown as { __rafCount: number }).__rafCount);
}
