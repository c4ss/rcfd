# Case schema: turbulence, compressible flow and solver controls

The case file (TOML or JSON) is a settings tree. `rcfd schema` prints its JSON Schema, the
service returns it from `settings.schema`, and the GUI settings editor and the assistant tools
read the same tree, so every key below is available everywhere. Validation errors from
`rcfd validate` and `Case::parse_toml` name the offending key and, for TOML, its line:

    case.toml: compressible.rk_order (line 22): forward Euler (rk_order 1) is unstable with a limited reconstruction ...

Diagnostics found only while the simulation is built (for example a pinned pressure cell outside
the mesh) are located in the file text afterwards by `rcfd validate`.

Example cases (all run in seconds to a minute in a debug build):

| File | What it shows |
| --- | --- |
| `cases/channel/kepsilon_channel.toml` | periodic Re_tau 395 channel, standard k-epsilon, y+ report, y+-plateau stop |
| `cases/channel/kepsilon_inlet.toml` | developing channel, RNG k-epsilon, inlet intensity + length scale |
| `cases/channel/sst_channel.toml` | wall-resolved Re_tau 395 channel, k-omega SST, y+ = 1 from `mesh.target_y_plus` with 16 inflation layers |
| `cases/sod/sod.toml` | Sod shock tube, HLLC + van Leer + SSP-RK3 |
| `cases/wedge/wedge.toml` | Mach 2 flow over a 10 degree ramp |

## `turbulence`

| Key | Default | Meaning |
| --- | --- | --- |
| `model` | `laminar` | `laminar`, `k_epsilon` or `k_omega_sst` (Menter 2003, see below) |
| `variant` | `standard` | `standard`, `rng`, `realizable` (k-epsilon only; rejected for SST) |
| `wall_treatment` | `smooth` | k-epsilon wall cells: `smooth` (compound viscous/log epsilon and blended production, no switch), `hysteresis` (+-10 percent dead band on the y* = 11.225 switch) or `switch` (original hard switch; its nu_t never settles, see docs/bfs-convergence-findings.md) |
| `wall_function` | `standard` | log-law wall functions (the only option; `standard` variant constants) |
| `kappa`, `e_wall` | 0.41, 9.8 | log-law constants |
| `convection` | `upwind` | scheme for k and epsilon transport |
| `relax`, `relax_nut` | 0.7, 0.5 | under-relaxation of k/epsilon and of the eddy viscosity |
| `k_initial`, `epsilon_initial` | 0 | initial values; both must be set together, or leave both 0 to derive them |
| `omega_initial` | 0 | k-omega SST only, replaces `epsilon_initial` (`k_initial` and `omega_initial` together, or both 0 to derive them) |
| `initial_intensity`, `initial_viscosity_ratio` | 0.05, 10 | used to derive the initial k, epsilon from the initial velocity when the two above are 0 |

`k_epsilon` runs with the steady algorithms (`simple`, `simplec`) only, needs at least one wall
patch, a positive `physics.nu`, and cannot be combined with `energy.enabled`. Walls must be at
rest (moving walls are rejected). The momentum equation uses the solved-H operator
(`solver.h_operator = from_solution`) automatically unless you choose otherwise.

### `k_omega_sst`

Menter's 2003 k-omega SST ("Ten years of industrial experience with the SST turbulence model",
Menter, Kuntz and Langtry, Turbulence, Heat and Mass Transfer 4, 2003), in the form of the NASA
Turbulence Modeling Resource page (constants and blending functions checked against it, see
docs/reference-data.md). Same restrictions as `k_epsilon` (steady algorithms, a wall patch, no
energy). Wall treatment: automatic. A Spalding (1961) law of the wall gives the wall shear for
any first-cell y+, `omega` on the wall faces is `10 * 6 nu / (beta1 y^2)` (y = first cell-centre
distance, the TMR recommendation), and for y+ > 5 the cell omega follows the blend of the viscous
and the log-layer value. Wall-resolved meshes (y+ about 1) and wall-function-like meshes (y+ 30
to 300) both work; y+ between about 3 and 20 is the least accurate range and
`validate_case` of the assistant warns with a y+ estimate. Convection of k and omega uses
`turbulence.convection` (`upwind` is the safe choice; `linearUpwind` diverged in tests). `kappa`
and `e_wall` define the Spalding law. Boundary values: `turbulence_spec = values` takes
`k_value` and `omega_value`; the other inlet specifications derive `omega = epsilon/(beta* k)`.
Outputs add the field `omega`.

### Mesh generator parameters

`cavity_2d` is an `nx` x `ny` uniform grid on `lx` x `ly` (patches `lid` = ymax, `walls`, `frontAndBack`); it is
square only when `nx = ny` and `lx = ly`. Before wave 11 it silently built an `nx` x `nx` cavity of side `lx`.
Options a generator does not apply are rejected with a located diagnostic instead of being ignored:
`mesh.wall_layers` (`box_2d` and `channel_2d` only), `mesh.periodic_x` / `periodic_y` (`box_2d` only) and
`mesh.grading != 1` (not for `cavity_2d`, `wedge_2d`, `nozzle_2d`).

### Non-conformal interface (`mesh.generator = "channel_2d_interface"`, F-114)

A `channel_2d` made of two blocks that meet at `x = lx/2`: the first has `nx` x `ny` cells, the second
`nx_right` x `ny_right` cells (0 = same as the first). When the counts across the channel differ the
interface is non-conformal; it is joined at mesh level (`rcfd_mesh::non_conformal::join_non_conformal`)
into an ordinary mesh with one internal face per overlap of an interface face of one block with one of
the other, so every solver, boundary condition and report works unchanged and flux through the
interface is conserved to round-off by construction. Patches: `inlet`, `outlet`, `walls`, `sides`.
The Rust function also joins general planar interfaces in 3-D (convex faces; the patches must cover
each other, otherwise the join is rejected). Use `solver.non_orthogonal_correctors` for accuracy on the
skewed interface faces.

### Annulus blocks for rotating-frame zones (`mesh.generator = "annulus_3d"`, `"annulus_3d_interface"`, F-031)

`annulus_3d`: a full cylindrical annulus `r_in <= r <= r_out` about the z axis, height `lz`, `nx` cells around, `ny`
radial, `nz` axial; patches `inner`, `outer`, `zmin`, `zmax` (walls unless the case sets other conditions).
`annulus_3d_interface`: two such blocks of height `lz/2` joined at `z = lz/2` through a non-conformal interface; the
upper block has `ny_right` x `nz_right` cells (0 = same as the lower block). Both blocks have `nx` cells around, so the
rim polygons coincide and the radial rings differ.

### Boundary-layer inflation (`mesh.wall_layers`, F-108)

For `box_2d` and `channel_2d` the y direction can carry geometric wall layers:

| Key | Default | Meaning |
| --- | --- | --- |
| `wall_layers` | 0 | number of inflation layers (0 = off) |
| `first_layer_height` | 0 | height of the wall-adjacent cell; 0 = derive from `target_y_plus` |
| `target_y_plus` | 0 | y+ of the wall-adjacent cell centre: `h1 = 2 y+ nu / u_tau` |
| `layer_growth` | 1.2 | ratio of successive layer heights |
| `layer_walls` | `both` | `both` (needs an even `ny`) or `lower` |
| `layer_u_tau` | 0 | friction velocity for `target_y_plus`; 0 = flat-plate estimate from `layer_reference_velocity` and `layer_reference_length` (White Cf = 0.455/ln^2(0.06 Re_x)) |

After the layers the remaining cells grow smoothly (ratio solved so that the sizes fill `ly`).
Impossible stacks (too many layers, layers thicker than the domain, no height information) are
rejected with located diagnostics. Block meshes only: inflation on unstructured wall patches is
not implemented.

Per-boundary keys (`boundaries.<patch>.`):

* `turbulence` = `auto` (default) | `wall` | `inlet` | `zero_gradient`. `auto` treats a fixed
  zero-velocity patch as a wall, any other fixed-velocity patch as an inlet, the rest as zero
  gradient.
* Inlet specification `turbulence_spec`:
  * `intensity_length` (default): `k = 3/2 (I U)^2`, `epsilon = Cmu^(3/4) k^(3/2) / l`, with
    `turbulence_intensity` (0.05) and `turbulence_length` (required, m). `U` is the local inlet
    speed per face, so expression profiles work.
  * `intensity_viscosity_ratio`: same `k`, `epsilon = Cmu k^2 / (ratio nu)` with
    `turbulence_viscosity_ratio` (10).
  * `values`: `k_value`, `epsilon_value`.
  `Cmu` is the constant of the chosen variant (0.09; 0.0845 for `rng`).

Outputs: fields `k`, `epsilon`, `nut`, `wall_distance`, `y_plus` (cell field, non-zero in the
wall-adjacent cells; boundary value `y_plus` on wall faces for reports), VTU arrays with the same
names, a `Turbulence` row in the HTML report, a `turbulence` object in the service status
(model, variant, wall patches, y+ range, eddy-viscosity change) and the monitor `res_nut`
(maximum relative change of the eddy viscosity per iteration, also required to fall below
`solver.tolerance_u` for the run to count as converged).

`y_plus` report: `type = "y_plus"`, `patch` = a wall patch, `statistic` = `average` (area
weighted, default), `min` or `max`. It reports `y* = Cmu^(1/4) sqrt(k) y / nu` of the wall-adjacent
cells, the quantity the wall functions use.
For `k_omega_sst` the report gives the y+ from the Spalding-law friction velocity, `u_tau y / nu`, of the wall-adjacent cell centres (the quantity the mesh targets in `mesh.target_y_plus`).

## `compressible` and `solver.algorithm = "compressible"`

Selecting the algorithm switches to the explicit density-based Euler solver (no viscosity, no
heat conduction). Settings: `flux` (`rusanov`, `roe`, `hllc`, `ausm_plus`), `reconstruction`
(`first_order` or a TVD limiter name), `rk_order` (1 to 3), `cfl` (0.01 to 1, default 0.4),
`gamma`, `gas_constant`, `mode` (`transient` to `solver.end_time`, or `steady` marching until the
largest relative density change per step is below `steady_tolerance`).

Initial condition `compressible.initial`: `mode = uniform` uses `left` everywhere;
`mode = riemann` uses `left` for coordinates below `split_position` along `split_axis` (x, y or
z) and `right` above. Each state has `density`, `velocity` (vector) and `pressure`.

Boundaries: `compressible_bc` = `auto` | `slip` | `extrapolate` | `fixed` | `total_inlet` | `pressure_outlet` |
`farfield` | `nonreflecting` (the last four are the characteristic conditions of ADR 0006, below). `fixed` needs
`density_value`, `pressure_value` (positive) and optionally `velocity_value` (`temperature_value`
may replace `density_value`). `auto` keeps the solver default by mesh patch kind: outlet patches extrapolate, all others are slip walls. The mesh generator `wedge_2d` (`ramp_start`, `ramp_angle`, plus `lx`, `ly`, `nx`,
`ny`) builds the compression-ramp domain.

Characteristic conditions (`docs/adr/0006-compressible-boundary-conditions.md`; one-dimensional
characteristics normal to the face, a boundary face state whose physical flux is used):

| `compressible_bc` | keys | meaning |
|---|---|---|
| `total_inlet` | `total_pressure_value` [Pa], `temperature_value` (total, K), `flow_direction` (vector; empty or zero = against the patch normal) | subsonic inflow; the outgoing invariant `u_n + 2a/(gamma-1)` comes from the interior, `p0`, `T0` and the direction are imposed. Uses `compressible.gas_constant`. |
| `pressure_outlet` | `pressure_value` | static pressure on subsonic outflow; supersonic outflow (normal Mach >= 1) extrapolates. |
| `nonreflecting` | `pressure_value`, `reflection_value` (sigma in [0, 1], default 0.02) | pressure outlet relaxed toward `pressure_value`; sigma 1 = `pressure_outlet`, sigma 0 = no pressure imposed. Only normally incident waves; waves slower than `dx/(sigma a)` are reflected (a 0.03-wide pulse on 400 cells: reflected/incident 0.94 for sigma 1, 0.46 for 0.02, 0.078 for 0.002, 0 for 0). |
| `farfield` | `mach_value`, `flow_angle` (degrees from +x towards +y), `pressure_value`, `temperature_value` (static) | Riemann-invariant free-stream boundary with inflow/outflow and supersonic switching. No vortex correction for lifting bodies. |

`validate` errors name the key: a missing or non-positive total/static pressure or temperature, a
`flow_direction` that is not 3 components, a `reflection_value` outside [0, 1], a negative
`mach_value`.

Unsupported combinations that `validate` rejects, each with the key and line: forward Euler
(`rk_order = 1`) with a limited reconstruction, a transient run with `end_time <= 0`,
non-positive initial density or pressure, a `fixed` state without positive pressure and
density/temperature, turbulence, `energy.enabled`, `solver.pseudo_mode`, periodic meshes,
`compressible_bc` without the compressible algorithm, turbulence keys on a boundary, velocity
expressions, and force, moment, force-coefficient, heat-flow and y+ reports.

Output and monitor semantics for this solver: fields `rho`, `p`, `T`, `mach`, `ux`, `uy`, `uz`,
`speed`; `res_u` is the largest relative density change per step, `res_p` the largest relative
pressure change per step, `continuity` the relative mass drift of the whole domain per step (which
is non-zero when fixed or extrapolating boundaries let mass through), and the monitors `dt`,
`courant` and `res_rho`. In a transient run these are per-step changes and do not tend to zero.
`mass_flow` reports use the adjacent-cell density and velocity of the boundary face; `p` and `rho`
reports carry pressure and density dimensions (the pressure is the absolute pressure, not kinematic).

## `fluid` variable-density solver: ideal gas and real gas (F-050, F-061, ADR 0005, ADR 0007)

`fluid.density = "constant"` (default) keeps the incompressible solvers. `fluid.density = "ideal_gas"` selects the
steady pressure-based variable-density SIMPLE/SIMPLEC solver (`rcfd_fv::varden`): density rho = p/(R T), momentum
with the density-weighted mass flux, a total-enthalpy energy equation, laminar. It requires
`solver.algorithm = "simple"` or `"simplec"` (steady) or `"pimple"` (transient, below) and `energy.enabled = true` (the temperature carries the thermal
boundary conditions; `energy.relax` is the enthalpy relaxation, `energy.buoyancy` and the Boussinesq `physics`
settings are ignored: buoyancy follows from the density with `physics.gravity`).

* `fluid.mode`: `compressible` (default; `p` is the absolute pressure, full equation of state, subsonic to
  transonic nozzles, stagnation inlets) or `low_mach` (`p` is the dynamic pressure; the density follows a thermodynamic
  pressure that is uniform and fixed by the mass of a closed domain; large temperature differences).
* `fluid.gas_constant` (287.058), `fluid.gamma` (1.4), `fluid.viscosity` (1.8e-5 Pa s), `fluid.viscosity_model`
  (`constant` | `sutherland` with `sutherland_t_ref` 273 K and `sutherland_s` 110.5 K; `viscosity` is the value at
  `sutherland_t_ref`), `fluid.prandtl` (0.71) and `fluid.conductivity` (0 = mu cp / Pr).
* `fluid.reference_pressure` (101325 Pa; low_mach thermodynamic pressure; in compressible mode the initial pressure
  is `initial.pressure` when positive, else this value), `fluid.density_scheme` (`upwind` | `central` |
  `linear_upwind`), `fluid.relax_density`, `fluid.hydrostatic_init` (start from the hydrostatic state of
  `physics.gravity`).
* Real gas (`fluid.density = "real_gas"`, ADR 0007): the same solver with a cubic equation of state. `fluid.eos`
  (`peng_robinson` default | `redlich_kwong`), `fluid.species` (`N2`, `O2`, `AR`, `CO2`, `CH4`, `H2`, `H2O`; the
  critical temperature, pressure, acentric factor and molar mass come from a built-in table), or leave `species` empty and give
  `fluid.critical_temperature` [K], `fluid.critical_pressure` [Pa], `fluid.molar_mass` [kg/mol] and
  `fluid.acentric_factor` (Peng-Robinson); an explicit non-zero value overrides the table. The gas constant is
  `R_u / molar_mass` (setting `fluid.gas_constant` is an error). `fluid.specific_heat` is the constant ideal-gas
  (zero-pressure) `cp0` [J/kg/K]; 0 uses `gamma R/(gamma-1)` from `fluid.gamma` (set it: the default gamma 1.4 is
  wrong for CO2). Density is the vapour-like root of the cubic, enthalpy is `cp0 T` plus the EOS departure, the
  energy diffusivity uses `k/cp0`, the Mach number uses the real speed of sound, and a stagnation inlet follows the
  real-gas isentrope (`s` and `h + |u|^2/2` conserved). Compressible mode only; no liquid root or phase change
  (a state without a vapour-like root falls back to the ideal gas), `fluid.mode = "low_mach"` and
  `fluid.hydrostatic_init` are errors, `mu` and `k` stay the user inputs (constant or Sutherland). van der Waals
  is not available. All the keys above need `density = "real_gas"`.
* Boundaries: `inlet = "stagnation"` with `total_pressure_value` [Pa] and the total `temperature_value` [K]
  (compressible mode only) replaces the velocity, pressure and temperature settings of the patch (zero-gradient
  velocity, static state from the isentropic relations). Fixed `pressure_value` is absolute in compressible mode
  and dynamic in low_mach mode. A symmetry plane uses `velocity = "symmetry"`.
* Mesh: `mesh.generator = "nozzle_2d"` is the upper half of a converging-diverging duct (inlet half-height `ly`,
  `throat_height` at x = lx/2, `exit_height` at xmax, 0 = `ly`; cosine wall; patches xmin, xmax, ymin = centre line,
  ymax = wall).
* Fields: `rho`, `T`, `mach`, `p_abs` (absolute pressure; `p` is the solver pressure), `ux`, `uy`, `speed`.
  Reports: `mass_flow` (kg/s through the patch of the mesh, positive out), `heat_flow` (W, conduction, positive out),
  averages, probes. Monitors: `res_u`, `res_p`, `res_T`, `continuity`.
* Transient (ADR 0009): `solver.algorithm = "pimple"` advances the variable-density equations in time with
  `solver.dt` and `solver.end_time`, `solver.time_scheme = "euler"` (first order) or `"bdf2"` (second order, Euler
  first step), including the density time derivative in continuity, momentum and energy. Each time step runs
  SIMPLE-inner iterations until the largest of `|dP|/max P`, `|dT|/max T` and `|du|/max(sound speed)` between two
  inner iterations falls below `fluid.inner_tolerance` (1e-8), at most `fluid.inner_iterations` (30, >= 1). Fixed time
  steps only (`solver.max_courant` > 0 is an error); the relaxation factors `solver.alpha_u`, `solver.alpha_p`,
  `energy.relax` apply to the inner iterations (ADR 0009 registers `alpha_p = 1`, `energy.relax = 1` for its tests).
  A closed compressible domain keeps its mass through the equation of state, so no pressure level is pinned.
  `piso` with `fluid.density` is an error ("use pimple"). The monitors `res_T`, `inner_iterations`, `dt` are recorded
  per step; `convergence.*` conditions work as for the other transient runs.
* `energy.viscous_heating` (false): viscous heating in the energy equation, the viscous work `div(tau . u)` of the
  total enthalpy in the compressible mode, the dissipation `tau : grad u` in the low_mach mode; `energy.heat_source`
  (W/m3, 0): a uniform volumetric heat source. Both need `fluid.density = "ideal_gas"` (errors otherwise) and work
  with the steady and the transient solver. With either of them, or in transient runs, an adiabatic wall also removes
  the kinetic-energy diffusion flux through the wall (ADR 0009 section 5); the steady runs without them keep the
  pre-ADR 0009 wall condition so that their results are unchanged.
* `initial.velocity_expression_x`, `_y`, `_z` (m/s, expressions of x, y, z evaluated at the cell centres): a
  non-uniform initial velocity, for example a start that is consistent with an imposed boundary profile
  (`boundaries.<patch>.velocity_expression_*`, evaluated once at the start in variable-density runs). The
  compressible transient solver diverges from a start that contradicts an imposed end profile (ADR 0009 results).
* Diagnosed as errors (with the setting path): piso, coupled and compressible algorithms, turbulence models,
  `rheology`, `properties`, `scalars`/`species`, `sources`, user-function momentum/viscosity, `solver.bulk_velocity`,
  `physics.body_force` (use `physics.gravity`), pseudo-transient modes, periodic meshes, `statistics`,
  `energy.enabled = false`, heat-flux temperature boundaries, stagnation inlets in low_mach mode, `solver.max_courant`
  with pimple, `fluid.inner_*` with a steady algorithm, `energy.viscous_heating` / `energy.heat_source` and any
  `fluid.*` or stagnation setting with `fluid.density = "constant"`.
* Convergence: the momentum residual `res_u` typically floors near 1e-5..1e-6, so the default `tolerance_u` may never be met and the run then stops at `max_iterations`; stop on `convergence.*` conditions (a window on `res_p`, `res_T` or a report) or set `tolerance_u` near the floor. `fluid.*` keys are flagged with `density = "constant"` only when they differ from their defaults.
* Limits: laminar, no non-orthogonal pressure correctors; choked or shock-containing flows need smaller relaxation
  factors and a back-pressure start-up (see ADR 0005 and the heavy nozzle test); no shock-tube or choked transient
  benchmark is claimed (ADR 0009).

## Other solver and physics settings now in the schema

* `physics.body_force` (m/s^2); steady incompressible algorithms only (rejected with piso, pimple and compressible).
* `solver.bulk_velocity` and `solver.bulk_direction`: mean-velocity driver for periodic channels (steady algorithms only).
* `solver.h_operator` = `default` | `from_solution` | `previous_iterate`.
* `solver.p_ref_cell`, `solver.p_ref_value`: reference pressure cell for closed domains.
* `solver.momentum_rel_tol`, `momentum_max_iter`, `pressure_rel_tol`, `pressure_max_iter` (0 keeps
  the algorithm default).
* `solver.ddt_correction` (transient Rhie-Chow time-step correction, default on).
* `convergence.<name>.monitor = "res_T"` (energy residual), `res_nut`, `res_rho` are valid monitors.
* New fields in `report.field` and the GUI field list: `y_plus`, `rho`, `mach`.

Energy (`energy.*`) and pseudo-transient (`solver.pseudo_*`) settings were already in the schema
and are unchanged.

`energy.enabled` also works with the transient `piso` and `pimple` algorithms (F-050): the temperature equation is advanced
with the flow's time scheme (implicit Euler or BDF2, one linear solve per step, `energy.relax` unused) on the flux of the previous step,
then the flow step sees the Boussinesq buoyancy of the new temperature (first-order coupling in time; a steady state is independent of it).
Temperature-dependent `properties`, scalars and species stay steady-only.

## `tables` and profile inputs (F-105)

`tables.<name>` reads a lookup table or profile from CSV (`file`, relative to the case file, or inline
`data`). The first line names the columns, a unit may follow in brackets (`T [degC], nu [m^2/s]`; values are
converted to SI), the delimiter is a comma, semicolon or tab, `#` lines are comments.

| Key | Meaning |
| --- | --- |
| `file` / `data` | CSV file or inline CSV text |
| `inputs` | input (coordinate) columns in argument order, 1 to 3 |
| `outputs` | output columns to keep (empty = all others) |
| `units` | `"column = unit"` entries for columns without a unit in the header; `x y z t ux uy uz T heat_flux` default to `m m m s m/s m/s m/s K W/m^2` |
| `extrapolation` | `error` (default: outside the data is an error) or `clamp` |

Interpolation is linear (1 input) or multilinear on a complete rectilinear grid (2 or 3 inputs, rows in any
order). Rows that do not form a grid are interpolated by inverse-distance weighting (power 2, nearest 6 or 8
points) inside their bounding box. Duplicate inputs, non-numeric or non-finite cells, missing columns, unknown
units, an unreadable file, and (with `extrapolation = "error"`) boundary faces outside the data range are
reported as diagnostics naming the table.

Tables are callable from every expression as `<name>(args)` (first output) or `<name>_<column>(args)`; each
argument must have the dimension of its input column (so inputs need a unit), the result has the dimension of
the output column. `boundaries.<patch>.profile = "<table>"` feeds the table's `ux/uy/uz`, `T` and `heat_flux`
columns into the matching fixed-velocity, fixed-temperature and heat-flux conditions (explicit `*_expression`
settings win); its inputs must be among `x, y, z, t`. Under the service sandbox (`--root`) table files must lie
inside the root. Not implemented: Fluent profile-file syntax, cubic/monotone interpolation, table values that
fall outside the range during a transient run (they evaluate to NaN; only the t = 0 state is checked at build).
Example: `cases/channel/profile_inlet.toml`.

## `statistics` and `output` policy (F-116)

`statistics.enabled`, `start_time` (s) and `fields` (`ux uy uz speed p`) collect the time-weighted mean, rms,
standard deviation, minimum and maximum of transient incompressible runs as fields `<name>_mean` and so on.
`output.write_interval`, `retain_snapshots`, `retain_checkpoints` and `fields` control numbered snapshots, backup
retention and the variables written. Details and limits: `docs/run-control.md` (Output policy and statistics).

## `rheology` viscosity laws (F-110)

`rheology.model` selects `power_law`, `carreau`, `carreau_yasuda`, `cross`, `herschel_bulkley` or `bingham` (default
`newtonian`, which keeps `physics.nu`). The settings carry SI units: dynamic viscosities in Pa s
(`zero_shear_viscosity`, `infinite_shear_viscosity`, `min_viscosity`, `max_viscosity`), `time_constant` and
`regularisation_time` in s, `yield_stress` in Pa, `consistency_index` in Pa s^n (the plastic viscosity for Bingham),
plus the dimensionless `power_index`, `yasuda_exponent`, `cross_exponent`. `rheology.shift = "arrhenius"`, `"wlf"` or `"table"` (`a_T(T)` from `tables.<shift_table>`: input column `T`, output column `a_T`; a temperature outside the table stops the run with a message unless the table clamps)
adds a temperature dependence by time-temperature superposition `mu(g, T) = a_T mu_ref(a_T g)` (needs
`energy.enabled`; with `model = "newtonian"` the reference viscosity is `physics.nu * physics.rho`;
`reference_temperature`, `activation_temperature`, `wlf_c1`, `wlf_c2`). The apparent viscosity follows the shear
rate `sqrt(2 S:S)` of the current velocity, is clamped to `[min_viscosity, max_viscosity]`, divided by `physics.rho`
and used on the faces of the momentum diffusion term (cell values interpolated, relaxed with `rheology.relax`).
Works in steady SIMPLE/SIMPLEC (also with the energy equation) and in transient PISO/PIMPLE runs. Not combined
with turbulence models, the compressible solver or a user-function viscosity (diagnostics). Not implemented: the
transposed-gradient viscous term of a variable viscosity, yield-stress laws with a temperature shift, checkpoints that carry the viscosity history.

## `solver.algorithm = "coupled"` block-coupled solver (F-032)

The steady block-coupled solver solves `(u, v[, w], p)` in one linear system per outer (Picard) iteration
instead of the segregated predictor/corrector of SIMPLE: implicit Green-Gauss pressure gradient in the
momentum rows, continuity rows with an implicit Rhie-Chow face flux, right-preconditioned GMRES with ILU(0) on
the momentum blocks and one AMG V-cycle (aggregation AMG) on the approximate pressure Schur complement.
`solver.alpha_u` is the implicit momentum relaxation (0.95 to 0.98 work on the cavity, where SIMPLEC is
limited to about 0.95); there is no pressure relaxation. `solver.pressure_rel_tol` and
`solver.pressure_max_iter` control the Krylov solve (defaults 1e-3, 400) and `solver.pressure_abs_tol` the absolute residual floor. Scope of the first version, each
diagnosed with a located error: laminar flow in a closed domain with the velocity prescribed on every boundary
(walls, moving walls, fixed inlets; no pressure outlets, no periodic patches), no turbulence model, rheology, user
functions, momentum sources, energy equation, uniform body force, bulk-velocity driver or non-orthogonal
correctors. The pressure level is fixed by `solver.p_ref_cell` / `solver.p_ref_value`.

## `scalars`, `species` and `properties` (F-053)

`scalars.<name>` and `species.<name>` transport extra variables on the flow flux of a steady run
(`simple`, `simplec`, `coupled`; other algorithms are rejected with a located diagnostic):
`div(phi s) - div(D grad s) = S` with constant density, the momentum convection scheme (`solver.convection`) and the
shared scalar routine that also solves the temperature (`rcfd_fv::scalar`). Per entry: `diffusivity` [m^2/s] or
`schmidt` (D = nu/Sc, which follows a temperature-dependent or non-Newtonian face viscosity), `initial`, `source`
(value per second, uniform), `relax` (default 1, the frozen-flux problem is linear), and
`boundaries.<patch>` with `type` = `zero_gradient` (default) | `fixed` | `gradient`, `value` and, for `fixed`, a
dimensionless `value_expression` of x, y, z (F-104). Fields appear as `sim.field("<name>")`, in VTU output
and in `res_<name>` monitors (usable in `convergence`). Reports: `scalar_flux` (`scalar`, optional `patch`) is the net
discrete outflow through a patch, or the conservation imbalance of all boundaries minus the integrated source when no
patch is given; `scalar_average` is the volume average.

`species.<name>` are mass fractions of a mixture: all but one are transported and the balance species
(`remainder = true`, else the last one by name) is closed by difference `Y_N = 1 - sum Y_i`, so the fractions sum to one
to round-off. The balance species takes no transport data and no boundary conditions (it follows from the others). The
initial fractions and the fixed values on a patch must sum to one (diagnosed). Every transported species is conserved
separately: the net boundary outflow (convection plus diffusion, from the same boundary coefficients as the matrix)
vanishes to round-off, and the balance species inherits the flow solver's mass imbalance, which
`solver.pressure_abs_tol` (default tolerance-limited at about 1e-11 of the volume flow) can drive to 1e-16. Not covered: variable density, mixture-averaged or multicomponent
diffusion (a constant diffusivity per species), reactions, and transient scalar transport.

`properties.viscosity` (dynamic, Pa s), `properties.conductivity` and `properties.specific_heat` each take
`law` = `constant` (the `physics` value, default) | `polynomial` (`coefficients` c0, c1, ... of T in kelvin) | `sutherland`
(`reference_value`, `reference_temperature`, `sutherland_constant`; value times (T/T_ref)^1.5 (T_ref+S)/(T+S)). They need
`energy.enabled` and a steady SIMPLE/SIMPLEC run. The viscosity enters through the face viscosity `mu(T_f)/rho` (the path of
`rheology`, with which and with a turbulence model or a user-function viscosity it is rejected); conductivity and
specific heat give the energy-equation diffusivity `k(T_f)/(rho cp(T_f))`, and `heat_flow` reports use the law at the wall
temperature. The density stays constant; no property data is shipped, the constants are inputs (the material library
F-111 provides sourced data).

### Material library (F-111)

`rcfd_core::fluids` and `rcfd_core::iapws` are Rust APIs (no case keys of their own; the `properties` laws above and the F-105 tables
carry their data into a case): NASA-polynomial species (N2, O2, AR, H2O, CO2, CH4, H2), mass/mole-fraction conversion,
mass-weighted and ideal-volume mixing, Wilke's gas-mixture rule, ideal-gas, Redlich-Kwong and Peng-Robinson equations of state
(roots, density, fugacity, residual enthalpy, saturation pressure), and water and steam from IAPWS-IF97 regions 1 and 2 (273.15-1073.15 K,
up to 100 MPa, outside region 3) with the IAPWS viscosity and conductivity correlations. Ready-to-paste `properties` laws for air at
100 kPa, valid 200-600 K (cubic fits to the CRC/NIST gas tables, see `docs/reference-data.md`; Sutherland's law with the textbook air constants
is within 1 % for the conductivity at 300-500 K but not for the viscosity, -0.2 % at 300 K, -1.1 % at 400 K, -1.5 % at 500 K):

```toml
[properties.viscosity]
law = "polynomial"
coefficients = [2.6e-7, 7.532142857142829e-8, -5.571428571428524e-11, 2.4999999999999777e-14]
[properties.conductivity]
law = "polynomial"
coefficients = [-9.4e-4, 1.1165476190476213e-4, -8.071428571428652e-8, 4.16666666666674e-11]
```

## `sources` momentum sources (F-031)

`sources.<name>` adds a momentum source in a cell zone of a steady (`simple`, `simplec`) run; other algorithms are
rejected with a located diagnostic. `zone` is `all` (default), `box` (cells whose centres lie in
`[box_min, box_max]`, metres) or `cell_zone` (a named zone of an imported mesh). A face belongs to the zone when both
its cells do. `type` selects:

| `type` | Keys | Meaning |
| --- | --- | --- |
| `porous` | `model = ergun`: `porosity` (0 < e < 1), `particle_diameter` [m]. `model = darcy_forchheimer`: `permeability` K [m^2], `inertial_resistance` C2 [1/m] | implicit sink `-(nu/K + C2 \|u\|/2) u` per unit mass, **superficial-velocity** formulation (the porosity does not enter the transport terms). Ergun maps to `1/K = 150 (1-e)^2/(e^3 d^2)`, `C2 = 3.5 (1-e)/(e^3 d)`; `\|u\|` is lagged one iteration |
| `rotating_frame` | `rotation_axis` (default +z), `rotation_origin`, `angular_velocity` [rad/s], `centrifugal` (default true) | relative-velocity form: `-2 Omega x u - Omega x (Omega x r)`. The centrifugal part is analytic at cell and face centres, the Coriolis part uses the previous iterate (boundary faces: the boundary velocity). Wall velocities are given in the rotating frame. `centrifugal = false` keeps Coriolis only (needed for periodic domains) |
| `mrf` | `rotation_axis` (default +z), `rotation_origin`, `angular_velocity` [rad/s] | multiple-reference-frame (frozen rotor) zone, **absolute-velocity** formulation (ADR 0008): the solved velocity stays absolute, momentum in the zone is convected with the relative flux `phi - (Omega x r).S` and gets the lagged acceleration `-Omega x u`; boundary conditions are absolute (a wall rotating with the zone is given `Omega x r` by `velocity_expression_x/y/z`). The interface to the rest of the mesh (an ordinary internal face set, also a non-conformal one from `annulus_3d_interface`) carries absolute momentum, which is the frozen-rotor treatment. |
| `acceleration` | `acceleration` [m/s^2] | constant acceleration in the zone |

MRF validation errors (path `sources.<name>...`): zero `rotation_axis`, non-finite `angular_velocity`,
a zone without cells, a zone boundary (walls, symmetry planes, the interface) that is not a surface of revolution about
the rotation axis (the frame flux `(Omega x r).S` must vanish there; planar interfaces perpendicular to the axis and
cylindrical ones qualify), an MRF zone sharing cells with another MRF zone or a `rotating_frame` source, and
transient algorithms. Pseudo-transient time steps use the absolute flux. Not validated: any pump or turbine
benchmark, turbulence models or the energy equation with an MRF zone; the checks are in ADR 0008 and
`crates/rcfd-api/tests/mrf.rs`.

Limits: steady runs only, the viscous porous term uses `physics.nu`; the non-conformal join of F-114 handles planar
interfaces only, so an MRF zone with a non-conformal interface is limited to interfaces perpendicular to the axis.
Examples: `cases/packed_bed/packed_bed.toml`, `cases/rotating_channel/rotating_channel.toml`.

## `udf` user functions (F-122)

`udf.<name>` loads a sandboxed WebAssembly module (`file`, `fuel_per_call`) and `momentum_source`,
`viscosity` and `monitor` select the roles it plays; `boundaries.<patch>.velocity_udf` names a module whose
`velocity_profile` export supplies a fixed velocity. The module may import nothing. The ABI, limits and failure
behaviour are in `docs/udf-abi.md`.

## Caveats

* Only the standard wall-function formulation is implemented (no scalable or enhanced wall
  treatment, no rough walls).
* k-epsilon is steady-only and incompressible, without heat transfer.
* k-omega SST: same limits. Its skin friction and channel velocity depend on the first-cell y+
  (flat plate Cf rises as y+ falls: -10.5 percent against White's formula at y+ 2 down to
  -4.8 percent at y+ 0.1; channel bulk velocity +5.4 percent at y+ 2 to +1.4 percent at 0.25);
  the validated accuracy statements are in the roadmap note of F-043.
* The compressible solver is inviscid and explicit; there is no implicit option.
