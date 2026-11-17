// The Sites section of the Studio's settings (F-848, ADR 0739 phase 1): the run sites of the user's sites file as the
// engine lists them (rcfd site list --json), each with whether the user confirmed it; Check starts the site's agent once
// (rcfd site check, no project data goes there) and says whether a run may go there now; a site that needs a yes asks
// in the shared decision panel, and yes is rcfd site check --confirm, which the engine records in the file. The file is
// opened in the system's editor: the Studio does not edit it (no editor here).
//
// G22, P9: the section is not there while the user has no sites file, so the page at rest is the page before F-848.
import { useState } from 'react';
import type { SiteCheck, SiteCheckResult } from '../../shared/sites.js';
import { Button, StateMark } from '../ui/index.js';
import { checkSite, openSitesFile } from '../project/door.js';
import { ConfirmSite, useSites } from '../runs/Sites.js';

type Checked = { readonly name: string; readonly result: SiteCheckResult };

export function SitesSection() {
  const { state, reload } = useSites(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [checked, setChecked] = useState<Checked | null>(null);
  const [asking, setAsking] = useState<{ name: string; check: SiteCheck; failure: string | null; busy: boolean } | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  if (!state.exists) return null;

  const check = async (name: string) => {
    setBusy(name);
    try {
      const result = await checkSite(name, false);
      setChecked({ name, result });
      if (result.ok && !result.check.admitted && result.check.protocolOk) setAsking({ name, check: result.check, failure: null, busy: false });
    } finally {
      setBusy(null);
    }
  };
  const confirm = async () => {
    const a = asking;
    if (!a) return;
    setAsking({ ...a, busy: true });
    const r = await checkSite(a.name, true);
    if (!r.ok || !r.check.admitted) {
      setAsking({ ...a, busy: false, failure: r.ok ? (r.check.reason ?? 'The engine did not admit the site.') : r.message });
      return;
    }
    setAsking(null);
    setChecked({ name: a.name, result: r });
    reload();
  };

  return (
    <section className="ss-section ss-sites" aria-labelledby="ss-sites">
      <h2 className="ss-section-title" id="ss-sites">
        Sites
      </h2>
      <div className="ss-sites-body">
        {state.error !== null ? (
          <p className="ss-sites-error" role="alert" data-words="exception">
            <StateMark state="failed" label="Failed to read the sites file" /> {state.error}
          </p>
        ) : (
          <ul className="ss-sites-list" aria-label="Run sites">
            {state.sites.map((s) => {
              const c = checked?.name === s.name ? checked.result : null;
              return (
                <li key={s.name} className="ss-site" data-site={s.name} data-confirmed={s.confirmed ? 'true' : 'false'}>
                  <span className="ss-site-name" data-words="data">
                    {s.name}
                  </span>
                  <span className="ss-site-where" data-words="data">
                    {s.host ?? s.kind}
                  </span>
                  <span className="ss-site-state" data-words={s.confirmed ? 'data' : 'exception'}>
                    {s.confirmed ? 'confirmed' : 'not confirmed'}
                  </span>
                  <Button onPress={() => void check(s.name)} isDisabled={busy !== null} aria-label={`Check ${s.name}`}>
                    {busy === s.name ? 'Check…' : 'Check'}
                  </Button>
                  {c && (
                    <span className="ss-site-check" role="status" data-words={c.ok && c.check.admitted ? 'data' : 'exception'} data-admitted={c.ok && c.check.admitted ? 'true' : 'false'}>
                      {c.ok ? (c.check.admitted ? `ready: ${c.check.engine}` : (c.check.reason ?? 'a run may not go there now')) : c.message}
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        )}
        <div className="ss-sites-file">
          <span className="ss-sites-path" data-words="data">
            {state.file}
          </span>
          <Button
            variant="ghost"
            onPress={async () => {
              const m = await openSitesFile();
              setOpenError(m || null);
            }}
          >
            Open the sites file
          </Button>
          {openError && (
            <span role="alert" data-words="exception">
              {openError}
            </span>
          )}
        </div>
      </div>
      {asking && (
        <ConfirmSite
          check={asking.check}
          site={asking.name}
          what={null}
          failure={asking.failure}
          busy={asking.busy}
          onConfirm={() => void confirm()}
          onCancel={() => setAsking(null)}
        />
      )}
    </section>
  );
}
