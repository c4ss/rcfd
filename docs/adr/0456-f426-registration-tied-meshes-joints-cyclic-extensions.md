# ADR 0456: F-426 registration: tied non-matching meshes, beam and shell joints to solids, non-matching cyclic boundaries and nodes on the axis

Status: **REGISTERED 2026-11-14, written before any code of F-426 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-426 was written, compiled or run in the repository, except (disclosed below) the bit
record B1 of the default paths (a test file using only the API of main b154dbe6, run once at that commit with no library line
changed) and an independent numpy prototype of the node-to-surface tie in the scratch folder. Results go in the addenda at the
end; nothing above the addenda is changed after a run.

Work package SA-19 of the structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)); stub
[ADR 0329](0329-tied-non-matching-meshes-joints-cyclic-extensions.md), kept unchanged. Roadmap item F-426 (deps F-408, F-391,
F-392, F-393, F-394). Size M. Number 0456 given by the lead. Branch `f426-work` from main b154dbe6, worktree `w49i-work/f426`,
scratch `w49i-work/f426-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited except the scaffold records-and-copies step (D8) of the maintainer's standing decision;
clean-room.

## What F-426 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. With matching meshes the tied model equals the merged one within 1e-10. Flat non-matching interfaces 2:3 and 3:5 under uniaxial stress: far-end displacement within 0.1 percent and interface stress within the registered 3 percent (a one-sided node-to-surface tie does not pass the patch test exactly), the error falling with an observed order >= 1 under refinement. A cantilever cut in two and tied 2:3 (Tet10 to Hex20): tip deflection within 1 percent of the closed form. The constraint forces of a tie have zero resultant and moment within 1e-10. A node farther from every face than the registered tolerance is refused with its id. Joints: a beam continuing a solid bar and a shell continuing a solid plate give the tip deflection of the one-piece cantilever within 1 and 2 percent. Cyclic: the ring sector of F-394 with a 2:3 boundary has its frequencies within 0.5 percent of the matching sector; a solid disc with nodes on the axis, spinning (index 0): centre stress (3 + nu) rho Omega^2 b^2/8 within 1 percent; a rod on the axis at index 1 has the bending frequency of the beam within 1 percent, and at index 2 its axis nodes are held.

Clauses: (a0) every limit registered before the run (this document); (a1) matching meshes: tied equals merged within 1e-10;
(a2) flat 2:3 and 3:5 interfaces, uniaxial stress: far-end displacement within 0.1 percent; (a3) interface stress within 3
percent; (a4) the error falling with an observed order >= 1 under refinement; (a5) the cantilever cut and tied 2:3, Tet10 to
Hex20, tip deflection within 1 percent of the closed form; (a6) the constraint forces of a tie have zero resultant and moment
within 1e-10; (a7) a node farther than the tolerance from every face refused with its id; (a8) a beam continuing a solid bar:
tip deflection of the one-piece cantilever within 1 percent; (a9) a shell continuing a solid plate: within 2 percent; (a10) the
F-394 ring sector with a 2:3 boundary: frequencies within 0.5 percent of the matching sector; (a11) a spinning solid disc with
nodes on the axis at index 0: centre stress `(3 + nu) rho Omega^2 b^2 / 8` within 1 percent; (a12) a rod on the axis at index 1:
the bending frequency of the beam within 1 percent; (a13) at index 2 its axis nodes held.

### What the arithmetic says about (a2) to (a4) (a correction of the roadmap's form)

The roadmap pairs the node-to-surface tie with a 3 percent interface stress and an error that falls with refinement. Both are
wrong for the one-pass node-to-surface (NTS) tie, by arithmetic 1 and the prototype (P below):

- The NTS tie fails the patch test by a self-similar pattern: at a fixed mesh ratio the force error per master node is a fixed
  fraction of the consistent nodal force (arithmetic 1: at 2:3 the trapezoid weights of the slave give the master corner
  `5/18` of the face width instead of `1/4`, the middle node `4/9` instead of `1/2`, 11 percent each way per direction). The
  local stress error near the interface therefore does not fall under refinement at a fixed ratio: P gives the largest
  integration-point error 0.1486 at 2:3 and 0.1490 at 4:6, 0.1011 at 3:5 and 6:10, five times the roadmap's 3 percent.
- The far-end displacement error is first order (P: 2.88e-3 at 2:3, 1.43e-3 at 4:6, observed order 1.014; 4.63e-4 at 3:5,
  2.29e-4 at 6:10, order 1.012), so the 0.1 percent of (a2) is missed by NTS at the coarser 2:3 level of this bar.

The roadmap clauses (a2), (a3) are therefore registered against the **mortar tie** (D2), which passes the flat patch test
exactly (arithmetic 2): its errors are rounding. The order clause (a4) is registered where an error exists, on the NTS far-end
displacement (T4c), and the corrected statement for the NTS interface stress is registered as a check that it does **not** fall
(T4d: the level-2 value within 2 percent of the level-1 value). The NTS numbers themselves are registered against P (T4a, T4b).

## What was read (main b154dbe6, read only)

- `crates/rcfd-fem/src/mpc.rs` (the transformation `x = T q + g`, the row elimination with designated dependents, the chain
  refusal, `cyclic_rows`, `harmonic_transformation`, `solve_static_mpc`, `modal_analysis_mpc`), `constraint.rs`, `model.rs`
  (`Constraint`, `LinearTerm`, `add_constraint`, the element checks, `DofMap::for_mesh`), `cyclic.rs` (`boundary`, `full_rotor`,
  `modal_analysis_harmonic`), `loads.rs` (`face_shape_functions`, `face_rule`), `mesh.rs` (faces, `face_area_vector`, sets),
  `builders.rs` (`hex_box`, `tet_box`, `upgrade_to_tet10`), `hex20.rs` (`upgrade_to_hex20_mapped`, face functions),
  `convert.rs` (`project_mid_edge_nodes`, `onto_cylinder`), `beam.rs` and `shell.rs` (headers, `check_beam_model`),
  `recover.rs` (`IntegrationPointField`), `tie.rs` (the stub of ADR 0440).
- Tests: `si_cyclic.rs` (the F-394 ring sector RS, the disc DP, R4), `sa_scaffold.rs` (X1 rows), `si_shell.rs` (CT1).
- ADRs 0329 (stub), 0386 (cyclic), 0296 (transformation), 0417 (Beam2), 0432 (Shell4: CT1, 15 elements, tip 1.05e-3 low),
  0440 (scaffold), 0452 (style; O6 Hex20 cantilever 3.9e-4 of the Timoshenko form, from ADR 0385).

## Closed forms and arithmetic (first principles)

1. **NTS patch test.** Two bars tied at a flat plane, uniform stress `sigma` normal to it. The linear field `u*` of uniaxial
   stress satisfies every NTS row (the master face interpolates a linear field exactly). The reduced residual at master node
   `a` is `sum_s N_a(x_s) f_s - F_a`, `f_s` the consistent nodal forces of `sigma` on the slave face and `F_a` those on the
   master face. In one direction, master hats on nodes `0, 1/2, 1`, slave nodes `0, 1/3, 2/3, 1` with trapezoid weights
   `1/6, 1/3, 1/3, 1/6`: `sum_s w_s N_0(x_s) = 1/6 + 1/9 = 5/18` against `1/4`, `N_{1/2}`: `4/9` against `1/2`. The residual is
   self-equilibrated (zero resultant and first moment: `sum N_a = 1`, `sum N_a x_a = x`, trapezoid exact for linear functions)
   and scale-invariant at a fixed ratio, so the local stress error is O(1) and the far-end error O(h) (the perturbation is a
   boundary layer of width h).
2. **Mortar patch test.** The mortar rows `int_S N_j (u_s - u_m) dA = 0` for each slave node `j` (the standard multiplier
   space: the slave face functions), `D u_s = M u_m`, `D_jk = int N_j N_k`, `M_ja = int N_j N_a^m`, both integrated on the
   segments of the slave faces cut by the master faces. (i) A field linear on the interface (quadratic for Tri6 and Quad8
   faces) is reproduced by both face interpolations, so it satisfies the rows. (ii) Eliminated as `u_s = W u_m`,
   `W = D^-1 M`, the reduced residual at master node `a` for a uniform traction `t` is `(W^T F_s)_a - F_a` with
   `F_s = D 1 t` (the consistent slave forces, `sum_k N_k = 1`): `W^T D 1 t = M^T 1 t = int N_a^m t = F_a` when the slave
   surface covers the master surface (`sum_j N_j = 1` on it). So `u*` is the discrete solution: the patch test passes up to
   rounding. Exactness needs exact segment integrals: for straight-edged planar faces with affine maps (triangles,
   parallelograms) the integrands are polynomials of total degree at most 6 (Quad8 times Quad8; serendipity Quad8 has total
   degree 3), integrated exactly by the collapsed 4 x 4 Gauss rule on each triangle of the clipped polygon (degree 7 in `s`,
   the Jacobian `s` included).
3. **Prescribed slave components (mortar).** For component `c`, the slave nodes split into free `F` and prescribed `P`
   (a `Displacement` constraint of the model on `c`); the multipliers of `P` are dropped (their row is the Dirichlet row) and
   `u_F = D_FF^-1 (M_F u_m - D_FP u_P)`. In the uniaxial set-ups the prescribed components carry no traction, so (ii) holds.
   NTS skips the row of a prescribed slave component (the Dirichlet row replaces it).
4. **Distributing coupling (joints).** A node `b` (beam end, shell edge node) follows the least-squares rigid motion of a set
   of solid nodes `i` with weights `w_i = 1`: minimise `sum w_i |u_i - t - theta x r_i|^2`, `r_i = x_i - x_b`, so
   `A (t, theta) = sum w_i B_i^T u_i`, `B_i = [I, -[r_i]x]`, `A = sum w_i B_i^T B_i`. For a set not on one line `A` is
   regular and the six rows `x_b = A^-1 sum w_i B_i^T u_i` hold; exact for rigid motions (`u_i = t + theta x r_i` gives
   back `(t, theta)`). For a set on one line of direction `d` (the through-thickness column of a shell node) `A` has the null
   vector `(0, d)` (rotation about the line): `x_b - A^+ b` lies in the null space, and the rows are written in solved form
   with the free rotation component of largest `|d_c|` left independent (D3). For a column `z_i d`, `theta_perp =
   sum z_i d x u_i / sum z_i^2`, the mean translation `t`.
5. **Axis nodes of a cyclic sector.** A node `p` on the axis is on both boundaries; its harmonic rows are `a = R (c a - s b)`,
   `b = R (s a + c b)` (doubled, `0 < 2k < N`), `a = R a` (`k = 0`), `a = -R a` (`2k = N`), with `R` the rotation by
   `alpha = 2 pi/N`. With `J = n x` (in-plane rotation by 90 degrees), `R = cos(alpha) I + sin(alpha) J` in the plane and `1`
   along `n`. Axial: `(1 - e^{i theta}) U_n = 0`, held unless `k = 0`. In-plane: `U = e^{i theta} R U` has a non-zero solution
   only when `e^{i theta} e^{+-i alpha} = 1`: `k = 0` held (`N >= 2`), `k = 1` doubled free with `b = J a` (2 unknowns of 4),
   `2k = N` held unless `N = 2`, every other `k` held. So at `k = 2` (`N >= 5`) every axis unknown is held, at `k = 1` the
   axial ones; at `k = 0` the axis nodes move along the axis only. The rows are generated from the null space of the node's
   own constraint matrix (D4), in solved form.
6. **Rotating solid disc** (plane stress, Timoshenko and Goodier, from memory; derived): `sigma_r = (3 + nu)/8 rho Omega^2
   (b^2 - r^2)`, `sigma_theta = rho Omega^2 ((3 + nu) b^2 - (1 + 3 nu) r^2)/8`; checked here: equilibrium `d(r sigma_r)/dr -
   sigma_theta + rho Omega^2 r^2 = 0` holds term by term (`(3 + nu)(b^2 - 3 r^2) - (3 + nu) b^2 + (1 + 3 nu) r^2 + 8 r^2 = 0`),
   compatibility `d(r eps_theta)/dr = eps_r` holds, `sigma_r(b) = 0`, finite at `r = 0`; centre `sigma_r = sigma_theta = (3 +
   nu) rho Omega^2 b^2/8`, the roadmap's form, right.
7. **Cantilever** (Timoshenko beam): `delta = P L^3/(3 E I) + P L/(kappa G A)`, `kappa = 10 (1 + nu)/(12 + 11 nu)` for a
   rectangle (Cowper, from memory), `5/6` at `nu = 0`. **Regular polygon** of `M` sides, circumradius `a`: `A = (M/2) a^2
   sin(2 pi/M)`; the polar moment of the triangle with apex at the centre and vertices `p1, p2` is `A_t (|p1|^2 + p1.p2 +
   |p2|^2)/6`, so `J = A a^2 (2 + cos(2 pi/M))/6` and `I = J/2 = A a^2 (2 + cos(2 pi/M))/12` (limit `pi a^4/4`).
8. **Default tolerance of the tie.** A slave node more than `GAP_FACTOR = 0.05` times the mean master face size
   `h_m = mean sqrt(area)` from every master face is refused (`FemError::TieGap { node, distance, tolerance }`); an absolute
   tolerance can be given instead.

## Arithmetic before this registration (disclosed)

- **B1** (`crates/rcfd-fem/tests/sa_tie_default_bits.rs`): FNV-1a digests of the bits of the default paths F-426 touches, run
  once on b154dbe6 with no library change: the ring sector's boundary pairs, its transformation at `k = 0`, its harmonic
  transformations at `k = 1, 6`, its modes at `k = 0, 1, 6`, its full rotor, the R4 refusal text, the spinning disc
  (displacement and constraint forces), a linear-constraint cantilever (displacement and constraint forces). The 13 words are
  in the test's `BASE`.
- **P** (`w49i-work/f426-scratch/proto_tie.py`, numpy, dense): Hex8 bar of two parts, `E = 1`, `nu = 0.3`, section `[0,1]^2`,
  part A `x in [0, 1]` with `m x m x m` cubes (master), part B `x in [1, 3]` with `s x s x 2s` cubes (slave), `u_x = 0` on
  `x = 0`, `u_y = 0` on `y = 0`, `u_z = 0` on `z = 0` (both parts), uniform traction 1 on `x = 3`; NTS rows `u_s = sum N_a
  u_a` (bilinear master faces), a prescribed slave component left to its Dirichlet row. Measured: far-end error (mean `u_x`
  of the `x = 3` nodes over `3/E`, minus 1), the largest `abs(sigma_xx - 1)` over the 2 x 2 x 2 Gauss points of the elements
  touching `x = 1` (both sides), the largest of the element means. Results: 2:3 `+2.8797762239e-03`, `1.4860378066e-01`,
  `9.3068612995e-02`; 4:6 `+1.4258034027e-03`, `1.4897849755e-01`, `9.2087955168e-02`; 3:5 `+4.6271030946e-04`,
  `1.0105716779e-01`, `5.6769147161e-02`; 6:10 `+2.2937058253e-04`, `1.0105258418e-01`, `5.6804678473e-02`.

## Design

### D1 `tie` (filled; `PENDING = None`): the search and the node-to-surface tie

- `TieMethod::{NodeToSurface, Mortar}`; `TieOptions { method, tolerance: Option<f64> }` (absolute; `None`: arithmetic 8).
- The master faces (a surface set) go into a bucket grid (cell size `h_m`, each face in the cells its box plus the tolerance
  covers). A slave node (of a surface set) is projected onto each candidate face: Gauss-Newton on `(x(u,v) - p) . x_u = 0`,
  `(x(u,v) - p) . x_v = 0` from the face centre (at most 25 iterations, step below `1e-14`), the face functions of
  `loads::face_shape_functions` (Tri3, Quad4, Tri6, Quad8), inside with slack `1e-8` in the face coordinates, clamped to the
  face. The nearest inside projection wins (the lowest face index on a tie); with none inside the nearest clamped point; a
  distance above the tolerance is refused naming the node.
- NTS rows: `u_s,c - sum_a N_a(u, v) u_a,c = 0` for `c = 0, 1, 2`, as `Constraint::Linear` (zero terms omitted), designated
  the slave component; a component of the slave node prescribed by a `Displacement` constraint of the model is skipped
  (arithmetic 3). A slave node that is also a master face node is refused (key `tie`). `tie::tie` returns a `TieReport`
  (constraints, projections with node, face, coordinates, distance, skipped components, terms); `tie::add_tie` adds the
  constraints to the model. Nothing in the transformation changes: the rows are linear constraints of ADR 0296.
- `tie::assemble_parts(&[(&str, &SolidMesh)])`: meshes side by side without merging nodes, sets prefixed `name.`, block
  properties kept (the parts to be tied).

### D2 Mortar tie

Arithmetic 2 and 3: for each slave face its plane (centroid, unit area vector), the master faces of the bucket grid near it,
both corner polygons projected on the plane, clipped (Sutherland-Hodgman, convex), fan-triangulated, the collapsed 4 x 4 Gauss
rule on each triangle; a point maps to the slave face coordinates by the in-plane inverse of the face map and to the master
face by projection along the slave normal (Gauss-Newton as D1). `D` and `M` dense over the interface; per component
`W = D_FF^-1 [M_F, -D_FP]` (LU with partial pivoting); rows `u_j,c - sum W u = 0` with every non-zero entry. A slave node with
`D_jj = 0` (no master overlap) is refused naming it (after the gap check of D1, which applies to the slave nodes as well).

### D3 Joints

`tie::beam_to_solid(model, beam_node, surface)` and `tie::shell_to_solid(model, shell_nodes: &str, surface, tolerance)` return
the rows of arithmetic 4 as `Constraint::Linear`: the beam node to all nodes of the surface set; each shell node of the node
set to the column of surface nodes within `tolerance` of the line through it along its normal (the mean of the unit facet
normals of its shell elements), at least two, else refused naming the node. Rows in solved form: the dependent components of
the node (all six for a regular `A`; for a line the component of the free rotation of largest `|d_c|` stays independent) get
unit coefficient, the other terms are the solid unknowns and the node's independent components. Null space from
`rcfd_linalg::eigen::symmetric_eigen` of `A` with the relative threshold `1e-10`.

### D4 Cyclic extensions (opt-in)

`CyclicOptions { interpolation: Option<TieOptions>, axis_nodes: bool }` (`Default`: none, false: the bits of ADR 0386), a new
field `options` of `Constraint::Cyclic`, set by the new `Constraint::cyclic_with(.., options)`; `Constraint::cyclic` gives the
default. In `cyclic::boundary`:

- `axis_nodes`: a node on both boundaries is accepted when within `MATCH_TOLERANCE L` of the axis (else refused naming it) and
  listed in `CyclicBoundary::axis_nodes`; its rows (in `mpc::cyclic_rows`, for every harmonic) come from the null space of
  its own constraint matrix (arithmetic 5), in solved form (largest-pivot elimination on the null-space basis).
- `interpolation`: the high nodes (other than axis nodes) are tied to the low faces rotated by `R` (positions `R x + (I - R) o`)
  with the method of D1/D2: `CyclicBoundary::interpolated` holds per high node its weights `(low node, w)`; the rows are
  `a_H - R (c sum w a_L - s sum w b_L) = 0` and the sine pair, as for a matched pair with the low unknowns weighted.
- Without options the matched path is untouched (B1). `full_rotor` refuses a boundary with axis nodes or interpolation
  (key `cyclic`, naming ADR 0456).

### D5 Error variant

`FemError::TieGap { node, distance, tolerance }` (new variant of the non-exhaustive enum).

### D6 Not changed

The elimination, the solvers, the element routines, `harmonic.rs`, `rotating.rs`, `free_motions.rs` (a cyclic constraint stays
refused there), every default path.

### D7 Tests

`crates/rcfd-fem/tests/sa_tie.rs` (the checks below; heavy ones `#[ignore]` per rule 7 with a fast twin),
`sa_tie_default_bits.rs` (B1).

### D8 Scaffold rows (`sa_scaffold.rs`, the standing decision)

The latest X1 copy `x1_modules_and_pending_after_f409` becomes an `#[ignore]` record citing ADR 0291 and ADR 0456 (tie
landed); a new copy `x1_modules_and_pending_after_f426` has the row `tie` "landed". No other line of the file changes.

## Checks

Common: `E = 2e11`, `rho = 7850` unless stated; `delta` of a tip load is `sum f . u / P` over the loaded nodes (work
conjugate). `rel(a, b) = abs(a/b - 1)`.

| ID | Clause | Set-up and measure | Limit | Estimate |
|---|---|---|---|---|
| T1 | a1 | Hex8 bar `[0,2] x [0,0.5]^2`, two parts `[2,2,2]` each (A `x <= 1` master, B slave), tied at `x = 1` (NTS, and mortar), against the merged `hex_box [4,2,2]`; `nu = 0.3`, `xmin` clamped, traction `(1e6, 5e5, -3e5)` on `xmax`; the same with Hex20 parts. Max over nodes `abs(u_tied - u_merged)` / max `abs(u_merged)` | `<= 1e-10` | 1e-14 (weights exact to rounding) |
| T2a | a2 | The bars of P (2:3, 4:6, 3:5, 6:10), Hex8, `E = 1`, mortar: `abs(far-end error)` | `<= 1e-3`, also `<= 1e-9` | rounding, 1e-12 |
| T2b | a3 | Same: largest `abs(sigma_xx - 1)` at the integration points of the elements touching `x = 1` | `<= 0.03`, also `<= 1e-9` | rounding |
| T2c | a2, a3 | Same: largest `abs` of every other stress component there | `<= 1e-9` | rounding |
| T4a | a2 (NTS) | The bars of P, NTS: far-end error against P | `abs(e - e_P) <= 1e-6 abs(e_P)` | 1e-9 (two solvers' rounding) |
| T4b | a3 (NTS) | NTS: the largest integration-point and element-mean errors against P | relative `<= 1e-6` | 1e-10 |
| T4c | a4 | NTS: observed order `log2(e_1/e_2)` of the far-end error, 2:3 to 4:6 and 3:5 to 6:10 | `>= 1` | 1.014, 1.012 (P) |
| T4d | a4 (corrected) | NTS: largest integration-point error at level 2 over level 1 (does not fall) | `>= 0.98` | 1.003, 1.000 (P) |
| T3 | a5 | Cantilever `L = 1`, section `0.1 x 0.1`, `nu = 0`: Hex20 `[10,2,2]` on `x <= 0.5` (master), Tet10 `tet_box [15,3,3]` on `x >= 0.5` (slave), `xmin` clamped, traction `(0, 0, -P/A)` on the Tet10 `xmax`, `P = 1000`, mortar: `rel(delta, delta_T)` (arithmetic 7) | `<= 0.01` | 4e-3 (Hex20 3.9e-4; Tet10 on the tip half, stiffer, some 1e-3; tie, mortar, small) |
| T3b | a5 | T3 with NTS | `<= 0.01` | as T3 plus `O(h/L)` |
| T5 | a6 | T3 (mortar) and T3b: resultant `abs(sum rho)` and moment about the origin `abs(sum x x rho)` over the nodes of the two interface surfaces, over `F_t = sum abs(rho)` there and `F_t * 0.1 sqrt(2)` | each `<= 1e-10` | 1e-14 (row coefficients sum to 1, flat interface, no gap) |
| G1 | a7 | The 2:3 bar of P with one interior slave node of `x = 1` moved by `0.1 h_m` along `+x`: `tie` refused with `FemError::TieGap { node }` of that node, `distance` within `1e-12` relative of `0.1 h_m`, `tolerance = 0.05 h_m`; moved by `0.02 h_m`: accepted, reported distance within `1e-12` relative of `0.02 h_m` (both methods) | as stated | as stated |
| J1 | a8 | Hex20 bar `[10,2,2]`, `x in [0, 0.5]`, section `0.1 x 0.1`, `nu = 0`, `xmin` clamped; node `b` at `(0.5, 0.05, 0.05)`, 15 Beam2 to `x = 2` (`A = 0.01`, `I = 0.1^4/12`, `J = 1.406e-5`, `kappa = 5/6`, orientation `(0, 1, 0)`), `beam_to_solid(b, B.xmax)`; tip force `(0, 0, -P)`: `rel(delta, delta_T)` with `L = 2` | `<= 0.01` | 5e-3 (solid 4e-4, beam exact, joint local `O(h/L)`) |
| J2 | a9 | Hex20 plate `[4,2,1]`, `x in [0, 0.2]`, `y in [0, 0.1]`, `z in [-0.01, 0.01]`, `nu = 0`, `xmin` clamped; Shell4 `16 x 2` from `x = 0.2` to `1` (`t = 0.02`), its `x = 0.2` nodes `shell_to_solid` to the plate's `xmax` (tolerance `1e-9`); tip force `-P` split `1/4, 1/2, 1/4` on the three tip nodes: `rel(delta, delta_T)` with `L = 1`, `I = b t^3/12` | `<= 0.02` | 5e-3 (Shell4 1.05e-3 at 15 elements, ADR 0432; joint local) |
| C1 | a10 | The F-394 ring (`R = 1`, `T = 0.05`, `W = 0.1`, `N = 12`, `nu = 0`, spoke clamp): two parts, A `phi in [0, alpha/2]` Hex20 `[1,2,2]`, B `[alpha/2, alpha]` `[1,2,3]`, B's `ymin` mortar-tied to A's `ymax`, cyclic low A `ymin`, high B `ymax` with mortar interpolation; against the matching sector `[1,4,3]`: the four lowest frequencies at each `k = 0..6`, max `rel` | `<= 0.005` | 1e-3 |
| C1b | a10 | C1 with NTS interpolation for the cyclic rows (the internal tie mortar) | `<= 0.005` | 2e-3 |
| C2 | a11 | Disc sector `N = 8`, `b = 1`, half thickness `0.025` (`u_z = 0` at `z = 0`), Tet10 from `tet_box` in `(r, phi, z)` `[8, 3, 1]` with the `r = 0` nodes merged (degenerate tetrahedra dropped), outer mid-edge nodes on the circle, `nu = 0.3`, `Omega = 100`, `u_y = 0` at `(b, 0, 0)`, `axis_nodes`: at every integration point of the elements with an axis node, `abs(sigma_xx - s_xx)`, `abs(sigma_yy - s_yy)` over `sigma_c = (3 + nu) rho Omega^2 b^2/8`, `s` the closed-form field (arithmetic 6) at the point; max | `<= 0.01` | 4e-3 (linear stress per Tet10 element against a quadratic field, `(h/b)^2/4`) |
| C2b | a11 | C2: the in-plane unknowns of the axis nodes are dependent with empty `T` rows and `g = 0` | `==` | `==` |
| C3 | a12 | Rod `a = 0.05`, `L = 1.5` on the axis, `N = 8`, Tet10 from `tet_box` in `(r, phi, z)` `[2, 2, 30]`, `r = 0` merged, straight edges (a 16-gon section), `nu = 0.3`, `zmin` clamped, `axis_nodes`; lowest value at `k = 1` against Beam2 (60 elements along z, the 16-gon's `A`, `I` of arithmetic 7, `kappa = 6 (1 + nu)/(7 + 6 nu)`, consistent mass, `modal_analysis_mpc`): `rel(f, f_beam)` | `<= 0.01` | 4e-3 |
| C4 | a13 | C3 at `k = 2`: every axis-node unknown (both parts) dependent with an empty `T` row and `g = 0`; C4b at `k = 1`: the axial ones so, and two of the four in-plane ones per node independent | `==` | `==` |
| B1 | rule 3 | `sa_tie_default_bits.rs` against `BASE`; and the existing `si_cyclic`, `si_mpc`, `si_cyclic_harmonic`, `si_rotating_cyclic`, `si_modal_mpc`, `sa_scaffold` (with D8) pass | `==` | `==` |
| X1 | D8 | `x1_modules_and_pending_after_f426` passes | as stated | as stated |

### Negative controls

| ID | Set-up | Expected |
|---|---|---|
| N1 | C1's two-part sector with the cyclic constraint without options | refused by `boundary` (key `cyclic`, a low node without image) |
| N2 | C2's sector with the cyclic constraint without `axis_nodes` | refused (node on both boundaries) |
| N3 | C2's mesh with the low set given as high also and `axis_nodes`: a node on both but off the axis | refused naming the node |
| N4 | J1 without the joint constraints | the static solve refused (singular, a beam unknown) |
| N5 | J2 with a shell node moved by `0.01` in y (no column) | `shell_to_solid` refused naming the node |
| N6 | `full_rotor` of C2 | refused (key `cyclic`) |

### Planned mutations (scratch copy `w49i-work/f426-scratch/mut`, test `sa_tie` per mutation)

| ID | Mutation | Expected to catch |
|---|---|---|
| Mu1 | NTS weights replaced by the nearest master node (weight 1) | T4a |
| Mu2 | mortar `D` lumped (row sums on the diagonal) | T2a/T2b |
| Mu3 | mortar rule: centroid of each clipped triangle only | T2b or T1 |
| Mu4 | clipping drops the closing edge of the clip polygon | T2 or T3 |
| Mu5 | gap check disabled | G1 |
| Mu6 | beam joint: the three rotation rows dropped | J1 |
| Mu7 | shell joint: sign of `theta_perp` flipped | J2 |
| Mu8 | cyclic interpolated rows: `s` replaced by `-s` | C1 |
| Mu9 | axis rows not generated | C2b, C4 |
| Mu10 | interpolation: low faces rotated by `R^-1` instead of `R` | C1 (refused or off) |
| Mu11 | matched cyclic path: pairs in descending order | B1 |

## Decision rule

**done**: every check of the table met, every negative control as expected, every planned mutation caught, B1 and X1 green,
no amendment after a run. **partial**: the tie part (T1 to T5, G1) met and any other check missed, or a mutation not caught, or
a check met only after an amendment made after its run. **todo**: a T check missed.

## Limits (what is not covered)

- Curved interfaces: NTS projects onto the curved face; mortar integrates on the slave face planes (an approximation, gaps and
  overlaps not treated). Exactness of D2 needs straight-edged planar faces with affine maps.
- Large interfaces: the mortar rows are dense over the interface (`D^-1` is full), filling `T` and `K_r`; the number of terms is
  reported. NTS rows have at most 8 master terms.
- One tie per slave node; overlapping ties are left to the elimination (redundant rows dropped, a chain refused).
- The joints use unit weights; no stiffness of the joint itself; the shell drilling rotation stays free at the joint (the shell
  drilling penalty holds it; ADR 0445/F-525: expect rounding-level drilling rotations, no limit is registered on them).
- The cyclic extensions are not in `full_rotor`, in `free_motions`, or in the rotating paths (the rows go through
  `cyclic_rows`, so those paths take them where they use the transformation, untested).
- The mortar tie is used for the cyclic interpolation as a scalar weight map rotated by `R`; prescribed high-boundary
  components are not excluded there.

## Amendment 1 (2026-11-14, after the registered run of commit 11ec84f8, before any corrected run)

The registered run of `sa_tie` (commit 11ec84f8, log `f426-scratch/run1.log`) stopped C3 and C4 before any check: the
subspace iteration of the rod at `k = 1` did not converge to the test's tolerance `1e-10` in 400 iterations (residuals of the
wanted pairs `1.13e-8`, `1.24e-8`, `2.96e-10`, `3.01e-10`). That tolerance was a set-up choice of the test, not a registered
limit; a residual of `1e-8` bounds the eigenvalue error far below the 1 percent of C3. Correction: the test `c3b_c4b_rod_on_the_axis`
runs C3 and C4 unchanged with the modal tolerance `1e-8` (the rod and the beam) and 600 iterations; the old test stays as an
ignored record. By the decision rule C3 and C4 count as met only after an amendment made after their run.

J1 and J2 missed in the same run (J1 `2.25e-2` against `1e-2`, J2 `2.36e-2` against `2e-2`); they become ignored
`*_known_miss` records with the limits intact (diagnosis in addendum 1).

## Amendment 2 (2026-11-14, after the run of `c3b_c4b_rod_on_the_axis` of commit d394fa73, before any corrected run)

The corrected test stopped again before any check: at tolerance `1e-8` and 600 iterations the residuals of the first pair
stayed at `1.21e-8`, `1.23e-8` (the second pair `3.0e-10`), the same as at 400 iterations: a floor of the residual, not slow
convergence. Correction: `c3c_c4c_rod_on_the_axis`, C3 and C4 unchanged with the modal tolerance `1e-6` (rod and beam; a residual
of `1e-6` changes the frequency by far less than the 1 percent of C3); `c3b_c4b_rod_on_the_axis` stays as an ignored record.
