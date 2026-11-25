// Comparing two runs in the run workspace (F-815; ADR 0801, addendum F-815; guardrails G2, G3, G6, G8, G16, G21, G22):
// the selected run (the baseline, A) and a compared run (B) at the same time (C1), side by side in one colour range
// with one legend (C4), or the difference B - A where the meshes match (C2, C3) and a short refusal in words where they
// do not. The scrubber of the time panel drives both: B's snapshot is found from A's stop, never interpolated.
//
// The Quiet Rule (G22): at rest the controls are pictograms whose names are their accessible names and tooltips; the
// two views carry no words of their own beyond F-808's caption and readout. Visible words appear only for an exception:
// no snapshot at that time, no common time, no difference (the meshes differ), a field that cannot be read.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Focusable, ToggleButton, ToggleButtonGroup, TooltipTrigger } from 'react-aria-components';
import { counterpart, counterpartWords, fieldsMatch, meshMatch, timeKind, unionSpan, type Counterpart, type Span } from '../../shared/compare.js';
import type { StoreInfo, StoreRef } from '../../shared/data.js';
import type { RunComparison } from '../../shared/run.js';
import { loadDifference } from '../data/door.js';
import { colourRange, lutBytes } from '../field/colormap.js';
import { FieldViewport, type FieldLoad } from '../field/FieldViewport.js';
import { Legend, tickKeys, type Tick } from '../field/Legend.js';
import { legendTick, openMember, type ProvenanceSubject } from '../provenance/index.js';
import { fmt } from '../plots/parts.js';
import { caseName, type RunRow } from '../runs/model.js';
import { Select, StatePictogram, Tooltip } from '../ui/index.js';
import { runComparison } from './door.js';
import { COMPARE_ICON, COMPARE_NAME, type CompareMode } from './compare-model.js';
import './compare.css';

export type { CompareMode } from './compare-model.js';

function CompareIcon({ name }: { name: keyof typeof COMPARE_ICON }) {
  return (
    <svg className="ui-icon" width={14} height={14} viewBox="0 0 14 14" aria-hidden="true" focusable="false" data-icon={`compare-${name}`}>
      <path d={COMPARE_ICON[name]} />
    </svg>
  );
}

export interface CompareState {
  readonly on: boolean;
  /** The compared run; null: the first other run. */
  readonly other: string | null;
  readonly mode: CompareMode;
}

/** The compare controls in the field bar: the toggle, the compared run and the view (pictograms). */
export function CompareControls({
  rows,
  selected,
  state,
  other,
  onChange,
}: {
  rows: readonly RunRow[];
  selected: string;
  state: CompareState;
  other: string | null;
  onChange: (s: CompareState) => void;
}) {
  const others = rows.filter((r) => r.id !== selected);
  if (others.length === 0) {
    const words = `${COMPARE_NAME.compare}, unavailable: the project has no other run to compare with`;
    return (
      <TooltipTrigger delay={300}>
        <Focusable>
          <span className="ui-unavailable cmp-off" role="button" aria-disabled="true" tabIndex={0} aria-label={words}>
            <CompareIcon name="compare" />
          </span>
        </Focusable>
        <Tooltip>{words}</Tooltip>
      </TooltipTrigger>
    );
  }
  return (
    <div className="cmp-controls" role="group" aria-label={COMPARE_NAME.compare}>
      <TooltipTrigger delay={600}>
        <ToggleButton
          className="ui-segment ui-segment--single cmp-toggle"
          aria-label={COMPARE_NAME.compare}
          isSelected={state.on}
          onChange={(on) => onChange({ ...state, on })}
        >
          <CompareIcon name="compare" />
        </ToggleButton>
        <Tooltip>{COMPARE_NAME.compare}</Tooltip>
      </TooltipTrigger>
      {state.on && (
        <>
          <Select
            aria-label={COMPARE_NAME.other}
            valueWords="data"
            className="cmp-other"
            options={others.map((r) => ({ id: r.id, label: `${caseName(r.case) || r.id} · ${r.id}` }))}
            selected={other}
            onChange={(id) => onChange({ ...state, other: id })}
          />
          <ToggleButtonGroup
            aria-label={COMPARE_NAME.mode}
            className="ui-segmented cmp-mode"
            selectionMode="single"
            disallowEmptySelection
            selectedKeys={[state.mode]}
            onSelectionChange={(keys) => {
              const k = [...keys][0];
              if (k === 'side' || k === 'difference') onChange({ ...state, mode: k });
            }}
          >
            {(['side', 'difference'] as const).map((m) => (
              <TooltipTrigger key={m} delay={600}>
                <ToggleButton id={m} className="ui-segment" aria-label={COMPARE_NAME[m]}>
                  <CompareIcon name={m} />
                </ToggleButton>
                <Tooltip>{COMPARE_NAME[m]}</Tooltip>
              </TooltipTrigger>
            ))}
          </ToggleButtonGroup>
        </>
      )}
    </div>
  );
}

/** C1: B's snapshot at A's snapshot `aSnapshot`; null while either store is not read. */
export function useCounterpart(a: StoreInfo | null, aSteady: boolean | null, aSnapshot: number | null, b: StoreInfo | null, bSteady: boolean | null): Counterpart | null {
  return useMemo(() => {
    if (!a || !b || aSnapshot === null) return null;
    const at = a.snapshots[aSnapshot];
    if (!at) return null;
    return counterpart(at, timeKind(a.snapshots, aSteady), b.snapshots, timeKind(b.snapshots, bSteady));
  }, [a, aSteady, aSnapshot, b, bSteady]);
}

/** A short exception in a view's place: a pictogram and its words (G22: shown because it is an exception). */
function Refusal({ words, view }: { words: string; view: string }) {
  return (
    <div className="cmp-refusal" role="status" data-view={view} data-refusal="" data-words="exception">
      <StatePictogram state="queued" size={20} />
      <p>{words}</p>
    </div>
  );
}

export interface CompareFieldsProps {
  readonly mode: CompareMode;
  readonly a: { readonly row: RunRow; readonly ref: StoreRef; readonly info: StoreInfo };
  readonly b: { readonly row: RunRow; readonly ref: StoreRef | null; readonly info: StoreInfo | null; readonly error: string | null };
  readonly field: string;
  /** A's snapshot at the scrubber's stop. */
  readonly snapshot: number;
  readonly reopenA: number;
  readonly reopenB: number;
  /** Called when the stop is drawn: A's view, and B's view or its refusal (Play waits for it). */
  readonly onDrawn: (snapshot: number) => void;
}

/** The fields of a comparison: side by side in one range, or the difference, at the same time (C1 to C4). */
export function CompareFields({ mode, a, b, field, snapshot, reopenA, reopenB, onDrawn }: CompareFieldsProps) {
  const c = useCounterpart(a.info, a.row.detail?.steady ?? null, snapshot, b.info, b.row.detail?.steady ?? null);
  const unit = a.info.fields.find((f) => f.name === field)?.unit ?? '';
  const bHas = b.info?.fields.some((f) => f.name === field && f.location === 'cell') ?? false;
  const bSnapshot = c?.kind === 'match' ? c.snapshot : null;
  const bWords =
    b.error !== null
      ? `No stored field: ${b.error}`
      : !b.info
        ? null
        : b.info.snapshots.length === 0
          ? 'No snapshot stored yet'
          : !bHas
            ? `No field ${field}`
            : c && c.kind !== 'match'
              ? counterpartWords(c)
              : null;

  // C4: each view's own range, as it reports it in the render that shows its values; one range from both
  const [spans, setSpans] = useState<{ a: Span | null; b: Span | null; bRun: string }>({ a: null, b: null, bRun: b.row.id });
  const own = spans.bRun === b.row.id ? spans : { a: spans.a, b: null, bRun: b.row.id };
  const bShown = bWords === null && bSnapshot !== null && b.ref !== null && b.info !== null;
  const union = unionSpan(own.a, bShown ? own.b : null);
  const range = useMemo(() => (union ? colourRange(union.lo, union.hi) : undefined), [union?.lo, union?.hi]);
  const onShownA = useCallback((s: { lo: number; hi: number }) => setSpans((p) => ({ ...p, a: { lo: s.lo, hi: s.hi } })), []);
  const bRun = b.row.id;
  const onShownB = useCallback((s: { lo: number; hi: number }) => setSpans((p) => ({ a: p.a, b: { lo: s.lo, hi: s.hi }, bRun })), [bRun]);
  const lut = useMemo(lutBytes, []);

  // Play waits until both views have drawn the stop (or B says why it has nothing)
  const drawn = useRef<{ a: number | null; b: string | null }>({ a: null, b: null });
  const bKey = bShown ? `${b.row.id}/${bSnapshot}` : `none/${snapshot}`;
  const report = useCallback(() => {
    if (drawn.current.a === snapshot && drawn.current.b === bKey) onDrawn(snapshot);
  }, [snapshot, bKey, onDrawn]);
  useEffect(() => {
    if (!bShown && bWords !== null) {
      drawn.current.b = bKey;
      report();
    }
  }, [bShown, bWords, bKey, report]);

  // ADR 0795: the one legend's ticks, named by both views
  const legendFact = (t: Tick): ProvenanceSubject | null => {
    const at = a.info.snapshots[snapshot];
    if (!at) return null;
    const of = a.info.snapshots.length;
    return {
      kind: 'number',
      source: legendTick({
        ...t,
        run: a.row.id,
        quantity: field,
        unit,
        range: bShown && b.info && bSnapshot !== null ? { kind: 'shared', other: b.row.id, snapshot: bSnapshot, of: b.info.snapshots.length } : { kind: 'snapshot' },
        snapshot,
        of,
        step: at.step,
        time: at.time ?? null,
      }),
    };
  };
  const aTitle = a.row.id;
  if (mode === 'difference') {
    const mesh = b.info ? meshMatch(a.info, b.info) : null;
    const fit = b.info ? fieldsMatch(field, a.info.fields.find((f) => f.name === field), b.info.fields.find((f) => f.name === field), a.info.nCells) : null;
    const refusal =
      mesh && !mesh.ok ? `No difference: ${mesh.reason}` : bWords !== null ? bWords : fit && !fit.ok ? `No difference: ${fit.reason}` : null;
    return (
      <div className="cmp cmp--difference" data-mode="difference" data-a-snapshot={snapshot} data-b-snapshot={bSnapshot ?? undefined}>
        {refusal !== null ? (
          <Refusal words={refusal} view="difference" />
        ) : b.ref && bSnapshot !== null ? (
          <DifferenceView a={a} bRef={b.ref} bRow={b.row} bSnapshot={bSnapshot} bOf={b.info?.snapshots.length ?? 0} field={field} snapshot={snapshot} reopen={reopenA} onDrawn={onDrawn} />
        ) : (
          <p className="cmp-reading" role="status">
            Reading the stored fields of {b.row.id}…
          </p>
        )}
      </div>
    );
  }
  return (
    <div className="cmp cmp--side" onClick={openMember} data-mode="side" data-a-snapshot={snapshot} data-b-snapshot={bSnapshot ?? undefined} data-range={range ? `${range.lo} ${range.hi}` : undefined}>
      <div className="cmp-views">
        <div className="cmp-view" data-view="a" data-run={a.row.id}>
          <FieldViewport
            store={a.ref}
            snapshot={snapshot}
            field={field}
            reopen={reopenA}
            legend={false}
            range={range}
            onShown={onShownA}
            names={tickKeys('cmp')}
            title={aTitle}
            onDrawn={(s) => {
              drawn.current.a = s;
              report();
            }}
          />
        </div>
        <div className="cmp-view" data-view="b" data-run={b.row.id}>
          {bShown ? (
            <FieldViewport
              key={b.row.id}
              store={b.ref!}
              snapshot={bSnapshot}
              field={field}
              reopen={reopenB}
              legend={false}
              range={range}
              onShown={onShownB}
              names={tickKeys('cmp')}
              title={b.row.id}
              onDrawn={(s) => {
                drawn.current.b = `${b.row.id}/${s}`;
                report();
              }}
            />
          ) : bWords !== null ? (
            <Refusal words={bWords} view="b" />
          ) : (
            <p className="cmp-reading" role="status">
              Reading the stored fields of {b.row.id}…
            </p>
          )}
        </div>
      </div>
      <div className="cmp-legend">{range && <Legend lut={lut} quantity={field} unit={unit} lo={range.lo} hi={range.hi} provenance={{ prefix: 'cmp', of: legendFact }} />}</div>
    </div>
  );
}

/** The difference B - A on A's surface: C3's values from the reader's worker, drawn as f32 with their own range. */
function DifferenceView({
  a,
  bRef,
  bRow,
  bSnapshot,
  bOf,
  field,
  snapshot,
  reopen,
  onDrawn,
}: {
  a: CompareFieldsProps['a'];
  bRef: StoreRef;
  bRow: RunRow;
  bSnapshot: number;
  bOf: number;
  field: string;
  snapshot: number;
  reopen: number;
  onDrawn: (snapshot: number) => void;
}) {
  const load = useCallback<FieldLoad>(
    async (s, f, options) => {
      const r = await loadDifference(a.ref, s, bRef, bSnapshot, f, options);
      // drawn as f32: Float32Array.from rounds each f64 to nearest, as `as f32` does (C3)
      return r.ok ? { ok: true, result: { values: Float32Array.from(r.result.values) } } : r;
    },
    [a.ref, bRef, bSnapshot],
  );
  return (
    <div className="cmp-view" data-view="difference" data-run={bRow.id}>
      <FieldViewport
        store={a.ref}
        snapshot={snapshot}
        field={field}
        reopen={reopen}
        load={load}
        loadKey={`${bRow.id}/${bSnapshot}`}
        quantity={`Δ${field}`}
        title={`${bRow.id} − ${a.row.id}`}
        difference={{ run: bRow.id, snapshot: bSnapshot, of: bOf, baseline: a.row.id }}
        derived={false}
        onDrawn={onDrawn}
      />
    </div>
  );
}

/** compare_runs of the two runs (the engine's metrics and differences), read again when either run moves on. */
export function useComparison(a: RunRow | null, b: RunRow | null): { comparison: RunComparison | null; error: string | null } {
  const [got, setGot] = useState<{ key: string; comparison: RunComparison | null; error: string | null } | null>(null);
  const key = a && b ? `${a.id}/${b.id}` : '';
  const live = a && b ? `${a.state}/${a.step}/${b.state}/${b.step}` : '';
  useEffect(() => {
    if (!a || !b) return;
    let alive = true;
    void runComparison(a.id, b.id).then((r) => {
      if (alive) setGot({ key, comparison: r.ok ? r.result : null, error: r.ok ? null : r.error.message });
    });
    return () => {
      alive = false;
    };
    // the runs are named by `key`; `live` changes with their state and step
  }, [key, live]);
  return got && got.key === key ? got : { comparison: null, error: null };
}

/** The engine's metrics of the two runs and their difference (compare_runs), as a hairline table of numbers. */
export function CompareMetrics({ a, b, comparison, error }: { a: RunRow; b: RunRow; comparison: RunComparison | null; error: string | null }) {
  if (error !== null) {
    return (
      <p className="ws-error cmp-metrics-error" role="alert" data-words="exception">
        <StatePictogram state="failed" /> Failed to compare the metrics: {error}
      </p>
    );
  }
  if (!comparison || comparison.metrics.length === 0) return null;
  const name = (r: RunRow) => caseName(r.case) || r.id;
  return (
    <table
      className="cmp-metrics"
      data-words="data"
      aria-label={`Metrics of ${name(a)} and ${name(b)}, and the difference ${b.id} minus ${a.id}, from the engine’s compare_runs; the engine gives no units for them`}
      data-baseline={comparison.baseline}
      data-compared={comparison.compared}
    >
      <thead>
        <tr>
          <th scope="col" aria-label="Metric" />
          <th scope="col" title={a.id}>
            {name(a)}
          </th>
          <th scope="col" title={b.id}>
            {name(b)}
          </th>
          <th scope="col" title={`${b.id} − ${a.id}`} aria-label="Difference">
            Δ
          </th>
        </tr>
      </thead>
      <tbody>
        {comparison.metrics.map((m) => (
          <tr key={m.name} data-metric={m.name} data-a={m.a ?? undefined} data-b={m.b ?? undefined} data-difference={m.difference ?? undefined}>
            <th scope="row">{m.name}</th>
            <td>{m.a === null ? '–' : fmt(m.a)}</td>
            <td>{m.b === null ? '–' : fmt(m.b)}</td>
            <td>{m.difference === null ? '–' : fmt(m.difference)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
