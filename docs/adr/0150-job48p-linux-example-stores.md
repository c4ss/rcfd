# ADR 0150: Job 48P, Linux-built example stores: a second tree `example-projects-linux/`, built by the same script in Docker

Status: **registered 2026-10-28**, before any registered run (the code, the tests and the Docker image were written,
built and compiled first; what ran before is listed under "What I already knew"). Job 48P, on local main 6867e40 (waves
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

## Files

This ADR; `scripts/make_example_projects.py`, `scripts/test_make_example_projects.py`,
`scripts/docker/ubuntu-24.04-rust.Dockerfile`; `crates/rcfd-mcp/tests/example_projects_w48h.rs`,
`crates/rcfd-studio/tests/example_projects_h.rs`; `example-projects-linux/**`; `README.md`.
