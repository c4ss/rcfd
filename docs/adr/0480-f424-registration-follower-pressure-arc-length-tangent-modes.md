# ADR 0480: F-424 registration: follower pressure with its load stiffness, arc-length continuation in the Newton driver, and modes about a nonlinear equilibrium

Status: **REGISTERED 2026-11-14, written before any code of F-424 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-424 was written or compiled in the repository except one scratch probe that is not
committed (disclosed in arithmetic 5: a test file that called the existing `nonlinear::internal_force` on the two-bar truss of D6
at closed-form states, then was deleted; its source and log are `w49i-work/f424-scratch/zz_probe_f424.rs` and `probe.log`), and a
plain-Python shooting integration of the thick ring (`f424-scratch/ring_ode.py`, no rcfd code, log in `probe.log`). The bit record
N1 of the default paths is a test file written and run on the base after this commit and before any library code (D8). Results go
in the addenda at the end; nothing above the addenda is changed after a run.

Registration of the stub [ADR 0327](0327-follower-pressure-arc-length-modes-about-a-nonlinear-state.md) (work package SA-17 of
the plan [ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Roadmap item F-424 (deps F-408, F-395, F-396, F-397). Number
0480 given by the lead. Branch `f424-work` at 4c422025, worktree `w49i-work/f424`, scratch `w49i-work/f424-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited except the records-and-copies scheme of ADR 0291 in `sa_scaffold.rs`; clean-room.

## What F-424 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Follower pressure: the load stiffness agrees with central differences of the load within 1e-6. Thin ring (thickness/radius 0.02, nu = 0) under internal pressure up to the stretch 1.5: p = E t (lambda^2 - 1)/(2 R) within 1 percent, the dead pressure on the same mesh being the registered control (off by the factor lambda). Linear buckling of the ring under external pressure with the load stiffness: 3 E I/R^3 within 2 percent; without it the registered control 4 E I/R^3 within 3 percent. Arc-length: a two-bar truss of solid bars (nu = 0, height h, bar length L0) is followed to the apex displacement 2.2 h with P = E A (h - w)(2 h w - w^2)/L0^3 within 1e-6 at the registered points and the limit load 2 E A h^3/(3 sqrt(3) L0^3) within 0.5 percent, load control failing at the first limit point (control). Modes about a state: a clamped-clamped beam stretched by an end displacement has the tension frequencies of F-395 within 1 percent, and at a small stretch the tangent modes equal the linear pre-stressed modes within 1e-4; for the rotating radial beam at 0.2 w_0 they equal F-396 within 1e-4, and the difference at higher speed is recorded.

The brief adds: (b1) reuse the Newton driver of `nonlinear.rs` and the buckling and pre-stressed modal paths; no second Newton
loop, arc-length goes into the driver as an option; (b2) another agent (F-423) edits `nonlinear.rs` at the same time: the code
goes into `follower.rs` (and `arclength.rs`) where it can, any edit of `nonlinear.rs` small and local; (b3) check how the
two-bar closed form is stated, derive what "within 1e-6" means for solid bars and probe before registering it; (b4) models that
do not use the new options keep their bits, `baseline_hashes` unchanged, a default-bits test records digests at the base
before any library change; (b5) a case-file key is the maintainer's call (recommended, not added); (b6) probe before a
`1e-12`-class limit, `1e-11` for sums over about `1e3` terms; (b7) the records-and-copies scheme of `sa_scaffold.rs`
(`x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432` becomes an ignored record citing ADR 0291, an `_after_f424` copy
added).

Clauses: (a0) every limit registered here; (a1) load stiffness against central differences of the load, `1e-6`; (a2) thin ring
under internal follower pressure up to the stretch 1.5, the thin-ring law within 1 percent; (a2c) the dead pressure as the
control, off by the factor `lambda`; (a3) ring buckling with the load stiffness, `3 E I/R^3` within 2 percent; (a3c) without it,
`4 E I/R^3` within 3 percent; (a4) the two-bar truss followed to `w = 2.2 h`, the closed form within `1e-6` at the registered
points; (a5) the limit load within 0.5 percent; (a4c) load control fails at the first limit point; (a6) clamped-clamped beam
stretched by an end displacement, tension frequencies of the F-395 path within 1 percent; (a7) at a small stretch, the tangent
modes equal the linear pre-stressed modes within `1e-4`; (a8) the rotating radial beam at `0.2 omega_0`, the tangent modes equal
F-396 within `1e-4`; (a9) the difference at higher speed recorded.

Readings, registered:

- **(a1)** S1: on one element of each solid kind (Hex8, Hex20, Tet4, Tet10; the faces Quad4, Quad8, Tri3, Tri6) with a
  pressure on one face, at a deformed state (nodal displacements of size `0.05` of the element, seed `0xF424_0480`), the full
  (unsymmetric) load stiffness `K_p = d f_p / d u` of D1 against central differences of the follower load `f_p(u)` with the step
  `h = 1e-6` times the element size, measure `max_ij |K_p - K_cd| / max_ij |K_p|`. Every face here has free edges, so this is the
  full matrix, not its symmetric part.
- **(a2)** R1: the quarter ring of D5 under the internal follower pressure `p_ref` in 20 equal load steps; at each converged
  step the mean stretch `lambda` (the mean over the nodes of the reference mid-surface `|X| = R` of `|x|/R`) and the applied
  pressure `p`; the thin-ring law with `R` the mean radius and `t` the thickness. Measure `|p / p_thin(lambda) - 1|`. The check
  is made at the steps with `lambda >= 1.2` (14 steps by arithmetic 2) and at the last one, whose stretch must reach 1.5 (R4);
  the steps below 1.2 are recorded (R1b), because the thin-ring law itself departs from the thick ring of `t/R = 0.02` by up to
  `+1.01` percent as `lambda -> 1` (arithmetic 2: the law has an `O(t/(2 R lambda))` error that the acceptance's 1 percent does
  not cover near `lambda = 1`). R2, the companion: the pressure against the exact plane solution of the thick SVK ring of
  `nu = 0` at the same outer radius (arithmetic 2, the shooting integration written in the test), at every step, `1e-3`.
- **(a2c)** R3: the same mesh and `p_ref` as a dead pressure (`Load::Pressure`, the existing path), last step: the measured `p`
  over `lambda p_thin(lambda)` within 2 percent of 1 (the factor `lambda`, arithmetic 2 gives `+1.02` percent), and `p /
  p_thin(lambda) - 1 >= 0.3` (arithmetic 2 gives `+0.40`): off by the factor `lambda`. R2 also covers the dead run against its own
  exact solution.
- **(a3)** B1: the quarter ring of D5 (24 elements over the quarter) under the external pressure `p_ref` on the outer surface:
  the pre-stress of the linear static solve with the dead pressure, the buckling factors of `(K + mu (K_sigma - K_p,sym)) x = 0`
  (D3), `p_cr = mu_1 p_ref` against `3 E I / R^3` with `I = t^3/12` per unit width and `R` the mean radius. Measure `|p_cr / (3 E
  I/R^3) - 1|`, limit 2 percent.
- **(a3c)** B2: the same with the existing `buckling::factors` (no load stiffness): `|p_cr / (4 E I/R^3) - 1|`, 3 percent.
- **(a4)** A1: the two-bar truss of D6 traced by arc-length (D4) from the unloaded state until the first point with `w >= 2.2
  h`; the "registered points" are every converged point of the trace. Measure `|lambda P_ref - P(w)| / P_max` with `w = -u_y` of
  the apex, `P_max = 2 E A h^3/(3 sqrt(3) L0^3)`, `A` the effective area of D6. Relative to `P_max` because `P(w)` passes through
  zero at `w = h` and `w = 2 h`.
- **(a5)** A3: the largest `lambda P_ref` over the points with `w < h` against `P_max`, `0.5` percent; no interpolation.
- **(a4c)** A6: load control (`nonlinear::solve`) of the same truss to `1.2 P_max` in 10 steps with the default halvings either
  returns `FemError::Solver` with the largest converged load factor (recorded by a `StepControl` hook that returns the driver's
  proposal unchanged) in `[0.97, 1.0] P_max` (the path is lost at the first limit point), or converges with a final `w > h (1 +
  1/sqrt 3)` (a snap through to the far branch: the descending branch is not followed). Either is "load control failing at the
  first limit point"; anything else is a miss.
- **(a6)** M1: the Hex20 beam of F-395 (`L = 2, H = B = 0.1`, `40 x 2 x 1`, `nu = 0`, plane), clamped at both ends, stretched by
  `u_x = eps L` at `x = L` with `eps = 1e-3`: the nonlinear state (D2) and its tangent modes against the F-395 path, the
  pre-stressed modes of the same mesh with the stress of the linear static solve of the same end displacement (`prestressed_modes`
  needs zero prescribed values, so the modal model is the same model with the end value 0). Modes 1 and 2, `|f_T / f_L - 1|`,
  1 percent.
- **(a7)** M2: the same at `eps = 1e-5`, `1e-4`.
- **(a8)** M3: the radial beam F of F-396 (`si_rotating.rs`: Hex20 `20 x 2 x 2`, `1 x 0.05 x 0.05`, clamped at `x = 0`, about
  `z`), at `Omega = 0.2 omega_0` (`omega_0` the lowest rest frequency): the nonlinear state under the centrifugal load with its
  deformation-dependent part (D2), its tangent modes against `RotatingPart::modes(Omega)` with the options of `si_rotating.rs`
  (`beam_options`), modes 1 to 4, `|f_T / f_F - 1|`, `1e-4`.
- **(a9)** M4: the same at `0.5, 1, 2 omega_0`, recorded (no limit).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0327 (stub), ADR 0310 (the SA-17 section, the module table, the driver extension points),
  ADR 0440 (D7 the contributions and the step-control hook, open point 2 on arc-length), ADR 0475 for the form of a
  registration; the F-424 entry in `docs/roadmap.json` and `docs/ROADMAP.md`.
- `crates/rcfd-fem/src/nonlinear.rs` (whole), `buckling.rs` (whole to the transformation path), `geometric.rs` (the
  `Prestress` type, `geometric_stiffness`, `prestressed_modes`), `rotating.rs` (head, `spin_softening_mass`), `modal.rs`
  (options, result, `from_parts`), `loads.rs` (`nodal_load_vector`, `add_face_load`, `face_shape_functions`, `face_rule`),
  `builders.rs` (`hex_box`), `model.rs` (`Load`, the `check_*` family), `lib.rs`, `follower.rs` and `arclength.rs` (stubs),
  `rcfd-linalg/src/skyline.rs` (options, pivot rule, symmetry test).
- Tests: `si_geometric.rs` (beam B, A6), `si_rotating.rs` (beam F, `beam_options`, `omega_0`), `sa_gyro.rs` (the mapped ring
  sector), `sa_scaffold.rs` (the X1 records), `sa_j2_default_bits.rs` (the form of a bit record).
- The worktree of F-423 (`w49i-work/f423`, read only) for its edits of `nonlinear.rs`, so that D4 touches other lines.

## Arithmetic done before the code (disclosed)

1. **Follower pressure and its load stiffness** (first principles; the linearisation of the pressure's virtual work as in
   Hibbitt, "Some follower forces and load stiffness", Int. J. Numer. Meth. Engng 14 (1979) 937 (the reference itself
   uncertain), and Bonet and Wood, Nonlinear Continuum Mechanics for Finite Element
   Analysis, Cambridge 1997, chapter 6, both cited from memory, NOT read). A face with nodes `x_a = X_a + u_a`, functions
   `N_a(u, v)` and derivatives `(N_a,u, N_a,v)` (the parametrisation of `loads::face_shape_functions`, whose `x_u x x_v` points
   out of the solid): the pressure `p` (positive pressing on the solid) on the current face gives `f_a = -p int N_a (x_u x x_v) du
   dv`, evaluated with `loads::face_rule` exactly as the dead `Load::Pressure` is (`loads.rs` `add_face_load`), so that at `u = 0`
   the two are the same operations. Since `x_u = sum_b N_b,u x_b`, `d(x_u x x_v)/d x_bj = N_b,u (e_j x x_v) + N_b,v (x_u x
   e_j)`, and `K_p,(ai)(bj) = d f_ai / d x_bj = -p int N_a [N_b,u (e_j x x_v) + N_b,v (x_u x e_j)]_i`. `x_u x x_v` is bilinear in
   the nodal positions, so `f_p` is a quadratic polynomial of `u` and its central difference is exact up to rounding (the third
   derivative vanishes): the error of S1 is rounding only, `~ eps |f| / h / |K| ~ 1e-16 L / (1e-6 L) = 1e-10`. The limit `1e-6`
   of (a1) is four decades above. On the quadrature: the integrand of `K_p` of the 8-node face has degree at most `2 + 1 + 2 = 5`
   in each parametric variable (and the force the same), so the 3 x 3 Gauss rule is exact for both.
2. **The thin ring and the thick ring** (first principles). Thin ring, mean radius `R`, thickness `t`, `nu = 0`, St.
   Venant-Kirchhoff (the linear material of the total Lagrangian driver, ADR 0404): with `nu = 0` the radial and axial
   Green-Lagrange strains vanish under zero radial and axial stress, so `t` and the width stay; the hoop stretch `lambda`, `S =
   E (lambda^2 - 1)/2`, the hoop force per unit width `N = t lambda S`; equilibrium on the deformed radius `lambda R` with the
   pressure on the current surface: `N = p lambda R`, `p = E t (lambda^2 - 1)/(2 R)`, the acceptance's law. The dead pressure acts
   on the reference length: `N = p R`, `p = lambda E t (lambda^2 - 1)/(2 R)`, off by `lambda`. Thick ring, exact (plane, `nu = 0`,
   SVK): `r(R)`, `P_rR = r' (r'^2 - 1) E/2`, `P_tt = (r/R)((r/R)^2 - 1) E/2`, `d(R P_rR)/dR = P_tt`, `P_rR(R_o) = 0`; on the bore
   `P_rR(R_i) = -p r_i/R_i` (follower: Nanson, the current area per reference area of a radial face is `r/R`) or `-p` (dead).
   Integrated from `R_o` inwards with `r(R_o) = r_o` given (RK4, 2000 steps; `r'` from the cubic by Newton), so `p` is explicit:
   `p_F = -R_i P_rR(R_i)/r_i`, `p_D = -P_rR(R_i)`. Probe (`ring_ode.py`, `probe.log`), `t/R = 0.02`, `lambda` the stretch of the
   mid-surface:

   | lambda | p_F / p_thin - 1 | p_D / p_thin - 1 | p_D / (lambda p_thin) - 1 |
   |---|---|---|---|
   | 1.001 | +1.014e-2 | +1.117e-2 | +1.015e-2 |
   | 1.051 | +9.66e-3 | +6.12e-2 | +1.016e-2 |
   | 1.101 | +9.22e-3 | +0.1122 | +1.017e-2 |
   | 1.202 | +8.44e-3 | +0.2143 | +1.018e-2 |
   | 1.303 | +7.78e-3 | +0.3163 | +1.020e-2 |
   | 1.404 | +7.21e-3 | +0.4184 | +1.021e-2 |
   | 1.505 | +6.71e-3 | +0.5204 | +1.023e-2 |

   The deviation of the follower case is `~ t/(2 lambda R)`: the bore radius `r_i = lambda R - t/2` carries the pressure while
   the law uses `lambda R`. With `p_ref = E t (1.51^2 - 1)/(2 R)` (registered), step `k` has `p = (k/20) p_ref`; the stretch
   reaches `lambda^2 = 1 + 1.2801/1.0067`, `lambda = 1.507` at step 20 (R4), and `lambda >= 1.2` from step 7 on. The dead run with
   the same `p_ref` ends at `lambda (lambda^2 - 1) 1.0102 = 1.2801`, `lambda = 1.384`, where `p / p_thin - 1 = +0.398` and `p /
   (lambda p_thin) - 1 = +1.02e-2`. The FE error against the exact ring (R2): Hex20 with one element through `t` interpolates the
   radial profile quadratically (the exact `r(R)` is nearly linear over `t`), the arc is the quadratic isoparametric arc of 8
   elements over 90 degrees (a radius error of order `(pi/16)^4 / 100 ~ 1e-5`); estimate `<= 1e-4`, limit `1e-3`.
3. **Ring buckling, the factors 3 and 4** (first principles; the results as in Timoshenko and Gere, Theory of Elastic
   Stability, 1961, chapter 7, cited from memory, NOT read; re-derived). Inextensional ring, radial `w` (outward), tangential `v`,
   mode `w = a cos n theta`. Inextensibility to second order: `(v' + w)/R + ((w' - v)/R)^2/2 = 0`. Bending energy `U = (E I/(2
   R^3)) int (w'' + w)^2 dtheta = (pi a^2/2)(E I/R^3)(n^2 - 1)^2`. Hydrostatic (follower) external pressure: the potential `p
   Delta A`, `A = (1/2) int [(R + w)(R + w + v') - v w' + v^2] dtheta`; with the second-order constraint `R int w = -(1/2) int
   (w' - v)^2` and `v' = -w` at first order, `Delta A_2 = (1/2) int (v^2 - w^2) - (1/2) int (w' - v)^2 = -(pi a^2/2)(n^2 - 1)`;
   `Pi_2 = (pi a^2/2)(n^2 - 1)[(n^2 - 1) E I/R^3 - p]`, `p_cr = (n^2 - 1) E I/R^3`, `3 E I/R^3` at `n = 2`. Pressure of fixed
   direction (dead): the potential of the fixed forces `p R int w dtheta = -(p/2) int (w' - v)^2 = -(pi a^2/2) p (n^2 - 1)^2/n^2`,
   `p_cr = n^2 E I/R^3`, `4 E I/R^3` at `n = 2`. The quarter ring with symmetry planes at `theta = 0` and `90` degrees admits only
   even `n`, so `n = 2` is the lowest for both. Expected departure of the solid model: the pressure acts on the outer radius `R_o
   = R + t/2`, the hoop force is `p R_o`: `p_cr` lower by `R/R_o`, `-1.0` percent, both cases; the through-thickness distribution
   and the 24 Hex20 over the quarter (the half wave of `n = 2` is the quarter) add a few tenths of a percent. Estimates: B1 `-1.0 +-
   0.5` percent (limit 2), B2 the same (limit 3). The load stiffness is used by its symmetric part (D3); the antisymmetric part of
   a pressure's load stiffness is a boundary term on the edges of the loaded surface, `p int (delta u x Delta u) . dx` along the
   edges (integration by parts of arithmetic 1, exact under the exact quadrature of arithmetic 1). On the ring's edges: along `z`
   on the symmetry planes, `(delta u x Delta u) . e_z = delta u_x Delta u_y - delta u_y Delta u_x` with one of `u_x` (plane `x = 0`)
   or `u_y` (plane `y = 0`) held; along `theta` at `z = 0, b`, `(delta u x Delta u) . e_theta = 0` for in-plane motions (`u_z`
   held). So the antisymmetric part couples only held unknowns and the reduced `K_p` is symmetric (S2: measured after the
   reduction, rounding only, the entries are sums of 9 quadrature terms of at most 2 faces, limit `1e-11` of `max |K_p|`). On a
   free edge it is not (S3, the control).
4. **The two-bar truss** (first principles; the shallow two-bar truss as in Crisfield, Non-linear Finite Element Analysis of
   Solids and Structures, vol. 1, Wiley 1991, chapter 1, cited from memory, NOT read). Supports `(+-a, 0)`, apex `(0, h)`, `L0^2
   = a^2 + h^2`, apex moved down by `w`: `L^2 = a^2 + (h - w)^2 = L0^2 - (2 h w - w^2)`, Green-Lagrange strain `E_GL = (L^2 -
   L0^2)/(2 L0^2) = -(2 h w - w^2)/(2 L0^2)`, SVK `S = E E_GL`, bar force (first Piola-Kirchhoff times the reference area) `N = A S
   L/L0`, its vertical component `N (h - w)/L = A S (h - w)/L0`; two bars: `P = E A (h - w)(2 h w - w^2)/L0^3`. **This is not the
   shallow form: it is exact for bars of Green-Lagrange strain and SVK stress**, at any depth (the brief's reading checked: the
   shallow approximation is not used anywhere). `dP/dw = 0` at `3 w^2 - 6 h w + 2 h^2 = 0`, `w_1,2 = h (1 -+ 1/sqrt 3)`, `P(w_1) =
   2 E A h^3/(3 sqrt 3 L0^3) = P_max`, `P(w_2) = -P_max`. For a solid bar with `nu = 0` the same law holds when each bar deforms
   homogeneously: `F = R (lambda a0 (x) a0 + I - a0 (x) a0)`, `R` the rotation of the bar, `lambda = L/L0`; the lateral
   Green-Lagrange strains are zero, so the lateral stresses vanish and the area stays. Whether the homogeneous state is the FE
   equilibrium depends on the ends (5).
5. **What "within 1e-6" means for solid bars; the probe** (b3). A prismatic solid bar cannot be pinned at a point: its end face
   must rotate with the bar, and a held end face bends the bar (a frame: the vertical force of the end moments is `~ 12 E I w
   /L^3`, against `P_max` a relative `~ 6 (t/h)^2`, `6e-4` at `t/h = 0.01`; far above `1e-6`); a pin at one node of the end face
   gives a point load with an `O(t/L)` local compliance (`1e-3` at `t/L = 0.01`). The FE equilibrium is the homogeneous state
   exactly only if every free node other than the two tips carries zero internal force at it: for uniform `S = S_aa a0 (x) a0`
   the force at node `m` is `F S int grad N_m dV = lambda R a0 S_aa int_dOmega N_m (a0 . n) dA`. The bar of D6 is a bipyramid: two
   pyramids on a square cross-section at mid-length, tips at the support and the apex; for a node of the square, the faces on the
   support side and on the apex side are mirror images, so `int N_m (a0 . n) dA` cancels; the tips carry `S_aa (s^2/3)`. The
   bipyramid is a bar of effective area `A = V/L0 = s^2/3` (`V` its volume), pinned at points (its tips), with the homogeneous
   state as an exact FE equilibrium. Probe (`zz_probe_f424.rs`, `probe.log`: `L0 = 1, h = 0.2, s = 0.1, E = 2e11`, Tet4, 4
   tetrahedra per bar around its axis, `u_z = 0` everywhere): at the closed-form homogeneous states of `w/h = 0.05, 0.4226, 1, 1.5,
   2.2` the internal force at the apex equals `-P(w)` with `A = s^2/3` and every free unknown balances, to `2.1e-12 .. 7.2e-12
   P_max` (with `A = s^2` the apex carries exactly one third: the effective area). So for these bars "within 1e-6" is a check of the
   kinematics, the material and the continuation to the Newton tolerance: the driver's relative residual `1e-10` against a
   reference that includes the support reactions (about `5 P_max`) leaves `~ 5e-10 P_max`; estimate `<= 1e-9`, limit `1e-6`.
6. **The limit load from the points** (A3). The trace of D6 (arc length `0.05 h` in the norm of D4) has, near `w_1`, a step `Dw ~
   0.05 h / sqrt 3 ~ 0.029 h` (the apex and the eight square nodes, which move about half as much, share the norm: `|Du| ~
   sqrt 3 Dw` where `Dlambda ~ 0`). The largest point is at most `Dw/2` from `w_1`: `P_max - P = (P''/2)(Dw/2)^2`, `P'' = 6(w - h)
   E A/L0^3 = -3.46 h E A/L0^3`, a relative `<= 9.4e-4` (limit `5e-3`).
7. **Modes about a state, estimates.** M1, M2 (`eps` end stretch, `nu = 0`, the homogeneous state `F = diag(1 + eps, 1, 1)`):
   the tangent `K_mat(F) + K_sigma(S)` against `K + K_sigma(E eps)`. In the bending energy the fibre strain variation is `(1 +
   eps) delta u_x,x`, so the material part is stiffer by `(1 + eps)^2`, `+2 eps` of the bending share (about 0.89 of `omega_1^2`
   at `eps = 1e-3`, the tension share `~ 118 eps` by the Rayleigh quotient of the clamped mode); `S = E (eps + eps^2/2)` against
   `E eps`, `+eps/2` of the tension share. Frequencies: `+9e-4` at `eps = 1e-3` (limit `1e-2`), `+1e-5` at `1e-5` (limit `1e-4`).
   M3: `omega_0 = 1.875^2 sqrt(E H^2/(12 rho)) / L^2 = 256.2` rad/s, `Omega = 51.2` rad/s; the centrifugal strain at the root
   `rho Omega^2 L^2/(2 E) = 5.15e-5`; the same `(1 + eps)^2` factor weighted by the curvature energy (near the root for the first
   pair, about `0.8 eps_root`) gives `+8e-5` on `omega^2`, `+4e-5` on the frequency of modes 1 and 2, less for 3 and 4; the
   deformation-dependent centrifugal load (D2) raises the axial stress by `Omega^2 / omega_axial^2 ~ 4e-5` of the stiffening
   share (`~5` percent): `2e-6`. Estimate `<= 5e-5` (limit `1e-4`; the margin is a factor 2, disclosed).

## Design

### D1. Follower pressure (`follower.rs`)

`FollowerPressure::new(model, set, pressure) -> Result<FollowerPressure>`: the faces of a surface set of solid elements (Hex8,
Hex20, Tet4, Tet10; a set or element of another kind refused naming it), each with its nodes, their three translational
equations and reference coordinates; a non-finite pressure refused (key `"pressure"`). Methods: `force(u, load_factor) ->
Vec<f64>` the load vector `f_p(u)` of arithmetic 1 times the load factor (one value per equation of the model's `DofMap`);
`load_stiffness(u, load_factor) -> Vec<(usize, usize, f64)>` the full `d f_p / d u` as triplets (unsymmetric); and the
`nonlinear::Contribution` impl: `add` subtracts `f_p(u)` from the force (an external load that depends on `u` enters with a
minus sign, ADR 0440 D7) and adds the triplets of `-(K_p + K_p^T)/2` of each face (the symmetric part, exact under the condition
of arithmetic 3; elsewhere the Newton iteration converges at a reduced rate to the same solution, the residual being exact). The
face arithmetic is that of `loads::add_face_load` (same rule, same functions, same order), so `force(0, 1)` is the vector of the
dead `Load::Pressure` on the same set, bit for bit (S4).

### D2. Deformation-dependent centrifugal load (`follower.rs`)

`CentrifugalFollower::new(model, threads)`: the model's single `Load::Centrifugal { axis, origin, omega }` (none or two refused,
key `"centrifugal"`); `M_perp = rotating::spin_softening_mass(model, axis, threads)` (ADR 0403 D1). The body force on the
deformed position is `rho Omega^2 (X + u - p)_perp`; its consistent load is the model's dead centrifugal load (on `X`) plus
`Omega^2 M_perp u`, because the dead load is integrated with the mass rule of the elements on the interpolated position and
`M_perp` with the same rule. The contribution: force `-lambda Omega^2 M_perp u`, tangent `-lambda Omega^2 M_perp` (the spin
softening of F-396 as the load stiffness). With the load factor scaling `Omega^2` (the driver scales the dead load the same
way).

### D3. Buckling with the load stiffness (`follower.rs`, `buckling.rs`)

`follower::buckling_factors(model, prestress, pressures: &[&FollowerPressure], options) -> Result<BucklingResult>`: the extra
matrix `-K_p,sym` of each pressure at `u = 0`, load factor 1, added to `K_sigma`: `(K + mu (K_sigma - K_p,sym)) x = 0`, by the
existing path. `buckling.rs` gets one `pub(crate) fn factors_with(model, prestress, extra: &[(usize, usize, f64)], options)`
holding the body of `factors`, with `if !extra.is_empty() { add_triplets(&mut kg, extra) }` after the assembly of `K_sigma`, and
`factors(...)` becomes `factors_with(model, prestress, &[], options)` (the same operations when `extra` is empty). The
transformation path refuses a non-empty `extra` (key `"load stiffness"`). The sign: the tangent is `K_int - d f_ext / d u`, and
the follower pressure's `f_ext` scales with the factor like the pre-stress.

### D4. Arc-length in the Newton driver (`nonlinear.rs`, `arclength.rs`)

In `nonlinear.rs`, small and local (the lines F-423 does not touch, checked against its worktree):

- `pub(crate) trait LoadFactorControl { fn correct(&mut self, k: usize, du_residual: &[f64], du_load: &[f64],
  negative_pivots: usize) -> std::result::Result<f64, String>; }`: called at every iteration of an attempt after the
  factorisation, with the solutions of `K_T du_r = R` and `K_T du_f = f` on the free equations; returns the change of the load
  factor `d lambda`.
- `Problem::attempt` becomes `attempt_with(.., control: Option<&mut dyn LoadFactorControl>)`; `attempt(..)` calls it with
  `None`. With a control: `lt` is a variable (`let mut lt = lt;`); after `reduce`, the reduced tangent is factored as indefinite
  (`PivotRule::NonZero` and the test `|d| > PIVOT_TOLERANCE |a_ii|`, a pivot below it refused as a singular tangent), the
  correction is `du = du_r + d lambda du_f`, `lt += d lambda`, and its energy `|du . (R + d lambda f)|` (the work of the
  correction against its own right-hand side, as without control). Without control: the same operations as now
  (`factor_solve`, `du . R`).
- `pub(crate) fn controlled_attempt(model, f, fixed, options, contributions, u_c, lambda_c, states_c, control)`: builds the
  `Problem` and runs `attempt_with` from the committed state with the load factor `lambda_c` (the attempt starts at the committed
  factor; the control's first correction, at `k = 0`, is the predictor); `pub(crate) fn check_driver_model` and `pub(crate) fn
  NewtonOptions::check` for `arclength.rs` (the first is the same edit as F-423's).

In `arclength.rs`: `ArcLengthOptions { length, load_scale: Option<f64>, max_points, max_halvings, newton: NewtonOptions }`,
`PathPoint { load_factor, displacement, length, halvings, negative_pivots, residuals, energies }`, `ArcLengthPath { points,
load_scale, states, stopped }` and `trace(model, prescribed, forces, options, contributions, stop) -> Result<ArcLengthPath>`:

- The reference load `f` is the model's loads and `forces` (the load factor multiplies it); every prescribed value must be 0
  (key `"arc length"`); `f` must be non-zero on the free equations.
- The spherical constraint (Crisfield 1981, Comput. Struct. 13, 55, cited from memory, NOT read; derived here): `|Du|^2 + c
  Dlambda^2 = Ds^2` on the free equations, `Du, Dlambda` the increment from the committed point, `c` the load scale (`psi^2 f .
  f` of Crisfield): given, or by default `|K_0^-1 f|^2` from the first factorisation (the first predictor then splits `Ds`
  equally between the two terms). At iteration `k` with `b = Du + du_r`: `a1 dl^2 + a2 dl + a3 = 0`, `a1 = du_f . du_f + c`, `a2 =
  2 (du_f . b + c Dlambda)`, `a3 = b . b + c Dlambda^2 - Ds^2`; no real root ends the attempt as failed. Of the two roots the one
  whose new increment makes the larger `Du_new . Du_ref + c Dlambda_new Dlambda_ref` is taken: at `k >= 1` the reference is the
  increment of the attempt so far (Crisfield's angle criterion), at `k = 0` the converged increment of the previous point (for
  the first point: `dl > 0`). This is the predictor at `k = 0` (where `du_r` is the residual of the converged point) and the
  corrector afterwards, in one rule. Convergence is the driver's (residual and energy, the options of `newton`).
- Points: from `u = 0, lambda = 0` with `Ds = length`; a failed attempt halves `Ds` (at most `max_halvings` times per point, then
  `FemError::Solver { solver: "arc-length" }` naming the point); after a converged point `Ds` doubles back towards `length`. The
  trace ends when `stop(&point)` returns `true` (`stopped = true`) or after `max_points` points (`stopped = false`). Element
  states committed per point as in the driver.
- Riks (Int. J. Solids Struct. 15 (1979) 529, cited from memory, NOT read) for the idea of the load factor as an unknown with a
  constraint; the linearised (normal-plane) form is not built.

### D5. The ring (tests)

Quarter ring, `R = 1` (mean), `t = 0.02`, width `b = 0.02`, `E = 2e11`, `nu = 0`, `rho = 7850`: `hex_box([R - t/2, 0, 0], [t,
pi/2, b], [1, n, 1])` upgraded to Hex20 by `upgrade_to_hex20_mapped` with `(r, theta, z) -> (r cos theta, r sin theta, z)`;
`u_y = 0` on `ymin` (`theta = 0`), `u_x = 0` on `ymax` (`theta = 90` degrees), `u_z = 0` on every node; the bore `xmin`, the
outer surface `xmax`. R1, R3: `n = 8`, 20 load steps, `NewtonOptions::new(20)`. B1, B2: `n = 24`, `p_ref = 1e6` on `xmax`
(dead for the pre-stress), `BucklingOptions::new(2)`; B3: the lowest mode of each has the shape `cos 2 theta` (correlation of the
radial displacement of the mid-surface nodes with `cos 2 theta`, `>= 0.99`).

### D6. The truss (tests)

`L0 = 1`, `h = 0.2`, `a = sqrt(L0^2 - h^2)`, `E = 2e11`, `nu = 0`, `rho = 7850`; supports `(-a, 0, 0)`, `(a, 0, 0)`, apex
`(0, h, 0)`; each bar a bipyramid: the square of side `s = 0.1` at mid-length, perpendicular to the bar, corners `M +- (s/2) n0
+- (s/2) e_z` (`n0` the in-plane normal), four Tet4 `(support, apex, c_i, c_i+1)` around the axis (ordered to a positive volume);
`A = s^2/3`. Supports held in `x, y`; `u_z = 0` on every node (the plane; it also holds the spin of each bar about its axis);
the apex free. The force `-P_ref e_y` at the apex, `P_ref = P_max`. Arc length `Ds = 0.05 h`, `load_scale` by default,
`max_points = 400`, `max_halvings = 5`, `newton = NewtonOptions::new(1)` (20 iterations, `1e-10`, `1e-10`); stop at the first
point with `w >= 2.2 h`.

### D7. Modes about a state (`follower.rs`)

`tangent_modes(model, prescribed, u, states, load_factor, contributions, options: &ModalOptions) -> Result<ModalResult>`: `K_T =
nonlinear::tangent(model, u, states)` plus each contribution's tangent at `(u, load_factor)` (`add_triplets`), the consistent
mass, both reduced on the model's prescribed equations and `prescribed` (the motions about the state hold them: their values
are not used), the eigensolver and the result of `prestressed_modes` (the same shift rule, the same mass test). The beam models:
M1, M2 the beam B of `si_geometric.rs` with `xmin` and `xmax` clamped in `x, y`, `u_x = eps L` on `xmax`, `u_z = 0` everywhere,
`NewtonOptions::new(2)`, `ModalOptions::new(2).residual_test(Backward)`; M3, M4 the beam F of `si_rotating.rs` with
`Load::centrifugal(z, 0, Omega)` and the `CentrifugalFollower`, `NewtonOptions::new(2)`, the modal options of `beam_options()`.

### D8. The bit record of the default (N1)

`crates/rcfd-fem/tests/sa_follower_default_bits.rs`, written after this commit and run on the unchanged base 4c422025 before any
library code; its digests (FNV-1a 64 over the bits) are written into the file then. Content: `nonlinear::solve` of an elastic
stretched Hex8 bar (prescribed end, 2 steps) and of a Hex20 cantilever with a tip traction (2 steps); `solve_with` with a zero
contribution that refuses one call (the halving path) and with a step-control hook that returns 0.6 of the proposal's
increment; `solve` of a Hex8 block with a dead `Load::Pressure` (2 steps); `buckling::factors` of a Hex20 column under an end
compression (2 factors, values and modes); `nonlinear::tangent` and `internal_force` of a distorted Tet10 model; the error message of
`solve_with` when the halvings are exhausted (a contribution that always refuses). Every field of each result in a fixed order.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| S1 | a1 | full `K_p` vs central differences of `f_p`, `h = 1e-6` x element size, one Hex8, Hex20, Tet4, Tet10 element, one loaded face, deformed (seed `0xF424_0480`): `max abs diff / max abs` | `<= 1e-6` each | rounding only, `~1e-10` (arithmetic 1) |
| S2 | a3 (condition) | reduced `K_p` of the B1 ring at `u = 0`: `max |K - K^T| / max |K|` | `<= 1e-11` | rounding only (arithmetic 3) |
| S3 | control | the S1 elements (free edges): `max |K_p - K_p^T| / max |K_p|` | `>= 1e-3` each | the edge term is of the order of `K_p` |
| S4 | D1 | `force(0, 1)` against `nodal_load_vector` of the dead `Load::Pressure` on the same set, the S1 elements and the R1 ring | equal bits | the same operations |
| R1 | a2 | ring, follower, every step with `lambda >= 1.2` and the last: `|p / p_thin(lambda) - 1|` | `<= 1e-2` each | `+0.84e-2 .. +0.67e-2` (arithmetic 2) plus R2's error |
| R1b | a2 (record) | the same at the steps with `lambda < 1.2` | recorded | `+1.0e-2 .. +0.85e-2` |
| R2 | a2 (companion) | ring, follower and dead, every step: `|p / p_exact(r_o) - 1|`, `p_exact` of the thick ring at the measured mean outer radius | `<= 1e-3` each | `<= 1e-4` (arithmetic 2) |
| R3 | a2c | ring, dead, last step: `|p / (lambda p_thin) - 1|` and `p / p_thin - 1` | `<= 2e-2` and `>= 0.3` | `+1.02e-2`, `+0.398` |
| R4 | a2 | ring, follower: the last mean stretch | `>= 1.5` | `1.507` |
| B1 | a3 | ring buckling with the load stiffness: `|p_cr / (3 E I/R^3) - 1|` | `<= 2e-2` | `-1.0e-2 +- 0.5e-2` (arithmetic 3) |
| B2 | a3c | without (existing `buckling::factors`): `|p_cr / (4 E I/R^3) - 1|` | `<= 3e-2` | the same |
| B3 | a3 (companion) | the lowest mode of B1 and of B2: correlation of the radial mid-surface displacement with `cos 2 theta` | `>= 0.99` each | `n = 2` lowest (arithmetic 3) |
| A1 | a4 | truss, every converged point: `|lambda P_ref - P(w)| / P_max` | `<= 1e-6` each | `<= 1e-9` (arithmetic 5) |
| A2 | a4 | the trace stops (`stopped`) at `w >= 2.2 h` within 400 points | holds | about 110 points (path length `~5.6 h`) |
| A3 | a5 | `|max_{w < h} lambda P_ref / P_max - 1|` | `<= 5e-3` | `<= 9.4e-4` (arithmetic 6) |
| A4 | D4 | every point: `| |Du|^2 + c Dlambda^2 - Ds^2 | / Ds^2` from the stored points (consecutive displacements and load factors, the free equations) | `<= 1e-10` each | the quadratic is solved at each iteration; rounding `~1e-14` |
| A5 | a4 (companion) | negative pivots of each point: 0 for `w < w_1 - 0.02 h`, 1 for `w_1 + 0.02 h < w < w_2 - 0.02 h`, 0 for `w > w_2 + 0.02 h` | holds | one unstable mode between the limit points (the bars' kink is stiff: `k_mat/k_geo ~ 1/(3 eps) ~ 16`) |
| A6 | a4c | load control to `1.2 P_max` in 10 steps: `Err(Solver)` with the largest converged load in `[0.97, 1.0] P_max`, or `Ok` with `w > h (1 + 1/sqrt 3)` | holds | `Err`, largest load `>= 0.99 P_max` (5 halvings of `0.12 P_max`) |
| A7 | a5 (companion) | `|min_{h < w < 2 h} lambda P_ref / P_max + 1|` (the second limit point) | `<= 5e-3` | as A3 |
| M1 | a6 | clamped beam, `eps = 1e-3`: modes 1, 2, `|f_T / f_L - 1|` | `<= 1e-2` each | `~9e-4` (arithmetic 7) |
| M1b | a6 (record) | the tension increments `(f_T^2 - f_0^2)/(f_L^2 - f_0^2) - 1` | recorded | |
| M2 | a7 | the same at `eps = 1e-5` | `<= 1e-4` each | `~1e-5` |
| M3 | a8 | beam F at `0.2 omega_0`: modes 1 to 4, `|f_T / f_F - 1|` | `<= 1e-4` each | `<= 5e-5` (arithmetic 7) |
| M4 | a9 | the same at `0.5, 1, 2 omega_0` | recorded | |
| M5 | control | beam F at `0.2 omega_0` without the `CentrifugalFollower` (no spin softening in the tangent): mode 1 `|f_T / f_F - 1|` | `>= 1e-3` | `~ Omega^2/(2 omega_0^2) ~ 2e-2` |
| M6 | D7 | `tangent_modes` at `u = 0` without load against `modal_analysis` of the same model (beam B clamped, end value 0): eigenvalues, relative | `<= 1e-8` each | `K_T(0) = K` up to the order of the sums; eigensolver tolerance `1e-10` |
| N1 | b4 | the digests of D8 at the base | exact | D4 executes the same operations without a control; D3 without `extra` |
| N2 | b4 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |
| X1 | b7 | `x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432` becomes an ignored record citing ADR 0291 and its `_after_f424` copy (`follower` and `arclength` landed) passes | pass | the scheme of ADR 0291 |

Files: `crates/rcfd-fem/tests/sa_follower.rs` (S1 to S4, R1 to R4, B1 to B3, M1 to M6), `sa_arclength.rs` (A1 to A7),
`sa_follower_default_bits.rs` (N1). A check that would take more than about two minutes in release is an ignored heavy record
run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `K_p` with the opposite sign | S1, B1 |
| MU2 | the second term `N_b,v (x_u x e_j)` of `K_p` dropped | S1, B1 |
| MU3 | `f_p` on the reference face (`u` ignored) | R1, S1 |
| MU4 | the contribution adds `+f_p` (sign of D1) | R1, R2 |
| MU5 | arc-length: the root with the smaller dot product taken | A1, A2 |
| MU6 | arc-length: `u` updated by `du_r` only (the `d lambda du_f` part dropped) | A4 |
| MU7 | `CentrifugalFollower` tangent with the opposite sign | M3 |
| MU8 | `tangent_modes` with the linear stiffness `K` in place of `K_T` | M1, M3 |
| MU9 | `buckling::factors_with` ignores `extra` | B1 |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, N1 and N2
  unchanged.
- **partial**: N1 and N2 unchanged, and at least S1, A1, A3, A6 met, with any other miss diagnosed and recorded as a
  `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- The follower pressure is uniform on a surface set of solid elements; no shell pressure that follows (the shell's own
  `ShellPressure` stays dead), no follower traction, no pressure of a fluid volume (the closed-volume term).
- The Newton driver uses the symmetric part of the load stiffness (the skyline factor is symmetric): exact when the loaded
  surface is closed or its edges hold the components of arithmetic 3; elsewhere the iteration converges linearly, not
  quadratically.
- Arc-length takes the load direction from the dead loads: a contribution's dependence on the load factor (a follower
  pressure) is in the residual but not in `du_f`; a reference load made only of follower pressure is refused (zero `f`), a mixed
  one converges at a reduced rate. No branch switching at bifurcation points; no automatic step-size rule beyond halving and
  doubling back; prescribed displacements must be zero.
- The modes about a state are those of the tangent at the converged state, real and symmetric (the symmetric part of a
  follower's load stiffness; no Coriolis term).
- No case-file key (b5).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Case-file key.** Not added (the brief). Recommendation for the lead: a load entry `follower_pressure = { surface = <set>,
   pressure = <Pa> }` read into `follower::FollowerPressure` and offered with the nonlinear static study only, and a study option
   `continuation = { arc_length = <length>, max_points = <n>, stop_displacement = { node = <set>, component = <c>, value = <m> }
   }` for `arclength::trace`.
2. **Where arc-length sits.** ADR 0440 open point 2 took a separate `arclength::trace` on the public driver pieces; the brief
   asks for an option of the driver without a second Newton loop. Taken: the per-iteration control is an option of the driver's
   attempt (D4), the sequence of points (the outer loop, not a Newton loop) is `arclength::trace`.
3. **The symmetric part of the load stiffness.** Taken: the driver and the buckling path use the symmetric part; the full
   matrix is offered for checks (`load_stiffness`). An unsymmetric factor is a follow-up if a free-edged follower load needs
   quadratic convergence.
4. **The two-bar truss of solid bars.** Taken: bipyramidal Tet4 bars (D6, arithmetic 5), the only solid bars found for which
   the closed form is the FE solution; a prismatic bar would test the end conditions, not the continuation.

## Addendum 1 (2026-11-14): results

Runs: `sa_follower` and `sa_arclength` once each at commit 70b552a2 (`--release -- --nocapture --test-threads 1`;
`f424-scratch/run1-follower.log`, `run1-arclength.log`). N1 was recorded at the base 4c422025 (`n1-base.log`) before any
library code and passed at 70b552a2. Then the whole `rcfd-fem` suite in release (611 passed, 0 failed, 145 ignored; it runs
the two registered files a second time, both passed), `baseline_hashes` of `rcfd-api` (3 passed, 1 ignored), `docs_site` of
`rcfd-cli` (2 passed). No limit was changed after a run.

| Id | Limit | Measured | Met |
|---|---|---|---|
| S1 Hex8, Hex20, Tet4, Tet10 | `<= 1e-6` | `2.43e-10, 3.55e-10, 1.45e-10, 1.29e-10` | yes |
| S2 ring, reduced `K_p` asymmetry | `<= 1e-11` | `3.41e-16` | yes |
| S3 the S1 elements | `>= 1e-3` | `2.0, 1.98, 2.0, 2.0` | yes |
| S4 follower at `u = 0` against the dead pressure | equal bits | equal on the four elements and the ring | yes |
| R1 steps 7 to 20 (`lambda >= 1.2`) and the last | `<= 1e-2` | `8.44e-3` falling to `6.71e-3` | yes |
| R1b steps 1 to 6 | recorded | `9.84e-3 .. 8.63e-3` | recorded |
| R2 follower; dead | `<= 1e-3` | `6.3e-7 .. 7.3e-7`; `6.5e-7 .. 8.6e-7` | yes |
| R3 dead, last step | `<= 2e-2`; `>= 0.3` | `1.021e-2`; `0.398` (final stretch 1.384) | yes |
| R4 follower final stretch | `>= 1.5` | `1.50717` | yes |
| B1 with the load stiffness | `<= 2e-2` | `3.45e-3` (`p_cr = 3.0104 E I/R^3`) | yes |
| B2 without | `<= 3e-2` | `1.64e-2` (`3.9345 E I/R^3`) | yes |
| B3 correlation with `cos 2 theta` | `>= 0.99` | `1.0, 1.0` | yes |
| A1 | `<= 1e-6` | `1.61e-13` | yes |
| A2 | stop at `w >= 2.2 h` within 400 | stopped, 88 points, `w_last = 2.2086 h` | yes |
| A3 | `<= 5e-3` | `6.19e-4` | yes |
| A4 | `<= 1e-10` | `8.5e-15` | yes |
| A5 negative pivots 0, 1, 0 | holds | holds (264 iterations, 0 halvings, `c = 4.47e-3`) | yes |
| A6 load control to `1.2 P_max` | `Err`, largest in `[0.97, 1.0] P_max` | `Err(Solver)`, largest `0.9975 P_max` | yes |
| A7 | `<= 5e-3` | `3.04e-4` | yes |
| M1 modes 1, 2 at `eps = 1e-3` | `<= 1e-2` | `2.83e-5, 2.22e-5` | yes |
| M1b tension increments | recorded | `5.27e-4, 7.68e-4` | recorded |
| M2 at `eps = 1e-5` | `<= 1e-4` | `2.7e-8, 8.8e-8` | yes |
| M3 `0.2 omega_0` (`omega_0 = 255.7545` rad/s), modes 1 to 4 | `<= 1e-4` | `1.22e-5, 1.17e-5, 6.4e-6, 6.4e-6` | yes |
| M4 `0.5 omega_0`, modes 1 to 4 | recorded | `7.5e-5, 6.0e-5, 4.0e-5, 4.0e-5` | recorded |
| M4 `1 omega_0` | recorded | `2.85e-4, 1.55e-4, 1.58e-4, 1.55e-4` | recorded |
| M4 `2 omega_0` | recorded | `1.02e-3, 2.96e-4, 6.21e-4, 5.84e-4` | recorded |
| M5 without the centrifugal follower | `>= 1e-3` | `1.96e-2` | yes |
| M6 `tangent_modes` at `u = 0` against `modal_analysis` | `<= 1e-8` | `0, 0` | yes |
| N1 default bits | exact | the 9 digests equal | yes |
| N2 `baseline_hashes` | pass | pass | yes |
| X1 the `_after_f424` copy | pass | pass; the earlier record ignored | yes |

M1 came out about thirty times under the estimate of arithmetic 7 (`9e-4`). This is recorded and not diagnosed further; the
limit is met by more than two decades.

### Mutations (scratch copy of 70b552a2 by `git archive`, one at a time, `f424-scratch/mutate.py`, `mutations.log`)

| Id | Caught by |
|---|---|
| MU1 `K_p` sign | S1 (all four, `2.0`), B1 (`0.89`); the ring test also fails at step 1 (singular tangent) |
| MU2 second term of `K_p` dropped | S1 (`0.87 .. 1.06`); B1 not caught |
| MU3 `f_p` on the reference face | S1 (`1.0`), R1 (steps 9 to 20), R2 (every step, `3.1e-2` and up), R4 |
| MU4 contribution adds `+f_p` | the ring test: the Newton driver fails at step 1 (singular tangent), R1 to R4 not reached |
| MU5 the root of the smaller dot product | the truss test: `trace` fails at point 1 after 5 halvings, A1 to A7 not reached |
| MU6 `u` by `du_r` only | the truss test: `trace` fails at point 1 (not converged in 20 iterations), A4 not reached |
| MU7 centrifugal tangent sign | M3 (`1.97e-2, 1.90e-2, 5.2e-4, 5.3e-4`) |
| MU8 linear `K` in `tangent_modes` | M1 (`5.5e-2, 2.9e-2`), M2 (`6.0e-4, 3.1e-4`), M3 (the eigensolver refuses the indefinite matrix) |
| MU9 `factors_with` ignores `extra` | B1 (`0.311`) |

Nine of nine caught. MU4, MU5 and MU6 are caught by an error of the solve inside the registered test before the named check is
evaluated (the test fails). MU2 is caught by S1 alone where S1 and B1 were planned.

### Deviations

- After the runs, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` refused `if !(disc >= 0.0)` in `arclength.rs`
  (`neg_cmp_op_on_partial_ord`). Commit 5576cd9e writes it `if disc.is_nan() || disc < 0.0`, which takes the same branch for
  every value, NaN included. Clippy is clean after it; fmt was clean before and after. The registered tests were not run again
  after this edit (each registered run is made once); the library compiles under clippy at that commit.
- None other.

### Not run

- The whole workspace suite (the brief). No case-file key was added (b5; open question 1). No unsymmetric factor of the load
  stiffness and no arc-length load direction from the contributions (see the limits above).

### Decision

**done** by the registered rule: every check met, every planned mutation caught, fmt and clippy clean, N1 and N2 unchanged.

### Remaining questions (recommendations for the lead)

- **Q1** (case-file key): as registered, `follower_pressure = { surface, pressure }` and `continuation = { arc_length,
  max_points, stop_displacement }`, recommended and not added.
- **Q2** (MU4 to MU6 caught by an error, not by the named check): options (a) accept, since a failed solve fails the test;
  (b) add checks that evaluate the path up to the failure. Recommendation (a).

## For the lead: roadmap entries

Proposed status of F-424: **done**. Note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. ADR 0480, 2026-11-14 (registration of stub ADR 0327): rcfd_fem::follower (stub of ADR 0440 filled, PENDING None): FollowerPressure on a surface set of solid faces, the force on the current face and its load stiffness (the full matrix for checks, the symmetric part in the driver), a Contribution of the Newton driver; CentrifugalFollower (the deformation-dependent centrifugal load); buckling with the load stiffness (buckling::factors_with); tangent_modes about a converged nonlinear state. rcfd_fem::arclength (filled, PENDING None): trace with the spherical constraint of Crisfield as a per-iteration load-factor control of the driver's own attempt (no second Newton loop), with halving and doubling back of the arc length. Load stiffness against central differences 1.3e-10 .. 3.6e-10 (limit 1e-6). Ring (t/R 0.02, nu 0) to the stretch 1.507: thin law within 8.4e-3 for lambda >= 1.2 (limit 1e-2; the thick-ring ODE within 7.3e-7), dead pressure off by lambda (+0.398). Ring buckling 3.0104 E I/R^3 with the load stiffness (limit 2 percent), 3.9345 without (limit 3 percent). Two-bar truss of bipyramidal Tet4 bars to w = 2.2086 h in 88 points: closed form within 1.6e-13 P_max (limit 1e-6), limit load within 6.2e-4 (limit 5e-3), negative pivots 0, 1, 0; load control fails at 0.9975 P_max. Tangent modes: stretched beam within 2.8e-5 of F-395 (limit 1e-2), 8.8e-8 at the small stretch (limit 1e-4); rotating beam at 0.2 w_0 within 1.2e-5 of F-396 (limit 1e-4), recorded 1.0e-3 at 2 w_0. Nine of nine mutations caught; default bits and baseline hashes unchanged. Done by the registered rule.

Follow-ups (title, deps, acceptance), for the lead to number or drop:

> (new item, number for the lead) Arc-length with a follower reference load and an unsymmetric factor of the load stiffness (deps F-424). Every limit not stated here is registered in the ADR before the run. The load direction of the arc-length iteration includes the derivative of the contributions with the load factor, so that a reference load made only of follower pressure is traced; a ring under follower internal pressure traced by arc-length reproduces the load-controlled path of ADR 0480 within 1e-6 at the common load factors; an unsymmetric factor of the tangent gives quadratic convergence on a free-edged follower load (registered rate), and the symmetric path keeps its bits.

> (new item, number for the lead) Follower pressure on shells (deps F-424). Every limit not stated here is registered in the ADR before the run. ShellPressure gains a follower form on the current mid-surface with its load stiffness, against central differences within 1e-6 on Shell4 and Shell3; a thin cylinder of shells under follower internal pressure follows p = E t (lambda^2 - 1)/(2 R) within 1 percent to the stretch 1.5; the dead ShellPressure keeps its bits.

> (case-file key, for the lead) follower_pressure = { surface, pressure } with the nonlinear static study, and continuation = { arc_length, max_points, stop_displacement } for arclength::trace (ADR 0480 open question 1); not added.
