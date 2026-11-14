# ADR 0460: F-543 registration: joint couplings with consistent face weights

Status: **REGISTERED 2026-11-14, written before any code of F-543 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-543 was written, compiled or run in the repository, except (disclosed below) the bit
record B1 of the default joint paths (a test file using only the API of main 2504fd62, run once at that commit with no library
line changed) and a plain-Python evaluation of Quad8 face integrals in the scratch folder. Results go in the addenda at the
end; nothing above the addenda is changed after a run.

Follow-up of F-426 ([ADR 0456](0456-f426-registration-tied-meshes-joints-cyclic-extensions.md), its J1 and J2 known misses).
Roadmap item F-543 (deps F-426). Number 0460 given by the lead. Branch `f543-work` from main 2504fd62, worktree
`w49i-work/f543`, scratch `w49i-work/f543-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited (new tests in new files); clean-room.

## What F-543 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The beam-to-solid and shell-to-solid couplings of ADR 0456 weighted by the consistent nodal areas of a linear traction on the solid face, a shell edge coupled to every node of the face through the edge interpolation, and a rigid-face option: the J1 and J2 set-ups of ADR 0456 give the tip deflection of the one-piece cantilever within 1 and 2 percent; a rigid face within 0.1 percent at nu = 0; the coupling exact for the six rigid motions within 1e-12; the known-miss records of ADR 0456 kept.

Clauses: (a0) every limit registered before the run (this document); (a1) the beam-to-solid coupling weighted by the
consistent nodal areas of a linear traction on the solid face; (a2) the same for the shell-to-solid coupling; (a3) a shell edge
coupled to every node of the face through the edge interpolation; (a4) a rigid-face option; (a5) J1 within 1 percent of the
one-piece cantilever; (a6) J2 within 2 percent; (a7) a rigid face within 0.1 percent at `nu = 0`; (a8) the coupling exact for
the six rigid motions within 1e-12; (a9) the known-miss records of ADR 0456 kept.

Reading of (a7): ADR 0456 addendum 1 measured "a rigid face" on J1 (D2, `-9.1e-6`), and the roadmap clause comes from that
remark; (a7) is registered on J1 with the rigid-face option (J1R). The rigid option of the shell joint is registered as J2R with
its own limit from its estimate (below). Taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What was read (main 2504fd62, read only)

- ADR 0456 in full (D3, arithmetic 4, the checks J1, J2, N4, N5, addendum 1 with the diagnosis D1 to D6, the mutations Mu6 and
  Mu7, the remaining question 1).
- `crates/rcfd-fem/src/tie.rs` (the faces, `Face::eval`, the collapsed Gauss rule of the mortar tie, `coupling`,
  `beam_to_solid`, `shell_to_solid`), `loads.rs` (`face_shape_functions`, `face_rule`, `nodal_load_vector`), `hex20.rs`
  (`traction_field_loads`), `model.rs` (`Constraint::Linear`, `LinearTerm`, `DofMap`), `mesh.rs` (`face_nodes`,
  `surface_set_nodes`).
- `crates/rcfd-fem/tests/sa_tie.rs` (`j1_model`, `j2_model`, the two known-miss tests, `rigid_rows`, `j1_j2_diagnosis`,
  `j2_shell_strip_diagnosis`), `sa_tie_default_bits.rs` (the style of B1).

## Closed forms and arithmetic (first principles)

1. **Why unit and lumped weights fail.** The work-equivalent of a plane section is the traction field of a rigid motion
   `t(x) = a + b x r(x)` (uniform force plus, for `b` in the face plane, a normal stress linear across the face, and for `b`
   along the normal a torsional shear). Its consistent nodal forces are `f_i = int N_i t dA`. On a Quad8 face
   (`[-1,1]^2`, scratch `quad8_areas.py`, a 5 x 5 Gauss rule in plain Python) the nodal areas `w_i = int N_i dA` are `-1/12`
   of the face area at the corners and `1/3` at the mid-edge nodes (so a uniform force already puts negative forces on the
   corners), and for the linear traction `t = u` the consistent forces `int N_i u dA` are `-/+1/9` at the corners and `+/-4/9`
   at the mid-edge nodes. The lumped form `w_i u_i` gives `+/-1/3` and `+/-4/3`: the same moment (`4/3` both) but corner forces
   of the wrong sign. Unit weights (ADR 0456) put `1/8` of the force on every node. Both load the face by concentrated
   self-equilibrated forces against the consistent ones, and the St-Venant zone behind it adds flexibility (ADR 0456 D3:
   `+2.25e-2`). So "weighted by the consistent nodal areas of a linear traction" is registered as the face-integrated form
   below, not as nodal weights `w_i` at the node positions (which would be wrong in sign at the Quad8 corners).
2. **Face-integrated distributing coupling.** The reference motion is `y` (six unknowns per reference node) with the face field
   it describes `v(x) = C(x) y`, `C` (3 x 6n) linear in `y`. The coupling is the L2 projection of the solid face field
   `u_h(x) = sum_i N_i(x) u_i` onto that space: `min_y int_S |u_h - C y|^2 dA`, so `A y = sum_i G_i u_i` with
   `A = int C^T C dA` and `G_i = int N_i C^T dA`. Duality: a generalised force `Q` on `y` puts on the solid nodes
   `f_i = G_i^T A^+ Q = int N_i C(x) z dA` with `z = A^+ Q`: exactly the consistent nodal forces of the traction field
   `t = C z`, which lies in the space of the reference motions (a linear traction for a beam node). This is clause (a1).
   - Beam node `b`: `C(x) = B(x - x_b)`, `B(r) = [I, -[r]x]` (the rigid motion `t + theta x r`). For `b` at the face centroid
     the off-diagonal block `int -[r]x dA` vanishes, so a force `F` alone gives `z = (F/S, 0)` and `f_i = w_i F/S`: the
     consistent nodal forces of a uniform traction (W1); a moment `M` about an in-plane axis gives the linear normal traction
     `M d/I` with `I` the face second moment (W2).
   - Shell edge (clause a3): the edge is the polyline of the shell element edges whose two nodes are in the node set. A face
     point `x` has its foot `p` on the nearest segment `(k, l)`, `p = (1 - lam) x_k + lam x_l`, and the shell kinematics give
     `v(x) = sum_{j = k, l} L_j (u_j + theta_j x (x - p))`, `L_k = 1 - lam`, `L_l = lam`: the edge interpolation of the
     translations and rotations, with the fibre offset `x - p`. `C(x) = [L_k B(x - p), L_l B(x - p)]` in the columns of `k` and
     `l`. Every face node is coupled through `G_i` (the mid-edge nodes at `y = 0.025, 0.075` of J2 included; ADR 0456 left them
     free). A uniform line force `q` on the edge, `Q_j = int L_j q ds`, gives the uniform traction `q/h` (W3); a uniform line
     moment about the edge direction gives the linear normal stress `m z / (h^3/12)` (W4) (`h` the face height, the solid plate
     thickness).
   - Null space: for a shell edge in the face plane with the face normal to the shell, `x - p` is along the shell normal `n`,
     so `theta_j . n` (the drilling rotation of each edge node) does not enter `C`: `A` has an `n`-dimensional null space. As in
     ADR 0456 D3 the null space is found from `rcfd_linalg::eigen::symmetric_eigen` (relative threshold
     `COUPLING_NULL_TOLERANCE = 1e-10`), the free components chosen by complete pivoting on the null basis, and the rows written
     in solved form `y_D = H_D b + K y_F`, `K = N_D N_F^-1`, `H = A^+_D - K A^+_F`. A null space larger than the number of
     reference nodes is refused (a degenerate face).
   - Exact for rigid motions (a8): `u_i = t + theta x (x_i - x_0)` for the solid nodes and `(t + theta x (x_j - x_0), theta)`
     for the reference nodes give `u_h(x) = t + theta x (x - x_0)` (an isoparametric face reproduces linear fields) and
     `C(x) y = t + theta x (x - x_0)` (beam: identically; shell: `sum L_j (t + theta x (x_j - x_0)) + theta x (x - p) =
     t + theta x (p - x_0) + theta x (x - p)` since `sum L_j x_j = p`); so the residual is zero pointwise, `y` minimises, and it
     satisfies the solved rows for any quadrature rule with positive weights for which `A` has the stated null space.
3. **Rigid-face option** (a4). Every node `i` of the face follows the reference motion at its own position:
   `u_i = sum_j L_j(x_i) (u_j + theta_j x (x_i - p_i))` (beam: `u_b + theta_b x (x_i - x_b)`, the rows of `rigid_rows` of the
   ADR 0456 diagnosis), the face node components dependent. Exact for rigid motions by the same identity. For `nu > 0` it holds
   the face's Poisson contraction (over-stiff); at `nu = 0` it is the plane-section kinematics of the beam and shell.
4. **Quadrature.** Per face: quadrilaterals the 4 x 4 Gauss rule on `[-1, 1]^2`, triangles the collapsed 4 x 4 rule of the
   mortar tie (`(u, v) = (s (1 - r), s r)`, weight `w_s w_r s`), each times `|x_u x x_v|`. On affine faces with the kinks of
   `L_j` on face edges (J2: the edge nodes at `y = 0, 0.05, 0.1` are face corners) the integrands are polynomials of degree
   at most 5 per direction (Quad8 `N_i` degree 2, `C` degree 1, `C^T C` degree 2), integrated exactly. A kink inside a face
   is integrated approximately (limits).

## Estimates on the registered meshes (from ADR 0456's measured diagnosis, before this run)

- **J1** (Hex20 `[10,2,2]` over `0.5`, 15 Beam2 over `1.5`, `nu = 0`): the parts are right, D1 (one-piece Hex20 at the same
  element length `0.05`) `-2.0e-5`; with a rigid face D2 `-9.1e-6` (the beam exact for a tip load, ADR 0456). With the
  consistent coupling the moment enters as the linear normal traction of beam theory; at `nu = 0` the solid's exact pure-bending
  field is quadratic in displacement (Hex20 reproduces it), so the bending part has no end zone. The shear enters as a uniform
  shear traction instead of the parabolic one; the shear deflection `P L/(kappa G A) = 2.4e-6 m` is `1.5e-3` of `delta`
  (`1.60e-3 m`), and an end effect is a fraction of it. Estimate `|e| <= 1e-3`; the limit `1e-2` has a factor 10.
- **J1R** (rigid face): D2 is the same rows as the rigid option at this node (arithmetic 3), `-9.1e-6`; estimate `1e-5`, limit
  `1e-3` (a7).
- **J2** (Hex20 plate `[4,2,1]` over `0.2`, Shell4 `16 x 2` over `0.8`): the plate at element length `0.05` D5 `-2.8e-4`, the
  shell strip at element length `0.05` D6 `-6.2e-4`; the shell part carries `0.8^3 / 1 = 51` percent of the bending compliance,
  the plate 49 percent, so the parts give about `-4.5e-4`. The joint with every face node coupled and the bending moment as a
  linear normal traction through the thickness (uniform in `y` at `nu = 0`) adds an end effect from the uniform shear only.
  Estimate `|e| <= 1.5e-3`; limit `2e-2` (factor 13).
- **J2R** (the shell rigid option): the same parts, the face held to the plate's plane section (exact at `nu = 0` apart from
  the shear warping it suppresses): estimate `|e| <= 1e-3`; limit `2e-3` (factor 2, fixed now from the estimate).
- **R1, W1 to W4** are identities (arithmetic 2, 3): rounding, `1e-15` relative.

## Design

### D1 `tie::JointCoupling` and two new functions (appended to `tie.rs`; nothing above changes)

- `pub enum JointCoupling { Consistent, Rigid }` (`#[non_exhaustive]`): the face-integrated distributing coupling of
  arithmetic 2, and the rigid face of arithmetic 3.
- `tie::beam_to_solid_with(model, beam_node, surface, coupling) -> Result<Vec<Constraint>>`: the beam node to the faces of the
  surface set (not only its nodes).
- `tie::shell_to_solid_with(model, shell_nodes, surface, coupling) -> Result<Vec<Constraint>>`: the shell edge (the segments of
  the Shell4/Shell3 element edges with both nodes in `shell_nodes`) to the faces of the surface set. Refused naming the set: no
  segment, a node of the set on no segment; refused naming the node: a reference node without six unknowns (as `coupling`).
- Rows as `Constraint::Linear` with zero terms (`|c| <= 1e-14`) omitted, as ADR 0456 D3; the consistent rows are dense over the
  face and the edge (`A^+` is full).
- `tie::coupling`, `tie::beam_to_solid`, `tie::shell_to_solid` (unit weights, the column search) are not changed: their bits
  are B1, and the known-miss records of ADR 0456 keep measuring them.

### D2 Not changed

Every existing line of `rcfd-fem` (the new code is appended to `tie.rs` and adds one item to its module documentation), the
existing tests (`sa_tie.rs` and its known-miss and diagnosis records untouched), every default path.

### D3 Tests

`crates/rcfd-fem/tests/sa_joint.rs` (the checks below; copies of `j1_model` and `j2_model` of `sa_tie.rs` with the new joint
calls), `sa_joint_default_bits.rs` (B1).

## Checks

Common: `E = 2e11`, `nu = 0`, `P = 1000`, the closed form `delta_T = P L^3 / (3 E I) + P L / (kappa G A)`, `kappa = 5/6`, as
ADR 0456; `delta` the work-conjugate tip deflection; `rel(a, b) = abs(a/b - 1)`.

| ID | Clause | Set-up and measure | Limit | Estimate |
|---|---|---|---|---|
| W1 | a1 | J1 face `A.xmax`, `Consistent`: solid-node forces dual to a reference force `Q = (0, 0, -P)` (`f_ic = sum_d Q_d c_d,ic` from the rows) against `hex20::traction_field_loads` of the uniform traction `Q/S`; max abs difference over max abs | `<= 1e-12` | 1e-15 |
| W2 | a1 | Same, a reference moment `M` about `y`: against the loads of `t = (M (z - z_b)/I, 0, 0)`, `I = 0.1^4/12` | `<= 1e-12` | 1e-15 |
| W3 | a2, a3 | J2 face, `Consistent`: forces dual to `Q_j = (0, 0, -P) int L_j ds / 0.1` on the three edge nodes against the loads of the uniform traction `(0, 0, -P/(0.1 x 0.02))`; also every one of the 21 face nodes gets a non-zero force | `<= 1e-12`; 21 of 21 | 1e-15 |
| W4 | a2, a3 | Same, a line moment `m` about `y`: `Q_j = (0,0,0, 0, m, 0) int L_j ds` against the loads of `t = (m z / (0.02^3/12), 0, 0)` | `<= 1e-12` | 1e-15 |
| R1 | a8 | The four couplings (J1 and J2, `Consistent` and `Rigid`), the six rigid motions (unit translations; unit rotations about `x_0 = (0.3, -0.2, 0.1)` scaled by `0.1`): max over rows of the residual over max abs nodal value | `<= 1e-12` | 1e-15 |
| J1 | a5 | J1 of ADR 0456 with `beam_to_solid_with(.., Consistent)`: `rel(delta, delta_T)`, `L = 2` | `<= 1e-2` | `<= 1e-3` |
| J1R | a4, a7 | J1 with `Rigid` | `<= 1e-3` | 1e-5 |
| J2 | a6 | J2 of ADR 0456 with `shell_to_solid_with(.., Consistent)`: `rel(delta, delta_T)`, `L = 1` | `<= 2e-2` | `<= 1.5e-3` |
| J2R | a4 | J2 with `Rigid` | `<= 2e-3` | `<= 1e-3` |
| K1 | a9 | `sa_tie.rs` and ADR 0456 unchanged from 2504fd62 (`git diff` empty); the two known-miss tests run with `--ignored` print the recorded `+2.25e-2` and `+2.36e-2` (four digits) | `==` | `==` |
| B1 | rule 3 | `sa_joint_default_bits.rs` against `BASE` (the rows of `coupling`, `beam_to_solid`, `shell_to_solid`, the J1 and J2 solves with them) after the library change; `sa_tie_default_bits` passes | `==` | `==` |

### Negative controls

| ID | Set-up | Expected |
|---|---|---|
| N1 | `shell_to_solid_with` with a node set of one shell node (no segment) | refused naming the set |
| N2 | `beam_to_solid_with` with a solid node (three unknowns) as the beam node | refused naming the node |
| N3 | J1 with `Consistent` on the face set but the beam node moved off the face by `0.05` along `x` (a lever) | accepted, R1 still met (rigid motions exact for any reference position) |

### Planned mutations (scratch copy `w49i-work/f543-scratch/mut`, test `sa_joint` per mutation)

| ID | Mutation | Expected to catch |
|---|---|---|
| Mu1 | consistent coupling: `G_i` replaced by `B(x_i - p)^T` (unit nodal weights, the ADR 0456 rule) | W1, W2, J1 |
| Mu2 | consistent coupling: `G_i` replaced by `w_i B(x_i - p)^T` (lumped nodal areas, arithmetic 1) | W2 |
| Mu3 | shell edge: `L_j` replaced by the nearer node (weight 1) | R1 |
| Mu4 | fibre offset `x - p` replaced by `p - x` in `C` | R1, W2 |
| Mu5 | rigid option: rotation terms dropped | R1, J1R |
| Mu6 | quadrature: one point (the face centre) per face | W1 |
| Mu7 | shell edge: the face nodes off the edge columns (`|y - y_j| > 1e-9` for every edge node) dropped from the coupling | W3 |

## Decision rule

**done**: every check of the table met, every negative control as expected, every planned mutation caught, B1 and K1 green, no
amendment after a run. **partial**: R1 and W1 to W4 met and any other check missed, or a mutation not caught, or a check met
only after an amendment made after its run. **todo**: R1 or one of W1 to W4 missed.

## Limits (what is not covered)

- `nu > 0`: the consistent coupling lets the face contract; the rigid option holds it (over-stiff); no check at `nu > 0`.
- Curved faces and kinks of the edge interpolation inside a face: the quadrature is then not exact (rigid motions stay exact).
- The consistent rows are dense over the face and the edge (`A` is `6n x 6n` for `n` edge nodes, `A^+` full); a long shell
  edge gives large rows and a dense eigen-decomposition of `A`. No limit on fill is registered.
- The shell drilling rotation stays free at the joint (held by the shell drilling penalty), as ADR 0456.
- No joint stiffness or offset beyond the face geometry; a reference node off the face acts as a rigid lever (N3).

## Amendment 1 (2026-11-14, before any run of the checks)

W3's count "21 of 21" is an arithmetic slip of the registration: the `xmax` face of the J2 plate (Hex20 `[4,2,1]`, two Quad8
faces) has 13 nodes (6 corners, 4 mid-edge nodes along `y` at `z = +/-0.01`, 3 mid-edge nodes along `z` at `y = 0, 0.05,
0.1`; ADR 0456: three columns of three nodes and the four mid-edge nodes). Corrected before any run: W3 requires a non-zero
force on every node of the face, 13 of 13. No limit changes.

## Addendum 1: results (2026-11-14)

Registered run: `sa_joint` and `sa_joint_default_bits` of commit c30fb0cf (release, one thread; the branch commits were later rebuilt with identical trees, that commit is now 61914a96), log `f543-scratch/run1.log`,
made once. Amendment 1 was written and committed (b976cc52, now 8618aee9) before that run and changed no limit.

| ID | Limit | Measured | Met |
|---|---|---|---|
| W1 beam, uniform force against `traction_field_loads` | `<= 1e-12` | 5.1e-16 | yes |
| W2 beam, moment about `y` against the linear traction | `<= 1e-12` | 4.5e-16 | yes |
| W3 shell, uniform line force; every face node loaded | `<= 1e-12`; 13 of 13 | 6.8e-16; 13 of 13 | yes |
| W4 shell, line moment about `y` against the linear traction | `<= 1e-12` | 2.6e-15 | yes |
| R1 six rigid motions: J1 consistent / rigid; J2 consistent / rigid | `<= 1e-12` | 4.2e-15 / 3.5e-17; 1.6e-13 / 1.3e-17 | yes |
| J1 beam continuing a solid bar, consistent | `<= 1e-2` | 5.44e-6 (signed -5.44e-6) | yes |
| J1R the same, rigid face (`nu = 0`) | `<= 1e-3` | 9.09e-6 (signed -9.09e-6) | yes |
| J2 shell continuing a solid plate, consistent | `<= 2e-2` | 5.56e-4 (signed -5.56e-4) | yes |
| J2R the same, rigid face | `<= 2e-3` | 5.56e-4 (signed -5.56e-4) | yes |
| N1 one-node shell set | refused naming the set | "shell edge 'B.one': no shell element edge with both nodes in the set" | yes |
| N2 solid node as beam node | refused naming the node | "coupling node 0 has 3 unknowns ..." | yes |
| N3 beam node `0.05` off the face (lever), R1 consistent / rigid | accepted, `<= 1e-12` | 1.9e-15 / 3.5e-17 | yes |
| K1 `sa_tie.rs`, `sa_tie_default_bits.rs`, ADR 0456 unchanged; known-miss tests re-run | `==` | `git diff 2504fd62` empty; J1 +2.2527e-2, J2 +2.3645e-2 (the recorded +2.25e-2, +2.36e-2) | yes |
| B1 default joint words | `==` | the 6 words equal to `BASE`; `sa_tie_default_bits` passes | yes |

The estimates held: J1 `-5.4e-6` (estimate `<= 1e-3`; it lies between D1 `-2.0e-5` and the rigid face), J1R reproduces ADR
0456 D2 (`-9.1e-6`), J2 `-5.6e-4` (estimate about `-4.5e-4` from the parts). J2 and J2R agree to `4e-11` relative: with one
Hex20 element through the plate thickness at `nu = 0` the plate's end section stays a plane section under the consistent
traction, so both rows give the same solution (noted, not a check). The R1 residual of the J2 consistent rows (`1.6e-13`) is
above the other couplings' because of the eigenvector noise of the threefold drilling null space; it is within the limit.

### Mutations (scratch copy `f543-scratch/mut` of commit c30fb0cf (61914a96), `sa_joint` per mutation, logs `f543-scratch/mut_Mu*.log`)

| ID | Mutation | Caught by |
|---|---|---|
| Mu1 | unit nodal weights at the node positions (A and G; the ADR 0456 rule) | J1 (2.25e-2, the ADR 0456 number), W1 (0.79), W2 (0.83), W3 (0.67), W4 (0.36) |
| Mu2 | lumped nodal areas `w_i` at the node positions | W2 (0.33); the shell couplings refused (7 undetermined motions), so W3, W4, R1, J2 stop |
| Mu3 | shell edge: nearer node instead of `L_j` | R1 J2 consistent (4.0e-2) and rigid (8.3e-2), J2R (4.5e-3) |
| Mu4 | fibre offset `p - x` | R1 (2.0), W2, W4 (2.0), J1 (0.28), J2 (0.19) |
| Mu5 | rigid option without rotation terms | R1 rigid (0.25, 0.09), N3, the J1R solve singular |
| Mu6 | one quadrature point per face | W1, W2 (1.0), J1 (2.6e-2); the shell couplings refused (12 undetermined motions) |
| Mu7 | face nodes off the edge columns dropped | W3 (0.5; 9 of 13 nodes loaded), W1, W2, R1, J1, J2 (0.43) |

All seven caught.

### Deviations and what was not run

- Amendment 1 (the W3 node count 21 to 13) was made before any run; no limit changed.
- The rest of `sa_tie` (its non-ignored checks) was not run: no existing line of `tie.rs` changed except one module
  documentation line; the new code is appended. `baseline_hashes` was not run (no rcfd-fv or rcfd-api source touched, and no
  default path changed: B1).
- No check at `nu > 0`, on curved faces or with edge kinks inside a face (limits of the registration).
- `cargo fmt -p rcfd-fem --check` and `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean.

### Decision by the registered rule

**done.** Every check of the table met, the negative controls as expected, all seven planned mutations caught, B1 and K1 green,
and no amendment after a run.

### Remaining questions

1. `nu > 0`. Options: (a) a check of J1 and J1R at `nu = 0.3` against a fine one-piece solid (the rigid face is expected to be
   over-stiff); (b) leave it documented. Recommendation: (a) as a small follow-up only if the joints are used with `nu > 0`
   material in a validation case.
2. Fill of the consistent rows (dense over the face and the edge, a dense `6n x 6n` eigen-decomposition). Recommendation: keep
   it until a long shell edge needs it, then split the edge into per-node patches of the edge interpolation.

## For the lead: roadmap entries

Proposed status of F-543: **done**.

> Structural solver, advanced tier. Follow-up of F-426 (ADR 0456, 2026-11-14): the J1/J2 misses. ADR 0460, 2026-11-14: tie::beam_to_solid_with and tie::shell_to_solid_with with JointCoupling::Consistent (the L2 projection of the solid face field onto the reference motions, so the face receives the consistent nodal forces of a linear traction; a shell edge coupled to every face node through its linear edge interpolation with the fibre offset, drilling rotations left free by complete pivoting on the null space) and JointCoupling::Rigid; the unit-weight couplings of ADR 0456 unchanged (bits B1) with their known-miss records kept. Met: beam continuing a solid bar 5.4e-6 (limit 1 percent), shell continuing a solid plate 5.6e-4 (limit 2 percent), rigid face 9.1e-6 at nu = 0 (limit 0.1 percent), shell rigid face 5.6e-4; the six rigid motions exact to 1.6e-13; the dual nodal forces equal hex20::traction_field_loads of the uniform and linear tractions to 2.6e-15 on both faces, every face node loaded. Seven mutations caught. Not covered: nu > 0, curved faces, dense rows over long edges.

Proposed note addition for F-426 (append to its note):

> F-543 (ADR 0460, 2026-11-14): the consistent and rigid joint couplings meet J1 (5.4e-6) and J2 (5.6e-4); the unit-weight couplings and their known-miss records are kept unchanged.
