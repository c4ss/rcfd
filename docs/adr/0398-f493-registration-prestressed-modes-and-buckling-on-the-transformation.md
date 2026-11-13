# ADR 0398: F-493 registration: pre-stressed modes and linear buckling on the transformation (linear constraints, rigid sets, elastic supports)

Status: **REGISTERED 2026-11-13, written before any code of F-493 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-493 was written, compiled or run: the files listed under "What was read" were read,
and the arithmetic below was done by hand and with a short scratch script of the closed forms (`w49i-work/f493-scratch/arith.py`,
disclosed under "Arithmetic"; it does not call rcfd). Results go in the addendum at the end; nothing above the addendum is changed
after a run.

Parent: [ADR 0390](0390-f395-registration-geometric-stiffness-prestressed-modes-and-buckling.md) (F-395: `geometric::
prestressed_modes`, `buckling::factors`; its D4 and D6 refuse the transformation path, key `"constraint"`, and its remaining
question 3 proposes this item). The pattern followed: [ADR 0384](0384-f486-registration-modal-analysis-on-the-transformation.md)
(`mpc::modal_analysis_mpc`: `T^T K T`, `T^T M T`, modes `x = T q`, the Auto shift). The transformation: [ADR
0296](0296-linear-multi-point-constraints-rigid-sets-elastic-supports.md) (`mpc.rs`). The knife-edge closed form: [ADR
0394](0394-f492-registration-knife-edge-buckling-closed-form.md). Roadmap item F-493 (deps F-395, F-486). Size S. Branch
`f493-prestress-transformation` from main 6b29e2b9, worktree `w49i-work/f493`, scratch `w49i-work/f493-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (every model
without a linear constraint, rigid set, cyclic constraint or elastic support runs exactly the lines of `prestressed_modes` and
`factors` it ran before); no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; no new dependency, no unsafe
code; clean-room. The only edit of an existing test is the one the maintainer's standing decision of 2026-11-13 permits (D6). A
parallel package (F-455, static solve refinement) works on the static solve: `solve.rs` and the static parts of `mpc.rs` are not
edited; the edits of `mpc.rs` are additive (D3).

## What F-493 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. prestressed_modes and buckling::factors accept a model
> with linear constraints, rigid sets and elastic supports through T^T K T and T^T K_sigma T; the knife-edge beam of ADR 0390
> with its knife edges written as linear constraints gives the factors and frequencies of the prescribed-displacement model
> within 1e-10; a nodal spring at the roller raises the first factor by the closed form of a spring-supported column within 1
> percent.

Clauses: (a1) `prestressed_modes` and `buckling::factors` accept linear constraints, rigid sets and elastic supports, reducing by
`T^T K T` and `T^T K_sigma T` (and `T^T M T` for the modes); (a2) ADR 0390's knife-edge beam with the knife edges written as
`Linear` constraints: factors and frequencies equal to the prescribed-displacement model's within 1e-10; (a3) a nodal spring at
the roller: the first factor raised by the closed form of a spring-supported column, within 1 percent.

Reading of the clauses, registered:

- **(a1)** "through `T^T K T` and `T^T K_sigma T`" is checked as the identity of the library's result with the reduction done in
  the test from public functions (`mpc::transformation`, `assemble::stiffness`, `mpc::add_supports`, `geometric::
  geometric_stiffness`, `Transformation::reduce`, the eigensolver, `Transformation::expand`): factors, frequencies and every
  mode value bit-equal, for one model of each kind (linear constraints, an elastic support, a rigid set).
- **(a2)** "the factors and frequencies": the two lowest buckling factors at the reference compression of ADR 0390 (`s_ref =
  -1e6 Pa`) and the two lowest pre-stressed eigenvalues `omega^2` at `s = +1e8 Pa` (tension) and `s = -1e8 Pa` (compression), the
  stresses of ADR 0390's A6 and A7. Each model takes its own static pre-stress (`Prestress::from_static` of its own static solve;
  the linear-constraint model's static solve runs on the transformation path), so the clause covers the whole chain. "Within
  1e-10" is relative, `|x_L/x_B - 1| <= 1e-10`, per value.
- **(a3)** "a nodal spring at the roller". A spring added beside the roller's `u_y = 0` acts on an equation the roller already
  holds: it is eliminated with that equation and adds nothing (derived in D1; registered as check C5, the factors unchanged).
  The reading registered: **the roller is replaced by a lateral nodal spring** at the same nodes (the knife edge at `x = L`
  becomes an elastic lateral support of total stiffness `k`), so that the column is pinned at `x = 0` and elastically held
  laterally at `x = L`. Without the spring that column is a mechanism (a rigid rotation about the pin is free: no factor, the
  refusal `RigidBodyMotionFree`); the spring raises its first factor from none to the closed form of D1, `P_1 = min(k L, P_b)`,
  with `k L` the branch reached for the spring chosen (`k L` well below the bending branch `P_b`). "Within 1 percent":
  `|lambda_1 |s_ref| A / (k L) - 1| <= 0.01`. Taken on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation): the alternative readings (an axial spring at the roller; a spring beside the roller) do not raise the factor
  at all, so they cannot be what the entry means.

## What was read (no run)

- ADR 0390 (whole: D1 to D9, the set-ups D7, the checks, addendum 1, remaining question 3), ADR 0394 (whole: D1, the quadratic in
  `P` with `r^2 k^2`, the measured factors of B20 `408.40047` and `1600.70961`), ADR 0384 (whole: method, D1, D2, checks, addendum,
  the shift rule and `rigid_free` on the transformation path), ADR 0296 (the method, D1 to D5, the row order, the redundant rows),
  ADR 0395 (the style of a registration), `AGENT_RULES.md`, the brief (the maintainer's decision of 2026-11-13 on retired refusal
  rows), `docs/roadmap.json` entry F-493.
- Code: `crates/rcfd-fem/src/geometric.rs` and `buckling.rs` (whole), `mpc.rs` (lines 1 to 365: `uses_transformation`,
  `RowOrigin`, `Transformation`, `reduce`, `expand`; 366 to 645: the elimination and `transformation`, the row order prescribed,
  then constraints in order, rigid-set rows by node and component; 940 to 1446: `support_triplets`, `add_supports`,
  `solve_static_mpc`, `MpcModes`, `unknown_name`, `zero_shift_refused`, `zero_values_only`, `modal_analysis_mpc`,
  `refuse_in_modal`), `modal.rs` (`ModalOptions`, `ModalResult::from_parts`, `modal_analysis_with` and its dispatch), `solve.rs`
  lines 120 to 200 (`StaticOptions`, `solve_static_report` and its dispatch), `model.rs` lines 200 to 300 (`LinearTerm`,
  `Constraint::linear`, `rigid_set`, `ElasticSupport::nodal_spring`: stiffness per node), `crates/rcfd-linalg/src/eigen.rs`
  (`Shift::Auto` and `factor_shifted`: candidates `0, 1e-4 rho, ..., 10 rho`; `BucklingEigenOptions`, `BucklingEigen`,
  `lowest_buckling_factors`).
- Tests: `crates/rcfd-fem/tests/si_geometric.rs` (helpers, `beam`, `static_prestress`, `c7_refusals`, whose row "B with a nodal
  spring" asserts the refusal this package retires, D6), `si_knife_edge_closed_form.rs` (`beam`, `lower_root`, `identification`),
  `si_modal_mpc.rs` (`remap`, the skew rollers SR, the D1 reduction in the test). Searched: no other test or crate calls
  `prestressed_modes` or `buckling::factors`, and no other test asserts the refusal text of `refuse_transformation`.
- **Not read:** any finite-element code or manual outside this repository.

## Design

### D1. The physics (first principles)

**Reduction.** In a pre-stressed modal or buckling analysis every constraint value is zero, so the admissible motions are `x =
T q` (ADR 0296; ADR 0384 for the modes). The second variation of the total potential energy at the fixed pre-stress (ADR 0390 D1)
restricted to `x = T q` is `q^T T^T (K_ext + K_sigma) T q / 2`, `K_ext` the stiffness with the supports added (ADR 0296 D4), and
the kinetic energy `qdot^T T^T M_ext T qdot / 2` (the reference points of rigid sets without mass of their own, ADR 0384).
Stationarity gives the pre-stressed modes `T^T (K_ext + K_sigma) T q = omega^2 T^T M T q` and the buckling factors `(T^T K_ext T
+ lambda T^T K_sigma T) q = 0` (Rayleigh-Ritz on the subspace `range(T)`, the same congruence as the static and modal reductions).
Each mode or buckling mode is `x = T q`. `K_sigma` is the mesh matrix of ADR 0390 D3 (the reference unknowns have none: the
pre-stress lives in the elements). The support's stiffness is not pre-stressed (a spring to ground carries no initial stress in
this model; its static force is part of the reactions, which do not enter `K_sigma`).

**Knife edges as linear constraints (a2).** A single-term row `a u_i = 0` (`a != 0`) and a tie `u_j - u_i = 0` with `u_i = 0`
make `u_i` and `u_j` dependents with empty expressions: `T` is then the selection of the free unknowns, the same set (ascending)
as the elimination of `constraint::reduce`, and `T^T K T` has the entries of `K_FF` (ADR 0384 L1 measured 0 for the eigenvalues).
The static pre-stress comes from a different solve (`solve_static_mpc` on `T^T K T`) and can differ in its last bits; the factors
and frequencies then differ at rounding level (estimate below).

**Rotated knife edges (companion).** The beam rotated by 30 degrees about `z` with the knife edges written as `u . e = 0` along
the rotated axes is the same body (as ADR 0384's SR against SA). The roller row is written with its `u_y` term first (rotated
frame: `c u_y - s u_x`), so that the designated dependent is `u_y` and the independent unknowns are the same equations as in the
unrotated model; then the start vectors of the eigensolvers coincide and the comparison sees rounding only.

**The spring at the roller (a3).** Column `0 <= x <= L` in compression `P`, pinned at `x = 0` (`w = 0`, no moment), at `x = L`
no moment and a lateral spring `k` (force `-k w(L)`). Euler-Bernoulli: `E I w'''' + P w'' = 0`, `w = A + B x + C sin(mu x) + D
cos(mu x)`, `mu^2 = P/(E I)`. `w(0) = 0` and `w''(0) = 0` give `A = D = 0`; `w''(L) = 0` gives `C sin(mu L) = 0`. The
transverse force at `x = L`: the section transmits `V = E I w''' + P w'` (shear plus the tilt of the axial force), which the
spring balances, `V(L) = k w(L)`. With `C = 0` (`sin(mu L) != 0`), `w = B x`, `V = P B` and `k w(L) = k B L`: `P = k L` (rigid
sway, no bending; equivalently the overturning moment `P w(L)` about the pin equals the restoring moment `k w(L) L`). With
`sin(mu L) = 0`: `P = pi^2 E I/L^2`, `w(L) = 0` and `V(L) = C mu cos(mu L) (P - E I mu^2) = 0` (the spring not engaged). So `P_1 =
min(k L, P_E)` (the column "on an elastic lateral support", Timoshenko and Gere, Theory of Elastic Stability, section 2.2, from
memory; derived here). The spring raises the first factor from none (`k = 0`: a mechanism) to `k L/(|s_ref| A)` while `k L <
P_b`.

*Do the solid's shear and rotating-section terms (ADR 0394) enter?* With the Timoshenko kinematics and the solid's initial-stress
term of ADR 0394 D1, the energy per length is `(E I psi_x^2 + Q (w_x - psi)^2 - P (w_x^2 + r^2 psi_x^2))/2` plus `k w(L)^2/2`.
The rigid sway `w = theta x`, `psi = theta` has `psi_x = 0` and `w_x - psi = 0`: no bending, no shear, no rotating-section term.
Its first variation is `int (-P theta) delta w_x dx + k theta L delta w(L) = (k L - P) theta delta w(L)` (with `delta w(0) = 0`):
it is stationary exactly at `P = k L`. **The sway branch is `k L` exactly in the Timoshenko column with the solid's term; neither
the shear term nor the rotating-section term enters the reference of (a3).** They enter the bending branch `P_b`, which is ADR
0394's lower root `P_1 = 4.083826e6 N` (the knife-edge mode `w = W sin(pi x/L)`, `psi = Psi cos(pi x/L)` has `w(L) = 0` and, from
the first row of ADR 0394's matrix, the natural transverse condition `Q (w_x - psi) - P w_x = 0` at `x = L` exactly, so the
spring is not engaged and the bending branch stays at `P_1` for any `k`). In the solid itself the rigid rotation about the pin
line, `u = theta (-(y - h/2), x, 0)`, satisfies every remaining constraint (pin, `u_z = 0`), has no strain, spring energy `k
theta^2 L^2` (all roller nodes move by `theta L`) and initial-stress energy `s theta^2 V` (ADR 0390 D1, `|grad u . e_x|^2 =
theta^2`). Its Rayleigh quotient is exactly `k L/(|s| A)`, and the lowest factor is a minimum of that quotient over admissible
motions, so **`lambda_1 <= k L/(|s_ref| A)`** holds exactly in the discrete model (check C3). The rigid rotation is not an exact
eigenvector: `K_sigma x` is the distributed transverse load `+-s theta` over the two end faces (the gradient of the linear field
integrates to the boundary), the spring force is concentrated on the knife-edge line, and the self-equilibrated difference
deforms the end regions. The first-order deficit is `r^T K^{-1} r / (x^T K x)` with `r` that difference: of the order `c_0 k/(G
b)`, `c_0` an O(1) local compliance factor (two ends, a line load on quadratic elements: `c_0 ~ 0.6` to a few), `G b = 1e10
N/m`.

**Spring beside the roller (companion C5).** With the roller kept, a spring on the roller nodes in `y` adds stiffness on
equations that are prescribed. `T` does not contain them (their rows are empty), so `T^T K_ext T = T^T K T` entry for entry: the
factors are those of the model without the spring.

**Rigid end face (companion, a1 with physics).** Beam B plus a rigid set on the end face `xmax` (reference at its centre `(L,
h/2, b/2)`): the face stays plane and rigid, the knife edge on the neutral axis keeps it free to rotate about `z`. The static
pre-stress is still exact (`u_x = s x/E` moves the face rigidly). Adding rows restricts the admissible subspace, so by the
min-max characterisation every pre-stressed eigenvalue and every factor of this model is at least that of B (check C7, exact up
to the rounding of the two pre-stresses, about 1e-12). The face restrains the end warping of the shear deformation only: of the
order of the shear share of the energy (0.5 percent for `n = 1`, ADR 0390 arithmetic 1) times the end region over the
half-wave, about `0.5 percent x h/(L/2) = 5e-4`; factors and shifts within 1 percent of the closed forms of ADR 0394 and ADR 0390
arithmetic 4.

### D2. The library: `geometric.rs` and `buckling.rs`

In `prestressed_modes` the line `refuse_transformation(model, "a pre-stressed modal analysis")?;` becomes `if
crate::mpc::uses_transformation(model) { return prestressed_modes_on_transformation(model, prestress, options); }`; in
`buckling::factors` the same with `factors_on_transformation`. Every other model runs the old lines unchanged (the refusal
returned `Ok` for them). `refuse_transformation` has no caller left and is removed (`pub(crate)`, no public item removed).

`prestressed_modes_on_transformation(model, prestress, options) -> Result<ModalResult>` (private, `geometric.rs`), as
`mpc::modal_analysis_mpc` with `K_ext + K_sigma` in place of `K_ext`:

1. Zero values only: every prescribed value and every `Linear` value must be 0, else `InvalidValue { key: "constraint" }` whose
   reason names the row ("constraint 5 (Linear): value 1e-3; a pre-stressed modal analysis takes zero values only"; D3).
2. `tr = mpc::transformation(model, &[])`; `K = stiffness_with(model, threads)`, `mpc::add_supports(&mut K, model)`; `K_sigma =
   geometric_stiffness(model, prestress, threads)` (the pre-stress checked against the model there); `A = add_on_pattern(K,
   K_sigma)` (the supports lie on the stiffness pattern, so the patterns are equal); `M = consistent_with(model, threads)`.
3. `A_r = tr.reduce(A, 0)`, `M_r = tr.reduce(M, 0)`; the mode count against `n_q`; the mass check on `M_r` naming the unknown
   (`mpc::unknown_name`, D3), as ADR 0384.
4. `lowest_eigenpairs(A_r, M_r)` with the options of the call and the shift `options.shift.unwrap_or(Shift::Auto)` (ADR 0384's
   rule on the transformation path); `rigid_free` as ADR 0384 (`eig.shift != 0` for the default, `mpc::zero_shift_refused(A_r)`
   for a given shift).
5. **Beyond the buckling load** the prescribed path refuses at the factorisation of the zero shift (ADR 0390 D4). On the
   transformation path the Auto rule then moves to a positive shift, at which `A_r + sigma M_r` may be positive definite and the
   eigensolver would return a negative `omega^2`. Registered rule: if the lowest returned eigenvalue is below `-1e-8 |sigma|`
   (`sigma` the shift used; the zero modes of a free body come out at about `1e-12 sigma`, ADR 0384 addendum), the result is
   refused, `FemError::Solver { solver: "lowest_eigenpairs" }`, the reason saying that `K + K_sigma` is not positive
   semidefinite on the admissible motions (beyond the buckling load). No negative `omega^2` is ever returned.
6. Each mode `x = tr.expand(q)`, the mesh part is the mode; `ModalResult::from_parts(eig, modes, n_q, rigid_free, dofs)`.

`factors_on_transformation(model, prestress, options) -> Result<BucklingResult>` (private, `buckling.rs`):

1. Zero values only (as above, "a buckling analysis takes zero values only").
2. `tr`; `K_ext` (stiffness plus supports); `K_sigma`; `K_r = tr.reduce(K_ext, 0)`, `G_r = tr.reduce(K_sigma, 0)`.
3. The factor count against `n_q`; `lowest_buckling_factors(K_r, G_r)` with the options of D6 of ADR 0390. `K_r` must be positive
   definite: a mechanism left by the constraints and supports is refused by the factorisation of `K_r` (`EigenError::
   ShiftNotPositiveDefinite`), returned as `FemError::Solver { solver: "lowest_buckling_factors" }` (the geometric rigid-motion
   test of the prescribed path does not see springs, linear rows and rigid sets, ADR 0384).
4. Each mode `x = tr.expand(q)`, mesh part; `unknowns = n_q`; the rest as ADR 0390 D6.

A `Cyclic` constraint (ADR 0386) is on the transformation path too; `mpc::transformation` writes its `k = 0` rows, so both
functions give the harmonic-index-0 family of a sector, as `modal_analysis_mpc` does. Not checked here (limits).

### D3. `mpc.rs` (additive only)

- `pub(crate) fn refuse_nonzero_values(model: &SolidModel, analysis: &str) -> Result<()>`: the zero-value check of
  `modal_analysis_mpc` (prescribed rows first, then `Linear` values, the reason `"{row}: value {v:e}; {analysis} takes zero
  values only"`), written as a new function; `modal_analysis_mpc` and `zero_values_only` are not changed.
- `unknown_name` becomes `pub(crate)` (its body unchanged).

Nothing else in `mpc.rs` changes; no static function is touched.

### D4. Files

`crates/rcfd-fem/src/geometric.rs`, `buckling.rs` (D2), `mpc.rs` (D3), new test file `crates/rcfd-fem/tests/si_prestress_mpc.rs`,
the permitted edit of `si_geometric.rs` (D6). Not touched: `solve.rs`, `modal.rs`, `constraint.rs`, `assemble.rs`, `eigen.rs`,
any other test, golden or hash table.

### D5. The set-ups

Material, section and mesh of ADR 0390 D7 / ADR 0394 B20: `E = 2e11 Pa`, `nu = 0`, `rho = 7850 kg/m^3`, `h = b = 0.1 m`, `L = 2
m`, `A = 0.01 m^2`, `I = 8.3333e-6 m^4`; `hex_box([0, 0, 0], [L, h, b], [40, 2, 1])` upgraded to Hex20; node sets `ke0` (`x = 0,
y = h/2`, three nodes), `ke1` (`x = L, y = h/2`, three nodes), `every`; `Load::traction` `(s, 0, 0)` on `xmax` and `(-s, 0, 0)`
on `xmin`. Pre-stress `Prestress::from_static` of each model's own static solve (`StaticOptions::default()`), except where
stated.

- **B** (prescribed): ADR 0390's beam: `Displacement` `ke0` 0 and 1, `ke1` 1, `every` 2. The old path.
- **BL** (linear knife edges): `Displacement("every", 2, 0)` kept; for each `ke0` node `a`: `Linear [(a, 0, 1.0)] = 0` and
  `Linear [(a, 1, 2.5)] = 0`; for the first `ke1` node `r0`: `Linear [(r0, 1, 1.0)] = 0`, for each other `ke1` node `r`:
  `Linear [(r, 1, 1.0), (r0, 1, -1.0)] = 0` (a tie to the first).
- **BR** (rotated): B's mesh rotated by 30 degrees about `z` (each node `x -> R x`, `R` the rotation; sets kept); `Displacement
  ("every", 2, 0)`; for each `ke0` node `Linear [(a, 0, c), (a, 1, s)] = 0` and `Linear [(a, 1, c), (a, 0, -s)] = 0`; for each
  `ke1` node `Linear [(r, 1, c), (r, 0, -s)] = 0` (`c = cos 30`, `s = sin 30`); tractions rotated with the mesh, `s (c, sin 30, 0)` on `xmax`
  and `-s (c, sin 30, 0)` on `xmin`. **BR31**: the same with the knife-edge rows at 31 degrees (negative control).
- **BS(k)** (spring at the roller): B without the roller `Displacement("ke1", 1, 0)`, with `ElasticSupport::nodal_spring("ke1",
  1, k/3)` (`k/3` per node, three nodes: total `k`). `k = 1.0e6 N/m` (`k L = 2e6 N`, `lambda = 200`), `k = 1.5e6 N/m` (`lambda =
  300`) and `k = 1e11 N/m` (stiff: `k L = 2e11 N`, far above the bending branch). **BF**: B without the roller and without a
  spring (a mechanism, the old path). **BK**: B (roller kept) plus `nodal_spring("ke1", 1, 1e6/3)`.
- **BM** (mechanism on the transformation path): BL without its roller rows (pin rows only).
- **R** (rigid end): B plus `Constraint::rigid_set("xmax", [L, h/2, b/2])`.

Options: modes `ModalOptions::new(2).residual_test(ResidualTest::Backward)` (ADR 0390); buckling `BucklingOptions::new(2)`.

## Arithmetic before this registration (disclosed)

By hand and with `arith.py` (closed forms only; `|s_ref| A = 1e4 N`):

1. Bending branch (ADR 0394 lower roots): `P_1 = 4.083826e6 N` (`lambda = 408.3826`), `P_2 = 1.600433e7 N` (`lambda = 1600.4326`).
   ADR 0394 measured B: `408.40047`, `1600.70961`.
2. Sway branch: `k = 1e6`: `k L = 2e6 N`, `lambda = 200` (0.49 of the bending branch); `k = 1.5e6`: `lambda = 300` (0.73); `k =
   1e11`: `lambda = 2e7`, so the two lowest factors of BS(1e11) are the bending ones.
3. The deficit of the sway factor (D1): `c_0 k/(G b)`, `G b = 1e10 N/m`: `6e-5 c_0` at `k = 1e6`, `9e-5 c_0` at `1.5e6`. With `c_0`
   up to 5 (the line load on the knife edge), estimate `<= 5e-4` against the limit 1e-2 of (a3).
4. Second factor of BS(1e6): the knife-edge mode does not engage the spring in the Timoshenko column (D1); in the solid its
   roller reactions are of the order of the end mismatch `epsilon` times `P w'(L)`, and releasing the roller to a spring changes
   the factor by `~ R^2/(k x^T K x)`: with `epsilon <= 1e-2`, `<= 4e-4`. Estimate `|lambda_2(BS)/lambda_1(B) - 1| <= 4e-4`. BS(1e11):
   the penalty limit, `<= 1e-5`.
5. Pre-stressed shifts (ADR 0390 arithmetic 3): `Delta_n,ref = (n pi/L)^2 s/rho = 3.14319e4, 1.25727e5 rad^2/s^2` at `s = 1e8`;
   measured on B 6.8e-4 and 2.4e-3 (ADR 0390 A6). For R estimate within `3e-3` and `5e-3` (the rigid face adds an end effect of
   the order of D1's 5e-4).
6. BL against B: `T^T K T` equals `K_FF` entry for entry (D1), the Auto shift accepts zero for B's matrix (B is solved at
   `Shift::Value(0)`; the factor of the same matrix at zero is accepted if every pivot ratio is `>= 1e-8`: B's `K + K_sigma` is
   positive definite and its pivots are not near zero, ADR 0390 measured the modes at zero shift), the buckling solve the same;
   the remaining difference is the static pre-stress of `solve_static_mpc` against `solve_static`, both direct on the same
   reduced matrix: estimate `<= 1e-13` per value (ADR 0390 C5: the eigenvalues move 1.5e-11 for a pre-stress change of 6e-13
   relative; here the pre-stresses differ by rounding of order 1e-15).
7. BR against B: the assembly in rotated coordinates differs by rounding (ADR 0384 G1: 8.9e-16 for the eigenvalues), the
   independent unknowns are the same equations (D1), the pre-stress tensor is rotated: estimate `<= 1e-12`; BR31 tilts the
   knife-edge normals by 1 degree: the roller then also holds `u_x sin 1 deg`, the column axis is held axially at both ends (an
   axial restraint of the roller), and the pre-stress itself changes (the roller carries axial reaction): far above 1e-6.
8. Run time: B has 773 nodes, 2319 unknowns; ADR 0394's file (three buckling solves) ran in 0.18 s. This file has about 14
   static solves, 14 buckling solves and 14 modal solves of that size: estimate under 10 s in release.

## Registered checks

All in `crates/rcfd-fem/tests/si_prestress_mpc.rs`, release profile, run once: `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem
--release --test si_prestress_mpc -- --nocapture --test-threads=1`. Relative differences are `|x - y|/|y|`. "Bit-equal" compares
`f64::to_bits`.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | a1 | BL, BS(1e6) and R: `buckling::factors` (`s_ref`) and `prestressed_modes` (`s = +1e8`; BS(1e6) also `s = +1e8`) against the reduction in the test (`mpc::transformation`, `assemble::stiffness` + `mpc::add_supports`, `geometric_stiffness`, for the modes `K_ext + K_sigma` value by value and `mass::consistent`, `Transformation::reduce` of each with a zero vector, `lowest_buckling_factors` with the options of `BucklingOptions::new(2)` / `lowest_eigenpairs` with `Shift::Auto` and the modal options, `Transformation::expand`, mesh part): factors / eigenvalues and every mode value | bit-equal | D2 is that sequence |
| A2 | a2 | BL against B, each with its own static pre-stress: the two factors at `s_ref`; the two eigenvalues at `s = +1e8` and at `s = -1e8`: `max |x_BL/x_B - 1|` | <= 1e-10 | <= 1e-13 (arith. 6) |
| A3 | a3 | BS(1e6): `|lambda_1 |s_ref| A/(k L) - 1|`, `k L = 2e6 N`; and BF (no spring) refused with `RigidBodyMotionFree` (the factor the spring raises from) | <= 0.01; as stated | <= 5e-4 (arith. 3) |

### C. Companions

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | BR against B: the two factors and the four eigenvalues of A2 | each relative <= 1e-9 | <= 1e-12 (arith. 7) |
| C2 | BS(1.5e6): A3's measure with `k L = 3e6 N`; and `lambda_1(1.5e6)/(1.5 lambda_1(1e6)) - 1` in absolute value | <= 0.01; <= 1e-3 | <= 5e-4; <= 3e-4 |
| C3 | BS(1e6) and BS(1.5e6): the Rayleigh bound `lambda_1 <= (k L/(|s_ref| A)) (1 + 1e-9)` | holds | exact (D1) |
| C4 | BS(1e6): `|lambda_2/lambda_1(B) - 1|` and `|lambda_2 |s_ref| A/P_1 - 1|` (ADR 0394 root); BS(1e11): `|lambda_1/lambda_1(B) - 1|`, `|lambda_2/lambda_2(B) - 1|`; and `lambda_n(BS) <= lambda_n(B) (1 + 1e-9)`, `n = 1, 2`, for BS(1e6) and BS(1e11) (a spring in place of a constraint relaxes: min-max) | <= 1e-3 each; holds | <= 4e-4, 1e-5 (arith. 4); exact |
| C5 | BK (spring beside the roller) against B with B's pre-stress for both: the two factors | each relative <= 1e-12 | equal (D1) |
| C6 | R: `|(omega_n,t^2 - omega_n,0^2)/Delta_n,ref - 1|`, `n = 1, 2`, `omega_n,0^2` from `modal_analysis` of R (ADR 0384 path), `omega_n,t^2` at `s = +1e8`; and the two factors of R against ADR 0394's roots `|lambda_n |s_ref| A/P_n - 1|` | each <= 0.01 | 3e-3, 5e-3; <= 1e-3 |
| C7 | R against B (min-max, D1): `lambda_n(R) >= lambda_n(B) (1 - 1e-9)` for the two factors and `omega_n^2(R) >= omega_n^2(B) (1 - 1e-9)` at `s = 0` and `s = +1e8`, `n = 1, 2` | holds | exact up to rounding |
| C8 | identification (ADR 0390 D8 correlation over the neutral line `y = h/2, z = 0`): BS(1e6) mode 1 against `x/L` (the sway), mode 2 against `sin(pi x/L)`; R's two buckling modes and its two modes at `s = +1e8` against `sin(n pi x/L)`; BL's two buckling modes against `sin(n pi x/L)` | each >= 0.99 | > 0.999 |
| C9 | zero pre-stress (`Prestress::uniform(model, [0; 6])`) on the transformation path: `prestressed_modes` of BS(1e6) and of R against `modal_analysis` of the same model | eigenvalues and every mode value bit-equal | equal (`K_ext + 0` value by value) |
| C10 | constraint rows on the expanded modes: BL and BR, every `Linear` row `c . x` rebuilt in the test on each buckling mode and each mode at `s = +1e8`: `|c . x|/(max_k |c_k| |x|_inf)` | <= 1e-12 | ~1e-16 |
| C11 | threads: BS(1e6) `buckling::factors` with `threads(1)` and `threads(8)`, `prestressed_modes` of R with `threads(1)` and `threads(8)` | bit-equal | equal |
| C12 | every buckling run of A and C returned two factors with the Sturm record counted = expected = 2; every modal run is `rigid_free == false` with shift 0 (the Auto rule accepted zero) | as stated | D2 |
| C13 | refusals: BL with one `Linear` value `1e-3` (the first pin row): both functions `InvalidValue { key: "constraint" }`, reason containing "(Linear)" and "zero values only"; BL at `s = -5e8` (beyond the buckling stress 4.08e8): `prestressed_modes` returns `FemError::Solver` (no negative eigenvalue); BM: `buckling::factors` returns `FemError::Solver` (a mechanism: `K_r` not positive definite) | each as stated | D2 |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | A3's measure with the spring counted per node (`(k/3) L` for `k L`) | `> 0.01` (about 2: the total stiffness of the three nodes is seen) |
| N2 | C1's measure for BR31 against B (the two factors) | `> 1e-6` (the knife-edge direction is seen) |
| N3 | A2's factor measure for BL run with B's static pre-stress scaled by `1 + 1e-8` against B | `> 1e-10` (about 1e-8: the limit of a2 sees a pre-stress change of 1e-8) |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `cargo test -p rcfd-fem --release` (every test of the crate; `si_geometric::c7_refusals` ignored as a record, its copy passing; ADR 0390's and ADR 0394's tests passing) | 0 failed |
| S2 | `cargo check -p rcfd-post --tests` | builds |
| S3 | `git diff 6b29e2b9 --stat -- crates`: only `geometric.rs`, `buckling.rs`, `mpc.rs` (D3 only), the new test file, `si_geometric.rs` (D6 only) | as stated |

`rcfd-api`'s `baseline_hashes` is not run: the brief limits the tests to `rcfd-fem` (memory shared with two other building
agents); no line executed by a static, modal, transient or harmonic solve changes (the edits of `mpc.rs` add a function and
widen a visibility; `geometric.rs` and `buckling.rs` are called by no `rcfd-api` path). Recorded, not hidden.

### Mutations (scratch copy `w49i-work/f493-scratch/copy`, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | `factors_on_transformation` without the supports (`add_supports` dropped) | A3 (BS refused: the sway is free), A1 |
| Mu2 | the modes of both functions expanded as a selection (`q` at the independent unknowns, 0 elsewhere) instead of `T q` | A1, C10 (BR) |
| Mu3 | `prestressed_modes_on_transformation` with `K_ext - K_sigma` | A2, C6 |
| Mu4 | the negative-eigenvalue guard (D2 step 5) removed | C13 (BL beyond the buckling stress returns modes) |
| Mu5 | the zero-value check removed from both functions | C13 (the non-zero `Linear` value accepted) |
| Mu6 | `factors_on_transformation` with `G_r` reduced from `2 K_sigma` | A1, A2, A3 (factors halved) |
| Mu7 | `M_r` replaced by its diagonal | A1, A2 |

## Decision rule

- **done**: A1 to A3 met, every C and S check met, the negative controls as expected, Mu1 to Mu7 caught.
- **partial**: any A check missed with a diagnosed cause, kept as a `*_known_miss` record; or a C or S check missed, a negative
  control not as expected, or a mutation surviving.
- **todo**: the checks could not be run.

### D6. The retired refusal row

`si_geometric.rs::c7_refusals` (ADR 0390 C7) asserts, among valid rows, that `buckling::factors` of B with an added nodal spring
is refused (`InvalidValue`, key `"constraint"`); after this package it returns factors. By the maintainer's standing decision of
2026-11-13 the test is marked `#[ignore = "record of ADR 0390 C7 before ADR 0398 (F-493) retired the nodal-spring row; superseded
by c7_refusals_after_transformation_landed (maintainer, 2026-11-13)"]`, body untouched, and a copy
`c7_refusals_after_transformation_landed` with the other four rows (pre-stress of B on C, B without its pin, modes beyond the
buckling stress; the element rows are in another test) is added at the end of the file.

## Limits of this registration

- Zero constraint values only; no pre-load (a fixed load plus a scaled one), as ADR 0390.
- The supports carry no initial stress; a pre-stressed spring (a spring with a static force whose direction rotates) is not
  modelled.
- Rigid sets: their reference points carry no mass and no geometric stiffness of their own.
- Cyclic constraints: the `k = 0` family only (D2), not checked here; buckling at a non-zero harmonic index is not covered.
- A free body under a compressive self-equilibrated pre-stress has negative initial-stress energy in its rigid rotations; such a
  model is refused by the guard of D2 step 5 as beyond the buckling load.
- One beam, one mesh, `nu = 0`; the spring check at the sway branch only, plus the bending branch at one `k`.
- The workspace suite and `rcfd-api` are not run (the lead's gate; the brief).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The reading of (a3) as the roller replaced by a lateral spring (above). Taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).
2. The new code is private to `geometric.rs` and `buckling.rs`, reached by the dispatch of the existing public functions; no
   `MpcModes`-like result with the reference motions is returned for these analyses. Taken on the maintainer's standing
   instruction of 2026-11-11 (take the recommendation).
3. The default shift on the transformation path is `Shift::Auto` (ADR 0384's rule), with the guard against negative eigenvalues
   beyond the buckling load (D2 step 5), rather than the zero shift of the prescribed path. Taken on the maintainer's standing
   instruction of 2026-11-11 (take the recommendation).

## Amendment 1 (2026-11-13, written while writing the test file, before any run of a check)

1. **N2 replaced.** Found while writing the test, by the arithmetic of D1, before any run: the knife-edge beam is insensitive to
   the direction of the roller row. Tilting the roller normal by 1 degree couples the end deflection to the axial displacement
   of the neutral axis at the roller, `cos 1 deg w(L) = sin 1 deg u_x(L)`; the static solution stays the uniform stress plus a
   rigid rotation (strain free, so the pre-stress is unchanged), and in the buckling mode `u_x = 0` on the neutral axis, so the
   tilt acts as a lateral spring of about `E A/(L tan^2 1 deg) = 3e12 N/m` on a roller whose reaction in the mode is near zero
   (D1: the knife-edge mode does not load the roller in the Timoshenko column). The factors would move by far less than the
   expected 1e-6, so BR31 does not test what N2 says. (Any two independent directions at the pin hold the same motions, so the
   pin rows are no better.) Corrected control: **N2: C1's factor measure for BR with its tractions along the global `x` axis,
   `(+-s_ref, 0, 0)`, not rotated with the mesh; expected `> 1e-6`** (the axial stress becomes `s_ref cos 30` and the end shear
   tractions form a couple carried by the knife edges, about 0.15). BR31 is not run. The other checks are unchanged.
2. **D6 wording.** The copy `c7_refusals_after_transformation_landed` keeps the **three** other rows of C7 (pre-stress of B on
   C, B without its pin, modes beyond the buckling stress); D6 said "four". No change of a check.

## Amendment 2 (2026-11-13, after the registered run of commit e6b7972a, before the corrected run)

The registered run (`f493-scratch/registered_run1.log`): 3 tests passed, 2 failed.

1. **BR a defective set-up.** `a2_c1_c8_c10_c12_n2_n3_linear_knife_edges` stopped at the static solve of BR: "constraint 1
   (Linear) closes a chain on node 41 component 1 through constraint 0 (Linear)". The second pin row was written `[(a, 1, c),
   (a, 0, -s)]`; its designated dependent (the first term, `u_y`) appears in the expression of the first row's dependent `u_x`
   (`u_x = -(s/c) u_y`), which the elimination of ADR 0296 refuses by design. The checks before it ran and are recorded (A2: 0,
   0, 0; C8 BL 1.000000, 1.000000; C10 BL 0); C1, C10 for BR, N2 and N3 did not run. Correction: **BR2**, the same model with
   the second pin row written `[(a, 0, -s), (a, 1, c)]` (designated `u_x`, already a dependent, so the row takes the largest
   remaining coefficient, `u_y`, as ADR 0296 does); the roller rows are unchanged (`u_y` first, `u_x` independent, as D1 wants).
   The test is kept as the ignored record `a2_c1_c8_c10_c12_n2_n3_linear_knife_edges_defective_setup` (it builds the old BR,
   `Kind::Br`), and the corrected test `a2_c1_c8_c10_c12_n2_n3_linear_knife_edges_corrected` is the same body with `Kind::Br2`
   (N2's model is built from BR2). It re-computes A2 deterministically; A2's registered values are those of the first run, and
   the corrected run is expected to reproduce them bit for bit (reported). Limits unchanged.
2. **C13 missed (not a set-up defect).** BM (the knife-edge beam without its roller rows, a mechanism on the transformation path)
   returned a factor `1.8176e-8` instead of a refusal: the factorisation of `K_r` accepted a rounding-level positive pivot of the
   free rotation (`PivotRule::Positive` refuses only `d_i <= (w + 1) eps (|a_ii| + sum)`), and the buckling eigensolver then
   found the null direction as a factor. D2 step 3 relied on the factorisation; it does not refuse a numerically singular `K_r`
   reliably. The library is not changed after the run. The test is kept as the ignored record
   `c9_c13_zero_prestress_and_refusals_known_miss` (limit intact, measured value in its reason), and its met rows (C9, the two
   non-zero-value refusals, the refusal beyond the buckling stress) stay active in `c9_c13_zero_prestress_and_refusals_met_rows`
   (the same body without the BM row). The cause and the options are in the addendum.

The two test bodies were moved into functions taking the variant (`linear_knife_edges(kind)`, `zero_prestress_and_refusals(
with_mechanism)`), each record calling it with the arguments of the first run, so that the records keep their bodies.
