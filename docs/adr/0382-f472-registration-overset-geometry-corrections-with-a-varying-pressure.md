# ADR 0382: F-472 registration: the overset geometry corrections with a varying pressure on B3d

Status: **REGISTERED 2026-11-13, written before any code of F-472 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-472 was written, compiled or run: the code and the documents listed under "What was
read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum
at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0367](0367-f368-registration-overset-skewness-and-non-orthogonal-corrections.md) (F-368: the opt-in
`OversetSolver::set_geometry_corrections`, its corrections C1 to C4 and gradients G, its runs P3o, W3o, ..., its question Q2
option (b), from which this item comes; the test `crates/rcfd-fv/tests/overset_f368.rs`, whose set-up code is reused),
[ADR 0255](0255-f357-registration-uniform-flow-on-a-warped-overset-mesh.md) (the motion `P(t)` of the component on B3 and
B3d, the uniform flow W3), [ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md) (the set-ups B3, B3d,
the quadratic donor interpolation), [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (the
overset step M5 to M10; its Taylor-Green set-up T, whose choice of a box on which the exact pressure has a zero normal
gradient is reused). Roadmap item F-472 (depends on F-368).

Number 0382 (given by the lead). Branch `f472-overset-geom-b3d` from main 4bcfd339, worktree `w49i-work/f472`, scratch
`w49i-work/f472-scratch`.

House rules (the campaign rules of 2026-11-12, applied on 2026-11-13): limits fixed here and not changed after a run; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment committed
before the corrected run; existing tests and records are not edited; `crates/rcfd-api/tests/baseline_hashes.rs`, the hash
tables and the goldens are not edited or re-recorded; each registered run is made once (runs shared by several tests are
computed once per test process through a `OnceLock`); no new crate, no unsafe code, no commit trailers, no push.

Clean-room. Nothing outside this repository was opened. The flow is the Taylor-Green vortex (G. I. Taylor and A. E. Green,
Proc. R. Soc. Lond. A 158, 1937, cited from memory) with a uniform axial velocity added; it is verified below as an exact
solution of the incompressible Navier-Stokes equations by direct substitution, which is all that is used. The weighted
least-squares gradient is textbook finite-volume practice (cited from memory: Ferziger and Peric, Computational Methods for
Fluid Dynamics, chapter on complex geometries) and is the library's own `gradient::least_squares`, already used by ADR 0367's
gradients G; its exactness for linear fields is derived below.

## What F-472 asks

The roadmap acceptance (quoted, binding): "With OversetSolver::set_geometry_corrections on, a three-dimensional flow with a
known varying pressure (a decaying vortex or a manufactured solution) on B3 and B3d with the component moving as in ADR 0255:
the error of the calculated and fringe cells on B3d at most twice that on B3 with the default and the quadratic donor
interpolation, continuity within 1e-9 at every step, with a run that names the share of the uncorrected Green-Gauss pressure
gradient."

**Reading (registered here).**

1. "With set_geometry_corrections on": ADR 0367's opt-in exactly as it is on main (C1 to C4; the cell pressure gradient of
   the momentum predictor and of the velocity correction stays the uncorrected Green-Gauss gradient, ADR 0367 "What stays
   as it is").
2. "A three-dimensional flow with a known varying pressure (a decaying vortex ...)": the flow V below, a decaying
   Taylor-Green vortex cell about the axis `z` with a uniform axial velocity `W`: a three-dimensional velocity field whose
   pressure varies in `x` and `y` by `V^2` (from `-0.5` to `+0.5`), on the three-dimensional meshes B3 and B3d, the
   component turned about the axis `(1, 2, 3)`.
3. "On B3 and B3d with the component moving as in ADR 0255": the meshes and the motion `P(t)` of ADR 0255 (40 steps of `dt =
   0.005`), unchanged.
4. "The error ... at most twice that on B3": `E` = the largest `abs(u_c - u_ex(x_c, t_k)) / V` over the steps `k = 1..40` and
   the non-hole cells (calculated and fringe). As in ADR 0367, "that on B3" is read in both senses, both registered: B3 with
   the opt-in, and B3 with the default solver.
5. "With the default and the quadratic donor interpolation": the two pairs, as ADR 0367.
6. "Continuity within 1e-9 at every step": `OversetInfo::continuity <= 1e-9` in every step of the four opt-in runs.
7. "A run that names the share of the uncorrected Green-Gauss pressure gradient": the same B3d runs with the Green-Gauss
   pressure gradient replaced by the least-squares gradient of the corrections (a new opt-in, L below), and the share
   `sigma = (E(W3o) - E(W3oL)) / E(W3o)` printed for each interpolation: the part of the error on B3d that the uncorrected
   Green-Gauss gradient carries. It is a named number, not a limit.

## What was read (no run)

ADR 0367 in full (its addendum numbers are the known values of the opt-in with a constant pressure),
`crates/rcfd-fv/tests/overset_f368.rs` in full; ADR 0255 in full; ADR 0219 the Taylor-Green set-up T
(`crates/rcfd-fv/tests/overset_f134.rs` lines 405 to 470). `crates/rcfd-fv/src/overset.rs`: the module text, `OversetSolver`,
`GeometryCorrections`, `new`, `set_geometry_corrections`, `prepare`, `advance`, `piso`, `overset_pressure` (main 4bcfd339,
after ADR 0379's BDF2 change). `pressure.rs` `pressure_bc` (a fixed-velocity face gets the zero-gradient pressure
coefficients, whatever the pressure field's condition), `boundary_values_pbc`, `boundary_hby_a`; `gradient.rs` `green_gauss`
(the header), `least_squares` (the header). `PimpleSettings::p_ref` (default `(0, 0.0)`).

What the code shows for this item: the cell pressure gradient of the overset step is `green_gauss(&g, p, bvals_p, 0)` in
two places of `piso` (the momentum predictor, `grad_p`, and the velocity correction, `grad_new`), with the corrections on as
off. With every patch at a fixed velocity the pressure boundary value is the owner cell's value (zero normal gradient).

## Why the Green-Gauss pressure gradient matters on B3d (derivation)

For a linear field `p = p0 + c . x`, the Green-Gauss gradient with the weights `w = d_n / (d_o + d_n)` takes on an internal
face the value `p(x_f') = p(x_f) - c . s_f` (`s_f = x_f - x_f'`, ADR 0367) and integrates `p(x_f) S_f` where the exact face
integral over the triangulated face is `p(x_f) S_f + W_f^T c` (ADR 0367's warp tensor, the same algebra as C1 with a scalar).
Summed over a closed cell, `sum_f p_exact,f = c V` (Gauss on the polyhedron of flat triangles), so

`V (g_GG - c) = - sum_f sigma_f [ (c . s_f) S_f + W_f^T c ]`

(`sigma_f` the outward sign). On B3 both terms vanish to round-off (`s_f = 0`, `W_f = 0`), on B3d neither: the Green-Gauss
gradient is **inconsistent** there, its error a fixed fraction of `abs(c)` that does not shrink with the cell size. The
weighted least-squares gradient minimises `sum_j w_j (p_j - p_c - g . d_j)^2` over the neighbours and boundary faces `j`;
for a linear `p` the residuals vanish at `g = c`, which is therefore the unique minimiser (the normal matrix `sum_j w_j d_j
d_j^T` is positive definite when the `d_j` span space): exact for linear fields on any mesh, its error for a smooth field
of the second order in the size of the stencil, `O(h abs(H))` with the Hessian `H`, and smaller still where the stencil is
nearly symmetric.

**The cell velocity sees the cell gradient.** In the step, `u_c = HbyA_c - rAU_c grad_p,c` with `rAU` close to `dt` and
`HbyA` carrying `u` of the step before; the face flux sees only the compact face difference of the pressure (and C4). A
cell-gradient error `eps_c` is therefore an extra source `-eps_c` in the cell momentum balance, not removed by the pressure
equation (which constrains the face fluxes), and the cell velocity drifts by about `t eps_c` over a time `t` that a fluid
particle spends in cells of similar error. This is the path F-368 could not see (its exact pressure was constant).

**L, the separating opt-in (new, library).** `OversetSolver::set_least_squares_pressure_gradient(on: bool)`: with it on, both
cell pressure gradients of `piso` (the predictor's `grad_p` and the correction's `grad_new`) are
`GeometryCorrections::gradients(&g, p, boundary_values_pbc(&g, &pbc, p), corr_conn)` (ADR 0367 G: the least-squares gradient
in every cell, the donor interpolation of those gradients in a fringe cell, zero in a hole cell; no calculated row uses the
fringe or hole value) instead of `green_gauss(.., 0)`. It acts only together with the geometry corrections: a step with it on
and the corrections off is refused, naming ADR 0382. Off (the default) is the solver as it is, bit for bit, with the
corrections on or off (the new code is behind one flag, the `green_gauss` calls are not touched). No other change of the
library is made by this item.

## The flow V (closed form)

`L = 2.4`, `k = pi / L`, `xi = x - L/2`, `eta = y - L/2`, `F(t) = exp(-2 nu k^2 t)`, `V = 1`, `W = 0.5`, `nu = 0.01`:

`u_ex = ( -V cos(k xi) sin(k eta) F, V sin(k xi) cos(k eta) F, W )`, `p_ex = -(V^2 / 4) (cos(2 k xi) + cos(2 k eta)) F^2`.

**Verification by substitution.** `div u = V F k (sin kxi sin keta - sin kxi sin keta) = 0`. Each of the first two
components is an eigenfunction of the Laplacian with eigenvalue `-2 k^2`, so `d u / dt = -2 nu k^2 u = nu lap u`; the third
is constant. The convective term of the first component: `u_x d_x u_x + u_y d_y u_x + W d_z u_x = V^2 F^2 k [ -cos kxi sin
kxi sin^2 keta - sin kxi cos kxi cos^2 keta ] + 0 = -(V^2 F^2 k / 2) sin 2kxi`, and `-d_x p_ex = -(V^2 F^2 k / 2) sin 2kxi`:
equal; the second component the same with `xi` and `eta` exchanged; the third: `(u . grad) W = 0 = -d_z p_ex`. So `d u / dt +
(u . grad) u = -grad p_ex + nu lap u` exactly.

**The box.** On `x = 0` and `x = 2.4` (`k xi = -pi/2, +pi/2`): `u_x = 0` (no normal flow), and `d_x p_ex = (V^2 F^2 k / 2)
sin 2kxi`, which is `sin(-pi) = sin(pi) = 0` there; the same on the `y` sides; on the `z` sides `u_z = W` (inflow at `zmin`, outflow at `zmax`,
net flux zero) and `d_z p_ex = 0`. So the zero-gradient pressure the library gives every fixed-velocity face (`pressure_bc`)
is exact for V on all six sides (as for ADR 0219's T), and no boundary error of the pressure condition hides the interior
geometry. The vortex centre is the domain centre `(1.2, 1.2)`, inside the moving component (centre `(1.23, 1.22, 1.19)`).

## Arithmetic before this registration (disclosed, by hand)

- Sizes: `max abs(u_ex) = sqrt(V^2 + W^2) = 1.118`; `max abs(grad p_ex) = V^2 k / 2 = 0.654` (at `xi, eta = +-0.6`);
  `abs(H) <= V^2 k^2 = 1.71`. Courant number `1.118 * 0.005 / 0.1 = 0.056`; with the mesh speed of ADR 0255 (at most `1.04`)
  the relative Courant number below `0.11`. Decay over the run: `2 nu k^2 * 0.2 = 0.0069` (`F(0.2) = 0.993`).
- Truncation on B3 (uniform `h = 0.1`, `kh = 0.131`): the second-order residual of the central convection about `(kh)^2 / 6
  abs(u) abs(grad u) = 0.0029 * 1.1 * 1.3 = 4e-3`, of the Green-Gauss gradient about `0.0029 * 0.654 = 2e-3`; over `t = 0.2`
  a velocity error of about `0.2 * 6e-3 = 1.2e-3`, plus the coupling and time error of the moving component that ADR 0367
  measured on B3 with the shear (`8.7e-4`, of which `2.1e-4` the time error). Estimate: `E(P3o)` between `1e-3` and `3e-3`,
  `E(P3o)` equal to `E(P3)` to about `1e-12` relative (on B3 the corrections vanish to round-off, ADR 0367 P-diff), and the
  quadratic runs within a few per cent of the linear ones (ADR 0367: `8.7203e-4`, `8.7204e-4`).
- The Green-Gauss inconsistency on B3d: interior points displaced by up to `0.015` per coordinate (uniform): a face centre
  moves by about `0.0087 / 2 = 0.0043` (rms per coordinate, four corners), a cell centre by about `0.003`, so `abs(s_f)` is
  about `0.005`; per face `(c . s_f) abs(S_f) / V` about `0.05 abs(c)`, six faces with random signs about `0.12 abs(c)` at
  most. The warp term: ADR 0367 K6 measured `abs(W_f) / abs(S_f)^{3/2}` up to `0.075`, so `abs(W_f^T c) / V` up to `0.075
  abs(c)` per face, typically a third of it: about `0.06 abs(c)` per cell. Together about `0.1 abs(c)` at most: with
  `abs(c) = 0.654`, a gradient error of about `0.065`, rms about `0.03`.
- The least-squares error on B3d for V: the stencil's asymmetry is the difference of two centre displacements, about `0.005`;
  the error about `abs(H) * 0.005 = 0.009` at most: about a seventh of the Green-Gauss error. On B3 (symmetric stencils) both
  are of the second order (`2e-3`).
- The velocity error on B3d with the opt-in: B3's error plus the drift `t eps`: with Green-Gauss `0.2 * (0.03 to 0.065) = 6e-3
  to 1.3e-2`; with the least-squares gradient `0.2 * 0.009 = 1.8e-3`, plus the second-order remainder of C2 and of the
  least-squares velocity gradients on the warped cells (ADR 0367 measured the warped meshes alone at `1.5e-4` to `3.1e-4` for
  the linear shear; for V, with curvature, I take up to `1e-3`). Estimates: `E(W3o)` between `6e-3` and `1.5e-2`, ratio
  `E(W3o) / E(P3o)` between `3` and `10`; `E(W3oL)` between `2e-3` and `5e-3`, ratio `E(W3oL) / E(P3oL)` between `1.2` and
  `2.5`; the share `sigma` between `0.5` and `0.85`.
- So the acceptance ratios A1, A1d, A2, A2d are **expected to miss** with the uncorrected Green-Gauss gradient (the opt-in
  as it is), and the separating ratios with L (L1, L2) to be met or near the limit. I register them as they are; a miss is
  recorded and diagnosed, not tuned.
- Control N1 (weights times `1.001`): a fringe value off by `1e-3 abs(u)`, about `8e-4` near the component's fringe; ADR
  0367's N1o raised `E` by `9.8e-4`. Limit on the increase `>= 3e-4`.
- Cost: the runs of ADR 0367 took 7 to 14 s each (B3d, 16568 cells); V adds only the boundary update per step. Twelve runs of
  40 steps and the fingerprints: about 2 to 3 minutes of release, spread over four test threads; no test is expected above
  two minutes alone.

## Set-ups

PS as ADR 0367: `PimpleSettings::piso(nu)`, central convection, two correctors, implicit Euler, `ddt_correction` on,
momentum and pressure `tol_abs = 1e-15`, `tol_rel = 1e-12`, at most 3000 iterations; mass mode conservative; `p_ref = (0,
p_ex(x_0, 0))` (cell 0 of the background, a calculated corner cell; the level of a closed domain). B3, B3d, the motion `P(t)`,
`dt = 0.005`, 40 steps, exactly as ADR 0367 (the same code). Start: `u_ex` and `p_ex` at the cell centres at `t = 0`; before
step `k` the six patches of the background get `u_ex(x_f, k dt)` at their face centres (fixed value per face; the
background does not move). The opt-in (and L) is set right after `OversetSolver::new`, before the first step.

- **P3o**, **P3qo**, **W3o**, **W3qo**: V on B3 / B3d, default / quadratic interpolation, with the geometry corrections.
- **P3**, **P3q**: V on B3 with the default solver (no opt-in), default / quadratic interpolation.
- **P3oL**, **P3qoL**, **W3oL**, **W3qoL**: as P3o, P3qo, W3o, W3qo with L on as well.
- **N1**: P3o with every stencil weight multiplied by `1.001` after `prepare` (ADR 0367 N1o).
- **U3oL**: ADR 0255's uniform flow W3 (B3d, default interpolation) with the corrections and L.

## Registered checks

New test file `crates/rcfd-fv/tests/overset_f472.rs`, release (`cargo test --release -p rcfd-fv --test overset_f472`). No
existing test is edited.

### A. The acceptance

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| A1 | default interpolation: `E(W3o) / E(P3o)` | `<= 2` | `3` to `10` (expected miss) |
| A1d | default interpolation: `E(W3o) / E(P3)` | `<= 2` | the same |
| A2 | quadratic: `E(W3qo) / E(P3qo)` | `<= 2` | the same |
| A2d | quadratic: `E(W3qo) / E(P3q)` | `<= 2` | the same |
| A3 | continuity of the calculated cells, every step of P3o, P3qo, W3o, W3qo | `<= 1e-9` each | `2e-14` (ADR 0367) |
| A5 | the share of the uncorrected Green-Gauss pressure gradient: `sigma = (E(W3o) - E(W3oL)) / E(W3o)` and `sigma_q` (quadratic); printed with the excess share `chi = (E(W3o) - E(W3oL)) / (E(W3o) - E(P3o))` when `E(W3o) > E(P3o)` | named, no limit | `0.5` to `0.85` |

### L. The separating runs with the least-squares pressure gradient

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| L0 (set-up) | L acts: `abs(E(W3oL) - E(W3o)) / E(W3o)` | `>= 1e-6` | `0.5` or more |
| L1 | `E(W3oL) / E(P3oL)` | `<= 2` | `1.2` to `2.5` |
| L2 | `E(W3qoL) / E(P3qoL)` | `<= 2` | the same |
| L3 | continuity every step of P3oL, P3qoL, W3oL, W3qoL | `<= 1e-9` | `2e-14` |
| L4 | U3oL every step, the non-hole cells: velocity `max abs(u - U) / abs(U)`, pressure `max abs(p) / abs(U)^2`; continuity | `<= 1e-10`, `<= 1e-10`; `<= 1e-9` | `1e-14`, `1e-13`; `1e-14` (the gradient of a constant is zero either way) |
| R1 | a step with L on and the corrections off is refused, the message naming ADR 0382 | refused | |

### K. The two gradients at operator level (exact boundary values)

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| K1 | `p = 2 + c . x`, `c = (0.3, -0.7, 0.4)`, on the warped background and the warped component (at `P(0)`): the least-squares gradient, `max_c abs(g_c - c) / abs(c)` | `<= 1e-10` | round-off |
| K2 | the same with the Green-Gauss gradient (`green_gauss(.., 0)`) on the warped background | `>= 1e-3` | about `0.1` (the inconsistency) |
| K3 | the same with the Green-Gauss gradient on the planar background and the planar component | `<= 1e-10` | round-off |
| K4 | `p_ex(., 0)` of V on the warped background, the cells without a boundary face: `max abs(g_LS - grad p_ex) / max abs(grad p_ex)` over `max abs(g_GG - grad p_ex) / max abs(grad p_ex)`; both printed, also on the other three meshes, with their rms | `<= 0.5` | about `0.15` |

### U. Set-up checks of the runs

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| U4 | kind changes over the steps, P3o and W3o | `>= 20` each | `99`, `92` (ADR 0367) |
| Q1 | P3qo, W3qo, W3qoL, every step: share of the fringe cells with a quadratic stencil | `>= 0.95` | `1.0` |
| U5 | unconverged linear solves of every run | printed | `0` |

Printed for every run (no limit): `E` with its step and cell, the classes BC, BF, CC, CF (background / component, calculated /
fringe), the error of the last step and its rms, the pressure error `max abs(p_c - p_ex(x_c, t_k) - m_k) / V^2` with `m_k` the
volume-weighted mean of `p - p_ex` over the non-hole cells of step `k`, the continuity, the kind changes, the time.

### N. Negative control

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| N1 | `E(N1) - E(P3o)` | `>= 3e-4` | about `1e-3` |

### D. The defaults

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| D1 | ADR 0367's `d1_defaults_fingerprint_equals_the_base_tree` (eight default fingerprints against the constants recorded on the base 2ecb97f6), run unedited on the final tree | passes | L is behind one flag |
| D1b | two fingerprints of the opt-in without L (5 steps of V on B3d with the corrections on, default and quadratic interpolation; the bits of `u`, `p`, `phi` folded by FNV-1a as ADR 0367 D1), recorded by a record test on the tree before any library change of F-472 and asserted on the final tree | equal, both | the same |
| D2 | `cargo test --release -p rcfd-api --test baseline_hashes`, unedited | passes | |
| D3 | the existing overset test files of `rcfd-fv` (`overset_f134`, `_f134_records`, `_f314`, `_f334`, `_f335`, `_f335_records`, `_f336`, `_f338`, `_f349`, `_f353`, `_f357`, `_f359`, `_f368`, `_f445`, `_f450`, `overset_motion_f442`) and `cargo test --release -p rcfd-fv --lib`, unedited, their ignored tests left ignored | pass | |

### Mutations (scratch copy, not committed)

Each alone, applied by a script with exact replacements that must occur once to a `git archive` copy of the final tree in
`E:/projects/cfd/w49i-work/f472-scratch` (own target directory); the named test is run once; the named check must fail. The
copy is compared with the worktree afterwards.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | L not applied: both pressure gradients stay Green-Gauss with the flag on | L0 |
| MU2 | L switched on in `OversetSolver::new` | D1b (and D1: the default steps are then refused) |
| MU3 | the default predictor gradient takes one skewness pass (`green_gauss(.., 1)` for `grad_p` with L off) | D1 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### Reading of a missed acceptance ratio (fixed now)

For each of A1, A1d, A2, A2d that misses: if the separating ratio of the same interpolation (L1 for A1, A1d; L2 for A2, A2d)
is met, the cause is named as **the uncorrected Green-Gauss pressure gradient**, with its share `sigma` (or `sigma_q`) and K2,
K4 as the operator-level evidence; if the separating ratio misses too, the cause is named as **the Green-Gauss gradient and a
remainder** (the part `E(W3oL) - 2 E(P3oL)`), located by the class and place of the largest error of W3oL, and reported
without a further diagnosis by this item.

### Classes

A miss of A1, A1d, A2 or A2d is kept as an ignored `*_known_miss` test with the limit intact and read by the rule above. A miss
of A3, L3, L4 or D1 to D3 is a defect of the opt-in or of its isolation (reported; the defaults are never changed to meet a
check). A miss of L1, L2 or K1 to K4 is a finding against the derivation (reported, not tuned). A miss of L0, U4, Q1, or a
control that does not hold, is a defective set-up, corrected by an amendment before any corrected run.

## Predictions (mine, before any run)

K1, K3 near `1e-14`; K2 near `0.1`; K4 near `0.15`. `E(P3o) = E(P3)` near `1.5e-3`; `E(W3o)` near `1e-2`: A1, A1d, A2, A2d
missed with ratios near `5`; `E(W3oL)` near `3e-3`, L1 and L2 near `1.8` (met, not by much); the share `sigma` near `0.7`.
A3, L3 near `2e-14`; L4 near `1e-14` and `1e-13`; U4 `99` and `92`; Q1 `1.0`; N1 near `1e-3`; D1, D1b equal; D2, D3 pass. The
mutations fail their checks.

## Decision rule

- **done**: A1, A1d, A2, A2d, A3 met; D1, D1b, D2, D3 met; L0 to L4, R1, K1 to K4, U4, Q1 met; N1 held; MU1 to MU3 caught by
  their registered checks; the share A5 printed.
- **partial**: A3, L3 and D1 to D3 met, the share printed, and any other registered check missed (every missed acceptance
  ratio read by the rule above), or a mutation not caught by its registered check.
- **todo**: A3 or one of D1 to D3 missed, or the runs could not be made.

## Limits of this registration

- One flow (a vortex independent of `z` with a uniform axial velocity), one warp, one resolution, one motion, 40 steps
  (`t = 0.2`, the vortex decays by `0.7 %`); no order of convergence.
- The flow is chosen so that the zero-gradient pressure of fixed-velocity faces is exact; flows with a normal pressure
  gradient at a fixed-velocity boundary are not covered (the library's `pressure_bc` gives such faces the zero gradient).
- L is a diagnostic opt-in: it is not proposed as the default and it is not combined with BDF2, the prescribed fringe flux or
  `rhie_chow_step_consistent` (the corrections refuse them).
- No change of a default, golden or hash table is needed or made.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
