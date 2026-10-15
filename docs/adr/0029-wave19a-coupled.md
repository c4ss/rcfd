# ADR 0029: Wave 19A block-coupled solver with open boundaries and body force, and solved flow with solid zones (F-032, F-051)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new code was compiled
or run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test
set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes that were not fixed
here, results, deviations) may be appended at the end without touching registered limits.

Roadmap: F-032 (primary: the first coupled solver of wave 16 is extended), F-051 (primary: the frozen-flow restriction of solid
zones, ADR 0024 D5, is removed for the coupled algorithm). F-027 (AMG) is used as it stands; its open gap (V-cycle evidence at
1M cells, default-run gate) is not touched. F-217 is not attempted.
Clean-room: published literature only; nothing from vendor material, vendor installs or vendor manuals. No vendor names
in the repository. Closed forms below are derived here; the benchmark numbers of the differentially heated cavity were re-verified for this wave in two independent secondary reproductions: arXiv:physics/0305049 (Nu 2.243, 4.519, 8.800 attributed to de Vahl Davis) and arXiv:1412.2866 (Nu 1.118, 2.243; u_max 3.649, 16.178; v_max 3.697, 19.617 for Ra 1e3, 1e4); the original 1983 tables were not reachable (stated in `docs/reference-data.md`).

## Audit of what exists before this wave

* `rcfd-fv/src/coupled.rs` (wave 16): steady `(u, v[, w], p)` in one system per Picard iteration, implicit Green-Gauss pressure
  gradient `G`, implicit Rhie-Chow continuity rows, GMRES(40) with ILU(0) momentum blocks and one AMG V-cycle on the pressure
  Schur complement. Scope: closed domains with the velocity prescribed on every boundary, laminar, no body force, no energy.
  `solver.algorithm = "coupled"`; the baseline hash `cavity-coupled` (59 steps) is registered in `baseline_hashes.rs`.
* `cht.rs` (api) and `rcfd_fv::cht`: solid cells, harmonic face conductivity, contact resistance, convective walls; all of this
  works on the full mesh in the energy solver for any face flux. Solids require `solver.frozen_flow = true` because the flux
  through solid cells cannot be removed in SIMPLE without a Rhie-Chow bypass (ADR 0024 D5).
* The segregated solver treats body forces with a balanced-force flux term and a fixed-flux wall pressure (gradient of the
  pressure on a prescribed-velocity face equal to the face body force), which is what makes a stably stratified fluid stay at rest
  (`natural_convection.rs`).
* The energy loop of a steady run is "flow iteration, then one `EnergySolver::iterate` with the new flux" in `sim.rs`.

## Decisions

### D1 Open boundaries in the coupled system

A boundary face is *prescribed* when the velocity condition fixes the face-normal velocity (existing `pressure_bc().fixed_u`);
the flux is the constant `u_b . S`. Every other non-empty boundary face is *open* and must carry a fixed pressure
(`FaceCoeffs` with `a = 0`; pressure value `p_b`, a located error otherwise: `boundaries.<patch>.pressure`). With owner cell `P`,
`rAU = V/a_P`, `dn` the normal distance cell-centre to face:

```
phi_b = u_P . S + rAU_P (grad p)_P . S - rAU_P (|S|/dn) (p_b - p_P) + Cb ,   Cb = rAU_P (a_f . S - a_P . S)
```

(velocity zero-gradient: `u_b = u_P`). This is the same boundary flux as the segregated solver (`phiHbyA` from the owner value
minus the compact pressure difference). The momentum row gets `S_k p_b` as an explicit constant instead of the matrix entry
`S_k p_P`; the continuity row of `P` gets `u_P . S` (a velocity-block entry) and the implicit pressure part. The pressure level
is fixed by the reference cell only when no open face exists (the existing behaviour for closed domains, bit-identical). Backflow
through an open face is not specially treated.

### D2 Body force (balanced force, wall pressure)

A body force (acceleration) `a` is an optional cell field `a_c` with face values `a_f . S_f` (the existing `BodyForce`). The
momentum rhs gains `V a_c`. Rhie-Chow with a body force: the face flux of a converged momentum balance is
`H/a|_f . S + rAU_f (a_f . S - snGrad p |S|)`, so the continuity row carries the explicit balanced-force term
`Cb_f = rAU_f a_f . S - sum_k S_k (w rAU_o a_o,k + (1-w) rAU_n a_n,k)` in addition to the implicit parts. On a prescribed-velocity
(zero flux) face the face pressure is extrapolated with the body force: `p_f = p_P + dn (a . n_out)`, which enters `G` as an
explicit constant (this is the fixed-flux wall pressure of the segregated solver, SIMPLE form with ratio 1). Without a body
force every one of these terms is skipped, so the closed no-force path is arithmetically unchanged.

### D3 Solid cells are excluded from the pressure-velocity system (no Rhie-Chow bypass)

A boolean cell mask `solid`. Classification of faces: both cells solid, or a boundary face of a solid cell: *skipped* (no flux,
no coefficients). Internal face with exactly one solid cell: a *solid wall* for the fluid cell `P` (no-slip, zero flux). Then:

* the flux on a skipped or solid-wall face is the constant zero, identically; it is never computed from an interpolation, so
  the interpolation of the inverse diagonal across a fluid/solid face (the leak of the rejected penalty approach, ADR 0024 D5)
  does not exist: there is no coupling coefficient to leak through;
* momentum, fluid cell `P`: convection with `phi = 0` on the face, diffusion with the face diffusivity zeroed there and a wall
  term `nu_f |S|/dn_P` added to the diagonal (`u_wall = 0`, `dn_P` the normal distance from the centre of `P` to the face, so the
  wall is at the interface, not at the neighbour's centre); the pressure on the face follows D2 (zero gradient without body force);
* solid cells carry identity rows `u_k = 0` in the momentum blocks and `p = 0` in the continuity block, with no entries in `G`,
  `D`, or any fluid row; no fluid row references a solid unknown;
* the energy equation is untouched: it runs on the whole mesh with the (masked) flux `phi`, which is exactly zero on solid faces.

Consequences: the discrete solution in the fluid is the solution of the same problem with the solid region removed and replaced
by no-slip walls at the interface; there is no pressure-equalisation in the solid and no bypass. Not covered (named gaps): the
least-squares cell gradient of higher-order convection schemes still sees the solid neighbour values (`u = 0`) at the neighbour
centre rather than at the wall for fluid cells next to a solid; this affects only `linear_upwind`/`tvd` accuracy next to the
interface, not mass conservation. Solids with the segregated algorithms (SIMPLE, PISO) remain `frozen_flow` only.

### D4 Settings tree

No new keys. `solver.algorithm = "coupled"` now accepts: open boundaries with `velocity = zero_gradient` and `pressure = fixed`
(D1), `physics.body_force` (uniform), `energy.enabled = true` with Boussinesq buoyancy (`physics.thermal_expansion`,
`physics.gravity`, `physics.reference_temperature`) and `solids` / `thermal_interfaces` / convection wall conditions. The
combination solids + a solved flow requires `solver.algorithm = "coupled"`; with any other algorithm the located error stays at
`solver.frozen_flow` (message updated). Located errors added: `solver.p_ref_cell` inside a solid; a closed fluid region that is
disconnected (one pressure level per component cannot be fixed; reported at `solids.<name>.zone`); a prescribed non-zero normal
flux on a boundary face of a solid cell (`boundaries.<patch>.velocity`); an open boundary face without a fixed pressure
(`boundaries.<patch>.pressure`). Still rejected with the coupled algorithm (each located): turbulence, rheology, user functions,
momentum sources, lumped components, radiation, `bc_kind`, `zone_sources`, property laws, scalars/species with solids,
non-orthogonal correctors, periodic patches, `solver.bulk_velocity`.

### D5 Energy coupling

The coupled engine of `sim.rs` gets the Steady-engine energy block (Boussinesq force masked to zero in solid cells, one
`EnergySolver::iterate` per outer iteration with the coupled flux), without chemistry heat, components or zone sources.
`energy.enabled` is no longer rejected for the coupled algorithm; the existing test entry that asserted that rejection
(`unsupported_combinations_are_diagnosed`, row `energy.enabled`) is replaced by a row for `turbulence.model` (disclosed here, not
a loosened limit: the capability is the deliverable) and `coupled_energy_is_supported` is added.

Complete list of existing assertions audited before the commit (grep of `crates/*/tests` and in-crate tests for `coupled`,
`frozen_flow`, `body_force`): the only assertion that flips is the `energy.enabled` row above. Everything else stays and keeps
passing unchanged: `cht_case.rs` F4 case "no `frozen_flow`" (algorithm simple, path `solver.frozen_flow`, kept), F4b, F5;
`zone_sources.rs` (`coupled` still rejects zone sources); `bc_catalogue.rs` V1 (`bc_kind` with coupled still rejected);
`properties.rs` (property law with coupled still rejected at `properties.viscosity.law`); the `rcfd-fv/tests/coupled.rs` cavity
test; `baseline_hashes.rs`. The code paths relaxed are: the `physics.body_force` and `energy.enabled` rejections for `coupled` in
`case.rs`, the `alg == "coupled"` row of the solid conflict list in `cht.rs` and its `energy.buoyancy` row for the coupled
algorithm, the `solids` documentation string and the `solver.algorithm` documentation string in `schema.rs` (documentation only).

### D6 Default paths are bit-identical

All new terms are behind `if` on features that default off (open faces, body force, solid mask). The closed-domain,
no-force, no-solid arithmetic is unchanged, evidenced by `baseline_hashes.rs` (`cavity-coupled`) passing unmodified.

## Derivations used by the checks

**Discrete 1-D channel.** For a flow parallel to a wall pair with uniform `x` the convective term vanishes identically in the
discrete equations (every face value in a row equals that row's velocity, cell flux balance is zero), whatever the scheme. With the
Green-Gauss cell gradient and fixed pressure at both ends, a linear `p(x) = G (L - x)` has the exact cell gradient `-G` in every
cell including the end cells, and the Rhie-Chow flux correction vanishes. Hence the discrete solution is `u_j` from
`nu (u_{j+1} - 2 u_j + u_{j-1})/h^2 = -G` for interior cells and, with the wall at distance `h/2`,
`nu ((u_2 - u_1)/h - 2 u_1/h)/h = -G` (and the mirror for the last cell), a tridiagonal system the test solves itself (Thomas
algorithm, independent of the code under test). Analytic limit: `u = (G/(2 nu)) y (H - y)`, `U_max = G H^2/(8 nu)`,
`Q = G H^3/(12 nu)`.

**Closed form of the discrete wall-bounded channel (checked by hand before the run).** Interior cells: the second difference is
exact for any quadratic. With `a = G/(2 nu)`, `u = a y (H - y) + c`, the wall cell (wall at `h/2`) gives
`(u_2 - u_1)/h - 2 u_1/h = -1.5 a h - 2c/h`; the equation requires `-2 a h`, so `c = a h^2/4 = G h^2/(8 nu)` (same at the other wall).
Thus `u_j = a y_j (H - y_j) + a h^2/4` exactly, the velocity error against the parabola is the constant `a h^2/4`, i.e. relative to
`U_max = a H^2/4` it is exactly `h^2/H^2`; the midpoint sum of the parabola is `I + a H h^2/12` so the discrete flow rate is
`Q (1 + 2 h^2/H^2)` exactly. Predictions: A2 (`ny = 32`) velocity error `9.77e-4 U_max`, `Q` error `1.953e-3`, observed order
exactly 2; C1 (`H_f = 0.5`, 16 fluid rows, `h = 1/32`): velocity error `3.91e-3 U_max`, `Q` error `7.81e-3`. The tests use this
closed form as the reference of check (i) and validate their own Thomas solve against it to `1e-12`.

**Couette.** `u = U y/H`, `p = 0`: the linear profile is represented exactly by the half-cell wall treatment (the wall term
`(U - u_N)/(h/2)` with `u_N = U (1 - h/(2H))` equals `U/H`), so the discrete solution equals the continuous one (to the
linear-solver tolerance), for any scheme.

**Hydrostatic.** Constant `a = (0, -g, 0)`, `u = 0`, `p(y) = p_0 - g (y - y_0)` (kinematic). The Green-Gauss gradient of a linear
`p` is exact in the interior and, with the wall extrapolation `p_f = p_P + dn (a . n_out)`, also in wall cells; the balanced-force
flux term makes the face flux vanish. So the discrete solution is `u = 0`, `phi = 0`, exact `p` up to the solver tolerance.

**Duct with a solid wall (Nusselt number).** As ADR 0024 section D: half channel, fluid `0 <= y <= H`, symmetry at `y = 0`, solid
`H <= y <= H + t`, flux `q` into the domain at `y = H + t`, `u = (3/2) U (1 - eta^2)`, `T = A x + theta(y)`,
`q = rho cp U H A`, bulk-to-wall difference `theta(H) - theta_b = (17/35) q H/k_f`, `Nu = h D_h/k_f = 140/17 = 8.2353` with
`D_h = 4H`, total outer-surface-to-bulk difference `Delta T_tot = q (17 H/(35 k_f) + t/k_s)`. Here the velocity is *solved*
(pressure outlet, parabolic inlet profile), so the discrete velocity differs from the parabola by the discretisation error.

**High-conductivity solid wall cavity.** A solid layer with `k_s/k_f = 1e4` and thickness `t = 0.1 H` has a temperature drop
`q t/k_s` with `q = Nu k_f Delta T/H`; for `Nu = O(1)` that is `1e-5 Delta T`. The fluid then sees an isothermal wall and the
differentially heated cavity result applies (benchmark `Nu`). The harmonic face conductivity between a fluid and a solid cell of
equal width `h` is `2 k_f k_s/(k_f + k_s) = 2 k_f (1 - O(1e-4))`, i.e. a Dirichlet face at distance `h/2`.

## Registered checks

Common: library tests `rcfd-fv/tests/coupled_open.rs` (A, B), `rcfd-fv/tests/coupled_solid.rs` (C, C5 library part),
case tests `rcfd-api/tests/coupled_cht.rs` (C5, D, E). Meshes from `rcfd_mesh::gen`. Coupled runs use the library defaults
(`alpha_u = 0.9`, GMRES(40), AMG V-cycle) unless stated, with `tol_rel = 1e-10` when a continuity limit is checked; iteration to
the stated residual. Velocities are divided by the stated scale. "SIMPLEC" is the existing segregated solver with its defaults.

### Stop criteria and tolerances (registered for all checks below)

* Coupled library runs: GMRES `tol_rel = 1e-10`, `tol_abs = 1e-14`, restart 40, `max_iter = 400`; outer iteration stops when the largest
  velocity change of an iteration is `< 1e-11` (in units of the velocity scale of the check) or after 3000 outer iterations,
  whichever comes first; a run that hits the cap is reported as such and its limits are still asserted. B1/B2 run a fixed
  500 iterations.
* SIMPLEC library runs (A1, A2, A3, B3, A7): default settings, `momentum_solver`/`pressure_solver` `tol_rel = 1e-10`, stop when the largest
  velocity change `< 1e-11` or after 12000 iterations (the cap of the existing natural-convection tests).
* Tree cases (C3, D, E): `tolerance_u = tolerance_p = 1e-10`, `max_iterations = 3000` (C3: 12000), `solver.pressure_rel_tol = 1e-10`,
  `solver.alpha_u = 0.9`, `energy.relax = 0.9` for the buoyant C3 and `1.0` for the one-way coupled D.
* B3 and C3 report their iteration counts; no limit on them is a pass/fail criterion.

### A. Open boundaries, exactness, agreement with the segregated solver

* A1 Couette, `box_2d` 8 x 16, `L = 2`, `H = 1`, `nu = 0.1`; `xmin`: fixed velocity `(U y/H, 0, 0)`, `U = 1`; `ymax`: moving
  wall `(1,0,0)`; `ymin`: no-slip; `xmax`: `u` zero-gradient, `p = 0`. Limits for coupled and for SIMPLEC, with upwind and with
  central convection: `max |u - U y_c/H| <= 1e-7 U`, `max |v| <= 1e-7 U`, `max |p| <= 1e-7 nu U/H`.
* A2 pressure-driven Poiseuille, `box_2d` `nx = 8`, `ny = 32`, `L = 4`, `H = 1`, `nu = 0.1`, `G = 0.8` (so `U_max = 1`),
  `xmin`: `p = G L`, `xmax`: `p = 0`, both with `u` zero-gradient. Limits (coupled and SIMPLEC each): (i) vs the discrete 1-D
  solution (closed form above, `u_j = a y_j (H - y_j) + a h^2/4`): `max |u - u_j| <= 1e-7 U_max` over all cells, `max |v| <= 1e-7 U_max`; (ii) vs the analytic parabola:
  `max |u - u_exact| <= 1e-2 U_max` (predicted `9.77e-4`); (iii) `|Q - G H^3/(12 nu)|/Q <= 1e-2` from the mass flux through `xmin`; (iv) observed order of
  the max error against the analytic parabola for `ny = 16, 32, 64`: both successive orders `>= 1.5`; (v) coupled vs SIMPLEC:
  `max |u_c - u_s| <= 1e-6 U_max`.
* A3 Poiseuille with a velocity inlet: as A2 but `xmin` fixed velocity `(4 U_max y (H - y)/H^2, 0, 0)` (per-face profile of the
  parabola at the face centres) and `xmax` `p = 0`; at `x = L/2`: coupled vs SIMPLEC `max |u_c - u_s| <= 1e-4 U_max`; coupled vs
  the parabola `<= 1e-2 U_max`; the mean pressure drop between the cell columns nearest `x = L/8` and `x = 7L/8` equals
  `G` times their distance within 2 percent. (Engineering bounds fixed before the run; the first cell column next to a
  fixed inlet sees a one-sided pressure gradient in Green-Gauss form, which is why the drop is taken between interior columns.)
* A4 continuity: for A1, A2, A3 (coupled, `tol_rel = 1e-10`): `max_c |sum_f phi_f| <= 1e-10 U_ref |S|_max` over all cells
  (`U_ref = U` resp. `U_max`).
* A5 3-D Couette: `box_mesh` 6 x 8 x 3, `L = (2, 1, 0.5)`, symmetry on both `z` faces, otherwise as A1: `max |u - U y_c/H| <= 1e-7 U`,
  `max |v|, |w| <= 1e-7 U`, coupled.
* A6 the existing `rcfd-fv/tests/coupled.rs` (cavity, Ghia gate 0.02, settle counts) and `rcfd-api/tests/coupled.rs` pass with the
  single disclosed change of D5; `baseline_hashes.rs` unmodified passes.
* A7 reported only (no pass/fail): outer iterations and total linear iterations of the coupled solver against SIMPLEC to a residual
  of 1e-8 on A1, A2, A3 and the cavity of A6. The roadmap acceptance of F-032 (fewer outer iterations than SIMPLE on the cavity) was
  already measured in wave 16 and is not re-claimed or extended here.

### B. Body force and buoyancy (coupled)

* B1 hydrostatic: closed `box_2d` 12 x 12, unit square, `a = (0, -10, 0)`, zero velocity, `p_ref` fixed: `max |u| <= 1e-9`,
  `max |phi| <= 1e-9` (face flux, `|S| = 1/12`), `max |p - p_hyd| <= 1e-8` with `p_hyd = -10 (y - y_ref)`.
* B2 stably stratified fluid at rest: B1 mesh, `T = y` fixed top/bottom, `beta = 1`, `T_ref = 0.5`, `g = 1e4 Pr` (Pr = 0.71), as
  `stably_stratified_fluid_stays_at_rest`: `max |phi| < 2e-3` after 500 iterations (the same limit the segregated solver was held
  to).
* B3 differentially heated cavity (Pr = 0.71, hot left, cold right, adiabatic top/bottom; `H = 1`, `alpha = 1`, `nu = Pr`,
  `beta = 1`, `|g| = Ra Pr`): Ra = 1e3 on 32 x 32 graded 4:1 and Ra = 1e4 on 40 x 40 graded 4:1 (the meshes of the segregated
  tests), `Tvd(VanLeer)` for flow and energy, energy relaxation 0.9. Limits: `Nu_avg`, `u_max(x = 0.5)`, `v_max(y = 0.5)` each within
  2 percent of 1.118/3.649/3.697 (Ra = 1e3) and 2.243/16.178/19.617 (Ra = 1e4); hot/cold wall flux imbalance `< 1e-6` relative;
  converged to a residual below 1e-9 within the cap of 12000 iterations of the segregated tests (the count is reported, not limited); coupled vs the segregated SIMPLEC solution on the same mesh:
  `|Nu_c - Nu_s|/Nu_s <= 5e-3`.

### C. Solid exclusion (library and tree)

* C1 solid slab in a pressure-driven channel (leak test), `box_2d` `nx = 8`, `ny = 32`, `L = 4`, `H = 1`, solid `y < 0.5` (cells whose
  centres satisfy it: 16 rows), fluid height `H_f = 0.5`, `nu = 0.1`, `G = 3.2` (`U_max = G H_f^2/(8 nu) = 1`), `xmin`: `p = G L`,
  `xmax`: `p = 0`, top wall no-slip. Limits: (i) `u = v = 0` in every solid cell exactly (`== 0.0`); (ii) the face flux is exactly
  `0.0` on every face with a solid cell on either side, including the inlet and outlet faces of the solid rows; (iii) fluid cells vs the
  discrete 1-D solution of the fluid layer (wall at the interface and at the top): `max |u - u_j| <= 1e-7 U_max`,
  `max |v| <= 1e-7 U_max`; (iv) vs the analytic `(G/(2 nu))(y - 0.5)(1 - y)`: `<= 1e-2 U_max`; (v) `|Q - G H_f^3/(12 nu)|/Q <= 1e-2`;
  (vi) `max_c |sum_f phi_f| <= 1e-10 U_max |S|_max` over fluid cells.
* C2 obstacle: `box_2d` 40 x 20, `L = 4`, `H = 1`, solid block `1.5 <= x <= 2.0`, `0.25 <= y <= 0.75`, `nu = 0.05`, inlet uniform
  `(1,0,0)`, outlet `p = 0`, walls no-slip, coupled. Limits: flux exactly `0.0` on all faces touching the block; `u = v = 0` in the
  block; inlet flux `+` outlet flux: `|sum| <= 1e-10 U H`; continuity as C1(vi); coupled-vs-recomputation: the same case with
  `solver.pressure_rel_tol = 1e-12` changes `u` by `<= 1e-6 U` (the linear-solver tolerance does not matter).
* C3 differentially heated cavity with high-conductivity solid walls through the settings tree: `mesh.generator = box_2d`,
  `nx = 60`, `ny = 50`, `lx = 1.2`, `ly = 1`; solids `x < 0.1` and `x > 1.1` with `k_s = 1e4`, `rho = cp = 1`; fluid `k = 1`,
  `rho = cp = 1`, `nu = 0.71`, Boussinesq as B3 with Ra = 1e3; `solver.convection = "central"` for flow and energy (no cell gradients are used, so the D3 gradient gap is not part of this comparison; at the cell Peclet number of about 0.1 central differencing is second-order and bounded), `xmin` fixed T = 1, `xmax` fixed T = 0, `ymin`/`ymax` adiabatic and
  no-slip. `Nu_avg = Q_xmin / (k_f Delta T / 1)` with `Delta T = 1` and `Q` the `heat_flow` report of the `xmin` patch divided by the patch area times `H = 1` (i.e. per unit depth: `Nu = (Q/A_patch)/(k_f Delta T/H)`). Limits:
  (i) `|Nu/1.118 - 1| <= 2e-2`; (ii) the same cavity without solids (`nx = 50`, `lx = 1`, same `h = 0.02`) with the coupled solver:
  `|Nu_solid - Nu_nosolid|/Nu_nosolid <= 1e-2`; (iii) heat balance across zones: `|Q_xmin + Q_xmax| <= 1e-9 |Q_xmin|` (heat flow out
  of the two ends), and the interface flux at `x = 0.1` equals `Q_xmin` within `1e-9` relative (per-zone balance of the left solid);
  (iv) all solid cells have `u = v = 0` and zero flux on solid faces exactly. No benchmark number for a *finite* wall conductivity is
  used: none was verified; no claim is made about such cases.

### D. Solved-flow CHT duct with a closed-form Nusselt number (settings tree, `coupled`)

Geometry and fluid/solid values of ADR 0024 section D (`H = 1`, `t = 0.5`, `k_f = 1`, `k_s = 10`, `U = 1`, `rho cp` fluid 1, solid 4,
`q = 1`, `L_x = 2`, 8 cells in x, `n_f` cells across the fluid and `n_f/2` across the solid, central convection), but the flow is
solved: `nu = 0.01`, inlet `u = (3/2) U (1 - y^2)` (zero in the solid rows), `xmax`: `p = 0`, `u` zero-gradient, `ymin` symmetry, the
solid top `ymax` no-slip with the heat flux. Temperature inlet/outlet profiles equal the exact `T` as in ADR 0024.

* D1 (acceptance-type): relative error of `Delta T_tot` (outer surface mean minus flux-weighted bulk mean at mid-length) `<= 1e-2` for
  `n_f = 16` and `n_f = 32`.
* D2 Nusselt number from the fluid: `h = q / (T_w - T_b)` with `T_w` the interface temperature extrapolated from the first fluid cell
  with the flux `q` (`T_w = T_P + q dy/(2 k_f)`) and `T_b` the flux-weighted bulk temperature at mid-length; `Nu = 4 H h/k_f`:
  `|Nu - 140/17|/(140/17) <= 2e-2` at `n_f = 32`.
* D3 the velocity: fluid `u` at mid-length vs the parabola `<= 1e-2 U_max`, `v` at mid-length `<= 1e-3 U_max`; solid cells exactly zero;
  the inlet and outlet mass flows agree to `1e-10 U H`.
* D4 energy balance: (a) whole domain: net outflow of enthalpy (`rho cp phi T_b` summed over boundary faces, using the boundary
  face values) plus the conductive heat flows out through all patches (per-face conductivity) plus `-q L_x` is `<= 1e-9 q L_x` in
  magnitude; (b) solid zone: heat in through `ymax` equals the heat passing the interface into the fluid, `<= 1e-9 q L_x`; (c) fluid
  zone: interface heat in equals the net outflow of enthalpy plus conduction through inlet/outlet, `<= 1e-9 q L_x`.

### E. Located validation errors (each asserted by path)

* E1 solids with `simple` and no `frozen_flow`: `solver.frozen_flow` (updated message names the coupled algorithm); solids with
  `coupled` and `frozen_flow = true`: `solver.frozen_flow`.
* E2 coupled + solids + `turbulence.model`: `turbulence.model`; coupled + solids + `properties` law: `properties`.
* E3 `solver.p_ref_cell` in a solid cell (closed domain): `solver.p_ref_cell`.
* E4 closed domain whose fluid is split into two components by a solid slab: `solids.<name>.zone`.
* E5 non-zero prescribed inflow through a boundary face of a solid cell: `boundaries.<patch>.velocity`.
* E6 open face with `velocity = zero_gradient` and `pressure = zero_gradient`: `boundaries.<patch>.pressure`.
* E7 coupled with a periodic mesh, `zone_sources`, `bc_kind`: still diagnosed.

### F. Regression

* F1 `crates/rcfd-api/tests/baseline_hashes.rs` passes unmodified (no hash re-recorded) before the work, after each slice and at
  the end. F2 existing `rcfd-fv` and `rcfd-api` tests pass, except the single disclosed change of D5. F3 hand-back gate on a `git
  archive` of the final commit: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --release --no-fail-fast`.

## Not delivered (expected; reconciled with the results at the end)

Transient coupled solve (a pseudo-transient or time-accurate coupled step is not cheap here: the coupled matrix would need the
time term and a transient Rhie-Chow form); solved flow with solids in the segregated algorithms (SIMPLE, PISO, PIMPLE); turbulence,
rheology, user functions, momentum sources, lumped components, radiation, property laws, scalars and species with the coupled
algorithm; non-orthogonal correction and periodic patches in the coupled solver; the gradient treatment at fluid-solid faces for
higher-order convection (D3); a verified finite-conductivity conjugate natural-convection benchmark; F-027's V-cycle evidence and
default gate; F-217; temperature-dependent solid conductivity (F-220).

## Results (addendum written after the runs; registered limits above are unchanged)

Commits: ADR `2dd8c96`; library (open boundaries, body force, solid exclusion, tests A, B, C1, C2) `438be81`; settings-tree glue and
tree tests (C3, D, E) `4d2eb38`. Measured values are printed by the tests (`cargo test --release -p rcfd-fv --test coupled_open
--test coupled_solid -- --nocapture`, `-p rcfd-api --test coupled_cht -- --nocapture`).

| Check | Measured | Limit | Result |
|---|---|---|---|
| A1 Couette, coupled upwind / central | max error u 1.2e-11 / 1.5e-11, v 6.5e-12 / 8.4e-12, p 9e-12 (47 / 49 outer) | 1e-7 U, 1e-8 | pass |
| A1 Couette, SIMPLEC upwind / central | u 6.4e-11, v 4.5e-11 / 6.7e-11, p 1.1e-9 / 1.7e-9 (120 / 117 outer) | same | pass |
| A2 coupled ny = 32: vs discrete closed form / v | 2.3e-10 / 2.0e-14 U_max | 1e-7 | pass |
| A2 coupled: vs analytic (predicted 9.766e-4) / Q error (predicted 1.953e-3) | 9.766e-4 / 1.953e-3 | 1e-2 | pass |
| A2 coupled observed orders (ny 16, 32, 64) | 2.000, 2.000 | >= 1.5 | pass |
| A2 SIMPLEC ny = 32: vs discrete / v / vs analytic / Q | 1.3e-2 / 7.4e-4 / 1.2e-2 / 3.8e-3 | 1e-7 / 1e-7 / 1e-2 / 1e-2 | MISS (known-miss test) |
| A2 SIMPLEC orders | 1.37, 1.74 | >= 1.5 | MISS |
| A2 coupled vs SIMPLEC | 1.3e-2 U_max | 1e-6 | MISS (follows from the SIMPLEC miss) |
| A3 coupled vs parabola at x = L/2 | 5.0e-3 U_max | 1e-2 | pass |
| A3 coupled vs SIMPLEC at x = L/2 | 4.6e-3 U_max | 1e-4 | MISS (known-miss test) |
| A3 pressure drop between columns 1 and 6 (coupled) | 1.842 against 2.000 (7.9 percent) | 2 percent | MISS (known-miss test) |
| A4 continuity (A1, A2, A3) | 1.7e-14, 3.8e-15, 3.7e-15 | 1e-10 | pass |
| A5 3-D Couette | u 3.4e-12, v/w 1.2e-12 (26 outer) | 1e-7 | pass |
| A6 existing tests, baseline hashes | pass (hashes unmodified, 3 passed, 1 ignored as before) | | pass |
| B1 hydrostatic | max u 3.9e-12, phi 2.7e-14, p 2.0e-13 | 1e-9, 1e-9, 1e-8 | pass |
| B2 stratified rest | max phi 9.4e-5 | 2e-3 | pass |
| B3 Ra = 1e3 (32 x 32): Nu / u_max / v_max against 1.118 / 3.649 / 3.697 | 1.1167 (-0.11 %) / 3.622 (-0.74 %) / 3.678 (-0.52 %) | 2 % | pass |
| B3 Ra = 1e4 (40 x 40): against 2.243 / 16.178 / 19.617 | 2.2434 (+0.02 %) / 16.162 (-0.10 %) / 19.516 (-0.51 %) | 2 % | pass |
| B3 hot/cold wall flux imbalance (1e3, 1e4); coupled vs SIMPLEC Nu (SIMPLEC: default alpha_u, momentum/pressure tol_rel 1e-10; 385 / 379 outer) | 8.0e-9, 5.9e-9; 3.1e-6, 7.0e-6 | 1e-6; 5e-3 | pass |
| C1 slab: solid u == 0, flux on solid faces | exactly 0.0, exactly 0.0 | exactly 0 | pass |
| C1 vs discrete / v / analytic (3.906e-3 predicted) / Q (7.812e-3 predicted) / continuity | 5.4e-11 / 7e-15 / 3.906e-3 / 7.812e-3 / 4e-15 | 1e-7 / 1e-7 / 1e-2 / 1e-2 / 1e-10 | pass |
| C2 obstacle: leak, in+out flux, continuity, change with rel-tol 1e-12 | 0.0, 2.3e-15, 8.3e-14, 4.7e-15 | 0, 1e-10, 1e-10, 1e-6 | pass |
| C3 tree cavity with solid walls (k_s = 1e4): Nu against 1.118 | 1.1183 (+0.03 %), 777 outer iterations | 2 % | pass |
| C3 against the same cavity without solids | 2.5e-5 relative (Nu 1.1183 both) | 1e-2 | pass |
| C3 heat balance \|Q_xmin + Q_xmax\| / \|Q_xmin\|; interface flux against Q_xmin | 4.2e-11; 7.8e-10 | 1e-9; 1e-9 | pass (second with little margin) |
| C3 solid u == 0 and flux on solid faces | exactly 0 | exactly 0 | pass |
| D1 Delta T_tot error, n_f = 16 / 32 | 2.4e-4 / 1.5e-4 (92 / 129 outer) | 1e-2 | pass |
| D2 Nu against 140/17, n_f = 32 | 8.2367 (1.7e-4) | 2e-2 | pass |
| D3 u against parabola / v / solid zero / inlet-outlet mass flow | 4.6e-4 U_max / 4.8e-5 U_max / exactly 0 / 4.0e-14 U H | 1e-2 / 1e-3 / 0 / 1e-10 | pass |
| D4 (a) whole domain / (c) fluid zone | 3.2e-11 / 1.6e-12 q L_x | 1e-9 | pass |
| D4 (b) solid zone as registered (no end conduction) | 2.55e-5 q L_x | 1e-9 | MISS (known-miss test) |
| E1 to E7 located errors | all asserted by path (E7: periodic at `solver.algorithm`, zone_sources at `zone_sources.q`; the bc_kind row is covered by the unchanged V1 test of `bc_catalogue.rs`) | | pass |

Run settings: B1, B2 and B3 use the registered coupled linear solver (GMRES `tol_rel` 1e-10, `tol_abs` 1e-14, 400 iterations; the first
runs of B1/B2/B3 did not set all of them, and B3's SIMPLEC run used `alpha_u = 0.8` and the default linear tolerance; they were
rerun with the registered settings and gave the same coupled numbers, SIMPLEC changed only in the iteration count and the
coupled-vs-SIMPLEC Nu difference, rows above). B3 follows its own registered stop text (residual below 1e-9 within 12000 iterations,
both algorithms) rather than the generic `max du < 1e-11` outer stop of the stop-criteria section; both converged below 1e-9 in
fewer than 420 iterations. The flux imbalance of B3 is the hot-versus-cold wall Nu difference, the quantity of the segregated test.

### Diagnoses of the misses (diagnosed, not tuned)

* A2 SIMPLEC. Fully converged (max du of the order of 1e-15 within 250 iterations) but the solution carries an odd-even modulation
  along x in u and p (u at mid-height 0.966, 1.018, 0.999, 1.005, 1.008, 0.997, 1.020, 0.974 along x for `ny = 16`), and v is not zero.
  As a diagnostic only (not a fix, not used for any assertion), the same run with `alpha_u = 0.5` gives 0.994, 1.004, 0.999, 1.001,
  1.002, 0.999, 1.004, 0.995: the converged segregated solution depends on the velocity relaxation factor, which is the known
  property of Rhie-Chow interpolation without a relaxation-consistent correction (this attribution is a hypothesis consistent with
  the diagnostic, not proven). The coupled solver, which forms the same Rhie-Chow flux inside one system, gives the discrete closed
  form to round-off for every `ny`. The failing limits are a property of the existing segregated solver at open boundaries; they are
  kept as the ignored test `a2_simplec_known_miss`.
* A3. The pressure drop between interior columns is 7.9 percent short: the pressure carries a decaying odd-even mode launched at the
  fixed-velocity inlet (column-to-column drops 0.71, 0.18, 0.55, 0.29, 0.48, 0.34, 0.45 against 0.40 for the coupled solver, similar
  with a different phase for SIMPLEC), the first column seeing the one-sided zero-gradient inlet pressure that the registered text
  anticipated. The mode was not removed (no inlet pressure treatment was changed). The coupled-vs-SIMPLEC difference of 4.6e-3 U_max
  is the difference of the two modulations. At `nx = 8` the target positions L/8 and 7L/8 fall between two columns; the drops over the other admissible column pairings (measured in a debug dump, not asserted) are about 4.5, 6.4, 7.3 and 7.9 percent short, so the miss does not depend on the pairing chosen. Known-miss tests: `a3_pressure_drop_known_miss`, `a3_coupled_vs_simplec_known_miss`. The
  velocity profile against the parabola (5.0e-3 U_max) passes.
* D4 (b). The registered statement omitted the conduction through the solid rows of the fixed-temperature inlet and outlet patches.
  In the discrete solution it is not zero: the two ends carry -2.55e-5 q L_x net. With that term the solid-zone balance closes to
  3.3e-11 q L_x. The corrected statement was written after the registered form missed; the registered form is kept as the ignored
  test `d4_solid_registered_form_known_miss` and the corrected one is `d4_energy_balances`. The interface heat flow used by (b) and
  (c) is recomputed in the test from the temperature field with the harmonic face conductivity (valid for the uniform duct mesh and
  no contact resistance), it is not read from the solver.
* E4 location: a split fluid region is reported at `solids.<first solid name in sorted order>.zone`; with several solids this may name
  a solid that is not the one causing the split (the message states the cause). Known limitation.
* A set-up slip in the first C3 run: the test compared the interface heat flow with `Q_xmin` with the wrong sign (ratio 2.0). The sign
  was corrected in the same test before it was committed (the criterion, 1e-9 relative, was not touched); the corrected value is
  7.8e-10.

### Deviations from the registered plan (disclosed)

* Procedure: the C3 sign slip was fixed in place in the uncommitted test instead of keeping the wrong-sign test as an ignored one
  (the rule asks for a corrected new test with the old kept); the sign error is documented above and the criterion was not touched.

* `crates/rcfd-api/tests/coupled.rs::unsupported_combinations_are_diagnosed`: the `energy.enabled` row became a `turbulence.model`
  row because the energy equation is now supported (D5); the new test `coupled_energy_is_supported` covers the supported case.
* A7 (reported only; measured differently from the registered text): iteration counts are to a `max du` of 1e-11 (Couette, A2) or a
  residual of 1e-9 (B3), not to the registered residual of 1e-8; linear (GMRES) iterations are given for A2 only; the A6 cavity was not
  measured. Outer iterations to 1e-11 on the Couette cases 47-49 (coupled) against 117-120 (SIMPLEC); on A2 `ny = 32` both
  need 529 outer iterations, the coupled solver with 10851 GMRES iterations in total; on the cavities of B3 (residual 1e-9, TVD)
  coupled needs 384 and 376, SIMPLEC 378 and 418 outer iterations. The coupled solver is therefore not shown faster in outer
  iterations on these cases and each of its iterations costs more. The cavity of A6 was not re-measured.

### Reference data

The de Vahl Davis values used in B3 (Nu 1.118 and 2.243, u_max 3.649 and 16.178, v_max 3.697 and 19.617) were re-checked against two
independent published papers that quote them; the original 1983 tables were not reachable, so they are recorded as checked through
secondary sources in `docs/reference-data.md`.

### Not delivered (final)

Transient coupled solve; solved flow with solids in the segregated algorithms; turbulence, rheology, user functions, momentum
sources, lumped components, radiation, property laws, scalars and species with the coupled algorithm; non-orthogonal correction and
periodic patches in the coupled solver; the gradient treatment at fluid-solid faces for gradient-based convection schemes (the
least-squares gradient of a fluid cell next to a solid still sees the solid cell value; the registered comparisons use central
convection for that reason); a verified finite-conductivity conjugate natural-convection benchmark (C3 is a consistency check
against the same cavity without solids, not a benchmark of a finite wall); F-027's V-cycle evidence and default gate; F-217; F-220.
The fixed-velocity-inlet pressure mode (A3) remains an open defect next to prescribed-flux boundaries.
