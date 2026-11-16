// The monitor feed of one run (F-811): the run's residual and monitor histories fetched once with `monitors.get` and then
// by increments (ADR 0353: `since` is applied before the reply is built, so an increment of 100 samples costs 0.3 ms
// where the whole history of 1e5 samples costs 80 ms). Pure code: the engine call is handed in (in the Studio it is
// `dispatch('monitors.get', ...)` from ../actions), so the unit tests run the same code against the fake service.
//
// How an increment is exact. `since` returns, for each series, its samples from the first one whose abscissa is greater
// than `since` (the service's partition_point on x <= since) to the end. The feed asks with `since` just below the last
// abscissa it holds (the next f64 down), so the reply of a series whose abscissa does not decrease repeats the held
// samples at that last abscissa (one, or several tied ones) and then has the new ones. The repeated samples are
// compared with the held ones (same x and y values); when they agree, the rest is appended, so nothing is added twice
// and nothing is skipped. When they disagree (the simulation was built again, a step back in x), the series is fetched
// whole again and counted as a resync. A series the run did not have before is fetched whole once. Nothing is ever
// taken from another run: a feed belongs to one run (service instance and session) and its replies are applied only to
// it.
import { nextDown, partitionPoint, readSeries, sameValue, SeriesData } from './series.js';

/** One run as the service knows it: the session, and the service instance it lives in (ServiceState.instance). */
export interface RunRef {
  readonly instance: number;
  readonly session: string;
}

export const runKey = (r: RunRef): string => `${r.instance}/${r.session}`;

export interface MonitorsGetParams {
  readonly session: string;
  readonly names?: string[];
  readonly since?: number;
}

/** The shape of an action reply (src/shared/actions/dispatcher.ts ActionReply), reduced to what the feed reads. */
export type GetReply = { readonly ok: true; readonly result: unknown } | { readonly ok: false; readonly error: { readonly code: number | string; readonly message: string } };

export type MonitorsGet = (params: MonitorsGetParams) => Promise<GetReply>;

/**
 * - `loading`: the first fetch has not answered yet.
 * - `ready`: the series held are the run's (possibly none: `series.size === 0`).
 * - `unavailable`: the service has no histories for this run (no simulation yet, unknown session); `message` says why.
 * - `error`: the first fetch failed for another reason; `message` says why. A later failure keeps the data and sets
 *   `lastError` instead.
 */
export type FeedStatus = 'loading' | 'ready' | 'unavailable' | 'error';

export interface FeedStats {
  /** monitors.get requests made, by kind. */
  readonly full: number;
  readonly increments: number;
  readonly discovery: number;
  /** Series fetched whole again because an increment did not line up. */
  readonly resyncs: number;
  /** Duration of the first full fetch (request to parsed result), ms. */
  readonly firstFetchMs: number | null;
  /** Duration of the last poll, ms. */
  readonly lastPollMs: number | null;
  /** Samples added by the last poll, all series together. */
  readonly lastAdded: number;
}

export interface FeedSnapshot {
  readonly run: RunRef;
  readonly status: FeedStatus;
  /** Why the status is `unavailable` or `error`, in words. */
  readonly message: string | null;
  /** The latest failed poll after data arrived (kept data stays shown), in words; null after a good poll. */
  readonly lastError: string | null;
  /** `iteration` or `time`, from the reply. */
  readonly xLabel: string | null;
  /** The run's series by name (shared objects: they grow in place; `version` tells a change). */
  readonly series: ReadonlyMap<string, SeriesData>;
  /** Raised on every change. */
  readonly version: number;
  readonly stats: FeedStats;
}

/** Codes of the service (docs/service-protocol.md, Errors) that mean "this run has no histories", not a failure. */
const UNAVAILABLE_CODES = new Set<number | string>([1001, 1002]);

/** A since value no abscissa exceeds: the reply names every series and holds no sample (a cheap list of names). */
const NAMES_ONLY_SINCE = Number.MAX_VALUE;

export interface FeedOptions {
  /** Least time between the start of two polls requested by `refresh` (live runs send many events), ms. */
  readonly minIntervalMs?: number;
  readonly now?: () => number;
}

export class MonitorFeed {
  private readonly series = new Map<string, SeriesData>();
  private status: FeedStatus = 'loading';
  private message: string | null = null;
  private lastError: string | null = null;
  private xLabel: string | null = null;
  private version = 0;
  private stats: FeedStats = { full: 0, increments: 0, discovery: 0, resyncs: 0, firstFetchMs: null, lastPollMs: null, lastAdded: 0 };
  private snap: FeedSnapshot | null = null;
  private readonly listeners = new Set<() => void>();
  private running: Promise<void> | null = null;
  private again = false;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private lastStart = -Infinity;
  private disposed = false;
  private readonly minIntervalMs: number;
  private readonly now: () => number;

  constructor(
    readonly run: RunRef,
    private readonly get: MonitorsGet,
    options: FeedOptions = {},
  ) {
    this.minIntervalMs = options.minIntervalMs ?? 100;
    this.now = options.now ?? (() => performance.now());
  }

  /** The current state; the same object until something changes (for React's useSyncExternalStore). */
  snapshot = (): FeedSnapshot => {
    this.snap ??= {
      run: this.run,
      status: this.status,
      message: this.message,
      lastError: this.lastError,
      xLabel: this.xLabel,
      series: this.series,
      version: this.version,
      stats: this.stats,
    };
    return this.snap;
  };

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  /**
   * Asks for a poll soon: at once when none ran in the last `minIntervalMs`, else when that time is up. Calls while a
   * poll runs collapse into one poll after it. Never blocks and never throws.
   */
  refresh(): void {
    if (this.disposed) return;
    if (this.running) {
      this.again = true;
      return;
    }
    if (this.timer !== null) return;
    const wait = Math.max(0, this.lastStart + this.minIntervalMs - this.now());
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.poll();
    }, wait);
  }

  /** Runs one poll now (after the one in flight, if any) and resolves when its replies are applied. */
  async poll(): Promise<void> {
    while (this.running) await this.running;
    if (this.disposed) return;
    this.lastStart = this.now();
    this.running = this.pollOnce().finally(() => {
      this.running = null;
      if (this.again && !this.disposed) {
        this.again = false;
        this.refresh();
      }
    });
    await this.running;
  }

  dispose(): void {
    this.disposed = true;
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    this.listeners.clear();
  }

  private changed(): void {
    this.version++;
    this.snap = null;
    for (const l of [...this.listeners]) l();
  }

  private async pollOnce(): Promise<void> {
    const t0 = this.now();
    let added = 0;
    try {
      if (this.status !== 'ready') {
        added = await this.first();
      } else {
        added = await this.increment();
        this.lastError = null;
      }
    } catch (e) {
      if (this.disposed) return;
      const why = e instanceof FeedError ? e.message : `the monitor histories could not be read: ${String(e)}`;
      if (this.status === 'ready') this.lastError = why;
      else {
        this.status = e instanceof FeedError && e.unavailable ? 'unavailable' : 'error';
        this.message = why;
      }
    }
    if (this.disposed) return;
    this.stats = { ...this.stats, lastPollMs: this.now() - t0, lastAdded: added };
    this.changed();
  }

  /** One monitors.get; a failed reply becomes a FeedError (unavailable for "no simulation" and "unknown session"). */
  private async fetch(kind: 'full' | 'increments' | 'discovery', params: MonitorsGetParams) {
    this.stats = { ...this.stats, [kind]: this.stats[kind] + 1 };
    const r = await this.get(params);
    if (!r.ok) throw new FeedError(r.error.message, UNAVAILABLE_CODES.has(r.error.code));
    const doc = readSeries(r.result);
    if (!doc) throw new FeedError('the service sent monitor histories in a form this Studio does not read (rcfd-monitors 1 expected)', false);
    if (this.xLabel === null || kind === 'full') this.xLabel = doc.xLabel;
    return doc.series;
  }

  private async first(): Promise<number> {
    const t0 = this.now();
    const got = await this.fetch('full', { session: this.run.session });
    this.stats = { ...this.stats, firstFetchMs: this.now() - t0 };
    if (this.disposed) return 0;
    let added = 0;
    for (const [name, s] of got) {
      this.take(name).replace(s.unit, s.x, s.y);
      added += s.x.length;
    }
    this.status = 'ready';
    this.message = null;
    return added;
  }

  private take(name: string): SeriesData {
    let s = this.series.get(name);
    if (!s) {
      s = new SeriesData(name, '');
      this.series.set(name, s);
    }
    return s;
  }

  private async increment(): Promise<number> {
    // the cursor of each series: just below its last abscissa (the samples at that abscissa come back as the check)
    const groups = new Map<number, string[]>();
    const whole: string[] = [];
    for (const [name, s] of this.series) {
      const last = s.length > 0 ? s.xAt(s.length - 1) : Number.NaN;
      const c = Number.isFinite(last) ? nextDown(last) : Number.NaN;
      if (!Number.isFinite(c)) whole.push(name);
      else {
        const g = groups.get(c);
        if (g) g.push(name);
        else groups.set(c, [name]);
      }
    }
    let added = 0;
    const unknown = new Set<string>();
    if (groups.size === 1 && whole.length === 0) {
      // the usual case: one cursor for every series, so one request without names also lists new series
      const [since, names] = [...groups][0]!;
      const got = await this.fetch('increments', { session: this.run.session, since });
      if (this.disposed) return 0;
      for (const name of names) {
        const s = got.get(name);
        if (s) added += this.extend(name, since, s, whole);
        else whole.push(name);
      }
      for (const name of got.keys()) if (!this.series.has(name)) unknown.add(name);
    } else {
      for (const [since, names] of groups) {
        const got = await this.fetch('increments', { session: this.run.session, names, since });
        if (this.disposed) return 0;
        for (const name of names) {
          const s = got.get(name);
          if (s) added += this.extend(name, since, s, whole);
          else whole.push(name);
        }
      }
      const listed = await this.fetch('discovery', { session: this.run.session, since: NAMES_ONLY_SINCE });
      if (this.disposed) return 0;
      for (const name of listed.keys()) if (!this.series.has(name)) unknown.add(name);
    }
    const fetchWhole = [...whole, ...unknown];
    if (fetchWhole.length > 0) {
      const got = await this.fetch('full', { session: this.run.session, names: fetchWhole });
      if (this.disposed) return 0;
      for (const name of fetchWhole) {
        const s = got.get(name);
        if (s) {
          this.take(name).replace(s.unit, s.x, s.y);
          added += s.x.length;
        } else {
          // the run no longer has this series (the simulation was built again): keep nothing of it
          this.series.delete(name);
        }
      }
    }
    return added;
  }

  /**
   * Applies an increment asked with `since`: the reply must begin with the held samples after `since` (found the way
   * the service finds them); the samples after those are appended. Otherwise the series is marked for a whole fetch.
   */
  private extend(name: string, since: number, s: { unit: string; x: Float64Array; y: Float64Array }, whole: string[]): number {
    const held = this.series.get(name)!;
    const from = partitionPoint(held.x, since);
    const repeat = held.length - from;
    let lines = s.unit === held.unit && s.x.length >= repeat && repeat > 0;
    for (let i = 0; lines && i < repeat; i++) {
      lines = sameValue(s.x[i]!, held.xAt(from + i)) && sameValue(s.y[i]!, held.yAt(from + i));
    }
    if (!lines) {
      this.stats = { ...this.stats, resyncs: this.stats.resyncs + 1 };
      whole.push(name);
      return 0;
    }
    held.append(s.x, s.y, repeat);
    return s.x.length - repeat;
  }
}

class FeedError extends Error {
  constructor(
    message: string,
    readonly unavailable: boolean,
  ) {
    super(message);
  }
}
