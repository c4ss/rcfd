// F-837: what the removed egui Studio's project view showed besides cases, meshes, runs and notes:
//
// - the Designs group: the top-level entries of designs/ (the engine's `Project::designs`, counted by project_info),
//   headed by the folder's own name (data, G22: no new chrome word);
// - data files kept apart from case files: a file of cases/ that is not .toml or .json is not a case (the engine's
//   rule, ADR 0148; the egui Studio's `is_case_file`), so it has no Edit or Run;
// - a mark on a case file the case editor holds unsaved changes of (its session's pending changes since it was opened or
//   last saved, from the case port: the same count Save writes).
import { useEffect, useState } from 'react';
import type { DesignEntry, Listing } from '../../shared/project.js';
import { caseDoor } from '../case/door.js';
import { caseHash } from '../case/hash.js';
import { StateMark } from '../ui/index.js';
import { clock, inFolder } from './format.js';

/** A case file: what the engine reads as a case (ADR 0148). */
export const isCaseFile = (p: string): boolean => /\.(toml|json)$/i.test(p);

/** How often the home asks the case editor which files it holds unsaved changes of. */
const UNSAVED_POLL_MS = 3000;

/**
 * The case files of `folder` the case editor holds unsaved changes of (project-relative path to the session that holds
 * them, the latest opened when several do), read on mount and every UNSAVED_POLL_MS while the home is shown. Reads
 * only: the editor's sessions and journals are not touched.
 */
export function useUnsavedCases(folder: string): ReadonlyMap<string, string> {
  const [paths, setPaths] = useState<ReadonlyMap<string, string>>(new Map());
  useEffect(() => {
    let live = true;
    const read = async () => {
      const s = await caseDoor.state();
      if (!live || !s.ok) return;
      const mine = s.result.bindings.filter((b) => b.folder === folder);
      const out = new Map<string, string>();
      for (const b of [...mine].sort((x, y) => x.openedAt.localeCompare(y.openedAt))) {
        const st = await caseDoor.status(b.session);
        if (st.ok && st.result.pending.count > 0) out.set(b.path, b.session);
      }
      if (!live) return;
      setPaths((was) => (was.size === out.size && [...out].every(([p, s]) => was.get(p) === s) ? was : out));
    };
    void read();
    const t = setInterval(() => void read(), UNSAVED_POLL_MS);
    return () => {
      live = false;
      clearInterval(t);
    };
  }, [folder]);
  return paths;
}

/**
 * The unsaved mark (the egui Studio's `*`): a shape, with its words for readers and in the tooltip. It leads to the
 * editor's session that holds the changes (Edit opens the file in a new session, without them).
 */
export function UnsavedMark({ path, session }: { path: string; session: string }) {
  const words = `Unsaved changes in the case editor (session ${session}): open them to save or discard`;
  return (
    <a className="ph-unsaved" href={caseHash.session(session)} data-unsaved={path} data-session={session} title={words} aria-label={words}>
      <span aria-hidden="true">*</span>
    </a>
  );
}

/** The files of cases/ that are not case files, under the cases table: names only, no Edit or Run. */
export function DataFiles({ files, changed }: { files: readonly { path: string }[]; changed: Map<string, string> }) {
  if (files.length === 0) return null;
  return (
    <section className="ph-datafiles" aria-label="Data files in cases/ (not case files: no Edit or Run)">
      <ul data-words="data">
        {files.map((f) => (
          <li key={f.path} data-path={f.path} data-data-file="">
            <span className="ph-name-cell">{inFolder(f.path, 'cases')}</span>
            {changed.get(f.path) && <span className="ph-changed">{clock(changed.get(f.path)!)}</span>}
          </li>
        ))}
      </ul>
    </section>
  );
}

/** The Designs group: designs/ and its entries, with the engine's count when it differs. */
export function DesignsGroup({ listing, engineCount, filter }: { listing: Listing<DesignEntry>; engineCount: number | undefined; filter: (p: string) => boolean }) {
  const shown = listing.items.filter((d) => filter(d.path));
  if (listing.error === null && listing.total === 0) return null;
  const disagree = engineCount !== undefined && listing.error === null && engineCount !== listing.total;
  return (
    <section className="ph-group" aria-labelledby="ph-g-designs" data-group="designs">
      <h2 id="ph-g-designs" className="ph-group-head">
        <span className="sr-only">Designs, </span>
        {/* G22: the folder's own name heads it (data), as a group title would add a chrome word */}
        <span className="ph-group-title" data-words="data">
          designs/
        </span>
        <span className="ph-group-count" data-words="data">
          {listing.total}
        </span>
        {disagree && <span data-words="exception">; the engine counts {engineCount}</span>}
      </h2>
      {listing.error !== null ? (
        <div className="ph-group-error" role="alert" data-words="exception">
          <StateMark state="failed" label="Failed to list designs/" />
          <span>{listing.error}</span>
        </div>
      ) : (
        <ul className="ph-designs" data-words="data">
          {shown.map((d) => (
            <li key={d.path} data-path={d.path}>
              {/* a folder ends in / (as the group's head does), so no chrome word says so */}
              <span className="ph-name-cell">
                {inFolder(d.path, 'designs')}
                {d.folder ? '/' : ''}
              </span>
              {d.folder && <span className="sr-only">, a folder</span>}
            </li>
          ))}
        </ul>
      )}
      {listing.error === null && listing.total > listing.items.length && (
        <p className="ph-more" data-words="exception">
          The first {listing.items.length} of {listing.total} are listed.
        </p>
      )}
    </section>
  );
}
