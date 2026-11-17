// The arrays the field viewport draws (F-808, src/renderer/field/surface.ts), on a box surface built here the way
// `data.surface` builds one (each boundary quadrilateral a fan of two triangles, its owner cell per triangle): every
// corner carries its owner cell's stored value unchanged (G3), the range is that of the shown values, and the mesh grid
// is exactly the edges of the boundary faces (no fan diagonal, every face edge once).
import { expect, test } from '@playwright/test';
import { benchSoup, bounds, cornerPositions, cornerScalars, gridEdges, shownRange } from '../../src/renderer/field/surface.js';

/** The boundary of an nx x ny x nz box of unit cells, as data.surface gives it (points used, fans, owner cells). */
function boxSurface(nx: number, ny: number, nz: number) {
  const id = (i: number, j: number, k: number) => i + (nx + 1) * (j + (ny + 1) * k);
  const cell = (i: number, j: number, k: number) => i + nx * (j + ny * k);
  const quads: { q: [number, number, number, number]; c: number }[] = [];
  for (let j = 0; j < ny; j++)
    for (let k = 0; k < nz; k++) {
      quads.push({ q: [id(0, j, k), id(0, j, k + 1), id(0, j + 1, k + 1), id(0, j + 1, k)], c: cell(0, j, k) });
      quads.push({ q: [id(nx, j, k), id(nx, j + 1, k), id(nx, j + 1, k + 1), id(nx, j, k + 1)], c: cell(nx - 1, j, k) });
    }
  for (let i = 0; i < nx; i++)
    for (let k = 0; k < nz; k++) {
      quads.push({ q: [id(i, 0, k), id(i + 1, 0, k), id(i + 1, 0, k + 1), id(i, 0, k + 1)], c: cell(i, 0, k) });
      quads.push({ q: [id(i, ny, k), id(i, ny, k + 1), id(i + 1, ny, k + 1), id(i + 1, ny, k)], c: cell(i, ny - 1, k) });
    }
  for (let i = 0; i < nx; i++)
    for (let j = 0; j < ny; j++) {
      quads.push({ q: [id(i, j, 0), id(i, j + 1, 0), id(i + 1, j + 1, 0), id(i + 1, j, 0)], c: cell(i, j, 0) });
      quads.push({ q: [id(i, j, nz), id(i + 1, j, nz), id(i + 1, j + 1, nz), id(i, j + 1, nz)], c: cell(i, j, nz - 1) });
    }
  // points in order of first use, as data.surface numbers them
  const map = new Map<number, number>();
  const pts: number[] = [];
  const use = (g: number) => {
    let p = map.get(g);
    if (p === undefined) {
      p = map.size;
      map.set(g, p);
      const i = g % (nx + 1);
      const j = Math.floor(g / (nx + 1)) % (ny + 1);
      const k = Math.floor(g / ((nx + 1) * (ny + 1)));
      pts.push(i, j, k);
    }
    return p;
  };
  const tri: number[] = [];
  const cells: number[] = [];
  for (const { q, c } of quads) {
    const [a, b, d, e] = q.map(use) as [number, number, number, number];
    tri.push(a, b, d, a, d, e);
    cells.push(c, c);
  }
  return { points: new Float32Array(pts), triangles: new Uint32Array(tri), cells: new Uint32Array(cells), quads: quads.length };
}

test('corners carry the triangle’s owner cell value bit for bit; positions follow the triangles', () => {
  const s = boxSurface(6, 5, 4);
  const values = new Float32Array(6 * 5 * 4).map((_, i) => Math.fround(Math.sin(i) * 1e-3 + i));
  const sc = cornerScalars(s.cells, values);
  expect(sc.length).toBe(3 * s.cells.length);
  for (let t = 0; t < s.cells.length; t++) for (let c = 0; c < 3; c++) expect(Object.is(sc[3 * t + c], values[s.cells[t]!])).toBe(true);
  const pos = cornerPositions(s);
  expect(pos.length).toBe(9 * s.cells.length);
  for (let c = 0; c < s.triangles.length; c++) expect([...pos.subarray(3 * c, 3 * c + 3)]).toEqual([...s.points.subarray(3 * s.triangles[c]!, 3 * s.triangles[c]! + 3)]);
  // reused buffer of the right length (scrubbing)
  const into = new Float32Array(sc.length);
  expect(cornerScalars(s.cells, values, into)).toBe(into);
  expect(bounds(s.points)).toEqual({ min: [0, 0, 0], max: [6, 5, 4] });
});

test('the range is that of the shown values (the cells that own a surface triangle)', () => {
  const s = boxSurface(3, 3, 3);
  const values = new Float32Array(27).fill(1);
  values[13] = 1000; // the one interior cell: not on the surface, not in the range
  values[0] = -2;
  expect(shownRange(s.cells, values)).toEqual({ lo: -2, hi: 1 });
  expect(shownRange(new Uint32Array(0), values)).toBeNull();
});

test('the mesh grid is the edges of the boundary faces: each once, no fan diagonal', () => {
  for (const [nx, ny, nz] of [
    [6, 5, 4],
    [1, 1, 1],
    [3, 2, 7],
  ] as const) {
    const s = boxSurface(nx, ny, nz);
    const seg = gridEdges(s);
    // a closed surface of quadrilaterals: each face has 4 edges, each edge 2 faces
    expect(seg.length / 6, `${nx}x${ny}x${nz}`).toBe(2 * s.quads);
    const keys = new Set<string>();
    for (let i = 0; i < seg.length; i += 6) {
      const a = [...seg.subarray(i, i + 3)];
      const b = [...seg.subarray(i + 3, i + 6)];
      // a face edge is parallel to an axis and one cell long; a diagonal is not
      const d = a.map((x, k) => Math.abs(x - b[k]!));
      expect(d.filter((x) => x === 0).length).toBe(2);
      expect(Math.max(...d)).toBe(1);
      keys.add([a, b].map((p) => p.join(',')).sort().join('|'));
    }
    expect(keys.size).toBe(seg.length / 6);
  }
});

test('the P8 surface has 1e6 triangles and a value per triangle', () => {
  const { surface, values } = benchSoup(10, 5);
  expect(surface.cells.length).toBe(100);
  expect(values.length).toBe(100);
  expect(surface.triangles.length).toBe(300);
  const big = benchSoup();
  expect(big.surface.cells.length).toBe(1_000_000);
});
