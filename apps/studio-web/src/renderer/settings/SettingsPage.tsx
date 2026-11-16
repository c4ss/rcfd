// The case settings page at #/settings and #/settings/<session> (F-825): the header band with the session in use, the
// schema-driven surface for that session, and the disclaimer footer. It reads the engine's schema once per service
// instance (settings.schema), lists the service's sessions (session.list), makes a session from the engine's defaults
// (session.create), and passes the service's events on to the surface so a change made by another writer shows.
// Loaded on demand from main.tsx. F-829: undo and redo of the session's edits in the header band, and Close session,
// which cannot be undone, asks first in words that name what is lost.
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ServiceState } from '../../shared/service.js';
import { dispatch, history, watchEvents, watchServiceState } from '../actions/index.js';
import { NO_INVERSE, statusFacts } from '../../shared/actions/loss.js';
import { DecisionPanel } from '../history/DecisionPanel.js';
import { UndoControls } from '../history/UndoControls.js';
import { Button, Select } from '../ui/index.js';
import { caseModel, type CaseModel } from './schema.js';
import { SettingsSurface } from './SettingsSurface.js';
import './settings.css';

export const SETTINGS_HASH = '#/settings';

/** The session named by the location hash, or null. */
export function sessionFromHash(hash: string): string | null {
  const m = /^#\/settings\/(.+)$/.exec(hash);
  return m ? decodeURIComponent(m[1]!) : null;
}

const STATE_KINDS = new Set(['created', 'initialized', 'started', 'paused', 'resumed', 'stopped', 'finished', 'error', 'closed', 'chunk_done']);

interface SessionRow {
  readonly session: string;
  readonly state: string;
}

export default function SettingsPage() {
  const [service, setService] = useState<ServiceState | null>(null);
  const [model, setModel] = useState<CaseModel | null>(null);
  const [schemaError, setSchemaError] = useState<string | null>(null);
  const [sessions, setSessions] = useState<readonly SessionRow[] | null>(null);
  const [session, setSession] = useState<string | null>(() => sessionFromHash(location.hash));
  const [epochs, setEpochs] = useState({ settings: 0, state: 0 });
  const [createError, setCreateError] = useState<string | null>(null);
  /** Close session asked: what closing loses, in words (F-829). */
  const [closing, setClosing] = useState<{ session: string; loss: string } | null>(null);
  const [closeError, setCloseError] = useState<string | null>(null);

  useEffect(() => {
    const onHash = () => setSession(sessionFromHash(location.hash));
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);

  useEffect(() => watchServiceState(setService), []);
  const connected = service?.status === 'connected';
  const instance = service?.instance ?? 0;

  const listSessions = useCallback(async () => {
    const r = await dispatch('session.list', {});
    if (r.ok) {
      const rows = (r.result as { sessions?: unknown }).sessions;
      setSessions(Array.isArray(rows) ? rows.map((x) => ({ session: String(x?.session ?? ''), state: String(x?.state ?? '') })) : []);
    }
  }, []);

  // the schema and the sessions, once per service instance (a restarted service may be another engine version)
  useEffect(() => {
    if (!connected) return;
    let live = true;
    void dispatch('settings.schema', {}).then((r) => {
      if (!live) return;
      if (!r.ok) {
        setSchemaError(r.error.message);
        return;
      }
      try {
        setModel(caseModel(r.result));
        setSchemaError(null);
      } catch (e) {
        setSchemaError(e instanceof Error ? e.message : String(e));
      }
    });
    void listSessions();
    return () => {
      live = false;
    };
  }, [connected, instance, listSessions]);

  // the service's events: settings changes of the session in use (from anyone), run states, sessions made and closed
  useEffect(() => {
    if (!connected) return;
    let stop: (() => void) | null = null;
    let live = true;
    void watchEvents((batch) => {
      let settings = false;
      let state = false;
      let list = batch.missed > 0;
      for (const e of batch.events) {
        if (e.kind === 'created' || e.kind === 'closed') list = true;
        if (e.session !== session) continue;
        if (e.kind === 'setting_changed') settings = true;
        if (STATE_KINDS.has(e.kind)) state = true;
      }
      if (batch.missed > 0) settings = state = true;
      if (settings || state) setEpochs((x) => ({ settings: x.settings + (settings ? 1 : 0), state: x.state + (state ? 1 : 0) }));
      if (list) void listSessions();
    }).then((r) => {
      if (!r.ok) return;
      if (live) stop = r.result;
      else r.result();
    });
    return () => {
      live = false;
      stop?.();
    };
  }, [connected, instance, session, listSessions]);

  const choose = (id: string) => {
    location.hash = `${SETTINGS_HASH}/${encodeURIComponent(id)}`;
  };

  const create = async () => {
    setCreateError(null);
    const r = await dispatch('session.create', {});
    if (!r.ok) {
      setCreateError(r.error.message);
      return;
    }
    const id = (r.result as { session?: unknown }).session;
    if (typeof id === 'string') {
      await listSessions();
      choose(id);
    }
  };

  const askClose = async (id: string) => {
    setCloseError(null);
    const st = await dispatch('run.status', { session: id });
    const facts = { ...(st.ok ? statusFacts(st.result) : {}), edits: history().view(id).titles };
    setClosing({ session: id, loss: NO_INVERSE['session.close']({ session: id }, facts) ?? '' });
  };

  const close = async (id: string) => {
    setClosing(null);
    const r = await dispatch('session.close', { session: id });
    if (!r.ok && r.error.code !== 1001) {
      setCloseError(r.error.message);
      return;
    }
    await listSessions();
    location.hash = SETTINGS_HASH;
  };

  const options = useMemo(() => (sessions ?? []).map((s) => ({ id: s.session, label: `${s.session} (${s.state})` })), [sessions]);
  const known = session !== null && sessions !== null && sessions.some((s) => s.session === session);

  let body;
  if (!connected) {
    body = (
      <div className="st-empty" role="status">
        <h2>{service?.status === 'off' ? 'No rcfd service' : 'Not connected to the rcfd service'}</h2>
        <p>
          {service === null
            ? 'Reading the connection state.'
            : service.status === 'off'
              ? 'The Studio was not told which service to use. Start it with RCFD_STUDIO_SERVICE (attach to a running rcfd serve) or RCFD_STUDIO_RCFD (start one).'
              : `The connection is ${service.status}${service.lastError ? `: ${service.lastError}` : ''}. The settings appear when it is back.`}
        </p>
      </div>
    );
  } else if (schemaError) {
    body = (
      <div className="st-empty" role="status">
        <h2>The case schema could not be read</h2>
        <p>The engine answered: {schemaError}</p>
      </div>
    );
  } else if (model === null || sessions === null) {
    body = (
      <div className="st-empty" role="status">
        <h2>Reading the case schema from the engine</h2>
      </div>
    );
  } else if (!known) {
    body = (
      <div className="st-empty">
        <h2>{session === null ? 'Choose a session' : `Session ${session} is not on the service`}</h2>
        <p>
          Settings belong to a session of the rcfd service. Pick one the service holds, or make a new one from the
          engine's defaults.
        </p>
        {sessions.length > 0 && (
          <ul className="st-session-list" aria-label="Sessions of the service">
            {sessions.map((s) => (
              <li key={s.session}>
                <Button onPress={() => choose(s.session)}>Open {s.session}</Button>
                <span>{s.state}</span>
              </li>
            ))}
          </ul>
        )}
        <Button variant="primary" icon="plus" onPress={() => void create()}>
          New session from defaults
        </Button>
        {createError && <p className="st-key-error">The engine answered: {createError}</p>}
      </div>
    );
  } else {
    body = <SettingsSurface model={model} session={session!} address={service?.address ?? null} settingsEpoch={epochs.settings} stateEpoch={epochs.state} />;
  }

  return (
    <div className="st-page">
      <header className="st-top">
        <div className="st-brand">
          rcfd <span>Studio</span>
        </div>
        <h1 className="st-title">Case settings</h1>
        <div className="st-top-right">
          <span className="st-conn">{service ? `Service ${service.status}${service.address ? `, ${service.address}` : ''}` : ''}</span>
          {connected && sessions !== null && sessions.length > 0 && (
            <Select aria-label="Session" options={options} selected={known ? session : null} onChange={choose} onBand placeholder="Choose a session" />
          )}
          {connected && (
            <Button variant="ghost" icon="plus" onPress={() => void create()}>
              New session
            </Button>
          )}
          {connected && known && (
            <Button variant="ghost" icon="close" onPress={() => void askClose(session!)}>
              Close session
            </Button>
          )}
          {connected && <UndoControls scope={session} onRenamed={(_from, to) => choose(to)} />}
        </div>
      </header>
      {closing && (
        <DecisionPanel
          title={`Close session ${closing.session}?`}
          onCancel={() => setClosing(null)}
          choices={[
            { label: 'Keep the session', onPress: () => setClosing(null), safe: true },
            { label: `Close ${closing.session} and lose it`, onPress: () => void close(closing.session) },
          ]}
        >
          <p>{closing.loss}</p>
        </DecisionPanel>
      )}
      {closeError && (
        <DecisionPanel title="Failed to close the session" failed onCancel={() => setCloseError(null)} choices={[{ label: 'Dismiss', onPress: () => setCloseError(null), safe: true }]}>
          <p>The engine answered: {closeError}</p>
        </DecisionPanel>
      )}
      {/* the surface renders its own three columns; the states before it fill the body */}
      {known && connected && model !== null ? body : <div className="st-body st-body--single">{body}</div>}
      <footer className="st-foot">
        <span>
          rcfd is early-stage software. It is not validated for engineering decisions or for safety-critical use, and it
          comes with no warranty.
        </span>
        <span className="st-foot-right">Values, defaults, ranges and messages come from the engine's schema and replies.</span>
      </footer>
    </div>
  );
}
