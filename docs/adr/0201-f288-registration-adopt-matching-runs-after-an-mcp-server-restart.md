# ADR 0201: F-288 registration: adopt matching runs after an MCP server restart

Status: **REGISTERED 2026-11-06**, written before any code of F-288. Not yet accepted by the maintainer. Results go in an
addendum at the end.

Parents: [ADR 0189](0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md) (F-274, Q5 and its
decision record of 2026-11-06: option (b)), [ADR 0200](0200-f287-registration-runner-runs-that-continue-from-a-previous-run-with-a-step-cap.md)
(F-287, registered together on branch `f287-f288-runner`; its chained conditions and launch settings are part of the key below),
[ADR 0123](0123-wave47b-run-service.md) and [ADR 0130](0130-wave48a-overview-and-run-record-contract.md) (the runner and the run
record). Roadmap item F-288.

Read at main 9edfbda with ADR 0200 registered. House rules as in ADR 0200. F-288 adds no run-record field: it reads `run.json`,
`status.json`, `metrics.json` and `solver-status.jsonl` only.

## What F-288 asks

The roadmap entry: in project mode, before a design-study candidate is submitted, the server adopts a run of the project's runner
with the same tags (`study`, `variant`, `condition`), case snapshot hash and entries, waiting for it or reading its `metrics.json`
(ADR 0189 Q5 (b)); after the MCP server is stopped during a study and started again, the next call submits no second run for a
candidate that has one, and the adopted records equal those of a study that was not interrupted, within limits fixed here; a
candidate without a matching run is submitted as before.

## What the code read shows

1. **Where records are written.** `Study::eval_points` (`crates/rcfd-api/src/design/study.rs`) appends the records of a batch to
   the store after the whole batch; a server stopped during a batch leaves the batch's runs in the runner and no record of them in
   `evaluations.jsonl`, so the next call evaluates those candidates again (ADR 0189 Q5).
2. **What a run records.** `FsStore::create_run_tagged` (`crates/rcfd-runner/src/store.rs`) writes `case` (project-relative when the
   case file is in the project), `case_snapshot_hash` (`rcfd_project::records::hash_bytes` of the file's bytes, `fnv64:...`),
   `solver_version` (the first line of `rcfd --version` next to the runner, from `Client::submit_tagged`, else `unknown`),
   `settings` (with `set`, and under ADR 0200 `continue_from` and `max_steps`) and the three tags. `run.json` is written once.
3. **The run's state.** `status.json`: `queued`, `running`, `paused`, `finished`, `failed` or `cancelled`. A run is `failed` also
   for reasons outside the evaluation (the runner killed or restarted while the solver died, a refused budget), and `cancelled` by
   request; `metrics.json` is written before the status turns terminal (ADR 0189 item 5).
4. **The evaluator.** `RunnerEvaluator::submit` (`crates/rcfd-workspace/src/design.rs`) checks the entries' settings tree, then
   calls `RunService::submit_tagged`; `wait_result` polls the status and reads `metrics.json`. Under ADR 0200 S6 the conditions of
   a chained candidate are submitted one after another, each knowing the run it continues from.
5. **Run ids** sort by creation second (`r-<UTC yyyymmddThhmmss>-<4 hex>`, `rcfd_project::runid`).

## Choices of this registration

**S1. The key.** A run of the project is a match for a candidate `x` at condition `c` when its `run.json` has:

1. `study`, `variant` and `condition` equal to the tags the evaluator would write (ADR 0189 S6);
2. `case` equal to the base case's project-relative path, as `create_run_tagged` writes it;
3. `case_snapshot_hash` equal to `hash_bytes` of the base case file's bytes now;
4. `settings.set` equal to the entries of `x` at `c` (ADR 0189 S4), in order;
5. `settings.continue_from` and `settings.max_steps` equal to the launch settings the evaluator would give (ADR 0200 S5, S6),
   absent as absent;
6. `solver_version` equal to the version a new run would record (`Client::solver_version`, the text `submit_with` writes; Q1).

`priority`, `threads` and `mem_mb` are not compared: they schedule the run and do not change its result (`rcfd run --threads`:
"results do not depend on it").

**S2. The state (Q2 (a)).** A match that is `queued`, `running` or `paused` is adopted and waited for like a run of the call
(ADR 0189 S4: status every 100 ms, the runner made sure of every second, no time limit); a `finished` match is adopted and its
result read from `metrics.json`; a `failed` or `cancelled` match is not adopted. Of several matches the newest `finished` one is
adopted, else the newest of the others (newest: the largest run id).

**S3. When.** In project mode, for every condition of every candidate the runner evaluates (`design_run_study` and
`design_evaluate`), before the run would be submitted. The project's runs are read once per call, when the source has chosen the
runner: every run folder's `run.json` and `status.json` (a folder that does not read is skipped); the runs this call submits are
not in that index, and the status of a match is read again when it is adopted. A chained condition's key names the run it would
continue from (the run of the condition before, adopted or new), so it can match only after its source matched; the cold repeat of
ADR 0200 S6 has no `continue_from` in its key.

**S4. The result.** A run object in `runs` (ADR 0189 S7, ADR 0200 S10) of an adopted run has `adopted: true`; `runs_total` keeps
its meaning (the runs this call submitted) and the new key `runs_adopted` counts the runs it adopted; `runs` lists both, in the
order of the conditions, at most 50. Without the runner, and without `--project`, nothing changes.

**S5. Not adopted.** Operating maps (`design_run_operating_map`): F-288 names a study's candidates, and an identical map call is
already answered from its table (Q3). Nothing is written by adopting: no file of the adopted run changes; its record is stored
like any evaluation.

**S6. Where the code goes.** `rcfd-workspace` `design.rs` (the index, the key, the choice), `rcfd-runner` `client.rs`
(`Client::solver_version`, which `submit_with` now uses). No MCP descriptor, no Studio file.

## Open questions

**Q1. The solver version in the key.** (a) Compared (as built): a run made by another solver build is not the evaluation the server
would make now; (b) not compared (the three items Q5 (b) names). Recommendation (a).

**Q2. Failed and cancelled runs.** (a) Never adopted (as built: those states also come from outside the evaluation, item 3); (b)
`failed` runs that have a solver result adopted. Recommendation (a): a deterministic failure costs one more run.

**Q3. Operating maps.** (a) No adoption (as built); (b) an untagged key for map points. Recommendation (a).

**Q4. The index.** (a) Every run's `run.json` read once per call (as built); (b) an index file kept by the runner. Recommendation
(a) until a project's run count makes it slow; the time of the read is recorded in A-R.

**Q5. Runs of an earlier definition.** A run made for an earlier study definition in the same folder whose key matches (same case
bytes, entries, launch settings, tags and solver) is adopted: it is the same evaluation. (a) As built; (b) the study's signature
in the key (a new run-record field). Recommendation (a).

## Acceptance checks and limits (fixed here, before any code)

Cases as in ADR 0200: the `lid_driven_cavity` template at 32 x 32 (`cases/cavity.toml`) and 16 x 16 (`cases/small.toml`) with
`max_iterations = 3000`. `REL_EXACT = 1e-12` as in ADR 0200.

* **A-R (a server stopped during a study; the `rcfd-mcp` binary).** A new project with both cases and
  `.rcfd/runner-config.json` `{"max_runs": 1, "cpu_threads": 4, "mem_budget_mb": 65536}`.
  * (i) Through `rcfd-mcp --project`: `design_create_study` with ADR 0189 C4-R's arguments (`designs/nu3`, three points, `workers`
    3), then the request `design_run_study {dir: "designs/nu3"}` is sent and, as soon as `runs/` holds three runs tagged
    `designs/nu3`, the server process is killed before its answer is read; the state of each of the three runs at the kill is
    recorded, and at least one of them had not ended (a set-up condition, asserted). A new `rcfd-mcp --project` process:
    `design_run_study {dir: "designs/nu3"}`. Pass: `evaluations_run` 3, `runs_total` 0, `runs_adopted` 3; `runs/` holds the same
    three folders and no other, each `finished`. Reference: the same definition at `designs/nu3-ref`, run without interruption by
    the second server: `runs_adopted` 0 and three new runs; the three records of each study agree in `params`, `ok`, `status` and
    `iterations`, metrics within `REL_EXACT`; and both agree so with the same study on a server without a project (control: every
    in-process record usable with a metric). The wall time of the index read of the second call is recorded.
  * (ii) The chained definition of ADR 0200 C-CH at `designs/two-chain` (2 candidates, conditions `base` and `slow`), killed as
    soon as `runs/` holds two runs of that study; a new server, `design_run_study` again. Pass: `evaluations_run` 2; afterwards
    `runs/` holds exactly one run of that study for each of the four (variant, condition) pairs; `runs_adopted` equals the number
    of the study's runs at the kill and `runs_total` four minus it; every `slow` run's `continue_from` is the `base` run of its
    variant. Reference: `designs/two-chain-ref`, the same definition without interruption: the records agree as in (i),
    iterations included, metrics within `REL_EXACT` (the same chains: each `slow` run continues from a `base` run of the same case
    and entries).
* **A-N (no adoption on a mismatch; an in-process server with the project, the runner binary).** A project with
  `cases/cavity.toml`, a byte-identical copy `cases/copy.toml` and the runner configuration of A-R with `max_runs` 2; the study
  `designs/nu1` (`nu` in [0.01, 0.02], one condition `base`). Before any call, decoys for `nu = 0.015` at `base` are queued with
  `RunService::submit_with`, each with the full key of S1 but one item: (a) `variant` `nu=0.0151`; (b) `condition` `other`; (c)
  `study` `designs/other`; (d) one more entry, `solver.alpha_p=0.3`; (e) `case_snapshot_hash`, queued while `cases/cavity.toml`
  ended in one more comment line, then restored; (f) `max_steps` 5000; (g) `case` `cases/copy.toml`; (h) `solver_version` changed
  in its `run.json` to `rcfd 0.0.0-decoy`. Each of (a) to (h) is paused (held before launch). Two decoys with the full key: (i)
  cancelled before the call; (j) `failed` before launch (`mem_mb` above the budget), with the runner started and the failure
  awaited before the call. Then `design_evaluate {dir: "designs/nu1", params: {nu: 0.015}}`. Pass: `runs_total` 1,
  `runs_adopted` 0, the one run new and none of the decoys in `runs`; the record equals the in-process one within `REL_EXACT`.
  Positive control (k): a queued run with the full key for the study `designs/pos` (same definition), then `design_evaluate` there:
  `runs_total` 0, `runs_adopted` 1, `runs[0].run` that run and `adopted` true, the call waits for it (it was `queued` when
  indexed), its record equal to the in-process one within `REL_EXACT`. Afterwards the paused decoys are cancelled.
* **A-U (unit, workspace).** The match function on (`run.json`, `status.json`) pairs: a full key matches; each of S1's six items
  changed, alone, does not; the states of S2; of a `finished` and a newer `queued` match the `finished` one is chosen, of two
  `queued` ones the newer.
* **Can fail.** Each made, the named check seen to fail, the file restored and compared byte for byte: (1) no adoption: A-R (i)
  fails at `runs_total`; (2) the hash not compared: A-N fails at (e); (3) `settings.set` not compared: A-N fails at (d); (4) the
  state not checked: A-N fails at (i) or (j); (5) the solver version not compared: A-N fails at (h).
* **Defaults.** As ADR 0200's; F-288 adds no run-record field and no tool; without `--project` nothing changes; the checks of
  ADR 0200 pass unchanged with F-288 built.

## Not in F-288

Adoption for operating maps (Q3); a runner-side index (Q4); adopting runs of another project or of a server without a project
(their studies evaluate in process); the Studio showing which runs a study adopted.
