# ADR 0814: F-975 registration: why the diagnosis test DG1 of the wheel sector stalls at a different set on Linux, and its remedy

Status: **REGISTERED 2026-11-26, written before any code of F-975 under this registration and before any run of its checks.**
Before this text no library or test code of this registration was written, compiled or run: the files under "What was read"
were read, the log of the CI run below was read, the container image was asked for its tool versions, and nothing else was
done. Results go in the addenda at the end; nothing above them is changed after a run.

This registration answers the brief [docs/adr/briefs/f975-dg1-on-linux.md](briefs/f975-dg1-on-linux.md) and follows
`docs/process/agent-rules.md`, version 5 ([the rules](../process/agent-rules.md)). Written by Claude Opus 5.5. It is a further
registration under F-975 (rule 13). Parent record: [ADR 0806](0806-f975-registration-normwise-deflation-fallback-of-the-state-qr.md)
(its rows DG1 to DG3). Read at the branch's base 3b6cc7cc.

## What is asked

The acceptance of F-975, read from `docs/roadmap.json` with a script: "RotatingSector::whirl_stability of the wheel sector W
of ADR 0595 (shells option, 4 modes, coriolis on) at harmonic indices 0 to 3 and speeds 0, 30 and 60 rad/s returns Ok, with
eigenvalues that agree with whirl_state of the same index and speed within the limit the registration states from the
tolerance table, and the defaults bit-identical." Its status is done (ADR 0806). This registration adds no acceptance clause:
it diagnoses and remedies the failure of a diagnosis test of ADR 0806 on Linux.

The lead's finding (the brief): on GitHub Actions, run 45881197914 (ubuntu-24.04, `cargo test --workspace --release --locked
--lib --bins --no-fail-fast`), the unit test `rotating::whirl::state::f975_diagnosis::dg1_dg2_dg3_the_stalled_window_of_the_wheel_sector`
fails at its DG1 assertion (`f975_diagnosis.rs` line 177). It passes on Windows, where ADR 0806 registered it.

## What was read

- `AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, whole; ADR 0806 whole.
- `crates/rcfd-fem/src/rotating/whirl/state/f975_diagnosis.rs` whole (180 lines).
- `crates/rcfd-fem/src/rotating/whirl/state.rs`: lines 1 to 60 (module doc, declarations, constants), 160 to 460 (`hessenberg`,
  `house`, `reflect`, `francis`, `eig2`, `hessenberg_eigenvalues`, `QrFailure`, `QR_FAILURES`, `record_failure`, `schur_sweep`,
  `real_eigenvalues`), 600 to 680 (`state_solve`). `crates/rcfd-fem/src/rotating/whirl/state/sector.rs` lines 1 to 215
  (`whirl_state`, `whirl_stability`, `whirl_state_at`).
- `crates/rcfd-fem/src/rotating.rs` lines 327 to 340 (`RotatingPart`'s fields), 440 to 480 (`operator`), 1090 to 1095
  (`RotatingSector`'s fields); `crates/rcfd-fem/src/rotating/whirl.rs` lines 715 to 721 (`coriolis`), and by search the
  private `dense_of` (line 660) and `doubled` (line 899).
- `crates/rcfd-fem/src/assemble.rs` lines 1 to 10 and 76 to 120 (the module doc: element matrices on worker threads, scattered
  sequentially in element order, "bit-identical for 1 and for 8 threads"; `worker_count`).
- `crates/rcfd-fem/src/mpc.rs` lines 1036 to 1070 (`harmonic_transformation`: `theta = 2 pi k / N`, `(theta.cos(),
  theta.sin())` at line 1065); `crates/rcfd-fem/src/cyclic.rs` lines 70 to 85 (`rotation_matrix`, `angle.sin()`, `1 -
  angle.cos()` at line 79), 1080 to 1130 (`t.cos()`, `t.sin()` at lines 1090 and 1124), and by search the callers of
  `rotation_matrix` (lines 354 and 407 with `2 pi / N`; 1082, 1140, 1176, 1253, 1281 with `2 pi j / N`).
- By search over `beam.rs`, `mass.rs`, `assemble.rs`, `cyclic.rs`, `mpc.rs`, `model.rs`, `material.rs`, `rotating.rs` and
  `rotating/whirl.rs`: the calls of `sin`, `cos`, `tan`, `exp`, `ln`, `powf`, `powi`, `hypot`, `atan2`, `mul_add`. Besides the
  sine and cosine sites above: `powi(2)` in `cyclic.rs` lines 104 and 344 (the matching of the cyclic boundary), and `hypot`,
  `atan2` in `whirl.rs` lines 444, 453, 574 (functions not on the path of `whirl_state_at`).
- `docs/process/tolerances.md` lines 13 to 24 (the rows "Bit-identity of default output" and "Same discrete problem by two
  routes"); `scripts/process_report.py` sections c and r (what a registration must hold); `scripts/push_main.py` lines 34 and 384
  (a committed mutation patch is found by its suffix `.mutation.patch`).
- The failed log of CI run 45881197914 (`gh run view 45881197914 --log-failed`), the test's printed lines; the Windows log of
  ADR 0806's registered run (`E:/projects/cfd/w49i-work/f975-whirl-stability-wheel-sector-scratch/logs/run_fem.log`, lines 71
  to 95).
- The image `rcfd-linux:ubuntu24.04-rust1.99.0`: rustc 1.99.0, rustfmt 1.10.0, clippy 0.1.99, glibc 2.39 (Ubuntu
  2.39-0ubuntu8.9), 16 processors.

### What the two logs show (before any run of this registration)

| Index, speed | Windows (ADR 0806 run) | Linux (CI run 45881197914) |
|---|---|---|
| (1, 0) | stalled: window 116..=119 of `m = 120`, Frobenius norm of `h0` `3.656689e5`, smallest subdiagonal `1.786e-11` (0.220 eps times the norm, 141 eps times its local sum) | converged in the first iteration (no record); DG3 `5.818e-14` |
| (2, 0) | stalled: window 116..=119, norm `3.567792e5`, smallest subdiagonal `9.381e-12` (0.118 eps times the norm, 129 eps times its local sum) | stalled: window 116..=119, norm `3.567792e5` (equal in the 7 printed digits), smallest subdiagonal `3.088e-10` (3.898 eps times the norm, 7672 eps times its local sum) |
| every other (k, w) | converged | converged |
| DG1, DG2, DG3 | met, met, met | **missed** (stalled at `[(2, 0.0)]` only), met, met |

So the platforms differ in the bits of the input of the QR iteration at (2, 0) (the same norm to 7 digits, a different
subdiagonal at the failure by a factor of 33), and at (1, 0) the first iteration converged on Linux. The fallback of ADR 0806
is not in question on either platform: wherever step 1 stalled, DG2 holds and step 2 converged.

Arithmetic done beforehand. The local test accepts a subdiagonal when it is at most `eps` times its local sum. In the logged
windows the local sums are 180 to 840 while the norm is about `3.6e5`, so the local test needs a subdiagonal of at most about
`1e-3 eps` times the norm, against the 0.1 to 4 `eps` times the norm that the rounding of G2 of ADR 0806 leaves. A subdiagonal
of rounding size that does fall that low in some step of 30 is then a matter of the last bits of the input: the hypothesis H-T
below.

## Diagnosis to be tested: hypotheses and their controls

Rust's `f64` operations `+ - * /` and `sqrt` are IEEE 754 correctly rounded on both targets, no floating-point contraction to
FMA is made without an explicit `mul_add`, and iterator sums run in a fixed order. What can differ between
`x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu` on the path of `whirl_state_at` is (i) the result of a function of the
platform's mathematical library (here `sin` and `cos`; `f64::sin_cos` is `(sin, cos)`), and (ii) an order of summation that
depends on the machine (the thread count of the assembly, `None` meaning the available parallelism, which differs between the
two machines). From these, three hypotheses, each with its control, all run on both platforms:

- **H-L (a library's result).** The two libraries return different bits for `sin` or `cos` at an argument on W's path (the
  test builder's node coordinates `p(phi, r, z)`, `rotation_matrix(n, 2 pi / 6)`, `harmonic_transformation`'s `theta = 2 pi k /
  6`), and the difference propagates to the Hessenberg matrix `h0` of the state matrix. **Control C3**: the test prints, on
  each platform, the bits of `sin` and `cos` at every argument of the set `{a / 4, a / 2, 3 a / 4, a}` with `a = 2 pi / 6`
  computed as the builder computes it, and `{2 pi j / 6, j = 1 .. 5}` computed as the library computes it, with their FNV-1a
  hash L0, and the FNV-1a hashes of the bits of the later stages: L1 the node coordinates of W, L2 the values of `K`, `M` and
  `C` of the part (`part.k`, `part.m`, `part.coriolis()`), L3 the reduced dense `K` and `M` of `whirl_state_at` at (k, 0), L4
  the state matrix `A` and its Hessenberg form `h0`, for k = 1 and 2. **Control C5**: on Windows, a scratch copy of the control
  commit in which every `sin` and `cos` at the sites `mpc.rs:1065`, `cyclic.rs:79`, `cyclic.rs:1090`, `cyclic.rs:1124` and the
  builder's `sin_cos` returns the bits printed by the container for that argument (by a table keyed on the argument's bits;
  an argument not in the table keeps the native result and is counted and printed): its stall set and its hashes L1 to L4 are
  compared with the container's.
- **H-S (an order of summation).** The thread count or a non-deterministic order changes the bits of `K`, `M` or `C`.
  **Control C2**: on each platform, the test builds the sector a second time with the default options and a third time with
  `ModalOptions::threads = Some(1)`, and compares the hashes L2 to L4 and the stall set of the 12 calls of the three builds.
- **H-T (the stall's threshold met near its limit).** Whether the first iteration stalls at a pair index at rest depends on
  whether a rounding-size subdiagonal happens to pass the local test in some step, so it changes under a change of the last
  bits of `h0`. **Control C4**: on each platform, for k = 1 and 2 at 0 rad/s, 16 copies of the platform's `h0`, in copy `j`
  each nonzero entry on or above the subdiagonal moved by one unit in the last place up, down or not at all (bits plus 1, minus
  1, or 0, chosen by the 64-bit linear congruential sequence `x <- x * 6364136223846793005 + 1442695040888963407` from the seed
  `0x9E3779B97F4A7C15 xor (j + 1)`, the choice `(x >> 33) mod 3`, one step per entry in row-major order); `schur_sweep(copy, m,
  None)` is run on each and the stalls counted, `S_k` of 16.
- **Faithful reproduction (control C1).** The controls C3 to C5 read `h0` from a reproduction in the test of the lines of
  `whirl_state_at` (sector.rs lines 159 to 200) and `state_solve` (state.rs lines 600 to 637) at 0 rad/s, followed by the
  private `hessenberg`. C1, on each platform, for k = 1 and 2: `schur_sweep(h0, m, None)` fails if and only if `QR_FAILURES`
  of `whirl_state(k, 0)` holds a record; where it does, the bits of the record's Frobenius norm equal those of the
  reproduction's; and the bits of the eigenvalues of `real_eigenvalues(A)`, sorted as `state_solve` sorts them, equal those of
  `whirl_state(k, 0).state.eigenvalues`.

A code defect would show as C2 missed (the result depends on the thread count or on the build within one process).

## Design

**D1 (the control test).** A new unit test `dg1_controls_library_order_threshold_f975_0814` in
`crates/rcfd-fem/src/rotating/whirl/state/f975_diagnosis.rs`, beside the existing test, which it does not edit (the builder
`wheel()` and the helpers are used unchanged). It prints C1 to C4 and asserts C1 and C2 only (bits within one platform). C3
and C4 are compared across the platforms from the printed lines, by the rules below. The reproduction of D1 uses the private
items of the enclosing modules (`super::hessenberg`, `super::schur_sweep`, `super::real_eigenvalues`, `super::cholesky`,
`super::lower_solve`, `super::transpose`, `super::super::doubled`, `super::super::dense_of`, the fields of `RotatingSector` and
`RotatingPart`), which a child module may read. No library file is changed.

**D2 (run R1).** After the commit of D1: on Windows, `cargo test -p rcfd-fem --release --lib f975_diagnosis -- --show-output`;
in the container, a `git archive` of the same commit, `cargo fetch --locked`, then `cargo test -p rcfd-fem --release --locked
--offline --lib f975_diagnosis -- --show-output`. Each once. It runs the existing test (which prints DG1's stalled set) and the
control test. Then C5 once on Windows (scratch copy, its own target folder), the existing and the control test.

**D3 (the outcomes, fixed now).**

- **O1, a library's result moves the stall.** C1 and C2 met on both platforms, the container's stall set differs from
  `[(1, 0.0), (2, 0.0)]`, and C5 met (with the container's library values, Windows gives the container's stall set and hashes
  L1 to L4, and no argument is missing from the table). The reason is stated as H-L, with H-T as C4 shows it: "near its
  threshold" if `0 < S_k < 16` for k = 1 or 2 on a platform; "not near its threshold" if `S_k` is 0 or 16 for both on both.
  **Remedy R-A**: a test whose stall set is stated per platform from the registered run R1. (1) An amendment, written and
  committed before the change, turns the existing test into the ignored record
  `dg1_dg2_dg3_the_stalled_window_of_the_wheel_sector_known_miss` (rule 2: its body and limits unchanged, `#[ignore]` naming
  this ADR, run R1, and the container's measured set). (2) A new test `dg1_stall_set_per_platform_f975_0814` in the same file:
  the 12 calls and the printed lines of the existing test, DG2 and DG3 with their limits unchanged, and DG1 as: on
  `all(windows, target_env = "msvc", target_arch = "x86_64")` the stall set equals `[(1, 0.0), (2, 0.0)]`; on
  `all(target_os = "linux", target_env = "gnu", target_arch = "x86_64")` it equals the set R1 printed in the container, in its
  order; on every platform the set is not empty and every stall is at k = 1 or 2 at 0 rad/s (G1 of ADR 0806: only there is the
  spectrum double); on a platform not named, the exact set is printed and not compared (no such platform is in the gate). Run
  once on each platform (run R2, below).
- **O2, the library's values do not explain it.** C1 and C2 met on both, C5 missed (another stage or another function differs).
  No remedy in this registration: the origin is not identified. Partial, with the hashes of the first differing stage, and a
  question for the maintainer.
- **O3, the container does not reproduce CI.** The container's stall set is `[(1, 0.0), (2, 0.0)]` and the existing test
  passes there. Not deciding for the remedy: no remedy; partial, with the difference between the CI runner and the container
  named as far as the hashes show it, and a question.
- **O4, a code defect.** C2 missed on a platform: the input of the QR iteration depends on the thread count or on the build.
  The defect is in a library file this registration does not own: no remedy; partial; stop and report, with the stage of the
  first differing hash, and a question.
- **O5, the reproduction is not faithful.** C1 missed on a platform: C3 (L3, L4), C4 and C5 do not decide there. No remedy;
  partial.

The second remedy the brief names, a diagnosis input that stalls on both platforms for the same reason, is not taken in any
outcome: every input of W reaches `sin` and `cos` inside the library (`mpc.rs:1065`, `cyclic.rs:79`), so no input of W has the
same bits on both platforms, and an input built without those functions would not be W. It is listed under "What is not
covered".

**D4 (run R2, under O1).** After the commit of the amendment and of R-A: on Windows, `cargo test -p rcfd-fem --release --lib --
--show-output` (all unit tests of the crate, the new test among them; T1, B3); in the container, a `git archive` of the same
commit, `cargo fetch --locked`, `cargo test -p rcfd-fem --release --locked --offline --lib -- --show-output` (T2, B4). Each
once. The whole-workspace command of the record-run recipe is not used: rule 7 forbids the whole workspace suite.

**Files changed**: `crates/rcfd-fem/src/rotating/whirl/state/f975_diagnosis.rs` (new functions; under O1 the rename and
`#[ignore]` of the existing test by the amendment), the committed mutation patch beside it, this ADR, and
`docs/roadmap/proposals/0814-F-975.json`. No library file, no default, no golden, no hash table, `H_W` and `baseline_hashes.rs`
unchanged.

## Checks

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| R1-W | R1 on Windows: the existing test `dg1_dg2_dg3_the_stalled_window_of_the_wheel_sector` | passes; DG1's set printed `[(1, 0.0), (2, 0.0)]` | ADR 0806 row DG1 and its addendum 1 | passes | 1 | code verification |
| R1-L | R1 in the container: the same test | its stalled set printed; compared with the CI set `[(2, 0.0)]` (equal: the container reproduces CI; equal to Windows: O3) | the brief, task 1; CI run 45881197914 | fails at DG1 with `[(2, 0.0)]` | 1 | code verification |
| C1 | Faithful reproduction, each platform, k = 1, 2 at 0 rad/s, as stated above | equal outcome, equal bits | tolerance table, "Bit-identity of default output" (the same function on the same input) | equal | 1 | code verification |
| C2 | Order of summation, each platform: hashes L2 to L4 and the stall set of the 12 calls, default build, second default build, build with threads 1 | equal | `assemble.rs` lines 5 to 6 (bit-identical for any thread count); tolerance table, "Bit-identity of default output" | equal | 1 | unchanged default output |
| C3 | A library's result: L0 on Windows against L0 in the container, then L1 to L4 | decides H-L: L0 differs and a later stage differs, H-L holds; L0 equal, H-L refuted | the brief, task 1 (hypothesis of a library's result); tolerance table, "Bit-identity of default output" | L0 differs | 1 | code verification |
| C4 | The threshold: `S_k` of 16 perturbed copies, k = 1, 2, each platform | decides H-T: `0 < S_k < 16` for a k on a platform, near its threshold there | the brief, task 1 (hypothesis of the threshold met near its limit) | `0 < S_k < 16` | 1 | code verification |
| C5 | Windows with the container's library values (scratch copy) | the stall set and L1 to L4 equal to the container's; 0 arguments missing from the table | the brief, task 1 (each hypothesis with its control); tolerance table, "Bit-identity of default output" | equal | 1 | code verification |
| T1 | R2 on Windows: `dg1_stall_set_per_platform_f975_0814` (under O1) | passes | the brief's decision rule; DG2 and DG3 limits of ADR 0806 | passes | 1 | code verification |
| T2 | R2 in the container: the same test (under O1) | passes | the brief's decision rule; DG2 and DG3 limits of ADR 0806 | passes | 1 | code verification |
| B1 | `cargo fmt -p rcfd-fem --check` on Windows; if it stops with os error 206 (as in ADR 0806), its output is shown and the same command runs once in the container on the R2 copy | exit 0 | rule 7 | 0 | 1 | unchanged default output |
| B2 | `cargo clippy -p rcfd-fem --all-targets -- -D warnings` on Windows | exit 0 | rule 7 | 0 | 1 | unchanged default output |
| B3 | `cargo test -p rcfd-fem --release --lib` on Windows (R2) | 0 failures | rule 7 | 0 | 1 | unchanged default output |
| B4 | `cargo test -p rcfd-fem --release --locked --offline --lib` in the container (R2) | 0 failures | rule 7; the brief, task 4 | 0 | 1 | unchanged default output |
| B5 | The named test files on Windows: `sa_whirl_stability_wheel_sector_f975`, `sa_beam_opt_ins_default_f672_bits` (the QR fallback's path) | 0 failures | rule 7; tolerance table, "Bit-identity of default output" | 0 | 1 | unchanged default output |
| B6 | `cargo test -p rcfd-api --release --test baseline_hashes` on Windows | pass | rule 3 | pass | 1 | unchanged default output |

Under O2 to O5, T1 and T2 are not run (no remedy) and are recorded as not run. B1 to B6 are run in every outcome, B3 and B4 on
the last commit of the code. No row is of kind validation: no measured case is claimed.

### Negative controls and planned mutations

Negative controls: C2 (the result does not depend on the thread count), C1 (the reproduction's outcome equals the library's),
and in T1 and T2 the assertion that no stall is at a single index or at 30 or 60 rad/s.

Mutations, under O1, each on its own `git archive` copy of the commit of R-A with its own target folder under the scratch
folder, `cargo test -p rcfd-fem --release --lib f975_diagnosis::dg1_stall_set_per_platform` run once on Windows:

| Id | Mutation | Must be caught by |
|---|---|---|
| M1 (committed, rule 16) | step 1 of `hessenberg_eigenvalues` runs with the widened test (`Some(tau)` computed first; X3 of ADR 0806): no call stalls. Patch `crates/rcfd-fem/src/rotating/whirl/state/f975_diagnosis.mutation.patch`, expect-fail `cargo test -p rcfd-fem --release --lib f975_diagnosis::dg1_stall_set_per_platform` | T1 (the set is empty) |
| M2 | the two platform sets exchanged in the new test | T1 (the Windows set is not the Linux one) |
| M3 | the fallback runs with `tau = 0` (X2 of ADR 0806): it fails where step 1 stalled, so `whirl_state(1, 0)` and `(2, 0)` return `Err` | T1 (DG3 not computed) |

## Checks of rule 7, and how they are run

All in the release profile. Windows: `CARGO_TARGET_DIR=E:/projects/cfd/w49i-work/f975-dg1-on-linux-scratch/target`,
`CARGO_BUILD_JOBS=8`. Container: one container of the image, started once and removed at the end, the archive extracted
inside it, its own target folder inside it, `CARGO_BUILD_JOBS=8`. Logs in `E:/projects/cfd/w49i-work/f975-dg1-on-linux-scratch/logs`.
After R2: B1, B2, B5, B6 on Windows, then `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py
check` and `python scripts/process_report.py --check`. `npm test` is not run: `apps/studio-web` is not touched.

## Size

Size (agent): done; 0; 2; 0; 130000

(Two heavy runs: the two container sessions, R1 and R2, with a fresh build of `rcfd-fem` in the first. No question expected.)

## Decision rule

- **done**: O1, and T1 and T2 pass in their one run, each of M1 to M3 is caught, and B1 to B6 hold.
- **partial**: otherwise: O2 to O5 (no remedy), or T1, T2 or a row B missed, or a mutation escapes.

## What is not covered

- A diagnosis input with the same bits on both platforms (D3): not built.
- Platforms other than Windows MSVC x86_64 and Linux GNU x86_64: the exact stall set is printed there, not compared.
- A version of the C library or of the Windows runtime other than those of R1: the per-platform set is a record of the two
  environments of R1 (glibc 2.39 of the image; the Windows runtime of this machine). A change of either may change the set;
  the test would then fail and name it.
- The other tests of ADR 0806 on Linux (`sa_whirl_stability_wheel_sector_f975`): not run in the container by this
  registration (B5 runs on Windows).

## Amendment 1 (2026-11-26, after run R1 and C5, before the remedy): the known miss of DG1 on Linux, and remedy R-A

Run R1 (commit 7e6526a0) and C5 gave outcome O1 of D3: C1 and C2 met on both platforms, the container's stall set
`[(2, 0.0)]` (equal to CI run 45881197914, not equal to Windows), and C5 met. The numbers are in addendum 1. By D3, O1:

1. The existing test `dg1_dg2_dg3_the_stalled_window_of_the_wheel_sector` of ADR 0806 missed its DG1 on Linux in R1 (stalled
   at `[(2, 0.0)]`, against the limit "at (1, 0) and (2, 0)"). It becomes the ignored record
   `dg1_dg2_dg3_the_stalled_window_of_the_wheel_sector_known_miss` (rule 2): its body and limits unchanged, the name and an
   `#[ignore = "known miss of ADR 0806 DG1 on Linux (ADR 0814 run R1, 7e6526a0): stalled at [(2, 0.0)] in the container rcfd-linux:ubuntu24.04-rust1.99.0, glibc sin one ulp off MSVC's at pi / 4 and 4 pi / 3; superseded by dg1_stall_set_per_platform_f975_0814"]`
   attribute the only changes.
2. The new test `dg1_stall_set_per_platform_f975_0814` of D3 is added with the Linux set `[(2, 0.0)]` printed by R1, and
   the Windows set `[(1, 0.0), (2, 0.0)]`. Run R2 of D4 follows, once on each platform. No check, limit or rule above changes.

## Addendum 1: results (2026-11-26)

Commits: the registration 8c492a9d (alone); the controls 7e6526a0; amendment 1 6ae57d3c (alone); remedy R-A 84da16d2; the
committed mutation M1 f6ffd657. Run R1 was made once on 7e6526a0 on each platform, C5 once, and run R2 once on f6ffd657 on
each platform, all in release. Container: one container of `rcfd-linux:ubuntu24.04-rust1.99.0` (glibc 2.39), the `git
archive` of the commit extracted inside it, `cargo fetch --locked`, then `cargo test -p rcfd-fem --release --locked
--offline --lib ...`; removed at the end. Logs in `E:/projects/cfd/w49i-work/f975-dg1-on-linux-scratch/logs` (`r1_windows.log`,
`r1_linux.log`, `c5_windows.log`, `r2_windows.log`, `r2_linux.log`, `mut_m1.log` to `mut_m3.log`, `fmt.log`, `fmt_linux.log`,
`clippy.log`, `named.log`, `baseline.log`).

| # | Limit | Measured | Met |
|---|---|---|---|
| R1-W | passes, set `[(1, 0.0), (2, 0.0)]` | passes; stalled at `[(1, 0.0), (2, 0.0)]`, the DG2 and DG3 numbers of ADR 0806 addendum 1 bit for bit in the 7 printed digits | met |
| R1-L | the set printed, compared with CI `[(2, 0.0)]` | fails at DG1 (line 177); stalled at `[(2, 0.0)]`; (2, 0): smallest subdiagonal `3.087789e-10`, norm `3.567792e5`, as CI. Equal to CI, not to Windows | met (the container reproduces CI) |
| C1 | equal outcome and bits | Windows: k = 1 stalls, k = 2 stalls; Linux: k = 1 converges, k = 2 stalls; on both the reproduction's outcome equals the library's record, the Frobenius norm bits equal, 120 of 120 eigenvalue bits equal | met on both |
| C2 | equal | on both, L2 to L4 and the stall set equal for the second build and for threads 1 (Windows `[(1, 0.0), (2, 0.0)]` three times, Linux `[(2, 0.0)]` three times) | met on both |
| C3 | L0 differs: H-L holds | L0 Windows `0x04708953fe90a592`, Linux `0xd83d012e42f1ccb6`. Of the 10 arguments, two differ, each by one unit in the last place of `sin`: at `pi / 4` (`0x3fe921fb54442d18`, the builder's `3 a / 4`) MSVC `0x3fe6a09e667f3bcd`, glibc `0x3fe6a09e667f3bcc`; at `4 pi / 3` (`0x4010c152382d7365`) MSVC `0xbfebb67ae8584ca9`, glibc `0xbfebb67ae8584ca8`. Every `cos` equal. L1 to L4 all differ (L1 `0xa153986af17d2930` against `0xfe76db583c8f0488`, ...) | H-L holds |
| C4 | `0 < S_k < 16` for a k: H-T holds | Windows `S_1 = 7`, `S_2 = 2`; Linux `S_1 = 2`, `S_2 = 8` (patterns in the logs) | H-T holds on both (near the threshold) |
| C5 | stall set and L1 to L4 equal to the container's; 0 missing | stalled at `[(2, 0.0)]`; L1 `0xfe76db583c8f0488`, L2 `0x5a858051d7a3a0db`, L3 and L4 of k = 1, 2 equal to the container's; C4 patterns equal to the container's; 0 arguments missing (8 distinct arguments in the table) | met |
| T1 | passes | passes: stalled at `[(1, 0.0), (2, 0.0)]` as expected for Windows MSVC; DG2, DG3 met | met |
| T2 | passes | passes: stalled at `[(2, 0.0)]` as expected for Linux GNU; DG2, DG3 met (DG3 `5.818e-14`, `8.589e-13`) | met |
| B1 | exit 0 | Windows: "The filename or extension is too long. (os error 206)", exit 1, as foreseen; the registered replacement, `cargo fmt -p rcfd-fem --check` in the container on the R2 copy: exit 0 | met (by its registered replacement) |
| B2 | exit 0 | exit 0 | met |
| B3 | 0 failures | 17 passed, 0 failed, 1 ignored (the known miss of amendment 1) | met |
| B4 | 0 failures | 17 passed, 0 failed, 1 ignored | met |
| B5 | 0 failures | `sa_beam_opt_ins_default_f672_bits` 1 passed; `sa_whirl_stability_wheel_sector_f975` 4 passed, 1 ignored (as on main) | met |
| B6 | pass | 3 passed, 1 ignored (as on main) | met |

**Diagnosis.** Outcome O1. The input of the QR iteration differs between the platforms because the C libraries return
different `sin` at two arguments on W's path (`pi / 4` in the test builder's node coordinates, `4 pi / 3` in the library),
each by one unit in the last place; with the container's values substituted, Windows reproduces the container's matrices
bit for bit and its stall set (C5), so nothing else differs. The stall of the first iteration at a pair index at rest is a
property of the last bits of `h0`, not of W: one-ulp perturbations stall it in 2 to 8 of 16 copies (C4), on both
platforms. G1 to G3 of ADR 0806 stand (every stall is at a pair index at rest, DG2 and DG3 hold everywhere); what ADR
0806's DG1 asserted beyond them, that both pair indices stall, held on Windows by chance of the rounding. Not a code
defect: the thread count and a rebuild change no bit (C2).

**Mutations** (each on its own `git archive` copy of f6ffd657, own target folder, the new test run once on Windows):

| Id | Caught by |
|---|---|
| M1 (committed) | T1: stalled at `[]`, DG1 and DG2 missed |
| M2 | T1: stalled at `[(1, 0.0), (2, 0.0)]` against the exchanged `[(2, 0.0)]`, DG1 missed |
| M3 | T1: `whirl_state(1, 0)` and `(2, 0)` return the solver's error, DG3 not computed; DG2 missed (no subdiagonal at most `tau = 0`) |

All three caught.

**Deviations.** (1) B1 on Windows stopped with os error 206 and ran by its registered replacement in the container. (2) In
C4 the sequence of the generator advances once per moved entry (nonzero, on or above the subdiagonal), the reading taken of
"one step per entry". (3) The table of C5 held the 8 distinct arguments of the 10 printed (`a / 2` equals `2 a / 4`, and
`2 pi 1 / 6` equals `4 a / 4`). Not run: none of the registered rows was left out; the other tests of ADR 0806 were not run in
the container (as registered).

Size (actual): done; 0; 2; 0; about 140000

**Decision by the registered rule: done.** Outcome O1; T1 and T2 pass in their one run; M1 to M3 are caught; B1 to B6 hold.

### Open questions for the maintainer

None of scope taken or left open by this work.

### For the lead: roadmap entries

Status proposed: done.

> ADR 0814, 2026-11-26, done: DG1 of ADR 0806 failed on Linux because glibc's sin differs from MSVC's by one ulp at pi/4 and 4pi/3 on W's path (with glibc's values Windows reproduces the Linux matrices bit for bit and its stall set), and the stall of a pair index at rest is a property of the last bits (one-ulp perturbations stall it in 2 to 8 of 16 copies on both platforms); thread count changes no bit; DG1 is now stated per platform from a registered run (Windows (1,0),(2,0); Linux (2,0)), the ADR 0806 test held as a known miss; passes on both; 3 of 3 mutations caught.
