// The primitives gallery (F-802): every primitive of DESIGN.md Components in its states, and the tokens, on one page. For development and the maintainer's view-and-report (npm run gallery, or the screenshots that
// tests/e2e/primitives.spec.ts writes); reached at #/dev/primitives. The keyboard tests drive this page.
//
// Every label here is a sample: the page shows controls, not solver results (G6), and says so in its footer.
import { useMemo, useState, type ReactNode } from 'react';
import type { SortDescriptor } from 'react-aria-components';
import { palette, role, typeRamp, type PaletteName, type TypeName } from '../design/tokens.js';
import {
  Button,
  Cell,
  Column,
  Icon,
  IconButton,
  ICON_NAMES,
  NumberField,
  Row,
  RUN_STATES,
  SegmentedToggle,
  Select,
  Slider,
  StateMark,
  StatePictogram,
  STATE_WORD,
  Switch,
  Table,
  TableBody,
  TableHeader,
  Tabs,
  TextField,
  Toggle,
  type RunState,
} from '../ui/index.js';
import './gallery.css';

const plural = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`;

function Section({ id, title, wide, span2, children }: { id: string; title: string; wide?: boolean; span2?: boolean; children: ReactNode }) {
  const cls = `gx-section${wide ? ' gx-section--wide' : ''}${span2 ? ' gx-section--span2' : ''}`;
  return (
    <section className={cls} aria-labelledby={`${id}-title`}>
      <h2 id={`${id}-title`} className="gx-head">
        {title}
      </h2>
      <div className="gx-body">{children}</div>
    </section>
  );
}

function Readout({ id, children }: { id: string; children: ReactNode }) {
  return (
    <p className="gx-readout" data-testid={id} aria-live="polite">
      {children}
    </p>
  );
}

// ---- buttons -------------------------------------------------------------------------------------------------

function Buttons() {
  const [presses, setPresses] = useState(0);
  const [last, setLast] = useState('none yet');
  const press = (name: string) => () => {
    setPresses((n) => n + 1);
    setLast(name);
  };
  return (
    <Section id="buttons" title="Buttons">
      <div className="gx-row">
        <Button onPress={press('Default')}>Default</Button>
        <Button variant="primary" icon="plus" onPress={press('Primary')}>
          Primary
        </Button>
        <Button icon="play" onPress={press('With icon')}>
          With icon
        </Button>
        <IconButton icon="send" label="Icon only" onPress={press('Icon only')} />
        <Button variant="chip" onPress={press('Chip')}>
          Chip
        </Button>
        <Button isDisabled>Disabled</Button>
      </div>
      <div className="gx-band" role="group" aria-label="On the header band">
        <Button variant="ghost" onPress={press('Ghost')}>
          Ghost
        </Button>
      </div>
      <Readout id="buttons-readout">
        {plural(presses, 'press', 'presses')}; last: {last}
      </Readout>
    </Section>
  );
}

// ---- segmented toggles -----------------------------------------------------------------------------------------

const SEGMENTS = [
  { id: 'one', label: 'Sample one' },
  { id: 'two', label: 'Sample two' },
  { id: 'three', label: 'Sample three' },
] as const;

function Segmented() {
  const [selected, setSelected] = useState<string>('one');
  const [follow, setFollow] = useState(false);
  return (
    <Section id="segmented" title="Segmented toggles">
      <SegmentedToggle
        aria-label="Sample view"
        options={SEGMENTS}
        selected={selected}
        onChange={setSelected}
        unavailable={[{ label: 'Sample four', reason: 'Unavailable here: the tooltip says why an option cannot be used.' }]}
      />
      <div className="gx-row">
        <Toggle isSelected={follow} onChange={setFollow}>
          Sample toggle
        </Toggle>
      </div>
      <Readout id="segmented-readout">
        Selected: {SEGMENTS.find((s) => s.id === selected)?.label}; toggle {follow ? 'on' : 'off'}
      </Readout>
    </Section>
  );
}

// ---- fields ----------------------------------------------------------------------------------------------------

function Fields() {
  const [text, setText] = useState('');
  const [num, setNum] = useState(2.5);
  return (
    <Section id="fields" title="Inputs and number fields">
      <TextField
        label="Sample text"
        placeholder="Type a sample word"
        description="Help in place, from the engine's documentation in the product (G19)."
        value={text}
        onChange={setText}
        className="gx-wide-field"
      />
      <NumberField
        label="Sample length, m"
        value={num}
        onChange={setNum}
        minValue={0}
        maxValue={10}
        step={0.5}
        formatOptions={{ minimumFractionDigits: 2, maximumFractionDigits: 2 }}
        className="gx-number-field"
      />
      <TextField label="Sample invalid" defaultValue="x" isInvalid errorMessage="Sample error: name the problem and how to recover." className="gx-wide-field" />
      <Readout id="fields-readout">
        Text: {text === '' ? '(empty)' : text}; length {num.toFixed(2)}
      </Readout>
    </Section>
  );
}

// ---- select ----------------------------------------------------------------------------------------------------

const STATE_OPTIONS = [
  { id: 'all', label: 'All states' },
  ...RUN_STATES.map((s) => ({ id: s, label: STATE_WORD[s], lead: <StatePictogram state={s} /> })),
];

function Selects() {
  const [state, setState] = useState('all');
  const [band, setBand] = useState('a');
  return (
    <Section id="select" title="Select">
      <Select label="Sample state filter" options={STATE_OPTIONS} selected={state} onChange={setState} />
      <div className="gx-band">
        <Select
          aria-label="Sample select on the header band"
          onBand
          options={[
            { id: 'a', label: 'Sample A' },
            { id: 'b', label: 'Sample B' },
            { id: 'c', label: 'Sample C (unavailable)', isDisabled: true },
          ]}
          selected={band}
          onChange={setBand}
        />
      </div>
      <Readout id="select-readout">Filter: {STATE_OPTIONS.find((o) => o.id === state)?.label}</Readout>
    </Section>
  );
}

// ---- switch and slider -----------------------------------------------------------------------------------------

function Switches() {
  const [on, setOn] = useState(false);
  return (
    <Section id="switch" title="Switch">
      <Switch isSelected={on} onChange={setOn} description="The knob's position carries the state; the fill reinforces it.">
        Sample switch
      </Switch>
      <Switch defaultSelected isDisabled>
        Sample switch, disabled
      </Switch>
      <Readout id="switch-readout">Switch {on ? 'on' : 'off'}</Readout>
    </Section>
  );
}

function Sliders() {
  const [v, setV] = useState(40);
  return (
    <Section id="slider" title="Slider">
      <Slider label="Sample position, %" minValue={0} maxValue={100} step={1} value={v} onChange={setV} />
      <Readout id="slider-readout">Position {v}</Readout>
    </Section>
  );
}

// ---- tabs ------------------------------------------------------------------------------------------------------

function SubTabs() {
  const [tab, setTab] = useState('one');
  const items = [
    { id: 'one', label: 'Sample tab one', panel: <p className="gx-panel">The first sample panel.</p> },
    { id: 'two', label: 'Sample tab two', panel: <p className="gx-panel">The second sample panel.</p> },
    { id: 'three', label: 'Sample tab three', panel: <p className="gx-panel">The third sample panel.</p> },
  ];
  return (
    <Section id="tabs" title="Sub-tabs" span2>
      <Tabs aria-label="Sample sub-tabs" items={items} selected={tab} onChange={setTab} />
      <Readout id="tabs-readout">Tab: {items.find((i) => i.id === tab)?.label}</Readout>
    </Section>
  );
}

// ---- table -----------------------------------------------------------------------------------------------------

interface SampleRow {
  id: string;
  name: string;
  state: RunState;
  note: string;
}

const ROWS: readonly SampleRow[] = [
  { id: 'row-c', name: 'Sample row C', state: 'running', note: 'Sample note' },
  { id: 'row-a', name: 'Sample row A', state: 'converged', note: 'Sample note' },
  { id: 'row-e', name: 'Sample row E', state: 'failed', note: 'Sample failure note' },
  { id: 'row-b', name: 'Sample row B', state: 'queued', note: 'Sample note' },
  { id: 'row-d', name: 'Sample row D', state: 'finished', note: 'Sample note' },
];

const STATE_ORDER: Record<RunState, number> = { running: 0, failed: 1, queued: 2, converged: 3, finished: 4 };

function Tables() {
  const [selected, setSelected] = useState('row-c');
  const [sort, setSort] = useState<SortDescriptor>({ column: 'name', direction: 'ascending' });
  const rows = useMemo(() => {
    const dir = sort.direction === 'descending' ? -1 : 1;
    return [...ROWS].sort((a, b) =>
      sort.column === 'state' ? dir * (STATE_ORDER[a.state] - STATE_ORDER[b.state]) : dir * a.name.localeCompare(b.name),
    );
  }, [sort]);
  return (
    <Section id="table" title="Table" wide>
      <Table
        aria-label="Sample rows"
        selectionMode="single"
        selectionBehavior="replace"
        disallowEmptySelection
        selectedKeys={[selected]}
        onSelectionChange={(keys) => {
          if (keys === 'all') return;
          const k = [...keys][0];
          if (k !== undefined) setSelected(String(k));
        }}
        sortDescriptor={sort}
        onSortChange={setSort}
      >
        <TableHeader>
          <Column id="name" isRowHeader allowsSorting>
            Name
          </Column>
          <Column id="state" allowsSorting>
            State
          </Column>
          <Column id="note">Note</Column>
        </TableHeader>
        <TableBody items={rows} dependencies={[selected]}>
          {(r) => (
            <Row id={r.id} failed={r.state === 'failed'} faded={r.id !== selected}>
              <Cell>
                <b className="gx-case">{r.name}</b>
              </Cell>
              <Cell>
                <StateMark state={r.state} faded={r.id !== selected} />
              </Cell>
              <Cell>{r.note}</Cell>
            </Row>
          )}
        </TableBody>
      </Table>
      <Readout id="table-readout">
        Selected: {ROWS.find((r) => r.id === selected)?.name}; sorted by {String(sort.column)}, {sort.direction}
      </Readout>
    </Section>
  );
}

// ---- pictograms and icons --------------------------------------------------------------------------------------

function Pictograms() {
  const grounds: Array<{ name: string; cls: string; states: readonly RunState[]; faded?: boolean }> = [
    { name: 'On white', cls: 'gx-ground-white', states: RUN_STATES },
    { name: 'On the selected row (sky-wash)', cls: 'gx-ground-sky-wash', states: RUN_STATES },
    { name: 'On hover (silver-wash)', cls: 'gx-ground-silver-wash', states: RUN_STATES },
    { name: 'On a failed row (fail-wash)', cls: 'gx-ground-fail-wash', states: ['failed'] },
    { name: 'Faded (rows not selected; a failure never fades)', cls: 'gx-ground-white', states: RUN_STATES, faded: true },
  ];
  return (
    <Section id="pictograms" title="State pictograms and icons" wide>
      <div className="gx-pictos">
        {grounds.map((g) => (
          <div key={g.name} className={`gx-picto-row ${g.cls}`}>
            <span className="gx-caption">{g.name}</span>
            <ul className="gx-list">
              {g.states.map((s) => (
                <li key={s}>
                  <StateMark state={s} faded={g.faded} />
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
      <ul className="gx-icons" aria-label="Icons">
        {ICON_NAMES.map((n) => (
          <li key={n}>
            <Icon name={n} />
            <span className="gx-caption">{n}</span>
          </li>
        ))}
      </ul>
    </Section>
  );
}

// ---- tokens ----------------------------------------------------------------------------------------------------

function Tokens() {
  const jobs = (p: PaletteName): string =>
    Object.entries(role)
      .filter(([, v]) => v === p)
      .map(([k]) => k)
      .join(', ');
  return (
    <Section id="tokens" title="Colour and type tokens" wide>
      <div className="gx-tokens">
        <table className="gx-plain">
          <caption className="gx-caption">Palette and the roles that use each colour</caption>
          <thead>
            <tr>
              <th scope="col">Colour</th>
              <th scope="col">Value</th>
              <th scope="col">Roles</th>
            </tr>
          </thead>
          <tbody>
            {(Object.keys(palette) as PaletteName[]).map((p) => (
              <tr key={p}>
                <td>
                  <span className={`gx-swatch gx-swatch--${p}`} aria-hidden="true" /> {p}
                </td>
                <td>{palette[p]}</td>
                <td className="gx-roles">{jobs(p)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <ul className="gx-ramp" aria-label="Type ramp">
          {(Object.keys(typeRamp) as TypeName[]).map((t) => (
            <li key={t}>
              <span className="gx-caption">
                {t}, {typeRamp[t].size}px {typeRamp[t].weight}
              </span>
              <span className={`type-${t}`}>Aa 0123456789</span>
            </li>
          ))}
        </ul>
      </div>
    </Section>
  );
}

// ---- page ------------------------------------------------------------------------------------------------------

export default function PrimitivesGallery() {
  const [project, setProject] = useState('a');
  return (
    <div className="gx-page">
      <header className="gx-top">
        <span className="gx-brand">
          rcfd <span>Studio</span>
        </span>
        <Tabs
          aria-label="Sample project tabs"
          variant="project"
          items={[
            { id: 'a', label: 'sample-project-a' },
            { id: 'b', label: 'sample-project-b' },
            { id: 'c', label: 'sample-project-c' },
          ]}
          selected={project}
          onChange={setProject}
          className="gx-projects"
        />
        <span className="gx-top-right">
          <Button variant="ghost" icon="plus">
            Sample ghost
          </Button>
        </span>
      </header>
      <main className="gx-main" aria-labelledby="gx-title">
        <h1 id="gx-title" className="gx-title">
          Primitives gallery
        </h1>
        <div className="gx-grid">
          <Buttons />
          <Segmented />
          <Fields />
          <Selects />
          <Switches />
          <Sliders />
          <SubTabs />
          <Tables />
          <Pictograms />
          <Tokens />
        </div>
      </main>
      <footer className="gx-foot">
        <span>
          rcfd is early-stage software. It is not validated for engineering decisions or for safety-critical use, and it
          comes with no warranty.
        </span>
        <span className="gx-foot-right">Development gallery: sample labels, no solver data</span>
      </footer>
    </div>
  );
}
