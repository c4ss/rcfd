// The case editor (F-812; .impeccable/surfaces/apps-studio-web-src-renderer-case.md): a case file of the open project
// opened into a service session, edited with the F-825 settings surface, checked and saved back through the engine.
// Loaded on demand from main.tsx for the routes of hash.ts.
//
// Every value, message, template and example name is the engine's. The session actions (session.create,
// session.close, settings.*, mesh.check) go through the F-827 dispatcher; the case-file actions (bind, save, new,
// duplicate, check the saved file) through the case port (door.ts), whose main-process side calls the engine's project
// server (src/main/case/editor.ts).
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from 'react';
import type { CaseDoorState, CaseSources, CaseStatus, ConflictView, CreatedCase, FileCheck, SaveOutcome } from '../../shared/case.js';
import { CASE_ACTIONS } from '../../shared/case.js';
import { useCommandHandlers } from '../commands/host.js';
import type { JsonValue } from '../../shared/actions/params.js';
import type { ProjectState } from '../../shared/project.js';
import type { ServiceState } from '../../shared/service.js';
import { dispatch, watchEvents, watchServiceState } from '../actions/index.js';
import { watchProjectState } from '../project/door.js';
import { caseModel, formatValue, type CaseModel } from '../settings/schema.js';
import { useSettings } from '../settings/context.js';
import { SettingsSurface } from '../settings/SettingsSurface.js';
import { UndoControls } from '../history/UndoControls.js';
import { Button, Icon, SegmentedToggle, Select, StatePictogram, TextField } from '../ui/index.js';
import { caseDoor } from './door.js';
import { caseHash, caseRoute, replaceHash, type CaseRoute } from './hash.js';
import { openCase, type OpenFailure } from './open.js';
import '../settings/settings.css';
import './case.css';

/** Keys of a pending save listed by name; the rest are counted. */
const LISTED_KEYS = 12;
const STATE_KINDS = new Set(['created', 'initialized', 'started', 'paused', 'resumed', 'stopped', 'finished', 'error', 'closed', 'chunk_done']);

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
const fileName = (p: string) => p.split('/').pop() ?? p;
const clock = (iso: string | null) => (iso ? new Date(iso).toLocaleTimeString() : 'not yet');

const CLAUDE = (id: string) => CASE_ACTIONS.find((a) => a.id === id)!;

function useRoute(): CaseRoute | null {
  const [route, setRoute] = useState(() => caseRoute(location.hash));
  useEffect(() => {
    const on = () => setRoute(caseRoute(location.hash));
    window.addEventListener('hashchange', on);
    return () => window.removeEventListener('hashchange', on);
  }, []);
  return route;
}

function useProject(): ProjectState | null {
  const [s, setS] = useState<ProjectState | null>(null);
  useEffect(() => watchProjectState(setS), []);
  return s;
}

/** The connection and the engine's schema, once per service instance. */
function useEngine(): { service: ServiceState | null; model: CaseModel | null; schemaError: string | null } {
  const [service, setService] = useState<ServiceState | null>(null);
  const [model, setModel] = useState<CaseModel | null>(null);
  const [schemaError, setSchemaError] = useState<string | null>(null);
  useEffect(() => watchServiceState(setService), []);
  const connected = service?.status === 'connected';
  const instance = service?.instance ?? 0;
  useEffect(() => {
    if (!connected) return;
    let live = true;
    void dispatch('settings.schema', {}).then((r) => {
      if (!live) return;
      if (!r.ok) return setSchemaError(r.error.message);
      try {
        setModel(caseModel(r.result));
        setSchemaError(null);
      } catch (e) {
        setSchemaError(e instanceof Error ? e.message : String(e));
      }
    });
    return () => {
      live = false;
    };
  }, [connected, instance]);
  return { service, model, schemaError };
}

export default function CaseEditorPage() {
  const route = useRoute();
  const project = useProject();
  const engine = useEngine();
  const [title, setTitle] = useState<string>('Case editor');
  const [stateWord, setStateWord] = useState<ReactNode>(null);

  let body: ReactNode;
  if (route === null || route.kind === 'index') body = <IndexView project={project} />;
  else if (route.kind === 'new') body = <NewCaseView project={project} />;
  else if (route.kind === 'open') body = <OpenView key={`${route.path}|${route.replaces ?? ''}`} path={route.path} replaces={route.replaces} />;
  else body = <EditorView key={route.session} session={route.session} engine={engine} project={project} setTitle={setTitle} setStateWord={setStateWord} />;

  const editing = route?.kind === 'session';
  useEffect(() => {
    if (!editing) {
      setTitle(route?.kind === 'new' ? 'New case' : route?.kind === 'open' ? `Opening ${fileName(route.path)}` : 'Case editor');
      setStateWord(null);
    }
  }, [editing, route]);

  return (
    <div className="st-page">
      <header className="st-top">
        <div className="st-brand">
          rcfd <span>Studio</span>
        </div>
        <div className="ce-tab" title={title}>
          {title}
        </div>
        <div className="ce-top-actions">
          <Button variant="ghost" icon="back" onPress={() => void (location.hash = '')}>
            Project home
          </Button>
          {project?.project && route?.kind !== 'new' && (
            <Button variant="ghost" icon="plus" onPress={() => void (location.hash = caseHash.new)}>
              New case
            </Button>
          )}
        </div>
        <div className="st-top-right">
          {/* F-829: undo and redo of this file's session edits; saving is the file's own step (hash guarded) */}
          {editing && <UndoControls scope={route.session} />}
          {stateWord}
          <span className="st-conn">{engine.service ? `Service ${engine.service.status}${engine.service.address ? `, ${engine.service.address}` : ''}` : ''}</span>
        </div>
      </header>
      {body}
      <footer className="st-foot">
        <span>
          rcfd is early-stage software. It is not validated for engineering decisions or for safety-critical use, and it
          comes with no warranty.
        </span>
        <span className="st-foot-right">The case file is read, checked and written by the engine.</span>
      </footer>
    </div>
  );
}

function Single({ children }: { children: ReactNode }) {
  return <div className="st-body st-body--single">{children}</div>;
}

// ---- #/case: the sessions open on case files

function IndexView({ project }: { project: ProjectState | null }) {
  const [state, setState] = useState<CaseDoorState | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    void caseDoor.state().then((r) => (r.ok ? setState(r.result) : setError(r.error.message)));
  }, []);
  return (
    <Single>
      <div className="st-empty">
        <h2>Case files open in the editor</h2>
        {error && <p className="st-key-error">{error}</p>}
        {state && state.bindings.length === 0 && (
          <p>
            No case file is open. Open one from the project home (Edit beside a case), or make a new case
            {project?.project ? '' : ' once a project is open'}.
          </p>
        )}
        {state && state.bindings.length > 0 && (
          <ul className="st-session-list" aria-label="Case files open in the editor">
            {state.bindings.map((b) => (
              <li key={b.session}>
                <Button onPress={() => void (location.hash = caseHash.session(b.session))}>Edit {b.path}</Button>
                <span>
                  session {b.session}, opened {clock(b.openedAt)}
                </span>
              </li>
            ))}
          </ul>
        )}
        {project?.project && (
          <Button variant="primary" icon="plus" onPress={() => void (location.hash = caseHash.new)}>
            New case
          </Button>
        )}
      </div>
    </Single>
  );
}

// ---- #/case/open/<path>: opening a case file

function OpenView({ path, replaces }: { path: string; replaces: string | null }) {
  const [failure, setFailure] = useState<OpenFailure | null>(null);
  const started = useRef(false);
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    void openCase(path).then(async (r) => {
      if (!r.ok) return setFailure(r.failure);
      if (replaces && replaces !== r.session) {
        await dispatch('session.close', { session: replaces });
        await caseDoor.forget(replaces);
      }
      replaceHash(caseHash.session(r.session));
    });
  }, [path, replaces]);
  return (
    <Single>
      <div className="st-empty" role="status">
        {failure === null ? (
          <h2>Opening {path}</h2>
        ) : (
          <>
            <h2 className="ce-failed-title">
              <StatePictogram state="failed" />
              Failed to open {path}
            </h2>
            <OpenFailureText failure={failure} path={path} />
            <div className="ce-row">
              <Button onPress={() => replaceHash(caseHash.open(path, replaces ?? undefined))}>Try again</Button>
              <Button variant="ghost" onPress={() => void (location.hash = '')}>
                Project home
              </Button>
            </div>
          </>
        )}
      </div>
    </Single>
  );
}

function OpenFailureText({ failure, path }: { failure: OpenFailure; path: string }) {
  if (failure.diagnostics.length > 0) {
    return (
      <>
        <p>The engine does not accept {path} as a case. Fix the file in a text editor, or ask Claude Code, then try again:</p>
        <ul className="st-diags" aria-label="Problems found by the engine">
          {failure.diagnostics.map((d, i) => (
            <li key={i}>
              <strong>
                {d.path || 'the file'}
                {d.line !== null ? `, line ${d.line}` : ''}
              </strong>
              <span>{d.message}</span>
            </li>
          ))}
        </ul>
      </>
    );
  }
  if (failure.code === 1004) {
    return (
      <p>
        The engine answered: {failure.message}. The solver service only reads files inside its sandbox; start the Studio with a
        service whose sandbox (RCFD_STUDIO_SERVICE_ROOT) holds the project folder, or none.
      </p>
    );
  }
  return <p>The engine answered: {failure.message}</p>;
}

// ---- #/case/new

function NewCaseView({ project }: { project: ProjectState | null }) {
  const [sources, setSources] = useState<CaseSources | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [kind, setKind] = useState<'template' | 'example'>('template');
  const [from, setFrom] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const open = project?.project ?? null;

  useEffect(() => {
    if (!open) return;
    void caseDoor.sources().then((r) => {
      if (!r.ok) return setError(r.error.message);
      setSources(r.result);
      setFrom(r.result.templates[0] ?? r.result.examples[0] ?? null);
      if (r.result.templates.length === 0) setKind('example');
    });
  }, [open?.folder]);

  const list = sources ? (kind === 'template' ? sources.templates : sources.examples) : [];
  const create = async () => {
    if (!from) return;
    setBusy(true);
    setRefused(null);
    const r = await caseDoor.create(name.trim(), { kind, name: from });
    setBusy(false);
    if (!r.ok) return setRefused(r.error.message);
    location.hash = caseHash.open(r.result.path);
  };

  if (!open) {
    return (
      <Single>
        <div className="st-empty">
          <h2>No project open</h2>
          <p>A new case is a file in the cases folder of a project. Open the project folder from the project home first.</p>
        </div>
      </Single>
    );
  }
  return (
    <Single>
      <form
        className="ce-new"
        aria-labelledby="ce-new-title"
        onSubmit={(e) => {
          e.preventDefault();
          void create();
        }}
      >
        <h2 id="ce-new-title">New case in {open.info?.name ?? open.folder}</h2>
        <p>
          The engine writes the new file into cases/, checks it first and never replaces a file. It then opens here for
          editing.
        </p>
        {error && <p className="st-key-error">The engine answered: {error}</p>}
        {sources && (
          <>
            <TextField
              label="Case name"
              description={sources.nameHelp || undefined}
              value={name}
              onChange={setName}
              placeholder="my-cavity"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Escape' && name !== '') {
                  e.preventDefault();
                  setName('');
                }
              }}
            />
            <div className="ce-field">
              <span className="ui-label" aria-hidden="true">
                Start from
              </span>
              <SegmentedToggle
                aria-label="Start from"
                options={[
                  { id: 'template', label: `A template (${sources.templates.length})` },
                  { id: 'example', label: `A shipped example (${sources.examples.length})` },
                ]}
                selected={kind}
                onChange={(k) => {
                  const next = k as 'template' | 'example';
                  setKind(next);
                  setFrom((next === 'template' ? sources.templates : sources.examples)[0] ?? null);
                }}
              />
              <span className="st-small">{kind === 'template' ? sources.templateHelp : sources.exampleHelp}</span>
            </div>
            <Select
              label={kind === 'template' ? 'Template' : 'Shipped example'}
              options={list.map((x) => ({ id: x, label: x }))}
              selected={from}
              onChange={setFrom}
              placeholder="Choose one"
            />
            <div className="ce-row">
              <Button variant="primary" icon="plus" type="submit" isDisabled={busy || !from || name.trim() === ''}>
                {busy ? 'Making the case' : 'Make the case'}
              </Button>
              {name.trim() === '' && <span className="st-small">Type a name first.</span>}
            </div>
            {refused && (
              <p className="ce-refused" role="alert">
                <StatePictogram state="failed" />
                <span>Failed: {refused}</span>
              </p>
            )}
            <p className="st-small">From Claude Code: the project tool {CLAUDE('case.new').mcp}.</p>
          </>
        )}
      </form>
    </Single>
  );
}

// ---- #/case/<session>: the editor

interface EditorProps {
  session: string;
  engine: ReturnType<typeof useEngine>;
  project: ProjectState | null;
  setTitle: (t: string) => void;
  setStateWord: (n: ReactNode) => void;
}

type Notice =
  | { kind: 'saved'; outcome: Extract<SaveOutcome, { kind: 'saved' }> }
  | { kind: 'refused'; outcome: Extract<SaveOutcome, { kind: 'refused' }> }
  | { kind: 'cannot'; outcome: Extract<SaveOutcome, { kind: 'cannot' }> }
  | { kind: 'conflict'; view: Extract<ConflictView, { changed: true }> }
  | { kind: 'nothing' }
  | { kind: 'failed'; message: string };

function EditorView({ session, engine, project, setTitle, setStateWord }: EditorProps) {
  const { service, model, schemaError } = engine;
  const connected = service?.status === 'connected';
  const [status, setStatus] = useState<CaseStatus | null>(null);
  const [statusError, setStatusError] = useState<{ code: unknown; message: string } | null>(null);
  const [epochs, setEpochs] = useState({ settings: 0, state: 0 });
  const [fileEpoch, setFileEpoch] = useState(0);

  const refresh = useCallback(async () => {
    const r = await caseDoor.status(session);
    if (r.ok) {
      setStatus(r.result);
      setStatusError(null);
    } else {
      setStatusError(r.error);
    }
  }, [session]);

  useEffect(() => {
    void refresh();
  }, [refresh, epochs.settings, fileEpoch]);

  // the service's events of this session: its settings (from any writer) and its run state
  useEffect(() => {
    if (!connected) return;
    let stop: (() => void) | null = null;
    let live = true;
    void watchEvents((batch) => {
      let settings = batch.missed > 0;
      let state = batch.missed > 0;
      for (const e of batch.events) {
        if (e.session !== session) continue;
        if (e.kind === 'setting_changed') settings = true;
        if (STATE_KINDS.has(e.kind)) state = true;
      }
      if (settings || state) setEpochs((x) => ({ settings: x.settings + (settings ? 1 : 0), state: x.state + (state ? 1 : 0) }));
    }, session).then((r) => {
      if (!r.ok) return;
      if (live) stop = r.result;
      else r.result();
    });
    return () => {
      live = false;
      stop?.();
    };
  }, [connected, service?.instance, session]);

  // the engine's change feed of the project: a change of this file (by anyone) reads its state again
  const path = status?.binding.path ?? null;
  const seen = useRef(-1);
  useEffect(() => {
    if (!path) return;
    const changes = project?.project?.changes ?? [];
    const latest = changes.find((c) => c.path === path)?.n ?? 0;
    if (seen.current >= 0 && latest > seen.current) setFileEpoch((x) => x + 1);
    seen.current = latest;
  }, [project, path]);

  useEffect(() => {
    if (status) setTitle(fileName(status.binding.path));
  }, [status, setTitle]);

  useEffect(() => {
    setStateWord(status ? <FileStateWord status={status} /> : null);
  }, [status, setStateWord]);

  if (statusError && !status) {
    return (
      <Single>
        <div className="st-empty">
          <h2>{statusError.code === 'not_bound' ? `Session ${session} is not open on a case file` : 'The case file could not be read'}</h2>
          <p>{statusError.message}</p>
          <div className="ce-row">
            <Button onPress={() => void (location.hash = '')}>Project home</Button>
            <Button variant="ghost" onPress={() => void (location.hash = `#/settings/${encodeURIComponent(session)}`)}>
              The session's settings
            </Button>
          </div>
        </div>
      </Single>
    );
  }
  if (!connected || schemaError || model === null || status === null) {
    return (
      <Single>
        <div className="st-empty" role="status">
          <h2>
            {!connected
              ? 'Not connected to the rcfd service'
              : schemaError
                ? 'The case schema could not be read'
                : `Reading ${status?.binding.path ?? `session ${session}`}`}
          </h2>
          {schemaError && <p>The engine answered: {schemaError}</p>}
          {!connected && service && <p>The connection is {service.status}{service.lastError ? `: ${service.lastError}` : ''}. The editor appears when it is back.</p>}
        </div>
      </Single>
    );
  }
  return (
    <SettingsSurface
      model={model}
      session={session}
      address={service?.address ?? null}
      settingsEpoch={epochs.settings}
      stateEpoch={epochs.state}
      aside={<CasePanel status={status} refresh={refresh} />}
    />
  );
}

function FileStateWord({ status }: { status: CaseStatus }) {
  const n = status.pending.count;
  if (status.file.state === 'changed') return <span className="ce-word">Changed outside</span>;
  if (status.file.state === 'missing') return <span className="ce-word ce-word--failed"><StatePictogram state="failed" />File missing</span>;
  return <span className="ce-word">{n === 0 ? 'Saved' : `${plural(n, 'change', 'changes')} not saved`}</span>;
}

// ---- the case file panel at the head of the right column (inside the settings surface's context)

function CasePanel({ status, refresh }: { status: CaseStatus; refresh: () => Promise<void> }) {
  const settings = useSettings();
  const b = status.binding;
  const pending = status.pending;
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [confirmClose, setConfirmClose] = useState(false);
  const [dupName, setDupName] = useState('');
  const [dup, setDup] = useState<{ ok: true; made: CreatedCase } | { ok: false; message: string } | null>(null);
  const [fileCheck, setFileCheck] = useState<FileCheck | { failed: string } | null>(null);
  const [meshCheck, setMeshCheck] = useState<{ ok: boolean; cells: number | null; patches: string[]; errors: string[]; warnings: string[] } | { failed: string } | null>(null);
  const noticeRef = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    if (notice && notice.kind !== 'saved' && notice.kind !== 'nothing') noticeRef.current?.focus();
  }, [notice]);

  const canSave = status.projectOpen && settings.editable && busy === null;
  const saveReason = !status.projectOpen
    ? `The project ${b.folder} is not open; open it again from the project home to save.`
    : !settings.editable
      ? settings.readOnlyReason
      : null;

  const save = useCallback(
    async (onto: 'opened' | 'current') => {
      setBusy('save');
      setConfirmClose(false);
      // a value typed but not yet committed (Enter or leaving the field commits it): commit it first
      const active = document.activeElement as HTMLElement | null;
      if (active && active.closest('.st-key') && active.matches('input, textarea')) {
        active.blur();
        await new Promise((r) => setTimeout(r, 150));
      }
      const r = await caseDoor.save(b.session, onto);
      setBusy(null);
      if (!r.ok) {
        setNotice({ kind: 'failed', message: r.error.message });
        return;
      }
      const o = r.result;
      if (o.kind === 'saved') {
        if (o.reopen) {
          location.hash = caseHash.open(b.path, b.session);
          return;
        }
        setNotice({ kind: 'saved', outcome: o });
      } else if (o.kind === 'conflict') {
        const c = await caseDoor.conflict(b.session);
        if (c.ok && c.result.changed) setNotice({ kind: 'conflict', view: c.result });
        else setNotice({ kind: 'failed', message: o.message });
      } else if (o.kind === 'refused') setNotice({ kind: 'refused', outcome: o });
      else if (o.kind === 'cannot') setNotice({ kind: 'cannot', outcome: o });
      else if (onto === 'current') location.hash = caseHash.open(b.path, b.session);
      else setNotice({ kind: 'nothing' });
      await refresh();
    },
    [b.session, b.path, refresh],
  );

  // Ctrl+S (Cmd+S) saves from anywhere on the page (G20: a frequent action has a shortcut)
  const saveRef = useRef(save);
  saveRef.current = save;
  const canSaveRef = useRef(canSave);
  canSaveRef.current = canSave;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 's') {
        e.preventDefault();
        if (canSaveRef.current) void saveRef.current('opened');
      }
    };
    // capture: React Aria's fields stop the propagation of the keys they handle
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  const reload = () => {
    location.hash = caseHash.open(b.path, b.session);
  };

  const close = async () => {
    if (pending.count > 0 && !confirmClose) {
      setConfirmClose(true);
      return;
    }
    setBusy('close');
    const r = await dispatch('session.close', { session: b.session });
    setBusy(null);
    if (!r.ok && r.error.code !== 1001) {
      setNotice({ kind: 'failed', message: r.error.message });
      return;
    }
    await caseDoor.forget(b.session);
    location.hash = '';
  };

  const duplicate = async () => {
    setBusy('duplicate');
    const r = await caseDoor.duplicate(b.path, dupName.trim());
    setBusy(null);
    setDup(r.ok ? { ok: true, made: r.result } : { ok: false, message: r.error.message });
  };

  const checkFile = async () => {
    setBusy('check');
    const r = await caseDoor.check(b.path);
    setBusy(null);
    setFileCheck(r.ok ? r.result : { failed: r.error.message });
  };

  const checkMesh = async () => {
    setBusy('mesh-check');
    const r = await dispatch('mesh.check', { session: b.session });
    setBusy(null);
    if (!r.ok) return setMeshCheck({ failed: r.error.message });
    const x = r.result as { ok?: unknown; patches?: { name?: unknown }[]; report?: { n_cells?: unknown; errors?: unknown; warnings?: unknown } };
    const strings = (v: unknown) => (Array.isArray(v) ? v.map((s) => (typeof s === 'string' ? s : JSON.stringify(s))) : []);
    setMeshCheck({
      ok: x.ok === true,
      cells: typeof x.report?.n_cells === 'number' ? x.report.n_cells : null,
      patches: Array.isArray(x.patches) ? x.patches.map((p) => String(p?.name ?? '')) : [],
      errors: strings(x.report?.errors),
      warnings: strings(x.report?.warnings),
    });
  };

  const file = status.file;
  const outside = file.state === 'changed' && notice?.kind !== 'conflict';

  // F-831: the case actions of this file as commands of the palette (Ctrl+S stays the editor's own key, above)
  useCommandHandlers({
    'case.save': {
      unavailable: () => saveReason ?? (busy !== null ? 'The editor is busy with another case action; wait for it.' : pending.count === 0 ? `Nothing to save: ${b.path} has no change since ${b.savedAt ? 'the last save' : 'it was opened'}.` : null),
      run: () => save('opened'),
    },
    'case.check_file': {
      unavailable: () => (file.state === 'missing' ? `${b.path} is not there: the saved file cannot be checked.` : busy !== null ? 'The editor is busy with another case action; wait for it.' : null),
      run: () => checkFile(),
    },
    'case.duplicate': {
      unavailable: () => (file.state === 'missing' ? `${b.path} is not there: duplicate needs the saved file.` : null),
      // its form: the "Duplicate as" field of the case actions
      run: () => document.querySelector<HTMLElement>('.ce-dup input')?.focus(),
    },
    'case.close': {
      unavailable: () => (busy !== null ? 'The editor is busy with another case action; wait for it.' : null),
      // asks first, naming the changes not saved, as the button does
      run: () => close(),
    },
  });

  return (
    <>
      <section className="ce-panel" aria-labelledby="ce-file-title">
        <h2 id="ce-file-title" className="ce-file-title">
          {b.path}
        </h2>
        <dl className="st-facts">
          <dt>State</dt>
          <dd>
            <FileStateWord status={status} />
          </dd>
          <dt>Project</dt>
          <dd>{b.folder}</dd>
          <dt>Opened</dt>
          <dd>{clock(b.openedAt)}</dd>
          <dt>Last saved</dt>
          <dd>{clock(b.savedAt)}</dd>
          <dt>Read at hash</dt>
          <dd>{b.hash}</dd>
        </dl>
        {pending.count > 0 && (
          <div className="ce-pending">
            <p>
              {plural(pending.count, 'change', 'changes')} since {b.savedAt ? 'the last save' : 'the file was opened'}; a save writes{' '}
              {plural(pending.paths.length, 'setting', 'settings')}:
            </p>
            <ul aria-label="Settings a save writes">
              {pending.paths.slice(0, LISTED_KEYS).map((p) => (
                <li key={p}>{p}</li>
              ))}
              {pending.paths.length > LISTED_KEYS && <li>and {pending.paths.length - LISTED_KEYS} more</li>}
            </ul>
            {pending.removals.length > 0 && <p className="st-small">Removed: {pending.removals.join(', ')}.</p>}
          </div>
        )}
        <div className="ce-row">
          <Button variant="primary" icon="check" onPress={() => void save('opened')} isDisabled={!canSave || pending.count === 0}>
            {busy === 'save' ? 'Saving' : 'Save'}
          </Button>
          <span className="st-small">{saveReason ?? (pending.count === 0 ? 'Nothing to save. Ctrl+S saves.' : 'Ctrl+S saves.')}</span>
        </div>
        {outside && (
          <div className="ce-notice" role="status">
            <h3 className="ce-notice-title">
              {b.path} changed outside the Studio
            </h3>
            <p>
              Another writer (Claude Code, the CLI or an editor) changed the file after it was read here.{' '}
              {pending.count === 0
                ? 'Load it again to edit it as it is now.'
                : 'Saving now shows both sides and asks which to keep; nothing is overwritten without your choice.'}
            </p>
            {pending.count === 0 && (
              <div className="ce-row">
                <Button onPress={reload}>Load the file as it is now</Button>
              </div>
            )}
          </div>
        )}
        {file.state === 'missing' && (
          <div className="ce-notice ce-notice--failed" role="alert">
            <h3 className="ce-notice-title">
              <StatePictogram state="failed" />
              Failed to find {b.path}
            </h3>
            <p>{file.message} The session still holds the settings; duplicate is unavailable, and saving needs the file back.</p>
          </div>
        )}
        {notice && <NoticeView notice={notice} titleRef={noticeRef} path={b.path} pendingCount={pending.count} onGo={settings.goTo} onMine={() => void save('current')} onReload={reload} onDismiss={() => setNotice(null)} />}
      </section>

      <section className="ce-panel" aria-labelledby="ce-actions-title">
        <h2 id="ce-actions-title">Case actions</h2>
        <div className="ce-row">
          <Button onPress={() => void checkMesh()} isDisabled={busy !== null}>
            {busy === 'mesh-check' ? 'Checking the mesh' : 'Check the mesh'}
          </Button>
          <Button onPress={() => void checkFile()} isDisabled={busy !== null || file.state === 'missing'}>
            {busy === 'check' ? 'Checking the file' : 'Check the saved file'}
          </Button>
        </div>
        <MeshCheckView check={meshCheck} />
        <FileCheckView check={fileCheck} />
        <div className="ce-dup">
          <TextField
            label="Duplicate as"
            description="A new name in cases/: the saved file is copied byte for byte. Changes not saved stay here."
            value={dupName}
            onChange={setDupName}
            placeholder={`${fileName(b.path).replace(/\.(toml|json)$/i, '')}-copy`}
            onKeyDown={(e) => {
              if (e.key === 'Escape' && dupName !== '') {
                e.preventDefault();
                setDupName('');
              }
              if (e.key === 'Enter' && dupName.trim() !== '') void duplicate();
            }}
          />
          <Button onPress={() => void duplicate()} isDisabled={busy !== null || dupName.trim() === '' || file.state === 'missing'}>
            Duplicate
          </Button>
          {dup?.ok === true && (
            <p role="status">
              Made {dup.made.path} ({dup.made.from}).{' '}
              <Button variant="chip" onPress={() => void (location.hash = caseHash.open(dup.made.path))}>
                Open {dup.made.path}
              </Button>
            </p>
          )}
          {dup?.ok === false && (
            <p className="ce-refused" role="alert">
              <StatePictogram state="failed" />
              <span>Failed: {dup.message}</span>
            </p>
          )}
        </div>
        <div className="ce-close">
          {!confirmClose ? (
            <Button icon="close" onPress={() => void close()} isDisabled={busy !== null}>
              Close the session
            </Button>
          ) : (
            <div className="ce-confirm" role="alert">
              <p>
                Close session {b.session} and lose {plural(pending.count, 'change', 'changes')} not saved to {b.path}
                {pending.paths.length > 0 ? ` (${pending.paths.slice(0, 4).join(', ')}${pending.paths.length > 4 ? ', …' : ''})` : ''}?
              </p>
              <div className="ce-row">
                <Button onPress={() => void close()}>Close and lose them</Button>
                <Button variant="primary" onPress={() => setConfirmClose(false)} autoFocus>
                  Keep editing
                </Button>
              </div>
            </div>
          )}
        </div>
        <p className="st-small">
          From Claude Code, the same file: {CLAUDE('case.save').mcp} to save, {CLAUDE('case.check_file').mcp} to check. Command line: {CLAUDE('case.check_file').cli}.
        </p>
      </section>
    </>
  );
}

function NoticeView(props: {
  notice: Notice;
  titleRef: RefObject<HTMLHeadingElement | null>;
  path: string;
  pendingCount: number;
  onGo: (path: string) => void;
  onMine: () => void;
  onReload: () => void;
  onDismiss: () => void;
}) {
  const { notice: n, titleRef, path } = props;
  if (n.kind === 'nothing') return <p className="st-small" role="status">Nothing to save: the file already holds every change of this session.</p>;
  if (n.kind === 'saved') {
    const keys = Object.keys(n.outcome.values);
    return (
      <div className="ce-saved" role="status">
        <p className="st-verdict">
          <Icon name="check" />
          Saved {plural(keys.length, 'setting', 'settings')} to {n.outcome.path}
        </p>
        <p className="st-small">
          {n.outcome.form === 'minimal' ? 'The changed lines were edited in place; comments and layout kept.' : `Written as ${n.outcome.form}.`} Hash {n.outcome.newHash}.
          {n.outcome.mesh ? (n.outcome.mesh.ok ? ' The engine built and checked the edited mesh.' : ' The engine built the edited mesh, and its check failed: check the mesh.') : ''}
        </p>
        <ul className="ce-values" aria-label="Settings written">
          {keys.slice(0, LISTED_KEYS).map((k) => (
            <li key={k}>
              <span>{k}</span>
              <span>{formatValue(n.outcome.values[k] as JsonValue) || 'empty'}</span>
            </li>
          ))}
        </ul>
      </div>
    );
  }
  if (n.kind === 'failed') {
    return (
      <div className="ce-notice ce-notice--failed" role="alert">
        <h3 className="ce-notice-title" ref={titleRef} tabIndex={-1}>
          <StatePictogram state="failed" />
          Failed to save
        </h3>
        <p>The engine answered: {n.message}</p>
        <div className="ce-row">
          <Button onPress={props.onDismiss}>Dismiss</Button>
        </div>
      </div>
    );
  }
  if (n.kind === 'refused') {
    return (
      <div className="ce-notice ce-notice--failed" role="alert">
        <h3 className="ce-notice-title" ref={titleRef} tabIndex={-1}>
          <StatePictogram state="failed" />
          Save failed: the engine refused the edit
        </h3>
        <p>{n.outcome.message}</p>
        {n.outcome.diagnostics.length > 0 && (
          <ul className="st-diags" aria-label="Problems found by the engine">
            {n.outcome.diagnostics.map((d, i) => (
              <li key={i}>
                <span>{d.message}</span>
                {d.path !== '' && (
                  <Button variant="chip" onPress={() => props.onGo(d.path)} aria-label={`Go to ${d.path}`}>
                    {d.path}
                  </Button>
                )}
              </li>
            ))}
          </ul>
        )}
        <p className="st-small">Nothing was written. Change the value the engine names, then save again.</p>
        <div className="ce-row">
          <Button onPress={props.onDismiss}>Dismiss</Button>
        </div>
      </div>
    );
  }
  if (n.kind === 'cannot') {
    return (
      <div className="ce-notice" role="alert">
        <h3 className="ce-notice-title" ref={titleRef} tabIndex={-1}>
          Not saved: a removed entry
        </h3>
        <p>{n.outcome.message}</p>
        <div className="ce-row">
          <Button onPress={props.onDismiss}>Keep editing</Button>
        </div>
      </div>
    );
  }
  const v = n.view;
  return (
    <div className="ce-notice ce-conflict" role="alert">
      <h3 className="ce-notice-title" ref={titleRef} tabIndex={-1}>
        Not saved: {path} changed outside the Studio
      </h3>
      <p>
        Another writer changed the file after it was read here, so nothing was written. Your changes and the file as it is now:
      </p>
      {v.fileError && <p className="st-small">{v.fileError}</p>}
      <table className="ce-conflict-table">
        <thead>
          <tr>
            <th scope="col">Setting</th>
            <th scope="col">Yours</th>
            <th scope="col">In the file now</th>
          </tr>
        </thead>
        <tbody>
          {v.rows.map((r) => (
            <tr key={r.path}>
              <th scope="row">{r.path}</th>
              <td>{formatValue(r.mine as JsonValue) || 'empty'}</td>
              <td>{r.inFile ? formatValue(r.file as JsonValue) || 'empty' : 'not set'}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {v.removals.length > 0 && <p className="st-small">You also removed {v.removals.join(', ')}.</p>}
      <p>Choose one:</p>
      <div className="ce-choices">
        <Button onPress={props.onMine} isDisabled={v.currentHash === null}>
          Write my changes into the file as it is now
        </Button>
        <Button onPress={props.onReload} isDisabled={v.currentHash === null}>
          Load the file as it is now, dropping my {plural(props.pendingCount, 'change', 'changes')}
        </Button>
        <Button onPress={props.onDismiss}>Keep editing</Button>
      </div>
      <p className="st-small">Writing yours changes only the settings listed; the other writer's other changes stay in the file.</p>
    </div>
  );
}

function MeshCheckView({ check }: { check: { ok: boolean; cells: number | null; patches: string[]; errors: string[]; warnings: string[] } | { failed: string } | null }) {
  if (check === null) return null;
  if ('failed' in check) {
    return (
      <p className="ce-refused" role="status">
        <StatePictogram state="failed" />
        <span>Mesh check failed: {check.failed}</span>
      </p>
    );
  }
  return (
    <div className="ce-check" role="status">
      <p className={check.ok ? 'st-verdict' : 'st-verdict st-verdict--failed'}>
        {check.ok ? <Icon name="check" /> : <StatePictogram state="failed" />}
        {check.ok ? 'The mesh builds' : 'Mesh check failed'}
        {check.cells !== null ? `: ${check.cells} cells` : ''}
      </p>
      {check.patches.length > 0 && <p className="st-small">Patches (the names boundaries use): {check.patches.join(', ')}</p>}
      {check.errors.map((e, i) => (
        <p key={`e${i}`} className="st-key-error">
          {e}
        </p>
      ))}
      {check.warnings.map((w, i) => (
        <p key={`w${i}`} className="st-small">
          {w}
        </p>
      ))}
    </div>
  );
}

function FileCheckView({ check }: { check: FileCheck | { failed: string } | null }) {
  const words = useMemo(() => ({ 0: 'ok', 3: 'invalid', 4: 'bad mesh' }) as Record<number, string>, []);
  if (check === null) return null;
  if ('failed' in check) {
    return (
      <p className="ce-refused" role="status">
        <StatePictogram state="failed" />
        <span>The check of the saved file failed to run: {check.failed}</span>
      </p>
    );
  }
  return (
    <div className="ce-check" role="status">
      <p className={check.ok ? 'st-verdict' : 'st-verdict st-verdict--failed'}>
        {check.ok ? <Icon name="check" /> : <StatePictogram state="failed" />}
        {check.ok ? 'The saved file checks out' : 'Check of the saved file failed'}
        {check.exitCode !== null ? ` (exit code ${check.exitCode}${words[check.exitCode] ? `, ${words[check.exitCode]}` : ''})` : ''}
      </p>
      {check.cells !== null && <p className="st-small">{check.cells} cells.</p>}
      {check.diagnostics.map((d, i) => (
        <p key={`d${i}`} className="st-key-error">
          {d.path ? `${d.path}: ` : ''}
          {d.message}
        </p>
      ))}
      {check.meshErrors.map((e, i) => (
        <p key={`m${i}`} className="st-key-error">
          {e}
        </p>
      ))}
      <p className="st-small">The same from the command line: {CLAUDE('case.check_file').cli}.</p>
    </div>
  );
}

