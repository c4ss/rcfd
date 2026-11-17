// F-833 (G22): counts the visible chrome words of one surface from the DOM, by the definition in
// src/shared/word-budget.ts. The counting function runs in the page (page.evaluate), so it is self-contained.
import type { Page } from '@playwright/test';

export interface WordCount {
  readonly count: number;
  /** Every counted word with the text it came from, for the report and a failing test's message. */
  readonly words: readonly { word: string; text: string }[];
  /** Whether the surface's root was found. */
  readonly found: boolean;
}

export interface SurfaceRoot {
  /** The surface's root element (the first match). */
  readonly root: string;
  /** Roots of other surfaces nested in it, not counted here. */
  readonly exclude?: readonly string[];
}

/** Counts the chrome words of the surface whose root is `s.root`. */
export function countWords(page: Page, s: SurfaceRoot): Promise<WordCount> {
  return page.evaluate(
    ({ root, exclude }) => {
      const el = document.querySelector(root);
      if (!el) return { count: 0, words: [], found: false };
      const NOT_CHROME = new Set(['data', 'exception', 'disclaimer', 'help']);
      const isWord = (t: string) => /[\p{L}\p{N}]/u.test(t);
      const excluded = (e: Element) => exclude.some((sel) => e.closest(sel) !== null && e.closest(sel) !== el && el.contains(e.closest(sel)));
      const kind = (e: Element) => e.closest('[data-words]')?.getAttribute('data-words') ?? 'chrome';
      const shown = (e: Element) => e.checkVisibility({ visibilityProperty: true, opacityProperty: true } as CheckVisibilityOptions);
      const big = (rects: DOMRectList | DOMRect[]) => {
        const list = Array.from(rects);
        return list.length > 0 && list.some((r) => r.width >= 2 && r.height >= 2);
      };
      const out: { word: string; text: string }[] = [];
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const text = n.textContent ?? '';
        if (!isWord(text)) continue;
        const parent = n.parentElement;
        if (!parent || excluded(parent) || NOT_CHROME.has(kind(parent)) || !shown(parent)) continue;
        if (parent.closest('script, style, template')) continue;
        const range = document.createRange();
        range.selectNodeContents(n);
        if (!big(range.getClientRects())) continue;
        // clipped away by a 1px box (the sr-only pattern) on an ancestor
        let clipped = false;
        for (let a: Element | null = parent; a && a !== el.parentElement; a = a.parentElement) {
          const r = a.getBoundingClientRect();
          const cs = getComputedStyle(a);
          if ((cs.overflow !== 'visible' || cs.clipPath !== 'none') && (r.width < 2 || r.height < 2 || cs.clipPath === 'inset(50%)')) {
            clipped = true;
            break;
          }
        }
        if (clipped) continue;
        for (const w of text.split(/\s+/)) if (isWord(w)) out.push({ word: w, text: text.trim().slice(0, 80) });
      }
      for (const f of Array.from(el.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>('input[placeholder], textarea[placeholder]'))) {
        if (f.value !== '' || excluded(f) || NOT_CHROME.has(kind(f)) || !shown(f) || !big([f.getBoundingClientRect()])) continue;
        for (const w of f.placeholder.split(/\s+/)) if (isWord(w)) out.push({ word: w, text: `placeholder: ${f.placeholder.slice(0, 70)}` });
      }
      return { count: out.length, words: out, found: true };
    },
    { root: s.root, exclude: [...(s.exclude ?? [])] },
  );
}
