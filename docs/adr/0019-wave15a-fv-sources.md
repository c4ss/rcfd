# ADR 0019: Wave 15A: initialisation methods (F-205), zone sources and fixed values (F-201), user transport equations (F-202)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new tests was
compiled or run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and
the assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned
away. A test set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes
that were not fixed here) may be appended at the end without touching registered limits.

Roadmap: F-205 (first), F-201, F-202. Clean-room: published literature and first principles only; nothing from
vendor material, vendor installs or vendor manuals; no vendor names in the repository. No registered check
depends on a literature value: every expected number is a closed form derived below. Statements attributed to the
literature are marked "(from memory, unverified)".

Process disclosure: before this ADR was written I read the existing code (scalar sweep, energy solver, registers, sources,
`Simulation::new`, `set_fields`) and used a small independent Python model of the *intended 1-D discretisation* (not of the
code under test) to derive the discrete results quoted in sections E1 to E4 and U1 to U4 (scripts in the untracked scratch
directory `<scratch dir>`). No new feature code existed and no new test had been compiled or run when the limits
below were fixed.

## Decisions

### D1 Initialisation (F-205)

New settings `initial.method` = `uniform` (default; today's behaviour, bit-identical) | `potential` | `coarse`, only for the
steady incompressible algorithms `simple` and `simplec` (other algorithms: located error at `initial.method`).

**Potential-flow initial solve** (`rcfd_fv::potential`). A scalar potential `psi` with `div(grad psi) = 0` (unit diffusivity,
the existing `laplacian` with deferred non-orthogonal correction: 4 outer correction passes) and `u = grad psi` (least-squares
gradient with the boundary face values). Boundary conditions from the case:

* a patch whose pressure condition is `fixed`: Dirichlet `psi = 0`;
* otherwise Neumann `d psi/d n = u_b . n` with `u_b` the boundary face velocity of the velocity condition: `fixed` (including
  expression profiles) its value, `moving_wall`, `shear`, `partial_slip`, `marangoni`, `symmetry` zero normal flux, `freestream`
  `u_ref . n`; a `zero_gradient` velocity patch without a fixed pressure is a located error (`boundaries.<p>.velocity`), as is a
  cyclic patch;
* with no Dirichlet patch the problem is a pure Neumann problem: the net boundary flux must vanish
  (`|sum u_b . S| <= 1e-8 sum |u_b . S|`, else a located error stating both numbers), and the level is fixed by doubling the
  diagonal of cell 0 (reference value 0).

Pressure from Bernoulli, kinematic: `p = H - |u|^2 / 2`. `H` is the area-weighted mean of `p_b + |u|^2/2` over the fixed-pressure
faces (potential velocity of the owner cell), else `H = initial.pressure + max|u|^2 / 2`. The face flux is rebuilt from the
cell velocity and the boundary values by the existing `set_fields` path. Viscous effects are absent; the result is an
*initial guess* and is overridden by nothing but a later restart (`load_checkpoint`/`continue-from` run after construction).

**Coarse-level pre-solve** (`initial.method = "coarse"`, `initial.coarse_factor` int 2 to 8, default 2,
`initial.coarse_iterations` int >= 1, default 200). The case is cloned with the cell counts of the generator divided by the factor
(`nx`, `ny`, `nz`, `nx_right`, `ny_right`, `nz_right` where used by the generator; a count that the factor does not divide
or that falls below 2 is a located error at `initial.coarse_factor`), the clone runs `coarse_iterations` SIMPLE iterations
(with `initial.method = "uniform"`), and velocity, pressure and temperature are transferred to the fine mesh with the existing
`Interpolator` (non-conservative mode) and `set_fields`. Imported meshes (no cell counts) are a located error. The turbulence state
and scalars are not transferred (they start from their own initial values).

**Region patching by registers** is the existing F-124 `initial.patches` (velocity, pressure, temperature). **Localised
turbulence initialisation**: `initial.patches.<n>.variable` gains `k`, `epsilon`, `omega`, `nutilde`; the value is applied inside
the register to the turbulence model's own field after the model is built (`scalar_mut`), so `k = ...` needs a model that has
that field (located error at `initial.patches.<n>.variable` otherwise: e.g. `epsilon` with `k_omega_sst`).

### D2 Zone sources and fixed values (F-201)

New collection `zone_sources.<name>`:

| Key | Meaning |
| --- | --- |
| `target` | `temperature` or the name of a `scalars.<s>` / transported `species.<s>` entry (the balance species is a located error: it follows from the closure and absorbs the sources of the others) |
| `kind` | `source` (default) or `fixed_value` |
| `zone`, `register`, `box_min`, `box_max`, `cell_zone` | the zone, exactly as `sources.<n>` (all, box, cell_zone, register) |
| `value` | source: volumetric source, `W/m^3` for temperature (divided by `physics.rho * physics.cp`), `[phi]/s` for scalars; fixed_value: the value in K or `[phi]` |
| `value_expression` | expression of `x, y, z, t` evaluated at the cell centres, re-evaluated every step/iteration. Dimension checked: temperature source `W/m^3`, temperature fixed `K`, scalar fixed dimensionless, scalar source dimensionless or `1/s` (read as per second). Overrides `value` |
| `linear_coefficient` | `k >= 0` [1/s], source kind only, implicit: `S = value - k (phi - reference)`, with `reference` (default 0) |

Implementation: `rcfd_fv::scalar::sweep_ext` takes extras (`sink[c] = k` added to the diagonal times the cell volume,
the constant part added to the source, and a fixed-cell mask with values). `sweep` calls `sweep_ext` with empty extras so the
existing path performs the same floating-point operations (baseline hashes). A fixed cell has its row replaced by the identity
(`a_cc = 1`, off-diagonals of the row 0, `b = value`) before the residual is formed, and the solved value is overwritten by
the fixed value after the linear solve, so a fixed cell holds its value bit-exactly. The neighbours keep their couplings to the
fixed cell. The residual of a fixed cell is zero. Several sources on one target add (name order); two fixed-value zones that
share a cell are a located error. Sources/fixed values act in steady `simple`/`simplec` and transient `piso`/`pimple` runs of
the incompressible engines (not compressible, not variable density, not `coupled`: located errors).

NOT attempted, stated now: mass sources, momentum sources/fixed values by expression (the F-031 `sources` remain as they are),
sources and fixed values for turbulence quantities, the variable-density and compressible engines. F-201 is therefore
`partial` whatever the numbers.

### D3 User transport equations (F-202)

`scalars.<name>` gains:

* `equation` = `transport` (default; today's steady convection-diffusion-source, now also transient), `diffusive`
  (`d s/dt = div(D grad s) + S`, no convection), `poisson` (`div(D grad s) + S = 0` at every step, no time derivative),
  `algebraic` (`s = expression`, no transport);
* `diffusivity_expression` (`m^2/s`, expression of `x, y, z, t`, evaluated at the face centres; overrides `diffusivity`/`schmidt`;
  not with `schmidt`);
* `source_expression` (added to `source`; cell centres; dimensionless or `1/s`);
* `initial_expression` (dimensionless, overrides `initial`);
* `expression` (algebraic only): may use `x, y, z, t`, `T` (when `energy.enabled`) and the names of the other scalars and
  transported species (previous iterate), dimensionless except `T` and the coordinates.

Transient support: `scalars` and `species` are advanced with the time scheme of the transient engine (`solver.time_scheme`, Euler
or BDF2 or the bounded variant) in `piso`/`pimple` runs, on the flux of the step; steady algorithms unchanged. `species`
transient advance keeps the balance-species closure.

NOT attempted, stated now: the *vector* variant (three-component diffusive/Poisson/algebraic variables) and coupling of the
user variables into properties, turbulence or other equations. F-202 is therefore `partial` whatever the numbers.

## Registered checks

All errors are against closed forms derived here. "Observed order" is `log2(e_h / e_{h/2})` of the stated error on the stated
pair of grids. Cell-centre values are compared with the exact function at the cell centre.

### F-205 (`rcfd-api/tests/init_methods.rs`, `rcfd-fv/tests/init_potential.rs`)

**I1 Potential flow around a circular cylinder (tree).** `mesh.generator = "cylinder_ogrid"`, `r_in = a = 1`, `r_out = R = 10`,
`depth = 1`, `grading = 10`, `physics.nu = 0.01`, `physics.rho = 1`, `cylinder` velocity `fixed` (0), `farfield` velocity
`fixed` `(U, 0, 0)` with `U = 1`, `initial.method = "potential"`, no iterations. Derivation: `psi = (A r + B/r) cos(theta)`, `theta`
from the +x axis, with `psi_r(a) = 0` giving `B = A a^2`, and `psi_r(R) = U cos(theta)` (the Neumann condition `u_b . n = U cos theta` on the
outer circle) giving `A = U R^2 / (R^2 - a^2)`. Velocity: `u_r = A (1 - a^2/r^2) cos(theta)`, `u_theta = -A (1 + a^2/r^2) sin(theta)`;
surface speed `|u_theta(a)| = 2 A |sin(theta)|`, peak `2A = 2.0202` (the finite-R factor `R^2/(R^2 - a^2) = 1.0101` is part of
the closed form, so there is no blockage excuse). Meshes M1 = (`nx = 64`, `ny = 40`), M2 = (`nx = 128`, `ny = 80`).
* I1a (M2): at the first cell ring (cell centres at radius `r_c`), `max |u_theta - u_theta,exact(r_c)| / (2A) <= 0.03`.
* I1b (M2): surface velocity by linear radial extrapolation of `u_theta` from the first two ring centres at the same angular index
  to `r = a`: `max |u_theta,surf - (-2A sin(theta_c))| / (2A) <= 0.03` (the acceptance wording is "surface velocity distribution
  within 3 percent"; both forms must hold).
* I1c: RMS (cell-volume weighted) error of the velocity vector over all cells, relative to `2A`, observed order M1 to M2 `>= 1.4`
  (boundary cells carry a first-order gradient, so the global RMS order is expected between 1.5 and 2; 1.4 is registered).
* I1d: first-ring maximum error of I1a, M1 to M2, observed order `>= 0.9`.

**I2 Backward-facing step, fewer iterations (library).** The step geometry and settings of the existing laminar-step test:
`backward_step_graded`, expansion ratio 2, `l_in = 2`, `l_out = 20`, `h_in = step = 1`, `n = [8, 80, 8, 8]`, uniform grading, parabolic
inlet of mean velocity 1, `Re_h = 100`, `LinearUpwind`, SIMPLE with the default relaxation, `h_from_solution`, stop when
`max(res_u, res_p) < 1e-6` (cap 3000). Initial states: A uniform `u = (1, 0, 0)`, B uniform `u = 0`, C potential velocity with
Bernoulli pressure. Limit: `iterations(C) < iterations(A)` and `iterations(C) < iterations(B)`, all three converged. Reported
without a limit: potential velocity with zero pressure.

**I3 Coarse pre-solve (tree).** `cavity_2d`, `nx = ny = 40`, `physics.nu = 0.01`, lid velocity `(1, 0, 0)`, SIMPLEC,
`tolerance_u = tolerance_p = 1e-6`, `max_iterations = 3000`. Initial states: uniform, and `coarse` with `coarse_factor = 2`,
`coarse_iterations = 300`. Limit: iterations to convergence of the coarse-initialised fine run `<` those of the uniform run. Reported:
the equivalent total work counting a coarse iteration as 1/4 of a fine one.

**I4 Localised turbulence initialisation (tree).** `k_epsilon` on a channel, register `core` (box), `initial.patches.a`
`variable = "k"` value 0.5 and `.b` `variable = "epsilon"` value 2.0 on `core`: the fields `k`, `epsilon` equal 0.5, 2.0 on exactly
the register cells (difference 0) and equal the derived uniform initial values (`turbulence.k_initial`/`epsilon_initial` set to 0.01
and 0.05) elsewhere (difference 0). Diagnostics (located): `epsilon` with `k_omega_sst` (`initial.patches.b.variable`), a turbulence
variable without a turbulence model.

**I5 Defaults.** `initial.method = "uniform"` and omitted give bitwise identical `ux, uy, p` after 20 iterations; the baseline hash
tests (`baseline_hashes.rs`) stay unchanged.

**I6 Diagnostics (located).** potential with `piso`; `zero_gradient` velocity with non-fixed pressure; closed domain with a net inflow;
`coarse_factor` not dividing the counts; `coarse` with an imported mesh case is not tested (no imported-mesh fixture in the api
tests; stated).

Verdict rule: F-205 `done` only if I1a to I1d, I2, I3, I4, I5 are all met; otherwise `partial`.

### F-201 (`rcfd-api/tests/zone_sources.rs`)

Common set-up (1-D slab through the tree): `box_2d`, `lx = L = 0.1`, `ly = 0.02`, `ny = 2`, `nx = n`, `physics.rho = 1`,
`physics.cp = 1000`, `physics.conductivity = k = 2` (so `alpha = 2e-3`), `energy.enabled`, all four velocity conditions `fixed` (0),
`xmin`, `xmax` temperature `fixed` 300, `ymin`, `ymax` adiabatic, `simplec`, `tolerance_u = 1e-12`, `max_iterations = 400`,
`energy.relax = 1`. Fluid at rest, so the flow is trivial and the energy equation is pure conduction.

**E1 Uniform volumetric heat source.** `zone_sources.q` target `temperature`, `zone = all`, `value = 1e4` W/m^3. Exact:
`T = 300 + q x (L - x) / (2k)`, `Delta T_max = q L^2 / (8k) = 6.25 K`. Discrete result derived from the scheme (orthogonal uniform
grid, Dirichlet half-cell treatment, exact cell source): interior cells satisfy the second-difference equation, which is exact for a
quadratic; the first cell gives, with `b = -q/(2k)`, a residual `0.5 b h` in the flux balance for the exact parabola, removed by a
uniform offset `delta` with `-2 delta/h = 0.5 b h`, i.e.
`T_h(x_i) = T_exact(x_i) + q h^2 / (8k)` exactly (the same at the last cell). Checked numerically in the independent model: relative
error `(1/n)^2` for `n = 8, 16, 32, 64`.
* E1a (`n = 64`): `max |T_h - T_exact - q h^2/(8k)| <= 1e-6 * Delta T_max` (the identity).
* E1b: `max |T_h - T_exact| / Delta T_max` at `n = 64` is `<= 3e-4` (model value 2.441e-4) and the observed order over
  `n = 8, 16, 32, 64` (two finest pairs) is in `[1.9, 2.1]`.
* E1c (energy conservation): the heat leaving through `xmin` and `xmax` (the `heat_flow` reports, sum of the two magnitudes)
  equals `q * V` (`V = L * ly * depth` of the mesh) to relative `1e-6`.

**E2 Linearly varying source by expression.** `value_expression = "2e4 [W/m^3] * x / 0.1 [m]"`, i.e. `q(x) = q0 x / L`, `q0 = 2e4`.
Exact: `T = 300 + q0 x (L^2 - x^2) / (6 k L)` (derivation: `-k T'' = q0 x / L`, `T' = -q0 x^2/(2kL) + C`, `T(0) = T(L)` fixes
`C = q0 L/(6k)`). The cell-centre value times the volume is the exact cell integral of a linear function. Model (independent):
`max |T_h - T_exact| / max T_exact` at `n = 64` is 4.72e-4. Limits: that relative error at `n = 64` `<= 6e-4`, observed order over
`n = 8, 16, 32, 64` (finest two pairs) in `[1.9, 2.1]`.

**E3 Fixed-value cell zone.** No source; `xmin` 300, `xmax` 400 K; `zone_sources.hold` target `temperature`, `kind = fixed_value`,
`value = 500`, `zone = box` with `box_min = [0.045, -1, -1]`, `box_max = [0.055, 1, 1]`, `n = 40` (fixed cells are those with centres
in the box: centres at `(i + 0.5) h`, `h = 0.0025`, i.e. `i = 18..21`, centres `0.0462..0.0537`, four cells; the test asserts exactly
four fixed cells). Limits: E3a every fixed cell equals 500 with difference exactly 0 (`<= 0`); E3b the cells outside lie on the
piecewise linear profile through the boundary values and the fixed-cell centres (the discrete solution is exactly linear between the
wall, whose face value is the Dirichlet value, and the first fixed centre, and constant inside): `max |T - T_pl| <= 1e-6 * 200 K`;
E3c the same with the zone given as a register (`registers.hold` box) gives a result identical to E3 (`<= 0`).

**E4 Species decay (transient, exp(-k t)).** Closed `box_2d` `4 x 4`, all velocity conditions `fixed` (0), `pimple`, `species.a`
(`initial 1`, `diffusivity 1e-3`) and `species.b` (balance, `initial 0`), `zone_sources.decay` target `a`, `value 0`,
`linear_coefficient = 1` (S = -a). Exact: `a(t) = exp(-t)`; `a + b = 1`. Discrete identity for implicit Euler on a uniform field:
`a_N = (1 + dt)^(-N)`. `dt = 0.1, 0.05, 0.025`, end time 1.
* E4a (Euler, `solver.time_scheme = "euler"`): `|a_N - (1 + dt)^(-N)| <= 1e-10` for each `dt`, and the observed error order
  against `exp(-1)` (finest two pairs) in `[0.9, 1.15]`.
* E4b (BDF2): observed order against `exp(-1)` (finest two pairs) in `[1.8, 2.3]`; error at `dt = 0.025` `<= 2e-4`.
* E4c: `max |a + b - 1| <= 1e-12` and `a` uniform over the cells to `1e-12` at the end.

**E5 Source restricted to a register and conservation.** The E1 set-up with the source only in a register `mid` (box
`[0.03, 0.07]`, `n = 40`): the heat leaving through the two walls equals `q * V_mid` to relative `1e-6` (`V_mid` the volume of
the register cells computed from the mesh), and the temperature is symmetric about `x = L/2` to `1e-9 * Delta T` (cell `i` against
cell `n - 1 - i`). Only these two limits are registered for E5.

**E6 Diagnostics (located):** unknown target; the balance species as target; `linear_coefficient < 0`; `kind = fixed_value` with
`linear_coefficient`; a fixed-value zone overlapping another; a zone with no cells; expression of the wrong dimension (temperature
source in K); `zone_sources` with `compressible` or `coupled`.

Verdict rule: F-201 is `partial` (named gaps in D2). The acceptance statement itself is met only if E1a, E1b, E3a, E3b, E4a, E4b hold.

### F-202 (`rcfd-api/tests/user_scalars.rs`)

Set-up as above: `box_2d`, fluid at rest unless stated, `simplec` (steady) or `pimple` (transient), scalar `s`.

**U1 Diffusive scalar, variable diffusivity, fixed-value walls.** `lx = 1`, `ly = 0.1`, `ny = 2`, `nx = n`; `scalars.s` `equation =
"diffusive"`, `diffusivity_expression = "1 [m^2/s] * (1 + x / 1 [m])"`, `xmin` fixed 0, `xmax` fixed 1, `ymin`, `ymax` zero gradient.
Exact: `D s' = const` gives `s = ln(1 + x)/ln 2`. Model (face diffusivity at the face centre, half-cell Dirichlet): volume-weighted RMS
error 4.17e-4, 1.04e-4, 2.61e-5, 6.53e-6 for `n = 16, 32, 64, 128`. Limits: observed order over the finest two pairs in `[1.9, 2.1]`;
RMS error at `n = 128` `<= 8e-6`. Also the constant-diffusivity run reproduces the linear profile to `1e-9`.

**U2 Poisson, manufactured solution.** `box_2d` unit square, `equation = "poisson"`, `D = 1 + x` (expression), exact `s = sin(pi x) cos(pi y)`,
Dirichlet on all four patches from the exact function (per-patch `value_expression`), `source_expression` from
`S = -div(D grad s) = -pi cos(pi x) cos(pi y) + 2 pi^2 (1 + x) sin(pi x) cos(pi y)` (`D_x = 1`, `s_x = pi cos(pi x) cos(pi y)`, `lap s = -2 pi^2 s`).
`n = 8, 16, 32, 64`. Limits: observed order of the volume-weighted RMS error over the finest two pairs in `[1.8, 2.3]`; RMS error at
`n = 64` `<= 2e-3`.

**U3 Transient diffusive eigenmode.** `box_2d` `lx = 1`, `ny = 2`, `nx = 128`, both end walls fixed 0, `D = 0.1`, `initial_expression =
"sin(pi * x / 1 [m])"`, exact `s = sin(pi x) exp(-pi^2 D t)`, error `max` at `t = 1`. Euler `dt = 0.1, 0.05, 0.025`: observed order
(finest two pairs) in `[0.9, 1.15]`; BDF2 `dt = 0.2, 0.1, 0.05`: in `[1.8, 2.3]` (estimated spatial error at `nx = 128`: relative eigenvalue error `(pi h)^2/12` of order 5e-5 times `lambda t e^(-lambda t)`, i.e.
about 2e-5, against smallest temporal errors of about 4e-3 (Euler) and 8e-4 (BDF2); the test also prints the `nx = 256` errors at the
finest `dt` of each sequence, without a limit).

**U4 Advection-diffusion-reaction, steady.** `channel_2d`-style 1-D: `lx = 1`, `ly = 0.1`, `ny = 2` through `channel_2d` with inlet velocity
`(1, 0, 0)`, symmetry side walls, outlet zero-gradient velocity and fixed pressure; `scalars.s` `diffusivity 0.1`, inlet fixed 1,
outlet zero gradient; `zone_sources.react` target `s` `linear_coefficient = 0.5` (zone all). Exact: `u s' - D s'' + k s = 0`,
`s(0) = 1`, `s'(1) = 0`: `s = A e^{m1 x} + B e^{m2 x}`, `m_{1,2} = (u +- sqrt(u^2 + 4 k D))/(2 D)` (10.4772, -0.47723),
`A = -m2 e^{m2}/(m1 e^{m1} - m2 e^{m2})`, `B = 1 - A`. Model (independent, half-cell inlet, zero-gradient outlet): RMS errors central
2.79e-4, 6.95e-5, 1.74e-5, 4.34e-6, 1.09e-6 for `n = 20..320`; upwind 3.01e-3, 1.72e-3, 9.37e-4, 4.93e-4, 2.53e-4. Limits:
`central` (`solver.convection = "central"`): observed order (finest two pairs of `n = 40, 80, 160, 320`) in `[1.9, 2.1]`; `upwind`:
finest two pairs of `n = 80, 160, 320` in `[0.9, 1.15]`.

**U5 Algebraic variable.** `scalars.a` (transport, inlet value 1, `diffusivity 0.1`, steady, as U4 without the reaction) and
`scalars.b` `equation = "algebraic"` `expression = "2 * a + x / 1 [m] + T / 1 [K]"` with `energy.enabled` and a fixed-temperature case
(`initial.temperature = 300`): `b` equals `2 a + x + 300` at every cell centre to `1e-12` after convergence.

**U6 Diagnostics (located):** `diffusivity_expression` with `schmidt`; a dimension error in `diffusivity_expression`; `expression`
with `equation != algebraic`; an algebraic equation that refers to itself; unknown `equation`; `algebraic` and `poisson` with an
unknown symbol; transient scalar on a compressible case (still rejected).

Verdict rule: F-202 is `partial` (vector variant, D3). The acceptance statement itself (diffusive second-order grid convergence,
Poisson manufactured-solution order 2) is met only if U1 and U2 hold.

## Hygiene

* H1 `baseline_hashes` bit-identical with default settings, existing tests unchanged and passing; `sweep` performs the same floating
  point operations as before when the extras are empty.
* H2 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast`
  on a `git archive` copy of the final commit, `CARGO_TARGET_DIR=<target dir>`.
* H3 every deviation and tuning is disclosed under "Results"; references that cannot be verified are marked in
  `docs/reference-data.md`.
* H4 roadmap statuses stay `partial` unless the whole acceptance text of the item and the scope in its title are met.

## Results

(to be filled after the runs)
