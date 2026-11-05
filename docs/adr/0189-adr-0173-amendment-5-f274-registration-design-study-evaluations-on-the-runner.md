# ADR 0189: ADR 0173 amendment 5, F-274 registration: design study evaluations on the runner in project mode

Status: **REGISTERED 2026-11-05**, written before any code of F-274. Not yet accepted by the maintainer.
Parent: [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) (proposed, not accepted),
work item W7, check C4, decision D2, the design section "Design studies on the runner", roadmap item F-274; it builds on
[ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) (F-270),
[ADR 0186](0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) (F-271) and
[ADR 0187](0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md) (F-272, merged at 5fdde2d), and on the
design study driver of [ADR 0067](0067-wave32c-design-loop.md) and F-268. ADR 0173 says that every limit of its checks is fixed by
an amendment before any test or run; this is that amendment for F-274. Results go in the addendum at the end.

Read at main eef6c09. Number: 0188 is reserved for F-273 (ADR 0188 on the unmerged branch `f273-journal-feed`, in the worktree
`w49i-work/f273`); no branch and no worktree holds a 0189. F-274 does not depend on F-273 (S10).

ADR 0173 is not accepted. Where it gives a recommendation, this text uses it and says so; that is a recommendation taken, not a
maintainer decision. Where the code read below raises a question ADR 0173 does not answer, the options and a recommendation are
listed (Q1 to Q8), and work proceeds only on a choice that is reversible.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)): defaults stay bit-identical; no solver result, case
hash or byte of an existing case changes; no case-schema key is added; `crates/rcfd-api/tests/baseline_hashes.rs` is not
re-recorded; no new third-party crate; no commit trailers. The run record gains the additive fields that ADR 0173 names for this
item (S6), written only for the runs of a design study, so every other `run.json` keeps its bytes; ADR 0130 has such a change go
through the lead, and this text registers it for that.

## What F-274 asks

The roadmap entry: in project mode each design evaluation is submitted to the runner as a run tagged with its study and variant
(additive run-record fields), never more than `max_runs` at once; the metrics match the in-process evaluator for the same cases
within the tolerance fixed by an amendment of ADR 0173, and the extra cost per evaluation is measured and recorded (ADR 0173 W7,
check C4). Its note: chained studies need the runner to start a run from a previous result, and runner support for that is not
established.

ADR 0173's design: a `RunnerEvaluator` implements `Evaluator` by submitting each condition as a run and reading `metrics.json`
when it finishes; the design tools use it in project mode. C4: a three-point study through `design_run_study` in project mode
produces three run folders tagged with study and variant, never more than `max_runs` at once, with metrics that match the
in-process evaluator's within a tolerance fixed by the amendment; the extra cost per evaluation is measured and recorded.

## What the code read shows (main eef6c09, nothing run)

1. **The study driver.** `rcfd_api::design::study::Study::eval_points` (`crates/rcfd-api/src/design/study.rs` line 382) evaluates
   the new candidates of a batch with `parallel_map` over `workers` threads (line 264), one `Evaluator::evaluate(def, x)` call per
   candidate (the trait, line 249), which returns one `CondResult` per operating condition; the records are appended to the store
   after the batch. A record's store `index` is given at the append, so it is not known while the candidate runs; its `key` is the
   bit pattern of the parameter values (`store::key_of`).
2. **The in-process evaluator.** `CaseEvaluator::evaluate` (`design/case_eval.rs` line 250) builds the case of each condition
   (`build_case`: every parameter target `scale * x + offset` through `rcfd_api::study::apply_value`, then the condition's `set`
   in order) and runs it with `run_case` (line 89): `runctl::drive` in the calling process, with `RunLimits` (`max_cells`, and
   `max_steps` = the study's `max_steps` or the service's `max_steps_per_call`, the smaller). The status is `CaseRun::status`
   (line 77): `converged` or `finished` with exit 0 is converged, `diverged`, `not_converged`/`step_limit`/`time_limit`, anything
   else failed; a condition is usable when it converged and has at least one finite report value, and its metrics are kept only
   then. With `chain` each condition after the first starts from the previous condition's converged fields (line 264);
   `parse_study` defaults `chain` to true (`design/tools.rs` line 401), so it acts only on a candidate with two or more
   conditions.
3. **The design tools.** `design.run_study` and `design.evaluate` (`design/tools.rs` lines 931 and 951) load the study
   (`load_study`, line 460: the folder through the sandbox, the base case's fingerprint checked, the store's signature), take the
   allowance from `max_design_evaluations`, make the `CaseEvaluator` and open the store. `load_study`, `Loaded`, the signature, the
   summary and the record objects are private to that file. `design.run_map` (an operating map) starts each point from the
   previous converged one by design. The MCP tools `design_run_study` and `design_evaluate` send their arguments unchanged to
   these methods (`crates/rcfd-mcp/src/lib.rs` lines 882 to 917); `dispatch` (line 699) asks the project tools first, and the
   workspace answers none of the design names.
4. **The runner.** `Client::submit` (`crates/rcfd-runner/src/client.rs` line 203) snapshots the case file into a new queued run
   through `FsStore::create_run` (`store.rs` line 129, the `RunStore` trait's signature at line 42); `case_dir` is the case file's
   folder. The runner launches `rcfd run <snapshot> --case-dir <dir>` with one `--set` per entry of `settings.set` and the fixed
   options of `launch` (`runner.rs` lines 599 to 669): no step cap, no `--continue-from`, though `rcfd run` has both
   (`--max-iterations`, `--continue-from`, `crates/rcfd-cli/src/main.rs` lines 53 and 376). The scheduler starts runs while fewer
   than `max_runs` hold a slot and the threads and memory fit (`schedule`, line 617).
5. **What a finished run leaves.** `metrics.json` (`runner.rs` line 945) holds `state`, `reason`, `exit_code`, `steps`, `time`,
   `solver_finish` (the solver's `finish` event: `state`, `exit_code`, `iterations`, `reason`, ...), `last_residuals` and
   `reports` (the report names of the last residual line, each the last value of its series in the solver's `monitors.json`;
   non-finite values are `null`). It is written before `status.json` turns terminal. The run's state is `finished` for exit 0
   *and* exit 5 (not converged, line 900), so the run state alone does not say whether a run converged. A run ended before launch
   (`end_pending`, line 306) has no `solver_finish`.
6. **Two ways of setting a value.** `rcfd run --set path=value` applies `rcfd_api::chain::apply_set` (`chain.rs` line 77): the
   value is JSON when it parses as JSON, else a bare string. `apply_value` (`study.rs` line 260) first tries a whole number below
   9e15 as an integer, then the value as given. So `physics.nu=2.0` and `apply_value(2.0)` can give different trees (2.0 against
   2); the text of an entry decides. serde_json is built with `float_roundtrip` (workspace `Cargo.toml` line 38), so a number
   written in serde_json's shortest form reads back to the same bits.
7. **The run record.** `rcfd_project::records::RunRecord` (`crates/rcfd-project/src/records.rs` line 67) is read through
   `RunRecordWire` (line 95) and built in `Project::create_run` (`project.rs` line 276), `FsStore::create_run`, the `TryFrom` of the
   wire form and the test `crates/rcfd-project/tests/records.rs` (line 43). Optional fields there are written only when set.
   `tests/legacy_w48a.rs` reads records with the strict reader of before ADR 0130.
8. **Thread count.** `rcfd run --threads` "sets RAYON_NUM_THREADS; results do not depend on it" (`main.rs` line 69). The runner
   passes the run's `threads` (default 1); an in-process evaluation uses the server's pool.
9. **Work in other worktrees.** The maintainer's uncommitted work (`studio-case-lineage`) changes `RunSettings` (`threads` optional,
   a `device` field) in `crates/rcfd-runner/src/records.rs`, the runner's `runner.rs` and `main.rs`, the manifest in
   `rcfd-project`'s `records.rs` and `project.rs`, and `rcfd-api`'s `study.rs`, `service.rs` and `runctl.rs`; not
   `rcfd-api/src/design/`, `rcfd-runner/src/client.rs` or `store.rs`. F-273 (branch `f273-journal-feed`) changes the workspace's
   `lib.rs` (`COMMANDS`, the `Workspace` fields), `runs.rs` (`RunService` journals its submissions), `write.rs` and the MCP
   descriptors; it leaves `design_recorded` to F-274 (ADR 0188 J1).

## Choices of this registration

**S1. The route (ADR 0173 W7 and C4, used; Q1 (a)).** In project mode (`rcfd-mcp --project`), the existing tools
`design_run_study` and `design_evaluate` evaluate their candidates on the project's runner through the workspace. Their
descriptors, their arguments and every other design tool are unchanged, and no tool is added, so `tools/list` is byte-identical
with and without `--project`. Without `--project` nothing changes. The MCP server's `dispatch` sends these two calls to
`Workspace::design_call(svc, method, args)` with its own `Service` (the sandbox root and the limits), in place of `svc.call`.

**S2. One study code path (`rcfd-api`).** `rcfd_api::design::tools` gains `call_with(svc, method, params, source)`, the trait
`EvaluatorSource` (`fn evaluator(&self, study: &StudyRun) -> Rpc<Option<Box<dyn Evaluator + '_>>>`) and `StudyRun` (the study
folder, definition, base case file, base case, `chain`, the study's own `max_steps`, and the `RunLimits` the in-process evaluator
gets). `design.run_study` and `design.evaluate` run through it; `None` from the source, or no source (the service's `call`), is
the in-process `CaseEvaluator`, as today. Loading the study, the allowance, the store and its signature, and the summary and record
objects stay the same functions. The source is asked after the study is loaded and the allowance read, before the store is opened,
so an error from it writes nothing. No other `design.*` method changes.

**S3. When the runner evaluates (Q2 (a), Q3 (a)).** The workspace's source chooses the runner when (a) the base case file and the
study folder lie in the project, (b) no run has to start from another (`chain` false, or one operating condition), and (c) the
study sets no `max_steps`. Otherwise it answers `None`: the study is evaluated in the server process as before F-274, and the
result says why (S7). Before it chooses the runner it makes sure the project's runner is alive (`ensure_runner`); a runner that
cannot be started is error -32000 with its message, and nothing is written.

**S4. One run per candidate and condition (`RunnerEvaluator`).** For each condition of a candidate:

* the entries: each parameter target as `path=<scale * x + offset>` in parameter and target order, then the condition's `set` in
  its order; a number that is whole and below 9e15 in magnitude is written as an integer (what `apply_value` tries first), any
  other number in serde_json's shortest form, any other value as its JSON text;
* the check: the base case file read as `rcfd run` reads it (`plan::parse_case`, its folder as the base) with the entries applied
  by `apply_set` must give the same settings tree (`tree.to_json()`) as `CaseEvaluator::build_case` of the candidate at that
  condition; when it does not, the condition fails with a message naming the first differing setting, and no run is made;
* the run: `RunService::submit_tagged(base case file, RunSettings { set: entries, ..RunSettings::default() }, tags)`, so the
  snapshot is the base case's bytes and `case_dir` its folder, as in process.

All conditions of a candidate are submitted, then each run is waited for: its status read every 100 ms, and every 1 s, when no
runner holds the project's lock, `ensure_runner` again (a failure there fails the condition with its message, the run left
queued). The wait has no time limit, as an in-process evaluation has none; `cancel_run` ends it.

**S5. The run's result.** From `metrics.json`: the status by `CaseRun::status`'s rule on `solver_finish.state` and
`solver_finish.exit_code` (no `solver_finish`: failed); `iterations` from `solver_finish.iterations` (0 without); the metrics are
the numeric values of `reports` (a `null` value is left out and named in the message as `report <name>: not finite`); `ok` and the
metrics kept as in `CaseEvaluator::evaluate`; the message is the run's status reason when the condition is not usable. The
runner's budgets (`max_runs`, `cpu_threads`, `mem_budget_mb`) bound these runs; the server's `max_cells` and `max_steps_per_call`
do not, as they do not bound `start_run`.

**S6. The run record (ADR 0173's design, used; Q4 (a)).** `RunRecord` gains three optional fields: `study` (the study folder,
project-relative, for example `designs/nu3`), `variant` (`<parameter>=<value>` for each parameter in definition order, joined by
`,`, each value in serde_json's shortest form, for example `nu=0.015`) and `condition` (the condition's label). Each is written
only when set and read as absent when missing, so every `run.json` written before F-274 or by any other path reads and writes as
before. They are set only by `rcfd_runner::client::Client::submit_tagged` (through `FsStore::create_run_tagged`; `create_run` and
the `RunStore` trait are unchanged) and `rcfd_workspace::runs::RunService::submit_tagged`, a pass-through added at the end of
`RunService`'s methods. `RunSettings` is not changed (item 9).

**S7. The result in project mode.** `design_run_study` and `design_evaluate` keep every key of their result and add `evaluator`
(`runner` or `process`), `evaluator_note` (only with `process`: the rule of S3 that was not met), and with the runner `runs` (at
most 50 objects {`run`, `variant`, `condition`, `state`} of the runs this call made, in submission order) and `runs_total`.
Without `--project` the result is unchanged.

**S8. Workers and `max_runs`.** `workers` stays the number of candidates in flight; the conditions of a candidate are submitted
together, so up to `workers` times the number of conditions runs are queued at once, and the runner's `max_runs` bounds how many
run.

**S9. Where the code goes.** A new module `crates/rcfd-workspace/src/design.rs` (the source, the evaluator, the entries, the result
rule, `Workspace::design_call`); the workspace's `lib.rs` gains only `pub mod design;`; `runs.rs` gains only `submit_tagged`.
`rcfd-mcp` gains the branch of S1 in `dispatch`. No Studio file, no MCP descriptor and no `initialize` text changes; the
`Cargo.lock` does not change (every crate used is a dependency already).

**S10. No journal line (Q6).** F-274 writes no `.rcfd/edits.jsonl` line: the `run_submitted` lines of these runs and the
`design_recorded` kind that ADR 0188 J1 leaves to F-274 need F-273's journal, which is not merged. They are a follow-up once both
are merged.

**S11. Separable commits.** (1) this text; (2) `rcfd-project` and `rcfd-runner`: the run-record fields and the tagged submission,
with their tests; (3) `rcfd-api`: the seam of S2; (4) the workspace module, `submit_tagged` and the MCP route, with their tests;
(5) documents; (6) the addendum. Each builds and passes on its own.

## Open questions

**Q1. The entry point (item 3, D2).** (a) The existing `design_run_study` and `design_evaluate` evaluate on the runner under
`--project` (ADR 0173 W7 and C4's wording; no new tool; the tool list unchanged); (b) new project tools beside unchanged session
tools, which D2 (b) would hide later; (c) an argument of the design tools that chooses the evaluator, which changes their schema
and so the tool list without `--project` (`default_f1.rs`). Recommendation (a); proceeding on (a); reversible (one branch in
`dispatch`).

**Q2. A chained study with several conditions (item 2, the roadmap note).** (a) Evaluated in the server process as before F-274,
the result saying so (`evaluator: process`); (b) refused in project mode with a located message; (c) run on the runner with each
condition started cold, with a note (its results differ from the chained in-process ones); (d) the runner starts a run from a
previous run's result: a run setting that the runner passes to `rcfd run --continue-from`, a change to `RunSettings` and the
launch (item 4), so not in F-274. Recommendation (d) in a later item and (a) until then; proceeding on (a). Operating maps
(`design_run_operating_map`) start each point from the previous one by design, stay in the server process in project mode, and
move with (d).

**Q3. A study's `max_steps`.** (a) Evaluated in the server process, as Q2 (a); (b) a `solver.max_iterations` entry (a steady
case only; a transient run's step cap has no setting); (c) refused in project mode; (d) a run setting for the step cap (a runner
change, as Q2 (d)). Recommendation (d) with Q2 (d), (a) until then; proceeding on (a).

**Q4. The tags.** (a) Three flat fields `study`, `variant` and `condition` (ADR 0173 names the first two; the third tells apart the
runs of one candidate), the variant as the readable parameter list of S6 (exact, since the numbers read back to the same bits);
(b) two fields, the label inside the variant (`nu=0.015@base`); (c) one object `design` {`study`, `variant`, `condition`}; and for
the variant value, the store's key (`key_of`, 16 hex digits per value) in place of the readable list. Recommendation (a); ADR 0130
has the lead approve run-record fields. Proceeding on (a); reversible until a reader depends on them.

**Q5. Runs that outlive the call.** When the MCP server stops during a study, its queued and running runs go on in the runner,
but the store has no record of them, so the next call submits the same candidates again. (a) As built: submitted again; (b)
before submitting, adopt a run with the same tags, case snapshot hash and entries (wait for it, or read its `metrics.json`).
Recommendation (b) in a later item; proceeding on (a).

**Q6. The journal (S10).** (a) Once F-273 is merged, `submit_tagged` writes a `run_submitted` line as `submit` does, and each
evaluation stored writes a `design_recorded` line; (b) no journal line for designs. Recommendation (a), after both are merged;
F-274 writes none.

**Q7. The access class.** In project mode `design_run_study` and `design_evaluate` (class `write_files`) now start runs, which
`start_run` does with class `execute`; a policy `allow` list or a skill's `allowed-tools` that permits them lets Claude start runs.
(a) Left to F-275's project-mode tool list; (b) change their class now (the tool list changes). Recommendation (a); not acted on.

**Q8. A store with records of both evaluators.** The store's signature does not name the evaluator, so a study evaluated without
`--project` and then with it holds records of both. (a) As is: C4-M is what makes them interchangeable; (b) the evaluator in each
record (a change of the store format). Recommendation (a); proceeding on (a).

## Acceptance checks and limits (fixed here, before any code)

The case of C4-R, C4-M and C4-T is the built-in `lid_driven_cavity` template (`LID_DRIVEN_CAVITY`: steady SIMPLEC, upwind,
tolerances 1e-6, the `lid_drag` report) at 32 x 32 cells with `max_iterations = 3000`, as `cases/cavity.toml`.

* **C4-R (runner, the `rcfd-mcp` binary).** A new project with that case and `.rcfd/runner-config.json`
  `{"max_runs": 2, "cpu_threads": 4, "mem_budget_mb": 65536}`, written before any runner starts. Through `rcfd-mcp --project`:
  `design_create_study {dir: "designs/nu3", base: "cases/cavity.toml", name: "nu3", parameters: [{name: "nu", lo: 0.01, hi:
  0.02, path: "physics.nu"}], objectives: [{metric: "lid_drag", sense: "min"}], method: {type: "factorial", levels: [3]},
  workers: 3}`, then `design_run_study {dir: "designs/nu3"}`. Pass: `state` `complete`, `evaluations_run` 3, `evaluator`
  `runner`, `runs_total` 3; `runs/` holds exactly three run folders, each `finished`, each `run.json` with `case`
  `cases/cavity.toml`, `study` `designs/nu3`, `condition` `base`, `settings.set` the one entry `physics.nu=<v>` and `variant`
  `nu=<v>`, the three values `<v>` those of the store's three records; the three runs' creation events come before the first
  terminal state event of any of them (they were queued together), and the largest number of these runs at one time between
  their `running` state event and their terminal state event is exactly 2. A second `design_run_study` gives `evaluations_run` 0
  and no new run folder.
* **C4-M (metrics against the in-process evaluator).** The same case file and study arguments on a server without `--project`
  (its sandbox a separate folder): `design_run_study` in process. Pass: every in-process record is usable (`ok`) with at least one
  metric (control: the comparison is not empty); the two stores hold the same records in the same order with equal `params`,
  `ok`, `status`, `iterations` and metric names; every metric value agrees within |a - b| <= 1e-12 * max(|a|, |b|). The largest
  relative difference is recorded; 0 is expected, since `rcfd run` documents results that do not depend on the thread count
  (item 8) and both paths run `runctl::drive` on the same settings tree (S4's check).
* **C4-T (the extra cost).** Then one `design_evaluate {params: {nu: 0.0175}}` on each server: the wall time of each call and
  their difference (the extra cost of one evaluation on the runner) are recorded, with the wall time per evaluation of the two
  three-point calls; no limit. The two new records are compared as in C4-M.
* **C4-C (conditions).** In the same project, the template at 16 x 16 cells with `max_iterations = 3000` as `cases/small.toml`,
  and a study with `nu` in [0.01, 0.02] at 2 levels and two conditions, `base` {} and `slow`
  {`boundaries.lid.velocity_value`: [0.5, 0.0, 0.0]}: (i) with `chain: false`, `evaluator` `runner` and four runs, each tagged
  with its variant and its condition and its `settings.set` the parameter entry then, for `slow`, the entry
  `boundaries.lid.velocity_value=[0.5,0.0,0.0]`; the records equal the in-process ones as C4-M; (ii) the same definition in
  another folder with `chain` left to its default: `evaluator` `process`, an `evaluator_note` naming `chain`, and no run folder
  added; (iii) the definition of (i) in a third folder with `max_steps: 3000`: `evaluator` `process`, an `evaluator_note` naming
  `max_steps`, and no run folder added.
* **C4-U (unit, workspace).** The entry text: 20.0 gives `20`, 0.015 gives `0.015`, 1e20 gives serde_json's text of it (not an
  integer: above 9e15), a string and an array their JSON text. On the 16 x 16 case each entry applied by `apply_set` gives the tree
  `apply_value` gives, for `mesh.nx` (20.0, an integer setting), `physics.nu` (0.015), `solver.algorithm` ("piso") and
  `boundaries.lid.velocity_value` ([2.0, 0.0, 0.0]); control: the text `mesh.nx=20.0` is refused by `apply_set` or gives another
  tree. The result rule on `metrics.json` texts: `converged` with exit 0 and a report gives a usable condition with that metric;
  `not_converged` with exit 5 (a run the runner calls `finished`) gives `not_converged`, not usable, no metrics; `diverged` with
  exit 6 gives `diverged`; no `solver_finish` gives `failed` with the run's reason; a `null` report is left out and named in the
  message. The variant of (0.015, 2.0) for the parameters `nu`, `u` is `nu=0.015,u=2.0`.
* **C-RR (the run record).** A `RunRecord` without tags serialises with no `study`, `variant` or `condition` key, and with them
  round-trips; a reader of the strict kind of `legacy_w48a.rs` (a struct without the three fields) reads a tagged record; a
  `run.json` text of the form main eef6c09 writes (in the test) reads with the three absent. `Client::submit` writes a `run.json`
  whose keys are those main eef6c09 writes (`id`, `case`, `case_snapshot_hash`, `solver_version`, `settings`, `created`,
  `case_source`, `case_dir`); `Client::submit_tagged` writes those keys plus the three, with the given values.
* **C-1 (parity).** With no edit to them: every test of `rcfd-api` (the design and ledger tests), `rcfd-mcp` (`design_tests.rs`
  T1 to T5, `default_f1.rs`, `project_list_f270.rs`, `project_w48f.rs`, `project_runs_w48f.rs`, `mesh_setup_f272.rs`),
  `rcfd-workspace`, `rcfd-runner`, `rcfd-studio` and `rcfd-project` except `tests/records.rs`, whose one `RunRecord` literal gains
  the three fields set to `None` (needed to build; nothing else changes); `workspace_graph.rs` passes.
* **Can fail.** Three mutations, each restored and the file compared byte for byte with the kept one afterwards: (1) the tags
  dropped from `submit_tagged`: C4-R fails on `study`; (2) the result rule taking the run state `finished` as converged (ignoring
  `solver_finish`): C4-U's not-converged text fails; (3) the chain rule of S3 removed: C4-C (ii) adds run folders.
  `scripts/limit_audit.py` is run on the new tests.
* **Defaults.** No solver result, case hash or existing case byte changes; `baseline_hashes.rs` and the H1 hashes untouched; no
  case-schema key added; the run record gains only the three optional fields, absent from every `run.json` but a design run's;
  `tools/list` byte-identical with and without `--project`; `Cargo.lock` unchanged. The full gate (fmt per package, clippy with
  `-D warnings`, every test in release on a `git archive` copy, one process per test, three jobs) reports no failure; a flaky
  failure is reported as a failure with its name.

## Not in F-274

Chained studies and operating maps on the runner, and a study's step cap there (Q2 (d), Q3 (d): a runner change); adopting runs
that outlive a call (Q5); journal lines for design runs and records (Q6, after F-273); the access class of the design tools and
hiding session tools (Q7, F-275); the Studio showing a study's runs by their tags.

## Where to go next

* [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) for the layer and its decisions.
* [ADR 0067](0067-wave32c-design-loop.md) and [the design loop](../design-loop.md) for studies, the store and the in-process
  evaluator.
* [ADR 0123](0123-wave47b-run-service.md) and [ADR 0130](0130-wave48a-overview-and-run-record-contract.md) for the runner and the
  run record.
* [MCP tools](../mcp-tools.md) for the design and project tools.
