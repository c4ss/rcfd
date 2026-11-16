// The design tokens of rcfd Studio: the one source of the values in DESIGN.md (and its sidecar .impeccable/design.json)
// that the renderer uses. tokens.css is generated from this file (npm run tokens:css) and tests/unit/tokens.spec.ts
// fails when the two disagree, or when a value here drifts from DESIGN.md.
//
// This file has no imports and uses only erasable TypeScript, so Node can run it with type stripping
// (scripts/write-tokens-css.mjs) and the unit tests can import it directly.

/** The colour scale of DESIGN.md (frontmatter `colors`), plus the footer's light ink tint from its Components prose. */
export const palette = {
  ink: '#1d3a5f',
  'ink-2': '#4a6382',
  'ink-hover': '#2a4d78',
  sky: '#62b4e8',
  'sky-wash': '#e4f0f9',
  meadow: '#6cc078',
  'meadow-wash': '#e6f3e8',
  'meadow-deep': '#2f7d3b',
  orange: '#f08a24',
  'orange-deep': '#b35a00',
  fail: '#9b1c24',
  'fail-wash': '#f7e4e5',
  silver: '#c9cdd1',
  'silver-wash': '#eef1f4',
  white: '#ffffff',
  /** DESIGN.md, Disclaimer footer: "secondary text in a light ink tint (#c7d3e2)". Footer (on ink) only. */
  'ink-tint': '#c7d3e2',
} as const;

export type PaletteName = keyof typeof palette;

/**
 * Each job a colour does, named once (the Colour Has a Job Rule). Components use roles, never palette names, so a
 * change of colour for a job is one line here. Each role is what DESIGN.md gives that job; where DESIGN.md is silent
 * (text selection, scrollbar) it is what the prototype (apps/studio-prototype/src/styles.css) drew.
 */
export const role = {
  // Grounds
  ground: 'white',
  'header-band': 'sky',
  viewport: 'silver-wash',
  footer: 'ink',
  // Text
  text: 'ink',
  'text-2': 'ink-2',
  'text-inverse': 'white',
  'text-inverse-2': 'ink-tint',
  'text-fail': 'fail',
  placeholder: 'ink-2',
  // Lines
  rule: 'ink',
  hair: 'silver',
  'field-border': 'silver',
  'field-border-hover': 'ink-2',
  'field-border-focus': 'ink',
  scrollbar: 'silver',
  // Interaction
  hover: 'sky-wash',
  pressed: 'silver',
  primary: 'ink',
  'primary-hover': 'ink-hover',
  'focus-ring': 'orange',
  caret: 'orange',
  'selection-bg': 'sky',
  'selection-text': 'white',
  'segment-selected': 'ink',
  'sub-tab-selected': 'meadow',
  'sub-tab-hover': 'meadow-wash',
  'project-tab-selected': 'white',
  'switch-on': 'meadow',
  'switch-knob': 'ink',
  'slider-track': 'ink',
  'slider-thumb': 'ink',
  'slider-thumb-active': 'orange',
  // Rows
  'row-hover': 'silver-wash',
  'row-selected': 'sky-wash',
  'row-selected-edge': 'ink',
  'row-failed': 'fail-wash',
  // State marks (pictograms); the running mark is orange-deep (design.json, State Pictogram with Word)
  'state-queued': 'ink',
  'state-running': 'orange-deep',
  'state-converged': 'meadow-deep',
  'state-finished': 'ink',
  'state-failed': 'fail',
  // Run band fills (F-810 draws the bands; the fills are named here so the job stays in one place)
  'band-iterating': 'sky',
  'band-converged': 'meadow',
  'band-live': 'orange',
  'band-failed': 'fail',
  hatch: 'silver',
  // The Claude Code panel (F-814; DESIGN.md Components, Claude Code panel): the user's message in sky-wash. Its meadow head
  // has no role yet: tests/unit/tokens.spec.ts lists meadow's users, and adding one is the maintainer's call.
  'chat-own': 'sky-wash',
  // Overlays
  tooltip: 'ink',
  'tooltip-text': 'white',
  popover: 'white',
  'popover-edge': 'ink',
} as const satisfies Record<string, PaletteName>;

export type RoleName = keyof typeof role;

/** Translucent tints that DESIGN.md gives as percentages of a palette colour. */
export const tint = {
  /** Ghost button hover on the header band: "a 35% white lift". */
  'ghost-hover': { color: 'white', alpha: 0.35 },
  /** Project tab hover in the header band (design.json `.ds-ptab:hover`). */
  'project-tab-hover': { color: 'white', alpha: 0.3 },
  /** Project tab dividers: "divided by 35% ink lines". */
  'tab-divider': { color: 'ink', alpha: 0.35 },
  /** The popover lift's shadow colour (ink at 18%). */
  'popover-shadow': { color: 'ink', alpha: 0.18 },
} as const satisfies Record<string, { color: PaletteName; alpha: number }>;

export const font = {
  family: "Archivo, 'Helvetica Neue', Arial, sans-serif",
  weights: { regular: 400, medium: 500, bold: 700 },
} as const;

export interface TypeStyle {
  readonly size: number;
  readonly weight: number;
  readonly lineHeight?: number;
  readonly tracking?: string;
  readonly tnum?: boolean;
}

/** The type ramp of DESIGN.md (frontmatter `typography`); px sizes. A missing lineHeight inherits body's. */
export const typeRamp = {
  display: { size: 30, weight: 700, lineHeight: 1, tracking: '-0.03em', tnum: true },
  headline: { size: 22, weight: 700, tracking: '-0.02em', tnum: true },
  title: { size: 15, weight: 700, tracking: '-0.01em' },
  'title-sm': { size: 13, weight: 700 },
  body: { size: 12, weight: 400, lineHeight: 1.35, tnum: true },
  label: { size: 11, weight: 500, tnum: true },
  'chart-micro': { size: 10, weight: 500, tnum: true },
} as const satisfies Record<string, TypeStyle>;

export type TypeName = keyof typeof typeRamp;

/** Spacing (frontmatter `spacing`), px. */
export const space = { hair: 2, xs: 4, sm: 6, md: 8, lg: 12, xl: 16, xxl: 24 } as const;

/** Corners (frontmatter `rounded`): every corner is square. */
export const radius = { none: 0 } as const;

/** Fixed sizes of DESIGN.md (Layout, Components), px unless a unit is given. */
export const size = {
  control: 26,
  'control-padding': 10,
  'input-padding': 8,
  'icon-gap': 6,
  chip: 22,
  'tab-row': 32,
  header: 48,
  'header-rule': 3,
  'run-row': 48,
  'run-band': 20,
  'flag-lane': 18,
  'time-panel': 248,
  footer: 24,
  'min-width': 1180,
  icon: 14,
  'switch-width': 28,
  'switch-height': 14,
  'measure-chat': '60ch',
  'measure-empty': '46ch',
} as const;

/** Lines (design.json `lines`): structure in ink, subdivision in silver hairlines, the "not yet" hatch. */
export const line = {
  rule: { width: 1, role: 'rule' },
  hair: { width: 1, role: 'hair' },
  'header-rule': { width: 3, role: 'rule' },
  hatch: { angle: 135, stripe: 4, role: 'hatch', between: 'ground' },
} as const;

/**
 * Focus (design.json `focus-outline`): on every focusable element, a 2px orange outline at a 1px offset. Tabs, table
 * cells, list options and panels take a negative offset, where neighbours would hide an outset outline; project tabs
 * -3px (design.json `.ds-ptab:focus-visible`), the others -2px (the prototype's sub-tabs).
 */
export const focus = { width: 2, offset: 1, inset: -2, 'inset-project-tab': -3 } as const;

/** Fade (the Fade by Colour Rule): a faded pictogram is half-filled, its opacity halved; faded text turns ink-2. */
export const fade = { pictogram: 0.5 } as const;

/** Shadows (design.json `shadows`): the popover lift only (the Flat Field Rule). */
export const shadow = { popover: { x: 0, y: 6, blur: 18, tint: 'popover-shadow' } } as const;

/** Motion (design.json `motion`), ms. Off under prefers-reduced-motion: reduce. */
export const motion = {
  ease: 'cubic-bezier(0.16, 1, 0.3, 1)',
  toggle: 160,
  fade: 200,
  'band-grow': 700,
  'band-stagger': 90,
} as const;

/** The pictogram grid (design.json `pictogramGrid`): a 14-unit square, strokes at 0, 45 and 90 degrees, square caps. */
export const pictogram = {
  units: 14,
  angles: [0, 45, 90],
  stroke: { state: 2, icon: 1.75, running: 2.4 },
  caps: 'square',
  joins: 'miter',
} as const;

export const breakpoint = { 'min-desktop': 1180 } as const;

// ---- CSS custom properties -----------------------------------------------------------------------------------

const px = (v: number | string): string => (typeof v === 'number' ? (v === 0 ? '0' : `${v}px`) : v);

function rgba(hex: string, alpha: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}

/**
 * Every token as a CSS custom property, in the order of tokens.css. Roles refer to the palette by var(), so the
 * cascade keeps the one-source structure in the browser too.
 */
export function cssCustomProperties(): Array<readonly [string, string]> {
  const out: Array<readonly [string, string]> = [];
  for (const [k, v] of Object.entries(palette)) out.push([`--color-${k}`, v]);
  for (const [k, v] of Object.entries(role)) out.push([`--role-${k}`, `var(--color-${v})`]);
  for (const [k, v] of Object.entries(tint)) out.push([`--tint-${k}`, rgba(palette[v.color], v.alpha)]);
  out.push(['--font-family', font.family]);
  for (const [k, v] of Object.entries(font.weights)) out.push([`--font-weight-${k}`, String(v)]);
  for (const [k, v] of Object.entries(typeRamp) as Array<[string, TypeStyle]>) {
    out.push([`--type-${k}-size`, px(v.size)]);
    out.push([`--type-${k}-weight`, String(v.weight)]);
    if (v.lineHeight !== undefined) out.push([`--type-${k}-line-height`, String(v.lineHeight)]);
    if (v.tracking !== undefined) out.push([`--type-${k}-tracking`, v.tracking]);
  }
  for (const [k, v] of Object.entries(space)) out.push([`--space-${k}`, px(v)]);
  for (const [k, v] of Object.entries(radius)) out.push([`--radius-${k}`, px(v)]);
  for (const [k, v] of Object.entries(size)) out.push([`--size-${k}`, px(v)]);
  out.push(['--line-rule', `${px(line.rule.width)} solid var(--role-rule)`]);
  out.push(['--line-hair', `${px(line.hair.width)} solid var(--role-hair)`]);
  out.push(['--line-header-rule', `${px(line['header-rule'].width)} solid var(--role-rule)`]);
  out.push([
    '--line-hatch',
    `repeating-linear-gradient(${line.hatch.angle}deg, var(--role-hatch) 0 ${px(line.hatch.stripe)}, var(--role-ground) ${px(line.hatch.stripe)} ${px(2 * line.hatch.stripe)})`,
  ]);
  out.push(['--focus-width', px(focus.width)]);
  out.push(['--focus-offset', px(focus.offset)]);
  out.push(['--focus-offset-inset', px(focus.inset)]);
  out.push(['--focus-offset-project-tab', px(focus['inset-project-tab'])]);
  out.push(['--fade-pictogram', String(fade.pictogram)]);
  const s = shadow.popover;
  out.push(['--shadow-popover', `${px(s.x)} ${px(s.y)} ${px(s.blur)} var(--tint-${s.tint})`]);
  out.push(['--motion-ease', motion.ease]);
  for (const k of ['toggle', 'fade', 'band-grow', 'band-stagger'] as const) out.push([`--motion-${k}`, `${motion[k]}ms`]);
  out.push(['--pictogram-units', String(pictogram.units)]);
  out.push(['--pictogram-stroke-state', String(pictogram.stroke.state)]);
  out.push(['--pictogram-stroke-icon', String(pictogram.stroke.icon)]);
  out.push(['--pictogram-stroke-running', String(pictogram.stroke.running)]);
  out.push(['--pictogram-caps', pictogram.caps]);
  out.push(['--pictogram-joins', pictogram.joins]);
  out.push(['--breakpoint-min-desktop', px(breakpoint['min-desktop'])]);
  return out;
}

/** The text of tokens.css, exactly. */
export function tokensCssText(): string {
  const head = [
    '/* GENERATED from tokens.ts by `npm run tokens:css`; do not edit by hand. tests/unit/tokens.spec.ts keeps the two',
    '   equal. The values are DESIGN.md\'s. */',
  ];
  const body = cssCustomProperties().map(([k, v]) => `  ${k}: ${v};`);
  return [...head, ':root {', ...body, '}', ''].join('\n');
}
