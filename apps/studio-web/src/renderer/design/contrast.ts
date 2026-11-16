// WCAG 2.2 contrast of the design tokens (guardrail G8). Every pair of a text or graphic role on a ground it is used on
// is listed here; tests/unit/contrast.spec.ts computes each one and fails below AA (4.5:1 text, 3:1 graphics and UI
// states, WCAG 1.4.3 and 1.4.11), and the primitives gallery shows the same table for the maintainer.
//
// Pure and DOM-free: imported by the renderer (the gallery) and by the unit tests.
import { palette, role, tint, type PaletteName, type RoleName } from './tokens.js';

/** WCAG 2.x relative luminance of an sRGB colour (#rrggbb). */
export function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const lin = (c: number): number => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
}

/** WCAG 2.x contrast ratio of two opaque colours, from 1 to 21. */
export function contrastRatio(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/** `top` at `alpha` over the opaque `base`, as an opaque #rrggbb (sRGB compositing, as browsers draw it). */
export function composite(top: string, alpha: number, base: string): string {
  const t = parseInt(top.slice(1), 16);
  const b = parseInt(base.slice(1), 16);
  const ch = (shift: number): string => {
    const v = Math.round(((t >> shift) & 255) * alpha + ((b >> shift) & 255) * (1 - alpha));
    return v.toString(16).padStart(2, '0');
  };
  return `#${ch(16)}${ch(8)}${ch(0)}`;
}

export const AA = { text: 4.5, graphic: 3 } as const;
export type PairKind = keyof typeof AA;

/** A ground: a role, or a translucent tint over a role (a hover lift on the header band, for example). */
export type Ground = RoleName | { readonly tint: keyof typeof tint; readonly over: RoleName };

export interface ContrastPair {
  readonly fg: RoleName;
  readonly bg: Ground;
  readonly kind: PairKind;
  /** Where the pair appears. */
  readonly use: string;
}

export const hexOfRole = (r: RoleName): string => palette[role[r] as PaletteName];

export function hexOfGround(g: Ground): string {
  if (typeof g === 'string') return hexOfRole(g);
  const t = tint[g.tint];
  return composite(palette[t.color], t.alpha, hexOfRole(g.over));
}

export const groundName = (g: Ground): string => (typeof g === 'string' ? g : `${g.tint} over ${g.over}`);

const LIGHT_GROUNDS = ['ground', 'hover', 'row-hover', 'row-selected'] as const satisfies readonly RoleName[];

const on = (fg: RoleName, grounds: readonly Ground[], kind: PairKind, use: string): ContrastPair[] =>
  grounds.map((bg) => ({ fg, bg, kind, use }));

/** Every text and graphic role on every ground it is drawn on. */
export const PAIRS: readonly ContrastPair[] = [
  // Text (4.5:1)
  ...on('text', [...LIGHT_GROUNDS, 'viewport', 'row-failed', 'pressed', 'sub-tab-selected', 'sub-tab-hover', 'header-band'], 'text', 'body, controls, cells, heads'),
  ...on('text', [{ tint: 'ghost-hover', over: 'header-band' }, { tint: 'project-tab-hover', over: 'header-band' }], 'text', 'ghost button and project tab hover on the header band'),
  ...on('text', ['band-live'], 'text', 'chart-micro label inside the live band segment'),
  ...on('text-2', [...LIGHT_GROUNDS, 'viewport', 'row-failed', 'sub-tab-hover'], 'text', 'secondary text: captions, ids, ticks, units'),
  ...on('placeholder', ['ground', 'hover'], 'text', 'input placeholder (white field; sky-wash select on the header band)'),
  ...on('text-inverse', ['primary', 'primary-hover', 'segment-selected', 'footer'], 'text', 'primary button, selected segment, footer, scrubber flag'),
  ...on('tooltip-text', ['tooltip'], 'text', 'tooltip'),
  ...on('text-inverse-2', ['footer'], 'text', 'footer secondary text'),
  ...on('text-fail', ['ground', 'row-failed', 'row-hover'], 'text', 'the word "Failed" and failure notes'),
  ...on('selection-text', ['selection-bg'], 'text', 'selected text (::selection)'),
  // Graphics and UI states (3:1)
  ...on('rule', ['ground', 'hover', 'header-band', 'sub-tab-selected', 'viewport'], 'graphic', 'button, segmented and switch rules; structural edges'),
  ...on('field-border', ['ground'], 'graphic', 'input, number field and select border at rest'),
  ...on('field-border-hover', ['ground'], 'graphic', 'field border on hover'),
  ...on('field-border-focus', ['ground', 'hover'], 'graphic', 'field border on focus; select on the header band'),
  ...on('focus-edge', [...LIGHT_GROUNDS, 'viewport', 'row-failed', 'header-band', 'sub-tab-selected', 'project-tab-selected', 'pressed'], 'graphic', 'focus ring, ink edge next to the ground'),
  ...on('focus-ring', ['primary', 'segment-selected', 'rule'], 'graphic', 'focus ring, orange next to an ink control'),
  ...on('scrollbar', ['ground', 'viewport'], 'graphic', 'scrollbar thumb (styled here, so not the user agent default)'),
  ...on('caret', ['ground', 'hover'], 'graphic', 'text caret'),
  ...on('segment-selected', ['ground', 'hover'], 'graphic', 'selected segmented toggle'),
  ...on('tab-selected-mark', ['sub-tab-selected', 'ground', 'header-band', 'project-tab-selected'], 'graphic', 'selected sub-tab bar, selected project tab side rules'),
  ...on('switch-knob', ['ground'], 'graphic', 'switch knob, off'),
  ...on('rule', ['switch-on'], 'graphic', 'switch track rule and knob edge, on'),
  ...on('slider-track', ['ground'], 'graphic', 'slider track'),
  ...on('slider-thumb', ['ground'], 'graphic', 'slider thumb'),
  ...on('row-selected-edge', ['row-selected', 'ground'], 'graphic', 'selected row, inset ink lines'),
  ...on('popover-edge', ['ground', 'viewport'], 'graphic', 'popover border'),
  ...on('state-queued', [...LIGHT_GROUNDS, 'viewport'], 'graphic', 'queued pictogram'),
  ...on('state-running', [...LIGHT_GROUNDS, 'viewport'], 'graphic', 'running pictogram'),
  ...on('state-converged', [...LIGHT_GROUNDS, 'viewport'], 'graphic', 'converged pictogram'),
  ...on('state-finished', [...LIGHT_GROUNDS, 'viewport'], 'graphic', 'finished pictogram'),
  ...on('state-failed', ['ground', 'row-failed', 'row-hover'], 'graphic', 'failed pictogram'),
  ...on('state-faded', ['ground', 'row-hover'], 'graphic', 'faded pictogram (rows that are not selected)'),
  ...on('band-failed', ['ground'], 'graphic', 'failed band segment'),
];

/**
 * Roles that are not held to AA, with the WCAG reason. The contrast test fails when a role is in neither PAIRS (as a
 * foreground), this list, nor GROUND_ROLES, so every new role must be classified.
 */
export const EXEMPT: ReadonlyArray<{ readonly role: RoleName; readonly why: string }> = [
  { role: 'hair', why: 'Silver hairlines subdivide panels and rows; they identify no control or state (1.4.11 applies to what is needed to identify a component). Input borders use field-border instead.' },
  { role: 'hatch', why: 'The silver hatch means "not yet" only together with a band label in words (F-810 must keep that label; see the report).' },
  { role: 'hover', why: 'Hover is not a state WCAG 1.4.11 requires to contrast; it is a ground for other pairs.' },
  { role: 'pressed', why: 'The pressed fill lasts as long as the press; it is a ground for the button text.' },
  { role: 'row-hover', why: 'Hover only; a ground for other pairs.' },
  { role: 'sub-tab-hover', why: 'Hover only; a ground for other pairs.' },
  { role: 'slider-thumb-active', why: 'While dragged only; the thumb keeps its ink edge and the focus ring.' },
  { role: 'band-iterating', why: 'Band phase fills carry their phase in the end label word (F-810); sky on white is 2.28:1, so the band keeps a word for every phase.' },
  { role: 'band-converged', why: 'As band-iterating; meadow on white is 2.22:1.' },
  { role: 'band-live', why: 'As band-iterating; orange on white is 2.51:1. A ground for the ink chart-micro label (4.59:1).' },
  { role: 'switch-on', why: 'The on state is carried by the knob position and its ink edge (5.18:1 on meadow); the fill reinforces it.' },
  { role: 'sub-tab-selected', why: 'The selected sub-tab is carried by tab-selected-mark (an ink bar) and weight; meadow is the ground.' },
  { role: 'project-tab-selected', why: 'The selected project tab is carried by tab-selected-mark (ink side rules) and weight; white is the ground.' },
  { role: 'selection-bg', why: 'A ground for selection-text.' },
];

/** Roles that are only ever grounds. */
export const GROUND_ROLES: readonly RoleName[] = [
  'ground',
  'header-band',
  'viewport',
  'footer',
  'primary',
  'primary-hover',
  'row-selected',
  'row-failed',
  'tooltip',
  'popover',
];

export interface PairResult extends ContrastPair {
  readonly fgHex: string;
  readonly bgHex: string;
  readonly ratio: number;
  readonly required: number;
  readonly pass: boolean;
}

/** True when `fg` on `bg` meets AA for its kind. Compared at two decimals, as ratios are reported (4.499 is not 4.5). */
export function passesAA(fg: string, bg: string, kind: PairKind): boolean {
  return Math.floor(contrastRatio(fg, bg) * 100) / 100 >= AA[kind];
}

export function evaluate(pair: ContrastPair): PairResult {
  const fgHex = hexOfRole(pair.fg);
  const bgHex = hexOfGround(pair.bg);
  const ratio = contrastRatio(fgHex, bgHex);
  return { ...pair, fgHex, bgHex, ratio, required: AA[pair.kind], pass: passesAA(fgHex, bgHex, pair.kind) };
}

export const evaluateAll = (): PairResult[] => PAIRS.map(evaluate);
