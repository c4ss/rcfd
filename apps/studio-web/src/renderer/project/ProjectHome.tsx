// The project home (F-805; surface brief .impeccable/surfaces/web-src-renderer-project-projecthome-tsx-41df8cd1.md):
// the window with no project open (open a folder, the recent list) and the home of an open project folder: one ruled
// index of what the folder holds, as the engine answers it, and a lane of every change the engine's feed reports.
//
// The renderer keeps no project state (G12): everything shown comes in the state the main process pushes on the
// project port (window.studioProject through ./door.ts, src/shared/project.ts), and the page only filters it for display.
import { useEffect, useMemo, useState, type ReactNode } from 'react';
import type {
  CaseEntry,
  ChangeEntry,
  Listing,
  MeshEntry,
  NoteEntry,
  OpenFailure,
  ProjectState,
  ProjectView,
  RecentProject,
  RunEntry,
} from '../../shared/project.js';
import { Button, IconButton, SegmentedToggle, StateMark, StatePictogram, TextField, Unavailable } from '../ui/index.js';
import { baseName, clock, dateTime, inFolder, kilobytes, plain, plural, runPictogram, runWord, sourceWord } from './format.js';
import { projectActions, watchProjectState } from './door.js';
import './project.css';

let reported = false;

/** Tells the main process, once, that the view is in the document and one animation frame has passed (P10). */
function reportReady(): void {
  if (reported) return;
  reported = true;
  requestAnimationFrame(() => {
    void window.studio.app.ready();
  });
}

function useProjectState(): ProjectState | null {
  const [state, setState] = useState<ProjectState | null>(null);
  useEffect(() => watchProjectState(setState), []);
  return state;
}

const actions = projectActions;

const NOT_CONFIGURED_SHORT = 'The Studio was not told where the engine’s project server is.';

export function ProjectHome() {
  useEffect(reportReady, []);
  const state = useProjectState();
  const project = state?.project ?? null;
  const canOpen = state === null || state.engine.configured;
  return (
    <div className="ph-page">
      <header className="ph-top">
        <div className="ph-brand">
          rcfd <span>Studio</span>
        </div>
        {project && (
          <div className="ph-ptab" title={project.folder}>
            {project.info?.name ?? baseName(project.folder)}
          </div>
        )}
        {project && (
          <div className="ph-top-actions">
            {canOpen ? (
              <Button variant="ghost" icon="plus" onPress={actions.pick} isDisabled={state?.opening != null}>
                Open project…
              </Button>
            ) : (
              <Unavailable label="Open project…" reason={NOT_CONFIGURED_SHORT} />
            )}
            <Button variant="ghost" onPress={() => void (location.hash = '#/settings')}>
              Case settings
            </Button>
          </div>
        )}
        {project && (
          <div className="ph-top-right">
            <LiveWord view={project} />
            <Button variant="ghost" icon="close" onPress={actions.close}>
              Close project
            </Button>
          </div>
        )}
      </header>
      <main className="ph-main">{project ? <OpenProject view={project} state={state!} /> : <NoProject state={state} />}</main>
      <footer className="ph-foot">
        <span>Early-stage software. Not validated for engineering decisions or for safety-critical use. No warranty.</span>
        <span className="ph-foot-right">{project ? 'Read through the engine, read-only' : 'rcfd Studio'}</span>
      </footer>
    </div>
  );
}

function LiveWord({ view }: { view: ProjectView }) {
  return view.liveness.live ? (
    <span className="ph-live" role="status">
      <span className="ph-live-mark" aria-hidden="true" />
      Live
    </span>
  ) : (
    <span className="ph-live ph-live--off" role="status">
      <StatePictogram state="failed" />
      Not live
    </span>
  );
}

// ---- no project open

function NoProject({ state }: { state: ProjectState | null }) {
  const engine = state?.engine;
  return (
    <div className="ph-empty">
      <section className="ph-empty-main" aria-labelledby="ph-title">
        <h1 id="ph-title">No project open</h1>
        <p>
          A project is a folder that holds an <span className="ph-file">rcfd-project.json</span> and the folders{' '}
          <span className="ph-file">cases</span>, <span className="ph-file">meshes</span>, <span className="ph-file">runs</span> and{' '}
          <span className="ph-file">notes</span>. The Studio reads it through the engine, changes nothing in it, and shows what Claude Code, the CLI
          or an editor change there as it happens.
        </p>
        <div className="ph-empty-actions">
          {engine === undefined || engine.configured ? (
            <Button variant="primary" icon="plus" onPress={actions.pick} isDisabled={state?.opening != null}>
              Open project folder…
            </Button>
          ) : (
            <Unavailable label="Open project folder…" reason={NOT_CONFIGURED_SHORT} />
          )}
        </div>
        {engine?.configured && <p className="ph-note">Projects are read by {engine.path}, without writing.</p>}
        {engine && !engine.configured && (
          <Notice title="Projects cannot be opened yet">
            <p>{engine.reason}</p>
          </Notice>
        )}
        {state?.opening && (
          <p className="ph-opening" role="status">
            Opening {state.opening}…
          </p>
        )}
        {state?.failure && <FailureNotice failure={state.failure} recent={state.recent} />}
      </section>
      <section className="ph-recent" aria-labelledby="ph-recent-title">
        <h2 id="ph-recent-title" className="ph-section-title">
          Recent projects
        </h2>
        {!state || state.recent.length === 0 ? (
          <p className="ph-empty-line">No recent projects. The folders you open appear here, the latest first.</p>
        ) : (
          <ul className="ph-recent-list">
            {state.recent.map((r) => (
              <RecentRow key={r.path} r={r} canOpen={state.engine.configured && state.opening === null} />
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function RecentRow({ r, canOpen }: { r: RecentProject; canOpen: boolean }) {
  const name = baseName(r.path);
  return (
    <li className={`ph-recent-row${r.exists ? '' : ' ph-recent-row--gone'}`}>
      <div className="ph-recent-text">
        <span className="ph-recent-name">{name}</span>
        <span className="ph-recent-path">{r.path}</span>
      </div>
      <span className="ph-recent-when">{r.exists ? `opened ${dateTime(r.opened)}` : <StateMark state="failed" label="Not found" />}</span>
      <div className="ph-recent-actions">
        {r.exists && canOpen ? (
          <Button onPress={() => actions.open(r.path)} aria-label={`Open ${name}`}>
            Open
          </Button>
        ) : (
          <Unavailable
            label="Open"
            reason={
              r.exists
                ? 'Projects cannot be opened yet: the Studio was not told where the engine’s project server is.'
                : 'The folder is not there any more: it may have been moved, renamed or deleted.'
            }
          />
        )}
        <IconButton variant="ghost" icon="close" label={`Forget ${name}`} onPress={() => actions.forget(r.path)} />
      </div>
    </li>
  );
}

function Notice({ title, children, failed }: { title: string; children: ReactNode; failed?: boolean }) {
  return (
    <div className={`ph-notice${failed ? ' ph-notice--failed' : ''}`} role={failed ? 'alert' : 'note'}>
      <p className="ph-notice-title">
        {failed && <StatePictogram state="failed" />}
        {title}
      </p>
      {children}
    </div>
  );
}

const FAILURE_TITLE: Record<OpenFailure['kind'], string> = {
  not_configured: 'Failed to open: no project server',
  not_found: 'Failed to open: folder not found',
  not_a_project: 'Failed to open: not an rcfd project',
  engine: 'Failed to open: the engine could not read the folder',
};

function FailureNotice({ failure, recent }: { failure: OpenFailure; recent: readonly RecentProject[] }) {
  const inRecent = recent.some((r) => r.path === failure.folder);
  return (
    <Notice title={FAILURE_TITLE[failure.kind]} failed>
      <p>{failure.message}</p>
      {failure.detail && (
        <p className="ph-detail">
          <span className="ph-detail-caption">The engine said</span>
          {failure.detail}
        </p>
      )}
      <div className="ph-notice-actions">
        {failure.kind !== 'not_configured' && <Button onPress={actions.pick}>Choose another folder…</Button>}
        {failure.kind === 'not_found' && inRecent && <Button onPress={() => actions.forget(failure.folder)}>Forget this folder</Button>}
        <Button variant="ghost" onPress={actions.dismiss}>
          Dismiss
        </Button>
      </div>
    </Notice>
  );
}

// ---- an open project

type Kind = 'all' | 'cases' | 'meshes' | 'runs' | 'notes';

const DESCRIBE: Record<Exclude<Kind, 'all'>, string> = {
  cases: 'settings files the solver reads',
  meshes: 'mesh files and recipes the cases name',
  runs: 'one folder per run, kept by the runner',
  notes: 'free text',
};

function OpenProject({ view, state }: { view: ProjectView; state: ProjectState }) {
  const [kind, setKind] = useState<Kind>('all');
  const [filter, setFilter] = useState('');
  const info = view.info;
  const changed = useMemo(() => changedAt(view.changes), [view.changes]);
  const q = filter.trim().toLowerCase();
  const match = (...texts: string[]) => q === '' || texts.some((t) => t.toLowerCase().includes(q));
  const cases = view.cases.items.filter((c) => match(c.path));
  const meshes = view.meshes.items.filter((m) => match(m.path, ...m.used_by));
  const runs = view.runs.items.filter((r) => match(r.id, r.case, r.state));
  const notes = view.notes.items.filter((n) => match(n.path));
  const shown = (k: Exclude<Kind, 'all'>) => kind === 'all' || kind === k;
  const nothing =
    q !== '' &&
    (!shown('cases') || cases.length === 0) &&
    (!shown('meshes') || meshes.length === 0) &&
    (!shown('runs') || runs.length === 0) &&
    (!shown('notes') || notes.length === 0);
  const total = view.cases.total + view.meshes.total + view.runs.total + view.notes.total;
  return (
    <div className="ph-open">
      <section className="ph-strip" aria-labelledby="ph-name">
        <h1 id="ph-name" className="ph-name">
          {info?.name ?? baseName(view.folder)}
        </h1>
        <dl className="ph-facts">
          <Fact caption="Folder">{view.folder}</Fact>
          {info && <Fact caption="Created">{dateTime(info.created)}</Fact>}
          {info && <Fact caption="Created with">{info.solver_version_created}</Fact>}
          {info && (
            <Fact caption="Folder format">
              {info.schema_version}
              {info.migrated_from !== null ? ` (read from ${info.migrated_from})` : ''}
            </Fact>
          )}
          {info && <Fact caption="Issues">{info.issues_total === 0 ? 'none' : info.issues_total}</Fact>}
          <Fact caption="Read by">
            {view.engine.name} {view.engine.version}, read-only
          </Fact>
        </dl>
      </section>
      {view.infoError && (
        <Notice title="Failed to read the project's description" failed>
          <p>{view.infoError}</p>
        </Notice>
      )}
      {state.failure && <FailureNotice failure={state.failure} recent={state.recent} />}
      {!view.liveness.live && <NotLive view={view} />}
      <div className="ph-tools">
        <TextField
          aria-label="Filter by name"
          placeholder="Filter by name, run id or state"
          value={filter}
          onChange={setFilter}
          className="ph-filter"
        />
        <SegmentedToggle
          aria-label="Show"
          options={[
            { id: 'all', label: `All ${total}` },
            { id: 'cases', label: `Cases ${view.cases.total}` },
            { id: 'meshes', label: `Meshes ${view.meshes.total}` },
            { id: 'runs', label: `Runs ${view.runs.total}` },
            { id: 'notes', label: `Notes ${view.notes.total}` },
          ]}
          selected={kind}
          onChange={(k) => setKind(k as Kind)}
        />
        <span className="ph-read">{state.opening ? `Opening ${state.opening}…` : `Read ${clock(view.readAt)}`}</span>
        {view.liveness.live && <Button onPress={actions.refresh}>Read again</Button>}
      </div>
      <div className="ph-work">
        <div className="ph-index" role="region" aria-label="What the project folder holds">
          {info && info.issues_total > 0 && <Issues info={info} />}
          {nothing && (
            <div className="ph-nomatch">
              <p>No entry matches “{filter.trim()}”.</p>
              <Button variant="ghost" onPress={() => setFilter('')}>
                Clear the filter
              </Button>
            </div>
          )}
          {shown('cases') && (
            <Group id="cases" title="Cases" listing={view.cases} engineCount={info?.counts.cases} shown={cases.length} filtered={q !== ''}>
              {cases.length > 0 && <CasesTable cases={cases} changed={changed} />}
              {view.cases.total === 0 && !view.cases.error && (
                <Empty>
                  No cases yet. A case is a settings file (<span className="ph-file">.toml</span> or <span className="ph-file">.json</span>) in{' '}
                  <span className="ph-file">cases/</span>. Ask Claude Code to make one (its <span className="ph-file">new_case</span> tool), or copy a
                  case file into the folder; it appears here.
                </Empty>
              )}
            </Group>
          )}
          {shown('meshes') && (
            <Group id="meshes" title="Meshes" listing={view.meshes} engineCount={info?.counts.meshes} shown={meshes.length} filtered={q !== ''}>
              {meshes.length > 0 && <MeshesTable meshes={meshes} changed={changed} />}
              {view.meshes.total === 0 && !view.meshes.error && (
                <Empty>
                  No mesh files. A case can build its own mesh from its settings, so this is often fine. To use a mesh file (Gmsh, Fluent or
                  OpenFOAM), ask Claude Code to bring it in with <span className="ph-file">import_mesh</span>.
                </Empty>
              )}
            </Group>
          )}
          {shown('runs') && (
            <Group id="runs" title="Runs" listing={view.runs} engineCount={info?.counts.runs} shown={runs.length} filtered={q !== ''}>
              {runs.length > 0 && <RunsTable runs={runs} changed={changed} />}
              {view.runs.total === 0 && !view.runs.error && (
                <Empty>
                  No runs yet. A run solves one case and keeps its results in <span className="ph-file">runs/</span>. The Studio cannot start runs
                  yet; Claude Code can (<span className="ph-file">start_run</span>), and its runs appear here as they start.
                </Empty>
              )}
              {view.skippedRuns.length > 0 && <SkippedRuns skipped={view.skippedRuns} />}
            </Group>
          )}
          {shown('notes') && (
            <Group id="notes" title="Notes" listing={view.notes} engineCount={info?.counts.notes} shown={notes.length} filtered={q !== ''}>
              {notes.length > 0 && <NotesTable notes={notes} changed={changed} />}
              {view.notes.total === 0 && !view.notes.error && (
                <Empty>
                  No notes. Notes are free text in <span className="ph-file">notes/</span>, written by you or by Claude Code (
                  <span className="ph-file">write_note</span>).
                </Empty>
              )}
            </Group>
          )}
        </div>
        <ChangesLane view={view} />
      </div>
    </div>
  );
}

function Fact({ caption, children }: { caption: string; children: ReactNode }) {
  return (
    <div className="ph-fact">
      <dt>{caption}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function NotLive({ view }: { view: ProjectView }) {
  const l = view.liveness;
  if (l.live) return null;
  return (
    <Notice title={l.serverStopped ? 'Live link failed: the project server stopped' : 'Live link failed: the change feed cannot be read'} failed>
      <p>
        {l.reason} What is shown is the folder as read at {clock(view.readAt)}; edits made since then do not appear until the link is back.
      </p>
      <div className="ph-notice-actions">
        <Button onPress={actions.refresh}>{l.serverStopped ? 'Reconnect' : 'Read again'}</Button>
      </div>
    </Notice>
  );
}

function Issues({ info }: { info: NonNullable<ProjectView['info']> }) {
  return (
    <section className="ph-group" aria-labelledby="ph-g-issues">
      <h2 id="ph-g-issues" className="ph-group-head">
        <span className="ph-group-title">Issues</span>
        <span className="ph-group-count">{info.issues_total}</span>
        <span className="ph-group-what">what the engine found wrong in the folder</span>
      </h2>
      <ul className="ph-issues">
        {info.issues.map((i) => (
          <li key={`${i.path}:${i.message}`}>
            <span className="ph-issue-word">Issue</span>
            <span className="ph-issue-path">{i.path}</span>
            <span>{i.message}</span>
          </li>
        ))}
        {info.issues_total > info.issues.length && <li className="ph-more">and {info.issues_total - info.issues.length} more</li>}
      </ul>
    </section>
  );
}

function Group<T>({
  id,
  title,
  listing,
  engineCount,
  shown,
  filtered,
  children,
}: {
  id: Exclude<Kind, 'all'>;
  title: string;
  listing: Listing<T>;
  engineCount: number | undefined;
  shown: number;
  filtered: boolean;
  children: ReactNode;
}) {
  const disagree = engineCount !== undefined && listing.error === null && engineCount !== listing.total;
  return (
    <section className="ph-group" aria-labelledby={`ph-g-${id}`} data-group={id}>
      <h2 id={`ph-g-${id}`} className="ph-group-head">
        <span className="ph-group-title">{title}</span>
        <span className="ph-group-count">{listing.total}</span>
        <span className="ph-group-what">
          <span className="ph-file">{id}/</span> {DESCRIBE[id]}
          {filtered && listing.total > 0 ? `; ${shown} shown` : ''}
          {disagree ? `; the engine counts ${engineCount}` : ''}
        </span>
      </h2>
      {listing.error !== null ? (
        <div className="ph-group-error" role="alert">
          <StateMark state="failed" label={`Failed to list ${title.toLowerCase()}`} />
          <span>{listing.error}</span>
          <Button onPress={actions.refresh}>Read again</Button>
        </div>
      ) : (
        children
      )}
      {listing.error === null && listing.total > listing.items.length && (
        <p className="ph-more">
          The first {listing.items.length} of {listing.total} are listed.
        </p>
      )}
    </section>
  );
}

function Empty({ children }: { children: ReactNode }) {
  return <p className="ph-empty-line ph-empty-line--group">{children}</p>;
}

function Changed({ at }: { at: string | undefined }) {
  if (!at) return null;
  return (
    <span className="ph-changed">
      <span className="ph-changed-word">changed</span> {clock(at)}
    </span>
  );
}

function CasesTable({ cases, changed }: { cases: readonly CaseEntry[]; changed: Map<string, string> }) {
  return (
    <table className="ph-table">
      <thead>
        <tr>
          <th scope="col">Case file</th>
          <th scope="col">Mesh file</th>
          <th scope="col" className="ph-num">
            Runs
          </th>
          <th scope="col">
            <span className="sr-only">Last change</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {cases.map((c) => (
          <tr key={c.path} data-path={c.path}>
            <th scope="row" className="ph-name-cell">
              {inFolder(c.path, 'cases')}
              {c.unread && (
                <span className="ph-unread">
                  <StateMark state="failed" label="failed to read" />
                  <span>{c.unread}</span>
                </span>
              )}
            </th>
            <td>{c.meshes.length > 0 ? c.meshes.map((m) => inFolder(m, 'meshes')).join(', ') : <span className="ph-quiet">none in meshes/</span>}</td>
            <td className="ph-num">{c.runs}</td>
            <td className="ph-change-cell">
              <Changed at={changed.get(c.path)} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function MeshesTable({ meshes, changed }: { meshes: readonly MeshEntry[]; changed: Map<string, string> }) {
  return (
    <table className="ph-table">
      <thead>
        <tr>
          <th scope="col">Mesh</th>
          <th scope="col">Kind</th>
          <th scope="col" className="ph-num">
            Size, kB
          </th>
          <th scope="col">Used by</th>
          <th scope="col">
            <span className="sr-only">Last change</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {meshes.map((m) => (
          <tr key={m.path} data-path={m.path}>
            <th scope="row" className="ph-name-cell">
              {inFolder(m.path, 'meshes')}
            </th>
            <td>{m.kind === 'openfoam' ? `OpenFOAM folder, ${plural(m.files, 'file')}` : m.files > 1 ? plural(m.files, 'file') : 'file'}</td>
            <td className="ph-num">{kilobytes(m.bytes)}</td>
            <td>
              {m.used_by.length > 0 ? m.used_by.map((c) => inFolder(c, 'cases')).join(', ') : <span className="ph-quiet">no case names it</span>}
            </td>
            <td className="ph-change-cell">
              <Changed at={changed.get(m.path)} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function RunsTable({ runs, changed }: { runs: readonly RunEntry[]; changed: Map<string, string> }) {
  return (
    <table className="ph-table ph-table--runs">
      <thead>
        <tr>
          <th scope="col">Case and run</th>
          <th scope="col">State</th>
          <th scope="col" className="ph-num">
            Step
          </th>
          <th scope="col" className="ph-num">
            Time, s
          </th>
          <th scope="col">Created</th>
          <th scope="col">
            <span className="sr-only">Last change</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {runs.map((r) => {
          const picto = runPictogram(r.state);
          return (
            <tr key={r.id} data-run={r.id} className={r.state === 'failed' ? 'ph-row--failed' : undefined}>
              <th scope="row" className="ph-name-cell">
                <span className="ph-run-case">{inFolder(r.case.replace(/\\/g, '/'), 'cases') || 'case not recorded'}</span>
                <span className="ph-run-id">{r.id}</span>
              </th>
              <td>
                {picto ? <StateMark state={picto} label={runWord(r.state)} /> : <span className="ui-state ph-state-word">{runWord(r.state)}</span>}
              </td>
              <td className="ph-num">{r.step}</td>
              <td className="ph-num">{plain(r.time)}</td>
              <td>{dateTime(r.created)}</td>
              <td className="ph-change-cell">
                <Changed at={changed.get(`run:${r.id}`)} />
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function NotesTable({ notes, changed }: { notes: readonly NoteEntry[]; changed: Map<string, string> }) {
  return (
    <table className="ph-table">
      <thead>
        <tr>
          <th scope="col">Note</th>
          <th scope="col">
            <span className="sr-only">Last change</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {notes.map((n) => (
          <tr key={n.path} data-path={n.path}>
            <th scope="row" className="ph-name-cell">
              {inFolder(n.path, 'notes')}
            </th>
            <td className="ph-change-cell">
              <Changed at={changed.get(n.path)} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function SkippedRuns({ skipped }: { skipped: ProjectView['skippedRuns'] }) {
  return (
    <ul className="ph-issues ph-issues--failed">
      {skipped.map((s) => (
        <li key={s.path}>
          <StateMark state="failed" label="failed to read" />
          <span className="ph-issue-path">{s.path}</span>
          <span>{s.message}</span>
        </li>
      ))}
    </ul>
  );
}

/** The latest change of each path (`cases/a.toml`) and run (`run:<id>`), from the feed entries (newest first). */
function changedAt(changes: readonly ChangeEntry[]): Map<string, string> {
  const m = new Map<string, string>();
  for (const c of changes) {
    const at = c.seen;
    if (c.path && !m.has(c.path)) m.set(c.path, at);
    if (c.run && !m.has(`run:${c.run}`)) m.set(`run:${c.run}`, at);
  }
  return m;
}

function ChangesLane({ view }: { view: ProjectView }) {
  return (
    <aside className="ph-lane" aria-labelledby="ph-lane-title">
      <div className="ph-lane-head">
        <h2 id="ph-lane-title" className="ph-lane-title">
          Changes
        </h2>
        <span className="ph-lane-since">since {clock(view.openedAt)}</span>
      </div>
      {view.changes.length === 0 ? (
        <p className="ph-empty-line ph-lane-empty">
          Nothing has changed since the project was opened. What Claude Code, the CLI, the runner or an editor changes in the folder appears here as
          the engine reports it.
        </p>
      ) : (
        <ol className="ph-changes" aria-live="polite" aria-relevant="additions">
          {view.changes.map((c) => (
            <li key={c.n} className="ph-change" data-source={c.source}>
              <span className="ph-change-head">
                <time dateTime={c.seen} title={c.t && c.t !== c.seen ? `written ${dateTime(c.t)}` : undefined}>
                  {clock(c.seen)}
                </time>
                <span className="ph-change-source">{sourceWord(c.source)}</span>
              </span>
              <span className="ph-change-text">{c.text}</span>
            </li>
          ))}
        </ol>
      )}
    </aside>
  );
}
