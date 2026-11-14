# ADR 0482: F-569 registration: static correction vectors for the split modes of a sector index, and the Ritz error eta of every split mode

Status: **REGISTERED 2026-11-14, written before any code of F-569 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-569 was written in the repository. One probe program on the existing API of 44b6c812
was run in the scratch folder (`w49i-work/f569-scratch/zz_probe_f569.rs`, log `probe1.log`); it measures only the dependence of
the correction vectors (no split frequency, no error, no check of this registration) and is disclosed in full under "Arithmetic
before this registration". Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-569 (deps F-555), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0481](0481-f555-registration-split-campbell-of-a-cyclic-sector-on-the-modal-subspace.md)
(remaining question 2). Number 0482 given by the lead. Branch `f569-work` from 44b6c812, worktree `w49i-work/f569`, scratch
`w49i-work/f569-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (the new behaviour is reached
only through a new option that is off by default); no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; no
existing test edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be
asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default
result, a golden or a hash table.

## What F-569 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The basis of RotatingSector::split_modes optionally adds the static correction vectors K_eff^-1 G phi_i of the F-396 harmonic modes of each index and reports the first-order Ritz error eta of ADR 0481 for every split mode; on the disc sector of ADR 0411 with 4 harmonic modes per index at 0.4 and 0.8 omega_ref the lowest split frequency of every index equals RotatingSector::whirl_modes within 1e-5, and eta agrees with the measured error within 10 percent where it exceeds 1e-9; without the option every result bit-identical.

Clauses: **a1** an option of `RotatingSector::split_modes` adds the static correction vectors `K_eff^-1 G phi_i` of the F-396
harmonic modes of each index to the basis; **a2** with it, the first-order Ritz error `eta` of ADR 0481 is reported for every
split mode; **a3** on DS of ADR 0411 with 4 harmonic modes per index (B4) at `0.4` and `0.8 omega_ref` the lowest split frequency
of every index equals `RotatingSector::whirl_modes` within 1e-5; **a4** on the same runs the reported `eta` agrees with the
measured error within 10 percent where it exceeds 1e-9; **a5** without the option every result bit-identical. The brief of the
lead adds: probe the dependence of the correction vectors before registering and register how dependent vectors are dropped;
reuse the orthogonalisation of ADR 0464 and ADR 0474 (`damped::m_orthonormalise`); a default-bits test recorded at 44b6c812
before any library change; `baseline_hashes` unchanged; no case-file key (recommended in the addendum).

## What was read (44b6c812, read only)

ADR 0481 whole (D1 to D5, in particular the rule for `eta` in D5, the set-ups DS, DSS, the addendum and its record `err/eta` in
`[0.968, 1.013]`); its registered log `f555-scratch/sa_gyro_sector_run1.log` (the per-mode `eta` and errors of B4 without
correction: the lowest mode of each index at `0.4 omega_ref` has errors 4.65e-6 (k = 0), 6.25e-6 (k = 1), 1.04e-5 (k = 2),
2.25e-5 (k = 3); at `0.8 omega_ref` 1.78e-5 (k = 0), 2.44e-5 (k = 1) and of the same order for k = 2, 3; the modes outside B4 of
index 0 lie above 1.2e4 rad/s, the lowest mode near 1.9e3 to 2.4e3 rad/s); ADR 0464 D1 (static residual vectors, classical
Gram-Schmidt twice in the M inner product, `RESIDUAL_DROP = 1e-8`); ADR 0474 (the shared `damped::m_orthonormalise`); the
source files `gyro.rs` (whole), `damped.rs` (`m_orthonormalise`, `RESIDUAL_DROP`), `rotating.rs` (`RotatingOptions`,
`RotatingSector::modes`: the reduced `K_eff`, `M` of the index and the transformation stored in `HarmonicModes`),
`cyclic.rs` (`HarmonicModes`, `doubled_matrix`), `mpc.rs` (`Transformation::expand`, `transpose_times`, `reduce`,
`independent`: `q_i = x_(independent()[i])`), `rcfd-linalg/src/sparse_chol.rs` (`SparseLdlt`); the tests `sa_gyro_sector.rs`
(set-ups, the reference `eta` of D5) and `sa_gyro_sector_default_bits.rs` (the digest pattern).

Published sources, cited from memory, **not read** for this work: J. M. Dickens, J. M. Nakagawa and M. J. Wittbrodt, A critique
of mode acceleration and modal truncation augmentation methods for modal response analysis, Computers and Structures 62 (1997)
985 (static correction vectors appended to a modal basis); G. L. G. Sleijpen and H. A. van der Vorst, A Jacobi-Davidson
iteration method for linear eigenvalue problems, SIAM J. Matrix Anal. Appl. 17 (1996) 401 (the projected correction equation);
C. C. Paige and M. A. Saunders, Solution of sparse indefinite systems of linear equations, SIAM J. Numer. Anal. 12 (1975) 617
(MINRES with a positive definite preconditioner); L. Meirovitch, Computational Methods in Structural Dynamics (Sijthoff and
Noordhoff 1980), for gyroscopic Ritz bases; the sources of ADR 0481 D5 for the Rayleigh functional. Nothing numeric is taken from
them; every formula used is derived below.

## Design

Notation of ADR 0481: index `k`, multiplicity `mu` (1 or 2), the reduced coordinates `q` of the rows `T_k` (`x = T_k q`, `x` the
expanded `(cos, sin)` vector of length `mu n`), `K_r = T_k^T K_eff,2(Omega) T_k`, `M_r = T_k^T M_2 T_k`, `C_r = T_k^T C_2 T_k`
(`C_2` the doubled Coriolis matrix per unit speed, `G = 2 Omega C`), all of order `n_q`; `Q(w) = K_r - w^2 M_r + i w G_r`
(Hermitian); the F-396 harmonic modes of the index `phi_i = T_k q_i`, `i = 1 .. m`, `m = m_b mu`, `M_r`-orthonormal.

### D1. The static correction vectors (a1)

For a kept mode `phi_a` the exact whirl mode near it has, to first order in `G`, the part outside the basis `e = sum_out b_j
phi_j`, `b_j = -i omega G_ja/(w_j^2 - omega^2)` (ADR 0481 D5). Its static limit (`omega^2 << w_j^2`) is `-i omega sum_out
phi_j G_ja / w_j^2`, which is the out-of-basis part of `-i omega K_r^-1 G_r q_a` (the modal expansion `K_r^-1 = sum_j q_j q_j^T /
w_j^2`). So `z_a = K_r^-1 C_r q_a` (the speed scale `2 Omega` and the factor `-i omega` dropped: the span is the same for `Omega >
0`, and at `Omega = 0` the vector is still defined) carries the leading part of every error component; with it in the basis the
remaining part of `b_j` is `-i omega G_ja omega^2/(w_j^2 (w_j^2 - omega^2))`, a factor `omega^2/w_j^2` smaller, so `eta` falls by
about `(omega^2/w_out^2)^2` (modal truncation augmentation, Dickens et al., from memory; derived here). This is the vector
`K_eff^-1 G phi_i` of the acceptance, computed in the reduced coordinates of the index so that it satisfies the cyclic rows.

The computation, for the basis of the index (the speed basis or the rest basis of ADR 0481 D1):

1. `q_a = x_a[independent]` for each kept mode (`x_a` the expanded basis vector; exact, since `T_k` has identity rows at the
   independent unknowns, `Transformation::independent`). A transformation with reference unknowns of rigid sets
   (`n_unknowns() != n_mesh()`) is refused with the option (`InvalidValue`, key `"constraint"`: the expanded basis does not
   carry the reference unknowns).
2. `C_r = T_k^T C_2 T_k` by `Transformation::reduce` (general, not only symmetric); `K_b` = the reduced stiffness stored with the
   basis (`HarmonicModes::reduced_stiffness`: `K_r` at the speed for the speed basis, `T_k^T K_2 T_k` at rest for the rest basis,
   where `K_r` at the speed is not positive definite), factored once by `SparseLdlt` (a failed factor is a `FemError::Solver`,
   solver `"sparse_ldlt"`).
3. For `a = 1 .. m` in the order of the modes: `z_a = K_b^-1 C_r q_a`, then **`damped::m_orthonormalise(M_r, z_a, [q_1 .. q_m,
   z kept so far])`** (ADR 0464 D1 step 3, ADR 0474: classical Gram-Schmidt in the `M_r` inner product made twice, then the
   `M_r`-norm; reused unchanged, `damped.rs` not edited).
4. **Drop rule (registered):** `z_a` is dropped when its `M_r`-norm before is zero or not finite, or after the two passes is at
   most `RESIDUAL_DROP = 1e-8` times before (the rule of ADR 0464, unchanged); kept vectors are `M_r`-normalised. The counts of
   kept and dropped vectors are reported.
5. The basis of the projection is `[phi_1 .. phi_m, T_k z_kept]` (expanded, the correction vectors appended after the modes);
   `project_and_solve` of ADR 0481 (the same doubled `K_eff`, `M`, `C`) gives the split modes on it. Everything else of ADR 0481
   D1 (stability, whirl number, wave sense, real roots, shapes) is unchanged.

### D2. The Ritz error of every split mode (a2)

ADR 0481 D5 defines, for an exact whirl mode `phi` at `omega` and its `M`-projection `s` on the basis, `e = phi - s`,

`eta = |e^H Q(omega) e| / (omega |s^H (2 omega M - i G) s|)`,

which needs the exact mode. The reported value uses only the split mode `(theta, psi)` of the basis `V` (`psi = V y`, reduced
coordinates; `V` the `M_r`-orthonormal reduced basis of D1): with `P = V V^T M_r` (the `M_r`-orthogonal projector on `V`),
`r = Q(theta) psi` and the complement `W = range(I - P)`, the complement part `t` of the exact mode solves, to first order,

`(I - P)^T Q(theta) (I - P) t = -(I - P)^T r`, `t = (I - P) t`

(derivation: `Q(omega)(s + e) = 0` tested with the complement gives `A_c(omega) e = -(I - P)^T Q(omega) s`, `A_c = (I - P)^T Q (I -
P)`; replacing `(omega, s)` by `(theta, psi)` changes `e` at second order, since `theta - omega` is `O(eta omega)` and `psi - s`
is of second order in the coupling; Sleijpen and van der Vorst's correction equation, from memory). Then `e^H Q(omega) e = e^H
A_c e ~ t^H A_c(theta) t = t . b` (`b = -(I - P)^T r`, the real inner product of the real form below), and the **reported Ritz
error** is

`eta = |t . b| / (theta |2 theta psi^H M_r psi + 2 psi_r^T G_r psi_i|)`

(`psi^H (2 theta M - i G) psi` in real arithmetic as ADR 0481 D5; `G_r = 2 Omega C_r`), equal to ADR 0481's `eta` to first order.
When the complement is empty (`m + p = n_q`, the basis is the whole space of the index) `eta = 0` exactly, without a solve.

**Solve.** In the real form `u = (u_r, u_i)` of length `2 n_q`, `Q(theta)` is the real symmetric `[[A, -theta G_r], [theta G_r,
A]]`, `A = K_r - theta^2 M_r` (symmetric since `G_r^T = -G_r`). The projected system is solved by MINRES (Paige and Saunders, from
memory; real arithmetic, the recurrence written out in the code) with the positive semi-definite preconditioner `B = (I - P) K_b^-1
(I - P)^T` on each half (the factor of D1 reused), from `t = 0`, stopped when the preconditioned residual estimate `phibar <=
RITZ_TOLERANCE beta_1`, `RITZ_TOLERANCE = 1e-10`, or after `min(4 n_q + 20, 2000)` iterations. MINRES is chosen over CG because
`A_c` is indefinite for the split modes above the lowest complement frequency (the top half of the enlarged basis).

**Reporting (registered):** with the option, `HarmonicSplitModes::eta[i]` is `Some(eta)` for every split mode at a speed whose
stability flag is `Stable`; `None` when the speed is `Unstable` (the real-frequency functional does not apply to a growing root)
or the MINRES did not meet its tolerance. Without the option every entry is `None`.

### D3. Option, fields and bits (a5)

`RotatingOptions` gains `split_correction: bool` (default `false`, builder `split_correction(on)`; `rotating.rs`, a field and a
builder, nothing else). It acts only in `RotatingSector::split_modes` and `split_campbell` with `coriolis` on (without
`coriolis` they return the F-396 results of ADR 0481 D2 whatever the option). `HarmonicSplitModes` (non-exhaustive) gains
`corrections: usize` (kept vectors), `dropped_corrections: usize` and `eta: Vec<Option<f64>>` (parallel to `modes.modes`); without
the option `0`, `0` and `None` for every mode, and every other field keeps the bits of 44b6c812, because the code path without the
option executes the same operations in the same order (B1 proves it with digests recorded at the base). `SplitModes::basis_size`
is `m + p` with the option. `split_campbell` with both options tracks the split modes of the enlarged bases (its family table
holds the modes; `eta` is not stored in the table). The non-cyclic `RotatingPart` paths and the F-396, F-499 paths are not
touched. Case-file key: none (the maintainer's call; recommendation in the addendum).

## Set-ups (fixed now)

As ADR 0481: steel DS (`N = 6`, Hex20 on `hex_box([0.25, 0, -0.1], [0.75, 2 pi/6, 0.2], [2, 1, 1])` mapped to the annulus,
`xmin` clamped, `Cyclic("ymin", "ymax", e_z, 0, 6)`, both effects on), modal options `ModalOptions::new(m_b).tolerance(1e-9)`,
400 iterations, `RotatingOptions::new(m_b).modal(..).coriolis(true)`, with `.split_correction(true)` where the option is on.
Bases **B4** (`m_b = 4`), **B12** (records only) and **B41** (`m_b = 41`). `omega_ref` = the lowest rest whirl frequency over `k
= 0 .. 3` of the B4 sector (1655.3226 rad/s, ADR 0481). Speeds `0.4` and `0.8 omega_ref`. Reference `RotatingSector::whirl_modes
(k, Omega)` of the sector with the same `m_b` (the lowest `m_b mu` whirl modes). **DSS**: DS with stress stiffening off, B4, at
`10 omega_ref` (ADR 0481 L1: every index loses stiffness, rest basis). **DSR** (refusal): DS with `Constraint::rigid_set("xmax",
[0.75, 0, 0.05])` added.

**Matching (as ADR 0481 D5):** each reference mode is matched to the split mode of the same index and speed with the largest
complex MAC (ties: the lower split mode); the match must have MAC `>= 0.99` and the matches of one index and speed must be
distinct. Measured error `err = |theta/omega - 1|` of the match.

## Arithmetic before this registration (disclosed)

1. **Probe** (`f569-scratch/probe1.log`, the program `zz_probe_f569.rs` on the API of 44b6c812, run in the worktree's test
   folder and moved to the scratch folder, never committed): for DS at `0.4` and `0.8 omega_ref`, every index, the vectors
   `K_r^-1 C_r q_a` orthogonalised by the rule of D1 (a copy of the loop of `m_orthonormalise`): ratio after/before of the
   `M_r`-norm. **B4**: smallest ratio 1.52e-3 (k = 0), every index above 1.0e-2 otherwise; no vector dropped; the two members of
   a pair give equal ratios (independent vectors). **B12**: smallest 3.2e-5; none dropped. **B41** (`n_q = 42` or `84`, `41 mu`
   modes): the first 1 (single) or 2 (pair) vectors kept with ratios 8.6e-7 to 1.6e-5, every other ratio `<= 7.1e-30` (the basis
   already spans the whole space): dropped; the kept counts equal `n_q - 41 mu`, so the enlarged B41 basis is the whole space
   of the index. Largest `|V^T M_r V - I|` of the enlarged bases 8.0e-15. `q_a = x_a[independent]` reproduces `x_a` exactly
   (`expand` deviation 0). The rest-basis transformation of index 1 has `n_unknowns = n_mesh = 192` (no reference unknowns).
   The drop rule separates the probe's ratios by more than 20 orders of magnitude on B41 and by five on B4 and B12.
2. **Accuracy of the lowest mode with the correction (a3).** Without correction the lowest modes of B4 have errors 4.65e-6 to
   2.5e-5 (ADR 0481 run log), the modes outside B4 lie at `w_out >= 1.2e4` rad/s, the lowest modes at `omega <= 2.5e3` rad/s.
   By D1 the remaining error is about `eta_0 (omega^2/w_out^2)^2 <= 2.5e-5 x (2.5e3/1.2e4)^4 = 4.7e-8`, plus the second-order
   coupling through modes outside the basis (of the order `eta_0^2`, 6e-10) and the rounding of the reduced solve (`<= 3.4e-11`,
   ADR 0472 amendment 1). Estimate `<= 1e-7` against the limit 1e-5.
3. **Agreement of eta (a4).** The reported `eta` equals ADR 0481's to first order (D2); ADR 0481 measured `err/eta` in `[0.968,
   1.013]` for `1e-10 <= eta <= 1e-3`. With the correction the lowest modes have `eta ~ 1e-8` (item 2), where the rounding of the
   reduced solve (3.4e-11) and of the reference (3e-13) is below 0.4 percent of the error; at `eta = 1e-9` it is 3.4 percent.
   The top reference modes of an index lie closer to the complement spectrum, where the neglected higher-order terms grow;
   estimate `|err/eta - 1| <= 0.05` over the scope of E2, against the limit 0.1.
4. **MINRES.** The preconditioned operator on the complement has eigenvalues `1 - (theta^2 + O(theta |G|))/w^2` over the
   complement Rayleigh quotients `w^2 >= w_out^2`; for the lowest modes they lie in `[0.95, 1]`, convergence to 1e-10 in a few
   tens of iterations; the order of the real system is at most 168 on DS.
5. **Raw skew defect** of the projected `C` on the enlarged basis: as ADR 0481 (1.5e-15 measured), estimate 1e-15 against 1e-11.

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_gyro_sector_correction.rs` (all but B1) and
`crates/rcfd-fem/tests/sa_gyro_sector_correction_default_bits.rs` (B1), release profile.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| E1 | a1, a3 | DS B4 with the option, `k = 0 .. 3`, `0.4` and `0.8 omega_ref`: `|theta_1/omega_1 - 1|`, `theta_1` the lowest split frequency (`modes.modes[0].omega`), `omega_1` the lowest of `whirl_modes(k, Omega)`; and the MAC of the two modes | `<= 1e-5`; MAC `>= 0.99` | `<= 1e-7` (arithmetic 2) |
| E2 | a4 | the same runs: every reference mode matched (matching rule above: MAC `>= 0.99`, distinct); for every match whose reported `eta` exceeds 1e-9, `|err/eta - 1|` | matched; `<= 0.1` | `<= 0.05` (arithmetic 3) |
| E3 | a2 | the same runs: every split mode has `eta = Some(v)` with `v` finite and `>= 0` | each | D2 (stable speeds) |
| K2 | a1 | the same runs: basis `Speed`, no loss, `Stable`, no real roots, `corrections = 4 mu`, `dropped_corrections = 0`, `basis_size = 8 mu`, `8 mu` split modes | each | probe (arithmetic 1) |
| D1 | a1 | DS B41 with the option at both speeds, every index: `corrections = n_q - 41 mu` (1 or 2) and `dropped_corrections = 41 mu - corrections` (the drop rule); every `eta == Some(0.0)` (empty complement); every reference mode matched and `max |theta/omega - 1|` | each; `<= 1e-9` | probe; the whole space, reduced-solve rounding `<= 3.4e-11` |
| G3 | a1 | raw skew defect of the projected doubled `C` on the enlarged basis, every run of E1 and D1 | `<= 1e-11` | 1e-15 |
| C2 | a1 | DS B4 with both options, `split_campbell(&[0, 1, 2, 3], {0, 0.2, 0.4, 0.6, 0.8} omega_ref)`: 4 families, tracks `8, 16, 16, 8`; at every speed each stored frequency of a track equals by bits a frequency of `split_modes(k, Omega_j)` with both options; each track's stored frequencies distinct per speed; every speed `Stable` | each | D3 |
| L2 | a1, a2 | DSS at `10 omega_ref` with both options, `k = 0 .. 3`: basis `Rest` with the loss of `RotatingSector::modes(k, Omega)`; `corrections >= 1`; every root finite; `eta[i]` is `None` for every mode exactly when the speed is `Unstable`, `Some` finite otherwise | each | D1, D2 |
| B1 | a5 | digests recorded at 44b6c812 by `sa_gyro_sector_correction_default_bits.rs` (written with the API of that commit, committed before any library line): DS B4 with `coriolis` on, sector `split_modes(k, f omega_ref)` for `k = 0 .. 3`, `f = 0.4, 0.8` (omega, growth, whirl, residual, waves, roots, real roots, skew defect, asymmetry, basis size, the first shape's `re` and `im`); the same with `coriolis` off (eigenvalue, frequency, shape); DS B4 `split_campbell` with `coriolis` on over the C2 speeds (eigenvalues, MAC, whirl numbers, waves, growth, stability); DSS `split_modes(k, 10 omega_ref)` (roots, growth, basis, loss): all equal after the change; ADR 0481's `sa_gyro_sector_default_bits` passes; `cargo test -p rcfd-api --release --test baseline_hashes` passes; `cargo test -p rcfd-fem --release` passes | `==`; pass | holds by construction (D3) |
| B3 | a5 | `split_correction(true)` with `coriolis` off on DS B4: `split_modes(k, 0.4 omega_ref)`, `k = 0 .. 3`, and `split_campbell` over the C2 speeds equal by bits those without the option; `corrections = 0`, every `eta` `None` | identical | D3 |
| X2 | D1 | DSR with both options: if `RotatingSector::new` and `RotatingSector::modes(1, 0.4 omega_ref)` accept the model, `split_modes(1, 0.4 omega_ref)` is refused with key `"constraint"` and without the correction option it is not refused by that key; if the sector refuses the model the path is unreachable and that is recorded (the check then holds vacuously) | as stated | D1 step 1 |

Records (reported, not in the decision rule): per index and speed of B4 and B12 the errors and reported `eta` of every matched
mode, the ratio `err/eta` (min, max) over `eta > 1e-9` and over `1e-10 < eta <= 1e-9`, the MINRES iteration counts, ADR 0481's
`eta` of the reference mode on the plain basis against the measured error on the enlarged one; B12 the lowest-mode errors;
the worst error over all matched modes of B4 with and without the option; the split modes of the top half of the enlarged
basis (not matched: their `eta`).

## Negative controls and mutations

| Id | Control | Expected |
|---|---|---|
| N1 | DS B4 without the correction (ADR 0481 path) at `0.8 omega_ref`: largest `|theta_1/omega_1 - 1|` over the indices | `> 1e-5` (1.78e-5 and 2.44e-5 for k = 0, 1 in ADR 0481's run log) |
| N2 | DS B4 at `0.8 omega_ref`, every index: the reported `eta` of the lowest split mode with the option against the measured error of the lowest split mode without it (ADR 0481 path) | `eta < 0.1 err_plain` for every index (the correction lowers the error by more than ten, arithmetic 2) |

Mutations (on a scratch copy of the worktree, each run once against `sa_gyro_sector_correction`; each must be caught by a
registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the correction vectors computed but not appended to the basis | E1 (k = 0, 1 at 0.8), K2 |
| MU2 | the load `M_r q_a` instead of `C_r q_a` (every vector in the span of the modes: dropped) | K2, E1 |
| MU3 | the complement projection of the eta solve removed (`P = 0` in `b` and the operator) | E2 or E3 |
| MU4 | the gyroscopic term removed from the denominator of `eta` (`2 theta psi^H M psi` only) | E2 (pair modes, whirl numbers up to 0.8) |
| MU5 | the drop rule disabled (every vector kept after normalisation) | D1 |
| MU6 | the correction vectors normalised but not `M_r`-orthogonalised (the projector of D2 then wrong) | E2 |
| MU7 | the correction vectors added also without the option (the option-off path through D1) | B1 |

## Decision rule

**done**: every check met at its registered run (E1 to X2), N1 and N2 as expected, every mutation MU1 to MU7 caught, no amendment
after a run. **partial**: a check missed (kept as a `*_known_miss` with its limit), N1 or N2 not as expected, a mutation MU1 to MU7 not caught, or a check
met only after an amendment made after its run. **todo**: the module does not build or the DS runs cannot be made.

## Limits (not covered)

- The reported `eta` is a first-order estimate, validated against the reference on one disc geometry; it is not a bound. For the
  split modes of the top half of an enlarged basis it is reported but not validated.
- One correction vector per kept mode, of first order in `G`; no second-order vectors `K^-1 C K^-1 C q`, no load-dependent
  vectors.
- The rest basis at a loss of stiffness uses the rest stiffness for the corrections; L2 checks the path, not its accuracy.
- Sectors whose transformation has rigid-set reference unknowns are refused with the option.
- No case-file key, no CLI output; the 120 mm fan sector is not run here.
- The MINRES solve costs one factor of `K_b` per index and speed (shared with D1) and two triangular solves per iteration and
  split mode; on a large sector the iteration counts are not measured here.
