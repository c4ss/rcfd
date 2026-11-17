// The derived pictures of the field viewport (F-809; guardrails G3, G6), in Node, against the engine, on real stores:
// the f447 boxes made afresh by the Rust example `f447_bench gen` (3D, with the Rust reader's reference bytes), the
// quick-start cavity (2D) and the axial-fan passage (3D) of example-projects, read by the TypeScript reader of
// store.rts (route R2, bit for bit the Rust reader's, F-804).
//
// - T1: the Studio's 2D verdict equals the engine's (`render_field` of the project server draws 2D cases only and
//   refuses a 3D one), on the quick-start and axial-fan runs;
// - availability: a cut plane on a 2D case, an iso-surface and streamlines everywhere, and vectors without their
//   components are unavailable, each with its reason in words (G6); the rest are available;
// - T2: the cut-plane rule of the removed rcfd-render (n . p <= d; F-838 removed the crate and the check of its shader
//   text), and the Studio's mirror of it;
// - T3: the constants of overlay_vectors and render.png read from the Rust source; every arrow against an independent
//   f64 recomputation (the same arrows, each point within 2^-20 of the largest extent) and the stored components bit
//   for bit (the Rust reader's reference bytes);
// - the controls' pictograms are on the 14-unit grid.
//
// The tolerances are the ones registered in ADR 0801, addendum F-809, before the code. Needs `f447_bench` for the boxes
// and `rcfd-mcp` for the engine's verdict; without them those tests are skipped and say why.
import { expect, test } from '@playwright/test';
import { cpSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { Store } from '../../src/main/data/tsstore.js';
import { ProjectServer } from '../../src/main/project/server.js';
import type { FieldInfo } from '../../src/shared/data.js';
import { pictogram } from '../../src/renderer/design/tokens.js';
import {
  availability,
  clipPlane,
  DERIVED_ICON,
  DERIVED_KINDS,
  FRONT_SIGN,
  keeps,
  meshLayers,
  VECTOR_ARROW,
  VECTOR_DENSITY,
  VECTOR_HEAD_BACK,
  VECTOR_HEAD_SIDE,
  vectorGlyphs,
  type Layers,
} from '../../src/renderer/field/derived.js';
import type { SurfaceArrays } from '../../src/renderer/field/surface.js';
import { findBench, generate, NO_BENCH, refBytes, storeDir } from '../support/f447.js';
import { findMcp, NO_MCP, repoRoot } from '../support/project.js';

test.describe.configure({ mode: 'serial' });

const bench = findBench();
const mcp = findMcp();
const SNAPSHOT = 19;
const rust = (rel: string): string => readFileSync(path.join(repoRoot, rel), 'utf8');

/** The first run of an example project that has a field store. */
function exampleRun(project: string): { id: string; dir: string } {
  const runs = path.join(repoRoot, 'example-projects', project, 'runs');
  const id = readdirSync(runs).sort()[0]!;
  return { id, dir: path.join(runs, id, 'fields') };
}

interface Opened {
  readonly surface: SurfaceArrays;
  readonly nCells: number;
  readonly fields: FieldInfo[];
  readonly field: (name: string, snapshot?: number) => Float32Array;
  readonly last: number;
}

function open(dir: string): Opened {
  const s = new Store(dir);
  const raw = s.surface();
  const surface = { points: new Float32Array(raw.points), triangles: new Uint32Array(raw.triangles), cells: new Uint32Array(raw.cells) };
  const last = s.snapshots.length - 1;
  const fields: FieldInfo[] = s.fields(last).map((f) => ({ name: f.name, unit: f.unit, location: f.location, n: f.n, precision: f.precision, codec: f.codec }) as FieldInfo);
  return { surface, nCells: s.nCells, fields, field: (name, snapshot = last) => new Float32Array(s.field(snapshot, name)), last };
}

let root = '';
const boxes: Record<string, Opened> = {};

test.beforeAll(async () => {
  if (!bench) return;
  test.setTimeout(300_000);
  root = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f809-unit-'));
  await generate(bench, root, ['tiny', '1e5'], 280_000);
  for (const size of ['tiny', '1e5']) boxes[size] = open(storeDir(root, size, 'raw'));
});

test.afterAll(() => {
  if (root) rmSync(root, { recursive: true, force: true });
});

// ---- T1 ------------------------------------------------------------------------------------------------------------

test('T1: the 2D verdict equals the engine’s render_field on the quick-start cavity (2D) and the axial-fan passage (3D)', async () => {
  test.skip(mcp === null, NO_MCP);
  test.setTimeout(120_000);
  const scratch = mkdtempSync(path.join(os.tmpdir(), 'rcfd-f809-t1-'));
  try {
    for (const [project, field] of [
      ['quick-start', 'ux'],
      ['axial-fan', 'ux'],
    ] as const) {
      const copy = path.join(scratch, project);
      cpSync(path.join(repoRoot, 'example-projects', project), copy, { recursive: true });
      const run = exampleRun(project);
      const o = open(path.join(copy, 'runs', run.id, 'fields'));
      const studio = meshLayers(o.surface, o.nCells).twoD;
      const server = await ProjectServer.start(mcp!, copy);
      let engine: boolean;
      try {
        await server.callTool('render_field', { run: run.id, field, width: 64 });
        engine = true;
      } catch (e) {
        expect(String((e as Error).message)).toMatch(/2D cases only \(one cell layer\)/);
        engine = false;
      } finally {
        await server.close();
      }
      expect(studio, `${project}: the Studio's verdict is the engine's`).toBe(engine);
      expect(engine, project).toBe(project === 'quick-start');
    }
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});

test('T1: the f447 boxes are 3D', () => {
  test.skip(!bench, NO_BENCH);
  for (const size of ['tiny', '1e5']) {
    const l = meshLayers(boxes[size]!.surface, boxes[size]!.nCells);
    expect(l.twoD, size).toBe(false);
  }
});

// ---- availability (G6) -----------------------------------------------------------------------------------------------

test('a cut plane on a 2D case, iso-surfaces and streamlines everywhere, and vectors without components are unavailable, with the reason', () => {
  const qs = open(exampleRun('quick-start').dir);
  const layers = meshLayers(qs.surface, qs.nCells);
  expect(layers.twoD).toBe(true);
  const ctx2 = { layers, fields: qs.fields, nCells: qs.nCells };
  const cut2 = availability('cut', ctx2);
  expect(cut2.ok).toBe(false);
  if (!cut2.ok) expect(cut2.reason).toMatch(/2D case \(one cell layer, thinnest along z\)/);
  // the quick-start store holds ux, uy, p: the in-plane arrows apply
  expect(availability('vectors', ctx2)).toEqual({ ok: true });
  for (const k of ['iso', 'streamlines'] as const) {
    const a = availability(k, ctx2);
    expect(a.ok, k).toBe(false);
    if (!a.ok) expect(a.reason, k).toMatch(/The engine has no/);
  }
  const iso2 = availability('iso', ctx2);
  if (!iso2.ok) expect(iso2.reason).toMatch(/2D case/);
  // without uy (a store of p only) the arrows say which component is missing
  const noUy = availability('vectors', { ...ctx2, fields: qs.fields.filter((f) => f.name !== 'uy') });
  expect(noUy.ok).toBe(false);
  if (!noUy.ok) expect(noUy.reason).toMatch(/no cell field uy/);
  // nothing applies before the surface is read
  for (const k of DERIVED_KINDS) expect(availability(k, { layers: null, fields: [], nCells: 0 }).ok, k).toBe(false);
});

test('on a 3D store the cut plane and the arrows apply; without uz the arrows do not', () => {
  test.skip(!bench, NO_BENCH);
  const b = boxes['tiny']!;
  const ctx = { layers: meshLayers(b.surface, b.nCells), fields: b.fields, nCells: b.nCells };
  expect(availability('cut', ctx)).toEqual({ ok: true });
  expect(availability('vectors', ctx)).toEqual({ ok: true });
  const noUz = availability('vectors', { ...ctx, fields: b.fields.filter((f) => f.name !== 'uz') });
  expect(noUz.ok).toBe(false);
  if (!noUz.ok) expect(noUz.reason).toMatch(/no cell field uz.*arrows of a 3D case need ux, uy, uz/);
  expect(availability('iso', ctx).ok).toBe(false);
  expect(availability('streamlines', ctx).ok).toBe(false);
});

// ---- T2 ------------------------------------------------------------------------------------------------------------

test('T2: the cut plane is rcfd-render’s rule, only points with n . p <= d are drawn', () => {
  // the reference was rcfd-render's shader (discard where dot(n, p) > d); the crate was removed (F-838), the rule stays
  // the Studio's shader mask says the same (FieldCanvas.tsx)
  expect(rust('apps/studio-web/src/renderer/field/FieldCanvas.tsx')).toContain('dot(vec3(n), positionWorld).lessThanEqual(d)');
  // the planes: n = e_a keeps p[a] <= d; n = -e_a keeps p[a] >= d; d is the f32 of the position
  expect(clipPlane({ axis: 1, at: 0.3, keep: 'below' })).toEqual({ n: [0, 1, 0], d: Math.fround(0.3) });
  expect(clipPlane({ axis: 2, at: 0.3, keep: 'above' })).toEqual({ n: [0, 0, -1], d: -Math.fround(0.3) });
  const at = Math.fround(0.3);
  for (const axis of [0, 1, 2] as const) {
    for (const x of [at - 1e-7, at, at + 1e-7, Math.fround(at + 1e-7), -5, 5]) {
      const p = [0.123, -7, 9];
      p[axis] = x;
      expect(keeps({ axis, at, keep: 'below' }, p), `${axis} ${x} below`).toBe(x <= at);
      expect(keeps({ axis, at, keep: 'above' }, p), `${axis} ${x} above`).toBe(x >= at);
    }
  }
  expect(keeps(null, [1e30, 0, 0])).toBe(true);
});

// ---- T3 ------------------------------------------------------------------------------------------------------------

test('T3: the arrows’ constants are overlay_vectors’ and render.png’s, read from the Rust source', () => {
  const render = rust('crates/rcfd-post/src/render.rs');
  expect(render).toContain('let arrow = (fr.w as f64 / density as f64 * 0.8).max(6.0);');
  expect(render).toContain('let s = arrow * inplane(c) / vmax;');
  expect(render).toContain('let vmax = (0..mesh.n_cells()).map(inplane).fold(0.0, f64::max).max(1e-300);');
  expect(render).toContain('let gi = (((cc[c][v.u] - bu[0]) / (bu[1] - bu[0]).max(1e-300)) * density as f64) as usize;');
  expect(render).toContain('if !taken.insert((gi, gj)) {');
  expect(render).toContain('let (hx, hy) = (-dx * 0.3 * s, -dy * 0.3 * s);');
  expect(render).toContain('line(image, b, (b.0 + hx - hy * 0.5, b.1 + hy + hx * 0.5), [0, 0, 0, 255]);');
  expect(rust('crates/rcfd-api/src/service.rs')).toContain('overlay_vectors(&sim.geom.mesh, &u, &opt, 24, &mut r.image)');
  expect([VECTOR_DENSITY, VECTOR_ARROW, VECTOR_HEAD_BACK, VECTOR_HEAD_SIDE]).toEqual([24, 0.8, 0.3, 0.5]);
});

/** An independent f64 recomputation of T3 (written apart from derived.ts): the arrows as [triangle, cell, 9 points]. */
function referenceArrows(s: SurfaceArrays, comps: (Float32Array | null)[], l: Layers, nCells: number) {
  const P = (i: number): number[] => [s.points[3 * i]!, s.points[3 * i + 1]!, s.points[3 * i + 2]!];
  const inPlane = l.twoD ? [0, 1, 2].filter((k) => k !== l.thin) : [0, 1, 2];
  const vec = (c: number): number[] => [0, 1, 2].map((k) => (inPlane.includes(k) ? comps[k]![c]! : 0));
  let vmax = 0;
  for (let c = 0; c < nCells; c++) vmax = Math.max(vmax, Math.sqrt(vec(c).reduce((a, x) => a + x * x, 0)));
  const ext = [0, 1, 2].map((k) => l.max[k]! - l.min[k]!);
  const step = Math.max(...ext) / 24;
  const front = l.twoD ? (FRONT_SIGN[l.thin] > 0 ? l.max[l.thin] : l.min[l.thin]) : NaN;
  const seen = new Set<string>();
  const out: Array<{ tri: number; cell: number; pts: number[] }> = [];
  for (let t = 0; t < s.cells.length; t++) {
    const [a, b, c] = [P(s.triangles[3 * t]!), P(s.triangles[3 * t + 1]!), P(s.triangles[3 * t + 2]!)];
    if (l.twoD && ![a, b, c].every((q) => q[l.thin] === front)) continue;
    const m = [0, 1, 2].map((k) => (a[k]! + b[k]! + c[k]!) / 3);
    const bin = [0, 1, 2].map((k) => Math.max(0, Math.floor(((m[k]! - l.min[k]!) / Math.max(ext[k]!, 1e-300)) * 24))).join(',');
    if (seen.has(bin)) continue;
    seen.add(bin);
    const cell = s.cells[t]!;
    const v = vec(cell);
    const mag = Math.sqrt(v[0]! ** 2 + v[1]! ** 2 + v[2]! ** 2);
    if (!(mag > 0) || !Number.isFinite(mag)) continue;
    const len = (0.8 * step * mag) / vmax;
    const u = v.map((x) => x / mag);
    const tip = m.map((x, k) => x + len * u[k]!);
    const n = [
      (b[1]! - a[1]!) * (c[2]! - a[2]!) - (b[2]! - a[2]!) * (c[1]! - a[1]!),
      (b[2]! - a[2]!) * (c[0]! - a[0]!) - (b[0]! - a[0]!) * (c[2]! - a[2]!),
      (b[0]! - a[0]!) * (c[1]! - a[1]!) - (b[1]! - a[1]!) * (c[0]! - a[0]!),
    ];
    let side = [n[1]! * u[2]! - n[2]! * u[1]!, n[2]! * u[0]! - n[0]! * u[2]!, n[0]! * u[1]! - n[1]! * u[0]!];
    if (Math.hypot(...side) <= 1e-9 * Math.hypot(...n)) {
      const k = [0, 1, 2].reduce((best, q) => (Math.abs(u[q]!) < Math.abs(u[best]!) ? q : best), 0);
      const e = [0, 0, 0];
      e[k] = 1;
      side = [u[1]! * e[2]! - u[2]! * e[1]!, u[2]! * e[0]! - u[0]! * e[2]!, u[0]! * e[1]! - u[1]! * e[0]!];
    }
    const sl = Math.hypot(...side);
    side = side.map((x) => x / sl);
    const h = 0.3 * len;
    const w = 0.5 * h;
    const h1 = tip.map((x, k) => x - h * u[k]! + w * side[k]!);
    const h2 = tip.map((x, k) => x - h * u[k]! - w * side[k]!);
    out.push({ tri: t, cell, pts: [...m, ...tip, ...tip, ...h1, ...tip, ...h2] });
  }
  return { arrows: out, vmax, big: Math.max(...ext) };
}

function checkArrows(name: string, o: Opened, comps: (Float32Array | null)[], expectComponents: (cell: number) => void) {
  const layers = meshLayers(o.surface, o.nCells);
  const g = vectorGlyphs(o.surface, comps, layers, o.nCells);
  const ref = referenceArrows(o.surface, comps, layers, o.nCells);
  // the scale: the same largest length (hypot and a square root of the sum differ in the last place at most)
  expect(Math.abs(g.vmax - ref.vmax), name).toBeLessThanOrEqual(4 * Number.EPSILON * ref.vmax);
  expect(ref.arrows.length, `${name}: some arrows`).toBeGreaterThan(10);
  expect([...g.triangles], `${name}: the same arrows`).toEqual(ref.arrows.map((a) => a.tri));
  expect([...g.cells], name).toEqual(ref.arrows.map((a) => a.cell));
  expect(g.segments.length).toBe(18 * ref.arrows.length);
  const tol = 2 ** -20 * ref.big;
  let worst = 0;
  ref.arrows.forEach((a, i) => {
    for (let k = 0; k < 18; k++) worst = Math.max(worst, Math.abs(g.segments[18 * i + k]! - a.pts[k]!));
  });
  expect(worst, `${name}: every drawn point within 2^-20 of the largest extent`).toBeLessThanOrEqual(tol);
  for (const c of g.cells) expectComponents(c);
  return g;
}

test('T3: the arrows of the f447 boxes (3D) against the recomputation, components bit for bit the Rust reader’s', () => {
  test.skip(!bench, NO_BENCH);
  for (const size of ['tiny', '1e5']) {
    const o = boxes[size]!;
    const comps = (['ux', 'uy', 'uz'] as const).map((f) => o.field(f, SNAPSHOT));
    const refs = (['ux', 'uy', 'uz'] as const).map((f) => {
      const b = refBytes(root, size, SNAPSHOT, f);
      return new Float32Array(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
    });
    checkArrows(size, o, comps, (c) => {
      for (let k = 0; k < 3; k++) expect(Object.is(comps[k]![c], refs[k]![c]), `${size} cell ${c} component ${k}`).toBe(true);
    });
  }
});

test('T3: the in-plane arrows of the quick-start cavity (2D) lie on the face the view looks at', () => {
  const o = open(exampleRun('quick-start').dir);
  const layers = meshLayers(o.surface, o.nCells);
  const comps: (Float32Array | null)[] = [o.field('ux'), o.field('uy'), null];
  const g = checkArrows('quick-start', o, comps, () => undefined);
  // every point of every arrow is on the front plane (z = max), so the arrows carry no z component
  const front = layers.max[2];
  for (let i = 2; i < g.segments.length; i += 3) expect(g.segments[i]).toBe(Math.fround(front));
});

// ---- the pictograms ------------------------------------------------------------------------------------------------

test('the derived-picture pictograms are on the 14-unit grid, at 0, 45 and 90 degrees', () => {
  for (const kind of DERIVED_KINDS) {
    const d = DERIVED_ICON[kind];
    const tokens = d.match(/[MLHVZ]|-?\d+/g) ?? [];
    expect(tokens.join(''), kind).toBe(d.replace(/[\s,]/g, ''));
    let i = 0;
    let cur = [0, 0];
    let start = [0, 0];
    let cmd = '';
    let n = 0;
    while (i < tokens.length) {
      if (/[MLHVZ]/.test(tokens[i]!)) cmd = tokens[i++]!;
      let next: number[];
      if (cmd === 'Z') next = start;
      else if (cmd === 'M' || cmd === 'L') next = [Number(tokens[i++]), Number(tokens[i++])];
      else if (cmd === 'H') next = [Number(tokens[i++]), cur[1]!];
      else next = [cur[0]!, Number(tokens[i++])];
      for (const v of next) expect(v >= 0 && v <= pictogram.units && Number.isInteger(v), `${kind}: ${v}`).toBe(true);
      if (cmd === 'M') {
        start = next;
        cmd = 'L';
      } else {
        const deg = ((Math.atan2(next[1]! - cur[1]!, next[0]! - cur[0]!) * 180) / Math.PI + 360) % 180;
        expect([0, 45, 90, 135].some((g) => Math.abs(deg - g) < 1e-9), `${kind}: ${cur} -> ${next}`).toBe(true);
        n++;
        if (cmd === 'Z') cmd = '';
      }
      cur = next;
    }
    expect(n, kind).toBeGreaterThan(2);
  }
});
