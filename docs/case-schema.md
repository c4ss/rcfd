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
| `model` | `laminar` | `laminar`, `k_epsilon`, `k_omega_sst` (Menter 2003), `spalart_allmaras` (SA-noft2) or `plugin` (see below) |
| `variant` | `standard` | `standard`, `rng`, `realizable` (k-epsilon only; rejected for SST) |
| `wall_treatment` | `smooth` | k-epsilon wall cells: `smooth` (compound viscous/log epsilon and blended production, no switch), `hysteresis` (+-10 percent dead band on the y* = 11.225 switch) or `switch` (original hard switch; its nu_t never settles, see docs/bfs-convergence-findings.md) |
| `wall_function` | `standard` | k-epsilon wall treatment: `standard` (log-law wall functions), `scalable` (y* limited below the viscous/log intersection) or `enhanced` (two-layer near-wall scales plus Kader's blended law, wall-resolved meshes); k_epsilon only. Library options (rcfd-turb); the build path does not apply non-default values yet, they are rejected with a located error (ADR 0015) |
| `roughness_height`, `roughness_constant` | 0, 0 | sand-grain roughness k_s (0: smooth) and constant C_s (0: Nikuradse value derived from `kappa`, `e_wall`); k_epsilon (standard or scalable) and k_omega_sst (`wall_omega = automatic`) |
| `wall_omega` | `automatic` | k_omega_sst: `automatic` or `low_re` (k = 0 on the wall, wall-resolved) |
| `sst_variant` | `sst` | k_omega_sst: `sst` (Menter 2003) or `bsl` (baseline) |
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
energy), except that a fixed wall may move: the wall law uses the cell velocity relative to the wall velocity (ADR 0075), so the
stationary casing of a rotor solved in the rotating frame is allowed (`turbulence = "wall"` must then be set on that patch, because the
automatic role calls a moving fixed-velocity patch an inlet). Wall treatment: automatic. A Spalding (1961) law of the wall gives the wall shear for
any first-cell y+, `omega` on the wall faces is `10 * 6 nu / (beta1 y^2)` (y = first cell-centre
distance, the TMR recommendation), and for y+ > 5 the cell omega follows the blend of the viscous
and the log-layer value. Wall-resolved meshes (y+ about 1) and wall-function-like meshes (y+ 30
to 300) both work; y+ between about 3 and 20 is the least accurate range and
`validate_case` of the assistant warns with a y+ estimate. Convection of k and omega uses
`turbulence.convection` (`upwind` is the safe choice; `linearUpwind` diverged in tests). `kappa`
and `e_wall` define the Spalding law. Boundary values: `turbulence_spec = values` takes
`k_value` and `omega_value`; the other inlet specifications derive `omega = epsilon/(beta* k)`.
Outputs add the field `omega`.

### `spalart_allmaras`

Spalart-Allmaras one-equation model, SA-noft2 form of the NASA Turbulence Modeling Resource page
(https://tmbwg.github.io/turbmodels/spalart.html, ADR 0014): working variable `nuTilda`, `nut = nuTilda fv1`,
coefficients sigma 2/3, cb1 0.1355, cb2 0.622, kappa 0.41, cw2 0.3, cw3 2, cv1 7.1 (`cw1` derived). Same restrictions as
the other RANS models (steady algorithms, a wall patch, no energy). Extra keys:

| Key | Default | Meaning |
| --- | --- | --- |
| `nutilde_ratio` | 3 | `nuTilda / nu` of the initial field and of inlets without `nutilde_value` (TMR: 3 to 5) |
| `boundaries.<patch>.nutilde_value` | 0 | inlet `nuTilda` in m^2/s (0: use `nutilde_ratio`); `turbulence_spec` and the k/epsilon/omega values are not used |
| `sa.wall_mode` | `auto` | `auto`: Spalding law of the wall for the wall shear, plus a wall-cell `nuTilda` matching the law for wall-cell y+ > 5 (works from y+ about 1 into the log layer); `resolved`: `nuTilda = 0` on the wall and no wall function (y+ of order 1) |
| `sa.limiter` | `allmaras` | `S~` limiter: `allmaras` (Allmaras, Johnson and Spalart 2012, c2 0.7, c3 0.9), `min_0p3` (`S~ >= 0.3 Omega`), `positive` |
| `sa.negative_nutilde` | true | SA-neg treatment of negative `nuTilda` (production `cb1 Omega nuTilda`, destruction `+cw1 (nuTilda/d)^2`, diffusion `nu + fn nuTilda`, `nut = 0`); off: clip at zero |
| `sa.des` | `off` | `off`: RANS length scale `d` (wall distance). `des`: detached-eddy simulation, `d~ = min(d, C_DES Delta)`. `ddes`: delayed DES, `d~ = d - f_d max(0, d - C_DES Delta)`, `f_d = 1 - tanh((8 r_d)^3)`, `r_d = (nu + nut)/(max(sqrt(U_ij U_ij), 1e-10) kappa^2 d^2)`. `des` and `ddes` run with the transient algorithms (`piso`, `pimple`) only and add an implicit-Euler time derivative on `nuTilda` (ADR 0032) |
| `sa.des_delta` | `max_extent` | Grid scale `Delta`: `max_extent` (largest distance between two face centres of the cell, empty directions excluded: `max(dx, dy, dz)` on boxes) or `cbrt_volume` (`V^(1/3)`) |
| `sa.c_des` | 0.65 | `C_DES`, range 0.1 to 2 (value from secondary literature, see `docs/reference-data.md`) |

`convection`, `relax` and `relax_nut` apply as for the other models; the fields `nuTilda`, `nut`, `y_plus` and
`wall_distance` are available. `k_initial`, `epsilon_initial`, `omega_initial`, `variant` and `wall_treatment` are
rejected for this model with a located diagnostic. Validation (ADR 0014): flat plate Cf(x = 0.97) +3.0 percent from the
TMR CFL3D SA value (y+ 0.5 and y+ 30 meshes: +3.0 and +5.8 percent).

The `sa.des*` keys are errors (located at the key) with any other model, and `sa.des != "off"` with a steady algorithm is a
located error at `turbulence.sa.des`. Plain SA (`sa.des = "off"`) stays steady-only. With the DES length scale the
model is a hybrid RANS-LES closure on the Spalart-Allmaras equation only: the SA-based DES and DDES of ADR 0032 are not
validated on a separated flow (no case in this version), IDDES, SAS, the Strelets variants and the DES-SST models are not
available. In a live session `turbulence.model` cannot be switched while `turbulence.sa.*` differ from their defaults;
`turbulence.sa.des` itself can be switched (`des` to `ddes`) and keeps the flow field.

### `smagorinsky`, `wale` and `mixing_length` (F-045, F-206, ADR 0026)

Algebraic eddy-viscosity models: no transported scalar, `nut` is computed from the velocity gradient (Green-Gauss) of the
current field. Fields: `nut` (and `wall_distance`, `y_plus` where a wall exists).

| Model | Algorithms | `nut` |
| --- | --- | --- |
| `smagorinsky` | `piso`, `pimple` only (large-eddy subgrid model; a steady algorithm is a located error at `turbulence.model`) | `(Cs Delta f)^2 sqrt(2 S_ij S_ij)`, `Delta = V^(1/3)` (cell volume), `f` the optional van Driest damping |
| `wale` | `piso`, `pimple` only | Nicoud-Ducros wall-adapting local eddy viscosity `(Cw Delta)^2 (Sd:Sd)^(3/2) / ((S:S)^(5/2) + (Sd:Sd)^(5/4))`; 0 where the denominator is 0; no damping option (it scales as `y^3` at a wall by construction) |
| `mixing_length` | `simple`, `simplec`, `piso`, `pimple` | `l^2 sqrt(2 S_ij S_ij)`; steady runs relax `nut` with `turbulence.relax_nut` (Picard), transient runs evaluate it explicitly each step |

Keys: `turbulence.wall_damping` (`none` | `van_driest`; Smagorinsky and mixing length only; needs a wall patch;
`f = 1 - exp(-y+/A+)`, y+ from the nearest wall face, wall shear from the first cell); `turbulence.length_scale` (m;
required and positive for `mixing_length`, the half height of a channel or the radius of a pipe; a located error at
`turbulence.length_scale` for any other model); `turbulence.mixing_profile` (`linear`: `l = min(kappa d f, l_max_ratio R)`
| `nikuradse`: `l = R (0.14 - 0.08 (1 - d/R)^2 - 0.06 (1 - d/R)^4) f`, the pipe formula; used for a channel it is an analogy).
Coefficients in `turbulence.coefficients.smagorinsky` (`cs` 0.17 in [0, 0.5], `a_plus` 25), `.wale` (`cw` 0.325 in [0, 1])
and `.mixing_length` (`kappa` 0.41, `l_max_ratio` 0.09, `a_plus` 26). The wall faces carry `nut = 0` (wall resolved; no wall
function). Subgrid models need no wall patch unless `wall_damping = van_driest`. They switch live like the other models
(`settings.set turbulence.model`; the flow is kept). Restrictions: no energy equation, no compressible or variable-density
engine; `k_initial`, `epsilon_initial` and the other transported-model keys are rejected. **Known limitations:** the
subgrid-stress divergence is discretised in Laplacian form `div(nu_eff grad u)` as for the RANS models (the transposed part
`div(nu_t (grad u)^T)` is missing by default, which changes the dissipation of a variable `nu_t`, ADR 0026 addendum A1; the
option `solver.viscous_stress = "full"` adds it, see below); no SGS heat or
scalar flux; DES-class hybrid models other than the SA-based DES / DDES below, dynamic Smagorinsky, Vreman, sigma, wall-modelled LES, Pr_t/Sc_t are not available.
Validation: closed-form checks and a Taylor-Green vortex (ADR 0026); mixing length on the periodic Re_tau 395 channel
within 4 percent of the DNS mean velocity (linear, van Driest) and 3 percent (Nikuradse analogy). Decaying isotropic
turbulence and pipe data were not run.

### `solver.viscous_stress`, SA-based DES, `rsm` (F-045, F-207, F-046, ADR 0032)

`solver.viscous_stress` (`laplacian` default | `full`): `laplacian` is the momentum diffusion `div(nu_eff grad u)` of every
earlier version (results bit-identical). `full` adds the transposed part of the viscous stress, `div(nu_eff (grad u)^T)`, as an
explicit source from the previous iterate (the part that is zero for a constant viscosity and divergence-free velocity, so the
laminar results change by the discretisation error only: 6e-6 in the tree test). It exists for the variable-viscosity closures
(`smagorinsky`, `wale`, `mixing_length`, `rsm`'s eddy viscosity, rheology laws). `full` is available with `piso` and `pimple`,
constant density; a steady algorithm or variable density is a located error at `solver.viscous_stress`. Translationally periodic
patches are included; non-periodic boundary faces and rotationally periodic faces contribute nothing to the explicit term (the
boundary value of `grad u` is not reconstructed), so near a wall the term is incomplete.

`turbulence.model = "rsm"` is a Reynolds stress transport model: six stress equations and the dissipation rate. Keys:
`turbulence.rsm.pressure_strain` (`ssg` default, Speziale-Sarkar-Gatski quadratic with `Ce2` 1.83 | `lrr_ip`, Launder-Reece-Rodi
isotropisation of production with `Ce2` 1.92); coefficients in `turbulence.coefficients.rsm` (`c1`, `c1s`, `c2`, `c3`, `c3s`, `c4`,
`c5`, `c1_lrr`, `c2_lrr`, `ce1`, `ce2_ssg`, `ce2_lrr`, `c_s`, `c_eps`, `c_mu`, with ranges); `turbulence.k_initial` and
`turbulence.epsilon_initial` (or `initial_intensity` and `initial_viscosity_ratio`) set the isotropic start `R_ij = 2/3 k delta_ij`.
Fields `Rxx`, `Ryy`, `Rzz`, `Rxy`, `Rxz`, `Ryz`, `epsilon`, `k` (= trace R / 2) and `nut` (= `c_mu k^2/eps`, added to the molecular
viscosity in the momentum equation). The divergence of the Reynolds stress enters the momentum equation as an explicit body
acceleration `-div R` minus the implicit eddy-viscosity Laplacian already in `nu_eff`, so the net effect is the stress itself.
Stress diffusion is generalised-gradient with the normal-stress part only (`nu + c_s (k/eps) n.R.n` through each face; the
off-diagonal `R_kl` terms are not implemented). Restrictions (located errors): transient algorithms (`piso`, `pimple`) only;
**no wall patch** (no wall treatment, no low-Reynolds terms, no wall reflection: error at `boundaries.<patch>.turbulence`) and no
`inlet` turbulence role (no stress inlet condition); no energy equation; `turbulence.rsm.*` with another model is an error. Valid
configurations are therefore fully periodic boxes (the only configuration tested); other wall-free boundaries are not validated. Validation (ADR 0032): term invariants, the
homogeneous-shear and isotropic-decay ODEs, the semi-implicit update and the momentum source to 1e-8 or better; the wall-bounded
channel check (C7) was **not delivered**. The decay exponent `1/(Ce2 - 1)` is a model statement, no comparison with
experiment was made. `plugin` with `turbulence.plugin = "rsm"` selects the same model.

### Model coefficients, plugin models and live switching (F-044)

`turbulence.coefficients.<model>.<name>` lists the coefficients of every model with their documentation and ranges in the
exported JSON schema: `k_epsilon` (`c_mu`, `c1`, `c2`, `sigma_k`, `sigma_e`; 0 keeps the value of the selected `variant`),
`k_omega_sst` (the eleven Menter 2003 values; **shown, not editable**: a non-default value is rejected because the
constants are compiled into the model), `sa` (`sigma`, `cb1`, `cb2`, `kappa`, `cw2`, `cw3`, `cv1`, `cn1`, `c2`, `c3`),
`smagorinsky`, `wale`, `mixing_length` and `rsm` (above). A
value outside its range is rejected when it is set; a non-default value in the group of a model that is not selected, or
`k_epsilon.c1` with `variant = "realizable"`, is a validation error at the coefficient's path. Not available: overriding
a coefficient by an expression or a user function.

`turbulence.realizable_a0` (float, default 0): the constant A0 of the Realizable variable `C_mu = 1/(A0 + A_s U* k/eps)`. 0 keeps the default 4.04, a commonly cited commercial-code variant; the original report (Shih et al., NASA TM-106721, checked against the report text) prints 4.0. Allowed 3 to 5; `turbulence.model = "k_epsilon"` with `variant = "realizable"` only, otherwise a located error at `turbulence.realizable_a0`. It is a separate key because the `k_epsilon` coefficient table is fixed at five entries (ADR 0014 K4). The default is unchanged and bit-identical; ADR 0017 records the effect of 4.0 on the step reattachment.

`turbulence.model = "plugin"` with `turbulence.plugin = "<key>"` selects a model registered by the embedding program with
`rcfd_turb::register_model` (trait `rcfd_turb::TurbulenceModel`); an unregistered key is a located error at
`turbulence.plugin`. The plugin's coefficients are set in `turbulence.plugin_coefficients.<name>.value`.

**Live switching.** In a service session that holds a built simulation in the `ready` or `paused` state, `settings.set`
on any `turbulence.*` path (for example `turbulence.model` from `k_epsilon` to `k_omega_sst` or `spalart_allmaras`, or a
coefficient) rebuilds only the turbulence state: velocity, pressure, flux, iteration counter and monitors are kept, the
new model starts from the current eddy viscosity, and the next iteration uses it. Errors (unregistered plugin, invalid or
non-editable coefficient, a case the model cannot run) are returned with their path and leave the run unchanged. Switching
from or to `laminar` needs a rebuild (other settings still reset the simulation as before). The session's
`run.status` reports `simulation.turbulence.key`.

### Mesh generator parameters

`cavity_2d` is an `nx` x `ny` uniform grid on `lx` x `ly` (patches `lid` = ymax, `walls`, `frontAndBack`); it is
square only when `nx = ny` and `lx = ly`. Before wave 11 it silently built an `nx` x `nx` cavity of side `lx`.
Options a generator does not apply are rejected with a located diagnostic instead of being ignored:
`mesh.wall_layers` (`box_2d` and `channel_2d` only), `mesh.periodic_x` / `periodic_y` (`box_2d` only) and
`mesh.grading != 1` (not for `cavity_2d`, `wedge_2d`, `nozzle_2d`).

The mesh quality report returned by `rcfd check-mesh`, `mesh.check` and `run --check-only` has an extended-measure summary and a
poor-mesh list since wave 14 (F-233); see `docs/mesh-quality.md`.

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

`annulus_3d_stage` (ADR 0059): the same two blocks WITHOUT any connection: one mesh, one point list, no shared face or
point. Patches `inner`, `outer` (both blocks), `zmin` (inlet of the lower block), `plane_out` (top of the lower block),
`plane_in` (bottom of the upper block), `zmax`. The upper block has `nx_right` x `ny_right` x `nz_right` cells (0 = same as
`nx`, `ny`, `nz`), so the numbers of cells around may differ. The plane is a pair of boundary patches coupled by the
`mixing_plane` section (below), not by the F-114 join.

### Swept pipe (`mesh.generator = "pipe_sweep"`, F-235)

An all-hex circular pipe of radius `r_out` and length `lz` along z: an `nx` x `nx` Cartesian core (half-width
`core_fraction * r_out`, default 0.5, range 0.2-0.8) inside an O-grid of `ny` rings, swept in `nz` axial layers
(`nx` odd is typical; `nx` = core cells per side). `mesh.grading != 1` grades the rings toward the wall. Patches: `inlet`
(z = 0), `outlet` (z = lz), `wall`; with `mesh.periodic_z = true` the end patches become a cyclic pair for fully developed flow
(use `physics.body_force`). The cross-section is an inscribed polygon of polygon, see `docs/meshing.md`
for the discrete volume and area deficit. Use `solver.non_orthogonal_correctors = 2`: the core-to-ring corners reach
about 35 degrees of non-orthogonality. With the default SIMPLEC the corrector diverges on this mesh for nz >= 4 (axial
mode); set `solver.simplec_non_orthogonal_compensation = true` (below) to converge it (ADR 0050).

```toml
[mesh]
generator = "pipe_sweep"
nx = 17
ny = 12
nz = 8
r_out = 1.0
lz = 1.0
core_fraction = 0.5
periodic_z = true
```

`mesh.periodic_z` and `mesh.core_fraction` on any other generator are rejected with a located error.

### Axial-fan periodic sector (`mesh.generator = "axial_fan_sector"`, F-133 partial, ADR 0065)

One blade passage of an axial fan as an all-hex periodic sector of angle `2 pi / n_blades`, settings in the nested section
`mesh.fan` (hub and tip station with linear variation; angles in degrees; every key has a default, the defaults describe the
five-bladed test fan of the ADR). Geometry keys: `n_blades`, `r_hub`, `r_tip`, `tip_clearance`, `axial_extent`, `z_le_root`,
`family` (`naca4` | `circular_arc`), `hub_` and `tip_` `chord`, `stagger`, `camber`, `thickness`, `sweep`, `lean`, and `camber_pos`.
Optional station lists (ADR 0075): `station_radius` (first value `r_hub`, last `r_tip`, strictly increasing) with `station_chord`,
`station_stagger` (deg), `station_camber`, `station_thickness` of the same length and the optional `station_camber_pos`, `station_sweep`,
`station_lean` (empty = `camber_pos`, 0, 0); when `station_radius` is not empty they replace the `hub_`/`tip_` values and the blade varies
linearly between neighbouring stations. Unequal lengths are located errors on `mesh.fan.station_<name>`, station errors of the library on
`mesh.fan.station_<name>[i]`.
Mesh keys: `n_up`, `n_chord`, `n_down` (axial cells), `n_side` (pitchwise cells from the seam to the blade), `n_mid` (across the blunt
edge faces), `n_span`, `n_gap` (radial cells in the blade span and the tip gap), `n_bl` and `growth` (boundary-layer cells of exact
geometric growth on every wall), `first_cell_height` (or, when 0, `y_plus`, `ref_speed`, `nu` for a flat-plate estimate),
`trunc_le`, `trunc_te` (chord fractions cut off by the flat blunt edge faces), `include_blade` (false = empty passage).
Patches: `inlet` (z = 0), `outlet`, `hub`, `shroud`, `blade`, and the cyclic rotational pair `seam_lo` / `seam_hi` (no boundary
entry is needed for the pair); cell zone `rotor` (all cells). Errors in the geometry or the mesh controls carry the path
`mesh.fan.<name>` (for example `mesh.fan.stations[1].chord`). The absolute-formulation `mrf` source is refused on this mesh (blade
and seam faces are not surfaces of revolution); use `rotating_frame`. Lower-fidelity mesh with blunt blade edges that misses the
registered quality gates; NOT validated for fan performance prediction (docs/meshing.md). Example:
`cases/axial_fan_sector/axial_fan_sector.toml`.

`mesh.fan.topology` (`h_slit`, the default, or `c_grid`, ADR 0068) selects the passage mesh. `c_grid` is a body-fitted C-grid that keeps the true
blade edges (rounded leading edge, closed trailing edge) and has exact boundary-layer stacks along the blade normal; it uses the keys above
except `n_mid`, `trunc_le` and `trunc_te` (ignored), and adds `n_nose` (cells from the leading edge to each seam corner; `n_nose + 2 <= n_chord`), `n_core` (cells across the blade thickness in the tip gap) and `front_gap` (distance of the front edge of the leading-edge fan
ahead of the leading edge, in local chords). Unfittable controls give located errors on `mesh.fan.<name>` (a folded leading-edge fan reports
`mesh.fan.front_gap`; a tangled cell family that the fold check does not see reports `mesh.fan.mesh` with the block, indices and layer).
The generator is not robust against arbitrary control changes: most neighbouring combinations of `n_nose`, `n_side` and `n_chord` fold
(docs/meshing.md). Example: `cases/axial_fan_cgrid/axial_fan_cgrid.toml`.
`mesh.fan.c_grid_construction` (`pursuit`, the default, the construction above, `smoothed`, ADR 0072, `camber_seam`, ADR 0079, or
`camber_seam_h`, ADR 0083) selects how
the `c_grid` lines are placed: `smoothed` relaxes every layer with an elliptic (Winslow) smoother whose seams are periodic interior lines, takes the far
grid of the thin end-wall layers from a remapped radius so it does not rotate between them, and falls back through a fixed chain of
weaker settings when a layer would fold; the blade stacks stay exact. `camber_seam` is `smoothed` with seams that follow the camber line
inside the chord and the wake downstream (for high-solidity hubs whose wake leaves straight seams, as FAN-01), a tip-gap first height fitted
to the gap, a radial filter of the far grid between layers and two Laplace-start fallback steps; it refuses controls outside its documented
range with a located error (`0.15 <= n_nose / n_chord <= 0.375`, `max(8, n_bl + 6) <= n_side <= 2 n_chord`, `0.15 <= front_gap <= 1.0`).
`camber_seam_h` is `camber_seam` with a second local untangling after the in-plane smoothing and blade wall stacks whose wall-normal (not in-plane)
thickness is `h1 g^q` on a leaning blade; its documented range is narrower (`0.15 <= n_nose / n_chord <= 0.3`,
`max(8, n_bl + 6) <= n_side <= min(2 n_chord, 16)`, `0.15 <= front_gap <= 0.6`), a restriction to where FAN-01 builds, not a fix.
Measured quality and robustness are in docs/meshing.md.

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
Wave 26C report types (ADR 0048, F-238; `field` is any field the existing averages accept, `patch` a boundary patch,
unknown patches are located errors at build time): `volume_std` is the volume-weighted standard deviation of a cell field
(population form `sqrt(sum w (x - mean)^2 / sum w)`, two-pass with a mean-correction term so a large offset does not cancel);
`area_std`, `area_min` and `area_max` are the area-weighted standard deviation and the smallest / largest face value of a boundary
field over `patch`; `mass_weighted_average` is `sum phi q / sum phi` over `patch` (the signed face flux is the weight, so the
value is the same on an inflow and an outflow patch; a patch whose net flux cancels is an error); `vector_flux` is the net
momentum flux `sum phi (u . d)` through `patch` along the unit vector of `direction` (unit N; incompressible fluxes are scaled by
`physics.rho`, the compressible flux is already a mass flux; not available with the variable-density solver). Surfaces (plane,
sphere, cylinder, cone and general quadric cuts, iso-clip, offset, rotational / translated instances), rakes, point clouds and
case difference fields are library objects (`rcfd_post::surfaces`, `rcfd_post::compare`) without case-tree keys.

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
* `solver.component_diagonal` = `averaged` (default) | `corrected` (ADR 0010): `corrected` makes the H operator consistent with the component-averaged diagonal where a symmetry plane (or any component-specific condition) gives the velocity components different diagonals; applies to the segregated steady/transient constant-density kernels (the transient `piso`/`pimple` kernel only since ADR 0074: before, the key was silently ignored there) and to the variable-density kernel; MRF zones always use it. Wall-resolved cells next to a symmetry plane (half channels, symmetry run-ups) need it: with `averaged` such a plane acts like a no-slip wall (ADR 0074 part A); `docs/component-diagonal-default.md` lists what a `corrected` default would change. It changes results of cases with symmetry planes (e.g. `cases/packed_bed` converges in 16 instead of stalling at 1700 iterations), so it is opt-in.
* `solver.p_ref_cell`, `solver.p_ref_value`: reference pressure cell for closed domains.
* `solver.momentum_rel_tol`, `momentum_max_iter`, `pressure_rel_tol`, `pressure_max_iter` (0 keeps
  the algorithm default).
* `solver.ddt_correction` (transient Rhie-Chow time-step correction, default on).
* `solver.time_advance` = `iterative` (default, the PISO corrector loop) | `fractional_step` (F-210, ADR 0037): the non-iterative
  projection advance of the transient `piso` algorithm, one momentum predictor and ONE pressure Poisson solve per step.
  `solver.fractional_form` = `incremental` (default; predictor with the old pressure gradient, increment Poisson problem) |
  `chorin` (no pressure in the predictor, full pressure from the projection; a first-order steady state);
  `solver.fractional_boundary` = `kim_moin` (default; boundary velocity of the intermediate field `u_b + dt grad(q)_t` with the
  lagged pressure (increment) q, after Kim and Moin 1985) | `plain`. Euler and `bdf2` time schemes, central and upwind-type
  convection. Located errors at `solver.time_advance` for `pimple`, steady algorithms, `energy`, turbulence, `rheology`,
  `base_body`, `dynamic_mesh`, `solver.viscous_stress = "full"` and `time_scheme = "bdf2_bounded"`; `physics.body_force` is refused
  for `piso` as before. A fractional-step run is not restartable (`supports_restart()` is false; the lagged pressure increment is
  not in the checkpoint). The default `iterative` leaves every existing run bit-identical (ADR 0037 F13).
* `convergence.<name>.monitor = "res_T"` (energy residual), `res_nut`, `res_rho` are valid monitors.
* New fields in `report.field` and the GUI field list: `y_plus`, `rho`, `mach`.

Energy (`energy.*`) and pseudo-transient (`solver.pseudo_*`) settings were already in the schema
and are unchanged.

`heat_flow` reports (constant-density energy equation): `reports.<n>.conductivity` is optional and has no effective default (0 = unset). Unset, the report uses `physics.conductivity`; a positive value overrides it for that report; a temperature-dependent `properties.conductivity` law takes precedence over both and is evaluated at the wall temperature. Before this fix the key defaulted to 1.0 and silently replaced `physics.conductivity` (wave 15A set-up defect E1/E5, ADR 0019); cases that relied on that now need an explicit report value to get the old number. The variable-density solver (`fluid.density = "ideal_gas"` or `"real_gas"`) does not use this key (its heat flux is built from `fluid.conductivity` or the Prandtl number); a positive `reports.<n>.conductivity` on a `heat_flow` report there is a located error at `reports.<n>.conductivity` instead of being ignored silently (ADR 0021 item W). `0` or absent is accepted.

`energy.enabled` also works with the transient `piso` and `pimple` algorithms (F-050): the temperature equation is advanced
with the flow's time scheme (implicit Euler or BDF2, one linear solve per step, `energy.relax` unused) on the flux of the previous step,
then the flow step sees the Boussinesq buoyancy of the new temperature (first-order coupling in time; a steady state is independent of it).
Temperature-dependent `properties`, scalars and species stay steady-only.

## Wave 13: schemes, wall motion, solution limits (F-211, F-204, F-208, ADR 0016)

**Convection schemes** (`solver.convection`): `blended`
(`solver.blend_factor` in 0..1, default 0.75: face value `upwind + f (central - upwind)`; 0 is `upwind`, 1 is `central`),
`high_resolution` (Gamma scheme, `solver.high_resolution_beta` in 0.1..0.5, default 0.2), `compressive`
(`solver.compressive_k` in 0..2, default 1.5; k = 0 is upwind, k = 1 minmod-like, k = 2 the sharpest member) and
`bounded_central` (normalised-variable form of central differencing; identical to the `tvd_minmod` limiter). The limited schemes
are deferred-correction schemes: a steady run converges only if the corrections settle, and the compressive members can stall in a limit
cycle of the limiter switching (ADR 0016 results). **Time scheme** `solver.time_scheme = "bdf2_bounded"`: per cell, BDF2 blended
towards implicit Euler by the ratio of the last two backward difference quotients, so a temporal extremum or an accelerating front falls back to the monotone Euler (project-defined, no verified source, ADR 0016 D1).

**Velocity kinds** of `boundaries.<patch>.velocity` (all impermeable, all `turbulence` role `wall`; the turbulence wall treatments
themselves assume a wall at rest, so a turbulence model together with one of these kinds is a located error; exception: `moving_wall`
with `k_omega_sst`, whose wall law uses the velocity relative to the wall (ADR 0075 S1, ADR 0078 S2; give the wall `turbulence = "wall"`)):
* `moving_wall`: wall velocity `velocity_value` (must be tangential on every face, located error otherwise) plus an optional rotation
  `wall_rotation_rate` [rad/s] about `wall_rotation_axis` (default z) through `wall_rotation_origin`; `u_w = u_t + Omega x (x - x0)`.
  `wall_velocity_projection = "tangential"` (default `"none"`) replaces the velocity of each face by its tangential part instead of
  rejecting it: a wall of revolution that rotates in itself is represented by flat or warped facets whose normals are not exactly
  perpendicular to `Omega x r`, so a prescribed `u_w . S_f` would carry fluid through every facet (ADR 0072: up to 1 % of the wall speed
  on the C-grid fan shroud). Valid only for a wall invariant under its own motion (for example the stationary shroud of a rotating-frame
  passage, `wall_rotation_rate = -Omega`); wrong where the wall's normal motion is real.
* `shear`: tangential traction `shear_stress` [Pa] exerted on the fluid, `mu du_t/dn_out = tau` with `mu = physics.rho * physics.nu`.
* `partial_slip`: Navier slip with `slip_length` [m] (0 is no-slip, large is symmetry) relative to the wall velocity of the
  `moving_wall` keys; alternatively `specularity` in [0, 1) with `mean_free_path` (Maxwell, `l = lambda (1 + s)/(1 - s)`; 1 is a located error).
* `marangoni`: `marangoni_coefficient` = d sigma/d T times the tangential temperature gradient of the boundary cell as a shear stress,
  refreshed every iteration / step; needs `energy.enabled` and one of simple, simplec, piso, pimple.
`shear`, `partial_slip` and `marangoni` behave like a symmetry plane in the momentum matrices: use `solver.component_diagonal = "corrected"`
with the steady algorithms, otherwise the iteration stalls (ADR 0010; reproduced for the existing `symmetry` kind in ADR 0016).

**Solution limits** `limits.temperature.{enabled, min, max}` and `limits.scalar.{enabled, min, max}` (off by default): after every
iteration or step, cells outside the band are clipped to it (temperature needs `energy.enabled`; scalar covers every transported scalar
and species). Clipping breaks conservation locally; the number of clipped cells so far is `clipped_cells` in the run status.

**Wall options** (`turbulence.wall_function`, `wall_omega`, `sst_variant`, `roughness_height`, `roughness_constant`, ADR 0015): the
build path now applies them to the model it creates (`TurbulenceModel::apply_wall_options`), with the equality check of ADR 0015 C5;
they are no longer rejected. Plugin models keep their own settings.

Not in the settings tree: the gradient options of F-208 (face-gradient correction, secondary-gradient limiter, boundary treatments) exist in
`rcfd-fv` (`diffusion::GradientOptions`, `laplacian_with`) and are exercised by the library tests only.

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

## SIMPLEC non-orthogonal compensation (ADR 0050 Part 1)

`solver.simplec_non_orthogonal_compensation` (bool, default `false`): with `solver.algorithm = "simplec"` and
`solver.non_orthogonal_correctors > 0` the SIMPLEC flux compensation `(rAtU - rAU)_f snGrad(p_old)` also carries the deferred
non-orthogonal secondary-gradient part of the pressure flux. Without it the corrector feeds back with a gain close to
`alpha_u` and an axial mode of a swept mesh grows (diverged at `alpha_u = 0.9`, converged at 0.8 and below). `false`
keeps every existing result bit for bit; the switch is inert without correctors and changes nothing beyond round-off on an
orthogonal mesh. Not available for the variable-density SIMPLEC (it has no non-orthogonal correctors).

### SIMPLEC on uniformly sheared meshes (ADR 0081 part X)

On a channel whose cells are uniformly sheared by an angle theta (every face non-orthogonal by theta), SIMPLEC with `alpha_u = 0.9`, `alpha_p = 1.0` and no
non-orthogonal correctors overshoots from 50 degrees and diverges from the impulsive start from 55 degrees, with `upwind` and `linear_upwind`, at cell Peclet
numbers of 10 and 0.2, and also on a doubly periodic box without walls or open boundaries (from 60 degrees). It is a start-up divergence: the converged state
at 60 degrees is stable under the same iteration (perturbations decay by a factor of 0.89 to 0.98 per iteration). Measured thresholds on that channel:
`alpha_p` 0.7: 60 degrees, 0.5: 70, 0.3: stable up to 79; SIMPLEC 0.7/0.3: 75; two correctors with `solver.simplec_non_orthogonal_compensation = true`: 70
(two correctors without it: unstable from 15 degrees); a lower `alpha_u` at `alpha_p = 1.0` and SIMPLE 0.7/0.3 are unstable from 45 degrees. On strongly
non-orthogonal meshes lower `solver.alpha_p` first. The mechanism is not identified beyond this (no boundary, wall or convection term is needed).

## Adaptive time stepping (F-209, ADR 0050 Part 2)

Transient incompressible `piso` / `pimple` runs only (a located error otherwise). Every key is optional; with all keys unset
(sentinel `0` / `false` / empty) the run is the fixed-`solver.dt` run (or the earlier `solver.max_courant` controller) bit for bit.
Setting any of them switches to the controller of ADR 0050, where `solver.dt` is the first step and, unless `solver.max_dt` is
given, the largest step.

| Key | Default | Meaning |
|-----|---------|---------|
| `solver.max_courant` | 0 | MAX Courant target (as before; with the keys below it joins the new controller) |
| `solver.max_courant_rms` | 0 | volume-weighted RMS Courant target; with `max_courant` the smaller step wins |
| `solver.dt_safety` | 0 (0.9) | the step aims at this fraction of the target |
| `solver.dt_growth` | 0 (1.2) | largest growth factor per step (>= 1) |
| `solver.dt_shrink` | 0 (0.5) | smallest factor per step |
| `solver.min_dt`, `solver.max_dt` | 0 (1e-3 dt, dt) | step limits [s] |
| `solver.error_tolerance` | 0 | local-error control by step doubling (implicit Euler, laminar constant density only): accepted when the relative velocity difference per unit time is below it; the global error then grows like tolerance x time |
| `solver.period`, `solver.steps_per_period` | 0 | period-based steps `dt = period / steps_per_period`, not combined with Courant or error control |
| `solver.extrapolate_initial` | false | start each step from the linear extrapolation of the two previous levels (changes the result at the time-error level) |
| `output.write_times` | empty | list of times on which a step lands (relative 1e-12); a numbered snapshot `snapshot_t<time>` is written when the run has an output directory |
| `output.write_time_interval` | 0 | land on every multiple of this interval |

Located errors: negative or inconsistent values (`min_dt > max_dt`, `dt_growth < 1`), a write time beyond `solver.end_time`,
`steps_per_period` without `period`, error control with energy, scalars, turbulence, rheology, sources, lumped components, dynamic
mesh, statistics, expression boundary conditions or `time_scheme` other than `euler`, and any of the keys on a steady run.
Restart files of such runs carry the controller state. Not wired: Aitken relaxation (library `stepctl::Aitken` only), local
timescale factors, residual smoothing, the compressible solver.

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
`solver.pressure_max_iter` control the Krylov solve (defaults 1e-3, 400) and `solver.pressure_abs_tol` the absolute residual floor.

Wave 19A (ADR 0029) added open boundaries, body forces, the energy equation and solid zones. Open boundary: `velocity = "zero_gradient"` with
`pressure = "fixed"` and `pressure_value` (outlets, and inlets with a prescribed pressure); the face flux there follows from the
cell velocity, the pressure difference and the Rhie-Chow term inside the system. An open face with `pressure = "zero_gradient"` is a
located error at `boundaries.<patch>.pressure`; with at least one open face the pressure level is set by the pressure condition and
`solver.p_ref_cell` is not used. `physics.body_force`, `physics.gravity` with `energy.buoyancy` (Boussinesq) and `energy.enabled`
are accepted (the energy equation is advanced after the flow in every iteration, as in the steady segregated algorithms; no
chemistry heat, components or zone sources). The balanced-force treatment keeps a hydrostatic or stably stratified fluid at rest.

Solid zones (`solids.<name>`, see the conjugate heat transfer section) may be combined with `coupled` without
`solver.frozen_flow`: the flow is solved in the fluid cells only. Solid cells are removed from the pressure-velocity system
(`u = 0`, `p = 0`), the faces between fluid and solid are no-slip walls with exactly zero flux (no Rhie-Chow bypass leak), and only the
energy equation sees the solids. Located errors: `solver.frozen_flow` (solids with a segregated algorithm and no `frozen_flow`, or
`coupled` together with `frozen_flow`), `solver.p_ref_cell` (reference cell inside a solid, closed domain; set a fluid cell
explicitly because the default is cell 0), `solids.<name>.zone` (a closed fluid region split by solids, or a fluid region without an
open boundary), `boundaries.<patch>.velocity` (non-zero prescribed flux through the boundary face of a solid cell).

Still diagnosed with a located error: turbulence models, rheology, user functions, momentum sources, lumped components, radiation,
property laws, `bc_kind` conditions, zone sources, `solver.bulk_velocity`, `solver.non_orthogonal_correctors`, periodic patches and
scalars or species together with solids. Known limits: steady only (no transient coupled solver); with gradient-based convection
schemes the least-squares gradient of a fluid cell next to a solid sees the solid cell value (use `central`/`upwind` for
verification); next to a prescribed-flux inlet the pressure shows a decaying odd-even mode (ADR 0029 addendum, A3).

## `scalars`, `species` and `properties` (F-053)

`scalars.<name>` and `species.<name>` transport extra variables on the flow flux of a steady run
(`simple`, `simplec`, `coupled`) or a transient incompressible run (`piso`, `pimple`, wave 15A: one implicit step per
time step with `solver.time_scheme`; other algorithms are rejected with a located diagnostic):
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
diffusion (a constant diffusivity per species) and reactions (a first-order sink can be given as a zone source, see
"Wave 15A" below).

`properties.viscosity` (dynamic, Pa s), `properties.conductivity` and `properties.specific_heat` each take
`law` = `constant` (the `physics` value, default) | `polynomial` (`coefficients` c0, c1, ... of T in kelvin) | `sutherland`
(`reference_value`, `reference_temperature`, `sutherland_constant`; value times (T/T_ref)^1.5 (T_ref+S)/(T+S)). They need
`energy.enabled` and a steady SIMPLE/SIMPLEC run. The viscosity enters through the face viscosity `mu(T_f)/rho` (the path of
`rheology`, with which and with a turbulence model or a user-function viscosity it is rejected); conductivity and
specific heat give the energy-equation diffusivity `k(T_f)/(rho cp(T_f))`, and `heat_flow` reports use the law at the wall
temperature. The density stays constant; no property data is shipped, the constants are inputs (the material library
F-111 provides sourced data).

Wave 21B (F-220, ADR 0036) adds three more laws to the same three keys: `law = "piecewise_linear"` (`temperatures` strictly
increasing, K, and `values` of the same length; linear interpolation, exact at the nodes), `law = "piecewise_polynomial"`
(`breakpoints`: n + 1 strictly increasing range limits [K] of n pieces, `piece_order` m >= 1 and `piece_coefficients`, n x m
coefficients piece by piece, lowest power first, in the absolute temperature T in kelvin; the pieces need not join), and
`law = "vogel"` (meant for the viscosity, accepted on every key; `vogel_a` > 0, `vogel_b`, `vogel_c` in K: `mu = a exp(b / (T - c))`, T > c). `extrapolation` =
`clamp` (default; holds the end value) | `linear` (continues the end segment) | `error` (NaN outside the range, which the finiteness checks of the solvers may report; it does not stop the run) outside the table.
Malformed tables are located errors (`properties.<key>.temperatures[i]`, `.values`, `.piece_coefficients`, `.vogel_a`, ...).
The default laws and all recorded baseline hashes are unchanged. The library in `rcfd_core::tabular` also has the Tait
compressible-liquid density and a pressure-temperature table with saturation line and phase selection; these are NOT
reachable from the case tree because the incompressible solvers keep a constant density.

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

`sources.<name>` adds a momentum source in a cell zone of a `simple`, `simplec`, `piso` or `pimple` run (ADR 0045: the
transient algorithms use the same mechanism, evaluated at the start of every outer iteration, so the solution-dependent
Lorentz force and porous sink follow the PIMPLE outer loop; with `piso` it is evaluated once per step from the old-time
velocity). `rotating_frame` and `mrf` sources, `solver.time_advance = fractional_step`, the `coupled` and compressible /
variable-density algorithms are rejected with a located diagnostic at `sources.<name>.type`. `zone` is `all` (default), `box` (cells whose centres lie in
`[box_min, box_max]`, metres) or `cell_zone` (a named zone of an imported mesh). A face belongs to the zone when both
its cells do. `type` selects:

| `type` | Keys | Meaning |
| --- | --- | --- |
| `porous` | `model = ergun`: `porosity` (0 < e < 1), `particle_diameter` [m]. `model = darcy_forchheimer`: `permeability` K [m^2], `inertial_resistance` C2 [1/m] | implicit sink `-(nu/K + C2 \|u\|/2) u` per unit mass, **superficial-velocity** formulation (the porosity does not enter the transport terms). Ergun maps to `1/K = 150 (1-e)^2/(e^3 d^2)`, `C2 = 3.5 (1-e)/(e^3 d)`; `\|u\|` is lagged one iteration |
| `rotating_frame` | `rotation_axis` (default +z), `rotation_origin`, `angular_velocity` [rad/s], `centrifugal` (default true) | relative-velocity form: `-2 Omega x u - Omega x (Omega x r)`. The centrifugal part is analytic at cell and face centres, the Coriolis part uses the previous iterate (boundary faces: the boundary velocity). Wall velocities are given in the rotating frame. `centrifugal = false` keeps Coriolis only (needed for periodic domains) |
| `mrf` | `rotation_axis` (default +z), `rotation_origin`, `angular_velocity` [rad/s] | multiple-reference-frame (frozen rotor) zone, **absolute-velocity** formulation (ADR 0008): the solved velocity stays absolute, momentum in the zone is convected with the relative flux `phi - (Omega x r).S` and gets the lagged acceleration `-Omega x u`; boundary conditions are absolute (a wall rotating with the zone is given `Omega x r` by `velocity_expression_x/y/z`). The interface to the rest of the mesh (an ordinary internal face set, also a non-conformal one from `annulus_3d_interface`) carries absolute momentum, which is the frozen-rotor treatment. |
| `acceleration` | `acceleration` [m/s^2] | constant acceleration in the zone |
| `actuator_disk` | `thrust` [N] (signed), `direction` (required, non-zero), `profile` = `uniform` (default) \| `linear_r` (force density proportional to the distance from the disk axis; needs `disk_centre`), `disk_centre` [m], `disk_axis` (default `direction`), `torque` [N m] about `disk_axis` (0 = no swirl; needs `disk_centre`), `density` [kg/m^3] (0 = `physics.rho`), `distribution` = `smooth` (default) \| `cell` | actuator disk (ADR 0036): the total force `thrust` is spread over the cells of the zone as a body force; the integral is exactly `thrust` (and `torque`) to round-off. `smooth` spreads the loading over one cell layer around the zone so that edge cells carry no unbalanced half force; `cell` is the raw top-hat (a jet of order sqrt(a dx) forms at the disk edge, see ADR 0036 A4c). Momentum-theory helpers are in `rcfd_fv::actuator`. |
| `lorentz` | `magnetic_field` [T] (required, non-zero, uniform), `conductivity` sigma [S/m] (required > 0), `applied_electric_field` [V/m] (default 0), `potential` = `none` (default) \| `solve` | MHD Lorentz force `J x B / rho` of a liquid conductor at low magnetic Reynolds number (ADR 0039), `J = sigma (E_ext - grad phi + u x B)`. `potential = none`: `grad phi = 0`, explicit part `J_c x B / rho` with `J_c = sigma E_ext` plus the implicit sink `sigma B^2/rho` (components of `u` perpendicular to `B`); `solve`: the potential equation `-div(sigma grad phi) = -div(sigma (E_ext + u x B))` is solved each iteration with the `boundaries.<patch>.electric*` conditions of the `electric` section (insulated by default; one cell is pinned when no patch fixes the potential). Output fields `lorentz_force_x/y/z` [m/s^2] and `current_density_x/y/z` [A/m^2]. Steady SIMPLE/SIMPLEC only; Joule heating, varying `B` and the induced field are not modelled (ADR 0039). Closed-form check: Hartmann channel flow, `crates/rcfd-api/tests/mhd.rs` |

MRF validation errors (path `sources.<name>...`): zero `rotation_axis`, non-finite `angular_velocity`,
a zone without cells, a zone boundary (walls, symmetry planes, the interface) that is not a surface of revolution about
the rotation axis (the frame flux `(Omega x r).S` must vanish there; planar interfaces perpendicular to the axis and
cylindrical ones qualify), an MRF zone sharing cells with another MRF zone or a `rotating_frame` source, and
transient algorithms (`piso`, `pimple`, `coupled`; the transient `rotating_frame` / `mrf` combinations are not built, ADR 0045 D4). Pseudo-transient time steps use the absolute flux. Not validated: any pump or turbine
benchmark, turbulence models or the energy equation with an MRF zone; the checks are in ADR 0008 and
`crates/rcfd-api/tests/mrf.rs`.

Limits: `rotating_frame` and `mrf` are steady-only; the viscous porous term uses `physics.nu`; in transient runs the sources use the
velocity of the previous outer iteration (one evaluation per step with `piso`: first-order lag of the solution-dependent
parts, e.g. the explicit part of the Lorentz force; raise `solver.n_outer` to converge it), the stability of the transient
algorithm at viscous numbers `nu dt/dy^2` above about 3 is a separate limit (ADR 0045 addendum); the non-conformal join of F-114 handles planar
interfaces only, so an MRF zone with a non-conformal interface is limited to interfaces perpendicular to the axis.
Examples: `cases/packed_bed/packed_bed.toml`, `cases/rotating_channel/rotating_channel.toml`.

## `chemistry` finite-rate and turbulent combustion chemistry (F-054, ADR 0027)

Reactions between the `species.<name>` mass fractions of a constant-density incompressible case (steady or transient;
compressible and variable-density engines are refused). The mechanism is a CHEMKIN-format file (`mechanism`, plus `thermo`)
or inline text (`mechanism_text`, `thermo_text`); every mechanism species must be a `species` entry and vice versa
(species not in the mechanism are refused), located by path, with the line inside the mechanism text in the message. Files are confined to the sandbox when
there is one. No mechanism is shipped.

| Key | Default | Meaning |
| --- | --- | --- |
| `model` | `off` | `off` \| `finite_rate` (Arrhenius) \| `eddy_dissipation` (irreversible reactions, needs `k_epsilon` or `k_omega_sst`; epsilon = 0.09 k omega for SST) \| `finite_rate_eddy_dissipation` (minimum of the two for a positive kinetic rate) \| `edc` (Eddy Dissipation Concept, fine-structure reactor integrated over tau*) |
| `density` | -1 (unset) | cell density for the rates; required, must equal `physics.rho` (relative 1e-12) |
| `heat_release` | false | couple the enthalpy release to `energy` (constant `cp`: `dT/dt = -sum h_k omega_k / (rho cp)`); requires `energy.enabled` |
| `heat_capacity` | -1 (unset) | required with `heat_release`, must equal `physics.cp` |
| `temperature` | -1 (unset) | fixed temperature of the chemistry (K) when the energy equation is off; refused when it is on |
| `minimum_temperature` | 0 (off) | cells colder than this are not integrated |
| `rtol`, `atol_species`, `atol_temperature` | 1e-8, 1e-14, 1e-8 | tolerances of the stiff cell integrator |
| `eddy_dissipation_a`, `eddy_dissipation_b` | 4.0, 0.5 | Magnussen-Hjertager constants (UNVERIFIED, see `docs/reference-data.md`) |
| `eddy_dissipation_product_limit` | true | include the product-limited term; false is the reactant-only form |
| `edc_c_xi`, `edc_c_tau` | 2.1377, 0.4082 | EDC constants (secondary source, see `docs/reference-data.md`) |
| `acceleration` | off | acceleration of the transient `finite_rate` step (F-223, ADR 0053): `off` (direct integration of every cell), `cluster` (one integration per group of cells within `accel_tolerance` in the scaled max-norm, 100 K and 0.01 in mass fraction; the other cells receive the representative's increment; tolerance 0 is bit-identical to `off`), `isat` (in-situ adaptive tabulation, Pope 1997, tabulation error `accel_tolerance`, table reset when `dt` changes). Located errors: other closures, the steady algorithms, an unset tolerance |
| `accel_tolerance` | -1 (unset) | tolerance of the acceleration; an error when `acceleration` is not `off` and it is unset, an error when given with `off`; `isat` needs a value > 0 |
| `write_rates` | false | also write the cell arrays `wdot_<species>` [mol/m3/s] (net production rate of the closure on the final fields; for `edc` the kinetic rate of the cell state) and, with `heat_release`, `heat_release_rate` [K/s] (the temperature source of the energy balance) to `solution.vtu` (ADR 0037 D2) |

Outputs of the run path (`rcfd run`, service `run.headless`, `write_results`; ADR 0037): `chemistry.json` whenever `model` is not `off`:
`model`, `mechanism {species, reactions, elements}`, per species `{min, max, volume_mean}`, `mass_fraction_sum {min, max}`,
`temperature {min, max, source}`, `element_moles` (moles of element atoms in the domain, `sum V rho sum_k Y_k a_ke / W_k`),
`density`, `cells`, and, only when `acceleration` is not `off`, an `acceleration` object (`cluster`: `steps`, `cells`, `integrations`; `isat`: `steps`, `queries`, `retrieved`, `grown`, `added`, `direct`, `records`). The species themselves are user scalars and are in `solution.vtu`. The chemistry was already advanced by
`Simulation::step` (transient Lie splitting after the scalar sweeps, steady source form before the scalar solve); the binary and the
service therefore give bit-identical fields to the in-process run (ADR 0037 C1, C2).

Transient runs (`piso`, `pimple`) use Lie operator splitting: after the flow step and the transport sweeps every cell is advanced
by the stiff integrator over `dt` and the species and temperature are written back (first order in `dt`). Steady runs use a source
form: the net rate is an explicit source with a diagonal implicit sink (finite-difference, plus Patankar's coefficient for
negative sources), the heat release a lagged source of the energy equation. A chemistry failure stops the run with the cell
index. Limits: constant density, sequential cell loop (tabulated or clustered chemistry only through `acceleration` on the transient `finite_rate` step; ISAT and DRG are also library tools in `rcfd-reactor`: `isat`, `drg`, `cluster`), no Strang splitting, no thickened-flame
or flamelet models, no comparison with published flames (F-054 stays `partial`).

Library additions in `rcfd-reactor` (F-129, ADR 0053, no case keys; mixture fraction and flamelet quantities are not wired into the case tree): `mixfrac` (mixture fraction, HP equilibrium table, beta-PDF mean, Burke-Schumann), `flamelet` (steady laminar flamelet equations, chi_st continuation table), `progress` (premixed progress variable, phase-plane front speed). Further library additions (no case keys): `pollutants` (extended Zeldovich thermal NO rate, rate constants as inputs),
`network` (pre-exponential sensitivities, PSR into plug flow), `json` (mechanism interchange, format `rcfd-mechanism` version 1)
and optional NASA nine-coefficient thermodynamics in `Species::nasa9`.

## `components` lumped zone components (F-113, ADR 0012)

`components.<name>` adds a lumped component to a constant-density run (`solver.algorithm` `simple`, `simplec`, `piso` or
`pimple`; ADR 0045 evaluates the face jumps and network pressures every outer iteration of the transient algorithms and the
heat-exchanger heat sources once per step from the flux and temperature of the previous step; the coupled solver, ideal-gas runs, `solver.time_advance = fractional_step` and `dynamic_mesh` are a located
diagnostic at `components.<name>.type`). `type`:

| `type` | Keys | Meaning |
| --- | --- | --- |
| `fan` | `zone` (`all`, `box` with `box_min`/`box_max`, `cell_zone`), `direction` (flow direction, required), `curve_flow` [m^3/s] strictly increasing, `curve_pressure` [Pa], `relax` (default 0.5) | pressure rise `dp(Q)` (piecewise-linear fan curve, end values held) applied as a pressure jump on the internal faces of the downstream boundary of the zone; `Q` is the volume flow through them. The applied rise is under-relaxed (`relax`, first evaluation relaxed from zero) |
| `porous_jump` | `zone`, `direction`, `thickness` t [m], `permeability` alpha [m^2], `inertial_resistance` C2 [1/m] | thin Darcy-Forchheimer jump `dp = (mu/alpha) t v + C2 t rho v^2/2` (`v` = face-normal superficial velocity) on the downstream boundary faces of the zone; the viscous part is implicit, `abs(v)` is lagged |
| `heat_exchanger` | `zone`, `direction`, `mode` = `fixed_duty` (`duty` [W], negative for a radiator) or `effectiveness` (`effectiveness`, `secondary_temperature` [K], `secondary_capacity_rate` [W/K], 0 = infinite) | uniform energy source over the zone volume; effectiveness mode uses `Q = eps C_min (T_secondary - T_in)` with `T_in` the flux-weighted upstream temperature of the zone entry faces. Needs `energy.enabled` |
| `heat_exchanger` (`mode = "ntu"`) | `arrangement` = `counterflow` (default) \| `parallel` \| `crossflow_unmixed` \| `crossflow_cmax_mixed` \| `crossflow_cmin_mixed` \| `shell_and_tube`, `ua` [W/K] (required > 0), `secondary_temperature` [K], `secondary_capacity_rate` [W/K] (0 = infinite), or `dual_cell = true` with a `[components.<n>.secondary]` table (`zone`, `box_min`, `box_max`, `cell_zone`, `direction`, `loss_coefficient`, `loss_linear`) | effectiveness-NTU macro cell (ADR 0039): `C_p = rho cp Q` of the actual zone flow, `NTU = UA/C_min`, `Q = eps(NTU, Cr) C_min (T_s,in - T_p,in)`; with `dual_cell` the secondary stream is a second flow-through zone whose inlet temperature and flow are measured and which receives exactly `-Q` (energy conserved to round-off). Monitors `<n>_eps`, `<n>_ntu`, `<n>_sec_flow`, `<n>_sec_t_in`, `<n>_sec_duty` |
| `heat_exchanger` (any mode) | `loss_coefficient` K, `loss_linear` r1 [Pa s/m] (default 0 = none) | pressure-loss closure on the downstream interior plane of the zone, `dp = K rho v^2/2 + r1 v` (needs an interior plane like `fan`); monitor `<n>_dp` |
| `fan_boundary` | `patch` (fixed-pressure boundary), `curve_flow`, `curve_pressure`, `reference_pressure` [Pa], `relax` | patch static pressure `p_ref + dp(Q_in)` with `Q_in` the volume flow into the domain through the patch (piecewise-linear curve, lagged and relaxed like `network_outlet`, the first evaluation takes the curve value at the initial flow; dynamic head not added). A lossless duct between two pressure boundaries diverges in the segregated iteration: give it a distributed resistance. Vent: `bc_kind = "vent"`; porous jump: `porous_jump` above |
| `network_outlet` | `patch` (fixed-pressure boundary), `reference_pressure` [Pa], `resistance_linear` R1 [Pa s/m^3], `resistance_quadratic` R2 [Pa s^2/m^6], `relax` | the patch pressure follows `p_ref + R1 Q + R2 Q abs(Q)` of the flow through the patch (lagged, relaxed) |

Monitors: `<name>_flow` [m^3/s] and `<name>_dp` [Pa] (fan rise, analytic jump drop at the current flow, network patch
pressure) or `<name>_duty` [W] and `<name>_t_in` [K]. Validation errors are located (`components.<name>.curve_flow`,
`.direction`, `.patch`, `.effectiveness`, `.type`, `.zone`, `.thickness`, ...). A fan or jump zone needs an interior
plane on its downstream side. Pressure-driven loops with little resistance are stiff: use a small `relax` (0.02 in
the ADR 0012 network test). Not done: 1D network coupling beyond the lumped outlet relation, shell-and-tube or porous
exchanger domains. Checks and measured results: ADR 0012, `crates/rcfd-api/tests/lumped.rs`. The NTU exchanger, loss closure, dual cell and `fan_boundary` (ADR 0039): `crates/rcfd-api/tests/heat_exchanger.rs`, library `rcfd_fv::hx` (`crates/rcfd-fv/tests/heat_exchanger.rs`).

## `radiation` grey radiation (F-130, F-052, ADR 0012)

`radiation.model` is `none` (default), `rosseland` or `p1`; both need `energy.enabled = true` and a steady
SIMPLE/SIMPLEC run. Keys: `absorption_coefficient` a, `scattering_coefficient` sigma_s [1/m] (extinction a + sigma_s
must be positive), `refractive_index` n. `rosseland` adds the radiative conductivity `16 sigma n^2 T^3 / (3 beta)` to
the energy diffusion (optically thick media). `p1` solves the incident radiation `G` with Marshak boundary conditions
using the wall emissivity `boundaries.<patch>.emissivity` in (0, 1] (default 1; patches with `velocity = symmetry` are
reflecting), couples `a (G - 4 sigma n^2 T^4)` linearised into the energy equation, and exposes the field `G`
(`field("G")`) and `Simulation::radiative_heat_flow(patch)`. Conductive wall heat flow is still the `heat_flow` report
(set its `conductivity`). Not done: non-grey band models, WSGG, discrete ordinates, surface-to-surface radiation,
transient and ideal-gas runs.

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

## `registers` and `initial.patches` (F-124, ADR 0011)

`registers.<name>` defines a named set of cells; see `docs/interchange-formats.md` for the semantics.

| Key | Default | Meaning |
| --- | --- | --- |
| `type` | `box` | `box`, `expression`, `boundary_adjacent`, `cell_zone` or `combine` |
| `box_min`, `box_max` | none | corners of the box (`box_max` above `box_min` in every component) |
| `expression`, `above`, `below` | none, -1e300, 1e300 | expression of the cell centre x, y, z (lengths with units); selects `above <= value <= below` (SI, inclusive) |
| `patches`, `layers` | none, 1 | `boundary_adjacent`: patches and number of cell layers |
| `cell_zone` | none | mesh cell zone name |
| `operation`, `operands` | `union`, none | `combine`: union, intersection, difference or complement of the named registers |
| `grow`, `shrink`, `neighbours` | 0, 0, `faces` | dilate / erode the result; adjacency by faces or by points |

`sources.<n>.zone = "register"` with `register = "<name>"` restricts a source. `initial.patches.<n>` has `register`,
`variable` (`velocity`, `pressure`, `temperature`, and the turbulence quantities `k`, `epsilon`, `omega`, `nutilde`) and
`value` (3 numbers for velocity, 1 otherwise) and overwrites the initial field inside the register (a turbulence
variable needs a turbulence model that has it, else a located error at `initial.patches.<n>.variable`). Errors name the setting, for example `sources.bed.register: unknown register 'x'
(registers: core)` or `registers.x.patches: the mesh has no patch 'nope' (patches: ...)`.

## Wave 15A: initialisation methods, zone sources, user transport equations (F-205, F-201, F-202, ADR 0019)

### `initial.method` (F-205)

| Key | Default | Meaning |
| --- | --- | --- |
| `initial.method` | `uniform` | `uniform` (the existing start, bit-identical), `potential` or `coarse` |
| `initial.coarse_factor` | 2 | `coarse`: the cell counts of the generated mesh are divided by this factor (2 to 8) |
| `initial.coarse_iterations` | 200 | `coarse`: iteration cap of the coarse pre-solve |

`potential` solves a Laplace problem `div(grad psi) = 0` for a scalar potential with the normal velocity of the velocity
boundary conditions as Neumann data (and `psi = 0` on patches with a fixed pressure) and starts from `u = grad psi` and the
Bernoulli pressure `p = H - |u|^2 / 2`; it needs the velocity of every patch or a fixed pressure on it, balanced fluxes for a
closed set of inlets and walls, and rejects periodic patches, all with located errors at `initial.method` or the patch.
`coarse` runs the same case on a mesh with the cell counts divided by `coarse_factor` (generated meshes only: `box_2d`,
`channel_2d`, `cavity_2d`, `wedge_2d`, `nozzle_2d`, `cylinder_ogrid`, `annulus_3d` and the two interface variants; the counts
must divide) with `initial.method = "uniform"` and `solver.max_iterations = coarse_iterations`, and interpolates the result
(non-conservative) to the fine mesh. Both methods apply to the steady `simple` / `simplec` algorithms. The start only
changes the initial field; the converged answer is the one the case defines. Not verified: that a potential start reduces
the iterations of a recirculating flow (it did not for the backward-facing step, ADR 0019 I2).

### `zone_sources.<name>` (F-201)

A volumetric source or a fixed value for the temperature or a `scalars` / transported `species` entry in a cell zone.

| Key | Default | Meaning |
| --- | --- | --- |
| `target` | `temperature` | `temperature` (needs `energy.enabled`) or a scalar / species name (the balance species is a located error) |
| `kind` | `source` | `source` or `fixed_value` |
| `zone`, `register`, `box_min`, `box_max`, `cell_zone` | `all` | the zone, as for `sources.<n>` |
| `value` | 0 | source: W/m^3 for the temperature (divided by `physics.rho * physics.cp`), value per second for a scalar; fixed_value: K or the scalar's unit |
| `value_expression` | none | expression of x, y, z, t at the cell centres, re-evaluated every step / iteration; overrides `value`. Dimension-checked: temperature source W/m^3, temperature fixed value K, scalar fixed value dimensionless, scalar source dimensionless or 1/s |
| `linear_coefficient`, `reference` | 0, 0 | source only: `S = value - k (phi - reference)` with `k >= 0` [1/s], treated implicitly |

Sources on one target add. A fixed-value cell has its equation replaced by `phi = value`, its column moved to the neighbours'
right-hand sides (a symmetric system stays symmetric) and the solved value overwritten, so it holds the value exactly.
Located errors: unknown target, balance species, negative `linear_coefficient`, `fixed_value` with a linear part, overlapping
fixed zones of one target, a zone without cells, an expression of the wrong dimension or not finite, `coupled`, variable
density and compressible runs, and temperature sources together with the radiation model. They act in the steady `simple` /
`simplec` and the transient `piso` / `pimple` runs. Not delivered: mass sources, momentum sources or fixed values by
expression (the `sources` of F-031 remain), sources and fixed values for turbulence quantities, the variable-density and
compressible engines.

### User transport equations in `scalars.<name>` (F-202)

| Key | Default | Meaning |
| --- | --- | --- |
| `equation` | `transport` | `transport` (convection-diffusion-source, steady or transient), `diffusive` (`ds/dt = div(D grad s) + S`, no convection; in a steady run the time derivative is absent and it is the steady diffusion problem), `poisson` (`div(D grad s) + S = 0` solved at every step without a time derivative) or `algebraic` (`s = expression`) |
| `diffusivity_expression` | none | diffusivity [m^2/s] as an expression of x, y, z, t at the face centres; replaces `diffusivity`; not together with `schmidt`; must be finite and positive |
| `source_expression` | none | source (dimensionless or 1/s, read as per second) at the cell centres, added to `source` |
| `initial_expression` | none | dimensionless initial field of x, y, z; replaces `initial` |
| `expression` | none | `algebraic` only: expression of x, y, z, t, `T` (with `energy.enabled`) and the names of the other scalars and species (their latest values); dimensionless result; the variable cannot refer to itself; no boundary conditions |

The Poisson equation uses the sign `-div(D grad s) = S`, like the transport equation. Not delivered: the vector variant
(three-component diffusive / Poisson / algebraic variables), feeding user variables into properties, turbulence or other
equations, and species with user equations. Errors are located at the key (`scalars.s.diffusivity_expression`,
`scalars.s.expression`, ...).

## Wave 16W: `decomposition` and `postprocessing`, `rcfd partition`, `rcfd sample`, `rcfd inflate` (F-236, F-072, F-108, ADR 0021)

These sections and commands surface library features that already existed. The solver does not use the partitions or the
reordering: they are reports and a VTK cell field.

### `decomposition` (F-236)

| Key | Default | Meaning |
|---|---|---|
| `method` | `none` | `none`, `bisection`, `inertial`, `sectors`, `graph` |
| `parts` | 1 | number of parts (at most the cell count; checked when the mesh is built) |
| `rule` | `longest` | bisection split direction: `longest`, `cycle`, `x`, `y`, `z` (bisection only) |
| `sector_kind` | `circumferential` | `sectors` only: `circumferential` or `radial` |
| `origin`, `axis`, `reference` | | `sectors` geometry (centre, rotation axis, zero-angle direction) |
| `weights.mode` | `none` | `box`: cells whose centre is inside `weights.box_min`..`box_max` weigh `weights.inside`, the others `weights.outside` |
| `tolerance`, `max_passes` | 0.03, 20 | boundary smoothing target imbalance and pass limit |
| `smooth` | false | run the boundary smoothing after the partition |
| `reorder` | false | also report the matrix bandwidth natural and with reverse Cuthill-McKee |
| `write_field` | false | add the cell array `partition` (part id) to `solution.vtu`; needs a `method` |

`rcfd partition <case|mesh> [--method m] [--parts n] [--rule r] [--sector-kind k] [--origin x,y,z] [--axis x,y,z]
[--reference x,y,z] [--weights-box x0,y0,z0:x1,y1,z1:inside:outside] [--tolerance t] [--max-passes n] [--smooth]
[--reorder] [--out parts.vtu] [--json] [--parts-vector]` takes the settings of the case section and lets the flags override
them. `--json` prints `method`, `n_parts`, `n_cells`, `part_cells`, `part_weights`, `imbalance` (weighted, as the library
function), `edge_cut` (internal faces between different parts), `ghost_cells` per part and, with `--reorder`, `bandwidth`
(`natural`, `rcm`). Invalid settings are reported at their path (`decomposition.parts`, `decomposition.method`, ...) with exit
code 3. The service method `mesh.partition` returns the same report (`with_parts` adds the part vector).

### `postprocessing` (F-072)

| Key | Meaning |
|---|---|
| `derived` | list of `vorticity`, `q_criterion`, `total_pressure` (array `p_total`), `mach`, `wall_shear`, `wall_y_plus` |
| `wall_patches` | wall patches for `wall_shear` and `wall_y_plus` (default: every wall patch) |
| `samples.<name>` | `type = "line"` (`p0`, `p1`, `points`) or `"plane"` (`origin`, `e1`, `e2`, `extent1`, `extent2`, `n1`, `n2`); `fields` (default `speed`, `p`; also `ux`, `uy`, `uz`, and for variable density `T`, `rho`, `mach`, ...); `gradient` (default true: linear reconstruction from the cell gradient) |

`vorticity`, `q_criterion`, `p_total` and `mach` are cell arrays in `solution.vtu`; wall quantities are written to
`wall_<patch>.vtp` (arrays `wall_shear`, `wall_y_plus`); each sample to `sample_<name>.csv` (columns `s,x,y,z,<fields>,inside`
for lines, `x,y,z,<fields>,inside` for planes; `inside` is 0 and the values are empty for points outside the mesh). Incompressible
total pressure uses `rho` times the kinematic pressure; the variable-density case uses the isentropic relation with `fluid.gamma`.
Located errors at `postprocessing.derived`: unknown names; `mach` without the compressible solver or `fluid.density =
"ideal_gas"`; `total_pressure` with the compressible solver or `real_gas`; `wall_shear` and `wall_y_plus` with a turbulence
model, the compressible or variable-density engines, a `rheology` law or a temperature-dependent viscosity (they use the constant
laminar `physics.nu`); an unknown or non-wall patch at `postprocessing.wall_patches`; bad sample geometry at
`postprocessing.samples.<name>.points`, `.p1`, `.e2`, `.n1`, `.fields`.

`rcfd sample <case> [--line x0,y0,z0:x1,y1,z1 [--points n]] [--plane o:e1:e2:ext1:ext2:n1:n2] [--fields a,b] [--no-gradient]
[--max-iterations n] [--out file-or-dir]` runs the case (or `n` iterations) and writes the sample; without `--line`/`--plane`
it writes the case's `postprocessing.samples`. The service method `field.sample` samples a live session and returns the CSV.
There is no solution-file reader yet, so `rcfd sample` always runs the case first.

### `rcfd inflate` (F-108)

`rcfd inflate <case|mesh> --patch p --layers n (--first h | --y-plus y --u-tau u --nu nu | --uniform total | --aspect a |
--last-ratio r --first h) [--growth g] [--out inflated.vtu] [--json]` adds prismatic layers to a boundary patch of any mesh
(not only the `mesh.wall_layers` generator layers) and reports `patch`, `sizing`, `cells_before`, `cells_after` and the library
report (first and last heights, thickness, growth, quality). Exit codes: 1 for a failure (unknown patch, invalid sizing value),
2 for usage errors, 4 if the inflated mesh has quality errors. The service method is `mesh.inflate`. There is no case-tree key
that inflates a mesh yet.

## Wave 16B: boundary-condition catalogue `bc_kind` (F-200, ADR 0022)

`boundaries.<patch>.bc_kind` (default `none`: the entry is read exactly as before) selects one of the kinds below for the
constant-density segregated algorithms (`simple`, `simplec`, `piso`, `pimple`). With a kind the kind owns the velocity and
pressure conditions: leave `velocity = "fixed"`, `pressure = "zero_gradient"` and `velocity_value` zero, and do not give
`velocity_expression_*`, `velocity_udf` or `profile` (located errors). `temperature`, `turbulence` and scalar keys keep their
meaning. A key that belongs to other kinds only, with a non-default value, is a located error that names the kinds. Compressible,
variable-density and `coupled` runs reject every kind at `boundaries.<patch>.bc_kind`. Pressures of the new kinds are in Pa
(`bc_pressure`, converted with `physics.rho`; the older `pressure_value` stays kinematic).

| Kind | Meaning |
|---|---|
| `mass_flow` | Prescribed mass flow (inlet, or outlet when negative). Exact: `rho sum u.S = -mass_flow_value`. |
| `outflow` | Outlet that takes a weighted share of the total inflow (the flow entering through every non-`outflow` patch). |
| `pressure_outlet` | Static pressure `bc_pressure` with backflow options. |
| `opening` | `pressure_outlet` with total-pressure backflow and normal backflow direction (entrainment); no further keys. |
| `vent` | Loss law `p = bc_pressure + K u_n |u_n| / 2` (kinematic, relaxed 0.5), either flow direction. |
| `total_pressure_inlet` | Total pressure `bc_pressure`, direction and swirl; `p = p0 - |u|^2/2` on inflow faces. |
| `radial_equilibrium_outlet` | Outlet pressure profile with `dp/dr = rho w^2 / r` about an axis. |
| `target_mass_flow_outlet` | Uniform outlet pressure updated each iteration toward `target_mass_flow`. |

| Key | Default | Used by |
|---|---|---|
| `mass_flow_value` [kg/s, positive INTO the domain] | 0 | mass_flow |
| `mass_flow_shape_expression` (velocity-dimensioned expression of x, y, z; only the shape counts, non-negative) | "" (uniform) | mass_flow |
| `mass_flow_source_patch`, `mass_flow_fraction` in (0, 1] | "", 1 | mass_flow: recirculation, `m = fraction * max(outward flow of the source patch, 0)`, replaces `mass_flow_value` |
| `flow_direction` | zero = along the inward normal | mass_flow, total_pressure_inlet |
| `swirl_rate` [rad/s], `swirl_axis`, `swirl_origin` | 0, [0,0,1], origin | mass_flow, total_pressure_inlet (axis and origin also radial_equilibrium_outlet) |
| `bc_pressure` [Pa] | 0 | pressure_outlet, opening, vent, total_pressure_inlet, radial_equilibrium_outlet, target_mass_flow_outlet (initial value) |
| `backflow_direction` `normal` or `zero_gradient` | normal | pressure_outlet |
| `backflow_pressure` `static` or `total` | static | pressure_outlet |
| `prevent_reverse_flow` | false | pressure_outlet (backflow faces are closed), outflow (negative shape clipped) |
| `pressure_average` `none` or `area` | none | pressure_outlet: area mean of the applied static pressure equals `bc_pressure` |
| `loss_coefficient` K >= 0 | 0 | vent |
| `flow_rate_weight` > 0 | 1 | outflow |
| `radial_reference` `hub` or `area_average` | hub | radial_equilibrium_outlet |
| `radial_frame_rate` [rad/s] | 0 | radial_equilibrium_outlet: in a `rotating_frame` case (with `centrifugal = true`, so `p` is the static pressure) set it to the frame's angular velocity about `swirl_axis`; the balance then uses the absolute swirl `w_theta + rate r` (ADR 0078 X1). The default balances the solved (relative) swirl, which is wrong in a rotating frame (47 % pressure error on the X1 check). |
| `target_mass_flow` [kg/s, positive OUT], `pressure_update` `bernoulli`, `secant` or `guarded_secant`, `pressure_relax` [0.01, 1] | 0, bernoulli, 0.5 | target_mass_flow_outlet |

Signs: `mass_flow_value` is positive into the domain, `target_mass_flow` positive out of the domain, and the `mass_flow` report
positive out. Located errors (see `bc_cat.rs`): a direction that does not enter (`flow_direction`), unknown, own, cyclic or doubly
given source (`mass_flow_source_patch`), fraction outside (0, 1], `flow_rate_weight <= 0`, negative `loss_coefficient`, a shape
expression with a compile error, negative values or zero total, negative `target_mass_flow`, a zero `swirl_axis`, a radial
outlet face on the axis, `initial.method = "potential"` with any kind but `mass_flow`, and a turbulence model on a
`total_pressure_inlet` or on a sourced `mass_flow` inlet unless `turbulence_spec = "values"` (their speed is a result).
`turbulence = auto` gives the role Inlet to `mass_flow` with a positive value (or sourced) and `total_pressure_inlet`, and
zero gradient to every other kind; the inlet speed for the turbulence helpers is `|m| / (rho A)` of the patch. A turbulence model
whose only inlet speed is a result (total-pressure inlet, sourced or negative mass flow) needs `turbulence.k_initial` and the
initial epsilon / omega.

Practical notes. Dynamic-head conditions (total backflow, `total_pressure_inlet`, `vent`, `radial_equilibrium_outlet`) are
lagged with a fixed relaxation weight 0.5. A frictionless duct driven by two pressure ends does not start from rest with plain
under-relaxation (the first iterations diverge); use `solver.pseudo_mode = "physical"` with a small `pseudo_dt`. Walls that behave
as symmetry planes need `solver.component_diagonal = "corrected"`. `pressure_update = "secant"` is a known miss (diverges on the
registered case, ADR 0022 addendum); use `bernoulli` or `guarded_secant` (ADR 0055 D5: the secant slope is taken only when the last mass-flow change is
at most half the present error and the slope lies within 0.1 to 10 times the dynamic-head slope, else the dynamic-head slope; the registered
F1 case reaches 4e-16 relative error in 1880 iterations). Not delivered: solid circular-pipe mesh, compressible, variable-density
and coupled support, temperature of a recirculated stream and per-boundary thermal conditions (F-051), time-varying mass flow,
`pressure_average` on other kinds.

Convergence controls (ADR 0023). `solver.residual_norm` = `reference` (default) | `backward` | `scaled` selects the normalisation of the
monitored residuals; `scaled` divides by sum(|A||x| + |b|) (momentum) and by the larger of the pressure-equation scale and half the
sum of the flux magnitudes (pressure), which stays meaningful for uniform or exactly zero components and does not amplify round-off. Use
it when `reference` stalls at 1e-12 on a converged field. `solver.pseudo_control` = `off` (default) | `ser` | `ser2` scales the pseudo time
step automatically (needs `solver.pseudo_mode` other than `off`, else a located error); `solver.pseudo_scale_max` (default 1000) caps the
scale. The controllers are NOT validated (known misses, ADR 0023 addendum A8); fixed `pseudo_cfl` is the reliable setting.

## Reactor cases and FW-H surface files (F-112, F-136)

`rcfd reactor <case.json>` and `rcfd acoustics <surface.json>` take their own JSON documents, described with
examples in [reactor-and-acoustics.md](reactor-and-acoustics.md); they are not sections of the CFD case file.

## Wave 17B: conjugate heat transfer (F-051, ADR 0024)

Solid cell zones with their own conductivity, density and specific heat, thermal contact resistance between materials, and a
convection / thin-wall / radiation-to-ambient wall condition. Every new key has an explicit unset sentinel (`-1`, `""` or an
empty list) and a located error when it is needed and unset; none shadows another value. All of it is additive: a case without
these keys behaves bit-identically to before.

The temperature equation is kept in the conductive form normalised by the fluid `physics.rho * physics.cp`; a solid enters through
per-cell conductivity and heat capacity (face conductivity is the distance-weighted harmonic mean, so the interface flux is exact
for equal-sized cells). A conforming interface and a non-conformal one (`mesh.generator = "channel_2d_interface"`, joined faces
are ordinary internal faces) are treated alike. Volumetric heat generation in a solid is a `zone_sources` entry on the temperature.

### `solids.<name>`

| Key | Default | Meaning |
| --- | --- | --- |
| `zone` | `"unset"` (an error) | `all`, `box`, `cell_zone` or `register`; with `box_min`, `box_max`, `cell_zone`, `register` as for `zone_sources` |
| `conductivity` | `-1` (an error) | W/m/K, constant, isotropic |
| `density` | `-1` (an error) | kg/m^3 |
| `specific_heat` | `-1` (an error) | J/kg/K |
| `initial_temperature` | `-1` = use `initial.temperature` | K |
| `electrical_conductivity` | `-1` = use `electric.conductivity` | S/m; `0` = insulator; only with `electric.enabled` (ADR 0035) |

Solid zones must not overlap; "fluid" is every cell in no solid. Solids need `energy.enabled = true` and either `solver.frozen_flow = true` (prescribed flow) or `solver.algorithm = "coupled"` (solved flow with the solid cells excluded from the pressure-velocity system, ADR 0029).

### `thermal_interfaces.<name>`

`solid_a`, `solid_b` (a `solids` name or `"fluid"`; empty is an error; the two must differ) and `resistance` (m^2 K/W, `-1` = unset
is an error, `0` = perfect contact). The resistance is added in series on the internal faces between exactly these two materials.

Added in wave 21A (ADR 0035 D1, all optional):

| Key | Default | Meaning |
| --- | --- | --- |
| `layer_thickness` [m], `layer_conductivity` [W/m/K] | empty = no layers | lists of equal length, every entry positive; the steady resistance `sum t_i/k_i` (no thermal capacity) is added to `resistance`. With layers, `resistance` may stay `-1` (counts as 0) |
| `connection` | `closed` | `open` makes the faces adiabatic (a gap); `resistance` and the layer keys must then be absent. In a steady (simple/simplec) problem an open interface that leaves a group of cells with no fixed-temperature or convecting boundary is a located error at `connection` |

Not delivered (F-215 stays partial): layer thermal capacity, in-plane shell conduction, solution-dependent open/closed switching, interfaces between separate meshes beyond the F-114 join.

### `electric` (F-213, ADR 0035 D2)

Quasi-static conduction `div(sigma grad phi) = 0` with the Joule heat `sigma |grad phi|^2` added to the energy equation. Available
with `solver.frozen_flow = true` and `energy.enabled = true` (prescribed flow; transient and steady paths), not with the compressible or
variable-density solvers. The coupling is lagged: the potential is solved with the conductivity of the temperature of the previous
step (steady: previous sweep).

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | solve the potential |
| `conductivity` | `-1` (an error when enabled) | sigma_0, S/m, of the fluid cells and of solids without `electrical_conductivity` |
| `temperature_coefficient` | `0` | a, 1/K, of `sigma = sigma_0 / (1 + a (T - reference_temperature))`; a cell where `1 + a (T - T_ref) <= 0` stops the run with a message naming this key |
| `reference_temperature` | `-1` (an error when `temperature_coefficient != 0`) | T_ref, K |
| `joule_heating` | `true` | `false` solves the potential only |

Per patch (`boundaries.<patch>`): `electric = "insulated"` (default, zero current), `"potential"` (needs `electric_potential`, V) or
`"current"` (needs `electric_current`, the TOTAL current in A flowing INTO the domain, spread as a uniform density over the patch
area). `electric_potential` and `electric_current` default to the sentinel `-1e30` (unset; any real is a legal value); setting one
that does not match `electric` is a located error. At least one patch must be `potential`; a case with only `current` patches is a
located error at `electric`. Located errors also: `electric.conductivity`, `electric.reference_temperature`, `electric.enabled`
(not `frozen_flow`, energy off), `solids.<n>.electrical_conductivity` (negative other than the `-1` sentinel), `boundaries.<p>.electric`
(used without `electric.enabled`). Fields (`probe`, `area_integral`, output): `electric_potential`, `joule_heat` (W/m^3),
`electric_conductivity` (S/m) in the cells; `electric_potential` and `electric_current_density` (A/m^2 INTO the domain, so the current
through a patch is the existing `area_integral` of that boundary field) on the boundary faces. On orthogonal meshes the Joule power
equals the electrical power input to round-off; on non-orthogonal meshes it holds only to truncation error (named gap).

Not delivered (F-213 stays partial): Joule heating with a solved flow, magnetic induction, higher-order or iterated coupling in time,
the exact power identity on non-orthogonal meshes.

### `phase_change` (F-212, ADR 0035 D3)

Enthalpy-method melting and solidification of a single material with equal phase properties: `h(T) = T + (L/cp) f(T)` with the
linear liquid fraction `f` between `solidus` and `liquidus`, implicit Euler, Newton on the apparent capacity with backtracking.
Available with `solver.frozen_flow = true` and a ZERO prescribed flux (the melt does not flow), `solver.algorithm = "piso"` or
`"pimple"`, `solver.time_scheme = "euler"`, `energy.enabled = true`; temperatures are Kelvin (> 0).

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | |
| `solidus`, `liquidus` | `-1` (errors when enabled) | K; `liquidus > solidus` |
| `latent_heat` | `-1` (an error when enabled) | J/kg, `>= 0` (`0` = sensible only) |
| `momentum_constant` | `-1` | reserved: the Carman-Kozeny damping exists in the library only; setting it is an error |
| `newton_max` | `100` | Newton iteration cap per step; not converging stops the run with a message naming this key |

Field `liquid_fraction` (cells, in [0, 1]). Located errors: `phase_change.solidus`, `.liquidus`, `.latent_heat`, `.momentum_constant`,
`solver.time_scheme`, `solver.algorithm`, `zone_sources.<n>.kind` (`fixed_value`), `zone_sources.<n>.linear_coefficient` (non-zero), and at `phase_change.enabled`: no `frozen_flow`,
energy off, non-zero prescribed flux, combined with `solids` or `electric`. Not delivered (F-212 stays partial): melt flow and natural
convection, the damping in the tree, phase-dependent properties, convected latent term, steady solidification, BDF2.

### `solver.frozen_flow`

Default `false`. When `true`, momentum and pressure are not solved: the face flux is `u(x_f) . S_f` from `initial.velocity` and
`initial.velocity_expression_x/y/z` evaluated at the FACE centres. Patches whose velocity condition is `symmetry`, a wall type
(`moving_wall`, `shear`, `partial_slip`, `marangoni`) or a `fixed` zero velocity carry no flux. Located errors when the flux does not
vanish on a face touching a solid (`initial.velocity_expression_x`), when the largest cell imbalance exceeds 1e-6 of the largest
face flux (`initial.velocity`), and for solids or a prescribed flow combined with `properties` laws, radiation, a turbulence model,
the compressible / variable-density solvers, `components`, `coupled` (with `frozen_flow`), buoyancy (thermal expansion with gravity), rheology, user
functions or `fixed_value` zone sources. Available with `simple`, `simplec`, `piso`, `pimple`; steady runs iterate the energy
equation (`solver.tolerance_u` is its residual target), transient runs advance it with `solver.dt`, `solver.time_scheme`. A solved
(pressure or buoyancy driven) flow coupled to solids is available only with `solver.algorithm = "coupled"` and without `frozen_flow` (steady, laminar, constant properties; ADR 0029); the segregated and transient algorithms still need `frozen_flow` (named gap of F-051). `coupled` together with `frozen_flow` is a located error.

### `boundaries.<patch>.temperature = "convection"`

| Key | Default | Meaning |
| --- | --- | --- |
| `heat_transfer_coefficient` | `-1` (an error); `0` allowed | W/m^2/K to the free stream |
| `free_stream_temperature` | `-1`; required when `h > 0` | K |
| `wall_thickness`, `wall_conductivity` | empty = no thin wall | lists of equal length: steady series resistance `sum t_i/k_i`, no thermal capacity |
| `wall_emissivity` | `0` = no radiation | emissivity of the external surface in [0, 1]; a separate key from `emissivity` (the P1 radiation wall) so that existing cases keep their meaning |
| `radiation_temperature` | `-1`; required when `wall_emissivity > 0` | ambient K the surface radiates to |

Heat leaves through the chain half cell `dn/k` (conductivity of the owner cell), wall layers, then `h (Ts - T_inf) + eps sigma (Ts^4 -
Ta^4)` at the external surface; the nonlinear boundary equation is solved by Newton and linearised exactly, so steady runs iterate
to its exact solution. These keys on a patch whose `temperature` is not `convection` are an error. Constants only (no expression forms).
`heat_flux` and the `heat_flow` report use the conductivity of the owner cell of each face when solids exist (a report-level
`conductivity` is then an error).

## `particles`

One-way coupled Lagrangian particle tracking (F-056, ADR 0028, `docs/particles.md`). `particles.enabled = false` by default; every
numeric key has the sentinel `-1` = unset with a located error when needed. The section is read by `rcfd_api::particles`
(`track_frozen`, `track_transient`) and, since ADR 0037, by the run loop itself (see "Run path" below).

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | enable the section (needs constant `physics.rho`, `physics.nu`; not with the compressible / variable-density solvers) |
| `density` | `-1` | particle density [kg/m^3], must be set (> 0) |
| `drag` | `schiller_naumann` | `schiller_naumann`, `stokes`, `none` |
| `integration` | `analytic` | `analytic`, `euler`, `rk2`, `rk4` |
| `interpolation` | `gradient` | `gradient` (cell value + least-squares gradient) or `cell_centre` |
| `gravity` | `true` | apply `physics.gravity` with buoyancy |
| `dt` | `-1` | tracking step of `track_frozen` (`-1` = `solver.dt`) |
| `end_time` | `-1` | end of tracking [s], must be set |
| `seed` | `1` | random seed |
| `wall_interaction`, `restitution` | `reflect`, `1` | action of wall patches and restitution in [0, 1] |
| `walls.<patch>.interaction`, `.restitution` | `default`, `-1` | per-patch `reflect`, `stick`, `escape`; `-1` = `particles.restitution` |
| `injection.<n>.kind` | `unset` | `point` (`position`) or `surface` (`patch`); `unset` is an error |
| `injection.<n>.rate`, `start_time`, `end_time` | `-1`, `0`, `-1` | parcels/s (must be set), window (`-1` = `particles.end_time`) |
| `injection.<n>.velocity_mode`, `velocity` | `given`, `[]` | initial velocity (empty list = at rest) or the local fluid velocity |
| `injection.<n>.parcel_count` | `1` | real particles per parcel |
| `injection.<n>.distribution` | `constant` | `constant` (`diameter`), `rosin_rammler` (`rr_d_bar`, `rr_n`, optional `d_min`, `d_max`) or `uniform` (diameter uniform in `[d_min, d_max]`, both required; ADR 0057) |
| `injection.<n>.mass_flow` | `-1` | ADR 0057: mass flow [kg/s]; every parcel carries `mass_flow/rate` and its weight (real particles per parcel) follows from its diameter, overriding `parcel_count` (equal-mass parcels, unbiased mass distribution); `-1` = off |
| `injection.<n>.cone_half_angle`, `cone_inner_half_angle`, `cone_axis`, `cone_speed` | `-1`, `0`, `[]`, `-1` | ADR 0057: cone injection. Half angle in degrees in (0, 180], inner half angle in `[0, half)` (hollow cone), non-zero axis, speed > 0; directions uniform in solid angle, overrides `velocity_mode`; `cone_half_angle = -1` = no cone. Positions follow `kind` (point or surface) |
| `dispersion` | `none` | ADR 0057: `none` or `drw` (discrete random walk in homogeneous isotropic turbulence): eddy lifetime exponential with mean `T_L = dispersion_cl k/eps`, eddy length `0.09^0.75 k^1.5/eps`, fluctuation `sqrt(2k/3)` per component |
| `dispersion_k`, `dispersion_epsilon` | `-1`, `-1` | `drw`: k [m^2/s^2] and epsilon [m^2/s^3] (> 0, homogeneous; the turbulence-model fields are not read); located errors when unset |
| `dispersion_cl`, `dispersion_crossing`, `dispersion_seed` | `0.15`, `true`, `1` | `drw`: `C_L`, limit the interaction time by the eddy crossing time, seed of the dispersion generator (separate from `seed`) |
| `output_file`, `output_every` | `""`, `0` | VTK PolyData file written by the LIBRARY hooks (relative to the working directory), extra files every n tracking steps; the run path writes `particles.vtp` / `particles_<n>.vtp` into the output directory instead |
| `tracks` | `false` | run path: also write `particle_tracks.csv` (`id,time,x,y,z,state`) |
| `track_every` | `1` (>= 1) | `tracks = true`: log a position every n tracking steps while the parcel is active (always at injection, when it leaves, and at the end) |

Two-way coupling (F-128, ADR 0059), all keys additive and inert when unset:

| Key | Default | Meaning |
| --- | --- | --- |
| `two_way` | `false` | give the momentum the parcels receive back to the carrier as a body acceleration `impulse/(rho V T)` with the opposite sign (momentum is conserved); needs `particles.enabled`; with `simple`/`simplec`/`piso`/`pimple` only |
| `two_way_relax` | `0.5` | steady: under-relaxation of the source, `a <- a + beta (a_new - a)` from 0, `0 < beta <= 1` |
| `two_way_every`, `two_way_start` | `1`, `0` | steady: re-track every n iterations from iteration `start` (a fresh tracker over `0..end_time` on the current field; every parcel must leave or deposit by `end_time`, else a located error at `particles.end_time`) |
| `two_way_tolerance` | `1e-6` | steady: the run is converged only if the relative change of the source (monitor `res_particles`) is below this, with at least 2 updates |
| `heat_transfer`, `particle_cp`, `injection.<n>.temperature` | `false`, `-1`, `-1` | Ranz-Marshall lumped heat transfer; with `two_way` the heat the parcels give the carrier enters the temperature equation (needs `energy.enabled`, `physics.cp`, `physics.conductivity`); dissipation is not fed in; `heat_transfer` without `two_way` is a located error |

Steady source: window `W = N/R` (parcels injected per injection over the injection rate; all injections must have the same rate and window);
the source is held fixed between updates. Transient: the tracker advances over each flow step on the old-level field, the source of that
step is `impulse/(rho V h)`; the source is cleared when no force is set. Not delivered: sub-iteration of the transient exchange, the
mechanical energy exchange, `heat_transfer` without `two_way`, `frozen_flow`, `dynamic_mesh` (located errors).

Run path (F-056, ADR 0037 D1). With `particles.enabled`, `Simulation::new` builds the tracker on the simulation mesh, so an unknown
patch, an injection point outside the mesh or a bad `particles.walls.<n>` is a located error of `Simulation::new`, `rcfd run
--check-only` (exit 3) and `rcfd validate`. `particles` with `dynamic_mesh.enabled` is refused (located at `dynamic_mesh.enabled`
by the dynamic-mesh check). Transient algorithms (`piso`, `pimple`): the tracker is advanced at the end of every `Simulation::step`
over the flow step (`h = min(dt, end_time - tracker time)`), bit for bit as `track_transient` does, by every driver (`rcfd run`,
service `run.headless`/`workflow`, `run.start`). Steady algorithms and `solver.frozen_flow`: the final field is tracked once from 0
to `end_time` in steps of `particles.dt` (`solver.dt` when -1) when the results are written, as `track_frozen` does. Outputs next to
`solution.vtu`: `particles.vtp` (final state), `particles_<n>.vtp` (every `output_every`-th tracking step, written during the run
when the driver has an output directory), `particle_report.json` (`time`, `injected`, `counts {active, deposited, escaped, stuck}`,
`mean_diameter`, `mass_by_state`, `steps`) and, with `tracks`, `particle_tracks.csv`. A transient run with particles is not
restartable (the tracker state is not in the checkpoint; the run-control warning is emitted). Service method `particles.report`
(`session`, optional `with_tracks`) returns the report of the live session (a steady session is tracked on demand). The flow is
never modified: its fields are bit-identical with `particles.enabled` true or false (ADR 0037 W3).

## `mixing_plane`

Stage (mixing-plane) interface between a frozen-rotor zone and a stationary zone in the steady loop (F-126, ADR 0055, ADR 0059).
`mixing_plane.enabled = false` by default; with it off nothing changes (bit-identical). The two zones are joined only by two boundary
patches, typically the mesh generator `annulus_3d_stage`. Before every `every`-th iteration the upstream outlet faces are circumferentially
averaged in radial bands (mass, angular momentum and normal momentum flux conserved) and the state is passed to the downstream inlet
faces as a per-face fixed absolute velocity; the band pressure of the downstream inlet is passed back to the upstream outlet faces as a
per-face fixed pressure. Both are under-relaxed.

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | switch |
| `upstream_patch` | `""` | outlet patch of the rotor zone; its `[boundaries]` section needs `velocity = "zero_gradient"`, `pressure = "fixed"` (the plane sets the value), no `bc_kind` |
| `downstream_patch` | `""` | inlet patch of the stationary zone; `velocity = "fixed"` (the plane sets the value), no `bc_kind` |
| `axis`, `origin` | `[0, 0, 1]`, `[0, 0, 0]` | rotation axis (non-zero) and a point on it |
| `bands` | `-1` | number of equal radial bands (>= 1), must be set; the bands must coincide with radial cell layers of both sides |
| `r_min`, `r_max` | `-1`, `-1` | band range; `-1` = the vertex radii extremes of the two patches; both or neither, `r_max > r_min >= 0` |
| `relax` | `0.5` | under-relaxation, `0 < relax <= 1` |
| `every` | `1` | exchange interval in iterations (>= 1) |
| `tolerance` | `1e-6` | the run is converged only if the relative change of the exchange (monitor `res_mixing`) is below this, with at least 2 exchanges |

Located errors: unknown or equal patches, patches without a `[boundaries]` section, a `bc_kind` on either patch, wrong velocity or pressure
kinds, `bands` unset, `r_min >= r_max`, zero axis, `relax`, `every`, `tolerance` out of range, any algorithm other than `simple`/`simplec`,
`solver.frozen_flow`; at run time a band whose flux cancels fails the run with a message starting `mixing_plane:`. Not delivered: transient
and coupled solvers, compressible mixed-out state, rothalpy, area-fraction splitting across band edges, curved or non-aligned interfaces,
sector periodicity, turbulence or scalar transfer through the plane. Unequal numbers of cells around are allowed (the passed axial
velocity is scaled by the ratio of the two polygon areas).

## Wave 19C: `restart`, `chain`, setup rules (F-115, F-241, F-240, ADR 0031)

### `restart` (F-115)

Start the run from the solution in a restart file (the file `rcfd run` writes to `<out>/restart.rcfd`). All keys have an
explicit unset sentinel, so a case without them behaves exactly as before.

| Key | Default | Meaning |
| --- | --- | --- |
| `from` | `""` | restart file (relative to the case file); `""` = fresh start |
| `source_case` | `""` | case that built the mesh of a restart file that does not embed its mesh (files written before ADR 0031); `""` = unset |
| `method` | `gradient` | transfer method when the file is on another mesh: `gradient`, `nearest`, `inverse_distance`, `least_squares`, `conservative` (gradient plus a uniform shift matching the volume average), `conservative_overlap` (axis-aligned box cells only) |

The same mesh restores the state of the writing engine exactly. A steady file starts a transient run on the same mesh (velocity,
pressure and flux are taken over, time 0, fresh history). Another mesh transfers velocity and pressure with `method` and rebuilds
the flux from the transferred velocity; the report gives `max_flux_imbalance`, which is not zero (a divergence-free projection is
not implemented). Command line: `rcfd run --restart-from f [--source-case c] [--method m]`, `rcfd restart <case> --from f [--mesh m]`.
`rcfd run` and the chain honour `restart.from`; the service method `run.headless` does not (use `continue_from` or `interpolate_*`).

### `chain` (F-241)

Stages run one after the other; each starts from the restart file of the previous one.

| Key | Default | Meaning |
| --- | --- | --- |
| `sequence` | `[]` | stage names in run order; empty = no chain |
| `stages.<name>.set` | `[]` | settings changed for this stage and the following ones, entries `"dotted.path=<JSON value>"`; applied in order on top of the previous stage |
| `stages.<name>.restart` | `auto` | `auto`/`previous`: start from the previous stage's restart file (the first stage uses `restart.from` when set); `none`: fresh start |
| `stages.<name>.method` | `gradient` | transfer method when `set` changes the mesh |
| `stages.<name>.allow_unconverged` | `false` | continue when this steady stage ends at its iteration cap (exit 5); otherwise the chain stops with that exit code |

Every name in `sequence` needs a `[chain.stages.<name>]` table (it may be empty). Example, steady then transient:

```toml
[chain]
sequence = ["steady", "transient"]
[chain.stages.steady]
[chain.stages.transient]
set = ["solver.algorithm=\"piso\"", "solver.dt=0.05", "solver.end_time=1.0"]
```

Static errors are located: an unknown or repeated stage in `chain.sequence.<i>`, a malformed entry, an unknown setting path or a
value of the wrong type in `chain.stages.<name>.set.<i>`. Results of a stage go to `<out>/<stage>/`. `rcfd run --no-chain` runs the
base case alone. See `docs/run-control.md` for the events.

### Setup rules (F-240)

`rcfd check <case>`, `rcfd validate`, `rcfd run` and `--check-only` evaluate the data-driven rules of
`crates/rcfd-api/rules/setup_rules.json` (format `rcfd-setup-rules` version 1) on the resolved settings and the mesh quality
before anything is solved. Each diagnostic has `id`, `severity` (`error`, `warning`, `info`), `category`, a located `path`, `message`
and sometimes a suggested `fix` (never applied). Errors stop the run with exit 3; warnings go to stderr and to `warning` status events
with the rule id. The 21 rules (SV001-SV013 errors, SV101-SV105 warnings, SV201-SV203 infos) and the grammar are in ADR 0031.

## Wave 20C: `study` (F-095, ADR 0034)

A parametric study is part of the settings tree; an empty `study.parameters` means no study and every older case is unchanged.
`rcfd study <case>` runs it (see `docs/studies.md`). All keys have explicit unset values.

| key | type, default | meaning |
| --- | --- | --- |
| `study.outputs` | list of strings, `[]` | report names that become table columns; empty = all reports, sorted |
| `study.combine` | `grid` (default) or `zip` | cartesian product (first parameter name, alphabetically, slowest) or parameters walking together |
| `study.parallel` | int, 1 | design points run at the same time |
| `study.write_results` | bool, true | write each point's result files to `<out>/points/pNNNN/` |
| `study.parameters.<name>.path` | string, `""` | settings path the parameter overrides (any path `--set` accepts) |
| `study.parameters.<name>.kind` | `list` (default), `linspace`, `logspace` | how the values are produced |
| `study.parameters.<name>.values` | list of floats, `[]` | `list`: numeric values |
| `study.parameters.<name>.strings` | list of strings, `[]` | `list`: string values (enums, file names) |
| `study.parameters.<name>.json` | list of strings, `[]` | `list`: values as JSON texts, for vectors and booleans |
| `study.parameters.<name>.start`, `.stop`, `.count` | float, float, int; 0 | `linspace`, `logspace` (geometric between the two values, both > 0) |

Static errors are located (exit 3, nothing runs): `study.parameters.<n>.path` (empty, unknown setting, the study's own section, a
path swept twice), `.values.<i>` / `.strings.<i>` / `.json.<i>` (value rejected by the setting's type, enum or range), `.count`
and `.start` (sweep definition), `study.combine` (zip lengths), `study.outputs.<i>` (no such report), `study.parallel`,
`study.parameters` (no parameters for `rcfd study`). `Case::set` now also creates named entries nested deeper than one level
(`--set study.parameters.k.path=...`).

## Wave 20B: `dynamic_mesh` (F-063, F-229, ADR 0033)

Moving mesh for incompressible laminar `piso` / `pimple` runs with `solver.time_scheme = "euler"`; see `docs/dynamic-mesh.md`. Every key is inert while
`dynamic_mesh.enabled = false` (the default) and older cases are unchanged. Unset values: empty strings and lists, 0.

| key | type, default | meaning |
| --- | --- | --- |
| `dynamic_mesh.enabled` | bool, false | move the mesh |
| `dynamic_mesh.motion` | `prescribed` (default), `six_dof` | source of the motion of the moving patches |
| `dynamic_mesh.deformation` | `rigid` (default), `smoothing` | all points follow, or the interior is smoothed |
| `dynamic_mesh.moving_patches` | list of strings, `[]` | patches that carry the motion (required) |
| `dynamic_mesh.follow_patches` | list of strings, `[]` | patches whose velocity is the mesh velocity (empty = the moving patches); their `velocity` must stay `"fixed"` and their `velocity_value` only sets the initial flux |
| `dynamic_mesh.fixed_patches` | list of strings, `[]` | smoothing: patches whose points do not move; all other non-moving patches slide in their plane |
| `dynamic_mesh.displacement_expression_x`, `_y`, `_z` | string, `""` | displacement in m, expression of `t` (e.g. `"1 [m/s^2] * t^2"`); empty = 0 |
| `dynamic_mesh.rotation_angle_expression` | string, `""` | dimensionless angle in radian, expression of `t` |
| `dynamic_mesh.rotation_origin`, `.rotation_axis` | list of 3 floats, `[]` | point on the axis (default origin), axis (default z) |
| `dynamic_mesh.smoothing.method` | `laplacian` (default), `inverse_distance` | diffusivity 1 or `1/l^exponent` (distance to the nearest moving face) |
| `dynamic_mesh.smoothing.exponent` | float, 1 | exponent of the inverse-distance diffusivity (not negative) |
| `dynamic_mesh.smoothing.substeps` | int, 1 | reserved; only 1 is accepted |
| `dynamic_mesh.smoothing.tolerance` | float, 1e-12 | relative tolerance of the displacement solve |
| `dynamic_mesh.six_dof.mass` | float, 0 | kg, must be > 0 for `motion = six_dof` |
| `dynamic_mesh.six_dof.inertia` | list, `[]` | 3 principal moments or 6 components `xx yy zz xy xz yz`, kg m^2 |
| `dynamic_mesh.six_dof.position`, `.velocity`, `.angular_velocity` | list of 3, `[]` | initial centre of mass, velocity, angular velocity |
| `dynamic_mesh.six_dof.orientation_axis`, `.orientation_angle` | list of 3, `[]`; float, 0 | initial orientation (empty axis = identity) |
| `dynamic_mesh.six_dof.translation_locked` | list of bool, `[]` | locked world axes (empty = none) |
| `dynamic_mesh.six_dof.rotation` | `locked` (default), `free`, `about_axis`, `hinge` | rotational freedom |
| `dynamic_mesh.six_dof.axis`, `.pivot` | list of 3, `[]` | axis for `about_axis` / `hinge`; a point on the hinge axis |
| `dynamic_mesh.six_dof.stiffness`, `.damping`, `.rest_position` | list of 3, `[]` | translational spring and damper per world axis |
| `dynamic_mesh.six_dof.torsion_stiffness`, `.torsion_damping`, `.torsion_rest_angle` | float, 0 | torsional spring and damper (single-axis rotation) |
| `dynamic_mesh.six_dof.gravity` | bool, false | weight from `physics.gravity` |
| `dynamic_mesh.six_dof.force_patches` | list of strings, `[]` | patches the flow force is integrated on (empty = moving patches) |
| `dynamic_mesh.six_dof.integrator` | `rk4` (default), `newmark` | body integrator (`newmark` not for free rotation) |

Static errors are located at the key (unknown patch names, expression syntax and dimension, zero rotation axis with a rotation expression, negative exponent,
`six_dof` without positive mass and inertia, wrong list lengths); combinations that are not implemented (`solver.algorithm` other than `piso` / `pimple`, a
`time_scheme` other than `euler`, turbulence, energy, scalars, particles, momentum sources, rheology, statistics, restart) are reported at `dynamic_mesh.enabled`.
A mesh that becomes invalid stops the run: `dynamic_mesh: step N, t = ...: mesh error: cell i has non-positive volume ...`.

## Wave 21B: `regions` (F-216, ADR 0036)

`regions.<name>` assigns a model to a cell zone. Keys: `zone` = `all` (default) | `box` | `cell_zone` | `register` with
`box_min`, `box_max`, `cell_zone`, `register` as for `zone_sources`; `turbulence` = `model` (default, the global
`turbulence.model` acts) | `laminar`. A laminar region sets the momentum-equation face viscosity to the molecular value
`physics.nu` on every face that touches a cell of the region (boundary faces by their owner cell); the turbulence transport
equations are still solved there, and the energy, radiation and species models are not region-scoped. Located errors:
`regions.<n>.turbulence` (unknown value, no turbulence model configured, Reynolds stress or plugin model, rheology or a
temperature-dependent viscosity), `regions.<n>.zone` (no cells), `regions.<n>.cell_zone`, `regions.<n>.register`.
Without any `regions` entry nothing changes (recorded baseline hashes are bit-identical).

## Wave 22: `linear_solver` (F-101, ADR 0038)

`linear_solver` selects where the segregated pressure solve (`simple`, `piso`, `pimple`) runs. See `docs/gpu.md`.

| Key | Value | Meaning |
| --- | --- | --- |
| `linear_solver.device` | `cpu` (default), `cuda`, `auto` | `cpu`: unchanged CPU path (bit-identical to a case without the section). `cuda`: preconditioned CG on an NVIDIA device; a missing device or toolkit is a located error. `auto`: the device for 100000 unknowns or more when one is usable, otherwise the CPU path (the reason is recorded) |
| `linear_solver.preconditioner` | `jacobi` (default), `chebyshev` | preconditioner of the device CG (the CPU path keeps its own preconditioners) |
| `linear_solver.chebyshev_degree` | int, 4 (0..32) | number of Chebyshev terms; only with `preconditioner = "chebyshev"` |

Unset keys keep their default. Errors are located at the key (`linear_solver.device`, `linear_solver.chebyshev_degree`). `device = cuda` or
`auto` with `solver.algorithm = coupled`, a compressible/energy setting or a non-constant density is a semantic error at
`linear_solver.device`. A build without the cargo feature `cuda` reports `device = cuda` at `linear_solver.device`; `auto` falls back to the CPU.

## Wave 27C: viscoelastic models and structural load export (F-135, F-237, ADR 0051)

No case-tree keys were added. The viscoelastic constitutive library and coupled channel solver (`rcfd_fv::viscoelastic`)
and the structural load export (`rcfd_post::structural`, `rcfd_io::nastran`, `rcfd_io::registry`) are library-level APIs
only, so existing cases parse, validate and run exactly as before (baseline hashes unchanged). Their errors are located
by the API argument (model parameter, patch name, element or grid id, deck line number), not by a case key. A case-tree
section is future work.

## Wave 27A: `multiphase` (F-055, ADR 0049)

`multiphase.enabled = true` selects the volume-of-fluid free-surface solver (`rcfd_fv::vof`) in place of the single-phase transient
kernel: two incompressible immiscible fluids, the phase fraction `alpha` (1 = phase 1) transported with interface compression and a
flux-corrected explicit limiter, sub-cycled to a Courant number `alpha_courant`; mixture density and viscosity linear in `alpha`;
CSF surface tension; well-balanced `p_rgh` gravity. Everything is inert and silent while `enabled = false`; setting any
`multiphase.*` key then is a located error. A case without the section is unchanged (baseline hashes bit-identical).

| Key | Default | Meaning |
|-----|---------|---------|
| `multiphase.enabled` | false | solve the two-phase free-surface flow |
| `multiphase.model` | "vof" | only the algebraic volume of fluid |
| `multiphase.density_1/2` | -1 (unset) | phase densities [kg/m^3], required > 0 |
| `multiphase.viscosity_1/2` | -1 (unset) | dynamic viscosities [Pa s], required > 0 |
| `multiphase.surface_tension` | 0 | sigma [N/m]; CSF, 90 degree contact angle at walls |
| `multiphase.compression` | 1 | compression coefficient `c_alpha` (0 = none) |
| `multiphase.alpha_courant` | 0.25 | largest Courant number of an alpha sub-step |
| `multiphase.curvature_smoothing` | 2 | smoothing passes of alpha before the curvature |
| `multiphase.initial.shape` | "unset" | `box` (`box_min`, `box_max`), `sphere` (`centre`, `radius`), `half_space` (`normal`, `point`: the side `n . (x - point) <= 0`); required |
| `multiphase.initial.inside` | "phase_1" | which phase fills the shape; the other fills the rest |

Cells are sub-sampled 10 per direction, so a face-aligned box is exact; in 2-D give the box a z range that covers the mesh thickness.
Gravity is `physics.gravity`; the phase densities replace `physics.rho` and the viscosities `physics.nu`. The pressure is in Pa;
`field("p")` is the pressure, `field("p_rgh")` the modified pressure `p - rho g . (x - x_ref)` (also what `pressure()` returns and what
a `pressure = "fixed"` boundary value sets), `field("alpha")` and `field("rho")` are new; `alpha`, `p_rgh`, `p`, `rho` are written to
`solution.vtu`. The time step is the fixed `solver.dt`; a time step above the capillary limit is flagged in the library
(`capillary_dt`), not in the tree.

Located errors (path in the tree) when `multiphase.enabled = true`: `solver.algorithm` not `piso`/`pimple` (steady, coupled and
compressible are refused); `fluid.density` not `constant`; `turbulence.model`, `energy.enabled`, `rheology.model`, `properties`,
`scalars`, `sources`, `components`, `radiation.model`, `statistics.enabled`, `dynamic_mesh.enabled`, `particles.enabled`, `chemistry`,
`electric.enabled`, `phase_change.enabled`, `solids`, `zone_sources`; `solver.time_advance = fractional_step`; `solver.max_courant > 0`;
`solver.time_scheme` not `euler`; `solver.viscous_stress = full`; `solver.bulk_velocity`; `solver.frozen_flow`; `physics.body_force`;
`mesh.periodic_x/y`; non-default `physics.rho` / `physics.nu`; a boundary velocity other than `fixed` (no-slip: value 0) or
`zero_gradient` (so no `symmetry`, `partial_slip`, `marangoni`, `freestream`, `moving_wall`, `shear`), a pressure other than `fixed` or
`zero_gradient`, a catalogue boundary kind; unset required keys. Reports that assume one density (force, moment, y_plus, heat_flow,
mass_flow, mass_weighted_average, vector_flux) are refused. The pressure solve is preconditioned CG, with smoothed-aggregation AMG
as the preconditioner above 2000 cells (wave 28A: the earlier breakdown on the 1000:1 density jump was a zero filtered diagonal in the
AMG setup, fixed in `rcfd-linalg`; ADR 0052 addendum 1).

## Wave 28A: `multiphase.mass_transfer`, `multiphase.waves`, `multiphase.reference_point` (F-226, F-227, ADR 0052)

All of these are inert (silent at their defaults, a located error when set) unless `multiphase.enabled = true` and the model or the
wave section is switched on.

| Key | Default | Meaning |
|-----|---------|---------|
| `multiphase.reference_point` | [] (origin) | `x_ref` of the gravity potential `g . (x - x_ref)`; for a tank with an open top give a point on that boundary so that still water is in equilibrium with `p_rgh = 0` there |
| `multiphase.mass_transfer.model` | "none" | `none`, `lee` or `hertz_knudsen` |
| `multiphase.mass_transfer.liquid_phase` | "phase_1" | which phase is the liquid (the other is its vapour) |
| `multiphase.mass_transfer.saturation_temperature` | -1 (unset) | `T_sat` [K], required > 0 with a model |
| `multiphase.mass_transfer.latent_heat` | -1 (unset) | `h_fg` [J/kg], required > 0 with a model; reported only, see below |
| `multiphase.mass_transfer.temperature` | -1 (unset) | prescribed temperature [K] at `temperature_point`, required > 0 with a model |
| `multiphase.mass_transfer.temperature_gradient` | [] | constant gradient [K/m] `[gx, gy, gz]`; empty = uniform |
| `multiphase.mass_transfer.temperature_point` | [] (origin) | where the temperature equals `temperature` |
| `multiphase.mass_transfer.evaporation_frequency`, `condensation_frequency` | 0 | `lee` only: `c_evap`, `c_cond` [1/s] |
| `multiphase.mass_transfer.accommodation`, `schrage` | 1, false | `hertz_knudsen` only: accommodation coefficient in (0, 1], Schrage factor |
| `multiphase.mass_transfer.molar_mass`, `vapour_pressure`, `saturation_pressure` | -1 (unset) | `hertz_knudsen` only: `M` [kg/mol], vapour-side pressure [Pa], `p_sat` at `T_sat` [Pa]; `p_sat(T)` is Clausius-Clapeyron with the constant `h_fg` |
| `multiphase.waves.inlet_patch` | "" | boundary patch at the smallest `x` that generates the wave; its velocity must be `fixed` (replaced every step) |
| `multiphase.waves.height`, `period`, `depth` | -1 (unset) | wave height `H`, period `T`, still-water depth `h` [m, s, m]; required > 0 with an inlet |
| `multiphase.waves.level` | -1 (= `depth`) | `y` of the still-water surface |
| `multiphase.waves.order`, `phase`, `ramp_periods` | 1, 0, 2 | 1 = Airy, 2 = second-order Stokes; phase offset [rad]; start-up ramp length in periods |
| `multiphase.waves.beach` | "none" | `still_water`: relaxation zone that blends `alpha` and `u` toward still water |
| `multiphase.waves.beach_x_inner`, `beach_x_outer` | 0, 0 | `x` where the computed solution has weight 1 / the target has weight 1; must differ |
| `multiphase.waves.beach_blend`, `beach_exponent` | "exponential", 3.5 | weight shape `1 - (exp(chi^p) - 1)/(e - 1)` or `(1 - chi)^n` |

Mass transfer. The liquid fraction loses `mdot dt / rho_l` (Lee: the exponential integrator of the frozen-temperature relaxation
`d alpha / dt = -c theta alpha`, `theta = (T - T_sat)/T_sat`, so `alpha` stays in [0, 1]; Hertz-Knudsen: the kinetic flux times
`|grad alpha|`), and the pressure equation receives the volume source `mdot (1/rho_vapour - 1/rho_liquid)`. The volume has to leave,
so at least one boundary needs `pressure = "fixed"` (located error otherwise). LIMITATIONS: there is NO energy equation in the
multiphase path. The temperature is a prescribed field (uniform plus a constant gradient); the latent heat `h_fg` is used for the
Clausius-Clapeyron `p_sat` and for a reported latent power only, there is no latent-heat feedback on the temperature, no Stefan problem
or boiling curve is solved, and the momentum equation has no phase-change term in `rho phi`. The rate is limited by the liquid or
vapour available in the cell (clipping, ADR 0052 addendum 2).

Waves. 2-D tank, propagation along +x, gravity along -y (any other gravity is a located error on `physics.gravity`), phase 1 the
water. The inlet sets per-face velocity and `alpha` from the linear or second-order Stokes kinematics (`rcfd_fv::waves`) with the
ramp; the relaxation zone is applied after the pressure correction as `w * computed + (1 - w) * target` with the weights of the
wave library, and the volume it adds or removes is reported (`relax_volume`) so that the volume balance stays an identity. Use
`multiphase.reference_point` on the open top. Not delivered: irregular and directional waves in the tree, a wave-making piston,
three-dimensional tanks, a relaxation target other than still water in the tree, and a general-direction inlet.

## Wave 29B: `multiphase.mixture` and `multiphase.cavitation` (F-127, ADR 0056)

Both sections are additive and inert (silent at their defaults, a located error when set) unless `multiphase.enabled = true` and the model
is not `none`. A case without them is unchanged (baseline hashes bit-identical).

| Key | Default | Meaning |
|-----|---------|---------|
| `multiphase.mixture.model` | "none" | `none` or `drift_flux` (algebraic-slip mixture model) |
| `multiphase.mixture.dispersed_phase` | "phase_2" | which phase is dispersed (particles, droplets, bubbles); the other is continuous |
| `multiphase.mixture.particle_diameter` | -1 (unset) | `d_p` [m], required > 0 with `drift_flux` |
| `multiphase.mixture.drag` | "stokes" | `stokes` or `schiller_naumann` (`C_D = 24/Re (1 + 0.15 Re^0.687)`) |
| `multiphase.cavitation.model` | "none" | `none` or `schnerr_sauer` |
| `multiphase.cavitation.liquid_phase` | "phase_1" | which phase is the liquid (the other is its vapour) |
| `multiphase.cavitation.saturation_pressure` | -1 (unset) | `p_sat` [Pa], required > 0 with a model |
| `multiphase.cavitation.bubble_number_density` | -1 (unset) | `n` per unit volume of liquid [1/m^3], required > 0 with a model |
| `multiphase.cavitation.nucleus_radius` | -1 (unset) | `R_0` [m], required > 0: smallest bubble radius and inception seed |
| `multiphase.cavitation.vaporisation_factor`, `condensation_factor` | 1, 1 | multipliers of the evaporation and condensation rates (>= 0) |

Mixture model. The phase fraction is transported with the extra slip flux `alpha (1 - alpha) v_12` (Godunov low-order flux, central
high-order flux, the existing Zalesak limiter; the sub-step Courant number includes the slip signal speed) and the momentum equation gets
the drift mass flux and the drift stress, `-div(j (x) J + T)`. The slip `v_12` is the gravity-driven single-particle speed with the
mixture buoyancy `(rho_d - rho_m) = (1 - alpha_d)(rho_d - rho_c)` and Stokes or Schiller-Naumann drag (Newton iteration). LIMITATIONS:
only gravity drives the slip (no centrifugal or inertial terms), the time derivative of the drift mass flux is dropped, no turbulent
dispersion, lift, virtual mass or wall lubrication, and no particle-size distribution. Zero slip (equal densities) reduces to the
plain VOF path bit for bit. Located errors: `multiphase.mixture.particle_diameter` (unset or not positive), `physics.gravity` (zero),
`multiphase.mixture.model` (with thermal `mass_transfer` or with `cavitation`, both untested combinations). NOT a multi-fluid
(per-phase momentum) Euler-Euler model.

Cavitation. Schnerr-Sauer with a bubble-number density: `alpha_v = x/(1 + x)`, `x = n (4/3) pi R^3`, inertia-controlled growth
`R' = sign(p_sat - p) sqrt(2 |p_sat - p|/(3 rho_l))`, vapour mass source `mdot = rho_v rho_l/rho_m alpha_v (1 - alpha_v) (3/R) R'`.
The source is coupled IMPLICITLY to the pressure equation: it is linearised by the chord to the saturation point, the slope goes on the
diagonal of the pressure matrix and the linearisation is repeated until the delivered source equals the exact one to 1e-10 of a vapour
fraction per step (at most 60 repetitions per pressure correction), so the pressure in a cavity sits at `p_sat` and the sign of the
source is that of `p_sat - p` at the solved pressure. The vapour fraction never leaves `[alpha_floor, 1]`, `alpha_floor = x_0/(1 + x_0)`
(every cell starts with the nuclei). The phase fraction is transported in advective form (`alpha_t + div(alpha phi) - alpha div phi`) when
cavitation is on. The pressure is the absolute `p = p_rgh + rho g . (x - x_ref)`. LIMITATIONS: isothermal (no thermal effect on `p_sat`),
no vapour compressibility, no non-condensable gas, no turbulence correction of the pressure, no surface tension in the growth law
(Rayleigh without Plesset terms), no bubble-number transport; the flow Courant number must stay below about 1 through a cavity collapse
(the incompressible liquid slug accelerates to several times the inlet speed; the registered `dt` did not). Located errors:
`multiphase.cavitation.saturation_pressure`, `bubble_number_density`, `nucleus_radius` (unset or not positive),
`multiphase.cavitation.model` (no `pressure = "fixed"` boundary for the volume to leave through; combined with `mass_transfer`).

## Divergence guard of the transient solvers (`solver.divergence_courant_limit`)

`solver.divergence_courant_limit` (float, >= 0, default 0) applies to `solver.algorithm = "piso"` and `"pimple"` (ADR 0067).
The solver stops a run at the first step whose velocity, predicted flux, pressure or flux is not finite, or whose Courant number is
not finite or exceeds the limit. The value 0 (unset) means the built-in limit 1e6; a positive value replaces it (`1e300` disables
the Courant criterion in practice; a non-finite state is always fatal). The run ends with the state `diverged`, exit code 6 and
a located message that names the quantity, step, outer iteration, stage and cell. For finite runs below the limit the checks only
read the state, so results are bit-identical to runs without the key.

## Divergence guard of the steady solvers (`solver.steady_divergence_guard`)

`solver.steady_divergence_guard` (`on` default, `off`) applies to `solver.algorithm = "simple"`, `"simplec"` and `"coupled"` (ADR 0077). After every iteration
the guard checks: any non-finite residual, continuity, cell velocity, cell pressure or face flux (always, also with `off`); with `on`, from the second
iteration, the continuity exceeding 1e6 times the larger of its smallest value over the previous 50 iterations and 1e-6 `U_ref`, the largest cell speed
exceeding 1e6 `U_ref`, or the pressure range exceeding 1e12 `P_ref` (`U_ref` the largest cell or boundary-face speed after the first iteration,
`P_ref = max(U_ref^2, pressure range after the first iteration)`; a zero reference switches its criteria off). The guard records the first trip (kind
`non_finite`, `continuity_growth`, `velocity_runaway` or `pressure_runaway`, the quantity, the cell and its position, the value, the limit, and the last
good iteration = the one with the smallest continuity before the trip). The run-control driver (`rcfd run`, the `run.headless` service and MCP tool, the
design loop) ends the run after that iteration with state `diverged`, exit code 6, the located message as reason and the record in the `finish` event
(`divergence`, see `docs/run-control.md`). A simulation stepped directly (GUI, library) keeps iterating and exposes the record
(`Simulation::steady_divergence`). The checks only read the state, so results are bit-identical with `on` and `off`.

## Pressure-lag check of PIMPLE (`solver.pressure_lag_check`)

`solver.pressure_lag_check` (`warn` default, `raise`, `off`) applies to `solver.algorithm = "piso"` and `"pimple"` (not the fractional step, not dynamic
meshes; ADR 0073 B4, ADR 0077) with `solver.n_correctors = 1`, the regime of the ADR 0073 model (with more correctors no estimate is made: in two-corrector
runs at large diffusion numbers, including a fluid at rest, the estimate exceeded 1 where nothing grew, ADR 0077 amendment 3). One estimate per run of the
per-step amplification of the lagged pressure error (step 1; step 2 with `bdf2` and `bdf2_bounded`, whose first step is an implicit Euler step). `warn`: an
amplification of at least 1 gives a `warning` event (`kind = "pressure_lag"`, `step`, `rho`, `cell`, `position`, and a message with the remedy
`solver.n_correctors >= 2` or a smaller `solver.dt`); the solution is unchanged. `raise`: the run stops (state `diverged`, exit 6). `off`: no estimate.
The estimate costs about as much as 15 steps, once per run (measured on a 48 x 48 PISO cavity).

## Least-squares velocity gradient on periodic faces (`solver.periodic_gradient`)

`solver.periodic_gradient` (`face_value` default, `partner_cell`) applies to the steady `simple`/`simplec` momentum gradient (used by gradient-based
convection schemes and non-orthogonal correctors; ADR 0077 F-H0). `face_value` takes the interpolated face value at the face centre; where the cells on
the two sides of a periodic seam are not aligned (a skewed blade-passage sector), the gradient of a seam cell is then wrong by O(1) of the gradient even
for a linear field (0.17 of it on the FAN-01 sector mesh). `partner_cell` uses the partner cell mapped into the owner's frame and is exact for linear
fields. Opt-in; not used by the transient kernel, scalars or turbulence equations.

### Momentum convection on high-aspect, strongly non-orthogonal wall cells (ADR 0077 part B)

With the steady solvers, `linear_upwind` and `tvd_van_leer` diverged from the first iterations on the FAN-01 H-slit sector mesh (13 332 cells, wall cells of
aspect ratio up to 944 with face non-orthogonality up to 79 degrees), laminar and with SST; `upwind` and `tvd_minmod` were stable. The divergence starts in the
wall cells with the largest aspect ratio times the tangent of their non-orthogonality: the unlimited extrapolation `grad(u) . (x_f - x_U)` over the long side of
such a cell turns the near-wall velocity jump into face values tens of times the largest boundary speed. Use `upwind` or `tvd_minmod` on such meshes, or
`linear_upwind` with `solver.linear_upwind_limiter = "barth_jespersen"` (below; ADR 0081).

## Limited linear-upwind extrapolation (`solver.linear_upwind_limiter`)

`solver.linear_upwind_limiter` (`none` default, `barth_jespersen`) applies to `solver.convection = "linear_upwind"` with `solver.algorithm = "simple"` or
`"simplec"` (incompressible, constant density); with any other algorithm or scheme a value other than `none` is a located error (ADR 0081 part L).
`barth_jespersen` multiplies the least-squares gradient of each velocity component by its Barth-Jespersen factor before the face extrapolation
`u_U + grad(u)_U . (x_f - x_U)`, so the face value stays within the range of the upwind cell's own and neighbour values (boundary values included); the
explicit non-orthogonal correction of the momentum Laplacian keeps the unlimited gradient. Turbulence, scalar and energy equations are not limited.
Measured (ADR 0081): stable on the FAN-01 sector mesh, laminar and with SST, where the unlimited scheme diverges at iteration 7; the converging cases
checked (a lid cavity, the MRF Taylor-Couette annulus across a non-conformal interface, the shipped fan sector) converge or stay stable as without it,
and with the limiter forced on in all 542 steady `linear_upwind` runs of the test suite no run tripped the divergence guard that does not trip without it;
second order is kept on the Kovasznay flow (L2 order 2.17 on the finest pair, error within 1 % of the unlimited scheme). Not checked: smooth flows with
interior extrema of a velocity component, where the limiter clips. Opt-in because it moves every `linear_upwind` result.

## Design-loop files (not part of the case schema)

Operating maps (`rcfd map`), design studies and the `design_*` tools do not add a section to the case: a map or a study is a driver
that overrides settings of a base case (`rcfd map <case> --axis path=v1,v2,...`; `study.json` next to `evaluations.jsonl` in a study
directory). Their formats are described in `docs/design-loop.md`. The existing `study` section (ADR 0034) is unchanged.
