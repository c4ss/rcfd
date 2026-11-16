// The empty project view: what the window shows before any project is open. A placeholder until F-805 (project home);
// it states plainly that nothing is connected yet (G6).
export function ProjectViewPlaceholder() {
  return (
    <main className="project-view" aria-labelledby="project-view-title">
      <h1 id="project-view-title">No project open</h1>
      <p>
        This is the F-800 skeleton of rcfd Studio. It does not open project folders or talk to the rcfd service yet.
      </p>
      <p className="disclaimer">
        rcfd is early-stage software. It is not validated for engineering decisions or for safety-critical use, and it
        comes with no warranty.
      </p>
    </main>
  );
}
