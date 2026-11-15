# ADR 0579: F-656 registration: curvature shell pressure on a non-quadric shell (a pressurised torus segment and a boundary curvature estimator)

Status: **REGISTERED 2026-11-15, written before any code of F-656 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-656 was written, compiled or run in the repository, except one disclosed measurement
of the unchanged base (design 5: four bit digests of the ADR 0567 load computed on a `git archive` copy of main e348eb34 in the
scratch folder, `f656-scratch/base`, by a throw-away test appended there; `f656-scratch/base_hashes.log`). The design was
prototyped first in plain numpy (`w49i-work/f656-scratch/proto`: `proto.py`, `curv2.py`, `sphere.py`, `cyl540.py`, `runs.py`
copied from `f633-scratch/proto` (the Shell3 prototype that matched rcfd to `2.2e-9`, ADR 0454, and the ADR 0567 load);
`curv3.py` (the estimator registered here), `tseg.py` (the torus segment), `estb.py`, `other.py`, `resid.py`, `where.py`; logs
`estb.log`, `other.log`, `resid.log`, `t_clamp.log`, `t_clamp_t001.log`, `t_nc.keep.log`, `mu5_proto.log`, `rf_proto*.log`; no
rcfd code; their numbers quoted below). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-656 (deps F-633), structural solver, advanced tier; the follow-up named by
[ADR 0567](0567-f633-registration-shell3-pressure-load-exact-for-the-uniform-membrane-state.md) (limit L2, remaining question
1, option (b)). Number 0579 given by the lead. Branch `f656-work` from main e348eb34, worktree `w49i-work/f656`, scratch
`w49i-work/f656-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited
(new tests in a new file); no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-656 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. A pressurised torus segment (Shell3, symmetry cuts) loaded by `Load::shell_pressure_curvature`: the radial and meridional displacements measured against the membrane solution of the torus on two refinements, with the error and its order recorded and the boundary-node curvature error of ADR 0567 (14 % at 32 x 32) reduced below 2 % by a registered boundary estimator, defaults bit-identical.

Clauses: (a0) every limit registered before the run (this document); (a1) a torus segment of Shell3 elements with symmetry cuts,
loaded by the curvature pressure of ADR 0567; (a2) its radial (here: normal, along the tube radius) and meridional displacements
against the membrane solution (design 2) on two refinements, the error and its order recorded (with limits); (a3) the
boundary-node curvature error of ADR 0567 check T1 (the torus patch, structured, 32 x 32: `1.3573e-1` over all nodes, all of it
at the boundary) below `2e-2` with a registered boundary estimator; (a4) defaults bit-identical.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0567 (whole: the k-ring implicit quadric, the boundary term of design 1.3, the torus patch
  of design 3.4, amendment 1 on signed zeros), ADR 0550 (the octant), the roadmap entry F-656; the style of ADR 0572 and 0573.
- `crates/rcfd-fem/src/shell3_curvature.rs` (whole), `loads.rs` (`nodal_load_vector`, `add_shell_pressure_curvature`),
  `model.rs` (`Load`, its builders, the validation arm of `ShellPressureCurvature`); the test
  `crates/rcfd-fem/tests/sa_shell_pressure_curvature.rs` (whole: its helpers, generators and checks).
- **Not read:** any finite-element or geometry-processing code, manual or documentation outside this repository, and no paper.
  Recalled from memory and not used beyond the idea: that fitting a truncated Taylor polynomial (a "jet") of the height function
  over the tangent plane is a common curvature estimator in mesh processing, with the k-th derivative error of a degree-n fit of
  order `h^(n - k + 1)` (the order argument is re-derived in design 1.2; no method or paper is followed); and the classical
  membrane theory of a pressurised torus (the forces of design 2 are re-derived here from equilibrium).

## Design

### 1. The boundary estimator (registered): a quartic height jet where the quadric is not exact

**Diagnosis of the 14 percent (prototype, `estb.log`).** On the ADR 0567 torus patch (centre radius `R_0 = 1`, tube radius `r =
0.4`, `theta` in `[0, pi/2]`, `phi` in `[-pi/4, pi/4]`) the quadric's error at the boundary-distance-0 and -1 nodes (whose
`k`-rings, `k = 4 - d`, are one-sided) is `2.78e-1, 2.11e-1, 1.36e-1, 7.82e-2` at N = 8 to 64: below first order. The implicit
quadric has eight free coefficients and is exact on quadrics; on a non-quadric surface the one-sided stencil leaves it free to
bend the fitted quadric away from the surface on the side without points, and the node is at the edge of the fitted region.

**The estimator.** At a node of boundary distance `d <= 1` (exactly the nodes whose ADR 0567 stencil has 3 or 4 rings):

1. The ADR 0567 quadric fit on the ADR 0567 stencil, as now, and its relative residual `rho = |A c - b| / |b|` (the scaled
   least-squares system of ADR 0567 design 3: rows `[s^2, r^2, q^2, s r, r q, q s, s, r]`, right side `b = -q`).
2. If `rho <= QUADRIC_RESIDUAL_TOLERANCE = 1e-9` the stencil lies on a quadric to rounding and the quadric's curvature and normal
   are kept (they are exact there, ADR 0567 design 3.1). Prototype (`resid.log`): on the octant at n = 16, 32, 64 the largest
   `rho` at these nodes is `2.9e-15, 1.4e-14, 6.5e-14`, on the cylinder O1 `8.0e-15`; on the torus patch the smallest is
   `3.9e-3, 9.4e-4, 2.3e-4, 5.8e-5, 1.5e-5` at N = 8 to 128 (jittered alike), falling as `h^2`. The tolerance lies between by
   four orders of magnitude on each side; at the torus's rate it would select the quadric only beyond N of about 15000.
3. Otherwise the **quartic height jet**: in the same frame `(t_1, t_2, n^aw)` (the angle-weighted normal of ADR 0567 design 3)
   the stencil points `(u, v, w)` of `x_j - x_i`, scaled by `h = sqrt(mean(u^2 + v^2))`, `(s, r) = (u, v) / h`; least squares
   (Householder QR, the rank test of ADR 0567: a diagonal of `R` at most `1e-10` of the largest refuses) for the 14 coefficients
   of `w / h = c_0 s + c_1 r + c_2 s^2 + c_3 s r + c_4 r^2 + (four cubic) + (five quartic)` (monomials of degree 1 to 4, in the
   order `s^a r^(k - a)`, `a = k .. 0`, `k = 1 .. 4`; no constant: through the node). Then `w_u = c_0`, `w_v = c_1`, `w_uu = 2
   c_2 / h`, `w_uv = c_3 / h`, `w_vv = 2 c_4 / h`, and
   
       kappa_i = -((1 + w_v^2) w_uu - 2 w_u w_v w_uv + (1 + w_u^2) w_vv) / (2 (1 + w_u^2 + w_v^2)^(3/2)),
       n_i = (-w_u t_1 - w_v t_2 + n^aw) / sqrt(1 + w_u^2 + w_v^2).

   Sign check: the sphere seen from outside, `w = -(u^2 + v^2) / (2 R)`, gives `kappa = 1 / R` (the ADR 0567 convention,
   positive where the surface bends away from its `e_3` side). A stencil of fewer than `JET_MIN_STENCIL = 14` points, a rank
   refusal or `|kappa| h < 1e-8` is refused with the key `"curvature"`, naming the node.

Nodes at distance `d >= 2` keep the ADR 0567 two-sided 2-ring quadric unchanged.

1.1 **Exactness kept on quadrics.** By step 2 the estimator equals the ADR 0567 one wherever the stencils lie on a quadric
(spheres, cylinders): the new option gives the same bits as the default there (check K5).

1.2 **Order on a smooth surface (derivation).** Over the tangent plane the surface is the graph `w = W(u, v)` with `W` smooth;
`W = P_4 + O(|x|^5)` with `P_4` its Taylor polynomial of degree 4 (zero constant, since `W(0) = 0`). In scaled coordinates the
data are `W / h = P_4 / h + O(h^4)`, and the least-squares fit of the 14 monomials, whose scaled matrix has a condition number
independent of `h` for a fixed stencil shape (it depends only on the scaled positions), recovers the scaled coefficients to
`O(h^4)`: `c_2 = h W_uu / 2 + O(h^4)`, so `w_uu = W_uu + O(h^3)`, likewise the gradient to `O(h^4)`. The curvature at a
boundary node is therefore third order on a structured family, whatever the side the stencil lies on. (The ADR 0567 quadric has
no such argument: its eight coefficients do not span the Taylor polynomials.) The angle-weighted frame need not be exact: a
tilt only adds first-order terms that the fit absorbs.

1.3 **Prototype (`estb.log`, `other.log`).** On the torus patch, the new estimator's largest error over **all** nodes:
structured `2.02e-1, 4.17e-2, 9.26e-3, 1.14e-3` at N = 8, 16, 32, 64 (all at distance-0
nodes; distance-1 nodes `3.5e-4` at N = 32), jittered (0.2 cell, the ADR 0567 LCG) `2.13e-1, 4.99e-2, 1.08e-2, 8.19e-3` (at N =
64 the maximum is an interior 2-ring node of the unchanged quadric; the distance-0 nodes `1.31e-3`). Observed boundary order
`log2(9.26e-3 / 1.14e-3) = 3.0`. The boundary normals: `2.1e-4` (N = 32, structured) against `1.5e-3` for the interior
quadric. Rejected (`estb.log`, N = 32 structured / jittered): the cubic jet on the same stencil `3.9e-2 / 5.8e-2` and on three
rings `2.5e-2 / 3.9e-2`; the quartic jet on three rings `1.9e-1`; the quadric (height) jet on two rings `5.8e-2`; the cubic
jet in the quadric's frame `2.7e-2`. Applying the jet at distance 0 only leaves the distance-1 nodes at `6.0e-2` (mutation MU4).
The quartic jet without the residual test on the octant gives `e = 2.8e-2` (n = 16) instead of `3e-14`: hence step 2.

### 2. The torus segment and its membrane solution

**Geometry.** Torus of centre radius `R_0 = 1`, tube radius `r = 0.4` (the ADR 0567 torus), point `x(theta, phi) = (rho cos
theta, rho sin theta, r sin phi)`, `rho = R_0 + r cos phi`; outward normal `n = (cos phi cos theta, cos phi sin theta, sin
phi)`, meridional tangent `e_phi = (-sin phi cos theta, -sin phi sin theta, cos phi)`, hoop tangent `e_theta`. Segment
`theta` in `[0, pi/2]`, `phi` in `[0, pi/4]`. Steel `E = 2e11`, `nu = 0.3`, `t = 0.001`, internal pressure `p = 1e5` (outward,
`+e_3` with the mesh orientation below).

**Membrane forces (derived).** Per radian of `theta`, the vertical equilibrium of the band of the shell between the crown of the tube `phi = pi/2` (where `rho = R_0` and the meridian is horizontal, so the meridional force there has no vertical component) and the parallel `phi`: the vertical component of the meridional force at the parallel, `N_phi cos phi rho`, balances the vertical resultant of the pressure on the band, which is `p` times the band's projection on the horizontal plane, the annulus between the radii `R_0` and `rho`: `p (rho^2 - R_0^2) / 2`. Hence `N_phi = p (rho^2 -
R_0^2) / (2 rho cos phi) = p r (rho + R_0) / (2 rho)` (using `rho - R_0 = r cos phi`). Laplace's relation `p = N_phi / r +
N_theta cos phi / rho` then gives `N_theta = p r / 2`. (Check: `N_phi / r + N_theta cos phi / rho = p (rho + R_0 + r cos phi) /
(2 rho) = p`.)

**Membrane displacements (derived).** `eps_theta = (N_theta - nu N_phi) / (E t)`, `eps_phi = (N_phi - nu N_theta) / (E t)`;
with the normal and meridional displacements `w`, `v`: `rho eps_theta = w cos phi - v sin phi` (the change of the parallel's
radius) and `r eps_phi = dv/dphi + w` (the meridian circle). Eliminating `w`: `d(v / cos phi)/dphi = (r eps_phi - rho eps_theta
/ cos phi) / cos phi`. With the symmetry `v(0) = 0` (which also removes the rigid axial translation, `v = c cos phi`) and the
algebra of the integrand (`R_0 / (rho cos phi) = 1 / cos phi - r / rho`):

    v(phi) = cos(phi) (p r / (2 E t)) [ r atanh(sin phi) - (2 r^2 / sqrt(R_0^2 - r^2)) atan( sqrt((R_0 - r)/(R_0 + r)) tan(phi/2) )
                                        - (1 - 2 nu) R_0 tan phi ],
    w(phi) = ( (p r / (2 E t)) ((1 - nu) rho - nu R_0) + v sin phi ) / cos phi.

Prototype: equal to a 64-point Gauss quadrature of the integral to `2e-16` relative at `phi = 0.1, 0.4, pi/4`. The meridional
rotation (about `e_theta`) of the membrane field is `beta = (dw/dphi - v) / r` (from `d u / ds = ((w' - v) n + (w + v')
e_phi) / r` along the meridian); `dw/dphi` is taken by the central difference of step `1e-5`.

**Boundary conditions.** The cuts `theta = 0` (plane `y = 0`), `theta = pi/2` (`x = 0`) and `phi = 0` (`z = 0`) are symmetry
planes of the torus and of the pressure; at their nodes the displacement normal to the plane and the two in-plane rotation
components are zero (the octant conditions of ADR 0550). The edge `phi = pi/4` is not a symmetry plane. A free edge loaded by
the membrane force `N_phi` was tried first and rejected (`where.py`, `t_clamp.log`): the membrane field has curvature changes
that a free edge cannot carry, and the bending layer it creates is a physical departure from the membrane solution (`1.6e-2`
in `w` at the edge row with the exact pressure, `t = 0.01`, n = 16), not an error of the load. Registered instead: at every
node of `phi = pi/4` all six unknowns are prescribed to the membrane state (`u = w n + v e_phi`, the rotation `beta
e_theta`); the shell then departs from the membrane solution only by the bending of the membrane field in the interior, of
relative order `(t / r)^2`. Prototype with the exact curvature and normal in the load (`t_clamp*.log`): the floor of this
comparison is `1.4e-4` at n = 32, `t = 0.001` (`3.5e-4` at `t = 0.01`, which is why `t = 0.001` is registered).

**Meshes.** `6 n` cells in `theta`, `n` in `phi` (cells of `0.37/n` by `0.31/n` at `phi = 0`), nodes numbered `(n + 1) i + j`,
`i` along `theta`, `j` along `phi`, each cell cut along `q0-q2` (the ADR 0567 torus pattern), structured; `n = 16` and `n = 32`
(the two refinements; 1717 and 6369 nodes). The element and node set `"all"`.

**Measures.** At every node, `w_h = u . n(phi_j)`, `v_h = u . e_phi(phi_j)` (`phi_j = (j / n) pi/4`, the node's exact
parameters); `e_w = max |w_h - w| / max |w|`, `e_v = max |v_h - v| / max |v|` over all nodes (the prescribed edge row has zero
error by construction and is included); order `log2(e(16) / e(32))`.

### 3. API (opt-in)

- `shell3_curvature.rs`: `pub enum CurvatureBoundary { Quadric, Jet }` (`#[non_exhaustive]`, `Default` = `Quadric`, the
  ADR 0567 estimator); `curvature_estimates_with(nodes, tris, boundary)` and `pressure_load_curvature_with(nodes, tris, p,
  boundary)`; the existing `curvature_estimates` and `pressure_load_curvature` keep their signatures and results (they are the
  `Quadric` case); `NodalCurvature` gains the field `jet: bool` (the node took the jet); constants
  `QUADRIC_RESIDUAL_TOLERANCE = 1e-9`, `JET_MIN_STENCIL = 14`.
- `model.rs`: the variant `Load::ShellPressureCurvature` gains the field `boundary: CurvatureBoundary`; the builder
  `Load::shell_pressure_curvature(set, p)` sets `Quadric` (unchanged behaviour); new builder
  `Load::shell_pressure_curvature_with(set, p, boundary)`. The variant is `#[non_exhaustive]`, so code outside the crate cannot
  construct or exhaustively destructure it and is unaffected. `loads.rs`: the arm passes `boundary` on.
- The default path: for `Quadric` the estimates are computed by the unchanged ADR 0567 function; the jet code runs only for
  `Jet`. The load assembly (the boundary term and the scaling of ADR 0567 design 1) is moved into a private helper shared by
  both (its arithmetic and order unchanged); check B1 proves the bits.

### 4. What is new in the tests

One new test file `crates/rcfd-fem/tests/sa_shell_pressure_curvature_torus.rs` with the torus patch of ADR 0567 (copied), the
octant generator of ADR 0550 and the cylinder O1 (copied from `sa_shell_pressure_curvature.rs`), the torus segment of design 2
and every check below.

### 5. Disclosed base measurement

`f656-scratch/base_hashes.log` (main e348eb34, release): FNV-1a 64 over the `to_bits()` little-endian bytes of every entry, in
order, of `nodal_load_vector` with `Load::shell_pressure_curvature("all", 1e5)`, `t = 0.01`: the unstructured octant n = 16
`c79b1b660e85edc7` (918 entries), the cylinder O1 `7f104d16c739c930` (3366), the jittered torus patch N = 16 `30fe5e5c181287d3`
(1734); and over `kappa`, the three normal components and `rings as f64` per node of `curvature_estimates` on the structured
torus patch N = 32: `323bfcb68bd39ddf` (5445 values).

## Checks (limits fixed now)

`Jet` = `Load::shell_pressure_curvature_with("all", p, CurvatureBoundary::Jet)`, `Quadric` = `Load::shell_pressure_curvature`;
the torus segment of design 2 at `t = 0.001` unless stated; the torus patch is that of ADR 0567 design 3.4.

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| M1 | a2 | the membrane solution: the residuals of `r eps_phi = v' + w` and `rho eps_theta = w cos phi - v sin phi` (`v'` by central difference, step `1e-5`) at `phi = j pi / 128`, `j = 0 .. 32`, relative to `max |w|` | `<= 1e-7` | rounding and `O(1e-10)` difference error; quadrature agreement `2e-16` |
| D1 | a1, a2 | segment n = 16, `Jet`: `e_w`; `e_v` | `<= 5e-3`; `<= 4e-3` | `2.36e-3`; `1.83e-3` (`t_clamp_t001.log`) |
| D2 | a1, a2 | segment n = 32, `Jet`: `e_w`; `e_v` | `<= 1e-3`; `<= 1.2e-3` | `4.08e-4`; `5.26e-4` |
| D3 | a2 | `Jet` orders `log2(e(16) / e(32))` for `w` and `v` | `>= 1.5` each | `2.53`; `1.80` |
| Q1 | a1, a2 | segment n = 16, `Quadric`: `e_w`; `e_v` | `<= 4e-2`; `<= 3e-2` | `1.69e-2`; `1.14e-2` (`t_nc.keep.log`) |
| Q2 | a1, a2 | segment n = 32, `Quadric`: `e_w`; `e_v` | `<= 8e-3`; `<= 5e-3` | `3.72e-3`; `2.13e-3` |
| Q3 | a2 | `Quadric` orders for `w` and `v` | `>= 1.5` each | `2.18`; `2.43` |
| D4 | a3 | the boundary estimator lowers the displacement error: `e_w(Jet) < e_w(Quadric)` and `e_v(Jet) < e_v(Quadric)` at n = 16 and 32 | strict, each | ratios 7.1, 6.2 (n = 16); 9.1, 4.0 (n = 32) |
| NC1 | a2 | the default `Load::shell_pressure` on the segment n = 32: `e_w` | `> 2e-2` | `7.40e-2` |
| K1 | a3 | torus patch structured N = 32, `Jet` estimates: `max |kappa / kappa_exact - 1|` over all nodes | `<= 2e-2` | `9.26e-3` |
| K2 | a3 | torus patch jittered (0.2 cell) N = 32, `Jet`: the same | `<= 2e-2` | `1.08e-2` |
| K3 | a3 | torus patch structured N = 64, `Jet`: the same over the nodes with `jet = true`; and the order `log2(K1' / K3)` with `K1'` the N = 32 maximum over the `jet` nodes | `<= 3e-3`; `>= 2.0` | `1.14e-3`; `3.0` |
| K4 | a3, a4 | torus patch N = 32 (structured and jittered): every node of boundary distance `<= 1` (248) has `jet = true` and every other node has `kappa`, normal and `rings` bit-identical to `curvature_estimates` | exact | design 1 step 2 (`resid.log`) |
| K5 | a4 | `Jet` on the octant n = 16 and 32 and the cylinder O1: no node takes the jet, and the load vector is bit-identical to the `Quadric` one | exact | design 1.1 (`rho <= 6.5e-14`) |
| S1 | design 1 | torus patch jittered N = 16, `Jet`: the load of `-p` is the load of `p` negated, bit for bit, two zeros of either sign counting as equal (the lesson of ADR 0567 amendment 1) | exact | the scaling is one multiplication by `p` |
| RF1 | design 1 | a 3 x 2-cell torus patch (the generator of ADR 0567 with 3 cells in `theta` and 2 in `phi`, every node's `rho` changed by `0.01 ((k^2 mod 7) - 3) / 3`, `k` the node number) under `Jet`: refused with key `"curvature"` (stencils of 10 and 11 points, quadric residuals `1.3e-3 .. 2.1e-3`); under `Quadric` accepted | as stated | `rf_proto3.log` |
| B1 | a4 | the four digests of design 5 reproduced on the branch with the default load and `curvature_estimates` | exact | design 3 |
| B2 | a4 | `cargo test -p rcfd-fem --release` (every test binary, the existing ADR 0567 tests unedited) | pass | design 3 |
| B3 | a4 | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | design 3 |
| B4 | a4 | `git diff e348eb34 -- crates/*/src` touches only `rcfd-fem/src/{shell3_curvature.rs, model.rs, loads.rs}`; the lines changed (not added) are: in `model.rs` and `loads.rs` the patterns and constructor of `ShellPressureCurvature` (the new field), in `shell3_curvature.rs` the body of `pressure_load_curvature` moved into the shared helper and the `NodalCurvature` constructor (the new field) | as stated | design 3 |

## Planned mutations (on a scratch copy of the library, one at a time, own `CARGO_TARGET_DIR`; the new test file only)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | the jet of degree 3 (nine coefficients) | K1, K2 (`3.9e-2`, `5.8e-2`) |
| MU2 | the jet never taken (tolerance infinite) | K1, K2, K4, D4 |
| MU3 | the jet always taken at distance `<= 1` (tolerance negative) | K5 |
| MU4 | the jet at distance 0 only | K1, K2, K4 (`6.0e-2`) |
| MU5 | the jet's normal discarded (the angle-weighted normal kept as the node normal) | D1, D2, D4 (`e_w = 3.2` at n = 16) |
| MU6 | the `Jet` option ignored by the load arm of `loads.rs` (the quadric used) | D1, D2, D4 |

## Decision rule

- **done:** M1, D1 to D4, Q1 to Q3, NC1, K1 to K5, S1, RF1, B1 to B4 met in the one registered run, every mutation caught by a
  registered check, no amendment after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the test does not build.

## Limits (not covered)

- L1. The edge `phi = pi/4` is clamped at the membrane state, not free: the load at a free edge of a non-quadric shell is not
  measured by D1 to D4 (the curvature there is, by K1 to K3, since every patch edge is free). The free edge's bending layer is a
  property of the shell, not of the load (design 2).
- L2. Only the outer part of the torus (`|phi| <= pi/4`, positive Gaussian curvature) is used: the membrane solution of the torus
  is singular at the crown `phi = pi/2` (`w` grows as `1 / cos phi`), where no membrane comparison exists.
- L3. Interior nodes keep the ADR 0567 2-ring quadric (second order; its normal error `1.5e-3` at N = 32 on the torus patch is
  larger than the jet's boundary normal error); the jet is not used inside.
- L4. The selection by the quadric residual is a threshold: a stencil that a quadric fits exactly because it has exactly eight
  points, or lies on a quadric by its arrangement (for example three rows of a structured torus grid, which lie on a quadric of
  revolution: `rf_proto.log`), keeps the quadric. Structured and jittered surfaces of the sizes here are far from the threshold
  (design 1 step 2).
- L5. Opt-in only; Shell3 only; dead load only; the default estimator and every default result unchanged.

## Open questions

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):

1. The API: a field on the existing variant with a second builder, rather than a new variant (one load, two estimators; the
   variant is `#[non_exhaustive]`, so nothing outside the crate changes).
2. The meaning of "radial" for the torus: the displacement along the tube's outward normal (the radius of the tube's cross
   section); the radial displacement from the axis, `w cos phi - v sin phi`, follows from it and the meridional one.
3. The edge `phi = pi/4` clamped at the membrane state (design 2), not free.

Not taken:

4. Whether `Jet` should become the default boundary estimator of `Load::shell_pressure_curvature`: it would change the default
   results on non-quadric shells (none in the hash tables, but a default change); recommendation to keep it opt-in until the
   maintainer decides.

## Addendum 1 (2026-11-15): results

Run 1 is commit 9814173d (`f656-scratch/run1.log`, the new test file, release, made once, every check printed). Commit 782f4d2c
then added a type alias that clippy asked for (`type_complexity`, layout only; the test file only); B2 ran on it and includes
the new test file, which passed there again (its numbers are not printed without `--nocapture`; `b2.log`). The mutations ran on a `git archive` copy of
782f4d2c (`f656-scratch/mutate.py`, `mutlogs/`, own target directory). No amendment was written.

| id | limit | measured | met |
| --- | --- | --- | --- |
| M1 | `<= 1e-7` | `1.30e-11` | yes |
| D1 | `<= 5e-3`; `<= 4e-3` | segment n = 16, `Jet`: `e_w = 2.3635e-3`, `e_v = 1.8323e-3` | yes |
| D2 | `<= 1e-3`; `<= 1.2e-3` | n = 32: `e_w = 4.0844e-4`, `e_v = 5.2585e-4` | yes |
| D3 | `>= 1.5` each | orders `2.53` (`w`), `1.80` (`v`) | yes |
| Q1 | `<= 4e-2`; `<= 3e-2` | n = 16, `Quadric`: `1.6874e-2`, `1.1440e-2` | yes |
| Q2 | `<= 8e-3`; `<= 5e-3` | n = 32: `3.7167e-3`, `2.1281e-3` | yes |
| Q3 | `>= 1.5` each | `2.18`, `2.43` | yes |
| D4 | strict | `Jet` below `Quadric` by the factors 7.14, 6.24 (n = 16), 9.10, 4.05 (n = 32) | yes |
| NC1 | `> 2e-2` | default `Load::shell_pressure` n = 32: `e_w = 7.4029e-2` (`e_v = 1.9168e-2`) | yes |
| K1 | `<= 2e-2` | torus patch structured N = 32, all nodes: `9.2628e-3` (ADR 0567: `1.3573e-1`) | yes |
| K2 | `<= 2e-2` | jittered N = 32: `1.0810e-2` (ADR 0567: `1.3742e-1`) | yes |
| K3 | `<= 3e-3`; `>= 2.0` | N = 64 jet nodes `1.1360e-3`; order `3.03` | yes |
| K4 | exact | 248 jet nodes = the nodes of distance `<= 1`, structured and jittered; every other node bit-identical | yes |
| K5 | exact | octant n = 16, 32, cylinder O1: 0 jet nodes, load bits equal | yes |
| S1 | exact | `true` (867 non-zero entries of 1734) | yes |
| RF1 | as stated | `Jet` refused, key `"curvature"` (node 0: 11 points, fewer than 14); `Quadric` accepted | yes |
| B1 | exact | the four digests `c79b1b660e85edc7`, `7f104d16c739c930`, `30fe5e5c181287d3`, `323bfcb68bd39ddf` reproduced | yes |
| B2 | pass | `cargo test -p rcfd-fem --release` on 782f4d2c: 280 test binaries, 0 failed (`b2.log`) | yes |
| B3 | pass | baseline_hashes 3 passed, 1 ignored (the existing record); docs_site 2 passed (with the `docs/structural-solver.md` bullet in the tree; `b3.log`) | yes |
| B4 | as stated | `git diff e348eb34 -- crates/*/src` (`b4.diff`) touches only the three files named. Changed lines: `model.rs` the validation pattern (`{ set, pressure, .. }`); `shell3_curvature.rs` the `NodalCurvature` constructor and the tail of `pressure_load_curvature` (moved into `assemble`); `loads.rs` the pattern of the arm, **and two lines the registration did not name**: the call `add_shell_pressure_curvature(.., *pressure)` (now also passing `*boundary`) and the `for` line over `pressure_load_curvature` (now a `match` on `boundary` whose `Quadric` arm calls the same function with the same arguments) | **no, as worded**: the two lines are the "arm passes `boundary` on" of design 3, but B4's list omitted them |

The prototype numbers are reproduced to three or four digits everywhere (the prototype solves with numpy's SVD least squares
and a block-tridiagonal solve; rcfd with Householder QR and its direct solver).

Mutations (all caught; `mutlogs/`):

| id | caught by |
| --- | --- |
| MU1 (cubic jet) | K1 `3.91e-2`, K2 `5.83e-2`, K3 `9.31e-3`, D1, D2, D4 (n = 16 `e_w`: the cubic is worse than the quadric there) |
| MU2 (jet never taken) | K1, K2 (the ADR 0567 values), K3 (no jet nodes: order NaN), K4, D1, D2, D4 (ratio 1), RF1 (not refused) |
| MU3 (jet always taken) | K5 (92, 200 and 184 jet nodes; bits differ) |
| MU4 (jet at distance 0 only) | K1 `5.98e-2`, K2 `7.10e-2`, K4, D1, D2 |
| MU5 (jet normal discarded) | D1 (`e_w = 3.24`), D2, D3, D4 |
| MU6 (the option ignored by `loads.rs`) | D1, D2, D4 (ratio 1), RF1 (not refused) |

fmt (`cargo fmt -p rcfd-fem --check`) clean; clippy (`cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`) clean
after the alias of 782f4d2c (`clippy.log` shows the one `type_complexity` error before it, `clippy2.log` clean). Not run: the
whole workspace suite (the lead's gate). Files: `crates/rcfd-fem/src/shell3_curvature.rs` (the enum, two constants, the field,
`assemble`, `stencil`, `least_squares_n`, `local_points`, `quadric_residual`, `jet_estimate`, `estimates_with`,
`curvature_estimates_with`, `pressure_load_curvature_with`), `model.rs` (the variant's field, the builder
`shell_pressure_curvature_with`, the validation pattern), `loads.rs` (the arm and `add_shell_pressure_curvature`), the new test
file `crates/rcfd-fem/tests/sa_shell_pressure_curvature_torus.rs`, this ADR, and a bullet in `docs/structural-solver.md`.

**Decision: partial**, by the rule: every acceptance clause is met (the torus segment at two refinements with errors `4.1e-4`
and `5.3e-4` and orders `2.5` and `1.8` with the boundary estimator, `3.7e-3` and `2.1e-3`, orders `2.2` and `2.4` with the
ADR 0567 load; the boundary-node curvature error `1.36e-1` to `9.26e-3` at 32 x 32; defaults bit-identical by B1, K4, K5, B2,
B3), every mutation caught, no amendment; but the diff-scope check B4 is missed as worded (two lines of `loads.rs` that pass
the new field on were not in its list). The miss is in the registration's wording, not in the bits.

Remaining questions (options, recommendation):

1. B4: (a) keep partial; (b) the maintainer accepts the two `loads.rs` lines as covered by design 3 ("the arm passes
   `boundary` on") and by B1, K5 and B2 (the bits). Recommend (b) if the maintainer agrees that the diff check's purpose (no
   default path changed) is shown by the digests; otherwise (a).
2. The default boundary estimator: (a) keep `Quadric` as the default of `Load::shell_pressure_curvature`; (b) make `Jet` the
   default (it keeps the ADR 0567 bits wherever ADR 0567 is exact, by K5, but changes the default result on non-quadric
   shells). Recommend (a) now; (b) is a maintainer decision (open question 4).
3. A free edge of a non-quadric shell under the curvature load (L1): (a) leave; (b) a follow-up against a shell (not membrane)
   reference with the edge free. Recommend (a): no user case asks for it yet.
4. The interior quadric's normal (L3, `1.5e-3` on the torus at N = 32, larger than the jet's boundary normal): (a) leave; (b)
   measure whether the jet at every node improves the torus segment further. Recommend (a), low priority.

## For the lead: roadmap entries

> Proposed status: partial.

> Structural solver, advanced tier. Follow-up of F-633 (ADR 0567, 2026-11-15). PARTIAL. ADR 0579, 2026-11-15: opt-in boundary curvature estimator for the curvature shell pressure: `Load::shell_pressure_curvature_with(set, p, CurvatureBoundary::Jet)` replaces the ADR 0567 quadric at the nodes of boundary distance 0 and 1 (one-sided stencils) by a quartic height jet fitted over the same stencil in the angle-weighted frame (third order at the boundary, derived), unless the quadric fits that stencil to a relative residual of 1e-9 (then the surface is a quadric there and the ADR 0567 estimate is kept: on the octant and the cylinder O1 the option gives the default's bits). Torus patch of ADR 0567 at 32 x 32: boundary-node curvature error 1.36e-1 to 9.26e-3 (jittered 1.37e-1 to 1.08e-2; 1.14e-3 at 64 x 64, order 3.0). Pressurised torus segment (R0 = 1, r = 0.4, t = 0.001, theta in [0, pi/2], phi in [0, pi/4], 6n x n Shell3 cells, symmetry cuts at theta = 0, pi/2 and phi = 0, the edge phi = pi/4 clamped at the membrane state) against the membrane solution derived in the ADR: normal and meridional displacement errors with the jet 2.36e-3, 1.83e-3 (n = 16) and 4.08e-4, 5.26e-4 (n = 32), orders 2.5 and 1.8; with the ADR 0567 quadric (`Load::shell_pressure_curvature`) 1.69e-2, 1.14e-2 and 3.72e-3, 2.13e-3, orders 2.2 and 2.4; the default pressure 7.4e-2. Defaults bit-identical (four digests of the base reproduced, the rcfd-fem suite, baseline_hashes, docs_site pass); six of six mutations caught. Partial: the diff-scope check B4 missed as worded (two loads.rs lines that pass the new field on were not in its list); every acceptance clause met.

No follow-up is proposed (remaining questions 3 and 4 recommend leaving the free edge and the interior normal until a case asks for them).
