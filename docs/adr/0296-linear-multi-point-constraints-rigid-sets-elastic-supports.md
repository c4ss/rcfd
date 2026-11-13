# ADR 0296: Linear multi-point constraints by a transformation, rigid sets with a reference point, elastic supports

Status: **REGISTERED 2026-11-13, written before any code of F-393 and before any run of its checks; planned by ADR 0290**.
Not yet accepted by the maintainer. Before this text nothing of F-393 was written, compiled or run: the code and documents
under "What was read" were read; no script and no library code was run. Results go in the addendum at the end; nothing above
the addendum is changed after a run.

Work package SI-6 of the structural solver, intermediate tier ([ADR 0290](0290-structural-solver-intermediate-tier-plan.md),
section 3, the entry for ADR 0296). Roadmap item F-393 (depends on F-388, F-374, F-375). Size M. Number 0296, fixed by the
plan; the file name of the planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as
written"). Branch `f393-mpc` from main 1c10db0a, worktree `w49i-work/f393`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (every
model without a linear constraint, a rigid set or an elastic support runs exactly the code it ran before); no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited; clean-room. A concurrent package (F-389, Hex20) works in `hex20.rs`: this package does not
touch it and keeps its edits of shared files to a few lines (D8).

## What F-393 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A model with prescribed displacements only gives the
> bits of the basic tier; the same constraints written as linear constraints give the same displacements within 1e-12. The
> reduced matrix is symmetric within 1e-13 and the constraint forces do no work on any admissible motion within 1e-12. Skew
> roller: a block in uniaxial stress along a direction 30 degrees off the axes, held by the constraint u.n = 0 on its inclined
> faces, has the rotated uniaxial solution within 1e-10. Periodic pair u(high) - u(low) = eps L on a box with displaced interior
> nodes: uniform strain within 1e-10. Rigid set on the end face of a bar: an axial force at the reference point gives F L/(E A)
> within 1e-10 (nu = 0), a moment the tip rotation M L/(E I) within 1 percent (Tet10, slenderness 20), and the constraint
> forces have the resultant and moment applied within 1e-10. Supports: a bar on an end spring k has the tip displacement F (1/k
> + L/(E A)) within 1e-10; a block on a foundation k_f under pressure p settles by p/k_f within 1e-10. A redundant constraint
> is dropped with a report, a contradictory one and a chain that closes on itself are refused naming the constraints.

Clauses: (a1) prescribed-only models keep the bits of the basic tier; (a2) the same constraints as linear constraints, 1e-12;
(a3) reduced matrix symmetric, 1e-13; (a4) constraint forces do no work on admissible motions, 1e-12; (a5) skew roller, 1e-10;
(a6) periodic pair, uniform strain, 1e-10; (a7) rigid set, axial force, F L/(E A), 1e-10; (a8) rigid set, moment, M L/(E I),
1 percent, Tet10, slenderness 20; (a9) rigid set constraint forces carry the applied resultant and moment, 1e-10; (a10) end
spring, 1e-10; (a11) foundation, p/k_f, 1e-10; (a12) a redundant constraint dropped with a report; (a13) a contradictory one
refused naming the constraints; (a14) a closed chain refused naming the constraints.

The brief adds: ADR 0291's `si_scaffold` tests assert that Linear, RigidSet and the supports are refused and are not edited; if
they fail because the variants now work, the exact assertions are listed as the remaining step that needs the maintainer. The
`si_scaffold_bits` digests stay equal. `baseline_hashes` is run in release. Cyclic stays refused (F-394).

## What was read (main 1c10db0a, read only)

1. `AGENT_RULES.md`; ADR 0290 sections 2.2 to 2.5 and its entry for 0296 (and the notes on 0294 and 0297 that build on this
   package); ADR 0291 (D5, D6, D8, the registered refusal strings, the checks R5 to R11 and X1); this ADR's stub.
2. Code: `crates/rcfd-fem/src/{mpc.rs, cyclic.rs, constraint.rs, solve.rs, modal.rs (head and modal_analysis_with), model.rs
   (DofMap, Constraint, LinearTerm, ElasticSupport, SolidModel::add_constraint, add_support, check_supported, SolidResult),
   loads.rs (nodal_load_vector, add_face_load, face_shape_functions, face_rule), mesh.rs (face_nodes, face tangents),
   builders.rs (hex_box, tet_box, upgrade_to_tet10, BOX_FACES), assemble.rs (stiffness_with), element.rs (strain_at),
   error.rs (variant list)}`; `crates/rcfd-linalg/src/{skyline.rs (factor, factor_with, envelope_size), matrix.rs (CsrMatrix)}`;
   `crates/rcfd-fem/tests/si_scaffold.rs` (X1, R5 to R11).
3. **Not read:** any finite-element code or manual outside this repository. The method is cited from memory (below) and
   derived here.

Findings that shape the design:

1. With `mpc::PENDING = None` the arms of `prescribed_equations` for Linear and RigidSet return nothing and
   `check_supported` passes the supports: no edit of `constraint.rs` or `model.rs` is needed to stop refusing, but every caller
   that only eliminates would then silently ignore them. Two callers exist in the library: `solve_static_report` (gets the
   dispatch, D6) and `modal_analysis_with` (keeps a refusal, D7).
2. `DofMap` has three unknowns per node (F-391 / ADR 0294 adds six). The reference point of a rigid set is a point, not a node
   (`RigidSet { set, reference: Vec3 }`), and no load names it. Its six unknowns (translation `t` and small rotation `w`) are
   therefore auxiliary unknowns of the transformation, numbered after the mesh's, six per rigid set in the order of the
   constraints; the force and moment at the reference point are an argument of the transformed solve (`ReferenceLoad`). This
   does not need six unknowns per node (open point 1).
3. `SkylineLdlt::factor` validates symmetry (1e-12) and uses the lower triangle; the reduced matrix must be square, finite and
   symmetric. `face_shape_functions` and `face_rule` are public and serve Tri3, Quad4 and Tri6 faces (no 8-node face: a
   foundation on Hex20 faces is refused by them).
4. The stiffness pattern couples every pair of nodes of an element, so a spring (diagonal) and a foundation (nodes of one face)
   add only inside it.

## Method and derivation

Cited from memory: master-slave elimination written as a congruence transformation (Cook, Malkus, Plesha and Witt, Concepts
and Applications of Finite Element Analysis, 4th ed., Wiley 2002, the chapter on constraints; Felippa, Introduction to Finite
Element Methods, lecture notes, University of Colorado, the chapters on multifreedom constraints). Nothing is taken from them
beyond what is derived here.

**Unknowns.** `x = (u, r)`: the `n` mesh unknowns of `DofMap` and six per rigid set, `r = (t, w)`. `K_ext` is `K` (with the
supports added, D4) padded with zero rows and columns for `r`; `f_ext` is the load vector with the reference loads `(F, M)` at
`r` (work conjugate: the virtual work of `(F, M)` on `(dt, dw)` is `F.dt + M.dw`).

**Constraints** are linear rows `c . x = b`: a prescribed displacement `u_i = g_i`; a linear constraint `sum a_k u_(i_k) = b`;
for a rigid set with reference point `x_R` and each node `a` of its set and component `c`, `u_(a,c) - t_c - (w x d_a)_c = 0`
with `d_a = x_a - x_R` (small rotation), i.e. for c = 0 the row `u_a0 - t_0 - w_1 d_2 + w_2 d_1`, for c = 1 `u_a1 - t_1 - w_2 d_0
+ w_0 d_2`, for c = 2 `u_a2 - t_2 - w_0 d_1 + w_1 d_0`.

**Transformation.** If a set of dependent unknowns `x_D` can be written as `x_D = E x_I + h` in terms of the independent ones
`x_I`, then every admissible `x` is `x = T q + g` with `q = x_I`, `T = [E; I]` (rows ordered back to `x`), `g = [h; 0]`. The total
potential `1/2 x^T K_ext x - f_ext^T x` restricted to admissible `x` is `1/2 q^T (T^T K_ext T) q - q^T T^T (f_ext - K_ext g) +
const`; its stationarity gives `K_r q = f_r` with `K_r = T^T K_ext T` (symmetric: `(T^T K T)^T = T^T K^T T`; positive definite
when `K_ext` is positive on the range of `T`) and `f_r = T^T (f_ext - K_ext g)`. The constraint force is `rho = K_ext x - f_ext`:
then `T^T rho = K_r q - f_r = 0`, i.e. `rho` does no work on any admissible variation `T dq` (clause a4: the measured work is
the residual of the reduced solve). With prescribed rows only, `T` is the selection of the free unknowns and `K_r = K_FF`, `f_r =
f_F - K_FP g`: the elimination of the basic tier is the special case (clause a2 to rounding).

For a rigid set, the columns of `T` of `t` and `w` give `T^T rho = 0` there as `sum_a rho_a - F = 0` and `sum_a d_a x rho_a - M
= 0` (since `(w x d).rho = w.(d x rho)`): the constraint forces at the set carry the applied resultant and moment (clause a9,
to the residual of the solve).

**Building E and h: elimination with designated dependents (Gauss-Jordan).** Rows are processed in a fixed order: the
prescribed rows (from `prescribed_equations`, ascending equation), then the model's Linear and RigidSet constraints in their
order (a rigid set node by node in set order, components 0, 1, 2). Each row has a designated dependent `d`: the unknown
itself for a prescribed row, the first term with a non-zero (merged) coefficient for a Linear constraint (the convention
"dependent = combination of the others"), the node component for a rigid-set row. For each row:

1. Substitute the current expressions of the dependents in the row: `sum_k c_k x_k = b'` over independents only. Track the
   coefficient scale `s = max |a|` of every term formed (original and substituted) and the value scale `s_b = |b| + sum |a h|`
   of the substituted constants. Coefficients with `|c_k| <= tau s` are rounding residues and are set to zero.
2. If `d` is independent and appears in the expression of a dependent of the row: **closed chain**: the row makes `d` depend on
   itself through earlier constraints. Refused, naming the row's constraint and the constraints of that chain.
3. If `d` is independent and does not: `d` becomes dependent, `x_d = (b' - sum_(k != d) c_k x_k)/c_d` (its coefficient is the
   original one, non-zero).
4. If `d` is already dependent and every `c_k` is zero: the row is implied by earlier ones. **Redundant** if `|b'| <= tau s_b`:
   dropped and reported (constraint, node, component, `|b'|`); otherwise **contradictory**: refused, naming the row's constraint
   and the constraints that define `d`.
5. If `d` is already dependent and some `c_k` remain (e.g. a node of two rigid sets, a tie to a prescribed node, two roller
   normals at an edge node): the pivot moves to the largest `|c_k|` (ties: the lowest index), which becomes dependent as in 3.
6. The new dependent's expression is substituted into every earlier expression that contains it (Gauss-Jordan), so every
   expression is always in independents only and chains resolve in any order.

`tau = DEPENDENCE_TOLERANCE = 1e-10` (registered). Estimate: the substituted coefficients carry the rounding of at most a few
hundred products of O(1) coefficients and lengths, `< 1e-13 s`; a genuinely independent row whose surviving coefficients are
below `1e-10 s` would make `K_r` ill-conditioned by `1e10` anyway. Contradictions in the tests are `1e-3` relative.

**Expected number of redundant rows of the periodic box (arithmetic, disclosed).** Box of `n^3` hexahedra, pairs on the three
face pairs, one corner prescribed. Per component the boundary nodes fall in periodic classes: face classes (2 nodes, 1 pair,
none redundant), edge classes (4 nodes, 4 pairs, 3 needed: 1 redundant; `3 (n - 1)` classes), the corner class (8 nodes, 12
pairs, 1 node prescribed, 7 needed: 5 redundant). Rows dropped = `3 (3 (n - 1) + 5)`; for `n = 3`: **33**. The count is
`rows - rank`, independent of the order, as long as no row is refused.

## Design

### D1. `mpc` module (filled)

`mpc::PENDING = None`. Public items: `DEPENDENCE_TOLERANCE = 1e-10`; `uses_transformation(model)` (true iff the model has a
Linear or RigidSet constraint or any support); `RowOrigin { Prescribed { node, component }, Linear { constraint }, RigidSet {
constraint, node, component } }` with a description; `DroppedRow { origin, residual }`; `Transformation` (data: `t()` as an
`N x n_q` `CsrMatrix`, `g()`, `independent()`, `n_mesh()`, `reference_offset(constraint)`, `dropped()`, `expand(q)`,
`reduce(k_ext, f_ext)`); `transformation(model, prescribed)`; `support_stiffness(model)` (D4) and `add_supports(k, model)`;
`ReferenceLoad { constraint, force, moment }`; `solve_static_mpc(model, prescribed, forces, reference_loads, options) ->
MpcSolution { result, report, transformation, reduced_matrix, references (translation, rotation per rigid set),
constraint_forces (rho over all N unknowns), support_forces (-K_s u), dropped }`; `refuse_in_modal(model)` (D7).

### D2. The reduced system

`A = K_ext T` row by row with a dense accumulator over `q` (rows of `r` are zero), then `K_r` row `alpha = sum_p T_(p alpha)
A_p` from the transpose of `T`; columns sorted; both triangles computed (the symmetry of clause a3 is measured, not imposed).
`f_r = T^T (f_ext - K_ext g)`. Every sum in a fixed order (rows, then the CSR order): the result does not depend on threads
(the stiffness is bit-identical for any thread count, ADR 0275).

### D3. The solve on the transformation

`StaticSolver::Direct`: `SkylineLdlt::factor` of `K_r` with the pivot test `d_i <= PIVOT_TOLERANCE a_ii` of ADR 0275 (1e-10);
a failing pivot is `SingularStiffness { node, component }` for a mesh unknown and `Solver { "skyline-ldlt", reason naming
the rigid set constraint and the reference component }` for a reference unknown. `StaticSolver::ConjugateGradient`: CG with
Jacobi as in `solve.rs`. `PreconditionedCg` is refused (`InvalidValue { key: "solver" }`, "not available on the transformation
path"); its node blocks and rigid modes do not map to `q` (open point 3). The rigid-motion check of ADR 0275 D5 is not used on
this path (springs and rigid sets hold motions it cannot see); a free motion is caught by the pivot test. `x = T q + g`; the
result is `SolidResult::new(dofs, u)` with reactions `rho` at every mesh unknown that appears in a constraint row or
expression (0 elsewhere) and the stats (solver name as in `solve.rs`, `n_q` unknowns).

### D4. Supports

`NodalSpring { set, component, k }`: `k` on the diagonal of each node's equation. `Foundation { set, k_f }`: for each face,
`K_ab += int k_f N_a N_b n n^T dA` with the face functions and rule of `loads.rs`; at a point `c = x_u x x_v`, `n n^T dA = w c
c^T/|c|`. Derivation of clause a11: for a uniform normal displacement `delta n` on a flat face, `sum_b K_ab delta n = k_f delta
int N_a n dA`, the consistent pressure load `p int N_a n dA` of `loads.rs` (same functions, same rule) for `delta = p/k_f`:
exact to rounding, whatever the rule's exactness. Supports are added to `K` before the transformation (they are stiffness, not
constraints) inside the stiffness pattern (finding 4).

### D5. Constraint forces and the report

`rho = K_ext x - f_ext` over all `N` unknowns (at a reference: `-(F, M)`). Dropped rows are listed in
`Transformation::dropped` and `MpcSolution::dropped`. Refusals use `FemError::InvalidValue { key: "constraint", reason }`
(no new error variant, `error.rs` untouched): contradictory: "`<row> contradicts <origins>: ... residual <b'> (scale <s_b>)`";
closed chain: "`<row> closes a chain on node <n> component <c> through <origins>`", where a row is described as "constraint
<i> (Linear)", "constraint <i> (RigidSet '<set>', node <a> component <c>)" or "the prescribed displacement of node <a>
component <c>".

### D6. Dispatch (shared file `solve.rs`, three lines)

In `solve_static_report`, after `model.check_supported()?`: if `mpc::uses_transformation(model)`, return
`mpc::solve_static_mpc(model, prescribed, forces, &[], options)` as `(result, report)`. A model without them runs the old code
unchanged (clause a1). ADR 0290 put the dispatch in `constraint.rs`; it is in `solve.rs` because `reduce` does not see the
model (finding 1); `constraint.rs` gets one doc sentence only.

### D7. Modal analysis keeps refusing (shared file `modal.rs`, one line)

`modal_analysis_with` calls `mpc::refuse_in_modal(model)` after `check_supported`: a Linear or RigidSet constraint or a
support is refused with `NotYetSupported { item "constraint" / "support", variant, location (the strings of ADR 0291 D6),
package "a follow-up of ADR 0296 (modal analysis on the transformation)" }`, never ignored (open point 2).

### D8. Files

`crates/rcfd-fem/src/mpc.rs` (the package), `solve.rs` (D6), `modal.rs` (D7), `constraint.rs` (doc sentence),
`crates/rcfd-fem/tests/si_mpc.rs` (new), the structural topic page under `docs/` if one exists (a section). Not touched:
`hex20.rs`, `model.rs`, `assemble.rs`, `error.rs`, any existing test file, any golden or hash table.

## Registered checks

All in `crates/rcfd-fem/tests/si_mpc.rs`, release profile, run once (`cargo test -p rcfd-fem --release --test si_mpc --
--test-threads=1 --nocapture`). Material E = 2e11, density 7850; nu as stated. Norms: `|v|_inf` over all components. Random
numbers: the 64-bit LCG `s <- 6364136223846793005 s + 1442695040888963407` (wrapping), value `(s >> 11)/2^53` mapped to
`[-1, 1)`, seed 296 for every use (registered).

Models:

- **P (patch)**: as ADR 0291's P: `hex_box([0.1, -0.2, 0.3], [1.2, 1.0, 0.8], [3, 3, 3])` and its Tet10 form (`tet_box` [2, 2,
  2] + upgrade), nu = 0.3, every boundary node prescribed per node (`solve_static_with`) with `u = 1e-3 (x + 2y - z, 0.5x - y +
  3z, -x + y + 0.7z)`.
- **S (skew roller)**: `hex_box([0, 0, 0], [1.0, 0.5, 0.4], [4, 2, 2])` rotated by 30 degrees about z (`x = R x'`), nu = 0.3,
  `e1 = (cos 30, sin 30, 0)`, `e2 = (-sin 30, cos 30, 0)`; Linear `u.e1 = 0` at every `xmin` node, `u.e2 = 0` at every `ymin`
  node (terms in the order x, y), `Displacement("zmin", 2, 0)`; `Load::traction("xmax", sigma e1)`, sigma = 1e6 Pa. Exact: `u =
  (sigma/E) (x'_1 e1 - nu x'_2 e2 - nu x_3 e3)`.
- **Q (periodic box)**: `hex_box([0,0,0], [1.0, 0.8, 0.6], [3, 3, 3])`, every node not on the boundary moved by `0.15 h_min`
  times a random number per coordinate (`h_min = 0.2`); nu = 0.3; node 0 (the corner at the origin) prescribed 0 in three
  components; for each face pair k and each node on the max face, its partner on the min face (same other coordinates, exact
  equality) and three Linear constraints `u_i(high) - u_i(low) = eps_ik L_k` (high first), pairs x then y then z, nodes in node
  order, components 0, 1, 2; `eps = 1e-3 [[1, 0.2, -0.1], [0.2, -0.5, 0.3], [-0.1, 0.3, 0.8]]`. Exact: `u = eps x`.
- **R (rigid end, Tet10, slenderness 20)**: `tet_box([0,0,0], [2.0, 0.1, 0.1], [20, 2, 2])` + upgrade, nu = 0, `xmin` clamped,
  `RigidSet("xmax", (2.0, 0.05, 0.05))`. R-F: reference force `(1e4, 0, 0)` N; exact `t_x = F L/(E A) = 1e-5` m. R-M: reference
  moment `(0, 1e3, 0)` N m; exact `w_y = M L/(E I) = 1.2e-3` rad (`I = 0.1^4/12`). Note (disclosed): with nu = 0 the exact
  solution of pure bending (`u = -k x z`, `w = k x^2/2`) is clamped at x = 0, plane (rigid) at x = L and quadratic, so it lies in
  the Tet10 space: the expected error is rounding, far inside 1 percent; the limit of the acceptance is kept.
- **K (end spring)**: `hex_box([0,0,0], [1.0, 0.1, 0.1], [10, 1, 1])`, nu = 0, `NodalSpring("xmin", 0, k/4)` (4 nodes), k =
  5e8 N/m, `Displacement("ymin", 1, 0)`, `Displacement("zmin", 2, 0)`, `Load::nodal_force("xmax", (F/4, 0, 0))`, F = 1e4 N.
  Exact `u_x(xmax) = F (1/k + L/(E A)) = 2.5e-5` m (one element across the section: the consistent shares of a uniform traction
  are equal, so equal springs give a uniform stress).
- **F (foundation)**: `hex_box([0,0,0], [1.0, 0.8, 0.5], [3, 3, 2])` and its Tet10 form (`tet_box` [2, 2, 2] + upgrade), nu =
  0.3, `Foundation("zmin", k_f = 1e10)`, `Load::pressure("zmax", p = 1e6)`, `Displacement("xmin", 0, 0)`, `Displacement("ymin",
  1, 0)`. Exact: `u_z = -p/k_f = -1e-4` m on `zmin`, `-p/k_f - p H/E` on `zmax`.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| A1 | a1 | `uses_transformation` false for P; `solve_static_with(P)` (Hex8, Tet10) equal bit for bit to the elimination done in the test (`prescribed_equations`, `stiffness`, `reduce`, `SkylineLdlt::factor().solve`, `expand`) | equal | D6: the old code runs |
| A2 | a1 | `si_scaffold_bits` (ADR 0291 B1 to B4, 26 digests) and `baseline_hashes` (release) | pass, files unedited | D6 |
| L1 | a2 | P with every prescribed value written as a single-term Linear constraint (coefficient 1; and coefficient 2.5 with value 2.5 g) against P: `|u_L - u_P|_inf / |u_P|_inf`, Hex8 and Tet10 | <= 1e-12 | same matrix `K_FF`; `f_r` rounds differently: ~1e-15 times the condition number of the patch (~1e2) |
| Y1 | a3 | `max |K_r(i,j) - K_r(j,i)| / max |K_r|` on S, Q, R, K, F (Hex8); pattern symmetric | <= 1e-13 | both triangles summed in different orders: a few ulp, ~1e-16 |
| W1 | a4 | five random `dq` (seed 296) on S, Q, R-F, R-M: `|rho . T dq| / (|rho|_2 |T dq|_2)` | <= 1e-12 | the residual of a backward-stable solve ~1e-15 |
| S1 | a5 | S: `|u - u_exact|_inf / |u_exact|_inf` | <= 1e-10 | linear field, Hex8 exact; rounding x condition ~1e-13 |
| Q1 | a6 | Q: strain at every integration point of every element (`strain_at` at the stiffness rule's points) against `eps` (engineering shear `2 eps_ij`): `max |e_h - e| / max |eps|`; and `|u - eps x|_inf / (|eps|_max L_max)` | <= 1e-10 both | patch test on a distorted Hex8 mesh: exact up to rounding ~1e-13 |
| Q2 | a12 | Q: the number of dropped rows; none refused; every dropped row's origin is a Linear constraint | exactly 33 | arithmetic above |
| R1 | a7 | R-F: `|t_x - F L/(E A)| / (F L/(E A))`, and every `xmax` node's `u_x` the same | <= 1e-10 | exact solution in the space |
| R2 | a8 | R-M: `|w_y - M L/(E I)| / (M L/(E I))` | <= 1e-2 | exact solution in the space; expected rounding |
| R3 | a9 | R-F and R-M: `|sum_set rho_a - F| / max(|F|, |M|/rho_max)` and `|sum_set d_a x rho_a - M| / max(|M|, |F| rho_max)` (`rho_max = max |d_a|`) | <= 1e-10 both | `T^T rho = 0` to the solve's residual |
| K1 | a10 | K: `|u_x - F (1/k + L/(E A))| / (F (1/k + L/(E A)))` at every `xmax` node | <= 1e-10 | uniform stress, exact |
| F1 | a11 | F (Hex8, Tet10): `|u_z + p/k_f| / (p/k_f)` at every `zmin` node; and `|u_z + p/k_f + p H/E| / (p/k_f + p H/E)` at every `zmax` node | <= 1e-10 | D4 derivation; uniaxial stress exact |
| C1 | a12 | a Hex8 cantilever (`hex_box` [1.0, 0.1, 0.1], [10, 2, 2], `xmin` clamped, `nodal_force("xmax", (0, 0, -100))`) with a Linear tie `u_z(a) - u_z(b) = 0` of two `xmax` nodes written twice: `dropped` lists exactly one row, the second constraint; the displacement against the model with the tie once | one row named; `<= 1e-12` relative | the dropped row adds nothing |
| C2 | a13 | the same cantilever with the tie once plus `u_z(a) - u_z(b) = 1e-3`; and `xmin` clamped plus a Linear `u_x(n) = 1e-3` on a node `n` of `xmin`: `Err(InvalidValue { key: "constraint" })` whose reason contains "contradicts", the second constraint's index and the first's description | as stated | D5 |
| C3 | a14 | the cantilever with `u_z(a) - u_z(b) = 0`, `u_z(b) - u_z(a) = 0` (two-chain) and with `u_z(a) - u_z(b)`, `u_z(b) - u_z(c)`, `u_z(c) - u_z(a)` (three-chain), each `= 0`: `Err(InvalidValue)` whose reason contains "closes a chain" and the indices of every constraint of the chain | as stated | D5 |
| T1 | D2 | R-M with `threads(1)` and `threads(8)`: displacement, references and constraint forces bit for bit | equal | D2 |
| T2 | D3 | S with `ConjugateGradient { 1e-12, 10000 }` against the direct solve: `|u_cg - u_d|_inf / |u_d|_inf` | <= 1e-7 | condition of the small block ~1e3 times 1e-12 |
| E1 | report | R (clamp + rigid set) against the same mesh clamped only: nonzeros of `K_r` against `K_FF`, `envelope_size` of each (RCM) | recorded, no limit | stub: the fill is reported, not hidden |

**Negative controls** (each must hold):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | R with the rigid set on the nodes of one `xmax` edge (collinear), reference force (1e4, 0, 0): the rotation about the edge is free | `Err`: singular (pivot), naming the reference rotation of the rigid set constraint |
| N2 | F without the two roller constraints | `Err`: singular |
| N3 | S with `PreconditionedCg { BlockJacobi, .. }` | `Err(InvalidValue { key: "solver" })` |
| N4 | `modal_analysis` of the cantilever with a Linear tie and of the cantilever with a spring | `Err(NotYetSupported)` naming "Linear" / "NodalSpring" |
| N5 | the cantilever with a Linear tie and a Cyclic constraint (surface sets of `ymin`, `ymax`) | `Err(NotYetSupported { variant: "Cyclic", package: "ADR 0297" })` |
| N6 | S with the roller normals at 31 degrees instead of 30 (the exact solution still at 30) | S1's measure > 1e-6 |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f393-scratch`, own target directory; each applied alone
by exact text replacement, `si_mpc` run once, the file restored):

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | step 6 skipped (earlier expressions not updated) | Q1 or S1 |
| MU2 | the rotation terms of the rigid-set row with the opposite sign | R3 |
| MU3 | `f_r = T^T f_ext` (the `K_ext g` term dropped) | L1 |
| MU4 | the closed-chain test (step 2) removed | C3 |
| MU5 | a contradictory row dropped as redundant | C2 |
| MU6 | foundation `w c c^T` without the division by `|c|` | F1 |
| MU7 | spring stiffness doubled | K1 |
| MU8 | `uses_transformation` always true | A1 |
| MU9 | `DEPENDENCE_TOLERANCE = 0` (only exact cancellation detected) | Q2 |
| MU10 | the reference moment applied to the next rotation component | R2 |

## Decision rule

**Done** if every check meets its limit, every negative control holds and every mutation is caught by its registered check on
the first run, the only failing existing tests being the `si_scaffold` assertions that the new variants are refused (listed as
the step that needs the maintainer); **partial** if a check misses or is not run, or a mutation survives, or another existing
test fails; **todo** if the transformation does not build or a basic-tier bit changes.

## Limits (what is not covered)

- Small rotations only at the reference point; no large-rotation rigid link (ADR 0300's nonlinear package).
- The reference point is not a node: no output of it in the VTK file; its motion is in `MpcSolution::references`.
- Loads at the reference point only through `solve_static_mpc`; a case file cannot name them yet (ADR 0306).
- Modal analysis with these constraints or supports is refused (D7); preconditioned CG on the transformation is refused (D3).
- Tied non-matching meshes are not included (advanced, with bonded contact). Cyclic stays refused (ADR 0297).
- A foundation on 8-node faces (Hex20) is refused by the face functions of `loads.rs`.
- The pivot choice of step 5 mixes the units of translations and rotations (largest `|c|`); a reference point far (> 1 m) from
  its set prefers rotation pivots. Correct but not scaled.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The six unknowns of a reference point are auxiliary unknowns of the transformation, not a node of `DofMap` (finding 2): this
   needs no six-unknown node (F-391); when ADR 0294 lands, a rigid set may name a node with rotations instead. Taken on the
   maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. Modal analysis on the transformation is not part of the acceptance; it is refused rather than half done, and proposed as a
   follow-up item. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
3. Preconditioned CG on the transformation is refused (node blocks and rigid modes of `q` are not defined for reference
   unknowns); Direct and Jacobi CG are offered. Taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation).
4. "A chain that closes on itself" is defined with designated dependents (the first term of a Linear constraint): a row whose
   dependent appears in the expression of another dependent of the row; a row whose dependent is already defined is redundant
   or contradictory. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
5. The refusals use the existing `InvalidValue { key: "constraint" }` with the constraints named in the reason, so that
   `error.rs` (shared with concurrent packages) is not touched. Taken on the maintainer's standing instruction of 2026-11-11
   (take the recommendation).

## Expected to fail, needs the maintainer

ADR 0291's `crates/rcfd-fem/tests/si_scaffold.rs` asserts that the variants of this package are refused. With the package
landed these assertions fail and are not edited here: `x1_the_scaffold_exists` (`assert_eq!(rcfd_fem::mpc::PENDING, Some("ADR
0296"))`), `r1_to_r9_static_solve_refuses_each_new_variant` and `r10_refused_on_every_static_path` (cases R5, R6, R8, R9 now
solve or fail otherwise), `r11_modal_and_stiffness_refuse` (case R8 modal: refused with the package string of D7, not "ADR
0296"). The exact failing assertions are recorded in the addendum.

## Amendment 1 (2026-11-13, before any code and any run): a check for the threshold

Arithmetic done after the registration and before any code (disclosed): in Q every substituted coefficient is a product of
the unit coefficients 1 and -1, so the redundant rows of Q cancel exactly and Q2 cannot see the threshold `tau` (mutation MU9
would survive). Added check, and MU9's target extended:

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| C4 | a12 | the cantilever of C1 with the tie `cos30 u_z(a) + sin30 u_z(b) = 0` and, as a second constraint, the same row times a factor `s`, for each `s` in (3.7, 0.3, 1.9, 7.1, 1/3) (five models): exactly one row dropped (the second constraint), the substituted residue printed, the displacement against the model with the tie once | one row named; `<= 1e-12` relative | residues of order 1e-16 relative, far below `tau` |

MU9 must fail Q2 or C4 (it is caught if at least one of the five residues is not exactly zero; if all five cancel exactly the
survival is recorded, not repaired).

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-6 of the structural solver, intermediate tier. Size M.

### Goal

Replace the reduction of the system by a transformation `u = T q + g` whenever a constraint other than a prescribed displacement
is present (the elimination of the basic tier stays for models that have none, bit for bit). On it: the general linear constraint
`sum a_i u_i = b`, a rigid set of nodes that follows a reference point with six unknowns (small rotation) and takes a force and a
moment there, and elastic supports (a spring to ground per node and direction, and a foundation stiffness per area on a surface
set). This is what cyclic symmetry (ADR 0297) is built on.

### Dependencies

ADR 0291; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-1, F-374, F-375.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A model with prescribed displacements only gives the bits of
the basic tier; the same constraints written as linear constraints give the same displacements within 1e-12. The reduced matrix is
symmetric within 1e-13 and the constraint forces do no work on any admissible motion within 1e-12. Skew roller: a block in
uniaxial stress along a direction 30 degrees off the axes, held by the constraint u.n = 0 on its inclined faces, has the rotated
uniaxial solution within 1e-10. Periodic pair u(high) - u(low) = eps L on a box with displaced interior nodes: uniform strain
within 1e-10. Rigid set on the end face of a bar: an axial force at the reference point gives F L/(E A) within 1e-10 (nu = 0), a
moment the tip rotation M L/(E I) within 1 percent (Tet10, slenderness 20), and the constraint forces have the resultant and
moment applied within 1e-10. Supports: a bar on an end spring k has the tip displacement F (1/k + L/(E A)) within 1e-10; a block
on a foundation k_f under pressure p settles by p/k_f within 1e-10. A redundant constraint is dropped with a report, a
contradictory one and a chain that closes on itself are refused naming the constraints.

### Reference to check against

Closed forms: uniaxial Hooke's law in rotated axes, springs in series, beam bending by an end moment. Method: master-slave
elimination written as a congruence transformation (Cook, Malkus, Plesha and Witt, Concepts and Applications of Finite Element
Analysis, the chapter on constraints; from memory); derivable from the principle of virtual work.

### Files

`crates/rcfd-fem/src/mpc.rs`, `crates/rcfd-fem/src/constraint.rs` (the dispatch to the transformation),
`crates/rcfd-fem/tests/si_mpc.rs`.

### What could make it miss

`T^T K T` fills the matrix where a rigid set couples many nodes (the direct solver's envelope grows; reported, not hidden).
Choosing the eliminated unknown of each constraint so that chains resolve; detecting redundancy needs a rank decision with a
registered threshold. Tied non-matching meshes are NOT included (advanced, with bonded contact).
