// A learner's first path (F-813; surface brief .impeccable/surfaces/apps-studio-web-src-renderer-first.md): from no
// project, or an empty folder, to a first honest result, with the plain explanation, the precise terms and the full
// controls one step away (PRODUCT.md Principle 5, G15, G19) and the limits and disclaimer on the result (G6).
//
// The Quiet Rule (G22, DESIGN.md): at rest the path is the station strip, one action and the numbers; what a step is,
// what it will do, where its full control lives, its precise terms and the same run from the command line are behind
// Explain (help on demand) and in accessible names. Visible sentences are kept for the exceptions: a folder that is not
// a project, a failure, a runner that has not picked the run up, a run with no stored pictures, and the result's limits.
//
// Three parts, all shown by the project home:
// - FirstPathIntro: with no project open, the four stations of the path.
// - NotAProjectYet: a folder the engine refused as no project: what is missing (the project file, which the Studio cannot
//   make: the engine has no command for it) and the next step; the home opens the folder by itself once the file is there
//   (src/main/project/home.ts).
// - FirstPath: an open project with nothing but the path's case and runs: make the example (the engine's new_case
//   through the case port, F-812), run it (start_run with the engine's start plan through run control, F-807), watch it,
//   and the result: the engine's numbers, the stored field drawn by the field viewport (F-808), the limits and the
//   disclaimer, and the link to the run (./target.ts, the one place it is decided).
//
// Nothing here writes or keeps project state (G12): the step is read from the engine's lists every time (./model.ts).
import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from 'react';
import type { OpenFailure, ProjectView, RunActionResult } from '../../shared/project.js';
import type { CaseSources } from '../../shared/case.js';
import type { StoreInfo, StoreRef } from '../../shared/data.js';
import { FIRST_CASE, FIRST_CASE_NAME, FIRST_FIELD, FIRST_RUN_EXAMPLE, FIRST_TEMPLATE, FULL_CONTROL } from '../../shared/actions/first.js';
import type { RunActionId } from '../../shared/actions/runs.js';
import { commandLine } from '../../shared/actions/runs.js';
import { Button, IconButton, SegmentedToggle, StatePictogram, Unavailable } from '../ui/index.js';
import { caseDoor } from '../case/door.js';
import { caseHash } from '../case/hash.js';
import { runAction } from '../project/door.js';
import { openStore } from '../data/door.js';
import { FieldViewport } from '../field/FieldViewport.js';
import { plain, type RunRow } from '../runs/model.js';
import { clock } from '../project/format.js';
import { STATIONS, fieldWord, fieldsFolder, firstRequest, firstStep, outcomeSentence, plainState, sameRunCommand, stationIndex, type FirstStep, type StationId } from './model.js';
import { DISCLAIMER, EXAMPLE_LIMITS, STEP_HELP, TERMS, type Term } from './words.js';
import { openFirstResult, RESULT_LINK, type ResultNavigation } from './target.js';
import './first.css';

// ---- shared pieces

const STATION_WORD = { done: 'done', here: 'current step', ahead: 'to come' } as const;

/** The ruled row of the four stations: done (a drawn check), here (the ink field) and ahead (ruled). Words for readers only. */
function Stations({ at }: { at: StationId | null }) {
  const here = at === null ? -1 : stationIndex(at);
  return (
    <ol className="fp-stations" aria-label="The steps of the first path">
      {STATIONS.map((s, i) => {
        const state = i < here ? 'done' : i === here ? 'here' : 'ahead';
        return (
          <li key={s.id} className={`fp-station fp-station--${state}`} aria-current={state === 'here' ? 'step' : undefined} data-station={s.id}>
            <span className="fp-station-mark" aria-hidden="true">
              {state === 'done' ? (
                <svg width="14" height="14" viewBox="0 0 14 14">
                  <path d="M2 7L5 10L12 3" />
                </svg>
              ) : (
                i + 1
              )}
            </span>
            <span className="fp-station-title">{s.title}</span>
            <span className="sr-only">, {STATION_WORD[state]}</span>
          </li>
        );
      })}
    </ol>
  );
}

/** Help on demand (G22): one toggle that opens what a part of the path is, what it does and where its full control lives. */
function Explain({ name, children }: { name: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <div className="fp-explain">
      <Button variant="ghost" icon={open ? 'minus' : 'plus'} onPress={() => setOpen((o) => !o)} aria-expanded={open} aria-controls={id} aria-label={`${open ? 'Hide' : 'Explain'} ${name}`}>
        {open ? 'Hide' : 'Explain'}
      </Button>
      {open && (
        <div id={id} className="fp-explain-body" role="region" aria-label={`About ${name}`}>
          {children}
        </div>
      )}
    </div>
  );
}

function Terms({ terms }: { terms: readonly Term[] }) {
  return (
    <dl className="fp-terms-list" aria-label="The precise terms">
      {terms.map((t) => (
        <div key={t.plain} className="fp-term">
          <dt>{t.plain}</dt>
          <dd>
            {t.precise}
            <span className="fp-term-source">{t.source}</span>
          </dd>
        </div>
      ))}
    </dl>
  );
}

function Caption({ children }: { children: ReactNode }) {
  return <span className="fp-caption">{children}</span>;
}

/** The help of a step: its plain explanation, where its full control lives (G15) and its precise terms (G19). */
function StepHelp({ step, children }: { step: StationId; children?: ReactNode }) {
  return (
    <>
      {STEP_HELP[step].map((p) => (
        <p key={p}>{p}</p>
      ))}
      <p className="fp-help-block">
        <Caption>Full control</Caption>
        {FULL_CONTROL[step]}
      </p>
      {children}
      <Terms terms={TERMS[step]} />
    </>
  );
}

/** A failure on the path: always visible (G6, G22), with its pictogram, the problem and the way out. */
function Problem({ title, problem, recovery }: { title: string; problem: string; recovery: string }) {
  return (
    <div className="fp-problem" role="alert">
      <p className="fp-problem-title">
        <StatePictogram state="failed" />
        {title}
      </p>
      <p>{problem}</p>
      <p className="fp-problem-next">{recovery}</p>
    </div>
  );
}

// ---- no project open

/** With no project open: the four stations of the path; what they are is one step away. */
export function FirstPathIntro() {
  return (
    <section className="fp fp--intro" aria-labelledby="fp-intro-title">
      <h2 id="fp-intro-title" className="fp-title">
        A first result in four steps
      </h2>
      <Stations at={null} />
      <Explain name="the first steps">
        <p>
          The Studio shows what the real solver computes. The first time, let it make a small example for you, run it and show what came out: a
          square box of fluid whose lid slides sideways and stirs the fluid round.
        </p>
        <p>
          First, a project folder: the place where your examples, runs and results are kept. Choose one with Open project folder… above. A folder
          rcfd made before opens at once; an empty folder needs rcfd’s project file first, and the Studio says how once you choose it.
        </p>
        <p>
          In a copy of the rcfd repository, <span className="ph-file">example-projects/quick-start</span> is a finished project with runs to look at
          (copy it out of the repository before you run anything in it).
        </p>
      </Explain>
    </section>
  );
}

// ---- a folder that is not a project

const MAKE_PROJECT_GAP = 'The engine has no command that makes a new project yet, and the Studio changes a project folder only through the engine.';

/** A folder the engine refused as no project (an exception, so in words): what is missing and the next step. */
export function NotAProjectYet({ failure }: { failure: OpenFailure }) {
  const st = failure.folderState;
  const holds = st?.entries == null ? '' : st.entries === 0 ? 'Empty folder: ' : `${st.entries === 1 ? '1 entry' : `${st.entries} entries`}, `;
  return (
    <section className="fp fp--folder" aria-labelledby="fp-folder-title" data-folder-state={st ? (st.watching ? 'watching' : 'still') : 'unknown'}>
      <h2 id="fp-folder-title" className="fp-title">
        This folder is not a project yet
      </h2>
      <p className="fp-missing">
        {holds}no <span className="ph-file">rcfd-project.json</span>.
      </p>
      <p className="fp-next">
        {st?.watching
          ? 'Next: make it with Claude Code or the older rcfd-studio; the folder then opens by itself.'
          : 'Next: make it with Claude Code or the older rcfd-studio, then open the folder again.'}
      </p>
      <div className="fp-actions">
        <Unavailable label="Make a project here" reason={`Not available: ${MAKE_PROJECT_GAP}`} />
        <Explain name="what a project needs">
          <p>
            Every rcfd project starts with a small project file, <span className="ph-file">rcfd-project.json</span>, at the top of its folder. The
            Studio cannot make it here: {MAKE_PROJECT_GAP}
          </p>
          <ul>
            <li>Ask Claude Code, in a terminal opened in this folder, to make it an rcfd project.</li>
            <li>Or use Project &gt; New project in the older rcfd-studio app.</li>
            <li>
              {st?.watching
                ? 'Keep this window open: the Studio checks the folder every second and opens it as soon as the project file is there.'
                : 'Then open the folder again with Open project folder….'}
            </li>
            <li>Or choose another folder, one that already holds a project.</li>
          </ul>
        </Explain>
      </div>
    </section>
  );
}

// ---- an open project on the path

export interface FirstPathProps {
  readonly view: ProjectView;
  readonly nav: ResultNavigation;
}

/** The path on the home of an open project; nothing when the project is past it (./model.ts firstStep). */
export function FirstPath({ view, nav }: FirstPathProps) {
  const step = useMemo(() => firstStep(view), [view]);
  const [hidden, setHidden] = useState(false);
  const box = useRef<HTMLElement>(null);
  const last = useRef<string | null>(null);
  // the step changed (by an action here, or by Claude Code or the CLI): move the focus to the new step's action when the
  // focus was on the path or nowhere, so a keyboard walk goes on from there
  useEffect(() => {
    const key = step ? `${step.at}:${'run' in step ? step.run.id : ''}` : null;
    const el = box.current;
    if (last.current !== null && key !== last.current && el) {
      const a = document.activeElement;
      const onPath = a === null || a === document.body || el.contains(a);
      const target = el.querySelector<HTMLElement>('.fp-primary button, .fp-primary [role="button"]');
      if (onPath && target) target.focus();
    }
    last.current = key;
  }, [step]);
  if (step === null) return null;
  if (hidden) {
    return (
      <div className="fp-hidden">
        <Button variant="ghost" icon="plus" onPress={() => setHidden(false)}>
          Show the first steps
        </Button>
      </div>
    );
  }
  return (
    <section className="fp fp--open" aria-labelledby="fp-title" data-step={step.at} ref={box}>
      <div className="fp-head">
        <h2 id="fp-title" className="fp-title">
          Your first result
        </h2>
        <Stations at={step.at} />
        <IconButton icon="close" label="Hide the first steps" variant="ghost" onPress={() => setHidden(true)} className="fp-hide" />
      </div>
      <div className="fp-body">
        {step.at === 'make' && <MakeStep />}
        {step.at === 'run' && <RunStep />}
        {step.at === 'watch' && <WatchStep view={view} step={step} nav={nav} />}
        {step.at === 'result' && <ResultStep view={view} step={step} nav={nav} />}
      </div>
    </section>
  );
}

/** A step: its name for readers (the station strip shows it), its content and its help. */
function Step({ id, children, wide }: { id: StationId; children: ReactNode; wide?: boolean }) {
  const title = STATIONS[stationIndex(id)]!.title;
  return (
    <div className={`fp-step${wide ? ' fp-step--result' : ''}`} role="group" aria-labelledby={`fp-step-${id}`} data-step={id}>
      <h3 id={`fp-step-${id}`} className="sr-only">
        {title}
      </h3>
      {children}
    </div>
  );
}

function MakeStep() {
  const [sources, setSources] = useState<CaseSources | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<{ problem: string; recovery: string } | null>(null);
  useEffect(() => {
    let live = true;
    void caseDoor.sources().then((r) => {
      if (!live) return;
      if (r.ok) setSources(r.result);
      else setSourceError(r.error.message);
    });
    return () => {
      live = false;
    };
  }, []);
  const offered = sources?.templates.includes(FIRST_TEMPLATE) ?? false;
  const make = async () => {
    setBusy(true);
    setFailure(null);
    const r = await caseDoor.create(FIRST_CASE_NAME, { kind: 'template', name: FIRST_TEMPLATE });
    setBusy(false);
    if (!r.ok) {
      setFailure({
        problem: `The engine did not make the example: ${r.error.message}`,
        recovery: 'Nothing was written. Try again; if it repeats, New case in the header band shows the engine’s templates and its words.',
      });
    }
    // on success the engine's change feed brings the case, and the path moves on by itself
  };
  return (
    <Step id="make">
      <div className="fp-step-text">
        {failure && <Problem title="Failed to make the example" problem={failure.problem} recovery={failure.recovery} />}
        {sourceError && (
          <Problem
            title="Failed to ask the engine for its examples"
            problem={`The engine’s project server did not answer: ${sourceError}`}
            recovery="Check that rcfd-mcp is built next to rcfd-runner (cargo build --release -p rcfd-mcp -p rcfd-runner), then close and open the project again."
          />
        )}
        <div className="fp-actions">
          <div className="fp-primary">
            {sources && !offered ? (
              <Unavailable label="Make the example" reason={`The engine does not offer the template ${FIRST_TEMPLATE}; New case lists the ones it has.`} />
            ) : (
              <Button variant="primary" icon="plus" onPress={() => void make()} isDisabled={busy || sources === null}>
                {busy ? 'Making the example…' : 'Make the example'}
              </Button>
            )}
          </div>
          <Explain name="this step">
            <StepHelp step="make" />
          </Explain>
        </div>
      </div>
    </Step>
  );
}

function RunStep() {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<RunActionResult | null>(null);
  const run = async () => {
    setBusy(true);
    setFailure(null);
    const r = await runAction(FIRST_RUN_EXAMPLE as RunActionId, { case: FIRST_CASE } as never);
    setBusy(false);
    if (r.ok) rememberPlan(r.request.run, r.result);
    else setFailure(r);
  };
  return (
    <Step id="run">
      <div className="fp-step-text">
        {failure && !failure.ok && <Problem title="Failed to start the run" problem={failure.failure.problem} recovery={failure.failure.recovery} />}
        <div className="fp-actions">
          <div className="fp-primary">
            <Button variant="primary" icon="play" onPress={() => void run()} isDisabled={busy}>
              {busy ? 'Starting the run…' : 'Run the example'}
            </Button>
          </div>
          <Explain name="this step">
            <StepHelp step="run" />
          </Explain>
        </div>
      </div>
    </Step>
  );
}

/**
 * A run's state as its pictogram, with the plain word and the precise one in its accessible name and tooltip (G8, G22);
 * the word is visible only for an exception (failed, stopped).
 */
function StateMark({ run }: { run: RunRow }) {
  const s = plainState(run);
  const exception = run.state === 'failed' || run.state === 'cancelled';
  const name = `${s.word} (${s.precise}). ${outcomeSentence(run)}`;
  return (
    <p className={`fp-state${run.state === 'failed' ? ' fp-state--failed' : ''}`} data-state={run.state}>
      <span className="fp-state-mark" role="img" aria-label={name} title={name}>
        {s.mark && <StatePictogram state={s.mark} size={18} />}
      </span>
      {exception && <span className="fp-state-word">{s.word}</span>}
      <span className="fp-state-run">{run.id}</span>
    </p>
  );
}

function WatchStep({ view, step, nav }: { view: ProjectView; step: Extract<FirstStep, { at: 'watch' }>; nav: ResultNavigation }) {
  const run = step.run;
  const req = firstRequest(view);
  const slow = req !== null && req.run === run.id && req.slow;
  const [runner, setRunner] = useState<RunActionResult | null>(null);
  return (
    <Step id="watch">
      <div className="fp-step-text">
        <StateMark run={run} />
        <dl className="fp-grid" aria-label={`Progress of ${run.id}`}>
          <div className="fp-num">
            <dt>Rounds of calculation</dt>
            <dd>{run.step}</dd>
          </div>
        </dl>
        {slow && (
          <div className="fp-problem fp-problem--blocked" role="status">
            <p className="fp-problem-title">The runner has not picked the run up</p>
            <div className="fp-actions">
              <Button onPress={async () => setRunner(await runAction('project.start_runner', {}))}>Start the runner</Button>
            </div>
            {runner && !runner.ok && <p className="fp-problem-next">{`${runner.failure.problem} ${runner.failure.recovery}`}</p>}
          </div>
        )}
        <div className="fp-actions">
          <div className="fp-primary">
            <Button onPress={() => openFirstResult(run.id, nav)}>Follow it in Runs</Button>
          </div>
          <Explain name="this step">
            <StepHelp step="watch">
              {slow && (
                <p>The project’s runner may not be running (the engine has no way to say whether it is). Start it, and it takes the run at once.</p>
              )}
              <p className="fp-help-block">
                <Caption>Read from the runner at</Caption>
                {run.detail ? clock(run.detail.updated) : '…'}
              </p>
            </StepHelp>
          </Explain>
        </div>
      </div>
    </Step>
  );
}

/** Report values: six significant figures, the engine's full value on hover (as the Runs view shows them). */
const report = (v: number): string => {
  const s = v.toPrecision(6);
  return s.includes('e') ? s : String(Number(s));
};

const REPORT_WORDS: Readonly<Record<string, string>> = {
  lid_drag: 'Force of the fluid on the lid',
};

function ResultStep({ view, step, nav }: { view: ProjectView; step: Extract<FirstStep, { at: 'result' }>; nav: ResultNavigation }) {
  const run = step.run;
  const failed = run.state === 'failed';
  // the exceptions speak (G22): a failure, a stop, or an end that did not settle
  const mark = plainState(run).mark;
  const exception = failed || run.state === 'cancelled' || (mark === 'finished' && run.detail !== null);
  const d = run.detail;
  const rounds = d?.finish?.iterations ?? run.step;
  const reports = Object.entries(run.metrics);
  return (
    <Step id="result" wide>
      <div className="fp-step-text">
        <StateMark run={run} />
        {exception && <p className={`fp-outcome${failed ? ' fp-outcome--failed' : ''}`}>{outcomeSentence(run)}</p>}
        {exception && d?.reason && (
          <p className="fp-engine">
            <Caption>The engine’s words</Caption>
            {d.reason}
          </p>
        )}
        {!failed && (
          <dl className="fp-grid" aria-label={`Numbers of ${run.id}`}>
            <div className="fp-num">
              <dt>Rounds of calculation</dt>
              <dd>{rounds}</dd>
            </div>
            <div className="fp-num">
              <dt>Solver time, s</dt>
              <dd>{d?.finish?.elapsed != null ? plain(d.finish.elapsed, 2) : 'not reported'}</dd>
            </div>
            {reports.map(([k, v]) => (
              <div className="fp-num fp-num--wide" key={k}>
                <dt>{REPORT_WORDS[k] ?? k}, no unit from the engine</dt>
                <dd title={String(v)}>{report(v)}</dd>
              </div>
            ))}
          </dl>
        )}
        <div className="fp-limits" role="note" aria-label="What this result is not">
          <p>{EXAMPLE_LIMITS}</p>
          <p>{DISCLAIMER}</p>
        </div>
        <div className="fp-actions">
          <div className="fp-primary">
            <Button variant="primary" onPress={() => openFirstResult(run.id, nav)}>
              {RESULT_LINK.label}
            </Button>
          </div>
          <Button onPress={() => void (location.hash = caseHash.open(step.casePath))}>Edit the case</Button>
          <Explain name="this result">
            <StepHelp step="result">
              {!exception && d?.reason && (
                <p className="fp-help-block">
                  <Caption>The engine’s words</Caption>
                  {d.reason}
                </p>
              )}
              <SameRun view={view} run={run} casePath={step.casePath} />
            </StepHelp>
          </Explain>
        </div>
      </div>
      {!failed && <FirstPicture folder={view.folder} run={run} />}
    </Step>
  );
}

/** The command that starts the same run from the shell (G17), with the plan's entries when this window started it. */
function SameRun({ view, run, casePath }: { view: ProjectView; run: RunRow; casePath: string }) {
  const req = (view.runRequests ?? []).find((q) => q.run === run.id && q.action === FIRST_RUN_EXAMPLE) ?? null;
  const set = req ? (firstPlans.get(run.id) ?? null) : null;
  const argv = sameRunCommand(casePath, view.folder, set ?? []);
  return (
    <p className="fp-help-block fp-same">
      <Caption>The same run from the command line</Caption>
      <span className="ph-file fp-command">{commandLine(argv)}</span>
      {set === null && <span>add the --set entries of the run’s start plan for the same stored fields</span>}
      <span>
        Solver {run.detail?.solverVersion ?? '…'}; read through the engine (run_status of runs/{run.id}){run.detail ? ` at ${clock(run.detail.readAt)}` : ''}.
      </span>
    </p>
  );
}

/** start_run's `store.set` per run, from this window's answers (display only; lost on reload, then the note says so). */
const firstPlans = new Map<string, string[]>();

/** The stored field of the run's last snapshot, drawn by the field viewport; which field is the toggle's to say. */
function FirstPicture({ folder, run }: { folder: string; run: RunRow }) {
  const store = useMemo<StoreRef>(() => ({ route: 'local', path: fieldsFolder(folder, run.id) }), [folder, run.id]);
  const [info, setInfo] = useState<{ ok: true; info: StoreInfo } | { ok: false; message: string } | null>(null);
  const [field, setField] = useState<string>(FIRST_FIELD);
  useEffect(() => {
    let live = true;
    setInfo(null);
    void openStore(store, { tag: `first-path-${run.id}` }).then((r) => {
      if (!live) return;
      setInfo(r.ok ? { ok: true, info: r.result } : { ok: false, message: r.error.message });
    });
    return () => {
      live = false;
    };
  }, [store, run.id]);
  if (info === null) return <div className="fp-picture fp-picture--empty" role="status" aria-label="Opening the run’s stored fields" aria-busy="true" />;
  if (!info.ok) {
    return (
      <div className="fp-picture fp-picture--empty" role="status">
        <p className="fp-problem-title">No stored pictures for this run</p>
        <p title={info.message}>Run the example from here to store them; Run in Runs stores only what the case asks for.</p>
      </div>
    );
  }
  const cells = info.info.fields.filter((f) => f.location === 'cell');
  const shown = cells.some((f) => f.name === field) ? field : (cells[0]?.name ?? field);
  const last = info.info.snapshots.length - 1;
  return (
    <div className="fp-picture">
      {cells.length > 1 && (
        <SegmentedToggle aria-label="What the picture shows" options={cells.map((f) => ({ id: f.name, label: fieldWord(f.name) }))} selected={shown} onChange={setField} />
      )}
      <div className="fp-view">{last >= 0 ? <FieldViewport store={store} snapshot={last} field={shown} /> : <p className="fp-problem-title">The store holds no snapshot</p>}</div>
    </div>
  );
}

/** Records the plan's entries of a first run's answer, for SameRun (called by the run step's dispatch). */
export function rememberPlan(run: string | null, result: Readonly<Record<string, unknown>>): void {
  if (run === null) return;
  const store = result['store'];
  if (typeof store !== 'object' || store === null) return;
  const set = (store as { set?: unknown }).set;
  if (Array.isArray(set) && set.every((s) => typeof s === 'string')) firstPlans.set(run, set as string[]);
}
