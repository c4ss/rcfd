# ADR 0384: F-486 registration: modal analysis on the transformation (linear constraints, rigid sets and elastic supports)

Status: **REGISTERED 2026-11-13, written before any code of F-486 and before any run of its checks**. Not yet accepted by
the maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0296](0296-linear-multi-point-constraints-rigid-sets-elastic-supports.md) (F-393: the transformation `x = T q + g`
in `rcfd_fem::mpc`; its D7 refuses these constraints in the modal analysis and its remaining question 2 proposes this item).
Modal analysis: ADR 0284 (`rcfd_fem::modal`), eigensolver ADR 0283 (`rcfd_linalg::eigen`). Roadmap item F-486 (depends on
F-393), needed later by the cyclic modes (F-394, ADR 0297). Number 0384 given by the lead. Branch `f486-modal-transformation`,
worktree `w49i-work/f486`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (every
model without a linear constraint, a rigid set or an elastic support runs exactly the modal code it ran before); no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; no existing test edited except the
one permitted by the maintainer's decision of 2026-11-13 for `si_scaffold.rs` (below); clean-room.

## What F-486 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. modal_analysis of a model with linear constraints,
> rigid sets or elastic supports reduces K and M by the transformation of ADR 0296 (T^T K T, T^T M T; zero right-hand sides
> only, a non-zero value refused naming the constraint) and expands the modes by T. A model with prescribed displacements only
> gives the bits of today's modal analysis. A bar (nu = 0) on an end spring k has its first axial frequency from the
> closed-form transcendental equation within 0.5 percent at 20 elements; the modes are M-orthonormal within 1e-10 and every
> constraint row holds on each expanded mode within 1e-12; a model held only by springs uses the automatic shift when a rigid
> motion is free.

Clauses: (c1) `modal_analysis` reduces `K` and `M` by `T` and expands the modes by `T`; zero right-hand sides only, a
non-zero value refused naming the constraint; (c2) prescribed-only models keep the bits of today's modal analysis; (c3) bar on
an end spring, first axial frequency within 0.5 percent at 20 elements; (c4) modes M-orthonormal within 1e-10; (c5) every
constraint row holds on each expanded mode within 1e-12; (c6) a model held only by springs uses the automatic shift when a
rigid motion is free.

## What was read (main e05871f6, read only)

1. `AGENT_RULES.md`; ADR 0296 (method, D1 to D8, checks, addendum, remaining question 2 and the proposed acceptance); the
   brief (the maintainer's decision of 2026-11-13 on `si_scaffold.rs`).
2. Code: `crates/rcfd-fem/src/mpc.rs` (all: `transformation`, `Transformation::reduce`, `expand`, `add_supports`,
   `refuse_in_modal`, `singular`), `modal.rs` (all), `constraint.rs` (`prescribed_equations`, `check_rigid_body_motions`),
   `crates/rcfd-linalg/src/eigen.rs` (`Shift`, `factor_shifted`, the Auto rule with `AUTO_PIVOT_RATIO = 1e-8`,
   `lowest_eigenpairs`, `GeneralizedEigen`), `skyline.rs` (`SkylineOptions`, `PivotRule`, `factor_with`, `pivots`, `order`).
   Tests: `crates/rcfd-fem/tests/si_mpc.rs` (helpers, the K, S and R models, `n4_modal_refuses_the_new_data`),
   `si_scaffold.rs` (the `_after_landed` pattern, `r11_modal_and_stiffness_refuse_after_landed`), `sb_modal.rs` (the fixed-free
   bar of ADR 0284, `bar_discrete`, `axial_fractions`, the free box and its N2).
3. **Not read:** any finite-element code or manual outside this repository.

Arithmetic done beforehand (disclosed): the roots of the transcendental equations below were computed with a short Python
script (bisection) and, for the spring bar, a dense 21-unknown 1-D chain (numpy, generalised symmetric eigenvalues) to
estimate the discretisation error; no rcfd code was compiled or run. Values: `kappa = k L/(E A) = 0.25`, `z_1 = 0.480094437`,
`c = sqrt(E/rho) = 5047.5447 m/s`, `f_1 = z_1 c/(2 pi L) = 385.67987 Hz`; the 20-element chain gives `385.68913 Hz`, relative
`+2.40e-5`; the discrete closed form below gives the same `385.68913 Hz` (to the printed digits). Clamped-free bar `c/(4L) =
1261.886 Hz`, the 20-element chain `+2.57e-4`.

## Method and derivation

**Reduction.** ADR 0296 writes every admissible motion as `x = T q + g`. In a modal analysis every constraint value is zero,
so `g = 0` and the admissible motions are exactly the range of `T`. The kinetic and potential energies restricted to `x = T q`
are `1/2 qdot^T (T^T M_ext T) qdot` and `1/2 q^T (T^T K_ext T) q`; Lagrange's equations give `K_r q = lambda M_r q`, `K_r = T^T
K_ext T`, `M_r = T^T M_ext T`, `lambda = omega^2`, and each mode is `x = T q` (Rayleigh-Ritz on the subspace `range(T)`,
derived; the same congruence as ADR 0296's static reduction). `K_ext` is the stiffness with the supports added (ADR 0296 D4);
`M_ext` is the consistent mass padded with zero rows and columns at the six reference unknowns of each rigid set (the reference
point has no mass of its own; the mass of the set's nodes reaches the reference unknowns through `T`). If `q^T M_r q' =
delta`, then `x^T M_ext x' = delta` for the expanded modes: M-orthonormality carries over (clause c4). Every constraint row
holds on `T q` for any `q` by construction (clause c5; a dropped redundant row holds to its residue).

**Zero right-hand sides.** A non-zero value (a `Displacement` constraint, a per-node prescribed value or a `Linear` value)
would make the admissible set affine, not a subspace; there is no modal analysis of it. Refused with `InvalidValue { key:
"constraint" }` whose reason names the row as ADR 0296 D5 describes it ("constraint 3 (Linear)", "the prescribed displacement
of node 12 component 0") and says "a modal analysis takes zero values only". A rigid set's rows have the value 0 always.

**Shift rule (clause c6).** The constraints of ADR 0296 hold motions that the geometric test of ADR 0275 D5 does not see
(springs, rigid sets, linear rows; ADR 0296 D3 makes the same remark for the static path). On the transformation path the
default shift is therefore `Shift::Auto` (ADR 0283 D3): its first candidate is `sigma = 0`, accepted when the factor of `K_r`
has every pivot ratio `d_i / a_ii >= 1e-8`; then the result is the same as `Shift::Value(0.0)` (the same matrix `K_r + 0 M_r`,
the same factorisation). A free rigid motion makes `K_r` singular and the rule moves to a positive shift. `rigid_free` is
reported as "the zero shift was not accepted": `eig.shift != 0` for the default; with a shift given by the caller, the same
test (a factor of `K_r` with `PivotRule::Positive`, RCM, ratio `1e-8`) is made separately. This reports a mechanism (a zero
energy motion that is not rigid) as free too; a model that ill-conditioned is shifted harmlessly (the eigenpairs of `K - lambda
M` do not depend on the shift).

**Masses.** A reference unknown of a rigid set whose rotation does not move any node (a collinear set, about its line) has a
zero column in `T` apart from itself: zero mass and zero stiffness. The existing check "no mass (diagonal ...)" of `modal.rs`
is applied to `M_r` and names the unknown: a mesh unknown by node and component, a reference unknown as "the reference rotation
y of constraint 3 (RigidSet)".

**Closed forms (clause c3).** Bar `0 <= x <= L`, `nu = 0`, end `x = 0` on a spring `k`, end `x = L` free. Axial motion `u(x)
cos(omega t)`, `u'' + beta^2 u = 0`, `beta = omega/c`, `c = sqrt(E/rho)`. At `x = 0`, `E A u'(0) = k u(0)` (the force of the
bar on its end equals the spring force; static check: a pull `F` at `L` gives `N = F`, `u(0) = F/k`); at `x = L`, `u'(L) = 0`.
With `u = cos(beta (L - x))`: `u'(L) = 0`, and `E A beta sin(beta L) = k cos(beta L)`, i.e. **`z tan z = kappa`**, `z = beta L`,
`kappa = k L/(E A)`. First root `z_1` in `(0, pi/2)`; `f_1 = z_1 c/(2 pi L)`.

The 3-D model reduces exactly to the 1-D chain: with `nu = 0` the elasticity matrix does not couple the normal strains, the
motion `u(x) e_x` (trilinear, uniform over the section) has no shear strain and no lateral stress, so `K` maps it to forces
along `x` only, equal at the four nodes of a section; the consistent mass restricted to it is the 1-D consistent mass `rho A h/6
[[2, 1], [1, 2]]`. Rollers on `ymin` (`u_y = 0`) and `zmin` (`u_z = 0`) do no work on it. So `u(x) e_x` with the chain's
eigenvector is an exact discrete eigenvector of the 3-D model. **Discrete closed form** (derived): interior nodes `u_j = cos(theta
(n - j))` satisfy the chain's equation for `lambda(theta) = (6 c^2/h^2)(1 - cos theta)/(2 + cos theta)` (as `bar_discrete` of
ADR 0284 D5); the free end `j = n` holds by the symmetry of `cos` about `j = n`; the spring end `j = 0` requires `r(theta) =
(E A/h)(u_0 - u_1) + k u_0 - lambda(theta) (rho A h/6)(2 u_0 + u_1) = 0`. `r -> k > 0` as `theta -> 0` and `r(pi/(2n)) = -(E A/h
+ lambda rho A h/6) sin(pi/(2n)) < 0`: the root in `(0, pi/(2n)]` is found by bisection (200 halvings) in the test; computed
beforehand `theta_1 = 0.0240047`, `f = 385.68913 Hz`.

Clamped-free (the axial roller of ADR 0284's bar at `x = 0`): `f_1 = c/(4 L)` and its discrete form `bar_discrete` (copied
from `sb_modal.rs`). With a rigid set on the end face `xmax` (reference at the face centre) the axial eigenvector of the bar
without the set is admissible (`t_x = u(L)`, all other reference unknowns 0) and is an eigenvector of the unreduced problem,
so `T^T (K - lambda M) x = 0`: it is an eigenpair of the reduced problem too, with the same frequency.

**Rotation invariance.** A block rotated by 30 degrees about `z` with the roller constraints written as `u . e1 = 0`, `u . e2
= 0` (Linear, ADR 0296's model S) is the same body as the unrotated block with `Displacement` rollers on `xmin` (x), `ymin`
(y) and `zmin` (z). Its eigenvalues are equal and its modes are the rotated modes. This compares the transformation path with
the old path on a model whose rows have coefficients `cos 30`, `sin 30`.

## Design

### D1. `mpc::modal_analysis_mpc` (new public function in `mpc.rs`)

`modal_analysis_mpc(model, extra, options) -> Result<MpcModes>`: validate; `prescribed_equations` and every `Linear` value
checked for zero (refusal above, in the order prescribed rows then constraints); `tr = transformation(model, extra)`; `K =
stiffness_with(model, threads)` plus `add_supports`; `M = consistent_with(model, threads)`; `(K_r, _) = tr.reduce(&K, 0)`, `(M_r,
_) = tr.reduce(&M, 0)` (the reduction of ADR 0296 D2, unchanged; zero load vector over all `N` unknowns); the mode count check
of `modal.rs` against `n_q`; the mass check on `M_r` (names above); the shift rule above; `lowest_eigenpairs(K_r, M_r, ...)`
with the options of the call; each mode `x = tr.expand(q)`, the mesh part `x[..n]` is the mode of the `ModalResult`.
`MpcModes { result: ModalResult, transformation, reduced_stiffness, reduced_mass, expanded (x over all N unknowns per mode,
so that the reference motions are available) }`.

### D2. Dispatch (shared file `modal.rs`)

In `modal_analysis_with` the line `crate::mpc::refuse_in_modal(model)?;` (ADR 0296 D7) becomes `if
crate::mpc::uses_transformation(model) { return crate::mpc::modal_analysis_mpc(model, extra, options).map(|s| s.result); }`.
Every other model runs the old lines (clause c2: `refuse_in_modal` returned `Ok` for them, so nothing they compute changes).
A `pub(crate)` constructor of `ModalResult` (its field `dofs` is private to `modal`) is added. `refuse_in_modal` stays public
(no caller in the library any more; its doc says so). A model with a `Cyclic` constraint is still refused (by
`check_supported` or `prescribed_equations`).

### D3. Files

`crates/rcfd-fem/src/mpc.rs` (D1), `modal.rs` (D2), new test file `crates/rcfd-fem/tests/si_modal_mpc.rs`, and the one
permitted edit of `crates/rcfd-fem/tests/si_scaffold.rs` (below). Not touched: `solve.rs`, `constraint.rs`, `model.rs`,
`error.rs`, `loads.rs`, `hex20.rs`, any other existing test, golden or hash table.

## Registered checks

All in `crates/rcfd-fem/tests/si_modal_mpc.rs`, release, run once (`cargo test -p rcfd-fem --release --test si_modal_mpc --
--test-threads=1 --nocapture`). Material `E = 2e11` Pa, `rho = 7850` kg/m^3, `nu` as stated. `ModalOptions::new(m)` defaults
(tolerance 1e-10, relative residual) unless stated. "Bit-equal" compares `f64::to_bits`.

Models:

- **KB (spring bar)**: `hex_box([0,0,0], [1.0, 0.1, 0.1], [20, 1, 1])`, `nu = 0`, `NodalSpring("xmin", 0, k/4)` (four nodes),
  `k = 5e8` N/m, `Displacement("ymin", 1, 0)`, `Displacement("zmin", 2, 0)`; 4 modes. `kappa = 0.25`.
- **KF (springs only)**: KB without the two rollers; 8 modes. Free: translations `y`, `z` and the rotation about `x` (the
  springs act along `x` only; they hold the translation `x` and, being at `z = 0, 0.1` and `y = 0, 0.1`, the rotations about
  `y` and `z`).
- **RB (rigid end)**: the bar of ADR 0284 at 20 elements (`hex_box` as KB, `Displacement("xmin", 0, 0)`, `ymin` 1, `zmin` 2)
  plus `RigidSet("xmax", (1.0, 0.05, 0.05))`; 4 modes.
- **RC (collinear rigid set)**: RB's mesh with a node set `edge` = the nodes of `xmax` with `z = 0` (two nodes, along `y`),
  `RigidSet("edge", (1.0, 0.05, 0.0))` instead of the one on `xmax`; 4 modes.
- **SR (skew rollers)**: ADR 0296's S geometry (`hex_box([0,0,0], [1.0, 0.5, 0.4], [4, 2, 2])` rotated by 30 degrees about
  `z`), `nu = 0.3`, Linear `u . e1 = 0` at every `xmin` node and `u . e2 = 0` at every `ymin` node, `Displacement("zmin", 2,
  0)`, no load; 6 modes. **SA**: the unrotated mesh with `Displacement` `xmin` 0, `ymin` 1, `zmin` 2; 6 modes (old path).
- **CP (cantilever)**: `hex_box([0,0,0], [1.0, 0.1, 0.1], [10, 2, 2])`, `nu = 0.3`, `xmin` clamped (three `Displacement`); 4
  modes. **CL1**, **CL2**: the clamp written as one single-term Linear constraint per node and component, coefficient 1
  (CL1) or 2.5 (CL2), value 0.
- **CN**: CP plus Linear `u_z(a) - u_z(b) = 1e-3` (`a`, `b` two `xmax` nodes; constraint index 3). **KN**: KB plus
  `Displacement("xmax", 0, 1e-3)` (index 2). **KE**: KB with a per-node prescribed value `1e-3` (node 0, component 1) through
  `modal_analysis_with`.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| P1 | c2 | CP: `uses_transformation` false; `modal_analysis` against the elimination done in the test (`prescribed_equations`, `stiffness`, `consistent`, `reduce` of both, `lowest_eigenpairs` with `Shift::Value(0.0)` and the default options, modes expanded with zeros at the prescribed equations): eigenvalues, every mode value, shift, `unknowns` | bit-equal | D2: the old lines run |
| P2 | c2 | existing tests unedited, release: `sb_modal`, `sb_modal_scale`, `si_scaffold_bits` | pass | D2 |
| L1 | c1, c2 | CL1 and CL2 against CP: `max_i |lambda_i - lambda_i^CP| / lambda_i^CP`; `rigid_free` false and shift 0 for both | `<= 1e-12`; as stated | single-term rows give `T` = selection and `K_r = K_FF` (ADR 0296 L1 measured 0 and 3e-16 statically) |
| D1 | c1 | KB, RB, SR: the result against the reduction done in the test (`mpc::transformation`, `stiffness` + `mpc::add_supports`, `consistent`, `Transformation::reduce` of both with a zero vector, `lowest_eigenpairs` with `Shift::Auto` and the default options, `expand`): eigenvalues, every mode value (mesh part), shift | bit-equal | D1 is that sequence |
| Z1 | c1 | CN, KN, KE: `Err(InvalidValue { key: "constraint" })`, reason contains "zero values only" and, for CN, "constraint 3 (Linear)"; for KN and KE, "the prescribed displacement of node" | as stated | D1 |
| B1 | c3 | KB: mode 0 is axial (kinetic share of the `x` components `>= 0.99`); `|f_0 / f_1 - 1|`, `f_1` from `z tan z = kappa` | `<= 5e-3` | `2.40e-5` (the chain, computed beforehand) |
| B2 | c3 | KB: `|f_0 / f_d - 1|`, `f_d` from the discrete closed form (bisection) | `<= 1e-9` | exact reduction to the chain; rounding and the residual tolerance (eigenvalue error of the order of the squared residual) |
| B3 | c1 | RB: mode 0 axial (`>= 0.99`); `|f_0 / (c/(4L)) - 1|`; `|f_0 / f_bar_discrete - 1|` | `<= 5e-3`; `<= 1e-9` | `2.57e-4`; exact (derivation above) |
| O1 | c4 | KB, KF, RB, SR: `max_ij |phi_i^T M phi_j - delta_ij|` with the full consistent `M` of the mesh and the expanded modes (mesh part) | `<= 1e-10` | `M_r`-orthonormal Ritz vectors, a few ulp times the subspace size (ADR 0284 measured ~1e-15) |
| H1 | c5 | KB, KF, RB, SR: every constraint row rebuilt in the test from the model (each prescribed equation; each Linear constraint; each rigid-set row `u_(a,c) - t_c - (w x d_a)_c` with the reference unknowns from `expanded`) on every expanded mode `x`: `|c . x| / (max_k |c_k| |x|_inf)`; and every prescribed equation exactly 0 | `<= 1e-12`; 0 | rows hold on `T q` by construction: the rounding of the Gauss-Jordan coefficients, ~1e-16 |
| A1 | c6 | KF: `rigid_free` true; shift `> 0`; `max_(i<3) |f_i| / f_3 <= 1e-6`; the lowest of the 8 modes with an `x` share `>= 0.99` has `|f / f_0^KB - 1|` | as stated; `<= 1e-9` | three free motions; the axial eigenvector is the same discrete chain as in KB (the rollers do no work on it) |
| A2 | c6 | KB, RB, SR: `rigid_free` false and shift 0 | as stated | `K_r` positive definite, pivot ratios far above 1e-8 for these small models |
| G1 | c1 | SR against SA: `max_i |lambda_i^SR / lambda_i^SA - 1|`; for every mode `i` with relative gap `min_(j != i) |lambda_j - lambda_i| / lambda_i >= 1e-3` (gaps from SA): `|R phi_i^SA -/+ phi_i^SR|_inf / |phi_i^SA|_inf` (sign by the dot product), `R` the rotation | `<= 1e-10`; `<= 1e-6` | the same body; rounding of the assembly in rotated coordinates, ~1e-13; the vectors to the residual tolerance over the gap, ~1e-7 |
| T1 | D1 | KB with `threads(1)` and `threads(8)`: eigenvalues and modes | bit-equal | stiffness and mass are thread-independent (ADR 0275); the reduction sums in a fixed order |

**Negative controls** (each must hold):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | KF with `shift(Shift::Value(0.0))` | `Err(FemError::Solver { .. })` (the free motions are real) |
| N2 | RC | `Err(InvalidValue { key: "density" })` whose reason contains "rotation y" and "constraint" |
| N3 | KB's `f_0` against the closed form with `k` doubled (`kappa = 0.5`) | relative difference `> 1e-2` (B1 discriminates the spring) |
| N4 | SR with the roller normals at 31 degrees against SA | `max_i |lambda_i^SR / lambda_i^SA - 1| > 1e-6` (G1 sees the constraint direction) |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f486-scratch`, own target directory; each applied
alone by exact text replacement, `si_modal_mpc` run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the supports are not added to `K` in `modal_analysis_mpc` | B1 |
| MU2 | the modes expanded as a selection (`q` at the independent unknowns, 0 at the dependents) instead of `T q` | H1 |
| MU3 | `M_r` replaced by its diagonal | O1 |
| MU4 | the check of the Linear values removed | Z1 |
| MU5 | `rigid_free` always false on the transformation path | A1 |
| MU6 | the default shift `Shift::Value(0.0)` instead of `Shift::Auto` on the transformation path | A1 |
| MU7 | the mass check on `M_r` removed | N2 |
| MU8 | `modal_analysis_with` dispatches every model to the transformation | P1 (a survival is recorded and explained, not repaired: with prescribed rows only `K_r = K_FF` and Auto accepts zero, so the bits may be the same) |

## Decision rule

**Done** if every check meets its limit, every negative control holds, MU1 to MU7 are caught by their registered checks on
the first run, and the only failing existing tests are those listed under "Expected to fail" (needing the maintainer);
**partial** if a check misses or is not run, a negative control fails, a mutation MU1 to MU7 survives, or another existing test
fails; **todo** if the reduced eigenproblem cannot be built or a prescribed-only bit changes (P1, P2).

## Expected to fail, needs the maintainer

1. `crates/rcfd-fem/tests/si_scaffold.rs`, `r11_modal_and_stiffness_refuse_after_landed`: its case "R8 modal" expects the
   refusal "a follow-up of ADR 0296 (modal analysis on the transformation)", which this package retires. By the maintainer's
   decision of 2026-11-13 it is marked `#[ignore]` with a reason naming this ADR as a record (its body untouched), and a copy
   `r11_modal_and_stiffness_refuse_after_modal_landed` without the R8 modal case is added (R4 modal and R4 stiffness kept).
2. `crates/rcfd-fem/tests/si_mpc.rs`, `n4_modal_refuses_the_new_data` (ADR 0296 N4): it asserts that `modal_analysis` refuses
   a Linear tie and a spring. The decision of 2026-11-13 covers `si_scaffold.rs` only, so this test is not edited; it fails
   once this package lands and is listed for the maintainer with the same pattern proposed (an `#[ignore]` record naming ADR
   0384; nothing of it remains valid, so no copy).

## Limits (what is not covered)

- Zero constraint values only; no prestressed or non-zero-offset modal analysis.
- The harmonic, transient and buckling analyses are not changed (they do not call `modal_analysis_with`).
- `rigid_free` on the transformation path is the pivot test of the Auto shift, not a geometric count of free rigid motions; a
  mechanism is reported as free too. The number of free motions is not reported.
- Cyclic stays refused (ADR 0297). Preconditioned solvers are not involved (the eigensolver uses the skyline factor).
- The reference points carry no mass of their own (no point mass element exists).
- Only Hex8 models are checked here (the transformation is element-independent; Tet10 statics are checked in ADR 0296).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The shift rule on the transformation path is the Auto rule's acceptance of the zero shift (above), instead of a
   generalisation of the geometric rigid-motion test to springs, linear rows and rigid sets. Taken on the maintainer's standing
   instruction of 2026-11-11 (take the recommendation).
2. The reference motions of each mode are returned by `mpc::modal_analysis_mpc` (`MpcModes::expanded`), and `ModalResult` is
   not given a new field (its `Debug` and `PartialEq` stay as they are). Taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).
3. `refuse_in_modal` stays public with no caller in the library, rather than being removed (no public item is removed).
   Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
