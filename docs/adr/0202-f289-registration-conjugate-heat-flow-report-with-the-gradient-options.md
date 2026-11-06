# ADR 0202: F-289 registration: the conjugate heat_flow report with the gradient options, checked by a heat balance

Status: **REGISTERED 2026-11-06**, written before any code of F-289 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum at the end.

Parent: [ADR 0190](0190-f256-registration-case-tree-wiring-of-the-gradient-options.md) Q3 and its decision record of
2026-11-06 (answer (b): the report takes the options, as its own item with a balance check of the reported flows against the
source to the solver tolerance). Roadmap item F-289 (depends on F-256).

Read at main 9edfbda. Number: 0197 is the highest on main and no branch holds 0198 or above; the caller assigned 0202 to F-289
and 0203 to F-290 (one branch, `f289-f290`).

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)): limits are fixed here before the code and the runs;
a miss is reported as measured and its assertion moves to an ignored `*_known_miss` test with the limit intact; a defective
set-up is corrected by an amendment written before the corrected run; defaults stay bit-identical; no settings hash of an
existing case changes; `crates/rcfd-api/tests/baseline_hashes.rs` and `case_hashes_w48u.rs` are not edited; no new third-party
crate; no commit trailers. Clean-room: first principles and our own ADRs only.

## What F-289 asks

The roadmap entry: the conjugate `heat_flow` report (`conductive_face_flows`) uses the gradient options of F-256 that the energy
equation was solved with (ADR 0190 Q3 (b)); on a skewed conjugate case with `gradient_boundary = corrected` or `dropped`, the face
correction or a limiter set, the reported flows balance against the source to the solver tolerance, within a limit fixed by its
registration ADR; without the options the report is unchanged bit for bit (baseline hashes unchanged).

## What the code read shows (main 9edfbda, nothing run)

1. **The report.** In `Simulation`'s report evaluation (`ReportKind::HeatFlow`, `rcfd-api/src/sim.rs`) a case with solid zones
   (`ChtModel::has_solids`) returns `ChtModel::heat_flow_out(geom, &en.solver, patch)`, which sums
   `rcfd_fv::cht::conductive_face_flows(geom, &solver.t, &self.face_k, self.non_orthogonal)` over the patch faces.
   `non_orthogonal` is `solver_conv_scheme(case).needs_gradient()`, the scheme of the energy solver.
2. **The formula.** `conductive_face_flows` gives every face the flow out of its owner in W, with `k` the face conductivity
   (harmonic mean, contact resistances in series): internal faces `k lap_coeff (T_o - T_n) - k kv . gf` with `gf` the
   interpolated unlimited least-squares gradient; boundary faces `-k A dT/dn` from the boundary coefficients, minus
   `k kv . grad_o` on fixed-value faces. These are the terms of `diffusion::laplacian` with `gamma = k / (rho cp)_ref`.
3. **The solved operator with options.** `EnergySolver::gradient` (ADR 0190 K3, set in `Simulation::new` from
   `sim::gradient_options`) makes `scalar::sweep_ext` assemble `diffusion::laplacian_with`: the cell gradients that enter the
   secondary flux are limited (`gradient::limiter`, then scaled), the face correction replaces the component of `gf` along the
   cell-centre line by `(T_n - T_o) / |d|` on internal faces, and fixed-value boundary faces take the owner gradient, the
   corrected one (component along `d` replaced by `(T_b - T_P) / |d|`) or none. The report ignores all three, so with the options
   on skewed cells it is not the flux the matrix solved (ADR 0190 Q3 (a), documented in `docs/case-schema.md`).
4. **What a patch report can see.** A patch sum uses boundary faces only. The face correction acts on internal faces only, so
   the patch reports of a case with `gradient_face_correction` alone are unchanged by this item; the limiters and
   `gradient_boundary` change boundary faces.
5. **Other users.** `rcfd_fv::electric` uses `conductive_face_flows` for the current (the potential takes no options, ADR 0190
   K6). The non-conjugate `heat_flow` report (`EnergySolver::heat_flow_out`, `heat_flow_out_var`) uses the boundary coefficients
   only, with no secondary flux, with or without the options (ADR 0190 item 6). `group_net_outflow` (per-material sums) is a
   library function, not a case-tree report.
6. **Sources in a conjugate case.** `zone_sources.<n>` with `target = "temperature"` adds `value / (physics.rho physics.cp)`
   per cell of its zone to `EnergySolver::source`; the sweep adds `source V` to the right-hand side. Fixed-value zone sources are
   refused with solid zones.
7. **Steady prescribed flow.** With `solver.frozen_flow = true` the steady loop iterates only the energy equation and stops when
   its normalised residual (before the solve) is below `solver.tolerance_u` or at `solver.max_iterations`. `ChtModel::configure`
   sets the linear solve of a case with solids to `tol_rel = 1e-13`.
8. **Refusals that already exist.** ADR 0190 R1 to R4 (`check_gradient_options` in `Simulation::new`) apply to conjugate cases
   as to any other: a conjugate energy equation is an `EnergySolver` without phase change, so it takes the options.

## Decisions

**D1. Library (`rcfd-fv/src/cht.rs`).** One new public function,
`conductive_face_flows_opt(geom, t, k, non_orthogonal, opts: Option<GradientOptions>)`. With `opts = None` it is the formula of
item 2, operation for operation; `conductive_face_flows` becomes a call of it with `None`. With `Some(o)` it applies the rules of
`laplacian_with` (item 3) to the report: the cell gradients limited by `gradient::limiter(geom, T, T_b, grad, kind)` and scaled when
`o.secondary_limiter` is set, the face correction on internal faces when `o.face_correction`, and `o.boundary` on fixed-value
boundary faces, all with the field's current cell and boundary values. Without a gradient (`non_orthogonal = false`) there is no
secondary flux and the options do nothing, as in the operator. The face flows are then the operator's face fluxes times
`(rho cp)_ref`, with the same cell values.

**D2. Case wiring (`rcfd-api/src/cht.rs`).** `ChtModel::heat_flow_out` passes `solver.gradient`, the options the energy solver
it reads was built with, so the report follows the solver without a second mapping of the keys. No other report, key, rule or
message changes. A case without the keys has `solver.gradient = None` and calls the formula of item 2 unchanged.

**D3. Not changed.** The non-conjugate `heat_flow` report (item 5, Q1), `electric` (no options), the variable-density and
compressible reports, every solver and every operator. No new key, no new enumeration value, no new refusal (item 8 covers the
conjugate case; L below checks it).

**D4. Separable commits.** (1) this text; (2) `rcfd-fv` (D1) with its library test; (3) `rcfd-api` (D2) with the tree test;
(4) documents and roadmap; (5) the addendum.

## Acceptance checks and limits (fixed here, before any code)

Option sets, as ADR 0190: O1 `gradient_face_correction = true`; O2 `gradient_boundary = "corrected"`; O3 `gradient_boundary =
"dropped"`; O4 `gradient_secondary_limiter = "barth_jespersen"`; O5 `gradient_secondary_limiter = "venkatakrishnan"` (K 5);
O6 the same with `gradient_limiter_k = 0.5`; C2 = O1 + O2 + O5. "The default" is no key.

**Library set-up K2** (`crates/rcfd-fv/tests/cht_flows_f289.rs`): ADR 0016's skewed mesh K at `n = 8`
(`box_mesh([8; 3], [1; 3])` warped by `x' = x + 0.7 y + 0.3 z`, `y' = y + 0.3 z`), two materials (cells whose centre has
`x < 0.8`: `k = 1`; the others `k = 10`), a contact resistance `R'' = 0.05` on the faces between them (`contact_faces`),
`(rho cp)_ref = 2` (`ChtMaterials`, so `gamma = k / 2` and the unit conversion is not trivial); fixed values on every boundary
patch from `f(p) = 1 + sin(1.3 x + 0.4) cos(1.1 y) cos(0.9 z + 0.2)` at the face centres; cell values `f(centre) + 0.05 (-1)^c`
(an arbitrary field with local extrema, so both limiters act). The gradient is `least_squares(geom, T, T_b)`.

**Tree set-up C** (case text inline in `crates/rcfd-api/tests/cht_flows_f289.rs`; no case file, fixture or docs TOML block is
added): ADR 0190's mesh W (`wedge_2d`, `nx = ny = 16`, `lx = ly = 1`, `ramp_start = 0`, `ramp_angle = 30`); `physics` `rho = 1`,
`cp = 1`, `conductivity = 0.1`; `energy.enabled`, `energy.relax = 0.9`, `initial.temperature = 0.5`; every patch
`velocity = "fixed"` (zero prescribed flux); `xmin` `temperature = "fixed"`, `temperature_value = 1`; `ymin` (the ramp) fixed at
0; `xmax`, `ymax` the default (zero gradient); `solver`: `simplec`, `convection = "linear_upwind"`, `frozen_flow = true`,
`max_iterations = 3000`, `tolerance_u = tolerance_p = 1e-13`; solid `s`: `zone = "box"`, `box_min = [-10, -10, -10]`,
`box_max = [0.5, 10, 10]`, `conductivity = 1`, `density = 1`, `specific_heat = 1`; `zone_sources.q`: `target = "temperature"`,
the same box, `value = 1` (W/m^3); `heat_flow` reports on `xmin`, `xmax`, `ymin`, `ymax`. Run with `Simulation::run(10000)`.

Quantities of a finished run of C (computed in the test with `rcfd_fv` calls only, on `Simulation::geom`):

* the library field: `ScalarField::uniform("T", geom, 0.5)` with the boundary conditions of C (`FixedValue::scalar(1)` on
  `xmin`, `FixedValue::scalar(0)` on `ymin`, `ZeroGradient` on `xmax`, `ymax`) and the tree's final `T` as values; the labels
  (solid where the cell centre has `x <= 0.5`), `ChtMaterials` with `k_cell` 1 (solid) and 0.1 (fluid), `rho_cp_ref = 1`, and its
  `face_conductivity` and `gamma`;
* `Q = sum over solid cells of 1 * V_c` (W), `Q_solid = Q`, `Q_fluid = 0`;
* `H_p`, the tree's `heat_flow` report of patch `p` (`Simulation::report_values` after the run); `H_new` and `H_old`, the patch
  sums of `conductive_face_flows_opt(.., true, opts)` and of `conductive_face_flows(.., true)` on the library field;
* the residual of the solved equation at the final field: `r_c = b_c + S_c V_c - (A T)_c`, with `(A, b)` =
  `laplacian_opt(geom, gamma, bc, Some(grad), T, T_b, opts)` (the diffusion operator of the sweep with the set's options; the
  convection part of the sweep is zero on a zero flux) and `S_c = 1` in solid cells, 0 in fluid cells;
* the solver tolerance `tol_W = (rho cp)_ref sum_c |r_c|` (W); its part over the cells of a group `g`, `tol_W(g)`;
* the round-off scale `F = sum over all non-empty faces |flow_f|` of the new flows (W).

| Id | Check | Limit |
|---|---|---|
| P | `check_mesh` of C's mesh: maximum non-orthogonality; the solid zone has cells and `Q > 0` | at least 20 degrees (set-up; a miss stops the test) |
| E-lib | K2, for the default and each of O1 to O6, C2: per cell, `| sum_f s_cf flow_f / (rho cp)_ref - ((A T)_c - b_c) |`, `s_cf` = +1 for the owner and -1 for the neighbour, `(A, b) = laplacian_opt(..)` with the same options and cell values, against `S = max_c sum_{f in c} |flow_f| / (rho cp)_ref` | max `<= 1e-12 S` |
| E-lib-T | K2, each of O1 to O6, C2: the same with the flows of `conductive_face_flows` (no options) | max `> 1e-9 S` (the check can fail) |
| D1 | K2: `conductive_face_flows_opt` with `None` and with `Some(GradientOptions::default())` against `conductive_face_flows`, `non_orthogonal` true and false; and `Some(C2)` with `non_orthogonal = false` against `conductive_face_flows(.., false)` | equal bits on every face |
| E-tree | C, the default and each set: E-lib's per-cell identity on the library field of the run | max `<= 1e-12 S` |
| R-eq | C, the default and each set: `|H_p - H_new,p|` for each of the four patches | `<= 1e-12 (Q + sum_p |H_p|)`; for the default also equal bits with `H_old` |
| B1 | C, the default and each set: the domain balance `|sum_p H_p - Q|` | `<= tol_W + 1e-12 F` |
| B2 | C, the default and each set: the solver tolerance itself | `tol_W <= 1e-8 Q` |
| B3 | C, the default and each set: per material, `|G_g - Q_g|` for `g` solid and fluid, `G_g = group_net_outflow(geom, new flows, label, 2)[g]` | `<= tol_W(g) + 1e-12 F` |
| T-B1 | C, each of O2 to O6 and C2: B1 with `H_old` in place of `H_p` | `> tol_W + 1e-12 F` (the unchanged report misses) |
| T-B3 | C, each of O1 to O6 and C2: B3 with the flows of `conductive_face_flows` | misses for at least one group |
| T-O1 | C, O1: `H_new,p` against `H_old,p` (item 4) | equal bits on every patch |
| D2 | C without keys and C with the four keys written at their defaults | same settings content hash; same bits of every report and of `T` |
| D3 | `rcfd-api`: `baseline_hashes.rs`, `case_hashes_w48u.rs`, `cht_case.rs`, `coupled_cht.rs`, `thermal_extras_case.rs`, `gradient_options_f256.rs`; `rcfd-fv`: `cht.rs`, `thermal_bc.rs`, `electric.rs`, `solidification.rs`, `gradient_sweep_f256.rs` | pass unedited |
| L | located errors on C (no new rule; item 8): `convection = "upwind"` with O2 at `solver.gradient_boundary`; `convection = "central"` with O1 at `solver.gradient_face_correction`; `gradient_limiter_k = 1` alone at `solver.gradient_limiter_k`; `gradient_secondary_limiter = "minmod"` at `solver.gradient_secondary_limiter` (schema). Controls: C and C with each set build | as stated |

B1 is the acceptance's balance: the reported flows out of the domain equal the source up to what the solver left in its residual
(`tol_W`), plus round-off. B2 keeps B1 from being met by a run that did not converge. B3 is the same balance per material; it uses
internal faces, so it also tests the face correction, which no patch report can show (item 4).

**Can fail.** Two mutations, each made, its test run, the file restored and compared byte for byte with the kept copy:
M1, `ChtModel::heat_flow_out` passes `None` in place of `solver.gradient`: R-eq and B1 of O2 must fail. M2, the face correction
skipped in `conductive_face_flows_opt`: E-lib of O1 must fail.

`scripts/limit_audit.py` is run on the new test files.

**Gate.** One gate for F-289 and F-290 together (ADR 0203): fmt per package, clippy `--workspace --all-targets -D warnings`, every
test in release on a `git archive` copy of the gated commit (`scripts/gate_test.py`, one process per test, three jobs). A failure
of a known flaky test is rerun alone and reported with its name.

**Decision rule.** Every check met: F-289 is done. A miss: an ignored `*_known_miss` test with the limit intact and the number
recorded, and F-289 stays open with the miss named.

## Open questions

**Q1. The non-conjugate `heat_flow` report.** `EnergySolver::heat_flow_out` (a case without solid zones) has no secondary flux on
fixed-value faces, with or without the options (item 5), so on skewed boundary cells it is not the solved flux either.
(a) Leave it; F-289 names the conjugate report. (b) A new item that gives it the conjugate formula; that changes the reports of
existing skewed cases, so it needs either an opt-in or a lifted default rule. Recommendation (a) now, (b) as an item only if a
case needs it. Proceeding on (a).

## Not in F-289

The non-conjugate report (Q1), a transient balance (the capacity term), the convective part of the heat flow in solved-flow
conjugate cases, a per-material or interface report in the case tree, the electric current, new options or limiters.

## Where to go next

* [ADR 0190](0190-f256-registration-case-tree-wiring-of-the-gradient-options.md) for the options and their wiring.
* [ADR 0024](0024-wave17b-cht.md) for the conjugate model and its reports.
* [Case schema](../case-schema.md), section "Gradient options of the energy and scalar diffusion term".
