# ADR 0367: F-368 registration: opt-in skewness and non-orthogonal corrections in the overset step

Status: **REGISTERED 2026-11-12, written before any code of F-368 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-368 was written, compiled or run: the code and the documents listed under "What was
read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum
at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0264](0264-f359-registration-linear-shear-flow-on-a-warped-overset-mesh.md) (F-359: the linear shear F on B3
and B3d, its runs P3, P3q, W3, W3q, its truncation T, its question Q1 option (b), from which this item comes; the test
`crates/rcfd-fv/tests/overset_f359.rs` whose set-up code is reused), [ADR 0255](0255-f357-registration-uniform-flow-on-a-warped-overset-mesh.md)
(F-357: the uniform flow on B3d with the component moving, `crates/rcfd-fv/tests/overset_f357.rs`), [ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md)
(the set-ups B3, B3d, the quadratic donor interpolation), [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md)
(the overset step M5 to M10), [ADR 0347](0347-f445-registration-overset-prescribed-fringe-flux.md) (the prescribed fringe
flux, an opt-in of the same step), [ADR 0359](0359-f442-registration-overset-bdf2-motion-remainder.md) (its defaults
fingerprint F0, whose method is reused for D1). Roadmap item F-368 (depends on F-359).

Number 0367 (given by the lead). Branch `f368-overset-skew-nonorth` from main 2ecb97f6, worktree `w49i-work/f368`, scratch
`w49i-work/f368-scratch`.

House rules (the campaign rules of 2026-11-12): limits fixed here and not changed after a run; a missed limit becomes an
ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment committed before the
corrected run; existing tests and records are not edited; `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and
the goldens are not edited or re-recorded; each registered run is made once (runs shared by several tests are computed once
per test process through a `OnceLock`); no new crate, no unsafe code, no commit trailers, no push.

Clean-room. Nothing outside this repository was opened. The corrections are derived below from first principles (the exact
flux of a linear field through the faces as the mesh itself defines them). The idea of a deferred skewness correction of
the face value with an interpolated gradient and of the over-relaxed orthogonal / explicit non-orthogonal split of the face
gradient are textbook finite-volume practice (Ferziger and Peric, Computational Methods for Fluid Dynamics, chapter on
complex geometries; Jasak, PhD thesis, Imperial College 1996), cited from memory; nothing of them is used beyond what is
derived here, and the second one is already the library's `diffusion::laplacian` with a gradient.

## What F-368 asks

The roadmap acceptance (quoted, binding): "An opt-in of the overset step that adds the explicit non-orthogonal correction to
the momentum diffusion and the pressure Laplacian and a skewness correction to the central face value and the Rhie-Chow
face flux; with it the linear shear of ADR 0264 on B3d with the component moving has an error of the calculated and fringe
cells at most twice that on B3 with the default and with the quadratic donor interpolation, continuity of the calculated
cells within 1e-9 at every step, the uniform flow of ADR 0255 within 1e-10, and the defaults bit-identical."

**Reading (registered here).**

1. "An opt-in of the overset step": a new method `OversetSolver::set_geometry_corrections(on: bool)`; off is the solver as it
   is, bit for bit. It acts on both paths of the step (with a coupling, and the trivial path without one, which the single
   mesh runs of ADR 0264 use).
2. The four corrections C1 to C4 below.
3. "The error ... at most twice that on B3": `E` exactly as ADR 0264 reading 3 (largest `abs(u_c - u_ex(x_c)) / abs(U0)`
   over the steps and the non-hole cells). B3d with the opt-in against B3; "that on B3" is taken in both senses, both
   registered: B3 with the opt-in (the same solver; on B3 the corrections vanish to round-off, derived below) and B3 with
   the default solver (the number of ADR 0264, re-run here).
4. "With the default and with the quadratic donor interpolation": the two pairs of ADR 0264.
5. "Continuity of the calculated cells within 1e-9 at every step": `OversetInfo::continuity <= 1e-9` in every step of the
   four opt-in runs.
6. "The uniform flow of ADR 0255 within 1e-10": the runs W3 and W3q of ADR 0255 (B3d, component moving, 40 steps, default
   and quadratic interpolation) with the opt-in: velocity and pressure of the non-hole cells within `1e-10` (as ADR 0255
   A1, A2) at every step, continuity within `1e-9`.
7. "The defaults bit-identical": D1 to D3 below.

## What was read (no run)

ADR 0264 in full (its addendum numbers are used below as the known values of the default), `overset_f359.rs` in full;
ADR 0255 checks A1, A2, U3 and `overset_f357.rs` lines 1 to 260 (the uniform set-up); ADR 0359 F0 (method).
`crates/rcfd-fv/src/overset.rs` in full; `diffusion.rs` `laplacian`, `laplacian_with`, `laplacian_impl`;
`pressure.rs` `face_interpolate`, `corrected_flux`, `corrected_flux_with`, `corrected_flux_opt`, `corrected_flux_impl`,
`solve_pressure_parts_grad` (the non-orthogonal passes), `boundary_hby_a`; `convection.rs` `face_value`, `convection_impl`,
`face_flux`; `gradient.rs` `green_gauss` (its skewness passes), `least_squares`; `geometry.rs` (weights, `delta`,
`lap_coeff`, `dn`); `bc.rs` `FixedValue` (coefficients with the normal distance `dn`); `rcfd-mesh/src/mesh.rs`
`polygon_geometry` and `compute_geometry`.

What the code shows: the overset step (both paths) assembles the momentum diffusion with `laplacian_opt(.., grad = None,
..)` and the pressure Laplacian with `laplacian(.., None)`, and refuses `non_orth_correctors`; the convected face value is
`w phi_o + (1 - w) phi_n` (`convection::face_value`, Central), the Rhie-Chow flux is `face_flux` of `HbyA` (the same
weights), neither with a skewness correction; there is no skewness-corrected face value anywhere in `convection.rs` (only
`green_gauss` has skewness passes, inside the gradient). The weights are `w = d_n / (d_o + d_n)` with the normal distances,
so `x_f' = w x_o + (1 - w) x_n` is the point where the centre line crosses the plane through `x_f` normal to `S_f`.

The mesh's own face geometry (`polygon_geometry`): a face of `n > 3` points is the fan of the `n` triangles `(a, b, e)` of
its edges `a b` and its vertex mean `e`; `S_f = sum_t S_t`, `S_t = (b - a) x (e - a) / 2`; `x_f = sum_t abs(S_t) c_t / sum_t
abs(S_t)`, `c_t = (a + b + e) / 3`. A warped face is therefore a surface of flat triangles, and this is the surface the
corrections below integrate over exactly.

## Design

Notation for an internal face `f` with owner `o` and neighbour `n`: `s_f = x_f - x_f'` (skewness vector), `d_f = x_n - x_o`,
`k_f = S_f - d_f abs(S_f)^2 / (S_f . d_f)` (non-orthogonal part, as `diffusion.rs`), and the **warp tensor**

`W_f = sum_t (c_t - x_f) S_t^T`  (`W_f[j][i] = sum_t (c_t - x_f)_j (S_t)_i`).

On a planar face all `S_t` are parallel to `S_f` (with positive multiples) and `x_f` is the area centroid, so `W_f = 0`
exactly up to round-off. These three are computed once per step from the step's geometry (`GeometryCorrections::new`).

**Cell gradients of the opt-in** (G). In every cell the weighted least-squares gradient `gradient::least_squares` of the
cell values with the boundary values of the field (exact for linear fields on any mesh when the boundary values are exact,
its own documentation and a line of algebra: the normal equations are solved exactly by the constant gradient). With a
coupling, a fringe cell's gradient is replaced by the donor interpolation `sum_k w_k G_k` of the least-squares gradients of
its donors (as M6 interpolates `rAU` and `HbyA`), because its own neighbours include hole cells, whose values are the
rigid velocity, not the field; a hole cell's gradient is zero (no calculated row uses it). The face gradient is `G_f = w
G_o + (1 - w) G_n` (as `green_gauss`'s skewness pass).

**C1, the face flux of a cell vector field `v` (the Rhie-Chow face flux).** For a linear field `v(x) = v(x_f') + G (x -
x_f')`, the exact flux through the fan of flat triangles is `sum_t v(c_t) . S_t` (midpoint rule, exact for a linear
integrand on a flat triangle), which is

`(v_f') . S_f + sum_ij G_ij sum_t (c_t - x_f')_j (S_t)_i = (v_f') . S_f + sum_ij G_ij (s_f,j S_f,i + W_f[j][i])`.

So the corrected flux is `phi_f = (w v_o + (1 - w) v_n) . S_f + sum_ij G_f,ij (s_f,j S_f,i + W_f[j][i])`. On a planar face
it is the classical skewness correction `G_f s_f . S_f`. Summed over a closed cell it is `sum_t v(c_t) . S_t = trace(G) V_t`
for a linear `v` (Gauss on the polyhedron of flat triangles, `V_t` its volume), so for a divergence-free linear field the
discrete divergence is zero to round-off on any of these meshes. It is used for: `phiHbyA` (the flux of `HbyA` with the
gradient of `HbyA`, boundary values `boundary_hby_a`); the flux of the faces that do not keep their flux (M9) and the
initial flux (when the opt-in is set before the first step); and the interpolation of the old velocity in the
`ddt_correction` defect (the defect must use the interpolation that `phiHbyA` uses, else the defect of an exact state is
the skewness error itself). Boundary faces keep their flux (`ub . S_f`): the boundary faces of B3 and B3d are planar.

**C2, the convected face value.** The convection of component `k` keeps the library's operator (upwind matrix plus the
deferred central correction) and adds the deferred correction `q_f` on every internal face, `-q_f` to the owner's
right-hand side and `+q_f` to the neighbour's:

`q_f = beta [ F_f (g_f . s_f) + g_f . (W_f (v_f' - m_f)) ]`,

`F_f` the convecting flux (relative to the mesh), `g_f` the face gradient of the convected component (G of the cells the
operator convects), `v_f' = w u_o + (1 - w) u_n` the interpolated cell velocity, `m_f = (x_f(new) - x_f(old)) / dt` the
velocity of the face centre over the step (zero at rest), `beta = 1` for Central, `beta` for `Blended(beta)`, `0` for
Upwind. Derivation: the exact convective flux through the fan, `sum_t ((u - m)(c_t) . S_t) phi(c_t)`, with `phi(x) =
phi_f' + g . (x - x_f')` and `u - m` linear, is `F_f phi_f' + g . (T_f (v_f' - m_f)) + (second order)` with `T_f = s_f
S_f^T + W_f`; the first-order part `g . (s_f S_f^T (v' - m)) = (g . s_f) ((v' - m) . S_f)` is replaced by `(g . s_f) F_f`,
which differs from it by a product of two first-order terms (the classical form on a planar face). What is left is of
second order in the cell size (the products `(A r . S)(g . r)` per triangle, which cancel between opposite faces on a
Cartesian cell and only partly on a warped one).

**C3, the momentum diffusion.** `laplacian(.., Some(G_k))` for component `k` (the library's explicit non-orthogonal
correction `gamma k_f . G_f` on internal faces and `gamma k_f . G_c` on Dirichlet boundary faces). With the exact gradient
of a linear field the face flux is `gamma (lap_coeff (phi_n - phi_o) + k_f . G) = gamma (abs(S)^2/(S . d) G . d + k_f . G)
= gamma G . S_f` (and the same on a Dirichlet face, `FixedValue` using the normal distance), which sums to `gamma G sum_f
S_f = 0` over a closed cell: exact on any mesh.

**C4, the pressure Laplacian.** The explicit non-orthogonal flux `N_f = gamma_p,f k_f . g_f`, `g` the gradient G of the
pressure each corrector starts from (`boundary_values_pbc` as boundary values), on the internal faces, subtracted from
`phiHbyA` before the pressure solve: `phiHbyA_f -= N_f`. This is algebraically the same as assembling the Laplacian with
the gradient (the right-hand side gains `div N`) and correcting the flux with the same gradient (`corrected_flux_with`), on
internal faces; the fixed-pressure boundary faces get no non-orthogonal part (the library's `BoundaryGradient::Dropped`).
Because the same `N` enters the equation and the flux, the corrected flux satisfies the discrete continuity of the solved
system exactly (to the solver's tolerance), as without the opt-in, and the mass offset `delta` of M7 and every overset row
need no change (they see a modified `phiHbyA`).

What stays as it is: the Green-Gauss pressure gradient of the momentum predictor and of the velocity correction (for the
flows here the exact pressure is constant); the time scheme; the fringe and hole rows; the M8 fluxes of faces without a
calculated cell (they are never used before being refreshed).

**Refusals.** A step with the opt-in on is refused together with BDF2 on, with a prescribed fringe flux (ADR 0347), and
with `rhie_chow_step_consistent` (their face interpolations would have to be corrected too; not covered here).

**On B3.** Every part of B3 is a Cartesian block (the component turned and shifted): `s_f = 0`, `k_f = 0`, `W_f = 0` to
round-off, so C1 to C4 add terms of size round-off times the gradient; the opt-in on B3 should equal the default to about
`1e-13` relative (printed as P-diff, no limit; the limits use both readings of reading 3).

## Arithmetic before this registration (disclosed, by hand)

- The warp tensor. A face corner of B3d is displaced by up to `0.015` per coordinate (uniform, standard deviation `0.0087`).
  For a saddle-shaped face `z = kappa x y` with corner heights `+-delta`, `kappa = 4 delta / h^2`; the fan's triangle
  centroids lie at about `+-h/3` along the axes, their area vectors tilt by `kappa r`, giving entries of `W_f` of about `2
  delta h^2 / 9 = 2.2e-5` for `delta = 0.01`, `h = 0.1`, against `V = 1e-3`: `W_f / V` about `0.02` per face, `0.05` per
  cell (six faces, random signs). Without the `W` term the continuity residual of the exact field (`abs(A) = 0.5`) would be
  about `0.5 * 0.05 = 0.025` per second, a third of `T_cont = 0.075` that ADR 0264 measured; the classical planar skewness
  correction alone would therefore not make the flux exact on B3d, and this is why C1 and C2 carry `W_f`.
- Second-order remainder of C2 per unit volume: per triangle `abs(A) r abs(S_t) abs(g) r` with `r = h/3`, `abs(S_t) = h^2 /
  4`, `abs(g) <= 0.5`: `0.5 * 0.033 * 0.0025 * 0.5 * 0.033 = 7e-7`; 24 triangles, an imbalance of about `0.15` (the relative
  distortion) with random signs: `0.15 * sqrt(24) * 7e-7 = 5e-7`, over `V`: `5e-4`. The cross term of C2 (`(F - (v' - m) .
  S)(g . s)`, the flux correction `G : T` of about `0.5 * 0.006 * 0.01 = 3e-5` times `g . s = 0.5 * 0.006`): `1e-7` per face,
  `2.5e-4` per volume. So the corrected `T_conv` is expected near `5e-4` at most, against `0.14` without (ADR 0264).
- Velocity error of the warped background alone at rest with the opt-in: ADR 0264's SB had `9.6e-3` for `T_conv = 0.14`,
  `T_diff = 0.025`, `T_cont = 0.075`; scaling with the convective residual alone, `9.6e-3 * 5e-4 / 0.14 = 3.4e-5`; with
  `T_cont = 0` the pressure is that of the momentum residual only, expected below `1e-4 abs(U0)^2`.
- Without C3 (mutation MU4): the diffusion residual `0.025` alone, `9.6e-3 * 0.025 / 0.165 = 1.5e-3` (ADR 0264's split of the
  residuals). Without C1 in `phiHbyA` (MU6): `T_cont = 0.075` returns and with it the pressure of ADR 0264's SB, `3e-2
  abs(U0)^2`.
- B3 and B3d with the opt-in: ADR 0264's P3 had `8.72e-4`, of which `2.1e-4` is the time error of the turning component
  (PC1) and the rest the coupling on planar meshes (not diagnosed, ADR 0264 Q2). With the geometric residual removed, W3
  with the opt-in should carry the same two parts plus the warped remainder of S1 (`2.1e-4 + 3e-5`): expected `E(W3o)`
  between `8e-4` and `1.4e-3`, ratio `E(W3o) / E(P3o)` between `0.9` and `1.6`. The risk to this: the coupling's excess on
  B3 is not understood (ADR 0264 Q2), and if it grows with the warp of the fringe region the ratio can exceed `2`.
- Uniform flow: every gradient is zero to round-off (`1e-15 / h = 1e-14`), the corrections are of size `1e-14 * h ~ 1e-15`:
  A4 is expected near the default's `8e-15` (velocity) and `1e-13` (pressure).
- Pressure operator K5: for `p = 2 + c . x` with `abs(c) = 0.86`, the uncorrected face flux differs from `c . S_f` by `c .
  k_f`, about `0.86 * 0.05 abs(S)`, i.e. `4e-2` relative; corrected: round-off, `1e-15` relative.
- Cost: least-squares gradients of 3 components per pass, of 3 components and the pressure per corrector, about 16 000
  cells: milliseconds each. The opt-in runs take about the time of ADR 0264's (7 to 14 s each).

## Set-ups

PS, F, the boundary values, B3, B3d, the motion `P(t)`, 40 steps of `dt = 0.005`, exactly as ADR 0264 (shear) and ADR 0255
(uniform). The opt-in is set right after `OversetSolver::new`, before the first step.

- **P3o**, **P3qo**, **W3o**, **W3qo**: ADR 0264's P3, P3q, W3, W3q with the opt-in.
- **P3**, **P3q**: the default runs of ADR 0264 (re-run; their numbers there are `8.7203e-4`, `8.7204e-4`).
- **U3o**, **U3qo**: ADR 0255's W3 and W3q (uniform flow) with the opt-in.
- **N1o**: P3o with every stencil weight multiplied by `1.001` after `prepare`.
- **SBo**, **S0o**, **S1o**, **PC1o**: ADR 0264's single-mesh runs SB, S0, S1, PC1 with the opt-in.
- **T** (kinematic, no step): on the four meshes of ADR 0264 K2 (planar and warped background at rest, planar and warped
  component at `P(0)`), `u_ex` in the cells and on the boundary faces: the corrected flux `phi` (C1 with the gradients G of
  `u_ex`); `T_cont` its divergence per volume; `T_conv` the residual of `convection(phi, Central)` plus C2, per volume, the
  largest over the components; `T_diff` the residual of `laplacian(nu, Some(G_k))` per volume (C3).

## Registered checks

New test file `crates/rcfd-fv/tests/overset_f368.rs`, release. No existing test is edited.

### A. The acceptance

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| A1 | default interpolation: `E(W3o) / E(P3o)` | `<= 2` | `0.9` to `1.6` |
| A1d | default interpolation: `E(W3o) / E(P3)` (B3 with the default solver) | `<= 2` | the same |
| A2 | quadratic interpolation: `E(W3qo) / E(P3qo)` | `<= 2` | the same |
| A2d | quadratic interpolation: `E(W3qo) / E(P3q)` | `<= 2` | the same |
| A3 | continuity of the calculated cells, every step of P3o, P3qo, W3o, W3qo | `<= 1e-9` each | ADR 0264 `2e-14`; C4 keeps the equation and the flux paired |
| A4 | U3o and U3qo, every step, the non-hole cells: velocity `max abs(u - U) / abs(U)`, pressure `max abs(p) / abs(U)^2`; continuity | `<= 1e-10`, `<= 1e-10`; `<= 1e-9` | `1e-14`, `1e-13`; `1e-14` |

### D. The defaults

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| D1 | fingerprints (the bits of `u`, `p`, `phi` folded by FNV-1a, and the step's error message if a step fails) of 5 steps of each of: the shear F on B3d default linear; the same quadratic; the same with `set_outer_iterations(2, 0.8, 0.5)`; with BDF2 (`set_time_scheme(Bdf2)`); with `FringeFlux::PrescribedOwnGradient`; with `rhie_chow_step_consistent`; the warped component alone moving (trivial path, S1); ADR 0255's uniform flow on B3d (the fixed-pressure path): run on the tree before any library change (recorded) and on the final tree | equal, each of the eight | the opt-in is behind one flag |
| D2 | `cargo test --release -p rcfd-api --test baseline_hashes`, unedited | passes | |
| D3 | the existing overset test files of `rcfd-fv` (`overset_f134`, `_f134_records`, `_f314`, `_f334`, `_f335`, `_f335_records`, `_f336`, `_f338`, `_f349`, `_f353`, `_f357`, `_f359`, `_f445`, `overset_motion_f442`) and `cargo test --release -p rcfd-fv --lib`, unedited, their ignored tests left ignored | pass | |

### K. Exactness of the derivation (operator level, on the public helper the solver uses)

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| K1 | T on the warped background and the warped component: `T_cont` (largest, per second) | `<= 1e-10` | round-off `1e-14` (default `0.075`) |
| K2 | the same: `T_diff` (largest, per unit volume) | `<= 1e-10` | round-off (default `0.025`) |
| K3 | the same: `T_conv` (largest, per unit volume) | `<= 5e-3` | about `5e-4` (default `0.14`) |
| K4 | T on the planar background and the planar component: `T_conv`, `T_diff`, `T_cont` | `<= 1e-10` each | round-off, as ADR 0264 K2 |
| K5 | the warped background and component: `p = 2 + c . x`, `c = (0.3, -0.7, 0.4)`, gamma 1: `max_f abs(lap_coeff (p_n - p_o) + N_f - c . S_f) / (abs(c) abs(S_f))` over the internal faces, `N_f` of C4 with G of `p` (exact boundary values) | `<= 1e-10` | round-off (without `N_f`: about `4e-2`, printed) |
| K6 | the warped faces are seen: the largest `abs(W_f) / abs(S_f)^{3/2}` on B3d's background and on B3's | `>= 1e-3` on B3d, `<= 1e-12` on B3 | `0.02` and `1e-16` |
| K7 | SBo (warped background alone at rest, opt-in): `E` | `<= 5e-4` | `3.4e-5` (default SB `9.6e-3`) |
| K7p | SBo: the largest `abs(p_c) / abs(U0)^2` | `<= 1e-3` | below `1e-4` (default `3.1e-2`) |

### U. Set-up checks of the runs

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| U4 | kind changes over the steps, P3o and W3o | `>= 20` each | ADR 0264: `99`, `92` |
| Q1 | P3qo and W3qo, every step: share of the fringe cells with a quadratic stencil | `>= 0.95` | `1.0` |
| K1s | every step of the four opt-in runs: the stencils reproduce `u_ex` (ADR 0264 K1) | `<= 1e-12` | `3e-15` |
| U5 | unconverged linear solves of every run | printed | `0` |

### S. Separating runs (used only if one of A1, A1d, A2, A2d misses; measured either way)

`E_Mo = max(E(SBo), E(S1o))`, printed with `E(S0o)`, `E(PC1o)` and the per-class errors. Reading, fixed now, for each
missed ratio: **the single warped mesh** is still the cause when `E_Mo >= 0.5 E(W3o)` (respectively `E(W3qo)`) and K1 to
K4 are met (then a residual of the corrected operators that T does not show, e.g. the Green-Gauss pressure gradient or the
ALE terms, is named by the separate runs at rest and moving); otherwise **the overset coupling on the warped mesh**
(`E(W3o) > 2 E_Mo`), located by the class and place of the largest error. Every case falls in one branch.

### N. Negative control

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| N1o | N1o (P3o with the weights times `1.001`): `E` | `>= 3e-4` | ADR 0264 N1 `1.85e-3` |

### Mutations (scratch copy, not committed)

Each alone, applied by a script with exact replacements that must occur once to a `git archive` copy of the final tree in
`E:/projects/cfd/w49i-work/f368-scratch` (own target directory); the new test file is run once (and, for MU7, D1); the named
check must fail. The copy is compared byte for byte with the worktree afterwards.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | C1 (helper): the warp term `W_f` left out of the face-flux correction (planar skewness only) | K1 (estimate `T_cont` about `0.02` to `0.05`) |
| MU2 | C4 (helper): the sign of `N_f` flipped | K5 |
| MU3 | C2 (helper): the sign of `q_f` flipped | K3 (about `0.28`) |
| MU4 | solver: the momentum diffusion assembled without the gradient (C3 not applied) | K7 (about `1.5e-3`) |
| MU5 | solver: the fringe cells keep their own least-squares gradient (no donor interpolation) | A1 (the background fringe next to the hole sees the rigid velocity of the hole cells: a gradient error of about `1 / h = 10`) |
| MU6 | solver: `phiHbyA` without the C1 correction | K7p (about `3e-2`) |
| MU7 | the opt-in switched on in `OversetSolver::new` | D1 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### Classes

A miss of A1, A1d, A2 or A2d is kept as an ignored `*_known_miss` test with the limit intact and diagnosed by S. A miss of
A3, A4 or D1 to D3 is a defect of the opt-in or of its isolation (reported; the defaults are never changed to meet a check).
A miss of K1 to K7p is a finding against the derivation or the code (reported, not tuned). A miss of U4, Q1 or K1s, or a
control that does not hold, is a defective set-up, corrected by an amendment before any corrected run.

## Predictions (mine, before any run)

K1, K2, K4, K5 near `1e-14`; K3 near `5e-4`; K6 `0.02` and `1e-16`; K7 near `3e-5`, K7p near `1e-5`; A3 near `2e-14`; A4
near `1e-14` and `1e-13`. `E(P3o)` equal to `E(P3) = 8.72e-4` to four digits; `E(W3o)` and `E(W3qo)` near `1e-3`, the
ratios near `1.2` (A1, A1d, A2, A2d met). S1o near `2.3e-4`, PC1o near `2.1e-4`. N1o near `1.8e-3`. D1 to D3 equal and
passing. The mutations fail their checks as estimated above; MU5 is the least certain in size, not in sign.

## Decision rule

- **done**: A1, A1d, A2, A2d, A3, A4, D1, D2, D3 met; K1 to K7p, U4, Q1, K1s met; N1o held; MU1 to MU7 caught by their
  registered checks, without a post-run amendment.
- **partial** (Rule I): any registered check missed (with A3, A4 and D1 to D3 met and every ratio miss diagnosed by S), or a
  registered mutation not caught by its registered check, or caught only after a post-run amendment.
- **todo**: A3, A4 or one of D1 to D3 missed, or the runs could not be made.

## Limits of this registration

- One flow with a non-zero gradient (linear, constant pressure), one warped mesh family, one motion, 40 steps, one
  resolution. The pressure corrections (C4) are checked at operator level (K5) and through K7p; no flow with a varying exact
  pressure is run, and the Green-Gauss pressure gradient of the momentum predictor is not corrected.
- Boundary faces keep their uncorrected flux; the corrections assume planar boundary faces (true for B3 and B3d).
- The opt-in is refused with BDF2, the prescribed fringe flux and `rhie_chow_step_consistent`; the M8 fluxes of faces
  without a calculated cell keep the plain interpolation.
- The second-order remainder of C2 is not removed (K3 has a non-round-off limit on purpose).
- No change of a default, golden or hash table is needed or made.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
