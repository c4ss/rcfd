// The derived pictures of the field viewport (F-809; guardrails G3, G6), in Node, against the engine, on real stores:
// the f447 boxes made afresh by the Rust example `f447_bench gen` (3D, with the Rust reader's reference bytes), the
// quick-start cavity (2D) and the axial-fan passage (3D) of example-projects, read by the TypeScript reader of
// store.rts (route R2, bit for bit the Rust reader's, F-804).
//
// - T1: the Studio's 2D verdict equals the engine's (`render_field` of the project server draws 2D cases only and
//   refuses a 3D one), on the quick-start and axial-fan runs;
// - availability: a cut plane and an iso-surface on a 2D case, vectors and streamlines without their components, and
//   every derived picture of a store read through the service (the engine makes them of a local store, ADR 0738) are
//   unavailable, each with its reason in words (G6); the rest are available;
// - T2: the cut-plane rule of the removed rcfd-render (n . p <= d; F-838 removed the crate and the check of its shader
//   text), and the Studio's statement of it (the mesh grid's mask; the cut surface is the engine's, G2 below);
// - T3: the constants of overlay_vectors and render.png read from the Rust source; every arrow against an independent
//   f64 recomputation (the same arrows, each point within 2^-20 of the largest extent) and the stored components bit
//   for bit (the Rust reader's reference bytes);
// - the controls' pictograms are on the 14-unit grid;
// - G2 (ADR 0738): the engine's `cut_surface` (`rcfd post derive`) is the Studio's T2 and T3 sets: the surface
//   triangles, the triangles the cut keeps whole, and the arrows, on the quick-start cavity and the axial-fan passage.
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
import { findRcfd, NO_RCFD } from '../support/rcfd.js';
import { cutOf, isoOf, linesOf, readGlb as readStudioGlb } from '../../src/renderer/field/glb.js';
import { execFileSync } from 'node:child_process';

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

test('a cut plane and an iso-surface on a 2D case, vectors and streamlines without components, and the service route are unavailable, with the reason', () => {
  const qs = open(exampleRun('quick-start').dir);
  const layers = meshLayers(qs.surface, qs.nCells);
  expect(layers.twoD).toBe(true);
  const ctx2 = { layers, fields: qs.fields, nCells: qs.nCells, route: 'local' as const, storedField: 'ux' };
  const cut2 = availability('cut', ctx2);
  expect(cut2.ok).toBe(false);
  if (!cut2.ok) expect(cut2.reason).toMatch(/2D case \(one cell layer, thinnest along z\)/);
  // the quick-start store holds ux, uy, p: the in-plane arrows and the streamlines (ADR 0738) apply
  expect(availability('vectors', ctx2)).toEqual({ ok: true });
  expect(availability('streamlines', ctx2)).toEqual({ ok: true });
  const iso2 = availability('iso', ctx2);
  expect(iso2.ok).toBe(false);
  if (!iso2.ok) expect(iso2.reason).toBe('A 2D case (one cell layer) has no volume for an iso-surface.');
  // without uy the streamlines say which component is missing too
  const noUyS = availability('streamlines', { ...ctx2, fields: qs.fields.filter((f) => f.name !== 'uy') });
  if (noUyS.ok) throw new Error('streamlines without uy');
  expect(noUyS.reason).toMatch(/no cell field uy; streamlines of a 2D case need ux, uy/);
  // a store read through the service: the engine makes derived pictures of local stores only
  for (const k of ['vectors', 'streamlines'] as const) {
    const a = availability(k, { ...ctx2, route: 'service' });
    if (a.ok) throw new Error(`${k} on the service route`);
    expect(a.reason, k).toMatch(/^The engine makes derived pictures of a store on this machine \(rcfd post derive\)/);
  }
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
  const ctx = { layers: meshLayers(b.surface, b.nCells), fields: b.fields, nCells: b.nCells, route: 'local' as const, storedField: 'p' };
  expect(availability('cut', ctx)).toEqual({ ok: true });
  expect(availability('vectors', ctx)).toEqual({ ok: true });
  const noUz = availability('vectors', { ...ctx, fields: b.fields.filter((f) => f.name !== 'uz') });
  expect(noUz.ok).toBe(false);
  if (!noUz.ok) expect(noUz.reason).toMatch(/no cell field uz.*arrows of a 3D case need ux, uy, uz/);
  // ADR 0738: the engine's iso-surface and streamlines apply; an iso-surface of values that are not one stored field
  // (a difference of two runs) does not, nor any picture on the service route
  expect(availability('iso', ctx)).toEqual({ ok: true });
  expect(availability('streamlines', ctx)).toEqual({ ok: true });
  const diff = availability('iso', { ...ctx, storedField: null });
  if (diff.ok) throw new Error('an iso-surface of a difference');
  expect(diff.reason).toMatch(/not one stored cell field/);
  for (const k of DERIVED_KINDS) expect(availability(k, { ...ctx, route: 'service' }).ok, k).toBe(false);
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

// ---- G2 (ADR 0738): the engine's cut and arrows are the Studio's ------------------------------------------------------

interface GlbPrimitive {
  readonly mode: number;
  readonly attributes: Record<string, Float32Array | Uint32Array>;
  readonly extras: Record<string, unknown>;
}

/** The primitives of a GLB the engine wrote (non-indexed, f32 and u32 accessors only, as rcfd_post::picture writes). */
function readGlb(bytes: Buffer): { asset: Record<string, unknown>; primitives: GlbPrimitive[]; bytes: Buffer } {
  const u32 = (o: number) => bytes.readUInt32LE(o);
  expect(u32(0)).toBe(0x46546c67);
  const jsonLen = u32(12);
  const doc = JSON.parse(bytes.subarray(20, 20 + jsonLen).toString('utf8'));
  const binAt = 20 + jsonLen + 8;
  const read = (i: number): Float32Array | Uint32Array => {
    const a = doc.accessors[i];
    const v = doc.bufferViews[a.bufferView];
    const comps = a.type === 'VEC3' ? 3 : 1;
    const off = binAt + (v.byteOffset ?? 0) + (a.byteOffset ?? 0);
    const b = bytes.buffer.slice(bytes.byteOffset + off, bytes.byteOffset + off + 4 * comps * a.count);
    return a.componentType === 5126 ? new Float32Array(b) : new Uint32Array(b);
  };
  const primitives: GlbPrimitive[] = [];
  for (const m of doc.meshes ?? []) {
    for (const p of m.primitives) {
      const attributes: Record<string, Float32Array | Uint32Array> = {};
      for (const [k, i] of Object.entries(p.attributes)) attributes[k] = read(i as number);
      primitives.push({ mode: p.mode, attributes, extras: p.extras ?? {} });
    }
  }
  return { asset: doc.asset, primitives, bytes };
}

const rcfd = findRcfd();

function engineCut(dir: string, flags: string[]): ReturnType<typeof readGlb> {
  const scratch = mkdtempSync(path.join(os.tmpdir(), 'rcfd-adr0738-g2-'));
  try {
    const out = path.join(scratch, 'cut.glb');
    execFileSync(rcfd!, ['post', 'derive', dir, '--kind', 'cut_surface', '--vectors', ...flags, '--out', out]);
    return readGlb(readFileSync(out));
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

test('G2: the engine’s cut_surface is the Studio’s surface, cut (T2) and arrows (T3), on the quick-start and axial-fan runs', () => {
  test.skip(rcfd === null, NO_RCFD);
  test.setTimeout(120_000);
  for (const project of ['quick-start', 'axial-fan']) {
    const dir = exampleRun(project).dir;
    const o = open(dir);
    const s = o.surface;
    const layers = meshLayers(s, o.nCells);
    const comps: (Float32Array | null)[] = (['ux', 'uy', 'uz'] as const).map((f) => (o.fields.some((x) => x.name === f) ? o.field(f) : null));
    const g = vectorGlyphs(s, layers.twoD ? comps.map((c, k) => (k === layers.thin ? null : c)) : comps, layers, o.nCells);
    // no cut: every surface triangle, its corners and cell exactly; the arrows within 2^-20 of the largest extent
    const whole = engineCut(dir, []);
    // the reader the viewport uses (src/renderer/field/glb.ts) reads the same arrays
    const viewportCut = cutOf(readStudioGlb(whole.bytes.buffer.slice(whole.bytes.byteOffset, whole.bytes.byteOffset + whole.bytes.byteLength) as ArrayBuffer));
    expect(viewportCut.arrowCount, project).toBe(g.cells.length);
    expect(viewportCut.source.length, project).toBe(s.triangles.length / 3);
    expect(Buffer.from(viewportCut.arrows!.buffer).equals(Buffer.from((whole.primitives.find((p) => p.mode === 1)!.attributes['POSITION'] as Float32Array).buffer)), project).toBe(true);
    expect(whole.asset['extras'] && (whole.asset['extras'] as Record<string, unknown>)['length_unit'], project).toBe('m');
    const tri = whole.primitives.find((p) => p.mode === 4)!;
    const nt = s.triangles.length / 3;
    expect(tri.attributes['POSITION']!.length, `${project}: all triangles`).toBe(9 * nt);
    // (mismatches are counted, not expected one by one: the passage has some 10^5 triangles)
    let differ = 0;
    for (let t = 0; t < nt; t++) {
      if (tri.attributes['_TRIANGLE']![3 * t] !== t || tri.attributes['_CELL']![3 * t] !== s.cells[t]) differ++;
      for (let k = 0; k < 3; k++) {
        const v = s.triangles[3 * t + k]!;
        for (let a = 0; a < 3; a++) if (!Object.is(tri.attributes['POSITION']![9 * t + 3 * k + a], s.points[3 * v + a])) differ++;
      }
    }
    expect(differ, `${project}: triangles, cells and corners bit for bit`).toBe(0);
    const lines = whole.primitives.find((p) => p.mode === 1)!;
    expect(lines.extras['arrows'], `${project}: the arrow count`).toBe(g.cells.length);
    const pos = lines.attributes['POSITION'] as Float32Array;
    expect(pos.length, project).toBe(g.segments.length);
    const ext = [0, 1, 2].map((k) => layers.max[k]! - layers.min[k]!);
    const tol = 2 ** -20 * Math.max(...ext);
    let worst = 0;
    for (let i = 0; i < pos.length; i++) worst = Math.max(worst, Math.abs(pos[i]! - g.segments[i]!));
    expect(worst, `${project}: arrows within 2^-20 of the largest extent`).toBeLessThanOrEqual(tol);
    // a cut through the middle of the longest axis: the triangles T2 keeps whole are the engine's, unchanged; the engine
    // has no triangle T2 drops whole; every corner it draws keeps n . p <= d
    const axis = ext.indexOf(Math.max(...ext)) as 0 | 1 | 2;
    const at = Math.fround(0.5 * (layers.min[axis]! + layers.max[axis]!));
    for (const keep of ['below', 'above'] as const) {
      const cut = { axis, at, keep };
      const c = engineCut(dir, ['--axis', 'xyz'[axis]!, '--at', String(at), '--keep', keep]).primitives.find((p) => p.mode === 4)!;
      const src = c.attributes['_TRIANGLE']!;
      const cp = c.attributes['POSITION']!;
      const corner = (t: number, k: number) => [0, 1, 2].map((a) => s.points[3 * s.triangles[3 * t + k]! + a]!);
      const kept = new Set<number>();
      for (let t = 0; t < nt; t++) if ([0, 1, 2].every((k) => keeps(cut, corner(t, k)))) kept.add(t);
      const some = new Set<number>();
      for (let t = 0; t < nt; t++) if ([0, 1, 2].some((k) => keeps(cut, corner(t, k)))) some.add(t);
      const whole3 = new Set<number>();
      const bad = { dropped: 0, outside: 0, moved: 0 };
      for (let i = 0; i < src.length / 3; i++) {
        const t = src[3 * i]!;
        if (!some.has(t)) bad.dropped++;
        for (let k = 0; k < 3; k++) if (!keeps(cut, [cp[9 * i + 3 * k]!, cp[9 * i + 3 * k + 1]!, cp[9 * i + 3 * k + 2]!])) bad.outside++;
        if (kept.has(t)) {
          whole3.add(t);
          for (let k = 0; k < 3; k++) for (let a = 0; a < 3; a++) if (cp[9 * i + 3 * k + a] !== corner(t, k)[a]) bad.moved++;
        }
      }
      expect(bad, `${project} ${keep}: no triangle T2 drops, no corner outside, the whole ones unchanged`).toEqual({ dropped: 0, outside: 0, moved: 0 });
      expect([...whole3].sort((x, y) => x - y), `${project} ${keep}: the triangles kept whole`).toEqual([...kept]);
    }
  }
});

test('ADR 0738: the viewport reads the engine’s iso-surface and streamlines of the axial-fan passage (3D) and the quick-start cavity (2D)', () => {
  test.skip(rcfd === null, NO_RCFD);
  test.setTimeout(120_000);
  const scratch = mkdtempSync(path.join(os.tmpdir(), 'rcfd-adr0738-iso-'));
  try {
    const run = (dir: string, flags: string[]) => {
      const out = path.join(scratch, 'p.glb');
      const reply = JSON.parse(execFileSync(rcfd!, ['post', 'derive', dir, ...flags, '--out', out]).toString('utf8'));
      const b = readFileSync(out);
      return { reply, glb: readStudioGlb(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength) as ArrayBuffer) };
    };
    const fan = exampleRun('axial-fan').dir;
    const o = open(fan);
    const p = o.field('p');
    let lo = Infinity;
    let hi = -Infinity;
    for (const v of p) {
      lo = Math.min(lo, v);
      hi = Math.max(hi, v);
    }
    const level = Math.fround(0.5 * (lo + hi));
    const iso = run(fan, ['--kind', 'iso', '--field', 'p', '--level', String(level)]);
    const surf = isoOf(iso.glb);
    expect(surf.triangles, 'the engine’s count').toBe(iso.reply.triangles);
    expect(surf.triangles).toBeGreaterThan(0);
    // every corner's value is the level (no colour field), and every corner is inside the stored mesh's box
    const layers = meshLayers(o.surface, o.nCells);
    // (counted, not expected one by one: some 10^4 triangles)
    let off = 0;
    for (let i = 0; i < surf.values.length; i++) if (surf.values[i] !== level) off++;
    let outside = 0;
    for (let i = 0; i < surf.positions.length; i++) {
      const k = i % 3;
      if (!(surf.positions[i]! >= layers.min[k]! - 1e-6 && surf.positions[i]! <= layers.max[k]! + 1e-6)) outside++;
    }
    expect({ off, outside }).toEqual({ off: 0, outside: 0 });
    // streamlines of the 2D cavity from the viewport's rake: one line per seed and direction, each segment joined
    const qs = exampleRun('quick-start').dir;
    const ql = meshLayers(open(qs).surface, open(qs).nCells);
    const rake = { axis: 1 as const, n: 5 };
    const ends = [0, 1, 2].map((k) => 0.5 * (ql.min[k]! + ql.max[k]!));
    const p0 = [...ends];
    const p1 = [...ends];
    p0[1] = ql.min[1]! + 0.02 * (ql.max[1]! - ql.min[1]!);
    p1[1] = ql.max[1]! - 0.02 * (ql.max[1]! - ql.min[1]!);
    const st = run(qs, ['--kind', 'streamlines', '--rake', [...p0, ...p1, rake.n].join(','), '--direction', 'both', '--max-steps', '2000']);
    const lines = linesOf(st.glb);
    expect(lines.lines).toBe(st.reply.lines_drawn);
    expect(lines.lines).toBeLessThanOrEqual(2 * rake.n);
    expect(lines.segments.length % 6).toBe(0);
    expect(lines.segments.length).toBeGreaterThan(0);
    let offPlane = 0;
    for (let i = 2; i < lines.segments.length; i += 3) if (!(Math.abs(lines.segments[i]! - Math.fround(ends[2]!)) < 1e-6)) offPlane++;
    expect(offPlane, 'the 2D lines stay in the mid-plane').toBe(0);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});
