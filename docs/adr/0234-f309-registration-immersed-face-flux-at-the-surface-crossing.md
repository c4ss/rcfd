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

## Addendum 1: results (2026-11-12)

Measured against the limits registered above. No limit was changed, no amendment was needed, and no registered run was repeated.
Release builds on Windows. The registration is c26acfdc; the library and the registered tests are c68c3665, committed before
their first run; every registered run was made once on that commit (one invocation of the test file, one thread). This
addendum's commit changes no code.

**Built.** `crates/rcfd-fv/src/immersed.rs`: `FluxFace`, the field `ImmersedForcing::flux_faces` (empty from `build`),
`ImmersedForcing::build_second_order`, `ImmersedForcing::reconstruct_face_flux` (D1 to D3, the panic of D7).
`crates/rcfd-fv/src/simple.rs`: the import of `solve_pressure_parts_src` in place of `solve_pressure_parts` (line 27), one
statement after the interpolated `phiHbyA` (lines 1174 to 1180), the plain pressure solve with the source (lines 1278 to 1287).
`crates/rcfd-fv/src/pimple.rs`: one statement after the `ddt` flux correction (lines 1346 to 1350) and the plain pressure solve of
the correctors with the source (lines 1418 to 1423, one line replaced by six). `crates/rcfd-fv/tests/immersed_second_order_f309.rs`
(new). `docs/reference-data.md` (a section for F-309). No case-schema key, golden, hash table or existing test file was touched;
`sim.rs` and `schema.rs` were not touched.

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A1 (acceptance) CC second: `e(96)` | `<= 1e-2` | `4.243e-4` (`e` 6.121e-3, 1.611e-3, 4.243e-4 at N 24, 48, 96) | met |
| A1 (acceptance) CC second: order 48 to 96 | `>= 1.5` | **1.925** (24 to 48: 1.926) | **met** |
| S1 CC convergence, six runs | `max_du <= 1e-10` within 20 000 | second 103, 102, 340 iterations; first 105, 106, 338 | held |
| C1 (negative control) CC first: order | `< 1.5` | 1.023 (`e` 8.293e-3, 3.202e-3, 1.576e-3, the numbers of ADR 0218) | held |
| V1 `e_second(96)/e_first(96)` | `<= 0.75` | 0.269 | met |
| V2 `eps_first(96)/eps_second(96)` | `>= 3` | 39.0 (`eps` second 1.903e-3, 4.737e-4, 1.612e-4; first 3.079e-2, 1.598e-2, 6.286e-3) | met |
| V3 PISO at N 48: steady; `e`; `eps` | `<= 1e-9`; `<= 1.2 e_SIMPLEC`; `<= 1.5 eps_SIMPLEC` | 1.5e-12; 1.61055e-3 against 1.61066e-3; 4.764e-4 against 4.737e-4 | met |
| V4 DFG 2D-1 second: convergence rule on M1, M2; `C_D`, `C_L` on M2 | within the caps 4000; within 5 percent | 470 and 750 iterations; `C_D` 5.56246 (-0.306 percent), `C_L` 0.0108880 (+2.534 percent) | met |
| U1 reconstruction of the exact field | max at N 96 `<= 2e-3`; rms(48)/rms(96) `>= 3` | 3.24e-4 (1.17e-3 at N 48); 3.889 | met |
| U2 net flux per body; sum of the sources; fluid cells; other faces | `<= 1e-12 A_b`; `<= 1e-12 sum A_b`; 0; bit-identical | -6.0e-18 and 9.5e-17 (areas 0.4, 0.8); -9.7e-17; 0; bit-identical (120 flux faces) | met |
| U3 the forcing of `build` | no flux faces; `None`, untouched; equal coefficients | as required | met |
| N1 defaults: state hashes | `3b49c4ed4863d1b6`, `9fff50792adb6d4a` | `3b49c4ed4863d1b6`, `9fff50792adb6d4a` | met |
| B1 `modified = false` | `Err` naming `modified` | "... needs the modified forcing (settings.modified = true)" | met |
| R1 gradient options of the pressure equation, SIMPLE and PIMPLE | panic naming `ADR 0234` | both | met |

Printed, no limit. CC second on N 192: `e` 1.034e-4, order 96 to 192 2.037 (`eps` 2.79e-5, order 2.53), 1223 iterations. Parts of
`e` at N 96: second radial 1.08e-4, azimuthal 4.10e-4; first radial 1.17e-3, azimuthal 1.05e-3 (the wall-normal error of the
forcing of ADR 0218 is ten times smaller with the flux of D1). Largest `abs(p)` over the fluid cells: first 0.26, 0.50, 0.62
(N 24, 48, 96), second 0.14, 0.13, 0.19 (0.11 at N 192), with the solid cells next to the fluid at the same size in the second
(0.14, 0.13, 0.19): the order-1 pressure jump derived above for the forcing of ADR 0218 is in its numbers and gone in the
second. (The solid-cell value is printed as 0 for the first forcing, which has no flux-face list; it was not measured there.)
2D-1 on M1 second: `C_D` 5.51738 (-1.11 percent), `C_L` 0.0114183 (+7.53 percent); ADR 0218 measured 5.52887 and 0.0117566 on M1,
5.57022 (-0.17 percent) and 0.0109084 (+2.73 percent) on M2.

**The torque is still first order (measured, printed; not a registered check).** The inner torque from the reaction of the
forcing (`force_moment`), exact 4.18879: second 4.04376, 4.12171, 4.15767, 4.17239 (N 24 to 192; errors 3.46, 1.60, 0.74, 0.39
percent, halving per refinement); first 4.07749, 4.14059, 4.17006 (2.66, 1.15, 0.45 percent). The velocity field is second
order with the new treatment; the force and moment are not, and on these meshes they are no better than before (2D-1 on M2:
`C_D` -0.31 percent against -0.17, `C_L` +2.53 against +2.73). `force_moment` sums the forcing terms, which now also hold the
momentum carried into the solid cells by the flux of D1, and takes the wall stress from the one-sided difference between the
crossing and the first fluid centre. Q5.

### Mutations

Applied one at a time to c68c3665 by `w49i-work/f309-scratch/mutate.py` (an exact text replacement checked to occur once), the
named tests run once in release, the file restored and compared byte for byte with the kept copy (equal after each of the eight).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 `phiHbyA_f` not replaced, source returned | A1 or V2 | failed: the N 48 and N 96 runs diverged (S1; `e`, `eps` NaN, so A1, V1 and V2 fail) |
| MU2 source returned as zeros | A1 or V2 | failed: the N 48 and N 96 runs diverged (S1, A1, V1, V2) |
| MU3 `t_f/theta_e -> t_f` | A1 or V2, and U1 | failed: A1 order 0.710, V1 ratio 1.96, V2 ratio 0.84; U1 max 1.37e-2, rms ratio 2.42 |
| MU4 no net-flux correction | U2 | failed: net -3.2e-2 and 0.17, sum of the sources -0.14 |
| MU5 SIMPLE: source not passed | A1 or V2 | failed: as MU2 |
| MU6 PIMPLE: statement removed | V3 | failed: `e` 3.195e-3 (the number of ADR 0218 V3), `eps` 1.598e-2 |
| MU7 PIMPLE: source not passed | V3 | failed: `e` 1.125e-2, `eps` 1.305e-2 |
| MU8 `modified = false` not refused | B1 | failed: `unwrap_err` on an `Ok` |

`scripts/limit_audit.py`: 0 hits.

### Tools

`cargo fmt -p rcfd-fv --check`: clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings`: clean. `cargo test --release -p
rcfd-fv --test immersed_second_order_f309`: 9 passed, 1 ignored (the printed N 192 run, run once with `--ignored`); in the debug
profile 5 passed, 5 ignored (the heavy ones). The existing `immersed_solid_f219.rs`, unedited, run once in release on the final tree: 11 passed, 2 ignored (its known
miss and its diagnostic), with the numbers of ADR 0218 (M3 `C_D` 5.57833680, `C_L` 0.0106899517, `St` 0.30073, V1 `e` 8.2929e-3,
3.2018e-3, 1.5761e-3) and the M1 state hash `8d13ea8c24b75cd0` that ADR 0218 recorded: a second measurement of N1.

### Deviations and disclosures

- The expectation written with the mutations was wrong for MU1, MU2 and MU5: I expected about the numbers of the forcing of
  ADR 0218; the SIMPLEC iteration diverges instead on N 48 and N 96 (in PISO, MU7, it stays bounded with `e` seven times
  larger). The flux of D1 without the source of D2 is not a usable scheme; the two are applied by one call and cannot be used
  apart through the library.
- The registration says the solid-cell pressure is printed for every run; for the first forcing it is not available (above).
- Before the first run only the build, `cargo fmt` and `cargo clippy` were run on the code; clippy asked for the comparisons of
  the A1 test to be written through a helper (`le`), which was done before the commit and the run. The first build warned of an import left unused in
  `simple.rs`, which was corrected. Nothing was tuned: the library of c68c3665 is the first version that was run.
- Not run: the workspace test suite and the gate (the lead's); the 2D-2 case, M3 and the Strouhal number with the second-order
  forcing; a non-uniform mesh at the surface in CC; the sphere and the cuboid; BDF2; more than one non-orthogonal corrector.

### Decision by the registered rule

A1 met with S1 and C1 held; V1 to V4, U1 to U3, N1, B1 and R1 met; each of the eight mutations failed a test it must fail.
**F-309 is done by the rule**, in the library, pending the maintainer's acceptance. "Done" does not include: a case key or a
Studio control (Q2), the treatment as the default (Q1), the force and moment (Q5), and the limits of the registration.

### Remaining questions

- **Q1. Default** (as registered). Options: (a) keep the opt-in; (b) make the flux of D1 to D3 part of the modified forcing, so
  that `build` gives it. (b) changes the numbers of every present immersed run (ADR 0218's A1 to A3 would be measured again; on
  M2 the drag moves from -0.17 to -0.31 percent). Recommendation: (a) until Q5 is settled, then (b) with a new registration.
- **Q2. Case key** (as registered; ADR 0218 Q2). Recommendation: one follow-up item for the key `immersed_solids` with a field
  for this treatment, with the schema-golden edit approved.
- **Q3. The refusal as a panic** (as registered). Recommendation: (b) the divergence guard in PIMPLE and a located stop in
  SIMPLE, in the item of Q2.
- **Q4. The continuity measure** (as registered). Recommendation: (b) subtract the sources, in the item of Q2.
- **Q5 (added). The force and moment are first order.** Options: (a) a follow-up item: the force from the stress at the crossing
  (the pressure extended to the surface and the wall gradient of the extension, integrated over the flux faces) or the reaction
  with the momentum flux of D1 taken out, registered with the CC torque as its check; (b) leave it. Recommendation (a).
- **Q6 (added). The corrector-count estimate** (`pick_correctors`, ADR 0156) solves its trial pressure without D1 to D3.
  Options: (a) refuse corrector control with the flux faces; (b) pass the reconstruction to the trial; (c) leave it (the estimate
  only chooses a count). Recommendation (b), when `pimple.rs` is free.

### For the lead: roadmap entries

**F-309: proposed status `done`** (pending acceptance). Proposed note (replacing the present one):

> DONE in the library (ADR 0234, 2026-11-12), pending acceptance. From ADR 0218 question 6. Delivered: rcfd-fv immersed.rs,
> `ImmersedForcing::build_second_order` (opt-in; `build`, the forcing of ADR 0218, is unchanged and bit-identical, state hashes
> checked): the flux of every fluid-solid face is that of the linear extension of the fluid profile through the no-slip value at
> the surface crossing, the net flux of each body is that of its rigid velocity, and the solid cells get the volume source that
> releases their continuity (`reconstruct_face_flux`; one statement and the source of the plain pressure solve in SimpleSolver and
> PimpleSolver). Measured (limits registered before the code): circular Couette, the F-219 V1 check, error 6.12e-3, 1.61e-3,
> 4.24e-4 at N 24, 48, 96, observed order 1.925 (limit 1.5; 2.04 from 96 to 192) against 1.023 of the forcing of ADR 0218, error
> at N 96 3.7 times smaller; face-flux error 39 times smaller; PISO equal to SIMPLEC to 1e-4; DFG 2D-1 on M2 C_D -0.31 %, C_L
> +2.53 %; eight mutations caught. Cause of the first order of ADR 0218 (derived and seen in the numbers): the continuity of a
> solid cell pins the flux of its fluid-solid face to the rigid velocity, a face velocity error of order h with a pressure jump
> of order 1. Not included: a case key or Studio control (schema golden); not the default; plain pressure path only (gradient
> options, body-force-weighted and implicit non-orthogonal pressure panic); the force and moment from the forcing stay first
> order (CC torque error 1.60 %, 0.74 %, 0.39 % at N 48, 96, 192); the continuity measure holds the solid-cell sources; the
> limits of F-219 (no moving bodies, momentum only, not in the coupled solver, the fractional step or on moving meshes).

**Proposed follow-up items:**

1. *Immersed solid: second-order force and moment.* Deps: F-309. Acceptance: "The torque on the immersed rotating cylinder of
   circular Couette flow (the CC set-up of ADR 0218, second-order forcing) reaches an observed order of 1.5 between N 48 and 96,
   and the DFG 2D-1 drag and lift on M2 are within 1 percent and 3 percent of the FeatFlow values." Note: from ADR 0234 Q5.
2. *Immersed solids in the case tree.* Deps: F-219, F-309. Acceptance: "A case key `immersed_solids` (shape, inverted, velocity,
   omega, origin, penalty, modified, second_order) runs the DFG 2D-1 case from a case file within 5 percent; a case without the
   key hashes as before; the second-order treatment with an unsupported pressure option is refused with a located diagnostic;
   the continuity residual excludes the solid-cell sources." Note: from ADR 0218 Q2, Q3 and ADR 0234 Q2 to Q4, Q6; needs the
   schema-golden edit approved.
