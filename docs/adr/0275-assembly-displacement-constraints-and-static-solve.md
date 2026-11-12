# ADR 0275: Assembly, displacement constraints, nodal forces and the linear static solve; patch test

Status: **REGISTERED 2026-11-12, written before any code of F-374 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-374 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script
and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Work package SB-5 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0275). Roadmap item F-374 (depends on F-370, F-372, F-373, all done). Size M. Number 0275, fixed by the plan; the
file name of the planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written").
Branch `f374-assembly-static-solve` from main 99ad1169, worktree `w49i-work/f374`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: only the four one-line stubs `assemble.rs`, `constraint.rs`, `solve.rs`, `loads.rs` of `rcfd-fem`
  are filled, `error.rs` of `rcfd-fem` gains variants (appended, D7), and one test file is added; no file outside `rcfd-fem`,
  no shared solver file, no golden, no hash table is touched;
- no new dependency (the crate keeps `rcfd-core`, `rcfd-mesh`, `rcfd-linalg`; threads come from `std::thread::scope`, D2), no
  unsafe code, the graph stays acyclic;
- clean-room: everything below is derived here from first principles (the principle of virtual work, Hooke's law, the
  rigid-body motions of a body, the Schur complement of a symmetric positive definite matrix); papers and books are cited from
  memory for orientation only.

## What F-374 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. The stiffness of a SolidModel is assembled into a
> CsrMatrix deterministically (bit-identical for 1 and 8 threads) and is symmetric within 1e-13 of its norm; displacement
> constraints per node and component, zero or not, are imposed by symmetric elimination and the reactions recovered. Patch test
> (Tet4, Hex8, Tet10; a box whose interior nodes are displaced by a registered seed; boundary displacements of a linear field
> with all six strains non-zero): interior displacements within 1e-10 of the field, relative to its maximum. Uniaxial tension
> by nodal forces with statically determinate constraints: elongation F L/(E A) and lateral contraction nu times the axial
> strain within 1e-10. The reactions balance the applied forces and their moment within 1e-10; a model that leaves a rigid-body
> motion free is refused with a message saying so, a constraint on a missing node with its id.

Clauses: (c1) the stiffness is assembled into a `CsrMatrix`, bit-identical for 1 and 8 threads; (c2) symmetric within 1e-13
of its norm; (c3) displacement constraints per node and component, zero or not, imposed by symmetric elimination; (c4) the
reactions are recovered; (c5) the patch test for Tet4, Hex8 and Tet10 within 1e-10; (c6) uniaxial tension by nodal forces with
statically determinate constraints: elongation within 1e-10; (c7) lateral contraction within 1e-10; (c8) the reactions balance
the applied forces within 1e-10; (c9) and their moment within 1e-10; (c10) a free rigid-body motion is refused with a message
saying so; (c11) a constraint on a missing node is refused with its id.

The brief adds: use `rcfd_linalg::CsrMatrix` and `rcfd_linalg::skyline` (F-373, ADR 0274); register how a free rigid-body
motion is detected, with its derivation; answer ADR 0273's open question 3 (the assembly adds the element index to element
errors); leave room for the loads of ADR 0276, the stress recovery of 0277, the thermal load of 0281, the iterative solver of
0282, the mass matrix of 0284, the generic assembly hook of 0291 and the transformation method of 0296.

## What was read (main 99ad1169, read only)

1. `AGENT_RULES.md`; ADR 0270 sections 1, 2 (2.2 conventions: Voigt order, `DofMap`, determinism by element index, every
   refusal located; 2.3 module table: `assemble`, `constraint`, `solve`, `loads` (nodal force only) are 0275's; 2.4 data model:
   `constraint::reduce` is the one function a transformation method replaces; 2.5 interfaces: `assemble::stiffness(&SolidModel)
   -> CsrMatrix`), 3 (the entries 0274 to 0284), 4, 5, 6 and the decision record; this ADR's stub; ADR 0273 in full with its
   addendum (open question 3; the trait `Element`, `element_for`, `ElementMatrix`, stiffness computed in both triangles);
   ADR 0274's stub entry in 0270; the stubs 0276, 0277, 0281, 0282, 0284, 0291, 0296 (goal, acceptance, files).
2. Code: `crates/rcfd-fem/src/{lib.rs, model.rs, mesh.rs (public API, add_block, sets), builders.rs, element.rs,
   material.rs, error.rs}` and the four stubs; `crates/rcfd-fem/tests/sb_elements.rs` (head: the SplitMix64 generator);
   `crates/rcfd-linalg/src/{skyline.rs (in full), matrix.rs (CsrMatrix), solvers.rs (cg, SolveControl), precond.rs (Jacobi),
   eigen.rs (symmetric_eigen), lib.rs}`; the use of rayon in `rcfd-linalg` (`par_chunks` over fixed chunks; tests build a
   `ThreadPool` of 1 and 8 threads) and that `rcfd-fem`'s manifest has no rayon.
3. **Not read:** the bodies of the books and papers cited; any finite-element code outside this repository.

Findings that shape the design:

1. `Constraint::Displacement` prescribes one value on every node of a node set, and node sets are validated when added (a
   member out of range is refused by `SolidMesh::add_node_set`). A constraint on a missing node can therefore only arise from
   a per-node list; the patch test needs one anyway (every boundary node has its own value). So the solve takes, beside the
   model, a list of per-node prescribed values and a list of per-node forces (D3, D4); `model.rs` is not edited.
2. `rcfd-fem` cannot use rayon without a new dependency. Threads come from `std::thread::scope` (D2).
3. `SkylineLdlt` refuses a pivot `d_i <= theta_i = (w_i + 1) eps (|a_ii| + sum |u l|)`, which is a rounding bound: a singular
   matrix can pass it with a garbage pivot of the same size (the stub's warning). The primary detection of a free rigid-body
   motion is therefore a check of the constraint set (D5), with a pivot test only as a backstop for mechanisms (D6).
4. `FemError` has no variant for an element-located error, a missing node of a per-node list, a free rigid-body motion or a
   singular stiffness. Variants are appended to `error.rs` (D7); the enum is `#[non_exhaustive]`, so no caller breaks.

## Arithmetic before this registration (disclosed, by hand)

1. **Rigid-body motions.** A small rigid motion is `r(x) = a + w x (x - c)` with six parameters `(a, w)`. Writing `y = (x -
   c)/L` (c the centroid of a part's nodes, L the largest distance of its nodes from c) and `q = (a, L w)`, the value of
   component k at node p is `phi_k(y_p) . q` with `phi_0 = (1, 0, 0, 0, y_z, -y_y)`, `phi_1 = (0, 1, 0, -y_z, 0, y_x)`, `phi_2
   = (0, 0, 1, y_y, -y_x, 0)` (from `(w x y)_x = w_y y_z - w_z y_y` and its cyclic forms). The prescribed set leaves a rigid
   motion free iff some `q != 0` has `phi_k(y_p) . q = 0` for every prescribed (p, k), i.e. iff the 6 x 6 Gram matrix `G =
   sum phi phi^T` over the prescribed components of the part is singular. The value prescribed does not matter (the
   homogeneous problem decides). Checks by hand: `u_x = 0` on the plane `x = 0` leaves `a_y, a_z, w_x` free (3 motions);
   the 3-2-1 set P1 (x, y, z) at the origin, P2 (x, y) at `(0, 0, c)`, P3 (x) at `(0, b, 0)`: P1 gives `a = 0`, P2 `w_y c = 0`
   and `-w_x c = 0`, P3 `-w_z b = 0`: none free; without P3, `w_z` is free (1 motion); three collinear points along x fully
   held leave `w_x` free (1 motion).
2. **Threshold of the Gram test.** For an exactly free motion the computed `G` has `q0^T G q0 ~ m eps^2` (the rounding of `y`)
   and Jacobi rotations give eigenvalues to `~ eps ||G||` absolutely, so `lambda_min / lambda_max <~ 1e-15`. For a properly
   held part the entries of `phi` are `O(1)` (scaled coordinates); `lambda_min / lambda_max` is `O(1e-2)` to `O(1)` for
   supports spread over the part and `~ (delta/L)^2` for supports whose spread perpendicular to a line is `delta`. Threshold
   `tau_G = 1e-10`: a motion is free iff `lambda <= tau_G lambda_max` (or `lambda_max = 0`, no constraint on the part). That
   is 5 decades above the rounding of an exact deficiency and refuses supports with `delta/L < 1e-5` as numerically free
   (listed under Limits).
3. **Parts.** Two sets of elements that share no node move independently; the rigid motions are those of each connected
   part (nodes joined through elements, union-find). A node of no element has a zero row in `K`: each of its components is a
   free motion unless prescribed.
4. **Pivot backstop.** For a symmetric positive definite `A` eliminated in some order, the pivot of the unknown at position
   `i` is `d_i = 1/(B^{-1})_ii` with `B` the leading block up to `i`; eliminating fewer other unknowns can only stiffen, so `d_i
   >= 1/(A^{-1})_ii` and `d_i / a_ii >= 1/(a_ii (A^{-1})_ii)`, the inverse of the flexibility ratio of that unknown. Estimate
   for a cantilever of square section h, length L, one element through the thickness: `a_ii ~ E h`, tip flexibility `4
   L^3/(E h^4)`, ratio `~ 4 (L/h)^3`: `d_i / a_ii >= 1e-10` holds for `L/h <~ 1360`. For an exactly singular `A` (a
   mechanism), the computed pivot is the exact one of `A + E` with `|E| <~ c w eps |A|` (backward stability of `L D L^T` on
   SPD matrices), so `|d_i| / a_ii ~ w eps |x|^2` with `x` the null vector scaled to 1 at the pivot: `~ 1e-13` to `1e-12` for
   envelope widths `w ~ 1e2` to `1e3`. Threshold `tau_P = 1e-10` on `d_i / a_ii` (a_ii the reduced matrix's diagonal of the
   same unknown), applied after the skyline's own test, refuses mechanisms with two decades of margin and refuses legitimate
   models only beyond a flexibility ratio of 1e10 (listed under Limits).
5. **Patch test.** For a linear field `u = c + A x` the isoparametric interpolation is exact, `B u_e` is the constant strain
   `eps = sym(A)` at every point, and the row of an interior node a is `sum_e sum_q w_q det J_q B_a^T D eps`. This vanishes iff
   the rule integrates `det J grad N_a` exactly (then it is `int grad N_a dV = 0` over the patch, N_a vanishing on its
   boundary). Tet4: constant, exact. Tet10 with straight edges: J constant, `grad N_a` linear, `tet_4` (degree 2) exact; with
   curved edges `det J grad N_a = adj(J)^T dN/dxi` is of degree 3, not exact; so the Tet10 patch has straight edges (mid-edge
   nodes at the midpoints of the perturbed corners). Hex8: every J entry has degree <= 1 in each reference variable
   (`dx/dxi` is constant in `xi` and bilinear in `(eta, zeta)`), the entries of `adj(J)` are products of two of them (degree
   <= 2 per variable), and times `dN/dxi` (degree <= 1 per variable) the integrand has degree <= 3 per variable: 2 x 2 x 2 Gauss (degree 3 per variable) is exact on any
   trilinear hexahedron. The patch test therefore holds to rounding for all three on the registered meshes.
6. **Perturbation.** Interior grid nodes move by `r h_k (2U - 1)` per coordinate k, `r = 0.1`, `h_k` the spacing. Hex8: each
   edge vector changes by at most `0.2 h_k` per coordinate, so the matrix of the three edge vectors at a corner, scaled by
   `h`, is `I + Delta` with `|Delta_ij| <= 0.2`, strictly diagonally dominant (0.8 > 0.4) and connected to `I`: `det J > 0`.
   Kuhn tetrahedron of the unit cell: heights 1, 0.707, 0.707, 1 (computed: e.g. face `(v0, v2, v3)` has normal `(1, -1, 0)/
   sqrt 2`, `v1` at distance `1/sqrt 2`); a node moves by at most `0.1 sqrt 3 = 0.17` of the spacing: no inversion. The tests
   assert that `SolidMesh::add_block` accepts every perturbed mesh (it checks `det J > 0` at every node).
7. **Rounding of the solves.** Patch meshes of 4 x 3 x 3 cells with all boundary nodes held: condition of the interior block
   `~ 1e2` to `1e4` (Tet10 the worst); forward error `~ cond eps <~ 2e-12` relative: margin 50 to 1e-10. Uniaxial bar 6 x 2 x
   2 cells with point supports: condition `~ 1e4` to `1e6` (point supports give soft modes of size `E h`), but the loads
   self-equilibrate and the exact solution is linear, so the error is the rounding of a smooth solution, estimated `~ 1e-13`;
   the worst-case bound `cond eps ~ 2e-10` exceeds the limit (the limit is the acceptance's): a miss there would be rounding
   and recorded as such.
8. **Balance.** `sum_i (K u)_i = (K 1_k)^T u` is zero to the rounding of the element rows (each `K_e` annihilates translations
   within `~ 1e-16 ||K_e||`, ADR 0273 A6) and the residual of the free rows is `~ eps ||K|| |u|`: the imbalance relative to
   `sum |f|` is `~ 1e-14`. Moment likewise with the rotations. Margin `~ 1e4` to 1e-10.
9. **Symmetry.** Each `K_e` is computed in both triangles (ADR 0273), asymmetric by `~ 1e-16 ||K_e||` (measured 1.9e-16 there);
   the sums over elements keep it `<~ 1e-15` of `max |K_ij|`: margin 100 to 1e-13.
10. **Uniaxial numbers.** Bar `[0, 2] x [0, 0.5] x [0, 0.4]` m, `A = 0.2 m^2`, `E = 2e11 Pa`, `nu = 0.3`, `F = 1e6 N`: `sigma =
    5e6 Pa`, `eps = 2.5e-5`, elongation `F L/(E A) = 5e-5 m`, lateral strain `-7.5e-6`. Exact field (pinned by the 3-2-1 set of
    arithmetic 1): `u = (eps x, -nu eps y, -nu eps z)`; at P1 all zero, at P2 `u_x = 0`, `u_y = 0`, at P3 `u_x = 0`: consistent.
    Consistent nodal loads of a uniform traction t on a flat face: Quad4 on a parallelogram `t A_f/4` per node; Tri3 `t A_f/3`;
    Tri6 0 at the corners and `t A_f/3` at the mid-edge nodes (`int (2 L^2 - L) dA = A/3 - A/3 = 0`, `int 4 L_i L_j dA = 4 x 2 A
    /4! = A/3`). With these loads on both ends (+ on `x = L`, - on `x = 0`) the finite-element solution is the exact field for
    all three kinds (the patch-test argument of arithmetic 5 with the traction terms integrated exactly).

## Design

### D1. Assembly (`assemble.rs`)

* `element_dofs(dofs: &DofMap, nodes: &[usize]) -> Vec<usize>`: the equations of an element, node-major, `dofs.components(n)`
  per node (three for the basic tier; nothing else in the code assumes three).
* `pattern(mesh, dofs) -> CsrMatrix` with zero values: the nodes adjacent to each node through elements (sorted, unique);
  row `(a, i)` has the columns `(b, j)` for every adjacent `b` in ascending order and every component `j` of `b`, which is
  ascending because `DofMap` numbers node by node. A node of no element has an empty row.
* `assemble_with(model, threads, f) -> Result<CsrMatrix>` where `f(&ElementContext) -> Result<ElementMatrix>` (context:
  element index, kind, node coordinates, material) is `Sync`: the generic hook (ADR 0291 may re-export it; ADR 0284 uses it
  for the mass matrix). An element matrix of the wrong size is refused (`FieldLength { key: "element matrix" }`), and every
  error of `f` is returned as `FemError::Element { element, source }` (ADR 0273 open question 3, option (a)).
* `stiffness(model) -> Result<CsrMatrix>` and `stiffness_with(model, threads)`: `assemble_with` with
  `element_for(kind).stiffness(coords, &material.elasticity_matrix()?)`. Materials from `SolidModel::material_indices` (an
  element without material is refused there). No symmetrisation: the matrix is what the elements give (clause c2 is
  measured, not imposed); the skyline solver reads its lower triangle.

### D2. Determinism and threads

Elements are processed in batches of 1024 in element order. Within a batch the element matrices are computed by `threads`
workers on contiguous ranges (`std::thread::scope`; `threads = None` means `std::thread::available_parallelism()`, and the
count is capped by the batch length); each matrix is a pure function of its element. The scatter into the CSR values is
sequential, in element order, row by row of the element matrix and column by column, so every entry is the sum of the same
terms in the same order for any thread count: the bits do not depend on it. The first error in element order is the one
returned. Everything after assembly (reduction, `SkylineLdlt`, the residual by `CsrMatrix::matvec` over fixed chunks) is
already independent of the thread count (ADR 0274; `rcfd-linalg` lib docs).

### D3. Constraints and their reduction (`constraint.rs`)

* `Prescribed { node, component, value }`: a per-node prescribed displacement.
* `prescribed_equations(model, extra) -> Result<Vec<(usize, f64)>>`: the model's `Constraint::Displacement` (every node of
  its set) and then `extra`, as (equation, value) sorted by equation. A node that does not exist is refused naming it
  (`MissingNode { role: "constraint", node, n_nodes }`), a component the node does not have likewise (`MissingComponent`), a
  non-finite value (`InvalidValue`). The same equation prescribed twice with bit-equal values is accepted once (edges shared
  by two face sets); with different values it is refused naming node, component and both values (`ConflictingConstraint`).
* `reduce(k, f, prescribed) -> Result<ReducedSystem>`: symmetric elimination by removal. With F the free and P the prescribed
  equations, `K_FF u_F = f_F - K_FP g`; the reduced matrix is the principal submatrix `K_FF` (rows and columns of F in
  ascending order, so it is symmetric exactly when K is). `ReducedSystem` holds the matrix, the right-hand side, the free
  equations, the prescribed pairs and `expand(u_F) -> u` (prescribed values copied bit for bit). This is the one function the
  transformation `u = T q + g` of ADR 0296 replaces.
* `reactions(k, u, f, prescribed) -> Vec<f64>`: `r = K u - f` at the prescribed equations, 0 at the free ones (the force the
  constraints exert on the body; `K u = f + r`).

### D4. Nodal forces (`loads.rs`)

* `NodalForceAt { node, force: Vec3 }`: a per-node force (the participant of ADR 0286 delivers one force per node).
* `nodal_load_vector(model, extra) -> Result<Vec<f64>>`: `Load::NodalForce` adds its force to every node of its set; `extra`
  adds per-node forces (missing node refused, `MissingNode { role: "nodal force" }`). Every other load variant is refused
  naming it (`UnsupportedLoad { load: "Traction" | "Pressure" | "Gravity" | "Centrifugal" | "BodyForceNodal" |
  "Temperature", package: "ADR 0276" or "ADR 0281" }`): nothing is ignored. ADR 0276 and 0281 replace those refusals in
  this file only; `solve.rs` calls this one function.

### D5. Free rigid-body motions: the check of the constraint set

Before any factorisation: parts by union-find over the element nodes (arithmetic 3); per part, the Gram matrix `G` of
arithmetic 1 over its prescribed translational components (components 3 to 5, which the basic tier does not have, would
enter as `e_{3 + k}` for the rotation `w_k`; written so that ADR 0294 only has to confirm it); its eigenvalues by
`rcfd_linalg::eigen::symmetric_eigen` (G is built symmetric exactly); the part leaves `count = #{lambda <= tau_G
lambda_max}` motions free (`tau_G = 1e-10`, arithmetic 2; all six if no component of the part is prescribed). Refusal
`RigidBodyMotionFree { part_node, part_nodes, count, motion }`, displayed as "the model leaves a rigid-body motion free: the
part containing node N (n nodes) has k unconstrained rigid-body motion(s), e.g. translation (..) with rotation (..) about
(..)", the example being the eigenvector of the smallest eigenvalue with `w = q[3..]/L`. Parts are checked in the order of
their smallest node. A node of no element with a component not prescribed: `FreeNode { node, component }`, "node N belongs
to no element and its component k is not prescribed: a free motion".

### D6. The static solve (`solve.rs`)

* `StaticSolver::Direct` (default: `SkylineLdlt` with the default options of ADR 0274, reverse Cuthill-McKee, positive
  pivots, symmetry tolerance 1e-12) or `StaticSolver::ConjugateGradient { tolerance, max_iterations }` (`rcfd_linalg::cg`
  with `Jacobi`, relative tolerance, absolute tolerance 0; not converged is an error stating iterations and residual).
  `StaticOptions { solver, threads }` with `Default`, `#[non_exhaustive]` so that ADR 0282 adds preconditioners.
* `solve_static(model, options)` and `solve_static_with(model, prescribed: &[Prescribed], forces: &[NodalForceAt],
  options) -> Result<SolidResult>`: validate (materials), `nodal_load_vector`, `prescribed_equations`, the check of D5,
  `stiffness_with`, `reduce`, solve, `expand`, `reactions`. Result: `SolidResult::new(dofs, u).with_reactions(r)
  .with_stats(SolverStats::new("skyline-ldlt" | "cg-jacobi", n_free, iterations, relative residual))`, the relative residual
  `|b_F - K_FF u_F|_2 / |b_F|_2` (0 when `b_F = 0`). No integration-point values (ADR 0277 adds them).
* Backstop for mechanisms (arithmetic 4): a skyline `Pivot` refusal, or a pivot with `d_i <= tau_P a_ii` (`tau_P = 1e-10`, a_ii
  the reduced diagonal), is `SingularStiffness { node, component, pivot, diagonal }`: "the reduced stiffness is singular at
  node N component k (pivot p, r of its diagonal): a mechanism or rigid-body motion is free, or the model is too
  ill-conditioned for the direct solver". Other skyline errors map to `Solver { solver, reason }`. With no free equation the
  solve is skipped (every displacement prescribed).

### D7. Error variants (`error.rs`, appended)

`Element { element, source: Box<FemError> }` ("element E: ..."), `MissingNode { role, node, n_nodes }`, `MissingComponent {
role, node, component, available }`, `ConflictingConstraint { node, component, first, second }`, `RigidBodyMotionFree {..}`,
`FreeNode {..}`, `SingularStiffness {..}`, `Solver {..}`, `UnsupportedLoad { load, package }`. Displayed as in D3 to D6;
`From<FemError> for rcfd_core::Error` keeps its catch-all arm.

### D8. Test set-up (`tests/sb_static.rs`)

* Generator: the SplitMix64 of ADR 0273 D6; **seed `0xF374_0275`**, streams `SEED + tag`: patch perturbation of kind k (Tet4
  0, Hex8 1, Tet10 2) tag `10 + k`; random forces of R1 tag `20 + k`.
* Material `E = 2.0e11 Pa`, `nu = 0.3` throughout.
* **Patch meshes:** origin `(0.1, -0.2, 0.3)`, lengths `(1.2, 1.0, 0.8)`, divisions `[4, 3, 3]`; Hex8 `hex_box`, Tet4
  `tet_box`, Tet10 = `upgrade_to_tet10` of the perturbed Tet4 mesh (straight edges). Interior grid nodes (in none of the six
  face node sets) perturbed by arithmetic 6; the mesh rebuilt through `SolidMesh::new` / `add_block` with the original
  connectivity and sets. Field `u = c + A x`, `c = (1e-3, -2e-3, 0.5e-3)`, `A = 1e-3 [[1.0, 0.3, -0.2], [0.5, -0.7, 0.4],
  [0.1, 0.6, 0.9]]`: strains `(1.0, -0.7, 0.9, 0.8, 1.0, -0.1) 1e-3`, all six non-zero. Every node of a face set prescribed in
  all three components by `Prescribed` (the per-node path); no load.
* **Uniaxial bars:** arithmetic 10, divisions `[6, 2, 2]` for the three kinds (Tet10 from the Tet4 box); P1, P2, P3 by
  `Prescribed`; consistent end loads computed in the test from `face_nodes`, `face_area` of the surface sets `xmax` (+) and
  `xmin` (-) and passed as `NodalForceAt`. Plus U3: Hex8 `[4, 1, 1]` through the model only: node sets `p1`, `p2`, `p3`,
  `Constraint::displacement` on them and `Load::nodal_force` on the node sets `xmax` and `xmin` with `+-F/4` (one face each end,
  four equal consistent loads).
* **Reaction block R1:** `[0, 1] x [0, 0.6] x [0, 0.5]`, divisions `[4, 3, 2]`, three kinds; `xmin` clamped by three
  `Constraint::displacement` (model path); forces on every node of `xmax` and `ymax`, each component uniform in `[-1e3, 1e3]
  N` (`NodalForceAt`), plus `Load::nodal_force("zmax", (100, -200, 300))`.

## Registered checks

All in `crates/rcfd-fem/tests/sb_static.rs`, release profile, one run (`cargo test -p rcfd-fem --release --test sb_static --
--nocapture --test-threads=1`). Every printed number is the maximum over the nodes (or entries) concerned.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| S1 | c2 | every stiffness of P, U, R (three kinds): the pattern is symmetric and `max |K_ij - K_ji| / max |K_ij|` | `<= 1e-13` | arithmetic 9: `<~ 1e-15` |
| D1 | c1 | stiffness of the P meshes (three kinds) by `stiffness_with(threads = 1)` and `(8)`: `row_ptr`, `col` equal and every value bit-equal (`to_bits`) | equal | D2: by construction |
| P1 | c3, c5 | patch, three kinds: `max_interior |u_h - u| / max_all |u|` (components) | `<= 1e-10` | arithmetic 5, 7: `<~ 2e-12` |
| P2 | c3 | patch: every prescribed component of the result equals its value bit for bit | equal | D3 `expand` |
| U1 | c6 | uniaxial, three kinds and U3: `max_{xmax} |u_x - F L/(E A)| / (F L/(E A))` | `<= 1e-10` | arithmetic 7, 10 |
| U2 | c7 | uniaxial, three kinds and U3: `max |u_y + nu eps y| / (nu eps b)` and `max |u_z + nu eps z| / (nu eps c)` over all nodes | `<= 1e-10` | arithmetic 7, 10 |
| R1 | c4, c8, c9 | block R1, three kinds: `|sum (f + r)| / sum |f|_2` and `|sum (x - x_c) x (f + r)| / (D sum |f|_2)`, D the box diagonal, `x_c` its centre | `<= 1e-10` each | arithmetic 8: `~ 1e-14` |
| R2 | c4, c8, c9 | patch, three kinds (no load): `|sum r| / sum |r|_2` and `|sum (x - x_c) x r| / (D sum |r|_2)` | `<= 1e-10` each | arithmetic 8 |
| F1 | c10 | Hex8 bar with `u_x = 0` on `xmin` only: refused `RigidBodyMotionFree`, count 3, message contains "rigid-body motion" | as stated | arithmetic 1 |
| F2 | c10 | uniaxial Hex8 bar with the 3-2-1 set without P3: refused, count 1 | as stated | arithmetic 1 |
| F3 | c10 | the bar with three collinear nodes `(0, 0, 0)`, `(1, 0, 0)`, `(2, 0, 0)` fully held: refused, count 1 | as stated | arithmetic 1 |
| F4 | c10 | a mesh of two disjoint Hex8 boxes, the first clamped: refused, `part_node` = the second box's first node, count 6 | as stated | arithmetic 3 |
| F5 | c11 | `Prescribed { node: n + 7 }`: refused, message contains "constraint" and the id `n + 7`; component 3 of a node: refused `MissingComponent`; `NodalForceAt { node: n + 7 }`: refused with the id | as stated | D3, D4 |
| E1 | D5 | a node of no element left free: `FreeNode` naming it; fully prescribed: accepted and solved | as stated | arithmetic 3 |
| E2 | D3 | the same equation twice: equal values accepted; different values refused `ConflictingConstraint` naming node and component | as stated | D3 |
| E3 | D4 | a model with `Load::gravity`: refused `UnsupportedLoad` naming "Gravity" | as stated | D4 |
| E4 | D1 | `assemble_with` whose element function fails for element 5: the error displays "element 5:" and the source | as stated | D1 |
| E5 | D6 | mechanism: two Hex8 cubes sharing one edge (a hinge), the first clamped: one part, D5 passes; refused `SingularStiffness` | as stated | arithmetic 4 |
| E6 | D6 | uniaxial Hex8: `ConjugateGradient { tolerance: 1e-13, max_iterations: 10 n }` against the direct solution: `max |u_cg - u_d| / max |u_d|` | `<= 1e-7` | cond `~ 1e5` x 1e-13 |
| E7 | D2 | the full solve of R1 (Tet10) with `threads` 1 and 8: displacements and reactions bit-equal | equal | D2 |
| E8 | D5 | the accepted models (P, U, R) print `lambda_min / lambda_max` of their Gram matrices | record (must be `> 1e-10`, else they would be refused) | arithmetic 2 |

**Negative controls** (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | P1's measure with the boundary values of the quadratic field `u = 1e-3 (x^2 + y z, y^2 - x z, z^2 + x y)` instead of the linear one (not an elastic solution: `div sigma = 2 (lambda + 2 mu) 1e-3 (1, 1, 1) != 0`), three kinds | `> 1e-6` |
| N2 | R1's force measure with the reactions left out (`sum f` alone) | `> 1e-3` |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f374-scratch`, own target directory; each applied
alone by exact text replacement, `sb_static` run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `element_dofs`: components in reverse order within a node | P1 |
| M2 | `reduce`: the term `- K_FP g` left out of the right-hand side | P1 |
| M3 | `reactions`: `K u` without `- f` | R1 |
| M4 | D5: the Gram matrix of the three translations only (rotation columns dropped) | F2, F3 |
| M5 | D5: all nodes in one part (no union-find) | F4 |
| M6 | D6: the `tau_P` test removed (skyline's own test only) | E5, or recorded as surviving with the measured pivot |
| M7 | D3: a conflicting second value overwrites the first | E2 |
| M8 | D3: the missing-node test removed | F5 (a panic is a failure) |
| M9 | D2: each worker's range scattered by the worker into its own matrix, the matrices summed in worker order | D1 |
| M10 | `loads`: `Load::NodalForce` adds only its x component | R1 |
| M11 | `pattern`: the columns of a row in descending node order (unsorted CSR) | P1 or S1 or a refusal |
| M12 | D5 threshold `tau_G = 1e-10` replaced by 1 (every part refused) | P1, U1, R1 |

**Decision rule.** F-374 is proposed **done** (pending the maintainer's acceptance) if S1, D1, P1, P2, U1, U2, R1, R2, F1 to F5
are met for every kind (every acceptance clause), N1 and N2 hold, and every mutation fails its test (M6 may survive if the
skyline's own test already refuses E5; that is then recorded with the pivot and is not a miss of the rule). E1 to E8 are
registered extensions: a miss among them is a `*_known_miss` record and a follow-up item and does not alone block done. A miss
or an unrun check among the acceptance checks, a failed negative control, or an uncaught mutation other than M6 (after
checking that the test covers it): **partial**, with the miss named. No crate that builds: **todo**.

## Limits (what is not covered)

- Only nodal forces; surface and body loads are refused by name until ADR 0276, the thermal load until ADR 0281.
- Only prescribed displacements per node and component; no multi-point constraint, no skew roller, no spring (ADR 0296).
- Rigid-body detection is exact for the rigid motions of each connected part and treats supports whose spread is below `1e-5`
  of the part size as free (arithmetic 2). A mechanism inside a part (parts joined by one node or one edge, a hinge) is not
  seen by D5; it is caught only by the pivot backstop, which is a numerical test (arithmetic 4): it refuses legitimate models
  whose flexibility ratio exceeds 1e10 (a solid cantilever beyond `L/h ~ 1000`) and could in principle pass a mechanism whose
  garbage pivot exceeds `1e-10` of its diagonal.
- The direct solver's reach (a few times 1e4 unknowns, ADR 0274); CG with Jacobi is the alternative, not tuned (ADR 0282).
- No stress or strain output (ADR 0277); the result carries displacement, reactions and solver statistics.
- The patch test is run on straight-sided Tet10 only; on curved Tet10 the 4-point rule does not pass it exactly (arithmetic 5).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Element errors are mapped to the element by the assembly (ADR 0273 open question 3, option (a)): `FemError::Element`.
2. Per-node prescribed values and forces are arguments of `solve_static_with`, not new fields of `SolidModel` (`model.rs`
   unedited; ADR 0291 owns its additions).
3. Threads from `std::thread::scope` rather than a rayon dependency of `rcfd-fem`.
4. Elimination by removal of the prescribed unknowns rather than by unit rows on the full matrix (no arbitrary diagonal value;
   the reduced matrix is a principal submatrix, symmetric when K is).
5. Variants appended to `error.rs` (D7), a file of ADR 0271 not in this package's file list; no other ADR of wave 3 edits it.
6. The thresholds `tau_G = 1e-10` and `tau_P = 1e-10` (arithmetic 2, 4) and the perturbation `r = 0.1` (arithmetic 6).

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-5 of the structural solver, basic tier. Size M.

### Goal

Assemble the global stiffness of a `SolidModel` into a `CsrMatrix` in a deterministic order, impose displacement constraints per
node and component (zero or not, so rollers and symmetry planes work) by symmetric elimination, apply nodal forces, solve with
the direct solver of ADR 0274 (CG with Jacobi as the alternative), and recover reactions. This is the first ADR that computes a
displacement field.

### Dependencies

ADR 0271, ADR 0273, ADR 0274; existing roadmap items: F-025.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. The stiffness of a SolidModel is assembled into a CsrMatrix
deterministically (bit-identical for 1 and 8 threads) and is symmetric within 1e-13 of its norm; displacement constraints per
node and component, zero or not, are imposed by symmetric elimination and the reactions recovered. Patch test (Tet4, Hex8,
Tet10; a box whose interior nodes are displaced by a registered seed; boundary displacements of a linear field with all six
strains non-zero): interior displacements within 1e-10 of the field, relative to its maximum. Uniaxial tension by nodal forces
with statically determinate constraints: elongation F L/(E A) and lateral contraction nu times the axial strain within 1e-10.
The reactions balance the applied forces and their moment within 1e-10; a model that leaves a rigid-body motion free is refused
with a message saying so, a constraint on a missing node with its id.

### Reference to check against

Closed forms: a linear displacement field is an exact solution for any conforming element (the patch test, Irons; Taylor, Simo,
Zienkiewicz and Chan, Int. J. Numer. Meth. Engng 22 (1986) 39, from memory); uniaxial Hooke's law.

### Files

`crates/rcfd-fem/src/{assemble.rs, constraint.rs, solve.rs, loads.rs}` (in `loads.rs` the nodal force only),
`crates/rcfd-fem/tests/sb_static.rs`.

### What could make it miss

Detecting an under-constrained model from a small pivot alone is fragile (a threshold registered from the stiffness scale is
needed); the patch-test limit of 1e-10 on Tet10 with a direct solve of a moderately conditioned matrix.
