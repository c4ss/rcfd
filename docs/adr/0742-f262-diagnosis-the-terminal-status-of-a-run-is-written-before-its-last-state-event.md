# ADR 0742: F-262 diagnosis: the terminal status of a run is written before its last state event and before its request files are removed (S7 of ADR 0123 fails intermittently on the held run)

Status: **REGISTERED 2026-11-19, written before any code of this diagnosis and before any run of its checks.** No test of
`rcfd-runner` was run and no library or test code was written before this text; what is below comes from reading the code and
from the lead's counts in the brief. Results go in the addenda at the end; nothing above them is changed after a run.

Brief: `docs/adr/briefs/f262-s7-cancel-of-a-held-run.md`. Rules: `docs/process/agent-rules.md`, version 3. This is a
follow-up of a miss under rule 13: it adds no acceptance clause, it is registered under F-262, and its parent check is S7 of
[ADR 0123](0123-wave47b-run-service.md). Branch `f262-s7-cancel-of-a-held-run` from main b52b882a.

## What was read

- The brief, the rules, ADR 0123 (whole), ADR 0739 Diagnosis 1 (lines 591 to 637).
- `crates/rcfd-runner/tests/runner_w47b.rs` (whole; `folder_problems`, `wait_until`, `wait_state` and S7 closely).
- `crates/rcfd-runner/src/runner.rs` (whole), `store.rs` (whole), `client.rs` (`request`, `wait`, `status`),
  `rcfd-project/src/atomic.rs` (`atomic_write_unsynced`) and `jsonl.rs` (`Appender::open`).

## What is observed (the lead, from the brief)

`s7_cancel_leaves_a_consistent_folder` failed in 3 of 20 runs at main 10ba1a34 and in 3 of 10 with one more merge, release,
Windows, the shared machine under load: 6 of 30, a rate of 0.2. Every failure is on the held run: "last state event
Some(String("paused")) differs from status.json", a left-over `cancel` file, and in two of the six a left-over `pause` file.

## Reading of the code (the hypothesis, and why it is the held run)

**H.** The order of the final writes of a run is in `runner.rs`:

- `finalize` (line 1062) writes `metrics.json` (1133), then calls `set_state` (1134), then `clear_requests` (1135).
- `set_state` (356) reads the old status, **writes `status.json`** (`put_status`, 358) and only then appends the state
  event to `events.jsonl` (363).
- `clear_requests` (385) removes `pause`, `pause.ack` and `cancel`, in that order.
- `end_pending` (367), the end of a run that was never launched, has the same order: metrics, `set_state`, `clear_requests`.

So the terminal `status.json` is visible while the last state event is still the earlier one (`paused` for a held run) and
all three request files are still there. Then the event appears, then `pause` goes, then `pause.ack`, then `cancel`. A
reader that takes the terminal status as the sign that the folder is done can see each of these intermediate folders. The
two observed pictures are two of them: before the event (event `paused`, `pause` and `cancel` left) and, the reader being
part way through its own reads, after `pause` was removed (the event still read as `paused` because `events.jsonl` is read
before the directory is listed; `cancel` left). `store.rs` says as much in its comments at `write_status` and
`append_event`: "a reader that sees the final status.json finds the event and no request files a moment later".

**Why only the held run.** S7 cancels `run`, `held`, `queued` in that order, then waits for the terminal status of each in
that order with a 10 ms poll, then checks the three folders. The runner finalizes `run` before `held` in one pass of
`tick_active` (launch order), and `finalize` is sequential: when the reader has seen `held` terminal, `run`'s folder was
completed before `held`'s status was written. The queued run is ended in `tick_pending`, which runs before `tick_active`
in a tick; if it ends in a later tick the reader waits for it about 100 ms. So `held` is the only run whose terminal status
the reader can see within a few milliseconds of its being written, with `folder_problems(run)` (four file reads and a
directory listing) as the only delay. The window is the runner's time for one append and three removals; it is open for
the reader only when the runner's thread is delayed in it for those milliseconds, which is what a loaded machine does.

**ADR 0123 on the order.** S7's limit reads "all three cancelled within 5.0 s. The folder check passes", and the check
reads the folder as soon as the status is terminal. The contract's other readers (`Client::wait`, the Studio, an MCP
client) have no other sign of the end than the terminal status. So the reading is that the cause is in the library, not in
the test's set-up: a terminal status must mean a finished folder. If the reproduction D1 below passes on the unchanged
library, this reading is refuted and no library change is made.

## Reproduction that does not depend on load

A unit test drives the private `Runner` with a store that wraps `FsStore` and looks at the folder **at the instant of each
write** (the existing unit tests of `runner.rs` wrap the store in the same way: `FailRunning`, `Flaky`). At the write of a
terminal status it records, from the disk: the `to` of the last state event of `events.jsonl`, the `state` of
`metrics.json`, and which of `pause`, `pause.ack`, `cancel` exist. At the append of a terminal state event it records
whether `status.json` on disk is already terminal. No timing is involved: the window is observed from inside it.

Run A is this test file on the unchanged library (commit of the tests, before the remedy). Run B is the same after the
remedy. Each is made once.

## Remedy registered (made only if D1 fails in run A as predicted)

1. **Status last.** The terminal paths (`finalize`, `end_pending` and with it `fail_pending`) go through one function
   that, in this order: reads the old state (for `from`), appends the state event, removes the request files, writes
   `status.json`, and removes the request files once more (a request a client wrote between its own status read and the
   terminal write, `Client::request`, is swept as it was before). `metrics.json` stays before all of them. Non-terminal
   state changes (`paused`, `resumed`, `adopted`, `held before launch`) keep `set_state` as it is.
2. **What is written does not change.** The values given to `write_metrics`, `append_event` and `write_status` are the
   ones of today for every run, cancelled or not; only the order of the calls changes, and with it the order of the two
   time stamps (`t` of the event is now not later than `updated` of the status). Check B1 shows the bytes.
3. **The crash window of the new order.** A runner killed between the removal of `cancel` and the status write leaves
   a run `running` or `paused` with a pid; the next runner adopts it and finalizes it again. For a finished or failed run
   that gives the same state and a second terminal state event. For a cancelled one `finalize` would no longer find the
   `cancel` file and would call the clean stop a failure. So `finalize` takes a run as cancelled on request when the
   `cancel` file exists **or** `events.jsonl` has a `cancel_requested` event (read only when the solver's finish says
   `interrupted` and the file is absent). The order before the remedy has its own crash window (status terminal, event
   missing and request files left for good); neither order is atomic.

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the double removal of item 1 and
item 3. Neither changes a default result, a golden or a hash table.

No source outside the repository is used; the design follows from the order of the writes (first principles) and from the
same remedy of ADR 0739 Diagnosis 1 for `site.json`.

## Checks

New files: `crates/rcfd-runner/src/runner/tests/f262.rs` (unit, a child module of `runner::tests`, so it reads the private
`Runner` and the existing helpers; one line `mod f262;` is added to that module and nothing else in it is touched) and
`crates/rcfd-runner/tests/cancel_held_f262.rs` (process test). `runner_w47b.rs` is not edited. Release profile.

"Final folder at the terminal status write" means: last state event `to` equals the status state, `metrics.json` has that
state, none of the three request files exists; and at the append of the terminal state event `status.json` is not terminal.

| id | check | limit | source of the limit | estimate | margin | kind |
| --- | --- | --- | --- | --- | --- | --- |
| D1 | adopted run, status `paused` with a pid, a `paused` state event, `pause`, `pause.ack`, `cancel` present, stream of 5 steps and a finish `interrupted` (exit 7), two missed probes | ends `cancelled`; final folder at the terminal status write; exactly one terminal state event | ADR 0123 S7 (parent acceptance) | run A: fails with last event `paused` and three files left; run B: met | exact | code verification |
| D2 | never launched: queued run with `pause` and `cancel`, one `tick_pending` | ends `cancelled`, `launched: false`; final folder at the terminal status write | ADR 0123 S7 | run A fails, run B met | exact | code verification |
| D3 | adopted run, finish `converged` (exit 0), two missed probes | ends `finished`; final folder at the terminal status write | ADR 0123 S7 (the folder check, any terminal state) | run A fails (event), run B met | exact | code verification |
| D4 | adopted run without a finish event, two missed probes | ends `failed`; final folder at the terminal status write | as D3 | run A fails (event), run B met | exact | code verification |
| D5 | the crash window of remedy 3: as D1 but the earlier runner got as far as the event and the removals (a `cancel_requested` and a `cancelled` state event in `events.jsonl`, no request file, status `paused` with a pid) | ends `cancelled`, reason "cancelled on request"; control: the same without the `cancel_requested` event ends `failed` | remedy 3 of this ADR, from ADR 0123 decision 4 (a stopped solver of a cancelled run is `cancelled`) | run A fails (`failed`), run B met | exact | code verification |
| B1 | the runs of D3 and D4 (not cancelled), after the end: `status.json` equals the pretty JSON of the status built in the test from the pre-remedy formula with the file's own `updated`; `metrics.json` equals the bytes `atomic::write_json` gives for the metrics built in the test; the last line of `events.jsonl` equals the state event built in the test with the line's own `t`, and the end added exactly that one line after the forwarded solver events; the file names of the folder are the expected list | byte equality | the brief (F-848's acceptance: local records unchanged for a run that is not cancelled); rule 3 | met in run A and in run B with the same test text | exact | unchanged default output |
| E1 | process test, one project, `max_runs` 1, 20 cycles: submit a long run, wait `running` at step >= 5, pause, wait `paused`, cancel, poll `status.json` every 1 ms, and at the first terminal read check the folder with a copy of `folder_problems` | 20 of 20 `cancelled` with no problem; each within 5.0 s of the cancel | ADR 0123 S7 | run B: 0 problems; run A: recorded, no limit (depends on load) | exact | code verification |
| E2 | `cargo test -p rcfd-runner --release --test runner_w47b s7_cancel` 20 times in a row, after the remedy | 20 of 20 pass | ADR 0123 S7; the count from the lead's rate: at 0.2 per run an unremedied library passes 20 in a row with probability 0.8^20 = 0.012 | 20 of 20 | 0.012 chance of a false pass | code verification |
| X | rule 7: `--lib` of `rcfd-runner`; `runner_w47b` (whole, once), `runner_wfix`, `contract_w48a`, `run_records_adr0734`, `continue_f287`; fmt and clippy of `rcfd-runner` | all pass, clean | rule 7 | pass | - | code verification |

E2 at 20 runs of about 10 s is a loop of a few minutes; E1 is expected under one minute and is not ignored.

**Negative controls.** D1 to D4 in run A (the unchanged library must fail them, D1 with the picture of the S7 failures);
the control inside D5; in E1 the folder checker is given a folder with a planted `cancel` file and must report it.

**Planned mutations (rule 16), on a scratch copy.** M1, committed as
`crates/rcfd-runner/src/runner/tests/f262.mutation.patch`: the terminal status written before the state event (the order
before the remedy); D1 to D4 must fail. M2: the request files removed only after the status; D1 and D2 must fail. M3: the
`cancel_requested` reading removed; D5 must fail. M4: the reason of the terminal state event changed; B1 must fail.

Size (agent): partial; 0; 0; 0; 215000

## Decision rule

- **The hypothesis** is confirmed if D1 fails in run A with last state event `paused` and request files present at the
  terminal status write; it is refuted if D1 passes in run A. If refuted: no library change, the result is reported with
  what run A showed, and an amendment of ADR 0123 is proposed if the cause is then the test's reading.
- **The remedy** is done if in run B every row D1 to D5, B1, E1, E2 and X is met. If a row is missed it stays as an ignored
  `*_known_miss` with its limit and the result is partial.
- **F-262** stays partial in either case (S5 is an ignored known miss; the gap about socket control and runs started from
  the application is not touched). With the remedy done, the status proposed for this diagnosis is closed and the note of
  F-262 gets one entry.

## Limits (what is not covered)

- Non-terminal changes keep the order status, then event: a reader that sees `paused` or `running` may read the event a
  moment later. S6 and the other checks wait on the status only.
- Runs on a remote site (`site/remote.rs`, ADR 0739) write their own terminal status and are not touched.
- A runner killed inside the final writes: remedy 3 covers the state of a cancelled run; the adopted run gets a second
  terminal state event. A failed terminal status write is repeated on every tick as before (ADR 0229 D4); until it
  succeeds the folder has the event and no terminal status.
- A request file written by a client after the last removal (the client read a non-terminal status just before) stays, as
  before the remedy.
- Nothing is synced to disk in a new place; the Unix paths are compiled and not run here.

## Addendum 1: results (2026-11-19)

Release profile, Windows, the shared machine. Run A at eebc653f (the checks on the unchanged library), run B at f62c532c
(the remedy). Each run was made once; no limit and no set-up was changed, and there is no amendment.

**The hypothesis is confirmed.** In run A, D1 saw at the write of the terminal status: last state event `paused`,
`metrics.json` already `cancelled`, and `pause`, `pause.ack` and `cancel` present; the `cancelled` event was appended
while `status.json` was terminal. That is the picture of the S7 failures, without any load.

| id | limit | run A (before the remedy) | run B (after) | met |
| --- | --- | --- | --- | --- |
| D1 | final folder at the terminal status write, one terminal event, `cancelled` | fails as predicted: last event `paused`, three request files | event first with status not terminal; status onto last event `cancelled`, metrics `cancelled`, no request file; 1 terminal event | yes |
| D2 | the same, never launched | fails: last event `queued`, `pause` and `cancel` present | met, `launched: false` | yes |
| D3 | the same, `finished` | fails: last event `queued` | met | yes |
| D4 | the same, `failed` | fails: last event `queued` | met | yes |
| D5 | `cancelled`, "cancelled on request"; control `failed` | fails: `failed` ("the solver ended in state 'interrupted' (exit 7)") | `cancelled` ("cancelled on request; the solver stopped cleanly after step 5"); control `failed` | yes |
| B1 | byte equality of `status.json`, `metrics.json`, the last event line, the event kinds, the file names (a finished and a failed run) | passes | passes, same test text | yes |
| E1 | 20 of 20 without a problem, each within 5.0 s | 0 of 20 with a problem, slowest 0.234 s | 0 of 20 with a problem, slowest 0.131 s; the planted `cancel` is reported | yes |
| E2 | S7 passes 20 of 20 | not run (registered for after the remedy) | 20 of 20, slowest "three cancels" 0.180 s | yes |
| X | pass, clean | - | `--lib` 35 passed, 1 ignored; `runner_w47b` 13 passed, 1 ignored (S5); `runner_wfix` 6; `contract_w48a` 2; `run_records_adr0734` 2; `continue_f287` 1 passed, 1 ignored; `cargo fmt -p rcfd-runner --check` clean; `cargo clippy -p rcfd-runner --all-targets -- -D warnings` clean | yes |

**What E1 does and does not show.** E1 passed in run A too: at the load of this session the process-level reader did
not fall into the window in 20 cycles, so E1 is not shown to be sensitive to the defect. The reproduction that carries
the diagnosis is D1. E2's 20 of 20 has the registered 0.012 chance of a false pass at the lead's rate, and that rate was
measured under a load that this session cannot show it had; taken alone E2 is weak evidence too. D1 to D4 are exact.

**Mutations** (scratch copy, `cargo test -p rcfd-runner --release --lib f262`): M1 (status before the event) failed D1,
D2, D3, D4; M2 (request files removed only after the status) failed D1, D2; M3 (no reading of `cancel_requested`) failed
D5; M4 (another reason in the terminal event) failed B1. Every other test passed under each. M1 is committed as
`crates/rcfd-runner/src/runner/tests/f262.mutation.patch`.

**The change.** `crates/rcfd-runner/src/runner.rs`: new `set_terminal` (state event, request files, status, request files
again) and `cancel_was_requested`; `end_pending` and `finalize` call `set_terminal` where they called `set_state` and
`clear_requests`; `finalize` takes a run as cancelled also from a `cancel_requested` event when the finish is
`interrupted`. `store.rs`: two comments. No shared solver file, no golden, no hash table. `runner_w47b.rs` is not edited.

**Deviations.** None from the registered checks. Not run: the whole integration suite of the crate and the workspace
(rule 7), `sites_adr0739` (remote runs are not touched), the Unix paths.

Size (agent), actual: partial; 0; 0; 0; 212000

**Decision by the registered rule.** The hypothesis is confirmed (D1 failed in run A as registered). The remedy is done:
every row is met in run B. F-262 stays partial (S5 is an ignored known miss; the gap is untouched).

### Open questions for the maintainer

None. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the second removal of the
request files after the status, and the reading of `cancel_requested` for a run whose runner died inside the final writes.

### For the lead: roadmap entries

> Status of F-262: partial (unchanged).

> Note entry for F-262: ADR 0742, 2026-11-19, done: the intermittent failure of S7 on the held run was the order of the final writes (terminal status.json before the last state event and before the request files were removed), shown without load by a store that looks at the folder at each write; the terminal status is now the last write of a run, the records of a run that is not cancelled are byte for byte the same, and S7 passed 20 of 20 afterwards.

> Gap of F-262: Socket control (files only) and runs started from the application are not done. Registered check S5 is an ignored known miss. Non-terminal state changes (paused, resumed) still write status.json before their state event, and a runner killed inside the final writes leaves a second terminal state event after adoption.

No follow-up item is proposed.
