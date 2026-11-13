# ADR 0399: F-488 registration: zero modes on the transformation (the eigenvalue measure against the shift, the count of free rigid motions, the explicit-shift report)

Status: **REGISTERED 2026-11-13, written before any code of F-488 and before any run of its checks.** Not yet accepted by
the maintainer. Before this text nothing of F-488 was written, compiled or run; the arithmetic below was done by hand from the
numbers recorded in ADR 0384 (disclosed). Results go in the addendum at the end; nothing above the addendum is changed after a
run.

Parent: [ADR 0384](0384-f486-registration-modal-analysis-on-the-transformation.md) (F-486: `mpc::modal_analysis_mpc`), its
known miss A1 (the three zero modes of the springs-only bar KF at `2.07e-6` of the first flexible frequency against a
registered `1e-6`, kept as the ignored record `a1_o1_h1_springs_only_and_n1_known_miss`, which this ADR does not touch) and its
remaining question 1, option (b), taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
Transformation: ADR 0296. Eigensolver and the `Shift::Auto` rule: ADR 0283 D3. Roadmap item F-488 (deps F-486). Number 0399
given by the lead. Branch `f488-zero-modes-transformation`, worktree `w49i-work/f488`, scratch `w49i-work/f488-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (nothing
that exists changes its result: the package adds one new function and its module, no existing function is edited); no edit of
`baseline_hashes.rs`, the hash tables or a golden file; no new dependency, no unsafe code; no existing test edited (this
package retires no refusal row); clean-room.

## What F-488 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. For a body held only by springs (ADR 0384's KF and a
> free block with one spring), the zero modes found with the automatic shift satisfy |lambda_i| <= 1e-9 sigma and their number
> equals the number of free rigid motions counted from the springs, the linear rows and the rigid sets; with a shift given by
> the caller, rigid_free is reported by the zero-shift factor test and agrees with the automatic case on every model of ADR
> 0384.

Clauses: (c1) on KF and on a free block with one spring (FB1), with the automatic shift, the zero modes satisfy `|lambda_i| <=
1e-9 sigma`; (c2) their number equals the number of free rigid motions counted from the springs, the linear rows and the rigid
sets (a counting rule, derived and registered here); (c3) with a shift given by the caller, `rigid_free` is reported by the
zero-shift factor test; (c4) it agrees with the automatic case on every model of ADR 0384.

## What was read (branch base f36519e9, read only)

1. `AGENT_RULES.md`; the brief; ADR 0384 (all: models, A1 and its diagnosis, remaining question 1 and the proposed
   acceptance).
2. Code: `crates/rcfd-fem/src/mpc.rs` (all: `transformation`, `Transformation`, `support_triplets`, `add_supports`,
   `modal_analysis_mpc`, `rigid_free`, `zero_shift_refused`, `ZERO_SHIFT_PIVOT_RATIO`), `modal.rs` (the shift rule of the old
   path, `rigid_free` from `check_rigid_body_motions`, `ModalOptions`), `crates/rcfd-linalg/src/eigen.rs` (`Shift`,
   `factor_shifted` and its Auto candidates `0, 1e-4 rho, ..., 10 rho`, `rho = sum |k_ii| / sum m_ii`, `AUTO_PIVOT_RATIO =
   1e-8`, `combine(k, m, 0) = k.clone()`, `SubspaceOptions::default` (ordering `Rcm`, tolerance `1e-10`, relative residual),
   `lowest_eigenpairs`), `skyline.rs` (`SkylineOptions::default`: `Rcm`, `PivotRule::Positive`, symmetry `1e-12`).
   Tests: `crates/rcfd-fem/tests/si_modal_mpc.rs` (the models KB, KF, RB, RC, SR, SA, CP, CL1, CL2, CN, KN, KE and helpers).
3. **Not read:** any finite-element code or manual outside this repository.

Observation from the reading (no run): `zero_shift_refused(k)` factors `k` with `SkylineOptions::default()` (RCM, positive
pivots, symmetry `1e-12`) and refuses a pivot ratio `d_i / a_ii < 1e-8` or a refused factor. The Auto rule's first candidate
factors `combine(K_r, M_r, 0) = K_r.clone()` with the ordering of `SubspaceOptions` (default RCM), `PivotRule::Positive`,
symmetry `1e-12`, and moves on at a ratio `< 1e-8` or a refused factor. With the default ordering the two decisions are the same
computation on the same matrix, so (c4) is expected to hold exactly on every model, and (c3) is already the code of ADR 0384;
what was missing is a check of it (ADR 0384 addendum, deviations: "the explicit-shift branch of `rigid_free` is not exercised
by a registered check"). No library change of `mpc.rs` is planned.

## Method and derivation

### The zero-mode measure against the shift (c1)

`lowest_eigenpairs` iterates on `A = K_r + sigma M_r` and accepts a Ritz pair `(mu, x)` when `||A x - mu M_r x|| / ||A x|| <=
tau = 1e-10`; the eigenvalue reported is `lambda = mu - sigma`. Let `x` be `M_r`-normalised and `r = A x - mu M_r x`. For the
symmetric definite pencil there is an eigenvalue `mu*` of `(A, M_r)` with `|mu - mu*| <= ||M_r^(-1/2) r|| <= ||r|| /
sqrt(m_min)` (the residual bound of the standard problem `M^(-1/2) A M^(-1/2)`, applied to `y = M^(1/2) x`, `||y|| = 1`), with
`m_min`, `m_max` the extreme eigenvalues of `M_r`. And `||A x|| <= |mu| ||M_r x|| + ||r|| <= mu sqrt(m_max) + ||r||`. So
`|mu - mu*| <= tau (mu sqrt(m_max) + ||r||) / sqrt(m_min) ~= tau mu sqrt(kappa(M_r))`. For a zero mode `mu* = sigma` (`lambda*
= 0`) and `mu ~= sigma`: **`|lambda| <= tau sqrt(kappa(M_r)) sigma`** to first order, plus the rounding of `mu - sigma`, a few
`eps sigma`. The consistent Hex8 mass of one element has the eigenvalues of the tensor product of the 1-D `[[2, 1], [1, 2]]/6`
(`1/2` and `1/6`), so its condition number is `3^3 = 27`; assembly and the congruence by `T` with entries of order one do not
change the order of magnitude for the models here. Bound: `1e-10 sqrt(27) sigma = 5.2e-10 sigma`, inside the acceptance's `1e-9
sigma`. (The Rayleigh-Ritz value is in fact accurate to the square of the residual over the gap; ADR 0384 measured `|lambda| <=
6.5e-7` on KF with `sigma = 3.058e6`, i.e. `2.1e-13 sigma`.)

The first flexible or spring mode must lie above the measure, so that the count of modes with `|lambda_i| <= 1e-9 sigma` is the
number of zero modes. KF: `lambda_3 = (2 pi 61.919)^2 = 1.51e5 = 4.9e-2 sigma` (ADR 0384), seven orders above. FB1 below:
the spring mode at `lambda_s = 1.28e4` (estimate below); `sigma <= 10 rho` and `rho <= lambda_max` of the model; with
`lambda_max` of a Hex8 block of `h = 0.2` m of order `1e10` at most, `1e-9 sigma <= 1e2`, two orders below `lambda_s`.

### The count of free rigid motions (c2): the registered rule

A free rigid motion is a rigid displacement of the body that every constraint row admits and that stores no energy. Let `x_c`
be the mean of the node coordinates and `l = max_a |x_a - x_c|`. The six scaled rigid fields, over the mesh unknowns, are
`X_j(a) = e_j` (`j = 0, 1, 2`, translations) and `X_(3+j)(a) = e_j x (x_a - x_c) / l` (rotations; every entry `<= 1` in
magnitude). They extend to the six reference unknowns of each rigid set with reference point `x_R`: translation `t = X_j(x_R)`
(the same formula at `x_R`) and rotation `w = 0` (`j < 3`) or `w = e_j / l` (`j >= 3`).

A combination `u = X c` (`c` in `R^6`) is admissible and of zero energy if and only if:

1. every constraint row holds on it: each prescribed equation (`u_i = 0`; the values are not used: the modal path requires
   them zero), each `Linear` row `sum a_k u_k = 0`, each rigid-set row `u_(a,comp) - t_comp - (w x (x_a - x_R))_comp = 0`; and
2. the supports store no energy: `u^T K_s u = 0`. The element stiffness gives zero energy on a small rigid motion (every
   element's strain vanishes on `a + w x x`), and `K_s` (the assembled `support_triplets`) is positive semidefinite (each nodal
   spring a positive diagonal entry, each foundation face `int k_f (N n)^T (N n) dA`), so `u^T K_s u = 0` if and only if `K_s u
   = 0`.

Every rigid-set row holds identically on the extended fields: with `u_a = t0 + w x (x_a - x_c)` and `t = t0 + w x (x_R - x_c)`,
`u_a - t = w x (x_a - x_R)` (derived). So a rigid set never reduces the count; its rows are nevertheless evaluated (they are
exactly zero up to rounding, removed by the cleaning below). A cyclic constraint's rows (ADR 0386) are not covered: the count
refuses a model with a `Cyclic` constraint.

Stack the conditions as a matrix `G` with six columns: one row per constraint row, `g = (c . X_0, ..., c . X_5) / max_k |c_k|`;
one row per mesh unknown `i` with a support entry, `g = (K_s X)_i / max_j |K_s(i, j)|`. Rounding residue: an entry `|g_j| <=
tau_c s_j` with `s_j = sum_k |c_k| |X_j(k)| / max_k |c_k|` (resp. `sum_j |K_s(i, j)| |X_j| / max |K_s(i, :)|`) is set to 0 (the
same idea as `DEPENDENCE_TOLERANCE` of ADR 0296). The free motions are the null space of `G`, and

**the number of free rigid motions is `6 - rank(G)`, `rank(G) = #{ s_i > tau_c s_max }` over the singular values `s_i` of `G`
(none if `s_max = 0`), `tau_c = 1e-8`.**

The singular values are computed by one-sided Jacobi (Hestenes 1958; the standard method, e.g. Golub and Van Loan, Matrix
Computations, section 8.6, cited from memory; the rotation is derived in the code comment: for columns `p`, `q` with `alpha =
|g_p|^2`, `beta = |g_q|^2`, `gamma = g_p . g_q`, the rotation by `zeta = (beta - alpha)/(2 gamma)`, `t = sign(zeta)/(|zeta| +
sqrt(1 + zeta^2))`, `c = 1/sqrt(1 + t^2)`, `s = c t` makes the two columns orthogonal), sweeps until no pair has `|gamma| >
1e-15 sqrt(alpha beta)` (at most 60 sweeps); the singular values are the column norms, the right singular vectors the
accumulated rotations; the free basis is the right singular vectors of the singular values at or below the threshold.
**Why `tau_c = 1e-8`**: the entries of `G` are of order one with relative rounding of a few `eps`; one-sided Jacobi gives the
singular values with an absolute error of order `eps sqrt(rows) s_max <= 1e-13 s_max` for the row counts here (below `10^5`). A
held direction has a singular value set by the geometry (lever arms over `l`, times the square root of the number of rows that
hold it), of order `1e-2` to `1` of `s_max` for every model here. `1e-8` lies five orders above the rounding and six below the
smallest geometric value. Registered limit of the rule: a direction held only by rows whose lever arms are below about `1e-8 l`
is counted as free.

The rule counts rigid motions. A zero-energy motion that is not rigid (a mechanism; a reference rotation of a collinear rigid
set, which `modal_analysis_mpc` refuses for its zero mass) is not counted. On the models here the mesh is one connected Hex8
block with full integration (no mechanism), so the nullity of `K_r` equals the count.

**Hand counts** (derived, the expected values of the checks):

- KF (bar `[0, 1] x [0, 0.1]^2`, springs along `x` at the four `xmin` nodes): rows `(1, 0, 0, 0, z_a'/l, -y_a'/l)` with `y_a',
  z_a' = +/-0.05`: rank 3 (`t_x`, `w_y`, `w_z`); **free 3** (`t_y`, `t_z`, `w_x`), as ADR 0384 states.
- KB, RB, SR, SA, CP, CL1, CL2: the rollers or the clamp (prescribed or Linear rows) hold all six: **free 0**. RC, CN, KN, KE
  are refused by `modal_analysis_mpc` before any eigenvalue (RC: no mass of the reference rotation; CN, KN, KE: non-zero values);
  the count is still evaluated on them where it is defined (RC: 0; CN, KN, KE: 0, the values are not used).
- FB1 (free block `hex_box([0,0,0], [1.0, 0.5, 0.4], [4, 2, 2])`, `nu = 0.3`, one `NodalSpring` on a one-node set `corner` at
  the origin, component 0, `k = 1e7` N/m): one row; **free 5**.
- FB2: FB1 plus a Linear tie `u_x(a) - u_x(b) = 0` (`a` at `(1, 0, 0)`, `b` at `(1, 0.5, 0)`: the row is `(0, 0, 0, 0, 0,
  -(y_a - y_b)/l)`, it holds `w_z`) and `RigidSet("xmin", (0.0, 0.13, 0.17))` (holds nothing): rank 2; **free 4**. The three
  sources of the acceptance (springs, linear rows, rigid sets) each act here.
- FR: the free block with `RigidSet("xmax", (1.0, 0.13, 0.17))` only (the reference point off the binary grid, so that the
  rigid-set rows leave rounding residues for the cleaning to remove): **free 6**.
- FS (negative control): the free block with two `x` springs of `k = 1e7` on the nodes at `(0, 0, 0)` and `(1, 0, 0)` (sets
  `corner` and `corner2`): both rows are `(1, 0, 0, 0, z'/l, -y'/l)` with the same `y'`, `z'`: rank 1, **free 5** although two
  springs act (the rule counts held directions, not supports).

Estimate of FB1's spring mode (arithmetic disclosed): mass `m = 7850 x 0.2 = 1570` kg, `I_y = m (1 + 0.16)/12 = 151.8`, `I_z =
m (1 + 0.25)/12 = 163.5` kg m^2 about the centre; the corner is at `(-0.5, -0.25, -0.2)` from the centre; the rigid-body
mobility of the corner along `x` is `1/m + 0.2^2/I_y + 0.25^2/I_z = 1.283e-3` 1/kg; `lambda_s ~= k x 1.283e-3 = 1.28e4`
rad^2/s^2 (the elastic deformation lowers it slightly).

### The explicit-shift report (c3, c4)

With `options.shift = Some(_)`, `modal_analysis_mpc` sets `rigid_free = zero_shift_refused(K_r)`, the factor test above; with
`None` it uses `eig.shift != 0` of the Auto rule. The checks run every model with the default, with `Some(Shift::Auto)`, with
`Some(Shift::Value(s_p))` and with `Some(Shift::Value(0.0))`; `s_p = 1e-3 rho`, `rho = sum |K_r ii| / sum M_r ii` of the default
run's reduced matrices (Auto's second positive candidate; positive, so `K_r + s_p M_r` is positive definite, and large enough
that the relative-residual floor `eps ||A|| / s_p` of a zero mode stays below the tolerance). The old path (SA, CP) has no
reduced matrices in its result; there `s_p = 1e-3 sum |K_ii| / sum M_ii` of the unreduced stiffness and consistent mass. On the
old path `rigid_free` comes from the geometric test whatever the shift (`modal.rs`); it is included because it is a model of ADR
0384.

The discriminating case is a model without a free motion run at a positive caller shift: the reported `shift` is not zero
there, so a report "the shift is not zero" would say `true`; the factor test says `false`.

## Design

### D1. New module `crates/rcfd-fem/src/free_motions.rs` (and its `pub mod` line in `lib.rs`)

```text
pub const COUNT_TOLERANCE: f64 = 1e-8;
pub struct FreeRigidMotions { pub count: usize, pub singular_values: [f64; 6] (descending, of G),
    pub free: Vec<[f64; 6]> (coefficients c of the free basis, orthonormal), pub centre: Vec3, pub length: f64,
    pub rows: usize }
impl FreeRigidMotions { pub fn fields(&self, model) -> Vec<Vec<f64>> (the free basis X c over the mesh unknowns) }
pub fn free_rigid_motions(model: &SolidModel, extra: &[Prescribed]) -> Result<FreeRigidMotions>
pub fn rigid_field(model: &SolidModel, centre: Vec3, length: f64, j: usize) -> Vec<f64>  (X_j over the mesh unknowns)
```

It reads the model (prescribed equations by `constraint::prescribed_equations`, Linear terms, rigid sets by their node sets and
reference points, supports by `mpc::support_triplets`); it does not call or change `mpc::transformation` or
`modal_analysis_mpc`. Refused: a `Cyclic` constraint; a mesh unknown of component above 2 (a model with rotational unknowns);
an empty mesh. `mpc.rs` and `modal.rs` are not edited (the parallel F-493 may touch `mpc.rs`; nothing here conflicts).

### D2. Files

New: `crates/rcfd-fem/src/free_motions.rs`, `crates/rcfd-fem/tests/si_zero_modes_mpc.rs` (the models of ADR 0384 rebuilt by
copies of its helpers; `si_modal_mpc.rs` is not edited). Edited: `crates/rcfd-fem/src/lib.rs` (one `pub mod` line). Nothing
else.

## Registered checks

All in `crates/rcfd-fem/tests/si_zero_modes_mpc.rs`, release, run once (`cargo test -p rcfd-fem --release --test
si_zero_modes_mpc -- --test-threads=1 --nocapture`). Material `E = 2e11` Pa, `rho = 7850` kg/m^3. `ModalOptions::new(8)` (eight
modes) for KF, FB1, FB2, FR, FS; the mode counts of ADR 0384 for its models. "Zero modes" are the computed modes with `|lambda_i|
<= 1e-9 sigma`.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| Z1 | c1, c2 | KF and FB1, default shift: `rigid_free` true and `sigma > 0`; `n0` = number of modes with `|lambda_i| <= 1e-9 sigma`; they are the lowest `n0` modes; `n0` equals `free_rigid_motions(...).count`; the count equals the hand count (KF 3, FB1 5); `max_(i<n0) |lambda_i| / sigma` recorded | `n0 = count = 3` (KF), `5` (FB1); `|lambda_i| <= 1e-9 sigma` | first-order bound `5.2e-10 sigma`; KF measured `2.1e-13 sigma` in ADR 0384; the next mode `>= 4.9e-2 sigma` (KF), `>= 1e2 / (1e-9 sigma)` margin (FB1) |
| Z2 | c2 | FB2 and FR as Z1 | `n0 = count = 4` (FB2), `6` (FR) | as Z1; the hand counts above |
| R1 | c2 | KF, FB1, FB2, FR: each zero mode (mesh part `x`) against its least-squares fit by the free fields `X c` of the count: `max |x - X c|_inf / |x|_inf` | `<= 1e-6` | the zero-mode vector carries the flexible modes with weights of the order of the residual times `sigma` over the gap; KF `1e-10 x 20 ~ 2e-9` |
| S1 | c2 | the singular values of `G`: smallest held value over `s_max` and largest free value over `s_max`, recorded for every model of Z1, Z2, C1 | held `> 1e-6`, free `<= 1e-12` (when there are both) | geometry `1e-2` to `1`; rounding `<= 1e-13` |
| C1 | c2, c4 | the count on every model of ADR 0384: KB, RB, SR, SA, CP, CL1, CL2 give 0, KF 3, RC 0, CN 0, KN 0, KE 0; and on every model that gives modes (ADR 0384's eight and FB1, FB2, FR, FS), default `rigid_free` equals `count > 0` | as stated | hand counts; `K_r` singular exactly when a rigid motion is free (no mechanism) |
| E1 | c3, c4 | every model of ADR 0384 that gives modes (KB, KF, RB, SR, SA, CP, CL1, CL2) and FB1, FB2, FR, FS: `rigid_free` with `Some(Shift::Auto)` and with `Some(Shift::Value(s_p))` equals the default's; with `Some(Shift::Value(0.0))`: equal and `shift = 0` where nothing is free, `Err(FemError::Solver { .. })` where a motion is free | as stated | the same factor test on the same matrix (reading above) |
| E2 | c3 | KB, RB, SR, CL1 at `Some(Shift::Value(s_p))`: the reported `shift` is `s_p` (not zero) and `rigid_free` is false | as stated | the factor test, not the shift, decides |
| E3 | c4 | RC, CN, KN, KE: the error of each of the three caller shifts equals (`Debug` text) the default's | equal | refused before the eigensolver |

**Negative controls** (each must hold):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | FS (two springs on one line) | count 5 and `n0 = 5` (the rule counts held directions, not supports) |
| N2 | KF with the springs taken out of the count (the count of the bare bar, no supports, no constraints, evaluated on a model without them) | 6, not 3 (the supports reduce the count) |
| N3 | FB1: the spring mode (index 5) | `lambda_5 > 1e-9 sigma` by more than `1e3` (the measure separates the spring mode from the zero modes) |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f488-scratch`, own target directory; each applied alone
by exact text replacement, `si_zero_modes_mpc` run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the supports left out of `G` | Z1 (KF count 6) |
| MU2 | the Linear rows left out of `G` | Z2 (FB2 count 5) and C1 (SR, CL1, CL2) |
| MU3 | the rotation fields set to zero (translations only) | Z1 (KF count 5) |
| MU4 | the cleaning of rounding residues removed | Z2 (FR; may survive if every residue of FR's rigid-set rows is exactly 0 in floating point; then recorded, not repaired) |
| MU5 | the rigid-set rows built with `+ w x d` | Z2 (FR count below 6) |
| MU6 | `mpc.rs`: the explicit-shift branch reports `eig.shift != 0.0` instead of the factor test | E2 |
| MU7 | `mpc.rs`: the explicit-shift branch reports `false` | E1 (KF) |

## Decision rule

**Done** if every check meets its limit, every negative control holds, MU1 to MU3 and MU5 to MU7 are caught by their
registered checks on the first run (MU4 caught or its survival explained as registered), and no existing test of `rcfd-fem`
fails; **partial** if a check misses or is not run, a negative control fails, or a mutation other than MU4 survives; **todo**
if the count cannot be computed or the zero modes cannot be found on KF or FB1.

## Limits (what is not covered)

- The count is of rigid motions: a mechanism (a zero-energy motion that is not rigid) is not counted, so on a model with one
  the count is below the nullity of `K_r` (the reference rotation of a collinear rigid set is refused by the modal path for
  its zero mass). Disconnected meshes are not covered (each part has its own rigid motions).
- A held direction whose singular value is below `1e-8` of the largest (lever arms below about `1e-8 l`) is counted as free.
- Cyclic constraints and models with rotational unknowns (beams, shells) are refused by the count.
- The count is not added to `ModalResult` or `MpcModes` (no existing type changes); it is a separate function.
- Caller orderings other than RCM are not checked (`ModalOptions` has no ordering; the two tests could differ only there).
- Only Hex8 models.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The count is a new function in a new module, rather than a field of `MpcModes` or `ModalResult` (no existing type or
   function changes, no conflict with the parallel F-493 in `mpc.rs`). Taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).
2. The counting rule is the rank of the constraint and support rows on the six rigid fields with `tau_c = 1e-8`, rather than
   the nullity of `K_r` itself (which counts mechanisms too and needs a tolerance on a large factor). Taken on the maintainer's
   standing instruction of 2026-11-11 (take the recommendation).
