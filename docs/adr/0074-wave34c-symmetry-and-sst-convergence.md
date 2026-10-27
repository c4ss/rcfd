# ADR 0074: Wave 34C, Symmetry boundary term audit and the SST flat-plate convergence order

Status: registered before any rcfd run or code change of this wave (this file is committed first; results go into an addendum, limits are never widened).
Branch main, written at HEAD c785dca. Parents: ADR 0010 (component-consistent diagonal), ADR 0070 (wave 33C, addendum: Symmetry finding, P2f known miss,
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

## Addendum (measured values; limits above untouched)

Outcome in one paragraph. (1) The half-channel defect is the averaged momentum diagonal of ADR 0010, not the `Symmetry` condition: every term of a
`Symmetry` face (momentum coefficients, scalar condition, pressure-correction flux) is what it should be (A1 to A3), and with the existing opt-in
`component_diag_correction` (`solver.component_diagonal = "corrected"`) the half channel reproduces the derived discrete Poiseuille solution to 1e-10, equals the
mirrored full channel to 2e-11 (laminar) and to the printed digits (SST), and the plug, Couette and transient exact solutions hold to 1e-13 (A4 to A9, A11, A12b).
With the averaged default the same set-ups are wrong by O(1). No new defect in `bc.rs`, `pressure.rs` or the SST transport. One wiring defect was found and
fixed (opt-in, no baseline moves): the case key never reached the transient PISO/PIMPLE kernel although it was documented for it (W1). (2) The SST plate's low
order (0.66) is a property of rcfd's own mesh family/set-up, not of the SST implementation: on the NASA TMR nested grids (H6c) rcfd converges with order
1.07 (TMR codes about 1.0 to 1.3), lies 0.82 to 0.87 percent below the mean of the three incompressible TMR codes on every grid, its Richardson value is
-0.44 percent from the TMR mean, and the +2 percent u+ offset falls to 0.66 percent on the 52224-cell grid (3.26, 1.51, 0.66 percent: a discretisation error
of the near-wall region). Every single-factor change on rcfd's own meshes (H1, H2, H3a, H3b, H4c) left the order below 0.8 or the sequence non-monotone.
Which part of the TMR family (near-wall spacing and growth, leading-edge clustering) is responsible was not separated. (3) A measured analysis of a
`corrected` default is in `docs/component-diagonal-default.md`: exactly one registered baseline hash moves (`packed_bed`); every other shipped case is
bit-identical.

### Part A (`crates/rcfd-fv/tests/w34c_symmetry.rs`, `crates/rcfd-turb/tests/w34c_symmetry_sst.rs`, `crates/rcfd-api/tests/w34c_component_diagonal_transient.rs`)

| id | measured | limit | result |
|----|----------|-------|--------|
| A1 | tangential matrix equals ZeroGradient, normal equals FixedValue(0) (diag, off-diagonals, b compared with `==`); zero flux through the face | equal | pass |
| A2 | scalar `Symmetry` matrix and boundary values equal ZeroGradient | equal | pass |
| A3 | every Symmetry face `fixed_u` in every iteration; abs(phi_f)/max abs(phi) = 0 (60302 iterations) | 1e-15 | pass |
| A4 (h_from_solution on) | profile error 9.6e-11 of u_max, tau_w/(f h) - 1 = -6.9e-11 (60302 iterations) | 1e-8 | pass |
| A4b (h_from_solution off) | 2.3e-11, -1.7e-11 (220481 iterations) | 1e-8 | pass |
| A4m averaged | off: 0.978, -0.495 (u_max 12.754, top cell 1.114); on: 0.920, -0.442 (u_max 15.570, top cell 4.021); exact u_max 50 | 1e-8 | expected MISS (ignored) |
| A5 own-equation residual | corrected 2.7e-10 of f V; averaged 71.2 (top row: the sink (a_P - a_x) u_x) | 1e-8 | pass |
| A6 mirror, graded | 2.2e-11 of u_max | 1e-8 | pass |
| A7 plug | corrected 1.1e-13; A7m averaged 0.813 | 1e-8 | pass / expected MISS (ignored) |
| A8 Couette, symmetry side planes | corrected 5.9e-13, cross-flow 6.1e-14; averaged 8.1e-2 and 7.0e-4 (no limit) | 1e-8, 1e-10 | pass |
| A9 transient plug at the exact solution | corrected 2.2e-16; averaged 0.789 | 1e-10 | pass |
| A10 pair mean unrelated | 1.19e-4 relative | 1e-12 | MISS of the registered prediction (ignored `a10_..._known_miss`), see below |
| A11 SST half vs mirrored full | U_b 18.166921 both, max profile difference 0.0000 percent, force balance 1.000000035 both (5947 / 5968 iterations) | 0.5 %, 1 %, 1e-6 | pass |
| A11m SST averaged | U_b 4.8648 vs 18.1669 (-73.2 %), centre 2.459 vs 20.409, force balance 0.243 | 0.5 %, 1 % | expected MISS (ignored) |
| A12 ADR 0070 P3, corrected, 40000 cap | KM -0.0098, -0.0067, -0.0057, -0.0054, -0.0054 against TMR; slopes 2.7627 (+2.6 %), 2.4759 (+1.3 %); du 9.97e-8 at the cap | P3 limits, du < 1e-8 | MISS of convergence only (ignored `a12_..._known_miss`) |
| A12b same, 250000 cap (added after A12; the ADR 0070 full-channel cap) | converged in 58087 iterations; KM and slopes identical to the ADR 0070 full channel to the printed digits; force balance 1.000000 | P3 limits | pass |
| W1 transient key (packed bed, PISO, 100 steps) | default bit-identical to `averaged`; `corrected` changes u_x by 1.8e-6; max abs(u_y) 8.4e-7 (averaged), 4.2e-14 (corrected) | as registered | pass |

A10 (prediction falsified): with the averaged diagonal the converged half channel is not x-invariant (max abs(u_y) 0.026, u_x varying by 0.73 along the four
periodic cells, already present after the first iteration; a candidate source is the momentum solve with a relative tolerance of 1e-3 seeding
x-variations that the averaged fixed point keeps, NOT tested), so the owner-value
body-force term of the periodic faces (the ADR 0071 defect) leaves a continuity defect of 4.0e-5 that never converges; `coupled_body_pair_mean` removes it
(2e-14) and moves u_max by 1.2e-4 relative. It is a second, small symptom, not the cause of the no-slip behaviour. With the corrected diagonal the state is
x-invariant to 1e-12 and the option does not matter.
Observation (no limit): the corrected iteration with `h_from_solution` off converges slowly (du falls by about a factor e per 10000 iterations).
Decision rule A: A4 passes and A4m misses, so the cause is the averaged diagonal; the fix is the existing opt-in; no new solver code except W1 (one line in
`sim.rs`, the schema doc string, `docs/case-schema.md`).

### Part B (`crates/rcfd-turb/tests/w34c_sst_plate_hypotheses.rs`; control H0: the copied runner reproduces the ADR 0070 coarse value, 2.576977e-3)

ADR 0070 baseline: 2.57698, 2.65138, 2.69840e-3, order 0.662, finest -0.67 % against the incompressible TMR mean 2.71647e-3, u+ offset about +2 %.

| id | coarse / medium / finest Cf(x_ref) (1e-3) | screen (coarse, medium against ADR 0070) | order | finest vs TMR mean | u+ offset y+ 30..300, finest | verdict |
|----|------------------------------------------|------------------------------------------|-------|--------------------|------------------------------|---------|
| H1 corrected diagonal | 2.576977 / 2.65138 / not run | -0.0001 %, +0.0000 % | - | - | - | FALSIFIED (screen) |
| H2 linear-upwind k and omega | not converged (du 6.7e-2, Cf -88 %) / 2.648934 / not run | n/a, -0.09 % | - | - | - | not testable (the coarse mesh diverges); no finest run, as registered |
| H3a wall omega with the cell height | 2.711722 / 2.716642 / 2.730924 | +5.23 %, +2.46 % | non-monotone (differences +4.9e-6, +1.43e-5) | +0.53 % | 0.77 % | NOT CONFIRMED (no monotone convergence); influential |
| H3b wall-cell omega fixed | 2.555507 / 2.640225 / 2.692630 | -0.83 %, -0.42 % | 0.693 | -0.88 % | 2.42 % | FALSIFIED |
| H4 run-up, averaged | not converged (du 1.1e-5 and 1.6e-5 at 14000; Cf 2.247, 2.340) | -12.8 %, -11.7 % | - | - | - | defective set-up (the run-up symmetry plane acts as a wall), amendment 1 |
| H4c run-up, corrected | 2.535471 / 2.604683 / 2.651336 | -1.61 %, -1.76 % | 0.569 | -2.40 % | 2.15 % | FALSIFIED |
| H5 mesh family | local refinement ratios 1.82 to 2.20 (y, coarse/medium), 1.91 to 2.04 (y, medium/fine), 1.99 to 2.05 (x); layer stacks 0.0419 / 0.0440 / 0.0450 | - | - | - | - | not exactly nested (up to 10 percent locally); no run |
| H6 TMR grids, averaged | stopped at about 28000 iterations, du 8e-6 | - | - | - | - | defective set-up (as H4), amendment 1 |
| H6c TMR grids 3264 / 13056 / 52224 cells, corrected | 2.626407 / 2.667342 / 2.686801 (-0.871, -0.823, -0.823 % against the incompressible-code mean of each grid) | - | 1.073 (GCI 0.82 %, difference 0.72 %, Richardson 2.704436e-3 = -0.44 %) | -1.09 % | 0.66 % (3.26 and 1.51 on the coarser grids) | CONSISTENT WITH THE TMR CODES (registered rule) |

Decision rule B: no single factor on rcfd's own mesh family is confirmed; H6c shows that on the TMR grid family rcfd converges like the TMR codes (order 1.07,
within 1 percent of their mean on every grid, u+ offset decaying with an order of about 1.1). The ADR 0070 P2f miss is therefore a property of rcfd's own mesh
family/set-up, not of the SST implementation. As registered, H6c becomes the NEW corrected P2f-type test (it asserts the H6 rule and the ADR 0070 P2a/P2f
limits); the ADR 0070 known miss stays ignored with its limits. The assertions were written after the run, which was not repeated (deviation 4).
H2 and the default wall-omega treatment, by inference from H6c (not a passed H2 run): H6c uses the same first-order upwind k and omega convection and
the same wall-omega face value as the ADR 0070 runs and reaches order 1.07, so neither is by itself sufficient to produce the order 0.66.
The H6c agreement is "consistent" only under the registered H6 rule: rcfd lies 0.82 to 0.87 percent below the mean of the three incompressible codes on
every grid, which on the 13056 and 52224 grids is BELOW the lowest of the three codes (by about 0.3 and 0.5 percent; inside their range only on the 3264
grid). Open: (a) that constant offset (candidates, not tested: the `Symmetry` top instead of the TMR farfield,
the fixed-velocity inlet instead of the TMR total-pressure inflow, Cf normalised with the local u at y = 0.25 instead of U = 1, the Spalding-based `Automatic`
wall shear; the three TMR codes themselves spread by 2.2, 1.0 and 0.6 percent on these grids); (b) which part of the TMR family (wall spacing 5e-7 x 2^level with
a near-wall growth of about 1.002, or the leading-edge clustering) restores the order: not separated; (c) H3a shows that the wall-omega distance convention
moves Cf by 5.2, 2.5 and 1.2 percent on rcfd's meshes; the TMR page does not define Delta d_1 (rcfd uses the cell-centre distance); no default was changed.

### Part C
Measured analysis: `docs/component-diagonal-default.md` (harness `crates/rcfd-api/tests/w34c_component_diagonal_default.rs` and one workspace run of an
uncommitted scratch copy with the schema default flipped). Results: the flipped default moves exactly one registered baseline hash (`packed_bed`: (60, 19e53df373637262) to (16, fb46cdfee5d0d781));
every other baseline case and every other shipped case is bit-identical (C1, C2). The flipped workspace run: 1777 passed, 6 failed, 422 ignored; the failures
are the `packed_bed` hash, three tests that assert today's default or its defect (`component_diagonal`, W1a, `duct_diagnosis` A6), and the two lumped
heat-exchanger tests, which do NOT converge with `corrected` (p keeps changing by 3 to 26 percent per 500 iterations up to 10000 iterations, where `averaged`
is stationary to 4e-10 from iteration 1000): a real cost of the change whose cause was not investigated. The decision is the user's; the default stays
`averaged`.

### Deviations from the registered plan
1. A4 with `h_from_solution` off needed 220481 iterations; my first run used an unregistered 60000-iteration cap and stopped unconverged. A4 was split into
   A4 (on) and the heavy A4b (off); the stop criterion du < 1e-13 became du < 1e-12 in the test (the limits are on the errors, which are 1e-10 or below).
2. A10's prediction missed (kept as a known miss with the measured value).
3. A12 missed only the convergence criterion at the registered cap; A12b with the 250000 cap was added after seeing A12.
4. H4 and H6 were defective set-ups (the part A defect on the run-up symmetry plane); H4c and H6c were registered in amendment 1 after a disclosed debug print of
   one coarse H4 run. I stopped H6 before its cap. The H6c assertions were added after its single run.
5. Stage-1 coarse/medium values are combined with the stage-2 finest run of the same hypothesis made in a separate process (the runs are deterministic: H0
   reproduces ADR 0070 exactly).
6. Up to four runs were made in parallel; iteration counts are unaffected, wall times are not comparable with ADR 0070.
7. The registered trigger for a NEW corrected P2f test was a CONFIRMED cause; H6c is "consistent with the TMR codes" under the separate H6 rule, not a
   confirmed single cause, yet it was made the new P2f-type test (with the ADR 0070 P2a/P2f limits). This is a deviation from the registered wording.

### Unverified / not read
Menter (1994) and Menter, Kuntz, Langtry (2003) were not read; the TMR SST page does not define `Delta d_1` in words. The TMR plate grid, the boundary-condition
map and the statement that the coarser grids are every-other-point were read from the TMR pages and files; the grid images were not viewed. The remark on
OpenFOAM's `fvMatrix::H` in the `simple.rs` doc comment predates this wave and was not checked (no source was read). All other reference values are those
vendored in wave 33C.
