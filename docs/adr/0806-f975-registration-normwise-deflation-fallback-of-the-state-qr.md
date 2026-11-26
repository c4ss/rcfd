# ADR 0806: F-975 registration: a fallback of the state QR iteration with a normwise deflation test, so that whirl_stability of the wheel sector converges

Status: **REGISTERED 2026-11-26, written before any code of F-975 and before any run of its checks.** Before this text no
library or test code of this registration was written, compiled or run: the files under "What was read" were read and
nothing else was done. Results go in the addenda at the end; nothing above them is changed after a run.

This registration answers the brief [docs/adr/briefs/f975-whirl-stability-wheel-sector.md](briefs/f975-whirl-stability-wheel-sector.md)
and follows `docs/process/agent-rules.md`, version 5 ([the rules](../process/agent-rules.md)). Written by Claude Opus 5.5.
Parent record: [ADR 0799](0799-f672-registration-whirl-and-full-rotor-of-a-beam-sector-by-default.md) (F-672, its row E-W
missed, P7). Read at main 482348e4.

## What is asked

The acceptance of F-975, read from `docs/roadmap.json` with a script: "RotatingSector::whirl_stability of the wheel sector W
of ADR 0595 (shells option, 4 modes, coriolis on) at harmonic indices 0 to 3 and speeds 0, 30 and 60 rad/s returns Ok, with
eigenvalues that agree with whirl_state of the same index and speed within the limit the registration states from the
tolerance table, and the defaults bit-identical."

On main, `whirl_stability(&[0, 1, 2, 3], &[0.0, 30.0, 60.0])` of W returns `Err(Solver { solver: "gyroscopic_state_eigen",
reason: "the QR iteration did not converge on rows 116..=119 in 30 steps" })` under the three option sets of ADR 0799 alike
(its addendum 1, row E-W).

## What was read

- `AGENTS.md`, `docs/process/agent-rules.md` (version 5), the brief, ADR 0799 whole, `docs/process/tolerances.md` whole.
- `crates/rcfd-fem/tests/sa_beam_opt_ins_default_f672.rs` lines 1 to 336 (the builders of W and of the shell sector, `whirl_texts`,
  `same_texts`, the known miss `e_w_..._known_miss`), and the list of its other functions; `sa_beam_opt_ins_default_f672_bits.rs`
  whole (the records `H_S`, `H_W`, `H_F` and how their texts are made and hashed).
- `crates/rcfd-fem/src/rotating/whirl/state.rs` lines 1 to 700: the module doc, the constants (`QR_ITERATIONS = 30`), `hessenberg`,
  `house`, `reflect`, `francis`, `eig2`, `hessenberg_eigenvalues` (lines 306 to 369), `real_eigenvalues` (371 to 375),
  `null_vector`, `normalise`, `state_residual`, `state_solve` (528 to 602), `gyroscopic_state_eigen_dense`, and
  `RotatingPart::whirl_state`, `whirl_stability`. `crates/rcfd-fem/src/rotating/whirl/state/sector.rs` whole
  (`RotatingSector::whirl_state`, `whirl_stability`, `whirl_state_at`). By search, every caller of `hessenberg_eigenvalues` and
  `real_eigenvalues`: `state_solve`, the unit test at state.rs line 813, and `arnoldi.rs` line 245 (the Hessenberg matrix of the
  Arnoldi process).
- `crates/rcfd-fem/src/rotating/whirl.rs` lines 150 to 200 (`WhirlMode`, `WhirlModes`, `WhirlSpeedModes`) and 880 to 990
  (`RotatingSector::whirl_modes`: at `omega = 0` the modes of `modal_analysis_harmonic`, each eigenvalue once; above, the reduced
  Hermitian problem of ADR 0411 D2, `modes x multiplicity` modes).
- [ADR 0416](0416-f505-registration-gyroscopic-stabilisation.md) D2 and D3 (lines 80 to 130: the state matrix, the deflation test,
  the exceptional shifts, the cap, the stability flag); [ADR 0450](0450-f414-registration-damped-modes-general-eigensolver.md)
  lines 143 to 150 (the deflation test of `rcfd_linalg::eigen_general`, which uses `||H||_F` where the local sum is 0).
- `crates/rcfd-fem/src/error.rs` lines 267 to 271 (`FemError::Solver { solver, reason }`).
- `scripts/process_report.py` sections c, r and s (what a registration must hold).
- By search, the test files of the workspace that call `whirl_state`, `whirl_stability`, `whirl_state_sparse` or
  `gyroscopic_state_eigen_dense` (11 files, listed under B1), and that no test asserts the text "QR iteration" of this solver
  except the ignored known miss of ADR 0799.

Arithmetic done beforehand: W has 7 nodes of 6 unknowns, 42; the hub (node 6) is clamped, 36 remain; node 4 (`high`) depends on
node 0 (`low`) by the cyclic constraint, 30 independent unknowns per sector. A single index (`k = 0`, and `k = 3 = N/2`) has
`nq = 30` and a state matrix of order `m = 2 nq = 60`; a pair index (`k = 1, 2`, doubled) has `nq = 60` and `m = 120`. Rows
116 to 119 are therefore the last four rows of a state matrix of a pair index. `whirl_stability` runs index-major (`k = 0` at
0, 30, 60, then `k = 1` at 0, ...) and stops at the first error; P6 of ADR 0799 (`whirl_state(k, 30.0)`, `k = 0 .. 3`) is `Ok`.
So the first failing call is a pair index, and the first such in the order is `k = 1` at 0 rad/s.

## Diagnosis (from the code)

**G1. At rest a pair index has every eigenvalue twice.** `whirl_state_at` (sector.rs lines 159 to 209) doubles `K_eff`, `M` and
`C` for a pair and reduces them by the harmonic transformation: the real 2 x 2-block form of the complex Hermitian problem of the
harmonic, "whose spectrum is that problem's together with its conjugate" (sector.rs module doc, ADR 0420 D1). The eigenvalues
`lambda_j` of a Hermitian pencil are real, so in the real form each appears twice. At `omega = 0`, `G = 2 omega C = 0`, and the
state matrix `A = [[0, c I], [-K^/c, 0]]` (state.rs lines 557 to 565) has the eigenvalues `+/- i sqrt(lambda_j)`, each twice.
They are semisimple: `K^` is symmetric positive definite, and for each eigenvector `x` of `K^` the vectors `[x; +/- i
sqrt(lambda) x / c]` are eigenvectors of `A`, `2 nq` independent ones. At `omega > 0` the Coriolis matrix splits each double pair
into a forward and a backward whirl (ADR 0411), which is why `k = 1, 2` converge at 30 rad/s.

**G2. An unreduced Hessenberg matrix cannot hold a double semisimple eigenvalue.** From first principles: for any `mu`, the rows
`1 .. m-1` and columns `0 .. m-2` of `H - mu I` form an upper triangular matrix whose diagonal is the subdiagonal of `H`; if no
subdiagonal is 0 its rank is `m - 1`, so every eigenvalue of `H` has geometric multiplicity 1. In exact arithmetic the
Hessenberg form of `A` therefore splits, by zero subdiagonals, into windows each of which holds each double pair at most once.
In floating point those zeros are entries of the size of the rounding of the reduction and of the QR steps, of order `eps
||A||`, not zeros.

**G3. The deflation test is local, and the window stagnates.** A subdiagonal is negligible when `|h_{l,l-1}| <= eps
(|h_{l-1,l-1}| + |h_{l,l}|)` (state.rs line 322; ADR 0416 D2 step 2). Inside a window whose eigenvalues are `+/- i omega` its
diagonal entries are of the order of `omega` (their sum is the sum of the real parts, 0), and for the low pairs of W `omega` is
far below `||A|| ~ c` (the rotations of a beam have little inertia, so `max|K^|`, hence `c`, is large). A subdiagonal of size
`eps ||A||` then fails the test. The double shift of the window cannot reduce it either: the two copies of the pair agree to
within the rounding of the trailing 2 x 2 block, so the shift polynomial is about as small on both and the subdiagonal between
them does not decrease. After 30 steps the solve is refused, with the window's rows. That is the diagnosis to be shown by the
controls DG1 to DG3.

## Design

**D1 (the remedy: a fallback with a normwise deflation test).** `hessenberg_eigenvalues` (state.rs lines 306 to 369) is split
into the iteration itself, `schur_sweep(h, m, widened)`, and a caller:

1. The Hessenberg matrix `h` is copied (`h0`), and `schur_sweep(h, m, None)` runs. It is the present loop unchanged: the same
   deflation test, the same exceptional shifts at the 10th and 20th step of a window, the same cap of 30 steps per window. If it
   converges its eigenvalues are returned.
2. If it fails on a window, `schur_sweep(h0, m, Some(tau))` runs on the copy, with `tau = m eps ||h0||_F` (`eps` the machine
   epsilon, `||.||_F` the Frobenius norm): a subdiagonal is also negligible when `|h_{l,l-1}| <= tau` (the local test is kept,
   the two joined by "or"). Nothing else of the iteration differs.
3. If the fallback also fails, the error is that of the present code, with the fallback's rows.

Derivation of `tau` (first principles, with one result cited from memory). Setting a subdiagonal of size `<= tau` to 0 makes
the iteration's matrix exactly similar to `H + E` with `||E||_F <= tau`, and each further deflation adds the same bound. An
orthogonal similarity keeps the Frobenius norm, so `||h0||_F = ||A||_F` up to rounding. The Householder reduction of an order
`m` matrix and the QR steps already make the computed form exactly similar to `A + E0` with `||E0||_F` of order `m eps
||A||_F` (the backward stability of products of Householder reflections, Golub and Van Loan, *Matrix Computations*, sections 5.1
and 7.5, cited from memory: no copy could be read). So the fallback's deflations add a perturbation of the order the solve
already accepts, and by the Bauer-Fike bound (cited from memory, as above) each semisimple eigenvalue moves by at most
`kappa(V) ||E||`. The repository already uses a normwise scale where the local one fails: the deflation of
`rcfd_linalg::eigen_general` takes `||H||_F` where the local sum is 0 (ADR 0450, lines 143 to 150, read). The factor `m` (and not
1) is taken because the subdiagonals that G2 makes zero in exact arithmetic carry the accumulated rounding of `m - 2`
reflections and of the QR steps, not one rounding.

**D2 (bits).** For a call whose iteration converges today, step 1 performs the same floating-point operations in the same order
(the test `widened.is_some_and(..)` is false without arithmetic, and the copy `h0` is not read), so its eigenvalues, and every
result built on them, keep their bits. Step 2 is reached only by a call that returned the error before. So the remedy is not
an opt-in (the brief: opt-in only if it would change a result that converges today). The one caller besides `state_solve`,
the Arnoldi path (`arnoldi.rs` line 245), gets the fallback in the same way, also only where it failed before.

**D3 (what changes).** A call that was refused with "the QR iteration did not converge" may now return a result. On W that is
P7 of ADR 0799, `whirl_stability`, and `whirl_state(k, w)` at the failing indices and speeds. The record `H_W` of
`sa_beam_opt_ins_default_f672_bits.rs` (W under `shells(true).beams(true)`, P1 to P8) holds P7's error text, so after the
change that test fails on `H_W`; `H_S` and `H_F` hold no such error and stay equal. The record is not mine to change (an
existing test, rule 2 and the brief); its re-recording is question 1. Row B-W shows that every other path of W keeps the bits
of main.

**D4 (the record of the diagnosis).** In state.rs, under `#[cfg(test)]` only, a thread-local record is written when step 1 fails:
`m`, the window `(l, hi)`, for each `i` in `l + 1 ..= hi` the subdiagonal `|h_{i,i-1}|` and the local sum `|h_{i-1,i-1}| +
|h_{i,i}|` at the moment of failure, `||h0||_F`, `tau`, and whether step 2 converged. It is not compiled into the library that
integration tests and users link, so it changes no bit there. It is read by a unit test in a new file
`crates/rcfd-fem/src/rotating/whirl/state/f975_diagnosis.rs` (declared `#[cfg(test)] mod f975_diagnosis;` in state.rs), which
builds W (the builder of `sa_beam_opt_ins_default_f672.rs`, copied, with `crate::` paths) and calls `RotatingSector::whirl_state`
for each index and speed.

**Lines of state.rs changed** (the owned file): the constants block (lines 28 to 35, a comment on the fallback beside
`QR_ITERATIONS`), the module declarations (lines 22 to 26, the one `#[cfg(test)] mod` line), and `hessenberg_eigenvalues` (lines
306 to 369) with the new `schur_sweep` and the diagnosis record next to it. `real_eigenvalues`, `state_solve`, `francis` and the
rest are not changed. No other library file is changed.

**New test file** `crates/rcfd-fem/tests/sa_whirl_stability_wheel_sector_f975.rs`, headed with ADR 0806 and F-975: the builders
of W and of the shell sector, `whirl_texts`, `same_texts` and the hash of the bits file copied from the two F-672 files. Set-up:
`RotatingSector::new(&sector(o), &RotatingOptions::new(4).modal(modal_options(4)).coriolis(true))`, `modal_options(4)` with
`tolerance = 1e-12` and `ResidualTest::Backward`, `o = CyclicOptions::new().shells(true)` (option set (a) of ADR 0799) unless
stated; indices `[0, 1, 2, 3]`, speeds `[0.0, 30.0, 60.0]`.

## Checks

The relative difference of two frequencies is `|a - b| / |b|`. Two results are equal when their `Debug` texts are equal (equal
bits, as ADR 0799 states).

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A1 | Acceptance, "returns Ok": `whirl_stability(&[0, 1, 2, 3], &[0.0, 30.0, 60.0])` of W | `Ok` | the acceptance of F-975 | `Ok` | 1 | code verification |
| A2 | Acceptance, "agree with whirl_state": for each of the 12 (index, speed), `results[h][j]` against `whirl_state(k, w)` | equal texts (so equal eigenvalues) | the acceptance of F-975; tolerance table, "Bit-identity of default output" (the same function on the same input) | equal | 1 | code verification |
| V1 | At 0 rad/s, the reported frequencies of each index against `modal_analysis_harmonic(W, &[], k, modal_options(4))`: for a pair `omegas[2j]` and `omegas[2j+1]` against `omega_j`, for a single index `omegas[j]` against `omega_j`, `j = 0 .. 3` | relative `<= 1e-8` | tolerance table, "Same discrete problem by two routes" | `1e-11` | 1000 | code verification |
| V2 | At 30 and 60 rad/s, the reported frequencies of each index against the modes of `whirl_modes(k, w)` (the Hermitian route of ADR 0411), one to one, ascending | relative `<= 1e-8` | tolerance table, "Same discrete problem by two routes" | `1e-11` | 1000 | code verification |
| V3 | Every (index, speed) flagged `Stable`, `real_defect <= 1e-8` (every speed below the lowest frequency, 135.6 rad/s, so `K_eff` positive definite and every `s` imaginary, ADR 0416 D3) | `real_defect <= 1e-8` | tolerance table, "Same discrete problem by two routes" (the real part of an imaginary eigenvalue computed two ways); ADR 0416 D3 | `1e-13` | 1e5 | code verification |
| V4 | The residual of every reported mode | `<= 1e-10` | tolerance table, "Linear and eigenvalue solver residual" | `1e-12` | 100 | code verification |
| E-W2 | The known miss of ADR 0799 as a further check (a new test with the same set-up, the record untouched): P1 to P8 of W under (a), (b) and (c) | every result `Ok`; the texts of (a), (b), (c) equal | ADR 0799 row E-W (the acceptance of F-672 as registered there) | holds | 1 | code verification |
| DG1 | Diagnosis: in the unit test, step 1 fails at (`k = 1`, 0 rad/s) and (`k = 2`, 0 rad/s), and at no single index (`k = 0, 3`) and at no index at 30 rad/s | as stated | the brief (the diagnosis shown by a registered control); G1, G3 | as stated | 1 | code verification |
| DG2 | Diagnosis: at each failure of DG1, the window's smallest subdiagonal `<= tau`, while every subdiagonal of the window fails the local test | as stated | the brief; G2, G3, D1 | as stated | 1 | code verification |
| DG3 | Diagnosis: at (`k = 1, 2`, 0 rad/s) the spectrum is double: the `2 nq` eigenvalues, ordered by `(Im s, Re s)`, in consecutive pairs with `|Im s_{2i} - Im s_{2i+1}| <= 1e-8 |s_{2i}|` | as stated | tolerance table, "Same discrete problem by two routes" (the cosine and sine halves are one problem); G1 | `1e-12` | 1e4 | code verification |
| B-W | The other paths of W keep main's bits: the text `H_W` of the ADR 0799 bits file computed now, with the text of P7 replaced by main's error text, `Err(Solver { solver: "gyroscopic_state_eigen", reason: "the QR iteration did not converge on rows 116..=119 in 30 steps" })` | its FNV-1a hash equal to the Windows record `0xe314b82213528f82` (on Windows x86_64; elsewhere printed, not compared: main's P7 text on Linux is not recorded in words) | rule 3; tolerance table, "Bit-identity of default output" | equal | 1 | unchanged default output |
| B-F | `sa_beam_opt_ins_default_f672_bits.rs` run once | it fails on `H_W` alone: the printed `H_S` and `H_F` equal to the record of the platform, `H_W` not (D3) | rule 3; tolerance table, "Bit-identity of default output"; D3 | as stated | 1 | unchanged default output |
| B1 | The named existing test files, unedited, but `sa_beam_opt_ins_default_f672_bits` (B-F): `sa_beam_opt_ins_default_f672`, `sa_damped`, `sa_gyro_default_bits`, `sa_gyro_sector_default_bits`, `sa_rotating_cyclic_beams_f675`, `sa_rotating_cyclic_shells`, `sa_rotating_cyclic_shells_default_bits`, `si_rotating_odd_index`, `si_rotating_whirl_sector_state`, `si_rotating_whirl_state`, `si_rotating_whirl_state_refine`, `si_rotating_whirl_state_sparse` | 0 failures | rule 7; tolerance table, "Bit-identity of default output" | 0 | 1 | unchanged default output |
| B2 | `cargo test -p rcfd-fem --release --lib` (holds DG1 to DG3 and the unit tests of state.rs) | 0 failures | rule 7 | 0 | 1 | unchanged default output |
| B3 | `cargo test -p rcfd-api --release --test baseline_hashes` (state.rs is a shared solver file) | pass | rule 3 | pass | 1 | unchanged default output |

The estimates of V1 to V4 and DG3 rest on G1 and D1: an absolute eigenvalue error of order `eps ||A||` (step 1) or `tau` (step
2), against frequencies of 135 rad/s and above. `||A||` of W is not known before the run; it is printed by DG2.

No row is of kind validation: the acceptance claims agreement with `whirl_state` and the registration adds agreement with two
other routes of the same discrete problem, not with a measured case.

### Negative controls and planned mutations

Controls: B-W, B-F's `H_S` and `H_F`, B1 and B3 (the converging calls keep their bits); DG1 (no fallback where the iteration
converges). Mutations, each on a scratch copy of the tree after the change, with its own target folder, the new test file (and
for X3 also the bits file) run once:

| Id | Mutation | Must be caught by |
|---|---|---|
| X1 (committed, rule 16) | the fallback removed: step 1's error is returned. Patch `crates/rcfd-fem/tests/sa_whirl_stability_wheel_sector_f975.mutation.patch`, expect-fail `cargo test -p rcfd-fem --release --test sa_whirl_stability_wheel_sector_f975 a1_a2` | A1, A2, V1 to V4, E-W2 |
| X2 | the fallback runs with `tau = 0` (a repeat of step 1) | A1 |
| X3 | the widened test from the start (step 1 with `Some(tau)`) | B-W or B-F (`H_S`) or B1: converging calls change bits |
| X4 | `tau = 1e-6 ||h0||_F` | V1, V2 or V3 |

## Checks of rule 7, and how they are run

All in the release profile, `CARGO_TARGET_DIR=E:/projects/cfd/w49i-work/f975-whirl-stability-wheel-sector-scratch/target`, output
to files in the scratch folder. The registered run is made once after the change: the new test file, B-F, B1, B2, B3. Then
`cargo fmt -p rcfd-fem --check` (if it stops with "The filename or extension is too long (os error 206)", as in ADR 0799, its
output is shown and `rustfmt --edition 2021 --check` on the changed and added Rust files takes its place), `cargo clippy -p
rcfd-fem --all-targets -- -D warnings`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py
check` and `python scripts/process_report.py --check`. `npm test` is not run: `apps/studio-web` is not touched.

## Size

Size (agent): done; 0; 0; 1; 160000

(No heavy run: the Linux container is not used, B-W compares on Windows only. One question: question 1.)

## Decision rule

- **done**: every row A1 to B3 holds in its one registered run and each of X1 to X4 is caught.
- **partial**: otherwise (a row missed, recorded by rule 2, or not run; or a mutation escapes).
- **todo**: the remedy cannot be made without changing the result of a call that converges today, or without an edit of
  `baseline_hashes.rs`, a hash table or a golden file.

## What is not covered

- At a double eigenvalue the two reported modes of a pair are the same vector (inverse iteration from one start vector, D4 of
  ADR 0416); a basis of the two-dimensional eigenspace is not returned. Their residuals are checked (V4), not their
  independence.
- The fallback on the Arnoldi path: reached by no test of this registration (no failure of it is known).
- Linux: B-W prints there and does not compare; B-F's Linux record is not run (no container run).
- Models other than W and the shell sector; speeds above the loss of stiffness of W.

## Open questions for the maintainer

1. **The record `H_W` of the ADR 0799 bits file.** After the change P7 of W is `Ok`, so `H_W` of
   `sa_beam_opt_ins_default_f672_bits.rs` no longer equals its record (D3), on Windows and on Linux. Options: (a) the lead
   re-records `H_W` on both platforms (the Windows hash printed by B-F, Linux from the lead's run), with B-W of this registration
   as the evidence that only P7 changed; (b) make the fallback an opt-in, so that P7 stays an error by default, which fails
   A1 and the acceptance; (c) remove P7 from `H_W`, an edit of an existing test's set-up. Recommendation: (a). The change turns a
   solver error into a result and changes no result that was returned before (D2, B-W, B1, B3).

## Addendum 1: results (2026-11-26)

The library change, the unit test of the diagnosis and the new test file are commit 6db6fe1e; the committed mutation X1 is
f4145eff; commit 78de7b1b applies rustfmt's layout to the three new or changed Rust files after the registered fmt check
(no token changed: each file is equal to its previous version with all whitespace removed). The registered run was made once
on f4145eff, in the release profile, with the target folder under the scratch folder. Its logs (`logs/run_fem.log`,
`logs/run_baseline.log`, `logs/mut_x1.log` to `mut_x4.log`, `logs/clippy.log`) are in
`E:/projects/cfd/w49i-work/f975-whirl-stability-wheel-sector-scratch`.

| # | Limit | Measured | Met |
|---|---|---|---|
| A1 | `Ok` | `Ok` | met |
| A2 | equal texts | 12 of 12 (index, speed) texts equal to `whirl_state`, eigenvalues equal (60, 120, 120, 60 per index) | met |
| V1 | relative `<= 1e-8` | not measured as registered: the test's reference repeated each eigenvalue of `modal_analysis_harmonic` `multiplicity` times, but for a pair index that function already returns `modes x multiplicity` = 8 values (each frequency twice), so the reference had 16 entries against 8 and the test gave `inf` for `k = 1, 2`; `k = 0`: `3.412e-12`, `k = 3`: `1.746e-13` | **missed** (defective set-up) |
| V2 | relative `<= 1e-8` | largest `6.055e-12` (`k = 0`, 60 rad/s), over the 8 (index, speed) pairs at 30 and 60 rad/s | met |
| V3 | `Stable`, `real_defect <= 1e-8` | all 12 `Stable`; largest `real_defect` `7.401e-15` | met |
| V4 | `<= 1e-10` | largest residual `4.701e-12` | met |
| E-W2 | every result `Ok`; texts of (a), (b), (c) equal | 17 of 17 paths `Ok` and equal | met |
| DG1 | stalls at (1, 0) and (2, 0), none at `k = 0, 3`, none at 30 rad/s | stalled first iterations at exactly (1, 0) and (2, 0); the fallback converged at both | met |
| DG2 | smallest subdiagonal `<= tau`, every one refused by the local test | (1, 0): window 116..=119 of `m = 120`, `norm(h0)_F = 3.657e5`, `tau = 9.74e-9`; subdiagonals `18.8`, `1.79e-11` (0.22 `eps norm(h0)_F`), `82.1`; the small one is 141 `eps` times its local sum `570`. (2, 0): `norm(h0)_F = 3.568e5`, `tau = 9.51e-9`; `80.5`, `9.38e-12` (0.12 `eps norm(h0)_F`, 129 `eps` times its local sum `328`), `237` | met |
| DG3 | consecutive pairs within `1e-8 abs(s)` | (1, 0): `6.351e-13`; (2, 0): `1.199e-12` (120 eigenvalues each) | met |
| B-W | hash equal to `0xe314b82213528f82` | `0xe314b82213528f82` | met |
| B-F | fails on `H_W` alone | fails; printed `[H_S, H_W, H_F] = [0xd135e8503d05e054, 0x9eee16dea5faf6a2, 0x21c219cfc1e34528]` against the record `[0xd135e8503d05e054, 0xe314b82213528f82, 0x21c219cfc1e34528]`: `H_S`, `H_F` equal, `H_W` not | met |
| B1 | 0 failures | 12 files: 49 passed, 0 failed, 8 ignored (all ignored on main) | met |
| B2 | 0 failures | 16 passed, 0 failed (with DG1 to DG3) | met |
| B3 | pass | 3 passed, 1 ignored (as on main) | met |

**Diagnosis confirmed.** The QR iteration stalls exactly where G1 says the spectrum is double (the pair indices at rest). The
window 116..=119 holds two copies of one pair split by a subdiagonal of 0.1 to 0.2 `eps norm(h0)_F`, the rounding of G2. The
local test refuses it by a factor of about 130 to 140, because the window's diagonal is of the order of its eigenvalues
(G3). The fallback's test accepts it, and the iteration then converges. The frequencies agree with the Hermitian route to
`6e-12` and with the modal route to `3e-12` where V1 was measured.

**The V1 miss.** It is a defect of the test, not of the solver. The registration (V1, "for a pair `omegas[2j]` and
`omegas[2j+1]` against `omega_j`, `j = 0 .. 3`") assumed that `modal_analysis_harmonic` returns 4 values for a pair. It
returns 8, each twice, as the printed reference shows (`153.3755669403876` twice, then `153.37556694040723` twice, ...). The
printed values agree with the eight state frequencies to about `4e-12` relative (for example `153.3755669398265` against
`153.3755669403876`), but that comparison is not the registered row and is not counted as met. The limit is kept. The row was
not corrected or run again: a corrected set-up needs an amendment before a run, which belongs to a further registration
(question 2).

**Mutations** (each on its own `git archive` copy of f4145eff with its own target folder, the new test file run once; for X3
also the bits file). V1 fails without a mutation, so it is counted for none of them.

| Id | Caught by |
|---|---|
| X1 (committed) | A1, A2 (`a1_a2_..` failed: `Err` of the solver), E-W2 (P7 not `Ok`), `v1_v4_..` (its `expect` on A1) |
| X2 | A1, A2, E-W2, as X1 |
| X3 | B-W (hash not equal to the record) and B-F (`H_S` changed to `0x8e668c7ef57f1d22`); V2 to V4 still met |
| X4 | V4 (largest residual `5.136e-7` against `1e-10`); V2 (`6.055e-12`) and V3 (`2.149e-9`) met |

All four were caught. The committed patch names `a1_a2` in its expect-fail line. That test passes without X1 and fails with
it, which is not true of the whole file (V1 fails anyway).

**Rule 7.** `cargo fmt -p rcfd-fem --check` stopped with "The filename or extension is too long. (os error 206)", as foreseen.
Its registered replacement, `rustfmt --edition 2021 --check` on the three changed or added Rust files, **failed** (exit 1),
on line breaks only. The layout was applied afterwards (commit 78de7b1b, no token changed), and the check was not run again.
`cargo clippy -p rcfd-fem --all-targets -- -D warnings` passed (exit 0), on 78de7b1b. `docs_site`, `roadmap_proposals.py
check` and `process_report.py --check` were run after this addendum; their results are in the report.

**Deviations.** (1) The fmt layout commit after the registered run (above). (2) V1 as stated. Nothing registered was left out.
Not run: the Linux records (none registered).

Size (actual): partial; 1; 0; 2; about 200000

(One further registration expected: V1 with a corrected set-up. Two questions: 1 and 2 below.)

**Decision by the registered rule: partial.** V1 is missed (defective set-up), and the fmt check failed in its one run. Every
other row is met, all four mutations are caught, and the calls that converged before keep their bits (B-W, B-F's `H_S` and
`H_F`, B1, B3).

### Open questions for the maintainer

1. As registered (the record `H_W` of the ADR 0799 bits file). Recommendation: (a), the lead re-records `H_W` (Windows
   `0x9eee16dea5faf6a2` from B-F; Linux from the lead's run). B-W shows that P7 is the only path of W whose bits changed.
2. **How the V1 miss is held.** Options: (a) the form of rule 2: the lead renames
   `v1_v4_frequencies_flags_and_residuals_against_the_other_routes` to `..._known_miss` with
   `#[ignore = "known miss of ADR 0806 run 1 (f4145eff): V1 reference doubled twice for a pair index (16 against 8 values); V2 to V4 met"]`,
   the body unchanged, and a further registration under F-975 (rule 13) registers V1 with the reference taken as returned (8
   values for a pair) and runs it once; (b) keep the test failing until that registration. Recommendation: (a). The other
   tests of the file stay in the gate.

### For the lead: roadmap entries

Status proposed: partial.

> ADR 0806, 2026-11-26, partial: the state QR iteration stalled on W because at rest every eigenvalue of a pair index is double and the local deflation test refuses the rounding-size subdiagonal between the two copies (shown: 0.1 to 0.2 eps times the Frobenius norm, refused by a factor of about 135); a fallback from a copy of the Hessenberg matrix with the normwise test m eps times the Frobenius norm makes whirl_stability of W Ok at k = 0..3 and 0, 30, 60 rad/s, equal to whirl_state, frequencies within 6.1e-12 of whirl_modes, all stable, residuals 4.7e-12; converging calls keep their bits (W's other paths hash to main's record, H_S, H_F, baseline_hashes); 4 of 4 mutations caught; V1 missed on a defective set-up; fmt failed on layout, then applied.

> whirl_stability of the wheel sector converges and agrees with whirl_state and whirl_modes. Missing: V1 of ADR 0806 (the rest frequencies of W against modal_analysis_harmonic) is a miss of a defective set-up (the reference doubled twice for a pair index), to be registered again with the reference as returned; the H_W record of sa_beam_opt_ins_default_f672_bits.rs is to be re-recorded (P7 now Ok).

## Amendment 1 (2026-11-26, after addendum 1): the cells of row DG3

`python scripts/process_report.py --check` reads the kind of row DG3 of the table of checks as "as stated". The row's check
and limit hold absolute-value bars inside code spans, which the table reader takes as cell separators, so the cells after
them are shifted. As registered, the row DG3 is: check "at (k = 1, 2, 0 rad/s) the spectrum is double: the 2 nq eigenvalues,
ordered by (Im s, Re s), in consecutive pairs with abs(Im s_2i - Im s_2i+1) <= 1e-8 abs(s_2i)"; limit "as stated"; source
"tolerance table, Same discrete problem by two routes; G1"; estimate 1e-12; margin 1e4; kind **code verification**. No check,
limit or result changes. The registration cannot be edited, and the form "| Id | Kind |" applies only to a table without a
column "kind", so the lead is asked to list the line
`0806-f975-registration-normwise-deflation-fallback-of-the-state-qr.md: DG3` in `docs/process/exceptions/registration-kind.txt`.
