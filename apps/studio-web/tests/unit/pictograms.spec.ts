// F-802: the item of this spec, by the commit that added it (d52d689f, 2026-11-16).
// The pictogram grid of DESIGN.md (Shapes): every state pictogram and icon on a 14-unit square, with strokes only at
// 0, 45 and 90 degrees between whole grid points; the queued ring is the one circle. The state shapes are the ones
// recorded in .impeccable/design.json.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { pictogram } from '../../src/renderer/design/tokens.js';
import { ICON_NAMES, ICON_PATHS, RUN_STATES, STATE_SHAPES, STATE_WORD } from '../../src/renderer/ui/pictogramPaths.js';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..', '..');
const sidecar = JSON.parse(readFileSync(path.join(repo, '.impeccable', 'design.json'), 'utf8')) as {
  extensions: { pictogramGrid: { states: Record<string, string> } };
};

type Pt = readonly [number, number];

/** The segments of an absolute path of M, L, H, V and Z commands (the only ones the grid uses). */
function segments(d: string): Array<[Pt, Pt]> {
  const tokens = d.match(/[MLHVZ]|-?\d+(\.\d+)?/g) ?? [];
  expect(tokens.join(''), `only M L H V Z in "${d}"`).toBe(d.replace(/[\s,]/g, ''));
  const out: Array<[Pt, Pt]> = [];
  let i = 0;
  let cur: Pt = [0, 0];
  let start: Pt = [0, 0];
  let cmd = '';
  const num = (): number => Number(tokens[i++]);
  while (i < tokens.length) {
    if (/[MLHVZ]/.test(tokens[i]!)) cmd = tokens[i++]!;
    if (cmd === 'Z') {
      out.push([cur, start]);
      cur = start;
      continue;
    }
    let next: Pt;
    if (cmd === 'M' || cmd === 'L') next = [num(), num()];
    else if (cmd === 'H') next = [num(), cur[1]];
    else next = [cur[0], num()];
    if (cmd === 'M') {
      start = next;
      cmd = 'L';
    } else out.push([cur, next]);
    cur = next;
  }
  return out;
}

function checkOnGrid(name: string, d: string) {
  const segs = segments(d);
  expect(segs.length, name).toBeGreaterThan(0);
  for (const [a, b] of segs) {
    for (const v of [...a, ...b]) {
      expect(Number.isInteger(v), `${name}: ${v} is a whole grid unit`).toBe(true);
      expect(v >= 0 && v <= pictogram.units, `${name}: ${v} inside the ${pictogram.units}-unit square`).toBe(true);
    }
    const dx = b[0] - a[0];
    const dy = b[1] - a[1];
    expect(dx !== 0 || dy !== 0, `${name}: no zero-length segment`).toBe(true);
    const deg = ((Math.atan2(dy, dx) * 180) / Math.PI + 360) % 180;
    expect(
      pictogram.angles.some((g) => Math.abs(deg - g) < 1e-9) || Math.abs(deg - 135) < 1e-9,
      `${name}: segment ${a} -> ${b} at ${deg} degrees`,
    ).toBe(true);
  }
}

test('the five run states, each with its word', () => {
  expect([...RUN_STATES]).toEqual(['queued', 'running', 'converged', 'finished', 'failed']);
  for (const s of RUN_STATES) expect(STATE_WORD[s].toLowerCase()).toBe(s);
});

test('the state pictograms are the shapes recorded in design.json', () => {
  const recorded = sidecar.extensions.pictogramGrid.states;
  expect(Object.keys(recorded).sort()).toEqual([...RUN_STATES].sort());
  for (const s of RUN_STATES) {
    const shape = STATE_SHAPES[s];
    const svg = 'circle' in shape ? `<circle cx="${shape.circle.cx}" cy="${shape.circle.cy}" r="${shape.circle.r}"/>` : `<path d="${shape.path}"/>`;
    expect(svg, s).toBe(recorded[s]);
  }
});

test('every state pictogram is on the grid; the queued ring is the one circle', () => {
  for (const s of RUN_STATES) {
    const shape = STATE_SHAPES[s];
    if ('circle' in shape) {
      expect(s).toBe('queued');
      const { cx, cy, r } = shape.circle;
      expect(cx - r >= 0 && cx + r <= pictogram.units && cy - r >= 0 && cy + r <= pictogram.units).toBe(true);
    } else checkOnGrid(s, shape.path);
  }
});

test('every icon is on the grid', () => {
  expect(Object.keys(ICON_PATHS).sort()).toEqual([...ICON_NAMES].sort());
  for (const n of ICON_NAMES) checkOnGrid(n, ICON_PATHS[n].path);
});
