// F-837: what the removed egui Studio showed of runs beside its run list, rebuilt for this one (src/main/run/extras.ts
// says where each comes from):
//
// - the latest residuals of each run, in two columns headed by the engine's own equation names (u, p), from
//   run_status's `last_residual` (the last line of residuals.jsonl);
// - the selected run's report (runs/<id>/output/report.html, opened in the system's program for HTML) and its log
//   (the end of log.txt) with its events, the runner's and the solver's warnings named as warnings and shown at rest;
// - the runner's budgets (.rcfd/runner-config.json, owned by rcfd-runner): read and written by the runner itself (ADR 0733,
//   its checks and its messages), never over another writer's change (G18), and the last write undone by sending the
//   keys it set back with their values before it.
//
// G22: every control is an icon with its name as tooltip and accessible name; at rest the surfaces gain no chrome word
// (a column head is the engine's equation name, a warning speaks as an exception).
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { BUDGET_KEYS, BUDGET_KIND, type BudgetKey, type BudgetsOutcome, type RunLog, type RunnerBudgets } from '../../shared/run.js';
import { Button, IconButton, NumberField, Quiet, StateMark } from '../ui/index.js';
import { IconToggle } from '../ui/IconToggle.js';
import { useCommandHandlers } from '../commands/host.js';
import { openRunReport, runLog, runnerBudgets, setRunnerBudgets, undoRunnerBudgets } from '../workspace/door.js';
import { clock } from './model.js';
import type { RunRow } from './model.js';

// ---- residual columns

/** The equations whose latest residual the run list shows (the egui Studio's two columns), in this order. */
export const RESIDUAL_EQUATIONS = ['u', 'p'] as const;

/** The equations some run of the list has a latest residual for. */
export function residualColumns(rows: readonly RunRow[]): string[] {
  return RESIDUAL_EQUATIONS.filter((k) => rows.some((r) => r.detail?.lastResidual?.[k] !== undefined));
}

/** A residual in three significant figures (1.23e-6); the engine's full value is the tooltip. */
export const residualText = (v: number): string => v.toExponential(2);

export function ResidualValue({ row, equation }: { row: RunRow; equation: string }) {
  const v = row.detail?.lastResidual?.[equation];
  if (v === undefined) return <span className="rv-res rv-res--none" aria-label={`No residual of ${equation} yet`} />;
  return (
    <span className="rv-res" data-words="data" data-residual={equation} data-value={v} title={`${equation}: ${v} (the last line of runs/${row.id}/residuals.jsonl)`}>
      {residualText(v)}
    </span>
  );
}

// ---- the selected run's log, events and report

const isActive = (state: string): boolean => state === 'running' || state === 'queued' || state === 'paused';
/** How often an open log of an active run is read again. */
const LOG_POLL_MS = 2000;
/** Warnings shown at rest (the latest ones). */
const WARNINGS_AT_REST = 3;

export function RunLogTools({ row, extra }: { row: RunRow; extra?: ReactNode }) {
  const [open, setOpen] = useState(false);
  const [log, setLog] = useState<RunLog | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reportError, setReportError] = useState<string | null>(null);
  const seq = useRef(0);
  const read = useCallback(() => {
    const n = ++seq.current;
    void runLog(row.id).then((r) => {
      if (n !== seq.current) return;
      if (r.ok) {
        setLog(r.result);
        setError(null);
      } else setError(r.error.message);
    });
  }, [row.id]);
  // a new selection starts closed and is read; a change of state or step reads it again
  useEffect(() => {
    setOpen(false);
    setLog(null);
    setError(null);
    setReportError(null);
  }, [row.id]);
  useEffect(read, [read, row.state, row.step]);
  useEffect(() => {
    if (!open || !isActive(row.state)) return;
    const t = setInterval(read, LOG_POLL_MS);
    return () => clearInterval(t);
  }, [open, row.state, read]);
  const report = log?.report ?? null;
  const openReport = async (): Promise<string[]> => {
    setReportError(null);
    const r = await openRunReport(row.id);
    if (r.ok) return [`${r.result.path} was handed to the system’s program for HTML.`];
    setReportError(r.error.message);
    throw new Error(r.error.message);
  };
  useCommandHandlers({
    'runs.log': {
      unavailable: () => null,
      run: () => {
        setOpen((o) => !o);
        read();
      },
    },
    'runs.report': {
      unavailable: () => (report ? null : `${row.id} has no report yet: the solver writes runs/${row.id}/output/report.html with its results when the run ends.`),
      run: openReport,
    },
  });
  const warnings = (log?.events ?? []).filter((e) => e.warning);
  return (
    <>
      <div className="rv-run-tools" data-log-open={open ? 'true' : 'false'}>
        <IconToggle icon="log" label={`Log and events of ${row.id}`} isSelected={open} onChange={setOpen} className="rv-log-toggle" />
        {report && (
          <IconButton
            variant="ghost"
            icon="report"
            label={`Open the report (${report.path})`}
            onPress={() => void openReport().catch(() => undefined)}
            className="rv-report"
            data-report={report.path}
          />
        )}
        {extra}
      </div>
      {reportError && (
        <p className="rv-why rv-why--failed" role="alert" data-words="exception">
          {reportError}
        </p>
      )}
      {/* G22: a warning speaks at rest (the egui Studio's "warning: ..." lines) */}
      {!open && warnings.length > 0 && (
        <ul className="rv-warnings" data-words="exception" aria-label={`Warnings of ${row.id}`}>
          {warnings.slice(-WARNINGS_AT_REST).map((w, i) => (
            <li key={`${w.t}-${i}`} data-kind={w.kind}>
              {w.text}
            </li>
          ))}
          {warnings.length > WARNINGS_AT_REST && <li>{`and ${warnings.length - WARNINGS_AT_REST} more in the events`}</li>}
        </ul>
      )}
      {open && <RunLogBlock row={row} log={log} error={error} />}
    </>
  );
}

function RunLogBlock({ row, log, error }: { row: RunRow; log: RunLog | null; error: string | null }) {
  return (
    <section className="rv-block rv-log" aria-label={`Log and events of ${row.id}`}>
      {error && (
        <p className="rv-why rv-why--failed" role="alert" data-words="exception">
          {error}
        </p>
      )}
      {!log ? (
        <Quiet>Reading the run’s log…</Quiet>
      ) : (
        <>
          <h3 className="rv-block-title sr-only">Events</h3>
          {log.eventsError && (
            <p className="rv-why rv-why--failed" data-words="exception">
              {log.eventsError}
            </p>
          )}
          {log.events.length > 0 ? (
            <ol className="rv-events" data-words="data">
              {log.events.map((e, i) => (
                <li key={`${e.t}-${i}`} data-kind={e.kind} data-warning={e.warning ? 'true' : 'false'} className={e.warning ? 'rv-event rv-event--warning' : 'rv-event'}>
                  {e.t && <time dateTime={e.t}>{clock(e.t)}</time>}
                  <span data-words={e.warning ? 'exception' : 'data'}>{e.text}</span>
                </li>
              ))}
            </ol>
          ) : (
            <Quiet>No events yet.</Quiet>
          )}
          <h3 className="rv-log-file" data-words="data">
            {log.log ? log.log.path : `runs/${row.id}/log.txt`}
          </h3>
          {log.logError && (
            <p className="rv-why rv-why--failed" data-words="exception">
              {log.logError}
            </p>
          )}
          {log.log ? (
            <pre className="rv-log-tail" data-words="data" tabIndex={0} role="region" aria-label={`The end of ${log.log.path}`} data-cut={log.log.cut ? 'true' : 'false'}>
              {log.log.lines.join('\n')}
            </pre>
          ) : (
            !log.logError && <Quiet>The run has no log yet.</Quiet>
          )}
          {log.log?.cut && <Quiet>{`The last ${log.log.lines.length} lines of ${log.log.size} bytes.`}</Quiet>}
        </>
      )}
    </section>
  );
}

// ---- the runner's budgets

const kindWords: Readonly<Record<(typeof BUDGET_KIND)[BudgetKey], string>> = {
  positive: 'a whole number from 1',
  count: 'a whole number from 0',
  real: 'a number from 0',
};

type Draft = Readonly<Record<BudgetKey, number>>;
const draftOf = (b: RunnerBudgets): Draft => Object.fromEntries(BUDGET_KEYS.map((k) => [k, b.values[k] ?? Number.NaN])) as Draft;

export function BudgetsButton({ open, onChange }: { open: boolean; onChange: (v: boolean) => void }) {
  useCommandHandlers({ 'runs.budgets': { unavailable: () => null, run: () => onChange(!open) } });
  return <IconToggle icon="budgets" label="Runner budgets (.rcfd/runner-config.json)" isSelected={open} onChange={onChange} className="rv-budgets-toggle" />;
}

/** The runner's budgets file, project-relative, as the engine's change feed names it. */
export const BUDGETS_FILE = '.rcfd/runner-config.json';

/**
 * The runner's budgets. `fileChange` (ADR 0801 Addendum 23): the number of the change feed's latest entry about the
 * budgets file; when it moves on while the panel is open, the file is read again (with nothing typed), or the panel says
 * another writer changed it (with values typed, which are kept).
 */
export function BudgetsPanel({ onClose, fileChange = 0 }: { onClose: () => void; fileChange?: number }) {
  const [b, setB] = useState<RunnerBudgets | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [note, setNote] = useState<{ kind: 'done' | 'conflict' | 'refused' | 'undo-conflict'; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const panelRef = useRef<HTMLElement>(null);
  const noteRef = useRef<HTMLParagraphElement>(null);
  // a button that went with a note (Undo anyway, Keep it, Read again) leaves the focus nowhere: it goes to the note, or
  // to the panel, so Escape and Tab still work from the keyboard
  useEffect(() => {
    if (document.activeElement && document.activeElement !== document.body) return;
    (noteRef.current ?? panelRef.current)?.focus();
  }, [note, busy]);
  const load = useCallback(async () => {
    const r = await runnerBudgets();
    if (!r.ok) {
      setNote({ kind: 'refused', text: r.error.message });
      return;
    }
    setB(r.result);
    setDraft(draftOf(r.result));
  }, []);
  useEffect(() => {
    void load();
  }, [load]);
  // a change of the file by another writer: read it again, or say so when values are typed here
  const seenChange = useRef(fileChange);
  const typedRef = useRef(false);
  useEffect(() => {
    if (fileChange === seenChange.current) return;
    seenChange.current = fileChange;
    if (!typedRef.current) void load();
    else setNote({ kind: 'conflict', text: `${BUDGETS_FILE} was changed by another writer; the values typed here are kept. Read it again to see its values.` });
  }, [fileChange, load]);
  const outcome = (o: BudgetsOutcome, undo: boolean) => {
    if (o.kind === 'written') {
      setB(o.budgets);
      setDraft(draftOf(o.budgets));
      setNote({ kind: 'done', text: undo ? `${o.budgets.path} is as it was.` : `Written to ${o.budgets.path}; applies at the next runner start.` });
    } else if (o.kind === 'conflict') {
      setB(o.budgets);
      setNote(
        undo
          ? { kind: 'undo-conflict', text: `${o.budgets.path} was changed by another writer after this window wrote it. Undo anyway sets the keys this window wrote back to their values before it.` }
          : { kind: 'conflict', text: `${o.budgets.path} was changed by another writer since it was read: nothing was written. Read it again, then set your values.` },
      );
    } else setNote({ kind: 'refused', text: o.message });
  };
  const changes = (): Partial<Record<BudgetKey, number | null>> => {
    const out: Partial<Record<BudgetKey, number | null>> = {};
    if (!b || !draft) return out;
    // a file the runner cannot read is replaced whole: every key, empty ones at the runner's default
    if (b.needsReplace) {
      for (const k of BUDGET_KEYS) out[k] = Number.isNaN(draft[k]) ? null : draft[k];
      return out;
    }
    for (const k of BUDGET_KEYS) {
      const now = draft[k];
      const was = b.values[k];
      if (Number.isNaN(now)) {
        if (was !== undefined) out[k] = null;
      } else if (now !== was) out[k] = now;
    }
    return out;
  };
  const save = async () => {
    if (!b) return;
    setBusy(true);
    const r = await setRunnerBudgets(changes(), b.hash, b.needsReplace);
    setBusy(false);
    if (!r.ok) setNote({ kind: 'refused', text: r.error.message });
    else outcome(r.result, false);
  };
  const undo = async (force: boolean) => {
    setBusy(true);
    const r = await undoRunnerBudgets(force);
    setBusy(false);
    if (!r.ok) setNote({ kind: 'refused', text: r.error.message });
    else outcome(r.result, true);
  };
  const changed = Object.keys(changes()).length > 0;
  typedRef.current = changed;
  const differs = b?.inUse ? BUDGET_KEYS.filter((k) => b.values[k] !== undefined && b.inUse!.values[k] !== b.values[k]) : [];
  return (
    <section
      className="rv-budgets"
      aria-label="Runner budgets"
      ref={panelRef}
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <h2 className="rv-block-title sr-only">Runner budgets</h2>
      <p className="rv-budgets-file" data-words="data">
        {b ? b.path : '.rcfd/runner-config.json'}
        {b && !b.exists && <Quiet>{' · no file: the runner uses its defaults'}</Quiet>}
      </p>
      {b?.problem && (
        <p className="rv-why rv-why--failed" role="alert" data-words="exception">
          {`The runner would refuse this file: ${b.problem}`}
        </p>
      )}
      {differs.length > 0 && (
        <p className="rv-why" data-words="exception" data-differs={differs.join(',')}>
          {`The runner running since ${clock(b!.inUse!.started)} started with other values of ${differs.join(', ')}; it reads the file when it starts again.`}
        </p>
      )}
      {draft && b && (
        <div className="rv-budgets-fields">
          {BUDGET_KEYS.map((k) => (
            <NumberField
              key={k}
              label={<span data-words="data">{k}</span>}
              aria-label={k}
              value={draft[k]}
              onChange={(v) => setDraft({ ...draft, [k]: v })}
              minValue={BUDGET_KIND[k] === 'positive' ? 1 : 0}
              step={BUDGET_KIND[k] === 'real' ? 0.5 : 1}
              formatOptions={{ useGrouping: false, maximumFractionDigits: BUDGET_KIND[k] === 'real' ? 6 : 0 }}
              description={
                <>
                  {kindWords[BUDGET_KIND[k]]}
                  {b.inUse?.values[k] !== undefined && (
                    <>
                      {'; in use '}
                      <span data-words="data">{b.inUse.values[k]}</span>
                    </>
                  )}
                  {'; empty: the runner’s default'}
                </>
              }
              className="rv-budget"
            />
          ))}
        </div>
      )}
      {note && (
        <p ref={noteRef} tabIndex={-1} className={note.kind === 'done' ? 'rv-why' : 'rv-why rv-why--failed'} role={note.kind === 'done' ? 'status' : 'alert'} data-words={note.kind === 'done' ? 'data' : 'exception'} data-note={note.kind}>
          {note.kind !== 'done' && <StateMark state="failed" label="Not written" />} {note.text}
        </p>
      )}
      <div className="rv-budgets-actions">
        <Button variant="primary" onPress={() => void save()} isDisabled={busy || !changed || !b} data-budgets="write">
          {b?.needsReplace ? 'Replace the file' : 'Write'}
        </Button>
        {note?.kind === 'conflict' && <Button onPress={() => void load().then(() => setNote(null))}>Read again</Button>}
        {note?.kind === 'undo-conflict' ? (
          <>
            <Button onPress={() => void undo(true)} isDisabled={busy}>
              Undo anyway
            </Button>
            <Button variant="ghost" onPress={() => setNote(null)}>
              Keep it
            </Button>
          </>
        ) : (
          b?.undo && (
            <Button onPress={() => void undo(false)} isDisabled={busy} data-budgets="undo">
              Undo the write
            </Button>
          )
        )}
        <Button variant="ghost" onPress={onClose}>
          Close
        </Button>
      </div>
    </section>
  );
}
