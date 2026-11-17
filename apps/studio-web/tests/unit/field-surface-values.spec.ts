// F-819b: a field gathered on the surface (one value per triangle, src/shared/surface-values.ts) draws, ranges, counts
// and reads out exactly what the whole field did. On fields with repeated owner cells, negative zero, infinities and
// NaNs with payloads: the gathered words are the cells' words; the corner scalars, the range (F-808's, the min and max
// over the cells that own a surface triangle), the count of cells outside a fixed range and the value at each triangle
// equal those taken from the whole field; and the field worker's `surfaceField` answer equals its `field` answer.
import { expect, test } from '@playwright/test';
import { gatherSurface } from '../../src/shared/surface-values.js';
import { benchSoup, cornerScalars, cornerScalarsOf, rangeOf, shownRange } from '../../src/renderer/field/surface.js';
import { cellsOutside, cellsOutsideOf } from '../../src/renderer/field/viewExtras.js';
import { FieldPrep } from '../../src/renderer/field/prepare.js';

const words = (a: Float32Array) => Array.from(new Uint32Array(a.buffer, a.byteOffset, a.length));

/** A field of `n` cells with the awkward values in it, and owner cells that repeat and skip (a cell owns 0 to 4 triangles). */
function fixture(n = 997, nTri = 3001) {
  const values = new Float32Array(n);
  const w = new Uint32Array(values.buffer);
  for (let c = 0; c < n; c++) values[c] = Math.sin(c * 12.9898) * 43758.5453 - Math.floor(Math.sin(c * 12.9898) * 43758.5453) - 0.5;
  values[3] = -0;
  values[5] = 0;
  values[7] = Number.POSITIVE_INFINITY;
  values[11] = Number.NEGATIVE_INFINITY;
  w[13] = 0x7fc00001; // a quiet NaN with a payload
  w[17] = 0xffc12345; // a negative NaN with a payload
  values[19] = 3.4028234663852886e38;
  values[23] = 1.401298464324817e-45;
  const cells = new Uint32Array(nTri);
  let x = 1;
  for (let t = 0; t < nTri; t++) {
    x = (x * 1103515245 + 12345) >>> 0;
    cells[t] = x % n;
  }
  for (const c of [3, 5, 7, 11, 13, 17, 19, 23]) if (c < n && 7 * c < nTri) cells[7 * c] = c; // every awkward value is on the surface
  return { values, cells };
}

test('the gathered values are the owner cells’ words, bit for bit (NaN payloads too)', () => {
  const { values, cells } = fixture();
  const g = gatherSurface(cells, values);
  const vw = words(values);
  expect(g.length).toBe(cells.length);
  expect(words(g)).toEqual(Array.from(cells, (c) => vw[c]!));
  // a view at an offset into a larger buffer
  const big = new Float32Array(values.length + 5);
  big.set(values, 5);
  expect(words(gatherSurface(cells, big.subarray(5)))).toEqual(words(g));
  expect(() => gatherSurface(new Uint32Array([0, values.length]), values)).toThrow(/names cell 997/);
});

test('corner scalars, range, cells outside and the value at each triangle equal those of the whole field', () => {
  for (const f of [fixture(), fixture(5, 40), { values: benchSoup(30, 10).values, cells: benchSoup(30, 10).surface.cells }]) {
    const g = gatherSurface(f.cells, f.values);
    expect(words(cornerScalarsOf(g))).toEqual(words(cornerScalars(f.cells, f.values)));
    const into = new Float32Array(3 * f.cells.length);
    expect(cornerScalarsOf(g, into)).toBe(into);
    const a = rangeOf(g);
    const b = shownRange(f.cells, f.values);
    expect(a).toEqual(b);
    if (a && b) expect([Object.is(a.lo, b.lo), Object.is(a.hi, b.hi)]).toEqual([true, true]);
    for (const [lo, hi] of [[-0.25, 0.25], [-1, 1], [0, 0.1]] as const) expect(cellsOutsideOf(f.cells, g, lo, hi)).toEqual(cellsOutside(f.cells, f.values, lo, hi));
    // the readout: FieldCanvas reads values[triangle] of surface values where it read values[cells[triangle]]
    for (let t = 0; t < f.cells.length; t++) {
      if (!Object.is(g[t], f.values[f.cells[t]!])) throw new Error(`triangle ${t}: ${g[t]} != ${f.values[f.cells[t]!]}`);
    }
  }
  expect(rangeOf(new Float32Array(0))).toBeNull();
});

test('the field worker: surfaceField answers what field answers, and refuses values that do not fit the surface', async () => {
  const { surface, values } = benchSoup(40, 20);
  // a field with more cells than the surface's triangles own, so the gather drops some
  const nCells = Math.max(...surface.cells) + 7;
  const full = new Float32Array(nCells);
  full.set(values);
  for (let c = values.length; c < nCells; c++) full[c] = 1e30;
  const replies: { op: string; scalars?: Float32Array; values?: Float32Array; range?: unknown; message?: string }[] = [];
  const scope: { onmessage: ((e: { data: unknown }) => void) | null; postMessage: (m: unknown) => void } = { onmessage: null, postMessage: (m) => replies.push(m as never) };
  (globalThis as { self?: unknown }).self = scope;
  try {
    // a query of its own, so this file runs the module afresh even after another file of this worker imported it
    await import('../../src/renderer/field/prepare.worker.ts?f819b' as string);
  } finally {
    delete (globalThis as { self?: unknown }).self;
  }
  expect(scope.onmessage).not.toBeNull();
  scope.onmessage!({ data: { op: 'surface', id: 1, points: surface.points.slice(), triangles: surface.triangles.slice(), cells: surface.cells.slice(), nCells } });
  scope.onmessage!({ data: { op: 'field', id: 2, surface: 1, values: full.slice() } });
  const g = gatherSurface(surface.cells, full);
  scope.onmessage!({ data: { op: 'surfaceField', id: 3, surface: 1, values: g.slice() } });
  scope.onmessage!({ data: { op: 'surfaceField', id: 4, surface: 1, values: g.slice(1) } });
  scope.onmessage!({ data: { op: 'surfaceField', id: 5, surface: 99, values: g.slice() } });
  const [, f, s, short, other] = replies;
  expect(s!.op).toBe('surfaceField');
  expect(words(s!.scalars!)).toEqual(words(f!.scalars!));
  expect(s!.range).toEqual(f!.range);
  expect(f!.range).toEqual(shownRange(surface.cells, full));
  expect(words(s!.values!)).toEqual(words(g));
  expect([short!.op, short!.message]).toEqual(['error', `${g.length - 1} surface values for ${g.length} triangles`]);
  expect(other!.op).toBe('error');

  // without a Worker (this process), FieldPrep makes the same arrays itself
  const prep = new FieldPrep();
  const ps = await prep.surface(surface, nCells);
  const a = await prep.field(ps, full.slice());
  const b = await prep.surfaceField(ps, g.slice());
  expect(prep.onMainThread).toBe(true);
  expect(words(b.scalars)).toEqual(words(a.scalars));
  expect(b.range).toEqual(a.range);
  await expect(prep.surfaceField(ps, g.slice(2))).rejects.toThrow(/surface values for/);
  prep.dispose();
});
