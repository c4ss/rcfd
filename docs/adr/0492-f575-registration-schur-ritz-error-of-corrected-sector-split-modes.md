# ADR 0492: F-575 registration: the second-order Ritz error of the corrected sector split modes (the Schur-complemented projected problem)

Status: **REGISTERED 2026-11-14, written before any code of F-575 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-575 was written in the repository. Two runs of one scratch probe program on the
existing API of 12fce5c9 were made in the scratch folder (`w49i-work/f575-scratch/zz_probe_f575.rs`, logs `probe1.log`,
`probe2.log`), as the lead's brief asks (probe the estimate on the same disc sector before registering the 10 percent limit);
they are disclosed in full under "Arithmetic before this registration". Results go in the addendum at the end; nothing above the
addendum is changed after a run.

Roadmap item F-575 (deps F-569), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0482](0482-f569-registration-static-correction-vectors-for-sector-split-modes.md) (remaining
question 1, option b). Number 0492 given by the lead. Branch `f575-work` from 12fce5c9, worktree `w49i-work/f575`, scratch
`w49i-work/f575-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical and the results of ADR 0481
and 0482 bit-identical (the new value is a new field, computed only where ADR 0482 computes `eta`); no edit of
`baseline_hashes.rs`, of the hash tables or of any golden file; no existing test edited; no new dependency, no unsafe code;
clean-room. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation); none changes a default result, a golden or a hash table.

## What F-575 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. RotatingSector::split_modes with split_correction reports beside the eta of ADR 0482 the Ritz error of the Schur-complemented projected problem (the coupling of the complement with every basis vector: one complement solve per basis vector and one more dense quadratic solve); on the disc sector of ADR 0411 with 4 harmonic modes per index at 0.4 and 0.8 omega_ref it agrees with the measured error of every matched mode within 10 percent where it exceeds 1e-9, and the results of ADR 0482 keep their bits.

Clauses: **a1** `RotatingSector::split_modes` with `split_correction` reports, beside ADR 0482's `eta`, the Ritz error of the
Schur-complemented projected problem, computed by one complement solve per basis vector and one more dense quadratic solve;
**a2** on DS of ADR 0411 with B4 at `0.4` and `0.8 omega_ref` it agrees with the measured error of every matched mode within 10
percent where it exceeds 1e-9; **a3** the results of ADR 0482 keep their bits. The brief of the lead adds: derive the estimate
and probe it before registering the 10 percent; say why it should hold; register checks that catch the mutations of the new
estimate matching MU3 and MU4 of ADR 0482 (which survived there); a default-bits test at the base before any library change;
`baseline_hashes` unchanged; probe before any 1e-12-class limit; every aggregated test error must be finite (an `f64::max` over
NaN hides a failure).

## What was read (12fce5c9, read only)

ADR 0482 whole (D1, D2, the set-ups, the addendum: E2 missed with `err/eta` in `[1.0002, 2.6212]`, the diagnosis by the Schur
complement, MU3 and MU4 not caught), ADR 0481 whole (D1, D5, the matching rule, the record `err/eta` in `[0.968, 1.013]` on bases
of eigenvectors), ADR 0411 for DS; the diagnostic `f569-scratch/zz_diag_f569.rs` and its log `diag1.log`; the source of
`gyro.rs` (whole: `split_modes_dense`, `project_and_solve`, `sector_split_at`, `split_campbell`, `CorrectionBasis`,
`correction_basis`, `ritz_error`, `minres`), `cyclic.rs` (`HarmonicModes`), `mpc.rs` (`Transformation::{independent, expand,
transpose_times, reduce}`), `rotating.rs` (`RotatingOptions::split_correction`); the tests `sa_gyro_sector_correction.rs` and
`sa_gyro_sector_correction_default_bits.rs` (set-ups, matching, the digest pattern).

Published sources, cited from memory, **not read** for this work: the Schur complement (block elimination) of a partitioned
linear system, textbook linear algebra (for example G. H. Golub and C. F. Van Loan, Matrix Computations, Johns Hopkins, any
edition); the eigenvalue-dependent condensation of a partitioned eigenproblem (exact condensation, frozen at a shift) as in
dynamic substructuring, for example R. J. Guyan, AIAA J. 3 (1965) 380 for its static limit, from memory; G. L. G. Sleijpen and H.
A. van der Vorst, SIAM J. Matrix Anal. Appl. 17 (1996) 401 (the projected correction equation, ADR 0482 D2); C. C. Paige and M.
A. Saunders, SIAM J. Numer. Anal. 12 (1975) 617 (MINRES, written out in `gyro.rs` by ADR 0482). Nothing numeric is taken from
them; every formula used is derived below.

## Design

Notation of ADR 0482: index `k` of multiplicity `mu`, reduced coordinates of order `n_q`, `K_r` (the reduced `K_eff` at the
speed; `kq` of `ritz_error`), `M_r`, `C_r` (doubled for a pair), `G_r = 2 Omega C_r`, `Q(w) = K_r - w^2 M_r + i w G_r`
(Hermitian); the enlarged basis `V = [v_1 .. v_nb]` (`n_b = m + p`, the modes then the kept correction vectors, real,
`M_r`-orthonormal; `CorrectionBasis::v`), `P = V V^T M_r`, the complement `W = range(I - P)`; the projected matrices
`K_VV`, `M_VV`, `C_VV` of `project_and_solve` (ADR 0481 D1: projected on the expanded basis with the doubled matrices,
symmetrised); a split mode `(theta, y)` of `split_modes_dense` on them (`y^H M_VV y = 1`).

### D1. The exact condensed problem (derivation)

Let `(omega, phi)` be an exact whirl mode of the index, `Q(omega) phi = 0`, and split `phi = V y + z`, `z` in `W` (unique, since
`P` is a projector). Testing with the basis and with the complement (`(I - P)^T`, which annihilates `M_r V`):

`V^T Q(omega) (V y + z) = 0`, `(I - P)^T Q(omega) (V y + z) = 0`.

The second, on `W`, is `A_c(omega) z = -(I - P)^T Q(omega) V y`, `A_c(w) = (I - P)^T Q(w) (I - P)`; when `A_c(omega)` is
nonsingular on `W` (`omega` is not an eigenvalue of the problem compressed to the complement), `z = sum_j y_j x_j(omega)` with

`x_j(w)` in `W`: `(I - P)^T Q(w) (I - P) x_j = -(I - P)^T Q(w) v_j` (the **complement solve of basis vector `j`**).

Substituted in the first: `S(omega) y = 0` with the condensed (Schur-complemented) projected matrix

`S_ij(w) = v_i^T Q(w) (v_j + x_j(w)) = [Q_VV(w) - E(w)]_ij`, `E_ij(w) = -v_i^T Q(w) x_j(w)` (`E = Q_VW Q_WW^-1 Q_WV`).

This is exact: `omega` is an eigenvalue of the nonlinear projected problem `S`. `E` is Hermitian (`E_ij = (v_i + x_i)^H Q (v_j +
x_j) - Q_VV,ij` since `(I - P)^T Q (v_j + x_j) = 0`), so `E = E_R + i E_I`, `E_R` symmetric, `E_I` skew. ADR 0482's `eta` is the
first-order shift of `S` in `E` along the single Ritz vector `y` (`y^H E y / y^H Q'_VV y`); the measured error also carries the
coupling of `E` with the other Ritz vectors of the basis, `sum_b |y_b^H E y|^2/(...)`, which on a non-invariant basis (modes
plus correction vectors: `V^T K_r W != 0`) is of first order in the coupling and is what ADR 0482's E2 missed (its addendum).

### D2. The reported estimate (a1)

For a split mode `(theta, y)` the condensed matrix is **frozen at the Ritz value**, `S(l) ~ Q_VV(l) - E(theta)`, and its
eigenvalue near `theta` is found by **one more dense quadratic solve**. Since `split_modes_dense` takes the gyroscopic form
`K - l^2 M + i l G`, the skew part is folded into the gyroscopic matrix at `l = theta`:

`K' = K_VV - sym(E_R)`, `C' = C_VV - skew(E_I) / (2 Omega theta)` (`sym(A) = (A + A^T)/2`, `skew(A) = (A - A^T)/2`; at `Omega
= 0`, where `G_r = 0` makes `E_I = 0` exactly, `C' = C_VV`),

`split_modes_dense(M_VV, C', Omega, K')`, and the corrected root `theta'` is the split mode of that solve with the largest
overlap `|y^H M_VV y'|^2` (ties: the lower root). The reported value is

`schur_eta = |theta / theta' - 1|`

(the relative error of `theta` against the condensed root, the form of the measured error `|theta/omega - 1|`). When the
complement is empty (`n_b = n_q`) `schur_eta = Some(0.0)` without a solve (as ADR 0482's `eta`). The fold and the freezing change
`theta'` only at second order in `theta' - theta` (D4).

**Complement solves.** For each basis vector `v_j` the system of D1 at `w = theta` in the real form of length `2 n_q` (`[[A,
-theta G_r], [theta G_r, A]]`, `A = K_r - theta^2 M_r`), the right side `-(I - P)^T Q(theta) v_j` (`v_j` real: `(-(I - P)^T A
v_j, -(I - P)^T theta G_r v_j)`), solved by the MINRES of ADR 0482 D2 unchanged (`minres` of `gyro.rs`, the same projected
operator and the same preconditioner `(I - P) K_b^-1 (I - P)^T` on each half, `RITZ_TOLERANCE = 1e-10`, at most `min(4 n_q +
20, 2000)` iterations). Then `E_R,ij = -v_i . (Q x_j)_re`, `E_I,ij = -v_i . (Q x_j)_im` (one application of `Q(theta)` per
`j`). So each split mode costs `n_b` complement solves and one dense quadratic solve of order `n_b`. A MINRES that does not meet
its tolerance, a failed dense solve, a root not found or a non-finite value gives `None`.

**Reporting (registered):** `HarmonicSplitModes` (non-exhaustive) gains `schur_eta: Vec<Option<f64>>`, parallel to
`modes.modes`. It is `Some` exactly where ADR 0482 reports `eta` (`split_correction` and `coriolis` on, a stable speed), unless
its own solves fail as above; `None` everywhere else (without the option, without `coriolis`, at an unstable speed). It is
computed only in `RotatingSector::split_modes`; `split_campbell` stores neither `eta` nor `schur_eta` and does not compute the
new value (its internal call passes a flag; ADR 0482's `eta` computation inside it is left as it is, so its results and cost are
unchanged). No other field changes: the projected matrices are returned by `project_and_solve` beside its present results by the
same operations in the same order, and the split modes, `eta`, `corrections` and every other field keep the bits of 12fce5c9
(F5 proves it with digests recorded at the base). No new option, no case-file key, no CLI output.

### D3. Why the estimate should be within 10 percent (the basis of the limit)

By D1 the condensed problem is exact; the estimate differs from the measured error only by:

1. **Freezing `E` at `theta`** (and folding `E_I` at `l = theta`). With `delta = theta - omega`, `E(omega) - E(theta) =
   -E'(theta) delta + O(delta^2)`, so `|theta' - omega| ~ |y^H E'(theta) y| |delta| / |y^H S'(theta) y|`, and the relative error
   of the estimate is `eps_f ~ |y^H E' y| / |y^H Q'_VV y|`. In complement modal terms `E ~ sum_c |Q_cV|^2/(w_c^2 - theta^2)` and
   `delta/theta ~ y^H E y/(2 theta^2)`, which gives `eps_f ~ (delta/theta) (2 theta^2/(w_c^2 - theta^2) + O(1))` (the `O(1)` from
   the `i w G_WV` part of `Q_WV`). The fold adds a term `(theta' - theta)/theta |E_I|` of the same order. Both are of the order of
   the error itself times a factor that is large only when `theta` is within a small fraction of a complement frequency `w_c`.
   For the matched modes of B4 (errors at most 4.5e-5, ADR 0482's records) `eps_f <= 0.1` would need a factor above 2000, that
   is `theta` within 0.05 percent of `w_c`.
2. **Rounding**: of `theta` and `theta'` (two dense solves on matrices that differ only by `E`) and of the reference (ADR 0411:
   residual about 1e-13); the probe below found the lowest-mode frequencies of the library and of an independent dense
   computation to agree within 1.1e-13 relative. At the threshold `1e-9` that is `<= 2e-4` of the error.
3. **The MINRES tolerance** `1e-10` on each complement solve: `E` is linear in `x_j`, so its relative error is about `1e-10`
   times the condition of the preconditioned complement operator (eigenvalues `1 - theta^2/w^2` plus the gyroscopic part, ADR 0482
   arithmetic 4); negligible for the matched (lower) modes.
4. **Matching**: the corrected vector differs from `y` by `O(delta)`; the closest pair of split modes of B4 is 1.3 percent apart
   (index 1 at `0.4 omega_ref`, 1.80834e3 and 1.83193e3 rad/s), so the overlap picks the right root.

So `|err/schur_eta - 1| <= eps_f + 2e-4 + (MINRES)`, estimate `<= 1e-3` over the scope of a2, a factor 100 below the limit 0.1.

### D4. Bits (a3)

Without `split_correction` (or without `coriolis`) the code path is unchanged except that `schur_eta` is filled with `None`.
With it, the split modes and `eta` are computed first, by the operations of 12fce5c9 in the same order; the new solves only read
their results. `split_campbell` does not compute the new value. `RotatingPart` paths, the F-396, F-499 and F-555 paths are not
touched. `damped.rs`, `rotating.rs`, `cyclic.rs`, `mpc.rs` are not edited.

## Set-ups (fixed now)

As ADR 0482: steel DS (`N = 6`, Hex20 on `hex_box([0.25, 0, -0.1], [0.75, 2 pi/6, 0.2], [2, 1, 1])` mapped to the annulus, `xmin`
clamped, `Cyclic("ymin", "ymax", e_z, 0, 6)`, both effects on), `ModalOptions::new(m_b).tolerance(1e-9)`, 400 iterations,
`RotatingOptions::new(m_b).modal(..).coriolis(true).split_correction(true)`. Bases **B4** (`m_b = 4`) and **B41** (`m_b =
41`). `omega_ref` = the lowest rest whirl frequency over `k = 0 .. 3` of the B4 sector without the option (1655.3226 rad/s).
Speeds `0.4` and `0.8 omega_ref`. Reference `RotatingSector::whirl_modes(k, Omega)` (the lowest `m_b mu` whirl modes).
**DSS**: DS with stress stiffening off, B4, at `10 omega_ref`. **Matching (as ADR 0481 D5, ADR 0482):** each reference mode is
matched to the split mode of the same index and speed with the largest complex MAC (ties: the lower split mode); MAC `>= 0.99`,
the matches of one index and speed distinct. Measured error `err = |theta/omega - 1|` of the match.

**Test-side reference of the estimate** (for F4): the same condensed problem computed in the test with dense algebra, independent
of the library's MINRES: the reduced `K_r` (`HarmonicModes::reduced_stiffness`), `M_r`, and `C_r` assembled column by column
from `Transformation::{expand, transpose_times}` and the doubled Coriolis matrix; the enlarged basis rebuilt test-side by the
rule of ADR 0482 D1 (dense solves, Gram-Schmidt twice, the 1e-8 drop rule); the projected matrices and `split_modes_dense`; the
complement solves as one bordered dense system `[[R, B], [B^T, 0]]` (`R` the real form of `Q(theta)`, `B = blockdiag(M_r V, M_r
V)`, the constraint `V^T M_r x = 0` replacing the projection) by Gaussian elimination with partial pivoting; then D2 as in the
library. This is the probe's program (below) made a test.

## Arithmetic before this registration (disclosed)

1. **Probe** (`f575-scratch/zz_probe_f575.rs`, run in the worktree's test folder and moved to the scratch folder, never
   committed; `probe1.log`): the test-side reference above on DS B4 at `0.4` and `0.8 omega_ref`, every index, for every reference
   mode with its best-MAC match (the test-side split modes; every MAC `>= 0.999982`). Over the 34 matched modes with `est > 1e-9`
   or `err > 1e-9` (both sets are the same 34): **`|err/est - 1| <= 3.36e-4`**; frozen a second time at `theta'` instead of
   `theta`, `<= 3.1e-6` (the freezing error of D3 item 1 is the 3.4e-4). Below 1e-9 (down to `err = 3.1e-12`) `err/est` lies in
   `[0.970, 1.007]` (rounding). ADR 0482's `eta` on the same modes: `err/eta` up to 2.62 (its known miss, reproduced). The
   test-side `theta` equals the library's split frequency within 1.1e-13 relative. The Hermitian defect of the test-side `E`
   (relative to its largest entry) is at most 1.6e-10. The corrected root `theta'` equals the reference `omega` within `1.1e-8`
   relative at worst (the mode with `err = 3.3e-5`) and within `3.2e-11` for every mode with `err <= 1e-6`.
2. **Probe of the mutations** (`probe2.log`, the same program with variants of the test-side estimate): the gyroscopic term
   removed from the complement operator and coupling (the counterpart of ADR 0482's MU4 for the new estimate): worst `|err/est -
   1|` 0.999; the skew part `E_I` not folded (only `K'` corrected): 0.999; the complement frozen at shift 0 (static condensation):
   0.999; the sign of `E` flipped: 0.9996. Each is caught by F1 by a factor 10 above its limit.
3. **Scope of F1** (the count): the probe's 34 modes with `est > 1e-9` (16 at 0.4, 18 at 0.8); the registered lower bound 30.
4. **Cost**: B4 pair index: 16 split modes, 16 complement solves each (order 168), MINRES convergence in tens of iterations (ADR
   0482 arithmetic 4); milliseconds per index.
5. **F4 tolerance**: library and test-side estimates differ by the rounding of `theta` (`<= 1.1e-13` relative, item 1), of the
   bases and of the solves; at `est = 1e-9` that is `<= 2e-4` of the estimate; limit 1e-2.

## Checks (limits fixed now)

Tests in a new file `crates/rcfd-fem/tests/sa_gyro_sector_schur.rs` (all but F5) and
`crates/rcfd-fem/tests/sa_gyro_sector_schur_default_bits.rs` (F5), release profile. Every aggregated value in a check is
required finite (a non-finite value fails the check, never enters a `max`).

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| F1 | a1, a2 | DS B4 with the option, `k = 0 .. 3`, `0.4` and `0.8 omega_ref`: every reference mode matched (MAC `>= 0.99`, distinct); for every match whose `schur_eta` exceeds 1e-9, `|err/schur_eta - 1|`; the number of such matches over both speeds | matched; `<= 0.1`; count `>= 30` | `<= 1e-3` (D3; probe 3.4e-4); 34 |
| F2 | a2 | the same runs: every match whose measured `err` exceeds 1e-9 has `schur_eta = Some(v)` and `|err/v - 1|` (the other reading of "where it exceeds 1e-9"; it fails a value set to zero) | `<= 0.1` | as F1 (the probe's two sets coincide) |
| F3 | a1 | the same runs: every matched mode has `schur_eta = Some(v)`, `v` finite, `>= 0` | each | D2 |
| F4 | a1 | the same runs: the library's `schur_eta` against the test-side reference (above) for every match whose test-side estimate exceeds 1e-9: `|lib/test - 1|` | `<= 1e-2` | `<= 2e-4` (arithmetic 5) |
| F6 | a1 | DS B4 at `0.4 omega_ref`, `k = 0 .. 3`: without `split_correction` (`coriolis` on) every `schur_eta` is `None`; with `split_correction` and without `coriolis` every `schur_eta` is `None`; `schur_eta.len() == modes.modes.len()` in every run of F1 | each | D2 |
| F7 | a1 | DS B41 with the option at both speeds, every index: every `schur_eta == Some(0.0)` (the corrections complete the space, ADR 0482 D1) | each | D2 |
| F8 | a1 | DSS at `10 omega_ref` with the option, `k = 0 .. 3`: `schur_eta[i]` is `None` exactly where `eta[i]` is `None`, `Some` finite otherwise | each | D2 (ADR 0482: all four unstable) |
| F5 | a3 | digests recorded at 12fce5c9 by `sa_gyro_sector_schur_default_bits.rs` (written with the API of that commit, committed before any library line): DS B4 `split_modes` with both options, `k = 0 .. 3`, 0.4 and 0.8 (every field of ADR 0482 B1's digest plus `corrections`, `dropped_corrections`, `eta`); DS B4 `split_campbell` with both options over `{0, 0.2, 0.4, 0.6, 0.8} omega_ref` (eigenvalues, MAC, whirl, waves, growth, stability, basis); DS B41 `split_modes` with both options at 0.4 and 0.8; DSS with both options at `10 omega_ref`; DS B4 with `split_correction` and without `coriolis` (`split_modes` at 0.4, `split_campbell`): all equal after the change; ADR 0481's `sa_gyro_sector_default_bits` (13) and ADR 0482's `sa_gyro_sector_correction_default_bits` (6) pass; `cargo test -p rcfd-api --release --test baseline_hashes` passes; `cargo test -p rcfd-fem --release` passes | `==`; pass | holds by construction (D4) |

Records (reported, not in the decision rule): per index and speed the error, `eta`, `schur_eta` and the two ratios of every
matched mode; `err/schur_eta` (min, max) below 1e-9; the `schur_eta` of the unmatched split modes (the top half of the enlarged
basis) and the count of `None` among them; the wall time of the B4 runs with and without the new value.

## Negative controls and mutations

| Id | Control | Expected |
|---|---|---|
| N1 | ADR 0482's `eta` on the matches of F1 (its scope `eta > 1e-9`): largest `|err/eta - 1|` | `> 0.1` (1.62 in ADR 0482) |
| N2 | the test-side reference with the gyroscopic term removed from the complement operator and coupling (the counterpart of MU4 of ADR 0482), on the scope of F1: largest `|err/est - 1|` | `> 0.1` (0.999 in the probe): F1's tolerance separates it |

Mutations (on a scratch copy of the worktree with its own target folder, each run once against `sa_gyro_sector_schur`; each
must be caught by a registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MV1 | the complement projection removed from the complement solves (operator, right side and preconditioner unprojected; the counterpart of MU3 of ADR 0482) | F1, F2, F4 (or F3 if MINRES fails) |
| MV2 | the gyroscopic term removed from `Q(theta)` in the complement solves and the coupling (the counterpart of MU4) | F1, F2, F4 |
| MV3 | `E_I` not folded (`C' = C_VV`) | F1, F4 |
| MV4 | the complement frozen at shift 0 instead of `theta` | F1, F4 |
| MV5 | the sign of `E` flipped (`K' = K_VV + sym(E_R)`, `C' = C_VV + ..`) | F1, F4 |
| MV6 | the corrected root taken as the lowest root of the corrected solve instead of the best overlap | F1, F2, F4 |
| MV7 | `schur_eta` filled with ADR 0482's `eta` | F1, F2, F4 |
| MV8 | the split modes reported from the corrected solve of each mode (the new solve leaking into the results of ADR 0482) | F5 |

## Decision rule

**done**: every check met at its registered run (F1 to F8), N1 and N2 as expected, every mutation MV1 to MV8 caught, no amendment
after a run. **partial**: a check missed (kept as a `*_known_miss` with its limit), N1 or N2 not as expected, a mutation not
caught, or a check met only after an amendment made after its run. **todo**: the module does not build or the DS runs cannot be
made.

## Limits (not covered)

- The estimate is exact only up to the freezing of the condensed matrix at the Ritz value; it is an estimate, not a bound, and
  its accuracy degrades for a split mode close to a frequency of the complement (the top half of an enlarged basis, recorded
  only).
- One disc geometry, one basis size with a non-empty complement (B4); the 120 mm fan sector is not run here.
- Cost: `n_b` complement solves per split mode, so `O(n_b^2)` solves per index: computed only in `split_modes`, not in
  `split_campbell`. A cheaper form (one solve per split mode giving the column `E y`, and a second-order perturbation sum) is not
  built here.
- The corrected root `theta'` itself is not reported (only its distance to `theta`).
- The rest basis at a loss of stiffness: as ADR 0482, the speed `K_eff` in `Q`, the rest factor as preconditioner; F8 checks
  the path only (all four indices unstable there).
- No case-file key, no CLI output.

## Addendum 1: results (2026-11-14)

Registered run: `f575-scratch/run1.log` (commit 140f15db, release, made once, `--test-threads=1`). F5 digests recorded at
12fce5c9 (commit 2ab63529) before any library line. Code: `crates/rcfd-fem/src/gyro.rs` only (the module comment; the field
`HarmonicSplitModes::schur_eta`; `project_and_solve` now wraps `project_and_solve_with`, which also returns the symmetrised
projected matrices; `sector_split_at` gains the flag `schur`, true from `split_modes`, false from `split_campbell`; the new
section at the end of the file: `shifted_apply`, `schur_error`). `rotating.rs`, `damped.rs`, `cyclic.rs`, `mpc.rs` not edited.
Tests: `crates/rcfd-fem/tests/sa_gyro_sector_schur.rs` (4 tests, 1.5 s together), `sa_gyro_sector_schur_default_bits.rs`.

`omega_ref = 1655.3226` rad/s.

| Id | Check | Limit | Measured | Met |
|---|---|---|---|---|
| F1 | matches (MAC `>= 0.99`, distinct), every index and speed | each | holds (smallest MAC 0.999982) | yes |
| F1 | `|err/schur_eta - 1|` where `schur_eta > 1e-9`; count | `<= 0.1`; `>= 30` | **3.363e-4** (`err/schur_eta` in `[0.99966, 1.00000]`); 34 (16 at 0.4, 18 at 0.8) | yes |
| F2 | the same where `err > 1e-9` | `<= 0.1` | 3.363e-4 over the same 34 modes | yes |
| F3 | `schur_eta` Some, finite, `>= 0`, every matched mode | each | holds (48 of 48) | yes |
| F4 | library against the test-side dense reference where the latter `> 1e-9` | `<= 1e-2` | 3.25e-7 (34 modes) | yes |
| F6 | `None` without `split_correction` and without `coriolis`; lengths | each | holds | yes |
| F7 | B41: every `schur_eta == Some(0)` | each | holds (42, 84, 84, 42 modes at both speeds) | yes |
| F8 | DSS at `10 omega_ref`: `None` exactly where `eta` is `None` | each | holds (all four indices unstable: every entry `None`, so the "Some otherwise" half is vacuous, as in ADR 0482 L2) | yes |
| F5 | 6 digests of 12fce5c9; the 13 digests of ADR 0481 and the 6 of ADR 0482; `baseline_hashes`; `cargo test -p rcfd-fem --release` | `==`; pass | 6 of 6 equal; 13 of 13; 6 of 6; `baseline_hashes` pass (3 passed, 1 ignored); `rcfd-fem` pass (668 passed, 0 failed, 169 ignored) | yes |
| N1 | the `eta` of ADR 0482, largest `|err/eta - 1|` where `eta > 1e-9` | `> 0.1` | 1.621 | as expected |
| N2 | test-side estimate without the gyroscopic coupling | `> 0.1` | 0.999 | as expected |

Records: below 1e-9 (14 modes, `err` from 3.2e-12 to 8.3e-10) `err/schur_eta` lies in `[0.9877, 1.0183]` (rounding of the
dense solves at the 1e-11 level). The unmatched split modes (the top half of the enlarged basis) all carry a `schur_eta` (no
`None`), from 9.8e-4 to 73.6: they are poor approximations, and near the complement spectrum the frozen estimate is not
validated (D3). Wall time of `split_modes` with the new value 0.007 to 0.21 s per index against 0.003 to 0.018 s for the same
index and speed through `split_campbell` (no `schur_eta`): a factor of up to 13 on the pair indices (16 modes x 16 solves).

### Mutations (scratch copy `f575-scratch/mut` of 140f15db, own target folder, script `f575-scratch/mutate.py`, logs `mut_MV*.log`)

| Id | Mutation | Caught by |
|---|---|---|
| MV1 | complement projection removed from the Schur solves | F1 (count 0: every value below 1e-9), F2 (1.7e10), F4 (1.0) |
| MV2 | gyroscopic term removed from `Q(theta)` in the Schur solves | F1 (1.000), F2, F4 (1.1e3) |
| MV3 | `E_I` not folded | F1 (1.000), F2, F4 (1.5e3) |
| MV4 | complement frozen at shift 0 | F1 (1.000), F2, F4 (1.1e3) |
| MV5 | sign of `E` flipped | F1 (3.23), F2, F4 (0.76) |
| MV6 | the lowest corrected root instead of the best overlap | F1 (1.000), F2, F4 (8.2e8) |
| MV7 | `schur_eta` filled with the `eta` of ADR 0482 | F1 (1.621), F2, F4 (0.62) |
| MV8 | split frequencies replaced by the corrected roots | F5 (the B4 `split_modes` digest), F1, F2 |

MV1 and MV2 are the counterparts of MU3 and MU4 of ADR 0482, which survived there; here each is caught by three checks.

### Deviations and what was not run

- After the registered run `cargo clippy -D warnings` refused `Option::is_none_or` (the workspace MSRV is 1.80); the comparison
  was rewritten as a `match` with the same result (commit fa79af7f). The checks and F5 were run once more on that commit only to
  confirm the rewrite (`f575-scratch/after_msrv_fix.log`: F1 3.363e-4, F2 3.363e-4, F4 3.25e-7, F5 equal; the same numbers). No
  limit and no test was changed.
- The registered limits were met at the first run; no amendment. `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p
  rcfd-fem --all-targets --release -- -D warnings` clean (after the rewrite); `cargo test -p rcfd-cli --release --test
  docs_site` pass (2 passed). The whole workspace suite was not run (the lead's gate).

### Decision

**Done** by the registered rule: every check F1 to F8 met at its registered run, N1 and N2 as expected, all eight mutations
caught, no amendment. Every clause holds: `split_modes` with `split_correction` reports `schur_eta` beside the `eta` of ADR 0482
(one complement solve per basis vector and one dense quadratic solve per split mode); on the disc sector with 4 modes per index
at 0.4 and 0.8 `omega_ref` it agrees with the measured error of all 34 matched modes above 1e-9 within 3.4e-4 (limit 0.1); the
results of ADR 0482 (and 0481) keep their bits.

### Remaining questions

1. Cost. `n_b` complement solves per split mode (a factor up to 13 in `split_modes` on B4). Options: (a) keep as is; (b) a
   follow-up with one solve per split mode (the column `E y`) and a second-order perturbation sum over the other Ritz vectors;
   (c) an option to switch `schur_eta` off. Recommendation: (a) until a fan sector shows the cost matters, then (b).
2. Report the corrected frequency `theta'` itself (within 1.1e-8 of the exact one on DS B4, against errors up to 4.5e-5 of
   `theta`)? Options: (a) no; (b) an opt-in field. Recommendation: (b) as a small follow-up; it is already computed. Taken
   here: not added (not asked by the acceptance), on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation of keeping the scope).
3. Case-file key: none added (as ADR 0482 question 2).

## For the lead: roadmap entries

Proposed status of F-575: **done**. New note:

> Structural solver, advanced tier (plan ADR 0310). DONE. Size S. ADR 0492, 2026-11-14: RotatingSector::split_modes with split_correction reports beside the first-order eta of ADR 0482 the field schur_eta, the Ritz error |theta/theta' - 1| of the projected problem condensed by the complement and frozen at the Ritz value (one MINRES complement solve per basis vector, then one dense quadratic solve per split mode with K_VV - Re E and the skew part of E folded into the gyroscopic matrix); not computed by split_campbell. On the disc sector of ADR 0411 with 4 harmonic modes per index at 0.4 and 0.8 omega_ref it agrees with the measured error of all 34 matched modes above 1e-9 within 3.4e-4 (the eta of ADR 0482 within a factor 2.62), and with an independent dense computation within 3.3e-7; with 41 modes per index it is 0. The results of ADR 0481 and 0482 and every default keep their bits (6 new digests plus 19). All eight mutations caught, including the counterparts of the uncaught MU3 and MU4 of ADR 0482. Cost: up to 13 times the split_modes time on the pair indices.

Proposed addition to the note of F-569 (its missed clause a4 is now covered), as a final sentence:

> The second-order Ritz error that clause a4 needed is reported by F-575 (ADR 0492, 2026-11-14): schur_eta within 3.4e-4 of the measured error.

Follow-up (optional, number left to the lead): title "Corrected frequency and cheaper second-order Ritz error of the sector split modes", deps F-575.

> Every limit not stated here is registered in the ADR before the run. RotatingSector::split_modes optionally reports the corrected frequency theta' of the Schur-complemented problem of ADR 0492, and computes schur_eta with one complement solve per split mode and a second-order perturbation sum over the other Ritz vectors; on the disc sector of ADR 0411 with 4 harmonic modes per index at 0.4 and 0.8 omega_ref it agrees with the schur_eta of ADR 0492 within 1 percent where it exceeds 1e-9, theta' equals RotatingSector::whirl_modes within 1e-7 for every matched mode, and the results of ADR 0492 keep their bits.
