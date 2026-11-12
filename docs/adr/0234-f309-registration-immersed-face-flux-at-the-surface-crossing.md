# ADR 0234: F-309 registration: second-order immersed forcing, the fluid-solid face flux at the surface crossing

Status: **REGISTERED 2026-11-12**, written before any code of F-309 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; nothing above the addendum is changed after a run.

Parents: [ADR 0218](0218-f219-registration-immersed-solid-with-modified-forcing.md) (F-219: the immersed solid, its V1 known miss
and its question Q6, from which F-309 was made). Roadmap item F-309 (depends on F-219).

Number: the lead gave 0234. Branch `f309-immersed-second-order` from main bf3376b1, worktree `E:/projects/cfd/w49i-work/f309`.

House rules (ADR 0218's, ADR 0230's): limits are fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded, and is diagnosed, not tuned away; a defective set-up is
corrected by an amendment written and committed before the corrected run, and the old test stays as an ignored record; each
registered run is made once; every new code path is opt-in, and a solver without it, or with the forcing of ADR 0218, produces the
bytes it produced before; no case-schema key, no hash table, no golden and no existing test file is edited; no new third-party
crate; no unsafe code; no commit trailers.

Clean-room: the treatment is the ghost-cell (ghost-fluid) view of a Dirichlet boundary on a Cartesian mesh: the equations of a
fluid cell next to the surface are closed with the smooth extension of the fluid solution through the surface, built from the
no-slip value at the crossing (Fedkiw, Aslam, Merriman and Osher, J. Comput. Phys. 152 (1999) 457-492; Gibou, Fedkiw, Cheng and
Kang, J. Comput. Phys. 176 (2002) 205-227; Tseng and Ferziger, J. Comput. Phys. 192 (2003) 593-623; Mittal, Dong, Bozkurttas,
Najjar, Vargas and von Loebbecke, J. Comput. Phys. 227 (2008) 4825-4852), together with a mass source in the cells of the body so
that the virtual flow inside the body does not constrain the fluid (Kim, Kim and Choi, J. Comput. Phys. 171 (2001) 132-150). The
face-flux form of the pressure-velocity coupling is that of Rhie and Chow (AIAA J. 21 (1983) 1525-1532). These are cited from
memory of the published papers; no text of them was read for this work and no formula is copied: the algebra below is derived
here on the discrete equations of `crates/rcfd-fv/src/simple.rs`, `pimple.rs` and `pressure.rs`. No vendor code, text, manual or
binary was used.

## What F-309 asks

The roadmap entry: "Second-order immersed forcing: fluid-solid face flux at the surface crossing"; acceptance "Circular Couette
flow with an immersed rotating cylinder reaches an observed order of 1.5 (the F-219 V1 check, limit unchanged)."

Reading (a reading, not a decision of the maintainer): the F-219 V1 check is the set-up CC of ADR 0218 (below), the error `e` of
its definition, the observed order `log2(e(48)/e(96)) >= 1.5` and `e(96) <= 1e-2`; nothing of that set-up is changed except that
the forcing is built with the new opt-in treatment.

## What was read and run before this text (disclosed)

Read: ADR 0218 in full (registration, addendum 1, Q1 to Q7); `crates/rcfd-fv/src/immersed.rs`; the circular Couette part, the
known miss and the diagnostic of `crates/rcfd-fv/tests/immersed_solid_f219.rs`; the momentum and pressure parts of
`SimpleSolver::iterate` and of the PIMPLE correctors; `pressure.rs` (`divergence`, `solve_pressure_parts_src`, the volume source
of ADR 0052); `convection.rs::face_flux`.

Run: no code of F-309 and no run of a check below. One probe of the existing code (a scratch test, deleted, not committed; release
build of main bf3376b1): the state hash (FNV-1a over the bits of `u` then `p`, the function `state_hash` of
`immersed_solid_f219.rs`) of the CC set-up of ADR 0218 at `N = 24` with the modified forcing of ADR 0218,

- SIMPLEC, the settings of ADR 0218 V1, after 40 iterations: `3b49c4ed4863d1b6`;
- PISO, the settings of ADR 0218 V3, after 20 steps of `dt = 0.0025`: `9fff50792adb6d4a`.

These two numbers are the reference of check N1 and nothing else.

Arithmetic done beforehand (by hand, no program): the estimates given with the limits below, and the analysis that follows.

## Why the forcing of ADR 0218 is first order (derived; ADR 0218 gave it as an unchecked hypothesis)

Let `c` be a fluid cell, `s` a solid neighbour across the internal face `f`, `theta` the crossing fraction on `x_c -> x_s`, `x_G`
the crossing, `u_G = u_b(x_G)`. ADR 0218 D2 replaces the viscous flux of `f` in row `c` by `(k_f/theta)(u_G - u_c)`: the
Laplacian of `c` then uses the ghost value `u_c + (u_G - u_c)/theta` in place of `u_s`, the linear extension of the fluid profile
through the surface. That part is the method of Gibou et al. and is second order.

The mass flux of `f` is not treated. The pressure equation of the solid cell `s` enforces its own continuity. The fluxes of the
faces between solid cells are those of the rigid velocity (the face interpolation of a rigid field is exact and its discrete
divergence over a cell is zero) with a face coefficient `rAU_f` of order `1/K`, so they do not move; the pressure `p_s`
therefore takes the value that makes the fluxes of the fluid-solid faces of `s` sum to the rigid value. A solid cell with one
fluid-solid face has that face's flux pinned to `u_b(x_f) . S_f`, whatever the fluid does. The continuity of the fluid cell `c`
thus contains, at `f`, the rigid velocity at the face centre, while the momentum equation of `c` was closed with the smooth
extension. At the face centre the two differ by `(theta - t_f) h [du/dn]` to first order (`t_f` the fraction of the face on the
segment, `[du/dn]` the jump of the normal derivative between the fluid profile and the rigid field): a face velocity error of
order `h`, a divergence error of order 1 in the layer of first fluid cells, and a velocity error of order `h` in the fluid, with
a wall-normal part of the size of the tangential part. That is what the diagnostic of ADR 0218 addendum 1 measured (radial and
azimuthal errors of equal size, spread over the annulus). The pressure shows the same thing: the flux mismatch `h [du/dn] |S|` is
removed by `rAU_f lap_f (p_s - p_c)` with `rAU_f` of order `h^2/nu`, so `p_s - p_c` is of order `nu [du/dn]`, a jump of order 1
over one cell, which enters the Green-Gauss pressure gradient of `c`.

## Design decisions

**D1. The face flux of a fluid-solid face (derived).** For every internal, non-empty face `f` between a fluid cell `c` and a solid
cell `s` of body `b` (the faces that carry a modification term of ADR 0218 D2), with `theta_e = max(theta, theta_min)` (the same
bounded fraction as the viscous term), `t_f` the fraction of the face on the segment `x_c -> x_s` (the face interpolation weight
of `s`; 1/2 on a uniform mesh), and `HbyA_c` the cell value the solver interpolates to the faces (`H/a_P`, without the pressure
gradient):

    U_f = HbyA_c + (t_f/theta_e) (u_G - HbyA_c),        phiHbyA_f = U_f . S_f   (replacing the interpolated value),

the linear extension of the fluid profile from `x_c` through the no-slip value at the crossing, evaluated at the face. The
pressure part of the face flux (`rAU_f lap_f (p_s - p_c)` and, in SIMPLEC and with the consistent correctors, the split of it) is
left as it is, as are the body-force and jump terms added after the interpolation. In PIMPLE the replacement is made after the
`ddt` flux correction, so that the correction (the old flux minus the interpolated old velocity, which at these faces is the
difference `D1` introduces) is not added to these faces.

Why the same `theta_e`: with the bounded fraction the viscous term makes the discrete `u_c` satisfy `(u_G - u_c)/theta_e` equal
to the wall gradient, so `U_f` is in error by `(theta_e - theta)` times that gradient, at most `theta_min` of a cell. With the
unbounded `theta` in the flux only, the error would be half the wall gradient at each bounded face.

**D2. The mass source of the solid cells (derived; Kim, Kim and Choi's source, here in the cells whose centre is in the body).**
Let `R_f = u_b(x_f) . S_f` be the rigid flux of the face and `sg_f = +1` when the fluid cell is the owner of `f` (`S_f` points
into the solid), else `-1`. The solid cell `s` gets the volume source (net outflow, m^3/s)

    q_s = - sum over its fluid-solid faces of sg_f (phiHbyA_f - R_f),

passed to the pressure equation as the cell volume source of `solve_pressure_parts_src` (ADR 0052's mechanism, unchanged). With
it the pressure-free part of the continuity of `s` is that of the rigid field, which is zero, so `p_s` is the
`rAU_f lap_f`-weighted mean of the pressures of its fluid neighbours (to order `1/K`): `p_s - p_c` is of order `h grad p`, the
pressure part of the flux of `f` is a face velocity of order `h^2 grad p / nu`, and the pressure gradient of `c` has an error of
order 1 in the first fluid cells only.

**D3. The net flux of a body (derived).** Before D2, per body `b`: `I_b = sum_f sg_f (phiHbyA_f - R_f)` over its fluid-solid
faces, `A_b = sum_f |S_f|`, and `phiHbyA_f -= sg_f |S_f| I_b / A_b`. Then `sum q_s = 0` per body, so the pressure equation of a
closed domain stays compatible and the body neither creates nor removes fluid. For the exact extension `I_b` is the midpoint-rule
flux of a divergence-free field through a closed staircase, of second order, so the correction is a face velocity of order `h^2`.

**D4. Order (the argument, not a proof).** With D1 to D3 the fluid cell `c` has: the viscous flux, the mass flux and the
continuity of the smooth extension, each with a face value in error by order `h^2`; a pressure gradient and a convected face value
(the solver's interpolation of `u_c` and `u_s`) in error by order 1 and order `h`, in the first fluid cells only. A truncation
error of order 1 confined to the cells next to a Dirichlet surface gives a solution error of order `h^2` (the argument of Gibou et
al. for the Laplacian). Remaining first-order parts, with small factors: the faces with `theta < theta_min` (1 percent of the
faces for a uniformly distributed `theta`; a wall displaced by at most `theta_min h`), and the terms of order `1/K`.

**D5. Opt-in (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; Q1).** A new constructor
`ImmersedForcing::build_second_order(geom, bodies, settings, gamma)`: the forcing of `build` (the same coefficients and sources,
bit for bit) plus the list of flux faces (`flux_faces`, per face: the face, the fluid and solid cells, the body, `sg_f`,
`t_f/theta_e`, `u_G`, `R_f`, `|S_f|`). It needs `settings.modified` (else `Err` naming `modified`: D1 without the viscous
modification is inconsistent). `ImmersedForcing::build` leaves the list empty; `ImmersedSettings` is not changed. New method
`ImmersedForcing::reconstruct_face_flux(&self, hby_a, phi_hby_a, plain) -> Option<Vec<f64>>`: with an empty list it returns
`None` without touching anything; otherwise it applies D1 and D3 to `phi_hby_a` and returns the cell sources of D2.

**D6. The solvers.** `SimpleSolver::iterate`: one statement after the interpolated `phiHbyA` is formed (before the body-force,
SIMPLEC and jump terms), and the plain pressure solve takes the source (`solve_pressure_parts_src` with `None` is the function
`solve_pressure_parts` calls, so the default is the same code). `PimpleSolver`: one statement after the `ddt` flux correction and
the plain pressure solve of the correctors takes the source. Nothing else changes.

**D7. Refusals.** The source is passed on the plain pressure path only. With the flux faces set and the gradient options of the
pressure equation (ADR 0204 K3), the body-force-weighted pressure (ADR 0193) or the implicit non-orthogonal pressure (ADR 0198;
SIMPLE) the run panics with a message naming ADR 0234 (`plain = false`). A panic, not the divergence guard: taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), because it keeps the change of `pimple.rs` to two
statements while that file is being reworked on other branches; Q3. The refusals of ADR 0218 D4 (fractional step, moving mesh,
`replace_mesh`) are unchanged and cover the new forcing.

## Set-ups

**CC** (ADR 0218, unchanged): `box_2d(N, N, 2.4, 2.4)`, centre `(1.2, 1.2)`; inner `Cylinder` radius 0.5 rotating with
`omega = (0, 0, 1)`; outer `Cylinder` radius 1.0, `inverted`, at rest; box walls velocity zero; `nu = 1`, `rho = 1`, `K = 1e8`,
`theta_min = 1e-2`; SIMPLEC (`SimpleSettings::new(1.0, Simplec)`), central convection, momentum and pressure `tol_abs 1e-20`,
from rest, until `max_du <= 1e-10` or 20 000 iterations; `e = sqrt(sum_F V |u - u_exact|^2 / sum_F V)/(omega R1)` over the fluid
cells. "Second" is `build_second_order`, "first" is `build` with `modified = true` (the forcing of ADR 0218).

**The flux measure** `eps`: over the flux faces, `eps = sqrt(sum_f (phi_f - u_ext(x_f) . S_f)^2 / sum_f |S_f|^2)/(omega R1)` with
the solver's converged `phi` and `u_ext` the exact Couette profile `A r + B/r` evaluated at the face centre on either side of the
surface (the smooth extension). For "first" the faces are those of the modification terms (the same faces).

**DFG 2D-1 on M1 and M2** (ADR 0218, unchanged: the mesh family, `U_m = 0.3`, `K = 1e7`, SIMPLEC with `LinearUpwind`, the
convergence rule and the caps 4000, M2 started from the injected M1 state), with `build_second_order`.

## Registered checks

All in the new file `crates/rcfd-fv/tests/immersed_second_order_f309.rs`, release (the heavy ones carry
`#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]`).

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| A1 (acceptance) | CC, second: `e(96)` and `log2(e(48)/e(96))` | `e(96) <= 1e-2`; order `>= 1.5` | the limits of ADR 0218 V1; by D4 the order tends to 2; ADR 0218 measured `e` 8.29e-3, 3.20e-3, 1.58e-3 and 1.02 |
| S1 (set-up) | every CC run (second and first, N 24, 48, 96) | `max_du <= 1e-10` within 20 000 iterations | ADR 0218: 105 to 353 iterations |
| C1 (negative control of A1) | CC, first: the same order | `< 1.5` | ADR 0218: 1.023 |
| V1 | CC: `e_second(96)` against `e_first(96)` | `<= 0.75` of it | with `e_second(48) <= e_first(48)` and order 1.5, `e_second(96) <= 0.72 e_first(96)` |
| V2 (the mechanism) | CC at N 96: `eps_first / eps_second` | `>= 3` | first: the pinned rigid flux differs from the extension by about `0.4 h [du/dn]` at the inner body, `eps` about 1e-2; second: order `h^2`, about 1e-3 |
| V3 | CC at N 48, second, PISO (`piso(1)`, Euler, central, `dt = 0.0025`, 400 steps from rest): set-up `max abs(u^{n+1} - u^n) <= 1e-9` at the last step; `e` and `eps` against SIMPLEC second at N 48 | `e <= 1.2 e_SIMPLEC`; `eps <= 1.5 eps_SIMPLEC` | the same steady equations up to the `rAU`-dependent terms of order `h^2`; ADR 0218 V3 measured 3.195e-3 against 3.202e-3 |
| V4 | DFG 2D-1 on M2, second: the convergence rule of ADR 0218 met on M1 and M2 (set-up); `C_D`, `C_L` | within 5 percent of 5.57953523384 and 0.010618948146 | ADR 0218 on M2: -0.17 percent and +2.73 percent |
| U1 | `reconstruct_face_flux` on the exact Couette field at the cell centres (rigid in the solid cells), CC geometry with `theta_min = 1e-9`: `abs(phiHbyA_f/abs(S_f) - u_ext(x_f) . n_f)` over the flux faces | maximum at N 96 `<= 2e-3`; rms(48)/rms(96) `>= 3` | linear extension: at most `0.125 h^2 abs(u'')`, about 7e-4 with `abs(u'') = 9`; nominal ratio 4 |
| U2 | CC at N 24, a pseudo-random `hby_a` (LCG, values in `[-1, 1]`): per body `abs(sum sg_f (phiHbyA_f - R_f))`; `abs(sum_c q_c)`; `q` in the fluid cells; `phiHbyA` at every other face | `<= 1e-12 A_b`; `<= 1e-12 sum A_b`; exactly 0; bit-identical | D3, round-off of a sum of about 300 terms of order 1e-2 |
| U3 | `build` (ADR 0218): `flux_faces`; `reconstruct_face_flux` | empty; `None` and `phiHbyA` bit-identical; `coeff`, `source`, `bodies` of `build_second_order` equal to those of `build` | D5 |
| N1 (defaults) | the two probes above, re-run on the final tree | hashes `3b49c4ed4863d1b6` and `9fff50792adb6d4a` | the default path is the same code |
| B1 | `build_second_order` with `modified = false` | `Err` naming `modified` | D5 |
| R1 | SIMPLE and PIMPLE with the flux faces set and `gradient_pressure = Some(default)`: one iteration, one step | panic naming `ADR 0234` | D7 |

Printed, no limit: `e` and `eps` on every mesh and their orders; CC second on a fourth mesh, N 192, with the order 96 to 192; the
radial and azimuthal parts of `e`; the inner torque against 4.18879; iteration counts; the 2D-1 values on M1; the largest
`abs(p)` over the solid cells next to the fluid against the largest over the fluid cells.

**Not a check of F-309:** the lead's gate (the existing suites unedited, `baseline_hashes` in particular). The existing tests of
`immersed_solid_f219.rs` are run once on the final tree and reported.

### Mutations (scratch, not committed)

Each applied alone to the final tree by an exact text replacement (checked to occur once), the named tests run once in release,
the file restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `reconstruct_face_flux` does not replace `phiHbyA_f` (the source still returned) | A1 or V2 |
| MU2 | the source of D2 returned as zeros | A1 or V2 |
| MU3 | `t_f/theta_e` replaced by `t_f` (the interpolation between `HbyA_c` and `u_G` as if the crossing were at `x_s`) | A1 or V2, and U1 |
| MU4 | the correction of D3 not applied | U2 |
| MU5 | SIMPLE: the source not passed to the pressure solve | A1 or V2 |
| MU6 | PIMPLE: the statement of D6 removed | V3 |
| MU7 | PIMPLE: the source not passed to the pressure solve | V3 |
| MU8 | `build_second_order` does not refuse `modified = false` | B1 |

Expected: MU1 leaves the interpolated flux and gives the solid cells a source computed from nothing changed (the flux of the
forcing of ADR 0218 plus an inconsistent source); MU2, MU5 and MU7 let the continuity of the solid cells pin the flux again
(the forcing of ADR 0218's numbers, about); MU3 puts the wall at the solid centre for the flux; MU4 leaves a net flux; MU6 is the
forcing of ADR 0218 in PISO.

## Decision rule

- **done**: A1 met with S1 and C1 held; V1 to V4, U1 to U3, N1, B1, R1 met; every mutation failed a test it must fail.
- **partial**: N1 and U1 to U3 met and the treatment runs (S1 held), but A1 missed (kept as an ignored known miss with a
  diagnosis), or A1 met and another of V1 to V4, B1, R1 missed, or a mutation was not caught and its amendment is not done.
- **todo** (unchanged): N1 missed, or the treatment does not converge on CC; the library part is then not proposed for a merge.

A set-up check or control that does not hold (S1, C1, the set-up parts of V3 and V4) is a defective set-up: an amendment before
any corrected run, never counted as a pass.

## Limits (not covered)

No case-schema key and no Studio control (the schema goldens would change; ADR 0218 Q2 stands; Q2 here). The treatment is not the
default for immersed solids (Q1). Plain pressure path only (D7). The corrector-count estimate of ADR 0156 (`pick_correctors`)
solves its trial pressure without D1 to D3. The solver's continuity measure includes the sources of the solid cells and does not
go to zero (Q4). Orthogonal meshes only are checked; the extension is along the cell-centre line. Momentum only; no moving
bodies; not in the block-coupled solver, the fractional step or on moving meshes (ADR 0218). The force of a body is still the
reaction of its forcing (the momentum carried by the flux of D1 into the solid cells is in that reaction). The sphere and the
cuboid are not run. Turbulence wall treatment at the surface is not addressed.

## Open questions

- **Q1. Default.** Options: (a) opt-in constructor, the forcing of ADR 0218 stays what `build` gives; (b) make D1 to D3 part of
  `modified`. (b) changes what a present caller of `build` computes, so it is not taken. Recommendation (a) now; (b) is the
  maintainer's to decide with the results.
- **Q2. Case key.** As ADR 0218 Q2; if a key `immersed_solids` is added, a field for this treatment belongs in it. Needs the
  schema-golden approval. Open.
- **Q3. The refusal of D7 as a panic.** Options: (a) panic (taken); (b) the divergence guard of PIMPLE and a located stop in
  SIMPLE, when `pimple.rs` is free. Recommendation (b) with the case key.
- **Q4. The continuity measure.** Options: (a) leave it and document it; (b) subtract the sources. Recommendation (b) with the
  case key (it touches the reporting of both solvers).
