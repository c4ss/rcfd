// Unit tests of the plots' range rule (F-811, src/renderer/plots/range.ts, G3): every range holds every value the plot
// draws, on linear and log axes, for seeded random data with extreme magnitudes, single values, ties and values that
// cannot be drawn; ticks sit inside their range. Also the packing of monitors.get for the service port
// (src/shared/monitors.ts) and the f64 helpers of the feed (src/renderer/plots/series.ts).
import { expect, test } from '@playwright/test';
import { Extent, holds, linearAxis, logAxis, niceStep } from '../../src/renderer/plots/range.js';
import { asFloat64, nextDown, partitionPoint, readSeries } from '../../src/renderer/plots/series.js';
import { packMonitors, packResult } from '../../src/shared/monitors.js';

function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** A value of a random kind: ordinary, huge, tiny, subnormal, zero, negative, not finite. */
function value(r: () => number): number {
  const k = r();
  if (k < 0.05) return Number.NaN;
  if (k < 0.08) return r() < 0.5 ? Number.POSITIVE_INFINITY : Number.NEGATIVE_INFINITY;
  if (k < 0.12) return 0;
  if (k < 0.15) return 5e-324 * Math.ceil(r() * 100);
  if (k < 0.2) return (r() < 0.5 ? -1 : 1) * 1.7e308 * r();
  const mag = 10 ** (Math.floor(r() * 40) - 20);
  return (r() < 0.3 ? -1 : 1) * mag * (1 + r() * 9);
}

test.describe('every plot range holds every value it draws (G3)', () => {
  test('linear and log axes over 4000 seeded random series', () => {
    const r = rng(8110);
    let checked = 0;
    const bad: string[] = [];
    for (let k = 0; k < 4000; k++) {
      const n = 1 + Math.floor(r() * 60);
      const log = r() < 0.5;
      const e = new Extent();
      const drawn: [number, number][] = [];
      let skipped = 0;
      const base = r() < 0.2 ? 1e9 * r() : 0;
      const tie = r() < 0.1;
      const y0 = value(r);
      for (let i = 0; i < n; i++) {
        const x = r() < 0.03 ? Number.NaN : tie ? base : base + i * (r() < 0.5 ? 1 : 1e-3);
        const y = tie ? y0 : value(r);
        const py = e.add(x, y, log);
        if (Number.isNaN(py)) skipped++;
        else drawn.push([x, py]);
      }
      if (e.drawn !== drawn.length || e.skipped !== skipped) bad.push(`series ${k}: counted ${e.drawn}/${e.skipped}, drew ${drawn.length}/${skipped}`);
      if (e.empty) continue;
      const ax = linearAxis(e.xlo, e.xhi, 5);
      const ay = log ? logAxis(e.ylo, e.yhi) : linearAxis(e.ylo, e.yhi, 3);
      if (!(ax.lo < ax.hi && ay.lo < ay.hi)) bad.push(`series ${k}: empty range`);
      if (!holds(ax, ay, e)) bad.push(`series ${k}: holds() is false`);
      for (const [x, y] of drawn) {
        if (!(ax.lo <= x && x <= ax.hi)) bad.push(`series ${k}: x ${x} outside [${ax.lo}, ${ax.hi}]`);
        if (!(ay.lo <= y && y <= ay.hi)) bad.push(`series ${k}: y ${y} outside [${ay.lo}, ${ay.hi}]`);
        checked++;
      }
      for (const t of ax.ticks) if (!(ax.lo <= t && t <= ax.hi)) bad.push(`series ${k}: x tick ${t} outside`);
      for (const t of ay.ticks) if (!(ay.lo <= t && t <= ay.hi)) bad.push(`series ${k}: y tick ${t} outside`);
      if (ax.labels.length !== ax.ticks.length || ay.labels.length !== ay.ticks.length) bad.push(`series ${k}: labels`);
    }
    expect(bad.slice(0, 10)).toEqual([]);
    expect(checked).toBeGreaterThan(50_000);
  });

  test('what is not drawn is counted, never clamped onto the edge', () => {
    const lin = new Extent();
    for (const y of [1, 2, Number.NaN, Number.POSITIVE_INFINITY, 3]) lin.add(1, y, false);
    expect([lin.drawn, lin.skipped, lin.ylo, lin.yhi]).toEqual([3, 2, 1, 3]);
    const log = new Extent();
    for (const y of [1e-3, 0, -1, 1e-6, Number.NaN, 5e-324]) log.add(1, y, true);
    expect([log.drawn, log.skipped]).toEqual([3, 3]);
    expect(log.ylo).toBeCloseTo(Math.log10(5e-324), 10);
    expect(log.yhi).toBeCloseTo(-3, 12);
    const ay = logAxis(log.ylo, log.yhi);
    expect(ay.lo).toBe(-324);
    expect(ay.hi).toBe(-3);
    expect(ay.ticks.length).toBeLessThanOrEqual(9);
  });

  test('single values, exact decades and nice steps', () => {
    expect(linearAxis(5, 5)).toMatchObject({ lo: expect.any(Number), hi: expect.any(Number) });
    const a = linearAxis(5, 5);
    expect(a.lo < 5 && 5 < a.hi).toBe(true);
    const z = linearAxis(0, 0);
    expect(z.lo < 0 && 0 < z.hi).toBe(true);
    expect(logAxis(-3, -3)).toMatchObject({ lo: -3, hi: -2 });
    expect(logAxis(-7, -1)).toMatchObject({ lo: -7, hi: -1, labels: ['1e-7', '1e-6', '1e-5', '1e-4', '1e-3', '1e-2', '1e-1'] });
    expect(linearAxis(0.3, 0.7).lo).toBeLessThanOrEqual(0.3);
    expect(linearAxis(0.1 + 0.2, 1).lo).toBeLessThanOrEqual(0.1 + 0.2);
    expect([0.07, 0.15, 3, 4.1, 999].map(niceStep)).toEqual([0.1, 0.2, 5, 5, 1000]);
    // a span far below the step's f64 grid still holds its values
    const tiny = linearAxis(1e15, 1e15 + 0.25);
    expect(tiny.lo <= 1e15 && 1e15 + 0.25 <= tiny.hi).toBe(true);
  });
});

test.describe('the feed’s f64 helpers', () => {
  test('nextDown is the next f64 below; partitionPoint is the service’s partition_point(x <= after)', () => {
    for (const x of [1, 10, 0.1, 1e-300, 5e-324, 1e308, -1, -5e-324, 123456.789]) {
      const d = nextDown(x);
      expect(d).toBeLessThan(x);
      const mid = d + (x - d) / 2;
      expect(mid === d || mid === x).toBe(true);
      expect(JSON.parse(JSON.stringify(d))).toBe(d);
    }
    expect(nextDown(0)).toBe(-5e-324);
    expect(Number.isNaN(nextDown(Number.NaN))).toBe(true);
    expect(partitionPoint([1, 2, 3, 3, 4], 3)).toBe(4);
    expect(partitionPoint([1, 2, 3, 3, 4], nextDown(3))).toBe(2);
    expect(partitionPoint([], 1)).toBe(0);
    expect(partitionPoint([5, 6], 1)).toBe(0);
  });

  test('asFloat64 and readSeries read the wire and the packed form alike, and refuse other shapes', () => {
    expect([...asFloat64([1, null, 2.5])!].map(String)).toEqual(['1', 'NaN', '2.5']);
    expect(asFloat64([1, 'a'])).toBeNull();
    const doc = { format: 'rcfd-monitors', version: 1, x_label: 'time', series: { a: { unit: 's', x: [1, 2], y: [3, null] } } };
    const read = readSeries(doc)!;
    expect(read.xLabel).toBe('time');
    expect([...read.series.get('a')!.y].map(String)).toEqual(['3', 'NaN']);
    expect(readSeries(packMonitors(doc))!.series.get('a')!.x).toBeInstanceOf(Float64Array);
    expect(readSeries({ ...doc, format: 'other' })).toBeNull();
    expect(readSeries({ ...doc, version: 2 })).toBeNull();
    expect(readSeries({ ...doc, series: { a: { unit: 's', x: [1], y: [1, 2] } } })).toBeNull();
  });
});

test.describe('monitors.get across the service port (src/shared/monitors.ts)', () => {
  test('packs x and y into Float64Arrays holding the same numbers, null as NaN, everything else kept', () => {
    const doc = { format: 'rcfd-monitors', version: 1, x_label: 'iteration', extra: 1, series: { res_u: { unit: '', x: [1, 2, 3], y: [0.1 + 0.2, null, -0], more: 'kept' } } };
    const packed = packMonitors(doc) as typeof doc & { series: { res_u: { x: Float64Array; y: Float64Array } } };
    expect(packed).not.toBe(doc);
    expect(packed.extra).toBe(1);
    expect(packed.series.res_u.more).toBe('kept');
    expect([...packed.series.res_u.x]).toEqual([1, 2, 3]);
    expect(Object.is(packed.series.res_u.y[0], 0.1 + 0.2)).toBe(true);
    expect(Number.isNaN(packed.series.res_u.y[1])).toBe(true);
    expect(Object.is(packed.series.res_u.y[2], -0)).toBe(true);
    // the input is not changed
    expect(doc.series.res_u.y).toEqual([0.1 + 0.2, null, -0]);
  });

  test('a reply of another shape, or of another method, passes unchanged', () => {
    const odd = [
      null,
      42,
      { format: 'rcfd-monitors', series: [] },
      { format: 'other', series: {} },
      { format: 'rcfd-monitors', series: { a: { unit: 1, x: [], y: [] } } },
      { format: 'rcfd-monitors', series: { a: { unit: '', x: [1], y: [1, 2] } } },
      { format: 'rcfd-monitors', series: { a: { unit: '', x: [1, 'x'], y: [1, 2] } } },
      { format: 'rcfd-monitors', series: { a: { unit: '', x: {}, y: [] } } },
    ];
    for (const o of odd) expect(packMonitors(o)).toBe(o);
    const doc = { format: 'rcfd-monitors', version: 1, x_label: 'iteration', series: { a: { unit: '', x: [1], y: [1] } } };
    expect(packResult('reports.get', doc)).toBe(doc);
    expect(packResult('monitors.get', doc)).not.toBe(doc);
  });
});
