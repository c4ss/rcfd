// rcfd-render's colour map, as the reference of the field viewport's tests (F-808, guardrail G3), taken from the Rust
// source itself: the stops are read out of `rcfd_post::render::colormap` (crates/rcfd-post/src/render.rs), and the
// arithmetic of `colormap`, which rcfd-render's `ColorMap::eval` repeated on the same stops, is repeated here operation for
// operation, in f64 as the Rust does. `rustSourceChecks` lists the lines of that Rust code this port relies on, so a change
// to the Rust map fails the tests instead of drifting from them. Nothing here is imported from the Studio's own colormap
// module. F-838 removed the crate rcfd-render (crates/rcfd-render/src/lib.rs, whose lines were checked here too); its map
// was `colormap` sampled at its 9 integer stops, so the reference and every recorded colour are unchanged, and
// `rcfd_post::render::render_scalar`, the engine's renderer that remains, draws with `colormap` itself.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { appRoot } from './rcfd.js';

const repo = path.resolve(appRoot, '..', '..');
export const RENDER_RS = path.join(repo, 'crates', 'rcfd-post', 'src', 'render.rs');

/** The STOPS of `rcfd_post::render::colormap`, read from the Rust source. */
export function rustStops(): [number, number, number][] {
  const src = readFileSync(RENDER_RS, 'utf8');
  const fn = src.slice(src.indexOf('pub fn colormap(t: f64) -> [u8; 3]'));
  const m = /const STOPS: \[\[f64; 3\]; (\d+)\] = \[([\s\S]*?)\];/.exec(fn);
  if (!m) throw new Error(`no STOPS in ${RENDER_RS}`);
  const stops = [...m[2]!.matchAll(/\[\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*\]/g)].map((x) => [Number(x[1]), Number(x[2]), Number(x[3])] as [number, number, number]);
  if (stops.length !== Number(m[1])) throw new Error(`STOPS declares ${m[1]} entries, ${stops.length} read`);
  return stops;
}

/** The lines of the Rust code that `rustEval` repeats; each must be present, verbatim, in its file. */
export const rustSourceChecks: { file: string; lines: string[] }[] = [
  {
    file: RENDER_RS,
    lines: [
      'let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };',
      'let x = t * (STOPS.len() - 1) as f64;',
      'let i = (x.floor() as usize).min(STOPS.len() - 2);',
      'let f = x - i as f64;',
      '[0, 1, 2].map(|k| (STOPS[i][k] * (1.0 - f) + STOPS[i + 1][k] * f).round() as u8)',
    ],
  },
];

/** Rust's f64::round: half away from zero. */
const roundRust = (x: number): number => Math.sign(x) * Math.floor(Math.abs(x) + 0.5);

/** `eval` of the Rust (rcfd_post's `colormap`, and so the removed rcfd-render's `ColorMap::eval` on its integer stops). */
export function rustEval(stops: readonly (readonly number[])[], t: number): [number, number, number] {
  const tt = Number.isFinite(t) ? Math.min(Math.max(t, 0), 1) : 0;
  const x = tt * (stops.length - 1);
  const i = Math.min(Math.floor(x), stops.length - 2);
  const f = x - i;
  return [0, 1, 2].map((k) => roundRust(stops[i]![k]! * (1 - f) + stops[i + 1]![k]! * f)) as [number, number, number];
}

/** rcfd-render's `ColorMap::viridis()`: rcfd_post's colormap sampled at k / 8. */
export function rcfdRenderViridis(): [number, number, number][] {
  const post = rustStops();
  return Array.from({ length: 9 }, (_, k) => rustEval(post, k / 8));
}

/** rcfd-render's colour of value v in the range [lo, hi] (the f64 path of `ColorMap::eval`; span 1 when hi <= lo). */
export function rcfdRenderColour(v: number, lo: number, hi: number, map = rcfdRenderViridis()): [number, number, number] {
  const span = hi > lo ? hi - lo : 1;
  return rustEval(map, (v - lo) / span);
}
