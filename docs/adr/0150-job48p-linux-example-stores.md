# ADR 0150: Job 48P, Linux-built example stores: a second tree `example-projects-linux/`, built by the same script in Docker

Status: **registered 2026-10-28**, before any registered run (the code, the tests and the Docker image were written,
built and compiled first; what ran before is listed under "What I already knew"); amendment 1 (a set-up defect of P1's
new self-test) before its corrected run; results in the addendum: every registered limit met and every control as
required (P1 after amendment 1); the Linux Re 100 cylinder run misses G3's crossing count as the Windows one does (the
same ignored known miss, limit intact); not yet accepted by the maintainer. Job 48P, on local main 6867e40 (waves
48A-48O merged). House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed here
before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective
set-up gets an amendment written before the corrected run; defaults of existing code stay bit-identical;
`crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new crate, no unsafe code, the crate graph stays
acyclic. Parents: [ADR 0142](0142-wave48h-example-projects.md) (the example projects and their checks), [ADR
0137](0137-linux-gate-platform-values.md) (per-platform values).

## Problem

The full gate on `ubuntu-latest` (run 40745280871, as relayed) failed one test: `crates/rcfd-mcp/tests/
example_projects_w48h.rs` `s5_run_stores_residuals_and_metrics_match_their_case`, `wind-tunnel/r-20261028T152128-d5cb:
store mesh hash` left 2159359318246829613, right 10791892230985098653. The committed example runs were built on Windows
(ADR 0142); the cylinder's `cylinder_ogrid` generator places its points with `sin`, `cos` and a graded spacing, whose
last bits differ per platform (`f64::powi` and libm, up to 5 ulp, ADR 0137), so the mesh Linux builds from the run's case
is not the mesh the store holds. Left is the store (its `index.json` says `1df795dbf2882e2d`, which is
2159359318246829613), right the mesh built on Linux.

## The maintainer's decision (relayed)

Ship Linux-built stores too: not a tolerance, not a known miss. The cleanest layout for per-platform stores such that
(1) on Windows everything stays exactly as today, (2) on Linux the same checks pass exactly against Linux-built runs,
(3) the Studio on either platform opens the example and shows the runs built on its own platform, with no
duplicate-looking run lists. Register the options considered and the pick with reasons; the size cap of 40 MB per tree
(ADR 0142 S8) and how it applies now; keep the repository's growth small; the Linux runs made by the real rcfd through
the script, none by hand; the script builds the Linux set in Docker (the command documented); `--check`,
`--rerun-check` and the tests extended; ADR 0142 G3's sanity limits evaluated again on the Linux runs (ADR 0137's
rule), a miss reported, never loosened.

## Context (read at 6867e40)

* What reads the committed examples: `crates/rcfd-mcp/tests/example_projects_w48h.rs` (ADR 0142 S1-S9, E1, E2, G1-G3;
  the only comparison with something computed on the test's platform is S5's store mesh hash against
  `build_mesh(case.snapshot).content_hash()`; everything else compares committed files with each other),
  `crates/rcfd-studio/tests/example_projects_h.rs` (H1: the Studio model lists 6 and 4 runs, `listed == runs.len()`),
  `scripts/make_example_projects.py --check` and `scripts/test_make_example_projects.py` (in CI's `scripts` job on
  `ubuntu-latest`; no mesh is built), `crates/rcfd-studio/tests/scale_n.rs` (a copy of the wind-tunnel project: captions,
  scale, "Refine x2"; no store is compared with a mesh) and `crates/rcfd-api/tests/end_time_landing_w48k.rs` (one case
  file). In the Linux gate only S5 failed.
* The Studio lists every folder under a project's `runs/` (`rcfd-project` `Project::runs`); a run record names no
  platform (ADR 0130); nothing in the Studio compares a store's mesh with its case's (`preview` hashes its own mesh).
* Which meshes depend on the platform: the cylinders' `cylinder_ogrid` (graded spacing); the step and the cavities are
  uniform boxes (in the Linux gate the four quick-start runs passed S5 before the loop stopped at the first cylinder run;
  the step was not reached); the aerofoil reads `meshes/naca0012-ogrid`, which the script builds with `rcfd convert`
  (the same generator) and Python's `math` (the platform's libm), and a mesh read from a file is the same on every
  platform.
* Per-platform precedent (ADR 0137, `baseline_hashes.rs`): `cfg!(windows)`, `cfg!(all(target_os = "linux", target_arch
  = "x86_64"))`, other platforms print and skip.
* `example-projects/.gitattributes` holds `* -text` (snapshot hashes, binary stores); the repository's `.gitattributes`
  normalises text otherwise.
* CI: `gate.yml` and `ci.yml` run on `ubuntu-latest`, Ubuntu 24.04 (glibc 2.39), with Rust 1.99.0
  (`RUST_GATE_TOOLCHAIN`). Earlier Linux work used Docker `rust:1.99-bookworm` (glibc 2.36, rustc 1.99.1; ADRs 0137, 0143,
  0146).

## Options considered (registered)

| Option | What it is | Verdict |
|---|---|---|
| A. Both platforms' runs in each project's `runs/`, the manifest naming the ids of each platform | one tree; the tests choose ids by platform | rejected: the Studio lists every folder under `runs/`, so each case shows two runs that look the same (requirement 3); H1's and S1's counts change on Windows (requirement 1); one 40 MB cap would hold both sets (about 63 MB) |
| B. A run folder per platform in each project (`runs/`, `runs-linux/`), chosen by a manifest flag | one tree; the Studio told which folder to list | rejected: the folder contract (ADR 0130, `docs/ui-restart-plan.md` section 8) and `rcfd-project` change, or a set-up step swaps folders in the checkout (a changed working tree on Linux) |
| C. A platform tag in `run.json`; the Studio and the project listing filter by it | one tree, filtered views | rejected: changes the run-record contract, `rcfd-runner`, `rcfd-project` and the Studio (production code and their behaviour); the committed Windows runs carry no tag; a user's runs copied between machines would vanish from the list |
| D. Mesh generators without libm | no second set | rejected: changes every generator mesh and every default hash (the bit-identical rule, `baseline_hashes.rs`); not this job's |
| E. A second project per platform inside `example-projects/` (`wind-tunnel-linux/`, ...) | one tree, four projects | rejected: the Windows tree's manifest and checks change (requirement 1); the tree's 40 MB cap would be exceeded |
| **F. A second tree `example-projects-linux/` holding the same two projects, built by the same script in Docker** | two trees, each with one platform's runs | **chosen** |

Reasons for F: `example-projects/` stays byte for byte as it is (requirement 1; T4); the Linux tree is a full set of the
same projects with runs made on Linux, so the Studio opened on it lists each case's run once (requirement 3; L5); every
check of ADR 0142 applies to it unchanged and S5 exactly (requirement 2; L4); no production code, run-record contract
or default changes; each tree keeps ADR 0142's caps on its own; git stores a file that is byte-identical in both trees
once, so the repository grows by the files that differ (T5 measures it).

## Decisions (as implemented)

1. **D1 Layout.** `example-projects/` (ADR 0142, runs built on Windows) is not changed. `example-projects-linux/` holds
   the same two projects (`wind-tunnel`, `quick-start`); a `manifest.json` in ADR 0142's format plus a `platform` block
   (`os`, `arch`, `libc`, `python`, `image`, filled in by the script where it runs) and `inputs_from`; a `README.md`
   written for this tree (what it is, how it is built, the Studio command on Linux); `.gitattributes` (the Windows
   tree's `* -text`).
2. **D2 The inputs come from the Windows tree.** The inputs of a project are the files a rebuild does not write: not
   under `runs/` or `.rcfd/`, not `designs/variants.ledger.jsonl`, not in a mesh folder built from a recipe
   (`meshes/<name>/` next to `meshes/<name>.recipe.toml`). A rebuild of the Linux tree first copies the Windows tree's
   inputs into it (and removes inputs the Windows tree no longer has), then rebuilds as ADR 0142 D2 does: the aerofoil
   polyMesh from its recipe on Linux, the runs, the ledger, the notes' run table. So `notes/README.md` differs between
   the trees only inside its generated run table, and `rcfd-project.json` is the Windows tree's (written once, ADR 0142).
3. **D3 A rebuild writes only the tree of its platform.** `this_platform()` is `windows` (`os.name == "nt"`), `linux`
   (Linux on x86_64), or none, as the Rust tests decide with `cfg!`. A rebuild, `--summarise-only` and `--rerun-check` of a
   committed tree run only on that tree's platform; elsewhere the script stops and names the way (`--docker` for the
   Linux tree). `--root <folder>` that is not a committed tree is not guarded (scratch rebuilds). Without `--tree` and
   `--root`, the tree of this platform: on Windows `example-projects/`, as before.
4. **D4 The Linux build in Docker.** `scripts/docker/ubuntu-24.04-rust.Dockerfile`: `ubuntu:24.04` (glibc 2.39, the CI
   runners' system) with `build-essential`, `pkg-config`, `python3` and Rust 1.99.0 (`RUST_GATE_TOOLCHAIN`) with clippy
   and rustfmt; image `rcfd-linux:ubuntu24.04-rust1.99.0`. The command, from a checkout on any machine with Docker:

       python scripts/make_example_projects.py --docker

   It builds the image when missing; writes a tar of the checkout's files as they are in the working tree (`git ls-files
   --cached --others --exclude-standard`); runs one container (`docker run --rm --init --cpuset-cpus 0-3 --memory 16g`,
   named volumes `rcfd-examples-target` for the cargo target folder and `rcfd-examples-cargo-registry` for the crates)
   that extracts the tar into its own file system at `/src` (`tar -m`, so every source is newer than a cached build),
   builds `rcfd` and `rcfd-runner` (`cargo build --release --locked`) and runs `python3 scripts/make_example_projects.py
   --tree linux --bin-dir /target/release` there (the run service's lock and status files never sit on a Windows bind
   mount); the finished tree comes back as a tar and replaces `example-projects-linux/`; then `--check` of both trees.
   `--docker --rerun-check` runs the script's `--rerun-check` of the Linux tree in a container and copies nothing back.
   `--docker-image`, `--docker-cpus` and `--docker-memory` change the image and the container's limits.
5. **D5 Checks of the script.** `--check` without `--root` checks both trees: ADR 0142's checks of each tree, the
   platform each was built on (T2) and the inputs the trees share (T1). `--check --root <folder>` checks that folder as
   before. The self-test checks both trees and a tampered pair of copies, and that a rebuild of the other platform's tree
   is refused.
6. **D6 The tests choose the tree per check.** `example_projects_w48h.rs` runs every check of ADR 0142 on both trees on
   every platform, with one exception: S5's store mesh hash is asserted for the tree of this platform (`cfg!(windows)`:
   `example-projects`; `cfg!(all(target_os = "linux", target_arch = "x86_64"))`: `example-projects-linux`) and printed for
   the other tree (and for both on other platforms). Cell counts and the other S5 limits hold for both trees. Counts are
   per tree: 2 projects (S1), 10 cases (S3), 8 generator cases (E1), the G2 sum. The controls that tamper a copy (S1, S7)
   use this platform's tree (the Windows tree elsewhere). New: T1, T2 and their controls, T3.
   `example_projects_h.rs` (H1) opens the tree of this platform (`example-projects-linux` on x86_64 Linux, else
   `example-projects`); on Windows it is the test it was. `scale_n.rs` and `end_time_landing_w48k.rs` keep reading the
   Windows tree: they compare no store with a mesh and passed in the Linux gate.
7. **D7 The size caps apply per tree.** ADR 0142 D7's caps apply to each tree on its own: `wind-tunnel` at most
   35 000 000 bytes, `quick-start` 6 000 000, a whole tree (its projects with their committed runs, `manifest.json`,
   `README.md`, `.gitattributes`) 40 000 000. Both trees together, about 63 MB of files in a checkout, are not capped
   together; the repository's growth is reported (T5).
8. **D8 Where Linux users are pointed.** The repository's `README.md` (next to the Studio launch lines) and
   `example-projects-linux/README.md`. `example-projects/README.md` is left as it is (requirement 1; question 1).

## Placement

No crate's source changes; no dependency, no workspace edge (`Cargo.toml`, `Cargo.lock`, `workspace_graph.rs`,
`docs/architecture.md` unchanged). Changed: `scripts/make_example_projects.py`, `scripts/test_make_example_projects.py`,
`crates/rcfd-mcp/tests/example_projects_w48h.rs`, `crates/rcfd-studio/tests/example_projects_h.rs`, `README.md` (one
paragraph). New: `scripts/docker/ubuntu-24.04-rust.Dockerfile`, `example-projects-linux/` (all of it written by the
counted build except its `README.md` and `.gitattributes`), this ADR. `ci.yml` (its `scripts` job runs the self-test,
which now covers both trees) and `gate.yml` unchanged.

## Registered checks (limits fixed here, before any of them ran)

Linux: the image of D4, `docker run --rm --init --cpuset-cpus 0-3 --memory 16g`, the volumes of D4, the sources a `git
archive` of the named commit extracted into the container's `/src` with `tar -m`, release, `--test-threads 1
--nocapture`, output to files. Windows: this host, release, `--test-threads 1 --nocapture`, this worktree's target
folder, output to files. Free memory is checked before each heavy run.

| Id | Check | Limit | Negative control (must fail or discriminate) |
|---|---|---|---|
| C0 | The failure reproduced on Linux before any change: `cargo test --release --locked -p rcfd-mcp --test example_projects_w48h` on 6867e40 | exactly one failure, `s5_run_stores_residuals_and_metrics_match_their_case`, at `wind-tunnel/r-20261028T152128-d5cb: store mesh hash` with left 2159359318246829613 and right 10791892230985098653 (the CI values); the 15 other tests pass, the known miss ignored. A different right value means that the image does not build the cylinder mesh as CI does: then no Linux tree is built from it (stop and report) | (it is the control of L4's S5) |
| L1 | The counted Linux build: `python scripts/make_example_projects.py --docker` on the registration commit (clean working tree) | the container exits 0; 10 of 10 runs `finished` with ADR 0142 G1's solver states (cylinders `finished` at step 1280 and time 80, exit 0; aerofoils `not_converged` at iteration 2000, exit 5; steps, cavities and heated cavities `converged`, exit 0); the closing `--check` of both trees: 0 problems | |
| L2 | ADR 0142 G2 on the Linux runs (the ledger's `wall_s`, measured in the container) | each <= 180 s; their sum <= 20 min | (reported) |
| L3 | ADR 0142 G3 on the Linux runs | the ten comparisons hold: `cl_amplitude` > 0.1 at Re 100 and 200, St(100) < St(200), 0 < `cl`(2) < `cl`(4), 0 < `x_reattach`(400) < `x_reattach`(800) < 15, 1 < `nu_hot`(1e3) < `nu_hot`(1e4), `lid_drag` < 0 for both cavities. The crossing count (>= 7 for both cylinder runs) is evaluated and reported (the known-miss test run once with `--ignored` in the L4 container: it prints every cylinder run's count of both trees, then fails on the Windows tree's Re 100 run as registered in ADR 0142); its test stays ignored, and a Linux miss is the same known miss | the 4 swapped variant comparisons fail |
| L4 | `example_projects_w48h.rs` on Linux, on the commit holding the Linux tree | every test passes, the known miss ignored: S1-S9, E1, E2, G1-G3 on both trees; S5's mesh hash equal for the 10 Linux runs bit for bit; T1, T2, T3 and their controls | T3: both cylinder runs of the Windows tree differ from the meshes built in the container; T1 control (a)-(c) reported, (d) not; T2 control (a), (b) reported, the unchanged copy clean; the S1 and S7 controls as in ADR 0142 |
| L5 | H1 on Linux (`example_projects_h.rs`) | the Linux tree's projects open in `Studio::in_memory`: 6 (4) cases, 6 (4) runs, 0 skipped; every committed run `Ready` with its store's snapshot count, `p` shown at the last snapshot, drawn as 2D; every case valid in the Case editor | |
| W1 | `example_projects_w48h.rs` on Windows, the same commit | every test passes, the known miss ignored; S5's mesh hash equal for the 10 Windows runs, as before | T3: both cylinder runs of the Linux tree differ from the meshes built on Windows; the other controls as in L4 |
| W2 | H1 on Windows | ADR 0142 H1 on `example-projects/`, unchanged | |
| T4 | The Windows tree is unchanged | `git diff --stat 6867e40 <final commit> -- example-projects/` is empty | |
| P1 | The script on both platforms | `python scripts/make_example_projects.py --check`: 0 problems on Windows and in the container; `python -m unittest scripts/test_make_example_projects.py` passes on both (Windows: this host's Python 3.9; Linux: the image's 3.12) | the self-test's tampered copies reported: the Windows tree's (ADR 0142) and the pair of trees (a case changed in one tree, a note line, an extra input, the manifest's platform, a Windows `case_dir`); a rebuild of the other platform's tree refused |
| R1L | `python scripts/make_example_projects.py --docker --rerun-check` on the commit holding the Linux tree (a second container) | ADR 0142 D10's files of the 10 Linux runs and the 5 polyMesh files byte-equal (55 of 55) | |
| T5 | Sizes and growth | each tree under D7's caps (S8, in L4 and W1); reported: both trees' bytes, the bytes of the blobs this job's commits add to the repository (unique, uncompressed, and packed), and which Linux run files are byte-identical to the Windows tree's | (reported) |
| Q1 | Lint | `cargo clippy --release -p rcfd-mcp -p rcfd-studio --tests -- -D warnings` clean on Windows and in the container; `cargo fmt -p rcfd-mcp -p rcfd-studio -- --check` clean | |

Order of the runs: C0; L1 (L2 and L3 are read from its output and from L4); the Linux tree committed; L4, L5 and P1
(Linux) in one container on an archive of that commit; R1L; W1, W2 and P1 (Windows) on this host; T4, T5 and Q1 on the
final commit.

## What I already knew (rule d)

* The code reading above; the CI message as relayed; the store's `index.json` mesh hash of d5cb (`1df795dbf2882e2d` =
  2159359318246829613, the left value).
* Docker 29.9.2 on this host. The image of D4, built before registration: `ubuntu:24.04` (digest `sha256:24737cef17f4`...,
  Ubuntu 24.04.5), glibc 2.39-0ubuntu8.9, rustc 1.99.0 (b940084d7 2026-09-28, LLVM 23.1.1), Python 3.12.3.
* Compiled, nothing run: the two changed test files in release on Windows, clippy of `rcfd-mcp` and `rcfd-studio` with
  `--tests -D warnings` and `cargo fmt --check` clean there; in the container (to show that the bare image builds what
  the registered runs need, eframe and wgpu included): `rcfd` and `rcfd-runner` in release (51 s), the two test files in
  release (3 min 21 s), clippy of both crates with `--tests -D warnings` and `cargo fmt --check` clean. The container
  compiled a draft that differed from the registered code only in the known-miss test, which now counts every cylinder
  run of both trees before it asserts (so that L3 can read the Linux counts); Windows compiled the registered code.
* The script at the new code: `--check` reports exactly `example-projects-linux: missing (ADR 0150)`; `--check --root
  example-projects` 0 problems; the self-test's helper tests (9, three of them new) pass; its tests of the committed trees
  not run (the Linux tree is not built). At 6867e40 the self-test passed 8 of 8.
* No registered check of this ADR has run; no Linux run of the examples has been made.

## Amendment 1 (2026-10-28, a set-up defect of P1's new self-test found by its first registered run; before the corrected run)

The registered runs so far: C0 (on 6867e40), L1 (on 963c871: the Linux tree, committed as b5428fc), W1 and W2 (on
b5428fc), and P1 on Windows (on b5428fc): `--check` 0 problems; the self-test 13 of 14, the new
`test_a_linux_tree_out_of_step_is_reported` failed at its first assertion ("the unchanged copies"): 18 problems, each a
notes file naming a `../../` path into the repository (`../../cases/naca0012/case.json`, ...). The test copies both trees
into a temporary folder, where those paths cannot resolve; ADR 0142's addendum saw the same in its S7 control ("in the
copy outside the repository, the `../../` paths"). A defect of the procedure: none of the test's assertions on the
tampered pair had been read. P1 on Linux runs in the L4 container started before this was found and will meet the same
assertion.

Corrected set-up (the same test name; P1's limit and every assertion unchanged): the test leaves out the problems that
name a `../../` path (they concern where the copies are, not the trees) before it compares. What I knew when writing
this: the output of that run, and a throwaway check of the same copies (18 problems, each one naming a `../../` path,
and nothing else). The self-test runs again on both platforms on the commit of this amendment.

## Addendum: results against the registered limits (2026-10-28)

Branch `worktree-agent-a33a297ef7d8884e0`. Commits: registration 963c871 (on main 6867e40); b5428fc, the Linux tree as
L1 wrote it (159 files, nothing edited); amendment 1 112a755; f25803f (one comment line in the known-miss test, after
L3); then this addendum. Linux: the image of D4 (image `sha256:485f28a2036497ed`..., base `ubuntu:24.04`
`sha256:24737cef17f4`...; glibc 2.39-0ubuntu8.9, rustc 1.99.0, Python 3.12.3), `--init --cpuset-cpus 0-3 --memory 16g`,
archives extracted into `/src`. Windows: this host (Windows 11, 16 threads, 93.6 GB; 22.7 to 27.3 GB free during the
runs, other agents' builds running), release, Python 3.9.13.

| Id | Limit | Measured | Result |
|---|---|---|---|
| C0 | one failure, S5 at d5cb with the CI values; 15 pass | 6867e40: `s5_run_stores_residuals_and_metrics_match_their_case` failed at `wind-tunnel/r-20261028T152128-d5cb: store mesh hash`, left 2159359318246829613, right 10791892230985098653; 15 passed, 1 failed, 1 ignored | as required: the image builds the cylinder mesh as CI does |
| L1 | exit 0; 10 of 10 with G1's states; `--check` 0 | 963c871: container exit 0 (1368 source files; `rcfd` and `rcfd-runner` built in 54 s; inputs copied: 12 and 7 files; wind-tunnel 344 s, quick-start 6 s); cylinders `finished` at step 1280 (end time 80), exit 0; aerofoils `not_converged` at 2000, exit 5; steps `converged` at 740 and 429; cavities at 121 and 158; heated cavities at 339 and 278 (the Windows runs' step counts); `--check` of both trees: 0 problems | pass |
| L2 | each <= 180 s; sum <= 20 min | 0.5, 0.7 (cavities), 2.0, 1.6 (heated cavities), 19.9, 19.0 (cylinders), 100.5, 98.9 (aerofoils), 65.9, 38.5 s (steps); 347.7 s in all (the Windows tree: 191.2 s) | pass |
| L3 | the ten comparisons; crossing count reported | `cl_amplitude` 0.30905 and 0.63030; St 0.151669 < 0.175138; 0 < `cl` 0.198833 < 0.348195; 0 < `x_reattach` 4.2480 < 5.8881 < 15; 1 < `nu_hot` 1.11879 < 2.25718; `lid_drag` -0.004911 and -0.001609. Crossings: Re 100 **6**, Re 200 7 (the Windows tree: 6 and 7) | pass; the crossing count misses for the Linux Re 100 run as for the Windows one, the same known miss (`g3_cylinders_cross_at_least_7_times_known_miss`, ignored, limit 7 intact) |
| L3 control | 4 swapped comparisons fail | 4 of 4 fail | as required |
| L4 | every test passes; S5 exact for the Linux runs | b5428fc: 21 passed, 0 failed, 1 ignored. S5 asserted for the 10 Linux runs: store mesh hash equal to the mesh built in the container (cavities 9134465315737883872, heated cavities 15615021881842952904, cylinders 10791892230985098653, aerofoils 11543926204880617909, steps 16720390881750798361); T1: 12 and 7 inputs, 0 differ; T2: 0 problems for both trees (the Linux manifest: `{"arch":"x86_64","image":"rcfd-linux:ubuntu24.04-rust1.99.0","libc":"glibc 2.39","os":"linux","python":"3.12.3"}`; the Windows one has none, as registered); S8: 27 402 312 and 4 248 948 bytes, the Linux tree 31 655 580 | pass |
| L4 controls | T3; T1, T2, S1, S7 controls | T3: the Windows tree's cylinder stores (2159359318246829613) differ from the meshes built in the container (10791892230985098653); its other 8 runs equal (uniform boxes; the aerofoils read the Windows tree's own polyMesh file). T1 control: (a) `cases/cavity-re100.toml: differs`, (b) `cases/extra.toml: in ... but not an input`, (c) `notes/README.md: differs`, (d) nothing. T2 control: the unchanged copy clean; (a) `manifest.json: built on "windows" "x86_64"`; (b) the run's `case_dir`. S1 (a)-(d) and S7 as in ADR 0142 | as required |
| L5 | the Studio model on the Linux tree | b5428fc: 1 passed: `/src/example-projects-linux/wind-tunnel` 6 cases, 6 runs, 0 skipped; `quick-start` 4, 4, 0; every run `Ready` with 80, 80, 10, 10, 19, 11, 25, 32, 34, 28 snapshots, `p` at the last one, drawn 2D; the 10 cases valid in the Case editor | pass |
| W1 | every test passes; S5 as before | b5428fc (this worktree): 21 passed, 0 failed, 1 ignored; the Windows tree's S5 asserted as before; T3: the Linux tree's cylinder stores (10791892230985098653) differ from the meshes built on Windows (2159359318246829613), its other 8 runs equal; S8 unchanged (31 658 565) | pass; T3 as required |
| W2 | ADR 0142 H1 unchanged | b5428fc: 1 passed; `example-projects` 6 cases, 6 runs, 0 skipped and 4, 4, 0; the snapshot counts of ADR 0142 | pass |
| T4 | empty diff | `git diff --stat 6867e40 f25803f -- example-projects/`: empty | pass |
| P1 | `--check` 0 on both; the self-test passes on both | b5428fc: `--check` 0 problems on Windows and Linux; the self-test 13 of 14 on both (`test_a_linux_tree_out_of_step_is_reported`, amendment 1). 112a755: 14 of 14 on Windows (Python 3.9.13) and Linux (3.12.3), `--check` 0 problems on both; the tampered pair reported as registered, the other platform's tree refused | pass (after amendment 1) |
| R1L | 55 of 55 byte-equal | 112a755: the Linux tree rebuilt in a temporary copy in a second container (the same states and steps): 50 of 50 run files byte-equal, the 5 polyMesh files equal (no problem), `rerun check: 0 problem(s)` | pass |
| T5 | caps per tree; growth reported | below | pass (caps); reported |
| Q1 | clippy and fmt clean on both | Windows: before registration and on the archive of f25803f; Linux: on b5428fc and on the archive of f25803f: clean | pass |

**Final runs on archives of f25803f** (whose code every later commit has). Linux, one container: `example_projects_w48h`
21 passed, 1 ignored; the known-miss test with `--ignored` printed 6, 7 (Windows tree) and 6, 7 (Linux tree) and failed
on the Windows tree's Re 100 run, as registered in ADR 0142; H1 1 passed; `--check` 0 problems; the self-test 14 of 14;
clippy and fmt clean. Windows, an extracted archive with its own target folder: `example_projects_w48h` 21 passed, 1
ignored; H1 1 passed; `--check` 0 problems; the self-test 14 of 14; clippy and fmt clean; `docs_site` (this ADR's
links) 2 passed.

**Sizes and growth (T5).** Bytes of every file: `example-projects/` 31 658 565, `example-projects-linux/` 31 655 580, each
under its 40 000 000 cap (projects under 35 000 000 and 6 000 000), about 63 MB together in a checkout. In git, the Linux
tree commit b5428fc adds 78 blobs the repository did not hold: 18 877 244 bytes uncompressed, 11 270 905 bytes as one
pack. The 81 Linux files that are byte-identical to a file of the Windows tree (11 983 271 bytes) are the same blobs and
add nothing. The whole job (6867e40..f25803f): 89 new blobs, 19 131 980 bytes uncompressed, 11 343 206 bytes packed.
For scale: the Windows tree's 161 blobs pack to 17 310 395 bytes. A transfer may be larger than what is stored: `git
pack-objects --revs b5428fc ^963c871` packs 17 312 577 bytes with git's default sparse walk (`pack.useSparse`), which does
not find the parent's identical blobs at other paths, and 11 270 905 with `pack.useSparse=false`.

Byte-identical across the platforms (matched by case): for the step, cavity and heated-cavity runs the store,
`index.json`, `residuals.jsonl`, `output/monitors.json`, `monitors.csv`, `report.html` and `check.json` (their meshes are
uniform boxes and their runs are bit-identical); for the cylinders and the aerofoils only `case.snapshot` (and the
cylinders' `report.html`). The aerofoil polyMesh's `boundary`, `faces`, `owner` and `neighbour` are identical, its
`points` differ (built by `rcfd convert` and Python's `math` on glibc). `run.json`, `status.json`, `events.jsonl`,
`log.txt` and `metrics.json` differ by ids, times and paths, as ADR 0142 D10 says.

**What the Linux runs give** against the Windows runs (orientation, not a check): the step, the cavities and the heated
cavities bit-identical; the cylinders equal to about 13 digits (St 0.1516691046868687 against ...878, mean C_D
1.3232575505167183 against ...181); the aerofoils to about 7 digits (C_L 0.34819494914633836 against 0.34819495674563833 at
4 degrees, 0.19883331320623607 against 0.19883327814014404 at 2; C_D 0.0253602676 against 0.0253602671). S9: the Linux
polyMesh lies 2.5e-17 off the aerofoil surface and 3.6e-15 off the far-field circle (the Windows file: 4.4e-15 and
7.1e-15 read on Windows).

**Deviations and notes.**
1. Amendment 1: the new self-test's comparison of unchanged copies failed on the `../../` paths of copies outside the
   repository; corrected before its rerun, its assertions unchanged.
2. The container compiled a draft before registration that differed only in the known-miss test (disclosed under "What
   I already knew"); every registered Linux run built the registered code.
3. After L3, the known-miss test's comment names the Linux Re 100 count (f25803f, a comment only); the final runs on its
   archive gave the same results.
4. The Linux aerofoil runs use the Linux tree's own polyMesh, whose `points` differ from the Windows file's (D2: each
   tree builds its recipe mesh on its platform); both are the recipe's within S9's 1e-12.
5. The container's runs took longer than the Windows runs (347.7 s against 191.2 s; four CPUs of a machine shared with
   other agents' builds), inside G2's limits.
6. `scripts/limit_audit.py` on the changed files: class (iii) for `t2_control_a_foreign_build_is_reported` (and ADR
   0142's `s1_controls_tampered_copies_are_reported`, `s7_control_a_missing_file_is_reported`): each control asserts the
   report it expects as a positive fact, the form the heuristic does not recognise; class (iv) for T3's early `return` on
   a platform without a tree of its own (registered in D6: print and skip, ADR 0137). No other hit.
7. `README.md` of the repository gained the pointer of D8; `example-projects/README.md` is unchanged (question 1).
8. Leftover processes: none (every container ran with `--rm`; no runner was started on Windows).

**Not run.** The CI `Full gate` on `ubuntu-latest` (no push): the real check. C0 showed that the image builds the cylinder
mesh as CI did; the other eight Linux stores hold meshes that do not depend on the platform (uniform boxes, and the
aerofoil's file), so S5 should hold there. The workspace gate and workspace-wide clippy (only two test files, two scripts
and docs changed); `scale_n.rs` and `end_time_landing_w48k.rs` (unchanged, they read the Windows tree); the Studio at a
real window on either platform (the checklist); macOS and Linux on ARM; `--docker` from a Linux host (its `chown` of the
bind mount is untested); `cargo deny` (no dependency changed); CI's `min-toolchain` job (Rust 1.90 type-checks the
changed `rcfd-mcp` test; nothing in it is newer than 1.90 as far as read, not compiled with 1.90); CI's `scripts` job on
setup-python's current `3.x` (the self-test ran with 3.9.13 and 3.12.3). Only the cylinder mesh hash was compared with a
value CI itself printed; that CI agrees for the other eight runs is inferred from their meshes, and CI's glibc patch level
is not known here.

**New dependencies:** none (no crate, no `Cargo.lock` change, no workspace edge). New tool: the Docker image of D4, built
from the repository's Dockerfile.

## Not in CI (by hand)

On Linux at the window: `sh scripts/studio.sh example-projects-linux/wind-tunnel`: 6 cases, 6 runs, a picture when a run
is selected; the same on Windows with `example-projects\wind-tunnel` as before.

## Questions for the maintainer (open at registration)

1. **A line in `example-projects/README.md`** naming the Linux tree? Left byte-identical here (requirement 1); the
   repository's `README.md` and the Linux tree's own `README.md` point to it. Recommendation: yes, one line (a document
   of the tree, no run or check touched).
2. **Who rebuilds the Linux tree.** Every change to an input in `example-projects/` now needs a Docker rebuild before the
   tests pass on any platform (T1 runs in the Windows gate too). Options: (a) keep (the trees cannot drift); (b) check T1
   only on Linux. Recommendation: (a).
3. **Other platforms** (macOS, Linux on ARM) have no tree: S5's hash and T3 print and skip there. Recommendation: no
   third tree until a gate runs there.
4. **When CI's image moves** (`ubuntu-latest` to a newer Ubuntu, another glibc), the Linux stores may stop matching.
   Options: (a) rebuild the tree with a new image then; (b) name `ubuntu-24.04` instead of `ubuntu-latest` in
   `gate.yml`. Recommendation: (b), as a separate one-line change.
5. **Binary stores in git** (ADR 0142 question 1), now two sets. Recommendation: keep them in git until the regenerations
   are frequent; T5 gives the cost of one set.

Added by the addendum:

6. **`--docker` builds what is on disk** (the files git tracks or would track, as they are in the working tree), not
   HEAD, so an edited input can be rebuilt before it is committed; the counted build ran on a clean tree. Options: (a)
   keep (it prints the number of files it sends); (b) build from `git archive HEAD` only. Recommendation: (a).
7. **The launch scripts** could take an example's name and open the tree of the platform (`scripts/studio.sh --example
   wind-tunnel`). Options: (a) no, the READMEs name the folder; (b) add it (ADR 0141's scripts and their self-test
   change). Recommendation: (a) for now.
8. **The Linux Re 100 crossing count** (6, limit 7) is the Windows known miss repeated; ADR 0142 amendment 1's reason (an
   80 D/U run holds about 6 periods in its second half) holds on both platforms. Nothing to decide unless the run length
   or the limit is to change (ADR 0142's open point).

## Files

This ADR; `scripts/make_example_projects.py`, `scripts/test_make_example_projects.py`,
`scripts/docker/ubuntu-24.04-rust.Dockerfile`; `crates/rcfd-mcp/tests/example_projects_w48h.rs`,
`crates/rcfd-studio/tests/example_projects_h.rs`; `example-projects-linux/**`; `README.md`.
