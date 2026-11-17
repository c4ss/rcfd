// The schema-driven case settings surface for one service session (F-825; G2, G8, G15, G16, G19; PRODUCT Principle 5).
// Every key of settings.schema gets a control made from the schema at run time (schema.ts), laid out by learner-first
// disclosure (disclosure.ts). Every read and change is an action of the F-827 registry dispatched through ../actions;
// what a change did, or why it was refused, is the engine's reply, shown as the engine wrote it.
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { ListBox, ListBoxItem, type Key } from 'react-aria-components';
import type { JsonObject, JsonValue } from '../../shared/actions/params.js';
import { availability, dispatch } from '../actions/index.js';
import { equivalents, getAction, SETTINGS_CONFLICT } from '../../shared/actions/registry.js';
import { Button, EmptyMark, Icon, Quiet, StatePictogram, Switch, TextField } from '../ui/index.js';
import { Settings, useSettings, type SettingConflict, type SettingsContext } from './context.js';
import { filledCollections, keyCount, search, sectionOf, sections, setCount, setKeys, type Section } from './disclosure.js';
import { CollectionBody, GroupBody } from './Groups.js';
import { KeyRow } from './KeyRow.js';
import { entryNames, entrySeed, formatValue, valueAt, type CaseModel, type SettingNode } from './schema.js';
import { recoveryOf, useErrorCatalogue, type CatalogueEntry } from '../help/engine.js';
import { EngineRecovery } from '../help/Recovery.js';

export interface SettingsSurfaceProps {
  readonly model: CaseModel;
  readonly session: string;
  /** host:port of the service, for the CLI equivalent of a change. */
  readonly address: string | null;
  /** Raised by the page when the service reports a settings change of this session (another writer included). */
  readonly settingsEpoch: number;
  /** Raised by the page when the session's run state may have changed. */
  readonly stateEpoch: number;
  /**
   * Sections put at the head of the right column, inside the surface's context (F-812's case file panel uses its goTo):
   * the surface stays the same for every page that wraps it.
   */
  readonly aside?: ReactNode;
}

const SET_ID = 'set';
const sectionKey = (id: string) => `s:${id}`;
const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
/** Search results drawn at once; the count says how many more a longer query would narrow to. */
const MAX_HITS = 200;

interface CheckResult {
  readonly ok: boolean;
  readonly diagnostics: readonly { path: string; message: string; line: number | null }[];
}

interface LastChange {
  readonly action: 'settings.set' | 'settings.remove';
  readonly params: JsonObject;
  readonly stored?: JsonValue | undefined;
}

/**
 * The engine's words for a refused call: the diagnostics of 1003 for this path (or all of them), else its message; then
 * the error catalogue's recovery line for its code (F-830, `help.errors`), when the engine has one. The message names
 * the problem, the catalogue the way out (G19); neither is the Studio's.
 */
function engineMessage(error: { code: unknown; message: string; data?: unknown }, path: string, catalogue: readonly CatalogueEntry[] | null): string {
  const data = error.data as { diagnostics?: unknown } | undefined;
  const diags = Array.isArray(data?.diagnostics) ? (data.diagnostics as { path?: unknown; message?: unknown }[]) : [];
  const mine = diags.filter((d) => d.path === path);
  const pick = (mine.length ? mine : diags).map((d) => String(d.message ?? '')).filter((m) => m !== '');
  const problem = pick.length ? pick.join('; ') : error.message;
  const recovery = recoveryOf(catalogue, error.code);
  return recovery ? `${problem} ${recovery}` : problem;
}

/** A shell word in single quotes. */
const sq = (s: string) => (/^[A-Za-z0-9_./:@=-]+$/.test(s) ? s : `'${s.replaceAll("'", `'\\''`)}'`);

function withValue(tree: unknown, path: string, value: JsonValue | undefined): unknown {
  const parts = path.split('.');
  const step = (node: unknown, i: number): unknown => {
    const o = typeof node === 'object' && node !== null && !Array.isArray(node) ? (node as Record<string, unknown>) : {};
    const k = parts[i]!;
    if (i === parts.length - 1) return { ...o, [k]: value };
    return { ...o, [k]: step(o[k], i + 1) };
  };
  return step(tree, 0);
}

export function SettingsSurface({ model, session, address, settingsEpoch, stateEpoch, aside }: SettingsSurfaceProps) {
  const list = useMemo(() => sections(model), [model]);
  const [tree, setTree] = useState<unknown>(null);
  const [loadError, setLoadError] = useState<{ message: string; code: unknown } | null>(null);
  const [runState, setRunState] = useState<string | undefined>(undefined);
  const [selected, setSelected] = useState<string>(SET_ID);
  const [query, setQuery] = useState('');
  const [showAll, setShowAll] = useState(false);
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const [errors, setErrors] = useState<ReadonlyMap<string, string>>(new Map());
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set());
  const [prefill, setPrefill] = useState<ReadonlyMap<string, string>>(new Map());
  const [check, setCheck] = useState<CheckResult | 'running' | { failed: string } | null>(null);
  const [last, setLast] = useState<LastChange | null>(null);
  /** Accepted changes so far (here or by another writer); a check result older than the latest one says so. */
  const [changes, setChanges] = useState(0);
  const [checkedAt, setCheckedAt] = useState(0);
  const [announce, setAnnounce] = useState('');
  const [conflicts, setConflicts] = useState<ReadonlyMap<string, SettingConflict>>(new Map());
  const [focusTarget, setFocusTarget] = useState<{ path: string; n: number } | null>(null);
  const mainRef = useRef<HTMLDivElement>(null);
  const catalogue = useErrorCatalogue();

  const ctxSession = useMemo(() => ({ session: { id: session, ...(runState !== undefined ? { state: runState } : {}) } }), [session, runState]);

  const refresh = useCallback(async () => {
    const [t, s] = await Promise.all([dispatch('settings.get', { session }), dispatch('run.status', { session })]);
    if (t.ok) {
      setTree((t.result as { value?: unknown }).value ?? null);
      setLoadError(null);
    } else {
      setLoadError({ message: t.error.message, code: t.error.code });
    }
    if (s.ok) {
      const st = (s.result as { state?: unknown }).state;
      setRunState(typeof st === 'string' ? st : undefined);
    }
  }, [session]);

  useEffect(() => {
    setTree(null);
    setErrors(new Map());
    setConflicts(new Map());
    setCheck(null);
    setLast(null);
    void refresh();
  }, [refresh]);

  // a change reported by the service (this surface's own, or another writer's: Claude Code, the CLI)
  useEffect(() => {
    if (settingsEpoch > 0 || stateEpoch > 0) void refresh();
  }, [settingsEpoch, stateEpoch, refresh]);
  useEffect(() => {
    if (settingsEpoch > 0) setChanges((n) => n + 1);
  }, [settingsEpoch]);

  const setAvail = availability('settings.set', { session }, ctxSession);
  const editable = setAvail.available;

  const mark = (path: string, on: boolean) =>
    setPending((p) => {
      const n = new Set(p);
      if (on) n.add(path);
      else n.delete(path);
      return n;
    });
  const putError = (path: string, message: string | null) =>
    setErrors((m) => {
      const n = new Map(m);
      if (message === null) n.delete(path);
      else n.set(path, message);
      return n;
    });

  const putConflict = (path: string, c: SettingConflict | null) =>
    setConflicts((m) => {
      const n = new Map(m);
      if (c === null) n.delete(path);
      else n.set(path, c);
      return n;
    });
  /** 1007 (ADR 0736 P2): another writer changed the value since this page read it; nothing was changed. */
  const conflicted = (error: { code: unknown; message: string; data?: unknown }, path: string, mine: JsonValue, remove: boolean): boolean => {
    if (error.code !== SETTINGS_CONFLICT) return false;
    const now = ((error.data as { current_value?: JsonValue } | undefined)?.current_value ?? null) as JsonValue;
    putConflict(path, { now, mine, remove });
    putError(path, null);
    setAnnounce(`${path} was not changed: another writer set it to ${formatValue(now) || 'empty'} since it was read.`);
    void refresh();
    return true;
  };
  // the value of the tree as this page read it; none when the tree does not hold the path (a new entry, an optional
  // object not made yet), so the engine's own default there is never taken for a change by another writer
  const treeRef = useRef<unknown>(null);
  treeRef.current = tree;
  const readBase = (path: string): JsonValue | undefined => valueAt(treeRef.current, path);

  const change = useCallback(
    async (path: string, value: JsonValue, base?: JsonValue): Promise<boolean> => {
      mark(path, true);
      const b = base === undefined ? readBase(path) : base;
      const r = await dispatch('settings.set', { session, path, value, ...(b !== undefined ? { base_value: b } : {}) }, { context: ctxSession });
      mark(path, false);
      if (!r.ok && conflicted(r.error, path, value, false)) return false;
      putConflict(path, null);
      if (!r.ok) {
        const msg = engineMessage(r.error, path, catalogue);
        putError(path, msg);
        setAnnounce(`${path}: change failed. ${msg}`);
        return false;
      }
      const stored = (r.result as { value?: JsonValue }).value;
      putError(path, null);
      setTree((t: unknown) => withValue(t, path, stored));
      setLast({ action: 'settings.set', params: r.entry.params, stored });
      setAnnounce(`${path} is now ${formatValue(stored) || 'empty'}.`);
      void refresh();
      return true;
    },
    [session, ctxSession, refresh, catalogue],
  );

  const ctx: SettingsContext = {
    session,
    ui: model.ui,
    tree,
    editable,
    readOnlyReason: editable ? null : setAvail.reason,
    errors,
    pending,
    prefill,
    isOpen: (id) => showAll || open.has(id),
    toggle: (id) => {
      if (showAll) {
        // "Show every key" is on: closing one disclosure turns it off and keeps every other one open
        setShowAll(false);
        setOpen(new Set(allDisclosures(model, tree).filter((d) => d !== id)));
        return;
      }
      setOpen((o) => {
        const n = new Set(o);
        if (n.has(id)) n.delete(id);
        else n.add(id);
        return n;
      });
    },
    change,
    conflicts,
    keepTheirs: (path) => putConflict(path, null),
    clearError: (path) => putError(path, null),
    addEntry: async (collectionPath, name) => {
      const key = `add:${collectionPath}`;
      const node = nodeAt(model, collectionPath);
      if (name.trim() === '') {
        putError(key, 'Type the name of the new entry first.');
        return false;
      }
      const entry = node?.kind === 'collection' ? node.entry : null;
      const seed = entry ? entrySeed(entry, `${collectionPath}.${name.trim()}`) : null;
      if (!seed) {
        putError(key, `The schema of ${collectionPath} has no key with a default to start an entry with; set one of its keys by path.`);
        return false;
      }
      mark(key, true);
      const made = `${collectionPath}.${name.trim()}`;
      // F-829: the entry is made by its first key, which had no value before; undo removes the entry it made
      const fresh = !entryNames(tree, collectionPath).includes(name.trim());
      const undo = fresh ? { inverse: [{ action: 'settings.remove', params: { session, path: made } }], title: `add ${made}` } : {};
      const r = await dispatch('settings.set', { session, path: seed.path, value: seed.value }, { context: ctxSession, undo });
      mark(key, false);
      if (!r.ok) {
        const msg = engineMessage(r.error, seed.path, catalogue);
        putError(key, msg);
        setAnnounce(`Adding ${name} failed. ${msg}`);
        return false;
      }
      putError(key, null);
      setOpen((o) => new Set([...o, `entry:${made}`, `group:${collectionPath}`]));
      setPrefill((m) => {
        const n = new Map(m);
        n.delete(collectionPath);
        return n;
      });
      setLast({ action: 'settings.set', params: r.entry.params, stored: (r.result as { value?: JsonValue }).value });
      setAnnounce(`${made} added.`);
      await refresh();
      return true;
    },
    removeEntry: async (path, base) => {
      mark(path, true);
      const b = base === undefined ? readBase(path) : base;
      const r = await dispatch('settings.remove', { session, path, ...(b !== undefined ? { base_value: b } : {}) }, { context: ctxSession });
      mark(path, false);
      if (!r.ok && conflicted(r.error, path, null, true)) return false;
      putConflict(path, null);
      if (!r.ok) {
        const msg = engineMessage(r.error, path, catalogue);
        putError(path, msg);
        setAnnounce(`Removing ${path} failed. ${msg}`);
        return false;
      }
      putError(path, null);
      setLast({ action: 'settings.remove', params: r.entry.params });
      setAnnounce(`${path} removed.`);
      await refresh();
      return true;
    },
    current: async (path) => {
      const r = await dispatch('settings.get', { session, path });
      if (r.ok) return { ok: true, value: ((r.result as { value?: JsonValue }).value ?? null) as JsonValue };
      // the engine answers -32602 for a path the case does not hold
      return r.error.code === -32602 ? { ok: true, value: null } : { ok: false };
    },
    goTo: (path) => {
      setQuery('');
      setSelected(sectionKey(sectionOf(path, list)));
      // every disclosure on the way: the prefixes of the path as groups, entries and "more keys"
      const parts = path.split('.');
      const ids: string[] = [];
      for (let i = 0; i < parts.length; i++) {
        const p = parts.slice(0, i + 1).join('.');
        const parent = parts.slice(0, i).join('.');
        ids.push(`group:${p}`, `entry:${p}`, `more:${parent}`);
      }
      setOpen((o) => new Set([...o, ...ids]));
      // a problem about an entry the case lacks: offer to add it by that name
      const parent = parts.slice(0, -1).join('.');
      const node = nodeAt(model, parent);
      if (node?.kind === 'collection' && !entryNames(tree, parent).includes(parts[parts.length - 1]!)) {
        setPrefill((m) => new Map(m).set(parent, parts[parts.length - 1]!));
      }
      setFocusTarget((f) => ({ path, n: (f?.n ?? 0) + 1 }));
    },
  };

  // after goTo: scroll to the key (or the nearest thing on its way) and put focus on its control
  useEffect(() => {
    if (!focusTarget) return;
    const id = requestAnimationFrame(() => {
      const root = mainRef.current;
      if (!root) return;
      const parts = focusTarget.path.split('.');
      for (let i = parts.length; i > 0; i--) {
        const p = parts.slice(0, i).join('.');
        const el =
          root.querySelector<HTMLElement>(`[data-path="${CSS.escape(p)}"]`) ??
          (i < parts.length ? root.querySelector<HTMLElement>(`[data-path="${CSS.escape(`add:${p}`)}"]`) : null);
        if (!el) continue;
        el.scrollIntoView({ block: 'center' });
        const f = el.matches('button') ? el : el.querySelector<HTMLElement>('input, button, [tabindex="0"]');
        f?.focus();
        return;
      }
    });
    return () => cancelAnimationFrame(id);
  }, [focusTarget]);

  const runCheck = async () => {
    setCheck('running');
    setCheckedAt(changes);
    const r = await dispatch('settings.validate', { session });
    if (!r.ok) {
      setCheck({ failed: engineMessage(r.error, '', catalogue) });
      return;
    }
    const res = r.result as { ok?: unknown; diagnostics?: unknown };
    const diagnostics = Array.isArray(res.diagnostics)
      ? (res.diagnostics as { path?: unknown; message?: unknown; line?: unknown }[]).map((d) => ({
          path: String(d.path ?? ''),
          message: String(d.message ?? ''),
          line: typeof d.line === 'number' ? d.line : null,
        }))
      : [];
    setCheck({ ok: res.ok === true, diagnostics });
    setAnnounce(res.ok === true ? 'The engine found no problems.' : `The engine found ${plural(diagnostics.length, 'problem', 'problems')}.`);
  };

  const hits = useMemo(() => (tree === null ? [] : search(model, tree, query)), [model, tree, query]);
  const setList = useMemo(() => (tree === null ? [] : setKeys(model, tree)), [model, tree]);

  const indexItems = useMemo(
    () => [
      { id: SET_ID, title: 'Set in this case', count: setList.length },
      ...list.map((s) => ({ id: sectionKey(s.id), title: s.title, count: tree === null ? 0 : setCount(s.node, tree, s.id), section: s })),
    ],
    [list, setList, tree],
  );

  const current = list.find((s) => sectionKey(s.id) === selected) ?? null;

  return (
    <Settings.Provider value={ctx}>
      <div className="st-body">
        <nav className="st-col" aria-label="Case sections">
          <div className="st-find">
            <TextField
              aria-label="Find a setting"
              placeholder="Find a setting"
              value={query}
              onChange={setQuery}
              onKeyDown={(e) => {
                if (e.key === 'Escape' && query !== '') {
                  e.preventDefault();
                  setQuery('');
                }
              }}
            />
            <Switch isSelected={showAll} onChange={setShowAll}>
              Show every key
            </Switch>
          </div>
          <ListBox
            aria-label="Sections of the case"
            className="st-index"
            selectionMode="single"
            selectionBehavior="replace"
            disallowEmptySelection
            selectedKeys={[selected]}
            onSelectionChange={(keys) => {
              if (keys === 'all') return;
              const k = [...keys][0] as Key | undefined;
              if (k !== undefined) {
                setSelected(String(k));
                setQuery('');
              }
            }}
            items={indexItems}
          >
            {(item) => (
              <ListBoxItem id={item.id} textValue={item.title} className={item.id === SET_ID ? 'st-index-item st-index-item--first' : 'st-index-item'}>
                <span className="st-index-name" data-words={item.id === SET_ID ? undefined : 'data'}>
                  {item.title}
                </span>
                <span className="st-index-count">
                  {item.count > 0 ? (
                    <>
                      <span data-words="data">{item.count}</span> set
                    </>
                  ) : (
                    ''
                  )}
                </span>
              </ListBoxItem>
            )}
          </ListBox>
        </nav>

        <main className="st-col" ref={mainRef} aria-labelledby="st-view-title">
          {!editable && setAvail.available === false && (
            <div className="st-banner" role="status" data-words="exception">
              {setAvail.reason} Values are shown read-only.
            </div>
          )}
          {loadError ? (
            <div className="st-empty" data-words="exception">
              <h2 id="st-view-title">The settings could not be read</h2>
              <p>The engine answered: {loadError.message}</p>
              {/* F-830: the catalogue's way out of its code (a session another writer closed, 1001) */}
              <EngineRecovery code={loadError.code} />
              <Button onPress={() => void refresh()}>Read again</Button>
            </div>
          ) : tree === null ? (
            <div className="st-empty">
              <h2 id="st-view-title">Reading the settings of {session}</h2>
            </div>
          ) : query.trim() !== '' ? (
            <SearchView hits={hits} query={query} />
          ) : current === null ? (
            <SetView model={model} tree={tree} rows={setList} />
          ) : (
            <SectionView section={current} />
          )}
        </main>

        <aside className="st-col st-check" aria-label="Check and changes">
          {aside}
          {/* G22: the session's normal facts are said to readers (the session is named in the header band) */}
          <section aria-labelledby="st-session-title" className="sr-only">
            <h2 id="st-session-title">
              Session <span data-words="data">{session}</span>
            </h2>
            <dl className="st-facts">
              <dt>Run state</dt>
              <dd data-words="data">{runState ?? 'unknown'}</dd>
              <dt>Keys set</dt>
              <dd>
                <span data-words="data">{setList.length}</span> of <span data-words="data">{model.leaves.length}</span> in the schema
              </dd>
              <dt>Service</dt>
              <dd data-words="data">{address ?? 'unknown address'}</dd>
            </dl>
          </section>
          <section aria-labelledby="st-check-title">
            <h2 id="st-check-title" className="sr-only">
              Check the case
            </h2>
            <Quiet id="st-check-what">The engine checks the settings against its rules and builds the mesh (settings.validate).</Quiet>
            <Button variant="primary" onPress={() => void runCheck()} isDisabled={check === 'running'} aria-describedby="st-check-what">
              {check === 'running' ? 'Checking' : 'Check the case'}
            </Button>
            <CheckView check={check} onGo={ctx.goTo} />
            {check !== null && check !== 'running' && changes > checkedAt && (
              <p className="st-small" data-words="exception">The settings changed after this check; check again for the engine's view of them now.</p>
            )}
          </section>
          {/* G22: before the first change this section says nothing on the screen */}
          <section aria-labelledby="st-last-title">
            <h2 id="st-last-title" className={last ? undefined : 'sr-only'}>
              Last change
            </h2>
            {last ? (
              <LastView last={last} address={address} />
            ) : (
              <Quiet>Changes made here are sent to the engine one key at a time; the command that repeats the last one appears here.</Quiet>
            )}
          </section>
        </aside>
      </div>
      <div className="sr-only" aria-live="polite">
        {announce}
      </div>
    </Settings.Provider>
  );
}

function SectionView({ section }: { section: Section }) {
  const { tree } = useSettings();
  const node = section.node;
  const n = setCount(node, tree, section.id);
  return (
    <>
      <header className="st-head">
        <h2 id="st-view-title" data-words="data">
          {section.title}
        </h2>
        <p className="st-head-counts sr-only">
          {node.kind === 'collection'
            ? `Named entries: ${plural(entryNames(tree, section.id).length, 'entry', 'entries')}, ${plural(keyCount(node.entry), 'key', 'keys')} each`
            : plural(keyCount(node), 'key', 'keys')}
          {n > 0 ? `, ${n} set` : ''}
        </p>
        {node.description && <Quiet>{node.description}</Quiet>}
      </header>
      {node.kind === 'collection' ? <CollectionBody collection={node} path={section.id} /> : <GroupBody group={node} path={section.id} />}
    </>
  );
}

function SearchView({ hits, query }: { hits: ReturnType<typeof search>; query: string }) {
  const c = useSettings();
  return (
    <>
      <header className="st-head">
        <h2 id="st-view-title">
          {plural(hits.length, 'key matches', 'keys match')} “{query.trim()}”
        </h2>
        <p>
          {hits.length > MAX_HITS ? `The first ${MAX_HITS} are shown; add a word to narrow the search. ` : ''}
          Search reads every key of the schema, at every step of disclosure.
        </p>
      </header>
      {hits.slice(0, MAX_HITS).map((h) =>
        h.path !== null ? (
          <KeyRow key={h.path} leaf={h.leaf} path={h.path} label={h.path} />
        ) : (
          <div className="st-pending-key" key={h.leaf.pattern} data-pattern={h.leaf.pattern} data-words="data">
            <strong>{h.leaf.pattern}</strong>
            <span>{h.collection} has no entry yet.</span>
            <Button onPress={() => c.goTo(h.collection ?? '')}>Go to {h.collection}</Button>
          </div>
        ),
      )}
    </>
  );
}

function SetView({ model, tree, rows }: { model: CaseModel; tree: unknown; rows: ReturnType<typeof setKeys> }) {
  const c = useSettings();
  const filled = filledCollections(model, tree);
  return (
    <>
      <header className="st-head">
        <h2 id="st-view-title">Set in this case</h2>
        <Quiet>The keys whose value differs from the schema's default, and the lists that hold values. Every other key has the engine's default.</Quiet>
      </header>
      {filled.map((f) => (
        <div className="st-filled" key={f.path} data-words="data">
          <strong>{f.path}</strong>
          <span>{f.entries.join(', ')}</span>
          <Button onPress={() => c.goTo(`${f.path}.${f.entries[0]}`)}>Go to {f.path}</Button>
        </div>
      ))}
      {rows.length === 0 && filled.length === 0 ? (
        <EmptyMark
          className="st-empty"
          text="Every key has its default. A key you change appears here, wherever it sits in the schema; pick a section on the left, or find a key by name."
        />
      ) : (
        rows.map((r) => <KeyRow key={r.path} leaf={r.leaf} path={r.path} label={r.path} />)
      )}
    </>
  );
}

function CheckView({ check, onGo }: { check: CheckResult | 'running' | { failed: string } | null; onGo: (p: string) => void }) {
  if (check === null || check === 'running') return null;
  if ('failed' in check) {
    return (
      <p className="st-verdict st-verdict--failed" role="status" data-words="exception">
        <StatePictogram state="failed" />
        The check failed to run. {check.failed}
      </p>
    );
  }
  if (check.ok) {
    return (
      <p className="st-verdict" role="status">
        <Icon name="check" />
        The engine found no problems.
      </p>
    );
  }
  return (
    <>
      <p className="st-verdict st-verdict--failed" role="status" data-words="exception">
        <StatePictogram state="failed" />
        Check failed: {plural(check.diagnostics.length, 'problem', 'problems')}
      </p>
      <ul className="st-diags" aria-label="Problems found by the engine" data-words="exception">
        {check.diagnostics.map((d, i) => (
          <li key={`${d.path}:${i}`}>
            <span>{d.message}</span>
            {d.path !== '' && (
              <Button variant="chip" onPress={() => onGo(d.path)} aria-label={`Go to ${d.path}`}>
                {d.path}
                {d.line !== null ? `, line ${d.line}` : ''}
              </Button>
            )}
          </li>
        ))}
      </ul>
    </>
  );
}

function LastView({ last, address }: { last: LastChange; address: string | null }) {
  const action = getAction(last.action)!;
  const eq = equivalents(action, last.params, address ?? undefined);
  const path = String(last.params['path'] ?? '');
  return (
    <>
      <p>
        {last.action === 'settings.remove' ? `Removed ${path}.` : `${path} is now ${formatValue(last.stored) || 'empty'} (the value the engine stored).`}
      </p>
      <p className="st-small">The same change from the command line:</p>
      <pre className="st-cli" data-words="data">
        {eq.cli.map(sq).join(' ')}
      </pre>
      {eq.mcp ? (
        <p className="st-small">From Claude Code: the MCP tool {eq.mcp.tool}.</p>
      ) : (
        <p className="st-small">{eq.mcpNote}</p>
      )}
    </>
  );
}

// ---- helpers

/** The schema node at a concrete path (entries matched by `*`), or null. */
function nodeAt(model: CaseModel, path: string): SettingNode | null {
  if (path === '') return model.root;
  let n: SettingNode = model.root;
  for (const part of path.split('.')) {
    if (n.kind === 'group') {
      const next: SettingNode | undefined = n.children.find((c) => c.name === part);
      if (!next) return null;
      n = next;
    } else if (n.kind === 'collection') {
      n = n.entry;
    } else {
      return null;
    }
  }
  return n;
}

/** Every disclosure id of the tree as it is (for turning "Show every key" off without closing anything). */
function allDisclosures(model: CaseModel, tree: unknown): string[] {
  const ids: string[] = [];
  const walk = (n: SettingNode, path: string) => {
    if (n.kind === 'group') {
      ids.push(`group:${path}`, `more:${path}`);
      for (const c of n.children) walk(c, path === '' ? c.name : `${path}.${c.name}`);
    } else if (n.kind === 'collection') {
      ids.push(`group:${path}`);
      for (const e of entryNames(tree, path)) {
        ids.push(`entry:${path}.${e}`);
        walk(n.entry, `${path}.${e}`);
      }
    }
  };
  walk(model.root, '');
  return ids;
}

