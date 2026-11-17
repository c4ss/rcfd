// Run sites in the window (F-848, ADR 0739 phase 1; src/shared/sites.ts): the Where choice of the run bar, the yes a
// site needs before a run goes there (the shared decision panel, G18), and the site badge with the link state of a
// remote run in the run list and the workspace.
//
// Nothing here shows while the user has no sites file: the Where choice is there only when the engine lists a site, and
// a badge only on a run whose run.json names a site (G22, P9). Every answer is the engine's (rcfd site list and check,
// rcfd-runner start --site) or the local runner's files, read in the main process.
import { useCallback, useEffect, useRef, useState } from 'react';
import type { RunActionResult } from '../../shared/project.js';
import { LINK_WORDS, LOCAL, NO_SITES, whereOf, type RunSite, type SiteCheck, type SitesState } from '../../shared/sites.js';
import { Select } from '../ui/index.js';
import { DecisionPanel } from '../history/DecisionPanel.js';
import { checkSite, listSites, runOnSite } from '../project/door.js';
import './sites.css';

/** The user's sites as the engine lists them, read when `key` changes (a project opened or closed). */
export function useSites(key: string | null): { state: SitesState; reload: () => void } {
  const [state, setState] = useState<SitesState>(NO_SITES);
  const [n, setN] = useState(0);
  useEffect(() => {
    let live = true;
    void listSites().then((s) => {
      if (live) setState(s);
    });
    return () => {
      live = false;
    };
  }, [key, n]);
  return { state, reload: () => setN((x) => x + 1) };
}

/** The Where choice of the run bar: this machine, or a site of the sites file. Shown only when there is a site. */
export function WhereSelect({ state, where, onChange }: { state: SitesState; where: string; onChange: (w: string) => void }) {
  if (state.sites.length === 0) return null;
  return (
    <Select
      aria-label="Where to run"
      valueWords="data"
      className="st-where"
      options={[{ id: LOCAL, label: LOCAL }, ...state.sites.map((s) => ({ id: s.name, label: s.name }))]}
      selected={where}
      onChange={onChange}
    />
  );
}

/** The run bar's Where: the user's pick in this window, else the project's remembered one (whereOf). */
export function useWhere(state: SitesState): [string, (w: string) => void] {
  const [picked, setPicked] = useState<string | null>(null);
  const listed = picked !== null && (picked === LOCAL || state.sites.some((s) => s.name === picked));
  return [listed ? picked! : whereOf(state), setPicked];
}

export interface SiteRunParams {
  readonly case: string;
  readonly site: string;
  readonly priority?: number;
  readonly threads?: number;
  readonly mem_mb?: number;
  readonly set?: readonly string[];
}

type Asking = { readonly params: SiteRunParams; readonly check: SiteCheck; readonly failure: string | null; readonly busy: boolean };

/**
 * A run on a site: the engine checks the site first (rcfd site check, which starts its agent once and sends no project
 * data); a site that may not take a run yet (not confirmed, or another engine) asks the user in the decision panel, and
 * yes is rcfd site check --confirm. Then the run starts (site.run_case). `onResult` gets the engine's answer, or a
 * refusal in words; nothing is started when the user says no.
 */
export function useSiteStart(onResult: (r: RunActionResult) => void) {
  const [asking, setAsking] = useState<Asking | null>(null);
  const [busy, setBusy] = useState(false);
  const result = useRef(onResult);
  result.current = onResult;
  const refused = (problem: string, recovery: string): RunActionResult => ({ ok: false, request: null, failure: { code: 'bad_state', problem, recovery, engine: null } });

  const start = useCallback(async (params: SiteRunParams) => {
    setBusy(true);
    try {
      const c = await checkSite(params.site, false);
      if (!c.ok) {
        result.current(refused(`Site ${params.site} could not be checked: ${c.message}`, 'Nothing was sent. Check the site’s entry in the sites file (Studio settings, Sites), or run here.'));
        return;
      }
      if (!c.check.admitted) {
        setAsking({ params, check: c.check, failure: null, busy: false });
        return;
      }
      result.current(await runOnSite(params));
    } finally {
      setBusy(false);
    }
  }, []);

  const confirm = useCallback(async () => {
    const a = asking;
    if (!a) return;
    setAsking({ ...a, busy: true });
    const c = await checkSite(a.params.site, true);
    if (!c.ok || !c.check.admitted) {
      setAsking({ ...a, busy: false, failure: c.ok ? (c.check.reason ?? 'The engine did not admit the site.') : c.message });
      return;
    }
    setAsking(null);
    setBusy(true);
    try {
      result.current(await runOnSite(a.params));
    } finally {
      setBusy(false);
    }
  }, [asking]);

  const panel = asking ? (
    <ConfirmSite
      check={asking.check}
      site={asking.params.site}
      what={asking.params.case}
      failure={asking.failure}
      busy={asking.busy}
      onConfirm={() => void confirm()}
      onCancel={() => setAsking(null)}
    />
  ) : null;
  return { start, busy: busy || asking !== null, panel };
}

export interface ConfirmSiteProps {
  readonly check: SiteCheck;
  readonly site: string;
  /** The case a run would send (null: the Sites page, no run yet). */
  readonly what: string | null;
  readonly failure: string | null;
  readonly busy: boolean;
  readonly onConfirm: () => void;
  readonly onCancel: () => void;
}

/** The yes a site needs (G10 as amended 2026-11-17, Q3, Q10): what it is, what goes there, and the engines. */
export function ConfirmSite({ check: c, site: name, what, failure, busy, onConfirm, onCancel }: ConfirmSiteProps) {
  const engineDiffers = !c.sameEngine;
  const title = !c.protocolOk
    ? `Site ${name} cannot take runs`
    : failure !== null
      ? `Failed to confirm site ${name}`
      : !c.confirmed
        ? `Confirm site ${name}?`
        : `Run on ${name} with another engine?`;
  const choices = !c.protocolOk
    ? [{ label: 'Close', onPress: onCancel, safe: true }]
    : [
        { label: 'Keep it here', onPress: onCancel, safe: true },
        { label: c.confirmed ? 'Yes, use this engine' : what !== null ? 'Confirm and run' : 'Confirm', onPress: onConfirm, isDisabled: busy },
      ];
  return (
    <DecisionPanel title={title} failed={!c.protocolOk || failure !== null} onCancel={onCancel} choices={choices}>
      <p>
        {what !== null ? `A run there sends ${what} and the files it reads` : 'A run there sends its case and the files it reads'} to{' '}
        {c.host ?? `the ${c.kind} site`}; its results come back here. The Studio reaches another machine only through a site you confirmed once.
      </p>
      <dl className="st-facts">
        <dt>Host key</dt>
        <dd data-words="data">{c.fingerprint ?? c.fingerprintError ?? 'not known'}</dd>
        <dt>Engine there</dt>
        <dd data-words="data">{c.engine}</dd>
        <dt>Engine here</dt>
        <dd data-words="data">{c.localEngine}</dd>
      </dl>
      {engineDiffers && c.protocolOk && <p>Results are bit-identical only for one build: yes is recorded for this site and this engine.</p>}
      {/* the engine's reason, unless it only says what the title asks (not confirmed) */}
      {c.reason && c.confirmed && <p className="st-reason">{c.reason}</p>}
      {failure && <p className="st-reason">{failure}</p>}
    </DecisionPanel>
  );
}

/** The site of a remote run and its link state: the name is data, the state a word (on remote runs only). */
export function SiteBadge({ site }: { site: RunSite | undefined }) {
  if (!site) return null;
  const words = LINK_WORDS[site.link];
  const attention = site.link === 'retrying' || site.link === 'refused';
  return (
    <span
      className={`st-badge st-badge--${site.link}`}
      data-site={site.site}
      data-link={site.link}
      title={site.message ? `${site.site}: ${words}. ${site.message}` : `${site.site}: ${words}`}
    >
      <span className="st-badge-name" data-words="data">
        {site.site}
      </span>
      <span className="st-badge-link" data-words={attention ? 'exception' : 'chrome'}>
        {words}
      </span>
    </span>
  );
}
