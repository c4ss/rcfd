import { useMemo, useState } from 'react';
import {
  Cell, Column, Row, SearchField, Input, Select, SelectValue, Button, Popover, ListBox, ListBoxItem, Label,
  Table, TableBody, TableHeader, type Key, type SortDescriptor,
} from 'react-aria-components';
import { RUNS, STATE_WORD, type Run, type RunState } from './data';
import { Pictogram } from './Pictogram';

interface Props {
  selected: string;
  onSelect: (id: string) => void;
  live: { iteration: number; time: number };
}

const ORDER: Record<RunState, number> = { running: 0, failed: 1, queued: 2, converged: 3, finished: 4 };

export function RunsPanel({ selected, onSelect, live }: Props) {
  const [filter, setFilter] = useState('');
  const [stateFilter, setStateFilter] = useState<Key>('all');
  const [sort, setSort] = useState<SortDescriptor>({ column: 'case', direction: 'ascending' });

  const rows = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const list = RUNS.filter((r) => (stateFilter === 'all' || r.state === stateFilter)
      && (!q || r.caseName.includes(q) || r.id.toLowerCase().includes(q)));
    const dir = sort.direction === 'descending' ? -1 : 1;
    return [...list].sort((a, b) => {
      if (sort.column === 'state') return dir * (ORDER[a.state] - ORDER[b.state]);
      if (sort.column === 'result') return dir * (a.iterations - b.iterations);
      return dir * a.caseName.localeCompare(b.caseName);
    });
  }, [filter, stateFilter, sort]);

  return (
    <div className="runs">
      <div className="runs-bar">
        <SearchField aria-label="Filter runs by case or id" value={filter} onChange={setFilter} className="search">
          <Input placeholder="Filter runs by case or id" />
        </SearchField>
        <Select aria-label="Run state" selectedKey={stateFilter} onSelectionChange={(k) => k != null && setStateFilter(k)} className="select">
          <Label className="sr-only">Run state</Label>
          <Button className="select-button"><SelectValue /><span className="caret" aria-hidden="true" /></Button>
          <Popover className="popover">
            <ListBox className="listbox">
              <ListBoxItem id="all">All states</ListBoxItem>
              {(Object.keys(STATE_WORD) as RunState[]).map((s) => (
                <ListBoxItem key={s} id={s} textValue={STATE_WORD[s]}><Pictogram state={s} /> {STATE_WORD[s]}</ListBoxItem>
              ))}
            </ListBox>
          </Popover>
        </Select>
      </div>

      <Table
        aria-label="Runs in wind-tunnel"
        className="runs-table"
        selectionMode="single"
        selectionBehavior="replace"
        disallowEmptySelection
        selectedKeys={[selected]}
        onSelectionChange={(keys) => { const k = [...keys][0]; if (k) onSelect(String(k)); }}
        sortDescriptor={sort}
        onSortChange={setSort}
      >
        <TableHeader>
          <Column id="case" isRowHeader allowsSorting>{({ sortDirection }) => <SortHead label="Case and run" dir={sortDirection} />}</Column>
          <Column id="state" allowsSorting>{({ sortDirection }) => <SortHead label="State" dir={sortDirection} />}</Column>
          <Column id="result" allowsSorting>{({ sortDirection }) => <SortHead label="Result" dir={sortDirection} />}</Column>
        </TableHeader>
        <TableBody dependencies={[live.iteration, selected]} renderEmptyState={() => <p className="empty">No runs match this filter.</p>} items={rows}>
          {(run) => <RunRow run={run} isFocus={run.id === selected} live={live} />}
        </TableBody>
      </Table>
    </div>
  );
}

function SortHead({ label, dir }: { label: string; dir?: 'ascending' | 'descending' }) {
  return (
    <span className="sort-head">
      {label}
      <svg width="8" height="10" viewBox="0 0 8 10" aria-hidden="true" className={dir ? 'on' : ''}>
        {dir !== 'descending' && <path d="M1 4L4 1L7 4" />}
        {dir !== 'ascending' && <path d="M1 6L4 9L7 6" />}
      </svg>
    </span>
  );
}

function RunRow({ run, isFocus, live }: { run: Run; isFocus: boolean; live: { iteration: number; time: number } }) {
  // Selection pulls the chosen run forward; others fade. A failure never fades (guardrail G6).
  const faded = !isFocus && run.state !== 'failed';
  const result = run.state === 'running'
    ? <><b>it {live.iteration}</b><small>t = {live.time.toFixed(2)} s of 20.00 s</small></>
    : run.state === 'queued' ? <small>{run.note}</small>
    : run.state === 'failed' ? <><b>{run.iterations} it</b><small>{run.note}</small></>
    : <><b>{run.iterations} it · {run.wallS} s</b><small>{run.note}</small></>;
  return (
    <Row id={run.id} className={`run-row state-${run.state}${faded ? ' faded' : ''}`}>
      <Cell><b className="case">{run.caseName}</b><small className="rid">{run.id}</small></Cell>
      <Cell><span className="state"><Pictogram state={run.state} />{STATE_WORD[run.state]}</span></Cell>
      <Cell className="react-aria-Cell result">{result}</Cell>
    </Row>
  );
}
