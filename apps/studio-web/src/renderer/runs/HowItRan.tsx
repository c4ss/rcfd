// "How this ran" (F-828, ADR 0734 P5): the selected run's launch as the runner recorded it, its build and its mesh, read
// through the engine (the provenance door: run_status `launch`, run.json, hash_project_file of the mesh), and the
// rcfd-runner rerun command with Copy.
//
// G22: at rest it is one icon whose name is its tooltip and accessible name; the facts appear when it is pressed. A
// mesh changed since the run, a launch not recorded and a read that failed speak as exceptions. Nothing is filled in
// (G6): what the runner did not record is said.
import { useEffect, useState } from 'react';
import { howItRanFacts, type RunProvenance } from '../../shared/provenance.js';
import { Button, Quiet } from '../ui/index.js';
import { IconToggle } from '../ui/IconToggle.js';
import { copyText, runProvenance } from '../provenance/door.js';
import type { RunRow } from './model.js';
import { RunLogTools } from './RunExtras.js';

/** A fact that is an exception: a changed mesh, a launch not recorded, a check that could not be made. */
const isException = (k: string, v: string): boolean =>
  (k === 'Mesh' && (v.includes('; changed') || v.includes('not checked'))) || (k === 'Command' && (v.startsWith('Not recorded') || v.startsWith('Not read')));

/** The selected run's tools row (Log and events, the report, How this ran) and their panels. */
export function HowItRanTools({ row }: { row: RunRow }) {
  const [open, setOpen] = useState(false);
  const [got, setGot] = useState<{ p: RunProvenance | null; error: string | null }>({ p: null, error: null });
  const [copied, setCopied] = useState<{ ok: boolean; message: string } | null>(null);
  // a new selection starts closed
  useEffect(() => {
    setOpen(false);
    setGot({ p: null, error: null });
    setCopied(null);
  }, [row.id]);
  // read when opened, and again when the run's state changes while it is open (its launch record appears at launch)
  useEffect(() => {
    if (!open) return;
    let alive = true;
    void runProvenance(row.id).then((r) => {
      if (alive) setGot(r.ok ? { p: r.result, error: null } : { p: null, error: r.error.message });
    });
    return () => {
      alive = false;
    };
  }, [open, row.id, row.state]);
  const p = got.p;
  const copy = async () => {
    if (!p?.rerun.text) return;
    const r = await copyText(p.rerun.text);
    setCopied({ ok: r.ok, message: r.ok ? 'Rerun command copied' : `Not copied: ${r.error.message}` });
  };
  return (
    <>
      <RunLogTools
        row={row}
        extra={<IconToggle icon="how-ran" label={`How ${row.id} ran`} isSelected={open} onChange={setOpen} className="rv-how-toggle" />}
      />
      {open && (
        <section className="rv-block rv-how" aria-label={`How ${row.id} ran`} data-run={row.id}>
          {got.error ? (
            <p className="rv-why rv-why--failed" role="alert" data-words="exception">
              {`Not read: ${got.error}`}
            </p>
          ) : !p ? (
            <Quiet>Reading the run’s launch record…</Quiet>
          ) : (
            <>
              <dl className="pv-facts rv-how-facts">
                {howItRanFacts(p).map(([k, v]) => (
                  <div key={k} data-fact={k} className={isException(k, v) ? 'pv-exception' : undefined}>
                    <dt>{k}</dt>
                    <dd data-words={isException(k, v) ? 'exception' : 'data'}>{v}</dd>
                  </div>
                ))}
              </dl>
              {p.rerun.gaps.length > 0 && (
                <ul className="pv-gaps" aria-label="Why a rerun would not give the same result" data-words="exception">
                  {p.rerun.gaps.map((g) => (
                    <li key={g}>{g}</li>
                  ))}
                </ul>
              )}
              <div className="pv-actions">
                <Button icon={copied?.ok ? 'check' : undefined} isDisabled={!p.rerun.text} onPress={() => void copy()}>
                  Copy rerun command
                </Button>
                <span className={copied && !copied.ok ? 'pv-copy-failed' : 'sr-only'} role="status" data-words="exception">
                  {copied?.message ?? ''}
                </span>
              </div>
            </>
          )}
        </section>
      )}
    </>
  );
}
