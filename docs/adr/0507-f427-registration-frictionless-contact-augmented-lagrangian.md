# ADR 0507: F-427 registration: frictionless node-to-surface contact with an augmented Lagrangian, rigid and deformable targets, interference fits

Status: **REGISTERED 2026-11-14, written before any code of F-427 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-427 was written or run in the repository except one disclosed probe (below: the size and
the time of the sparse factorisation of the Hertz mesh's stiffness, existing API only, an uncommitted example file, no contact
code). The arithmetic below was done by hand and with a few lines of numpy in the scratch folder (`w49i-work/f427-scratch`,
`cycle_search.py`; no rcfd code). The bit record B0 of the default paths is a test file written and run on the base after this
commit and before any library code (D9). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-427 (deps F-426, F-397), the plan of [ADR 0330](0330-frictionless-contact.md) (SA-20 of
[ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Number 0507 given by the lead. Branch `f427-work`, worktree
`w49i-work/f427`, scratch `w49i-work/f427-scratch`. Base 031b09c6.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case
and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph unchanged; no existing test edited
except by the records-and-copies scheme of ADR 0291 (D10); clean-room.

## What F-427 asks

The acceptance, verbatim from the brief, binding:

> Every limit not stated here is registered in the ADR before the run. A bar pushed against a rigid stop across a gap g: the piecewise-linear force-displacement closed form within 1e-8, the penetration after augmentation below 1e-8 L. Two stacked blocks with matching meshes under pressure p: uniform stress and contact pressure p within 1e-8; pulled apart they separate with no tensile contact force (every multiplier >= 0). Elastic sphere on a rigid plane: contact radius (3 F R/(4 E*))^(1/3), peak pressure 3 F/(2 pi a^2) and approach a^2/R within 5, 5 and 3 percent at a/R = 0.1 with at least ten nodes across the contact radius. Interference fit of a ring (outer radius b) on a solid shaft (radius a) of the same material with radial interference d, plane stress: contact pressure E d (b^2 - a^2)/(2 a b^2) within 2 percent, and the hoop stress of the ring that of Lame under that pressure within 2 percent. The contact tangent agrees with central differences within 1e-6 in a fixed active set; the last residuals of a step with a settled active set show an order >= 1.8; an active set that cycles beyond the registered count is an error with the nodes.

Clauses: (a1) the bar against a rigid stop; (a2) the stacked blocks, pressed and pulled apart; (a3) Hertz; (a4) the
interference fit; (a5) the tangent against central differences; (a6) the order of the last residuals; (a7) cycling is a located
error. The brief adds: (b1) the Hertz check is costly: symmetry and a graded mesh, probe memory and time first, an ignored heavy
record if it must be heavy, run once alone; check the Hertz formulas by hand; (b2) check the fit pressure by hand and register how
the interference is imposed; (b3) no case-file key (a proposal in the open questions); (b4) other agents edit `builders.rs`,
`nonlinear.rs`, `nl_transient.rs`, `hex8x.rs` and the beam and shell geometric stiffness: contact in its own module, `nonlinear.rs`
edits minimal and named; (b5) defaults keep their bits, `baseline_hashes` unchanged, a default-bits test at the base before any
library change; (b6) every aggregated error finite; (b7) the fan target: blade-hub and hub-shaft press fits of a 120 mm fan.

Readings, registered:

- **"The piecewise-linear force-displacement closed form"** (a1). A bar of length `L`, section `A`, held at `x = 0`, an axial end
  force `F` toward a rigid stop at the gap `g` beyond its free end. Small displacements: the end displacement is
  `u(F) = min(F L/(E A), g)` and the stop's force `R(F) = max(0, F - E A g/L)`; both are checked at every load step, `u` against
  `g` and `R` against the largest force (D7 P1).
- **"The penetration after augmentation below 1e-8 L"** (a1): the largest `max(0, -g_s)` over the contactor nodes after the
  driver has accepted the step (D3), at every step with contact.
- **"Uniform stress and contact pressure p within 1e-8"** (a2): every component of the stress at every integration point of
  both blocks against `(0, 0, -p, 0, 0, 0)`, over `p`; every contactor node's pressure (D4) against `p`.
- **"Pulled apart they separate with no tensile contact force (every multiplier >= 0)"** (a2). A floating block cannot be pulled
  by a force (no equilibrium), so the separation is driven by a displacement of the upper block's top face, from a pressed state
  (an initial overlap) through touching to apart (D7 S1): at every step every multiplier is finite and `>= 0`; once the closed
  form says apart, every multiplier is exactly 0 and every gap is the closed-form opening.
- **"Elastic sphere on a rigid plane"** (a3). Hertz's theory (Johnson, *Contact Mechanics*, Cambridge 1985, chapters 3 and 4;
  from memory, re-derived where used) is the half-space idealisation: an elastic half-space whose surface lies at the sphere's
  profile `s(r)` from the plane, `r` small against `R`, linear elasticity. The registered model is that idealisation (D7 H): a
  quarter of a block of the half-space (two symmetry planes), flat surface, the gap of each surface node offset by the sphere's
  profile `s(r) = R - sqrt(R^2 - r^2)` (the initial-gap offset of D2), linear kinematics, and on the far faces the displacement of
  Boussinesq's point force plus the rigid approach `delta` (the half-space's displacement at a distance is that of a point force
  `F` up to `O((a/rho)^2)`). The approach of the theory is the approach of distant points, which a finite sphere does not have:
  its centre moves less than `delta` by about `0.73 (a/R) delta` (arithmetic 3), outside the 3 percent. The measured force
  `F` is `4` times the sum of the contactor nodes' forces (quarter); each of the three closed forms is evaluated at that `F`:
  `a_H = (3 F R/(4 E*))^(1/3)`, `p_H = 3 F/(2 pi a_H^2)`, `delta_H = a_H^2/R`. The measured contact radius is the fit of D7 H1,
  the measured peak pressure the pressure of the node at the centre, the measured approach the prescribed `delta`.
- **"At least ten nodes across the contact radius"** (a3): the contactor nodes on the symmetry line `y = 0` with a positive
  force: at least ten.
- **"Plane stress"** (a4): one layer of Hex8 through the thickness `t`, `u_z = 0` on the bottom face, the top face free (`sigma_zz
  = 0` on both faces and, for fields linear in `z`, inside).
- **"Radial interference d"** (a4), how it is imposed (b2): the two meshes overlap. The shaft is a polygon of `N` sides whose side
  mid-points lie at the radius `a`; the ring's inner nodes lie at `a - d`, at the angles of those mid-points (half a side from the
  shaft's vertices), so that each ring node's initial gap is `-d` exactly in the reference configuration (D7 I).
- **"The hoop stress of the ring that of Lame under that pressure"** (a4): at the centre of every ring element (the mean of its
  eight integration-point stresses, rotated to the polar axes of the centre of its nodes) against
  `sigma_theta(r) = p a^2 (1 + b^2/r^2)/(b^2 - a^2)` at the centre's radius, with `p` the measured mean contact pressure.
- **"The contact tangent agrees with central differences within 1e-6 in a fixed active set"** (a5): the matrix of the contact
  contribution (D5) against the central differences of its force, `max |K - K_fd| / max |K|`, on a warped deformable target with
  penetrating and non-penetrating active nodes, and on a rigid plane; the active set is the same at every perturbed state (that
  is checked, not assumed).
- **"The last residuals of a step with a settled active set show an order >= 1.8"** (a6): the measure of ADR 0404 (a4): the
  relative residuals of a converged Newton solve below `1e-12` are dropped (rounding carries no order); of the rest the last three
  `r_a, r_b, r_c` give `p = ln(r_c/r_b)/ln(r_b/r_a)`. A solve qualifies when at least three remain and the active set is the same
  at the three evaluations they come from. The model is nonlinear (total Lagrangian, F-397) with a bending deformable target.
- **"An active set that cycles beyond the registered count is an error with the nodes"** (a7): D6.

## What was read (no run)

`crates/rcfd-fem/src/nonlinear.rs` (the driver, `Contribution`, `DriverExtensions`, `solve_from`, `add_triplets`, the residual
reference), `constraint.rs` (`Prescribed`, `prescribed_equations`, `reduce`), `assemble.rs` (`stiffness`), `loads.rs`
(`nodal_load_vector`), `mesh.rs` (`HEX_FACES`: corners counter-clockwise seen from outside; `FaceRef`, `face_nodes`), `tie.rs`
(`assemble_parts`, the bucket search and the projection of ADR 0456), `recover.rs` (`IntegrationPointField`), `builders.rs`
(`hex_box` sets), `rcfd-linalg/src/sparse_chol.rs` (`SparseLdlt`, minimum degree), `contact.rs` (the stub of ADR 0440, `PENDING`),
`tests/sa_scaffold.rs` (the X1 rows), `tests/sa_follower_default_bits.rs` (the digest pattern); ADR 0330, ADR 0310 (sections 2
and 7), ADR 0404 (the order measure), ADR 0456 (the search and the records), ADR 0502 (the form of a registration).

## D0. What exists

`contact.rs` is a stub with `PENDING = Some("ADR 0330 (F-427)")`. The Newton driver of ADR 0404 has the extension points of ADR
0440 D7: `Contribution` (a force added to the internal force and triplets added to the tangent, at every iteration, `&self`),
`solve_with`, and `solve_from` (one Newton attempt from a start displacement, at full load, the prescribed values carried by the
start). The search of ADR 0456 (`tie.rs`) is private to the tie and projects once; contact needs the projection at every state,
so `contact.rs` has its own projection (D1) and uses none of `tie.rs`'s private items. **`nonlinear.rs` is not edited** (b4): the
total Lagrangian path calls `solve_from` with the contact as a `Contribution`.

## Design

### D1. The gap (first principles; Wriggers, *Computational Contact Mechanics*, Springer 2006, chapter 4, from memory, every formula re-derived here)

A **pair** is a contactor surface (a surface set of Hex8 faces; its corner nodes are the contactor nodes) and a target. The
current position of a node is `x = X + u`.

- **Rigid plane** (point `p`, unit normal `n` toward the side where the contactor may be): `g_s = (x_s - p) . n + g0_s`.
- **Deformable surface** (a surface set of Hex8 faces, four corners counter-clockwise seen from outside): the bilinear face
  `x(xi) = sum_a N_a(xi) x_a`, `N_a = (1 + xi xi_a)(1 + eta eta_a)/4` with the corners at `(-1,-1), (1,-1), (1,1), (-1,1)`; tangents
  `a_alpha = x_{,alpha}`, metric `m = [a_alpha . a_beta]`, outward unit normal `n = a_1 x a_2 / |a_1 x a_2|`, curvature
  `h_{alpha beta} = n . x_{,alpha beta}` (`x_{,xi xi} = x_{,eta eta} = 0`, `x_{,xi eta} = sum_a xi_a eta_a x_a/4`). The closest point
  `xi*` makes `d = x_s - x(xi*)` normal to the face: Newton on `(x_s - x(xi)) . a_alpha = 0` with the matrix
  `A = m - g h` (`d = g n`), from `xi = 0`, at most 30 iterations, until `|dxi| <= 1e-14`. The gap `g_s = d . n + g0_s`.
- `g0_s` is the **initial-gap offset** of the node (default 0): a sphere's profile on a flat surface (D7 H), or an interference
  without overlapping meshes.

**Search** (repeated at every evaluation, ADR 0330 "the search ... repeated as the state moves"): for a contactor node, the
candidate faces are the target faces that do not contain the node, whose current bounding box grown by the search distance
contains `x_s`, whose projection converges with `xi*` in `[-1 - 1e-8, 1 + 1e-8]^2`, and with `|d . n| <=` the search distance
(default: the face's longest current edge; an option). The face of smallest `|d . n|` is taken; ties (equal bits) go to the
lower face index. No candidate: the node is not in contact at that state (inactive).

**Gradient** (`q` = the 15 coordinates `x_s, x_1 .. x_4`). Because `xi*` makes `|d|^2` stationary, `dg = n . (dx_s - sum N_a
dx_a)`: `G = dg/dq` has the blocks `n` (contactor) and `-N_a n` (face nodes). Rigid plane: `G = n` on the contactor only.

**Hessian of the gap** (derived): with the vectors over `q` `T_alpha` (blocks `a_alpha`, `-N_a a_alpha`) and `P_alpha` (blocks
`0`, `N_{a,alpha} n`), the variation of the closest point `A dxi = T dq + g P dq` and of the normal `dn = -m^{alpha beta}
(n . da_beta) a_alpha`, `n . da_alpha = P_alpha dq + h_{alpha gamma} dxi_gamma`, give

    d2g/dq2 = - T^T C T - T^T A^-1 P - P^T A^-1 T - g P^T A^-1 P,   C = m^-1 h A^-1,

symmetric (`m^-1 h A^-1 = (I - g m^-1 h)^-1 m^-1 h m^-1`, a symmetric product). For a flat face (`h = 0`) at `g = 0` it is
`-T^T m^-1 P - P^T m^-1 T`. Rigid plane: 0.

### D2. The augmented Lagrangian (Simo and Laursen, Comput. Struct. 42 (1992) 97; Wriggers 2006 chapter 6; from memory)

Per contactor node a multiplier `lambda_s >= 0` (a force, N), held fixed during a Newton solve, and the node penalty `eps_s =
eps A_s`: `eps` is the pair's penalty per unit area (N/m^3) and `A_s` the node's area (D4). The node's force magnitude is
`t_s = max(0, lambda_s - eps_s g_s)` and its energy `W_s = max(0, lambda_s - eps_s g_s)^2/(2 eps_s)`, so that

    f_c = dW/dq = -t_s G,   K_c = d2W/dq2 = eps_s G G^T - t_s d2g/dq2   when lambda_s - eps_s g_s >= 0 (the node is active),

and `f_c = 0, K_c = 0` otherwise. `>=` (not `>`): a node touching at `g = 0` with `lambda = 0` carries no force but its stiffness
`eps_s G G^T` (a floating body held only by contact, B1, is regular at the first iteration). The force enters the driver as an
internal force (ADR 0440 D7: "a contact force enters as it is").

**Uzawa updates** (the augmentation): after each converged Newton solve `lambda_s <- t_s`; the step is accepted when the largest
penetration `max_s max(0, -g_s)` over the pair's nodes is at most the registered tolerance (an absolute length, an option), else
another Newton solve follows from the converged state with the new multipliers. `max_augmentations` (default 20) Newton solves
after the first; if the tolerance is still not met the driver returns `FemError::Solver { solver: "contact" }` naming the step and
the nodes above the tolerance with their penetrations. `max_augmentations = 0` is the pure penalty (no augmentation, no
tolerance test; a negative control). The multipliers of a step start from those of the previous step.

### D3. The driver

`contact::solve(model, prescribed, forces, pairs, options)` and `contact::solve_with(..., extra: &[&dyn Contribution])`: load
control `lambda = k/n`, `k = 1..n` (`options.steps`), the model's loads and `forces` times `lambda`, the prescribed values times
`lambda`; per step the augmentation loop of D2, each Newton solve from the previous converged displacement with the step's
prescribed values written into it. Two kinematics (`ContactOptions::kinematics`):

- **`Linear`** (the default; the closed forms of (a1) to (a4) are small-displacement forms): the body's force is `K u` with `K`
  of `assemble::stiffness`; the gap is evaluated at `X + u` (D1) as in the nonlinear case. Newton in `contact.rs`, the iteration
  and the criteria of ADR 0404 D5 (relative residual over `max(|lambda f_F|, |f_int + f_c|)`, energy ratio,
  `NewtonOptions::{residual_tolerance, energy_tolerance, max_iterations}`); the reduced tangent factored by
  `rcfd_linalg::sparse_chol::SparseLdlt` (minimum degree); a reduced tangent with the same bits as the last one factored reuses
  that factor (exact: a rigid target gives a contact matrix that depends on the active set only).
- **`TotalLagrangian`** (F-397): each Newton solve is `nonlinear::solve_from` with the contact set as the first contribution and
  `extra` after it. `solve_from` takes the model's own prescribed values at full size; this path therefore refuses a model whose
  own displacement constraints are not all zero (they cannot be scaled by `lambda`), naming the first one; non-zero values go
  through `prescribed`. The committed element states advance with the accepted steps.

The result: the `SolidResult` at `lambda = 1` (displacement; reactions `f_int + f_c + f_extra - f` at the prescribed equations),
per step the Newton solves (residuals, energies, the active set at every evaluation, the largest penetration), and per contactor
node its gap, force `t_s`, pressure, area, face, local coordinates, normal and status at the end.

### D4. Area and pressure

`A_s = sum over the contactor faces of int N_s dA` (2 x 2 Gauss on the reference face): the consistent nodal share of a unit
pressure (`A/4` per corner of a parallelogram). The pressure of a node is `t_s/A_s`.

### D5. The contribution

`ContactSet` implements `nonlinear::Contribution` (name `"contact"`): `add(u, ...)` runs the search at `X + u`, adds `f_c` and the
triplets of `K_c` in equations (components 0, 1, 2 of each node), and records the active set of the evaluation (D6). Its
multipliers can be read and set, and `evaluate(u)` gives the node states without adding anything (used by the driver and by T1).

### D6. Cycling (a7)

Within one Newton solve the active set of evaluation `k` is `S_k`. A **cycle event** at `k` is `S_k != S_{k-1}` with `S_k` equal to
some `S_j`, `j <= k - 2`. When the number of events of the solve exceeds `max_cycles` (default **3**, the registered count), the
evaluation fails with `FemError::Solver { solver: "contact", reason }`, the reason naming the step, the Newton iteration `k`, and
the contactor nodes whose status differs between `S_k` and `S_{k-1}` (ascending ids). The driver returns that error as it is
(also when the evaluation came through `solve_from`, whose own wrapping is replaced by the recorded error).

### D7. The models of the checks

Units SI unless stated. Penalty, unless stated, `eps = 100 E/h` per area with `h` the contactor body's element size normal to the
face (a contraction of the penetration of about `1/100` per augmentation). `NewtonOptions::new(1).max_iterations(30)`.

- **P (bar, a1).** `hex_box([0;3], [1, 0.1, 0.1], [4, 1, 1])`, `E = 2e11`, `nu = 0.3`; `u_x = 0` on `xmin`, `u_y = 0` on `ymin`, `u_z
  = 0` on `zmin` (uniaxial stress); the traction `F/A` on `xmax`; rigid plane at `x = L + g`, `n = (-1, 0, 0)`, `g = 1e-2`; contactor
  `xmax`. `F_max = 2 E A g/L = 4e7 N`, 7 steps (`F_k = k F_max/7`: no contact for `k <= 3`, contact for `k >= 4`, never exactly
  touching). `eps = 100 E/0.25 = 8e13`; tolerance `1e-11 L`. Linear.
- **B1 (blocks pressed, a2).** Two `hex_box` blocks of `[2, 2, 2]` cells, `[0,1]^2 x [0,1]` (lower) and `[0,1]^2 x [1,2]` (upper),
  side by side by `tie::assemble_parts` (sets `lower.*`, `upper.*`); `E = 2e11`, `nu = 0.3`; `u_z = 0` on `lower.zmin`, `u_x = 0`
  on both `xmin`, `u_y = 0` on both `ymin`; the pressure `p = 1e8` on `upper.zmax`; contactor `upper.zmin`, target `lower.zmax`
  (matching nodes, `g = 0` exactly at the start); `eps = 4e13`; 2 steps; tolerance `1e-12`. Linear.
- **S1 (blocks pressed then pulled apart, a2).** B1's meshes with the upper block lowered by `d = 1e-3` (overlap `d`), no pressure,
  the upper top face `upper.zmax` given `u_z = 2 d lambda` (through `prescribed`), 8 steps. Closed form (uniaxial, both blocks of
  height 1): for `lambda < 1/2` the stress `sigma_zz = E d (lambda - 1/2)`, pressure `-sigma_zz`; at `lambda = 1/2` touching; for
  `lambda > 1/2` both stress-free, gap `d (2 lambda - 1)`. Tolerance `1e-13`. Linear.
- **H (Hertz, a3).** `E = 1`, `nu = 0.3`, `R = 1`, `a = 0.1`. The quarter block `[0, 5a]^2 x [-5a, 0]`; each direction the
  coordinates `0, h, ..., 15 h` with `h = a/12` (to `1.25 a`), then 9 cells growing by the factor `q` with `h q (q^9 - 1)/(q - 1)
  = 3.75 a` (`q = 1.31539`, by bisection) to `5a`; 24 x 24 x 24 Hex8, 15625 nodes, 40320 free equations. `u_x = 0` on `x = 0`,
  `u_y = 0` on `y = 0`; on the faces `x = 5a`, `y = 5a`, `z = -5a` the remaining components are those of Boussinesq's field for
  the force `F_th` (arithmetic 3) with `u_z` plus `delta = a^2/R` (the body moves toward the plane by `delta`); the rigid plane
  `z = 0`, `n = (0, 0, -1)`; contactor the face `z = 0` with `g0_s = R - sqrt(R^2 - r_s^2)`; `eps = 100 E/h = 12000`; 1 step;
  tolerance `1e-8` (`1e-6 delta`). Linear.
  - The fit of H1: over the contactor nodes with pressure `>= 0.5` of the largest, least squares of `p_s^2 = c_0 + c_1 r_s^2`
    (Hertz: `p^2 = p_0^2 (1 - r^2/a^2)`, exactly linear in `r^2`), `a_fe = sqrt(-c_0/c_1)`.
  - **H fast** (the fast test of the same path, rule 7): `h = a/6`, 8 uniform cells, 6 graded (`q = 1.38797`) to `5a`, 14^3
    Hex8, `eps = 6000`.
- **I (interference fit, a4).** `E = 2e11`, `nu = 0.3`, `a = 1`, `b = 2`, `d = 1e-3`, `t = 0.1`, `N = 64` sides, `Delta = 2 pi/N`.
  Shaft: an O-grid of a disc, a core square `[-0.5, 0.5]^2` of 16 x 16 cells and 6 layers to the polygon whose vertices lie at
  `a/cos(Delta/2)` at the angles `(i + 1/2) Delta`, the core perimeter node `i` (counter-clockwise from `(0.5, 0)`) joined to the
  vertex `i` linearly. Ring: nodes at the angles `j Delta` and the radii `r_k = (a - d) + k (b - a + d)/8`, `k = 0..8` (8 x 64
  cells). One layer `z in [0, t]`. Contactor the ring's inner faces, target the shaft's outer faces: each ring node lies on the
  normal through a side's mid-point with `g = -d`. Constraints: `u_z = 0` at `z = 0` (every node); the shaft's centre node `u_x =
  u_y = 0`; its node `(0.5, 0, *)` `u_y = 0`; the ring's outer nodes at `theta = 0` `u_y = 0`, at `pi/2` and `3 pi/2` `u_x = 0`
  (zero in the symmetric solution; they remove the six in-plane rigid motions of the two bodies, arithmetic 4). `eps = 100
  E/(1/8) = 1.6e14`; 1 step; tolerance `1e-9` (`1e-6 d`). Linear.
- **T1 (tangent, a5).** One Hex8 target, bottom corners `(0,0,0), (1,0,0), (1,1,0), (0,1,0)`, top corners `(0,0,1.05), (1,0,0.96),
  (1,1,1.06), (0,1,0.97)` (a warped face); one Hex8 contactor above, bottom corners `(0.25,0.3,0.98), (0.8,0.2,0.99),
  (0.75,0.7,0.97), (0.2,0.75,1.0)`, top corners the same plus `(0,0,1)`. Pair: contactor the contactor's bottom face, target the
  target's top face, `eps = 10`, multipliers `(0.5, 0.4, 0.6, 0.3)` on the contactor nodes in ascending order. The state `u_i =
  0.01 sin(1.7 i + 0.3)` on every equation `i`. Central differences of `f_c` with the step `1e-6` on every one of the 48
  equations. **T1r**: the same contactor against the rigid plane `z = 1`, `n = (0, 0, -1)`, the offsets `g0 = (0.01, 0, 0.02, 0)`.
- **T2 (order, a6).** A clamped plate `[0,2] x [0,0.5] x [0,0.25]` of `[8, 2, 2]` Hex8 (all components zero on `xmin` and `xmax`)
  under a punch `[0.8, 1.2] x [0, 0.2] x [0.25, 0.45]` of `[3, 1, 1]` (its top `u_x = 0`, a dead pressure `p = 0.01 E` on its top);
  `u_y = 0` on both `ymin` (symmetry); `E = 1`, `nu = 0.3`; contactor the punch's bottom, target the plate's top (non-matching:
  every contactor node projects inside or on the edge of a face); `eps = 100 E/0.125 = 800`; 2 steps; tolerance `1e-8`.
  TotalLagrangian.
- **C1 (cycling, a7).** The bar P with the gap `g`, no traction, and an extra contribution (the test's `Oscillator`) that adds
  the force `+-F_max` (alternating sign at each call, starting by pushing toward the stop) along `x` at the four end nodes, as an
  external load (entering with the minus sign); `max_cycles = 3`. The active sets alternate `{} , {end}, {}, ...`: events at the
  evaluations `k = 2, 3, 4, 5`, the error at `k = 5` naming the four end nodes. Linear (C1) and TotalLagrangian (C1t).
- **C2 (augmentation error).** P at `F_max` in one step with `max_augmentations = 1` and the tolerance `1e-30`: the error names
  the four end nodes.

## Arithmetic done before the code (disclosed)

1. **Bar.** `E A/L = 2e11 x 0.01 = 2e9 N/m`; contact from `F > E A g/L = 2e7 N`, i.e. `k/7 x 4e7 > 2e7`: `k >= 4`. Node penalty
   `eps A_s = 8e13 x 0.0025 = 2e11 N/m`, four nodes `8e11`, 400 times the bar's `2e9`: pure penalty penetration at `F_max`
   `(F_max - 2e7)/8e11 = 2.5e-5 = 2.5e-5 L` (the negative control NP, `> 1e-8 L`); each augmentation divides it by about 401, so
   three augmentations reach `1e-11 L`. With the tolerance `1e-11 L` the displacement error is below `1e-9 g`.
2. **Blocks.** B1: the consistent nodal force of a uniform pressure on a rectangle is `p A/4` per corner, and the contactor
   node projects on a target corner (`N = 1` there): the exact discrete solution is the uniform stress with node forces `p A_s`,
   whatever the penetration. S1 (uniaxial stress `sigma`, both heights 1): the lower top moves by `u_lt = sigma/E`, the
   upper bottom by `u_ub = 2 d lambda - sigma/E`; closing the contact, `(1 - d) + u_ub = 1 + u_lt`, gives `sigma = E (2 d lambda -
   d)/2 = E d (lambda - 1/2)`; a remaining penetration `-g` changes it by `E g/2`: tolerance `1e-13` against `E d/8` at `lambda =
   3/8`: relative `4e-10`.
3. **Hertz** (b1). `E* = E/(1 - nu^2) = 1.098901` (a rigid plane: `1/E* = (1 - nu^2)/E + 0`). Theory at `a = 0.1`:
   `F_th = 4 E* a^3/(3 R) = 1.465201e-3`, `p_0 = 3 F/(2 pi a^2) = 2 E* a/(pi R) = 0.0699577`, `delta = a^2/R = 0.01`; check
   `(9 F^2/(16 R E*^2))^(1/3) = (1.93213e-5/19.3213)^(1/3) = 0.0100` (consistent). Boussinesq (Johnson 1985 section 3.2, from
   memory), depth `zeta = -z >= 0`, `rho = sqrt(r^2 + zeta^2)`: `u_zeta = F (1 + nu)/(2 pi E) (zeta^2/rho^3 + 2 (1 - nu)/rho)`
   (into the body), `u_r = F (1 + nu)/(2 pi E) (r zeta/rho^3 - (1 - 2 nu) r/(rho (rho + zeta)))`; at the surface `u_zeta = (1 -
   nu^2) F/(pi E r)` and `u_r = -(1 - 2 nu)(1 + nu) F/(2 pi E r)` (the known surface forms); check H0 verifies the transcription.
   Far field against `delta`: on the axis at depth `5a`, `u_zeta = F (1 + nu)(3 - 2 nu)/(2 pi E 5 a) = 1.465e-3 x 1.3 x 2.4/(2 pi x
   0.5) = 1.455e-3`, 15 percent of `delta`; the error of the point-force field against the Hertz field at `rho >= 5 a` is of order
   `(a/rho)^2 = 4e-2` of that, about `0.6` percent of `delta`. A finite sphere: its centre at the depth `R` would move
   `u_zeta(R) = 0.073 delta` less (the reading above). The sphere profile against Hertz's paraboloid: `r^4/(8 R^3)` more, `0.25`
   percent of `s(a)`, a change of `a` near `0.1` percent. Strain scale `p_0/E = 0.07`: linear kinematics (the theory's).
   Negative control NH: `delta` with `E` in place of `E*` is `(E*/E)^(2/3) = 1.0649` times larger: 6.5 percent `> 3`.
   Probe (disclosed; an uncommitted example of existing API, `f427-scratch/probe_hertz_f427.rs`): the H mesh's reduced stiffness,
   40320 equations, minimum-degree factor `3.87e7` entries, `SparseLdlt::factor` 40.3 s and 619 MB; the H fast mesh 7840
   equations, 0.56 s, 48 MB. H runs about five factorisations (one per distinct active set): about 4 minutes, so it is an ignored
   heavy record run once alone (b1), H fast the fast test.
4. **Interference fit** (b2), plane stress, same material, `a` the shaft radius, `b` the ring's outer radius. Solid shaft under
   the external pressure `p`: `sigma_r = sigma_theta = -p`, `u(a) = -(1 - nu) p a/E`. Ring under the internal pressure `p`:
   `u(a) = (p a/E) ((b^2 + a^2)/(b^2 - a^2) + nu)`. Interference `d = u_ring(a) - u_shaft(a) = (p a/E)(2 b^2/(b^2 - a^2))`, so
   `p = E d (b^2 - a^2)/(2 a b^2)` (the acceptance's form; `nu` cancels). Here `p = 2e11 x 1e-3 x 3/8 = 7.5e7 Pa`; hoop at the
   bore `p (b^2 + a^2)/(b^2 - a^2) = 1.25e8 Pa`. Negative control NI: a rigid shaft gives `p = E d (b^2 - a^2)/(a ((1 + nu) b^2
   + (1 - nu) a^2)) = E d x 0.50847`, 1.356 times the same-material value (26 percent below it the other way). Discrete exactness:
   the shaft's uniform strain is in every mesh's space and the contact forces on a side are the consistent forces of a uniform
   pressure (each ring node at a side's mid-point, `N = 1/2` to each end), so the shaft is exact and the solution symmetric; the
   six constraints of D7 I are zero in it (a ring rotation `omega` with a translation `(t_x, t_y)`: `u_y(0) = t_y + omega r`,
   `u_x(pi/2) = t_x - omega r`, `u_x(3pi/2) = t_x + omega r`: all zero only for the zero motion). Estimates: the polygon
   (`1 - cos(Delta/2) = 1.2e-3`), the ring's bore at `a - d` (`1e-3`), 8 linear elements through `b - a` (a few `1e-3`), the
   centre stress against the centre value (`0.2 h^2/a^2 = 3e-3` on the inner layer): well inside 2 percent.
5. **Tangent.** `f_c` is smooth in a fixed active set (the face fixed); central differences with `1e-6` on unit-size data:
   truncation `1e-12`, rounding `1e-16/1e-6 = 1e-10`: the error near `1e-10` against `1e-6`. The curvature part `t d2g` against
   `eps G G^T`: `t` near 0.5 against `eps_s = 10 A_s` near 0.75 and `|d2g|` of order 1: dropping it (NT) gives an error of order
   0.3.
6. **Cycling search** (`cycle_search.py`, numpy): semismooth Newton for three-unknown penalty problems with random positive
   definite matrices revisits active sets (e.g. `(0,1,1), (0,1,0), (1,0,0), (1,1,1), (0,1,0), ...`), so the detection is needed; a
   reproducible finite-element cycle is made by the alternating extra contribution of C1 rather than searched for.

## Checks (limits fixed now)

Every aggregated error is the maximum of finite values; a non-finite value is a miss of its check (b6). Relative errors are
`|x/x_ref - 1|` unless stated.

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| P1 | a1 | bar, each of the 7 steps: `|u_end - min(F L/(E A), g)| / g` (the mean `u_x` of the four end nodes) and `|R - max(0, F - E A g/L)| / F_max` (`R` the sum of the contactor forces), and the reaction at `xmin` against `-F` over `F_max` | `<= 1e-8` each | arithmetic 1: `1e-9` |
| P2 | a1 | bar, each step with contact: the largest penetration after the accepted augmentation | `< 1e-8 L` | `<= 1e-11 L` |
| B1 | a2 | blocks pressed: every stress component at every integration point of both blocks against `(0,0,-p,0,0,0)`, over `p`; every contactor node's pressure against `p`; the sum of the reactions on `lower.zmin` against `p x 1` | `<= 1e-8` each | arithmetic 2: rounding |
| S1 | a2 | blocks pressed then apart, each of the 8 steps: every multiplier finite and `>= 0`; for `lambda < 1/2` the stresses against `E d (lambda - 1/2)` (as B1, over `E d/8`) and the node pressures against `E d (1/2 - lambda)` (over `E d/8`); at `lambda = 1/2` every node pressure `<= 1e-8 E d`; for `lambda > 1/2` every multiplier exactly 0, every gap against `d (2 lambda - 1)` within `1e-8 d`, every stress `<= 1e-8 E d` | as stated | arithmetic 2 |
| H0 | (a3) | Boussinesq's field: the Navier equations `G lap u + (lambda + G) grad div u = 0` by central differences (step `1e-4 rho`) at 6 points, the norm of the residual over `G |u|/rho^2`; the surface traction `sigma_zz, sigma_rz` at `z = 0`, `r > 0`, over `F/(r^2)`; the integral of `sigma_zz` over the plane `zeta = 0.3` (radius `300`, 4000 intervals in `ln r`) against `-F` | `<= 1e-5`, `<= 1e-5`, `<= 1e-4` | FD truncation `1e-8` |
| H1 | a3 | `a_fe` (the fit of D7 H) against `a_H(F)` | `<= 5e-2` | discretisation, BC: 1 to 2 percent |
| H2 | a3 | the centre node's pressure against `3 F/(2 pi a_H^2)` | `<= 5e-2` | 1 to 3 percent |
| H3 | a3 | `delta` against `a_H(F)^2/R` | `<= 3e-2` | 1 to 2 percent |
| H4 | a3 | contactor nodes on `y = 0` with a positive force | `>= 10` | 12 |
| H5 | (a3) | the penetration after augmentation | `<= 1e-8` | tolerance |
| HF | (a3, fast) | H fast: H1, H2, H3 with the limits | `0.15`, `0.20`, `0.10` | coarse: a few percent to 10 |
| I1 | a4 | every ring node's pressure against `E d (b^2 - a^2)/(2 a b^2)`, and their mean | `<= 2e-2` each | arithmetic 4: `< 1e-2` |
| I2 | a4 | every ring element's centre hoop stress against Lame's at its radius with the measured mean pressure | `<= 2e-2` | `< 1e-2` |
| I3 | (a4) | the shaft: every integration point `sigma_xx, sigma_yy` against `-p_fe` and `sigma_xy` against 0, over `p_fe` | `<= 1e-6` | exact up to rounding |
| I4 | (a4) | the penetration after augmentation | `<= 1e-9` | tolerance |
| T1 | a5 | `max |K_c - K_fd| / max |K_c|` on the warped pair, the active set (all four) unchanged at every perturbed state | `<= 1e-6` | arithmetic 5: `1e-10` |
| T1r | a5 | the same on the rigid plane | `<= 1e-6` | rounding |
| T2 | a6 | the order of every qualifying Newton solve of the punch (TL), at least one qualifying | `>= 1.8` each | 2 |
| C1 | a7 | the oscillating bar, Linear: the error is `FemError::Solver { solver: "contact" }`, its reason names the Newton iteration 5 and exactly the four end nodes | holds | D6 |
| C1t | a7 | the same through `solve_from` (TotalLagrangian) | holds | D6 |
| C2 | (D2) | the augmentation error names the four end nodes | holds | D2 |
| R1 | (D1) | refused: an unknown contactor or target set (`UnknownSet`), a penalty `<= 0` or not finite, a tolerance `< 0` or not finite (`InvalidValue`), a Tet4 contactor face (`InvalidValue` naming the face's element) | refused | D1 |
| R2 | (D3) | TotalLagrangian with a non-zero model displacement constraint: refused naming it | refused | D3 |
| X1 | (D10) | `contact::PENDING` is `None` (the X1 copy) | holds | D10 |

Negative controls (each must show the check is not vacuous):

| id | control | expected |
|---|---|---|
| NP | P at the step `k = 7` with `max_augmentations = 0` (pure penalty) | penetration `> 1e-8 L` (arithmetic 1: `2.5e-5 L`) |
| NH | H3 with `E` in place of `E*` in `a_H` | `> 3e-2` (arithmetic 3: 6.5 percent, if H3 holds within 1.5 percent; recorded either way) |
| NI | I1's mean pressure against the rigid-shaft form | `> 0.2` away |
| NT | T1 without the curvature part `- t d2g/dq2` | `> 1e-2` |
| NC | C1 with the oscillation's amplitude 0 (a plain step without load) | converges, no error |
| B0 | the bit record of D9 | the base digests, unchanged |
| B1h | `baseline_hashes` of `rcfd-api` | passes unchanged |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | drop the curvature part `- t d2g/dq2` of `K_c` | T1, T2 |
| MU2 | the Uzawa update `lambda <- t` left out (no augmentation) | P2 or C2/P (the augmentation error), S1 |
| MU3 | `t = lambda - eps g` without the `max(0, .)` (a node may pull) | S1 |
| MU4 | the active rule `>` in place of `>=` | B1 (the floating upper block is singular at the first iteration) |
| MU5 | the projection force on the face nodes with `N_a` at the face centre in place of `xi*` | I1 or I3, B1 |
| MU6 | the node area `A_s` from half the face area per node | B1, I1 |
| MU7 | the cycle detection off (no event counted) | C1, C1t |
| MU8 | the term `- g P^T A^-1 P` dropped from `d2g/dq2` | T1 |
| MU9 | the deformable normal reversed (`a_2 x a_1`) | B1, I1 |

## Decision rule

- **done**: every check met, every negative control as expected, every planned mutation caught, B0 and B1h unchanged, the
  existing tests of `rcfd-fem` passing (with the records of D10), fmt and clippy clean.
- **partial**: B0 and B1h unchanged, the module landed with P1, P2, B1, S1, T1 and C1 met, and any other miss (or a check not run)
  diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## D8. Files

`crates/rcfd-fem/src/contact.rs` (the module; `PENDING = None`), `crates/rcfd-fem/tests/sa_contact.rs` (the checks),
`crates/rcfd-fem/tests/sa_contact_default_bits.rs` (B0), `crates/rcfd-fem/tests/sa_scaffold.rs` (D10, attribute and copy only),
`docs/structural-solver.md` (a contact section, if it has a place for it). Not touched: `nonlinear.rs`, `nl_transient.rs`,
`hex8x.rs`, `builders.rs`, `geometric_struct.rs`, `beam.rs`, `shell*.rs`, `tie.rs` (b4).

## D9. The bit record (B0, b5)

`crates/rcfd-fem/tests/sa_contact_default_bits.rs`, written and run on the base 031b09c6 after this commit and before any library
code, its digests printed by that run and written into the file in the same commit, FNV-1a 64 over the bits of: (1)
`solve_static` of a Hex8 bar under an end traction; (2) `nonlinear::solve` of a stretched Hex8 bar (displacement, reactions,
residuals); (3) `nonlinear::solve_from` of that bar with a zero contribution; (4) `solve_static` of two blocks side by side by
`tie::assemble_parts`, each supported, with a pressure on one; (5) `IntegrationPointField::compute` of (4). B0 must give the same
digests after the change.

## D10. Existing tests superseded by design (ADR 0291 records and copies)

`sa_scaffold.rs::x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432_f423_f424_f410_f411_f428` expects
`contact::PENDING == Some("ADR 0330 (F-427)")`; landing the module makes it `None`. It gets `#[ignore = "record: ..."]` (the
attribute only, the body unchanged) and the copy `..._f428_f427` with the contact row `landed`, the standing pattern. If the
crate's run shows another existing test that fails by design, it is handled the same way by an amendment written before its
corrected run.

## Limits (not covered)

- Friction, self-contact, contact in dynamics (the transient integrators), large sliding across many faces (the search is
  repeated but the checks slide little), smoothing of the faceted target (a node at a convex vertex takes the face of smaller
  `|g|`, lower index on ties: the fit avoids the vertices by D7 I).
- Rigid cylinder and sphere targets of ADR 0330's goal: only the rigid plane (a sphere is represented as in Hertz's theory by an
  initial-gap offset, D7 H).
- Faces of Hex8 only (4 corners): Tet4, Tet10, Hex20, shell faces refused.
- The search is a bounding-box filter over all target faces per node (quadratic in the counts; the checks have at most 64 target
  faces); a bucket search is a follow-up.
- The TotalLagrangian path refuses non-zero model displacement constraints (D3).
- Hertz is checked in its half-space idealisation (the reading above), not on a finite sphere's mesh; the approach of a finite
  sphere is not checked.
- No case-file key, no CLI or API surface (b3).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Kinematics of the checks.** Options: (a) everything on the total Lagrangian driver; (b) a linear-body option with the
   geometrically exact gap for the small-displacement closed forms, the total Lagrangian driver for (a5), (a6) and C1t. Taken (b):
   the closed forms of (a1) to (a4) are linear-elastic small-displacement results (Hertz's strain scale is 7 percent, where a St.
   Venant-Kirchhoff body differs by percents).
2. **The sphere of (a3).** Options: (a) a meshed sphere (its approach is not Hertz's, arithmetic 3); (b) the half-space
   idealisation with the sphere's profile as an initial-gap offset and Boussinesq's far field. Taken (b).
3. **Active at `g = 0`.** Options: `>=` or `>`. Taken `>=` (D2).
4. **A case-file key** (b3): not added. Recommendation for later: a `[[contact]]` table with `contactor` (surface set), `target`
   (surface set or `plane = { point, normal }`), `penalty`, `tolerance`, optional `initial_gap` (per node set), read by the static
   nonlinear case, the result writing node pressures and gaps.

## Amendment 1 (2026-11-14): the reaction of P1; written while writing the tests, before any run of a check

P1 registered "the reaction at `xmin` against `-F` over `F_max`". That is wrong once the stop carries load: the equilibrium of
the bar is `F - R + S = 0` with the stop's force `R` and the support's reaction `S`, so the closed form is `S = -(F - R) =
-min(F, E A g/L)` (the reactions of the driver are `f_int + f_c - f` at the prescribed equations, summed over the `x` components
of `xmin`). The corrected P1 quantity is `|S + min(F, E A g/L)| / F_max <= 1e-8`; the other two parts of P1 and every other
check are unchanged. No check had been run.

## Amendment 2 (2026-11-14): three driver defects and one test defect found by the registered run of 8ca0d696; written after that run and before any corrected run

The registered run of `sa_contact.rs` at 8ca0d696 (`f427-scratch/run1-sa_contact.log`) met P1, P2, NP, B1, C1, C1t, NC, C2, H0,
HF, T1, NT, T1r and R1, and stopped four tests with errors before any of their limits was evaluated:

- **I (fit)**: `tangent factorisation at Newton iteration 0: pivot -1.29e10 ... not positive definite`. Diagnosis (arithmetic): at
  the first iteration the multipliers are 0 and every ring node carries the whole penalty force of the overlap, `t = eps A_s d =
  1.6e14 x 4.9e-3 x 1e-3 = 7.9e8 N`, 2100 times its force at equilibrium (`p A_s = 3.7e5 N`). The curvature part `- t d2g/dq2` of
  the exact contact matrix is of order `t/|a|` (`|a|` half a face): `7.9e8/0.05 = 1.6e10`, the order of the elastic stiffness
  `E h = 2e10`, and indefinite (the rotation of a face under a follower-like force). The exact tangent of the penalty energy is
  indefinite there; the solver refuses it.
- **T2 (punch, total Lagrangian)**: `singular tangent: pivot -0.469 at node 57 component 2`. Diagnosis (a spy contribution,
  scratch only): after the first iteration the four punch nodes on the symmetry plane `y = 0` (on the target's boundary edge
  `eta = -1`) project to `eta = -1 - 7.3e-7`, outside the registered slack `1e-8`, so they lose contact; the punch then sinks
  `4.4e-3` into the plate at those nodes and the second iteration carries forces `t = 0.024`, whose curvature part makes the
  tangent indefinite as in I.
- **S1 (blocks pulled apart)**: `load step 4 of 8, augmentation 0: not converged in 30 iterations (relative residual 1.1e-7,
  energy ratio 4.1e-23)`. Diagnosis: at `lambda = 1/2` the blocks just touch with no force, so the reference of the relative
  residual (`max(|lambda f_F|, |f_int + f_c|)`, ADR 0404 D5) is itself of the order of rounding and the relative residual stays
  at `1e-7` although the energy has converged.
- **R2**: the test's extra constraint `u_y = 1e-3` on `xmax` conflicts with the model's `u_y = 0` on `ymin` at node 4; the
  refusal came from `prescribed_equations` (`ConflictingConstraint`), before the check. A defect of the test.

Corrections, registered before their run (no limit changes):

1. **D1, the slack of the inside test**: `1e-8` becomes `1e-2` in face coordinates: a node may take the bilinear face extended by
   one percent of its half-width beyond its edges (the gap, the gradient and the second derivative are those of the extended
   bilinear surface, exact). A node on the boundary edge of a target drifts off it by the face's twist times the gap (here
   `7.3e-7`); one percent covers that with a wide margin while the extension stays within one percent of a face. Inside a
   target surface two faces may then both take a node near their common edge; the smaller `|g|` decides, as before.
2. **D3, the linear body, an indefinite tangent**: when the reduced tangent with the exact contact matrix fails the pivot test
   of `SparseLdlt`, that iteration is taken with the contact matrix without its curvature part, `eps_s G G^T` (positive
   semi-definite, the Gauss-Newton matrix of the penalty energy); the exact matrix is used at every iteration where it factors,
   so the iteration near the solution, where `t` is near its equilibrium value, is Newton's. The number of such iterations is
   reported per Newton solve (`NewtonSolve::convex_iterations`). The total Lagrangian path is unchanged (its factorisation is in
   `nonlinear.rs`, not edited).
3. **D3, the linear body, the reference of the relative residual**: `max(|lambda f_F|, |f_int + f_c + f_extra|, r_run)` with
   `r_run` the largest reference of the converged Newton solves of the run so far (0 at the first): a state without force after
   a loaded one compares its residual with the run's force scale. The first solve of a run is unchanged.
4. **R2, the test**: the non-zero model constraint becomes `u_x = 1e-3` on `xmax` (no other constraint on that component there).

Records and copies (ADR 0291 pattern): the four tests of the registered run stay with `#[ignore = "record: ..."]` and their
bodies unchanged; the copies `i1_to_i4_..._a2`, `s1_..._a2`, `t2_..._a2` (bodies identical, run on the corrected driver) and
`r1_r2_refusals_a2` (R2 corrected) are the corrected runs, each made once. The tests that passed are not re-run to select a
result; they run again only as part of the crate's suite, on the corrected driver, and their numbers are reported from that
suite run in the addendum next to the numbers of the registered run.

## Amendment 3 (2026-11-14): two driver defects and one arithmetic defect found by the corrected runs of 043ee2ea; written after those runs and before any further run

The corrected runs of amendment 2 at 043ee2ea (`f427-scratch/run2-a2.log`) met R1 and R2 (`r1_r2_refusals_a2`; R2 refused
naming `xmax`) and S1's multiplier, apart-gap (`1.10e-13`) and apart-stress (`3.38e-15`) checks, and missed or stopped:

- **S1 pressed and touching**: stress and node pressure `2.50e-3` of `E d/8` (limit `1e-8`), touching pressure `3.13e-4` of
  `E d` (limit `1e-8`). Diagnosis (a scratch test, not committed): the multipliers carried from the loaded steps leave the
  contact nodes active with a positive gap (`6.25e-7` m) at the unloading steps; the acceptance of D2 tests the penetration
  `max(0, -g)` only, so a state with `lambda > 0` and `g > 0` (complementarity violated) is accepted after one augmentation.
- **I (fit)**: `not converged in 30 iterations (relative residual 2.29e-8, energy ratio 1.24e-26)`. Diagnosis (a temporary
  print in the driver, removed): `r` falls `1, 2.5e-2, 1.8e-7` and then stays at `2.0e-8` to `2.5e-8` with the energy at
  `1e-18`, 128 nodes active, one iteration of amendment 2's `eps G G^T`. Two causes, each alone enough: (i) the reference
  `max(|lambda f_F|, |f_int + f_c|)` is that of a self-equilibrated body: `f = 0` and `f_int + f_c` is the small reaction
  (`5.2e4 N`), while the contact forces are `|f_c| = 4.2e6 N`; (ii) the gap is formed from positions `x = X + u` of size 1 m, so
  its rounding is `1.1e-16 m` and the contact force's `eps_s A_s x 1.1e-16 = 7.85e11 x 1.1e-16 = 8.6e-5 N` per node, `1e-3 N`
  over 128 nodes, the measured residual norm. With (i) corrected alone the floor would be `1e-3 / 4.2e6 = 2.4e-10`, above the
  tolerance `1e-10`.
- **T2 (punch, total Lagrangian)**: five qualifying solves, orders `[1.48, 0.191, 0.249, 1.57, 0.171]`, least `0.171` (limit
  `>= 1.8`), missed. Diagnosis: the last kept residual of every solve is at the rounding floor (`5.8e-13` to `2.1e-12`, just
  above or below the cut `1e-12`) and the first kept residual of the first solves is before the asymptotic range (`1.0, 0.65,
  2.97e-4, 1.53e-7`: order `0.98` on the first three even without the floor); the quadratic rate is visible in the ratios
  `r_{k+1} / r_k^2` (`2.97e-4 -> 1.53e-7`: `1.7e-0`; `6.06e-6 -> 1.60e-11`: `0.44`). The registered measure does not separate
  these; no correction is made to it. **T2 is a known miss**: `t2_..._a2` becomes `t2_..._a2_known_miss` (ignored, these
  numbers in its ignore text) and is not run again.

Corrections, registered before their run (no limit changes):

1. **D2, the acceptance of an augmentation**: a solve is accepted when every active node has `|g| <= penetration_tolerance`
   (a penetrating node is always active, so this contains the penetration test). Otherwise the multipliers are updated as before.
   The error after `max_augmentations` names the nodes with their `|g|` ("the gap tolerance ... not met after N augmentations").
   `NewtonSolve::penetration` is unchanged.
2. **D1, the arithmetic of the gap**: the same gap, formed from differences: on a face `g = n . sum_a N_a(xi) ((X_s - X_a) +
   (u_s - u_a))` (as `sum_a N_a = 1`), on a plane `g = n . (X_s - p) + n . u_s`. Its rounding is then of the order of the
   machine epsilon times a face size (here `0.05 m`) instead of times the coordinates; the projection, the normal and the
   derivatives are unchanged. Both bodies.
3. **D3, the linear body, the reference of the relative residual**: also at least `|K u|` and `|f_c|`, both over every
   equation (the pattern of ADR 0479 D2 for a contribution in the reference): `max(|lambda f_F|, |K u + f_c + f_extra|, |K u|,
   |f_c|, r_run)`. The total Lagrangian body is unchanged.

Records and copies: `i1_to_i4_..._a2` and `s1_..._a2` stay as ignored records with their bodies unchanged; the copies
`i1_to_i4_..._a3` and `s1_..._a3` (bodies identical) are the corrected runs, each made once. The other tests run again only in
the crate's suite on the corrected driver; their numbers are reported from that run.

## Addendum 1 (2026-11-14): results, decision

Runs: the registered run at 8ca0d696 (`f427-scratch/run1-sa_contact.log`), the corrected runs of amendment 2 at 043ee2ea
(`run2-a2.log`) and of amendment 3 at bf5fb1e1 (`run3-a3.log`), the heavy H once alone at f7ce92ab (`run4-heavy.log`, 315 s,
three Newton solves of 6, 2 and 2 iterations), the crate's suite at 686f84db (`run5-suite.log`, exit 0) and `sa_contact` with
its prints on the same head (`run6-sa.log`). The numbers below are from the last run of each check; where an earlier run of a
passing check gave other numbers both are given.

| check | limit | measured | met |
|---|---|---|---|
| P1 end displacement / g | `<= 1e-8` | `1.11e-11` (run 1: `1.107e-11`) | yes |
| P1 stop force / F_max | `<= 1e-8` | `5.54e-12` (run 1: `5.91e-12`) | yes |
| P1 support reaction / F_max (amendment 1) | `<= 1e-8` | `5.52e-12` (run 1: `5.53e-12`) | yes |
| P1 contact from step 4 | holds | 4 steps | yes |
| P2 penetration / L | `< 1e-8` | `1.10e-13` | yes |
| B1 stress / p, node pressure / p, reaction / p | `<= 1e-8` | `2.09e-15`, `2.43e-14`, `4.47e-16` (run 1: `1.69e-13`, `3.79e-11`, `5.96e-16`) | yes |
| S1 multipliers >= 0; pressed stress, pressure; touching; apart zero, gap, stress (`_a3`) | `0`, `1e-8` x 5 | `0`, `1.16e-10`, `1.16e-10`, `4.86e-12`, `0`, `1.73e-15`, `2.15e-15` | yes |
| H1 contact radius | `<= 5e-2` | `3.03e-4` | yes |
| H2 peak pressure | `<= 5e-2` | `7.57e-4` | yes |
| H3 approach | `<= 3e-2` | `6.13e-4` | yes |
| H4 nodes on y = 0 in contact | `>= 10` | 13 | yes |
| H5 penetration | `<= 1e-8` | `2.44e-11` | yes |
| H0 Navier, traction, resultant | `1e-5`, `1e-5`, `1e-4` | `2.91e-7`, `2.47e-9`, `4.25e-9` | yes |
| HF radius, peak, approach | `0.15`, `0.20`, `0.10` | `5.59e-3`, `4.88e-4`, `1.19e-3` | yes |
| I1 every node, mean (`_a3`) | `<= 2e-2` | `7.96e-3`, `3.42e-3` | yes |
| I2 hoop stress against Lame (`_a3`) | `<= 2e-2` | `3.62e-3` | yes |
| **I3 shaft uniform stress** (`_a3`) | `<= 1e-6` | **`7.02e-3`** | **no (known miss)** |
| I4 penetration (`_a3`) | `<= 1e-9` | `2.33e-10` | yes |
| T1 warped, T1r plane | `<= 1e-6` | `2.30e-10` (run 1: `2.95e-10`), `5.21e-11` | yes |
| **T2 order of every qualifying solve** (`_a2`) | `>= 1.8` | **least `0.171`** of `[1.48, 0.191, 0.249, 1.57, 0.171]` | **no (known miss)** |
| C1, C1t, C2 | hold | iteration 5, nodes `[4, 9, 14, 19]`; C2 names the four end nodes | yes |
| R1, R2 (`_a2`) | refused | as registered; R2 names `xmax` | yes |
| X1 | holds | `PENDING` is `None` | yes |
| NP, NH, NI, NT, NC | as registered | `7.14e-6 L`, `6.04e-2`, `0.260`, `0.737`, converges | yes |
| B0 | unchanged | the five base digests and the count | yes |
| B1h | passes | 3 passed, 1 ignored | yes |

The known misses, both diagnosed and recorded as ignored `*_known_miss` tests with their numbers:

- **I3**: the shaft's stress is not uniform to rounding. The node pressures are `0.9955` of the mean on the ring's bottom
  nodes (`z = 0`) and `1.0045` on its top nodes (`z = T`) at every angle, and `sigma_xx` in the shaft ranges from `-7.545e7` to
  `-7.473e7`: a variation through the thickness, not around the ring (around it the pressure varies by `2.4e-4` only). The top ring
  nodes slide `3.2e-5` m along the shaft's faces (`eta = 0.99936`), from the Poisson mismatch of the hoop-tensioned ring and the
  compressed shaft with the bottom held by `u_z = 0`. The cause of the 0.9 percent difference is not identified: the slide's
  share of a face, `3e-4`, is too small to explain it. The registered estimate "exact up to rounding" assumed a state without
  through-thickness variation. I1, I2 and I4 met in the same run.
- **T2**: amendment 3's diagnosis. The measure of ADR 0404 (a4) on the last three residuals above `1e-12` mixes pre-asymptotic
  residuals (`1.0, 0.65, 2.97e-4`) with residuals at the rounding floor (`1e-12` to `2e-12`) in solves of three to five
  iterations. `r_{k+1}/r_k^2` stays bounded (`1.7`, `0.44`), so the rate is quadratic.

Mutations (a `git archive` copy of 686f84db per mutation, with its own `CARGO_TARGET_DIR`, running the non-ignored tests of
`sa_contact.rs`): all nine caught.

| id | caught by |
|---|---|
| MU1 curvature part dropped | T1 |
| MU2 no Uzawa update | P1/P2, B1, C2, S1 (`_a3`), HF |
| MU3 no `max(0, .)` | P1, NP, C1, S1, HF |
| MU4 `>` for `>=` | B1 |
| MU5 face-centre `N_a` | B1, S1, T1 |
| MU6 half-face areas | B1, S1, HF |
| MU7 cycle detection off | C1 |
| MU8 `- g P^T A^-1 P` dropped | T1 |
| MU9 normal reversed | B1, S1 |

The expected catches that did not happen belong to tests that are now ignored as known misses (I1/I3 for MU5, MU6, MU9; T2 for
MU1).

Gates: `cargo test -p rcfd-fem --release` passes (exit 0); `cargo clippy -p rcfd-fem --all-targets -- -D warnings` is clean;
`cargo fmt -p rcfd-fem` applied; `cargo test -p rcfd-cli --release --test docs_site` passes (2 tests).

**Decision by the rule: partial.** B0 and B1h unchanged, and the module landed with P1, P2, B1, S1, T1 and C1 met. I3 and T2
missed, were diagnosed and are recorded as `i1_to_i4_..._a3_known_miss` and `t2_..._a2_known_miss`.

Deviations from the registration, all in the three amendments: P1's reaction (amendment 1); the inside slack `1e-2`, the
`eps G G^T` iteration on a refused pivot, the run's reference floor and R2's test (amendment 2); the acceptance on `|g|` at
active nodes, the gap from differences, and `|K u|` and `|f_c|` in the linear reference (amendment 3). No limit was changed.

Remaining questions, with recommendations:

1. **I3**: find the through-thickness variation. Recommendation: run the same fit with two element layers and with `u_z` free
   on both bodies, to see whether the variation follows the `z0` constraint or the one-layer Hex8; then re-register I3 or its
   estimate.
2. **T2's measure**: an order measure for short contact solves. Recommendation: `r_{k+1}/r_k^2` bounded over the kept
   residuals, or the order of a solve started near the solution, registered in a new ADR; the ADR 0404 measure stays for ADR
   0404.
3. **A case-file key**: the `[[contact]]` table of open question 4. Recommendation: as stated there, in its own item.
4. **The total Lagrangian body's reference**: amendment 3's reference change applies to the linear body only (the gap from
   differences applies to both). Recommendation: give `ContactSet` `in_residual_reference` (ADR 0479 D2) when a total
   Lagrangian interference case is registered.

## For the lead: roadmap entries

> F-427 status partial. Note: Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. ADR 0507, 2026-11-14: frictionless node-to-surface contact (contact.rs) on bilinear Hex8 target faces or a rigid plane. It uses closest-point projection with the exact gap gradient and second derivative, a per-node augmented Lagrangian force max(0, lambda - eps A g) with Uzawa updates until every active gap is within the tolerance, a linear-body Newton driver (sparse L D L^T, eps G G^T on a refused pivot) and the total Lagrangian driver through nonlinear::solve_from, and gives located errors for active-set cycling and exhausted augmentations. Met: bar on a rigid stop (1.1e-11); stacked blocks pressed and pulled apart (1.2e-10, no tensile multiplier); Hertz sphere on a rigid plane at a/R = 0.1 (radius 3.0e-4, peak 7.6e-4, approach 6.1e-4, 13 nodes); interference-fit pressure (8.0e-3) and Lame hoop stress (3.6e-3); tangent against central differences (2.3e-10); the cycling error names the nodes. Nine of nine mutations caught; default bits unchanged. Missing: uniform shaft stress in the fit (7.0e-3 against 1e-6, a through-thickness variation not yet explained) and the residual order of the total Lagrangian punch (least 0.17 against 1.8, the ADR 0404 measure on solves of three to five iterations). Both are recorded as known misses. No case-file key.

> Follow-up (new item): F-427 interference-fit through-thickness variation. Find why the one-layer ring-on-shaft fit of ADR 0507 has node pressures of 0.9955 and 1.0045 of the mean at the bottom and top, then re-register I3. Fan relevance: hub-shaft press-fit stresses.

> Follow-up (new item): a contact Newton order measure for short solves (r_{k+1}/r_k^2 bounded, or started near the solution), registered for the total Lagrangian contact punch of ADR 0507 T2.

> Follow-up (new item): a case-file `[[contact]]` table (contactor, target surface or plane, penalty, tolerance, initial gaps) for the static nonlinear case, writing node pressures and gaps (ADR 0507 open question 4).

> Follow-up (new item): contact search and targets: a bucket search in place of the all-faces loop, rigid cylinder and sphere targets, and Hex20 and Tet faces (ADR 0507 limits).
