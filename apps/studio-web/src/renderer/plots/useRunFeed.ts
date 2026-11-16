// The React side of a run's monitor feed (F-811): one MonitorFeed per run, fed by `monitors.get` through the action
// door (once, then by increments) and woken by the service's events for that session, so a live run's plots follow it
// and a finished run costs one fetch. The run's state comes from `run.status`, read again on every event that is not an
// iteration. Components get a snapshot that belongs to the run they asked for, or null, never another run's.
import { useEffect, useState, useSyncExternalStore } from 'react';
import { dispatch, watchEvents } from '../actions/index.js';
import type { ServiceEventBatch } from '../../shared/service.js';
import { MonitorFeed, runKey, type FeedSnapshot, type RunRef } from './feed.js';

/** The members of a `run.status` reply the plots use (docs/service-protocol.md). */
export interface RunStatus {
  readonly session: string;
  /** defined, ready, running, paused, finished, stopped or failed. */
  readonly state: string;
  readonly converged?: boolean;
  /** The failure's message, for state failed. */
  readonly error?: string;
  readonly simulation?: { readonly iteration?: number; readonly time?: number };
}

export interface RunView {
  readonly feed: FeedSnapshot | null;
  readonly status: RunStatus | null;
  /** Why run.status failed, in words. */
  readonly statusError: string | null;
}

const noSubscribe = () => () => undefined;
const noSnapshot = () => null;

export function useRunFeed(run: RunRef | null): RunView {
  const key = run ? runKey(run) : null;
  const [feed, setFeed] = useState<MonitorFeed | null>(null);
  const [status, setStatus] = useState<{ key: string; status: RunStatus | null; error: string | null } | null>(null);

  useEffect(() => {
    if (!run) {
      setFeed(null);
      return;
    }
    const k = runKey(run);
    const f = new MonitorFeed(run, (p) => dispatch('monitors.get', p));
    setFeed(f);
    let alive = true;
    let stop: (() => void) | null = null;
    const readStatus = async () => {
      const r = await dispatch('run.status', { session: run.session });
      if (!alive) return;
      setStatus(r.ok ? { key: k, status: r.result as RunStatus, error: null } : { key: k, status: null, error: r.error.message });
    };
    const onEvents = (b: ServiceEventBatch) => {
      if (b.instance !== run.instance) return;
      f.refresh();
      if (b.missed > 0 || b.events.some((e) => e.kind !== 'iteration')) void readStatus();
    };
    // subscribe before the first fetch, so no sample between the two is missed (increments are by abscissa anyway)
    void watchEvents(onEvents, run.session).then((r) => {
      if (!alive) {
        if (r.ok) r.result();
        return;
      }
      if (r.ok) stop = r.result;
      void f.poll();
      void readStatus();
    });
    return () => {
      alive = false;
      stop?.();
      f.dispose();
    };
    // the run's identity is its key
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  const snap = useSyncExternalStore(feed ? feed.subscribe : noSubscribe, feed ? feed.snapshot : noSnapshot);
  const own = snap && key !== null && runKey(snap.run) === key ? snap : null;
  const st = status && status.key === key ? status : null;
  return { feed: own, status: st?.status ?? null, statusError: st?.error ?? null };
}
