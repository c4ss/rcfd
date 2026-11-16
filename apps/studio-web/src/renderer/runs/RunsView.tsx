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
import { Button, Cell, Column, Row, Select, StateMark, StatePictogram, Table, TableBody, TableHeader, TextField, markWord } from '../ui/index.js';
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
  timeNote,
  timeText,
  unknownWord,
  type RunRow,
  type Sort,
  type StateFilter,
} from './model.js';
import './runs.css';

/** The order of the state filter's options. */
const FILTER_STATES: readonly MarkState[] = ['running', 'paused', 'queued', 'failed', 'cancelled', 'converged', 'finished'];

export interface RunsViewProps {
  view: ProjectView;
  /** The selected run (kept by the project view, so it survives a switch of tab). */
  selected: string | null;
  onSelect: (id: string | null) => void;
}

export function RunsView({ view, selected, onSelect }: RunsViewProps) {
  const [filter, setFilter] = useState('');
  const [stateFilter, setStateFilter] = useState<StateFilter>('all');
  const [sort, setSort] = useState<Sort>({ column: 'case', direction: 'ascending' });
  const all = useMemo(() => runRows(view.runs.items, view.runDetails, view.runDetailErrors), [view.runs.items, view.runDetails, view.runDetailErrors]);
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
  return (
    <div className="rv">
      <section className="rv-list" aria-labelledby="rv-title">
        <div className="rv-tools">
          <h1 id="rv-title" className="rv-title">
            Runs <span className="rv-total">{view.runs.total}</span>
          </h1>
          <TextField aria-label="Filter runs by case or run id" placeholder="Filter by case or run id" value={filter} onChange={setFilter} className="rv-filter" />
          <Select aria-label="Run state" options={options} selected={stateFilter} onChange={(k) => setStateFilter(k as StateFilter)} className="rv-state" />
          <span className="rv-read">{filtered ? `${rows.length} of ${all.length} shown · ` : ''}read {clock(view.readAt)}</span>
        </div>
        {view.runs.error !== null ? (
          <div className="rv-error" role="alert">
            <StateMark state="failed" label="Failed to list the runs" />
            <span>{view.runs.error}</span>
            <Button onPress={projectActions.refresh}>Read again</Button>
          </div>
        ) : all.length === 0 ? (
          <p className="rv-empty">
            No runs yet. A run solves one case and keeps its results in <span className="rv-file">runs/</span>; the runner owns it, so it goes on when
            the Studio is closed. The Studio cannot start runs yet; Claude Code can (<span className="rv-file">start_run</span>), and so can the CLI (
            <span className="rv-file">rcfd-runner start</span>). Their runs appear here as they start.
          </p>
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
              </TableHeader>
              <TableBody
                items={rows}
                dependencies={[selected, view.runDetails]}
                renderEmptyState={() => (
                  <div className="rv-nomatch">
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
                {(r) => <RunTableRow row={r} faded={selected !== null && r.id !== selected} />}
              </TableBody>
            </Table>
          </div>
        )}
        {view.skippedRuns.length > 0 && (
          <ul className="rv-skipped">
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
      <SelectedRun row={picked} selected={selected} onClear={() => onSelect(null)} />
    </div>
  );
}

function StateCell({ row, faded }: { row: RunRow; faded: boolean }) {
  const s = shownState(row);
  return s ? <StateMark state={s} faded={faded} /> : <span className="ui-state rv-state-word">{unknownWord(row.state)}</span>;
}

function RunTableRow({ row, faded }: { row: RunRow; faded: boolean }) {
  const r = result(row);
  const failed = row.state === 'failed';
  return (
    <Row id={row.id} textValue={`${caseName(row.case)} ${row.id}`} failed={failed} faded={faded} className="rv-row">
      <Cell className="rv-cell-case">
        <span className="rv-case" data-run={row.id} data-state={row.state}>
          {caseName(row.case) || 'case not recorded'}
        </span>
        <span className="rv-id">{row.id}</span>
      </Cell>
      <Cell className="rv-cell-state">
        <StateCell row={row} faded={faded} />
      </Cell>
      <Cell className="rv-cell-result">
        <span className="rv-head" data-step={row.step}>
          {r.head}
        </span>
        <span className={failed ? 'rv-note rv-note--failed' : 'rv-note'}>{r.note}</span>
      </Cell>
    </Row>
  );
}

// ---- the selected run

function Num({ caption, children, word, wide }: { caption: string; children: ReactNode; word?: boolean; wide?: boolean }) {
  return (
    <div className={`rv-num${wide ? ' rv-num--wide' : ''}`}>
      <dt>{caption}</dt>
      <dd className={word ? 'rv-num-word' : undefined}>{children}</dd>
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

function SelectedRun({ row, selected, onClear }: { row: RunRow | null; selected: string | null; onClear: () => void }) {
  if (row === null) {
    return (
      <aside className="rv-run rv-run--none" aria-labelledby="rv-run-title">
        <h2 id="rv-run-title" className="rv-run-title">
          {selected === null ? 'No run selected' : 'The selected run is gone'}
        </h2>
        <p className="rv-empty-line">
          {selected === null
            ? 'Select a run in the table (arrow keys and Space, or a click) to bring it forward: its numbers, why it is in its state and the state changes the runner recorded. Escape clears the selection.'
            : `${selected} is no longer in the project folder: it was removed or renamed outside the Studio.`}
        </p>
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
        <h2 id="rv-run-title" className="rv-run-title">
          {caseName(row.case) || 'case not recorded'}
        </h2>
        {s ? <StateMark state={s} /> : <span className="ui-state">{unknownWord(row.state)}</span>}
        <span className="rv-id">{row.id}</span>
        <Button variant="ghost" icon="close" onPress={onClear} className="rv-clear">
          Clear selection
        </Button>
      </header>
      {row.detailError !== null && (
        <div className="rv-error" role="alert">
          <StateMark state="failed" label="Failed to read the run’s status" />
          <span>{row.detailError}</span>
          <Button onPress={projectActions.refresh}>Read again</Button>
        </div>
      )}
      <dl className="rv-grid" aria-label={`Numbers of ${row.id}`}>
        <Num caption={steady === true ? 'Iterations' : 'Steps'}>{row.step}</Num>
        <Num caption="Time, s" word={timeNote(row) !== null}>
          {timeText(row)}
        </Num>
        <Num caption="Solver time, s" word={d?.finish?.elapsed == null}>
          {d?.finish?.elapsed != null ? plain(d.finish.elapsed, 2) : ended ? 'not reported' : 'not finished'}
        </Num>
        <Num caption="Solver process" word={d?.pid == null}>
          {d?.pid ?? (d ? 'none' : '…')}
        </Num>
        <Num caption="Exit code" word={d?.exitCode == null}>
          {d?.exitCode ?? (ended ? 'none' : 'not ended')}
        </Num>
        <Num caption="Updated" word>
          {d ? (sameDay(d.updated) ? clock(d.updated) : dateTime(d.updated)) : '…'}
        </Num>
        {reports.map(([k, v]) => (
          <Num key={k} caption={k}>
            <span title={String(v)}>{report(v)}</span>
          </Num>
        ))}
      </dl>
      {reports.length > 0 && (
        <p className="rv-caption">
          {reports.length === 1 ? 'The report value' : 'The report values'} of the case, as the runner recorded them in metrics.json; the engine gives
          no unit for them.
        </p>
      )}
      <section className="rv-block" aria-labelledby="rv-why">
        <h3 id="rv-why" className="rv-block-title">
          {ended ? 'Why it ended' : row.state === 'queued' ? 'Why it is waiting' : `Why it is ${s ? markWord(s).toLowerCase() : row.state}`}
        </h3>
        <p className={row.state === 'failed' ? 'rv-why rv-why--failed' : 'rv-why'}>
          {d?.reason ?? (d ? (row.state === 'running' ? 'Launched by the runner; the solver is working.' : 'The runner recorded no reason.') : 'Reading the run’s status…')}
        </p>
      </section>
      <section className="rv-block" aria-labelledby="rv-history">
        <h3 id="rv-history" className="rv-block-title">
          State changes the runner recorded
        </h3>
        {d && d.history.length > 0 ? (
          <ol className="rv-history">
            {d.history.map((h, i) => (
              <li key={`${h.t}-${i}`}>
                <time dateTime={h.t}>{clock(h.t)}</time>
                <span className="rv-history-to">{h.from === null ? `created ${h.to}` : `${h.from} to ${h.to}`}</span>
                {h.reason && <span className="rv-history-why">{h.reason}</span>}
              </li>
            ))}
          </ol>
        ) : (
          <p className="rv-empty-line">{d ? 'None among the run’s last events.' : 'Reading the run’s events…'}</p>
        )}
      </section>
      <p className="rv-source">
        Created {dateTime(row.created)}
        {d ? `, solver ${d.solverVersion}. Read through the engine (run_status of runs/${row.id}) at ${clock(d.readAt)}.` : '.'}
      </p>
    </aside>
  );
}
