// The plots' development view at #/dev/plots (F-811), loaded on demand: the residual plot and the monitor chart of one
// session of the connected service, the way the time panel will hold them (DESIGN.md Layout: the monitor column is
// 380px), with a few run controls to make a session and drive it, and the feed's counters. Every call goes through the
// action door. Not a product surface: F-810 places the plots in the time panel and F-806 picks the run.
//
//   #/dev/plots                         the sessions of the service; pick one
//   #/dev/plots?session=s1              open on session s1
//   #/dev/plots?case=cases/x.toml       the case file the "New session" button uses (inside the service's sandbox)
import { useCallback, useEffect, useMemo, useState } from 'react';
import { dispatch, watchEvents, watchServiceState } from '../actions/index.js';
import type { ServiceState } from '../../shared/service.js';
import { MonitorPlot, ResidualPlot, runKey, useRunFeed, type RunRef } from '../plots/index.js';
import { pictogramOf } from '../plots/words.js';
import { Button, Select, STATE_WORD, StateMark, StatePictogram, TextField } from '../ui/index.js';
import { Provenance, sessionIterationValue } from '../provenance/index.js';
import './plots-dev.css';

interface SessionRow {
  readonly session: string;
  readonly state: string;
}

function query(): URLSearchParams {
  const h = location.hash;
  const q = h.indexOf('?');
  return new URLSearchParams(q >= 0 ? h.slice(q + 1) : '');
}

export default function PlotsDev() {
  const [service, setService] = useState<ServiceState | null>(null);
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [selected, setSelected] = useState<string | null>(() => query().get('session'));
  const [casePath, setCasePath] = useState(() => query().get('case') ?? 'cases/cavity-re100.toml');
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => watchServiceState(setService), []);
  // a new ?session= in the address opens that session
  useEffect(() => {
    const onHash = () => {
      const q = query().get('session');
      if (q) setSelected(q);
    };
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);

  const listSessions = useCallback(async () => {
    const r = await dispatch('session.list', {});
    if (r.ok) {
      const rows = (r.result as { sessions?: SessionRow[] }).sessions ?? [];
      setSessions(rows);
    }
  }, []);

  const connected = service?.status === 'connected';
  const instance = service?.instance ?? 0;
  useEffect(() => {
    if (!connected) return;
    void listSessions();
    let stop: (() => void) | null = null;
    let alive = true;
    void watchEvents((b) => {
      if (b.events.some((e) => e.kind !== 'iteration')) void listSessions();
    }).then((r) => {
      if (!alive && r.ok) r.result();
      else if (r.ok) stop = r.result;
    });
    return () => {
      alive = false;
      stop?.();
    };
  }, [connected, instance, listSessions]);

  const run: RunRef | null = useMemo(() => (connected && selected ? { instance, session: selected } : null), [connected, instance, selected]);
  const view = useRunFeed(run);

  const act = async (what: string, call: () => Promise<{ ok: boolean; error?: { message: string } }>) => {
    setMessage(null);
    const r = await call();
    if (!r.ok) setMessage(`${what} failed: ${r.error?.message ?? 'no reason given'}`);
    void listSessions();
  };

  const create = () =>
    act('New session', async () => {
      const r = await dispatch('session.create', { path: casePath });
      if (r.ok) setSelected((r.result as { session: string }).session);
      return r;
    });

  const s = selected;
  return (
    <main className="plots-dev" aria-labelledby="plots-dev-title">
      <header className="plots-dev-head">
        <h1 id="plots-dev-title">Plots</h1>
        <p>Development view of F-811: the residual plot and the monitor chart of one session, from the connected service.</p>
      </header>

      <section className="plots-dev-bar" aria-label="Session">
        <span className="plots-dev-service" data-status={service?.status ?? 'unknown'}>
          Service: {service ? `${service.status}${service.address ? ` at ${service.address}` : ''}` : 'reading'}
        </span>
        <Select
          aria-label="Session"
          className="plots-dev-session"
          placeholder={sessions.length ? 'Pick a session' : 'No sessions'}
          options={sessions.map((r) => ({ id: r.session, label: `${r.session} · ${r.state}`, lead: <StatePictogram state={pictogramOf({ session: r.session, state: r.state })} size={12} /> }))}
          selected={s}
          onChange={setSelected}
          isDisabled={!connected}
        />
        <TextField aria-label="Case file" className="plots-dev-case" value={casePath} onChange={setCasePath} isDisabled={!connected} />
        <Button variant="primary" icon="plus" onPress={create} isDisabled={!connected}>
          New session
        </Button>
        <Button onPress={() => s && act('Initialize', () => dispatch('run.initialize', { session: s }))} isDisabled={!s}>
          Initialize
        </Button>
        <Button onPress={() => s && act('Step', () => dispatch('run.step', { session: s, count: 20 }))} isDisabled={!s}>
          Step 20
        </Button>
        <Button onPress={() => s && act('Start', () => dispatch('run.start', { session: s }))} isDisabled={!s}>
          Start
        </Button>
        <Button onPress={() => s && act('Stop', () => dispatch('run.stop', { session: s }))} isDisabled={!s}>
          Stop
        </Button>
      </section>
      {message && (
        <p className="plots-dev-message" role="alert">
          {message}
        </p>
      )}

      {s && run ? (
        <section className="plots-dev-panel" aria-label={`Plots of session ${s}`} data-run={runKey(run)}>
          <div className="plots-dev-state">
            <StateMark state={pictogramOf(view.status)} label={`${STATE_WORD[pictogramOf(view.status)]} · ${view.status?.state ?? 'reading'}`} />
            {view.status?.simulation?.iteration !== undefined && (
              <span>
                iteration{' '}
                <Provenance subject={{ kind: 'session', source: sessionIterationValue({ session: s, iteration: view.status.simulation.iteration }) }}>
                  {view.status.simulation.iteration}
                </Provenance>
              </span>
            )}
            {view.status?.state === 'failed' && view.status.error && <span className="plots-dev-fail">{view.status.error}</span>}
            {view.statusError && <span>run.status failed: {view.statusError}</span>}
          </div>
          <div className="plots-dev-plots">
            <ResidualPlot key={`r-${runKey(run)}`} session={s} feed={view.feed} status={view.status} height={150} />
            <MonitorPlot key={`m-${runKey(run)}`} session={s} feed={view.feed} status={view.status} height={64} />
          </div>
          {view.feed && (
            <p className="plots-dev-stats" data-testid="feed-stats" data-full={view.feed.stats.full} data-increments={view.feed.stats.increments} data-resyncs={view.feed.stats.resyncs} data-first-fetch-ms={view.feed.stats.firstFetchMs ?? ''}>
              monitors.get: {view.feed.stats.full} whole, {view.feed.stats.increments} increments, {view.feed.stats.discovery} name lists, {view.feed.stats.resyncs} resyncs
              {view.feed.stats.firstFetchMs !== null && ` · first fetch ${view.feed.stats.firstFetchMs.toFixed(1)} ms`}
              {view.feed.stats.lastPollMs !== null && ` · last poll ${view.feed.stats.lastPollMs.toFixed(1)} ms, ${view.feed.stats.lastAdded} samples added`}
            </p>
          )}
        </section>
      ) : (
        <p className="plots-dev-none">{connected ? 'Pick a session, or make one from a case file of the service’s sandbox.' : 'No service connected: the plots need a running rcfd serve (the Studio starts its engine’s with the first project; --service attaches to another).'}</p>
      )}

      <footer className="plots-dev-foot">rcfd is early-stage software. It is not validated for engineering decisions or for safety-critical use, and it comes with no warranty.</footer>
    </main>
  );
}

