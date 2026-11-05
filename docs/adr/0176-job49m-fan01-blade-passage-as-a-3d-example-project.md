# ADR 0176: Job 49M, the FAN-01 blade passage as a 3D example project (`axial-fan`) for the Studio

Status: **registered 2026-10-30**, before any registered run (the inputs, the script entry and the changed tests were
written and the tests compiled first; the runs that will be stored have not run). Amendments follow this text, each
written before the runs it covers; results go in an addendum. Not yet accepted by the maintainer.

Before this job, every case of the shipped example projects was one cell thick, so a user who opened the examples never
saw the Studio's 3D view. This job adds a third example project, `axial-fan`, in both trees (`example-projects` and
`example-projects-linux`): one periodic FAN-01 blade passage, with one case, `cases/fan01-passage-upwind.toml`, and one
stored run per tree. The case is B3 of ADR 0110 with first-order upwind, because the second-order run does not end (limit
cycle) and a stored example must finish in bounded time with a steady field to look at. It is for viewing the 3D geometry
and flow pattern only, and no agreement with measured data is claimed: the notes give the run's torque as 1.40 N m, 35 %
below the measured 2.142 N m. The addendum of 2026-10-30 reports one miss, the 5 % part of P5: torque 1.4023 N m on both
trees, +5.04 % against the 1.335 N m of ADR 0110 S10, kept as an ignored known miss. Every other registered check passed
or came out as required, including P1 (`finished`, solver state `not_converged` at iteration 1500, exit code 5). The full
suites of any crate, the other `rcfd-studio` tests on either platform, CI and a person at the Studio window were not run.
The addendum of 2026-11-04 takes recommendation (a) on questions 1 and 3 under the 2026-11-04 loop instruction and leaves
question 2, the 6 MB per-tree cap of `axial-fan`, open for the maintainer.

Rules and parents:

* Job 49M, on main edb4839 (worktree branch `job49m-fan01-studio-example`). Number: 0172 is the highest on main; 0173 and
  0174 are job 49K's branch, 0175 is job 49L's (running in parallel); 0176 was assigned to this job.
* House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed here before the registered
  runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment
  before the corrected run; defaults of existing code stay bit-identical (no crate source changes in this job: only an
  example project, the example script's project table, tests and docs); `crates/rcfd-api/tests/baseline_hashes.rs` is not
  re-recorded; no new crate, no unsafe code, the crate graph unchanged; no commit trailers; nothing merged or pushed.
* Parents: [ADR 0142](0142-wave48h-example-projects.md) (the example projects, their checks and caps),
  [ADR 0150](0150-job48p-linux-example-stores.md) (the Linux tree),
  [ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md)
  and [ADR 0157](0157-job48w-merge-48u-rotated-cases-with-the-linux-tree.md) (`--add-missing` per tree, DV5/DV6),
  [ADR 0169](0169-job49f-sst-and-k-epsilon-far-field-default-and-the-naca-example-runs.md) (regenerated example runs),
  [ADR 0110](0110-wave43c-fan01-rotor-passage-non-convergence-diagnosis.md) (FAN-01 B3 and its limit cycle),
  [ADR 0086](0086-wave37c-fan01-solution-verification.md) (the C-grid family F1-F3).

## Problem

The user: "everything on the studio is in 2d". The Studio already draws a mesh that is not flat in an orbitable 3D view
(`crates/rcfd-studio/src/preview.rs` and `viewport.rs`, the `two_d` flag from `surface_of`; the caption says "3D" or "2D,
front view"). But every case of the shipped example projects (`example-projects/quick-start`, `wind-tunnel` and their
Linux mirror) is one cell thick, so a user who opens the examples never sees the 3D view. The repository has a 3D case
with published geometry: FAN-01, one periodic blade passage of the FAU / EAA low-pressure axial fan (CC BY 4.0 CAD and
data). Its case text exists only inside a test (`crates/rcfd-api/tests/fan01_stall_w43c.rs`, `case_toml`); the
`cases/axial_fan_*` files are five-bladed laminar smoke cases, not FAN-01.

Constraint: the registered FAN-01 set-up (SST, Barth-Jespersen-limited `linear_upwind`) does not converge on
these meshes (ADR 0110: a limit cycle, residuals stall near 1e-3), and no FAN-01 result agrees with the measured data
(`docs/milestone-fan.md`). An example run must say so and must not be presented as a fan prediction.

## Decisions

1. **D1 A new, third example project `axial-fan`** in both trees, in the folder format of ADR 0142 (`cases/`, `meshes/`
   with a README only, `runs/`, `designs/`, `notes/`, `.rcfd/.gitignore`, `rcfd-project.json` written by the script), with
   one case, `cases/fan01-passage-upwind.toml`, and one stored run per tree. Options: (a) a new project; (b) the case in
   `wind-tunnel` (no room: 173 kB under its cap at ADR 0157; it is now 28.1 MB after ADR 0169, but a rotating 3D passage
   is not a wind-tunnel flow); (c) a case without a stored run (the preview is 3D, but no field result would show in 3D).
   **Pick (a)** with a run.
2. **D2 The case is B3 of ADR 0110 with three changes.** The text of `case_toml(F3, true, "", false)` of
   `fan01_stall_w43c.rs` (emitted by a scratch copy of that test, not committed; the 16 CAD sections inline, every number
   as the test formats it), re-ordered into commented tables, with: `solver.convection = "upwind"` (the
   `linear_upwind_limiter` line removed), `solver.max_iterations = 1500` (B3: 100000), and an `[output]` table
   (`store_interval = 150`, f32, fields `ux`, `uy`, `uz`, `p`, `nut`). Reason for first order: the second-order run does
   not end (limit cycle), and a stored example must finish in bounded time with a steady field to look at. The case's
   head comment and the project notes state that it is for viewing the 3D geometry and flow pattern only, that it uses
   first-order upwind, that the second-order set-up stalls, and the torque figures of ADR 0110 S10 (1.335 N m, 38 % below
   the measured 2.142 N m) and ADR 0086 (F3 1.850 N m); no agreement with measured data is claimed.
3. **D3 Runs made by the real solver through the run service, per tree.** Windows: `python
   scripts/make_example_projects.py --project axial-fan --add-missing --bin-dir E:/projects/cfd/rcfd/target-w49m/release`.
   Linux: `python scripts/make_example_projects.py --docker --project axial-fan --add-missing` (ADR 0150's image and
   container; the inputs are copied from the Windows tree). `--project axial-fan` keeps the ledgers and run tables of
   `wind-tunnel` and `quick-start` untouched; the manifests are rewritten (their run lists unchanged; the `bytes` figures
   refresh).
4. **D4 Caps.** The new project gets its own cap, registered here: `axial-fan` <= 6 000 000 bytes per tree. The caps of
   ADR 0142 D7 are unchanged (wind-tunnel 35 MB, quick-start 6 MB, a whole tree 40 MB). Expected (from the size smoke
   below): about 3.8 MB per tree (store: mesh about 1.03 MB plus 11 snapshots of about 99 kB; run files about 1.2 kB per
   iteration); the trees then about 36.2 MB each. **Stop rule:** if a tree exceeds any cap, nothing of that tree's run is
   committed, the tree is restored from git, and the options are reported.
5. **D5 Tests.** `crates/rcfd-mcp/tests/example_projects_w48h.rs`: the counts of a tree become 3 projects (S1, T1), 13
   case files (S3), 9 generator cases (E1); `CAP_PROJECTS` gains `axial-fan`; G1's expectation for the fan run is P1
   below; a new test G4 holds P2-P5. `crates/rcfd-studio/tests/example_projects_h.rs` H1 opens `axial-fan` too (1 case,
   1 run) and expects `two_d` false for it (true for the others, as before), and a non-empty frame for every run. New
   `crates/rcfd-studio/tests/example_fan_w49m.rs` M1 (the preview). `crates/rcfd-studio/tests/saved_store_r.rs`: DV5
   takes 20 texts (was 19); DV6's list of cases without a recorded plan gains the fan case, by name (as ADR 0157 D4 did);
   DV6's record and recorder are unchanged. `scripts/make_example_projects.py`: the project entry and its cap only.
   Tests that enumerate projects by a fixed list (`case_hashes_w48u`, `w49j_warnings` LW8, `correctors_x` X1,
   `far_field_w49f` F5) do not see the new project and are not changed.

## Placement

New:

* `example-projects/axial-fan/` (the case, `meshes/README.md`, `notes/README.md`; the rest written by the script);
* `example-projects-linux/axial-fan/` (written by the script);
* `crates/rcfd-studio/tests/example_fan_w49m.rs`;
* this ADR.

Changed:

* `scripts/make_example_projects.py`;
* the four test files of D5;
* `crates/rcfd-studio/tests/dv/mod.rs` (a doc comment);
* `example-projects/README.md`, `example-projects-linux/README.md` (by hand: the trees' READMEs are not written by the
  script);
* `README.md`;
* both `manifest.json` (by the script).

## Registered checks (limits fixed here, before any of them ran)

Set-up:

* Windows, this host, release, target folder `E:/projects/cfd/rcfd/target-w49m`, `--test-threads 1 --nocapture`, output
  to files in the session scratchpad.
* Linux: ADR 0150's image `rcfd-linux:ubuntu24.04-rust1.99.0` (`sha256:485f28a2036497ed`...), the script's own container
  for L1; for L4 one container on a `git archive` of the regeneration commit, extracted with `tar -m`, release,
  `--test-threads 1 --nocapture`.

| Id | Check | Limit | Control |
|---|---|---|---|
| M0 | `python scripts/make_example_projects.py --check` on the registration commit, before any run | problems only about `axial-fan` (no run, no manifest entry, no ledger; in the Linux tree its inputs missing); none about the other projects | (the control of the closing checks) |
| W-L1 | D3's Windows command | exit 0; exactly 1 run added, of `cases/fan01-passage-upwind.toml`; its closing check 0 problems in `example-projects`; `git status` changes only `example-projects/axial-fan/**` and `example-projects/manifest.json` (run lists of the other projects unchanged) | |
| L-L1 | D3's Linux command, on a commit holding the Windows run (clean working tree) | container exit 0; 1 run added; the closing `--check` of both trees 0 problems; `git status` changes only `example-projects-linux/axial-fan/**` and `example-projects-linux/manifest.json` | |
| P1 | G1 on the fan run of each tree | `finished`, solver state `not_converged` at iteration 1500, exit code 5. Prediction from ADR 0110 S10: with `upwind`, `is_converged()` stayed false (the SST change criterion) while u and p were below 1e-5 | |
| P2 | G4: the stored mesh | cell centres span more than 0.05 m along each of x, y, z | M1's control |
| P3 | G4: the last residual line | `u` and `p` below 1e-5 (the case's tolerances). Uncertain: S10 reached them 114 iterations after a switch from the stalled second-order state; this run starts cold | |
| P4 | G4: mass balance | \|m_in + m_out\| <= 1e-3 max(\|m_in\|, \|m_out\|) | |
| P5 | G4: rotor torque -9 (m_blade + m_hub) | positive, and within 5 % of S10's 1.335 N m (the same mesh and scheme; a reproduction of the first-order fixed point, not a comparison with measurement) | |
| G2 | the ledger's `wall_s` | the fan run <= 180 s in each tree; each tree's sum <= 20 min (ADR 0142 G2, unchanged) | |
| T5 | sizes | `axial-fan` <= 6 000 000 bytes per tree, each tree <= 40 000 000 (S8 and the script's check); reported: both trees' bytes and the bytes the commits add | D4 stop rule |
| W1 | `cargo test --release --locked -p rcfd-mcp --test example_projects_w48h` on Windows, after both runs are committed | every test passes, the known misses ignored; S5's mesh hash equal for the 13 Windows runs | T3 (both Linux cylinder runs differ) and the existing controls |
| W2 | `cargo test --release --locked -p rcfd-studio --test example_projects_h --test example_fan_w49m --test saved_store_r` on Windows | H1: 3 projects, axial-fan with 1 case and 1 run, its frame `two_d` false and non-empty, every other run `two_d` true; M1: the preview `two_d` false, 4 920 cells, a non-empty surface with a `blade` part, extent > 0.05 m on each axis, mesh hash equal to the run's store; DV5 20 texts; DV6 22 of 22 recorded plans equal, the three left out by name | M1: the quick-start cavity previews with `two_d` true |
| L4 | `example_projects_w48h`, `example_projects_h` and `example_fan_w49m` in one Linux container on an archive of the commit holding both runs | as W1 and W2 for the Linux tree (S5 exact for the 13 Linux runs; H1 and M1 on `example-projects-linux`) | T3 on Linux |
| P6 | the script | `--check` 0 problems on Windows; `python -m unittest scripts/test_make_example_projects.py` passes on Windows (Python 3.9.13) | the self-test's tampered copies |
| Q1 | lint and baselines | `cargo clippy --release --locked -p rcfd-mcp -p rcfd-studio --tests -- -D warnings` and `cargo fmt -p rcfd-mcp -p rcfd-studio -- --check` clean on Windows; `git diff edb4839 -- crates/rcfd-api/tests/baseline_hashes.rs` empty; no file under `crates/*/src` changed | |

A missed P1-P5 or G2 is not tuned: the case stays as registered, the limit becomes an ignored `*_known_miss` test, and
the notes keep reporting what the run reached.

Order: M0; W-L1 (P1-P5, G2, T5 read from its files); the Windows run commit; L-L1; the Linux run commit; W1, W2, P6,
Q1; L4.

## What I already knew (rule d)

* The code, tests and ADRs named above; the sizes of both trees (Windows: wind-tunnel 28 096 175, quick-start 4 250 540
  bytes; Linux: 28 093 048 and 4 248 948); that the Docker image above exists on this host.
* `rcfd check` of the case (exit 0, one warning SV101: maximum non-orthogonality 78.4 deg; mean 18.9 deg).
* Two set-up smokes of the case with `rcfd run` outside the project (scratchpad copies, nothing kept): 30 iterations of
  the registered settings took 1.5 s (res_u 1.1e-2, res_p 7.7e-3 at iteration 30); size smokes of 40 and 80 iterations
  with `store_interval = 10` gave a store of 1 426 594 and 1 823 650 bytes (4 and 8 snapshots: about 99 kB per snapshot,
  about 1.03 MB of mesh and header; 5 888 points), and `monitors.csv`, `monitors.json`, `report.html` of about 225, 270
  and 130 bytes per iteration. Earlier drafts of the case had `max_iterations = 1000` and `store_interval = 100`; they
  were changed to 1500 and 150 after these smokes and before this registration (more room for a cold start within G2's
  180 s; 11 snapshots).
* Compiled, nothing run: the changed test binaries in release on Windows.

## Not run (planned)

* The full suites of any crate (the brief asks for targeted test binaries only; the whole `rcfd-studio` suite takes about
  12 minutes and the gate over an hour).
* The `rcfd-studio` tests other than W2's on Linux.
* CI.
* The "Not in CI" checklist (a person at the window).

A screenshot of the Studio window is attempted only if time allows and is not a registered check.

## Questions for the maintainer (not decided here)

1. Whether a first-order run belongs in the examples at all. Options: (a) as here, labelled for viewing; (b) the case
   without a stored run (the preview is 3D, results only after the user runs it); (c) the second-order set-up with a
   capped iteration count (stored at its stall, `not_converged`). **Recommendation (a)**: (b) shows no 3D field, and
   (c) stores a state of a limit cycle, which looks like a solution and is not one.
2. The new cap (6 MB per tree) is registered here, not by the maintainer; the whole-tree cap of 40 MB is unchanged.

## Addendum: results against the registered limits (2026-10-30)

Commits: registration 7305c5c; the Windows run 2d7e3ea (W-L1); the Linux run 6fa072b (L-L1); P5's known miss 70e6c9e
(test only); then the notes sentence on the run's outcome (both trees, outside the generated run table) and this
addendum. Logs in the session scratchpad (not kept).

| Id | Limit | Result | Verdict |
|---|---|---|---|
| M0 | problems only about `axial-fan` | 11 problems, all `axial-fan` (no `rcfd-project.json`, `runs`, `designs`, `.rcfd`, ledger, manifest entry; Linux tree: project and 3 inputs missing) | as required |
| W-L1 | exit 0; 1 run; check 0; only `axial-fan/**` and `manifest.json` | exit 0; `r-20261030T223519-0489` added; check 0 problems; `git status`: `example-projects/axial-fan/` (new files, run table) and `manifest.json` (the axial-fan entry and cap; wind-tunnel's `bytes` refreshed 28 095 017 -> 28 096 175, its run list unchanged) | pass |
| L-L1 | exit 0; 1 run; closing check 0; only Linux `axial-fan/**` and `manifest.json` | container exit 0 (`rcfd`, `rcfd-runner` built in 48.5 s); 4 inputs copied; `r-20261030T223744-ad62` added; closing `--check` of both trees 0 problems; `git status`: `example-projects-linux/axial-fan/` and its `manifest.json` only | pass |
| P1 | `finished`, `not_converged` at 1500, exit 5 | both trees: finished, solver `not_converged` at 1500 ("iteration limit 1500 reached without meeting the residual targets"), exit 5 | pass |
| P2 | extent > 0.05 m on each axis | cell centres span 0.1825 / 0.2185 / 0.3669 m (both trees) | pass |
| P3 | u, p < 1e-5 at the last line | res_u 4.673e-6, res_p 7.927e-7 (both trees) | pass |
| P4 | mass imbalance <= 1e-3 | m_in -0.1860073, m_out 0.1860073, imbalance 1.98e-10 | pass |
| P5 | torque > 0 and within 5 % of 1.335 N m | 1.4023 N m on both trees: positive, but +5.04 % against S10 | **missed** (the 5 % part): kept as the ignored `g4_fan_torque_within_5_percent_of_s10_known_miss`; the sign part stays in G4 |
| G2 | fan run <= 180 s; tree sum <= 20 min | Windows 65.5 s, Linux 125.6 s (container, 4 cpus); Linux tree sum 361.1 s | pass |
| T5 | axial-fan <= 6 MB; tree <= 40 MB | Windows: axial-fan 3 499 558 bytes, tree 35 850 618; Linux: 3 499 676, tree 35 846 833 (store.rts 2 022 206, residuals.jsonl 484 236 of the run); about 3.5 MB added per tree | pass |
| W1 | `example_projects_w48h` on Windows | 22 passed, 0 failed, 3 ignored (the two earlier known misses and P5's); S5 exact for the 13 Windows runs; T3: the Linux fan store differs from the mesh built on Windows (12850526753399533395 against 13719360332065067188: the fan generator calls libm, as the cylinder does) | pass |
| W2 | H1, M1, DV5, DV6 on Windows | H1: axial-fan 1 case, 1 run, `Ready`, `p` at snapshot 9 (step 1500), 2D false, 1 872 faces; the other runs 2D true. M1: `two_d` false, 4 920 cells, surface 1 872 faces / 3 740 triangles, parts inlet, outlet, seam_lo, seam_hi, hub, shroud, blade; extent 0.188 / 0.257 / 0.4 m; preview mesh hash = store's; control cavity 2D true. `saved_store_r` 6 passed (DV5 20 texts, DV6 as registered) | pass |
| L4 | the same in a Linux container on an archive of 70e6c9e | `example_projects_w48h` 22 passed, 3 ignored (S5 exact for the 13 Linux runs; T3 the Windows fan store differs); H1 and M1 passed on `example-projects-linux` (M1's preview hash = the Linux store's) | pass |
| P6 | `--check` 0; self-test passes | 0 problems; 15 tests OK | pass |
| Q1 | clippy, fmt, baselines, no `src` change | clippy `-D warnings` clean; fmt clean; `git diff edb4839 -- crates/rcfd-api/tests/baseline_hashes.rs 'crates/*/src'` empty | pass |

**Deviations.**

1. L4's first container attempt failed before compiling (Git Bash rewrote `CARGO_TARGET_DIR=/target` to a Windows path).
   It was rerun with `MSYS_NO_PATHCONV=1`, otherwise the same command.
2. After the runs, one sentence on the run's outcome (residuals, `not_converged`, torque 1.40 N m, 35 % below the measured
   2.142 N m) was added to both trees' `notes/README.md`, outside the run table. `--check` and `example_projects_w48h`
   (22 passed) re-ran on Windows after it.

**Not run.** The full suites of any crate; the other `rcfd-studio` tests on either platform; CI; a person at the
Studio window (no screenshot was taken).

## Questions for the maintainer (continued)

3. P5: the cold-start first-order torque (1.4023 N m) is 5.04 % above S10's switched-from-second-order value (1.335 N m).
   Both runs stop with a small cycle left, so the two first-order states need not be the same point. Options: (a) leave
   the known miss; (b) nothing more for an example that claims no accuracy. **Recommendation (a)**, no further work here.

## Addendum: the open questions (2026-11-04)

Written when the FAN-01 diagnosis stack (jobs 49K-49R and 49M) was merged into main. No result, limit or known miss
above changes.
1. Question 1: **(a)**. The first-order run stays in the examples, labelled for viewing. Decided under the 2026-11-04
   loop instruction to take the documented recommendation.
2. Question 2 (the 6 MB per-tree cap of `axial-fan`): **open**. This ADR gives no recommendation for it, so none is
   invented here and the maintainer still has to decide it. Until then, the script and the tests check the registered
   value.
3. Question 3: **(a)**. The known miss `g4_fan_torque_within_5_percent_of_s10_known_miss` stays, and no further work is
   done here. Decided under the 2026-11-04 loop instruction to take the documented recommendation.
