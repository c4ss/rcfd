# ADR 0264: F-359 registration: a non-uniform closed-form flow (a linear shear flow u = U0 + A x) on the warped three-dimensional overset mesh with the component moving

Status: **REGISTERED 2026-11-12**, written before any code of F-359 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run and nothing was compiled: the code and the documents listed under "What was
read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the
addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0255](0255-f357-registration-uniform-flow-on-a-warped-overset-mesh.md) (F-357: the uniform flow on B3d with
the component moving, the motion `P(t)`, the test `crates/rcfd-fv/tests/overset_f357.rs` whose set-up code is reused, its
question Q1 which is this item), [ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md) (F-336: the
set-ups B3 and B3d, the quadratic donor interpolation), [ADR 0253](0253-f352-registration-overset-donor-search-on-non-planar-faces.md)
and [ADR 0260](0260-f356-registration-overset-donor-search-on-non-convex-cells.md) (the donor search on warped and
non-convex cells, which makes B3d usable), [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md)
(F-134: `OversetSolver`, the model M1 to M11, the settings PS). Roadmap item F-359 (depends on F-357).

Number: the lead gave 0264 to F-359. Branch `f359-warped-overset-flow` from main 2fa39daf, worktree `w49i-work/f359`.

House rules (ADR 0130, ADR 0213, ADR 0219, ADR 0251, ADR 0255, and the campaign rules of 2026-11-12):

- limits are fixed here, before the code and the runs, and are not changed after a run; the tests and ignored records of
  F-134, F-314, F-334, F-335, F-336, F-338, F-352, F-356 and F-357 are not edited;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- this is a measuring item: it adds a test file and this text. No library file is edited. If a check fails and a cause is
  found in the library, the cause is reported and the library is not changed by this item;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded;
- each registered run is made once, and nothing is re-run to pick a result (a deterministic run that two tests share is the
  same run; the tests share it through a `OnceLock`, so it is computed once per test process);
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. Nothing outside this repository was opened for this work. The flow is an elementary exact solution of the
incompressible Navier-Stokes equations, derived below; the statements about the discrete operators are derived from the
library's own code, read for this item.

## What F-359 asks

The roadmap entry (acceptance, quoted, binding): "A three-dimensional closed-form flow (a divergence-free linear field u = A
x with its pressure, or a decaying three-dimensional vortex with a known solution) on the set-ups B3 and B3d of ADR 0251
with the component moving as in ADR 0255: the error of the calculated and fringe cells on B3d at most twice that on B3,
with the default and with the quadratic donor interpolation, and continuity of the calculated cells within 1e-9 at every
step."

**Reading (registered here).**

1. "A divergence-free linear field u = A x with its pressure": the linear shear flow F below, `u = U0 + A (x - x0)` with
   `A = s a b^T` (an affine field; the constant `U0` makes the flow cross the whole domain and is allowed by "linear" in the
   sense of a field of degree one). Its pressure is constant (derived below).
2. "On the set-ups B3 and B3d of ADR 0251 with the component moving as in ADR 0255": the two parts of B3 (planar faces)
   and of B3d (interior points displaced by up to `0.15 h`, warped faces) exactly as in `overset_f357.rs`, the component at
   `P(0)` and moved to `P(k dt)` in step `k`, 40 steps of `dt = 0.005`, the hole and the fringe rebuilt every step by
   `OversetSolver::prepare`/`advance`. Only the boundary conditions differ from ADR 0255 (the flow needs other values).
3. "The error of the calculated and fringe cells": `E = max over the steps k = 1..40 and over the cells that are not holes
   of abs(u_c - u_ex(x_c)) / abs(U0)`, `x_c` the cell centre of the current mesh (the volume centroid). Printed with it, no
   limit: the error at the last step, its volume-weighted root mean square, and the largest error of each class (BC
   calculated background, BF fringe background, CC calculated component, CF fringe component) with the cell and step where
   it lies.
4. "On B3d at most twice that on B3": `E(B3d) <= 2 E(B3)` for the same motion, settings and interpolation.
5. "With the default and with the quadratic donor interpolation": two pairs of runs, the second with
   `Overset::set_donor_interpolation(DonorInterpolation::Quadratic)` on both B3 and B3d.
6. "Continuity of the calculated cells within 1e-9 at every step": `OversetInfo::continuity` `<= 1e-9` in every step of
   the four runs of 4 and 5.

## What was read (no run)

- ADR 0255 in full, `crates/rcfd-fv/tests/overset_f357.rs` in full; ADR 0251 (the set-ups, D1 to D5, amendment 1, the
  results); of ADR 0253 the set-up B3d and the checks A1 to A3; of ADR 0260 its B3d remark; of ADR 0219 the TG_L set-up and
  `overset_f134.rs` (`tg_conditions`, the per-face Dirichlet profile, `tg_error`).
- `crates/rcfd-fv/src/overset.rs` `piso` (the momentum predictor, M6 to M8, the pressure solve) and the refusals;
  `crates/rcfd-fv/src/pressure.rs` (`pressure_bc`: a face whose velocity is prescribed gets a zero-gradient pressure;
  `face_interpolate`); `crates/rcfd-fv/src/diffusion.rs` `laplacian_impl` (over-relaxed orthogonal part
  `abs(S)^2 / (S . d)`, the explicit non-orthogonal part only when a gradient is passed); `crates/rcfd-fv/src/convection.rs`
  (the central face value `w phi_o + (1 - w) phi_n`, `face_flux`); `crates/rcfd-fv/src/gradient.rs` `green_gauss`;
  `crates/rcfd-fv/src/geometry.rs` (the weights `d_n / (d_o + d_n)`); `rcfd_mesh::overset::linear_weights`.

What the code shows, relevant here: in the overset step (also in its "trivial" path without coupling, used for the single
meshes below) the momentum diffusion is `laplacian_opt(.., grad = None, ..)`, i.e. **without the explicit non-orthogonal
correction**; the pressure Laplacian is `laplacian(.., None)`, also without it, and the solver refuses non-orthogonal
correctors; the pressure gradient is Green-Gauss with **no skewness correction** (`green_gauss(.., 0)`); the convected face
value and the Rhie-Chow `HbyA` flux use the **linear weights along the centre line with no skewness correction**. On a
Cartesian mesh none of these omissions matters for a linear field (derived below); on a warped mesh each one leaves an
error proportional to the velocity gradient. That is what this item measures.

## The flow F and why it is exact

`U0 = (1, 0.5, 0.25)` (the flow of ADR 0255), `x0 = (1.2, 1.2, 1.2)` (the centre of the background), the unit vectors `a =
(1, 2, 1) / sqrt(6)` and `b = (0, 1, -2) / sqrt(5)`, the shear rate `s = 0.5`:

`u_ex(x) = U0 + s a (b . (x - x0))`,  `p_ex = 0`,  so `grad u = s a b^T = A` (constant, rank one).

The incompressible Navier-Stokes equations with unit density, `du/dt + (u . grad) u = - grad p + nu lap u`, `div u = 0`:

- `div u = trace A = s (a . b) = s (0 + 2 - 2) / sqrt(30) = 0`.
- Convective acceleration: `(u . grad) u = A u = s a (b . u)`, and `b . u = b . U0 + s (b . a)(b . (x - x0)) = (0.5 - 0.5)
  / sqrt(5) + 0 = 0`. So `(u . grad) u = 0` everywhere: the pressure that balances it is a constant, and `p_ex = 0` is the
  one the reference level below picks. (For a general traceless `A` the convective acceleration is `A U0 + A^2 (x - x0)`,
  which a pressure balances only when `A^2` is symmetric, with a quadratic `p`; such a `p` has a normal gradient on the
  walls, which the solver's zero-gradient pressure on prescribed-velocity faces would contradict, and a quadratic `p` is not
  reproduced by the default donor stencils. The rank-one choice with `b . U0 = 0` and `a . b = 0` avoids both, so that the
  only errors left are those of the geometry. In three dimensions `A^2 = 0` forces rank one: `A u = 0` along the flow is the
  shear `a b^T` with `b` normal to `a`.)
- Viscous term: `lap u = 0` because `u` is linear; also `div (grad u + grad u^T) = grad (div u) + lap u = 0`, so the result
  does not depend on whether the stress is written in full. F is an exact steady solution for every `nu`.
- Time: steady in the absolute frame. The component cells move, so their centre values `u_ex(x_c(t))` change at the rate
  `A w_c` (`w_c` the centre velocity); the ALE form must carry that through its mesh flux.

Boundary values. All six patches of the background: velocity fixed to `u_ex` at the face centres (`FixedValue` with
`Profile::PerFace`, set once; the background does not move). The pressure keeps its default condition; on prescribed-velocity
faces `pressure_bc` forces zero gradient, which is exact because `grad p_ex = 0`. With no Dirichlet pressure the level is
fixed by the reference `p_ref = (0, 0.0)` of `PimpleSettings::piso` (cell 0 is the calculated background corner cell);
`p_ex = 0` is consistent with it. The net boundary flux of `u_ex` is zero: the boundary faces of both B3 and B3d are planar
squares (the distortion leaves boundary points in place) and the midpoint rule integrates a linear field exactly, so the sum
is `integral div u_ex = 0` to round-off. The outer patches of the component are its fringe and need no value. In the single
mesh runs below, the six patches of the mesh get `u_ex` at the face centres of the current mesh, every step for a moving
mesh (computed from the geometry after `move_mesh`, before `advance_mesh`).

Size of the field over the background box: `b . (x - x0)` lies in `[-1.610, 1.610]` (`1.2 (1 + 2) / sqrt(5)`), so `abs(u -
U0) <= 0.805` and `abs(u) <= 1.95`; Courant number `<= 1.95 * 0.005 / 0.1 = 0.1`, with the mesh speed of ADR 0255 (`<=
1.04`) relative `<= 0.15`.

## Why the planar B3 keeps F (up to the time error of the moving component), and B3d cannot

On a Cartesian part (the background of B3, and the component of B3, a turned and shifted Cartesian block), with cell centres
at the centroids and every internal face midway between its two centres with its normal along the centre line (`w = 1/2`,
`S` parallel to `d`):

1. **Central face value**: `(u_P + u_N) / 2 = u_ex(x_f)` exactly for a linear field. The face flux of `u_ex`, `u_ex(x_f) .
   S_f`, is the exact integral over the planar face (midpoint rule, linear integrand).
2. **Convection**: `sum_f phi_f u_f` against `integral_cell div(u u) = integral (u . grad) u = 0`. The midpoint rule for the
   quadratic integrand `(u . n) u` on a face errs by `(area h^2 / 24)` times its tangential second derivatives, which are
   constant (the field is linear) and change sign with `n` on the opposite face: the errors cancel in pairs, so the discrete
   sum is the exact integral, zero.
3. **Diffusion** without the correction: `nu abs(S) (u_N - u_P) / abs(d) = nu (A d) abs(S) / abs(d) = nu A S` for `S`
   parallel to `d`; it sums to `nu A sum_f S_f = 0` over the closed cell. Dirichlet faces: `(u_b - u_P) / (h / 2)` is the
   exact normal derivative.
4. **Pressure**: `p = 0` exactly, its Green-Gauss gradient is zero; `HbyA = u` for a state that satisfies the discrete
   momentum equation, so the Rhie-Chow flux is `u_ex(x_f) . S_f` and its divergence zero; the ddt correction of the flux
   vanishes for the same reason.
5. **Donor interpolation**: the linear stencils of M4 (`linear_weights`, a weighted least-squares fit of the monomials of
   degree one) and the quadratic ones (ADR 0251) reproduce linear fields exactly on any mesh, and a rigid motion maps linear
   fields to linear fields. So the fringe values, the interpolated `HbyA` and the fringe velocity after a corrector are
   exact, and a cell that changes kind takes or gives an exact value.
6. **The moving component** (ALE, implicit Euler): the discrete transport of a cell centre value through the mesh flux,
   `V (u_c1 - u_c0) = sum_f u_ex(x_f1) W_f`, is exact for a translation (`sum_f x_f S_f^T = V I` on a planar polyhedron)
   but not for a rotation: evaluating the face value at the end of the sweep instead of the centroid of the swept volume
   leaves, per cell and step, `(V / 2) A Omega Delta_0` with `Omega` the rotation increment and `Delta_0` the displacement of
   the centre (the derivation: `sum_f (S_f . Delta_f) Delta_f / 2` with `Delta_f = Delta_0 + Omega (x_f - x_c)`, the term in
   `Delta_0 Delta_0` vanishes by `sum S_f = 0`, the term `Delta_0 trace(Omega)` by the skew `Omega`, the rest is `V Omega
   Delta_0 / 2`). As an acceleration `abs(A) omega abs(w) dt / 2 <= 0.5 * 0.5 * 1.04 * 0.005 / 2 = 6.5e-4`. This is the
   first-order time error of the scheme on a turning mesh, the same on B3 and B3d, and it is what B3 is expected to show.

On B3d none of the cancellations of 1 to 3 holds exactly: the face centre is off the centre line (**skewness**), `S` is not
parallel to `d` (**non-orthogonality**) and the faces are warped. For a linear field the errors are first order in the
displacement and proportional to `abs(A)`:

- central face value: `A (x_f - x_f')`, `x_f'` the point of the centre line that the weights interpolate to;
- diffusion without correction: `- nu A k_f`, `k_f = S_f - d abs(S_f)^2 / (S_f . d)` the non-orthogonal part;
- Rhie-Chow flux: the interpolated `HbyA` carries the same skewness error as the face value, so the continuity residual of
  the exact field is not zero, and the pressure equation answers it with a non-zero pressure.

These are inconsistencies of order zero in `h` for the operators (their sum over a cell, divided by the volume, does not
vanish as `h -> 0` at a fixed relative distortion), not round-off. **This is why F is a fair probe of the warped geometry and
non-orthogonality, unlike the uniform flow**: a uniform field has a zero gradient, so every one of these terms multiplies
zero (ADR 0255 could only see the closure of the area vectors, the GCL and the sum of the weights); a linear field has a
constant non-zero gradient, so it excites every one of them, while on the planar B3 it is reproduced exactly by the same
operators (1 to 5). The comparison B3d against B3 then isolates the geometry: B3 carries only the time error of item 6,
which B3d carries too.

The consequence for the acceptance, stated before the runs: unless these geometric terms happen to be small against the
time error of item 6, `E(B3d) <= 2 E(B3)` will be missed, and the miss will lie in the finite-volume operators of a single
warped mesh, not in the coupling. The separating checks S below are registered to decide that by measurement.

## Arithmetic before this registration (disclosed, by hand)

- B3d displacement: each coordinate of an interior point uniform in `[-0.015, 0.015]` (standard deviation `0.0087`). A face
  centre (mean of four points) moves by about `0.0043` per coordinate, a cell centre (eight points) by about `0.0031`. The
  tangential offset of a face centre from the centre line of its two cells, `abs(x_f - x_f')`, is then about `0.005` to
  `0.007` (`0.05 h` to `0.07 h`), the angle between `S` and `d` about `0.04` to `0.06`.
- Truncation of the exact field on the warped background (the residual per unit volume of the discrete steady equations
  with `u_ex` in the cells), with `abs(A) = s = 0.5`, faces of area `h^2 = 0.01`, `V = 1e-3`, six faces with random signs
  (`sqrt(6) = 2.45`):
  - convection: `phi ~ 1.15 * 0.01`, face value error `~ 0.5 * 0.006 = 3e-3`: `2.45 * 0.0115 * 3e-3 / 1e-3 ~ 0.08`;
  - diffusion: `nu abs(A) abs(k) ~ 0.01 * 0.5 * 0.05 * 0.01`: `2.45 * 2.5e-6 / 1e-3 ~ 6e-3`;
  - continuity of the central face flux: `2.45 * 3e-3 * 0.01 / 1e-3 ~ 0.07` per second.
  On the planar meshes all three are round-off (items 1 to 3): about `1e-14` (terms of size `phi u / V ~ 20` cancelling).
- Velocity error on B3d: a steady random forcing of `0.08` per cell, balanced by the convection over a cell (`f h /
  abs(u)`) gives `~ 7e-3`, and grows at most like `f t = 0.016` over the run; the pressure answers the continuity residual
  with `p ~ rAU^{-1}` times the residual times `h^2 / 6`: with `rAU ~ dt` about `0.07 / 0.005 * 0.01 / 6 = 0.02`. Expected
  `E(B3d)` between `3e-3` and `3e-2`.
- Velocity error on B3: the forcing `6.5e-4` of item 6 in the component, of which part is a gradient that the pressure
  removes; at most `6.5e-4 * 0.2 = 1.3e-4` absolute, `1.1e-4` relative to `abs(U0) = 1.146`. Expected `E(B3)` between
  `1e-5` and `2e-4`.
- So the ratio is expected near `30` to `1000`, far above `2`, for both interpolations (the donor interpolation is exact for
  F in both modes; the two pairs should differ only in the pressure noise that the fringe rows carry).
- The control N1 (weights times `1.001` on B3): a fringe value off by `1e-3 abs(u_c) >= 1e-3 * 0.4`; with `abs(u_c)` up to
  `1.95`, `E` at least `5e-4` near the fringe.
- Cost: ADR 0255 measured `5.4` s for W3 and `4.5` s for P3 of 40 steps; F adds only per-face boundary values. About ten
  overset runs and five single-mesh runs: about one minute in all, so no run needs to be an ignored heavy record.

## Set-ups

PS as ADR 0219 and ADR 0255: `PimpleSettings::piso(nu)`, central convection, two correctors, no non-orthogonal correctors,
implicit Euler, `ddt_correction` on, momentum and pressure `tol_abs = 1e-15`, `tol_rel = 1e-12`, at most 3000 iterations;
`nu = 0.01`; mass mode conservative; `dt = 0.005`, 40 steps; the motion `P(t)` of ADR 0255. Start `u = u_ex` at the cell
centres, `p = 0`; the solver builds the first fringe values and the flux.

- **P3** / **P3q**: B3 (planar), default / quadratic interpolation.
- **W3** / **W3q**: B3d (warped), default / quadratic interpolation.
- **N1**: P3 with every stencil weight multiplied by `1.001` after `prepare` (as N1 of ADR 0255).
- **X1** (diagnostic, printed): the warped background of B3d with the planar component of B3, default interpolation.
  **X2** (diagnostic, printed): the planar background of B3 with the warped component of B3d, default.
- Single meshes, no coupling (`OversetSolver` without coupling, `advance_mesh` with `move_mesh`, as in ADR 0255; every cell
  calculated; all six patches of the mesh `u_ex`):
  - **PB**: the planar background of B3 at rest.
  - **PC1**: the planar component of B3 moving with `P(t)` (its reference points mapped by `P(k dt)` every step).
  - **SB**: the warped background of B3d at rest.
  - **S0**: the warped component of B3d at `P(0)`, at rest.
  - **S1**: the warped component of B3d moving with `P(t)`.
- **T** (kinematic, no step): on the background of B3 and of B3d at rest and on the component of B3 and of B3d at `P(0)`:
  the cells hold `u_ex(x_c)`, the boundary faces `u_ex(x_f)`; the face flux `face_flux` of that field (the central flux the
  solver builds); per cell and divided by the volume: `T_conv` the residual of `convection(phi, u_ex, Central)`, `T_diff`
  the residual of `laplacian(nu, grad = None)` (as the overset step assembles it), each the largest over the components,
  and `T_cont` the divergence of the flux. Largest over the cells and root mean square.

## Registered checks

New test file `crates/rcfd-fv/tests/overset_f359.rs`, release (`cargo test --release -p rcfd-fv --test overset_f359`). No
existing test is edited.

### A. The acceptance

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| A1 | default interpolation: `E(W3) / E(P3)` | `<= 2` | `30` to `1000` (arithmetic above): expected missed |
| A2 | quadratic interpolation: `E(W3q) / E(P3q)` | `<= 2` | the same: expected missed |
| A3 | continuity of the calculated cells, every step of P3, P3q, W3, W3q | `<= 1e-9` each | ADR 0255 `6e-15`; the solves converge to `1e-12` relative |

### K. Exactness checks of the derivation

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| K1 | every step of P3, P3q, W3, W3q: every stencil reproduces `u_ex`, `max abs(sum_k w_k u_ex(x_k) - u_ex(x_c)) / abs(U0)` over the stencils | `<= 1e-12` | round-off; ADR 0253 A2 had `1e-15` for a linear field on B3d |
| K2 | T on the planar background and the planar component: `T_conv`, `T_diff` (per unit volume) and `T_cont` (per second), largest | `<= 1e-10` each | items 1 to 3; round-off near `1e-14` on terms of size `20` |
| K3 | PB: `E` and the largest `abs(p_c) / abs(U0)^2` over the steps; its continuity | `<= 1e-10`, `<= 1e-10`; `<= 1e-9` | the derivation (the planar single mesh at rest keeps F to round-off) |
| K4 (set-up) | the net boundary flux of `u_ex` on the background of B3 and of B3d, over `abs(U0) * 2.4^2` | `<= 1e-13` | midpoint rule on planar boundary faces |

### U. Set-up checks of the runs

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| U4 | kind changes summed over the steps, P3 and W3 | `>= 20` each | ADR 0255: 92 and 99 |
| Q1 | P3q and W3q, every step: the share of the fringe cells with a quadratic stencil | `>= 0.95` | ADR 0255 A2s: `1.0` |
| U5 | unconverged linear solves of every run | printed, no limit | |

### S. Separating checks (used only if A1 or A2 misses; measured either way)

`E_M = max(E(SB), E(S1))`, the error of the warped meshes alone, without coupling, at rest (background) and moving
(component). Printed with it: `E(S0)`, `E(PC1)`, `E(X1)`, `E(X2)`, the per-class errors of every overset run, T on the
warped meshes.

Reading, fixed now, for each of A1 (with `W3`) and A2 (with `W3q`) that misses:

- **the single warped mesh** is the cause when `E_M >= 0.5 E(W3)` (the warped meshes alone, with no overset, produce at
  least half of the overset error) and K2 and K3 are met (the planar operators are exact, so the difference is the
  geometry); X1 and X2 then say which part carries it, and T on the warped meshes shows which operator is inexact
  (`T_conv` against `T_diff`, both accelerations; `T_cont` is printed beside them);
- **the overset coupling on the warped mesh** is the cause otherwise (`E(W3) > 2 E_M`), and the class and place of the
  largest error say where (fringe or calculated, background or component).

The two branches cover every case.

Stated plainly: the first branch is the expected one. Naming the operator by T is a printed diagnosis, not a limit.

### N. Negative control

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| N1 | N1 (P3 with the weights times `1.001`): `E` | `>= 3e-4` | `>= 5e-4` (arithmetic above): the measure sees an error of the fringe values on the planar set-up, so a small `E(P3)` is not a blind measure |

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f359-scratch` (own target
directory), the new test file is run once, and the named check must fail; the numbers go in the addendum. The copy is
restored and compared byte for byte afterwards.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `rcfd_fv::convection`, the central face value: `w phi_o + (1 - w) phi_n` becomes `(w + 0.1) phi_o + (0.9 - w) phi_n` (still exact for a uniform field) | K2 (`T_conv` on the planar meshes) and K3 |
| MU2 | `rcfd_mesh::overset::linear_weights`: the returned weights replaced by the normalised kernel weights `omega_k / sum omega` (they sum to one, so a uniform field is still exact, but a linear one is not) | K1 on P3 and W3 |
| MU3 | `rcfd_fv::overset::OversetSolver::piso`: the corrected flux built from the pressure before the corrector (`p_old`) instead of the new one | A3 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### Classes

A miss of A1, A2 or A3 is reported with its number and kept as an ignored `*_known_miss` test with the limit intact; its
cause is diagnosed by the separating checks S. A miss of K1 to K3 is a finding against the derivation (reported, the
derivation is not edited). A miss of K4, U4 or Q1 is a defective set-up: corrected by an amendment before any corrected run.
A control (N1) that does not hold, or a mutation that its check does not catch, is a defective set-up too, never counted as
a pass.

## Predictions (mine, before any run)

A3 met near `1e-14`. K1 near `1e-15`, K2 near `1e-14`, K3 near `1e-14`, K4 near `1e-16`. `E(P3)` and `E(P3q)` between
`1e-5` and `2e-4`, largest in a component cell; `E(W3)` and `E(W3q)` between `3e-3` and `3e-2`; **A1 and A2 missed** by a
factor `30` to `1000`. `E(SB)` and `E(S1)` within a factor of two of `E(W3)` (the single warped mesh is the cause); `E(PC1)`
near `E(P3)`; X1 and X2 each near `E(W3)`, larger on the part with more cells near the largest error. T on the warped
meshes: `T_conv` near `0.08`, `T_diff` near `6e-3`, `T_cont` near `0.07` (largest over the cells two to three times these);
so the convective skewness and the Rhie-Chow continuity residual dominate and the missing non-orthogonal correction of the
diffusion is the smaller term at `nu = 0.01`. N1 near `1e-3`. MU1 fails K2 near `1e1` and K3; MU2 fails K1 near `1e-2`;
MU3 fails A3, I do not know by how much (the pressure changes little between correctors once the flow has settled).

## Decision rule

- **done**: A1, A2 and A3 met; K1 to K4, U4, Q1 met; N1 held; the mutations were caught.
- **partial**: A3 met and A1 or A2 missed with its cause diagnosed by the separating checks (either branch), the other
  checks met or their misses diagnosed; or a control or mutation did not hold and was not corrected.
- **todo**: A3 missed, or A1 or A2 missed without a diagnosed cause, or the runs could not be made.

## Limits of this registration

- One flow (a steady linear shear with constant pressure), one warped mesh family, one motion, 40 steps, one resolution: no
  order of convergence, and no flow whose pressure varies (the pressure path is probed only through the pressure noise that
  the continuity residual of the warped mesh produces).
- The overset step has no non-orthogonal or skewness correction and refuses non-orthogonal correctors; this item measures
  the consequence and does not change it.
- The background does not move; the component moves rigidly.
- A decaying three-dimensional vortex (the other flow the acceptance allows) is not run: its truncation error on B3 is of
  second order everywhere, which would let a geometric error of the same size hide inside the factor two; the linear field
  leaves B3 with only the time error and so shows the geometric error alone.

## Open questions

Listed with options and a recommendation in the addendum, after the results.

## Addendum 1: results (2026-11-12)

Measured against the limits registered above; none was changed and no amendment was needed. Release builds on Windows.
Commits: the registration `5b0c067a` (alone, before any code); the test file, compiled, formatted and linted without
running, `076138d8`. The test file was then run once (`cargo test --release -p rcfd-fv --test overset_f359 -- --nocapture
--test-threads=2`, 53 s for the seven tests, each acceptance run computed once and shared). After the run, A1 and A2 were
made ignored `*_known_miss` records with their limits intact (`445ac769`). No library file was changed. The logs are in
`E:/projects/cfd/w49i-work/f359-scratch` (`f359-run1.log`, `f359-MU1.log` to `f359-MU3.log`).

### Checks

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A1 default interpolation: `E(W3) / E(P3)` | `<= 2` | `15.60` (`E(P3) = 8.7203e-4`, `E(W3) = 1.3603e-2`) | **missed** |
| A2 quadratic interpolation: `E(W3q) / E(P3q)` | `<= 2` | `15.57` (`E(P3q) = 8.7204e-4`, `E(W3q) = 1.3576e-2`) | **missed** |
| A3 continuity of the calculated cells, every step: P3, P3q, W3, W3q | `<= 1e-9` | `1.91e-14`, `1.89e-14`, `1.44e-14`, `9.14e-15` | met |
| K1 the stencils reproduce `u_ex`, every step: P3, P3q, W3, W3q | `<= 1e-12` | `1.30e-15`, `3.33e-15`, `1.39e-15`, `2.78e-15` | met |
| K2 T on the planar background: `T_conv`, `T_diff`, `T_cont` | `<= 1e-10` | `1.82e-14`, `3.90e-15`, `8.67e-15` | met |
| K2 T on the planar component at `P(0)` | `<= 1e-10` | `1.39e-14`, `4.34e-15`, `9.54e-15` | met |
| K3 PB (planar background alone): velocity, pressure, continuity | `<= 1e-10`, `<= 1e-10`, `<= 1e-9` | `8.16e-15`, `3.63e-14`, `6.59e-15` | met |
| K4 (set-up) net boundary flux of `u_ex`, B3 and B3d backgrounds | `<= 1e-13` | `1.91e-16`, `1.91e-16` | met |
| U4 (set-up) kind changes, P3 and W3 | `>= 20` | `99`, `92` | met |
| Q1 (set-up) quadratic share, P3q and W3q | `>= 0.95` | `1.0`, `1.0` (no fallback in any step) | met |
| U5 unconverged solves | printed | `0` in every run | - |
| N1 (control) P3 with the weights times 1.001: `E` | `>= 3e-4` | `1.85e-3` | held |

Errors per class (largest over the steps; BC, BF, CC, CF) and where the largest lies; every largest error is at the last
step (step 40), so the errors were still growing at the end of the run:

| Run | `E` | cell | BC | BF | CC | CF | rms at step 40 | pressure `max abs(p) / abs(U0)^2` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P3 | `8.720e-4` | 16202 (CC) | `2.45e-4` | `1.63e-4` | `8.72e-4` | `2.22e-5` | `8.13e-5` | `4.76e-4` |
| P3q | `8.720e-4` | 16202 (CC) | `2.45e-4` | `1.65e-4` | `8.72e-4` | `2.23e-5` | `8.13e-5` | `4.76e-4` |
| W3 | `1.360e-2` | 14389 (CC) | `9.36e-3` | `4.23e-3` | `1.36e-2` | `5.97e-3` | `2.34e-3` | `3.15e-2` |
| W3q | `1.358e-2` | 14389 (CC) | `9.41e-3` | `4.51e-3` | `1.36e-2` | `6.87e-3` | `2.35e-3` | `3.15e-2` |

Run times (two tests at once): P3 7.6 s, P3q 10.2 s, W3 13.5 s, W3q 13.7 s, the single meshes 0.8 to 4.9 s, X1 9.1 s,
X2 8.5 s.

### The separating runs and the diagnosis

| Run | What it is | `E` | largest at | pressure |
| --- | --- | --- | --- | --- |
| PB | planar background alone at rest | `8.2e-15` | | `3.6e-14` |
| PC1 | planar component alone moving | `2.13e-4` | cell 405 | `1.80e-3` |
| SB | warped background alone at rest | `9.57e-3` | cell 6130 | `3.14e-2` |
| S0 | warped component alone at rest | `8.38e-3` | cell 565 | `2.98e-2` |
| S1 | warped component alone moving | `1.21e-2` | cell 565 | `3.09e-2` |
| X1 | warped background, planar component, overset | `9.27e-3` | cell 6130 (BC) | `3.12e-2` |
| X2 | planar background, warped component, overset | `1.35e-2` | cell 14389 (CC) | `2.43e-2` |

T on the warped meshes (largest over the cells, root mean square in brackets): background `T_conv = 0.140` (`0.032`),
`T_diff = 0.0246` (`0.0054`), `T_cont = 0.0751` (`0.0190`); component `0.130` (`0.030`), `0.0245` (`0.0052`), `0.0742`
(`0.0190`). On the planar meshes all three are below `2e-14`.

The registered rule: `E_M = max(E(SB), E(S1)) = 1.21e-2`; `E_M / E(W3) = 0.889` and `E_M / E(W3q) = 0.891`, both `>= 0.5`,
and K2 and K3 are met. **By the rule the cause of both misses is the single warped mesh, not the overset coupling.** The
other numbers say the same thing in more detail:

- The largest error of W3 and W3q lies in cell 14389, which is cell 565 of the component (`14389 - 13824`): the same cell
  as the largest error of the warped component alone, at rest (S0) and moving (S1). The largest error of X1 lies in
  background cell 6130, the same cell as in SB. So the overset run's worst cells are those of each warped part alone.
- X2 (only the component warped) has `1.35e-2`, the same as W3; X1 (only the background warped) has `9.3e-3`, the same as
  SB. Each part carries its own error into the overset run, unchanged by the coupling.
- On the planar meshes the operators are exact for F (K2: `1e-14`, K3: `8e-15`). On the warped meshes the convective face
  value (skewness) is the largest inexact operator, `T_conv` about six times `T_diff` (the missing non-orthogonal
  correction of the diffusion) at `nu = 0.01`, and the central flux of the exact field leaves a continuity residual
  `T_cont` that the pressure equation answers with a pressure of about `3e-2 abs(U0)^2`, where the exact pressure is zero.
  This is the size of the inconsistency predicted under "Why the planar B3 keeps F": first order in the displacement,
  zeroth order in `h`.

Said plainly: the overset step on the warped mesh is as accurate as the finite-volume step on one warped mesh; both lose
the exactness for linear fields that they have on the planar mesh, by about `1e-2`, because the overset step (and its
path without coupling) uses no skewness correction of the face values and no non-orthogonal correction. The factor `2`
of the acceptance is therefore not a property the coupling can reach on its own: B3 carries only the time error of the
turning mesh and the small error discussed next.

### Deviations and disclosures

- **`E(B3)` was larger than predicted**: `8.72e-4` against the registered estimate `1e-5` to `2e-4`. The planar component
  alone moving (PC1) has `2.13e-4`, inside the estimate (the time error of a turning mesh, item 6). The rest comes from the
  coupling on planar meshes: the largest P3 error lies in a calculated component cell (16202, component cell 2378, the
  second layer from the component's outer boundary, i.e. next to its fringe), it is the same with both interpolations
  (both are exact for F, K1), and N1 shows the measure sees fringe defects. Where in the coupling it arises (the fringe
  pressure, which carries the background's pressure noise into the component; the mass offset; the M9 flux of faces that
  change kind) was not measured: it is not part of the acceptance and makes the ratio smaller, not larger. Had `E(P3)` been
  `2e-4` as predicted, the ratio would have been about `68`. Listed as question Q2.
- Predictions that held in size: `E(W3)` `1.4e-2` (predicted `3e-3` to `3e-2`); the misses of A1 and A2 (predicted, by a
  factor of `30` to `1000`; measured `15.6`, smaller because of the item above); the separation (predicted: the single
  warped mesh); T on the warped meshes (predicted `0.08`, `6e-3`, `0.07` as typical values, largest two to three times
  these; measured largest `0.14`, `0.025`, `0.075`, root mean square `0.032`, `0.0054`, `0.019`); N1 (predicted near
  `1e-3`, measured `1.85e-3`).
- The errors were still growing at step 40 in every run; the ratio after 40 steps is what was registered. A longer run was
  not made (it would be a new run chosen after seeing the result).
- No library code was changed: the cause is a known property of the operators (they carry no skewness or non-orthogonal
  correction in the overset path, and the overset solver refuses non-orthogonal correctors), not a defect that a measuring
  item should repair.

### Mutations

Applied alone to a `git archive` copy of `445ac769` (`f359-scratch/mut`, own target directory) by `mutate.py` (exact
replacements that must occur once), the test file run once (the two known-miss records stay ignored), the file restored
and compared byte for byte with the worktree (equal for all three).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 the central face value weighted `w + 0.1`, `0.9 - w` | K2 and K3 | failed: K2 `T_conv` `4.08e-2` (background) and `4.34e-2` (component); K3 velocity `8.68e-3`, pressure `3.84e-3`. A3 still met |
| MU2 `linear_weights` returns the normalised kernel weights | K1 on P3 and W3 | failed: K1 `2.06e-2` (P3) and `2.53e-2` (W3); the quadratic runs unchanged (no fallback) |
| MU3 the corrected flux built from the pressure before the corrector | A3 | failed: continuity `1.67e-6` (P3, P3q) and `1.05e-4` (W3, W3q) |

`scripts/limit_audit.py` on the new test file: no hit.

### After the tests

`cargo fmt -p rcfd-fv --check` clean (after `rustfmt` of the new file before its commit). `cargo clippy -p rcfd-fv
--all-targets --release -- -D warnings` clean. Only `rcfd-fv` (a new test file) touched. What the gate runs of this file:
the five tests that are not ignored, about 50 s (they compute all four acceptance runs for A3 and K1).

### Not run

- The whole workspace test suite (not asked for; the lead runs the gate). No library file was changed, so no existing test
  was re-run.
- The decaying three-dimensional vortex the acceptance also allows (see "Limits"); a second resolution or a second warp
  amplitude; a run longer than 40 steps; the operators with skewness and non-orthogonal corrections (the overset step has
  none and refuses the correctors).

### Decision by the registered rule

**Partial.** A3 is met (continuity `<= 1.9e-14` in every step of the four runs), K1 to K4, U4 and Q1 are met, the control
N1 held and the three mutations were caught; A1 and A2 are missed (ratio `15.6` against `2`) with the cause diagnosed by
the registered separating checks as the finite-volume operators of a single warped mesh (`E_M / E(W3) = 0.89`, the worst
cells of the overset runs being those of the warped parts alone), not the overset coupling.

### Remaining questions (each with options and a recommendation)

Each recommendation is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) as a
proposal to the lead; none is a decision the maintainer made in person.

- **Q1. Accuracy of the overset step on warped cells.** Options: (a) accept: B3d is a strong distortion (`0.15 h` random,
  faces warped up to `0.14 h`) and the error `1.4e-2` of a linear field is the price of operators without corrections;
  (b) an opt-in for the overset step: the explicit non-orthogonal correction of the momentum diffusion and of the pressure
  Laplacian (deferred, from the Green-Gauss gradient with skewness passes) and a skewness-corrected central face value,
  with the acceptance that the linear shear of ADR 0264 on B3d meets A1 and A2 and the defaults stay bit-identical; (c)
  allow the existing `non_orth_correctors` in the overset solver. Recommendation (b), as a follow-up item (text below):
  the measurement shows the gap lies in the single-mesh operators of the overset path, so the remedy belongs there; it must
  be an opt-in because it changes results.
- **Q2. The planar coupling's share of `E(B3)`.** P3 has `8.7e-4` against `2.1e-4` for the planar component alone; the
  stencils are exact for F. Options: (a) record only; (b) a small diagnostic item that separates the fringe pressure, the
  mass offset and the M9 flux by measurement on P3. Recommendation (b), at low priority: the coupling is meant to keep a
  linear field up to the time error, and this is the one place where it measurably does not.

### For the lead: roadmap entries

No roadmap file was edited.

**F-359**, proposed status: `partial`. Proposed note:

> From ADR 0255, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0264, 2026-11-12: PARTIAL. The linear shear u = U0 + 0.5 a (b . (x - x0)) with a = (1, 2, 1)/sqrt(6), b = (0, 1, -2)/sqrt(5), U0 = (1, 0.5, 0.25) (an exact steady Navier-Stokes solution with zero pressure for every viscosity, all six background patches Dirichlet) on B3 and B3d with the component moving as in ADR 0255, 40 steps: continuity of the calculated cells 1.9e-14 or less at every step with the default and the quadratic donor interpolation (met), the stencils reproduce the field to 3e-15, the planar operators keep it to 2e-14 (K2, K3); the error of the calculated and fringe cells 1.36e-2 on B3d against 8.7e-4 on B3, ratio 15.6 with either interpolation, against the limit 2 (missed, kept as ignored known-miss records). Diagnosed by measurement: the single warped mesh, not the coupling. The warped parts alone have 9.6e-3 (background at rest) and 1.21e-2 (component moving), 0.89 of the overset error, and the worst cell of the overset run is the worst cell of the warped component alone; the discrete operators leave a residual of the exact field of 0.14 (convection, skewness of the central face value), 0.025 (diffusion without the non-orthogonal correction) and 0.075 per second (continuity) on the warped cells, zero on the planar ones. Control (weights times 1.001: 1.85e-3) held; three mutations caught. No library change. Limits: one flow, one warp, one motion, 40 steps; B3's own error is 4 times that of the planar component alone (not diagnosed).

Proposed follow-up items (the numbers are the lead's):

**Overset: opt-in skewness and non-orthogonal corrections in the overset step.** Depends on F-359. Acceptance:

> An opt-in of the overset step that adds the explicit non-orthogonal correction to the momentum diffusion and the pressure Laplacian and a skewness correction to the central face value and the Rhie-Chow face flux; with it the linear shear of ADR 0264 on B3d with the component moving has an error of the calculated and fringe cells at most twice that on B3 with the default and with the quadratic donor interpolation, continuity of the calculated cells within 1e-9 at every step, the uniform flow of ADR 0255 within 1e-10, and the defaults bit-identical.

**Overset: the coupling's share of the error of a linear field on the planar B3.** Depends on F-359. Acceptance:

> On the planar B3 with the linear shear of ADR 0264 (error 8.7e-4 against 2.1e-4 for the planar component alone moving), the share of the fringe pressure, of the mass offset and of the flux of the faces that change kind measured separately, each by a run that removes it, with the cause of the excess named or the excess shown to be the time error of the turning mesh.
