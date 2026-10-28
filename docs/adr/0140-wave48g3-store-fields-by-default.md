# ADR 0140: Wave 48G job 3, first run gives a picture: Studio-started runs store fields by default, helpful empty states

Status: **PROPOSED 2026-10-28** (registered before any run of the checks below). Written at main f3bf005 (waves 47 and 48 and the
Linux gate fixes merged). One of four parallel jobs "48G, first run to first picture" (1 mesh preview from a case, 2 case starter
menu, 3 this ADR, 4 one-click start-and-follow). House rules of ADRs 0110-0133 apply: limits are fixed here before the registered
runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment
registered before the corrected run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not
re-recorded; the crate graph stays acyclic. Parents: ADR 0130 D3/D4 (W48-4 (b), W48-5 (a)), ADR 0132 (viewport), ADR 0133 (start
plan, R10, `output.store_precision`), [`docs/ui-restart-plan.md`](../ui-restart-plan.md) section 2 ("scrub a run through time").

## Problem (reported by the maintainer)

The Studio's viewport shows nothing for a run unless the run was started with fields stored. W48-5 (a) decided that Studio-started
runs store fields by default. This ADR checks whether that default really holds for every case type and makes the empty viewport
say why it is empty and what to do.

## Context (read at main f3bf005)

* The default is on in the model: `runs::propose_store` returns `StorePlan { on: true, .. }` for every case that sets no store key
  and has no moving mesh; `StartPlan::set` then adds `output.store_interval` or `output.store_time_interval` and
  `output.store_precision=f32`; the start dialog's checkbox "Store fields for scrubbing (ux, uy, uz, p)" shows `on`
  (`ui.rs` `store_ui`). The runner always passes `--store-dir runs/<id>/fields` (`rcfd-runner` `runner.rs` `launch`), which is where
  the viewport looks (`viewport.rs` `Viewport::sync`). R8 of ADR 0133 showed this end to end for the 12 x 12 cavity.
* Gaps found by reading:
  1. **Compressible.** `propose_store` gives the time rule only to `piso` and `pimple`. A transient `compressible` run (shipped
     `cases/sod/sod.toml`, `cases/wedge/wedge.toml`) gets the step rule from `solver.max_iterations` (default 2000, so every 20
     steps), but it stops at `solver.end_time` with a CFL-limited step (`sim.rs`, `Engine::Compressible`), so the dialog's "about 100
     snapshots" is unrelated to the run (Sod takes about 100 steps: about 5 snapshots). ADR 0130 D4 (accepted as W48-5 (a)) says
     "`output.store_time_interval` (transient)"; ADR 0133's addendum narrowed it to `piso`/`pimple` and asked (its question 4).
  2. **Snapshot count.** `StartPlan::snapshots` returns 100 for every time rule; with R10 (e)'s `dt` 0.01 above the 0.003 interval
     every step is stored (`fieldstore.rs` `after_step`), 30 snapshots. Steady runs that converge early store fewer.
  3. **Size.** `StartPlan::estimated_bytes` counts only field values (cells x 4 x snapshots x 4 or 8 bytes). The store also holds the
     mesh once, a 28-byte head per frame, a JSON directory per snapshot, a footer and `index.json` (`rcfd-tsstore` `lib.rs` format
     comment, `Writer`). For the 12 x 12 cavity that overhead is of the order of the field data.
  4. **Cell count.** Known only for generators with plain counts; the shipped fan cases (`axial_fan_sector`) and mesh files show
     "cell count unknown", so no size and no warning for a large mesh.
  5. **Cases that set their own store keys.** The dialog shows only a weak note "the case sets its own field store"; a case setting
     `store_interval = 0` turns the store off silently. A case setting `output.store_precision = "f64"` still shows the f32 radio.
     `output.store_fields` is ignored by the label and the estimate. A user override `output.store_interval=0` in the overrides box
     wins over the proposal (it comes last in `set()`) while the dialog still shows the store on.
  6. **Chains.** `rcfd run` of a case with `chain.sequence` drives each stage with `out_dir = <out>/<stage>` and no store directory
     (`cmd_w19c.rs` `run_chain_cli`, `chain.rs`), so a chain run started by the runner stores per stage in
     `runs/<id>/output/<stage>/fields/`, which the viewport does not read.
  7. **Runs not started by the Studio.** The plugin's `start_run` tool (`rcfd-mcp` `project.rs`) and `rcfd-runner submit` add no
     store keys; such runs have no store unless the case sets one.
  8. **Viewport.** It looks only at `runs/<id>/fields/` (ADR 0132 integration, decision by reading ADR 0130 D3 (b)); runs made before
     48A have their store at `runs/<id>/output/fields/` (ADR 0132 question 3, accepted as recommended: find them). Every empty state
     shows one text: `no stored fields: <path> not found (start the run with "Store fields" on)`, also for a queued run, a failed run,
     a moving mesh, or a run whose store the solver refused; a store with no snapshot shows "The store has no snapshots yet."
* The shipped cases (`cases/`): ten case files (`axial_fan_cgrid/axial_fan_cgrid.toml`, `axial_fan_sector/axial_fan_sector.toml`,
  `channel/{kepsilon_channel,kepsilon_inlet,profile_inlet,sst_channel}.toml`, `packed_bed/packed_bed.toml`,
  `rotating_channel/rotating_channel.toml`, `sod/sod.toml`, `wedge/wedge.toml`): eight steady `simplec`, two transient
  `compressible`; none sets a store key, none has a moving mesh or a chain; `kepsilon_channel` and `sst_channel` set
  `output.directory`. Six `case.json` files are validation-case descriptions (reference data and citations), not case files.

## Decisions (within accepted decisions; the open points are questions at the end)

* **D1 default on in the model.** The default stays in `StartPlan::new` / `propose_store` (jobs 2 and 4 build plans through the same
  constructor and inherit it); the dialog only draws. R10 and R8 limits are not moved: `set()`, `snapshots()` and
  `estimated_bytes()` keep their results for R10's cases (a)-(g) and R8.
* **D2 transient compressible gets the time rule** (W48-5 (a)'s text). Transient = `piso`, `pimple`, or `compressible` unless
  `compressible.mode = "steady"`; transient with `solver.end_time > 0`: `output.store_time_interval = end_time / 100`; everything
  else: `output.store_interval = ceil(max_iterations / 100)` (unchanged).
* **D3 effective store and snapshot count** (new pure functions; `snapshots()` is left as it was). The effective settings are read
  in this order, the last one winning: the case text (resolved with defaults), the proposal's keys when on, the user's overrides
  (`path=value`, in order). From them:
  * fields: `output.store_fields`, else `ux, uy, uz, p`; precision: `output.store_precision`, else f64 (the proposal sets f32);
  * steady (simple, simplec, coupled; compressible with mode steady), every `k` steps: `n = ceil(M / k)`, "at most" (the run may
    converge earlier);
  * transient, every `dt_s` of simulated time: `B = ceil(T / dt_s - 1e-9)`; with a fixed time step (piso or pimple,
    `solver.max_courant = 0` and none of the step-control keys of `rcfd-api` `timectl::requested` set) `N = ceil(T / dt - 1e-9)`
    and `n = min(B, N)`, exact when `dt >= dt_s`, or when `T / dt_s` is an integer within 1e-9 and `dt <= dt_s`, else "at most";
    with a CFL-limited (compressible) or adaptive step `n = B`, "at most";
  * transient every `k` steps with a fixed step: `n = ceil(N / k)`, exact; otherwise the count is unknown.
* **D4 size estimate** (new; `estimated_bytes()` unchanged). From the format of `rcfd-tsstore` with the default `WriteOptions`
  (raw blocks of 65 536 values): with `C` cells, `F` fields, `w` 4 (f32) or 8 (f64) bytes, `B = ceil(C / 65536)` blocks per field
  and `n` snapshots,
  `bytes = 900 + mesh + n * (115 + F * (113 + 40 B + C w))`,
  where `mesh` is `24 P + 8 (Fm + 1) + 8 FP + 8 Fm + 8 Nm` of the built mesh (points, faces, face-point entries, internal faces)
  when the mesh was built to count its cells, else `264 C` for the 2D generators (`cavity_2d`, `box_2d`, `channel_2d`,
  `wedge_2d`, `nozzle_2d`, `cylinder_ogrid`) and `196 C` otherwise. 900 bytes cover the file header and trailer, the mesh frame's
  head and prefix and its JSON, the footer frame and `index.json`; 115 per snapshot its directory frame head, its JSON frame, its
  footer entry and its `index.json` entry; 113 + 40 B per field and snapshot the field's JSON and its block heads.
* **D5 cell count.** The plain-count table of `case_facts` first (instant); otherwise the mesh is built (`rcfd_api::sim::build_mesh`)
  on a short-lived control thread of the run worker (never in the tick loop) and the count arrives as a new run-worker update that
  fills the open plan's `cells` if it is still the same case; the dialog shows "counting" meanwhile.
* **D6 dialog text, warnings and fixes** (texts registered below, SF9). The checkbox label names the stored fields. A store line,
  a size line, and lines for the case's own output settings are shown under it. Warnings: store switched off (fix: "Store fields");
  the case turns the store off (fix: "Store fields for this run", the proposal's keys as `--set` entries, the case file unchanged);
  an override turns it off (no button: the override is the user's text); more than 100 snapshots (fix: "Store at most 100
  snapshots", the interval re-derived from the effective budget); estimate above **1 GB** (`1 000 000 000` bytes; fix: f32 and the
  largest snapshot count whose estimate fits); moving mesh; chain. A fix changes only the plan.
* **D7 store lookup.** The viewport opens `runs/<id>/fields/store.rts` when it exists, else `runs/<id>/output/fields/store.rts`
  (pre-48A runs), and says so when it uses the second.
* **D8 empty states.** When the selected run has no store, or a store without snapshots, the viewport worker explains it from
  `run.json` (its `set` overrides), `case.snapshot` (both read once per run), `status.json` (each poll) and the `solver` warning
  lines of `events.jsonl` (read again only when its length changed, at most its last 256 KiB). Precedence: record unreadable;
  moving mesh; chain; store not asked for (no store key > 0 in the case text and the `set` overrides, last wins), with the
  turned-off variant when a key is explicitly 0; a solver warning containing "field store"; then by state (queued, paused,
  running, failed, cancelled, finished). Texts registered below (SF7).
* **D9 re-run with fields stored.** Offered under "not stored" and "turned off" only, when the case named in `run.json` is a file
  whose bytes hash to `case_snapshot_hash` (the same case as run; the `fnv1a64:` legacy spelling accepted). The new run is queued
  through the run worker (a new command; the UI thread does no file I/O) with `Client::start(<project>/<case>)`, so the runner
  takes `case_dir` from the case's folder as for the original run (not the old run folder: a case's relative tables and mesh
  files resolve there). Settings: the old run's priority, threads and declared memory; `set` = the store proposal of the case
  (D2, with the interval derived from the effective budget of the old overrides, D3), then the old overrides without any
  `output.store_interval`, `output.store_time_interval` or `output.store_precision` entry. Otherwise the reason is shown and no
  button.
* **D10 no change outside the Studio crate.** `rcfd-api`, `rcfd-cli`, `rcfd-runner`, `rcfd-mcp` and every other crate are not edited,
  so `rcfd run` without store options writes exactly what it wrote before; only Studio-started runs change.

## Placement

Only `rcfd-studio`. New modules `store_plan.rs` (effective store, snapshot count, size, dialog text and fixes, cell count, re-run
plan), `run_fields.rs` (store lookup, empty-state explanation) and `fields_ui.rs` (drawing of both). Small hooks: `runs.rs`
(D2 and three fields), `poll.rs` (the cell-count update, the re-run command), `studio.rs` (two arms, one method), `viewport.rs`
(lookup and explanation in `try_open`, two fields), `viewport_ui.rs` (two arms), `ui.rs` (`store_ui` delegates), `lib.rs`. No new
dependency and no new workspace edge (`rcfd-api`, `rcfd-mesh`, `rcfd-project`, `rcfd-runner`, `rcfd-tsstore` are already
dependencies); `workspace_graph.rs` and `docs/architecture.md` unchanged.

## Registered checks (limits fixed here, before any run)

All in release builds of this branch with `--test-threads 1`; process tests start `rcfd-runner serve` directly with `rcfd` and
`rcfd-runner` of the same build (ADR 0128 rule), `max_runs` 3, `mem_mb` 10 and threads 1 in the plans (as ADR 0133's tests, so
no pre-launch estimate wait; neither changes the store).

| Id | Check | Limit | Negative control (must fail or discriminate) |
|---|---|---|---|
| SF1 | Shipped cases, plan: `StartPlan::new` (project-relative path, the case's folder as base) for the ten case files | every plan has the store on and `set()` exactly: `axial_fan_cgrid` and `axial_fan_sector` `output.store_interval=60`; `kepsilon_channel` 80; `kepsilon_inlet` 4; `profile_inlet` 20; `sst_channel` 90; `packed_bed` 17; `rotating_channel` 17; `sod` `output.store_time_interval=0.002`; `wedge` `output.store_time_interval=0.03`; each followed by `output.store_precision=f32`; the six `case.json` are refused with an error starting `cases/<name>/case.json: not TOML` | the 48D rule (copied in the test) gives `sod` `output.store_interval=20`, not the registered entry |
| SF2 | Shipped cases, store written: each case with its plan's `set()` applied (`chain::apply_set`), `Simulation::new`, `runctl::drive` with `max_steps` 3, `out_dir` and `store_dir` as the runner passes them (`output`, `fields`), `status_monitors` on | 10 of 10: no `warning` event containing "field store"; `StoreView::open` (the viewport's reader) opens the store; at least 1 snapshot; fields `ux, uy, uz, p`; every field `F32` | the cavity with `output.store_fields = ["nosuch"]` gives a "field store disabled" warning and no store |
| SF3 | Cell counts | the plain-count table's count equals `build_mesh(case).n_cells()` for each shipped case it counts; for the two fan cases the Studio model (run worker, no runner) fills the plan's cell count from the background build within 60 s, equal to `build_mesh` | the `nx * ny` formula applied to the fan cases gives 0, not the count |
| SF4 | Estimate against a real store, through the Studio's start path (`Studio::start_run` of `StartPlan::new`, store default untouched), to the end: (a) the 12 x 12 cavity of ADR 0133 (`simplec`, 400 iterations, tolerances 1e-14); (b) the same with `algorithm = "piso"`, `dt = 0.0025`, `end_time = 0.5`; (c) the shipped `sod.toml` | (a): the store's snapshot count equals the plan's count, 100 (D3's maximum; the tolerances keep the run from converging early); (b): equals the plan's count, 100 (exact by D3); (a), (b): the bytes of all files in `runs/<id>/fields/` within 10 % of the plan's estimate (D4); (c): 1 <= snapshots <= the plan's count (100, "at most") and bytes <= 1.10 x the estimate | R10's data-only `estimated_bytes()` misses (a)'s actual bytes by more than 10 % |
| SF5 | The same three runs in the viewport's location | a Studio model selecting each run: `Ready` at `runs/<id>/fields` (not the legacy location), its snapshot count equal to the store's, the last snapshot's `p` face values bit-equal to `face_values_from_cells(read_field(last, "p"))` | (a)'s `fields/` renamed away: the viewport shows SF7's "missing" text, not `Ready` |
| SF6 | Old-location store: the pre-48A fixture run folder of ADR 0130 A4 (`crates/rcfd-project/tests/fixtures/legacy_runner_1248a76/r-20261028T010219-cc06`) in a temporary project, a store written at `runs/<id>/output/fields/` (12 x 12 cavity mesh, 3 snapshots, `p` and `ux`) | the viewport is `Ready` with 3 snapshots, values bit-equal as V4, the legacy note shown exactly; with a second store (2 snapshots) at `runs/<id>/fields/` the contract location wins (2 snapshots) | the pre-change lookup (only `runs/<id>/fields/store.rts`) finds nothing for the folder |
| SF7 | Empty states, exact text: fixture runs (no runner) for each state below, through the explanation function and, for queued, not stored (with re-run), failed and moving mesh, through the Studio model's viewport (`Status::NoStore` text) | every text equal to the registered one character for character, including the "Looked for" line and the re-run lines | the 48C text (copied in the test) is the same for all thirteen fixtures and equals none of the registered texts |
| SF8 | Re-run with fields stored: the cavity started through the Studio with ADR 0133's test plan (`tests/common` `RunnerProject::plan(400, false)`: override `solver.max_iterations=400`, "Store fields" switched off), finished; the viewport offers the re-run; `Studio::rerun_with_fields` | a new run whose `run.json` `set` is exactly `output.store_interval=4`, `output.store_precision=f32`, `solver.max_iterations=400`, whose `case` equals the old run's and whose `case.snapshot` is byte-equal to the old; it finishes and the viewport shows its store `Ready` with 100 snapshots | after the case file is changed, the re-run is refused with the registered "changed" text and no run is created |
| SF9 | Start dialog, pure logic and a drawn frame | texts and `set()` exactly as registered below for (a)-(h); the estimate after the huge fix <= 1 000 000 000 bytes; a headless frame of the start dialog for the cavity draws the checkbox label, the store line and the size line, with the plan's store on; with the store off it draws the warning and the "Store fields" button | the default plan has no warning and the switched-off plan has W-off (the warning check discriminates); a frame drawn before the plan arrived has no store line |
| SF10 | CLI defaults byte-identical | (a) `git diff f3bf005 -- .` outside `crates/rcfd-studio/` and `docs/` is empty (no `Cargo.lock` change); (b) `rcfd run <case> --out <dir> --quiet` with the `rcfd` saved from the f3bf005 build and with this branch's `rcfd`, for the cavity fixture of `rcfd-studio` (`tests/fixtures/cavity.toml`) and `cases/sod/sod.toml`: every file of `--out` byte-equal; (c) `rcfd-cli` `run_flags_w48a` (A6, A7) and `rcfd-api` `baseline_hashes` pass unchanged | this branch's `rcfd` with `--set output.store_interval=4` writes a `fields/` folder the saved binary's run does not have |
| SF11 | Earlier Studio checks | the whole `rcfd-studio` suite (B1-B9, R1-R12 with R6 the ignored known miss, V2-V11, parity) passes unchanged | (their own controls) |

### Registered texts

`{id}` run id, `{case}` the case as in `run.json`, `{reason}` of `status.json`, `{warning}` the solver warning's message,
`{k}` an integer, `{dt}` `f64` printed with `{}`, `{size}` 1 234 bytes as `1 kB` (below 1e6: `{:.0} kB` of bytes / 1e3; below 1e9:
`{:.1} MB`; else `{:.2} GB`), `{a}`, `{b}` the full paths `<run>/fields/store.rts` and `<run>/output/fields/store.rts`.

Viewport (SF7; `Status::NoStore` holds the main text, a newline and the "Looked for" line; a store without snapshots shows the
main text only):

| State | Text |
|---|---|
| queued | `Run {id} has not started yet. Its fields appear here once it stores its first snapshot.` |
| paused | `Run {id} is paused and has not stored a snapshot yet. Its fields appear here once it stores one.` |
| running, store asked for | `Run {id} is running and stores its fields {cadence}. Waiting for the first snapshot.` with `{cadence}` `every iteration`, `every {k} iterations` or `every {dt} s of simulated time` |
| not stored | `Run {id} was started without storing fields, so there is nothing to show.` and the hint `Runs started from Claude Code or the command line store fields only when the case or an override sets output.store_interval or output.store_time_interval.` |
| turned off | `Run {id} was started with the field store turned off ({key} = 0), so there is nothing to show.` |
| failed | `Run {id} failed before it stored its first snapshot: {reason}` (no reason: ends after `snapshot.`) |
| cancelled | `Run {id} was cancelled before it stored its first snapshot.` |
| store refused by the solver | `The solver did not store fields for run {id}: {warning}` |
| moving mesh | `Run {id} uses a moving mesh (dynamic_mesh.enabled), which the field store does not support, so there is nothing to show.` |
| chain | `Run {id} is a chain run: each stage stores its fields in runs/{id}/output/<stage>/fields/, which the Viewport does not read yet.` |
| finished, store asked for, none found | `Run {id} asked for stored fields, but no store was found.` |
| ended, store without snapshot | `Run {id} ended, but its field store holds no snapshot.` |
| `run.json` unreadable | `Run {id}: run.json could not be read ({error}).` |
| "Looked for" line | `Looked for {a} and {b}.` |
| re-run offered | button `Re-run with fields stored`, line `Starts a new run of {case} (unchanged since this run) with the field store on.` |
| case changed | `{case} changed since this run, so it is not re-run; start a new run with Runs > Start run... ("Store fields" is on by default).` |
| case not found | `{case} is not found, so it is not re-run; start a new run with Runs > Start run... ("Store fields" is on by default).` |
| legacy store shown | `Fields read from {dir}: runs made before wave 48A keep their store there.` (`{dir}` = `<run>/output/fields`) |

The thirteen SF7 fixtures: queued; paused; running with the store asked for (`set` with `output.store_interval=4`); running
without store keys (re-run offered); finished with the case text's `store_interval = 0`; failed with a reason; cancelled; failed
with a header-only store (a `Writer` created and dropped); finished with a `solver` warning `field store disabled:
output.store_fields: ...`; a moving-mesh snapshot (the piston case of R10 (f)); a chain snapshot; finished with the store asked for
and no store; the legacy fixture without any store (not stored, case not found). Plus the case-changed variant of the
re-run line.

Start dialog (SF9; `{cells}` an integer):

* store line: `Stores {fields} every {k} iterations: {count}, {precision}.` or `Stores {fields} every {dt} s of simulated time:
  {count}, {precision}.` with `{fields}` comma-separated, `{precision}` `f32` or `f64`, `{count}` `{n} snapshots` (exact),
  `at most {n} snapshots (fewer if the run converges earlier)` (steady), `at most {n} snapshots (the time step follows the CFL
  limit)` (compressible), `at most {n} snapshots (the time step is adaptive)` (adaptive piso/pimple);
* size line: `Estimated size {size} ({cells} cells), in runs/<run id>/fields/ where the Viewport reads it.`; while counting
  `Estimated size: counting the cells of the mesh...`; failed count `Estimated size unknown: the mesh could not be built ({error}).`
* case's own store: `The case sets its own field store ({key} = {value}), which is kept.` then the store line of the case's values;
* fix applied over the case: `Overrides the case's {key} = 0 for this run.`;
* `output.directory` set by the case: `The case's output.directory is not used: the runner writes the results to runs/<run id>/output/.`;
  other `output.*` keys set by the case (not store keys): `The case's own output settings are kept: {keys}.`
* W-off: `Fields will not be stored, so the Viewport will have nothing to show for this run.` (button `Store fields`)
* W-case-off: `The case turns the field store off ({key} = 0), so the Viewport will have nothing to show for this run.` (button
  `Store fields for this run`)
* W-override-off: `The override {entry} turns the field store off, so the Viewport will have nothing to show for this run.`
* W-many: `{n} snapshots will be stored, more than the 100 the Studio proposes.` (button `Store at most 100 snapshots`)
* W-huge: `The store is estimated at {size}, more than 1.00 GB: check that the disk has room, or store fewer snapshots.` (button
  `Store fewer snapshots (about {n})`)
* W-moving: `The case uses a moving mesh (dynamic_mesh.enabled), which the field store does not support, so the Viewport will have
  nothing to show for this run.`
* W-chain: `The case is a chain: each stage stores its fields in runs/<run id>/output/<stage>/fields/, which the Viewport does not
  read yet.`

SF9 cases (the cavity is `tests/common` `RUN_CASE`, 144 cells, `simplec`, 400 iterations):
* (a) default: store on; store line `Stores ux, uy, uz, p every 4 iterations: at most 100 snapshots (fewer if the run converges
  earlier), f32.`; size line `Estimated size 342 kB (144 cells), in runs/<run id>/fields/ where the Viewport reads it.` (D4:
  900 + 264 x 144 + 100 x (115 + 4 x (153 + 576)) = 342 016 bytes); no warning.
* (b) switched off: W-off; after the fix `set()` equals (a)'s.
* (c) the cavity plus `[output] store_interval = 0`: `set()` empty (R10 b0); W-case-off with `output.store_interval`; after the fix
  `set()` = `output.store_interval=4`, `output.store_precision=f32` and the line `Overrides the case's output.store_interval = 0 for
  this run.`
* (d) `box_2d` 1000 x 1000 (`simplec`, 2000 iterations): 1 000 000 cells, interval 20, estimate 1 864 313 600 bytes, W-huge with
  `1.86 GB`; button `Store fewer snapshots (about 45)`; after the fix `set()` = `output.store_interval=45`,
  `output.store_precision=f32` and the estimate 984 141 615 bytes.
* (e) overrides `solver.max_iterations=40000`: W-many with 10000; after the fix `set()` = `output.store_interval=400`,
  `output.store_precision=f32`, `solver.max_iterations=40000`.
* (f) override `output.store_interval=0`: W-override-off with `output.store_interval=0`.
* (g) R10 (e)'s `piso` cavity (`end_time` 0.3, `dt` 0.01): `Stores ux, uy, uz, p every 0.003 s of simulated time: 30 snapshots,
  f32.`
* (h) `sod.toml`: `Stores ux, uy, uz, p every 0.002 s of simulated time: at most 100 snapshots (the time step follows the CFL limit),
  f32.`

## What I already knew (rules c, d)

The code reading above. A release build of main f3bf005 in this worktree (`rcfd`, `rcfd-runner`, `rcfd-studio` and its test
binaries); its `rcfd.exe` was copied aside (sha256 `911b94e2...27c7`) as SF10's previous binary. No test of this ADR and no Studio
test had run in this worktree. Prior data from ADR 0133: R11's stores of the lid cavity (144 cells, 6 snapshots, 4 fields) were
71 355 bytes (f64) and 57 531 bytes (f32), and R8's store of the cavity held 100 f32 snapshots; D4's constants were checked against
R11's sizes by hand (the formula gives about 69 500 and 55 700 for those stores, within 4 %). Nothing of SF1-SF11 was observed.

## Not in CI (view-and-report checklist for the maintainer)

Build: `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`, then `target\release\rcfd-studio.exe <project folder>`.
1. Runs > "Start run...", pick a case: the checkbox "Store fields for scrubbing (ux, uy, uz, p)" is ticked, and under it the
   store line (how often, how many snapshots, f32) and the size line (cells and MB). For a fan case the size line first says
   "counting the cells of the mesh..." and then shows the size.
2. Untick the checkbox: the orange warning and a "Store fields" button appear; the button ticks it again.
3. Type `solver.max_iterations=40000` in the overrides: the "more than the 100" warning and its button appear; the button brings
   the count back to 100. Type `output.store_interval=0`: the override warning appears.
4. Start the run; select it while it is queued and while it runs: the viewport says it has not started / is waiting for the first
   snapshot, then shows the field.
5. Start a run with the checkbox unticked, select it when finished: the viewport says it was started without storing fields and
   offers "Re-run with fields stored"; click it, select the new run when it is finished, the field shows.
6. A run made before wave 48A (store in `runs/<id>/output/fields/`), if one exists: it shows, with the note about the old location.
7. A run started from the Claude Code sidebar with the plugin's `start_run` (no store keys): the viewport says it was started
   without storing fields, with the hint about Claude Code and the command line.
8. Report wording that is unclear, text that does not fit the panel, or a button that does nothing.

## Questions for the maintainer

1. D2 gives transient `compressible` runs the time rule (W48-5 (a)'s wording), which differs from the rule ADR 0133 implemented
   (its question 4). Accept?
2. The "huge" threshold is 1 GB (decimal) and its fix switches to f32 and lowers the snapshot count to fit. Another number, or a
   check of the free disk space (std has no portable call; it would need a dependency or platform code)?
3. The re-run is offered only when the case file is unchanged since the run. Should it also re-run the stored `case.snapshot` when
   the case changed (correct only for cases without relative tables or mesh files)?
4. Runs started from Claude Code (`rcfd-mcp` `start_run`) or `rcfd-runner submit` store no fields unless the case asks. Should the
   plugin's tool propose the store too (a change of ADR 0135's tool, outside this job)?
5. Chain runs store per stage in `runs/<id>/output/<stage>/fields/`; should the viewport offer those stages?
6. The cell count builds the mesh inside the Studio process (a short memory peak for a large mesh file). Accept, or count in a
   child process (`rcfd check-mesh --json`)?

## Files

This ADR. `crates/rcfd-studio/src/{store_plan,run_fields,fields_ui}.rs` (new), `crates/rcfd-studio/src/{runs,poll,studio,viewport,
viewport_ui,ui,lib}.rs` (hooks), tests in new files of `crates/rcfd-studio/tests/`.
