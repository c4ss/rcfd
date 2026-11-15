# ADR 0567: F-633 registration: a Shell3 pressure load built from a local curvature estimate, exact for the uniform membrane state on irregular curved meshes

Status: **REGISTERED 2026-11-15, written before any code of F-633 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-633 was written, compiled or run in the repository. The design below was prototyped
first, in plain numpy in the scratch folder (`w49i-work/f633-scratch/proto`: `proto.py` and `sphere.py`, copies of
`f546-scratch/proto.py` (the F-419 prototype of Shell3 that matched rcfd to `2.2e-9`, ADR 0454) and of the ADR 0550 mesh
generator; `cyl540.py`, a copy of `f540-scratch/diag.py` (the cylinder of ADR 0461); `curv.py` and `curv_v1.py`, the rejected
first versions; `curv2.py`, the load registered here; `runs.py`, `est.py`, `est_load.py`, `fitonly.py`, `piv.py`; logs `d1.log`
to `d4.log`, `est_sphere.log`, `est_cyl.log`, `est_nc.log`, `est_torus.log`, `est_load.log`; dense matrices, no rcfd code;
disclosed, their numbers quoted below). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-633 (deps F-546), structural solver, advanced tier; the follow-up named by
[ADR 0550](0550-f546-registration-angle-weighted-shell-pressure-on-an-unstructured-sphere-octant.md) (limit L3, remaining
question 1, option (b)). Number 0567 given by the lead. Branch `f633-work` from main b29c1383, worktree `w49i-work/f633`, scratch
`w49i-work/f633-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited
(new tests in a new file); no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-633 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. An opt-in Shell3 pressure load built from a local curvature estimate, on the unstructured sphere octant of ADR 0550 (t = 0.01): the radial displacement within 2 % of `p R^2 (1 - nu)/(2 E t)` at every node at n = 16 and n = 32, the one-diagonal cylinder O1 of ADR 0461 within 1 %, defaults bit-identical.

Clauses: (a0) every limit registered before the run (this document); (a1) an opt-in Shell3 pressure load built from a local
curvature estimate; (a2) the unstructured octant of ADR 0550 design 1 and 2 (R = 1, t = 0.01, E = 2e11, nu = 0.3, p = 1e5, its
symmetry conditions) at n = 16 and n = 32: `max_nodes |u_r / ref - 1| <= 2e-2`, `ref = p R^2 (1 - nu) / (2 E t)`; (a3) the
one-diagonal 32 x 16 cylinder quarter O1 of ADR 0461 clause (a1) (R = 1, H = 0.5, t = 0.01, the end z = H free):
`max_nodes |u_r / (p R^2 / (E t)) - 1| <= 1e-2`; (a4) defaults bit-identical.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0550 (whole) and ADR 0461 (the diagnosis, the design, the checks); the roadmap entries F-546
  and F-633.
- `crates/rcfd-fem/src/shell3.rs` (`facet`, `Facet3`, `pressure_load_vector`, `pressure_load_vector_angle_weighted`),
  `loads.rs` (`nodal_load_vector`, `add_shell_pressure`, `add_force`), `model.rs` (`Load`, its builders and the validation arm
  of `ShellPressure`), `lib.rs` (the module list); the matches on `Load` in `beam.rs`, `follower.rs`, `fracture.rs` (all with a
  catch-all arm); `crates/rcfd-core/src/vec3.rs`.
- Tests: `crates/rcfd-fem/tests/sa_shell_pressure_sphere.rs` (the mesh generator and the octant model, copied into the new test
  file), `sa_shell_pressure_angle.rs` (the cylinder model and its supports).
- **Not read:** any finite-element or geometry-processing code, manual or documentation outside this repository, and no paper.
  Everything used below is derived here. Recalled from memory and not used beyond the idea: that the area gradient of a
  triangulated surface is a discrete mean-curvature normal (the first variation of area, a classical result of differential
  geometry; it is re-derived in design 1.2 to the extent used) and that fitting a local quadric to a vertex neighbourhood is a
  common curvature estimator in mesh processing (no particular method or paper is followed; the fit of design 3 is derived
  here, with its exactness argument).

## Design

### 1. The load and why it is exact for the uniform membrane state

Notation: for a triangle `t` with nodes `(i, j, k)` in cyclic order, unit normal `e_3` (that of `shell3::facet`, along `(x_1 -
x_0) x (x_2 - x_0)`), the area gradient `dA_t / dx_i = (1/2) e_3 x (x_k - x_j)`; `G_i = sum_t dA_t / dx_i` over the triangles of
the loaded set at node `i`. For an edge `e` of triangle `t`, `nu_e` is the unit vector in the plane of `t`, perpendicular to `e`,
pointing out of `t`. A boundary edge is an edge of exactly one triangle of the set.

**The load (registered).** At every node `i` of the set,

    F_i = (p / (2 kappa_i)) * ( G_i - sum_{boundary edges e at i} (|e| / 2) (nu_e - (nu_e . m_e) m_e) ),

`kappa_i` the mean curvature at node `i` from the local fit of design 3 (signed: positive where the surface bends away from its
`e_3` side, so a sphere meshed with outward normals has `kappa = 1/R`), `m_e = unit(n_a + n_b)` the bisector of the fitted
normals at the two end nodes of `e`. No nodal moments (the default has none either).

1. **The uniform isotropic membrane state is the exact discrete solution of `N G`.** Under the uniform isotropic membrane force
   `N`, the CST nodal forces of a triangle are `N dA_t / dx_i` (the virtual work of an isotropic stress is `N dA`; ADR 0550
   design 3.1). So nodal loads `N G_i` are equilibrated by the uniform state `N`. On a triangulation inscribed in a sphere the
   uniform expansion `u = alpha x` produces exactly that state with zero bending and shear (ADR 0550 design 3.1, measured
   `6.4e-14` there); with `N = p R / 2` it is the pressurised sphere.
2. **`p / (2 kappa)` is the membrane force of pressure, and the load is a pressure load.** Equilibrium of an isotropic membrane
   normal to itself: `p = N (kappa_1 + kappa_2) = 2 N kappa` (Laplace's relation, from the force on a small patch: its edge
   tractions `N nu` integrate to `-N int 2 kappa n dA` by the divergence theorem on the surface, `div_S n = 2 kappa`); hence `N = p
   / (2 kappa)`. And under a normal variation `delta x = phi n` the area changes by `int 2 kappa phi dA` (the same identity), so
   on a fine smooth mesh `G_i -> 2 kappa n_i A_i` with `A_i` the node's share of the area: `F_i -> p A_i n_i`, the pressure.
   For a general curved surface the load is therefore a consistent pressure load (not exact); it is exact wherever the uniform
   isotropic state is (the sphere), and on the cylinder (design 1.4).
3. **Boundary edges.** At a node of a boundary edge, `G_i` contains `(|e| / 2) nu_e`: the membrane's pull across the edge
   (`dA / dx_i` of a triangle is `(|e_ij| nu_ij + |e_ik| nu_ik) / 2` over its two edges at `i`). On a free edge there is no such
   pull; on a cut (a symmetry plane) the pull belongs to the facets of the part not modelled. The load replaces it by the part
   that a smooth continuation of the surface across the edge would leave at the node: reflect triangle `t` through the plane
   spanned by the edge direction `t_e` and the surface normal `m_e` at the edge; the reflected facet's outward conormal is `nu_v =
   nu_e - 2 (nu_e . c) c`, `c = t_e x m_e`, and the pair's force at the edge, shared half and half between the two facets, is
   `(|e| / 2) (nu_e + nu_v) / 2 = (|e| / 2) (nu_e . m_e) m_e` (since `nu_e` is perpendicular to `t_e`, it lies in the span of `c`
   and `m_e`). The removed part `(|e| / 2)(nu_e - (nu_e . m_e) m_e) = (|e| / 2)(nu_e . c) c` lies along the in-surface conormal
   `c`.
   - On a symmetry plane of a symmetric surface the continuation is the mirror image; with the exact normal, `m_e` lies in the
     plane and is perpendicular to the chord, so `c` is the plane normal: the removed part is along the constrained direction and
     the result equals the raw `G_i` up to the reaction. The sphere octant is therefore exact (design 1.1).
   - On the cylinder's free edge z = H (a hoop chord of a planar rectangle cell) `nu_e = e_z` and `m_e` is radial: everything is
     removed, which is the free edge of the hoop state (design 1.4).
4. **The cylinder O1 (arithmetic).** With `kappa = 1/(2R)`: `p / (2 kappa) = p R`. Each cell of the quarter is a plane rectangle
   (two generators at `theta_i`, `theta_{i+1}`, chord `2 R sin(dth / 2)`, height `dz`); under a unit isotropic tension the CST
   forces of its two triangles sum to the rectangle's edge pulls, independent of the diagonal (the diagonal's pulls cancel; ADR
   0461 diagnosis 2). At an interior node the axial pulls of the cells above and below cancel and the hoop pulls give `2 dz sin(dth
   / 2)` radially; at a node of the free edge, after design 1.3, `dz sin(dth / 2)` radially; at a node of the planes
   `theta = 0, pi/2` the raw `G_i` (design 1.3, first case) leaves `dz sin(dth / 2)` radially plus the constrained part; z = 0
   likewise. So `F_i = p R 2 sin(dth / 2) dz w_i` along the radius, `w_i` the tributary weight (1/2 on the edges, 1/4 at
   corners): the polygon's tributary radial load of ADR 0461 diagnosis 2, under which the one-diagonal facet mesh reproduces the
   uniform hoop state `N_theta = p R`, `N_z = 0`, so `u_r = p R^2 / (E t)` at every node (ADR 0461 run D1: `8e-10`). The load
   gives the reference itself, not the polygon's `cos(dth / 2)` of the angle split (ADR 0461 design 2).

### 2. Prototype runs of the load with exact geometry (disclosed)

`curv.py` with the curvature and the normals replaced by the exact ones (`d2.log`): the octant `9e-13` at n = 16, the cylinder
O1 `7.9e-10`. With the same exact values but other boundary treatments (`d1.log`, `d2.log`, `est_nc.log`): the raw `G_i`
(no boundary term changed) gives the sphere exactly (`9.7e-13`) but the cylinder `u_r / ref = 0.7000000` at every node (`1 -
nu`: the free end then carries the isotropic pull, a closed-end vessel with `N_z = p R`), a miss of the acceptance; removing
the whole boundary term `(|e| / 2) nu_e` gives the sphere `1.68` and the cylinder `33.6` (the symmetry nodes lose their
load). So the treatment of design 1.3 is needed, and with it both meshes are exact when `kappa` and `n` are.

### 3. The local curvature estimate (registered)

Rejected first (`curv_v1.py`, `d1.log`, `d2.log`): a least-squares quadratic height function `w = a u^2 + b u v + c v^2 + d u +
e v` over the 2-ring. Its curvature is only second-order accurate (`kappa` errors `4.6e-3 .. 3.5e-2` on the octant at n = 16,
`6e-4 .. 8e-3` on the cylinder) and its normal error matters at the boundaries: `u_r` errors `5.3e-2` (sphere n = 16) and
`9.4e-2` (cylinder), misses of both limits. The cause: on the symmetry columns the radial load is the small difference `dz sin(dth
/ 2)` of design 1.4 and a normal error `delta` changes it by `delta / (dth / 2)` relative.

**Registered: an implicit quadric through the node.** At node `i`, the frame `(t_1, t_2, n^aw)` with `n^aw` the angle-weighted
normal of the facets at `i` (`sum theta_i^t e_3^t`, normalised), `t_1 = unit(n^aw x a)` with `a = e_x` if `|n^aw_x| < 0.9`
else `e_y`, `t_2 = n^aw x t_1`. The stencil points `x_j` (design 3.2) in local coordinates `(u, v, w)` of `x_j - x_i`, scaled by
`h = sqrt(mean(u^2 + v^2 + w^2))`: `(s, r, q) = (u, v, w) / h`. Least squares (Householder QR) for the eight coefficients of

    Q(s, r, q) = a s^2 + b r^2 + c q^2 + d s r + e r q + f q s + g s + k r + q = 0

(the quadric through the node with its `q` coefficient normalised to 1). The gradient at the node `grad = (g, k, 1) / h`, the
Hessian `Hs = [[2a, d, f], [d, 2b, e], [f, e, 2c]] / h^2`, the normal `n_i = grad / |grad|` (in global axes; its `w`
component is positive, so it is on the `e_3` side), and the mean curvature of the level surface (`-(1/2) div(grad Q / |grad
Q|)`, written out):

    kappa_i = (|grad|^2 tr(Hs) - grad^T Hs grad) / (2 |grad|^3).

Check: `Q = |x|^2 - R^2` gives `1/R`, `Q = x^2 + y^2 - R^2` gives `1/(2R)`.

1. **Exact for every quadric surface whose stencil determines it.** If the stencil points lie on a quadric through `x_i` with a
   non-zero `q` component of the gradient, that quadric (scaled) makes every residual zero; if no other quadric through the
   points exists, the least-squares solution is it, and its gradient and curvature are exact. Sphere: the quadric functions
   restricted to a sphere span the spherical harmonics of degree at most 2 (a 9-dimensional space, `x^2 + y^2 + z^2` being
   constant); one through the node and eight more points in general position determine it. Cylinder: restricted to the
   cylinder, a quadric through the node is a combination of `{1, cos theta, sin theta, cos 2 theta, sin 2 theta, z, z cos theta,
   z sin theta, z^2}`; for each fixed `z` a trigonometric polynomial of degree 2, which vanishes at five distinct angles only if
   it is zero. So a stencil that spans **five distinct angles** (and three axial levels) determines the cylinder; four angles
   leave a one-parameter family of spurious quadrics (a kernel vector of the 5 x 4 trigonometric system), and the fit fails.
2. **The stencil (registered).** The `k_i`-ring of node `i` in the set's triangles (the nodes reached by at most `k_i` edges),
   without `i`, with `k_i = max(2, 4 - d_i)` and `d_i` the graph distance of `i` to the nearest node of a boundary edge (0 on the
   boundary). An interior node's 2-ring spans five angular positions (`-2 .. 2`); a node at distance `d` from the boundary has `d`
   rings on one side, so it needs `4 - d` on the other. Rejected (`est_nc.log`): two rings everywhere (the cylinder `4.0e-2`, the
   sphere `2.1e-3`); three rings everywhere (the cylinder `1.0e-1`); and a growth rule by the pivot ratio of the normal equations
   (`curv.py`, `piv.py`), because that ratio falls as `h^4` with refinement on a sphere (`9e-5, 2e-6, 8e-8` at n = 8, 16, 32): a
   fixed threshold would treat fine meshes as degenerate.
3. **Prototype (`curv2.py`, `est_sphere.log`, `est_cyl.log`, `fitonly.py`).** `max |kappa - 1|` on the unstructured octant
   `3.3e-14, 1.4e-13, 6.7e-13, 3.1e-12` at n = 16, 32, 64, 128; the normal to `1.4e-14`; on the cylinder O1 `|2 kappa R - 1|
   <= 3.6e-13`. Condition number of the scaled least-squares matrix at most `1.5e3, 7.6e3, 3.6e4, 1.7e5` at n = 16 to 128.
   Stencils (`est_load.log`): octant n = 32 361 nodes with 2 rings, 104 with 3, 96 with 4; cylinder O1 377, 88, 96.
4. **A surface that is not a quadric (`est_torus.log`; an observation, design limit L2).** The outer quarter-patch of a torus
   (centre radius 1, tube radius 0.4, `theta` in `[0, pi/2]`, `phi` in `[-pi/4, pi/4]`), N x N cells cut along one diagonal,
   exact mean curvature `(R_0 + 2 r cos phi) / (2 r (R_0 + r cos phi))`: structured, the interior (2-ring) nodes have `kappa`
   errors `1.99e-2, 4.89e-3, 1.26e-3, 4.31e-4` at N = 8 to 64 (second order); the boundary nodes, with their one-sided 4-ring
   stencils, `2.8e-1, 2.1e-1, 1.36e-1, 7.8e-2` (first order, slowly). With the interior nodes jittered by up to 0.2 cell in each
   parameter direction (the LCG of ADR 0550, seed `633 + N`; nodes numbered `(N + 1) i + j`, `i` along `theta`; every node draws
   two numbers in turn, `(2 u - 1) 0.2 / N` added to `i / N` and `j / N` at interior nodes only): interior `2.8e-2, 1.9e-2, 1.08e-2, 8.2e-3`, boundary as structured. Weighting the fit by `1 /
   |d|^2` or `1 / |d|^4` (`fitonly.py`) lowered the boundary error by about a third only and is not taken. The estimate is exact
   on quadrics and consistent elsewhere, with a first-order error at the boundary.

### 4. Refusals (registered)

- An element of the set that is not Shell3 (the model check of `add_load`, key `"shell pressure"`; Shell4 cells keep their own
  loads, design limit L4); a triangle refused by `shell3::facet` (its own keys).
- An edge of more than two triangles of the set, or an interior edge traversed in the same direction by its two triangles (key
  `"orientation"`: the normals and the curvature sign need a consistently oriented manifold surface).
- A stencil of fewer than eight points, a least-squares matrix whose QR has `|R_jj| <= 1e-10 max_l |R_ll|` (the stencil does not
  determine a quadric: a flat or straight-lined patch), or `|kappa_i| h_i < 1e-8` (no curvature: the membrane force `p / (2
  kappa)` is not defined); key `"curvature"`, naming the node. A flat plate is refused by the second condition (its `q` columns
  vanish), a saddle point of zero mean curvature by the third.

### 5. Code (opt-in; defaults bit-identical by construction)

- New module `crates/rcfd-fem/src/shell3_curvature.rs`: `NodalCurvature { kappa, normal, rings }`,
  `curvature_estimates(nodes: &[Vec3], tris: &[[usize; 3]]) -> Result<Vec<NodalCurvature>>` (design 3) and
  `pressure_load_curvature(nodes, tris, p) -> Result<Vec<Vec3>>` (design 1), on a local triangle list; every set and map
  ordered (`BTreeSet`, `BTreeMap`), sums in a fixed order, no threads.
- `model.rs`: the variant `Load::ShellPressureCurvature { set, pressure }` (`#[non_exhaustive]`, like its neighbours) and the
  builder `Load::shell_pressure_curvature(set, pressure)`; its validation arm (set, finite pressure, Shell3 only). `loads.rs`:
  its arm in `nodal_load_vector` (the set's elements in set order, local node numbers by first appearance, the forces added node by
  node in that order). `lib.rs`: one `pub mod` line. A dead load: the follower forms (`follower.rs`) do not take it.
- No existing line of a code path that a model without the new load reaches is changed: the new `match` arms are reached only
  by the new variant; the existing arms, functions and constants are unchanged. Checked by B1 to B3.
- One new test file `crates/rcfd-fem/tests/sa_shell_pressure_curvature.rs` with the octant generator of ADR 0550 (copied), the
  cylinder quarter of ADR 0461 (rebuilt on its pattern), a torus patch and every check below.

## Checks (limits fixed now)

`e` = `max_nodes |u_r / ref - 1|`; the curvature load unless stated; the octant unstructured (ADR 0550 design 2) unless stated.

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| A1 | a2 | octant n = 16, t = 0.01: `e` | `<= 2e-2` | design 2, 3.3: `9.1e-13` |
| A2 | a2 | octant n = 32, t = 0.01: `e` | `<= 2e-2` | `3.1e-12` |
| A3 | a3 | cylinder O1 (32 x 16, one diagonal): `e` against `p R^2 / (E t)` | `<= 1e-2` | design 1.4: `7.5e-10` |
| X1 | design 1 | the exactness on the octant: `e` at n = 16, 32, 64 | `<= 1e-8` each | `9.1e-13`, `3.1e-12`; n = 64 not prototyped, estimated `1e-11` from the growth with n |
| X2 | design 1.4 | the cylinder 32 x 16 with the other diagonal, the checkerboard, the axially warped rows of ADR 0461 design 5 (amplitude 0.3), and 16 x 8 one-diagonal: `e` | `<= 1e-7` each | `1.6e-10`, `7.5e-11`, `1.6e-9`, `1.4e-11` |
| X3 | design 1 | the octant at t = 0.001, n = 16 and 32: `e` (the `1/t` growth of ADR 0550 D5 gone) | `<= 1e-7` each | `1.8e-12`, `1.6e-11` |
| X4 | design 1 | the structured octant n = 16: `e` | `<= 1e-8` | `6.2e-13` |
| L1 | design 1.3 | the load vector of the octant n = 32 against the area-gradient load `(p R / 2) G_i`, the components along the plane normals removed at the nodes of each symmetry plane: `max |F - F_ag| / max |F_ag|` | `<= 1e-10` | `7.1e-15` |
| L2 | design 1.4 | the load vector of the cylinder O1 against the polygon's tributary radial load `p 2 R sin(dth / 2) dz w_i e_r`, the same removal: `max |F - F_poly| / max |F_poly|` | `<= 1e-10` | `6.7e-14` |
| K1 | design 3.1 | `curvature_estimates` on the octant n = 32: `max |kappa - 1/R|`, `max |n - x/R|`; on the cylinder O1: `max |2 R kappa - 1|`, `max |n - e_r|` | `<= 1e-10` each | `1.4e-13`, `2.8e-15`; `3.6e-13`, `5.8e-15` |
| K2 | design 3.2 | the stencil rule: the counts of nodes with 2, 3, 4 rings on the octant n = 32 and the cylinder O1 | `(361, 104, 96)`, `(377, 88, 96)` exactly | `est_load.log` |
| T1 | design 3.4 | the torus patch, structured, N = 32: `max |kappa / kappa_exact - 1|` at the 2-ring nodes; at all nodes; the 2-ring order `log2(err(32) / err(64))` | `<= 3e-3`; `<= 2e-1`; `>= 1.5` | `1.26e-3`; `1.36e-1`; `1.55` |
| T2 | design 3.4 | the torus patch jittered (0.2 cell, LCG seed `633 + N`), N = 32: the same two maxima | `<= 2e-2`; `<= 2e-1` | `1.08e-2`; `1.37e-1` |
| S1 | design 1 | the load with `-p` is the load with `p` negated, bit for bit, on the octant n = 16 | exact | the formula is linear in `p` with one multiplication |
| NC1 | a2 | the default `Load::shell_pressure` on the octant at n = 16 and 32 misses 2 %: `e` | `> 2e-2` each | ADR 0550: `7.88e-1`, `2.26e-1` |
| NC2 | a2 | `Load::shell_pressure_angle_weighted` on the octant at n = 32 misses 2 %: `e` | `> 2e-2` | ADR 0550: `1.40e-1` |
| NC3 | a3 | the default `Load::shell_pressure` on the cylinder O1 misses 1 %: `e` | `> 1e-2` | ADR 0461: `9.5e-2` |
| RF1 | design 4 | refusals: a Shell4 element in the set (`add_load`, key `"shell pressure"`); a flat 4 x 4 Shell3 plate (`nodal_load_vector`, key `"curvature"`); the octant n = 8 with one triangle reversed (key `"orientation"`); an unknown set (`UnknownSet`); a non-finite pressure (`add_load` refused) | each refused with that key | design 4 |
| B1 | a4 | `git diff b29c1383 -- crates/*/src` touches only `rcfd-fem/src/{shell3_curvature.rs (new), lib.rs, model.rs, loads.rs}`, and in the last three adds lines only (no existing line removed or changed) | as stated | design 5 |
| B2 | a4 | `cargo test -p rcfd-fem --release`: every test binary of the crate (the bit records `sa_shell_pressure_bits`, `sa_shell3_bits` and the others unedited) and the new one | pass | design 5 |
| B3 | a4 | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | design 5 |

B1 allows the formatting of a line that `cargo fmt` changes only because a new arm was added beside it (a line rewrapped, its
tokens unchanged); any such line is listed in the addendum.

## Planned mutations (on a scratch copy of the library, one at a time, own `CARGO_TARGET_DIR`; the new test file only)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | the boundary term removed whole (`nu_e` in place of `nu_e - (nu_e . m_e) m_e`, design 1.3) | A1 to A3, X1, X2, L1, L2 |
| MU2 | the boundary term left as is (raw `G_i`) | A3, X2, L2 |
| MU3 | the stencil fixed at two rings (`k_i = 2`) | A3, X1, X2, K1, K2 |
| MU4 | the curvature without the gradient term (`kappa = tr(Hs) / (2 |grad|)`) | K1, A1 to A3, X1 |
| MU5 | the quadric without its `q^2` term (seven coefficients) | K1, X1, L1 |
| MU6 | `m_e = n_a` (the normal of one end) in place of the bisector | X1, L1 |
| MU7 | the new arm of `nodal_load_vector` calls the angle-weighted path instead | A1, A3, X1, L1 |

## Decision rule

- **done:** A1 to A3, X1 to X4, L1, L2, K1, K2, T1, T2, S1, NC1 to NC3, RF1, B1 to B3 met, every mutation caught by a registered
  check, no amendment after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the test does not build.

## Limits (not covered)

- L1. Exact only where the uniform membrane state is what pressure produces and the surface is a quadric the stencil
  determines: the sphere (any triangulation inscribed in it, symmetry cuts included) and the circular cylinder with rectangle
  cells (free hoop edges, symmetry cuts). On a cylinder whose triangulation is irregular, the isotropic `N G_i` has tangential
  parts that the anisotropic hoop state does not balance; that case is not measured here (the warped mesh of X2 keeps
  plane cells along each generator pair).
- L2. On a surface that is not a quadric the curvature estimate is consistent but not exact: second order inside, first order
  at the boundary with a large constant (design 3.4: 14 percent at N = 32 on the torus patch); the load at boundary nodes is
  then off by that ratio. No displacement problem on such a surface is measured.
- L3. Refused where the mean curvature vanishes (flat, saddle points) or the stencil does not determine a quadric (design 4); a
  model with flat parts takes the default pressure there, on its own element set (whose boundary with the curved set is then a
  free edge of each).
- L4. Shell3 only; Shell4 cells are refused, not loaded. Dead load only.
- L5. The boundary treatment assumes the smooth continuation of the surface across every boundary edge of the set (design 1.3):
  right for free edges and symmetry cuts of a smooth surface; at a crease or at a junction with another shell the edge is treated
  as free.

## Open questions

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):

1. The API: a new `Load` variant with its builder, mirroring `Load::shell_pressure_angle_weighted`, rather than a flag on the
   existing variants (which would change their shape).
2. The refusal of non-Shell3 elements (L4) rather than a mixed treatment: the boundary rule of design 1.3 would need Shell4
   edges too.

Not taken:

3. Whether a future default should use this load on curved Shell3 sets: no (it changes default results); recommendation to keep
   it opt-in.
