# ADR 0527: F-584 registration: the corrected frequency and a second-order perturbation form of the Schur Ritz error of the sector split modes

Status: **REGISTERED 2026-11-14, written before any code of F-584 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-584 was written in the repository. Three runs of one scratch probe program on the
existing API of f168d994 were made in the scratch folder (`w49i-work/f584-scratch/zz_probe_f584.rs`, copied into the worktree's
test folder for each run and removed after it, never committed; logs `probe1.log`, `probe2.log`, `probe3.log`); they are
disclosed in full under "Arithmetic before this registration". They show that the formula the acceptance prescribes misses its 1
percent limit on this basis; the limit is kept and the miss is predicted here, not tuned away. Results go in the addendum at the
end; nothing above the addendum is changed after a run.

Roadmap item F-584 (deps F-575), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0492](0492-f575-registration-schur-ritz-error-of-corrected-sector-split-modes.md) (remaining
questions 1 b and 2 b). Number 0527 given by the lead. Branch `f584-work` from f168d994, worktree `w49i-work/f584`, scratch
`w49i-work/f584-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical and the results of ADR 0492 (and 0481, 0482) bit-identical; no edit of `baseline_hashes.rs`, of the hash tables or
of any golden file; no existing test edited; no new dependency, no unsafe code; clean-room. The brief overrides rule 9 for this
item: the F-584 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated on the branch in a separate last commit (the
svgs are not regenerated). Every choice below that the maintainer would normally be asked about is taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation); none changes a default result, a golden or a hash table.

## What F-584 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. RotatingSector::split_modes optionally reports the corrected frequency theta' of the Schur-complemented problem of ADR 0492, and computes schur_eta with one complement solve per split mode and a second-order perturbation sum over the other Ritz vectors; on the disc sector of ADR 0411 with 4 harmonic modes per index at 0.4 and 0.8 omega_ref it agrees with the schur_eta of ADR 0492 within 1 percent where it exceeds 1e-9, theta' equals RotatingSector::whirl_modes within 1e-7 for every matched mode, and the results of ADR 0492 keep their bits.

Clauses: **a1** `RotatingSector::split_modes` optionally reports `theta'`, the corrected frequency of the Schur-complemented
problem of ADR 0492; **a2** it (optionally) computes `schur_eta` with one complement solve per split mode and a second-order
perturbation sum over the other Ritz vectors; **a3** on DS of ADR 0411 with B4 at `0.4` and `0.8 omega_ref` the a2 value agrees
with the `schur_eta` of ADR 0492 within 1 percent where it exceeds 1e-9; **a4** `theta'` equals `RotatingSector::whirl_modes`
within 1e-7 (relative) for every matched mode; **a5** the results of ADR 0492 keep their bits. The brief adds: the new path
opt-in, a default-bits record first (before library code), `baseline_hashes` unchanged, no case-file key (open question), the
second-order formula registered with its derivation, mutations the checks catch, every aggregated test error finite.

Readings fixed here. "Where it exceeds 1e-9": the `schur_eta` of ADR 0492 (G1; the other reading, the new value, is G2).
"Agrees within 1 percent": `|new/old - 1| <= 0.01`. The scope of a3, as in ADR 0492 F1: the split modes matched to the reference
modes (the unmatched top half of the enlarged basis is recorded only; ADR 0492 did not validate its estimate there). "theta' of
the Schur-complemented problem of ADR 0492": the root of ADR 0492 D2 (its dense frozen solve); with the a2 path the reported
`theta'` is that path's estimate of the same root (`theta + delta_1 + delta_2`, D2 below). Both are checked against a4 (G3 for
the ADR 0492 root, G4 for the a2 estimate).

## What was read (f168d994, read only)

ADR 0492 whole (D1 to D4, the set-ups, the test-side reference, the addendum: F1 3.363e-4, F4 3.25e-7, the records of the
unmatched split modes from 9.8e-4 to 73.6, the cost factor 13, remaining questions 1 and 2), ADR 0411 (D5, the sector
`whirl_modes`, Q1 residuals about 1e-13), the source of `gyro.rs` (`split_modes_dense`, `project_and_solve_with`,
`sector_split_at`, `CorrectionBasis`, `ritz_error`, `minres`, `shifted_apply`, `schur_error`), `rotating.rs`
(`RotatingOptions`), the tests `sa_gyro_sector_schur.rs` and `sa_gyro_sector_schur_default_bits.rs` (set-ups, matching, the
test-side dense reference, the digest pattern).

Published sources, cited from memory, **not read** for this work: Rayleigh-Schroedinger perturbation theory of a Hermitian
eigenproblem, first and second order (any text on matrix perturbation, for example G. W. Stewart and J.-G. Sun, Matrix
Perturbation Theory, Academic Press 1990); the linearisation of a quadratic eigenproblem to a Hermitian pencil (F. Tisseur and K.
Meerbergen, SIAM Rev. 43 (2001) 235); the Epstein-Nesbet partition (P. S. Epstein, Phys. Rev. 28 (1926) 695; R. K. Nesbet, Proc.
R. Soc. A 230 (1955) 312) and the Pade resummation of a perturbation series (G. A. Baker, Essentials of Pade Approximants,
Academic Press 1975), both probed only (arithmetic 2). Nothing numeric is taken from them; every formula used is derived below.

## Design

Notation of ADR 0492: index `k`, reduced coordinates of order `n_q`, `K_r`, `M_r`, `C_r` (doubled for a pair), `G = 2 Omega
C`, `Q(l) = K_r - l^2 M_r + i l G` (Hermitian for real `l`), the enlarged basis `V = [v_1 .. v_nb]` (real, `M_r`-orthonormal),
`P = V V^T M_r`, the complement `W`; the projected matrices `K_VV`, `M_VV`, `C_VV`, `G_VV = 2 Omega C_VV`; the Ritz pairs
`(theta_b, y_b)`, `b = 1 .. n_b`, of `split_modes_dense` on them (`y_b^H M_VV y_b = 1`, `theta_b > 0`). The condensed matrix of
ADR 0492 D1 frozen at the Ritz value `theta = theta_a` of the split mode `a`: `S(l) = Q_VV(l) - E`, `E = E(theta_a) = Q_VW
Q_WW(theta_a)^-1 Q_WV`, Hermitian.

### D1. The second-order perturbation sum (derivation)

**Linearisation.** `Q_VV(l) y = 0` with `D = i G_VV` (Hermitian) is `(K + l D - l^2 M) y = 0`. With `z = [y; l y]`,
`A = -blockdiag(K_VV, M_VV)` and `B = [[D, -M_VV], [-M_VV, 0]]` (both Hermitian) it is `A z = l B z`: row 1 reads `-K y = l D y -
l^2 M y`, row 2 `-M (l y) = l (-M y)`. At a stable speed with `K_VV` positive definite the `2 n_b` eigenvalues are real: the
split modes `(theta_b, y_b)` and their mirrors `(-theta_b, conj(y_b))` (since `Q(-l) = conj(Q(l))` for real `K`, `M`, `G`). For
distinct eigenvalues `z_b^H B z_c = 0` (`b != c`, from `z_b^H A z_c = l_c z_b^H B z_c = l_b z_b^H B z_c`), and

`s_b = z_b^H B z_b = y_b^H D y_b - 2 l_b y_b^H M_VV y_b = y_b^H Q_VV'(l_b) y_b`

(real, nonzero for a simple root). With `y = y_r + i y_i`, `y^H D y = i y^H G y = -2 (2 Omega) y_r^T C_VV y_i`, so `s(l, y) = -2 l
y^H M y - 4 Omega y_r^T C_VV y_i`; for the mirror `s(-theta_b, conj(y_b)) = -s_b`.

**Perturbation.** The frozen condensed problem `(K_VV - E) + l D - l^2 M` is the pencil with `A + Delta A`, `Delta A =
blockdiag(E, 0)` (a constant matrix: `E` frozen at `theta_a`). Expanding `l = l_a + delta_1 + delta_2 + ..`, `z = z_a + sum_{b !=
a} c_b z_b + ..` and testing with `z_a^H` and `z_b^H`:

`delta_1 = y_a^H E y_a / s_a`, `c_b = y_b^H E y_a / ((l_a - l_b) s_b)`, `delta_2 = sum_{b != a} |y_b^H E y_a|^2 / ((l_a - l_b) s_a
s_b)`,

the sum over **every other eigenpair of the pencil**: the other split modes `b != a` and the mirrors of all split modes
(including `a`'s own mirror at `-theta_a`). Only the column `e = E y_a` enters: `y_b^H E y_a = y_b^H e`. The corrected root is

`theta' = theta_a + delta_1 + delta_2`, and the reported value `schur_eta = |theta_a / theta' - 1|`

(the form of ADR 0492 D2). `delta_1` is the first-order shift (the `eta` of ADR 0482 up to its denominator: both `y^H E y / |s|`
in the same normalisation, ADR 0492 D1); `delta_2` is the second-order coupling of `E` with the other Ritz vectors that ADR 0482
missed.

**One complement solve per split mode.** `e = E y_a = -V^T Q(theta_a) x`, `x` in `W` solving `(I - P)^T Q(theta_a) (I - P) x =
-(I - P)^T Q(theta_a) V y_a` (ADR 0492 D1 with the right side `V y_a` instead of `v_j`: by linearity `x = sum_j y_aj x_j`). In
the real form of ADR 0482 D2 (`Q(theta)` commutes with the multiplication by `i`, `(u_r, u_i) -> (-u_i, u_r)`, so a complex right
side is one solve of length `2 n_q`): `u = (V Re y_a, V Im y_a)`, the right side `-(I - P)^T Q(theta_a) u`, the MINRES of ADR 0482
D2 unchanged (the same projected operator, the same preconditioner `(I - P) K_b^-1 (I - P)^T` on each half, `RITZ_TOLERANCE =
1e-10`, at most `min(4 n_q + 20, 2000)` iterations), then `e_r,i = -v_i . (Q x)_re`, `e_i,i = -v_i . (Q x)_im`. Cost per split
mode: one MINRES solve, one application of `Q(theta_a)`, `O(n_b^2)` dense work (against `n_b` solves and one dense quadratic
solve in ADR 0492).

Refusals inside the estimate (value `None`): a MINRES that does not meet its tolerance; fewer than `n_b` split modes in the
projected problem (a real root: the pencil's eigenvectors are then not the complete set used above); a non-finite `s_b`,
`delta_1`, `delta_2` or `schur_eta`. When the complement is empty (`n_b >= n_q`) `schur_eta = Some(0.0)` and `theta' = theta_a`
without a solve, as ADR 0492.

### D2. The corrected frequency (a1)

With the new option the split mode also reports `theta'`: on the path of ADR 0492 the root of its dense frozen solve (the value
`theta_c` its `schur_error` already computes; `schur_eta = |theta / theta_c - 1|` from the same number, so both report the bits of
ADR 0492's `schur_eta`), on the path of D1 `theta_a + delta_1 + delta_2`. `None` exactly where `schur_eta` is `None`, and
everywhere without the option.

### D3. API (registered)

`rotating.rs`: `RotatingOptions` (non-exhaustive) gains `split_schur: SplitSchur` (new enum, `#[non_exhaustive]`, variants
`Dense`, the default, ADR 0492 as it is, and `Perturbation`, D1) with the builder `split_schur(SplitSchur)`, and
`corrected_frequency: bool` (default false) with the builder `corrected_frequency(bool)`. Both have effect only where ADR 0492
computes `schur_eta` (`RotatingSector::split_modes` with `coriolis` and `split_correction` at a stable speed); `split_campbell`
computes neither. `gyro.rs`: `HarmonicSplitModes` (non-exhaustive) gains `corrected_frequency: Vec<Option<f64>>`, parallel to
`modes.modes`. No case-file key, no CLI output, no change to `RotatingPart`. `schur_error` returns its root beside its value by
the same operations; the new function `schur_perturbation` (D1) is added beside it.

### D4. Why the 1 percent and the 1e-7 are expected to be missed (predicted before the run)

The RS series of D1 is an expansion in `|y_b^H E y_c| / (|l_b - l_c| |s|)`; it is short only when every Ritz vector of the
basis is perturbed little by `E`. The enlarged basis of ADR 0482 is the four modes plus their correction vectors: the Ritz pairs
of the top half are poor (ADR 0492's records: their `schur_eta` from 9.8e-4 to 73.6), and their own first-order shifts
`delta_1,b / theta_b` reach `-1.19` (arithmetic 2). Through them the third and higher orders of the low modes are of the size of
the second: the probe finds `delta_2/delta_1` up to 0.62 and the miss of the second-order sum `|eta_2/schur_eta - 1|` equal to
`(delta_2/delta_1)^2` to the probe's accuracy (0.616^2 = 0.379 against 0.383), the signature of a series with ratio `r =
delta_2/delta_1` whose remainder is `delta_1 r^2/(1 - r)`. No form that uses only the column `E y_a` can remove the terms in
`y_b^H E y_c` (`b, c != a`), which carry that remainder (arithmetic 2: the self-consistent and Epstein-Nesbet forms do no better,
the Pade [0/1] resummation `delta_1/(1 - delta_2/delta_1)` reaches 9.9 percent). So G1, G2 and G4 are predicted to miss; the
checks that validate the implementation of D1 as a formula (G6 against an independent dense computation, G5, G7 to G12) are
predicted to be met.

### D5. Bits (a5)

With `split_schur = Dense` and `corrected_frequency = false` (the defaults) the code path is that of f168d994 except that
`corrected_frequency` is filled with `None` and `schur_error` hands back a number it already computed. With `Perturbation` the
split modes, `eta` and every other field are computed first by the operations of f168d994 in the same order; only `schur_eta`
(and `corrected_frequency`) differ. `damped.rs`, `cyclic.rs`, `mpc.rs` are not edited; in `rotating.rs` only the options struct,
its constructor (two new defaults) and two builders.

## Set-ups (fixed now)

As ADR 0492: steel DS (`N = 6`, Hex20 on `hex_box([0.25, 0, -0.1], [0.75, 2 pi/6, 0.2], [2, 1, 1])` mapped to the annulus, `xmin`
clamped, `Cyclic("ymin", "ymax", e_z, 0, 6)`, both effects on), `ModalOptions::new(m_b).tolerance(1e-9)`, 400 iterations,
`RotatingOptions::new(m_b).modal(..).coriolis(true).split_correction(true)`; **B4** (`m_b = 4`), **B41** (`m_b = 41`);
`omega_ref` the lowest rest whirl frequency over `k = 0 .. 3` of B4 without the option (1655.3226 rad/s); speeds `0.4` and `0.8
omega_ref`; reference `RotatingSector::whirl_modes(k, Omega)`; **DSS** DS without stress stiffening at `10 omega_ref`. Matching as
ADR 0492 (best complex MAC, `>= 0.99`, distinct). The **dense run** is B4 with `corrected_frequency(true)` (path of ADR 0492), the
**perturbation run** B4 with `split_schur(SplitSchur::Perturbation).corrected_frequency(true)`; the ADR 0492 `schur_eta` is the
dense run's (its bits are those of the default, G11).

**Test-side reference of D1** (G6, N1, records): the dense algebra of ADR 0492's test-side reference (reduced `K_r`, `M_r`, `C_r`
dense; the enlarged basis rebuilt by the rule of ADR 0482 D1; the complement solves as one bordered dense system by Gaussian
elimination with partial pivoting), giving the full `E(theta_a)`; then D1 from its column `E y_a` on the test-side Ritz pairs.
This is the probe's program made a test.

## Arithmetic before this registration (disclosed)

1. **Probe 1** (`probe1.log`): DS B4 at both speeds, every index; for every reference mode its best-MAC library split mode
   (every MAC `>= 0.999982`) and test-side Ritz pair; D1 test-side. Over the 34 matched modes whose ADR 0492 `schur_eta` (library,
   default path) exceeds 1e-9: `max |eta_2 / schur_eta - 1| = 0.3835` (16 of 34 within 1 percent); the first order alone
   (`delta_2` dropped) 0.6185. `|theta'/omega - 1|` over the 48 matched modes: D1 (`theta + delta_1 + delta_2`) at most
   **7.15e-7** (43 of 48 within 1e-7; the misses 1.07e-7 to 7.15e-7, at `err` from 2.8e-7 to 4.5e-5); the root of ADR 0492's
   dense frozen solve at most **1.12e-8** (all 48 within 1e-7). Where `delta_2/delta_1 <= 0.01` the agreement is 1e-6 to 5e-4.
2. **Probe 2** (`probe2.log`), variants of the sum, the same scope: the first-order shifts of every Ritz pair at its own shift,
   `delta_1,b/theta_b`, `-3e-12` to `-1.19` for the split modes and up to `+1.36` (the top half); the Epstein-Nesbet form
   (denominators between those shifted levels) `max |eta/schur_eta - 1| = 15.9`, `theta'` 3.2e-6; the Pade [0/1] form
   `delta_1/(1 - delta_2/delta_1)` 0.0989, `theta'` 1.9e-8. Not adopted: the acceptance prescribes the second-order sum, and
   neither variant meets 1 percent.
3. **Probe 3** (`probe3.log`): the share of the mirror terms in `delta_2`: 0.15 to 0.41; dropping them changes `eta_2` by 7e-5 to
   0.15 (relative), so a mutation that drops them is visible to G6 (limit 1e-3) on most modes.
4. **Cost** (ADR 0492 records): the dense path 0.007 to 0.21 s per index and speed against 0.003 to 0.018 s through
   `split_campbell` (no `schur_eta`), the difference `n_b^2` MINRES solves; D1 needs `n_b` of them, so the perturbation run is
   expected to take about `t_base + (t_dense - t_base)/n_b`: summed over the 8 B4 runs, about 0.2 to 0.3 of the dense run.
5. **G6 tolerance**: ADR 0492's F4 (library MINRES against the same bordered dense solves) measured 3.25e-7; D1 adds only dense
   work on the column; limit 1e-3.

## Checks (limits fixed now)

Tests in a new file `crates/rcfd-fem/tests/sa_gyro_sector_split_schur.rs` (all but G11) and
`crates/rcfd-fem/tests/sa_gyro_sector_split_schur_default_bits.rs` (G11), release profile. Every aggregated value in a check is
required finite (a non-finite value fails the check, never enters a `max`). The relative frequency error is `|theta'/omega - 1|`.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| G1 | a2, a3 | DS B4, `k = 0 .. 3`, `0.4` and `0.8 omega_ref`: every reference mode matched (MAC `>= 0.99`, distinct) in the dense and the perturbation run (the same split mode index); for every match whose dense `schur_eta` exceeds 1e-9: `|perturbation/dense - 1|`; the count over both speeds | matched; `<= 0.01`; count `>= 30` | **0.38: predicted miss** (D4; probe 0.3835); 34 |
| G2 | a3 | the same where the perturbation `schur_eta` exceeds 1e-9 (the other reading) | `<= 0.01` | predicted miss (as G1) |
| G3 | a1, a4 | the dense run: every matched mode has `corrected_frequency = Some(t)`, `|t/omega - 1|` | `<= 1e-7` | 1.12e-8 (probe) |
| G4 | a1, a2, a4 | the perturbation run: the same | `<= 1e-7` | **7.15e-7: predicted miss** (probe; 5 of 48 above) |
| G5 | a1 | both runs: for every split mode with `schur_eta = Some(v)`, `corrected_frequency = Some(t)` with `(theta/t - 1).abs()` equal to `v` bit for bit; `None` exactly together | each | D2 |
| G6 | a2 | the perturbation run's `schur_eta` against the test-side D1 for every match whose test-side value exceeds 1e-9: `|lib/test - 1|`; the count | `<= 1e-3`; `>= 30` | `~1e-6` (arithmetic 5) |
| G7 | a2 | the wall time of `split_modes` summed over the 8 B4 runs, perturbation over dense (each pair measured back to back) | `<= 0.5` | 0.2 to 0.3 (arithmetic 4) |
| G8 | a1 | DS B4 at `0.4 omega_ref`, `k = 0 .. 3`: without `corrected_frequency` (both `split_schur`) every `corrected_frequency` is `None` and the length is the number of split modes; with `corrected_frequency` and `Perturbation` but without `split_correction`, and with them but without `coriolis`: every `schur_eta` and `corrected_frequency` `None` | each | D3 |
| G9 | a2 | DS B41, both speeds, every index, perturbation run: every `schur_eta == Some(0.0)` and `corrected_frequency[i] == Some(theta_i)` bit for bit | each | D1 |
| G10 | a2 | DSS at `10 omega_ref`, `k = 0 .. 3`, perturbation run: `schur_eta` and `corrected_frequency` `None` exactly where `eta` is `None`, `Some` finite otherwise | each | D1 (ADR 0492: all four unstable) |
| G11 | a5 | digests recorded at f168d994 by `sa_gyro_sector_split_schur_default_bits.rs` (written with the API of that commit, committed before any library line): DS B4 `split_modes` with `coriolis` and `split_correction`, `k = 0 .. 3`, 0.4 and 0.8 (the words of ADR 0492's F5 digest plus `schur_eta`); DS B41 the same; DSS at `10 omega_ref`; DS B4 `split_campbell` over `{0, 0.2, 0.4, 0.6, 0.8} omega_ref`: all equal after the change; ADR 0481's (13), 0482's (6) and 0492's (6) default-bits tests and `sa_gyro_sector_schur` pass; `cargo test -p rcfd-api --release --test baseline_hashes` passes; `cargo test -p rcfd-fem --release` passes | `==`; pass | holds by construction (D5) |
| G12 | a5 | the perturbation run and the dense run against the default (no new option): every field of the G11 words except `schur_eta` equal bit for bit, and the dense run's `schur_eta` equal to the default's bit for bit | `==` | D5 |

Records (reported, not in the decision rule): per matched mode `err`, both `schur_eta`, both `theta'` errors, the test-side
`delta_2/delta_1`, the predicted miss `(delta_2/delta_1)^2` against the measured G1 ratio, and the Pade [0/1] value; the
perturbation `schur_eta` of the unmatched split modes and its `None` count; per index the wall times.

## Negative controls and mutations

| Id | Control | Expected |
|---|---|---|
| N1 | the test-side first order alone (`delta_2` dropped) on the scope of G1: largest `|eta_1/schur_eta - 1|` | `> 0.1` (0.62 in the probe): the second-order sum is not the first order |

Mutations (on a scratch copy of the worktree with its own target folder, each run once against `sa_gyro_sector_split_schur`
and, for MP9, the default-bits test; each must be caught by a registered check other than the predicted misses G1, G2, G4):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MP1 | the mirror terms dropped from `delta_2` | G6 |
| MP2 | the sign of the denominators `l_a - l_b` flipped | G6 |
| MP3 | `s_b` replaced by `s_a` in `delta_2` | G6 |
| MP4 | the gyroscopic term removed from `Q(theta)` in the complement solve (the counterpart of MU4 of ADR 0482) | G6 |
| MP5 | the complement projection removed from the solve (the counterpart of MU3) | G6 |
| MP6 | `delta_2` omitted (first order only) | G6 |
| MP7 | the perturbation path computing the dense estimate of ADR 0492 instead | G6, G7 |
| MP8 | the corrected frequency reported as `theta` itself (uncorrected) | G3, G5 |
| MP9 | the default `split_schur` set to `Perturbation` | G11, G12 |
| MP10 | the term `b = a` (positive root) not skipped | G6 (or the finite requirement) |

## Decision rule

**done**: every check G1 to G12 met at its registered run, N1 as expected, every mutation MP1 to MP10 caught, no amendment after
a run. **partial**: a check missed (kept as a `*_known_miss` with its limit), N1 not as expected, a mutation not caught, or a check
met only after an amendment made after its run. **todo**: the module does not build or the DS runs cannot be made. By D4 the
expected outcome is **partial** (G1, G2, G4 predicted missed).

## Limits (not covered)

- The second-order sum is a truncated series; on a basis whose poor Ritz vectors are perturbed by `E` at the size of their gaps
  (the enlarged basis of ADR 0482) it is not accurate (D4). It is not a bound.
- The frozen-at-`theta` approximation of ADR 0492 is kept (constant `E`); the fold of `Im E` into the gyroscopic matrix of ADR
  0492 D2 is not used in D1 (it changes the root at second order in `theta' - theta`, about 1e-5 of the shift here).
- One disc geometry, one basis size with a non-empty complement; no fan sector.
- No case-file key, no CLI output; `split_campbell` computes neither value.

## Addendum 1: results (2026-11-14)

Registered run: `f584-scratch/run1.log` (commit ad8c247d, release, made once, `--test-threads=1`); G11 and the earlier
default-bits tests `f584-scratch/run1_g11.log` on the same commit. G11 digests recorded at f168d994 (commit a71861ee) before any
library line. Code: `crates/rcfd-fem/src/rotating.rs` (the enum `SplitSchur`, the fields `split_schur` and
`corrected_frequency` of `RotatingOptions` with their defaults and builders, nothing else) and `crates/rcfd-fem/src/gyro.rs`
(the module comment; the field `HarmonicSplitModes::corrected_frequency`; in `sector_split_at` the choice of the path and the
unpacking of `(schur_eta, theta')`; `schur_error` returns its root; the new section at the end: `pencil_norm`,
`schur_perturbation`). Tests: `crates/rcfd-fem/tests/sa_gyro_sector_split_schur.rs` (5 tests, one of them the ignored known
miss; about 4 s together), `sa_gyro_sector_split_schur_default_bits.rs`. After the run the test of G1, G2 and G4 was renamed
`g1_g2_g4_acceptance_limits_known_miss` and ignored with the measured numbers (commit 326b3419); its limits are unchanged.

`omega_ref = 1655.3226` rad/s.

| Id | Check | Limit | Measured | Met |
|---|---|---|---|---|
| G1 | matches (MAC `>= 0.99`, distinct, stable), every index and speed | each | holds | yes |
| G1 | `|perturbation/dense - 1|` where the dense `schur_eta > 1e-9`; count | `<= 0.01`; `>= 30` | **0.3835** (16 of 34 within 1e-2); 34 | **no** (predicted, D4) |
| G2 | the same where the perturbation `schur_eta > 1e-9` | `<= 0.01` | **0.3835** (16 of 34) | **no** (predicted) |
| G3 | dense run, `|theta'/omega - 1|`, 48 matched modes | `<= 1e-7` | 1.117e-8 | yes |
| G4 | perturbation run, the same | `<= 1e-7` | **7.154e-7** (43 of 48 within 1e-7) | **no** (predicted) |
| G5 | `theta'` and `schur_eta` consistent bit for bit, both runs, every split mode | each | holds (8 of 8 cases) | yes |
| G6 | library D1 against the test-side dense D1 where the latter `> 1e-9`; count | `<= 1e-3`; `>= 30` | 1.102e-7; 34 | yes |
| G7 | wall time, perturbation over dense, 8 B4 runs | `<= 0.5` | 0.136 (0.108 s against 0.792 s) | yes |
| G8 | `None` without `corrected_frequency` (both paths), without `split_correction`, without `coriolis`; lengths | each | holds | yes |
| G9 | B41: `schur_eta == Some(0)`, `theta' == theta` bit for bit | each | holds (both speeds, every index) | yes |
| G10 | DSS at `10 omega_ref`: `None` exactly where `eta` is `None` | each | holds (all four indices unstable, every entry `None`; the "Some otherwise" half is vacuous, as in ADR 0492 F8) | yes |
| G11 | 4 digests of f168d994; ADR 0481 (13), 0482 (6), 0492 (6) default bits and `sa_gyro_sector_schur`; `baseline_hashes`; `cargo test -p rcfd-fem --release` | `==`; pass | 4 of 4 equal; 13, 6, 6 equal; `sa_gyro_sector_schur` 4 passed; `baseline_hashes` 3 passed, 1 ignored; `rcfd-fem` 890 passed, 0 failed, 253 ignored | yes |
| G12 | perturbation and dense runs against the default: every other field equal, the dense `schur_eta` equal | `==` | holds (8 of 8 cases) | yes |
| N1 | first order alone, `max |eta_1/schur_eta - 1|` | `> 0.1` | 0.6185 | as expected |

Records: the measured G1 ratio differs from the prediction `-(delta_2/delta_1)^2` of D4 by at most 0.094 (the prediction is the
leading term of the remainder only); the Pade [0/1] value `delta_1/(1 - delta_2/delta_1)` (test-side) is within 0.0989 of the
dense `schur_eta` on the scope of G1. The perturbation `schur_eta` of the unmatched split modes carries no `None`. Wall time of
`split_modes` per index and speed is in `run1.log`.

### Mutations (scratch copy `f584-scratch/mut` of 326b3419, own target folder, script `f584-scratch/mutate.py`, logs `mut_MP*.log`)

| Id | Mutation | Caught by |
|---|---|---|
| MP1 | mirror terms dropped | G6 (0.151) |
| MP2 | denominators `l_a - l_b` sign flipped | G6 (0.763) |
| MP3 | `s_b` replaced by `s_a` | G6 (0.0271) |
| MP4 | gyroscopic term removed from the complement solve | G6 (1.07e3) |
| MP5 | complement projection removed from the solve | G6 (1.000) |
| MP6 | `delta_2` omitted | G6 (0.381) |
| MP7 | the perturbation path computing the dense estimate | G6 (0.622), G7 (1.02) |
| MP8 | `theta'` reported as `theta` | G3 (4.50e-5), G5 |
| MP9 | default `split_schur = Perturbation` | G12 (every case). G11 was not reached: cargo stops at the first failing test binary, and `sa_gyro_sector_split_schur` runs first |
| MP10 | the term `b = a` not skipped | G6 (NaN, through the finite requirement) |

All ten caught. The script's first pass stopped at MP5 on a wrong count of the projection calls it replaces (10, not 8): a
defect of the script, not of the mutation; the count was corrected and MP5 to MP10 run once.

### Deviations and what was not run

- No amendment; no limit and no existing test changed. The registered run was made once.
- `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets --release -- -D warnings` clean; `cargo test -p
  rcfd-cli --release --test docs_site` 2 passed. The whole workspace suite was not run (the lead's gate).

### Diagnosis of the miss

As predicted in D4. The second-order sum is computed correctly (G6 against an independent dense computation: 1.1e-7), but the
frozen condensed problem is far from second order on this basis: the poor Ritz pairs of the top half of the enlarged basis are
shifted by `E` by up to 1.2 times their own frequency, so the series ratio `delta_2/delta_1` reaches 0.62 and the truncation
leaves about `(delta_2/delta_1)^2` of the shift. No form built from the single column `E y_a` reaches 1 percent here (probe 2).
The corrected frequency of the ADR 0492 path meets a4 (1.1e-8); that of the perturbation path does not (7.2e-7).

### Decision

**Partial** by the registered rule: G1, G2 and G4 missed (kept as `g1_g2_g4_acceptance_limits_known_miss` with their limits),
every other check met, N1 as expected, all ten mutations caught. Clause by clause: a1 met (`corrected_frequency`, opt-in); a2 met
as a computation (one complement solve per split mode, the second-order sum, verified against an independent computation, a
factor 7 cheaper); a3 missed (0.38 against 0.01); a4 met for the root of ADR 0492, missed for the perturbation estimate; a5 met.

### Remaining questions

1. The cheaper estimate on this basis. Options: (a) keep `SplitSchur::Perturbation` as built, documented as a second-order
   estimate that is reliable only where `delta_2/delta_1` is small; (b) a follow-up that reports `delta_2/delta_1` beside it (a
   cheap indicator of its own accuracy) and falls back to the dense path above a registered threshold; (c) drop the option.
   Recommendation: (b); kept as (a) here.
2. Case-file key for `split_schur` and `corrected_frequency`: none added (as ADR 0492 question 3). Recommendation: none until a
   fan-sector use asks for one.

## For the lead: roadmap entries

Proposed status of F-584: **partial**. New note:

> Structural solver, advanced tier (plan ADR 0310). PARTIAL. ADR 0527, 2026-11-14: opt-in RotatingOptions::corrected_frequency reports per split mode the corrected frequency theta' beside schur_eta (on the default path the root of the dense condensed solve of ADR 0492), and RotatingOptions::split_schur(SplitSchur::Perturbation) computes schur_eta with one complement solve per split mode (the column E y) and the second-order Rayleigh-Schroedinger sum over the other eigenpairs of the linearised projected pencil (the other split modes and the mirrors of all), theta' = theta + delta_1 + delta_2. Met: theta' of the ADR 0492 path equals RotatingSector::whirl_modes within 1.1e-8 for all 48 matched modes of the disc sector (limit 1e-7); the perturbation value equals an independent dense computation of the same sum within 1.1e-7 and costs 0.14 of the dense path; the results of ADR 0492 (and 0481, 0482) and every default keep their bits (4 new digests plus 25); all ten mutations caught. Missed (predicted at registration, kept as known misses): the perturbation schur_eta agrees with that of ADR 0492 only within 0.38 (16 of 34 within 1 percent) and its theta' is within 7.2e-7 of whirl_modes (43 of 48 within 1e-7): the poor Ritz pairs of the corrected basis make the series ratio delta_2/delta_1 reach 0.62, so the second-order truncation leaves about its square. Follow-up proposed in ADR 0527.

Follow-up (optional, number left to the lead): title "Accuracy indicator and fallback of the perturbation Schur Ritz error of the sector split modes", deps F-584.

> Every limit not stated here is registered in the ADR before the run. With SplitSchur::Perturbation each split mode reports the ratio delta_2/delta_1 of ADR 0527 and falls back to the dense computation of ADR 0492 where it exceeds a threshold registered from the records of ADR 0527; on the disc sector of ADR 0411 with 4 harmonic modes per index at 0.4 and 0.8 omega_ref the resulting schur_eta agrees with that of ADR 0492 within 1 percent where it exceeds 1e-9, the cost stays below half of the dense path, and the results of ADR 0527 keep their bits.
