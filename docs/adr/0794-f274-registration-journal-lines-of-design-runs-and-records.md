# ADR 0794: F-274 registration: journal lines of design runs and of stored design records

Status: **REGISTERED 2026-11-25, written before any code of F-274's journal lines and before any run of its checks.** Before this
text no test or library code was written and no file of the repository was changed. What was done: reading (below), searches of
the source tree, and a build of the unchanged binaries `rcfd`, `rcfd-runner` and `rcfd-mcp` into this work's own target folder,
which the tests need beside them; after the text was written, the link check `docs_site` the brief asks for, on this text. Results
go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f274-design-journal.md` ([the file](briefs/f274-design-journal.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f274-design-journal` from main 25311db5, scratch
`w49i-work/f274-design-journal-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-274 in `docs/roadmap.json`: "In project mode each design evaluation is submitted to the runner as a run
tagged with its study and variant (additive run-record fields), never more than max_runs at once; the metrics match the
in-process evaluator for the same cases within the tolerance fixed by an amendment of ADR 0173, and the extra cost per evaluation
is measured and recorded (ADR 0173 W7, check C4)." Those clauses were met by ADR 0189 (its addendum 1). The `gap` is "No journal
line for design runs and records yet (ADR 0189 Q6)". The lead decided Q6 (a) (the brief, 2026-11-25): a design evaluation
submitted to the runner writes the `run_submitted` line of ADR 0188 with its study, variant and condition; each evaluation stored
writes one `design_recorded` line, the kind ADR 0173 D4 (c) names; an adopted run gets no second `run_submitted` line; the files
the tools write under `designs/` are in the feed as the workspace's own and not as `external`.

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, each whole;
[ADR 0188](0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) and
[ADR 0189](0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md) (whole);
[ADR 0200](0200-f287-registration-runner-runs-that-continue-from-a-previous-run-with-a-step-cap.md) to its checks,
[ADR 0201](0201-f288-registration-adopt-matching-runs-after-an-mcp-server-restart.md) (whole); D4 and the design section of
[ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md); the items F-273, F-274, F-287 and
F-288 (by a script); `crates/rcfd-workspace/src/{runs,design,journal,feed}.rs`, `crates/rcfd-project/src/{journal,watch}.rs` (whole);
of `crates/rcfd-api/src/design` the store, the map writer of `opmap.rs`, `Study::eval_points`, and the study methods of `tools.rs`;
the `dispatch` of `crates/rcfd-mcp/src/lib.rs`; the submissions of `crates/rcfd-runner/src/client.rs`; the tests
`design_runner_f274.rs` (whole), `adopt_f288.rs`, `design_runner_f287.rs`, `design_f274.rs` (in part), the test helpers, every
place of the test trees that reads `.rcfd/edits.jsonl`; of the Electron Studio `session.ts`, `format.ts` and `project.ts` where they
read the feed; `docs/mcp-tools.md` where it speaks of design runs and the journal; the [tolerance table](../process/tolerances.md).

## Findings of the reading (before any run)

1. **Where a run is submitted.** `Queue::queue` (`design.rs`) returns an adopted run before it submits; every other evaluation goes
   through `RunService::submit_with`, which writes no line. `RunService::submit_tagged` has no caller left in the sources.
2. **Where an evaluation is stored.** In `rcfd-api` (`Study::eval_points` appends a whole batch after the batch was evaluated; a map
   rewrites `map.csv` and `map.json` after each point and `map.meta.json` at the end), which this work does not own. What the
   workspace holds is the call (before and after) and the evaluator, which is asked again at the start of the next batch or point.
   A line written there is written after the store holds the record and before anything of the next batch.
3. **The feed explains a path** by a journal line that names it with the hash the file has now, within 500 ms of its modification
   time (ADR 0188 J6). A line after the whole study would be late for every batch but the last, so finding 2's moments are needed.
4. **Temporary files.** The design tools write `study.json` and the tables through `<name>.tmp` and a rename, a name the watcher
   does not skip (it skips `.*.tmp`). A scan between the two would report the vanished file as `external`.
5. **Two servers in one project.** `rcfd_project::journal::append` gives `seq` under the project-wide lock file; each process
   reads the last `seq` while it holds the lock. The edit lock of a file plays no part. So numbers are unique between processes;
   a writer that does not get the lock in 2 s writes its line without `seq` (ADR 0188 addendum 2). Row S holds it.
6. **No existing test holds that no design line is written.** The tests that read the journal were read; none runs a design tool.
7. **The Electron Studio** keeps `source`, `kind`, `text`, `path`, `run`, `t`, `who` and `actor` of each entry, has no list of kinds
   and shows the entry's `text`; a kind it does not know is shown like any other. Nothing to report and nothing is changed there.
8. **No design tool has a `reason` argument** (only `design_ledger_append` has a property of that name, the reason of a verdict).
   The tool list is a golden, so the lines of the design tools carry no `reason`; `RunService::by(who, reason)` carries one into
   `run_submitted` as it does for `submit`.

## Design (registered)

**D1. `run_submitted`.** `RunService::submit_with` and `submit_tagged` write the line of `submit_launch` after the client accepted the
run, with `study`, `variant` and `condition` added, each only when the run has that tag. Fields: `t`, `kind`, `who`, `seq`, `actor`
(when the service has one), `reason` (when given), `run`, `case`, `set` (and `set_total`), `priority`, `threads`, `mem_mb`,
`continue_from` and `max_steps` when set, and the tags. A refused submission returns before the line. In the design tools `who` is
the tool's name (`design_run_study`, `design_evaluate`, `design_run_operating_map`). A map's run is untagged (ADR 0200 S8), so its
line has no tags.

**D2. `design_recorded`, a stored evaluation.** One line per record of a study's store the call added, in store order: `t`, `kind`,
`who`, `seq`, `actor`, `path` (`<study>/evaluations.jsonl`), `hash` (`hash_bytes` of the file's bytes up to the end of that record's
line, so the last line's hash is the file's), `study` (the project-relative folder), `variant` (as the runs' tag, in the order of
`study.json`), `index`, `source` (`evaluated` or `recorded`), `ok` and `conditions` [{`condition`, `status`}] (at most 100,
`conditions_total` when cut). Written for `design_run_study`, `design_evaluate` and `design_record`, whatever evaluated them
(runner or server process), when the study folder is in the project.

**D3. `design_recorded`, a point of an operating map.** One line per row of `map.json` the call added: `path` (`<out_dir>/map.json`),
`hash` (the table as it stands), `files` [{`path`, `hash`}] (`map.csv`, and `map.meta.json` in the last one), `point`, `label`, `ok`,
`status`. A repeated identical call returns the stored table and writes nothing.

**D4. `design_created`**, an added kind (ADR 0188 allows added kinds), because `study.json` has to be explained from the moment it is
written and it is no stored evaluation: `design_create_study` (`path` and `hash` of `study.json`, `files` with `base.toml` when the
study was made from a session, `study`) and `design_create_variant` (`path` and `hash` of the case file). One line per accepted call.

**D5. When.** The lines of D2 and D3 are written (i) at the start of each evaluation or map point, before anything of it is
submitted, for what the store or table gained since, and (ii) after the call, also one that failed. (i) is in a wrapper around the
evaluator the source gives, also around the in-process one of a study whose base case is outside the project; a map evaluated in
the server process has (ii) only. The lines are appended as every line is (`journal::append`): numbered under the lock, without
`seq` when the lock is not had in 2 s, not at all when the journal cannot be written; the call's result and the store do not
depend on it.

**D6. The route.** `Workspace::design_call` also takes `design.create_study`, `design.create_variant` and `design.record`
(`design::JOURNALED`); `rcfd-mcp`'s one call in `dispatch` asks that list in place of `ON_RUNNER`. Replies are the service's.

**D7. The feed.** `claims_of` takes `hash` of `design_recorded` and `design_created` for `path`, and for each path these kinds name
also the absence of its `.tmp` (finding 4). `journal_text` has a text for the two kinds and names the tags of a tagged
`run_submitted`; the text of an untagged one is unchanged.

**The lines of a call, and their count** (`R` `run_submitted`, `D` `design_recorded`):

| Call | Lines |
|---|---|
| cold study, n candidates at m conditions | n m `R` (tags), n `D` |
| chained study, n candidates at m conditions | n m `R`, conditions 2..m with `continue_from` the run before; n `D`; a run whose restart file does not load is submitted again cold (ADR 0200 S6), one more `R` |
| study with `max_steps` | as above, each `R` with `max_steps` |
| operating map, p points | alternating `R` (no tags; `continue_from` from the second converged one on) and `D`, p each; a cold retry is one more `R` before its `D`; a cached call none |
| study after a restart, all runs adopted | 0 `R`, one `D` per record stored |
| evaluation that fails before it is submitted | 0 `R`, one `D` with `ok` false |
| evaluation whose run fails or does not converge | its `R`, one `D` with `ok` false |
| refused submission | 0 `R`; within a study the evaluation is stored as failed: one `D` with `ok` false |
| cached evaluation (nothing stored) | none |
| `design_record` | one `D` with `source` `recorded` |
| `design_create_study`, `design_create_variant` | one `design_created` |

## Set-up of the checks (fixed here, not searched for)

Cases: the `lid_driven_cavity` template at 16 x 16 with `max_iterations = 3000` (`cases/small.toml`), as in ADR 0189. Runner
configuration `{"max_runs": 2, "cpu_threads": 4, "mem_budget_mb": 65536}`. Studies: `nu3` (`nu` in [0.01, 0.02], factorial 3,
workers 3), `two` (factorial 2, conditions `base` and `slow`, workers 2, as ADR 0200's chained study), `cap` (factorial 2, one
condition, `max_steps` 5), `bad` (`n` on `mesh.nx` in [16, 16.5], `init` 16, factorial 2: 16.5 is refused as "expected integer"
before a run), `nm` (`nelder_mead`, budget 4, workers 1); the map of three lid velocities of ADR 0200 C-MAP. "Binary" is
`rcfd-mcp --project`, "workspace" an in-process `Workspace` with a `Service`; a workspace without a runner evaluates in process
with the base case outside the project (sandbox the parent folder). The whole journal is compared: every line of the file is
accounted for in a row, with `seq` 1, 2, ... in file order.

"Main" (row B) is a record made by the same test on the unchanged library of 25311db5 before the change and committed with it, as
the second commit: per item the FNV-1a hash of its text. Items: `evaluations.jsonl` and `study.json` of `nu3`, `two` and `cap`,
`map.csv`, `map.json` and `map.meta.json`, for each run in the order of (study, variant, condition, `set`) its `run.json` and
`metrics.json`, the replies of the create, run, evaluate, record and map calls and of `tools/list`. Run ids, the project's path,
times, process ids and what a run's duration decides are replaced by tokens or removed (`created`, `case_source`, `case_dir`, and
of `metrics.json` every key but `state`, `reason`, `exit_code`, `steps`, `reports`, `last_residuals` and of `solver_finish` `state`,
`exit_code`, `iterations`, `reason`); the names of the keys of `run.json` are compared whole. The record holds numbers of one
mathematical library: it is asserted on Windows x86_64, where it is made; no Linux record is registered.

No limit on time is set. Rows F are polled after the call that wrote the files has returned, so that a stall of the shared machine
is not measured; what happens inside a call is held by the order of the lines (rows J4, J8).

## Checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| J1 | cold study | binary: `nu3`: create, run | 1 `design_created`, 3 `R`, 3 `D`, the fields of D1, D2 and D4 exactly; tags of each `R` those of its `run.json`; one `D` per record in store order, after the `R` of its candidate; each `hash` that of the store's prefix, the last the file's; `who`, actor `mcp` with the server's pid in every line; a second run call: none | brief, task 1; ADR 0189 Q6 (a) | as stated | exact | code verification |
| J2 | chained study | binary: `two` with `chain` left to its default | 4 `R`, 2 `D`; each `slow` `R` has `continue_from` the run of its variant's `base` `R`, and no `base` `R` has one | brief, task 1 | as stated | exact | code verification |
| J3 | `max_steps`, not converged | binary: `cap` | 2 `R` with `max_steps` 5, 2 `D` with `ok` false and the status of the store's record | brief, task 1 | as stated | exact | code verification |
| J4 | operating map | binary: the map of three points, then the same call again | `R` `D` `R` `D` `R` `D` in this order, the `R` untagged with `continue_from` the run before from the second on, `D` with `point` 0, 1, 2 and the last one's hashes those of the three files; the repeated call none | brief, task 1 | as stated | exact | code verification |
| J5 | adopted after a restart | binary: `nu3` on a server killed once the journal holds three `R`; a second server runs the study | three `R` in all, by the first server's pid; three `D` by the second's; `runs_adopted` 3, `runs_total` 0 | brief, task 1 (an adopted run was submitted once) | as stated | exact | code verification |
| J6 | failed evaluation | binary: `bad` | 1 `R` (`n=16.0`), 2 `D`: index 0 `ok` true, index 1 `ok` false with status `failed` | brief, task 1 | as stated | exact | code verification |
| J7 | refused submission | workspace: `RunService::submit_with` with `continue_from` of no run, with `max_steps` 0, with a case that does not exist; `submit_tagged` with such a case | an error, no run folder, no line; control: an accepted `submit_with` by `by("x", reason)` and an accepted `submit_tagged` one line each, with the tags, the reason where given, the actor, and no `actor` from a service without one | brief, task 1 | as stated | exact | code verification |
| J8 | several batches | binary: `nm` | one `R` and one `D` per record; the `D` of index 0 comes before the last `R`; one `D` per index in order | D5 (brief, task 1: the feed shows each change; ADR 0188 J6's 500 ms) | as stated | exact | code verification |
| J9 | after the store | workspace: a study in process whose `evaluations.jsonl` holds its header and is read-only, so that the append fails | the call fails, the store's bytes unchanged, no `design_recorded`; control: writable again, one `D` per record, each `hash` that of the store's prefix | brief, task 1 | as stated | exact | code verification |
| J10 | journal not appendable | workspace: (a) `.rcfd/edits.jsonl` is a folder; (b) `.rcfd/locks/journal.lock` held | (a) the call's reply equal to that of the same study with a journal, the store's bytes equal, the folder empty; (b) the same, and the one `D` written without `seq` | brief, task 1; ADR 0188 J2 and addendum 2 (best effort, no `seq` without the lock) | as stated | exact | code verification |
| J11 | the other tools | binary: `design_record` of a new candidate, the same again (refused), `design_evaluate` of a stored candidate (cached) and of a new one, `design_create_study` refused (it exists), `design_create_variant`, `design_get_metrics`, `design_list_study` | one `D` `source` `recorded`; none; none; one `R` and one `D`; none; one `design_created`; none; none | D2, D4 (brief, task 1) | as stated | exact | code verification |
| J12 | evaluated in process | workspace: base case outside the project: `nu3`, two batches of `nm`, and the map | 0 `R`; one `D` per record, per point; a study folder outside the project: no line | D2, D3, D5 | as stated | exact | code verification |
| F1 | feed | workspace feed over the binary's project: polled after each of create, run of `nu3`, the map, `design_record`, `design_create_variant`, then twice more at 600 ms | each line once with its `who` and actor, no `external` entry, no path under `designs/` left pending as one; texts as D7; `read_feed {from_seq: 1}` of a second server gives the same lines | brief, task 1 | 0 external | exact | code verification |
| F2 | outside writes | in the same project: `designs/nu3/evaluations.jsonl` with one byte more, `study.json` rewritten with one space more, `designs/new.txt` | one `external` each within 4 polls of 250 ms, path only | brief, task 1; ADR 0188 C3-F (2) | 3 external | exact | code verification |
| F3 | temporary name | workspace: a feed has seen `designs/s/study.tmp`; it is renamed to `study.json` and the `design_created` line appended; polls for 1.2 s; control: another `.tmp` made and removed with no line | no `external` entry; control one `external` | D7 (brief, task 1) | 0, and 1 | exact | code verification |
| S | two servers | two binaries in one project, each running its own study `nu3` at the same time (both calls sent before either reply is read), actors by pid | 2 `design_created`, 6 `R`, 6 `D`, each server's pid on its own 7; every `seq` written once and increasing in file order; lines without `seq` counted, no limit on them (ADR 0188 addendum 2) | brief, task 1; ADR 0188 J2 | no `seq` twice | exact | code verification |
| B | bytes of main | binary: the items of "Main" above, compared with the record of main | every hash equal, the same number of items | brief, task 1; [tolerance table](../process/tolerances.md), bit-identity | equal | exact | unchanged default output |
| N | without `--project` | a server without a project, its sandbox a project folder: create, run of `nu3`, evaluate, record, map, variant | no file under `.rcfd` made or changed, no `run`; the replies have no `evaluator` | brief, task 1 | as stated | exact | unchanged default output |
| X | existing tests | unedited: every test file of `rcfd-workspace` and `rcfd-mcp` whose name holds f273, f274, f287 or f288, `project_list_f270`, `tool_list_f275`, `tool_list_adr0730` and `default_f1` (the tool list), `cargo test -p rcfd-project --release --lib`, `cargo test -p rcfd-workspace --release --lib`, `cargo test -p rcfd-mcp --release --lib` | pass, no file edited | brief, task 4; rules 3 and 7 | pass | exact | unchanged default output |

No check is of kind validation: the acceptance's agreement is with the in-process evaluator (ADR 0189 C4-M, whose test is in row X),
not with a measured case.

The registered run of the two new files is made once:
`cargo test -p rcfd-workspace --release --test design_journal_f274 -- --nocapture --test-threads 1` and
`cargo test -p rcfd-mcp --release --test design_journal_f274 -- --nocapture --test-threads 1`, then row X, then rule 7's checks.
`baseline_hashes` is not run: no solver file is touched. While the code is written a test may be run to find errors of the test
file itself; such runs are not the registered run, and if one shows a defect of a row's set-up, an amendment comes first.

### Mutations (rule 16)

Each alone on a scratch copy of the final code commit with a target folder of its own, the test file of its row run once, the file
restored; the unchanged copy is run too. A mutation the compiler refuses is replaced by an amendment.

| Id | Change | Must fail |
|---|---|---|
| M1 | no line in `submit_with` | J1 |
| M2 | a line also for an adopted run | J5 |
| M3 | no `design_recorded` line for a study's records | J1 |
| M4 | the `design_recorded` line written by the evaluator when it returns, before the store | J9 |
| M5 | the line written before the client answers, so also for a refused submission | J7 |
| M6 | the tags left out of `run_submitted` | J1 |
| M7 | the actor left out of the design calls' `run_submitted` | J1 |
| M8 | the actor left out of `design_recorded` | J1 |
| M9 | `claims_of` without the hash of `design_recorded` and `design_created`: the files under `designs/` are `external` again | F1 |
| M10 | the lines only after the call (D5 (i) left out) | J4, J8 |
| M11 | no `design_recorded` line for a map's points | J4 |
| M12 | no `design_created` line | J1 |
| M13 | `hash` of the whole store in every line in place of the prefix | J1 |
| M14 | the `.tmp` claim of D7 left out | F3 |
| M15 | a stored record of the study before the call's start counted as new (the position at the start 0) | J1 (the second run call), J11 |

Two are committed: M9 as `crates/rcfd-workspace/tests/design_journal_f274.mutation.patch` (its row in the workspace file is the part
of F3's test that looks at `study.json` itself, and J9's control polled through a feed) and M2 as
`crates/rcfd-mcp/tests/design_journal_f274.mutation.patch`; the first line of each is `# expect-fail:` with the command above without
`-- --nocapture --test-threads 1`.

Size (agent): done; 0; 0; 0; 520000

## Decision rule

**done** if every registered row holds, each mutation is caught by its row and the named existing tests pass unedited; **partial**
otherwise, with the piece named. (The brief's rule. Where a mutation names several rows it is caught if each of them fails.)

## What is not covered

- The design ledger (`design_ledger_append`, `design_ledger_export`, `design_uq`, `design_sobol`): its file is not a study's store,
  its entries are not evaluations of F-274, and two of the four tools do not pass the one call of `rcfd-mcp` the brief allows. A
  ledger under `designs/` is still `external` in the feed; proposed to the lead as a follow-up.
- A server killed between the store's append and the line: the record has no line, and no later call writes it (best effort, as
  for every write since ADR 0188). A store's torn last line that a later call cuts off has no line of its own.
- Two servers that evaluate into the same store at the same time: the store itself does not support it (its index order).
- A map evaluated in the server process (base case outside the project) is in the feed as the workspace's own only from the end of
  the call; a feed that polls during it can report its tables as `external`.
- The Studio's display of the new kinds beyond their `text`.

## Open questions

None for the maintainer. Details taken here on the lead's word that details are the registration's: the kind `design_created` (D4);
one `design_recorded` line per map point (D3).
