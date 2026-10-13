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

Boundaries: `compressible_bc` = `auto` | `slip` | `extrapolate` | `fixed`. `fixed` needs
`density_value`, `pressure_value` (positive) and optionally `velocity_value` (`temperature_value`
may replace `density_value`). `auto` keeps the solver default by mesh patch kind: outlet patches extrapolate, all others are slip walls. The mesh generator `wedge_2d` (`ramp_start`, `ramp_angle`, plus `lx`, `ly`, `nx`,
`ny`) builds the compression-ramp domain.

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

## Caveats

* Only the standard wall-function formulation is implemented (no scalable or enhanced wall
  treatment, no rough walls).
* k-epsilon is steady-only and incompressible, without heat transfer.
* k-omega SST: same limits. Its skin friction and channel velocity depend on the first-cell y+
  (flat plate Cf rises as y+ falls: -10.5 percent against White's formula at y+ 2 down to
  -4.8 percent at y+ 0.1; channel bulk velocity +5.4 percent at y+ 2 to +1.4 percent at 0.25);
  the validated accuracy statements are in the roadmap note of F-043.
* The compressible solver is inviscid and explicit; there is no implicit option.
