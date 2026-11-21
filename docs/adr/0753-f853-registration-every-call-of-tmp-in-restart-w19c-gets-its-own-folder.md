# ADR 0753: F-853 registration: every call of `tmp` in `restart_w19c` gets its own folder

Status: **REGISTERED 2026-11-21, written before any code of F-853 and before any run of its checks.**
Before this text no test of the file was run by this work and nothing was changed. What was done: reading, the reasoning
below, and a release build with `--no-run` of the unchanged test binary `restart_w19c` of `rcfd-api`, so that the later
runs do not also compile the dependencies. Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f853-restart-w19c-shared-folder.md` and follows `docs/process/agent-rules.md`,
version 3. Branch `f853-restart-w19c-shared-folder` from main e097520e, scratch
`w49i-work/f853-restart-w19c-shared-folder-scratch`. The record of the tests concerned is
[ADR 0031](0031-wave19c-restart-chain.md) (S1 to S5).

## What is asked

The acceptance of F-853 (`docs/roadmap.json`): "Every limit not stated here is registered in the ADR before the run. The
three tests of crates/rcfd-api/tests/restart_w19c.rs that call s5_numbers() no longer share a folder (tmp(\"s5\") is
keyed by the process id only, and each call removes it), shown by a test or a repeated run that fails before the change;
no assertion and no number of the file changes."

The brief adds: the failure shown before the change by a registered number of repeated runs or by a forced control; the
test file alone changed; every other call of `tmp` looked at for the same fault; no library code changed and
`baseline_hashes` not touched; the registered repeats run after the change; one committed mutation (the shared folder put
back fails the control).

## What was read

- The brief; `docs/process/agent-rules.md` (version 3); `crates/rcfd-api/tests/restart_w19c.rs` whole;
  `docs/adr/0031-wave19c-restart-chain.md`, the registration of S5 (the bullet "S5 coarse-to-fine iterations") and its
  results (the bullets "S5.", "S5a MISS", "S5b PASS"); the entry F-853 of `docs/roadmap.json`.
- `scripts/process_report.py` lines 297 to 511 (the registration lint and the table of checks); `scripts/push_main.py`
  lines 300 to 330 (a mutation patch is applied with `git apply`, its command must exit non-zero, the patch is removed).
- ADR 0750, its head, its table of checks and its sections on size and the decision rule, for the form.

Facts used, from the test file at e097520e:

- `tmp(tag)` (line 36) returns `<temp>/rcfd_restart_w19c_<tag>_<process id>`, after removing that folder with everything
  in it and creating it again.
- `s5_numbers()` (line 309) calls `tmp("s5")`, runs the 12x12 case to its tolerance, saves `c.rcfd` in the folder, runs
  three 24x24 cases without the file (the reference to 1e-10 and two cold starts), and then reads `c.rcfd` six times with
  `restart_from`, with a 24x24 run after each read.
- `s5_numbers()` is called by three tests: `s5_coarse_to_fine_iteration_counts_are_reported`,
  `s5b_iterations_to_distance_are_fewer_than_cold` and the ignored `s5a_iterations_to_tolerance_known_miss`. The tests of
  one file run as threads of one process, so the three calls give the same folder and the same file.
- The other calls of `tmp`: `"s1"` (line 93), `"s2"` (line 124, three times in the loop of one test, one after the other),
  `"s2d"` (line 152), `"s3"` (line 184), `"s4"` (line 219). Each of these tags is used by one test only.

## The fault, by reading

A call of `tmp("s5")` by one test removes the folder of another test that is between its `save_restart` and its last
`restart_from`. The next read of that test finds no file: this is the lead's observation ("The system cannot find the
file specified" after 0.73 s). For it the second test has to start later than the first by more than the first needs for
its 12x12 run, and by less than the first needs in all. Two further forms follow from the same sharing and were not
seen: a `save_restart` between the other test's removal and its `create_dir_all` finds no folder; a read while the other
test writes the same file may see a file that is not complete.

When the tests start together (libtest starts as many as it has threads, and this machine has 16 processors), every call
of `tmp` is made before any file is saved, and the fault needs an unusual delay of one thread. When there are fewer
threads than tests, a test starts when another ends: libtest takes the tests in the order of their names, so with two
threads `s5_coarse_to_fine_iteration_counts_are_reported` starts when one of the tests before it ends, and
`s5b_iterations_to_distance_are_fewer_than_cold`, the next and last in the order, starts when the test that was running
beside it ends (`s4_embedded_mesh_source_case_and_refusals` or an earlier one). If that is inside the window above, the
first S5 test fails. This is a derivation from the order of the names; no time of any test was measured for it, and how
often the window is hit is not known beforehand.

The other tags (by reading, to be confirmed in the addendum): no two tests share one. `"s2"` is taken three times by one
test, in sequence; each pass removes the file of the pass before it, which that pass no longer needs, so this is not the
fault. Two processes never share a folder, because the process id is in the name, and the remains of an earlier process
with the same id are removed by the call. So the fault is expected at `"s5"` alone.

## Design

The test file alone changes; no library file, no other test file.

**D1. `tmp`.** The name of the folder gets the number of the call within the process:
`rcfd_restart_w19c_<tag>_<process id>_<n>`, where `n` comes from a `static AtomicUsize` inside `tmp` that is read and
raised by one with `fetch_add`. `fetch_add` gives every call of a process a different `n`, whichever thread makes it, so
no two calls of one process get one folder, and so no two tests do; this holds for every tag at once and for any test
added later. The removal of what an earlier process left under the same name and the creation stay as they are. The
signature and every caller are unchanged. Not chosen: a tag per test (`"s5"`, `"s5a"`, `"s5b"` handed to `s5_numbers`),
which removes this instance and leaves the same fault open to the next shared helper; a lock around `s5_numbers`, which
makes the three tests run one after the other and keeps the shared folder that the acceptance asks to remove.

**D2. The forced control**, a new test of the file, `tmp_gives_every_call_its_own_folder` (the name sorts after every
other test of the file, so the order in which libtest starts the old tests does not change):

1. `first = tmp("s5")`; write a file `c.rcfd` with the four bytes `f853` into it;
2. `second = tmp("s5")`, which is the call another S5 test makes while the first is between its save and its read;
3. assert that `first != second`, that `first/c.rcfd` still exists and that its bytes are `f853`.

It is the interleaving of the fault, forced: on the unchanged `tmp`, step 2 removes the file and the test fails; after D1
it passes. The tag is `"s5"` on purpose, the tag of the faulty call. The control does not disturb the S5 tests after D1
(its folders are its own); before D1 it is run alone, by its name.

**D3. Header.** The leading comment of the file gets a sentence that names ADR 0753 and F-853 (rule 12). No assertion, no
number, no case text and no other line of an existing test changes. The whole diff of the file is: the header sentence,
the import of the atomic types, the body of `tmp`, the new test.

## The repeats, registered

Each series is run once before the change (on the unchanged file) and once after it, 20 runs each, from the worktree,
with no load added by this work (another agent works on the machine; times are not results). A script in the scratch
folder runs the command, keeps the output of every run and writes one line per run.

| Series | Command | A run counts as failed when |
| --- | --- | --- |
| R1, the three S5 tests in parallel | `cargo test -p rcfd-api --release --test restart_w19c s5 -- --include-ignored` | see below |
| R2, the whole file on two threads | `cargo test -p rcfd-api --release --test restart_w19c -- --test-threads=2` | the command exits non-zero |
| R3, the whole file as the gate runs it | `cargo test -p rcfd-api --release --test restart_w19c` | the command exits non-zero |

R1 is the series the brief names. Its filter `s5` selects exactly the three tests that call `s5_numbers()` (the name of
the control does not contain it). `--include-ignored` is needed to have the third, which is the registered known miss of
ADR 0031: it fails in every run at its own assertion, with a message that starts `S5a ` and contains
` iterations vs cold `. That failure is the record and is not counted. A run of R1 counts as failed when
`s5_coarse_to_fine_iteration_counts_are_reported` or `s5b_iterations_to_distance_are_fewer_than_cold` fails, or when the
panic of `s5a_iterations_to_tolerance_known_miss` is any other than that assertion, or when the three results are not
all printed. The ignored test stays ignored in the file; nothing about it is changed.

R2 is the derivation of the section above put to the test: it is fixed here from the order of the names and is not
searched for after a result. R3 is the run in which the lead saw the failure (1 of 9).

In every run of R1 after the change the lines that `s5_numbers()` prints are compared with ADR 0031 (check N). libtest
shows the output of a test that fails and keeps back that of a test that passes, so the lines read are those of the call
made by the known miss, which fails in every run; `--nocapture` is not added, so that R1 stays the command above. The
calls of the two other tests print the same lines from the same deterministic runs and are not shown.

## Table of checks

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | acceptance, "no longer share a folder ... shown by a test or a repeated run that fails before the change": the control D2, run alone by its name on the unchanged `tmp` (the working tree with the control added and `tmp` as at e097520e) | fails, at the assertion that the file exists or that the folders differ | the acceptance of F-853; the brief ("a forced control, that fails on the base commit") | fails | n/a | code verification (negative control, before the change) |
| A2 | the same control after D1, run alone and in every run of R2 and R3 after the change | passes | the acceptance of F-853; the decision rule of the brief | passes | n/a | code verification |
| B1 | R1, R2, R3 before the change: failed runs of 20 each | none (reported, with the test and the message of each failure) | the brief ("a registered number of repeated runs ... If the registered repeats do not fail, say so with the count") | R1 0 to 2, R3 0 to 4 (the lead saw 1 of 9), R2 more than R3; not known beforehand | n/a | code verification (reported without a limit) |
| B2 | R1, R2, R3 after the change: failed runs of 20 each | 0 of 20 in each | the decision rule of the brief ("the registered repeats all pass after") | 0 | n/a | code verification |
| N | acceptance, "no assertion and no number of the file changes": in every run of R1 after the change the printed S5 numbers are those of ADR 0031 (cold 89 iterations and 23 to the distance; gradient 103, least_squares 96, conservative_overlap 103; 10 to the distance for each); and `git diff e097520e -- crates/rcfd-api/tests/restart_w19c.rs` removes no line outside the body of `tmp` | equal; by reading | the acceptance of F-853; ADR 0031, results, the bullets "S5.", "S5a MISS", "S5b PASS" | equal | n/a | unchanged default output (the printed numbers of the tests) |
| E | `git diff --stat e097520e` names the test file, its mutation patch and files under `docs/adr` only | no other file | the brief ("Change the test file alone"; "no library code changes; `baseline_hashes` is not touched") | holds | n/a | unchanged default output (no library file changes) |
| O | the other calls of `tmp` (`"s1"`, `"s2"`, `"s2d"`, `"s3"`, `"s4"`): which tests use each tag, by `grep` of the file; and the folders a run of the whole file leaves, one per call | none (reported) | the brief ("Look at every other call of `tmp` in the file for the same fault and say what you found") | no tag but `"s5"` is used by two tests | n/a | code verification (reading, reported) |
| M | mutation M1 (below) applied to the changed file: the control fails | fails, at an assertion of the control, and the file compiles | rule 16; the brief ("the shared folder put back fails the control") | fails | caught | code verification (negative control) |
| F | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site` | clean; clean; passes | rule 7 of `docs/process/agent-rules.md` | clean | n/a | code verification (the house checks) |
| P | `python scripts/process_report.py --check` on the last commit | 0 failures | the brief | 0 | n/a | code verification (the process check) |

## Mutation

**M1, the shared folder put back** (the committed one, `crates/rcfd-api/tests/restart_w19c.mutation.patch`, first line
`# expect-fail: cargo test --release -p rcfd-api --test restart_w19c tmp_gives_every_call_its_own_folder`): in `tmp`,
`fetch_add(1, ...)` becomes `load(...)`, so `n` is 0 in every call and the folder is again one per tag and process. The
control must fail at an assertion; a failure to compile would not count. It is run in the worktree with `git apply` and
taken back with `git apply -R`, as the push script does it; no scratch copy of the tree is made, because the patch
touches the test file alone and the tree is clean before and after. One mutation is planned; the fault has one cause.

## Order of the work

1. This file, committed alone.
2. B1: the three series on the unchanged file.
3. The control added to the working tree with `tmp` unchanged; A1 run. Nothing is committed in this state.
4. D1 and D3; A2; then one commit of the test file.
5. B2 with N; M; the patch committed; O, E, F; the addendum; P.

If no run of B1 fails, the addendum says so with the counts, and A1 is the showing of the failure before the change; the
control is registered here already, so no further registration is needed for it.

## Size

Size (agent): done; 0; 0; 0; 60000

(No heavy run: 120 runs of a test file whose tests take seconds, and one build.)

## Decision rule

The brief's:

- **done**: the control fails before the change (A1) and passes after it (A2), the registered repeats all pass after the
  change (B2: 0 of 20 in each of R1, R2, R3), and N, E, M, F and P are met.
- **partial**: the failure is not reproduced before the change (A1 does not fail), or a registered run was not made, or
  a run of B2 fails.
- **stopped and reported**: anything outside the test file would have to change.

B1 has no limit: a series without a failure before the change is reported with its count and does not by itself make the
work partial, since the brief allows the forced control in its place.

## What is not covered

- Why the lead's run was delayed as it was; the cause of the delay is the load of a gate run and is not looked for.
- Runs under an added load, and runs on a hosted runner.
- The folders are not removed when a test ends, as before; after the change a run of the file leaves one folder per call
  (ten in a default run, each with at most a few small restart files) where it left six.
- Other test files that key a temporary folder by the process id alone. They are not searched here.
