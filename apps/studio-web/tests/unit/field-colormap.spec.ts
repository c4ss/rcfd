// The field viewport's colour table against rcfd-render's colour map (F-808, guardrail G3; DESIGN.md, the Data Owns the
// Field Rule). The reference is read from the Rust source (tests/support/rcfd-render-map.ts): rcfd_post's stops, and
// rcfd-render's ColorMap::eval repeated operation for operation.
//
// - Every one of the 256 entries equals rcfd-render's colour at k / 255, and both ends equal its end colours.
// - How a value finds its entry (lutIndex, the shader's arithmetic in f32): over the full range the Studio's colour is
//   rcfd-render's colour at the nearest table point, equal to rcfd-render's colour of the value itself within the stated
//   tolerance (LUT_TOLERANCE_LEVELS, 3 of 255) and exactly at the table points and at and beyond both ends.
// - The legend's tick labels are rcfd-studio's `{:.4e}`; the readout's text reads back as the same f32.
// - field.css uses only defined tokens and no colour literals.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { cssCustomProperties } from '../../src/renderer/design/tokens.js';
import {
  colourOf,
  colourRange,
  f32Text,
  LUT_SIZE,
  LUT_TOLERANCE_LEVELS,
  lutBytes,
  lutIndex,
  tickLabel,
  ticks,
  VIRIDIS_STOPS,
} from '../../src/renderer/field/colormap.js';
import { appRoot } from '../support/rcfd.js';
import { rcfdRenderColour, rcfdRenderViridis, rustEval, rustSourceChecks, rustStops } from '../support/rcfd-render-map.js';

const lut = lutBytes();
const entry = (k: number) => [lut[4 * k]!, lut[4 * k + 1]!, lut[4 * k + 2]!];
const diff = (a: readonly number[], b: readonly number[]) => Math.max(...a.map((x, i) => Math.abs(x - b[i]!)));

test.describe('the colour table is rcfd-render’s colour map (G3)', () => {
  test('the Rust code the reference repeats is unchanged', () => {
    for (const { file, lines } of rustSourceChecks) {
      const src = readFileSync(file, 'utf8');
      for (const l of lines) expect(src.includes(l), `${path.basename(file)}: ${l}`).toBe(true);
    }
  });

  test('the stops are rcfd_post’s, and rcfd-render’s viridis samples them exactly', () => {
    const post = rustStops();
    expect(post).toHaveLength(9);
    expect(VIRIDIS_STOPS.map((s) => [...s])).toEqual(post);
    expect(rcfdRenderViridis()).toEqual(post);
    // DESIGN.md: "9 control stops, #440154 to #fde725"
    const hex = (c: readonly number[]) => `#${c.map((x) => x.toString(16).padStart(2, '0')).join('')}`;
    expect([hex(post[0]!), hex(post[8]!)]).toEqual(['#440154', '#fde725']);
  });

  test('every one of the 256 entries equals rcfd-render’s colour at k / 255, alpha 255', () => {
    const map = rcfdRenderViridis();
    expect(lut).toHaveLength(4 * LUT_SIZE);
    const wrong: string[] = [];
    for (let k = 0; k < LUT_SIZE; k++) {
      const want = rustEval(map, k / (LUT_SIZE - 1));
      if (diff(entry(k), want) !== 0 || lut[4 * k + 3] !== 255) wrong.push(`${k}: ${entry(k)} vs ${want}`);
    }
    expect(wrong).toEqual([]);
  });

  test('the end points: entry 0 and 255 are rcfd-render’s colours at 0 and 1, and values at and beyond the ends take them', () => {
    const map = rcfdRenderViridis();
    expect(entry(0)).toEqual(rustEval(map, 0));
    expect(entry(255)).toEqual(rustEval(map, 1));
    expect(entry(0)).toEqual([68, 1, 84]);
    expect(entry(255)).toEqual([253, 231, 37]);
    for (const [lo, hi] of [
      [0, 1],
      [-0.0881664, 0.134064],
      [101325, 101325.5],
      [-3e8, 7e-3],
      [1e-30, 3e-30],
    ] as const) {
      const r = colourRange(Math.fround(lo), Math.fround(hi));
      // at and beyond both ends; a non-finite value takes the first stop in rcfd-render's eval, and in the table
      for (const v of [r.lo, Math.fround(r.lo - Math.abs(r.lo) - 1), r.hi, Math.fround(r.hi + Math.abs(r.hi) + 1), -Infinity, Infinity, Number.NaN]) {
        expect(colourOf(lut, v, r), `${v} in [${lo}, ${hi}]`).toEqual(rcfdRenderColour(v, r.lo, r.hi, map));
      }
      expect(colourOf(lut, r.lo, r)).toEqual(entry(0));
      expect(colourOf(lut, r.hi, r)).toEqual(entry(255));
    }
  });

  test('at the table points the colour of a value is exactly rcfd-render’s colour of that value', () => {
    // a range where the 256 table points are f32 values themselves: v = k, t = k / 255 exactly
    const map = rcfdRenderViridis();
    const r = colourRange(0, 255);
    for (let k = 0; k < LUT_SIZE; k++) {
      expect(lutIndex(k, r)).toBe(k);
      expect(colourOf(lut, k, r)).toEqual(rcfdRenderColour(k, 0, 255, map));
    }
  });

  test('over the full range the colour is within the stated tolerance of rcfd-render’s colour of the same value', () => {
    const map = rcfdRenderViridis();
    let worst = 0;
    let n = 0;
    let notEntry = 0;
    for (const [lo, hi] of [
      [0, 1],
      [-0.0881664, 0.134064],
      [101325, 101326],
      [-2.5e6, 4.75e7],
    ] as const) {
      const r = colourRange(Math.fround(lo), Math.fround(hi));
      const steps = 400_000;
      for (let i = 0; i <= steps; i++) {
        const v = Math.fround(r.lo + ((r.hi - r.lo) * i) / steps);
        const got = colourOf(lut, v, r);
        const want = rcfdRenderColour(v, r.lo, r.hi, map);
        worst = Math.max(worst, diff(got, want));
        // the table colour is rcfd-render's colour at the value's nearest table point
        if (diff(got, entry(lutIndex(v, r))) !== 0) notEntry++;
        n++;
      }
    }
    expect(n).toBe(4 * 400_001);
    expect(notEntry).toBe(0);
    expect(worst).toBeLessThanOrEqual(LUT_TOLERANCE_LEVELS);
    // the bound is reached (it is the true bound of a 256-entry table, not a loose one)
    expect(worst).toBe(LUT_TOLERANCE_LEVELS);
  });

  test('the entry of a value is the nearest table point to its place in the range', () => {
    const r = colourRange(Math.fround(-1.5), Math.fround(2.25));
    let worst = 0;
    for (let i = 0; i <= 100_000; i++) {
      const v = Math.fround(-1.5 + (3.75 * i) / 100_000);
      const t = (v - r.lo) / (r.hi - r.lo);
      worst = Math.max(worst, Math.abs(lutIndex(v, r) - t * 255));
    }
    expect(worst).toBeLessThanOrEqual(0.5 + 1e-4);
  });
});

test.describe('labels and readout', () => {
  test('tick labels are rcfd-studio’s {:.4e}', () => {
    expect(tickLabel(-0.08816641569137573)).toBe('-8.8166e-2');
    expect(tickLabel(0)).toBe('0.0000e0');
    expect(tickLabel(101325)).toBe('1.0133e5');
    expect(tickLabel(1.5e-7)).toBe('1.5000e-7');
    expect(ticks(0, 1)).toEqual([0, 0.25, 0.5, 0.75, 1]);
    expect(ticks(2, 2)).toEqual([2]);
  });

  test('the readout text reads back as the same f32, and is the shortest that does', () => {
    const f = new Float32Array([0.1, -0.08816641569137573, 101325.5, 1e-38, 3.4e38, 1 / 3, 7]);
    for (const v of f) {
      const s = f32Text(v);
      expect(Math.fround(Number(s))).toBe(v);
      const digits = s.replace(/^-/, '').replace(/e.*$/, '').replace('.', '').replace(/^0+/, '').length;
      if (digits > 1) expect(Math.fround(Number(Number(s).toPrecision(digits - 1)))).not.toBe(v);
    }
    expect(f32Text(Math.fround(0.1))).toBe('0.1');
    expect(f32Text(7)).toBe('7');
    expect(f32Text(Number.NaN)).toBe('NaN');
  });
});

test('field.css and field-dev.css use only defined tokens and no colour literals', () => {
  const defined = new Set(cssCustomProperties().map(([k]) => k));
  for (const sheet of [
    ['field', 'field.css'],
    ['dev', 'field-dev.css'],
  ]) {
    const css = readFileSync(path.join(appRoot, 'src', 'renderer', ...sheet), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    const used = [...css.matchAll(/var\((--[a-z0-9-]+)/g)].map((m) => m[1]!);
    expect(used.length).toBeGreaterThan(0);
    expect(used.filter((v) => !defined.has(v)), sheet.join('/')).toEqual([]);
    expect(css.match(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(|oklch\(/gi) ?? [], sheet.join('/')).toEqual([]);
  }
});
