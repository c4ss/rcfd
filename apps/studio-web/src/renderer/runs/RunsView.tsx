// The run list and run state (F-806; surface brief .impeccable/surfaces/web-src-renderer-runs-runsview-tsx.md): the
// runs table of DESIGN.md (case and run, state pictogram and word, result; sortable, filtered by text and state;
// selection pulls one run forward and fades the others by colour, never a failure) and, beside it, the selected run's
// numbers and the state changes the runner recorded.
//
// Everything shown is the engine's answer as the main process pushes it on the project port (list_runs and run_status
// through the engine's project server; ../project/door.ts): the runner owns the runs (G7) and the page keeps nothing of
// them but the selection, the sort and the filter (G12).
import { useMemo, useState, type ReactNode } from 'react';
import type { SortDescriptor } from 'react-aria-components';
import type { ProjectView } from '../../shared/project.js';
import { Button, Cell, Column, Dash, EmptyMark, IconButton, Quiet, Row, Select, StateMark, StatePictogram, Table, TableBody, TableHeader, TextField, markWord } from '../ui/index.js';
import type { MarkState } from '../ui/index.js';
import { dateTime } from '../project/format.js';
import { projectActions } from '../project/door.js';
import {
  caseName,
  clock,
  filterRows,
  plain,
  result,
  runRows,
  shownState,
  sortRows,
  stateCounts,
  timeIsQuiet,
  timeNote,
  timeText,
  unknownWord,
  type RunRow,
  type Sort,
  type StateFilter,
} from './model.js';
import { pendingByRun, requestWords, withStartedRuns } from './control-model.js';
import { RunBar, SelectedRunControls } from './RunControls.js';
import type { RunRequest } from '../../shared/project.js';
import { Provenance, reportValue, statusValue } from '../provenance/index.js';
import { BudgetsButton, BudgetsPanel, ResidualValue, residualColumns } from './RunExtras.js';
import { HowItRanTools } from './HowItRan.js';
import './runs.css';

/** The order of the state filter's options. */
const FILTER_STATES: readonly MarkState[] = ['running', 'paused', 'queued', 'failed', 'cancelled', 'converged', 'finished'];

export interface RunsViewProps {
  view: ProjectView;
  /** The selected run (kept by the project view, so it survives a switch of tab). */
  selected: string | null;
  onSelect: (id: string | null) => void;
  /** F-810: opens the run workspace on the selected run (absent: no such button). */
  onOpenWorkspace?: () => void;
}

export function RunsView({ view, selected, onSelect, onOpenWorkspace }: RunsViewProps) {
  const [filter, setFilter] = useState('');
  const [stateFilter, setStateFilter] = useState<StateFilter>('all');
  const [sort, setSort] = useState<Sort>({ column: 'case', direction: 'ascending' });
  // F-807: a run this window started shows at once, from start_run's answer, until the list holds its record
  const items = useMemo(() => withStartedRuns(view.runs.items, view.runRequests), [view.runs.items, view.runRequests]);
  const all = useMemo(() => runRows(items, view.runDetails, view.runDetailErrors), [items, view.runDetails, view.runDetailErrors]);
  const pending = useMemo(() => pendingByRun(view.runRequests), [view.runRequests]);
  const rows = useMemo(() => sortRows(filterRows(all, filter, stateFilter), sort), [all, filter, stateFilter, sort]);
  const counts = useMemo(() => stateCounts(all), [all]);
  const picked = selected === null ? null : (all.find((r) => r.id === selected) ?? null);
  const name = view.info?.name ?? view.folder;
  const options = [
    { id: 'all', label: `All states, ${all.length}` },
    ...FILTER_STATES.map((s) => ({ id: s, label: `${markWord(s)}, ${counts.get(s) ?? 0}`, lead: <StatePictogram state={s} /> })),
    ...(counts.has('other') ? [{ id: 'other', label: `Other, ${counts.get('other')}` }] : []),
  ];
  const filtered = filter.trim() !== '' || stateFilter !== 'all';
  // F-837: the latest residuals of the engine's equations u and p, when some run has them
  const resCols = useMemo(() => residualColumns(all), [all]);
  const [budgetsOpen, setBudgetsOpen] = useState(false);
  return (
    <div className="rv">
      <section className="rv-list" aria-labelledby="rv-title">
        <RunBar view={view} onStarted={onSelect} lead={<BudgetsButton open={budgetsOpen} onChange={setBudgetsOpen} />} />
        {budgetsOpen && <BudgetsPanel onClose={() => setBudgetsOpen(false)} />}
        <div className="rv-tools">
          <h1 id="rv-title" className="rv-title sr-only">
            Runs <span className="rv-total" data-words="data">{view.runs.total}</span>
          </h1>
          <TextField aria-label="Filter runs by case or run id" placeholder="Filter" value={filter} onChange={setFilter} className="rv-filter" />
          <Select aria-label="Run state" options={options} selected={stateFilter} onChange={(k) => setStateFilter(k as StateFilter)} className="rv-state" />
          {/* G22: a filter's count is shown; the time of the reading is said to readers */}
          <span className="rv-read">
            {filtered ? <span data-words="data">{`${rows.length} of ${all.length} shown`}</span> : ''}
            <Quiet>{` · read ${clock(view.readAt)}`}</Quiet>
          </span>
        </div>
        {view.runs.error !== null ? (
          <div className="rv-error" role="alert" data-words="exception">
            <StateMark state="failed" label="Failed to list the runs" />
            <span>{view.runs.error}</span>
            <Button onPress={projectActions.refresh}>Read again</Button>
          </div>
        ) : all.length === 0 ? (
          <EmptyMark
            className="rv-empty"
            text="No runs yet. A run solves one case and keeps its results in runs/; the runner owns it, so it goes on when the Studio is closed. Choose a case above and Run it, or Queue it with a priority. Claude Code (start_run) and the CLI (rcfd-runner start) start the same runs; theirs appear here as they start."
          />
        ) : (
          <div
            className="rv-scroll"
            onKeyDownCapture={(e) => {
              // Escape clears the selection; the table itself does not (disallowEmptySelection, so that Space selects and
              // never toggles the selected run off), and React Aria stops the key's propagation, hence the capture phase
              if (e.key === 'Escape' && selected !== null) onSelect(null);
            }}
          >
            <Table
              aria-label={`Runs of ${name}`}
              className="rv-table"
              selectionMode="single"
              selectionBehavior="replace"
              disallowEmptySelection
              selectedKeys={selected === null ? [] : [selected]}
              onSelectionChange={(keys) => {
                if (keys === 'all') return;
                const k = [...keys][0];
                onSelect(k === undefined ? null : String(k));
              }}
              sortDescriptor={sort as SortDescriptor}
              onSortChange={(d) => setSort({ column: d.column as Sort['column'], direction: d.direction })}
            >
              <TableHeader>
                <Column id="case" isRowHeader allowsSorting className="rv-col-case">
                  Case and run
                </Column>
                <Column id="state" allowsSorting className="rv-col-state">
                  State
                </Column>
                <Column id="result" allowsSorting className="rv-col-result">
                  Result
                </Column>
                {resCols.map((k) => (
                  <Column key={k} id={`res-${k}`} className="rv-col-res" textValue={`Last residual of ${k}`}>
                    <span className="sr-only">Last residual of </span>
                    <span data-words="data">{k}</span>
                  </Column>
                ))}
              </TableHeader>
              <TableBody
                items={rows}
                dependencies={[selected, view.runDetails, view.runRequests, resCols]}
                renderEmptyState={() => (
                  <div className="rv-nomatch" data-words="exception">
                    <p>No run matches this filter.</p>
                    <Button
                      variant="ghost"
                      onPress={() => {
                        setFilter('');
                        setStateFilter('all');
                      }}
                    >
                      Clear the filter
                    </Button>
                  </div>
                )}
              >
                {(r) => <RunTableRow row={r} faded={selected !== null && r.id !== selected} pending={pending.get(r.id) ?? null} resCols={resCols} />}
              </TableBody>
            </Table>
          </div>
        )}
        {view.skippedRuns.length > 0 && (
          <ul className="rv-skipped" data-words="exception">
            {view.skippedRuns.map((s) => (
              <li key={s.path}>
                <StateMark state="failed" label="Failed to read" />
                <span className="rv-file">{s.path}</span>
                <span>{s.message}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
      <SelectedRun row={picked} selected={selected} onClear={() => onSelect(null)} view={view} onSelect={onSelect} onOpenWorkspace={onOpenWorkspace} />
    </div>
  );
}

function StateCell({ row, faded }: { row: RunRow; faded: boolean }) {
  const s = shownState(row);
  return (
    <span data-words={row.state === 'failed' ? 'exception' : 'chrome'}>
      {s ? <StateMark state={s} faded={faded} quiet /> : <span className="ui-state rv-state-word">{unknownWord(row.state)}</span>}
    </span>
  );
}

function RunTableRow({ row, faded, pending, resCols }: { row: RunRow; faded: boolean; pending: RunRequest | null; resCols: readonly string[] }) {
  const r = result(row);
  const failed = row.state === 'failed';
  return (
    <Row id={row.id} textValue={`${caseName(row.case)} ${row.id}`} failed={failed} faded={faded} className="rv-row">
      <Cell className="rv-cell-case">
        <span className="rv-case" data-run={row.id} data-state={row.state} data-words="data">
          {caseName(row.case) || 'case not recorded'}
        </span>
        <span className="rv-id" data-words="data">
          {row.id}
        </span>
      </Cell>
      <Cell className="rv-cell-state">
        <StateCell row={row} faded={faded} />
      </Cell>
      <Cell className="rv-cell-result">
        <span className="rv-head" data-step={row.step} data-words="data">
          {row.state === 'queued' || row.step === 0 ? (
            r.head
          ) : (
            // F-828: not a tab stop (the rows take the arrow keys); a click, or Ctrl+I on the focused row, opens it
            <Provenance
              tabStop={false}
              subject={{ kind: 'number', source: statusValue({ run: row.id, quantity: row.detail?.steady === true ? 'iterations' : 'steps', shown: r.head, value: row.step, unit: { why: 'a count of the solver’s steps, status.json step' } }) }}
            >
              {r.head}
            </Provenance>
          )}
        </span>
        {/* G22: a failure's or a stop's note is shown; a normal state's note is said to readers and in the tooltip */}
        {failed || row.state === 'cancelled' ? (
          <span className={failed ? 'rv-note rv-note--failed' : 'rv-note'} data-words="exception">
            {r.note}
          </span>
        ) : (
          <Quiet>{` ${r.note}`}</Quiet>
        )}
        {pending && (
          // F-807: what this window asked of the runner and has not seen it do yet (the state cell stays the runner's)
          <span className="rc-pending" data-request-seq={pending.seq} data-answered-at={pending.answeredAt} data-slow={pending.slow ? 'true' : 'false'}>
            {requestWords(pending)}
          </span>
        )}
      </Cell>
      {resCols.map((k) => (
        <Cell key={k} className="rv-cell-res">
          <ResidualValue row={row} equation={k} />
        </Cell>
      ))}
    </Row>
  );
}

// ---- the selected run

function Num({ caption, children, word, wide, named }: { caption: string; children: ReactNode; word?: boolean; wide?: boolean; named?: boolean }) {
  return (
    <div className={`rv-num${wide ? ' rv-num--wide' : ''}`}>
      <dt data-words={named ? 'data' : undefined}>{caption}</dt>
      <dd className={word ? 'rv-num-word' : undefined} data-words={word ? 'chrome' : 'data'}>
        {children}
      </dd>
    </div>
  );
}

/** Whether an RFC 3339 time is today, local time (then the clock alone says it). */
const sameDay = (iso: string): boolean => new Date(iso).toDateString() === new Date().toDateString();

/** Report values: six significant figures on screen, the engine's full value on hover. */
const report = (v: number): string => {
  const s = v.toPrecision(6);
  return s.includes('e') ? s : String(Number(s));
};

function SelectedRun({
  row,
  selected,
  onClear,
  view,
  onSelect,
  onOpenWorkspace,
}: {
  row: RunRow | null;
  selected: string | null;
  onClear: () => void;
  view: ProjectView;
  onSelect: (id: string | null) => void;
  onOpenWorkspace?: () => void;
}) {
  if (row === null) {
    return (
      <aside className="rv-run rv-run--none" aria-labelledby="rv-run-title">
        {/* G22: nothing selected is the normal state and says nothing; a selected run that is gone speaks */}
        <h2 id="rv-run-title" className={selected === null ? 'rv-run-title sr-only' : 'rv-run-title'} data-words={selected === null ? undefined : 'exception'}>
          {selected === null ? 'No run selected' : 'The selected run is gone'}
        </h2>
        {selected === null ? (
          <EmptyMark text="Select a run in the table (arrow keys and Space, or a click) to bring it forward: its numbers, why it is in its state and the state changes the runner recorded. Escape clears the selection." />
        ) : (
          <p className="rv-empty-line" data-words="exception">
            {`${selected} is no longer in the project folder: it was removed or renamed outside the Studio.`}
          </p>
        )}
        {selected !== null && <Button onPress={onClear}>Clear the selection</Button>}
      </aside>
    );
  }
  const d = row.detail;
  const s = shownState(row);
  const steady = d?.steady ?? null;
  const ended = row.state === 'finished' || row.state === 'failed' || row.state === 'cancelled';
  const reports = Object.entries(row.metrics);
  return (
    <aside className="rv-run" aria-labelledby="rv-run-title" data-run={row.id}>
      <header className="rv-run-head">
        <h2 id="rv-run-title" className="rv-run-title" data-words="data">
          {caseName(row.case) || 'case not recorded'}
        </h2>
        <span data-words={row.state === 'failed' ? 'exception' : 'chrome'}>{s ? <StateMark state={s} quiet /> : <span className="ui-state">{unknownWord(row.state)}</span>}</span>
        <span className="rv-id" data-words="data">
          <Provenance runOpener subject={{ kind: 'run', run: row.id }}>
            {row.id}
          </Provenance>
        </span>
        <IconButton variant="ghost" icon="close" label="Clear selection" onPress={onClear} className="rv-clear" />
      </header>
      {onOpenWorkspace && (
        <div className="rv-open-workspace">
          <Button onPress={onOpenWorkspace}>Open in workspace</Button>
        </div>
      )}
      {row.detailError !== null && (
        <div className="rv-error" role="alert" data-words="exception">
          <StateMark state="failed" label="Failed to read the run’s status" />
          <span>{row.detailError}</span>
          <Button onPress={projectActions.refresh}>Read again</Button>
        </div>
      )}
      <SelectedRunControls view={view} row={row} onStarted={onSelect} />
      {/* Log and events, the report and How this ran (F-828, ADR 0734): one row of icons */}
      <HowItRanTools row={row} />
      <dl className="rv-grid" aria-label={`Numbers of ${row.id}`}>
        <Num caption={steady === true ? 'Iterations' : 'Steps'}>
          <Provenance subject={{ kind: 'number', source: statusValue({ run: row.id, quantity: steady === true ? 'iterations' : 'steps', shown: String(row.step), value: row.step, unit: { why: 'a count of the solver’s steps, status.json step' } }) }}>
            {row.step}
          </Provenance>
        </Num>
        {/* ADR 0734 P2: the unit run_status names; the caption kept its s before the engine named one */}
        <Num caption={`Time, ${d?.timeUnit ?? 's'}`} word={timeNote(row) !== null}>
          {timeIsQuiet(timeNote(row)) ? (
            <Dash words={timeText(row)} />
          ) : timeNote(row) !== null ? (
            <span data-words="exception">{timeText(row)}</span>
          ) : (
            <Provenance
              subject={{
                kind: 'number',
                source: statusValue({ run: row.id, quantity: 'time', shown: timeText(row), value: row.time, unit: 'time' }),
              }}
            >
              {timeText(row)}
            </Provenance>
          )}
        </Num>
        <Num caption="Solver time, s" word={d?.finish?.elapsed == null}>
          {d?.finish?.elapsed != null ? (
            <Provenance
              subject={{
                kind: 'number',
                source: statusValue({
                  run: row.id,
                  quantity: 'solver time',
                  shown: plain(d.finish.elapsed, 2),
                  value: d.finish.elapsed,
                  unit: { unit: 's', from: 'the key elapsed_s of the solver’s finish event' },
                  file: `runs/${row.id}/events.jsonl`,
                  readBy: 'run_status (rcfd-mcp, read-only), the solver’s finish event',
                }),
              }}
            >
              {plain(d.finish.elapsed, 2)}
            </Provenance>
          ) : ended ? (
            <span data-words="exception">not reported</span>
          ) : (
            <Dash words="not finished" />
          )}
        </Num>
        <Num caption="Solver process" word={d?.pid == null}>
          {d?.pid != null ? (
            <Provenance subject={{ kind: 'number', source: statusValue({ run: row.id, quantity: 'solver process', shown: String(d.pid), value: d.pid, unit: { why: 'a process id, status.json pid' } }) }}>
              {d.pid}
            </Provenance>
          ) : d ? (
            <Dash words="none" />
          ) : (
            '…'
          )}
        </Num>
        <Num caption="Exit code" word={d?.exitCode == null}>
          {d?.exitCode != null ? (
            <Provenance subject={{ kind: 'number', source: statusValue({ run: row.id, quantity: 'exit code', shown: String(d.exitCode), value: d.exitCode, unit: { why: 'the solver’s exit code, status.json exit_code' } }) }}>
              {d.exitCode}
            </Provenance>
          ) : (
            <Dash words={ended ? 'none' : 'not ended'} />
          )}
        </Num>
        <Num caption="Updated" word>
          <span data-words="data">{d ? (sameDay(d.updated) ? clock(d.updated) : dateTime(d.updated)) : '…'}</span>
        </Num>
        {reports.map(([k, v]) => (
          <Num key={k} caption={k} named>
            <span title={String(v)}>
              <Provenance subject={{ kind: 'number', source: reportValue({ run: row.id, name: k, shown: report(v), value: v }) }}>{report(v)}</Provenance>
            </span>
          </Num>
        ))}
      </dl>
      {reports.length > 0 && (
        // a known miss stays visible (G6, G22): the engine gives these no unit; where they come from is said to readers
        <p className="rv-caption" data-words="exception">
          <Quiet>{reports.length === 1 ? 'The report value' : 'The report values'} of the case, as the runner recorded them in metrics.json; </Quiet>
          No unit from the engine.
        </p>
      )}
      <section className="rv-block" aria-labelledby="rv-why">
        <h3 id="rv-why" className="rv-block-title sr-only">
          {ended ? 'Why it ended' : row.state === 'queued' ? 'Why it is waiting' : `Why it is ${s ? markWord(s).toLowerCase() : row.state}`}
        </h3>
        {/* G22: the runner's reason is data and stays; the Studio's own words for a normal state are said to readers */}
        {d?.reason || row.state === 'failed' ? (
          <p className={row.state === 'failed' ? 'rv-why rv-why--failed' : 'rv-why'} data-words={row.state === 'failed' ? 'exception' : 'data'}>
            {d?.reason ?? (d ? 'The runner recorded no reason.' : 'Reading the run’s status…')}
          </p>
        ) : (
          <Quiet>{d ? (row.state === 'running' ? 'Launched by the runner; the solver is working.' : 'The runner recorded no reason.') : 'Reading the run’s status…'}</Quiet>
        )}
      </section>
      <section className="rv-block" aria-labelledby="rv-history">
        <h3 id="rv-history" className="rv-block-title sr-only">
          State changes the runner recorded
        </h3>
        {d && d.history.length > 0 ? (
          <ol className="rv-history" data-words="data">
            {d.history.map((h, i) => (
              <li key={`${h.t}-${i}`}>
                <time dateTime={h.t}>{clock(h.t)}</time>
                <span className="rv-history-to">{h.from === null ? `created ${h.to}` : `${h.from} to ${h.to}`}</span>
                {h.reason && <span className="rv-history-why">{h.reason}</span>}
              </li>
            ))}
          </ol>
        ) : (
          <Quiet>{d ? 'None among the run’s last events.' : 'Reading the run’s events…'}</Quiet>
        )}
      </section>
      {/* G22: where the numbers come from is said to readers and one step away in the provenance (F-828) */}
      <p className="rv-source sr-only">
        Created <span data-words="data">{dateTime(row.created)}</span>
        {d ? (
          <>
            , solver <span data-words="data">{d.solverVersion}</span>. Read through the engine (run_status of runs/<span data-words="data">{row.id}</span>) at{' '}
            <span data-words="data">{clock(d.readAt)}</span>.
          </>
        ) : (
          '.'
        )}
      </p>
    </aside>
  );
}
