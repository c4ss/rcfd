// The provenance inspector (F-828; guardrails G17, G22, G3, G2): any number on screen opens where it came from (run,
// snapshot, file, the engine version that made it, its unit as the engine gave it) and the command that reproduces its
// run, with Copy.
//
// Quiet by default (G22, DESIGN.md The Quiet Rule): at rest a number looks as it did, with no word, mark or chrome of
// its own. Its words are in its accessible name ("Provenance of lid_drag: -0.00817 N"), and the facts appear only on
// demand: a click, Enter or Space on the number, or Show provenance (Ctrl+I, the command palette) on the focused or
// pointed number. A hover or focus shows only the affordance (a dotted ink underline). The popover is the one lifted
// surface of DESIGN.md (1px ink edge, the popover lift); Escape closes it and the focus goes back to the number.
//
// Everything shown is the engine's: the number's own facts come from the surface that read it (its NumberSource), the
// run's from runs/<id>/run.json through the provenance door. A fact the engine did not record is said in words, never
// filled in (G6); the reasons a command would not reproduce the run are the exception that speaks (G22).
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { flushSync } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import { Dialog, Popover } from 'react-aria-components';
import {
  numberFacts,
  provenanceText,
  runFacts,
  versionLine,
  type Fact,
  type NumberSource,
  type RunProvenance,
} from '../../shared/provenance.js';
import { Button } from '../ui/index.js';
import { attachedEngine, copyText, runProvenance } from './door.js';
import { addRunOpener, addTrigger, registerProvenanceCommands, setHovered, type TriggerEntry } from './registry.js';
import './provenance.css';

export type ProvenanceSubject = { readonly kind: 'number'; readonly source: NumberSource } | { readonly kind: 'run'; readonly run: string };

export interface ProvenanceProps {
  /** What the number is; a function is read when the popover opens (a live readout). */
  readonly subject: ProvenanceSubject | (() => ProvenanceSubject | null);
  /** The number as shown; unchanged by the trigger. */
  readonly children: ReactNode;
  /**
   * A button and a tab stop of its own (default). Off inside a grid row or a list option, whose row takes the click, the
   * focus and the arrow keys: the number is then passive (no role, no click of its own, so a click still selects the
   * row) and Show provenance (Ctrl+I) opens it while its row has the focus or the pointer is on it.
   */
  readonly tabStop?: boolean;
  /** The key an element names with data-provenance (the scrubber names the counter). */
  readonly pkey?: string;
  /** The run whose own provenance Show provenance opens when nothing is focused (the selected run's id). */
  readonly runOpener?: boolean;
  /** Told when the popover opens and closes (the field view keeps its readout while it is open). */
  readonly onOpenChange?: (open: boolean) => void;
  readonly className?: string;
}

const unitWords = (s: NumberSource): string => (s.unit.kind === 'engine' && s.unit.unit !== '' ? ` ${s.unit.unit}` : '');

/** The accessible name of a trigger: its words at rest (G22). */
export function triggerName(subject: ProvenanceSubject | null): string {
  if (!subject) return 'Provenance';
  return subject.kind === 'run' ? `Provenance of run ${subject.run}` : `Provenance of ${subject.source.quantity}: ${subject.source.shown}${unitWords(subject.source)}`;
}

registerProvenanceCommands();

// The popover is drawn in a React root of its own, outside the tree of the surface the number sits in: a key pressed in
// it (Escape, the arrows) then never reaches the surface's own handlers, as it would through a portal (React's events
// follow the component tree), so closing the popover in a row of the runs table does not also clear the selection.
// Closing (Escape, a click outside) unmounts it at once (`now`): while it is open React Aria makes the rest of the page
// inert, and lifts that in an effect of this root; an ordinary render of this root would run that effect in a later
// task, often after the next frame, and a focus given back to the number in that frame (ours, or React Aria's own
// restore) would be refused by the inert page and left on the body.
let popRoot: Root | null = null;
let popOwner = 0;
let popClose: (() => void) | null = null;
function showPopover(node: ReactNode, owner: number, close: () => void): void {
  // one popover at a time: the one it replaces is told it closed
  if (popOwner !== owner) popClose?.();
  popClose = close;
  if (!popRoot) {
    const host = document.createElement('div');
    host.className = 'pv-host';
    document.body.append(host);
    popRoot = createRoot(host);
  }
  popOwner = owner;
  popRoot.render(node);
}
function hidePopover(owner: number, now = false): void {
  if (popRoot && popOwner === owner) {
    const root = popRoot;
    popOwner = 0;
    popClose = null;
    if (now) flushSync(() => root.render(null));
    else root.render(null);
  }
}
let owners = 0;

export function Provenance({ subject, children, tabStop = true, pkey, runOpener, onOpenChange, className }: ProvenanceProps) {
  const ref = useRef<HTMLElement>(null);
  const [open, setOpenState] = useState<ProvenanceSubject | null>(null);
  const told = useRef(onOpenChange);
  told.current = onOpenChange;
  const setOpen = (s: ProvenanceSubject | null) => {
    setOpenState(s);
    told.current?.(s !== null);
  };
  const read = () => (typeof subject === 'function' ? subject() : subject);
  const now = typeof subject === 'function' ? null : subject;
  const entry = useMemo<TriggerEntry>(
    () => ({
      el: () => ref.current,
      key: pkey ?? null,
      open: () => {
        const s = readRef.current();
        if (s) setOpen(s);
      },
    }),
    [pkey],
  );
  const readRef = useRef(read);
  readRef.current = read;
  useEffect(() => addTrigger(entry), [entry]);
  const run = now?.kind === 'run' ? now.run : null;
  useEffect(() => (runOpener && run !== null ? addRunOpener(run, entry.open) : undefined), [runOpener, run, entry]);
  const name = triggerName(now ?? read());
  const owner = useMemo(() => ++owners, []);
  // where the focus goes back when the popover closes: the number, or (a passive one) the row that had the focus
  const back = useRef<HTMLElement | null>(null);
  useEffect(() => {
    if (open === null) return;
    const active = document.activeElement;
    back.current = tabStop ? ref.current : active instanceof HTMLElement && active !== document.body ? active : null;
    const close = () => {
      // the popover and the page's inert are gone before the focus goes back (see popRoot above)
      hidePopover(owner, true);
      setOpen(null);
      const restore = () => {
        const a = document.activeElement;
        if (a === null || a === document.body || a.closest('.pv-host')) back.current?.focus();
      };
      restore();
      // and once more on the next frame, should a close from inside React's own work have left the unmount for later
      requestAnimationFrame(restore);
    };
    showPopover(<ProvenancePopover subject={open} triggerRef={ref} onClose={close} />, owner, () => setOpen(null));
    return () => hidePopover(owner);
  }, [open, owner]);
  const cls = `pv-num${tabStop ? '' : ' pv-num--passive'}${className ? ` ${className}` : ''}`;
  return (
    <>
      {tabStop ? (
        <button
          ref={(e) => {
            ref.current = e;
          }}
          type="button"
          className={cls}
          aria-label={name}
          aria-haspopup="dialog"
          aria-expanded={open !== null}
          data-provenance-trigger=""
          onClick={() => entry.open()}
          onPointerEnter={() => setHovered(entry)}
          onPointerLeave={() => setHovered(null, entry)}
        >
          {children}
        </button>
      ) : (
        <span
          ref={(e) => {
            ref.current = e;
          }}
          className={cls}
          data-provenance-trigger=""
          data-provenance-name={name}
          onPointerEnter={() => setHovered(entry)}
          onPointerLeave={() => setHovered(null, entry)}
        >
          {children}
        </span>
      )}
    </>
  );
}

function useRunProvenance(run: string): { p: RunProvenance | null; error: string | null; engine: { name: string; version: string } | null } {
  const [got, setGot] = useState<{ p: RunProvenance | null; error: string | null }>({ p: null, error: null });
  const [engine, setEngine] = useState<{ name: string; version: string } | null>(null);
  useEffect(() => {
    let alive = true;
    void runProvenance(run).then((r) => {
      if (alive) setGot(r.ok ? { p: r.result, error: null } : { p: null, error: r.error.message });
    });
    void attachedEngine().then((e) => {
      if (alive) setEngine(e);
    });
    return () => {
      alive = false;
    };
  }, [run]);
  return { ...got, engine };
}

function ProvenancePopover({ subject, triggerRef, onClose }: { subject: ProvenanceSubject; triggerRef: React.RefObject<HTMLElement | null>; onClose: () => void }) {
  const run = subject.kind === 'run' ? subject.run : subject.source.run;
  const { p, error, engine } = useRunProvenance(run);
  const [copied, setCopied] = useState<{ what: 'command' | 'all'; ok: boolean; message: string } | null>(null);
  const title = subject.kind === 'run' ? `run ${run}` : subject.source.quantity;
  const facts: Fact[] = subject.kind === 'run' ? (p ? runFacts(p) : [['Run', run]]) : numberFacts(subject.source, p ?? (error ? fallback(run, error) : null));
  // the engine of the attached service, said only when it is not the one that made the run (G22: the exception speaks)
  const made = p?.record?.solverVersion ?? null;
  const serviceDiffers = engine !== null && made !== null && versionLine(engine) !== made;
  const copy = async (what: 'command' | 'all') => {
    const text = what === 'command' ? p?.reproduce.text : provenanceText(`Provenance of ${title}`, facts, p);
    if (!text) return;
    const r = await copyText(text);
    setCopied({ what, ok: r.ok, message: r.ok ? (what === 'command' ? 'Command copied' : 'Provenance copied') : `Not copied: ${r.error.message}` });
  };
  return (
    <Popover triggerRef={triggerRef} isOpen onOpenChange={(o) => !o && onClose()} placement="bottom start" offset={4} className="ui-popover pv-pop">
      <Dialog aria-label={`Provenance of ${title}`} className="pv-dialog" data-run={run}>
        {subject.kind === 'number' && (
          <header className="pv-head">
            <span className="pv-quantity">{subject.source.quantity}</span>
            <span className="pv-value">{subject.source.shown}</span>
            {subject.source.unit.kind === 'engine' && subject.source.unit.unit !== '' && <span className="pv-unit">{subject.source.unit.unit}</span>}
          </header>
        )}
        <dl className="pv-facts">
          {facts.map(([k, v]) => (
            <div key={k} data-fact={k}>
              <dt>{k}</dt>
              <dd>{v}</dd>
            </div>
          ))}
          {serviceDiffers && (
            <div data-fact="Service engine" className="pv-exception">
              <dt>Service</dt>
              <dd>{`${versionLine(engine)}, not ${made}`}</dd>
            </div>
          )}
        </dl>
        <section className="pv-repro" aria-label={`Reproduce ${run} from the command line`}>
          {p?.reproduce.text ? (
            <p className="pv-cmd" data-command="">
              {p.reproduce.text}
            </p>
          ) : (
            <p className="pv-reading">{error ? `Not read: ${error}` : p ? '' : 'Reading the run’s record…'}</p>
          )}
          {p && p.reproduce.gaps.length > 0 && (
            <ul className="pv-gaps" aria-label="Why the command would not give the same result">
              {p.reproduce.gaps.map((g) => (
                <li key={g}>{g}</li>
              ))}
            </ul>
          )}
        </section>
        <div className="pv-actions">
          <Button icon={copied?.what === 'command' && copied.ok ? 'check' : undefined} isDisabled={!p?.reproduce.text} onPress={() => void copy('command')}>
            Copy command
          </Button>
          <Button icon={copied?.what === 'all' && copied.ok ? 'check' : undefined} onPress={() => void copy('all')}>
            Copy
          </Button>
          <span className={copied && !copied.ok ? 'pv-copy-failed' : 'sr-only'} role="status">
            {copied?.message ?? ''}
          </span>
        </div>
      </Dialog>
    </Popover>
  );
}

/** A run whose record could not be read: the number's own facts still show, with the reason. */
function fallback(run: string, error: string): RunProvenance {
  return {
    run,
    folder: '',
    recordPath: `runs/${run}/run.json`,
    recordHash: null,
    record: null,
    recordError: error,
    snapshotCheck: null,
    caseCheck: null,
    engineNow: null,
    units: {},
    unitsNote: null,
    reproduce: { argv: null, text: null, gaps: [], leftOut: [] },
    readAt: '',
  };
}
