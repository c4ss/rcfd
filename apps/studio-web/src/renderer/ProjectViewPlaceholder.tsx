// The empty project view: what the window shows before any project is open. A placeholder until F-805 (project home);
// it states plainly that nothing is connected yet (G6).
import { useEffect } from 'react';

let reported = false;

/** Tells the main process, once, that the view is in the document and one animation frame has passed (P10). */
function reportReady(): void {
  if (reported) return;
  reported = true;
  requestAnimationFrame(() => {
    void window.studio.app.ready();
  });
}

export function ProjectViewPlaceholder() {
  useEffect(reportReady, []);
  return (
    <main className="project-view" aria-labelledby="project-view-title">
      <h1 id="project-view-title">No project open</h1>
      <p>This early shell of rcfd Studio does not open project folders or talk to the rcfd service yet.</p>
      <p className="disclaimer">
        rcfd is early-stage software. It is not validated for engineering decisions or for safety-critical use, and it
        comes with no warranty.
      </p>
    </main>
  );
}
