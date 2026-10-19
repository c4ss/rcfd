# ADR 0074: Wave 34C, Symmetry boundary term audit and the SST flat-plate convergence order

Status: registered before any rcfd run or code change of this wave (this file is committed first; results go into an addendum, limits are never widened).
Branch main, written at HEAD 1f571fb. Parents: ADR 0010 (component-consistent diagonal), ADR 0070 (wave 33C, addendum: Symmetry finding, P2f known miss,
+2 percent u+ offset), ADR 0071 (wave 33D, `coupled_body_pair_mean`), ADR 0063 (HA1 to HA10).

## Open items

(1) A body-forced periodic half channel with a `Symmetry` top behaves like a channel with a second no-slip wall in the laminar solver (ADR 0070: tau_w 0.558 / u_max 15.6
with `h_from_solution`, 0.505 / 12.75 without, against 1 / 50), the full channel is right.
(2) The SST flat plate (ADR 0070 P2f): Cf(x_ref = 0.970084) = 2.57698e-3, 2.65138e-3, 2.69840e-3 on 70 x 55, 140 x 110, 280 x 220, observed order 0.662 (limit 0.8),
with a near-uniform +2 percent u+ offset against the TMR SST profile and Cf -0.67 percent below the incompressible TMR mean 2.71647e-3. e21 = 7.440e-5, e32 = 4.702e-5.
(3) A written analysis of what `solver.component_diagonal = "corrected"` as the default would change (decision: the user's).

## Disclosure of what is known before any run (reading only)

* ADR 0010 states the mechanism: the SIMPLE/SIMPLEC kernels build the pressure equation and `HbyA` with `a_P`, the MEAN of the component diagonals; a `Symmetry`
  face puts its Dirichlet term on the normal component only (`bc.rs`: tangential `a = 1, b = 0`, i.e. zero gradient; normal `a = 0`), so in the adjacent cell
  `a_n > a_t`. Reading `SimpleSolver::iterate`: the converged velocity is `u_k = (H_k + V (f - grad p)_k) / a_P` while the component matrix is
  `a_k u_k = H_k + V (f - grad p)_k`, so the fixed point carries an extra sink `(a_P - a_k) u_k` on the tangential component, and with the under-relaxed matrix
  (`a/alpha_u` on the diagonal, `(1 - alpha_u)/alpha_u a u_old` in `b`) the sink is `(a_P - a_k)` of the relaxed diagonals, i.e. about
  `(a_n - a_t)/(2 alpha_u) u_t` in 2-D. In a diffusion-dominated laminar half channel `a_n - a_t = nu A/(dy/2)`, the coefficient of a wall: the top acts like a partial
  no-slip wall, which matches the ADR 0070 numbers. On the high-Re plate the same term is negligible against convection (ADR 0015 B6 measured a Cf(0.97) change of
  0.000 percent on the y+ 0.5 plate). The existing opt-in `component_diag_correction` (`solver.component_diagonal = "corrected"`) removes it. This expectation
  comes from reading the code and ADR 0010; it has NOT been run for this case before this ADR.
* `coupled_body_pair_mean` (ADR 0071) changes only the body-force term of periodic pair faces; in an x-invariant periodic channel the two cells of every pair
  have equal `rAU`, so it is predicted to change nothing.
* Reading `rcfd-api/src/sim.rs`: the key `solver.component_diagonal` is applied to the steady SIMPLE/SIMPLEC kernel (and to MRF) and to the variable-density
  kernel (`vd.rs`), but NOT to the transient PISO/PIMPLE kernel (`PimpleSettings::component_diagonal`, ADR 0069 F3, is never set by the API), while
  `docs/case-schema.md` says the key applies to "the segregated steady/transient constant-density kernels". The VOF kernel also averages the diagonal and has
  no correction (not changed here, reported).
* Discrete solution of the laminar half channel derived here (uniform cells `dy = h/N`, wall at y = 0 with the half-cell coefficient `nu/(dy/2)`, zero flux at the
  top, periodic in x, body force f): the interior and the top-face balances are exact for a quadratic, the wall balance is not; inserting `u_i = u_e(y_i) + C` with
  `u_e = f (h y - y^2/2)/nu` gives `C = f dy^2/(8 nu)` exactly and a discrete wall shear `nu u_1/(dy/2) = f h` exactly. So with a consistent momentum equation
  the discrete solution is the exact profile shifted by the constant C (no other error), and the discrete wall shear is exact.
* Literature read for this ADR (curl of the raw HTML, 2026-10-19): https://tmbwg.github.io/turbmodels/sst.html (the wall condition
  `omega_wall = 10 * 6 nu/(beta_1 (Delta d_1)^2)` is printed as an equation; the page does NOT define `Delta d_1` in words next to it; ADR 0063/0070 read it as the
  first-cell distance, and rcfd uses the cell-centre distance), https://tmbwg.github.io/turbmodels/flatplate.html (the TMR case is compressible, M = 0.2; the top at
  y = L "has very little influence": moving it to 0.48 L changed Cf(0.97) by less than 0.2 percent), https://tmbwg.github.io/turbmodels/flatplate_grids.html (five
  nested grids, "each coarser grid is exactly every-other-point of the next finer grid", 545 x 385 to 35 x 25, finest wall spacing 5e-7, "clustered near the plate
  leading edge"), and the grid file `flatplate_clust2.p2dfmt` (PLOT3D, from the NASA zip `flatplategrids-grids.zip` linked on that page) and
  `grid_quad_all_vol.mapbc` (boundary kinds: two y-symmetry patches, tunnel inflow, tunnel outflow, the y = 0 symmetry run-up, viscous wall, farfield top).
  Reading the grid file (my script, no CFD code): the grid is an exact tensor product (x depends only on i to 1e-14, y only on j to 3e-12), x from -0.33333 to 2,
  the plate starts at node 96 (x = 0, 96 cells of symmetry run-up, 448 cells on the plate), first plate cell dx = 1e-3, wall spacing 5e-7 with a near-wall growth
  ratio of about 1.002 (rcfd: 1.21 / 1.10 / 1.0488).
* Prior measurements that bear on item 2 (from earlier ADRs, not re-run): ADR 0063 HA1 (wall-face omega factor 1 instead of 10: Cf 2.8329 / 2.7749 / 2.7589e-3,
  order 1.9, i.e. the wall omega value controls the order), HA2 (first-cell omega fixed, medium only: -0.45 percent), ADR 0061 (linear-upwind k and omega on the medium
  mesh: Cf deficit -7.02 to -7.13 percent, i.e. 0.11 percent; twice the streamwise cells: 0.08 percent), ADR 0015 B6 (corrected diagonal: 0.000 percent at y+ 0.5),
  ADR 0015 (linear-upwind k/omega "diverged in the F-043 note").
* Arithmetic fixed before any run: for the observed order to reach 0.8 with the coarse and medium values unchanged, e32 must fall to e21/2^0.8 = 4.27e-5, i.e. the
  finest Cf must move by +4.3e-6 = 0.16 percent; a mechanism that moves Cf by less than 0.05 percent on the coarse AND the medium mesh is not a candidate cause of
  the low order (assumption stated: the effect of a set-up change does not grow under refinement; if it moves the coarse/medium values by 0.05 percent it changes
  p by at most about 0.03 through e21).

## Part A: Symmetry boundary, term by term (tests; laminar unless stated)

All runs: SIMPLEC (`SimpleSettings::new`), converged to `max_du < 1e-13` or the stated cap; "corrected" = `component_diag_correction = true`, "averaged" = default.
Laminar channel: 4 cells in x periodic, nu = 0.01, body force f = 1 along x, height h = 1 with `Symmetry` top (half channel) or h = 2 with two walls (full channel).
New file `crates/rcfd-fv/tests/w34c_symmetry.rs` (fv level).

| id | check | limit |
|----|-------|-------|
| A1 momentum coefficients | on a box with `Symmetry` at ymax: the assembled `laplacian` + `convection` matrix of the tangential component (x) equals bit for bit the one with `ZeroGradient` on that patch (diag and b); of the normal component (y) equals bit for bit the one with `FixedValue(0)` | bit-identical |
| A2 scalar | `Symmetry` on a scalar field gives the same matrix as `ZeroGradient` (the k and omega equations of the SST see a zero-gradient plane) | bit-identical |
| A3 pressure correction | after every iteration of the laminar half channel: `pressure_bc` marks every Symmetry face `fixed_u`; the face flux on the Symmetry faces is zero | abs(phi_f) <= 1e-15 max abs(phi) |
| A4 Poiseuille, uniform 4 x 40, corrected, `h_from_solution` false and true | max_i abs(u_i - (u_e(y_i) + f dy^2/(8 nu))) / (f h^2/(2 nu)) and abs(tau_w/(f h) - 1) with tau_w = nu u_1/(dy/2) averaged over the wall faces | <= 1e-8 both |
| A4m same with the averaged default | same limits, registered as an EXPECTED MISS (`*_known_miss`, ignored), values reported | (1e-8) |
| A5 own-equation residual | at the converged A4 state: the x-momentum balance of every cell assembled from the same operators (diffusion, convection, body force, the cell pressure gradient), relative to f V | corrected <= 1e-8; averaged reported (expected about (a_P - a_x) u_x/(f V) in the top row) |
| A6 mirror equivalence on a graded mesh | half channel with 40 cells graded towards the wall (`WallLayers`, first 0.005, growth 1.1, 20 layers) against the full channel whose y-nodes are the half-channel nodes mirrored at y = 1 (exact mirror image), both corrected: max abs(u_half - u_full) over the lower half / u_max | <= 1e-8 |
| A7 shear-free plug (symmetry-plane Couette 1) | bottom wall moving at U_w = 1, `Symmetry` top, periodic x, no force, 4 x 20 cells, started at rest: max abs(u - U_w)/U_w (exact u = U_w) | corrected <= 1e-8; averaged registered as EXPECTED MISS |
| A8 plane Couette with symmetry side planes (symmetry-plane Couette 2) | 3-D box 4 x 16 x 4 (x periodic), fixed wall y = 0, wall moving at U = 1 along x at y = 1, `Symmetry` at zmin and zmax: max abs(u_x - U y/h)/U, max(abs(u_y), abs(u_z))/U (exact linear Couette, which a second-order FV scheme reproduces exactly) | corrected <= 1e-8 and <= 1e-10; averaged reported, no limit (predicted NOT exact: the z-normal Dirichlet term raises a_z in the side cells, and the averaged a_P then acts as an extra sink (a_P - a_x) u_x on the tangential u_x) |
| A9 transient kernel (PIMPLE, fv level) | A7 plug started AT the exact solution u = U_w with `PimpleSettings::piso`, dt 0.05, 40 steps: corrected (`component_diagonal = true`) max abs(u - U_w)/U_w | <= 1e-10; averaged reported |
| A10 `coupled_body_pair_mean` | A4 averaged with the option on and off | identical to 1e-12 relative (prediction: unrelated) |

SST (rcfd-turb, new file `crates/rcfd-turb/tests/w34c_symmetry_sst.rs`), corrected unless stated:
| id | check | limit |
|----|-------|-------|
| A11 SST half versus mirrored full channel | Re_tau = 395, nu = 1/395, f = 1, `Automatic`, half channel 30 cells (`WallLayers` first 2/395, growth 1.1, 16 layers) against the full channel built from the mirrored nodes (60 cells), initial U = 20, k = 1, omega = 1/(10 nu), converged to du < 1e-9 and nu_t change < 1e-8 (cap 30000): (a) abs(U_b,half/U_b,full - 1); (b) max abs(u_half - u_full)/u_centre over the lower half; (c) the discrete force balance sum(tau_w A)/(f V) for both | (a) <= 0.5 percent; (b) <= 1.0 percent; (c) within 1e-6 of 1 |
| A11m the same half channel with the averaged default | (a) and (b) as above | EXPECTED MISS, ignored, reported |
| A12 ADR 0070 P3 re-run, NEW corrected set-up | the registered ADR 0070 P3 half channel (Re_tau 1e6, 150 cells, 40000 cap, Symmetry top) with the correction on; limits exactly those of P3a (KM within 0.02 of TMR at y+ 100, 316, 1000, 3000, 10000) and P3b (slopes within 4 / 3 percent) | as ADR 0070 (heavy, ignored) |

Decision rule A: if A4 (corrected) passes and A4m misses, the half-channel defect is the averaged diagonal of ADR 0010 and the opt-in fix already exists; no new
solver code for it. A miss of A1 to A3 would be a defect in `bc.rs`/`pressure.rs` and is fixed (opt-in if a baseline moves). A11 is the turbulence-transport
check: a pass means the SST transport at a Symmetry plane is consistent (to the stated limits) once the momentum equation is.
Transient wiring (W1): the API key will be passed to the transient kernel (`s.component_diagonal = get_s(case, "solver.component_diagonal") == "corrected"` in
`sim.rs`, one line, opt-in: nothing changes unless the key is set). Checks in `crates/rcfd-api/tests/w34c_component_diagonal_transient.rs` on the packed bed
(`cases/packed_bed`, symmetry walls) with `algorithm = "piso"`, `dt = 0.01`, 100 steps: (W1a) without the key and with `averaged` the fields are bit-identical; (W1b) with
`corrected` the fields differ from (W1a) (the key is honoured); (W1c) max abs(u_y) (exact 0) with `corrected` is not larger than with `averaged`.
`baseline_hashes` must stay unchanged (no baseline case uses the key).

## Part B: SST flat plate, hypotheses for the low observed order (registered before any run of this wave)

Set-up of every run: the runner of `validation_w33c_sst_plate.rs` copied unchanged (U = 1, nu = 2e-7, x in [0, 2], height 1, Symmetry top, inlet k = 2.25e-7,
omega = 125, SIMPLEC, linear-upwind momentum, `Automatic`, du < 1e-9 and nu_t change < 1e-6, cap 14000) with ONE factor changed per hypothesis. Meshes 70 x 55,
140 x 110, 280 x 220 as in ADR 0070. Each mesh is run at most once per hypothesis. New file `crates/rcfd-turb/tests/w34c_sst_plate_hypotheses.rs` (heavy, ignored).
Metrics (definitions of ADR 0070): Cf(x_ref), observed order p, |Cf_f - Cf_m|/Cf_f, Richardson value, Cf of each mesh against the incompressible TMR mean 2.71647e-3,
u+ offset at y+ 5, 10, 15, 30, 50, 100, 200, 300 against the TMR profile (column nearest x_ref).

Stage 1 (screen, coarse + medium): the hypothesis survives only if abs(Cf' - Cf) / Cf > 0.05 percent on the coarse or the medium mesh (against the ADR 0070 values,
which are deterministic and reproduced by ADR 0063 and 0070), or if the run fails to converge (then it is reported, no finest run). Stage 2 (finest) only for survivors.
| id | hypothesis | single change |
|----|------------|---------------|
| H1 | the Symmetry top (item 1) | `component_diag_correction = true` |
| H2 | first-order upwind convection of k and omega | `KOmegaSst::convection = LinearUpwind` |
| H3a | wall omega: the TMR `Delta d_1` read as the wall-cell HEIGHT (node-based reading) instead of rcfd's cell-centre distance | wall-face omega `60 nu/(beta1 (2 dn)^2)` (a factor 1/4 on the face value) |
| H3b | wall omega: the face value on stretched meshes; fixing omega in the wall cell instead | `omega_fix_y_plus = 0` (wall-cell omega fixed to the existing blended viscous/log value in every wall cell, face value kept) |
| H4 | leading edge / inlet: rcfd puts the Dirichlet inlet on the leading edge; TMR has a symmetry run-up | upstream section x in [-1/3, 0] with a `Symmetry` bottom, nx/5 cells (14, 28, 56) graded geometrically so that the cell at x = 0 equals the first plate cell; inlet moved to x = -1/3; plate cells unchanged |
| H5 | the mesh family (not exactly nested: the layer stacks and the bulk cells of the three meshes are generated independently) | no run: the actual local refinement ratios of the three meshes are computed (wall cell, layer-stack height, cell at y = 0.01, 0.02, bulk; x cells at 0.01, 0.5, 0.97) and reported |
| H6 | the set-up as a whole (mesh family, leading-edge clustering, run-up) against identical grids: rcfd on the TMR nested grids | the TMR grids 69 x 49, 137 x 97, 273 x 193 nodes (3264, 13056, 52224 cells; every-other-point of the vendored 545 x 385 node coordinates), inlet at x = -1/3 (fixed U = 1, k, omega as above), `Symmetry` for y = 0, x < 0, wall for x >= 0, `Symmetry` top (TMR: farfield; see flatplate.html on the top's small influence), fixed p outlet; cap 30000. No stage-1 screen: all three grids. |
The node coordinates of the TMR grid are vendored as `validation-data/reference/w34c_tmr_flatplate_545x385_axes.csv` (the 545 x-coordinates and 385 y-coordinates;
the grid is a tensor product), manifest entry `w34c-tmr-flatplate-grid-axes`.

Decision rules B (fixed now):
* A hypothesis is FALSIFIED as the cause of the low order if it fails the stage-1 screen, or if with its change the observed order stays below 0.8.
* A hypothesis is CONFIRMED as the cause only if with its change (i) p is in [0.8, 2.5] and |Cf_f - Cf_m|/Cf_f <= 1.5 percent (the P2f limits), AND (ii) independent
  check (rule b): the finest Cf and the Richardson value both lie within 2.0 percent of the incompressible TMR mean 2.71647e-3, AND (iii) the u+ offset of the finest
  mesh at y+ 30 to 300 is below 1.0 percent in magnitude (the mechanism must move the offset and the order together). Otherwise it is "influential, not isolated".
* H6 additionally: rcfd Cf(x_ref) on each TMR grid against the mean of the three incompressible codes on the same grid (3264: 2.64948e-3; 13056: 2.68947e-3;
  52224: 2.70909e-3, means of the vendored values). H6 is called CONSISTENT WITH THE TMR CODES if on the 52224 grid rcfd is within 1.0 percent of that mean and the
  observed order over the three grids is in [0.8, 2.5]. If H6 is consistent and rcfd's own family is not, the low order is a property of rcfd's mesh/set-up family,
  not of the SST implementation.
* No hypothesis change becomes a default. An improvement is claimed only as measured against the TMR data (mean of the incompressible codes; the CFL3D u+ profile).
  A confirmed cause that is a set-up property gets a NEW corrected P2f-type test (the ADR 0070 known miss stays ignored with its limits); a confirmed cause that is a
  solver option stays opt-in (no baseline moves).
* Linear-upwind k/omega (H2) may diverge: then it is reported as not converged, not retried with other relaxation.

## Part C: what `solver.component_diagonal = "corrected"` as the default would change (analysis, no change of the default)

Method (fixed now): in a scratch copy of the tree only (never committed), the default of the case key is flipped to "corrected" (the `get_s` comparison in
`sim.rs` and `vd.rs` treated as corrected unless the case says `averaged`; the transient wiring of W1 included), `baseline_hashes::check` prints instead of asserting,
and the whole workspace test suite is run once with `--release --no-fail-fast`. Reported: every baseline hash that moves (old and new steps and hash), the size of the
change of each moved shipped case at equal iterations and at convergence (iterations, max field differences), every test that changes outcome, and the shipped `cases/`
that contain a symmetry-type boundary. Library defaults (`SimpleSolver::new`, `PimpleSettings`, `VdSettings`) are not part of the analysis (fv-level tests set them
directly); this is stated. Output: `docs/component-diagonal-default.md`. Hashes are not re-recorded; the default is not changed.

## Process

ADR first; tests are new files; a miss becomes `*_known_miss` (ignored) with the limit unchanged; a defective set-up gets a NEW test. Files touched:
this ADR, the new test files, `validation-data/reference/w34c_*.csv` and its manifest entry, one line in `crates/rcfd-api/src/sim.rs` (W1, own hunk only, opt-in),
`docs/case-schema.md` (W1 wording, if needed), `docs/component-diagonal-default.md`. Not touched: `baseline_hashes.rs`, any default.

## Amendment 1 (registered after the stage-1 runs and before any further run; no limit changed)

Stage 1 results so far (details go to the addendum): H1 -0.0001 / +0.0000 percent (falsified by the screen); H2 not converged on the coarse mesh (14000
iterations, du 6.7e-2), -0.09 percent on the medium mesh (no finest run, as registered); H3a +5.23 / +2.46 percent (survives); H3b -0.83 / -0.42 percent (survives);
H4 not converged on either mesh (du 1.1e-5 and 1.6e-5 after 14000 iterations, Cf -12.8 and -11.7 percent); H6 (same run-up topology) was stopped after about
28000 iterations on the two coarser TMR grids because du was still 8e-6 (it would have ended unconverged at the 30000 cap; no result is taken from it).
Debug print (one coarse H4 run, disclosed, not a registered result): the largest changes sit in the run-up, where u falls to 0.04 to 0.05 in the cells next to the
`Symmetry` bottom at x = -0.05 to -0.06 (y = 1.8e-5) and nu_t/nu reaches about 800 upstream of the leading edge: the run-up symmetry plane with wall-resolved cells
(dy 5.6e-6, diffusion-dominated) acts as a no-slip wall, which is exactly the Part A defect of the averaged diagonal (A4m, A7m). The registered H4 and H6 set-ups
therefore test the solver defect rather than the leading-edge topology. NEW corrected set-ups, registered now:
* H4c: H4 with `component_diag_correction = true` (coarse + medium, the same screen rule).
* H6c: H6 with `component_diag_correction = true`, all three TMR grids, cap 30000, the same decision rule (within 1.0 percent of the incompressible-code mean on
  the 52224 grid and order in [0.8, 2.5]).
H1 showed that the correction changes the plate without run-up by less than 1e-6 relative, so H4c and H6c differ from H4 and H6 only in the run-up treatment.
The original H4 / H6 entries stay as measured (not converged).
Stage 2 (finest) runs now: H3a and H3b (both survived stage 1).
