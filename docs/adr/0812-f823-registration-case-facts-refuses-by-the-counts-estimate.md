# ADR 0812: F-823 registration, `case.facts` refuses an over-limit generator mesh by the counts' estimate before it builds it

REGISTERED 2026-11-26, written before any code of F-823 under this registration and before any run of its checks.

Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead. It answers the brief
[briefs/f823-case-facts-estimate.md](briefs/f823-case-facts-estimate.md) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. A further registration of F-823 by rule 13: it adds
no acceptance clause of its own; it remedies what [ADR 0809](0809-f823-registration-error-walk-time-under-webgpu.md)
found (its question 1, answered (a) by the lead in the brief). Branch `f823-case-facts-estimate`, at the brief's commit
3b6cc7cc (equal to `origin/main`). Scratch folder `E:/projects/cfd/w49i-work/f823-case-facts-estimate-scratch`.

## The item

F-823's acceptance (read from `docs/roadmap.json` by a script): "The Studio's unit and Playwright tests run as a job in
.github/workflows/ci.yml (ADR 0800 Q3, maintainer 2026-11-16), with npm ci from the committed lock file and Electron run
headless; the job fails the workflow on any test failure; docs/test-process.md names the job. A weekly, grouped
Dependabot npm entry for /apps/studio-web lands with the job (ADR 0801 Q2). The job builds rcfd-cli, rcfd-runner and
rcfd-mcp first so the service e2e tests and the F-824 inventory check do not skip."

This registration serves the clause "the job fails the workflow on any test failure" for one test whose time depends on
the engine: `apps/studio-web/tests/e2e/error-walk.spec.ts:177` ("1004 LIMIT: a mesh over the preview limit"), which waits
up to 60 s for the preview's refusal while `case.facts` builds a mesh of 2,250,000 cells on the same connection
(ADR 0809, amendment 1). The test and its 60 s are not changed.

## What was read

AGENTS.md, the rules (version 5) and the brief, in full. `crates/rcfd-api/src/facts.rs` in full (`case_facts` lines 367
to 429: `semantic_errors`, then `sim::build_mesh` at line 384, then the `max_cells` check at lines 386 to 388, then the
facts; `FactsError::TooLarge { cells, limit }` at lines 136 to 142). `crates/rcfd-api/src/mesh_preview.rs` in full
(`estimate` lines 101 to 127, `ESTIMATE_LIMIT` line 26, its use in `preview` lines 216 to 229). Of
`crates/rcfd-api/src/sim.rs`: `build_mesh` and `build_untransformed_mesh` (lines 688 to 900, every generator `estimate`
names), `cylindrical_interface_mesh` and `cylindrical_interface_blocks` (lines 494 to 593). `mesh_transform::apply`
(`crates/rcfd-api/src/mesh_transform.rs` lines 133 to 142: a rotation about z, the cells unchanged). The callers of
`case_facts`: the service's `case.facts` (`crates/rcfd-api/src/service/case_tools.rs` lines 35 to 62, `facts_error`: 1004
`"mesh has {cells} cells, limit is {limit}"`, with `Some(max_cells)`, default 2,000,000 at `service.rs` line 156);
`rcfd-workspace`'s `case_facts` and `start_run` (`crates/rcfd-workspace/src/facts.rs` line 75, `runs.rs` line 389) and
`rcfd check --facts` (`crates/rcfd-cli/src/cmd_w19c.rs` line 381), all three with `None`. The tests
`crates/rcfd-api/tests/mesh_preview_adr0732.rs` (its M1 checks `estimate` against the run's mesh for the shipped cases)
and `crates/rcfd-api/tests/case_facts_adr0737.rs` (helpers; C3's settings of each generator, lines 220 to 312). ADR 0809
in full (amendment 1, addendum 1, its questions). Of the Studio: `src/renderer/case/estimate.tsx` lines 1 to 53 (the
estimate line asks `case.facts` after every change) and `tests/e2e/error-walk.spec.ts` lines 177 to 190. The tolerance
table ([tolerances.md](../process/tolerances.md)) and guardrail G9 of
[studio-guardrails.md](../studio-guardrails.md). About 120,000 bytes. Before this file, ADR 0737's existing test
`c3_refinement_of_each_generator_is_the_refined_mesh` was run once with `--nocapture` to see which of its settings build
(it skips those that do not): all 15 generators built (`cavity_2d` 32 cells to
`annulus_3d_sector_cylindrical_interface` 128), so `GENS` and `OTHERS` below take its settings.

## Design

### Where, and what it gives

In `case_facts`, after `semantic_errors` and before `sim::build_mesh`, and only when the caller gives a limit:

    if let (Some(limit), Some(n)) = (max_cells, crate::mesh_preview::estimate(case)) {
        if n > limit {
            return Err(FactsError::TooLarge { cells: n, limit });
        }
    }

The estimate is `mesh_preview::estimate`, the one `mesh.preview` uses (the brief: "the same one"), unchanged. The refusal
is the existing `FactsError::TooLarge` with the estimate as `cells`. So the reply is the existing one: code 1004
(`code::LIMIT`) and the message `mesh has N cells, limit is L`, with no `data`. No code is added, so nothing is mapped in
`rcfd-workspace` (whose callers pass `None` and never reach it) and `case_tools.rs` and `cmd_w19c.rs` are not touched.

### Why the same reply is right: the estimate is the built count (from the code, first principles)

For each generator `estimate` names, `build_untransformed_mesh` builds structured hexahedral blocks whose cells are the
product of the same counts, with the same defaults:

- `cavity_2d`, `box_2d`, `channel_2d`, `wedge_2d`, `nozzle_2d`: one block of `nx × ny × 1` (`cavity_2d_rect`,
  `box_2d_graded`, `channel([nx, ny, 1])`, `box_2d` then `warp`, which moves points only).
- `channel_2d_interface`: two `channel` blocks `nx × ny` and `nx_right × ny_right` (0 meaning the left count), joined by
  `join_non_conformal`, which joins faces and keeps every cell of both.
- `annulus_3d`, `annulus_3d_sector`: one block `nx × ny × nz`, `nz` at least 1 (`get_i(..).max(1)` in both).
- `annulus_3d_interface`: `nx × ny × nz` and `nx × ny_right × nz_right` (the right block takes the left `nx`), joined.
- `annulus_3d_stage`, `annulus_3d_cylindrical_interface`: `nx × ny × nz` and `nx_right × ny_right × nz_right`, the
  stage's blocks left unconnected, the cylindrical pair joined by `join_non_conformal_cylindrical` (cells kept); the
  `rotor` zone and the non-orthogonality check change no count.

`mesh_transform::apply` then rotates the points or leaves the mesh as it is. Grading changes spacing, not counts. So
where `estimate` gives `Some(n)` and the case builds, the built mesh has `n` cells: the refusal's `cells` and message are
the ones the build would have given, byte for byte, and only the time changes. E1 checks this by two routes; C5 checks the
whole reply before and after.

The one class of reply that changes: a case whose estimate is over the limit and whose build would fail for another reason
(for example `r_interface` outside `r_in..r_out` with over-limit counts) was answered 1003 with the build's diagnostic and
is now answered 1004, as `mesh.preview` already answers it. That is within the lead's decision ("the reply changes ...
only where the case is over the limit"). Not checked by a row: no shipped or registered case is in it.

### What it does not cover (unchanged)

A mesh file (`estimate` gives `None`), and every generator `estimate` does not name (`axial_fan_sector`, `pipe_sweep`,
`box_3d`, `cylinder_ogrid`, `annulus_3d_sector_cylindrical_interface`, and the rest): built, then refused over
`max_cells` as before (E6). Callers without a limit (`rcfd check --facts`, the workspace's `case_facts` and `start_run`)
build as before (E5). `mesh.refine`, `run.start` and `mesh.preview` are not touched.

The alternative of an added code or key (`data: {estimate, limit}`) was weighed and not taken: the estimate is exact for
every generator it covers, so the existing reply is true; an added code would change the Studio's recovery line and
needs the service and CLI mappings, outside the files the brief gives. Taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation).

## Arithmetic done beforehand

`ny` 1500 with `nx` 1500: 2,250,000 > 2,000,000, refused. The intermediate states of the e2e test: 1500 × 32 = 48,000 and
32 × 1500 = 48,000, built as before. The refusal does `case.get` of nine keys after `semantic_errors`: estimated under
5 ms. The build of the 2,250,000-cell cavity on this host under load: estimated 5 s (ADR 0809 saw the whole `case.facts`
at 26 s to 60 s in the container under load), peak memory about 1.5 GB (4.5 million points, about 9 million faces with
their owners, neighbours, point lists, centres and areas, at about 150 bytes a face). A test process that creates a
service, a session and asks for the refusal: estimated 20 MB peak working set.

## Tests (fixed now)

New file `crates/rcfd-api/tests/case_facts_estimate_adr0812.rs`. Services as `case_facts_adr0737.rs` makes them
(`Service::new`, `enable_ui`, `handle_request`). The cavity `BIG`: ADR 0737's `PISO` cavity text with `nx = 1500`,
`ny = 1500` and `algorithm = "simple"` (a steady case: `case_facts` builds no `Simulation` for it). The generator set
`GENS`: the eleven generators `estimate` names, each `Case::new()` with `mesh.nx = 8`, `mesh.ny = 4` and the extra settings
of ADR 0737's C3 for that generator (`case_facts_adr0737.rs` lines 222 to 268), and in addition right counts that differ
from the left: `channel_2d_interface` `nx_right 5, ny_right 3` (C3's), `annulus_3d_interface` `ny_right 3, nz_right 3`,
`annulus_3d_stage` `nx_right 6, ny_right 3, nz_right 3`, `annulus_3d_cylindrical_interface` `ny_right 3`. The set
`OTHERS`: the generators of C3 that `estimate` does not name (`cylinder_ogrid`, `pipe_sweep`, `box_3d`,
`annulus_3d_sector_cylindrical_interface`) with C3's settings.

- `e1_the_estimate_is_the_built_count`: for every shipped case (`cases/**/*.toml`, 11 files) whose `estimate` is `Some`,
  and every case of `GENS`: `estimate(case) == sim::build_mesh(case).n_cells()`. Every case of `GENS` builds.
- `e2_over_the_limit_is_refused_before_a_build`: a default service (`max_cells` 2,000,000) with the UI methods, a session
  of `BIG`: `case.facts` timed (T_r, wall clock around the call); the reply's error is code 1004 with the message equal
  to `format!("mesh has {} cells, limit is 2000000", n_built)`, where `n_built` is `sim::build_mesh(BIG).n_cells()`,
  built after the call and timed (T_b). Asserts T_r <= 1 s and T_r / T_b <= 0.5; prints both.
- `e3_the_refusal_alone`: the call of e2 without the reference build: code 1004, message
  `mesh has 2250000 cells, limit is 2000000`. For the memory row.
- `e4_at_and_over_a_small_limit`: a service with `max_cells = 100`: `cavity_2d` 10 × 10 (100 cells, at the limit) is
  answered with `cells: 100`; `cavity_2d` 101 × 1 is refused 1004 `mesh has 101 cells, limit is 100`; each case of
  `GENS` under a service with `max_cells = 20` is refused 1004 with the message of its built count.
- `e5_without_a_limit_the_mesh_is_built`: `facts::case_facts(BIG, InText::NotDefault, None)` is `Ok` with
  `cells == 2_250_000`.
- `e6_a_mesh_file_and_other_generators_are_built_then_refused`: `case_facts(.., Some(10))` of a mesh-file case
  (`crates/rcfd-io/tests/data/hex_v2.2_ascii.msh` by absolute path, with `mesh.nx = mesh.ny = 1000000` set and ignored)
  gives `TooLarge { cells, limit: 10 }` with `cells` its built count; each case of `OTHERS` with `Some(10)` gives
  `TooLarge` with its built count.

The test file's header names ADR 0812 and F-823 (rule 12).

## Table of checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| E1 | the brief: "a two-route check that the estimate equals the built count" | `e1_` | equal integers for all 11 + the shipped cases with an estimate; all of `GENS` build | the brief; tolerance table row "same discrete problem by two routes" (exact where nothing separates the two) | equal | exact | code verification |
| E2 | the brief: "the over-limit case is refused without a build (time ...)" | `e2_`: T_r and T_r / T_b | T_r <= 1 s and T_r / T_b <= 0.5; the message equal to the built count's | tolerance table row "run time or memory, as a budget" (a stated budget with the estimate; a ratio <= 0.5 where a speed-up is the claim) | T_r 5 ms; T_b 5 s; ratio 0.001 | 200; 500 | unchanged default output (the row's kind) |
| E3 | the brief: "... and peak memory measured" | peak working set of the test process running `e3_` alone (P_r) and `e5_` alone (P_b), by the sampler below | P_r <= 200 MB and P_r / P_b <= 0.5 | the same row | P_r 20 MB; P_b 1.5 GB | 10; 37 | unchanged default output (the row's kind) |
| E4 | the reply at and over the limit as before | `e4_` | as stated in the test | the brief ("every result of a case under the limit unchanged"); rule 3 | passes | exact | unchanged default output |
| E5 | a caller without a limit builds as before | `e5_` | `Ok`, 2,250,000 cells | rule 3 | passes | exact | unchanged default output |
| E6 | mesh files and other generators unchanged | `e6_` | as stated in the test | the brief ("a mesh file: unchanged") | passes | exact | unchanged default output |
| C5 | every reply under the limit unchanged, bit for bit | the dump below on the base and on the code commit | the two dumps equal byte for byte | tolerance table row "bit-identity of default output"; rule 3 | equal | exact | unchanged default output |
| BH | rule 3 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes | rule 3 (facts.rs is not a solver file; run as the goal asks) | passes | exact | unchanged default output |
| EW | the brief: "error-walk.spec.ts:177 within its unchanged 60 s on Windows" | `npm test` on Windows (R7w): the test's line in the list output | passed, its waits unchanged; its time recorded | ADR 0809 E2; the brief | about 5 s | 12 | code verification |
| EL | the same, "in the Ubuntu 24.04 container" | `npm test` in the container (R7l) | as EW | ADR 0809 E1; the brief | about 10 s | 6 | code verification |
| R7w | rule 7 | the worktree's `apps/studio-web`, `CARGO_TARGET_DIR` this work's: `npm run typecheck`, then `npm test` | both exit 0; 0 failed, 0 skipped, 0 did not run | rule 7; ADR 0809 R7w | passes | exact | code verification |
| R7l | rule 7 | in the container on a `git archive` copy of the code commit: `npm run typecheck`, then `RCFD_TEST_ELECTRON_ARGS="--ignore-gpu-blocklist --enable-unsafe-webgpu" xvfb-run --auto-servernum npm test` | as R7w | rule 7; ADR 0809 R7l | passes | exact | code verification |
| R7c | rule 7 | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-api --release --lib`; `--test case_facts_adr0737`, `--test mesh_preview_adr0732`, `--test case_facts_estimate_adr0812` | each exit 0 | rule 7 | passes | exact | code verification |
| DS | rule 7 (docs) | `cargo test -p rcfd-cli --release --test docs_site` | passes | rule 7 | passes | exact | code verification |
| P1 | rules 7 and 9 | `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | the proposal valid; the report clean but for a generated file only not current | rules 7 and 9 | passes | exact | code verification |
| A1 | only the owned files change | `git diff --stat 3b6cc7cc..HEAD` | `crates/rcfd-api/src/facts.rs`, the new test file and its patch, this ADR, `docs/roadmap/proposals/0812-F-823.json` | the brief, files owned | as the limit | exact | code verification |

### Set-up of the runs (fixed here)

- All cargo commands with `CARGO_TARGET_DIR=E:/projects/cfd/w49i-work/f823-case-facts-estimate-scratch/target` and
  `CARGO_BUILD_JOBS=8`, release profile. The host CPU load (the mean `LoadPercentage` of `Win32_Processor`) is recorded
  before each timed run and, for the long ones, every 30 s.
- **E3's sampler** (scratch, PowerShell): starts the test executable of the code commit with
  `--exact <test> --test-threads=1`, and until it exits reads `PeakWorkingSet64` of the process every 5 ms; the value
  recorded is the largest read (the counter only grows; what the last 5 ms add is not seen). Run once for `e3_` and once
  for `e5_`, after the file's run R7c.
- **C5's dump** (scratch, not committed): a test file copied into a `git archive` copy of the registration commit under
  the scratch folder, which writes one line per reply (`{"what": .., "reply": ..}`, the reply of `handle_request` as
  JSON) of `case.facts` for: every shipped case by its path in that copy, under a default service; every case of `GENS`
  and `OTHERS` and `BIG` under a default service; every case of `GENS` and `OTHERS` under a service with
  `max_cells = 20`; the cavity 10 × 10 and 101 × 1 under `max_cells = 100`. Run once on the copy as archived (the base),
  then once with `crates/rcfd-api/src/facts.rs` of the code commit put into the same copy (the code). The two files are
  compared with `cmp`. Over-limit replies are in it too: by the derivation above they are equal as well.
- **EW and EL**: R7w is the Windows run, in the worktree after `npm ci` and with this work's target folder holding
  `rcfd`, `rcfd-runner`, `rcfd-mcp` and the example `f447_bench` of the code commit (`cargo build --release -p rcfd-cli
  -p rcfd-runner -p rcfd-mcp` and `-p rcfd-api --example f447_bench`). R7l is the container run, ADR 0809's set-up (the
  image `rcfd-studio-linux-test:ubuntu24.04-f823b`, the inside script of its R7l) with this work's own volumes
  `rcfd-f823e-target` and `rcfd-f823e-cache` and `CARGO_BUILD_JOBS=8`, on a `git archive` copy of the code commit. The
  two are made one after the other, never at the same time, and the container's `/proc/loadavg` and the host's load are
  recorded with each.

Each registered run is made once. A miss is recorded as a miss with its number; nothing is run again to pass. A test
that fails on a file refused by the operating system under load is reported, not retried.

## Negative controls and planned mutations

On a scratch copy (C5's copy, with the code commit's `facts.rs` and test file), each once, by `cargo test --release -p
rcfd-api --test case_facts_estimate_adr0812`:

| Id | Mutation | Caught when |
|---|---|---|
| MU1 | the estimate's refusal removed (the code before this item: the negative control of E2) | `e2_` fails on T_r / T_b |
| MU2 | committed: `n > limit` made `n >= limit` | `e4_` fails (the 100-cell cavity refused at a limit of 100) |
| MU3 | the refusal gives `cells: limit` | `e2_`, `e3_` and `e4_` fail on the message |
| MU4 | the estimate applied without a limit too (`max_cells.or(Some(mesh_preview::ESTIMATE_LIMIT))` in the new block) | `e5_` fails |
| MU5 | `estimate` drops the right block of `channel_2d_interface` (`mesh_preview.rs` line 121 to `m(nx, ny, 1)`) | `e1_` fails |

MU2 is committed as `crates/rcfd-api/tests/case_facts_estimate_adr0812.mutation.patch` (rule 16), its first line
`# expect-fail: cargo test --release -p rcfd-api --test case_facts_estimate_adr0812 e4_`. No Studio file changes, so no
e2e patch.

## Size

Size (agent): done; 0; 2; 0; 140000

(Heavy runs: R7w and R7l, the Studio's whole suite on Windows and in the container.)

## Decision rule

**Done** for this registration if every row (E1 to E6, C5, BH, EW, EL, R7w, R7l, R7c, DS, P1, A1) holds and each of MU1
to MU5 is caught; **partial** otherwise, with the miss recorded. F-823 itself stays as the lead reads it: ADR 0809's
question 2 (the first GitHub Actions run of the Linux job) is the lead's, and not decided here.

## What is not covered

The generators `estimate` does not name and mesh files: `case.facts` still builds them before its `max_cells` check
(a question of scope for the addendum). The time limits of the Linux job under load (ADR 0809 question 2, the lead's). The GitHub runners. The
egui Studio. The over-limit case whose build would fail for another reason (above; no row).

## Open questions for the maintainer

None at registration. Any question the runs raise is listed in the addendum.

## Amendment 1 (2026-11-26): E6's set-up is defective; a corrected test `e6b_`

Written and committed after the registered run of the test file (R7c's `--test case_facts_estimate_adr0812`, at
1c455481, host CPU 56 % before) and before the corrected run. Nothing above is changed.

In that run `e6_` failed in its set-up, before any call of `case_facts`: `Case::set("mesh.nx", 1000000)` returned
`InvalidValue { path: "mesh.nx", reason: "1000000 outside [Some(1), Some(100000)]" }` (the test file, line 295). The
schema bounds the counts at 100,000, which the registration did not check. The row E6 is therefore not decided by that
run; it is recorded as such in the addendum, not as a met or missed limit of the code.

Correction: a new test `e6b_a_mesh_file_and_other_generators_are_built_then_refused`, the same as `e6_` but with
`mesh.nx = mesh.ny = 100000` (the schema's maximum; as generator counts their estimate would be 10,000,000,000, far over
the limit of 10, so the file case still shows the counts are not used). Its limit is E6's, unchanged. `e6_` stays in the
file as an ignored record (`#[ignore]` with the reason), not edited otherwise. The corrected run is `e6b_` alone, once:
`cargo test --release -p rcfd-api --test case_facts_estimate_adr0812 e6b_`. The other rows of that run stand as they
came out (in the addendum). The mutations' runs on the scratch copy run the whole file, `e6b_` included.

## Amendment 2 (2026-11-26): E3's sampler cannot read a process of a few milliseconds; read once after exit

Written and committed after E3's registered runs and before the corrected ones. Nothing above is changed.

The sampler of the registration (every 5 ms while the process runs) gave for `e3_` "peak working set 0 bytes over 1
reads" (host CPU 34 % before): the test ended (in 0.00 s) before a read succeeded, so P_r was not measured. For `e5_` it
gave 6,248,591,360 bytes (5,959.1 MB) over 1,062 reads (host CPU 25 % before; the test 28.43 s). The row E3 is not
decided by those runs.

Correction of the set-up: the sampler keeps the process handle open, and after the process has exited reads
`PeakWorkingSetSize` once from `GetProcessMemoryInfo` (psapi, through PowerShell's `Add-Type`) on that handle: the
peak of the whole life of the process, not a sample. Scratch tool `sample-peak2.ps1`. Run once for `e3_` and once for
`e5_`; E3's limits (P_r <= 200 MB, P_r / P_b <= 0.5) are unchanged and are read from these two runs.

## Addendum 1: results (2026-11-26)

The code is commit 6b91b216 (`facts.rs`, 9 added lines: the doc comment at lines 370 to 373 and the check at lines 388
to 392) with the tests of 6b91b216 and a4811e11 (amendment 1) and the patch of 1c455481. Each registered run was made
once, in the order below; the host CPU load (mean `LoadPercentage`) is given with each. Logs in the scratch folder
(`logs/`). Others loaded the machine through every run.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| E1 | equal for every case; all of `GENS` build | 20 cases (9 shipped cases with an estimate, the 11 of `GENS`), all equal (for example `channel_2d_interface` 47 and 47, `annulus_3d_stage` 118 and 118); all built (CPU 56 % before the file's run) | met |
| E2 | T_r <= 1 s; T_r / T_b <= 0.5; the built count's message | T_r 0.000583 s; T_b 28.712 s; ratio 0.0000203; reply `{"code":1004,"message":"mesh has 2250000 cells, limit is 2000000"}`, built 2,250,000 (CPU 56 %) | met |
| E3 | P_r <= 200 MB; P_r / P_b <= 0.5 | first sampler (registration): P_r not measured (0 bytes over 1 read), P_b 5,959.1 MB: not deciding (amendment 2). Corrected (amendment 2): P_r 7,725,056 bytes (7.4 MB, CPU 97 % before), P_b 6,247,583,744 bytes (5,958.2 MB, CPU 76 %), ratio 0.0012 | met (amendment 2's run) |
| E4 | as stated | passed | met |
| E5 | `Ok`, 2,250,000 cells | passed | met |
| E6 | as stated | `e6_`: failed in its set-up (`mesh.nx` 1000000 outside the schema's range), not deciding (amendment 1). `e6b_` (CPU 41 %): passed; the file case built 64 cells, `cylinder_ogrid` 32, `pipe_sweep` 384, `box_3d` 64, `annulus_3d_sector_cylindrical_interface` 128, each refused at 10 with its built count | met (amendment 1's run) |
| C5 | the two dumps equal byte for byte | 44 replies, 23,967 bytes each, `cmp` equal, sha256 `fe341dc3...bfd162` for both (CPU 49 %); the base took 30.78 s, the code 0.34 s | met |
| BH | passes | 3 passed, 1 ignored (as on main) | met |
| R7c | each exit 0 | `cargo fmt -p rcfd-api --check` exit 0; `cargo clippy -p rcfd-api --all-targets -- -D warnings` exit 0; `--lib` 160 passed, 4 ignored (as on main); `case_facts_adr0737` 8 passed; `mesh_preview_adr0732` 5 passed; `case_facts_estimate_adr0812`: E1 to E6 above (5 passed, `e6_` failed in its set-up in the first run; `e6b_` passed) | met but for `e6_`'s set-up (amendment 1) |
| EW | passed, waits unchanged | `error-walk.spec.ts:177` passed in 2.7 s (ADR 0809's R7w: 36.8 s) | met |
| R7w | typecheck and `npm test` exit 0; 0 failed, 0 skipped, 0 did not run | typecheck exit 0; `npm test` exit 1: 880 passed, 1 failed, 4 skipped, 5 did not run (12.8 m; host CPU 23 % to 93 %, median 64 %, 25 samples). Failed: `run-control.spec.ts:268`, `project.cancel_run` answered the renderer in 23.6 ms, limit P2 20 ms; the 5 later tests of that file did not run. Skipped: `canvas-f979.spec.ts` E0 to E3, ADR 0807 amendment 1's skipped record on main (`test.skip(true, ...)`) | **missed** |
| EL | as EW | `error-walk.spec.ts:177` passed in 2.1 s (ADR 0809's R7l: 26.0 s) | met |
| R7l | as R7w | typecheck exit 0; `npm test` exit 0: 886 passed, 0 failed, 4 skipped, 0 did not run (13.0 m; container loadavg 5.78 before, 4.22 after; host CPU 32 % to 99 %, median 54 %, 29 samples). Skipped: the same 4 of ADR 0807's record | **missed** (the 4 skips only) |
| DS | passes | passed on this addendum before its commit (in the report) | met |
| P1 | the proposal valid; the report clean but for a generated file only not current | in the report | see the report |
| A1 | only the owned files | `git diff --stat 3b6cc7cc..HEAD`: `crates/rcfd-api/src/facts.rs`, `crates/rcfd-api/tests/case_facts_estimate_adr0812.rs` and its `.mutation.patch`, this ADR, `docs/roadmap/proposals/0812-F-823.json` (in the report) | met |

### Mutations

On the scratch copy (the code commit's `facts.rs`, `mesh_preview.rs` and test file), each once, the whole file:

| Id | Caught by |
|---|---|
| MU1 (the refusal removed) | `e2_` (T_r 36.77 s; the reply's message unchanged, as derived); `e3_` passed, as expected without the check (CPU 70 %) |
| MU2 (committed, `>=`) | `e4_` (the 10 × 10 cavity refused: "mesh has 100 cells, limit is 100") (CPU 43 %) |
| MU3 (`cells: limit`) | `e2_`, `e3_`, `e4_` (the message gives 2000000 or 100) (CPU 48 %) |
| MU4 (the estimate without a limit) | `e5_` (`TooLarge { cells: 2250000, limit: 2000000 }`) (CPU 40 %) |
| MU5 (`channel_2d_interface` without its right block) | `e1_` and `e4_` (channel_2d_interface) (CPU 79 %) |

All five caught.

### Deviations

Two set-up defects, each corrected by an amendment committed before its corrected run: E6's counts over the schema's
range (amendment 1; `e6_` kept as an ignored record) and E3's sampler, which could not read a process of a few
milliseconds (amendment 2). R7w's limit "0 skipped" was registered without counting the skipped record ADR 0807 left on
main (4 tests); it is recorded as written, a miss. P_b came out at about 6 GB, four times the estimate of 1.5 GB (the
estimate left out the build's intermediate copies); the row holds by far either way.

### What was not run

The GitHub runners (the lead reads them, ADR 0809 question 2). Nothing registered was left out.

### Size

Size (agent, actual): partial; 0; 2; 1; 160000

### Decision by the registered rule

**Partial.** E1 to E6, C5, BH, R7c, EW, EL, DS and A1 hold (E3 and E6 on their amendments' runs) and MU1 to MU5 are caught; R7w and R7l miss: on Windows one time limit of the run-control spec under load (P2, 23.6 ms against 20 ms) with the 5 tests after it not run, and on both platforms the 4 tests ADR 0807 keeps as a skipped record, which the registered "0 skipped" did not allow for. The remedy itself holds: the over-limit cavity is refused without a build, with the reply the build gave, and `error-walk.spec.ts:177` passes in 2.7 s and 2.1 s.

## Open questions for the maintainer

- **Q1 (scope).** `case.facts` still builds a mesh before its `max_cells` check where the counts give no estimate: a
  mesh file and the generators `estimate` does not name (`axial_fan_sector`, `pipe_sweep`, `box_3d`, `cylinder_ogrid`,
  `annulus_3d_sector_cylindrical_interface` and others). An over-limit case of those holds the Studio's connection as the
  cavity did. Options: (a) leave it, since none of the Studio's tests or shipped cases meets it; (b) a new item that gives
  `estimate` the counts of the remaining generators, each checked by E1's two routes; (c) the service answers `case.facts`
  apart from the connection's order. Recommendation: (b), as an engine item with its own registration, since it changes
  only the time of over-limit replies, as here.
- **Q2.** R7w's miss (`run-control.spec.ts:268`, 23.6 ms against P2's 20 ms on a loaded host) is the limit ADR 0809's L
  missed under load. Options: (a) the lead reads it with the first GitHub Actions run (ADR 0809 question 2, answered (a));
  (b) a registration that measures it. Recommendation: (a).

## For the lead: roadmap entries

> Status: partial

> Note: ADR 0812, 2026-11-26, partial: case.facts refuses a generator mesh over its cell limit by the counts' estimate before it builds it, with the reply the build gave (1004 with the built count, equal by derivation and by two routes on 20 cases; 44 replies byte-identical before and after); the 2.25-million-cell cavity is refused in 0.6 ms instead of 28.7 s, and error-walk.spec.ts:177 passed in 2.7 s on Windows and 2.1 s in the Ubuntu 24.04 container; npm test on Windows missed one P2 time limit under load, and both runs skip ADR 0807's 4 recorded tests against a registered 0.

> Gap: case.facts refuses an over-limit generator mesh by its counts (ADR 0812); error-walk.spec.ts:177 passes in 2 to 3 s. npm test here: Windows 880 passed, 1 failed (run-control.spec.ts:268, 23.6 ms against P2's 20 ms on a loaded host), 5 did not run; the container 886 passed; both skip ADR 0807's 4 recorded tests. Mesh files and generators without an estimate still build first. The first run on GitHub Actions is to be read.

No follow-up item is proposed here; question 1 names the one this work would recommend.

## Amendment 3 (2026-11-26): the brief by its path

Written after addendum 1, before the report; nothing above is changed and no check or run is affected.
`python scripts/process_report.py --check` gave "registration cites no brief file" for this ADR: its opening names the
brief by a relative link only. The brief this registration answers is `docs/adr/briefs/f823-case-facts-estimate.md`.

## Answers of the maintainer (2026-11-26), written by the lead

1. Option (b): a new engine item, F-989, gives `estimate` the counts of mesh files and of the generators without one, checked by two routes; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
2. Option (a): the lead reads R7w's 20 ms miss of `run-control.spec.ts:268` with the first GitHub Actions run; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
