# ADR 0400: F-491 registration: static correction of the harmonic response for free bodies (inertia relief), on the transformation path and with a pre-stressed basis

Status: **REGISTERED 2026-11-13, written before any code of F-491 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-491 was written, compiled or run: the code listed under "What was read" was read, and
the arithmetic below was done with a short scratch script (`f491-scratch/predict.py`, numpy only, on the 1-D consistent-mass
chain that the Hex8 bars below reduce to exactly; disclosed under "Arithmetic"; it does not call rcfd). Results go in the
addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0391](0391-f399-registration-harmonic-response-by-mode-superposition.md) (F-399, `harmonic.rs`: mode superposition
and the static correction of the truncated modes, refused for a free body and on the transformation path), its remaining
questions 1 and 2, option (a) each, taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
Transformation: ADR 0296; modal analysis on it: ADR 0384 (`mpc::modal_analysis_mpc`); pre-stressed modes: ADR 0390 and ADR 0398
(`geometric::prestressed_modes`, also on the transformation path); the count of free rigid motions: ADR 0399
(`free_motions::free_rigid_motions`). Roadmap item F-491 (deps F-399, F-395). Number 0400 given by the lead. Branch
`f491-harmonic-static-correction` from main b9d6c7a1, worktree `w49i-work/f491`, scratch `w49i-work/f491-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (a model
that is not a free body, not on the transformation path and gets no pre-stress takes the code of ADR 0391 unchanged, checked
bit by bit as B1); no edit of `baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code;
clean-room. Library edits: `harmonic.rs` only (`buckling.rs` is not touched: another package works there; `mpc.rs`,
`geometric.rs`, `free_motions.rs` are used read only through their public and crate-public items). Tests: a new file
`tests/si_harmonic_correction.rs`; in `tests/si_harmonic.rs` the test `c6_refusals` asserts two refusal rows this package
retires (D6), so by the maintainer's standing decision of 2026-11-13 it is marked `#[ignore]` with a reason naming this ADR
(body untouched) and a copy `c6_refusals_after_landed` without the two retired rows is added; no other existing test is edited.

## What F-491 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A free-free bar under a harmonic end force with the
> static correction by inertia relief gives the tip amplitude of the closed-form free-free bar within 0.5 percent at 0.5 and
> 1.5 times the first elastic frequency with 10 modes, the same without the correction above 1 percent at 0.5 (control); a
> bar on an end spring (the support of ADR 0384) with the correction on the transformation path gives the response of the
> direct dense solve within 1e-3 at 10 modes; with a pre-stressed basis the correction uses the stiffness the modes were
> computed with; models without these features give the bits of the solver of ADR 0391.

Clauses: (f1) free-free bar, end force, inertia relief, 10 modes: tip amplitude of the closed form within 0.5 percent at 0.5
and 1.5 `omega_1` (first elastic frequency); (f2) without the correction the error at 0.5 above 1 percent (control); (f3) bar
on an end spring (`ElasticSupport::NodalSpring`, ADR 0384), correction on the transformation path, 10 modes: the response of
the direct dense solve within 1e-3; (f4) with a pre-stressed basis the correction uses the stiffness the modes were computed
with (`K + K_sigma`); (f5) models without these features: the bits of the solver of ADR 0391.

**Reading of (f1) (an interpretation, taken on the maintainer's standing instruction of 2026-11-11).** The free-free bar
`0 <= x <= L` with the force `F` at `x = L` and `x = 0` free has (first principles: `u'' + k^2 u = 0`, `u'(0) = 0`, `E A u'(L) =
F`) `u(x) = -F cos(k x) / (E A k sin(k L))`, so the driven end moves `u(L) = -F cot(k L)/(E A k)` and the far end `u(0) = -F/(E
A k sin(k L))`. The first elastic frequency is `k L = pi`; at 0.5 and 1.5 times it `k L = pi/2, 3 pi/2`, where `cot(k L) = 0`:
**both registered frequencies are antiresonances of the driven end**, whose closed-form amplitude is zero (with the discrete
`omega_1` it is `4e-4` of `F/(E A k)`, arithmetic 2), so an error relative to it is not a meaningful number. The clause is
read as: the error of the driven-end amplitude measured against the natural amplitude scale of the bar at that frequency, `F/(E
A k)` (which is the far-end amplitude `|u(0)|` at these two frequencies): `|X_tip - u_cf(L)| / (F/(E A k)) <= 0.005` (A1), the
control the same measure above 0.01 (A2). The far-end amplitude relative to its closed form is registered as a companion (C1,
same limit 0.005), and its control is recorded without a limit (it does not discriminate: `1.9e-3`, arithmetic 2), so the
reading that discriminates is the one registered for the clause. "10 modes" is read as 10 modes in the basis (the rigid mode
and the 9 lowest elastic ones), the basis of `modal_analysis` with 10 modes.

## What was read (branch base b9d6c7a1, read only)

- `AGENT_RULES.md`; the brief; ADR 0391 (all), ADR 0384 (the KB set-up, the closed form `z tan z = kappa`, the 1-D reduction of
  the `nu = 0` bar), ADR 0390/0398 (the headings and D2 of 0398), ADR 0399 (head, the counting rule).
- `crates/rcfd-fem/src/harmonic.rs` (all), `mpc.rs` (`uses_transformation`, `Transformation::{t, independent, n_mesh,
  n_unknowns, expand, transpose_times, reduce}`, `transformation`, `add_supports`, `modal_analysis_mpc` and `MpcModes`,
  `refuse_nonzero_values` and `unknown_name` (crate-public), `solve_static_mpc` signature), `geometric.rs` (`Prestress` with
  `Debug, Clone, PartialEq`, `Prestress::uniform`, `geometric_stiffness`, `add_on_pattern` (crate-public), `prestressed_modes`
  and its transformation branch: `T^T (K_ext + K_sigma) T`, supports added before `K_sigma`), `free_motions.rs`
  (`free_rigid_motions`, `FreeRigidMotions::fields`, refused for cyclic constraints and rotational unknowns), `modal.rs`
  (`ModalResult`, the automatic shift of a free body), `tests/si_harmonic.rs` (all; `c6_refusals` rows "static correction of a
  free body" and "static correction with a nodal spring").

## Design

Notation of ADR 0391: the reduced system `M x'' + C x' + K x = Re(F e^{i omega t})`, M-orthonormal modes `phi_r`, `K phi_r =
lambda_r M phi_r`, retained set `m`, `X = sum_{r in m} phi_r H_r phi_r^T F + X_res`.

### D1. Inertia relief for a free body (first principles)

Let the reduced `K` (prescribed equations removed) have the null space spanned by the `n_r` free rigid motions, `K R = 0`, `R`
(`n x n_r`) M-orthonormal, `R^T M R = I` (no mechanism: the zero-energy motions are rigid). The modes split into the rigid ones
(`lambda = 0`, spanning `R`) and the elastic ones (`lambda_s > 0`, `R^T M phi_s = 0`). For `omega > 0` the rigid modes have
`H = 1/(-omega^2 + i c omega)` and are kept in the sum (they must be in the basis); what the static correction must supply is
the elastic residual flexibility

    X_res = G_e F - sum_{elastic s in m} phi_s phi_s^T F / lambda_s,    G_e = sum_{all elastic s} phi_s phi_s^T / lambda_s,

the same truncation argument as ADR 0391 D2 restricted to the elastic modes (the truncated modes are elastic; the rigid ones
are all retained). `K^{-1}` does not exist; `G_e` is computed without it:

1. **The equilibrated load.** `P = I - M R R^T`. Then `R^T P F = R^T F - (R^T M R) R^T F = 0`: `P F` does no work on any rigid
   motion (self-equilibrated). For an elastic mode `phi_s^T P F = phi_s^T F - (phi_s^T M R) R^T F = phi_s^T F`.
2. **A statically determinate support.** Choose `n_r` equations `S` such that `R_S` (the rows of `R` at `S`, `n_r x n_r`) is
   non-singular. `K_c` = `K` with the rows and columns `S` removed is non-singular: `K_c y = 0` with `y_S = 0` means `K y = r`
   supported on `S`; `R^T K y = 0` gives `R_S^T r_S = 0`, so `r = 0`, `y` is in the null space, `y = R a`, `R_S a = y_S = 0`, `a =
   0`. Solve `K_c y_c = (P F)_c`, `y_S = 0`. Then `K y = P F + r` with `r` supported on `S`, and `R^T`: `0 = R^T P F + R_S^T
   r_S = R_S^T r_S`, so `r = 0` (the reactions of a statically determinate support under an equilibrated load vanish): `K y = P F`.
3. **The rigid part removed.** `y = sum_all a_r phi_r`; `phi_s^T K y = lambda_s a_s = phi_s^T P F = phi_s^T F` fixes the elastic
   coefficients, the rigid ones are arbitrary (`y_S = 0` chooses them). `P^T = I - R R^T M` kills the rigid components (`P^T R =
   R - R (R^T M R) = 0`) and keeps the elastic modes (`P^T phi_s = phi_s`). So

       G_e F = P^T K_c^{-1} P F    (with the S rows of y zero),

   independent of the choice of `S`. (This is the inertia-relief construction of the structural-dynamics literature, e.g. Craig
   and Kurdila, from memory; only the derivation above is used.) Check of the arithmetic (arithmetic 1): on the 40-element free
   chain `P^T K_c^{-1} P F` equals the spectral `G_e F` within `8.1e-14`.

**Implementation.** Taken when the static correction is on, the model is not on the transformation path and
`free_rigid_motions(model, &[])` counts `n_r > 0` (a model with `n_r = 0`, or for which the count is refused, takes the code of
ADR 0391 unchanged). `R`: the `n_r` free fields `FreeRigidMotions::fields`, set to exactly 0 at the prescribed equations, made
M-orthonormal by modified Gram-Schmidt with the model's full consistent `M` (the fields are 0 at the prescribed equations, so
this is the reduced `R_f^T M_ff R_f`). `S`: pivoted selection over the free equations: `W = R` restricted to the free
equations, repeated `n_r` times: take the row of largest Euclidean norm (the first on a tie), normalise it to `u`, subtract
`(w_i . u) u` from every row (the rows chosen give the largest successive volumes, a well-conditioned `R_S`). `K_c` factored
by `SkylineLdlt` with the pivot rule of ADR 0391 (`d_i <= PIVOT_TOLERANCE a_ii` refused, key `"static correction"`; a mechanism
lands here). The basis: each mode's rigid weight `p_r = |R^T M phi_r|^2` (in `[0, 1]` for an M-normalised mode). A mode with
`p_r > 1 - RIGID_SEPARATION` is a rigid mode of the basis, one with `p_r < RIGID_SEPARATION` an elastic one, anything between
refused (`RIGID_SEPARATION = 1e-6`; key `"static correction"`, "a mode mixes rigid and elastic motion"); the number of rigid
basis modes must equal `n_r` (refused otherwise: the response of a free body needs its rigid modes); every elastic basis mode
must have `lambda_s > 0` (refused otherwise). Per load part `f` (real and imaginary): `a = R^T f`, `P f = f - (M R) a`, solve on
the free equations without `S`, `b = (M R)^T y`, `y -= R b`, then `y -= sum_{elastic s} phi_s (phi_s^T f)/lambda_s`.

A free body with a pre-stress is refused (key `"static correction"`): the null space of `K + K_sigma` is not the rigid motions
in general (a rotation under stress stores energy), which this derivation needs.

### D2. The transformation path (first principles)

On the transformation `x = T q` (ADR 0296; zero values only) the modes of `modal_analysis_mpc` (and of the transformation branch
of `prestressed_modes`) are `phi_r = (T q_r)` restricted to the mesh unknowns, with `K_r q_r = lambda_r M_r q_r`, `K_r = T^T K_ext
T` (`K_ext` with the supports), `M_r = T^T M_ext T`, `q_r^T M_r q_s = delta_rs`. The mesh load `F` is extended by zeros at the
reference unknowns (`F_ext`), so `q_r^T T^T F_ext = phi_r^T F` (the reference rows carry no load). The spectral expansion of
ADR 0391 D2 holds for the reduced pair: `K_r^{-1} = sum_all q_r q_r^T / lambda_r` when `K_r` is positive definite. So the static
correction on the transformation path is

    X_res = [T K_r^{-1} T^T F_ext]_mesh - sum_{r in m} phi_r phi_r^T F / lambda_r,

the bracket the mesh part of the expanded static solution. `K_r` is built as in `modal_analysis_mpc` (`stiffness_with`,
`add_supports`, `Transformation::reduce`) and factored once with the pivot rule above (rows named by `mpc::unknown_name`).
Refused (key `"static correction"`): a non-zero constraint value (`mpc::refuse_nonzero_values`), a cyclic constraint (its `k = 0`
transformation would only fit a `k = 0` load; a limit of this item), a free rigid motion counted by `free_rigid_motions` (inertia
relief on the transformation path is not included: a limit), a basis eigenvalue `<= 0`, a singular `K_r` (a mechanism).

### D3. A pre-stressed basis

`HarmonicOptions` gains `prestress: Option<Prestress>` (default `None`) and the builder `.prestress(p)`. With `Some(p)` the
stiffness of the correction is `K + K_sigma` (`geometric_stiffness(model, p, threads)` added by `add_on_pattern` after the
supports, the order of `prestressed_modes`), the stiffness the modes of `prestressed_modes(model, p, ..)` satisfy; the spectral
argument of ADR 0391 D2 then holds for `K + K_sigma` (positive definite below the buckling load; else the factor refuses). Without
the correction the pre-stress is not used (the modes carry it). The caller passes the same `Prestress` as to
`prestressed_modes`; that they match is the caller's (it cannot be read from a basis). With `Some(zero stress)` the result has
the bits of `None` (`K + 0` is `K` value by value; checked as C7).

### D4. Unchanged (f5)

`HarmonicSolver::new` keeps the code of ADR 0391 for a model not on the transformation path with `n_r = 0` and no pre-stress:
the same calls in the same order, the same factor and the same subtraction loop. The added work is one call of
`free_rigid_motions` (it reads the model, changes nothing). Everything else in `harmonic.rs` is unchanged.

### D5. The set-ups

Material `E = 2e11`, `nu = 0`, `rho = 7850`, `c = sqrt(E/rho)`, `b = 0.04`, `A = b^2`. Modal options of ADR 0391 (`tolerance
1e-12`, `ResidualTest::Backward`), 10 modes.

- **FF (free-free bar)**: `hex_box([0; 3], [1.0, b, b], [40, 1, 1])`, `("all", 1, 0)`, `("all", 2, 0)`, no axial constraint:
  one free rigid motion (the axial translation). End force 0.25 N along `x` on each node of `xmax` (`F = 1` N). Basis:
  `modal_analysis` with 10 modes (automatic shift; the rigid mode and 9 elastic). `omega_1 = sqrt(lambda)` of the first elastic
  mode of the basis. Driven end: node `xmax[0]`, component 0; far end `xmin[0]`, component 0. As in ADR 0391 D6 (and ADR 0384)
  the axial motion uniform over a section is the 1-D linear chain with consistent mass exactly; the first section-varying mode is
  at `61 c` against `omega_10 ~ 9 pi c` (rad/s per metre).
- **KS (spring bar)**: FF plus `NodalSpring("xmin", 0, k/4)`, `k = kappa E A / L`, `kappa = 0.25` (`k = 8e7` N/m): on the
  transformation path (a support), no free rigid motion. Basis: `modal_analysis` (dispatches to `modal_analysis_mpc`), 10 modes.
  Direct dense solve: from `modal_analysis_mpc(&ks, &[], ..)`'s `reduced_stiffness`, `reduced_mass` and `transformation`, the
  dense `(K_r - omega^2 M_r) q = T^T F_ext` by Gaussian elimination with partial pivoting in the test, `x = (T q)_mesh`.
- **KSP (pre-stressed spring bar)**: KS with `Prestress::uniform(sigma_xx = 0.05 E)` (a tension; the geometric stiffness of the
  axial chain is `sigma A/h [[1, -1], [-1, 1]]`, the same 1-D form, so the chain predicts it). Basis `prestressed_modes`, 10
  modes. Direct dense: `K_r + T^T K_sigma T`.
- **CP (pre-stressed cantilever, plain path)**: `hex_box([0; 3], [1.0, 0.05, 0.05], [20, 1, 1])`, clamped `("xmin", 0..3, 0)`
  (240 free unknowns), `Prestress::uniform(sigma_xx = 5e7)` Pa (tension; Euler-Bernoulli estimate of the tip-deflection change
  under the axial force `P = sigma A = 1.25e5` N: `alpha^2 = P/(E I) = 1.2`, `delta = F (alpha L - tanh alpha L)/(P alpha) =
  0.675` of `F L^3/(3 E I)`, a 32 percent change; the single Hex8 through the depth is stiffer in bending (shear locking), which
  lowers the relative change but not below a few percent). Tip load `0.25` N along `y` on each node of `xmax`. Basis
  `prestressed_modes`, 10 modes. Direct dense: `K + K_sigma` and `M` assembled (`assemble::stiffness`,
  `geometric::geometric_stiffness`, `mass::consistent`), restricted to the free equations.
- **SF (small free block)**: `hex_box([0; 3], [1.0, 0.5, 0.5], [2, 1, 1])` without constraints (ADR 0391's S-free, 36 unknowns,
  six free rigid motions), `ModalBasis::all_modes` (6 rigid + 30 elastic modes). Load: 1 N along `(1, 2, 3)/sqrt(14)` at node 0
  only (it has a rigid part, so `P` is exercised).

### D6. Retired refusal rows

ADR 0391 C6 asserts two refusals this package removes: "C6 static correction of a free body" (S-free with its all-modes basis:
now taken by D1) and "C6 static correction with a nodal spring" (the clamped S with a nodal spring, on the transformation path:
now taken by D2). `c6_refusals` is marked `#[ignore = "record: ADR 0400 retired two of its refusal rows (static correction of a
free body, with a nodal spring); see c6_refusals_after_landed"]`, body untouched; `c6_refusals_after_landed` is the same body
without those two rows.

### Arithmetic before this registration (disclosed)

Scratch script `f491-scratch/predict.py` (numpy, 1-D chain of 40 linear elements, consistent mass, the set-ups FF, KS, KSP):

1. FF: discrete rigid eigenvalue `5.9e-5` (rounding), first elastic `omega_1 = 1.58614e4` rad/s. `P^T K_c^{-1} P F` (support at
   node 0) against the spectral `G_e F`: `8.1e-14`.
2. FF, `scale = F/(E A k)`: at `s = 0.5` the closed driven-end amplitude is `4.0e-4 scale` (the discrete `omega_1` is not exactly
   the continuum `pi c / L`). Driven end `|X - u_cf(L)|/scale`: with the correction `1.318e-4` (`s = 0.5`), `3.566e-3` (`s =
   1.5`); without `3.205e-2` (`0.5`), `9.931e-2` (`1.5`); the full discrete solve `1.010e-4`, `2.724e-3` (the error with the
   correction is the chain's dispersion). Far end relative to `u_cf(0)`: with `6.9e-5`, `4.48e-4`; without `1.90e-3`, `5.05e-3`.
   Against the direct solve over every equation, `max |X - X_d| / max |X_d|`: with `3.08e-5`, `8.42e-4`; without `3.19e-2`,
   `9.65e-2`.
3. KS (`kappa = 0.25`, `omega_1 = 2.4233e3`): `max |X - X_d|/max |X_d|` with the correction `4e-16` (`s = 0`), `6.96e-8`
   (`0.5`), `1.11e-6` (`1.5`); without `4.06e-3`, `3.10e-3`, `5.51e-3`.
4. KSP (`sigma = 0.05 E`): at `omega = 0` the elastic-only correction differs from `(K + K_sigma)^{-1} F` by `9.6e-3` of the max;
   at `0.5 omega_1` (pre-stressed) with the `K + K_sigma` correction `6.4e-8`, with the elastic `K` `7.3e-3`.
5. SF with all 36 modes: the elastic residual `G_e F - sum_{30} phi phi^T F/lambda` is zero in exact arithmetic (the basis is
   complete); rounding of a 36-unknown block of aspect 2: estimate `1e-12` of the response.
6. CP (hand, above): the pre-stress changes the static tip deflection by up to 32 percent (Euler-Bernoulli), at least a few
   percent with the stiffer Hex8. Dense against skyline rounding: `cond(K) eps`, `cond ~ 1e6` to `1e8` for the cantilever, the
   max-norm error of the dominant tip deflection smaller: estimate `1e-10`.

## Registered checks

All in `crates/rcfd-fem/tests/si_harmonic_correction.rs`, release profile, run once (`CARGO_BUILD_JOBS=4 cargo test -p
rcfd-fem --release --test si_harmonic_correction -- --nocapture --test-threads=1`). `Ratio(0)` (undamped) unless stated. "Over
every equation" is `max_j |X_j - Y_j| / max_j |Y_j|` over the mesh equations of the complex responses.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | f1 | FF, 10 modes, correction (inertia relief): driven end `|X_tip - u_cf(L)| / (F/(E A k))` at `s = 0.5` and `1.5`, `omega = s omega_1`, `k = omega/c`, `u_cf(L) = -F cot(kL)/(EAk)` | each <= 0.005 | 1.32e-4, 3.57e-3 (arith. 2) |
| A2 | f2 | the same without the correction at `s = 0.5` | > 0.01 | 3.2e-2 |
| A3 | f3 | KS, 10 modes, correction on the transformation path, against the direct dense solve, over every equation, at `s = 0.5` and `1.5` (`omega_1` of KS) | each <= 1e-3 | 7e-8, 1.1e-6 (arith. 3) |
| A4 | f4 | KSP, 10 modes of `prestressed_modes`, correction with `.prestress(p)`, against the direct dense solve with `K_r + K_sigma,r`, over every equation, at `s = 0` and `0.5` | each <= 1e-3 | 1e-15, 6.4e-8 (arith. 4) |
| A5 | f4 | CP (plain path), 10 modes of `prestressed_modes`, correction with `.prestress(p)`, against the direct dense solve of `K + K_sigma - omega^2 M`, over every equation, at `s = 0` and `0.5` | each <= 1e-3 | ~1e-10, ~1e-6 |
| A6 | f5 | B1 below | bit-equal | D4 |

### C. Companions and further checks

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | FF, A1's solver: far end `|X_0 / u_cf(0) - 1|`, `u_cf(0) = -F/(E A k sin kL)`, at `s = 0.5`, `1.5` | each <= 0.005 | 6.9e-5, 4.5e-4 |
| C2 | FF, A1's solver against the direct dense solve `(K - omega^2 M)^{-1} F` on the free equations, over every equation, at `s = 0.5`, `1.5` | each <= 2e-3 | 3.1e-5, 8.4e-4 |
| C3 | FF, A1's solver: the residual of the correction is M-orthogonal to the rigid motion: `|r^T M X_res| / (|r|_M |X_res|_M)` for the real part at any `omega` (the residual is frequency independent), `r` the axial translation | <= 1e-10 | ~1e-15 |
| C4 | KS at `omega = 0`: the response with the correction against `solve_static_mpc` with the tip forces, over every equation | <= 1e-10 | ~1e-14 |
| C5 | CP at `omega = 0`: with the correction against the dense `(K + K_sigma)^{-1} F` | <= 1e-8 | ~1e-10 (arith. 6) |
| C6 | SF, all 36 modes, inertia relief: response with the correction against the response without it at `omega = 0.5 omega_7` (the first elastic), over every equation (the residual of a complete basis is zero) | <= 1e-8 | ~1e-12 (arith. 5) |
| C7 | B40 of ADR 0391 (clamped, plain path) with `.prestress(Prestress::uniform(0))` against `None`: A4-of-0391's response at `0.5 omega_1` | bit-equal | D3 |
| C8 | KS, A3's solver: `sweep` over `[0.5, 1.5] omega_1` against `response` at each | bit-equal | the sweep reuses the prepared load |
| C9 | refusals, each with key `"static correction"`: FF with a basis of its 9 elastic modes only (`ModalBasis::new` without the rigid mode); FF with a pre-stress; FF with a `Linear` constraint `u_x(xmax[0]) - u_x(xmax[1]) = 0` (on the transformation path, the axial translation still free); KS with a basis whose first eigenvalue is replaced by `-1` (`ModalBasis::new`); KS plus a `Linear` row `u_x(xmax[0]) - u_x(xmax[1]) = 1e-3` (a non-zero constraint value) | each refused as stated | D1, D2 |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | A1's measure without the correction at `s = 1.5` | `> 0.005` (9.9e-2) |
| N2 | A3 without the correction at `s = 0.5` | `> 1e-3` (3.1e-3: A3 discriminates the correction) |
| N3 | A4 at `s = 0` with the correction built without the pre-stress (elastic `K`) | `> 1e-3` (9.6e-3: A4 sees the stiffness of the correction) |
| N4 | C5 with the correction built without the pre-stress | `> 1e-3` (a few percent to 32 percent) |
| N5 | C1's far-end measure without the correction at `s = 0.5` | recorded, no limit (1.9e-3; the far end does not discriminate, see the reading of f1) |

### B. Bits of ADR 0391 (f5)

| Id | Measure | Limit |
| --- | --- | --- |
| B1 | A probe test (`f491-scratch/probe_bits.rs`, not committed) copied into an archive of the branch base b9d6c7a1 and into an archive of the branch tip, each built with its own target directory: it prints `f64::to_bits` of (i) B40 of ADR 0391 with 10 modes, default options (correction on), `Ratio(0.01)` and `Ratio(0)`, the tip load and the complex load of ADR 0391 C8, `response` at `[0, 0.2, 0.5, 1.5, 2.5] omega_1` (every equation, real and imaginary) and `modal_amplitudes`; (ii) the clamped S of ADR 0391 with `all_modes` and the correction, at `0.5 omega_1`; (iii) B40 without the correction. The two outputs compared byte by byte | identical |
| B2 | `cargo test -p rcfd-fem --release` (every test of the crate, `si_harmonic` included: its other nine tests unchanged and passing, `c6_refusals_after_landed` passing); `cargo check -p rcfd-post --tests` | 0 failed; builds |

### Mutations (scratch copy of `harmonic.rs`, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | inertia relief: the load not projected (`f` in place of `P f`) | A1, C1, C2 |
| Mu2 | inertia relief: the rigid part not removed (`P^T` dropped) | A1, C1, C2, C3 |
| Mu3 | inertia relief: the rigid basis modes not skipped in the subtraction | A1, C2 (or a refusal) |
| Mu4 | support selection: the first `n_r` free equations instead of the pivoted rows | C6 (singular `K_c` for SF: refused) |
| Mu5 | transformation path: the supports not added to `K` | A3, C4 (singular: refused) |
| Mu6 | pre-stress not added to the correction's stiffness | A4, A5, C5 |
| Mu7 | transformation path: the subtraction of the retained modes dropped | A3, C4 |
| Mu8 | the rigid-weight test reversed (a rigid mode counted as elastic) | C9 / A1 (refusal) |

## Decision rule

- **done**: every A check met (A1 to A6), every C and B check met, the negative controls N1 to N4 as expected.
- **partial**: any A check missed with a diagnosed cause, kept as a `*_known_miss` record; or a C or B check missed.
- **todo**: the checks could not be run.

## Limits of this registration

- Inertia relief on the plain path only (prescribed displacements, zero values); a free rigid motion on the transformation
  path (springs, linear rows, rigid sets leaving a rigid motion) is refused, as is a free body with a pre-stress.
- Cyclic constraints are refused for the static correction (the `k = 0` transformation does not fit a load of another harmonic
  index); without the correction they are taken as before.
- Mechanisms (zero-energy motions that are not rigid) are refused by the factor's pivot rule; they are not counted.
- The static correction neglects the inertia and damping of the truncated modes (second order in `omega/omega_{m+1}`), as in ADR
  0391. The rigid modes of a free body must be in the basis.
- That the pre-stress given to the solver is the one the basis was computed with is the caller's responsibility.
- The reading of clause f1 (the driven end at its antiresonances, measured against `F/(E A k)`) is an interpretation, recorded
  above; the far-end relative error is registered as a companion.

## Addendum 1: results (2026-11-13)

Registered run: commit ed496342, once, `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release --test si_harmonic_correction --
--nocapture --test-threads=1` (7 tests, 0.09 s; log `f491-scratch/run1.log`). No line of the test was changed between writing
it and the run; the test file was built (`--no-run`) before it. FF: rigid eigenvalue `-1.0e-5` (rounding of the automatic
shift), first elastic `omega_1 = 1.5861405174e4` rad/s (arithmetic 1 to every printed digit). KS: `omega_1 = 2.4233126530e3`
(arithmetic 3). CP: first bending pair `omega_1^2 = 1.27032e5` (a degenerate y/z pair of the square section).

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| A1 FF driven end, inertia relief, `/(F/(EAk))`, s = 0.5 / 1.5 | <= 0.005 | 1.318e-4 / 3.566e-3 | yes |
| A2 the same without the correction, s = 0.5 | > 0.01 | 3.205e-2 | yes |
| A3 KS against the direct dense solve, transformation path, s = 0.5 / 1.5 | <= 1e-3 | 6.96e-8 / 1.11e-6 | yes |
| A4 KSP pre-stressed against the direct dense solve, s = 0 / 0.5 | <= 1e-3 | 5.0e-14 / 6.39e-8 | yes |
| A5 CP pre-stressed against the direct dense solve, s = 0 / 0.5 | <= 1e-3 | 1.13e-11 / 4.04e-8 | yes |
| A6 = B1 bits of ADR 0391 | identical | identical (61 lines, 514003 bytes, md5 7536e084...) | yes |
| C1 FF far end relative, s = 0.5 / 1.5 | <= 0.005 | 6.90e-5 / 4.48e-4 | yes |
| C2 FF against the direct dense solve, s = 0.5 / 1.5 | <= 2e-3 | 3.08e-5 / 8.41e-4 | yes |
| C3 residual M-orthogonal to the rigid motion | <= 1e-10 | 9.2e-14 | yes |
| C4 KS omega = 0 against `solve_static_mpc` | <= 1e-10 | 4.2e-16 | yes |
| C5 CP omega = 0 against the dense `(K + K_sigma)^-1 F` | <= 1e-8 | 1.13e-11 | yes |
| C6 SF complete basis, with against without the correction | <= 1e-8 | 2.6e-15 | yes |
| C7 zero pre-stress against none | bit-equal | bit-equal | yes |
| C8 KS sweep against response | bit-equal | bit-equal | yes |
| C9 five refusals, key `"static correction"` | each refused | all five | yes |
| N1 FF without the correction, s = 1.5 | > 0.005 | 9.93e-2 | as expected |
| N2 KS without the correction, s = 0.5 | > 1e-3 | 3.09e-3 | as expected |
| N3 KSP, correction with the elastic K, s = 0 | > 1e-3 | 9.62e-3 | as expected |
| N4 CP, correction with the elastic K, s = 0 | > 1e-3 | 0.319 | as expected |
| N5 FF far end without the correction, s = 0.5 | recorded | 1.90e-3 | (no limit) |
| B1 probe, base b9d6c7a1 against tip ed496342 | identical | identical | yes |
| B2 `cargo test -p rcfd-fem --release` / `cargo check -p rcfd-post --tests` | 0 failed / builds | 252 passed, 0 failed, 54 ignored (the ignored record `c6_refusals` among them; `c6_refusals_after_landed` passes) / builds | yes |

Deviations and remarks:

- Every measured value lies within a few percent of its estimate from the 1-D chain (the Hex8 bars reduce to it exactly). The
  CP estimate of N4 (Euler-Bernoulli, up to 0.32) was met at 0.319: at `L/h = 20` with `nu = 0` the single Hex8 through the
  depth stiffens the bending less than the registration feared.
- `cargo fmt -p rcfd-fem` changed the layout of `harmonic.rs` and `si_harmonic_correction.rs` after the run (commit b0f92eeb,
  layout only); the mutations below ran on the formatted code.
- Not run: the rcfd-api baseline hashes (no file outside `rcfd-fem` changed and no other crate calls `harmonic`); the whole
  workspace (the lead's gate).

Mutations (archive copy of b0f92eeb in `f491-scratch/copy`, `f491-scratch/mutate.py`, own target directory; log
`f491-scratch/mutate.log`):

| Id | Mutation | Caught by |
| --- | --- | --- |
| Mu1 | load not projected | A1, C1, C2 (both frequencies), C6 |
| Mu2 | rigid part not removed (`P^T` dropped) | A1, C1, C2, C3, C6 |
| Mu3 | rigid basis modes not skipped | A1, C1, C2, C3, C6 (errors of order 1e12) |
| Mu4 | the first rows with a non-zero residual as support (no pivoting) | C6 (SF refused: singular `K_c`) |
| Mu5 | supports not added on the transformation path | A3, C4, C8 and A4 (refused: singular `K_r`) |
| Mu6 | pre-stress not added to the correction | A4 (both), A5 (both), C5 |
| Mu7 | transformation path without the subtraction of the retained modes | A3 (both), C4, A4 (both) |
| Mu8 | rigid-weight test reversed | A1's set-up and C6 (refused) |

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --all-targets --release -- -D warnings`: clean.

**Decision by the rule: done** (A1 to A6, every C and B check met, N1 to N4 as expected, every mutation caught).

Remaining questions:

1. Inertia relief on the transformation path (a body held by springs, linear rows or rigid sets that leave a rigid motion) is
   refused. Options: (a) a follow-up that builds `R` over the independent unknowns `q` (the extended rigid fields of ADR 0399 at
   the independent unknowns of `T`, reference points included) and the determinate support among them; (b) leave it refused.
   Recommendation: (a) when a fan model needs it (a rotor on a free shaft); not urgent.
2. A free body with a pre-stress is refused (the null space of `K + K_sigma` is not the rigid motions in general). Options: (a)
   take the null space from the zero modes of the basis instead of the counted rigid fields; (b) leave it refused.
   Recommendation: (b) for now.
3. The reading of clause f1 (both registered frequencies are antiresonances of the driven end, measured against `F/(EAk)`) is an
   interpretation taken on the standing instruction. Options: (a) keep the reading and re-word the acceptance for the record;
   (b) re-register with frequencies away from the antiresonances. Recommendation: (a), with the far end (C1) quoted alongside.
4. That the `Prestress` given to `HarmonicOptions` is the one the basis was computed with is the caller's. Options: (a) let
   `ModalBasis` carry an optional pre-stress from `prestressed_modes` (needs a field on `ModalResult`, outside this package);
   (b) leave it to the caller. Recommendation: (b) until a model-level harmonic case exists.

## For the lead: roadmap entries

F-491, proposed status **done**, note:

> ADR 0400, 2026-11-12: rcfd-fem harmonic: the static correction of the truncated modes now covers free bodies by inertia relief (residual flexibility P^T K_c^-1 P F, P = I - M R R^T, R the free rigid motions counted by free_rigid_motions and made M-orthonormal, K_c the stiffness on a statically determinate support chosen by pivoted rows of R; the rigid modes must be in the basis and are skipped in the subtraction), the transformation path (T K_r^-1 T^T F with K_r = T^T K_ext T, the supports added; cyclic constraints and a free rigid motion there refused) and a pre-stressed basis (HarmonicOptions::prestress: K + K_sigma in the correction). Free-free bar of 40 Hex8 with 10 modes (one rigid): the driven end, at its antiresonances 0.5 and 1.5 omega_1, within 1.3e-4 and 3.6e-3 of F/(EAk) from the closed form, 3.2e-2 without the correction at 0.5 (control); far end 6.9e-5 and 4.5e-4 relative. Bar on an end spring (kappa 0.25), transformation path: 7e-8 and 1.1e-6 of the direct dense solve at 0.5 and 1.5 omega_1 (3.1e-3 without). Pre-stressed spring bar and cantilever: 6.4e-8 and 4.0e-8 of the direct dense solve with K + K_sigma at 0.5 omega_1 (the elastic K in the correction gives 9.6e-3 and 0.32 at omega = 0). Models without these features give the bits of ADR 0391 (probe on base and tip identical); zero pre-stress bit-equal to none. Eight mutations caught. si_harmonic c6_refusals kept as an ignored record (two refusal rows retired), c6_refusals_after_landed added.

F-399, the last sentence of its note becomes:

> Static correction refused for free bodies and on the transformation path until F-491 (ADR 0400), which adds inertia relief, the transformation path and the pre-stressed stiffness.
