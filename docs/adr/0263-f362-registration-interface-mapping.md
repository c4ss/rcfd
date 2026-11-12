# ADR 0263: F-362 registration: interface mapping by nearest-element projection and radial-basis interpolation, with an unmapped-point policy

Status: **REGISTERED 2026-11-12**, written before any code of F-362 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run and no flow solver was run: the code and documents under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done with scratch scripts (numpy, no flow solver).
Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0256](0256-f139-registration-cosimulation-participant.md) (F-139: `rcfd_fv::cosim`, `InterfaceMap`, the
consistent displacement map `H` and its transpose for the forces, G2 and G3; the model problem CWs of amendment 1; question
Q3, from which this item comes), [ADR 0261](0261-f361-registration-quasi-newton-coupling.md) (F-361: the least-squares update,
whose suite must pass unedited), [ADR 0244](0244-f237-registration-heat-rate-load-export.md) (the one-way load export of
`rcfd_post::structural`). Roadmap item F-362 (depends on F-139).

Number: the lead gave 0263 to F-362. Branch `f362-interface-mapping` from main 2fa39daf, worktree `w49i-work/f362`.

House rules (`w49i-work/AGENT_RULES.md`, ADR 0256, ADR 0261):

- limits are fixed here, before the code and the runs, and are not changed after a run; the support radius, the distance
  limits and the rank threshold are fixed here from the derivation and are not searched for after seeing results;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- defaults stay bit-identical: `InterfaceMap::new`, `piecewise_linear_rows`, `coupled_step` and every participant compute
  exactly what they computed before; the builders are new functions that nothing calls;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded; no existing
  test is edited (`cosim_f139.rs` and `cosim_f361.rs` must pass unedited);
- each registered run is made once; no new third-party crate (no linear-algebra crate), no unsafe code, no commit
  trailers, no push.

Clean-room. Every formula below is derived here from first principles. No paper, book, code or coupling library was opened
for this work. Names, cited from memory only so the reader can place the ideas (nothing is taken on their authority): the
pairing of a consistent motion transfer with its transpose for the loads (C. Farhat, M. Lesoinne, P. Le Tallec, Comput.
Methods Appl. Mech. Engrg. 157 (1998) 95-114); radial-basis interpolation with a polynomial augmentation for conditionally
positive definite kernels (C. A. Micchelli, Constr. Approx. 2 (1986) 11-22) and its use for mesh coupling (A. Beckert, H.
Wendland, Aerosp. Sci. Technol. 5 (2001) 125-134); the compactly supported positive definite function `(1 - r)^4 (4r + 1)`
(H. Wendland, Adv. Comput. Math. 4 (1995) 389-396). The positive definiteness of that function is not taken on trust: the
Cholesky factorisation below fails, and the build refuses, if a matrix is not numerically positive definite.

## What F-362 asks

The acceptance text of `docs/roadmap.json`, binding:

> InterfaceMap weights are built from the two interface meshes alone (nearest-element projection and a radial-basis
> interpolation) for a planar and a cylindrical wall with non-matching meshes; rigid translation and rotation are
> reproduced to 1e-12, resultant force, moment and work are conserved to 1e-12 by the transposed map, a wall point outside
> the structure is refused or mapped by a stated policy, and the model problem of ADR 0256 gives the same frequencies within
> 1e-3 with the built weights.

Clauses: (a1) the weights are built from the two interface meshes alone, by (a1p) nearest-element projection and (a1r) a
radial-basis interpolation; (a2) for a planar and a cylindrical wall with non-matching meshes; (a3) rigid translation and
rotation reproduced to `1e-12`; (a4) resultant force, (a5) moment and (a6) work conserved to `1e-12` by the transposed map;
(a7) a wall point outside the structure refused or mapped by a stated policy; (a8) the model problem of ADR 0256 gives the
same frequencies within `1e-3` with the built weights (both methods).

## What was read (no run)

1. ADR 0256 in full (G1 to G4, CWs of amendment 1, the addendum and Q3), ADR 0261 (status, derivations D1 to D8),
   `crates/rcfd-fv/src/cosim.rs` (`InterfaceMap::new` checks indices, finiteness and row sums within `1e-12`;
   `displacement_to_points`, `face_displacements`, `forces_to_nodes` the transpose through the face means;
   `piecewise_linear_rows` for a line of nodes, refusing a point outside the nodes), `crates/rcfd-fv/tests/cosim_f139.rs`
   (the channel, CWs, `cw_parts`, `cw_settings`, the modal coordinates, crossings), `docs/cosimulation-protocol.md`.
2. ADR 0244 and `crates/rcfd-post/src/structural.rs`: `overlap_weights` between two sets of parametrised rectangles (plane
   or cylinder `(theta, z)`), rows summing to 1 (consistent), and the conservative heat-flux map. **Not reusable here**:
   it maps face values by area overlap in a parametrisation the caller chooses, not wall points to structural nodes from the
   geometry; it reproduces constants only (no rotation); and `rcfd-post` depends on `rcfd-fv`, so `cosim` cannot call it
   without a cycle. `shell_tube` there builds a tube of shell elements in the same `(theta, z)` layout as the cylinder
   below, which is built in the test instead (no dependency on `rcfd-post`).
3. `rcfd_core::vec3` (add, sub, scale, dot, cross, norm). `rcfd-linalg` has sparse iterative solvers only; there is no dense
   factorisation in the workspace that `rcfd-fv` could reuse for an `n x n` dense symmetric system.

## Derivation

**D1. What a map with scalar weights reproduces.** `InterfaceMap` applies one weight per (wall point, node) pair to all three
components: `d_p = sum_i H_pi q_i`. A rigid motion of the structure, translation `a` and rotation `R` (finite, any angle)
about `c`, displaces a material point from its reference position `x` by `u(x) = a + (R - I)(x - c)`: **an affine function
of `x`**. With `q_i = u(x_i)`:

    d_p = (sum_i H_pi) (a - (R - I) c) + (R - I) sum_i H_pi x_i .

This equals `u(x_p)` for every `a`, `R`, `c` if and only if

    (P0)  sum_i H_pi = 1          and          (P1)  sum_i H_pi x_i = x_p        (linear precision),

(necessity: take `R = I` for P0; then `R - I` ranges over a set of matrices whose span is all of `R^{3x3}`, so P1). So the
acceptance's clause (a3) is exactly: every row sums to 1 and reproduces the point's own coordinates. P0 alone (the present
`InterfaceMap::new` check) gives the translation. A small rotation `u = omega x (x - c)` is the special case `R - I ~
[omega]_x`; P0 and P1 reproduce finite and small rotations alike, to rounding.

**D2. Why the transposed map conserves force, moment and work (clauses a4 to a6).** The fluid returns face forces `F_f`.
The map gives the point shares `G_p = sum_{f contains p} F_f / n_f` and the nodal forces `f_i = sum_p H_pi G_p` (`B^T F`,
`B = P H`, ADR 0256 G3).

- Work, for any `H`: `sum_i f_i . q_i = sum_p G_p . sum_i H_pi q_i = sum_p G_p . d_p = sum_f F_f . (1/n_f) sum_{p in f} d_p =
  sum_f F_f . d_f`. This is the transpose identity `(B^T F) . q = F . (B q)`: an identity of finite sums, exact up to
  rounding, whatever the weights.
- Resultant: `sum_i f_i = sum_p (sum_i H_pi) G_p = sum_p G_p = sum_f F_f` by P0 and because each face's force is shared out
  in parts summing to it.
- Moment about any `c`: `sum_i (x_i - c) x f_i = sum_p (sum_i H_pi (x_i - c)) x G_p = sum_p (x_p - c) x G_p = sum_f (xbar_f -
  c) x F_f` by P0 and P1, with `xbar_f` the mean of the face's points (the point where the protocol's equal shares apply the
  face force; for a parallelogram face it is the centroid).

Equivalently by duality: the virtual work of the nodal forces in a rigid translation (rotation) equals that of the face
forces exactly when the map reproduces that translation (rotation). So **conservation of the resultant and of the moment
is the same property as the reproduction of rigid translation and rotation (D1), and conservation of work holds for every
transposed map.** A map that is not the transpose (ADR 0256 MN1, the averaging map) conserves neither.

**D3. The structure's interface mesh.** Nodes `x_i` and elements: a point (1 node), a segment (2), a triangle (3) or a
quadrilateral (4 nodes, split internally into the triangles `(0, 1, 2)` and `(0, 2, 3)`); anything else is refused. The
fluid's interface mesh is the wall points and wall faces (`Fluid::point_coordinates`, `Fluid::face_points`). The builders
take these two meshes and the settings and nothing else (clause a1).

**D4. The distance of a wall point to the structure and the nearest point.** For each element the nearest point `y` and its
barycentric coordinates `lambda` (summing to 1, `y = sum lambda_j x_j`):

- point: `y = x_0`;
- segment `a, b`: `t = clamp((x - a) . (b - a) / |b - a|^2, 0, 1)`, `y = a + t (b - a)`, `lambda = (1 - t, t)`;
- triangle `a, b, c` with `n = (b - a) x (c - a)`: the projection `x' = x - ((x - a) . n / |n|^2) n` onto the plane and
  `lambda_a = ((b - x') x (c - x')) . n / |n|^2`, `lambda_b = ((c - x') x (a - x')) . n / |n|^2`, `lambda_c = 1 - lambda_a -
  lambda_b`. If all three are `>= 0`, `y = x'`. Otherwise `y` is the nearest of the three edges' nearest points, because
  `|x - z|^2 = |x - x'|^2 + |x' - z|^2` for every `z` in the plane and the nearest point of a convex polygon to an outside
  point `x'` lies on its boundary. A triangle with `|n| = 0` is treated as its three edges.

The distance is `delta_p = min over elements |x_p - y|`; the nearest element is the first one (in the given order) with the
smallest distance (strict comparison, so the result does not depend on rounding ties beyond the order). The search is a
loop over all elements, `O(points x elements)`; fine for interface meshes of a few thousand entities, stated as a limit.

**D5. Nearest-element projection (a1p).** Row of wall point `p`: the barycentric coordinates `lambda` of its nearest point
`y_p` on the nearest element `e`. P0 holds exactly (by construction, rounding only). P1 holds when `y_p = x_p`, i.e. on a
planar wall with the points on the structure. On a curved wall the two meshes are different polyhedra approximating one
surface, and a wall point lies off the structural facets by up to the sagitta `h^2/(8 r)` (radius `r`, chord `h`); then
`sum lambda_j x_j = y_p != x_p` and the rotation is reproduced only to `|R - I| delta_p`. **Offset correction** (on by default,
`correct_offset: true`): add to the row a correction `c` supported on the patch `S` of `e` (the nodes of every element that
shares a node with `e`) that removes the defect `g = x_p - sum lambda_j x_j` while keeping P0:

    minimise sum_{j in S} c_j^2   subject to   sum_j c_j = 0,   sum_j c_j x_j = g.

Lagrange: `c_j = alpha + beta . x_j`; the first constraint gives `alpha = -beta . xbar_S`, so `c_j = beta . (x_j - xbar_S)`,
and the second gives `C beta = g` with the scatter matrix `C = sum_{j in S} (x_j - xbar_S)(x_j - xbar_S)^T` (3 x 3,
symmetric, semi-definite). `C` is diagonalised by the cyclic Jacobi method (plane rotations, no library), `C = sum_k mu_k
e_k e_k^T`; with the **rank rule** "a direction counts when its spread `sqrt(mu_k / mu_max) >= 1e-6`" the solution is `beta =
sum_{k kept} e_k (e_k . g) / mu_k`. When `S` spans the three directions (a curved wall: the neighbouring facets are tilted)
the corrected row satisfies P0 and P1 to rounding. When `S` is flat or a line (a planar structure, a line of nodes) the part
of `g` along the dropped directions cannot be represented by the structure's nodes at all (no scalar weights on coplanar
nodes reproduce an off-plane coordinate) and is left; its largest size is reported (`MapReport::precision_defect`). Why
`1e-6`: an exactly flat patch has `mu = 0` up to rounding of the squared coordinates, `sqrt(mu/mu_max)` about `1e-8` or
less; a genuinely curved one has the depth of the facets over their width, `h/(4 r)` or more (cylinder below: `0.1`); the
threshold sits between, three decades from rounding.

The corrected weights are not all positive (the cylinder below: smallest `-0.056`). Positivity is not required by
`InterfaceMap` or by D1 and D2; with `correct_offset: false` the row is the plain barycentric one (non-negative, one
element), which reproduces translation and conserves the resultant and the work but not the rotation and the moment on a
curved wall (the negative control N1).

**D6. Radial-basis interpolation (a1r): what it needs to reproduce rigid motion exactly.** The interpolant of nodal values
`q_i` is

    s(x) = sum_j alpha_j phi(|x - x_j| / rho) + sum_k beta_k p_k(x),   s(x_i) = q_i,   sum_j alpha_j p_k(x_j) = 0 for all k,

with `p_k` a polynomial basis. Its value at a wall point is linear in `q`: `s(x_p) = sum_i H_pi q_i`. If a polynomial `pi`
of the space is the data, `q_i = pi(x_i)`, then `alpha = 0, s = pi` solves the system, so by uniqueness `s(x_p) =
pi(x_p)`: the interpolant **reproduces every polynomial in the space exactly**. P0 is reproduction of the constant, P1 of
the three coordinate functions. So rigid motion (D1) is reproduced exactly if and only if the polynomial space contains
`1, x, y, z` restricted to the nodes, i.e. **an augmentation of at least degree 1**; without it (`beta` absent) the rows do
not even sum to 1 (the mutation MU2). Uniqueness needs the polynomial part to be determined by the nodes: the matrix `P`
(`P_ik = p_k(x_i)`) of full column rank. On a planar structure the nodes are coplanar and `1, x, y, z` are dependent on
them; on a line of nodes (the model of ADR 0256) they span one direction. So the basis is built from the nodes: centroid
`xbar`, the eigenvectors `e_k` of their scatter matrix that pass the rank rule of D5, `L = max_i |x_i - xbar|`, and

    p_0 = 1,    p_k(x) = (x - xbar) . e_k / L    (k kept).

Then `P^T P = diag(n, mu_k / L^2)`, full rank by construction and well scaled. A planar structure gets `1` and two in-plane
coordinates: P1 holds for wall points in the plane (the planar wall), and in general along the structure's directions; the
part of P1 normal to a flat structure cannot be met by any scalar weights (D5) and is reported in the same way.

**The basis function and its support.** `phi(r) = (1 - r)^4 (4 r + 1)` for `0 <= r < 1`, `0` beyond (Wendland's `C^2`
function, positive definite in up to three dimensions; cited from memory, checked by the factorisation). Compact support
`rho` [m], a setting. Rule fixed here: **`rho = 3 h`, `h` the longest edge of the structural elements** (quadrilateral sides,
not diagonals). Reasons: every wall point within a structural element is at most the element's diagonal, `<= sqrt(2) h`,
from each of its nodes, so `rho > sqrt(2) h` keeps the nodes of its own element inside the support (with `rho < h` the
interpolant would be the polynomial with spikes at the nodes); `3 h` is twice that diagonal rounded up, so the ring of
neighbours is inside too; a larger `rho` gives a smoother interpolant at the price of conditioning (below) and a denser
matrix. The model of ADR 0256 has `h = 0.08` (`rho = 0.24`); the planar wall `h = 0.08` (`rho = 0.24`); the cylinder `h =
0.025` (`rho = 0.075`).

**How the dense system is solved without a linear-algebra crate.** With `Phi_ij = phi(|x_i - x_j| / rho)` (symmetric
positive definite for distinct nodes) the system is `[Phi P; P^T 0] [alpha; beta] = [q; 0]`, symmetric indefinite. Block
elimination: `alpha = Phi^{-1}(q - P beta)` and `P^T Phi^{-1} P beta = P^T Phi^{-1} q`. For the weights at a wall point
with `phi_p = (phi(|x_p - x_j| / rho))_j` and `p_p = (p_k(x_p))_k`: by symmetry `s(x_p) = h_p . q` with

    a = Phi^{-1} phi_p,    h_p = a - G S^{-1} (P^T a - p_p),    G = Phi^{-1} P,    S = P^T G,

(check: `P^T h_p = P^T a - S S^{-1}(P^T a - p_p) = p_p`, the polynomial reproduction). `Phi = L L^T` by Cholesky (written
out, `n^3/6` operations once), `G` by `m` pairs of triangular solves, `S` (`m x m`, `m <= 4`, symmetric positive definite
since `P` has full rank) by Cholesky, and each wall point costs two triangular solves (`n^2`). A pivot `<= 0` in either
factorisation refuses the build ("not positive definite": coincident nodes, or a kernel that is not positive definite at
this size). The rows are dense (`n` entries; zeros dropped). Cost `O(n^3 + points n^2)`, memory `points x n`: interface
meshes up to a few thousand nodes; stated as a limit (no sparse factorisation of the compact support).

**Conditioning.** The reproduction (P0, P1) is enforced by the solve, so its rounding error is about `kappa eps`, with
`kappa` the condition number of `Phi` (the `m x m` `S` is well scaled by the centred, scaled basis). For Wendland's
function the smallest eigenvalue of `Phi` falls with the ratio of the separation distance of the nodes to `rho`, the
largest grows with the number of nodes in a support; the arithmetic below gives `kappa(Phi) = 8.2` (model), `85` (planar),
`70` (cylinder) at `rho = 3 h`, and on the planar set `1.6e3, 1.1e4, 4.3e4, 1.1e5` at `6, 10, 15, 20 h`: at `3 h` the
rounding of the reproduction is far below `1e-12` (measured in the arithmetic: `3e-16` to `1.3e-15`). The builder reports
an estimate of `kappa(Phi)`: the largest eigenvalue by 100 steps of the power iteration, the smallest by 100 steps of
inverse iteration with the Cholesky factor, both from the start vector of ones, as Rayleigh quotients (each a bound from
inside, so the estimate is at most `kappa`).

**D7. The unmapped-point policy (a7).** A wall point is **outside the structure** when its distance `delta_p` (D4) exceeds
the setting `max_distance` [m] (finite, `>= 0`, no default: the caller states it). Two policies, `Unmapped`:

- `Refuse` (the default): the build fails, and the message names the first outside wall point, its coordinates, its
  distance and `max_distance`. No map is returned.
- `Nearest`: an outside point **moves with its nearest point on the structure**: its row is the row of the method
  evaluated at `y_p` instead of `x_p` (projection: the plain barycentric row, no offset correction; radial basis: the
  interpolant at `y_p`). Every row still sums to 1 (translation and resultant exact); such a point has the displacement of
  `y_p` in a rotation (not its own), and its share of the face forces acts at `y_p`, so the moment changes by exactly
  `sum_{p outside} (x_p - y_p) x G_p` (D2 with `x_p` replaced by `y_p` for those rows). The report lists the outside
  points.

`MapReport` returns per wall point `delta_p`, the list of outside points, `precision_defect` (the largest `|sum_i H_pi x_i -
x_p|` over the points inside), and for the radial basis the condition estimate. How to choose `max_distance`: above the
largest gap the two discretisations of one surface can have (on a curved wall the sagitta `h^2/(8 r)`, on a thin structure
represented by its mid-surface or axis half its thickness), below the distance to a different wall. Fixed here for the
checks: `0.1 h` for the planar and the cylindrical wall (the cylinder's sagitta is `0.049 h`, so a margin of 2), `0.6 Lz =
0.06` for the model of ADR 0256 (the line of nodes stands for the whole wall across the thickness `Lz = 0.1`, whose points lie
`Lz/2` from it; margin 20 percent).

**D8. The model problem with built weights (a8).** CWs of ADR 0256 amendment 1: six nodes at `(0, 0.08 i, 0.05)`, the
elements the five segments between them, the 27 wall points at `(0, 0.05 j, 0 | 0.05 | 0.1)`. Projection: every wall
point's nearest point is on the line at the same `y` (distance 0 or `0.05`), its barycentric coordinates are those of
`piecewise_linear_rows` (to rounding: `t` is computed as a dot product over a squared length instead of a quotient of
differences), the patch is a line, so the offset correction is not applied (the defect `0.05` along z is left and reported:
`precision_defect = 0.05`). The frequencies should equal ADR 0256's to rounding of the weights. Radial basis (`rho = 0.24`,
polynomial `1, y`): the wall points at `z = 0` and `z = 0.1` are farther from the nodes than those at `z = 0.05` and get
other weights; the two face layers are mirror images, so the faces see the mean of the rows at `z = 0` and `0.05`. The
displacement between nodes is the smoother radial-basis shape instead of the piecewise-linear one, which changes the
added mass and so the frequencies a little (arithmetic below: `-5.1e-4` for mode 0, `+1.9e-4` for mode 1 in the time-stepped
model).

## Arithmetic before this registration (disclosed)

Scratch `w49i-work/f362-scratch/arith.py` (imports ADR 0256's `model.py` unchanged), `arith2.py`, `arith3.py`, `arith4.py`
(numpy, float64; the projection, the correction and the radial-basis rows as derived above, solved with numpy's dense
solvers; the constraint check of D6 as written). Logs `arith*.log`. A variant of the radial-basis rows with an extra
projection onto the polynomial constraints was computed and is not used (no difference at these condition numbers). The
radial basis evaluated at the nearest point `y_p` for every point (not only outside ones) was computed for the model, for
information.

| Model of ADR 0256 (650 steps, `T_0/100`, Aitken from 1/3, tolerance `1e-8`) | `omega_m/omega` of `Q_0`, `Q_1` | iterations max, mean |
| --- | --- | --- |
| piecewise-linear weights (ADR 0256's model) | 0.998625, 1.011470 | 11, 8.642 |
| radial basis `rho = 0.24` at the wall points | 0.998113, 1.011662 (`-5.13e-4`, `+1.92e-4`) | 11, 9.142 |
| radial basis at the nearest point of the line (information; not the design) | 0.999512, 1.005612 (`+8.9e-4`, `-5.9e-3`) | 10, 8.032 |
| coupled eigenfrequencies (no time stepping), radial basis against piecewise linear | `-6.4e-4`, `+2.6e-4` | |
| `kappa(Phi)`, model, `rho = 0.24`; row sums; linear reproduction in y | 8.21; `2.2e-16`; `5.6e-17` | |

| Walls (below, B-set-ups) | rigid motion error / `max abs(u)` | P1 defect [m] | smallest weight | other |
| --- | --- | --- | --- | --- |
| planar, projection (plain = corrected) | `3.4e-16` | `5.6e-17` | 0 | distance `2e-17` |
| planar, radial basis `rho = 0.24` | `3.4e-16` | `1.4e-16` | `-0.148` | `kappa(Phi) = 85.2` |
| cylinder, projection plain | `5.6e-3` | `9.4e-4` (largest component) | 0 | largest distance `9.598e-4` = the sagitta |
| cylinder, projection corrected | `1.1e-15` | `6.9e-17` | `-0.056` | |
| cylinder, radial basis `rho = 0.075` | `9.5e-16` | `9.7e-17` | `-0.132` | `kappa(Phi) = 69.9` |
| cylinder, plain projection, moment defect with uniform random point forces (3 draws) | `3.1e-4, 2.0e-4, 2.4e-4` relative | | | |

Reading. The radial-basis frequency shift of mode 0 in the model, `5.1e-4`, is **half the acceptance limit `1e-3`: the
smallest margin of this registration.** ADR 0256 found the flow step within `7e-6` and `2e-5` of this model, so the shift
should carry over; a miss would mean the flow sees the off-line rows differently from the model. Evaluating the radial basis
at the nearest point (which would make the rows independent of z) would miss mode 1 by a factor 6 and also gives up P1
on a curved wall; it is not the design.

## Design

**S1. Where it lives.** New file `crates/rcfd-fv/src/cosim/mapping.rs`, declared from `cosim.rs` (`mod mapping;` and a `pub
use` of its items; `cosim.rs` gains those lines and the `InterfaceMap::build` constructor, nothing else changes there). No
change to `fsi.rs`, `lib.rs`, the participants, the traits or `coupled_step`. Items:

- `MapMethod::{Projection { correct_offset: bool }, RadialBasis { support: f64 }}`;
- `Unmapped::{Refuse (default), Nearest}`;
- `MapSettings { method, max_distance, unmapped }` with `MapSettings::projection(max_distance)` (offset correction on,
  refuse) and `MapSettings::radial_basis(support, max_distance)` (refuse); `validate` (finite positive support, finite
  `max_distance >= 0`);
- `MapReport { distances: Vec<f64>, outside: Vec<usize>, precision_defect: f64, condition: Option<f64> }`;
- `interface_rows(nodes, elements, points, &settings) -> Result<(Vec<Vec<(usize, f64)>>, MapReport), String>` (D3 to D7);
- `InterfaceMap::build(nodes, elements, points, face_points, &settings) -> Result<(InterfaceMap, MapReport), String>`: the
  rows of `interface_rows` through `InterfaceMap::new` (which keeps its checks).

Refused: no node; a non-finite coordinate; an element with no node or more than four, or an index out of range; no element;
a pivot `<= 0` (D6).

**S2. The protocol.** `docs/cosimulation-protocol.md` gets the option: the two builders, the settings, the policy and the
report, and the note that the structure's `describe` must then also give its interface elements (in the library the caller
passes them to the builder; the `Structure` trait is not changed). `docs/cosimulation-protocol.md` is the topic page.

**S3. Tests.** New file `crates/rcfd-fv/tests/cosim_f362.rs`; the flow runs `#[cfg_attr(debug_assertions, ignore)]`.

Set-ups (built in the test, no flow solver unless stated; vectors in metres):

- **BP (planar wall).** The plane through `o = (0.02, 0.01, -0.03)` with normal `(1, 2, 2)/3`, in-plane unit axes `e1 = n x
  (0, 0, 1) / |.|`, `e2 = n x e1`; the rectangle `0 <= a <= 0.4`, `0 <= b <= 0.15`, point `o + a e1 + b e2`. Structure: 6 x 4
  nodes (`a` in 5 steps of 0.08, `b` in 3 of 0.05), 15 quadrilateral elements; `h = 0.08`, `rho = 0.24`, `max_distance =
  0.008`. Fluid: 14 x 6 points (`a` in 13 steps, `b` in 5), 65 quadrilateral faces. The nodes and the points coincide only on
  the rectangle's corners and edges where the two grids happen to share a coordinate.
- **BC (cylindrical wall).** Radius `r = 0.05`, `0 <= z <= 0.1`, the full circle. Structure: 16 x 5 nodes (`theta_i = 2 pi i /
  16`, `z` in 4 steps of 0.025), 80 quadrilaterals (closed around); `h = 0.025`, `rho = 0.075`, `max_distance = 0.1 h =
  0.0025` (the sagitta `9.6e-4`; the chord `0.0195`). Fluid: 27 x 9 points on the circle at `theta = 2 pi (j + 0.37) / 27`, `z`
  in 8 steps, 216 quadrilateral faces (closed around).
- **BU (a wall point outside).** BP with the fluid rectangle extended to `-0.3 h <= a <= 0.4` (the first fluid column at `a =
  -0.024`, then 13 steps to 0.4): the 6 points of the first column lie `0.024` beyond the structure, three times
  `max_distance`; the others inside.
- **CWs** of ADR 0256 amendment 1 exactly (copied from `cosim_f139.rs`), with the rows of (i) `piecewise_linear_rows` (the
  reference, ADR 0256's set-up), (ii) projection, (iii) radial basis `rho = 0.24`; structure elements the five segments;
  `max_distance = 0.06`; settings C3 of ADR 0256 (Aitken from 1/3, `1e-8`, cap 20, stop). The initial mesh of each run is
  built from its own rows (as `cw_parts` does).

Rigid motion: translation `a = (0.3, -1.1, 0.7) x 1e-3` and the rotation by `0.3` rad about the axis `(0.2, 0.9, -0.4)/|.|`
through `c = (0.1, -0.05, 0.02)` (Rodrigues' formula), `u(x) = a + (R - I)(x - c)`. Random data: the LCG of `cosim_f139.rs`
(seed fixed per check), components in `(-1, 1)`.

## Registered checks

All in `crates/rcfd-fv/tests/cosim_f362.rs`, release, each run once. Relative errors: of displacements to `max_p |u(x_p)|`;
of the resultant to `sum_f |F_f|`; of the moment to `sum_f |xbar_f - c| |F_f|`; of the work to `sum_f |F_f . d_f|`; of P1 to
the size of the wall (the largest `|x_p - xbar|`).

### Acceptance

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| R1 (a1, a2, a3) | BP and BC, both methods (four maps), built with `InterfaceMap::build` from the two meshes: `Ok`; rigid motion (translation alone, rotation alone, both) at the wall points against `u(x_p)` | `Ok`; each `<= 1e-12` relative | D1, D5, D6: `3e-16` to `1.3e-15` |
| R2 (P0, P1) | the same four maps: largest `abs(sum_i H_pi - 1)`; largest `|sum_i H_pi x_i - x_p|` relative; `precision_defect` of the report | `<= 1e-13`; `<= 1e-13`; `<= 1e-13` relative | `1e-15`; `1e-16` |
| C1 (a4) | the four maps, random face forces (3 seeds): resultant of `forces_to_nodes` against `sum F_f` | `<= 1e-12` relative | D2: rounding |
| C2 (a5) | the same: moment about `c` of the nodal forces at the nodes against `sum_f (xbar_f - c) x F_f` | `<= 1e-12` relative | D2: rounding |
| C3 (a6) | the same with random nodal displacements: `sum f_i . q_i` against `sum F_f . d_f` | `<= 1e-12` relative | D2: identity |
| U1 (a7, refuse) | BU, both methods, `Refuse`: `Err`; the message names wall point 0 (the first of the outside column), its distance `0.024` and `max_distance` `0.008` | an error containing `wall point 0`, `0.024` and `0.008` | D7 |
| U2 (a7, mapped) | BU, both methods, `Nearest`: `Ok`; `outside` = the six points of the first column; their rigid-motion displacement against `u(y_p)` (`y_p` the point moved to `a = 0`); the inside points against `u(x_p)`; translation at all points | `Ok`; exactly those six; each `<= 1e-12` relative | D7 |
| U3 (a7, what the policy does to the balance) | BU `Nearest`, both methods, random face forces: resultant (C1's limit); the moment defect against the predicted `sum_{p outside} (x_p - y_p) x G_p`; that predicted defect itself | `<= 1e-12`; `<= 1e-12` relative; `>= 1e-6` relative | D7; the control half: the points are `0.3 h` off, a percent of the arms |
| F1 (a8) | CWs, 650 steps, with the projection rows and with the radial-basis rows: `omega_m/omega` of `Q_0` and of `Q_1` against those of the piecewise-linear run | each `abs(difference) <= 1e-3` | projection: rounding (`< 1e-9`); radial basis: `-5.1e-4`, `+1.9e-4` (model) |
| F0 (set-up of F1) | CWs: every step of all three runs converged; the piecewise-linear run's `omega_m/omega` against ADR 0256's recorded 0.998618 and 1.011453 | all; each within `1e-6` | the same set-up and code (the record has six digits) |

### Mapping details, conditioning, errors, controls

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| P1 (D8) | CWs: the projection rows against `piecewise_linear_rows` (a missing entry is 0); the report's `precision_defect` | every weight within `1e-15`; `0.05` within `1e-15` | rounding of `t` |
| G1 (D4) | BC: every wall point's distance against the closed form `r cos(theta - theta_mid) - r cos(pi/16)` (`theta_mid` the mid-angle of its facet); BP: every distance | `<= 1e-15` m; `<= 1e-15` m | rounding |
| K1 (D6) | the condition estimate of the report against the dense eigenvalues of the arithmetic: model `8.208`, BP `85.20`, BC `69.86` | within 5 percent, and not above `1.0001` times | 100 power and inverse steps |
| E1 | refused: no node; a NaN coordinate; an element with 5 nodes; an element index out of range; no element; support `0` or NaN; `max_distance` `-1` or NaN; two coincident nodes with the radial basis | an `Err` each | S1 |
| N1 (negative control of R1, C2) | BC, `Projection { correct_offset: false }`: rigid rotation error; moment defect (random face forces) | `>= 1e-4`; `>= 1e-5` relative | `5.6e-3`; `2e-4` to `3e-4` |
| D0 (defaults) | `cosim_f139` and `cosim_f361`, release, unedited, every test (the heavy ones in release); `cargo test -p rcfd-api --release --test baseline_hashes`; `git diff main` touches only `cosim.rs` (the module lines and `build`), the new `cosim/mapping.rs`, the new test file, this ADR and `docs/cosimulation-protocol.md` | all pass; exactly those files | nothing existing is changed |

Printed, no limit: the smallest weights of every map, the iteration counts of the three CWs runs, the condition estimates.

### Mutations (scratch, not committed)

Each applied alone to the final tree by an exact text replacement that must occur once; `cosim_f362` is run once (release)
and the named tests must fail; the file is restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the offset correction is never applied | R1 (BC projection), R2 |
| MU2 | the radial basis has no polynomial part (`m = 0`) | R1 (radial basis) |
| MU3 | the polynomial part is the constant only | R1 (rotation, radial basis), C2 |
| MU4 | `Refuse` never refuses | U1 |
| MU5 | under `Nearest` an outside point is evaluated at `x_p` | U2 |
| MU6 | the triangle's nearest point is always the plane projection (no edge fallback) | G1 or U2 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### After the code

`cargo test --release -p rcfd-fv --test cosim_f362 -- --test-threads=1 --nocapture`; D0's suites; `cargo fmt -p rcfd-fv
--check`; `cargo clippy -p rcfd-fv --all-targets -- -D warnings`. The whole-workspace suite is not run.

## Predictions (mine, before any run)

R1, R2, C1 to C3 at `1e-15` or below; U1 to U3 as derived; F1 for the projection below `1e-9` (or exactly 0), for the
radial basis `-5e-4` and `+2e-4`; F0 within `1e-6`; P1 and G1 at rounding; K1 within one percent; N1 `5e-3` and `2e-4`. The
check I trust least is F1 for the radial basis, mode 0: half the limit in the model, and the flow has never seen rows that
differ between the z layers of a wall column.

## Decision rule

- **done**: R1, R2, C1 to C3, U1 to U3, F1 are met, F0 holds, P1, G1, K1, E1, D0 are met, N1 holds, every mutation fails its
  named tests, and the option is documented in `docs/cosimulation-protocol.md`. The acceptance stays the maintainer's.
- **partial**: the builders exist and R1 is met for at least one method on both walls, and another acceptance check is
  missed (kept as `*_known_miss`, diagnosed), or one of P1, G1, K1, E1, D0 is missed, or a mutation is not caught.
- **todo**: no builder reproduces rigid motion on both walls, or the build fails on the set-ups.

A set-up check (F0) or a negative control (N1, U3's control half) that does not hold means the check is not shown to be
able to fail: reported as a defective set-up, corrected by an amendment before any re-run, never counted as a pass.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. Offset correction on by default for the projection.** It buys exact rigid motion and moment on curved walls at the
  price of small negative weights and a support of one ring of elements. Options: plain barycentric by default (positive,
  one element; moment off by `2e-4` on BC). Recommendation and choice: on, with `correct_offset: false` available.
- **T2. `Refuse` is the default policy, and `max_distance` has no default.** Options: map every point (silent extrapolation,
  what a tolerance-free nearest-point search does); a default distance from the node spacing. A wrong pairing of walls is a
  set-up error that silent mapping would hide; the right distance depends on the geometry (D7).
- **T3. Wendland's `C^2` function with a compact support and a linear polynomial.** Options: a global function (thin-plate
  spline, multiquadric, Gaussian), denser and worse conditioned; a quadratic polynomial (would reproduce more but needs more
  nodes and is not needed for rigid motion).
- **T4. The `Structure` trait is not extended with elements.** The builder takes them from the caller. Options: a default
  method `elements()` on the trait (the describe call would carry them). Left as a question (Q1 of the addendum).

## Limits of this registration

A brute-force nearest-element search; dense radial-basis rows and an `O(n^3)` factorisation (no sparse solve of the compact
support, no partition of unity for large meshes); elements up to four nodes, quadrilaterals split into two triangles (a
warped quadrilateral is approximated by its two triangles); scalar weights only (no rotational degrees of freedom of a
shell or beam: a line or a flat structure cannot reproduce the rotation of points off it, D5); no consistent (mass-matrix)
or mortar map; no time dependence of the map (it is built once on the reference configurations); no transport. The model
problem is the only flow run.

## Amendment 1 (2026-11-12, after the registered run of commit 8061d17a, before any other run)

**E1 missed on one case: two coincident structural nodes with the radial basis were accepted.** Every other case of E1
and every other registered check was met in that run (the numbers go in the addendum). Diagnosis by reading and by a
scratch replay (`f362-scratch/diag_e1.py`, the Cholesky of `mapping.rs` with plain sequential sums on the BP nodes with
node 5 appended): the registered refusal rule of D6, "a pivot `<= 0`", is an absolute test on a computed quantity. With two
equal rows of `Phi` the exact pivot of the second is 0, but the computed one is `a_jj - sum_k l_jk^2` after `j` rounded
products and is of size `j eps a_jj` with either sign; the replay (its kernel values computed by numpy, not bit for bit
those of the test) happened to give exactly `0.0` and refused, the test's arithmetic gave a positive rounding residue and
accepted. The rule is defective; the set-up of E1 is not. So:

- The registered test stays in the file as `e1_invalid_meshes_and_settings_are_refused_known_miss`, ignored, unchanged
  (its limit, "an `Err` each", intact); the miss counts against the decision rule as registered (**E1 missed**).
- **Amended rule (D6').** A pivot is refused when `d <= 2 n eps a_jj` (`n` the order of the matrix, `eps` the machine
  epsilon, `a_jj` the diagonal entry), in both factorisations (`Phi` and `S`). Derivation: the computed pivot of a
  rounding-level (singular) column differs from 0 by at most about `j eps a_jj <= n eps a_jj` (each of the `j` products
  `l_jk^2 <= a_jj` carries a relative rounding of `eps`, and so does the sum), so the factor 2 puts every such pivot below
  the threshold; a pivot of a positive definite matrix is at least its smallest eigenvalue, so a matrix is refused only if
  `lambda_min <= 2 n eps a_jj`, i.e. `kappa(Phi) >~ 1/(2 n eps)` (`2.8e13` for `n = 81`), where the reproduction would be
  off by `kappa eps`, about `1e-2`, in any case. On every set-up of this registration the smallest pivot is at least
  `lambda_min(Phi) >= 0.055` (arithmetic), eleven decades above the threshold (`2 x 81 x 2.2e-16 = 3.6e-14`), so the
  amended code computes the same bits on them as the registered one; only the refusal predicate changes.
- **E1b (new, run once on the amended code):** the ten cases of E1 unchanged, each must return an `Err`; the
  coincident-node case's message must name the radial-basis matrix. Run alone (`--exact`); the registered checks are not
  re-run on the amended code except through the mutation runs, whose results are not counted as registered runs.
- Mutation MU7 (added): the amended threshold back to the absolute test (`d > 0`): E1b must fail.
- The decision by the registered rule is therefore at best **partial** (E1 missed, not a defective set-up); the addendum
  reports E1b as the evidence for the amended rule.
