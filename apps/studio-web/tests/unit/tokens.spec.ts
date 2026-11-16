// The design tokens (F-802): tokens.css is exactly what tokens.ts generates, every stylesheet uses only defined tokens
// and no colour literals, and tokens.ts holds DESIGN.md's values (its frontmatter and .impeccable/design.json), with no
// deviations; where DESIGN.md is silent (text selection, scrollbar) the values are the prototype's.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  breakpoint,
  cssCustomProperties,
  fade,
  focus,
  font,
  line,
  motion,
  palette,
  pictogram,
  radius,
  role,
  shadow,
  size,
  space,
  tint,
  tokensCssText,
  typeRamp,
} from '../../src/renderer/design/tokens.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const repo = path.resolve(root, '..', '..');
const read = (...p: string[]): string => readFileSync(path.join(...p), 'utf8').replace(/\r\n/g, '\n');
const renderer = path.join(root, 'src', 'renderer');

type Yaml = { [k: string]: Yaml | string | number };

/** The subset of YAML that DESIGN.md's frontmatter uses: nested maps of scalars, two-space indents. */
function frontmatter(md: string): Yaml {
  const m = /^---\n([\s\S]*?)\n---\n/.exec(md);
  if (!m) throw new Error('DESIGN.md has no frontmatter');
  const top: Yaml = {};
  const stack: Array<{ indent: number; node: Yaml }> = [{ indent: -1, node: top }];
  for (const raw of m[1]!.split('\n')) {
    if (!raw.trim() || raw.trim().startsWith('#')) continue;
    const indent = raw.length - raw.trimStart().length;
    const kv = /^\s*([^:]+):\s*(.*)$/.exec(raw);
    if (!kv) throw new Error(`unparsed frontmatter line: ${raw}`);
    while (stack.at(-1)!.indent >= indent) stack.pop();
    const parent = stack.at(-1)!.node;
    const key = kv[1]!.trim();
    const value = kv[2]!.trim();
    if (value === '') {
      const child: Yaml = {};
      parent[key] = child;
      stack.push({ indent, node: child });
    } else {
      const unq = value.replace(/^"(.*)"$/, '$1');
      parent[key] = /^-?\d+(\.\d+)?$/.test(unq) && !value.startsWith('"') ? Number(unq) : unq;
    }
  }
  return top;
}

const design = frontmatter(read(repo, 'DESIGN.md'));
const designMd = read(repo, 'DESIGN.md');
const sidecar = JSON.parse(read(repo, '.impeccable', 'design.json')) as {
  extensions: {
    colorMeta: Record<string, { canonical: string }>;
    lines: Array<{ name: string; value: string }>;
    motion: Array<{ name: string; value: string }>;
    shadows: Array<{ name: string; value: string }>;
    focus: Array<{ name: string; value: string }>;
    pictogramGrid: { units: number; angles: number[]; stroke: { state: string; icon: string }; caps: string; joins: string };
  };
  components: Array<{ name: string; css: string }>;
};
const ext = sidecar.extensions;
const named = <T extends { name: string }>(list: T[], name: string): T => {
  const hit = list.find((x) => x.name === name);
  if (!hit) throw new Error(`design.json has no ${name}`);
  return hit;
};

test.describe('tokens.css and tokens.ts', () => {
  test('tokens.css is exactly the output of tokens.ts (npm run tokens:css)', () => {
    const file = read(renderer, 'design', 'tokens.css');
    expect(file, 'tokens.css is stale: run npm run tokens:css').toBe(tokensCssText());
  });

  test('tokens.css declares each custom property once, and every one is a token of tokens.ts', () => {
    const file = read(renderer, 'design', 'tokens.css');
    const declared = [...file.matchAll(/^\s*(--[a-z0-9-]+):\s*(.+);$/gm)].map((m) => [m[1]!, m[2]!] as const);
    expect(declared).toEqual(cssCustomProperties());
    expect(new Set(declared.map(([k]) => k)).size).toBe(declared.length);
  });

  const sheets = [
    ['base.css'],
    ['ui', 'primitives.css'],
    ['dev', 'gallery.css'],
  ] as const;
  // Set by React Aria on the popover (the width of the select button).
  const external = new Set(['--trigger-width']);

  for (const sheet of sheets) {
    test(`${sheet.join('/')} uses only defined tokens and no colour literals`, () => {
      const css = read(renderer, ...sheet).replace(/\/\*[\s\S]*?\*\//g, '');
      const defined = new Set(cssCustomProperties().map(([k]) => k));
      const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
      expect(used.length).toBeGreaterThan(0);
      expect(used.filter((v) => !defined.has(v) && !external.has(v))).toEqual([]);
      expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? []).toEqual([]);
      const radii = [...css.matchAll(/border-radius:\s*([^;]+)/g)].map((m) => m[1]!.trim());
      expect(radii.filter((v) => v !== 'var(--radius-none)' && v !== '0')).toEqual([]);
    });
  }
});

test.describe('tokens.ts holds the values of DESIGN.md', () => {
  test('palette: the frontmatter colours, the sidecar canonical values, and the footer tint from the prose', () => {
    const colors = design['colors'] as Record<string, string>;
    const fromPalette = Object.fromEntries(Object.entries(palette).filter(([k]) => k !== 'ink-tint'));
    expect(fromPalette).toEqual(colors);
    for (const [k, v] of Object.entries(ext.colorMeta)) expect(palette[k as keyof typeof palette], k).toBe(v.canonical);
    expect(designMd).toContain(`light ink tint (${palette['ink-tint']}`);
  });

  test('type ramp: family, sizes, weights, line heights, tracking and tabular figures', () => {
    const typography = design['typography'] as Record<string, Record<string, string | number>>;
    expect(Object.keys(typeRamp)).toEqual(Object.keys(typography));
    for (const [name, t] of Object.entries(typography)) {
      const ours = typeRamp[name as keyof typeof typeRamp] as Record<string, unknown>;
      expect(t['fontFamily'], name).toBe(font.family.replace(/'/g, ''));
      expect(`${ours['size']}px`, name).toBe(t['fontSize']);
      expect(ours['weight'], name).toBe(t['fontWeight']);
      expect(ours['lineHeight'], name).toBe(t['lineHeight']);
      expect(ours['tracking'], name).toBe(t['letterSpacing']);
      expect(ours['tnum'] === true, name).toBe(t['fontFeature'] === 'tnum');
    }
  });

  test('spacing, corners and the component sizes', () => {
    const spacing = design['spacing'] as Record<string, string>;
    expect(Object.fromEntries(Object.entries(space).map(([k, v]) => [k, `${v}px`]))).toEqual(spacing);
    expect(radius.none).toBe(0);
    expect((design['rounded'] as Record<string, string>)['none']).toBe('0px');
    const c = design['components'] as Record<string, Record<string, string>>;
    expect(c['button']!['height']).toBe(`${size.control}px`);
    expect(c['button']!['padding']).toBe(`0 ${size['control-padding']}px`);
    expect(c['input']!['height']).toBe(`${size.control}px`);
    expect(c['input']!['padding']).toBe(`0 ${size['input-padding']}px`);
    expect(c['segmented-toggle-selected']!['height']).toBe(`${size.control}px`);
    expect(c['header-band']!['height']).toBe(`${size.header}px`);
    expect(c['sub-tab-selected']!['height']).toBe(`${size['tab-row']}px`);
    expect(c['run-row']!['height']).toBe(`${size['run-row']}px`);
    expect(c['run-band']!['height']).toBe(`${size['run-band']}px`);
    expect(c['footer']!['height']).toBe(`${size.footer}px`);
    expect(breakpoint['min-desktop']).toBe(size['min-width']);
    expect(designMd).toContain(`${size['min-width']}px minimum width`);
    expect(designMd).toContain(`a ${size['time-panel']}px time panel`);
    expect(designMd).toContain(`a ${size['switch-width']} by ${size['switch-height']}px ink-ruled track`);
    expect(designMd).toContain(`${size.chip}px tall with a silver hairline`);
  });

  test('component colours of the frontmatter are the roles of tokens.ts', () => {
    const c = design['components'] as Record<string, Record<string, string>>;
    const ref = (v: string | undefined): string | undefined => v?.replace(/^\{colors\.(.+)\}$/, '$1');
    expect(ref(c['button']!['backgroundColor'])).toBe(role.ground);
    expect(ref(c['button']!['textColor'])).toBe(role.text);
    expect(ref(c['button-hover']!['backgroundColor'])).toBe(role.hover);
    expect(ref(c['button-pressed']!['backgroundColor'])).toBe(role.pressed);
    expect(ref(c['button-primary']!['backgroundColor'])).toBe(role.primary);
    expect(ref(c['button-primary']!['textColor'])).toBe(role['text-inverse']);
    expect(ref(c['button-primary-hover']!['backgroundColor'])).toBe(role['primary-hover']);
    expect(ref(c['input']!['backgroundColor'])).toBe(role.ground);
    expect(ref(c['segmented-toggle-selected']!['backgroundColor'])).toBe(role['segment-selected']);
    expect(ref(c['header-band']!['backgroundColor'])).toBe(role['header-band']);
    expect(ref(c['project-tab-selected']!['backgroundColor'])).toBe(role['project-tab-selected']);
    expect(ref(c['sub-tab-selected']!['backgroundColor'])).toBe(role['sub-tab-selected']);
    expect(ref(c['sub-tab-hover']!['backgroundColor'])).toBe(role['sub-tab-hover']);
    expect(ref(c['run-row-selected']!['backgroundColor'])).toBe(role['row-selected']);
    expect(ref(c['run-row-failed']!['backgroundColor'])).toBe(role['row-failed']);
    expect(ref(c['run-row-failed']!['textColor'])).toBe(role['text-fail']);
    expect(ref(c['tooltip']!['backgroundColor'])).toBe(role.tooltip);
    expect(ref(c['tooltip']!['textColor'])).toBe(role['tooltip-text']);
    expect(ref(c['footer']!['backgroundColor'])).toBe(role.footer);
  });

  test('lines, shadow and motion: the sidecar values', () => {
    expect(named(ext.lines, 'rule').value).toBe(`${line.rule.width}px solid ${palette[role[line.rule.role]]}`);
    expect(named(ext.lines, 'hair').value).toBe(`${line.hair.width}px solid ${palette[role[line.hair.role]]}`);
    expect(named(ext.lines, 'header-rule').value).toBe(`${line['header-rule'].width}px solid ${palette[role.rule]}`);
    const h = line.hatch;
    expect(named(ext.lines, 'hatch').value).toBe(
      `repeating-linear-gradient(${h.angle}deg, ${palette[role.hatch]} 0 ${h.stripe}px, ${palette[role.ground]} ${h.stripe}px ${2 * h.stripe}px)`,
    );
    const s = shadow.popover;
    const t = tint[s.tint];
    const n = parseInt(palette[t.color].slice(1), 16);
    expect(named(ext.shadows, 'popover-lift').value).toBe(
      `${s.x} ${s.y}px ${s.blur}px rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${t.alpha})`,
    );
    expect(named(ext.motion, 'ease-out').value).toBe(motion.ease);
    expect(named(ext.motion, 'toggle').value).toBe(`${motion.toggle}ms`);
    expect(named(ext.motion, 'fade').value).toBe(`${motion.fade}ms`);
    expect(named(ext.motion, 'band-grow').value).toBe(
      `${motion['band-grow']}ms ease-out, ${motion['band-stagger']}ms stagger, once on load`,
    );
    expect(designMd).toContain('35% white lift');
    expect(tint['ghost-hover'].alpha).toBe(0.35);
    expect(designMd).toContain('divided by 35% ink lines');
    expect(tint['tab-divider']).toEqual({ color: 'ink', alpha: 0.35 });
  });

  test('pictogram grid: units, angles, strokes, caps and joins', () => {
    const g = ext.pictogramGrid;
    expect(pictogram.units).toBe(g.units);
    expect([...pictogram.angles]).toEqual(g.angles);
    expect(`${pictogram.stroke.state}px`).toBe(g.stroke.state);
    expect(`${pictogram.stroke.icon}px`).toBe(g.stroke.icon);
    expect(pictogram.caps).toBe(g.caps);
    expect(pictogram.joins).toBe(g.joins);
    // The running mark is drawn heavier (design.json, State Pictogram with Word).
    expect(named(sidecar.components, 'State Pictogram with Word').css).toContain(`stroke-width:${pictogram.stroke.running}`);
  });
});

test.describe('focus, caret, fields, tabs, selection and pictograms are what DESIGN.md says', () => {
  const prototype = read(repo, 'apps', 'studio-prototype', 'src', 'styles.css');
  const css = (name: string): string => named(sidecar.components, name).css;

  test('focus: a 2px orange outline at a 1px offset; project tabs -3px', () => {
    expect(named(ext.focus, 'focus-outline').value).toBe(
      `outline: ${focus.width}px solid ${palette[role['focus-ring']]}; outline-offset: ${focus.offset}px`,
    );
    expect(designMd).toContain(`**Focus:** ${focus.width}px orange outline, ${focus.offset}px offset, on every focusable element.`);
    expect(css('Header Band with Project Tabs')).toContain(
      `.ds-ptab:focus-visible{outline:${focus.width}px solid var(--orange, ${palette.orange});outline-offset:${focus['inset-project-tab']}px;}`,
    );
    expect(prototype).toContain(`.sub-tabs .react-aria-Tab[data-focus-visible] { outline: 2px solid var(--orange); outline-offset: ${focus.inset}px; }`);
  });

  test('inputs: silver hairline border, ink-2 on hover, ink on focus, ink-2 placeholder, orange caret', () => {
    const input = css('Text Input');
    expect(input).toContain(`border:1px solid var(--${role['field-border']}, ${palette[role['field-border']]})`);
    expect(input).toContain(`.ds-input:hover{border-color:var(--${role['field-border-hover']}, ${palette[role['field-border-hover']]});}`);
    expect(input).toContain(`.ds-input:focus{border-color:var(--${role['field-border-focus']}, ${palette[role['field-border-focus']]});`);
    expect(input).toContain(`.ds-input::placeholder{color:var(--${role.placeholder}, ${palette[role.placeholder]});}`);
    expect(input).toContain(`caret-color:var(--${role.caret}, ${palette[role.caret]})`);
    expect(designMd).toContain('a silver hairline border, square, ink-2 placeholder, orange caret.');
  });

  test('tabs: the selected sub-tab a meadow field, the selected project tab white, both with weight alone', () => {
    expect(role['sub-tab-selected']).toBe('meadow');
    expect(role['project-tab-selected']).toBe('white');
    expect(css('Header Band with Project Tabs')).toContain('.ds-ptab.on{background:var(--white, #fff);font-weight:700;}');
    expect(prototype).toContain('.sub-tabs .react-aria-Tab[data-selected] { background: var(--meadow); font-weight: 700; }');
  });

  test('pictograms: the running mark orange-deep, a faded mark half-filled', () => {
    expect(css('State Pictogram with Word')).toContain(`.ds-state-running svg{color:${palette[role['state-running']]};`);
    expect(css('State Pictogram with Word')).toContain(`.ds-state-converged svg{color:${palette[role['state-converged']]};}`);
    expect(designMd).toContain('pictograms half-filled');
    expect(prototype).toContain(`.run-row.faded .picto { opacity: ${fade.pictogram}; }`);
  });

  test('selection and scrollbar, where DESIGN.md is silent: the prototype', () => {
    expect(prototype).toContain(`::selection { background: var(--${role['selection-bg']}); color: var(--${role['selection-text']}); }`);
    expect(prototype).toContain(`scrollbar-color: var(--${role.scrollbar}) transparent;`);
  });

  test('every role keeps the job DESIGN.md gives its colour', () => {
    // The Colour Has a Job Rule: orange only for the live band, focus and the caret; meadow for the active sub-tab, the
    // switch and converged; sky for the header band, iterating and selection; red for failure.
    const users = (c: string): string[] =>
      Object.entries(role)
        .filter(([, v]) => v === c)
        .map(([k]) => k)
        .sort();
    expect(users('orange')).toEqual(['band-live', 'caret', 'focus-ring', 'slider-thumb-active']);
    expect(users('meadow')).toEqual(['band-converged', 'sub-tab-selected', 'switch-on']);
    expect(users('sky')).toEqual(['band-iterating', 'header-band', 'selection-bg']);
    expect(users('fail')).toEqual(['band-failed', 'state-failed', 'text-fail']);
    expect(users('fail-wash')).toEqual(['row-failed']);
  });
});
