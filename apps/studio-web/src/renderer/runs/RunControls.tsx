// Run control in the window (F-807): the run bar of the Runs view (run or queue a case, start the runner), the controls
// of the selected run (pause, resume, stop, cancel, run again), the confirmation in words for what loses something
// (G18), and the Run button of a case on the project home.
//
// Every control is a named action of src/shared/actions/runs.ts dispatched through ../project/door.ts (runAction); its
// availability and its reason come from the same registry and the run list's answer, and every reply or refusal is the
// main process's, with the engine's words (G2, G19). Nothing here keeps run state (G7, G12): what a run is, is what the
// runner wrote, pushed with the project view.
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { TextArea, TextField as AriaTextField, Label, Text } from 'react-aria-components';
import { commandLine, type Confirmation, type RunActionId, type RunActionParams } from '../../shared/actions/runs.js';
import type { ProjectView, RunActionFailure, RunActionResult, RunRequest } from '../../shared/project.js';
import { Button, IconButton, NumberField, Quiet, Select, StateMark, Unavailable, type IconName } from '../ui/index.js';
import { containTab } from '../ui/containFocus.js';
import { runAction, runActionAvailability, runActionConfirmation } from '../project/door.js';
import { latestFor, requestWords, setEntries } from './control-model.js';
import { caseName, type RunRow } from './model.js';
import './control.css';

/** One action in flight at a time per control group; the last answer for its feedback line. */
function useAction(onStarted?: (run: string) => void) {
  const [busy, setBusy] = useState<RunActionId | null>(null);
  const [last, setLast] = useState<RunActionResult | null>(null);
  const run = async <Id extends RunActionId>(id: Id, params: RunActionParams<Id>): Promise<RunActionResult> => {
    setBusy(id);
    try {
      const r = await runAction(id, params);
      setLast(r);
      if (r.ok && r.request.run && (id === 'project.run_case' || id === 'project.queue_case' || id === 'project.rerun_run')) onStarted?.(r.request.run);
      return r;
    } finally {
      setBusy(null);
    }
  };
  return { busy, last, run, clear: () => setLast(null) };
}

/** A control: the button when the action is available, else the unavailable chip that says why (DESIGN.md Buttons). */
function Control<Id extends RunActionId>({
  id,
  params,
  view,
  label,
  onPress,
  busy,
  variant,
  icon,
  name,
  iconOnly,
}: {
  id: Id;
  params: Partial<RunActionParams<Id>>;
  view: ProjectView;
  label: string;
  onPress: () => void;
  busy: boolean;
  variant?: 'primary' | 'default' | 'ghost';
  icon?: IconName;
  /** The accessible name when it differs from the label (a case's Run button names its case). */
  name?: string;
  /** G22: the icon alone, the name its tooltip (a row's Run). An unavailable action still says its label and why. */
  iconOnly?: boolean;
}) {
  const av = runActionAvailability(id, params, view);
  if (!av.available) return <Unavailable label={label} reason={av.reason} />;
  if (iconOnly && icon) return <IconButton variant={variant ?? 'default'} icon={icon} label={name ?? label} onPress={onPress} isDisabled={busy} data-action={id} />;
  return (
    <Button variant={variant ?? 'default'} icon={icon} onPress={onPress} isDisabled={busy} aria-label={name} data-action={id}>
      {busy ? `${label}…` : label}
    </Button>
  );
}

/** A refusal or failure: the problem, the way out, and the engine's own words when it gave them (G19). */
export function RunFailure({ failure, onDismiss, onStartRunner }: { failure: RunActionFailure; onDismiss?: () => void; onStartRunner?: () => void }) {
  return (
    <div className="rc-failure" role="alert" data-code={failure.code} data-words="exception">
      <StateMark state="failed" label="Refused" />
      <div className="rc-failure-text">
        <p className="rc-problem">{failure.problem}</p>
        <p className="rc-recovery">{failure.recovery}</p>
      </div>
      <div className="rc-failure-actions">
        {failure.code === 'runner' && onStartRunner && <Button onPress={onStartRunner}>Start the runner</Button>}
        {onDismiss && (
          <Button variant="ghost" onPress={onDismiss}>
            Dismiss
          </Button>
        )}
      </div>
    </div>
  );
}

/** The CLI and Claude Code equivalents of a request (G16, G17). */
function Equivalents({ request }: { request: RunRequest }) {
  return (
    <p className="rc-equiv">
      <span className="rc-equiv-caption">CLI</span>
      <Quiet>, in the project folder</Quiet>
      <code className="rc-code" data-words="data">
        {commandLine(request.cli.argv)}
      </code>
      {request.mcp && (
        <>
          <span className="rc-equiv-caption">Claude Code</span>
          <code className="rc-code" data-words="data">
            {request.mcp.tool} {JSON.stringify(request.mcp.arguments)}
          </code>
        </>
      )}
    </p>
  );
}

/** What happened to the last request of a control group: the runner's progress on it, or why it was refused. */
function Feedback({
  request,
  failure,
  onDismiss,
  onStartRunner,
}: {
  request: RunRequest | null;
  failure: RunActionFailure | null;
  onDismiss: () => void;
  onStartRunner: () => void;
}) {
  if (failure) {
    return (
      <div className="rc-feedback">
        <RunFailure failure={failure} onDismiss={onDismiss} onStartRunner={onStartRunner} />
        {request && <Equivalents request={request} />}
      </div>
    );
  }
  if (!request) return null;
  return (
    <div className="rc-feedback">
      <p
        className={request.slow ? 'rc-status rc-status--slow' : 'rc-status'}
        data-words={request.slow ? 'exception' : 'chrome'}
        role="status"
        data-request-seq={request.seq}
        data-settled={request.settled ? 'true' : 'false'}
        data-slow={request.slow ? 'true' : 'false'}
      >
        {requestWords(request)}
      </p>
      {request.slow && (
        <div className="rc-slow-actions" data-words="exception">
          <Button onPress={onStartRunner}>Start the runner</Button>
        </div>
      )}
      <Equivalents request={request} />
    </div>
  );
}

/** The request of `r` as the view now has it (settled and slow change as the runner acts). */
const current = (view: ProjectView, r: RunActionResult | null): RunRequest | null =>
  r?.request ? ((view.runRequests ?? []).find((q) => q.seq === r.request!.seq) ?? r.request) : null;

// ---- the run bar of the Runs view

/** Run or queue a case of the project, and start the runner (F-807). Above the runs table. */
export function RunBar({ view, onStarted, lead }: { view: ProjectView; onStarted: (run: string) => void; lead?: ReactNode }) {
  const cases = view.cases.items.map((c) => c.path);
  const [picked, setPicked] = useState<string | null>(null);
  const casePath = picked !== null && cases.includes(picked) ? picked : (cases[0] ?? null);
  const [queueOpen, setQueueOpen] = useState(false);
  const act = useAction(onStarted);
  const startRunner = () => void act.run('project.start_runner', {});
  const request = current(view, act.last);
  return (
    <section className="rc-bar" aria-label="Start a run">
      <div className="rc-bar-row">
        <span className="rc-bar-title sr-only">Start a run</span>
        {/* F-837: the runner's budgets first (the keys of F-807's bar keep their order: case, Run, Queue, Start the runner) */}
        {lead}
        {cases.length > 0 ? (
          <Select
            aria-label="Case to run"
            valueWords="data"
            className="rc-case"
            options={cases.map((c) => ({ id: c, label: caseName(c) }))}
            selected={casePath}
            onChange={setPicked}
          />
        ) : (
          <span className="rc-quiet">No case in cases/ to run.</span>
        )}
        <Control
          id="project.run_case"
          params={casePath ? { case: casePath } : {}}
          view={view}
          label="Run"
          icon="play"
          variant="primary"
          busy={act.busy === 'project.run_case'}
          onPress={() => casePath && void act.run('project.run_case', { case: casePath })}
          name={casePath ? `Run ${caseName(casePath)}` : undefined}
        />
        <Button onPress={() => setQueueOpen((o) => !o)} aria-expanded={queueOpen} isDisabled={casePath === null} data-action="project.queue_case">
          Queue…
        </Button>
        <span className="rc-spacer" />
        <Control
          id="project.start_runner"
          params={{}}
          view={view}
          label="Start the runner"
          variant="ghost"
          busy={act.busy === 'project.start_runner'}
          onPress={startRunner}
        />
      </div>
      {queueOpen && casePath && (
        <QueueForm
          view={view}
          casePath={casePath}
          busy={act.busy === 'project.queue_case'}
          onQueue={async (p) => {
            const r = await act.run('project.queue_case', p);
            if (r.ok) setQueueOpen(false);
          }}
          onClose={() => setQueueOpen(false)}
        />
      )}
      <Feedback request={request} failure={act.last && !act.last.ok ? act.last.failure : null} onDismiss={act.clear} onStartRunner={startRunner} />
    </section>
  );
}

/** The queue form: the start_run arguments that the runner's queue uses (priority, threads, memory, per-run settings). */
function QueueForm({
  view,
  casePath,
  busy,
  onQueue,
  onClose,
}: {
  view: ProjectView;
  casePath: string;
  busy: boolean;
  onQueue: (p: RunActionParams<'project.queue_case'>) => void;
  onClose: () => void;
}) {
  const [priority, setPriority] = useState(0);
  const [threads, setThreads] = useState(1);
  const [mem, setMem] = useState<number>(Number.NaN);
  const [set, setSet] = useState('');
  const params: RunActionParams<'project.queue_case'> = {
    case: casePath,
    ...(priority !== 0 && Number.isFinite(priority) ? { priority } : {}),
    ...(threads !== 1 && Number.isFinite(threads) ? { threads } : {}),
    ...(Number.isFinite(mem) ? { mem_mb: mem } : {}),
    ...(setEntries(set).length ? { set: setEntries(set) } : {}),
  };
  const first = useRef<HTMLDivElement>(null);
  useEffect(() => {
    first.current?.querySelector('input')?.focus();
  }, []);
  return (
    <div
      className="rc-queue"
      role="group"
      aria-label={`Queue ${caseName(casePath)}`}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div ref={first} className="rc-queue-fields">
        <NumberField label="Priority" value={priority} onChange={setPriority} step={1} description="Higher runs first among the queued runs." />
        <NumberField label="Threads" value={threads} onChange={setThreads} minValue={1} maxValue={256} step={1} description="Solver threads of the run." />
        <NumberField label="Memory, MB" value={mem} onChange={setMem} minValue={1} step={1} description="Leave empty for the runner’s estimate." />
        <AriaTextField className="ui-field rc-set" value={set} onChange={setSet}>
          <Label className="ui-label">Settings for this run</Label>
          <TextArea className="ui-input rc-set-input" rows={3} placeholder="solver.max_iterations=500" />
          <Text slot="description" className="ui-description">
            One path=value per line, as rcfd run --set takes them; the case file is not changed.
          </Text>
        </AriaTextField>
      </div>
      <div className="rc-queue-actions">
        <Control
          id="project.queue_case"
          params={params}
          view={view}
          label="Add to the queue"
          variant="primary"
          icon="plus"
          busy={busy}
          onPress={() => onQueue(params)}
        />
        <Button variant="ghost" onPress={onClose}>
          Close
        </Button>
      </div>
    </div>
  );
}

// ---- the selected run

/** The confirmation strip (G18): what is lost, in words; Escape or the keep button leaves everything as it is. */
function Confirm({ words, onConfirm, onKeep }: { words: Confirmation; onConfirm: () => void; onKeep: () => void }) {
  const keep = useRef<HTMLDivElement>(null);
  useEffect(() => {
    keep.current?.querySelector<HTMLButtonElement>('button[data-keep]')?.focus();
  }, []);
  return (
    <div
      className="rc-confirm"
      role="alertdialog"
      aria-modal="true"
      data-words="exception"
      aria-labelledby="rc-confirm-title"
      aria-describedby="rc-confirm-body"
      ref={keep}
      onKeyDown={(e) => {
        // F-816 (G8): Tab stays in the strip while it asks
        containTab(e);
        if (e.key === 'Escape') {
          e.stopPropagation();
          onKeep();
        }
      }}
    >
      <p id="rc-confirm-title" className="rc-confirm-title">
        {words.title}
      </p>
      <p id="rc-confirm-body" className="rc-confirm-body">
        {words.body}
      </p>
      <div className="rc-confirm-actions">
        <Button variant="primary" onPress={onConfirm} data-confirm="">
          {words.confirm}
        </Button>
        <Button onPress={onKeep} data-keep="">
          {words.keep}
        </Button>
      </div>
    </div>
  );
}

/** Pause or resume, stop or cancel, and run the case again: the controls of the selected run. */
export function SelectedRunControls({ view, row, onStarted }: { view: ProjectView; row: RunRow; onStarted: (run: string) => void }) {
  const act = useAction(onStarted);
  const [asking, setAsking] = useState<'project.stop_run' | 'project.cancel_run' | null>(null);
  // a new selection starts clean
  useEffect(() => {
    setAsking(null);
    act.clear();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [row.id]);
  const p = { run: row.id };
  const holdSlot = row.state === 'paused' ? 'project.resume_run' : 'project.pause_run';
  const endSlot = runActionAvailability('project.cancel_run', p, view).available ? 'project.cancel_run' : 'project.stop_run';
  const words = asking ? runActionConfirmation(asking, p, view) : null;
  const latest = latestFor(view.runRequests, row.id);
  const shown = act.last ? current(view, act.last) : latest;
  const startRunner = () => void act.run('project.start_runner', {});
  // focus goes back to the Stop or Cancel control when the confirmation closes
  const endRef = useRef<HTMLSpanElement>(null);
  const refocus = () => requestAnimationFrame(() => endRef.current?.querySelector<HTMLElement>('button, [role="button"]')?.focus());
  return (
    <section className="rv-block rc-run" aria-labelledby="rc-run-title">
      <h3 id="rc-run-title" className="rv-block-title sr-only">
        Run control
      </h3>
      <div className="rc-run-row">
        {holdSlot === 'project.pause_run' ? (
          <Control id="project.pause_run" params={p} view={view} label="Pause" icon="pause" busy={act.busy !== null} onPress={() => void act.run('project.pause_run', p)} />
        ) : (
          <Control id="project.resume_run" params={p} view={view} label="Resume" icon="play" busy={act.busy !== null} onPress={() => void act.run('project.resume_run', p)} />
        )}
        <span ref={endRef} className="rc-slot">
          {endSlot === 'project.cancel_run' ? (
            <Control id="project.cancel_run" params={p} view={view} label="Cancel…" icon="close" busy={act.busy !== null} onPress={() => setAsking('project.cancel_run')} />
          ) : (
            <Control id="project.stop_run" params={p} view={view} label="Stop…" icon="close" busy={act.busy !== null} onPress={() => setAsking('project.stop_run')} />
          )}
        </span>
        {row.case && (
          <Control
            id="project.run_case"
            params={{ case: row.case.replace(/\\/g, '/') }}
            view={view}
            label="Run again"
            icon="play"
            variant="ghost"
            busy={act.busy === 'project.run_case'}
            onPress={() => void act.run('project.run_case', { case: row.case.replace(/\\/g, '/') })}
            // F-816 (axe label-content-name-mismatch): the name starts with the words shown
            name={`Run again: ${caseName(row.case)}`}
          />
        )}
        {row.case && (
          // F-837: the egui Studio's Re-run with fields stored: start_run's store plan (ADR 0140), the icon alone (G22)
          <Control
            id="project.queue_case"
            params={{ case: row.case.replace(/\\/g, '/'), store: 'plan' }}
            view={view}
            label="Run again with fields stored"
            icon="stored"
            variant="ghost"
            iconOnly
            busy={act.busy === 'project.queue_case'}
            onPress={() => void act.run('project.queue_case', { case: row.case.replace(/\\/g, '/'), store: 'plan' })}
            name={`Run ${caseName(row.case)} again with fields stored`}
          />
        )}
        {/* F-828 (ADR 0734): the run again from its own case snapshot and settings, the icon alone (G22) */}
        <Control
          id="project.rerun_run"
          params={p}
          view={view}
          label="Rerun as it ran"
          icon="rerun"
          variant="ghost"
          iconOnly
          busy={act.busy === 'project.rerun_run'}
          onPress={() => void act.run('project.rerun_run', p)}
          name={`Rerun ${row.id} as it ran`}
        />
      </div>
      {asking && words && (
        <Confirm
          words={words}
          onKeep={() => {
            setAsking(null);
            refocus();
          }}
          onConfirm={() => {
            const id = asking;
            setAsking(null);
            refocus();
            void act.run(id, p);
          }}
        />
      )}
      <Feedback
        request={shown}
        failure={act.last && !act.last.ok ? act.last.failure : shown?.outcome === 'refused' ? shown.failure : null}
        onDismiss={act.clear}
        onStartRunner={startRunner}
      />
    </section>
  );
}

// ---- the project home

/** The Run button of a case on the project home: a quick run of that case; the page follows the new run. */
export function CaseRunButton({ view, casePath, onResult }: { view: ProjectView; casePath: string; onResult: (r: RunActionResult) => void }) {
  const [busy, setBusy] = useState(false);
  return (
    <Control
      id="project.run_case"
      params={{ case: casePath }}
      view={view}
      label="Run"
      icon="play"
      iconOnly
      name={`Run ${caseName(casePath)}`}
      busy={busy}
      onPress={async () => {
        setBusy(true);
        try {
          onResult(await runAction('project.run_case', { case: casePath }));
        } finally {
          setBusy(false);
        }
      }}
    />
  );
}
