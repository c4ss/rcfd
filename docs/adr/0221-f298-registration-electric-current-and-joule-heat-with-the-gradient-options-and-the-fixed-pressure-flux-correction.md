# ADR 0221: F-298 registration: the electric current and the Joule heat with the gradient options, and the flux correction on fixed-pressure faces

Status: **REGISTERED 2026-11-07**, written before any code of F-298 and before any run of its checks. Results in addendum 1 (every
registered check met; measurement M: the default's missing boundary term costs a converged run 2.0e-3 of the inflow, so D3 was
built; F-298 done by the decision rule). Not yet accepted by the maintainer.

Parent: [ADR 0204](0204-f291-registration-gradient-options-on-turbulence-momentum-pressure-and-the-electric-potential.md) (F-291)
questions Q2 and Q3, where the lead took recommendation (b) for Q2 (the derived electric quantities take the potential's options,
as their own item, with a current balance) and (c) for Q3 (measure first what the default's mismatch does to the mass balance of a
converged run, then (b) only if it matters). Roadmap item F-298 (depends on F-291, merged). Closest precedent: [ADR
0202](0202-f289-registration-conjugate-heat-flow-report-with-the-gradient-options.md) (F-289: a derived flux that follows the
solved operator, checked by a balance; its unchanged formula must miss).

Read at main 50c4a83c. Number: the lead assigned 0221 (main holds 0218 as its highest; 0219 is on `f134-overset`, 0220 on
`f297-nsga2-crossover`). Branch `f298-joule-heat`.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), as ADR 0204 and ADR 0218 apply them):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded; it is diagnosed,
  not tuned away;
- a defective set-up gets a corrected test by an amendment written before the corrected run; the old test stays as an ignored record;
- defaults stay bit-identical: no case-schema key is added, no settings hash of an existing case changes, and
  `crates/rcfd-api/tests/baseline_hashes.rs`, `case_hashes_w48u.rs` and the goldens are not edited; no existing test file is edited;
- no new third-party crate; no commit trailers. Clean-room: first principles and our own ADRs only.

**Run before this text (disclosed).** No code of F-298 and no run of a check below. Two convergence probes of the existing code
(scratch tests in `crates/rcfd-api/tests/`, deleted, not committed; release build of main 50c4a83c) on the channel set-up CH below,
printing only the iteration count and the two normalised residuals, never a flux or a balance: with `simple` (`alpha_u = 0.7`,
`alpha_p = 0.3`) the default run and the runs with `pressure = true` and O1, O3, O4 or C2 reach `res_u` 2.2e-16 to 2.5e-16 and a
`res_p` floor of 1.9e-12 to 3.1e-12 by iteration 1000 and stay there to 20 000 (a tolerance of 1e-12 is never met); O2, O5 and O6
ran 5000 iterations without a failure (residuals not printed). With `simplec`, `alpha_u = 0.7` and
`simplec_non_orthogonal_compensation = true` the same five runs meet tolerances of 1e-12 after 150 to 152 iterations. These numbers
set the run lengths and tolerances of CH and CH-C and nothing else.

## What F-298 asks

The roadmap entry: every limit is fixed by its registration ADR before any code. (a) When `electric_potential` takes the gradient
options (F-291), the current density and the Joule heat use the same face flows, with a current balance check (the F-289 pattern;
the default must miss it, the option must meet it). (b) First measure what the 2.9e-2 mismatch of the default flux correction
beside skewed fixed-pressure faces (ADR 0204 Q3) does to the mass balance of a converged run; only if it matters, give the flux
correction the equation's boundary term under the options. Defaults bit-identical.

## What the code read shows (main 50c4a83c, read only)

1. **The derived electric quantities.** `ElectricState::update` (`rcfd-api/src/electric.rs`) solves the potential with
   `electric::solve_opt(geom, phi, sigma, ctl, 20, self.gradient)` (`self.gradient` is `sim::gradient_selected(case,
   "electric_potential")`, ADR 0204 K4), then sets `self.heat = electric::joule_heat(geom, phi, sigma_face, true)`; the energy
   equation receives `heat / (rho cp)` when `electric.joule_heating`. `boundary_current_density` (the boundary field
   `electric_current_density`) is `-F_b / A_b` with `F = electric::face_currents(geom, phi, sigma_face, true)`, which is
   `cht::conductive_face_flows`, the default operator's flows. `electric::boundary_power` is a library function; the tree does not
   call it. `lorentz.rs` uses `face_currents` for the potential of a Lorentz source (ADR 0204 Q5; not wired to the options).
2. **The solved operator.** The potential's sweep (`scalar::sweep_ext`, `LinearUpwind` on a zero flux) assembles
   `laplacian_opt(geom, sigma_f, bc, Some(grad), cells, bvals, opts)` with `grad = least_squares(cells, bvals)`. Its face fluxes
   are `cht::conductive_face_flows_opt(geom, phi, sigma_f, true, opts)` (ADR 0202 D1, checked there to 7e-15 of the flux scale on
   the skewed box K2). With `opts = Some(..)` the default flows of item 1 are not the flows of the solved potential.
3. **The Joule heat formula.** `joule_heat` gives each cell half of `sigma_f lap_f (phi_o - phi_n)^2` of each internal face (the
   orthogonal part of the face current times the potential difference) and `F_b (phi_P - phi_b)` of each boundary face. With
   `F_f = sigma_f lap_f d_f - sigma_f k_f . g_f` on internal faces (`d_f = phi_o - phi_n`, `k_f` the non-orthogonal remainder of
   the area vector, `g_f` the face gradient) and `net_c` the current leaving cell `c` through its faces, summation by parts gives
   `sum_c q_c V_c = P + sum_c phi_c net_c + sum_int sigma_f (k_f . g_f) d_f`, with `P = -sum_b F_b phi_b` the power the boundaries
   deliver. On an orthogonal mesh the last sum is zero and a converged potential (`net_c` at the solver's residual) gives
   `sum q V = P` (the module's statement). On a skewed mesh the last sum stays, with or without the options: the default Joule heat
   is not the dissipation of the default flows there.
4. **The flux correction on fixed-pressure faces** (ADR 0204 Q3). The steady kernel solves the pressure equation for `p` and sets
   `phi = corrected_flux*(p_new)`. With non-orthogonal correctors the equation's assembly (`laplacian` or `laplacian_with`) adds,
   on a boundary face whose pressure coefficient has `a = 0` (fixed pressure; fixed-velocity faces are Neumann, periodic faces have
   `a > 0`), the secondary flux `gamma_f k_f . g_b` (owner gradient, corrected, or none with `gradient_boundary = "dropped"`); the
   flux correction (`corrected_flux_impl`, also under F-291's options) takes the orthogonal part `gamma_f A_f snGrad(p)` only. At a
   converged fixed point (`p_new = p_old`, the corrector's gradient equal to the flux's) the divergence of the corrected flux in a
   cell beside such a face is the equation's residual plus that missing term, which does not shrink with the iterations. ADR 0204
   P-lib-B measured the operator gap on an arbitrary pressure field (2.94e-2 of the cell flux scale for the default), not on a
   flow. `corrected_flux_opt` is called by `SimpleSolver` (`gradient_pressure`) and by each corrector of `PimpleSolver`; the
   fractional step and VOF call `corrected_flux` (no options).

## Decisions

**D1. Library: the electric quantities with the options** (`rcfd-fv/src/electric.rs`). Three new public functions; the existing
`face_currents`, `joule_heat` and `boundary_power` are not changed.

* `face_currents_opt(geom, phi, sigma_face, non_orthogonal, opts)` = `conductive_face_flows_opt(geom, phi, sigma_face,
  non_orthogonal, opts)`: the face currents of the operator solved with `opts`.
* `joule_heat_opt(geom, phi, sigma_face, non_orthogonal, opts)`: `None` calls `joule_heat` (the same operations). `Some(o)`: with
  `F = face_currents_opt(.., Some(o))`, each internal face gives half of `F_f (phi_o - phi_n)` to each of its two cells and each
  boundary face gives `F_b (phi_P - phi_b)` to its owner, divided by the cell volume. By the summation of item 3 this gives
  `sum_c q_c V_c = P + sum_c phi_c net_c` exactly (no secondary remainder), so the Joule heat equals the boundary power up to the
  potential's residual. The formula is chosen by `Some`: `Some(GradientOptions::default())` gives this formula with the default
  operator's flows, not the bits of `joule_heat` (the tree never passes it: ADR 0204 K2 and R5). Without a gradient
  (`non_orthogonal = false`) the flows have no secondary part and the formula is `joule_heat`'s up to round-off.
* `boundary_power_opt(geom, phi, sigma_face, non_orthogonal, opts)` = `-sum_b F_b phi_b` with `F = face_currents_opt(..)`; `None`
  calls `boundary_power`.
* Disclosed: the face power `F_f (phi_o - phi_n)` is not a square, so under the options a cell's Joule heat can be negative on a
  strongly skewed mesh while the total equals the power (Q3). The runs report the smallest cell value.

**D2. Case wiring** (`rcfd-api/src/electric.rs`). `ElectricState::update` sets `self.heat = joule_heat_opt(geom, phi, sigma_face,
true, self.gradient)`, and `boundary_current_density` uses `face_currents_opt(geom, phi, sigma_face, true, self.gradient)`.
Without the selector `self.gradient` is `None` and both calls run the unchanged functions. With it, the Joule source of the energy
equation changes with the heat (so does the temperature). No key, value, rule or message changes.

**D3. The flux correction's boundary term under the options** (`rcfd-fv/src/pressure.rs`, `rcfd-fv/src/diffusion.rs`), **built
only if measurement M below matters** (its decision rule is fixed here). `corrected_flux_opt` with options and a gradient adds,
on every non-empty boundary face that is not a fixed-velocity face, not periodic, and whose pressure coefficient has `a = 0`, the
term the paired equation has: `phi_f -= gamma_f k_f . g_b` with `g_b` the owner cell's gradient as the equation uses it (limited
when `secondary_limiter` is set), corrected (`(p_b - p_P)/|d|` along the cell-to-face vector) for `Corrected`, and no term for
`Dropped`. A new public helper `diffusion::secondary_boundary_gradient(geom, f, grad, cells, bvals, boundary) -> Option<Vec3>`
states the rule of `laplacian_with` for boundary faces; `laplacian_impl` is not changed. `corrected_flux`, `corrected_flux_with`
and the default path of both kernels are not changed, so the default keeps the mismatch (Q2). Both kernels take it through their
existing `corrected_flux_opt` calls. If M does not matter, D3 is not built and the boundary faces stay as ADR 0204 left them,
documented with M's numbers.

**D4. Not changed.** Every default path; `face_currents`, `joule_heat`, `boundary_power`, `corrected_flux`,
`corrected_flux_with`, `conductive_face_flows*`; the Lorentz potential (ADR 0204 Q5); the SIMPLEC compensations (ADR 0050 on
internal faces, ADR 0102 on boundary faces; Q4); the pressure-lag estimate; the fractional step, VOF, the coupled solver. No new
key, enumeration value or refusal (the selector's rules of ADR 0204 K5 cover every case F-298 touches; D2 adds no path).

**D5. Separable commits.** (1) this text; (2) the measurement test of (b) (no library change) with its run; (3) `rcfd-fv` (D1,
and D3 if M matters) with the library tests; (4) `rcfd-api` (D2) with the tree tests; (5) documents and roadmap; (6) the addendum.

## Set-ups

**Common.** Mesh W of ADR 0190: `wedge_2d`, `nx = ny = 16`, `lx = ly = 1`, `ramp_start = 0`, `ramp_angle = 30` (patches `xmin`,
`xmax`, `ymin` the ramp, `ymax`). Option sets as ADR 0190: O1 `gradient_face_correction = true`; O2 `gradient_boundary =
"corrected"`; O3 `gradient_boundary = "dropped"`; O4 `gradient_secondary_limiter = "barth_jespersen"`; O5
`gradient_secondary_limiter = "venkatakrishnan"` (K 5); O6 the same with `gradient_limiter_k = 0.5`; C2 = O1 + O2 + O5. "The
default" is a case without the F-256 keys and without `[solver.gradient_equations]`. Case texts are inline in the test files; no
case file, fixture or docs TOML block is added.

**Library set-up KE** (`crates/rcfd-fv/tests/electric_opt_f298.rs`): ADR 0016's skewed mesh K at `n = 8` (`box_mesh([8; 3],
[1; 3])` warped by `x' = x + 0.7 y + 0.3 z`, `y' = y + 0.3 z`); cell conductivity `sigma_c = 1 + 0.5 sin(1.7 x + 0.3) cos(0.8 y -
0.2 z)` at the centres; `sigma_f = electric::face_conductivity(geom, sigma)`; the potential with `FixedValue` per face from
`f(p) = x + sin(1.3 x + 0.4) cos(1.1 y) cos(0.9 z + 0.2)` at the face centres on `xmin` and `xmax`, a uniform `FixedGradient` of
0.3 on `ymin` (a current patch), `ZeroGradient` elsewhere (insulated); cell values `f(centre) + 0.05 (-1)^c` (local extrema, so
both limiters act); `grad = least_squares(geom, phi, phi_b)`; `(A, b) = laplacian_opt(geom, sigma_f, bc, Some(grad), phi, phi_b,
opts)`. For a set of flows `F`: `net_c` the current leaving cell `c`; `S = max_c sum_{f in c} |F_f|`; `W = sum_int |F_f| (|phi_o| +
|phi_n|) + sum_b |F_b| (|phi_P| + |phi_b|)` (the round-off scale of the power sums).

**Library set-up K-B** (`crates/rcfd-fv/tests/flux_boundary_f298.rs`, only with D3): ADR 0204's K-B, as its test builds it: mesh K
at `n = 8`, zero `FixedValue` velocity on every patch except `xmax` (velocity `ZeroGradient`, pressure `FixedValue` 0), cell
pressures `cos(1.3 x + 0.2) sin(0.9 y + 0.4) cos(1.1 z) + 0.05 (-1)^c`, face diffusivity `1 + 0.3 sin(2.1 x + 0.7 y - 0.4 z)`;
`(A, b) = laplacian_opt(geom, gamma, pbc.coeffs, Some(grad), p, bv, opts)` and `phi = corrected_flux_opt(geom, gamma, pbc, 0, p, 0,
Some(grad), opts)` (or `corrected_flux_with` where named); the per-cell miss `|div(phi)_c - ((A p)_c - b_c)|` against `S = max_c
sum_{f in c} |phi_f|`, split into the cells with an `xmax` face and the others.

**Tree set-up E2** (`crates/rcfd-api/tests/electric_opt_f298.rs`): ADR 0204's E1 with `max_iterations = 200`: mesh W, `physics`
`rho = 1`, `cp = 1`, `conductivity = 0.1`; `energy.enabled`, `relax = 0.9`, `initial.temperature = 0.5`; every patch `velocity =
"fixed"`; `xmin` `temperature = "fixed"`, `temperature_value = 1`, `electric = "potential"`, `electric_potential = 1`; `ymin` (the
ramp) `temperature_value = 0`, `electric_potential = 0`; `xmax`, `ymax` insulated with zero-gradient temperature; `[electric]`
`enabled = true`, `conductivity = 1` (Joule heating on, its default); `solver`: `simplec`, `convection = "linear_upwind"`,
`frozen_flow = true`, `tolerance_u = tolerance_p = 1e-14`. A set's run adds the set's keys and `[solver.gradient_equations]`
`electric_potential = true`. Run with `Simulation::run(10000)`.

Quantities of a finished run of E2 (`rcfd_fv` calls only, on `Simulation::geom`): the library field (`ScalarField` with
`FixedValue` 1 on `xmin`, 0 on `ymin`, `ZeroGradient` elsewhere, and the tree's final `electric_potential` as values); `sigma_f =
face_conductivity(geom, 1 in every cell)`; `F_new = face_currents_opt(.., true, opts)` (`opts` the set's, `None` for the default)
and `F_old = face_currents(.., true)`; the residual `r_c = b_c - (A phi)_c` of the solved equation at the final field, `(A, b)` as
in KE with `sigma = 1`; `tol_A = sum_c |r_c|` (A) and its part `tol_A(g)` over the cells of group `g`; the groups `g = 0` for
cells whose centre has `x < 0.5`, `g = 1` for the others; `F = sum over non-empty faces |F_new,f|`; `I_in = sum_b max(-F_new,b,
0)` (the current into the domain); `R = sum_c |phi_c| |r_c|` (W); `W` as in KE with `F_new`; from the tree: `j_b` (boundary field
`electric_current_density`), `q_c` (field `joule_heat`), and the tree's boundary currents `F_tree,b = -j_b A_b`.

**Tree set-up CH** (`crates/rcfd-api/tests/flux_boundary_f298.rs`): ADR 0204's channel on W (`xmin` `velocity = "fixed"`,
`velocity_value = [1, 0, 0]`; `xmax` `velocity = "zero_gradient"`, `pressure = "fixed"`, `pressure_value = 0`; `ymin`, `ymax`
fixed zero velocity; `physics.nu = 0.1`; `convection = "linear_upwind"`, `non_orthogonal_correctors = 1`) with `algorithm =
"simple"`, `alpha_u = 0.7`, `alpha_p = 0.3`, `max_iterations = 3000`, `tolerance_u = tolerance_p = 1e-14` (never met: the run
ends at its cap, past the residual floor of the probe). Run step by step (`Simulation::step`) to its end. **CH-C**
(supplementary): CH with `algorithm = "simplec"`, `alpha_u = 0.7`, `simplec_non_orthogonal_compensation = true`, `tolerance_u =
tolerance_p = 1e-12` (the probe's convergence in about 150 iterations). A set's run adds the set's keys and
`[solver.gradient_equations]` `pressure = true`.

Quantities of a finished run of CH or CH-C: `phi = Simulation::flux()`; `Q_in = -sum over xmin faces of phi_f` (the inflow);
`div_c = pressure::divergence(geom, phi)`; `B` the cells with an `xmax` face; `m_dom = |sum over non-empty boundary faces of
phi_f| / Q_in` (the domain mass balance); `e_b = max_{c in B} |div_c| / Q_in`; `e_i = max_{c not in B} |div_c| / Q_in`; the last
`res_u`, `res_p`; a 64-bit FNV-1a hash of the bits of `phi`, `ux`, `uy` and `p` (recorded, to compare runs of two commits).

## Registered checks

### (a) The electric quantities

| Id | Check | Limit |
| --- | --- | --- |
| P | `check_mesh` maximum non-orthogonality: K; W through the tree | K at least 25, W at least 20 degrees (set-up; a miss stops the test) |
| L1 | KE, the default and each set: per cell `|net_c - ((A phi)_c - b_c)|` with `F = face_currents_opt(.., true, opts)` | max `<= 1e-12 S` |
| L1-T | KE, each set: the same with `F = face_currents(.., true)` (no options) against the operator with the set | max `> 1e-9 S` |
| L2 | KE, each set: `|sum_c q_c V_c - P - sum_c phi_c net_c|`, `q = joule_heat_opt(.., true, Some(o))`, `P = boundary_power_opt(.., true, Some(o))`, `net` of `face_currents_opt` | `<= 1e-12 W` |
| L2-T | KE, the default (`joule_heat`, `boundary_power`, `face_currents`, all with a gradient): the same quantity | `> 1e-9 W` (the default formula is not the dissipation of its flows on a skewed mesh) |
| L3 | KE: `face_currents_opt` and `boundary_power_opt` with `None` and with `Some(GradientOptions::default())`, `joule_heat_opt` with `None`, against `face_currents`, `boundary_power`, `joule_heat`, each with `non_orthogonal` true and false; control: `face_currents_opt` with C2 and a gradient moves a face current | equal bits on every face and cell; control `> 0` |
| L4 | KE: `joule_heat_opt(.., false, Some(C2))` against `joule_heat(.., false)` (no gradient) | max cell difference `<= 1e-12 max_c |q_c|` |
| F | E2, the default and each set: the run ends without a failure; `electric_potential`, `joule_heat`, `T` finite | set-up for the default (a miss stops the test); for a set, a miss makes every check of that set NOT MET |
| E-tree | E2, the default and each set: L1's per-cell identity on the library field of the run (with `F_new`) | max `<= 1e-12 S` |
| R-eq-J | E2: the tree's boundary currents `F_tree` against `F_new` on the boundary faces | each set: max `<= 1e-12 max_b |F_new,b|`; the default: `j_b` equal bits with `-F_old,b / A_b` |
| R-eq-Q | E2: the tree's `q` against `joule_heat_opt(.., true, opts)` on the library field | each set: max `<= 1e-12 max_c |q_c|`; the default: equal bits with `joule_heat(.., true)` |
| B1 | E2, the default and each set: the domain current balance `|sum_b F_tree,b|` | `<= tol_A + 1e-12 F` |
| B2 | E2, the default and each set: the solver tolerance | `tol_A <= 1e-8 I_in` |
| B3 | E2, the default and each set: per group, `|G_g|` with `G = cht::group_net_outflow(geom, F_new, groups, 2)` | `<= tol_A(g) + 1e-12 F` for `g = 0, 1` |
| J1 | E2, each set: the power balance `|sum_c q_c V_c - P|`, `q` the tree's, `P = boundary_power_opt(.., true, opts)` | `<= R + 1e-12 W` |
| T-B1 | E2, each of O2 to O6 and C2: B1 with `F_old` on the set's run | `> tol_A + 1e-12 F` (the unchanged current misses) |
| T-O1 | E2, O1: the boundary values of `F_new` and `F_old` (the face correction acts on internal faces only) | equal bits |
| T-B3 | E2, each of O1 to O6 and C2: B3 with `F_old` | a miss for at least one group |
| T-J1 | E2, each set: J1 with `joule_heat(.., true)` and `boundary_power(.., true)` (the unchanged functions) on the set's run | `> R + 1e-12 W` |
| T-J1-D | E2, the default run: J1 with its own (unchanged) `q` and `boundary_power(.., true)` | `> R + 1e-12 W` (the default misses; Q1) |
| D1 | E2 without keys and E2 with the four F-256 keys and the table with its four selectors written at their defaults | same settings content hash; equal bits of `electric_potential`, `joule_heat`, `T` and `electric_current_density` |

B1 is the acceptance's current balance: the current through the boundary adds up to zero within what the solver left in its residual,
plus round-off; B2 keeps it from being met by an unconverged run; B3 is the same per group of cells, so it sees the internal faces
(the face correction), which no boundary sum can (T-O1). J1 is the Joule heat's balance against the electric power. The T rows are
"the default must miss it": the unchanged quantities on the potential solved with the options, and the default's own Joule heat.
The smallest cell value of `q` of each run is reported (D1, Q3).

### (b) The flux correction on fixed-pressure faces

**Measurement M** (commit (2), on F-291's code, before any library change of F-298):

| Id | Check | Limit |
| --- | --- | --- |
| S-CH | CH, the default: no failure; the last `res_u <= 1e-12` and `res_p <= 1e-10`; `e_i <= 1e-8` | set-up (a miss stops the measurement; an amendment corrects it) |
| M | CH, the default: `m_dom`, `e_b` (and `e_i`, the iterations, the last residuals, the hash) | reported; **decision rule:** the mismatch **matters** if `m_dom > 1e-6` or `e_b > 1e-6` (one part per million of the inflow, 100 times the converged level S-CH requires) |
| M-O3 | CH with `pressure = true` and O3 (the equation without the boundary term, so equation and flux agree there): `m_dom`, `e_b` | each `<= 1e-8` with the convergence of S-CH (the control that attributes M to the missing term) |
| M-sets | CH with `pressure = true` and each of O1, O2, O4, O5, O6, C2: the quantities of M | reported (the options without D3 keep the mismatch) |
| M-C | CH-C, the default: the quantities of M | reported (supplementary: the same measurement with SIMPLEC and its compensation) |

**Checks of D3** (binding only if M matters; otherwise not run, and (b) is met by M):

| Id | Check | Limit |
| --- | --- | --- |
| B-lib | K-B, each set: the per-cell miss on the cells with an `xmax` face and on the others | both `<= 1e-12 S` |
| B-lib-T | K-B, each of O1, O2, O4, O5, O6, C2: the miss on the cells with an `xmax` face of `corrected_flux_with` (no options, no boundary term) against the operator with the set | `> 1e-9 S` |
| B-lib-D | K-B: `corrected_flux_opt` with `None` against `corrected_flux_with`; with `Some(O3)` its boundary faces against those of `corrected_flux_with`; with `Some(C2)` and no gradient against `corrected_flux_with(.., None)` | equal bits |
| B-tree | CH with `pressure = true` and each of O1, O2, O4, O5, O6, C2, after D3: the convergence of S-CH, then `m_dom` and `e_b` | each `<= 1e-8`; a run that does not meet the convergence: NOT MET for that set |
| B-tree-T | M-sets: O1, O2 and C2 | `e_b > 1e-6` each (the fix removes a miss that was there) |
| B-D | CH after D3: the hash of the default run and of the O3 run | equal to the hashes M recorded |

### Common

| Id | Check | Limit |
| --- | --- | --- |
| D2 | `rcfd-api`: `baseline_hashes.rs`, `case_hashes_w48u.rs`, `electric_case.rs`, `mhd.rs`, `phase_change_case.rs`, `gradient_equations_f291.rs`, `gradient_options_f256.rs`, `cht_flows_f289.rs`, `rules_w19c.rs`; `rcfd-fv`: `electric.rs`, `gradient_pressure_f291.rs`, `cht_flows_f289.rs`, `gradient_sweep_f256.rs` | pass unedited (with D3, ADR 0204's P-lib-B prints smaller numbers for the sets; it bounds only the other cells) |
| N | the gate | passes, known flaky tests rerun alone and named |

**Can fail.** Mutations, each made in the working tree, its test run once in release in a separate target dir, the file restored
and compared byte for byte with the kept copy:

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `ElectricState::update` calls `joule_heat` in place of `joule_heat_opt` | R-eq-Q and J1 of O1 |
| MU2 | `boundary_current_density` calls `face_currents` in place of `face_currents_opt` | R-eq-J and B1 of O2 |
| MU3 | `joule_heat_opt` gives internal faces half of `sigma_f lap_f d^2` in place of half of `F_f d` | L2 of O1 |
| MU4 | (with D3) the boundary term of `corrected_flux_opt` skipped | B-lib of O1; B-tree of O1 |
| MU5 | (with D3) the `Corrected` arm of the boundary term takes the owner gradient | B-lib of O2 |

`scripts/limit_audit.py` is run on the new test files.

**Gate.** fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release on a `git archive` copy of the
gated commit (`scripts/gate_test.py`, one process per test, three jobs), serialised by the machine's gate lock, after free physical
memory reaches 19 GB. A failure of a known flaky test is rerun alone and reported with its name.

**Decision rule.** Every check of (a) met, and (b) met (M does not matter; or it matters and every check of D3 is met): F-298 is
done. A miss: an ignored `*_known_miss` test with the limit intact and the number recorded, and F-298 is partial with the miss
named.

## Not in F-298

The default's Joule heat formula (Q1) and the default flux correction (Q2) (both need the bit-identical rule lifted); the Lorentz
potential (ADR 0204 Q5); the SIMPLEC compensations on boundary faces (Q4); the pressure-lag estimate; other solvers; a claim that
the options improve accuracy.

## Open questions and choices for the maintainer

**Q1. The default Joule heat on a skewed mesh.** Item 3: `joule_heat` drops the secondary part of the internal face power, so the
default Joule heat does not equal the boundary power on a non-orthogonal mesh (T-J1-D measures it). (a) Leave the default and
document the gap; (b) use the D1 formula by default (lifts the bit-identical rule: every skewed electric case changes its heat and
temperature); (c) an opt-in key for the D1 formula with the default operator. Recommendation (a), and (c) only if a case needs the
balance without the options. Proceeding on (a).

**Q2. The default flux correction on fixed-pressure faces.** D3 acts under the options only. (a) Leave the default and document M's
numbers; (b) give the default the term too, as its own item with the rule lifted (it changes every converged result of a skewed
fixed-pressure case with correctors). Recommendation: (b) as its own item if M matters, since the gap is a consistency defect of the
default and not a modelling choice; (a) otherwise. Proceeding on (a).

**Q3. A negative cell Joule heat under the options** (D1). (a) Accept and document (the total is the power); (b) clip at zero (breaks
J1). Recommendation (a). Proceeding on (a).

**Q4. The SIMPLEC compensations at fixed-pressure faces under the options.** ADR 0102's boundary compensation has an orthogonal part
only. (a) Leave; (b) an item. Recommendation (a) until a case needs it. Proceeding on (a).

## Where to go next

* [ADR 0204](0204-f291-registration-gradient-options-on-turbulence-momentum-pressure-and-the-electric-potential.md) for the
  selectors and the wiring.
* [ADR 0202](0202-f289-registration-conjugate-heat-flow-report-with-the-gradient-options.md) for the face flows with the options.
* [ADR 0035](0035-wave21a-thermal-extras.md) for the electric model.
* [Case schema](../case-schema.md), section "Further equations (`[solver.gradient_equations]`)".

## Amendment 1 (2026-11-07, written before any run of a check): where B-tree-T and B-D are evaluated

No limit changes. B-tree-T is a property of F-291's code, which D3 replaces, so it is asserted by the test of M-sets at commit (2)
(`m_sets_before_d3`); if D3 is built, that test is kept as an ignored record of commit (2) (its numbers in the addendum), because the
code it measures no longer exists. B-D compares the hashes printed by M and M-O3 at commit (2) with those of the same runs after D3:
the test of D3 asserts the recorded values, written into it after M ran.

## Amendment 2 (2026-11-07, written after the first runs of the D2 suites and before any corrected run): `Some(GradientOptions::default())` is `None`

No limit changes. Run before this text, on the code of D1 to D3 as registered (commit e6635637 and the D2 wiring, uncommitted): the
library tests (P, L1 to L4, B-lib, B-lib-T, B-lib-D), the tree tests (E2: F, E-tree, R-eq-J, R-eq-Q, B1 to B3, J1, the T rows and
D1; CH: S-CH, M, M-O3, M-C, B-tree, B-D) and part of D2: `rcfd-fv` `electric.rs`, `gradient_pressure_f291.rs`, `cht_flows_f289.rs`,
`gradient_sweep_f256.rs` (all passed); `rcfd-api` `baseline_hashes.rs`, `case_hashes_w48u.rs`, `cht_flows_f289.rs`,
`electric_case.rs` (passed) and `gradient_equations_f291.rs`, where one test failed (cargo stopped there, so `mhd.rs`,
`phase_change_case.rs`, `gradient_options_f256.rs` and `rules_w19c.rs` did not run).

**Finding: D3 as registered breaks ADR 0204's G4 (iv).** `g4_iv_library_default_options_equal_no_options` asserts that every
library run with `Some(GradientOptions::default())` equals the run with `None` bit for bit; for P2 (the channel with a fixed-pressure
outlet) 768 of 768 values now differ, because D3 adds the owner-gradient boundary term whenever options are given, the default
options included. D2 requires that test to pass unedited, and F-291 made "the default options reproduce the default operator, bit
for bit" a library-wide rule. A defect of D3's text, corrected as follows:

* **D3 (corrected):** `corrected_flux_opt` adds the boundary term only when the options differ from `GradientOptions::default()`;
  with `Some(GradientOptions::default())` it is `corrected_flux_with`, bit for bit, as before F-298. The tree never passes the default
  options (ADR 0204 K2, R5), so no tree run changes against the registered code.
* **D1 (made uniform, not forced by a failure):** `joule_heat_opt` and `boundary_power_opt` with `Some(GradientOptions::default())` are
  `joule_heat` and `boundary_power`, bit for bit, so that one rule holds for every function F-298 adds. No registered check used
  `joule_heat_opt` with the default options (L3 compared `None` only).
* **New checks** (with the limits of their kind): **B-lib-D2**, K-B: `corrected_flux_opt` with `Some(GradientOptions::default())`
  and a gradient against `corrected_flux_with`, equal bits on every face (the fixed-pressure faces included); **L3b**, KE:
  `joule_heat_opt` with `Some(GradientOptions::default())` against `joule_heat`, gradient true and false, equal bits on every cell.
* The other checks are run again on the corrected code, D2 in full.

## Addendum 1: what was built and the results (2026-11-07)

Branch `f298-joule-heat` on main 50c4a83c. Commits: the registration `c6ce7039` and amendment 1 `de4c00a8`; the measurement M
`3834e1af` (a test only; M, M-O3, M-C and M-sets ran there, on F-291's code); D1 and D3 in `rcfd-fv` with the library tests
`e6635637`; amendment 2 `3b830157`; its library change with L3b and B-lib-D2 `3d735d6c`; D2 in `rcfd-api` with the tree tests
`bdcd0f4b`; the documents and this addendum after them. Release builds on Windows. Every number below is from the code of
`bdcd0f4b` unless it says commit (2); the tree runs of E2 and CH printed the same numbers before and after amendment 2 (the tree
never passes the default options).

**Built.** `rcfd-fv/src/electric.rs`: `face_currents_opt`, `joule_heat_opt`, `boundary_power_opt` (D1; `None` and the default
options call the unchanged functions, amendment 2), and a private `power_of_flows` that `boundary_power` now calls with the same
operations. `rcfd-fv/src/diffusion.rs`: `secondary_boundary_gradient` (the boundary rule of `laplacian_with`; `laplacian_impl`
unchanged). `rcfd-fv/src/pressure.rs`: `corrected_flux_opt` with non-default options gives every fixed-pressure face (`a = 0`, not a
fixed-velocity or periodic face) the equation's non-orthogonal part (D3, built because M matters); `corrected_flux_impl` takes the
boundary treatment as an argument, `None` from `corrected_flux_with` (no operation added on the default path).
`rcfd-api/src/electric.rs`: the two calls of D2. No key, rule, message, golden, `baseline_hashes.rs`, `case_hashes_w48u.rs`, Studio
file or existing test file changed. New tests: `crates/rcfd-fv/tests/electric_opt_f298.rs` (2), `crates/rcfd-fv/tests/flux_boundary_f298.rs`
(2), `crates/rcfd-api/tests/electric_opt_f298.rs` (2), `crates/rcfd-api/tests/flux_boundary_f298.rs` (4, and the ignored record
`m_sets_before_d3` of amendment 1). Documents: the section "Further equations" of `docs/case-schema.md`, the F-298 entries of
`docs/ROADMAP.md` and `docs/roadmap.json`.

### Results of (a)

| Check | Measured | Limit | Verdict |
| --- | --- | --- | --- |
| P | K 38.309 degrees; W 29.641 degrees | at least 25; at least 20 | met |
| L1 | default 5.8e-15; O1 to O6, C2 5.2e-15 to 6.2e-15 of `S` (`S` 0.117) | `<= 1e-12 S` | met |
| L1-T | O1 1.74e-1, O2 8.07e-2, O3 1.39e-1, O4 8.82e-2, O5 1.09e-2, O6 8.38e-2, C2 1.85e-1 of `S` | `> 1e-9 S` | met |
| L2 | 1.9e-17 to 1.2e-16 of `W` (`W` 68 to 75; `P` 1.09 to 1.41 W) | `<= 1e-12 W` | met |
| L2-T | the default formula with its own currents: 3.71e-1, that is 5.0e-3 of `W` | `> 1e-9 W` | met |
| L3, L3b | 0 of 1728 faces and 0 of 512 cells differ in a bit (currents and power with `None` and the default options, the Joule heat with `None` and, L3b, the default options; gradient true and false); control: C2 moves a face current by 9.7e-3 | equal bits | met |
| L4 | 0 (the two formulas give the same bits here) | `<= 1e-12 max q` | met |
| F | every E2 run ended after its 200 iterations without a failure, finite | set-up | met |
| E-tree | 2.2e-16 to 4.5e-16 of `S` over the eight runs | `<= 1e-12 S` | met |
| R-eq-J | default: 0 faces differ in a bit from the unchanged current density; sets 0 to 1.4e-16 of the largest boundary current | bits; `<= 1e-12` | met |
| R-eq-Q | default: 0 cells differ in a bit from `joule_heat`; sets 0 (the same bits) | bits; `<= 1e-12` | met |
| B1 | `abs(sum F_b)` 1.7e-17 to 2.2e-16 A against bounds of 2.18e-12 to 2.25e-12 A (`tol_A` 2.8e-15 to 3.3e-15 A) | `<= tol_A + 1e-12 F` | met |
| B2 | `tol_A / I_in` 1.07e-14 to 1.28e-14 (`I_in` 0.239 to 0.267 A) | `<= 1e-8` | met |
| B3 | both groups 2.6e-18 to 2.6e-16 A against bounds of about 2.2e-12 A | `<= tol_A(g) + 1e-12 F` | met |
| J1 | `abs(sum qV - P)` 0 to 5.0e-16 W against bounds of 1.89e-12 to 1.96e-12 W (`P` 0.2390 to 0.2666 W) | `<= R + 1e-12 W` | met |
| T-B1 | the unchanged currents: O2 4.35e-4, O3 1.95e-3, O4 4.71e-7, O5 6.36e-5, O6 2.48e-4, C2 6.96e-4 A | `> tol_A + 1e-12 F` | met |
| T-O1 | 0 boundary currents differ in a bit | equal bits | met |
| T-B3 | the unchanged currents, (group 0, group 1) in A: O1 (1.48e-5, 1.48e-5), O2 (1.85e-4, 2.50e-4), O3 (1.67e-3, 2.83e-4), O4 (1.0e-16, 4.71e-7: group 1 misses), O5 (1.43e-4, 7.95e-5), O6 (7.88e-4, 5.41e-4), C2 (3.72e-4, 3.24e-4) | a miss in at least one group | met |
| T-J1 | the unchanged Joule heat and power on the sets' runs: 3.00e-2 to 3.64e-2 W | `> R + 1e-12 W` | met |
| T-J1-D | the default run's own Joule heat against its power: 3.146e-2 W, 11.9 % of `P = 0.2648` W | `> R + 1e-12 W` | met |
| D1 | settings hash `664afb0ba53bf26f` with and without the keys and the table at their defaults; 200 iterations both; 0 values differ in a bit | equal | met |

The smallest cell Joule heat of the E2 runs is 9.0e-3 to 9.4e-3 W/m^3 (none negative; on KE 1.15 to 1.46).

### Results of (b)

| Check | Measured | Limit | Verdict |
| --- | --- | --- | --- |
| S-CH | default: 3000 iterations, no failure, `res_u` 2.40e-16, `res_p` 2.15e-12, `e_i` 3.2e-14 | `1e-12`, `1e-10`, `1e-8` | held |
| M | default: `m_dom` **2.003e-3**, `e_b` **6.844e-4** of the inflow (`Q_in` 0.0625); hash `4a8d6c1e2e8a48f2` | matters above 1e-6 | **matters**: D3 built |
| M-O3 | O3: `m_dom` 2.0e-13, `e_b` 2.4e-14 (converged: `res_p` 1.92e-12, `e_i` 2.0e-14); hash `85ca4cac6091df27` | `<= 1e-8` | met |
| M-sets (commit (2)) | `m_dom`, `e_b`: O1 2.10e-3, 7.06e-4; O2 4.25e-3, 1.17e-3; O4 8.62e-4, 3.50e-4; O5 7.62e-4, 3.08e-4; O6 7.58e-4, 3.07e-4; C2 4.67e-3, 1.09e-3; all converged | reported | |
| M-C | CH-C default: 152 iterations to 1e-12, `m_dom` 4.96e-3, `e_b` 2.22e-3, `e_i` 1.8e-14 | reported | |
| B-lib | cells with an `xmax` face 4.8e-16 to 7.3e-16 of `S`, the others 3.1e-16 to 4.0e-16 (`S` 0.228 to 0.303) | `<= 1e-12 S` | met |
| B-lib-T | O1 6.58e-2, O2 1.14e-1, O4 9.80e-2, O5 4.06e-2, O6 9.84e-2, C2 2.57e-1 of `S` | `> 1e-9 S` | met |
| B-lib-D, B-lib-D2 | `None` 0 of 1728 faces; O3's boundary faces 0 of 384; C2 without a gradient 0 of 1728; the default options with a gradient 0 of 1728 | equal bits | met |
| B-tree | after D3, all converged (`res_p` 2.3e-12 to 3.1e-12): `m_dom` O1 1.06e-12, O2 1.26e-12, O4 6.7e-14, O5 6.2e-13, O6 8.9e-13, C2 4.8e-13; `e_b` 1.5e-14 to 2.7e-14 | `<= 1e-8` | met |
| B-tree-T | commit (2): `e_b` O1 7.06e-4, O2 1.17e-3, C2 1.09e-3 | `> 1e-6` | met |
| B-D | default `4a8d6c1e2e8a48f2`, O3 `85ca4cac6091df27`: the hashes of M and M-O3 | equal | met |

So the default flux correction's missing boundary term leaves a converged SIMPLE run with a mass imbalance of 2.0e-3 of the inflow on
this 30-degree outlet (5.0e-3 with SIMPLEC and its compensation), which does not converge away (the interior cells are at 3e-14),
and the run with `gradient_boundary = "dropped"` on the pressure closes it to 2e-13, so the missing term is its cause. Under the
options D3 closes it for every set.

### Common checks

| Check | Measured | Verdict |
| --- | --- | --- |
| D2 | `rcfd-api`: `baseline_hashes.rs` (3 passed, 1 ignored), `case_hashes_w48u.rs` (1), `electric_case.rs` (4), `mhd.rs` (5), `phase_change_case.rs` (3), `gradient_equations_f291.rs` (13, 2 ignored; G4 (iv) P2 0 of 768 values after amendment 2), `gradient_options_f256.rs` (5), `cht_flows_f289.rs` (3), `rules_w19c.rs` (7, 1 ignored); `rcfd-fv`: `electric.rs` (7), `gradient_pressure_f291.rs` (3), `cht_flows_f289.rs` (2), `gradient_sweep_f256.rs` (1): all pass unedited | met |
| N | the gate section below | |

With D3 the F-291 pressure cases with options carry the boundary term, so ADR 0204's printed numbers of those cases move (its limits
hold): G3 P1 O2 2.83e-3 (was 1.44e-3), P2 C2 1.49e-2 (was 9.45e-3), P3 C2 3.66e-1 (was 3.99e-1), P2-all O2 1.64e-2 (was 8.48e-3);
its P-lib-B now prints 4.8e-16 to 7.3e-16 for the sets (the default 2.94e-2 as before); every G2 stays 0.

`scripts/limit_audit.py` (workspace scan) reports one hit in the new files: `crates/rcfd-api/tests/flux_boundary_f298.rs`, class
(iii), `m_o3_control_dropped_boundary_term_closes_the_balance`: a false positive, a positive control that asserts success (a closed
balance), like `duct_diagnosis.rs`'s `a_control_...` in docs/test-process.md.

### Mutations

Applied one at a time to the code of `bdcd0f4b` by `w49i-work/f298-scratch/mutations.sh` (an exact text replacement that checks
that the text occurs once, `f298-scratch/mutate.py`), the named test run once in release in `target-f298-mut`, the file restored and
compared byte for byte with the kept copy (equal after each of the five).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 `update` with `joule_heat` | R-eq-Q and J1 of O1 | failed: R-eq-Q O1 4.66e-2 of the largest cell heat, J1 O1 3.29e-2 W against 1.96e-12 W (and every other set) |
| MU2 `boundary_current_density` with `face_currents` | R-eq-J and B1 of O2 | failed: R-eq-J O2 7.10e-2, B1 O2 4.35e-4 A against 2.22e-12 A (and O3 to C2; O1 passed, as T-O1 predicts) |
| MU3 internal faces of `joule_heat_opt` with `sigma_f lap_f d^2` | L2 of O1 | failed: L2 O1 9.49e-3 of `W` (every set 3.9e-3 to 1.0e-2) |
| MU4 the boundary term of `corrected_flux_opt` skipped | B-lib O1; B-tree O1 | failed: B-lib O1 3.17e-2 of `S`; B-tree O1 `m_dom` 2.10e-3, `e_b` 7.06e-4, the hash `148d28b7157a338c` of M-sets O1 at commit (2) (the mutated code is F-291's flux correction) |
| MU5 `Corrected` with the owner gradient | B-lib of O2 | failed: B-lib O2 1.21e-1 of `S` (and C2 1.85e-1) |

### Disclosures

- Before the registration only the two convergence probes named there were run; between the registration and M only the build.
- M ran once at commit (2) and decided D3. After amendment 2 every check was run again; the tree numbers did not change.
- After the mutations, clippy (`incompatible_msrv`) refused `Option::is_none_or` in `joule_heat_opt` and `boundary_power_opt` (stable
  since Rust 1.82, `rcfd-fv` declares 1.80); it was replaced by `opts.unwrap_or_default() == GradientOptions::default()`, the same
  truth table. L3b, E2 and ADR 0204's G4 (iv) were run again on it and printed the same numbers.
- The B-lib-D test also asserts a control that the registration did not name (C2 with a gradient moves a fixed-pressure boundary
  flux: 4.8e-2), so that its bit comparisons can fail.
- Not exercised: a current patch in the tree (E2 has two potential patches; KE has one), insulating cells with the options, PIMPLE
  runs of D3 to convergence (the transient kernel takes D3 through the same function; ADR 0204's P3 tree and library runs still
  agree bit for bit), a negative cell Joule heat (none occurred).

### Status by the decision rule

Every check of (a) met; (b): M matters, D3 built and every check of D3 met; D2 met; every mutation failed its test; N is the gate
below. F-298 is set **done** (`[x]`).

### Questions for the maintainer (revised by the results)

- **Q1 (the default Joule heat on a skewed mesh).** T-J1-D: on the 29.6-degree wedge the default Joule heat adds up to 11.9 % less
  or more than the electric power (3.1e-2 W of 0.265 W), so the energy equation of a default electric case on a skewed mesh gets a
  heat that is not the power the boundaries deliver. Revised recommendation: (b) as its own item with the bit-identical rule lifted
  for it (the D1 formula with the default currents; every skewed electric case changes its heat and temperature), because the gap is
  a conservation defect of the default rather than a modelling choice; (a) until then. Proceeding on (a).
- **Q2 (the default flux correction on fixed-pressure faces).** M: 2.0e-3 of the inflow (5.0e-3 with SIMPLEC) in a converged run on a
  30-degree outlet with one corrector. Recommendation as registered: (b) as its own item with the rule lifted (every converged
  result of a skewed fixed-pressure case with correctors changes; the goldens and baselines it moves would be listed there). Until
  then a case that needs the balance sets `[solver.gradient_equations] pressure = true` with any option. Proceeding on (a).
- **Q3, Q4** stand as registered (proceeding on (a)).
