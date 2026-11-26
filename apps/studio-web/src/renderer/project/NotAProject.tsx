// A folder the engine refused as no project (F-835): what is missing (the project file) and the next step, Make a
// project here (the engine's `rcfd project init`, ADR 0731; the Studio writes nothing itself); the home also opens the
// folder by itself once the file is there, made by anyone (src/main/project/home.ts). An exception, so said in words
// (G6, G22); what the button does is behind Explain and in its accessible description.
//
// Moved unchanged in behaviour from first/FirstPath.tsx (F-813) when ADR 0808 (F-981) retired the learner's first path
// that held it (the maintainer's answer of 2026-11-26, ADR 0803); its classes keep the fp- names they had.
import { useId, useState, type ReactNode } from 'react';
import type { OpenFailure } from '../../shared/project.js';
import { Button } from '../ui/index.js';
import { projectActions } from './door.js';
import './not-a-project.css';

function Explain({ name, children }: { name: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <div className="fp-explain">
      <Button variant="ghost" icon={open ? 'minus' : 'plus'} onPress={() => setOpen((o) => !o)} aria-expanded={open} aria-controls={id} aria-label={`${open ? 'Hide' : 'Explain'} ${name}`}>
        {open ? 'Hide' : 'Explain'}
      </Button>
      {open && (
        <div id={id} className="fp-explain-body" role="region" aria-label={`About ${name}`} data-words="help">
          {children}
        </div>
      )}
    </div>
  );
}

/** F-835: what Make a project here does (its description for readers and in Explain). */
const MAKE_PROJECT =
  'The engine makes the project file and the folders cases, meshes, runs, designs and notes here (rcfd project init); what the folder holds already stays. Then it opens.';

/** A folder the engine refused as no project (an exception, so in words): what is missing and the next step. */
export function NotAProjectYet({ failure }: { failure: OpenFailure }) {
  const st = failure.folderState;
  const holds = st?.entries == null ? '' : st.entries === 0 ? 'Empty folder: ' : `${st.entries === 1 ? '1 entry' : `${st.entries} entries`}, `;
  return (
    <section className="fp fp--folder" aria-labelledby="fp-folder-title" data-folder-state={st ? (st.watching ? 'watching' : 'still') : 'unknown'} data-words="exception">
      <h2 id="fp-folder-title" className="fp-title">
        This folder is not a project yet
      </h2>
      <p className="fp-missing">
        {holds}no <span className="ph-file">rcfd-project.json</span>.
      </p>
      <p className="fp-next">Next: make it a project here.</p>
      <div className="fp-actions">
        <Button variant="primary" onPress={projectActions.createHere} aria-describedby="fp-make-what">
          Make a project here
        </Button>
        <span className="sr-only" id="fp-make-what">
          {MAKE_PROJECT}
        </span>
        <Explain name="what a project needs">
          <p>
            Every rcfd project starts with a small project file, <span className="ph-file">rcfd-project.json</span>, at the top of its folder. Make a
            project here: {MAKE_PROJECT}
          </p>
          <ul>
            <li>
              The same from a terminal: <span className="ph-file">rcfd project init {failure.folder}</span>; Claude Code makes it with create_project.
            </li>
            <li>
              {st?.watching
                ? 'Made elsewhere, the project opens by itself: the Studio checks the folder every second and opens it as soon as the project file is there.'
                : 'Made elsewhere, open the folder again with Open project folder….'}
            </li>
            <li>Or choose another folder, one that already holds a project.</li>
          </ul>
        </Explain>
      </div>
    </section>
  );
}
