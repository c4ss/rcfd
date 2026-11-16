// The schema-driven case settings surface for one service session (F-825; G2, G8, G15, G16, G19; PRODUCT Principle 5).
// Every key of settings.schema gets a control made from the schema at run time (schema.ts), laid out by learner-first
// disclosure (disclosure.ts). Every read and change is an action of the F-827 registry dispatched through ../actions;
// what a change did, or why it was refused, is the engine's reply, shown as the engine wrote it.
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { ListBox, ListBoxItem, type Key } from 'react-aria-components';
import type { JsonObject, JsonValue } from '../../shared/actions/params.js';
import { availability, dispatch } from '../actions/index.js';
import { equivalents, getAction } from '../../shared/actions/registry.js';
import { Button, Icon, StatePictogram, Switch, TextField } from '../ui/index.js';
import { Settings, useSettings, type SettingsContext } from './context.js';
import { filledCollections, keyCount, search, sectionOf, sections, setCount, setKeys, type Section } from './disclosure.js';
import { CollectionBody, GroupBody } from './Groups.js';
import { KeyRow } from './KeyRow.js';
import { entryNames, entrySeed, formatValue, type CaseModel, type SettingNode } from './schema.js';

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

/** The engine's words for a refused call: the diagnostics of 1003 for this path (or all of them), else its message. */
function engineMessage(error: { code: unknown; message: string; data?: unknown }, path: string): string {
  const data = error.data as { diagnostics?: unknown } | undefined;
  const diags = Array.isArray(data?.diagnostics) ? (data.diagnostics as { path?: unknown; message?: unknown }[]) : [];
  const mine = diags.filter((d) => d.path === path);
  const pick = (mine.length ? mine : diags).map((d) => String(d.message ?? '')).filter((m) => m !== '');
  return pick.length ? pick.join('; ') : error.message;
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
  const [loadError, setLoadError] = useState<string | null>(null);
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
  const [focusTarget, setFocusTarget] = useState<{ path: string; n: number } | null>(null);
  const mainRef = useRef<HTMLDivElement>(null);

  const ctxSession = useMemo(() => ({ session: { id: session, ...(runState !== undefined ? { state: runState } : {}) } }), [session, runState]);

  const refresh = useCallback(async () => {
    const [t, s] = await Promise.all([dispatch('settings.get', { session }), dispatch('run.status', { session })]);
    if (t.ok) {
      setTree((t.result as { value?: unknown }).value ?? null);
      setLoadError(null);
    } else {
      setLoadError(t.error.message);
    }
    if (s.ok) {
      const st = (s.result as { state?: unknown }).state;
      setRunState(typeof st === 'string' ? st : undefined);
    }
  }, [session]);

  useEffect(() => {
    setTree(null);
    setErrors(new Map());
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

  const change = useCallback(
    async (path: string, value: JsonValue): Promise<boolean> => {
      mark(path, true);
      const r = await dispatch('settings.set', { session, path, value }, { context: ctxSession });
      mark(path, false);
      if (!r.ok) {
        const msg = engineMessage(r.error, path);
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
    [session, ctxSession, refresh],
  );

  const ctx: SettingsContext = {
    session,
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
        const msg = engineMessage(r.error, seed.path);
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
    removeEntry: async (path) => {
      mark(path, true);
      const r = await dispatch('settings.remove', { session, path }, { context: ctxSession });
      mark(path, false);
      if (!r.ok) {
        const msg = engineMessage(r.error, path);
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
      setCheck({ failed: engineMessage(r.error, '') });
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
              placeholder="Find a setting by name or description"
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
                <span className="st-index-name">{item.title}</span>
                <span className="st-index-count">{item.count > 0 ? `${item.count} set` : ''}</span>
              </ListBoxItem>
            )}
          </ListBox>
        </nav>

        <main className="st-col" ref={mainRef} aria-labelledby="st-view-title">
          {!editable && setAvail.available === false && <div className="st-banner" role="status">{setAvail.reason} Values are shown read-only.</div>}
          {loadError ? (
            <div className="st-empty">
              <h2 id="st-view-title">The settings could not be read</h2>
              <p>The engine answered: {loadError}</p>
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
          <section aria-labelledby="st-session-title">
            <h2 id="st-session-title">Session {session}</h2>
            <dl className="st-facts">
              <dt>Run state</dt>
              <dd>{runState ?? 'unknown'}</dd>
              <dt>Keys set</dt>
              <dd>
                {setList.length} of {model.leaves.length} in the schema
              </dd>
              <dt>Service</dt>
              <dd>{address ?? 'unknown address'}</dd>
            </dl>
          </section>
          <section aria-labelledby="st-check-title">
            <h2 id="st-check-title">Check the case</h2>
            <p className="st-small">The engine checks the settings against its rules and builds the mesh (settings.validate).</p>
            <Button variant="primary" onPress={() => void runCheck()} isDisabled={check === 'running'}>
              {check === 'running' ? 'Checking' : 'Check the case'}
            </Button>
            <CheckView check={check} onGo={ctx.goTo} />
            {check !== null && check !== 'running' && changes > checkedAt && (
              <p className="st-small">The settings changed after this check; check again for the engine's view of them now.</p>
            )}
          </section>
          <section aria-labelledby="st-last-title">
            <h2 id="st-last-title">Last change</h2>
            {last ? <LastView last={last} address={address} /> : <p className="st-small">Changes made here are sent to the engine one key at a time; the command that repeats the last one appears here.</p>}
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
        <h2 id="st-view-title">{section.title}</h2>
        <p className="st-head-counts">
          {node.kind === 'collection'
            ? `Named entries: ${plural(entryNames(tree, section.id).length, 'entry', 'entries')}, ${plural(keyCount(node.entry), 'key', 'keys')} each`
            : plural(keyCount(node), 'key', 'keys')}
          {n > 0 ? `, ${n} set` : ''}
        </p>
        {node.description && <p>{node.description}</p>}
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
          <div className="st-pending-key" key={h.leaf.pattern} data-pattern={h.leaf.pattern}>
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
        <p>
          The keys whose value differs from the schema's default, and the lists that hold values. Every other key has the
          engine's default.
        </p>
      </header>
      {filled.map((f) => (
        <div className="st-filled" key={f.path}>
          <strong>{f.path}</strong>
          <span>{f.entries.join(', ')}</span>
          <Button onPress={() => c.goTo(`${f.path}.${f.entries[0]}`)}>Go to {f.path}</Button>
        </div>
      ))}
      {rows.length === 0 && filled.length === 0 ? (
        <div className="st-empty">
          <p>
            Every key has its default. A key you change appears here, wherever it sits in the schema; pick a section on the
            left, or find a key by name.
          </p>
        </div>
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
      <p className="st-verdict st-verdict--failed" role="status">
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
      <p className="st-verdict st-verdict--failed" role="status">
        <StatePictogram state="failed" />
        Check failed: {plural(check.diagnostics.length, 'problem', 'problems')}
      </p>
      <ul className="st-diags" aria-label="Problems found by the engine">
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
      <pre className="st-cli">{eq.cli.map(sq).join(' ')}</pre>
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

