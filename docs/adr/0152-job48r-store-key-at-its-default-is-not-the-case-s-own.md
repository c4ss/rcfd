# ADR 0152: Job 48R, a store key at its schema default is not the case's own: a case saved in the Studio stores its fields when the Studio starts it

Status: **registered 2026-10-28**, before any registered run (what had run before is listed under "What I already
knew"); not yet accepted by the maintainer. Job 48R, on local main 6867e40 (waves 48A-48O merged). House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed here before the registered runs; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before
the corrected run; defaults of existing code stay bit-identical outside the Studio; `crates/rcfd-api/tests/baseline_hashes.rs`
is not re-recorded; no new crate, no unsafe code, the crate graph stays acyclic. Clean-room: our own Studio code.

## The maintainer's decision (2026-10-28, relayed)

[ADR 0149](0149-job48o-linked-selection-drafts-per-case-json-cases.md) question 9, answer **(b)**: the start plan takes a
zero store key that equals the schema's default as not set, so that the Studio's store-fields default applies to a case
saved in the Studio; an explicit non-zero interval stays the case's own. The brief of this job adds: define precisely what
"default-valued" means from the defaults of `rcfd-api` and its schema (`output.store_interval`,
`output.store_time_interval`, `output.store_precision` and the rest); a non-default value is honoured exactly as today; an
explicit zero cannot be told from the default, which is documented in the docs and here with the Start run dialog's
"Store fields" tick as the override; Studio-side planning only, unless the investigation shows otherwise.

## Context (read at 6867e40)

* **The defect** (ADR 0149 addendum deviation 4, found by O10). The case editor's save writes every setting
  (`CaseEditor::to_bytes`: `Case::from_json_in(settings).to_toml()`, or `to_json_string()` for a `.json` case; `rcfd-api`'s
  `Case::to_toml` serialises the whole settings tree), and so does `rcfd-api`'s writer. `rcfd run
  crates/rcfd-studio/tests/fixtures/cavity.toml --print-settings` (the same tree as JSON) shows the `output` table every
  saved case carries: `directory = "results"`, `fields = []`, `retain_checkpoints = 1`, `retain_snapshots = 0`,
  `store_fields = []`, `store_interval = 0`, `store_precision = "f64"`, `store_time_interval = 0.0`, `write_interval = 0`,
  `write_time_interval = 0.0`, `write_times = []`.
* **The schema** (`crates/rcfd-api/src/schema.rs`, the `output` object): `store_interval` integer, default 0, range
  [0, 1e9], "0 = off"; `store_time_interval` float, default 0.0, at least 0, "0 = off"; `store_fields` list of strings,
  default `[]` ("empty = ux, uy, uz, p"); `store_precision` enumeration `f64` | `f32`, default `"f64"`. `rcfd-core`'s
  `settings::coerce` normalises every value before it is kept: an integer field as an integer, a float field's number (an
  integer `0` included) or unit string as an `f64`. So the resolved value (`Case::get`) of `store_time_interval` is `0.0`
  whether the text says `0`, `0.0` or `-0.0`, and a key the text does not set resolves to its default.
* **The solver** (`rcfd-api` `fieldstore::Hook::open`): the store is written when the resolved `store_interval > 0` or
  `store_time_interval > 0`; f32 only for `"f32"`; fields `[]` are the four defaults. For `rcfd run` a key at its default
  and an absent key are the same.
* **The Studio's start plan** reads the store keys from the case text, not from the resolved settings:
  `runs::case_facts_with` sets `sets_store` when the text has `output.store_interval` or `output.store_time_interval`
  (any value) and `sets_precision` when it has `output.store_precision`; `store_plan::case_extra` lists in `case_store`
  every store key the text has. Then `propose_store` makes no proposal when `sets_store`; `StartPlan::set` adds no
  `output.store_precision=f32` when `sets_precision`; `store_plan::effective` applies `case_store` first (an interval at 0:
  `OffBy::Case`); `case_off_key`, `case_lines` and `warnings` give "The case turns the field store off ({key} = 0) ..."
  (W-case-off of ADR 0140) with its fix "Store fields for this run" and, once fixed, the line "Overrides the case's {key} =
  0 for this run."; `fields_ui::store_section` greys the precision radios out "(set by the case)" when `sets_precision`.
* **The plan's users**: Run and F5 (`quick_run::start_job`, `StartPlan::new_with`), the Start run dialog (`poll`,
  `Command::Plan`, `StartPlan::new_with`), and the re-run with fields stored (`store_plan::rerun_plan`: `StartPlan::new`,
  then `retarget`, which makes a proposal whatever the case sets).
* **Other places that read keys from a case's text** rather than its settings: `store_plan::case_extra`'s `output_keys`
  (every other `output.*` key the text has, for the dialog's lines "The case's output.directory is not used ..." and "The
  case's own output settings are kept: ..."); `run_fields` (ADR 0140 D8, the viewport's explanation of a run without
  fields), which reads `case.snapshot` as TOML only: `store_asked` takes an interval key at 0 as "turned off", `read_record`
  reads `dynamic_mesh.enabled` and `chain.sequence`. Everything else the plan reads (`solver.*`, the step-control keys of
  ADR 0140 D3, `compressible.mode`, `mesh.*`, `dynamic_mesh.enabled`, `chain.sequence`) comes from the resolved settings,
  where a saved case equals its source (the `--print-settings` output above: the step-control keys all 0 or false,
  `dynamic_mesh.enabled` false, `chain.sequence` empty).
* **The shipped cases**: the ten files under `cases/` (the case starter's ten examples are these files) set no store key;
  the ten cases of `example-projects/` set their own stores with non-default values (`store_interval` 5, 10, 40 or 200, or
  `store_time_interval = 1.0`; `store_precision = "f32"`; a field list).

## Decisions

1. **"Set by the case", for a store key** (the definition; `store_plan::store_key_set`). A store key is the case's own when
   its value, as `rcfd-api` resolves the case, differs from the schema's default for that key. "Equal" is the same number
   for numbers (`0`, `0.0` and `-0.0` are one value) and the same JSON value otherwise. The defaults are read once from
   `rcfd-api`'s schema (`Case::new()`): `output.store_interval` 0, `output.store_time_interval` 0.0,
   `output.store_fields` `[]`, `output.store_precision` `"f64"`. A key the text does not set and a key the text sets to its
   default (as every case saved by the Studio, or written by `rcfd-api`'s writer, does) are the same: not set. The rule is
   the same for TOML and JSON cases (it reads the resolved settings, not the text).
2. **Scope: the four store keys** (my reading of the brief, which names `store_precision` in the definition; question 1
   offers the narrower reading). `CaseFacts::sets_store` (either interval key set by the rule), `CaseFacts::sets_precision`
   and `CaseExtra::case_store` follow the rule. Consequences:
   * a case whose store keys are at their defaults (saved or not) gets the Studio's proposal (W48-5 (a), ADR 0140 D2): the
     store on, every `ceil(M / 100)` iterations or `T / 100` s, f32; the plan is exactly that of the same case without the
     store keys;
   * a non-default value is the case's own exactly as before: a positive interval (no proposal; "The case sets its own field
     store (...), which is kept."), `"f32"` (no precision key added, the radios greyed out), a field list (stored, named in
     the label);
   * an interval at 0 can no longer turn the Studio's store off, since the only zero of either key is its default. From the
     Studio a run without stored fields is started with the Start run dialog: untick "Store fields" (W-off), or the override
     `output.store_interval=0` in its overrides (W-override-off). Run and F5 have no tick: they always store (a moving mesh
     and a chain as before). An explicit `"f64"` cannot be told from the default either: the dialog's "f64 (exact)" radio is
     its override; Run stores f32;
   * W-case-off, its fix `Fix::StoreForCase`, `OffBy::Case` and the line "Overrides the case's ... = 0 for this run." stay
     in the code; from a valid case text they are reached only through the route before (decision 4);
   * nothing else changes: the re-run (`rerun_plan`, ADR 0140 D9) proposed a store whatever the case set and still does (for
     a saved case its precision is now f32 too); the viewport's explanation (`run_fields`) is unchanged (question 3);
     `rcfd run`, the runner and `rcfd-mcp`'s `start_run` read a case's 0 as off, as before.
3. **The ambiguity documented**: `docs/run-control.md` (Field store) says what the Studio does with a store key at its
   default and how to run without stored fields; this ADR; the doc comments of `CaseFacts` and `store_key_set`.
4. **Test switch**: `runs::PlanRoute` with `toml_only` (ADR 0149's route before) and `default_store_keys_set` (this ADR's
   route before: a store key in the text is the case's own whatever its value), `StartPlan::new_route`,
   `runs::case_facts_route`; `quick_run::Switches::default_store_keys_set` (Run and F5) and
   `poll::Switches::default_store_keys_set` (the Start run dialog's plan). `StartPlan::new_with` and `case_facts_with` keep
   their signatures (the app's route with `default_store_keys_set` off).
5. **No change outside the Studio**: `rcfd-api`, `rcfd-cli`, `rcfd-runner`, `rcfd-mcp` and every other crate are read, not
   edited; `baseline_hashes.rs` is untouched.

## Placement

`crates/rcfd-studio/src/{runs,store_plan,quick_run,poll}.rs`; `docs/run-control.md`; this ADR; dated notes in ADRs 0133,
0140 and 0149. New tests `crates/rcfd-studio/tests/saved_store_r.rs` (DV1-DV6) and `plans_record_r.rs` (DV6 (a)'s
record), a shared module `tests/dv/mod.rs`, the record `tests/fixtures/plans_6867e40.txt`. No new crate, no new dependency,
no new workspace edge; `Cargo.toml`, `Cargo.lock`, `crates/rcfd-cli/tests/workspace_graph.rs` and `docs/architecture.md`
unchanged.

## Registered checks (limits fixed here, before any of them ran)

Release builds of this branch, `--test-threads 1 --nocapture`, output to files, this host; `rcfd-runner` and `rcfd` of
the same build next to the test binaries (`common::binaries`); `TMP` and `TEMP` outside every checkout ("What I already
knew"). Every wait has a 30 s limit (60 s for a run to finish), an upper bound, not a timing claim. "The cavity" is
`common::RUN_CASE` (12 x 12, `simplec`, 400 iterations, tolerances 1e-14, the `lid_drag` report); "the 30-iteration
cavity" is `g4::cavity(30)`; "the piso cavity" is SF4 (b)'s (`piso`, `dt = 0.0025`, `end_time = 0.5`). "Saved by the
editor" means: the case selected in the Studio model (`Studio::open_case`) and settled, `physics.nu` committed as `0.02`
through `CaseEditor::edit_text` (the field's commit), Case > Save (`Studio::save_case`), drained until the editor is clean
and not saving; for a pure check, the same edit through a `CaseEditor` and its `to_bytes` (the bytes a save writes).
"The route before" is `default_store_keys_set` of decision 4.

| Id | Check | Limit | Negative control (must fail) |
|---|---|---|---|
| DV1 | The rule, pure (`StartPlan::new`, `case_facts`): the four schema defaults; the cavity (the piso cavity for the time key) with an `[output]` table of (i) `store_interval = 0`; (ii) `store_time_interval = 0.0`; (iii) `store_time_interval = 0`; (iv) `store_time_interval = -0.0`; (v) `store_precision = "f64"`; (vi) `store_fields = []`; (vii) all four at their defaults; (viii) `store_interval = 7`; (ix) `store_time_interval = 0.05`; (x) `store_precision = "f32"`; (xi) `store_fields = ["p"]`; (xii) `store_interval = 0` and `store_time_interval = 0.05`; and the JSON text of each tree (`{`, as `rcfd run` reads it) | the defaults exactly 0, 0.0, `[]`, `"f64"`; (`sets_store`, `sets_precision`, the keys of `case_store`): (i)-(vii) (false, false, none); (viii) (true, false, `output.store_interval`); (ix) (true, false, `output.store_time_interval`); (x) (false, true, `output.store_precision`); (xi) (false, false, `output.store_fields`); (xii) (true, false, `output.store_time_interval`); each JSON text as its TOML twin; (i)-(vii): the whole `StartPlan` equal to that of the same case without the `[output]` table | the route before: every key in the text is the case's own: `sets_store` true for (i)-(iv) and (vii), `sets_precision` true for (v) and (vii), all four in (vii)'s `case_store` |
| DV2 | Checks 1 and 5, TOML, end to end. A served project (`RunnerProject`, `max_runs` 3) with `cases/s.toml` the 30-iteration cavity; the Studio model with the watcher and the run worker on, Run searching this build's runner. (a) `cases/s.toml` saved by the editor; (b) Run (F5's call): run A, until finished; (c) A on the Run side; (d) the route before (`QuickRun::switches`), Run: run B, until finished; B on the Run side; (e) the model in a drawn app (1280 x 800; a wgpu adapter of this host set on the viewport when there is one, as ADR 0141 G1), A selected, frames until its last snapshot is shown; then B selected, frames until its empty state; (f) A and B compared | (a) the saved text parses with `physics.nu` 0.02 and its `[output]` table holds `store_interval = 0` (an integer), `store_time_interval = 0.0` (a float), `store_precision = "f64"` and `store_fields = []` (else the check is void); (b) A's `run.json` `settings.set` exactly `["output.store_interval=1", "output.store_precision=f32"]`; finished; `runs/<A>/fields/store.rts` holds 30 snapshots (the plan's count), fields `ux, uy, uz, p`, each `F32`; (c) the side Run, the viewport `Ready` for A, 30 snapshots, the last shown, its `p` face values bit-equal to `face_values_from_cells(read_field(29, "p"))` of the store (SF5's rule), no store note; (e) drawn for A: `stored fields of run <A>` and `hover over the surface for its value; <MOUSE_HINT>` (drawn only with a picture); with an adapter the viewport rendered A (its render count up, no GPU error), without one `SKIP` printed; (f) check 5: `case.snapshot` of A and of B byte-equal to each other and to the saved file; `case_snapshot_hash` equal; every `run.json` field other than `id`, `created` and `settings.set` equal; A's `set` is the two store entries followed by B's; `residuals.jsonl` of A and B equal bit for bit (step, time, residuals, monitors), 30 lines | B, the route before (the defect as found): `settings.set` `[]`; no `runs/<B>/fields/store.rts`; the viewport for B `NoStore` (its text reported); drawn for B: the hover line not drawn |
| DV3 | Check 4: DV2 with `cases/j.json`, the 30-iteration cavity written by `rcfd-api`'s writer (`Case::to_json_string`, as O10), saved by the editor (it writes JSON for a `.json` case) | as DV2 (a)-(f), the precondition on the saved JSON's `output` object (`store_interval` 0, `store_time_interval` 0.0, `store_precision` `"f64"`, `store_fields` `[]`) | as DV2's (B's text reported) |
| DV4 | Check 1, the Start run dialog: a project (not served; the run worker on, the watcher off) with `cases/s.toml` saved by the editor as DV2 (a); (a) the dialog's plan (`Studio::plan_start`, through the run worker); (b) drawn: the dialog opened on the case (`UiState::start_dialog`), frames until the plan arrived, then 3 frames | (a) `Ok`; the store on; no note; `set()` exactly `["output.store_interval=1", "output.store_precision=f32"]`; no warning; `sets_precision` false; the case lines reported (question 2); (b) drawn: `Store fields for scrubbing (ux, uy, uz, p)` and `Stores ux, uy, uz, p every iteration: at most 30 snapshots (fewer if the run converges earlier), f32.`; not drawn: `(set by the case)`, the W-case-off text, `Store fields for this run` | a second Studio on the project with `poll::Switches::default_store_keys_set`: (a) no store, the note `the case sets its own field store (output.store_*)`, `set()` empty, the warnings exactly W-case-off for `output.store_interval` with the fix `Store fields for this run`; (b) that warning and its button drawn |
| DV5 | Check 2, explicit non-default keys (pure): the cavity with (a) `store_interval = 7`; (b) `store_fields = ["p"]`; (c) `store_precision = "f32"`; (d) `store_interval = 5` and `store_time_interval = 0.0`; the piso cavity with (e) `store_time_interval = 0.05`; (f) `store_interval = 0` and `store_time_interval = 0.05`; (g) the cavity saved by the editor with `output.store_interval` committed as `7` (every key spelled out, `store_precision = "f64"`); (h) the ten cases of `example-projects/` | each the same in the app's route and the route before: `store`, `note`, `set()`, `settings()`, `effective()`, the snapshot count, the estimate, the store line, the size line, the case lines, the warnings; (h) the whole `StartPlan` equal | the saved cavity of DV2 (a): its `set()` differs between the routes (`[]` against the proposal), so the comparison is not blind |
| DV6 | Check 3, unedited TOML cases, the plan bit-identical: the ten files under `cases/`, the ten cases of `example-projects/`, `common::RUN_CASE` and `common::CAVITY` (22 plans), and as the control the 30-iteration cavity with `physics.nu` 0.02 written by `rcfd-api`'s writer (`Case::to_toml`, every key spelled out). (a) Recorded once on 6867e40, before any code change of this job: per case the `Debug` text of `StartPlan::new`'s result and of its `set()`, written by `plans_record_r.rs` (an ignored test that writes only to a new file named by `RCFD_DV6_RECORD`) and kept as `tests/fixtures/plans_6867e40.txt`; after the change, the same text of each; (b) after the change, the app's route against the route before | (a) the 22 texts byte-equal to the record; (b) the 22 plans equal (`==` and `Debug`), `set()` and `settings()` equal | the saved control: its app-route text differs from the record, and its route-before text equals it (the switch reproduces 6867e40's reading) |
| DV7 | The rest: the whole `rcfd-studio` suite (`fake-claude` built first) with the changed earlier tests below; `cargo clippy -p rcfd-studio --release --all-targets -- -D warnings`; `cargo fmt -p rcfd-studio -- --check`; `rcfd-cli`'s `workspace_graph`; `rcfd-api`'s `baseline_hashes` (run, not re-recorded); `git diff 6867e40` outside `crates/rcfd-studio/` and `docs/` empty and `Cargo.lock` unchanged; `scripts/third_party_licenses.py --check` | pass; the four known misses ignored as before (ADR 0131 B9, ADR 0133 R6, ADR 0141 G1, ADR 0144 J3b) | (their own) |

How the registered runs are made: DV6 (a)'s record first, on 6867e40 with only the recording files added (`cargo test
--release -p rcfd-studio --test plans_record_r -- --ignored`), committed; then the code; then `cargo test --release -p
rcfd-studio --test saved_store_r -- --test-threads 1 --nocapture` and the changed earlier test files, each output to a file;
then the whole `rcfd-studio` suite once with `--no-fail-fast -- --test-threads 1 --nocapture` (`fake-claude` built first),
output to a file; clippy, fmt, `workspace_graph`, `baseline_hashes`, the diff and the licence check on the final commit.

## Changes to earlier tests (registered here, none run yet)

Each follows from the decision; limits not named are unchanged.

* **ADR 0133 R10 (b0)** (`start_plan_r10.rs`): the cavity of R10 with `[output] store_interval = 0`: `set()` exactly
  `output.store_interval=4`, `output.store_precision=f32` (the proposal; before: empty). The registered "empty" is kept as a
  check of the route before.
* **ADR 0140 SF9 (c)** (`store_plan_sf.rs`): the cavity with `[output] store_interval = 0`: the plan of (a): the store on,
  `set()` `output.store_interval=4`, `output.store_precision=f32`, no warning, no case line. The registered limits of (c)
  (`set()` empty; W-case-off with its fix; after the fix (a)'s `set()` and the "Overrides" line, then no warning) are kept,
  checked with the route before.
* **ADR 0140 SF9 (k)** (`case_output_sf9.rs`): the cavity with `store_precision = "f64"`: `"f64"` is the default, so the
  precision is the proposal's: `set()` exactly `output.store_interval=4`, `output.store_precision=f32`; the store line ends
  `, f32.`; the estimate 342 016 bytes (SF9 (a)'s); no case line; no warning; the control's value unchanged (R10's
  `estimated_bytes()` 230 400, against D4's 342 016). The registered limits of (k) (`set()` `output.store_interval=4`, `,
  f64.`, 572 416 bytes) are kept, checked with the route before.
* **ADR 0149 O10** (`json_o.rs`): (b)'s reported text changes (`sets its store false; --set ["output.store_interval=1",
  "output.store_precision=f32"]`; before `true; --set []`), its limit (the text's start) unchanged; in (c)-(d) the JSON
  case's run now stores fields, the limits (the snapshot byte-equal to the file, the residuals bit-equal to the TOML run's)
  unchanged.
* From reading, no other earlier test plans or runs a case that holds a store key at its default: the tests that save a
  case (`save_case`, `to_bytes`) do not plan or run the saved file through the Studio's plan (ADR 0144 J7 runs its saved
  case with `rcfd run --set output.store_interval=1`); SF7's "turned off" fixture is the viewport's explanation
  (`run_fields`), which this job does not change.

## What I already knew (rule d)

* The code reading above.
* `rcfd run crates/rcfd-studio/tests/fixtures/cavity.toml --print-settings` with the base build: the `output` table given
  under Context, the step-control keys of ADR 0140 D3 all 0 or false, `dynamic_mesh.enabled` false, `chain`
  `{"sequence": [], "stages": {}}`, `compressible.mode` `"transient"`.
* **Baseline on 6867e40**: a release build in this worktree's target folder (the dependencies from job 48O's target folder,
  copied; 95 s for `rcfd-studio`, `rcfd-runner`, `rcfd-cli`, `fake-claude` and the test binaries); then the whole
  `rcfd-studio` suite (`--no-fail-fast -- --test-threads 1 --nocapture`, output to a file, 401 s, 23.6 GB free): 50 test
  binaries, 236 passed, 3 failed, 4 ignored. The 3 failures (ADR 0141 G3, two tests of `bins_g4`, and G7 of `oneclick_g4`)
  came from `TMP` pointing inside this worktree's target folder: the binary search walks up from the test's temporary
  folder and found this checkout's `target\release` (and the main checkout's around it). With `TMP` outside every checkout
  the two binaries passed 13 of 13. So the baseline is 239 passed, 0 failed, 4 ignored, as ADR 0149's O13. Every run of
  this ADR uses a `TMP` outside every checkout.
* A review of the approach (no data): the rule defined on the resolved value, the four keys, one switch, the checks above.
* No test of this ADR has been written or run; DV6 (a)'s record does not exist yet.

## Amendment 1 (2026-10-28, DV4's control after the first registered run; before its corrected run)

The first registered run of `saved_store_r.rs` (commit 6563bfa, the code and the tests as registered) passed DV1, DV2,
DV3, DV5 and DV6 and every limit of DV4 ((a) and (b)); DV4's negative control stopped at its warning: the route before
gave W-case-off for **`output.store_time_interval`**, not for `output.store_interval` as registered. The registration's
text was a wrong prediction: `store_plan::effective` applies the case's keys in `STORE_KEYS` order and reports the last
interval at 0 (`OffBy::Case`), and a saved case has both intervals at 0, so 6867e40 names `output.store_time_interval`
for it (SF9 (c), whose text has only `store_interval = 0`, names that key). A defect of the control's set-up, not of the
code: DV6's control showed on the same run that the route before plans the saved case exactly as the record made on
6867e40.

Corrected control of DV4 (the same test; every limit of DV4 unchanged): (a) no store, the note `the case sets its own
field store (output.store_*)`, `set()` empty, the warnings exactly W-case-off for `output.store_time_interval` (`The case
turns the field store off (output.store_time_interval = 0), so the Viewport will have nothing to show for this run.`) with
the fix `Store fields for this run`; (b) that warning and its button drawn. What I knew when writing this: the output of
that run (its values go into the addendum); the corrected control had not run. Every registered test file runs again on
the commit of this amendment.

## Addendum: results against the registered limits (2026-10-28)

Branch `worktree-agent-ae06d5f587d6943c4`. Commits: registration 839ff09 (on main 6867e40); DV6 (a)'s record bbac993 (the
source still 6867e40's); the code, the docs and the tests 6563bfa; amendment 1 6fbdbb7 (the final code); then this
addendum and the dated notes. Every run: release, `--test-threads 1 --nocapture`, output to files, this host (Windows 11,
16 threads, 93.6 GB; 22.4 to 23.9 GB free during the runs), `TMP` in this job's scratch folder (outside every checkout);
`rcfd`, `rcfd-runner` and `fake-claude` built at the baseline from the same sources (no file of theirs changed).
Registered runs: `saved_store_r.rs` on 6563bfa (first) and on 6fbdbb7 (after amendment 1), the changed earlier files
(`start_plan_r10`, `store_plan_sf`, `case_output_sf9`) and `json_o` on 6fbdbb7, then the whole suite on 6fbdbb7 (DV7). The
table gives the 6fbdbb7 values; where the first run differed, it says so.

| Id | Limit | Measured | Result |
|---|---|---|---|
| DV1 | the defaults; the twelve texts' (`sets_store`, `sets_precision`, `case_store`) as registered, the JSON texts the same; (i)-(vii) the plan of the case without the keys | defaults `0` (an integer), `0.0` (a float), `[]`, `"f64"`; (i)-(vii) (false, false, none); (viii) (true, false, `output.store_interval`); (ix) (true, false, `output.store_time_interval`); (x) (false, true, `output.store_precision`); (xi) (false, false, `output.store_fields`); (xii) (true, false, `output.store_time_interval`); every JSON text as its TOML twin; (i)-(vii) the whole `StartPlan` equal to the bare cavity's (piso cavity's), TOML and JSON | pass |
| DV1 control | the route before takes every key in the text as the case's own | (i)-(iv), (vii) `sets_store` true; (v), (vii) `sets_precision` true; (vii) all four keys in `case_store`; (xii) both intervals | fails as required |
| DV2 | (a)-(f) as registered | (a) the saved `cases/s.toml` 15 924 bytes, `physics.nu` 0.02, `store_interval = 0` (integer), `store_time_interval = 0.0` (float), `store_precision = "f64"`, `store_fields = []`; (b) A's `set` `["output.store_interval=1", "output.store_precision=f32"]`, finished, 30 snapshots, each `ux, uy, uz, p` in `F32`; (c) the side Run, `Ready`, 30 snapshots, the last shown (field `p`), bit-equal to the store, no note; (e) `stored fields of run <A>` and the hover line drawn; on the first adapter (NVIDIA GeForce RTX 5080, Vulkan) the viewport rendered A (renders 0 to 2), no GPU error; (f) `case.snapshot` of A and B byte-equal to each other and to the saved file, `case_snapshot_hash` `fnv64:e0fb38f122f2bc1a` for both, the rest of `run.json` equal (`case`, `case_dir`, `case_source`, `case_snapshot_hash`, `settings` without `set`: priority 0, 1 thread, `mem_mb` null, `solver_version`), A's `set` = the two store entries followed by B's (none), 30 residual lines equal bit for bit | pass |
| DV2 control | B, the route before: no `set`, no store, `NoStore`, no picture drawn | B's `set` `[]`, no store file; the viewport `NoStore` (at the moment of the check the model still had B's row as queued: `Run <B> is queued (#1 in the queue); this run stores no fields (the case sets its own field store (output.store_*)), so there is no picture to show; the plots follow it.`; the explanation: `TurnedOff`, `... turned off (output.store_time_interval = 0) ...`); drawn without the hover line | fails as required |
| DV3 | as DV2, the JSON case | (a) the saved `cases/j.json` 21 309 bytes, the four keys at their defaults in its `output` object; (b) as DV2 (30 snapshots, `F32`); (c) as DV2; (e) as DV2 (renders 0 to 2, no error); (f) as DV2, the hash `fnv64:6afa8cec59f36d24` for both, 30 residual lines equal | pass |
| DV3 control | as DV2's | as DV2's; B's explanation `NotStored`, `Run <B> was started without storing fields, so there is nothing to show.` (the explainer reads a JSON snapshot as no keys: question 3) | fails as required |
| DV4 | (a) store on, no note, the `set()`, no warning, `sets_precision` false; (b) the label and the store line drawn, no `(set by the case)`, no W-case-off, no `Store fields for this run` | as registered; the case lines (reported, question 2): `The case's output.directory is not used: the runner writes the results to runs/<run id>/output/.` and `The case's own output settings are kept: output.fields, output.retain_checkpoints, output.retain_snapshots, output.write_interval, output.write_time_interval, output.write_times.` | pass |
| DV4 control | (amendment 1) no store, the note, `set()` empty, W-case-off for `output.store_time_interval` with its fix; drawn | as amended. First run (6563bfa): stopped at the warning, W-case-off named `output.store_time_interval` where `output.store_interval` was registered (amendment 1) | fails as required (after amendment 1) |
| DV5 | the 17 fixtures the same in both routes (the eleven parts of `Seen`); (h) the whole plan | 17 of 17 equal; (g)'s saved text holds `store_interval = 7` and `store_precision = "f64"` and plans as its hand-written twin (no proposal, every 7 iterations, f64, 348 346 bytes, the "which is kept" line, plus question 2's two lines in both routes); (h) 10 of 10 whole plans equal | pass |
| DV5 control | the saved cavity's `set()` differs between the routes | `["output.store_interval=1", "output.store_precision=f32"]` against `[]` | discriminates |
| DV6 | (a) 22 texts byte-equal to the record; (b) the 22 plans equal between the routes | (a) 22 of 22 equal to `plans_6867e40.txt` (sha256 `F5DF79AA...C383`, recorded on bbac993 with the source of 6867e40); (b) 22 of 22 equal (`==`, `Debug`, `set()`, `settings()`) | pass |
| DV6 control | the saved control's app-route text differs from the record; its route-before text equals it | differs (the app's route proposes the store); the route before equal to the record byte for byte | as required |
| DV7 | the whole suite and the rest | DV7_PENDING | DV7_RESULT |

**The changed earlier tests** (registered under "Changes to earlier tests"), on 6fbdbb7: R10 `start_plan_r10.rs` 2 of 2 ((b0) `set()` the proposal; the route before empty); SF1, SF3, SF9 `store_plan_sf.rs` 4 of 4 ((c) the plan of (a), no warning, no case line; the registered (c) with the route before: W-case-off for `output.store_interval`, its fix, the "Overrides" line); SF9 (j)-(l) `case_output_sf9.rs` 4 of 4 ((k) as changed: `set()` the proposal, `, f32.`, 342 016 bytes, the control 230 400; the registered (k) with the route before: `output.store_interval=4`, `, f64.`, 572 416); O10, O11 `json_o.rs` 4 of 4 (O10 (b) now `simplec 30 iterations, Some(144) cells; sets its store false; --set ["output.store_interval=1", "output.store_precision=f32"]`; the JSON case's run stored its fields and its residuals equal the TOML run's, 30 lines).

**Other keys a saved case spells out** (the brief's check of the editor's saved bytes): the `output` table above, read
where the Studio reads a case's text rather than its settings.
* `store_plan::case_extra`'s `output_keys` (the Start run dialog's lines): every saved case shows the two lines measured in
  DV4 (a), because `directory`, `fields`, `retain_checkpoints`, `retain_snapshots`, `write_interval`,
  `write_time_interval` and `write_times` are spelled out at their defaults. Lines only; the run does not change. Question 2.
* `run_fields::store_asked` (the viewport's explanation of a run without fields): a run of a saved TOML case started with
  its keys as written (outside the Studio, or through the route before) is explained "turned off (output.store_time_interval
  = 0)", one of a saved JSON case "not stored" (it reads `case.snapshot` as TOML only): DV2 and DV3's controls. Runs the
  Studio starts now store, so they do not reach it. Question 3.
* Not flipped (they read resolved settings, where a saved case equals its source, or treat the default as absent): the
  step-control keys of ADR 0140 D3 (`fixed_dt`), `compressible.mode`, `solver.*`, `mesh.*`, `dynamic_mesh.enabled` and
  `chain.sequence` (`run_fields::read_record` reads them from the text but takes only `true` and a non-empty list), the
  viewport's run kind (from the stored times), `quick_run` (it reads the plan, not the text). From reading.

**Deviations and notes.**
1. Amendment 1: DV4's control text named the wrong key in the registration (a prediction error: 6867e40 names the last
   interval at 0); corrected before its rerun; every registered file ran again on the amendment's commit.
2. The baseline's 3 failures were the environment (`TMP` inside the worktree's target folder, which the binary search of
   ADR 0141 G3/G7 walks up from), not the code; every registered run used a `TMP` outside every checkout.
3. Decision 2's scope (the four store keys) is my reading of the brief; question 1 offers the narrower one.
4. DV2 and DV3's controls checked `NoStore` while the model's row of B still read queued (the run worker's next tick had
   not come); the limit (`NoStore`, no store, no picture) does not depend on the text, which is reported.
5. `scripts/limit_audit.py` on the touched test files: one hit, `saved_store_r.rs` DV6 (b) `assert_eq!(format!("{a:?}"),
   format!("{b:?}"))` (class v), read: the `Debug` texts of two different plans (the app's route and the route before), not
   a value compared with itself.
6. Nothing outside `crates/rcfd-studio` and `docs` changed (DV7).

**Not run.** A person at the real window (the checklist below); Linux; debug builds of the new tests; the workspace gate
and workspace-wide clippy (only `rcfd-studio` changed); the other crates' own tests beyond `workspace_graph` and
`baseline_hashes`; `cargo deny` (no dependency changed).

## DV7's first whole-suite run and its rerun (2026-10-28, written before the rerun)

The whole `rcfd-studio` suite on 6fbdbb7 (`fake-claude` built first; 438 s; 25.0 GB free at its start): 52 test binaries,
245 passed, **1 failed**, 5 ignored (the four known misses and DV6 (a)'s recorder). The failure: ADR 0134's E11
(`chat_e.rs`, `e11_confirmation_dialog_gates_the_first_chat`) panicked in the test helper `chat_common::Fake::calls`
(`.fake-claude-calls.jsonl` read while a `fake-claude` process was still appending to it: `EOF while parsing a string`
on its last, unterminated line, `unwrap`ped). Not this job's code (the chat model and the fake share no code path with the
start plan); `chat_e` passed 14 of 14 in the baseline and, run alone three times on 6fbdbb7 after this run, 14 of 14 each
time. The rerun: the same command on the same commit, once, its result reported below whatever it is; E11 is not changed
or ignored (question 6).

## Not in CI (by hand)

A person at the real window: a case edited and saved, then Run: the Run side shows the field; Runs > Start run... on the
saved case: "Store fields" ticked, f32 selectable, no orange warning, and unticking it starts a run without fields; the
same with a `.json` case.

## Questions for the maintainer (not decided here)

1. **The scope of the rule** (decision 2). Options: (a) the four store keys (this ADR: a saved case plans exactly as its
   unsaved source; an explicit `store_precision = "f64"` gives way to the Studio's f32 unless "f64 (exact)" is picked in the
   Start run dialog; SF9 (k) changed); (b) the two interval keys only (the decision's words "a zero store key": a saved case
   is stored in f64, twice the size, and the dialog greys the precision out "(set by the case)" although nobody chose it;
   SF9 (k) as registered). Recommendation: (a).
2. **A saved case's other output keys in the Start run dialog** (`case_extra`'s `output_keys`, read from the text): every
   saved case shows "The case's output.directory is not used: ..." and "The case's own output settings are kept:
   output.fields, ..." because the writer spells out their defaults. Lines only, the run is not changed. Options: (a) keep;
   (b) the same rule (a key at its schema default is not listed). Recommendation: (b), a small Studio-only follow-up.
3. **The viewport's explanation of a run without fields** (`run_fields::store_asked`, ADR 0140 D8): it reads
   `case.snapshot` as TOML only and takes an interval at 0 as "turned off". A run of a saved TOML case started outside the
   Studio (Claude Code's `start_run`, `rcfd-runner submit`, `rcfd run`) is explained as "turned off
   (output.store_time_interval = 0)", one of a saved JSON case as "not stored" (with the hint). Options: (a) keep; (b) the
   same rule there (a key at its default: "not stored", with the hint) and the snapshot read as JSON or TOML by the `{` rule
   (SF7's "turned off" fixture changes). Recommendation: (b).
4. **Run and F5 always store** (decision 2): a case whose author meant `store_interval = 0` is stored when run with Run;
   without fields only through the Start run dialog. Options: (a) keep; (b) a Studio preference "Run stores fields".
   Recommendation: (a).
5. ADR 0149 question 9 (c) (the save leaves out settings at their defaults) is no longer needed for the store; it stays with
   ADR 0135's question on edits that rewrite a case in its canonical form.
6. **ADR 0134 E11's helper reads a file another process appends to** (`chat_common::Fake::calls` `unwrap`s every line of
   `.fake-claude-calls.jsonl`, the last one possibly unterminated), so E11 fails now and then (DV7's first run). Options:
   (a) keep; (b) the helper skips an unterminated last line (a test-only change of ADR 0134's helper, with its own
   check). Recommendation: (b), as a small job of its own.

## Files

This ADR; `crates/rcfd-studio/src/{runs,store_plan,quick_run,poll}.rs`; `docs/run-control.md`; new tests
`crates/rcfd-studio/tests/{saved_store_r,plans_record_r}.rs`, `tests/dv/mod.rs`, `tests/fixtures/plans_6867e40.txt`; changed
tests `crates/rcfd-studio/tests/{start_plan_r10,store_plan_sf,case_output_sf9}.rs`; dated notes in ADRs 0133, 0140, 0149.
