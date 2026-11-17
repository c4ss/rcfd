// Unit tests of the plots' data layer (F-811, src/renderer/plots/feed.ts) against a fake service that holds monitor
// histories (tests/support/fake-monitors.ts), through the same path the renderer's calls take: the action dispatcher,
// the main process's client, the port bridge (which packs monitors.get into Float64Arrays) and its checks.
// The contract (roadmap F-811, ADR 0353): the histories are fetched once and then by increments, and after every poll
// the feed holds exactly the service's samples, no sample twice and none missing; a run never gets another run's data.
import { expect, test } from '@playwright/test';
import { Dispatcher } from '../../src/shared/actions/index.js';
import { MonitorFeed, runKey, type MonitorsGet, type RunRef } from '../../src/renderer/plots/feed.js';
import { nextDown } from '../../src/renderer/plots/series.js';
import { portTransport, type PortTransport } from '../support/action-transport.js';
import { FakeMonitorService, type FakeSeries } from '../support/fake-monitors.js';

/** A seeded generator (mulberry32), so a failure is reproducible. */
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

/** What the wire carries for a value: JSON's number, or null for a non-finite one, read back as NaN. */
const wire = (v: number) => (Number.isFinite(v) ? (JSON.parse(JSON.stringify(v)) as number) : Number.NaN);

/** The feed's series equal the service's histories exactly: same names, units, lengths and values (SameValue). */
function expectExact(feed: MonitorFeed, histories: Map<string, FakeSeries>, what: string) {
  const snap = feed.snapshot();
  expect([...snap.series.keys()].sort(), `${what}: names`).toEqual([...histories.keys()].sort());
  for (const [name, h] of histories) {
    const s = snap.series.get(name)!;
    expect(s.unit, `${what}: unit of ${name}`).toBe(h.unit);
    expect(s.length, `${what}: samples of ${name}`).toBe(h.x.length);
    const bad: number[] = [];
    for (let i = 0; i < h.x.length; i++) {
      if (!Object.is(s.xAt(i), wire(h.x[i]!)) || !Object.is(s.yAt(i), wire(h.y[i]!))) bad.push(i);
    }
    expect(bad.slice(0, 5), `${what}: differing samples of ${name}`).toEqual([]);
  }
}

async function rig() {
  const fake = await FakeMonitorService.start();
  const t: PortTransport = await portTransport(fake.port);
  const d = new Dispatcher(t.transport);
  const get: MonitorsGet = (p) => d.dispatch('monitors.get', p);
  return { fake, t, get, close: async () => (await t.close(), await fake.stop()) };
}

test.describe('the monitor feed (F-811)', () => {
  test('fetches the histories once, then by increments that append exactly: no sample twice, none missing', async () => {
    const { fake, get, close } = await rig();
    try {
      fake.addSession('s1');
      const r = rng(811);
      let it = 0;
      let t = 0;
      const step = () => {
        it++;
        t += 0.01 + r() * 0.01;
        // three residuals and a report on the iteration; one value in 40 not finite, now and then exactly zero
        for (const name of ['continuity', 'res_p', 'res_u']) {
          const v = r() < 0.025 ? Number.NaN : r() < 0.01 ? 0 : 10 ** (-1 - r() * 6);
          fake.push('s1', name, '', it, v);
        }
        fake.push('s1', 'lid_drag', 'N', it, r() < 0.02 ? Number.POSITIVE_INFINITY : 1e-3 * (1 + r()));
        // a series on another abscissa (the compressible solvers push dt against the step index while others use time)
        fake.push('s1', 'dt', 's', t, 1e-3 * (1 + r()));
        // a report that appears later in the run
        if (it >= 300) fake.push('s1', 'late', '', it, r());
      };
      for (let i = 0; i < 25; i++) step();
      const feed = new MonitorFeed({ instance: 1, session: 's1' }, get);
      await feed.poll();
      expect(feed.snapshot().status).toBe('ready');
      expectExact(feed, fake.histories('s1'), 'after the first fetch');
      expect(fake.monitorRequests()).toEqual([{ session: 's1' }]);

      let samples = 0;
      for (let poll = 0; poll < 80; poll++) {
        const n = poll % 9 === 0 ? 0 : 1 + Math.floor(r() * 12);
        for (let i = 0; i < n; i++) step();
        await feed.poll();
        expectExact(feed, fake.histories('s1'), `after poll ${poll}`);
        samples += feed.snapshot().stats.lastAdded;
      }
      const stats = feed.snapshot().stats;
      // whole fetches: the first, and the late series once when it appeared
      const whole = fake.monitorRequests().filter((p) => p['since'] === undefined);
      expect(whole).toEqual([{ session: 's1' }, { session: 's1', names: ['late'] }]);
      expect(stats.resyncs).toBe(0);
      expect(stats.full).toBe(2);
      // every other request is an increment by `since` (two cursors: the iteration and dt's time) or a list of names
      const rest = fake.monitorRequests().filter((p) => p['since'] !== undefined);
      expect(rest.length).toBe(stats.increments + stats.discovery);
      expect(stats.increments).toBeGreaterThanOrEqual(80 * 2);
      // the samples added by the increments are exactly the samples pushed after the first fetch (late's whole fetch aside)
      const h = fake.histories('s1');
      const total = [...h.values()].reduce((a, s) => a + s.x.length, 0);
      expect(samples).toBe(total - 25 * 5);
    } finally {
      await close();
    }
  });

  test('one cursor for every series: one request per poll, which also finds a new series', async () => {
    const { fake, get, close } = await rig();
    try {
      fake.addSession('s1');
      for (let i = 1; i <= 10; i++) for (const n of ['res_u', 'res_p']) fake.push('s1', n, '', i, 1 / i);
      const feed = new MonitorFeed({ instance: 1, session: 's1' }, get);
      await feed.poll();
      const before = fake.monitorRequests().length;
      for (let i = 11; i <= 20; i++) {
        for (const n of ['res_u', 'res_p']) fake.push('s1', n, '', i, 1 / i);
        await feed.poll();
      }
      expect(fake.monitorRequests().slice(before)).toEqual(Array.from({ length: 10 }, (_, k) => ({ session: 's1', since: nextDown(10 + k) })));
      fake.push('s1', 'res_u', '', 21, 0.5);
      fake.push('s1', 'res_p', '', 21, 0.5);
      fake.push('s1', 'cd', '', 15, 1.2);
      fake.push('s1', 'cd', '', 21, 1.3);
      await feed.poll();
      // the increment shows cd's suffix only; cd is then fetched whole
      expect(fake.monitorRequests().slice(-2)).toEqual([
        { session: 's1', since: nextDown(20) },
        { session: 's1', names: ['cd'] },
      ]);
      expectExact(feed, fake.histories('s1'), 'with the new series');
    } finally {
      await close();
    }
  });

  test('an increment that does not line up (the simulation built again, a tie, a step back) is fetched whole again', async () => {
    const { fake, get, close } = await rig();
    try {
      fake.addSession('s1');
      for (let i = 1; i <= 50; i++) fake.push('s1', 'res_u', '', i, 1 / i);
      const feed = new MonitorFeed({ instance: 1, session: 's1' }, get);
      await feed.poll();
      // built again: new histories from iteration 1
      fake.reset('s1');
      for (let i = 1; i <= 60; i++) fake.push('s1', 'res_u', '', i, 2 / i);
      await feed.poll();
      expectExact(feed, fake.histories('s1'), 'after a rebuild');
      expect(feed.snapshot().stats.resyncs).toBe(1);
      // a tie at the end: two samples with the same abscissa
      fake.push('s1', 'res_u', '', 60, 0.01);
      await feed.poll();
      fake.push('s1', 'res_u', '', 61, 0.02);
      await feed.poll();
      expectExact(feed, fake.histories('s1'), 'after a tie');
      // a step back in x
      fake.push('s1', 'res_u', '', 30, 0.03);
      fake.push('s1', 'res_u', '', 31, 0.04);
      await feed.poll();
      fake.push('s1', 'res_u', '', 32, 0.05);
      await feed.poll();
      expectExact(feed, fake.histories('s1'), 'after a step back');
      // a series the run no longer has is dropped
      fake.reset('s1');
      fake.push('s1', 'res_p', '', 1, 1);
      fake.push('s1', 'res_p', '', 2, 0.5);
      await feed.poll();
      expectExact(feed, fake.histories('s1'), 'after the series changed');
    } finally {
      await close();
    }
  });

  test('a run never gets another run’s samples, whatever order the replies come back in', async () => {
    const { fake, get, close } = await rig();
    try {
      fake.addSession('s1', { delayMs: 40 });
      fake.addSession('s2');
      for (let i = 1; i <= 30; i++) {
        fake.push('s1', 'res_u', '', i, 1 / i);
        fake.push('s2', 'res_u', '', i, 100 + i);
        fake.push('s2', 'only_in_s2', '', i, i);
      }
      const a = new MonitorFeed({ instance: 1, session: 's1' }, get);
      const b = new MonitorFeed({ instance: 1, session: 's2' }, get);
      // the same session id in a restarted service is another run
      const c = new MonitorFeed({ instance: 2, session: 's1' }, get);
      expect(new Set([a, b, c].map((f) => runKey(f.run))).size).toBe(3);
      for (let k = 0; k < 6; k++) {
        for (let i = 0; i < 5; i++) {
          const x = 31 + k * 5 + i;
          fake.push('s1', 'res_u', '', x, 1 / x);
          fake.push('s2', 'res_u', '', x, 100 + x);
          fake.push('s2', 'only_in_s2', '', x, x);
        }
        fake.setDelay('s1', k % 2 ? 40 : 0);
        fake.setDelay('s2', k % 2 ? 0 : 40);
        await Promise.all([a.poll(), b.poll()]);
        expectExact(a, fake.histories('s1'), `s1 after round ${k}`);
        expectExact(b, fake.histories('s2'), `s2 after round ${k}`);
      }
      expect([...a.snapshot().series.keys()]).toEqual(['res_u']);
      expect(a.snapshot().series.get('res_u')!.y.every((v) => v <= 1)).toBe(true);
      // the feed of s1 in instance 2 asks only for s1 and holds s1's histories, not s2's
      await c.poll();
      expectExact(c, fake.histories('s1'), 'the other instance');
      expect(fake.monitorRequests().every((p) => p['session'] === 's1' || p['session'] === 's2')).toBe(true);
    } finally {
      await close();
    }
  });

  test('a run without series: no simulation, unknown session, no samples yet; then the first samples', async () => {
    const { fake, get, close } = await rig();
    try {
      fake.addSession('defined', { simulation: false });
      fake.addSession('empty');
      const none = new MonitorFeed({ instance: 1, session: 'defined' }, get);
      await none.poll();
      expect(none.snapshot()).toMatchObject({ status: 'unavailable', message: 'no simulation yet; call run.initialize or run.start' });
      expect(none.snapshot().series.size).toBe(0);
      const unknown = new MonitorFeed({ instance: 1, session: 'gone' }, get);
      await unknown.poll();
      expect(unknown.snapshot()).toMatchObject({ status: 'unavailable', message: "unknown session 'gone'" });
      // the simulation is built: the next poll reads it
      fake.build('defined');
      await none.poll();
      expect(none.snapshot().status).toBe('ready');
      const empty = new MonitorFeed({ instance: 1, session: 'empty' }, get);
      await empty.poll();
      expect(empty.snapshot()).toMatchObject({ status: 'ready', message: null });
      expect(empty.snapshot().series.size).toBe(0);
      for (let i = 1; i <= 3; i++) fake.push('empty', 'res_u', '', i, 1 / i);
      await empty.poll();
      expectExact(empty, fake.histories('empty'), 'the first samples');
      fake.push('empty', 'res_u', '', 4, 0.25);
      await empty.poll();
      expectExact(empty, fake.histories('empty'), 'the next sample');
    } finally {
      await close();
    }
  });

  test('refresh collapses a burst of events into few polls, never two at once', async () => {
    let inFlight = 0;
    let most = 0;
    let calls = 0;
    const get: MonitorsGet = async () => {
      calls++;
      inFlight++;
      most = Math.max(most, inFlight);
      await new Promise((r) => setTimeout(r, 30));
      inFlight--;
      return { ok: true, result: { format: 'rcfd-monitors', version: 1, x_label: 'iteration', series: {} } };
    };
    const run: RunRef = { instance: 1, session: 's1' };
    const feed = new MonitorFeed(run, get, { minIntervalMs: 20 });
    for (let i = 0; i < 200; i++) {
      feed.refresh();
      if (i % 50 === 0) await new Promise((r) => setTimeout(r, 5));
    }
    await new Promise((r) => setTimeout(r, 300));
    expect(most).toBe(1);
    expect(calls).toBeGreaterThanOrEqual(1);
    expect(calls).toBeLessThanOrEqual(4);
    feed.dispose();
    const after = calls;
    feed.refresh();
    await new Promise((r) => setTimeout(r, 60));
    expect(calls).toBe(after);
  });

  test('the main process hands monitors.get over as Float64Arrays holding the same numbers', async () => {
    const { fake, get, close } = await rig();
    try {
      fake.addSession('s1');
      const ys = [1.5, Number.NaN, 0.1 + 0.2, 5e-324, 1.7976931348623157e308, -0, Number.NEGATIVE_INFINITY];
      ys.forEach((y, i) => fake.push('s1', 'res_u', 'm/s', i + 1, y));
      const r = await get({ session: 's1' });
      expect(r.ok).toBe(true);
      const s = (r as { result: { series: Record<string, { unit: string; x: unknown; y: unknown }> } }).result.series['res_u']!;
      expect(s.x).toBeInstanceOf(Float64Array);
      expect(s.y).toBeInstanceOf(Float64Array);
      expect(s.unit).toBe('m/s');
      expect([...(s.y as Float64Array)].map((v, i) => Object.is(v, wire(ys[i]!)))).toEqual(ys.map(() => true));
    } finally {
      await close();
    }
  });
});
