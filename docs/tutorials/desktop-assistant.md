# The desktop Studio and the Claude Code panel

This tutorial opens a project in the desktop Studio, adds a cavity case to it, edits and runs the case, looks at the result
through time, compares two runs and asks Claude Code about them. It is for people who prefer a window to the command line. If
you want to script runs instead, start with the [quick start](quick-start-cli.md).

The Studio is the Electron app in `apps/studio-web/` of the repository (ADR 0800). It is a client of the engine: it reads a
project folder through the engine's project server (`rcfd-mcp`), starts and stops runs through the project's runner
(`rcfd-runner`), edits cases through the solver service (`rcfd serve`), and talks to Claude Code in a side panel. It is in
development: there is no installer yet (roadmap F-820), so you run it from a checkout. The two older desktop apps,
`rcfd-studio` and `rcfd-gui`, are frozen but still work until they are retired (F-449); the last section says what they
have that the new Studio does not.

**What is and is not tested on this page.** The tutorial test executes only the command-line steps (writing and checking
the case file). Every Studio step is marked `not-executed`: the tutorial test cannot click through a window. The Studio's
own Playwright tests (`apps/studio-web/tests/e2e/`) drive the packaged app through the same controls, by the names this
page uses, against the real `rcfd-mcp`, `rcfd-runner` and `rcfd` on a copy of the quick-start example project. Those tests
use a fake `claude` program, so no real Claude Code session has been run through the panel by the test suite. Nobody has
walked through this page step by step against the running app.

**How this page names controls.** The Studio shows few words at rest: many controls are pictograms. This page names each
control by its accessible name. A screen reader reads it, and for a pictogram the tooltip shows the same words on hover or
on keyboard focus. Every action is also in the command palette (Ctrl+K) under the name given here.

What you need: a checkout of this repository, a Rust toolchain with cargo, and Node.js 22.12 or later with npm. The Studio
was measured on Windows 11. On Linux its tests pass in CI under a virtual display, and nothing else is measured there;
macOS is not measured. Claude Code is optional (step 9).

## 1. Write and check a case

Save this as `cavity.toml`.

```toml file=cavity.toml
title = "cavity for the desktop app"

[mesh]
generator = "cavity_2d"
nx = 32
ny = 32

[physics]
nu = 0.01

[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.walls]
velocity = "fixed"

[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 1000
tolerance_u = 1e-6
tolerance_p = 1e-6
```

Check the case.

```console run
$ rcfd check cavity.toml
```

```text output
cavity.toml: ok, 1024 cells
```

## 2. Build the engine and start the Studio

The Studio starts `rcfd serve` and `rcfd-mcp` from the folder of the `rcfd` you name, and `rcfd-runner` from beside
`rcfd-mcp`. It never looks for them on `PATH`. Build the three in release, then build the Studio:

```console not-executed
cargo build --release -p rcfd-cli -p rcfd-runner -p rcfd-mcp
cd apps/studio-web
npm ci
npm run build
```

Start it with `RCFD_STUDIO_RCFD` set to the `rcfd` you built (with `CARGO_TARGET_DIR` set, the programs are in
`$CARGO_TARGET_DIR/release` instead). In PowerShell, from `apps/studio-web`:

```console not-executed
$env:RCFD_STUDIO_RCFD = (Resolve-Path ..\..\target\release\rcfd.exe).Path
npm start
```

In a POSIX shell:

```console not-executed
RCFD_STUDIO_RCFD="$PWD/../../target/release/rcfd" npm start
```

`npm run package:dir` makes a packaged copy, `out/packaged/rcfd-studio.exe` (`out/packaged/rcfd-studio` on Linux), which
takes the same variables; it is what the tests drive. Do not confuse it with the older app of the same name in
`target/release/`. The other variables (where the recent list is kept, an already running service) are in
`apps/studio-web/README.md`.

Without `RCFD_STUDIO_RCFD` the window still opens, but Open project folder… is unavailable and the home says what to set.

## 3. Open a project (not executed)

The Studio opens project folders: a folder with an `rcfd-project.json` and the folders `cases/`, `meshes/`, `runs/` and
`notes/`. It cannot make a new project (the engine has no command for it yet), so copy the quick-start example project out
of the repository and open the copy. Use `example-projects/quick-start` on Windows and `example-projects-linux/quick-start`
on Linux (a stored mesh equals the mesh its case builds only on the platform that made the run; ADR 0150).

```console not-executed
Copy-Item -Recurse example-projects\quick-start $HOME\rcfd-quick-start
cp -r example-projects-linux/quick-start ~/rcfd-quick-start
```

In the Studio, choose Open project folder… (Ctrl+O) and pick the copy. The Home view (Ctrl+1) lists what the folder
holds: four cases, their four finished runs, the meshes folder and the notes. Filter by name narrows the list. Reading a
project writes nothing into it; the header says Live while the Studio follows the folder's changes.

The project home with no project open also shows "A first result in four steps"; Explain the first steps says what they
need. This tutorial takes the longer path through the other views.

## 4. Add the case and edit it (not executed)

Copy `cavity.toml` into the project's `cases/` folder, from a terminal or a file manager. Home lists `cases/cavity.toml`
within about two seconds, without a reload: the Studio shows changes made by the CLI, an editor or Claude Code as they
happen.

```console not-executed
Copy-Item cavity.toml $HOME\rcfd-quick-start\cases\
cp cavity.toml ~/rcfd-quick-start/cases/
```

Then, in the Studio, open the case and give it a field store, so that its run keeps pictures to look at (the case of step
1 stores none):

```text not-executed
Home > Edit cases/cavity.toml        opens the case editor on the engine's settings of the file
Find a setting                       type a key; each setting has the engine's unit, range, default and description
output.store_interval                set 10: a snapshot of ux, uy, uz and p every 10 iterations
physics.nu                           set 0.0025 (Re 400); the engine refuses a value it cannot read, in its own words
Check the case                       the engine's check of the edited settings
Save (Ctrl+S)                        the engine writes the changed keys into the file; other lines and comments stay
Undo (Ctrl+Z)                        undoes the last edit; the Undo button names the step
```

If another program changed the file since the editor read it, Save writes nothing and shows both values with three
choices. Ctrl+1 goes back to Home.

## 5. Run it (not executed)

```text not-executed
Home > Run cavity.toml               starts a run and switches to Runs with the new run selected
F5                                   the same from anywhere: asks which case
Runs > Start a run > Case to run     pick a case, then Run, or Queue… for priority, threads, memory and --set lines
```

A run started this way stores the fields the case asks for, and no others. The run is the runner's, not the window's:
closing the Studio leaves it going, and the next start shows its true state. The selected run's panel holds its run
control: Pause or Resume, Stop… (or Cancel… for a run that has not started) and Run again. Stop… and Cancel… ask first
and say what is lost; Escape keeps the run. If the runner does not pick a request up within three seconds, the Studio
says so and offers Start the runner.

The Runs table (Ctrl+2) shows each run's state as a pictogram; its tooltip and accessible name give the word (converged,
failed, running and so on). Filter runs by case or run id and Run state narrow the table.

## 6. Look at the result through time (not executed)

Select the run in Runs and choose Open in workspace. The Workspace view shows the run's stored field on the right and the
time panel underneath.

```text not-executed
Field                         choose ux, uy, uz or p; the legend gives the range of the shown snapshot
pointer on the field          reads the stored value of the cell under it
drag, right drag, wheel       turn, pan and zoom; with the view focused: arrow keys, + and -, Home to reset
Mesh grid, Reset view         the cell edges; back to the first view
Vectors                       arrows of the stored velocity
Time of cavity.toml           the scrubber: it stops at the stored snapshots (arrow keys, Page Up, Page Down, Home, End)
First snapshot, Previous snapshot, Play, Next snapshot
Follow live                   for a running run: stay on the newest snapshot
Monitor                       the chart under the residual plot: the case's reports (lid_drag of the quick-start
                              cavities); Monitor series picks one when there are several; cavity.toml has none
```

Moving the scrubber moves the field, the residual cursor, the monitor readout and the snapshot counter together. The
axis is iterations for every run.

Cut plane, Iso-surface and Streamlines are dashed when they cannot be used; the tooltip says why. On this 2D case the cut
plane does not apply. Iso-surfaces and streamlines are not available on any case: the engine has no entry point for them
on a stored field.

## 7. Compare two runs (not executed)

```text not-executed
Compare runs                  the pictogram toggle in the field bar
Compared run                  pick the committed run of cases/cavity-re400.toml
Side by side | Difference     one scrubber moves both; one legend covers both views
```

The selected run is A and the compared run B. Both views show the same iteration (transient runs: the same stored time);
where B has no snapshot there, its view says so instead of borrowing a near one. Difference shows B minus A and is refused,
in words, when the meshes differ or one run lacks the field. The residual plot and the monitor chart draw B's lines with
A's, and a table under the run's numbers gives the engine's comparison metrics.

## 8. Where a number came from (not executed)

Click a number (Enter or Space when it has the focus), or press Ctrl+I, to see where it came from: the run, the snapshot,
the file it was read from, the engine version that made the run and the unit as the engine wrote it. Copy command copies
the command that runs the same run again from the command line; the palette's Copy reproduce command does the same for a
run you choose. It has this form:

```console not-executed
rcfd-runner start <case> --project <folder> --threads <n> --priority <n> [--mem-mb <n>] [--set path=value]...
```

When the command cannot reproduce the run exactly (the case file changed since, or a setting has no `rcfd-runner start`
option), the popover says so.

## 9. Ask Claude Code (not executed)

The Claude Code panel is on the right of the project views; Hide Claude Code and Show Claude Code fold it away and back.
It lists the project's Claude Code sessions, from a terminal or an editor as well. A session open elsewhere is shown
read-only as its transcript grows.

To write to Claude Code from the panel, start the Studio with `RCFD_STUDIO_CLAUDE` set to the path of the Claude Code
program (`claude.exe` on Windows; a `.js`, `.mjs` or `.cjs` path runs on the Studio's own runtime). Without it the panel
only reads.

```text not-executed
New chat                      starts a session in the project folder
Ctrl+L                        goes to the message field; Enter sends, Shift+Enter starts a new line, Escape stops the reply
```

Claude Code gets the engine's project server for this folder (`rcfd-mcp --project`) and no other MCP server. Without an
allow-list file it may call only the 15 read tools of the project server (`project_info`, `list_runs`, `run_status`,
`read_residuals`, `compare_runs` and others) and is refused everything else without being asked; a refused call shows the
narrowest rule that would allow it. The rules come from `.rcfd/claude-allow.json` in the project when that file is
valid; the Studio never writes it. The first chat in a folder that holds Claude Code control files (`.mcp.json`,
`CLAUDE.md`, `.claude/`) asks you to confirm them by name. Run ids and project files named in a reply are links that open
the run in Runs or the file on Home.

A question such as "Which runs converged, and how does cavity.toml differ from cavity-re400.toml?" is answered from the
read tools. What a real model answers depends on the model; none is run by the tests.

## 10. The command palette and the keys

Ctrl+K (or Ctrl+Shift+P, or Commands in the header) opens the command palette: every action of the Studio, found by name,
with its shortcut and its command-line and Claude Code equivalents. A command that cannot run stays in the list with the
reason. Ctrl+/ lists every shortcut by where it works. The keys that work anywhere in the window:

| Keys | What they do |
|---|---|
| Ctrl+K, Ctrl+Shift+P | open the command palette |
| Ctrl+/ | show the keyboard shortcuts |
| Ctrl+1, Ctrl+2 | Home, Runs |
| Ctrl+L | the Claude Code message field |
| Ctrl+O | open a project folder |
| Ctrl+N | new case, from a template or a shipped example |
| F5 | run a case (asks which) |
| Ctrl+. | stop a project run (asks which, then confirms) |
| Ctrl+I | where the focused or pointed number came from |
| Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y | undo and redo |

Ctrl is Cmd on macOS.

## What the Studio does not do yet

* No installer (F-820): it runs from a checkout as in step 2.
* No new project: the engine has no command that makes one. Copy an example project, or make one with the older
  `rcfd-studio` (below).
* No delete of a run, a result or a case: the engine has none, so the Studio offers none.
* No picture of a case's mesh before a run, no iso-surfaces and no streamlines.
* No model of your own choice: the panel talks to Claude Code only. `rcfd-gui` (below) has the local-model sidebar.

## The older desktop apps (frozen until F-449)

`rcfd-studio` (egui on wgpu) and `rcfd-gui` (egui on OpenGL) keep building and passing their tests but get no new
features. They are removed only after the new Studio reaches parity and the maintainer approves (F-448, F-449).

`rcfd-studio` opens the same project folders, can make a new one (Project > New project...) and shows a case's mesh
before it is run. One line builds it with the programs it starts and opens it on a folder:

```console not-executed
powershell -ExecutionPolicy Bypass -File scripts\studio.ps1 my-project
sh scripts/studio.sh my-project
```

`rcfd-gui` opens one case file, not a project. Its assistant sidebar talks to a model you choose through the same tools
as the [MCP server](mcp-design-loop.md), with four quick actions that need no model (Assess mesh, Validate case, Suggest
BCs, Explain residuals) and an Approve writes mode that shows each tool call that changes the case as a card with Approve
and Deny. Its provider presets are `ollama`, `lm-studio`, `llama-cpp` and `vllm` (local, OpenAI-compatible), `openai` and
`anthropic`. Start it on the case of step 1:

```console not-executed
cargo run --release -p rcfd-gui -- --root . cavity.toml
```

Headless tests in `crates/rcfd-gui/tests/` cover its logic with a mock model; no real or local model has been run
through it by the test suite (roadmap F-145, F-150).

## Where to go next

* [MCP design loop](mcp-design-loop.md): the engine's MCP tools, called directly.
* [MCP tools](../mcp-tools.md): the access classes and the project tools Claude Code gets in the Studio.
* [The case schema](../case-schema.md): every setting the case editor edits.
* [Run control](../run-control.md): run states, stored fields and the run service.
