# ADR 0405: F-494 registration: the single-corrector growth of the immersed bounded forcing over several steps

Status: **REGISTERED 2026-11-13, written before any code of F-494 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-494 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand from numbers already in the repository's
records. Results go in the addendum at the end; a defective set-up is corrected by an amendment written and committed before the
corrected run; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0392](0392-f482-registration-immersed-step-linearisation.md) (F-482: the state vector D1, the map and its central
difference D2, Arnoldi D3, the complex Hessenberg QR D4; the one-step spectra K5, K10, K15, F15, C15, H30; question Q2 from
which F-494 was made), [ADR 0374](0374-f363-registration-immersed-flux-with-one-piso-corrector.md) (F-363: the set-up R, R0: B
with one corrector stops at step 34 at `dt = 1e-3`, F runs 200 steps with `max abs(u)` 2.210; G1 the alternating growth),
[ADR 0388](0388-f483-registration-pressure-lag-estimate-at-an-immersed-body.md) (F-483: the frozen pressure-lag map; C2 the
channel without the cylinder at 0.788). Roadmap item F-494 (deps F-482). Branch `f494-immersed-multistep`, worktree
`w49i-work/f494`, scratch `w49i-work/f494-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded; each registered run is made once; defaults stay
bit-identical; no hash table, golden or existing test file is edited (the helpers and the linear algebra of
`immersed_step_linearisation_f482.rs` are copied into the new file, that file is not edited); no new third-party crate, no
unsafe code; unsigned commits without trailers; no push.

Clean-room: the step is rcfd's own PISO (Issa, J. Comput. Phys. 62 (1986) 40-65, as written in `pimple.rs` and its ADRs; cited
from memory, nothing of it used here). The linearisation of a composition of maps by the chain rule, `D(S_k o ... o S_1)(x_0) =
J_k ... J_1` with `J_j` the Jacobian of the step at the `j - 1`-th state of the trajectory, and its central difference, are
first principles. Arnoldi, the Hessenberg QR and inverse iteration are those of ADR 0392 D3, D4 (Golub and Van Loan; Saad;
cited from memory, written from their definitions and checked on known spectra). That `||A V_m y|| = ||Hbar_m y||` for the
Arnoldi relation `A V_m = V_{m+1} Hbar_m`, so that the largest singular value of the `(m + 1) x m` matrix `Hbar_m` is a lower
bound of that of `A`, is derived below from the orthonormality of `V_{m+1}`. No vendor code, text, manual or binary.

## What F-494 asks

Acceptance (`docs/roadmap.json`), binding, clause by clause:

> The growth of DFG 2D-2 on M2 with build_second_order_bounded and one PISO corrector at dt 1e-3 (ADR 0392: the one-step
> linearisation at step 15 has eigenvalues up to 1.93 for the bounded form and 1.58 for the first-order forcing, which runs,
> and a period-two pair of magnitude 1.16 for the bounded form) is measured by a registered finite-difference linearisation of
> k >= 10 consecutive steps along the trajectory, whose dominant eigenvalue per step is above one for the bounded form and below
> one for the first-order forcing; and the one-step eigenvalues above one of the first-order forcing are explained by a
> registered measurement (the channel without the cylinder, the state of the step).

1. *a registered finite-difference linearisation of k >= 10 consecutive steps along the trajectory* (D1, D2; the fast checks U3
   and U4, the linearity check G0);
2. *dominant eigenvalue per step above one for the bounded form* (G1 a, b);
3. *below one for the first-order forcing* (G2; G3 the same over 20 steps);
4. *the one-step eigenvalues above one of the first-order forcing explained by a registered measurement (the channel without the
   cylinder, the state of the step)* (X1, X2, X3; X4 printed).

**What "dominant eigenvalue per step" means here (taken on the maintainer's standing instruction of 2026-11-11, take the
recommendation).** For the product `P_k = J_k ... J_1` along the trajectory from the state after step `n0`, `mu` its eigenvalue
of largest magnitude (from Arnoldi on `P_k`, converged by the true residual of ADR 0392 D3), the eigenvalue per step is
`g = abs(mu)^(1/k)`. This is what the acceptance names and it is the registered measure. It is not a Lyapunov exponent: the
eigenvalues of a product of different matrices are not the products of their eigenvalues, and over a finite `k` they depend on
the window. Two finite-time growth measures are **printed**, not limited, beside it: (i) `sigma = s_max(Hbar_m)^(1/k)`, the
largest singular value of the Arnoldi matrix of `P_k`, a lower bound of the largest singular value of `P_k` in the norm of the
scaled state of ADR 0392 D1 (it depends on that scaling; `g` does not, for a fixed change of basis); (ii) the growth of one
perturbation propagated through the nonlinear steps with the settings of R, `|x'_n - x_n| / e` per step. Why `g` should
separate the two forcings is derived below; why one-step eigenvalues above one need not compound is illustrated by U4 (ii).

## What was read (disclosed)

ADR 0392 in full (registration, amendment 1, addendum 1); ADR 0388 in full; ADR 0374 (registration, amendments, addendum: R0,
G1, the max `abs(u)` of F at step 200); the roadmap entries F-494, F-482, F-483, F-363;
`crates/rcfd-fv/tests/immersed_step_linearisation_f482.rs` in full (the code copied here); `crates/rcfd-fv/src/ddt.rs`
`courant_numbers` and `max_courant` (cell Courant number `0.5 sum_f |phi_f| dt / V`, the empty faces skipped; public);
`pimple.rs` the use of `max_courant` in the guard at the end of the step; `rcfd-mesh` `Mesh::face_area_vectors`,
`cell_volumes`.

## Arithmetic before this registration (disclosed)

**B, the period-two mode.** ADR 0392 measured at the states of steps 5, 10, 15 a left eigenvalue of the one-step map of
magnitude 1.03, 1.27, 1.16, located at the body each time; ADR 0374 G1 measured the alternating amplitude growing by 1.3 to 1.5
per two steps from step 13 (1.15 to 1.22 per step). If a mode's eigenvector changes slowly along the trajectory, the product
has an eigenvalue close to the product of the one-step eigenvalues of that mode, so `g` of B over steps 16 to 25 is expected
**about 1.15 to 1.2**; even if the positive pressure modes (1.93 at step 15) compound, `g` of B is above one. Expected: `g > 1`
for B with a wide margin. Not predicted: whether the dominant of `P_10` for B is the period-two mode or a pressure mode (the
mode's shares and cell are printed).

**F, a bound from its run.** F with one corrector at `dt = 1e-3` runs 200 steps (ADR 0374 R0) with `max abs(u)` 2.21. The solves
of R stop at `tol_rel = 1e-6`, so each step leaves an error of relative size between about 1e-10 and 1e-6 of the state (the
measured `|S_R(0) - S(0)| / |S(0) - x_0|` of ADR 0392 is 3e-8 to 1.2e-7). A linear growth by `g` per step sustained over the 185
steps after step 15 would bring such an error to the size of the flow when `g^185 >= 1e6` to `1e10`, i.e. `g >= 1.077` to
`1.13`: so the bounded run of F implies that a growth per step above about 1.08 to 1.13 is **not sustained** along its
trajectory. The one-step eigenvalue 1.58 (ADR 0392) therefore cannot be the per-step growth of F over long windows. That `g`
of F is below one is **not** derived: the slow physical modes lie near `exp(sigma dt)`, and with the shedding instability of
the cylinder wake (a growth rate of order 1/s, from memory, not used) that is about 1.001 per step. ADR 0392 measured the
dominant one-step eigenvalue of the settings that run without a pressure mode (B at `5e-4`, 0.979; B with two correctors,
0.970), which suggests that the physical part of the spectrum at this state lies below one; that is the hope of G2, and G4
(two correctors) measures the physical part of `g` directly.

**Why one-step eigenvalues above one need not compound.** If `J_j` varies along the trajectory, `P_k` can have a spectral radius
far below the product of the spectral radii: two diagonal maps alternating `diag(1.5, 0.5)` and `diag(0.5, 1.5)` have spectral
radius 1.5 each, and their product `diag(0.75, 0.75)`, `g = 0.866`. U4 (ii) checks this on a known example. For a highly
non-normal `J` (the pressure modes of ADR 0392 have almost all their weight in `p`, which the next step reads only through the
gradient of the predictor) the eigenvectors are sensitive to the state, and a small change of state from step to step can turn
them.

**The one-step eigenvalues above one: the hypothesis HC.** ADR 0073 (and ADR 0374, ADR 0388) describe the pressure lag of one
corrector: the predictor solves `a_P u* - H(u*) = b - V grad p_n`, and the corrected velocity keeps `H(u*)/a_P`, which contains a
part of `grad p_n`. For a smooth pressure perturbation, with `m` the share of the neighbour coefficients in the diagonal
(`m = 1 - a_t / a_P` up to the sign structure, `a_t = 1.5 V / dt` the BDF2 time coefficient), the perturbation of the predictor
is `-(I - M)^{-1} rAU grad dp`, of which `H/a_P` keeps `M (I - M)^{-1}`, gain `m / (1 - m)`, above one where `m > 1/2`, i.e.
where the convective and diffusive coefficients exceed the time coefficient. On M2 at the core spacing `h = 0.0025`, `dt = 1e-3`:
`a_t = 1.5 h^2 dz / dt`; the convective part relative to it is about `C / 1.5` (`C` the cell Courant number), the diffusive part
`4 nu dt / (1.5 h^2) = 0.43`. In the channel without the cylinder (`C` about 0.6 at `U_m = 1.5`) this gives `m` about 0.45 and a
gain about 0.8 (ADR 0388 C2 measured the frozen lag map there at 0.788); over the cylinder the flow is faster (`max abs(u)` 2.2,
`C` about 0.9), `m` about 0.5 and the gain about 1. This crude estimate does not give 1.58 (the full step has the flux history,
the extrapolated convecting flux and the ddt flux correction besides), so **no number is predicted**; it gives the hypothesis
HC: *the one-step eigenvalues above one of F are those of the single-corrector step at the cell Courant number that the
cylinder makes by accelerating the flow, not of the immersed forcing.* Consequences, used as checks: the channel without the
cylinder at `U_m = 1.5` has none (X1); the channel without the cylinder at an inflow raised so that its largest cell Courant
number equals that of F at step 15 has one (X2); F at a later state, with the same Courant number, still has one (X3). The
reading the other way (a transient of the start field of ADR 0249, which the penalty removes in the first steps and which ADR
0388 found raising the frozen map at step 2) predicts none at step 100: X3 separates the two.

## Design (D1 to D4), all in a new test file

New file `crates/rcfd-fv/tests/immersed_multistep_linearisation_f494.rs`. Copied from `immersed_step_linearisation_f482.rs`
without change of behaviour: the complex type and the Hessenberg QR (D4 of ADR 0392), inverse iteration, Arnoldi with two
Gram-Schmidt passes, the Ritz values and vectors, the true residual, the start vector (seed 482), the DFG helpers and `r_solver`,
the state layout `Layout` (D1 of ADR 0392: `u_n`, `u_{n-1}`, `p_n`, `phi_n / |S|`, `phi_{n-1} / |S|`), `tighten` (momentum and
pressure `tol_rel = 1e-12`, `tol_abs = 0`), `fluid_cells`, `surface_distance`; the checks U1 and U2 of ADR 0392 are copied with
them (they test the copied code).

**D1. The k-step map.** `S^k(dx)`: `restore` of the perturbed state (D1 of ADR 0392) once, then `k` calls of `step(dt)` without
any restore in between (no stop allowed: a stop panics), then the packed state of the last snapshot. With `dx = 0` it is the
trajectory from `x_{n0}`. The map is that of the step with the converged solves (`tighten`), as in ADR 0392 D2; the base state
`x_{n0}` is reached with the settings of R.

**D2. The product and its linearisation.** `P_k v = (S^k(e v) - S^k(-e v)) / (2 e)`, `|v| = 1`, `e = 1e-4`. By the chain rule
this is `J_k ... J_1 v + O(e^2)`, `J_j` the Jacobian of the step about the `j - 1`-th state of the trajectory from `x_{n0}` (the
product along the trajectory, not the `k`-th power of `J_1`; U3 (c) checks the composition, U3 (d) prints the difference to the
power). The remainder: a perturbation grown by `G` along the window has a relative nonlinear remainder of order `(e G)^2`; for
`G` up to 100, `1e-4`. The solve noise of `1e-12` relative, divided by `2 e`, is about `1e-8` of the product per step.

**D3. Arnoldi on `P_k`.** As ADR 0392 D3: start `lcg_vector(n, 482)`, `m = 40`, two Gram-Schmidt passes, Ritz values from `H_m`
by D4. The dominant Ritz value `mu` (largest magnitude), its vector `z = V_m y`, the true residual `|P_k z - mu z| / (|mu| |z|)`
(one product, two for a complex pair); **converged** if at most 0.03 (as ADR 0392). `g = abs(mu)^(1/k)`. Why 40 is enough: the
product spreads the spectrum by the `k`-th power, so a dominant per-step 1.15 against a bulk at most 0.98 is, for `k = 10`, 4.0
against 0.82, an outlier at a relative distance far larger than the one-step one ADR 0392 resolved with 40 vectors; for `F`, a
dominant per-step `g` and the next at `0.99 g` are, for `k = 10`, 10 percent apart in `P_k`, against 1 percent in `J`.

The printed growth measures. `sigma`: `Hbar_m` is the `(m + 1) x m` Arnoldi matrix; with `P_k V_m = V_{m+1} Hbar_m` (exact up to
the finite difference) and `V_{m+1}` orthonormal, `|P_k V_m y| = |Hbar_m y|` for every `y`, so `s_max(Hbar_m) <= s_max(P_k)`.
`s_max(Hbar_m)` is the square root of the largest eigenvalue of `Hbar_m^T Hbar_m` (40 x 40, symmetric positive semi-definite),
computed by 4000 power iterations from the all-ones vector. The nonlinear propagation: from `x_{n0}`, the run with the settings
of R and the run from `x_{n0} + e v_1` (`v_1` the Arnoldi start, `e = 1e-4`, settings of R), `|x'_n - x_n| / e` for each of the
`k` steps (the solve noise of 1e-6 is then about `1e-2` of it: printed only).

For the dominant vector of `P_k`: the shares of `|z|^2` in the five blocks, the mode cell (largest `|z_u|^2` of `u_n`, whether a
first fluid cell, its distance to the surface), the cell of the largest `abs(p)` entry. All Ritz values with `abs(mu)^(1/k)`
above 0.9 are printed with their estimated residuals.

**D4. The channel without the cylinder and the Courant number.** `channel_solver(level, um)`: the set-up R of `r_solver` with
`dfg_fields(xs, ys, um)` and `two_d_two_start(.., um)` (the blob of amplitude 0.2 unchanged) and `immersed = None`; every cell is
fluid. The cell Courant number is the library's `rcfd_fv::ddt::courant_numbers(geom, phi, dt)`; `C_max` its largest value at the
state after a step, with its cell. The matched inflow of X2: `U_m' = 1.5 C_F / C_0`, `C_F` the `C_max` of F after step 15 and
`C_0` that of the channel at `U_m = 1.5` after step 15 (both measured in record X before the run of X2; a formula fixed here, not
a parameter chosen after a result).

## The runs

Set-up R of ADR 0374 / ADR 0392 (copied): DFG 2D-2 on M2 (`dfg_family(2)`), `U_m = 1.5`, `nu = 1e-3`, `K = 1e7`, the start field
of ADR 0249, `PimpleSettings::piso` with BDF2, `LinearUpwind`, AMG pressure, momentum and pressure `tol_rel` 1e-6, `dt = 1e-3`,
`flux_weight_check = false`, one corrector unless stated. The windows start from the state after step `n0 = 15` (the state of
ADR 0392 K15 and F15; the growth of B is under way from step 13, G1; B stops at step 34).

| Id | Forcing, correctors | Window | Role |
| --- | --- | --- | --- |
| PB | B, 1 | steps 16 to 25 (`k = 10`) | G0, G1 |
| PC | B, 2 | steps 16 to 25 (`k = 10`) | G4 (control: the physical part of `g`) |
| PF | F, 1 | steps 16 to 25 (`k = 10`) | G2 |
| PF20 | F, 1 | steps 16 to 35 (`k = 20`) | G3 |
| X1 | no cylinder, 1, `U_m = 1.5` | one step after step 15 | X1 |
| X2 | no cylinder, 1, `U_m = U_m'` (D4) | one step after step 15 | X2 |
| X3 | F, 1 | one step after step 100 | X3 |
| X4 | F, 1 | one step after steps 16, 20, 25 | printed: the one-step dominant along the window of PF |

The one-step linearisations X1 to X4 are those of ADR 0392 (`k = 1` of D1 to D3, `m = 40`).

## Registered checks

The heavy records carry `#[ignore]` and are run once by me in release: record P1 (PB, PC, R1), record P2 (PF, PF20, R2), record
X (X1 to X4). The fast checks run in release in the gate (U1, U2, U4 also in debug).

| Id | Clause | Check | Limit |
| --- | --- | --- | --- |
| R1 (reproduction; P1) | (set-up) | B, one corrector, from the snapshot of step 15 on with the settings of R | stops at step 34 by the Courant guard (ADR 0374 R0); otherwise a defective copy |
| R2 (reproduction; P2) | (set-up) | F, one corrector, from the snapshot of step 15 on with the settings of R to step 200 | no stop, `max abs(u)` at step 200 within 0.01 of 2.210 (ADR 0374 R0) |
| G0 (P1, P2) | 1 | for PB and PF: `P_k z` of the dominant vector with `e = 1e-4` against `e = 1e-5` (real part for a complex pair) | `<= 1e-3` relative, both |
| G1 a (P1) | 2 | PB: the dominant Ritz value of `P_10` converged | true residual `<= 0.03` |
| G1 b (P1) | 2 | PB: `g = abs(mu)^(1/10)` | `> 1` |
| G2 (P2) | 3 | PF: the dominant Ritz value of `P_10` converged and `g < 1` | true residual `<= 0.03` and `g < 1` |
| G3 (P2) | 3 | PF20: as G2 for `P_20` | as G2 |
| G4 (P1; control) | (control) | PC (two correctors): as G2 | as G2 |
| X1 (X) | 4 | the channel without the cylinder at `U_m = 1.5`, one step after step 15: no Ritz value with `abs(theta) >= 1` converged | every Ritz value with `abs(theta) >= 1` has a true residual above 0.03 |
| X2 (X) | 4 | the channel at `U_m'` (D4), one step after step 15: a converged Ritz value above one of positive real part | some Ritz value with `abs(theta) > 1`, `Re theta > 0`, true residual `<= 0.03` |
| X3 (X) | 4 | F one step after step 100: as X2 | as X2 |
| U1 (fast; copied) | (tool) | ADR 0392 U1 on the copied QR | as there (1e-10) |
| U2 (fast; copied) | (tool) | ADR 0392 U2 on the copied Arnoldi | as there |
| U3 (fast) | 1 | DFG M1, B, one corrector, `dt = 1e-3`, the state after step 2, `k = 3`: (a) `S^3(0)` with the settings of R gives the bits of `u`, `p`, `phi` of the direct trajectory's step 5; (b) `P_3 v` (start vector) with `e = 1e-4` against `1e-5`; (c) `P_3 v` against `J_3 (J_2 (J_1 v))`, each `J_j` the one-step product of ADR 0392 about the `j - 1`-th state of the converged trajectory from step 2 | (a) bit-equal; (b) `<= 1e-3`; (c) `<= 1e-3` relative |
| U4 (fast) | 1 | Arnoldi (`m = 40`) on exact products of `k = 10` maps of dimension 400, `A_j = Q D_j Q` (`Q` the Householder reflection of U2; `D_j` diagonal, entries 2 to 399 `0.05 + 0.55 i / 399`): (i) `D_j[0] = -(1.1 + 0.02 j)`, `D_j[1] = 0.7`, `j = 0..9`; (ii) `D_j[0] = 1.5, D_j[1] = 0.5` for even `j`, swapped for odd `j` | (i) `g` within 1e-8 of `(prod_j (1.1 + 0.02 j))^(1/10)` and `mu` real positive; (ii) `g` within 1e-8 of `0.75^(1/2) = 0.8660254`, below one, while every `A_j` has spectral radius 1.5 |
| N1 | (defaults) | no library file changed (`git diff --stat` against the base lists only the new test file and docs) | as stated |

Printed, no limit: for every product the Ritz values with `g` above 0.9, `sigma`, the nonlinear propagation series, the shares
and cells of the dominant vector; `|S^k_R(0) - S^k(0)| / |S^k(0) - x_0|` at the base (the R trajectory against the converged
one); for X1 to X4 the twelve largest Ritz values, the overall dominant, `C_max` and its cell, the cell of the largest `abs(p)`
of the overall dominant vector and its distance to the cell of `C_max`; `C_F`, `C_0`, `U_m'`; the wall time of each record.

**Negative controls.** G4 (two correctors: `g < 1` expected); X1 (the channel, expected inside); U4 (ii) (one-step spectral
radius 1.5, product below one); U3 (a) (the map at zero is the trajectory).

### Mutations (scratch copy of the final tree, not committed)

Each applied alone by an exact text replacement checked to occur once, the named test run once in release, the file restored
and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | the k-step map restores the perturbed state before every step (the frozen state: the step repeated from `x_{n0}`) | U3 (a) |
| M2 | the finite difference of the k-step map divided by `e` instead of `2 e` | U3 (c) |
| M3 | the eigenvalue per step taken as `abs(mu)^(1/(k - 1))` | U4 |
| M4 | the sign of the Givens update of the rows (`c * ak1 - s * ak` made `+`) | U1 |
| M5 | Arnoldi stores `h_{j+1,j}` as the square of the norm | U2 or U4 |

## Decision rule

- **done**: R1, R2, G0, G1 a and b, G2, G3, G4, X1, X2, X3, U1 to U4 and N1 met, and every mutation caught by its named check.
- **partial**: U1 to U4, N1, R1, R2 and G0 met (the k-step linearisation delivered and valid) and any of G1 to G4, X1 to X3
  missed, or a mutation not caught: the measured `g`, the growth measures and the one-step spectra are recorded as the located
  description.
- **todo**: U1 to U4, R1, R2 or G0 missed and not repaired.

A set-up check that does not hold (R1, R2, U3 (a)) is a defective set-up: an amendment before any corrected run, never a pass.

## Limits (not covered)

One flow (DFG 2D-2) on one mesh (M2), BDF2 at a constant step, `LinearUpwind`, one penalty, AMG-preconditioned CG; two
dimensions, a body at rest. One window per setting (steps 16 to 25, and 16 to 35 for F): `g` is a property of that window, not
a Lyapunov exponent and not a proof of the run's long-time behaviour. The map is that of the step with converged solves. The
dominant part of the spectrum of `P_k` is reached from one generic start; `sigma` is a lower bound in the scaled norm of ADR 0392
D1 only. HC is tested by its consequences X1 to X3; the coefficient that makes the step's eigenvalue 1.58 (rather than the
estimate's gain near one) is not identified. No solver code is changed, so the refusal of ADR 0374 stays as it is.

## Open questions

- **Q1. Library diagnostic.** Options: (a) test code only (registered); (b) a public `PimpleSolver` method for the k-step
  linearisation. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (a).
