# ADR 0157: Job 48W, merging job 48U (rotated aerofoil cases) with job 48P's per-platform trees; Linux-built runs of the two rotated cases

Status: **registered 2026-10-29**, before any registered run (the merge was resolved and the changed tests compiled
first; nothing was run); amendments 1 and 2 (two set-up defects, Q1's package name and S1's binaries) before their
corrected runs; results in the addendum: every registered limit met except S1, whose one failure is a pre-existing
timing test of ADR 0133 (R1) in code this job does not touch, which passed three times on its own (question 2). Not yet
accepted by the maintainer. House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed here before the registered runs; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before
the corrected run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not
re-recorded; no new crate, no unsafe code, the crate graph stays acyclic. Parents: [ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md)
(job 48U: `[mesh.transform]`, two rotated aerofoil cases and their Windows-built runs, `--add-missing`),
[ADR 0150](0150-job48p-linux-example-stores.md) (job 48P: the Linux tree `example-projects-linux/`, per-tree checks,
`--docker`), [ADR 0152](0152-job48r-store-key-at-its-default-is-not-the-case-s-own.md) (job 48R: DV5, DV6),
[ADR 0142](0142-wave48h-example-projects.md) (the example projects, their checks and caps).

## Problem

Job 48U's branch (`worktree-agent-a19b8077ee9b9bd5a`, based on 6867e40) is merged into local main aad08fe, which holds
job 48P (the Linux tree) and job 48R (DV5, DV6). Textual conflicts: `crates/rcfd-mcp/tests/example_projects_w48h.rs`
(three hunks) and `scripts/make_example_projects.py` (two hunks). Semantic conflicts without a textual one: the Linux
tree has neither the two rotated cases nor their runs, so ADR 0150's cross-tree checks (T1 inputs byte for byte, S3's
per-tree count, S4, S5's exact mesh hash on Linux) fail; ADR 0152's DV5 and DV6 enumerate `example-projects/*/cases/`
and assert counts and labels made on 6867e40 (17 texts; 22 cases), and with the two new cases there are 19 and 24.

## Decisions

1. **D1 The test file keeps both sides.** `example_projects_w48h.rs`: 48P's `TREES` and run-table markers with 48U's four
   `MESH_FILE_CASES`; S3 counts 12 case files per tree (10 of ADR 0142, 2 of ADR 0155); S4 runs per tree (48P) with
   48U's branch for a rotated case (flow along +x, angle `-rotate_z_deg` about the quarter chord). Every other per-tree
   count holds unchanged with the two cases: 8 generator cases (E1), 2 projects (S1), 2 cylinder runs in the other tree
   (T3), the G2 limits (each run <= 180 s, the sum per tree <= 20 min). `crates/rcfd-studio/tests/example_projects_h.rs`
   merged without a conflict: H1 opens the tree of this platform with 8 wind-tunnel cases and runs (4 quick-start), so on
   x86_64 Linux it needs the Linux tree's two new runs.
2. **D2 `--add-missing` works per platform tree.** `scripts/make_example_projects.py`: `--add-missing` (48U) runs only the
   spec's cases without a run in the tree of this platform (48P's `resolve_tree`); for the Linux tree the Windows tree's
   inputs are copied first (48P's `sync_inputs`), so the new case files and notes arrive before the missing runs are
   looked for. `--docker --add-missing` passes `--add-missing` to the script in the container (`docker_inner`).
   `--add-missing` with `--rerun-check` or `--summarise-only` is refused. The self-test gains one test for the forwarding
   and the refusals. Nothing else in the script changes.
3. **D3 The Linux tree's regeneration command.** Options: (a) `python scripts/make_example_projects.py --docker
   --add-missing`: ADR 0150 D4's container (same image, limits, volumes, sources, closing `--check` of both trees) with
   48U's option, which copies the inputs (cases, notes, mesh recipe) from the Windows tree, runs only the two rotated
   cases, keeps the ten Linux runs of ADR 0150 (and the Linux-built polyMesh) byte for byte, and rewrites the ledger, the
   run table and the manifest; this mirrors how ADR 0155 added the Windows runs. (b) `python
   scripts/make_example_projects.py --docker` (ADR 0150 D4's documented full rebuild): all twelve Linux runs again, ten
   of them replacing ADR 0150's (new ids, time stamps, absolute paths; the byte-reproducible files equal, per ADR 0150
   R1L), so the Linux tree would then differ from the Windows tree in how its runs came about, and the repository gains
   a second copy of every non-reproducible file of the ten runs. **Pick (a)**: the smallest change that gives the Linux
   tree the two runs, made by the real solver through the same script and container. That a full rebuild (b) gives the
   same bytes is checked anyway: R1L below runs `--docker --rerun-check`, which rebuilds all twelve Linux runs in a
   temporary copy and compares. (This is the one place where this job departs from the wording of its brief, which
   named `--docker`; question 1.)
4. **D4 DV5 and DV6 (ADR 0152).** DV6 takes the cases its record (`crates/rcfd-studio/tests/fixtures/plans_6867e40.txt`)
   names: still 22, compared byte for byte as registered in ADR 0152; the cases it leaves out must be exactly the two
   rotated cases, by name; a recorded case that is gone still fails its label comparison. The record and its recorder
   (`plans_record_r.rs`, `dv::unedited`) are unchanged; `dv::unedited`'s doc comment says that cases added since 6867e40
   are listed too. DV5 compares two routes of today's code with no record, so it takes the two new cases: 19 texts
   (was 17). A note in ADR 0152 says so.
5. **D5 Size caps unchanged.** ADR 0142 D7 / ADR 0150 D7 per tree: `wind-tunnel` <= 35 000 000 bytes, `quick-start`
   <= 6 000 000, a whole tree <= 40 000 000. Expected (Windows tree after 48U: wind-tunnel 34 826 293, 173 707 under its
   cap; Linux tree now 27 402 312; an aerofoil run about 3.71 MB): Linux wind-tunnel about 34.82 MB, total about
   39.08 MB. **Stop rule:** if the Linux tree exceeds any cap after L1, nothing is committed, `example-projects-linux/` is
   restored from git, and the options (raise a cap; store fewer snapshots; one platform's stores in git, the other built
   on demand) are reported for the maintainer to choose; no limit is changed here.
6. **D6 Notes in the parent ADRs.** ADR 0133 keeps both dated notes (48R's and 48U's; they merged without a conflict).
   ADR 0152 gets the D4 note. ADR 0150 and ADR 0155 get a short note pointing here.

## Placement

No crate's source changes in this job (the merge brings 48U's source). Changed here: `crates/rcfd-mcp/tests/
example_projects_w48h.rs` (conflict resolution), `crates/rcfd-studio/tests/saved_store_r.rs` (DV5, DV6),
`crates/rcfd-studio/tests/dv/mod.rs` (a doc comment), `scripts/make_example_projects.py`,
`scripts/test_make_example_projects.py`, `docs/adr/0150`, `0152`, `0155` (notes), `example-projects-linux/README.md`
(one command line, by hand: the tree's README is not written by the script), this ADR; then the rest of
`example-projects-linux/**` written by L1 only. No dependency, no workspace edge, `Cargo.toml` and `Cargo.lock` as
merged.

## Registered checks (limits fixed here, before any of them ran)

Set-up. Windows: this host, release, `--test-threads 1 --nocapture`, this worktree's target folder, `TMP`/`TEMP`
`E:\tmp\w48w` (outside any checkout), output to files, free memory checked before each heavy run (about 24 GB free of
94 GB at registration; other agents share the machine). Linux: ADR 0150 D4's image `rcfd-linux:ubuntu24.04-rust1.99.0`
(`sha256:485f28a2036497ed`..., built by job 48P from `scripts/docker/ubuntu-24.04-rust.Dockerfile`), `docker run --rm
--init --cpuset-cpus 0-3 --memory 16g`, the named volumes of D4; for L1 and R1L the script's own container; for L4, L5
and P1 (Linux) one container on a `git archive` of the named commit extracted into `/src` with `tar -m`, release,
`--test-threads 1 --nocapture`. The Windows tree is not rebuilt.

| Id | Check | Limit | Negative control |
|---|---|---|---|
| M0 | `python scripts/make_example_projects.py --check` on the merge commit, before L1 | problems only under `example-projects-linux` (its wind-tunnel manifest lists 6 runs and the spec 8; inputs that are missing or differ: the two rotated case files, the notes); 0 in `example-projects` | (it is the control of L1's closing check) |
| L1 | `python scripts/make_example_projects.py --docker --add-missing` on the merge commit (clean working tree) | container exit 0; the inputs of both projects copied; exactly 2 runs added, of `cases/naca0012-aoa4-rotated.toml` and `cases/naca0012-aoa2-rotated.toml`, each `finished` with solver state `not_converged` at iteration 2000, exit 5 (ADR 0142 G1 for the aerofoils); the closing `--check` of both trees: 0 problems; `git diff --stat` of the result: the 10 runs of ADR 0150 and `meshes/naca0012-ogrid/` unchanged byte for byte, changes only in the two new run folders, the copied inputs, the ledger, the run tables and `manifest.json` | |
| L2 | G2 on the Linux runs (the ledger's `wall_s`, measured in the container) | each <= 180 s; the sum of the 12 <= 20 min | (reported) |
| L3 | ADR 0155 E2 and E3 evaluated on the Linux runs (ADR 0137 rule; a scratch script mirrors `example_rotated_w48u.rs`'s `compare` and reads either tree; run on the Windows tree too, where it must print the test's values) | Linux: rotated against flow-rotated, `cl` and `cd` at iteration 2000 within 1e-6 relative and over iterations 1001-2000 within 1e-5; 0 < `cl`(2, rotated) < `cl`(4, rotated). A Linux miss is reported, not turned into a test or a tolerance here (question) | the 4-degree rotated run against the 2-degree flow run fails the 1e-6 limit for both; the swapped order fails E3 |
| T5 | Sizes | each tree under D5's caps (S8 in L4 and W1, and the script's check); reported: both trees' bytes, the margins, the bytes the regeneration commit adds | stop rule of D5 |
| L4 | `cargo test --release --locked -p rcfd-mcp --test example_projects_w48h` on Linux, on the commit holding the Linux tree | every test passes, the known miss ignored: S1-S9, E1, E2, G1-G3 on both trees; S3 12 case files per tree; S5's mesh hash equal for the 12 Linux runs bit for bit; T1, T2, T3 and their controls | T3: both cylinder runs of the Windows tree differ from the meshes built in the container; the T1, T2, S1 and S7 controls as in ADR 0150 |
| L5 | `cargo test --release --locked -p rcfd-studio --test example_projects_h` on Linux, the same container | H1: the Linux tree's projects open in `Studio::in_memory` with 8 (4) cases and 8 (4) runs, 0 skipped, every committed run `Ready`, every case valid in the Case editor | |
| W1 | `example_projects_w48h` on Windows, the same commit | every test passes, the known miss ignored; S5's mesh hash equal for the 12 Windows runs | T3: both cylinder runs of the Linux tree differ from the meshes built on Windows; the other controls as in L4 |
| W2 | `example_projects_h` on Windows | H1 with 8 (4) cases and runs from `example-projects/` | |
| R1L | `python scripts/make_example_projects.py --docker --rerun-check` on the commit holding the Linux tree | ADR 0142 D10's files of the 12 Linux runs and the 5 polyMesh files byte-equal (65 of 65) | |
| T4 | The Windows tree is 48U's | `git diff --stat worktree-agent-a19b8077ee9b9bd5a <final commit> -- example-projects/` is empty | |
| P1 | The script on both platforms | `--check`: 0 problems on Windows and in the L4 container; `python -m unittest scripts/test_make_example_projects.py` passes on both (Windows Python 3.9.13; the image's 3.12.3) | the self-test's tampered copies reported as in ADR 0150; the new test: `--add-missing` forwarded to the container, refused with `--rerun-check` and `--summarise-only` |
| DV | ADR 0152 DV5 and DV6 on Windows (in the suite run S1) | DV5: 19 texts, each plan equal between the two routes; DV6: the cases left out are exactly the two rotated ones, 22 of 22 plans byte-equal to the record and between the routes | DV6's saved control as in ADR 0152 |
| S1 | The whole `rcfd-studio` suite on Windows (`cargo build --release -p rcfd-claude --bin fake-claude` first; `--test-threads 1`) | 0 failed; ignored only the known misses and DV6's recorder, each named | |
| Q1 | Lint and graph | `cargo clippy --release -p rcfd-mcp -p rcfd-studio --tests -- -D warnings` clean on Windows; `cargo fmt -p rcfd-mcp -p rcfd-studio -- --check` clean; `cargo test --release -p rcfd-mcp --test workspace_graph` passes; `baseline_hashes.rs` unchanged (`git diff aad08fe -- crates/rcfd-api/tests/baseline_hashes.rs` shows only what 48U brought, i.e. nothing) | |

Order: M0; L1 (L2, L3 and T5 read from its output and files); the regeneration commit; L4, L5 and P1 (Linux) in one
container on an archive of that commit; R1L; W1, W2, P1 (Windows), Q1; S1 last (the heaviest; not overlapped with a
container).

## What I already knew (rule d)

* The code and ADRs read above; the sizes of both trees and the manifests at aad08fe and on 48U's branch; that no rcfd
  container was running at registration and that the image above exists on this host.
* Compiled, nothing run: the `rcfd-mcp` and `rcfd-studio` tests in release on Windows at the resolved merge.

## Questions for the maintainer (not decided here)

1. The regeneration command (D3): (a) `--docker --add-missing` as here; (b) a full `--docker` rebuild of the Linux tree.
   **Recommendation (a)**; R1L shows whether (b) would give the same bytes.

## Amendment 1 (2026-10-29, before the corrected run)

Q1 named the graph test in the wrong package: `cargo test --release -p rcfd-mcp --test workspace_graph` stops with `no test
target named workspace_graph in rcfd-mcp` (exit 101, nothing ran); the test is `crates/rcfd-cli/tests/workspace_graph.rs`.
Corrected command: `cargo test --release --locked -p rcfd-cli --test workspace_graph`; the limit (it passes) is unchanged.

## Amendment 2 (2026-10-29, before the corrected run)

S1's set-up built only `fake-claude`; the suite also needs `rcfd` and `rcfd-runner` of its profile (`crates/rcfd-studio/tests/
common/mod.rs` `binaries()`). The first run stopped after 14 s in `bins_g4` `g3_solver_check_refuses_a_stale_or_broken_rcfd`:
`target\release\rcfd-runner.exe missing` (3 passed, 1 failed there; no other test binary ran). Corrected set-up: `cargo build
--release --locked -p rcfd-cli -p rcfd-runner` and `-p rcfd-claude --bin fake-claude` first, then the suite with
`--no-fail-fast` (so that every binary reports); the limit is unchanged.

## Addendum: results against the registered limits (2026-10-29)

Commits: the merge 5f4b07e (parents aad08fe, 48U's d91c2d7; the conflict resolution, D2's script change, DV5/DV6, the
notes, this ADR's registration); the regeneration 1226977 (`example-projects-linux/` only, written by L1); then this
addendum and the two amendments (docs only). Logs: `E:\tmp\w48w\` (outside the checkout).

| Id | Limit | Result | Verdict |
|---|---|---|---|
| M0 | problems only under `example-projects-linux` | 5 problems, all in the Linux tree: wind-tunnel's manifest lists 6 runs and the spec 8; the two rotated case files missing; `notes/README.md` and `notes/naca0012.md` differ; 0 in `example-projects` | as required |
| L1 | exit 0; inputs copied; exactly 2 runs added, `finished` / `not_converged` at 2000, exit 5; closing check 0; the 10 runs and the polyMesh unchanged | container exit 0 (1579 source files; `rcfd` and `rcfd-runner` built in 1 min 41 s); inputs copied: 14 files (wind-tunnel), 7 (quick-start); added `r-20261029T000812-6c75` (aoa4-rotated) and `r-20261029T000812-00f3` (aoa2-rotated), both `finished`, solver `not_converged` at 2000, exit code 5; quick-start: nothing to add; closing `--check` of both trees: 0 problems; `git status`: 2 new run folders (28 files), the 2 copied case files, `manifest.json`, the ledger, `notes/README.md` (the copied text and the run table), `notes/naca0012.md`; no other file of the tree changed | pass |
| L2 | each <= 180 s; sum of 12 <= 20 min | the new runs 112.5 s and 108.9 s; the ten of ADR 0150 unchanged (largest 100.5 s); sum 569.0 s | pass |
| L3 | E2 1e-6 final, 1e-5 over 1001-2000; E3 order; the controls fail | Linux: aoa4 `cl` 6.43e-8 / 1.81e-7, `cd` 5.96e-8 / 1.43e-7; aoa2 `cl` 1.36e-8 / 4.69e-8, `cd` 7.94e-9 / 2.48e-8 (largest differences 7.9e-4 at iteration 26, early as on Windows); E3 `cl` 0.3482 (4 deg) > 0.1988 (2 deg) > 0. Control: 7.51e-1 and 3.44e-1 against 1e-6, fails as required; E3 swapped fails. The mirror script on the Windows tree printed the values of `example_rotated_w48u.rs` digit for digit (aoa4 `cl` 1.772e-8, ...), and that test passed in the L4 container too | pass |
| T5 | caps per tree; stop rule | Linux: wind-tunnel 34 823 333 bytes (176 667 under 35 000 000), quick-start 4 248 948, the tree 39 076 957 (923 043 under 40 000 000); Windows (48U's, unchanged): 34 826 293 and 39 080 436. The regeneration commit adds 30 blobs, 27 of them new to the repository, 7 425 291 bytes uncompressed (two `store.rts` of 2 574 878 each make most of it). Of the new runs' reproducible files only `case.snapshot` is byte-equal to the Windows tree's; `residuals.jsonl`, `store.rts`, `index.json` and `monitors.json` differ (the Linux tree's aerofoil polyMesh is its own, ADR 0150) | pass; stop rule not triggered |
| L4 | `example_projects_w48h` on Linux: all pass, the known miss ignored; S5 exact for the 12 Linux runs; T1-T3 | on an archive of 1226977: 21 passed, 0 failed, 1 ignored (`g3_cylinders_cross_at_least_7_times_known_miss`); S1 8 (4) runs per tree with 0 problems; S3 12 per tree; S4 the rotated cases 4 and 2 deg from the transform, equal to each tree's ledger; S5's mesh hash asserted for all 12 Linux runs; S8 as in T5; G2 Linux sum 569.0 s. T3: both Windows cylinder runs differ (stored 2159359318246829613, built 10791892230985098653); every other Windows run equal, including the two rotated ones. The S1, S7, T1 (a)-(c) reported, (d) not, T2 and G3 controls as required | pass |
| L5 | H1 on Linux: 8 (4) cases and runs | `/src/example-projects-linux/wind-tunnel`: 8 cases, 8 runs, 0 skipped; quick-start 4 and 4; 1 passed | pass |
| W1 | `example_projects_w48h` on Windows | 1226977: 21 passed, 0 failed, 1 ignored (the known miss); S5 exact for the 12 Windows runs; T3: both Linux cylinder runs differ (stored 10791892230985098653, built 2159359318246829613); the Linux rotated runs' stored hashes (503197937189928989, 8049855970594904693) equal the meshes built on Windows from the Linux tree's polyMesh, i.e. the rotation itself gave the same bits on both platforms here | pass |
| W2 | H1 on Windows with 8 (4) | `example-projects/wind-tunnel`: 8 cases, 8 runs, 0 skipped; 1 passed | pass |
| R1L | 65 of 65 byte-equal | `--docker --rerun-check` on 1226977 (1607 source files, 16 min 31 s): all 12 Linux runs rebuilt in a temporary copy with the states and step counts of the committed ones; 60 of 60 run files byte-equal and the 5 polyMesh files equal (0 problems); so a full `--docker` rebuild (D3 option b) gives the same reproducible bytes as L1's `--add-missing` | pass |
| T4 | `git diff --stat worktree-agent-a19b8077ee9b9bd5a 1226977 -- example-projects/` empty | empty | pass |
| P1 | `--check` 0 and the self-test on both | Windows (Python 3.9.13): `--check` 0 problems, self-test 15 passed; container (3.12.3): 0 problems, 15 passed (the new forwarding/refusal test among them) | pass |
| DV | DV5 19, DV6 22 and exactly the two left out | `dv5_explicit_non_default_store_keys_behave_as_before` ok, `dv6_unedited_plans_are_bit_identical` ok (in S1's run) | pass |
| S1 | the whole `rcfd-studio` suite: 0 failed | first attempt stopped by a set-up defect (amendment 2). Corrected run on 1226977 (12 min 24 s, about 10 GB free at its start): 249 passed, **1 failed**, 7 ignored. Ignored: the four known misses (ADR 0131 B9, ADR 0133 R6, ADR 0141 G1, ADR 0144 J3b), DV6's recorder, and ADR 0154's two tools `w48t_display_matches_store`, `w48t_dump_stores` (ignored by design). The failure: `run_control_r1_r3.rs` `r1_model_states_follow_status_json` (ADR 0133 R1): one of 43 snapshots showed `r-20261029T004404-452c` Queued while `status.json` read just before said Queued and just after Running, but the run's own state sequence held only `[Queued]` at the time the test read it. This test and `crates/rcfd-studio/src/runs.rs` are untouched by this job and by 48U. Re-run alone three times (`--test run_control_r1_r3 --test-threads 1`): 5 passed each (R1 checked 47, 37 and 38 snapshots, 0 problems) | **missed on the counted run** (a timing test outside this job's change); question 2 |
| Q1 | clippy, fmt, graph, baselines | `cargo clippy --release --locked -p rcfd-mcp -p rcfd-studio --tests -- -D warnings` clean on Windows and in the container; `cargo fmt -p rcfd-mcp -p rcfd-studio -- --check` clean (run on the resolved merge before its commit, after the registration was written); `workspace_graph` (amendment 1: `-p rcfd-cli`) 3 passed; `git diff aad08fe 1226977 -- crates/rcfd-api/tests/baseline_hashes.rs`: empty | pass |

**Deviations.**
1. D3: the Linux tree was regenerated with `--docker --add-missing`, not the plain `--docker` the brief named; registered
   with reasons before the run; R1L shows the plain rebuild gives the same reproducible bytes (question 1).
2. Amendment 1 (Q1's package) and amendment 2 (S1's binaries): set-up defects, corrected before the corrected runs.
3. S1's one failure was followed by three re-runs of that test file alone; the counted result stays a miss.
4. Unregistered, run in the L4 container: `example_rotated_w48u` on Linux (2 passed; it reads the Windows tree) and
   clippy there.
5. `example-projects-linux/README.md` got one command line by hand in the merge commit (the tree's README is not
   written by the script), noted in Placement before the run.

**Not run.** The full gate (`rcfd-api`, `rcfd-core`, `rcfd-mesh` suites and the rest) on this merge, on either platform:
only `rcfd-studio`'s suite, the example tests and `workspace_graph` ran (48U's ADR 0155 H2 ran its suites on its own
branch). The `rcfd-studio` suite on Linux. `--add-missing` on Windows after the merge (the Windows tree needed nothing;
its code path is the Linux one minus the copy). The "Not in CI" checklist (a person at the window). CI.

## Questions for the maintainer (continued)

2. S1's miss, `r1_model_states_follow_status_json` (ADR 0133 R1), failed once in the whole suite and passed three times
   alone. Options: (a) accept as a timing flake of the shared host (the suite ran while other agents and their
   containers used the machine) and leave the test as it is; (b) make it an ignored `*_known_miss` with its limit, and
   open a job to find whether the test's before/after bracket of `status.json` can miss a transition the Studio already
   showed; (c) re-run the whole suite on a quiet machine before deciding. **Recommendation (c), then (a) if it passes**:
   nothing this job changed is on that path, and a known miss would hide a test that passes.
3. Size margin: the Linux wind-tunnel is 176 667 bytes under its cap, the Windows one 173 707; the next aerofoil run in
   either tree does not fit. Options (as ADR 0155 question 5): (a) raise the per-project caps; (b) keep one platform's
   stores in git, the other built on demand (`--docker`); (c) move the stores out of git (release asset or LFS).
   **Recommendation (b)** before the examples grow, (c) later; nothing changed here.

## Amendment 3 (2026-10-29, after the addendum, before these runs)

The merge also brings 48U's record-based `rcfd-api` tests, which nothing above ran on the merged tree, and touches
`rcfd-core`, `rcfd-mesh` and `rcfd-api`, which Q1 did not lint. Added checks, limits fixed here:

| Id | Check | Limit |
|---|---|---|
| H1m | `cargo test --release --locked -p rcfd-api --test case_hashes_w48u --test mesh_transform_w48u --test w48t_cylinder_spokes --test end_time_landing_w48k -- --test-threads 1 --nocapture` on 1f110bf, on Windows and in the container of L4 (an archive of that commit) | every test passes, their own ignored tests ignored. If `case_hashes_w48u` fails only on case files added after 6867e40 (labels), the fix mirrors DV6 (D4) with a note here and in ADR 0155; if it fails on a hash of a recorded case, nothing is changed: stop and report |
| Q2 | `cargo clippy --release --locked -p rcfd-core -p rcfd-mesh -p rcfd-api --all-targets -- -D warnings` and `cargo fmt -p rcfd-core -p rcfd-mesh -p rcfd-api -- --check`, on Windows and in the same container | clean |
| S1r | The whole `rcfd-studio` suite again on Windows (question 2 option (c)), set-up of amendment 2, not overlapped with a container | 0 failed; reported next to S1, whose counted miss stays as recorded |

### Results of amendment 3 (2026-10-29)

| Id | Result | Verdict |
|---|---|---|
| H1m | Windows (c08e6d8, the same code as 1f110bf): `case_hashes_w48u` 1 passed (the cases added after 6867e40 are printed as "not in the record (new)": the two rotated cases and one block of `docs/case-schema.md`; it compares only recorded labels, so no DV6-style fix was needed), `mesh_transform_w48u` 13 passed, `end_time_landing_w48k` 8 passed, `w48t_cylinder_spokes` 0 passed and 7 ignored (its tools and runs ignored by design). Container (an archive of c08e6d8): the same, 1, 13, 8 passed, 7 ignored | pass |
| Q2 | `cargo clippy --release --locked -p rcfd-core -p rcfd-mesh -p rcfd-api --all-targets -- -D warnings` and `cargo fmt ... --check` clean on Windows and in the container | pass |
| S1r | the whole `rcfd-studio` suite again on Windows (10 min 49 s, about 9.4 GB free at its start): 250 passed, 0 failed, 7 ignored (the same seven as S1); `r1_model_states_follow_status_json` ok | pass; S1's counted miss stays recorded |

Not run after amendment 3: the remaining suites of `rcfd-api`, `rcfd-core`, `rcfd-mesh`, `rcfd-mcp`, `rcfd-cli`, `rcfd-io`,
`rcfd-post`, `rcfd-gui` on the merge (48U's ADR 0155 H2 ran them on its own branch), and the `rcfd-studio` suite on Linux.
