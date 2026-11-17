// The project home (F-805; surface brief .impeccable/surfaces/web-src-renderer-project-projecthome-tsx-41df8cd1.md):
// the window with no project open (open a folder, the recent list) and the home of an open project folder: one ruled
// index of what the folder holds, as the engine answers it, and a lane of every change the engine's feed reports.
//
// The renderer keeps no project state (G12): everything shown comes in the state the main process pushes on the
// project port (window.studioProject through ./door.ts, src/shared/project.ts), and the page only filters it for display.
import { lazy, Suspense, useEffect, useMemo, useState, type ReactNode } from 'react';
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
  RunActionResult,
} from '../../shared/project.js';
import { Button, Dash, EmptyMark, IconButton, Quiet, SegmentedToggle, StateMark, StatePictogram, Tabs, TextField, Unavailable } from '../ui/index.js';
import { baseName, clock, dateTime, inFolder, kilobytes, plural, sourceWord } from './format.js';
import { RunsView } from '../runs/RunsView.js';
import { runRows, shownState, timeIsQuiet, timeNote, timeText, unknownWord, type RunRow } from '../runs/model.js';
import { cancelOpening, projectActions, watchProjectState } from './door.js';
import { useWorkWhile, WorkProgress } from '../work/WorkProgress.js';
import { caseHash } from '../case/hash.js';
import { CaseRunButton, RunFailure } from '../runs/RunControls.js';
import { ClaudePanel } from '../claude/ClaudePanel.js';
import { requestCommand, useCommandFact, useCommandHandlers } from '../commands/host.js';
import { FirstPath, FirstPathIntro, NotAProjectYet } from '../first/FirstPath.js';
import { DataFiles, DesignsGroup, UnsavedMark, isCaseFile, useUnsavedCases } from './HomeExtras.js';
import './project.css';

// F-810: the run workspace, loaded on demand (it brings three.js), so the project home costs nothing more at start
const RunWorkspace = lazy(() => import('../workspace/RunWorkspace.js'));

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

/** F-832: the scope of the project home's works (opening a project) in the work tracker. */
const WORK_SCOPE = 'project-home';

const NOT_CONFIGURED_SHORT = 'The Studio was not told where the engine’s project server is.';

export function ProjectHome() {
  useEffect(reportReady, []);
  const state = useProjectState();
  const project = state?.project ?? null;
  // F-832 (G21): an opening shows its progress and Cancel after PROGRESS_AFTER_MS; Cancel keeps what was open before
  useWorkWhile(
    state?.opening != null,
    { kind: 'project.open', scope: WORK_SCOPE, label: `Opening project ${state?.opening ?? ''}`, word: 'Opening project', onCancel: cancelOpening },
    state?.opening ?? '',
  );
  const canOpen = state === null || state.engine.configured;
  return (
    <div className="ph-page">
      <header className="ph-top">
        <div className="ph-brand">
          rcfd <span>Studio</span>
        </div>
        {project && (
          <div className="ph-ptab" title={project.folder} data-words="data">
            {project.info?.name ?? baseName(project.folder)}
          </div>
        )}
        {project && (
          <div className="ph-top-actions">
            <WorkProgress scope={WORK_SCOPE} placement="band" />
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
            <Button variant="ghost" icon="plus" onPress={() => void (location.hash = caseHash.new)}>
              New case
            </Button>
          </div>
        )}
        {project && (
          <div className="ph-top-right">
            <LiveWord view={project} />
            <IconButton variant="ghost" icon="close" label="Close project" onPress={actions.close} />
          </div>
        )}
        {/* F-831: the command palette, also from the pointer */}
        <div className="ph-top-commands">
          {/* G20, G22 (approved 2026-11-17): the shortcut is the button's description, not part of its name or text */}
          <Button variant="ghost" onPress={() => requestCommand('palette.open')} aria-keyshortcuts="Control+K" aria-describedby="ph-commands-keys">
            Commands
          </Button>
          <span className="ph-top-key sr-only" id="ph-commands-keys">
            Ctrl+K
          </span>
        </div>
      </header>
      <main className="ph-main">{project ? <OpenProject view={project} state={state!} /> : <NoProject state={state} />}</main>
      <footer className="ph-foot">
        <span data-words="disclaimer">Early-stage software. Not validated for engineering decisions or for safety-critical use. No warranty.</span>
        {/* G22: the footer's normal facts are said to readers; the disclaimer is the only standing text */}
        <Quiet>{project ? 'Read through the engine, read-only; runs started through the runner' : 'rcfd Studio'}</Quiet>
      </footer>
    </div>
  );
}

function LiveWord({ view }: { view: ProjectView }) {
  return view.liveness.live ? (
    // G22: live is the normal state: the mark alone, the word its name and tooltip
    <span className="ph-live" role="status" aria-label="Live" title="Live: what the folder holds is kept up to date as the engine reports changes">
      <span className="ph-live-mark" aria-hidden="true" />
    </span>
  ) : (
    <span className="ph-live ph-live--off" role="status" data-words="exception">
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
        {/* G22: no project open is the normal first state: the action carries it; what a project is, is said to readers */}
        <h1 id="ph-title" className="sr-only">
          No project open
        </h1>
        <Quiet id="ph-what">
          A project is a folder that holds an rcfd-project.json and the folders cases, meshes, runs and notes. The Studio reads it through the engine,
          changes nothing in it, and shows what Claude Code, the CLI or an editor change there as it happens.
          {engine?.configured ? ` Projects are read by ${engine.path}, without writing.` : ''}
        </Quiet>
        <div className="ph-empty-actions">
          {engine === undefined || engine.configured ? (
            <Button variant="primary" icon="plus" onPress={actions.pick} isDisabled={state?.opening != null} aria-describedby="ph-what">
              Open project folder…
            </Button>
          ) : (
            <Unavailable label="Open project folder…" reason={NOT_CONFIGURED_SHORT} />
          )}
        </div>
        {engine && !engine.configured && (
          <Notice title="Projects cannot be opened yet">
            <p>{engine.reason}</p>
          </Notice>
        )}
        <WorkProgress scope={WORK_SCOPE} placement="inline" />
        {state?.failure && <FailureNotice failure={state.failure} recent={state.recent} />}
        {/* F-813: a learner's first path, from here to a first result */}
        {engine?.configured !== false && !state?.failure && <FirstPathIntro />}
      </section>
      <section className="ph-recent" aria-labelledby="ph-recent-title">
        <h2 id="ph-recent-title" className="ph-section-title sr-only">
          Recent projects
        </h2>
        {!state || state.recent.length === 0 ? (
          <EmptyMark text="No recent projects. The folders you open appear here, the latest first." />
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
        <span className="ph-recent-name" data-words="data">{name}</span>
        <span className="ph-recent-path" data-words="data">{r.path}</span>
      </div>
      <span className="ph-recent-when">
        {r.exists ? (
          <>
            <Quiet>opened </Quiet>
            <span data-words="data">{dateTime(r.opened)}</span>
          </>
        ) : (
          <span data-words="exception">
            <StateMark state="failed" label="Not found" />
          </span>
        )}
      </span>
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
    <div className={`ph-notice${failed ? ' ph-notice--failed' : ''}`} role={failed ? 'alert' : 'note'} data-words="exception">
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
    <>
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
      {/* F-813: what the folder is missing and the next step, beside the failure */}
      {failure.kind === 'not_a_project' && <NotAProjectYet failure={failure} />}
    </>
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

type View = 'home' | 'runs' | 'workspace';

/** F-831: puts the focus on the first element that matches, once the view has changed (G8: the focus follows a command). */
function focusLater(...selectors: string[]): void {
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      for (const sel of selectors) {
        const el = document.querySelector<HTMLElement>(sel);
        if (el) return el.focus();
      }
    }),
  );
}

/**
 * An open project: three views under the header band, Home (the index of the folder, F-805), Runs (F-806) and the run
 * workspace (F-810), which share the selected run, and the Claude Code panel beside them (F-814), whose run and file
 * references open the run in Runs or the file on Home.
 */
function OpenProject({ view, state }: { view: ProjectView; state: ProjectState }) {
  const [tab, setTab] = useState<View>('home');
  // the selected run belongs to the project view, so it survives a switch of view; it is display state, not project state
  const [run, setRun] = useState<string | null>(null);
  // F-807: a case run from the home is followed in the Runs view
  const onStarted = (id: string) => {
    setRun(id);
    setTab('runs');
  };
  // F-814: a file a Claude Code reply named, shown on Home (n changes on every request), and whether the panel is shown
  const [focus, setFocus] = useState<{ path: string; n: number } | null>(null);
  const [claude, setClaude] = useState(true);
  // F-815: the palette's Compare runs, handed to the run workspace (`n` changes on every request)
  const [compareReq, setCompareReq] = useState<{ run: string; n: number } | null>(null);
  const openRun = (id: string) => {
    setRun(id);
    setTab('runs');
  };
  const openFile = (p: string) => {
    setFocus((f) => ({ path: p, n: (f?.n ?? 0) + 1 }));
    setTab('home');
  };
  // F-831: the command palette's commands of these views (src/shared/commands/studio.ts), and the run selected in Runs
  // for the palette to offer first
  useCommandFact('selectedRun', run);
  useCommandHandlers({
    'go.home': { run: () => (setTab('home'), focusLater('.ph-views [role="tab"][aria-selected="true"]')) },
    'go.runs': {
      run: (a) => {
        if (typeof a['run'] === 'string') setRun(a['run']);
        setTab('runs');
        focusLater('.ph-views [role="tab"][aria-selected="true"]');
      },
    },
    'go.claude': { run: () => (setClaude(true), focusLater('.cc textarea', '.cc button')) },
    'view.toggle_claude': { run: () => (setClaude(!claude), focusLater(claude ? '.cc-show button' : '.cc textarea', '.cc button')) },
    'workspace.compare': {
      run: (a) => {
        const other = a['run'];
        if (typeof other === 'string') setCompareReq((c) => ({ run: other, n: (c?.n ?? 0) + 1 }));
        setTab('workspace');
        focusLater('.cmp-other button', '.ph-views [role="tab"][aria-selected="true"]');
      },
    },
  });
  return (
    <div className="ph-work">
    <div className="ph-open">
      <Tabs
        aria-label="Project views"
        className="ph-views"
        selected={tab}
        onChange={(t) => setTab(t as View)}
        items={[
          { id: 'home', label: 'Home', panel: <HomeBody view={view} state={state} onStarted={onStarted} focus={focus} /> },
          {
            id: 'runs',
            label: 'Runs',
            panel: (
              <div className="ph-runs">
                {state.failure && <FailureNotice failure={state.failure} recent={state.recent} />}
                {!view.liveness.live && <NotLive view={view} />}
                <RunsView view={view} selected={run} onSelect={setRun} onOpenWorkspace={() => setTab('workspace')} />
              </div>
            ),
          },
          {
            id: 'workspace',
            label: 'Workspace',
            panel: (
              <div className="ph-runs">
                {!view.liveness.live && <NotLive view={view} />}
                <Suspense fallback={<p className="ph-empty-line">Opening the run workspace…</p>}>
                  <RunWorkspace view={view} selected={run} onSelect={setRun} compareRequest={compareReq} />
                </Suspense>
              </div>
            ),
          },
        ]}
      />
    </div>
    {claude ? (
      <ClaudePanel project={view} onOpenRun={openRun} onOpenFile={openFile} onHide={() => setClaude(false)} />
    ) : (
      <div className="cc-show">
        <IconButton variant="ghost" icon="back" label="Show Claude Code" onPress={() => setClaude(true)} />
      </div>
    )}
    </div>
  );
}

function HomeBody({
  view,
  state,
  onStarted,
  focus,
}: {
  view: ProjectView;
  state: ProjectState;
  onStarted: (run: string) => void;
  focus?: { path: string; n: number } | null;
}) {
  const [kind, setKind] = useState<Kind>('all');
  const [runFailure, setRunFailure] = useState<RunActionResult | null>(null);
  const onRunResult = (r: RunActionResult) => {
    if (r.ok && r.request.run) onStarted(r.request.run);
    else setRunFailure(r);
  };
  const [filter, setFilter] = useState('');
  // F-814: a file named in a Claude Code reply is shown by filtering the index to it
  useEffect(() => {
    if (!focus) return;
    setKind('all');
    setFilter(focus.path);
  }, [focus]);
  const info = view.info;
  const changed = useMemo(() => changedAt(view.changes), [view.changes]);
  const q = filter.trim().toLowerCase();
  const match = (...texts: string[]) => q === '' || texts.some((t) => t.toLowerCase().includes(q));
  const cases = view.cases.items.filter((c) => match(c.path));
  const meshes = view.meshes.items.filter((m) => match(m.path, ...m.used_by));
  const runs = view.runs.items.filter((r) => match(r.id, r.case, r.state));
  const notes = view.notes.items.filter((n) => match(n.path));
  // F-837: the files of cases/ that are not case files are listed apart; the case editor's unsaved files are marked
  const caseFiles = cases.filter((c) => isCaseFile(c.path));
  const dataFiles = cases.filter((c) => !isCaseFile(c.path));
  const unsaved = useUnsavedCases(view.folder);
  const shown = (k: Exclude<Kind, 'all'>) => kind === 'all' || kind === k;
  const nothing =
    q !== '' &&
    (!shown('cases') || cases.length === 0) &&
    (!shown('meshes') || meshes.length === 0) &&
    (!shown('runs') || runs.length === 0) &&
    (!shown('notes') || notes.length === 0);
  const total = view.cases.total + view.meshes.total + view.runs.total + view.notes.total;
  return (
    <div className="ph-home">
      <section className="ph-strip" aria-labelledby="ph-name">
        <h1 id="ph-name" className="ph-name" data-words="data">
          {info?.name ?? baseName(view.folder)}
        </h1>
        <dl className="ph-facts">
          <Fact caption="Folder" plain>
            {view.folder}
          </Fact>
          {/* G22: the folder's normal facts are said to readers; issues and a migrated format speak */}
          {info && (
            <Fact caption="Created" quiet>
              {dateTime(info.created)}
            </Fact>
          )}
          {info && (
            <Fact caption="Created with" quiet>
              {info.solver_version_created}
            </Fact>
          )}
          {info && (
            <Fact caption="Folder format" quiet={info.migrated_from === null}>
              {info.schema_version}
              {info.migrated_from !== null ? ` (read from ${info.migrated_from})` : ''}
            </Fact>
          )}
          {info && (
            <Fact caption="Issues" quiet={info.issues_total === 0}>
              {info.issues_total === 0 ? 'none' : info.issues_total}
            </Fact>
          )}
          <Fact caption="Read by" quiet>
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
      {runFailure && !runFailure.ok && <RunFailure failure={runFailure.failure} onDismiss={() => setRunFailure(null)} />}
      {/* F-813: the first path while the project holds nothing but its example */}
      <FirstPath view={view} nav={{ openRun: onStarted }} />
      <div className="ph-tools">
        <TextField
          aria-label="Filter by name"
          placeholder="Filter"
          value={filter}
          onChange={setFilter}
          className="ph-filter"
        />
        <SegmentedToggle
          aria-label="Show"
          options={[
            { id: 'all', label: 'All', count: total },
            { id: 'cases', label: 'Cases', count: view.cases.total },
            { id: 'meshes', label: 'Meshes', count: view.meshes.total },
            { id: 'runs', label: 'Runs', count: view.runs.total },
            { id: 'notes', label: 'Notes', count: view.notes.total },
          ]}
          selected={kind}
          onChange={(k) => setKind(k as Kind)}
        />
        {state.opening ? (
          <span className="ph-read">
            Opening <span data-words="data">{state.opening}</span>…
          </span>
        ) : (
          <Quiet id="ph-read">Read {clock(view.readAt)}</Quiet>
        )}
        {view.liveness.live && (
          <Button onPress={actions.refresh} aria-describedby="ph-read">
            Read again
          </Button>
        )}
      </div>
      <div className="ph-work">
        <div className="ph-index" role="region" aria-label="What the project folder holds">
          {info && info.issues_total > 0 && <Issues info={info} />}
          {nothing && (
            <div className="ph-nomatch" data-words="exception">
              <p>No entry matches “{filter.trim()}”.</p>
              <Button variant="ghost" onPress={() => setFilter('')}>
                Clear the filter
              </Button>
            </div>
          )}
          {shown('cases') && (
            <Group id="cases" title="Cases" listing={view.cases} engineCount={info?.counts.cases} shown={cases.length} filtered={q !== ''}>
              {caseFiles.length > 0 && <CasesTable cases={caseFiles} changed={changed} view={view} onRunResult={onRunResult} unsaved={unsaved} />}
              <DataFiles files={dataFiles} changed={changed} />
              {view.cases.total === 0 && !view.cases.error && (
                <Empty text="No cases yet. A case is a settings file (.toml or .json) in cases/. Ask Claude Code to make one (its new_case tool), or copy a case file into the folder; it appears here." />
              )}
            </Group>
          )}
          {shown('meshes') && (
            <Group id="meshes" title="Meshes" listing={view.meshes} engineCount={info?.counts.meshes} shown={meshes.length} filtered={q !== ''}>
              {meshes.length > 0 && <MeshesTable meshes={meshes} changed={changed} />}
              {view.meshes.total === 0 && !view.meshes.error && (
                <Empty text="No mesh files. A case can build its own mesh from its settings, so this is often fine. To use a mesh file (Gmsh, Fluent or OpenFOAM), ask Claude Code to bring it in with import_mesh." />
              )}
            </Group>
          )}
          {shown('runs') && (
            <Group id="runs" title="Runs" listing={view.runs} engineCount={info?.counts.runs} shown={runs.length} filtered={q !== ''}>
              {runs.length > 0 && <RunsTable runs={runRows(runs, view.runDetails, view.runDetailErrors)} changed={changed} />}
              {view.runs.total === 0 && !view.runs.error && (
                <Empty text="No runs yet. A run solves one case and keeps its results in runs/. Run a case from the list above; Claude Code (start_run) and the CLI start the same runs, and theirs appear here as they start." />
              )}
              {view.skippedRuns.length > 0 && <SkippedRuns skipped={view.skippedRuns} />}
            </Group>
          )}
          {kind === 'all' && view.designs && <DesignsGroup listing={view.designs} engineCount={info?.counts.designs} filter={(p) => match(p)} />}
          {shown('notes') && (
            <Group id="notes" title="Notes" listing={view.notes} engineCount={info?.counts.notes} shown={notes.length} filtered={q !== ''}>
              {notes.length > 0 && <NotesTable notes={notes} changed={changed} />}
              {view.notes.total === 0 && !view.notes.error && (
                <Empty text="No notes. Notes are free text in notes/, written by you or by Claude Code (write_note)." />
              )}
            </Group>
          )}
        </div>
        <ChangesLane view={view} />
      </div>
    </div>
  );
}

/** A fact of the folder: `quiet` when it is normal (said to readers only, G22); `plain` when the value says what it is. */
function Fact({ caption, children, quiet, plain }: { caption: string; children: ReactNode; quiet?: boolean; plain?: boolean }) {
  return (
    <div className={quiet ? 'ph-fact sr-only' : 'ph-fact'}>
      <dt className={plain ? 'sr-only' : undefined}>{caption}</dt>
      <dd data-words="data">{children}</dd>
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
        <span className="ph-group-count" data-words="data">{info.issues_total}</span>
        <span className="ph-group-what">what the engine found wrong in the folder</span>
      </h2>
      <ul className="ph-issues">
        {info.issues.map((i) => (
          <li key={`${i.path}:${i.message}`}>
            <span className="ph-issue-word">Issue</span>
            <span className="ph-issue-path" data-words="data">{i.path}</span>
            <span data-words="data">{i.message}</span>
          </li>
        ))}
        {info.issues_total > info.issues.length && <li className="ph-more" data-words="data">and {info.issues_total - info.issues.length} more</li>}
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
        <span className="ph-group-count" data-words="data">{listing.total}</span>
        {/* G22: what the folder is for is said to readers; a filter's count and a disagreement are shown */}
        <Quiet>
          {id}/ {DESCRIBE[id]}
        </Quiet>
        <span className="ph-group-what">
          {filtered && listing.total > 0 ? (
            <>
              ; <span data-words="data">{shown}</span> shown
            </>
          ) : (
            ''
          )}
          {disagree ? <span data-words="exception">; the engine counts {engineCount}</span> : ''}
        </span>
      </h2>
      {listing.error !== null ? (
        <div className="ph-group-error" role="alert" data-words="exception">
          <StateMark state="failed" label={`Failed to list ${title.toLowerCase()}`} />
          <span>{listing.error}</span>
          <Button onPress={actions.refresh}>Read again</Button>
        </div>
      ) : (
        children
      )}
      {listing.error === null && listing.total > listing.items.length && (
        <p className="ph-more" data-words="exception">
          The first {listing.items.length} of {listing.total} are listed.
        </p>
      )}
    </section>
  );
}

/** An empty group at rest (G22): the hatch, its sentence the name and tooltip. */
function Empty({ text }: { text: string }) {
  return <EmptyMark text={text} className="ph-empty-line--group" />;
}

function Changed({ at }: { at: string | undefined }) {
  if (!at) return null;
  return (
    <span className="ph-changed">
      <Quiet>changed </Quiet>
      <span data-words="data">{clock(at)}</span>
    </span>
  );
}

function CasesTable({
  cases,
  changed,
  view,
  onRunResult,
  unsaved,
}: {
  cases: readonly CaseEntry[];
  changed: Map<string, string>;
  view: ProjectView;
  onRunResult: (r: RunActionResult) => void;
  unsaved: ReadonlyMap<string, string>;
}) {
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
          <th scope="col">
            <span className="sr-only">Edit</span>
          </th>
          <th scope="col">
            <span className="sr-only">Run</span>
          </th>
        </tr>
      </thead>
      <tbody data-words="data">
        {cases.map((c) => (
          <tr key={c.path} data-path={c.path}>
            <th scope="row" className="ph-name-cell">
              {inFolder(c.path, 'cases')}
              {unsaved.has(c.path) && <UnsavedMark path={c.path} session={unsaved.get(c.path)!} />}
              {c.unread && (
                <span className="ph-unread" data-words="exception">
                  <StateMark state="failed" label="failed to read" />
                  <span>{c.unread}</span>
                </span>
              )}
            </th>
            <td>{c.meshes.length > 0 ? c.meshes.map((m) => inFolder(m, 'meshes')).join(', ') : <Dash words="none in meshes/" />}</td>
            <td className="ph-num">{c.runs}</td>
            <td className="ph-change-cell">
              <Changed at={changed.get(c.path)} />
            </td>
            <td className="ph-edit-cell" data-words="chrome">
              {/* F-812: the case editor opens the file into a service session through the engine */}
              <Button variant="chip" onPress={() => void (location.hash = caseHash.open(c.path))} aria-label={`Edit ${c.path}`}>
                Edit
              </Button>
            </td>
            <td className="rc-case-run" data-words="chrome">
              <CaseRunButton view={view} casePath={c.path} onResult={onRunResult} />
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
      <tbody data-words="data">
        {meshes.map((m) => (
          <tr key={m.path} data-path={m.path}>
            <th scope="row" className="ph-name-cell">
              {inFolder(m.path, 'meshes')}
            </th>
            <td>{m.kind === 'openfoam' ? `OpenFOAM folder, ${plural(m.files, 'file')}` : m.files > 1 ? plural(m.files, 'file') : 'file'}</td>
            <td className="ph-num">{kilobytes(m.bytes)}</td>
            <td>
              {m.used_by.length > 0 ? m.used_by.map((c) => inFolder(c, 'cases')).join(', ') : <Dash words="no case names it" />}
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

function RunsTable({ runs, changed }: { runs: readonly RunRow[]; changed: Map<string, string> }) {
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
      <tbody data-words="data">
        {runs.map((r) => {
          const picto = shownState(r);
          return (
            <tr key={r.id} data-run={r.id} className={r.state === 'failed' ? 'ph-row--failed' : undefined}>
              <th scope="row" className="ph-name-cell">
                <span className="ph-run-case">{inFolder(r.case.replace(/\\/g, '/'), 'cases') || 'case not recorded'}</span>
                <span className="ph-run-id">{r.id}</span>
              </th>
              <td data-words={r.state === 'failed' ? 'exception' : 'chrome'}>
                {picto ? <StateMark state={picto} quiet /> : <span className="ui-state ph-state-word">{unknownWord(r.state)}</span>}
              </td>
              <td className="ph-num">{r.step}</td>
              {/* F-806: a steady run's solver reports time 0, which is no simulated time; words where the engine has no number (G3) */}
              <td className="ph-num" data-words={timeNote(r) === null ? 'data' : timeIsQuiet(timeNote(r)) ? 'chrome' : 'exception'}>
                {timeIsQuiet(timeNote(r)) ? <Dash words={timeText(r)} /> : timeText(r)}
              </td>
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
      <tbody data-words="data">
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
    <ul className="ph-issues ph-issues--failed" data-words="exception">
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
        <span className="ph-lane-since">
          <Quiet>since </Quiet>
          <span data-words="data">{clock(view.openedAt)}</span>
        </span>
      </div>
      {view.changes.length === 0 ? (
        <EmptyMark
          className="ph-lane-empty"
          text="Nothing has changed since the project was opened. What Claude Code, the CLI, the runner or an editor changes in the folder appears here as the engine reports it."
        />
      ) : (
        <ol className="ph-changes" aria-live="polite" aria-relevant="additions" data-words="data">
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
