//! The case settings schema: the single source of truth for the case file,
//! the JSON Schema, the CLI, the service and the MCP tools (F-080).

use rcfd_core::settings::Schema;
use rcfd_core::units::Dimension;

/// Convection scheme names accepted by `solver.convection`.
pub const CONVECTION_SCHEMES: &[&str] = &[
    "upwind",
    "central",
    "linear_upwind",
    "quick",
    "tvd_minmod",
    "tvd_van_leer",
    "tvd_van_albada",
    "tvd_superbee",
    "tvd_muscl",
    "tvd_koren",
    "blended",
    "high_resolution",
    "compressive",
    "bounded_central",
];

/// Names of fields that can be requested from a simulation.
pub const FIELD_NAMES: &[&str] = &[
    "ux",
    "uy",
    "uz",
    "speed",
    "p",
    "T",
    "G",
    "vorticity_z",
    "k",
    "epsilon",
    "omega",
    "nut",
    "wall_distance",
    "y_plus",
    "rho",
    "mach",
];

/// Limiter names accepted by `compressible.reconstruction` (besides `first_order`).
pub const RECONSTRUCTIONS: &[&str] = &[
    "first_order",
    "tvd_minmod",
    "tvd_van_leer",
    "tvd_van_albada",
    "tvd_superbee",
    "tvd_muscl",
    "tvd_koren",
];

/// Riemann flux names accepted by `compressible.flux`.
pub const COMPRESSIBLE_FLUXES: &[&str] = &["rusanov", "roe", "hllc", "ausm_plus"];

fn vec3(default: [f64; 3], doc: &str) -> Schema {
    // A list of three numbers; the length is checked by semantic validation.
    let _ = default;
    Schema::list(Schema::float(0.0)).doc(doc)
}

/// The complete case schema.
pub fn case_schema() -> Schema {
    let mesh = Schema::object()
        .doc("Computational mesh: a built-in generator or a mesh file")
        .field("fan", crate::fan_case::schema())
        .field(
            "source",
            Schema::enumeration("generator", &["generator", "file"]).doc("Where the mesh comes from"),
        )
        .field(
            "generator",
            Schema::enumeration(
                "cavity_2d",
                &["cavity_2d", "box_2d", "channel_2d", "channel_2d_interface", "cylinder_ogrid", "wedge_2d", "nozzle_2d", "annulus_3d", "annulus_3d_interface", "annulus_3d_stage", "pipe_sweep", "axial_fan_sector"],
            )
            .doc("Built-in generator (used when source = generator); wedge_2d is a box_2d whose lower wall turns by ramp_angle at ramp_start (patches xmin, xmax, ymin, ymax); nozzle_2d is the upper half of a converging-diverging duct (box_2d with a cosine wall profile from the inlet half-height ly through the throat half-height throat_height to exit_height at xmax; patches xmin, xmax, ymin = centre line, ymax = wall; F-061); channel_2d_interface is a channel_2d made of two blocks of nx x ny and nx_right x ny_right cells that meet at x = lx/2 through a non-conformal interface (F-114; patches inlet, outlet, walls, sides); annulus_3d is a full cylindrical annulus r_in <= r <= r_out about the z axis, height lz, nx cells around, ny radial, nz axial (patches inner, outer, zmin, zmax; F-031); annulus_3d_interface is two such blocks (heights lz/2) joined at z = lz/2 through a non-conformal interface, the upper block having ny_right x nz_right cells (0 = same as ny / nz); annulus_3d_stage is the same two blocks NOT connected (patches inner and outer of both blocks, zmin, plane_out = top of the lower block, plane_in = bottom of the upper block, zmax; the upper block has nx_right x ny_right x nz_right cells, 0 = same as nx / ny / nz) for a mixing plane (mixing_plane section, F-126, ADR 0059); pipe_sweep is an all-hexahedral circular pipe along z of radius r_out and length lz made by sweeping a multizone O-grid + core disc (nx x nx Cartesian core of half-width core_fraction * r_out, four ring sectors of nx x ny cells, 4 nx cells around, nz axial layers, grading = last/first ring cell size ratio; patches wall, zmin, zmax; periodic_z makes zmin/zmax a cyclic pair; F-235, ADR 0047); axial_fan_sector is a periodic single blade passage of an axial fan (hex mesh with a blunt axially truncated blade and boundary-layer stacks, settings in mesh.fan, ADR 0065; patches inlet, outlet, hub, shroud, blade and the rotational pair seam_lo / seam_hi; cell zone rotor); a lower-fidelity mesh, not validated for accurate fan performance prediction"),
        )
        .field(
            "nx",
            Schema::int(20)
                .range(1.0, 100000.0)
                .doc("Cells in x (theta for cylinder_ogrid)"),
        )
        .field(
            "ny",
            Schema::int(20)
                .range(1.0, 100000.0)
                .doc("Cells in y (radial for cylinder_ogrid)"),
        )
        .field(
            "nx_right",
            Schema::int(0)
                .range(0.0, 100000.0)
                .doc("Cells in x of the second block of channel_2d_interface (0 = same as nx); cells around the upper block of annulus_3d_stage (0 = same as nx)"),
        )
        .field(
            "ny_right",
            Schema::int(0)
                .range(0.0, 100000.0)
                .doc("Cells across the channel in the second block of channel_2d_interface (0 = same as ny); a different count makes the interface non-conformal"),
        )
        .field(
            "nz",
            Schema::int(1).range(1.0, 100000.0).doc("Axial cells of annulus_3d (and the lower block of annulus_3d_interface); axial layers of pipe_sweep"),
        )
        .field(
            "nz_right",
            Schema::int(0)
                .range(0.0, 100000.0)
                .doc("Axial cells of the upper block of annulus_3d_interface and annulus_3d_stage (0 = same as nz)"),
        )
        .field(
            "lz",
            Schema::float(1.0).at_least(1e-12).dim(Dimension::LENGTH).doc("Axial length of annulus_3d / annulus_3d_interface"),
        )
        .field(
            "lx",
            Schema::float(1.0)
                .at_least(1e-12)
                .dim(Dimension::LENGTH)
                .doc("Length in x"),
        )
        .field(
            "ly",
            Schema::float(1.0)
                .at_least(1e-12)
                .dim(Dimension::LENGTH)
                .doc("Length in y"),
        )
        .field(
            "depth",
            Schema::float(0.1)
                .at_least(1e-12)
                .dim(Dimension::LENGTH)
                .doc("Thickness of 2D meshes"),
        )
        .field(
            "r_in",
            Schema::float(0.5)
                .at_least(1e-12)
                .dim(Dimension::LENGTH)
                .doc("Inner radius (cylinder_ogrid)"),
        )
        .field(
            "r_out",
            Schema::float(10.0)
                .at_least(1e-12)
                .dim(Dimension::LENGTH)
                .doc("Outer radius (cylinder_ogrid)"),
        )
        .field(
            "grading",
            Schema::float(1.0)
                .at_least(1e-6)
                .doc("Last/first cell size ratio in the radial or y direction (1 = uniform)"),
        )
        .field(
            "periodic_x",
            Schema::boolean(false).doc("Make xmin/xmax cyclic (box_2d)"),
        )
        .field(
            "periodic_y",
            Schema::boolean(false).doc("Make ymin/ymax cyclic (box_2d)"),
        )
        .field(
            "periodic_z",
            Schema::boolean(false).doc("Make zmin/zmax cyclic (pipe_sweep)"),
        )
        .field(
            "core_fraction",
            Schema::float(0.5)
                .range(0.2, 0.8)
                .doc("Half-width of the Cartesian core of the pipe_sweep cross section as a fraction of the pipe radius"),
        )
        .field(
            "ramp_start",
            Schema::float(0.2)
                .at_least(0.0)
                .dim(Dimension::LENGTH)
                .doc("x position where the lower wall starts to rise (wedge_2d)"),
        )
        .field(
            "throat_height",
            Schema::float(0.05)
                .at_least(1e-12)
                .dim(Dimension::LENGTH)
                .doc("Half-height of the throat at x = lx/2 (nozzle_2d)"),
        )
        .field(
            "exit_height",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::LENGTH)
                .doc("Half-height at xmax (nozzle_2d); 0 = ly"),
        )
        .field(
            "ramp_angle",
            Schema::float(10.0)
                .range(0.0, 60.0)
                .doc("Ramp angle in degrees (wedge_2d)"),
        )
        .field(
            "wall_layers",
            Schema::int(0)
                .range(0.0, 1000.0)
                .doc("Number of boundary-layer inflation layers in y at the wall(s) (box_2d, channel_2d; 0 = off). Layer heights grow by layer_growth from first_layer_height (or the height derived from target_y_plus); the remaining cells grow smoothly to fill ly"),
        )
        .field(
            "first_layer_height",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::LENGTH)
                .doc("Height of the wall-adjacent cell; 0 = derive it from target_y_plus"),
        )
        .field(
            "target_y_plus",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Target y+ of the wall-adjacent cell centre when first_layer_height is 0: h1 = 2 y+ nu / u_tau, u_tau from layer_u_tau or from the flat-plate skin friction at layer_reference_velocity and layer_reference_length"),
        )
        .field(
            "layer_growth",
            Schema::float(1.2)
                .range(1.0, 3.0)
                .doc("Growth ratio of successive inflation layers"),
        )
        .field(
            "layer_walls",
            Schema::enumeration("both", &["both", "lower"]).doc(
                "Inflation at both y walls (needs an even ny) or at the lower wall (ymin) only, e.g. a flat plate under a free stream",
            ),
        )
        .field(
            "layer_reference_velocity",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::VELOCITY)
                .doc("Free-stream or bulk speed for the target_y_plus estimate"),
        )
        .field(
            "layer_reference_length",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::LENGTH)
                .doc("Distance from the leading edge for the flat-plate Reynolds number of the target_y_plus estimate (use lx for the downstream end)"),
        )
        .field(
            "layer_u_tau",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::VELOCITY)
                .doc("Friction velocity for target_y_plus (0 = estimate from the flat-plate correlation)"),
        )
        .field(
            "file",
            Schema::string("").doc("Mesh file path (Gmsh .msh, Fluent .msh/.cas or OpenFOAM polyMesh directory; source = file)"),
        )
        .field(
            "scale",
            Schema::float(1.0)
                .at_least(1e-12)
                .doc("Scale factor applied to mesh file coordinates"),
        );

    let physics = Schema::object()
        .doc("Fluid properties")
        .field(
            "nu",
            Schema::float(0.01)
                .at_least(0.0)
                .dim(Dimension::KINEMATIC_VISCOSITY)
                .doc("Kinematic viscosity"),
        )
        .field(
            "rho",
            Schema::float(1.0)
                .at_least(1e-12)
                .dim(Dimension::DENSITY)
                .doc("Density (used to convert kinematic results to dynamic quantities)"),
        )
        .field(
            "cp",
            Schema::float(1005.0)
                .at_least(1e-12)
                .dim(Dimension::SPECIFIC_HEAT)
                .doc("Specific heat capacity (energy equation, expressions)"),
        )
        .field(
            "conductivity",
            Schema::float(0.0257)
                .at_least(0.0)
                .dim(Dimension::CONDUCTIVITY)
                .doc("Thermal conductivity (energy equation, expressions)"),
        )
        .field(
            "reference_temperature",
            Schema::float(293.15)
                .at_least(0.0)
                .dim(Dimension::TEMPERATURE)
                .doc("Reference temperature (Boussinesq buoyancy, expressions)"),
        )
        .field(
            "thermal_expansion",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::TEMPERATURE.powi(-1))
                .doc("Thermal expansion coefficient (Boussinesq buoyancy, expressions)"),
        )
        .field(
            "gravity",
            vec3([0.0; 3], "Gravity vector [gx, gy, gz] (empty list for zero)"),
        )
        .field(
            "body_force",
            vec3(
                [0.0; 3],
                "Uniform body acceleration [m/s^2] added to the momentum equation (steady algorithms; e.g. the streamwise driving force of a periodic channel). Empty list for zero",
            ),
        );

    let initial = Schema::object()
        .doc("Uniform initial conditions")
        .field("velocity", vec3([0.0; 3], "Initial velocity [ux, uy, uz]"))
        .field(
            "velocity_expression_x",
            Schema::string("").doc("Expression [m/s] of x, y, z for the initial x velocity (cell centres); overrides initial.velocity[0] (ADR 0009: a start consistent with an imposed velocity profile)"),
        )
        .field(
            "velocity_expression_y",
            Schema::string("").doc("Expression [m/s] for the initial y velocity (see velocity_expression_x)"),
        )
        .field(
            "velocity_expression_z",
            Schema::string("").doc("Expression [m/s] for the initial z velocity (see velocity_expression_x)"),
        )
        .field("pressure", Schema::float(0.0).doc("Initial kinematic pressure"))
        .field(
            "temperature",
            Schema::float(293.15)
                .at_least(0.0)
                .dim(Dimension::TEMPERATURE)
                .doc("Initial temperature (energy equation)"),
        )
        .field(
            "method",
            Schema::enumeration("uniform", &["uniform", "potential", "coarse"]).doc("How the initial velocity and pressure are built (steady simple/simplec): uniform (the values above), potential (potential-flow solve, div(grad psi) = 0, u = grad psi, Bernoulli pressure; ADR 0019), coarse (a pre-solve on a mesh coarsened by coarse_factor, interpolated to the fine mesh)"),
        )
        .field(
            "coarse_factor",
            Schema::int(2).range(2.0, 8.0).doc("method = coarse: the cell counts of the generator are divided by this factor for the pre-solve"),
        )
        .field(
            "coarse_iterations",
            Schema::int(200).range(1.0, 1e6).doc("method = coarse: SIMPLE iterations of the coarse pre-solve"),
        )
        .field("patches", crate::registers::initial_patches_schema());

    let energy = Schema::object()
        .doc("Energy (temperature) equation with Boussinesq buoyancy (steady algorithms only)")
        .field(
            "enabled",
            Schema::boolean(false).doc("Solve the temperature equation together with the flow"),
        )
        .field(
            "buoyancy",
            Schema::boolean(true).doc("Couple the flow through the Boussinesq body force -beta (T - T_ref) g"),
        )
        .field(
            "relax",
            Schema::float(0.9)
                .range(0.05, 1.0)
                .doc("Implicit under-relaxation factor of the temperature equation"),
        )
        .field(
            "viscous_heating",
            Schema::boolean(false).doc("fluid.density = ideal_gas only: viscous heating in the energy equation (viscous work div(tau.u) in the total enthalpy of the compressible mode, dissipation tau:grad u in the low_mach mode; ADR 0009)"),
        )
        .field(
            "heat_source",
            Schema::float(0.0).doc("fluid.density = ideal_gas only: uniform volumetric heat source [W/m3] in the energy equation (ADR 0009)"),
        );

    let rheology = Schema::object()
        .doc("Generalised-Newtonian viscosity (shear-rate and temperature dependent); steady and transient laminar runs. The viscosities below are dynamic (Pa s); the solver divides by physics.rho")
        .field(
            "model",
            Schema::enumeration(
                "newtonian",
                &["newtonian", "power_law", "carreau", "carreau_yasuda", "cross", "herschel_bulkley", "bingham"],
            )
            .doc("Viscosity law: newtonian (physics.nu; only changes with a temperature shift), power_law K g^(n-1), carreau, carreau_yasuda, cross, herschel_bulkley (K g^(n-1) + tau0 (1 - exp(-m g))/g) or bingham (herschel_bulkley with n = 1, K the plastic viscosity)"),
        )
        .field(
            "consistency_index",
            Schema::float(1.0).doc("K [Pa s^n] of the power-law, Herschel-Bulkley and Bingham laws (plastic viscosity [Pa s] for Bingham)"),
        )
        .field(
            "power_index",
            Schema::float(1.0).range(0.05, 5.0).doc("Power-law index n (n < 1 shear thinning, n > 1 shear thickening); ignored by bingham (n = 1)"),
        )
        .field(
            "zero_shear_viscosity",
            Schema::float(1.0).dim(Dimension::DYNAMIC_VISCOSITY).doc("mu_0 [Pa s] of the Carreau, Carreau-Yasuda and Cross laws"),
        )
        .field(
            "infinite_shear_viscosity",
            Schema::float(0.0).dim(Dimension::DYNAMIC_VISCOSITY).doc("mu_inf [Pa s] of the Carreau, Carreau-Yasuda and Cross laws"),
        )
        .field(
            "time_constant",
            Schema::float(1.0).dim(Dimension::TIME).doc("lambda [s] of the Carreau, Carreau-Yasuda and Cross laws"),
        )
        .field(
            "yasuda_exponent",
            Schema::float(2.0).doc("Exponent a of the Carreau-Yasuda law (2 gives Carreau)"),
        )
        .field(
            "cross_exponent",
            Schema::float(1.0).doc("Exponent m of the Cross law"),
        )
        .field(
            "yield_stress",
            Schema::float(0.0).dim(Dimension::PRESSURE).doc("tau_0 [Pa] of the Herschel-Bulkley and Bingham laws"),
        )
        .field(
            "regularisation_time",
            Schema::float(100.0).dim(Dimension::TIME).doc("Papanastasiou stress-growth time m [s] that regularises the yield stress: a larger value approaches a true yield stress and is stiffer"),
        )
        .field(
            "min_viscosity",
            Schema::float(0.0).dim(Dimension::DYNAMIC_VISCOSITY).doc("Lower clamp of the apparent viscosity [Pa s]"),
        )
        .field(
            "max_viscosity",
            Schema::float(1.0e6).dim(Dimension::DYNAMIC_VISCOSITY).doc("Upper clamp of the apparent viscosity [Pa s] (also bounds the viscosity of the first iterations, where the shear rate is still small)"),
        )
        .field(
            "relax",
            Schema::float(0.7).range(0.05, 1.0).doc("Under-relaxation of the face viscosity between iterations or steps (1 = none)"),
        )
        .field(
            "shift",
            Schema::enumeration("none", &["none", "arrhenius", "wlf", "table"])
                .doc("Temperature dependence by time-temperature superposition mu(g, T) = a_T mu_ref(a_T g); needs the energy equation. table reads a_T from tables.<shift_table>"),
        )
        .field(
            "shift_table",
            Schema::string("").doc("Name of a lookup table (tables.<name>) with the input column T [K] and the output column a_T (dimensionless) for shift = table"),
        )
        .field(
            "reference_temperature",
            Schema::float(293.15).dim(Dimension::TEMPERATURE).doc("Reference temperature [K] of the shift"),
        )
        .field(
            "activation_temperature",
            Schema::float(0.0).dim(Dimension::TEMPERATURE).doc("E_a/R [K] of the Arrhenius shift a_T = exp(E (1/T - 1/T_ref))"),
        )
        .field(
            "wlf_c1",
            Schema::float(17.44).doc("C1 of the WLF shift log10 a_T = -C1 (T - T_ref)/(C2 + T - T_ref)"),
        )
        .field(
            "wlf_c2",
            Schema::float(51.6).dim(Dimension::TEMPERATURE).doc("C2 [K] of the WLF shift"),
        );

    let boundary = Schema::object()
        .doc("Boundary conditions for one mesh patch")
        .field(
            "velocity",
            Schema::enumeration("fixed", &["fixed", "zero_gradient", "symmetry", "freestream", "moving_wall", "shear", "partial_slip", "marangoni"])
                .doc("Velocity condition (fixed with value 0 is a no-slip wall). moving_wall: impermeable wall moving with velocity_value (tangential) plus an optional rotation (wall_rotation_*); shear: specified tangential traction shear_stress; partial_slip: Navier slip (slip_length, or specularity with mean_free_path); marangoni: thermocapillary stress marangoni_coefficient * tangential temperature gradient (needs the energy equation). All four are walls for the turbulence model (ADR 0016)"),
        )
        .field(
            "velocity_value",
            vec3([0.0; 3], "Velocity vector for fixed / freestream / moving_wall / partial_slip conditions"),
        )
        .field(
            "wall_rotation_rate",
            Schema::float(0.0).doc("moving_wall / partial_slip: rotation rate [rad/s] about wall_rotation_axis through wall_rotation_origin; the wall velocity is velocity_value + Omega x (x - origin)"),
        )
        .field(
            "wall_rotation_axis",
            vec3([0.0, 0.0, 1.0], "Rotation axis direction (any non-zero length) of the wall rotation"),
        )
        .field(
            "wall_rotation_origin",
            vec3([0.0; 3], "Point on the rotation axis [m]"),
        )
        .field(
            "wall_velocity_projection",
            Schema::enumeration("none", &["none", "tangential"])
                .doc("moving_wall only: none (default) rejects a wall velocity that is not tangential to every face; tangential replaces the velocity of each face by its tangential part, so the faceted wall carries no flux (ADR 0072). Only for a wall invariant under its own motion (a surface of revolution rotating about its axis, a plane translating in itself), e.g. a stationary shroud in the rotating frame; wrong where the wall's normal motion is real"),
        )
        .field(
            "shear_stress",
            vec3([0.0; 3], "shear: tangential traction [Pa] exerted on the fluid by the boundary (its normal part is ignored); mu du_t/dn_out equals this value"),
        )
        .field(
            "slip_length",
            Schema::float(0.0).at_least(0.0).doc("partial_slip: Navier slip length [m]; 0 is no-slip"),
        )
        .field(
            "specularity",
            Schema::float(-1.0).doc("partial_slip: Maxwell specularity in [0, 1) used with mean_free_path instead of slip_length (slip length = mean_free_path (1 + s)/(1 - s)); negative = not used"),
        )
        .field(
            "mean_free_path",
            Schema::float(0.0).at_least(0.0).doc("partial_slip: mean free path [m] of the specularity form"),
        )
        .field(
            "marangoni_coefficient",
            Schema::float(0.0).doc("marangoni: d sigma/d T [N/(m K)] of the surface tension; the stress on the fluid is this times the tangential temperature gradient"),
        )
        .field(
            "pressure",
            Schema::enumeration("zero_gradient", &["zero_gradient", "fixed", "freestream"])
                .doc("Pressure condition (freestream = fixed on outflow faces only)"),
        )
        .field(
            "pressure_value",
            Schema::float(0.0).doc("Kinematic pressure for fixed / freestream conditions"),
        )
        .field(
            "velocity_expression_x",
            Schema::string("").doc("Expression [m/s] for the x velocity component of a fixed condition (overrides velocity_value; may use x, y, z, t)"),
        )
        .field(
            "velocity_expression_y",
            Schema::string("").doc("Expression [m/s] for the y velocity component (see velocity_expression_x)"),
        )
        .field(
            "velocity_expression_z",
            Schema::string("").doc("Expression [m/s] for the z velocity component (see velocity_expression_x)"),
        )
        .field(
            "velocity_udf",
            Schema::string("").doc("Name of a user function (udf.<name>, WebAssembly module with a velocity_profile export) that supplies the velocity of a fixed condition per face; an explicit velocity_expression_* component takes precedence"),
        )
        .field(
            "temperature_expression",
            Schema::string("").doc("Expression [K] for a fixed wall/inlet temperature (energy equation)"),
        )
        .field(
            "temperature",
            Schema::enumeration("zero_gradient", &["zero_gradient", "fixed", "heat_flux", "convection"])
                .doc("Temperature condition (energy equation): adiabatic, fixed value, prescribed heat flux or convection (heat-transfer coefficient to a free-stream temperature, optional thin wall layers and radiation to an ambient, ADR 0024)"),
        )
        .field(
            "temperature_value",
            Schema::float(293.15)
                .at_least(0.0)
                .dim(Dimension::TEMPERATURE)
                .doc("Temperature for a fixed condition"),
        )
        .field(
            "heat_flux_value",
            Schema::float(0.0)
                .dim(Dimension::HEAT_FLUX)
                .doc("Heat flux INTO the domain for a heat_flux condition (positive heats the fluid)"),
        )
        .field(
            "heat_flux_expression",
            Schema::string("").doc("Expression [W/m^2] for a prescribed wall heat flux into the domain (energy equation)"),
        )
        .field(
            "heat_transfer_coefficient",
            Schema::float(-1.0).doc("temperature = convection: heat-transfer coefficient h [W/m^2/K] to free_stream_temperature (0 = none); -1 = unset, an error (ADR 0024)"),
        )
        .field(
            "free_stream_temperature",
            Schema::float(-1.0).doc("temperature = convection: free-stream (fluid) temperature [K], required when heat_transfer_coefficient > 0; -1 = unset"),
        )
        .field(
            "wall_thickness",
            Schema::list(Schema::float(0.0)).doc("temperature = convection: thickness [m] of each thin wall layer between the surface and the external convection, steady resistance only (no thermal capacity); empty = no thin wall"),
        )
        .field(
            "wall_conductivity",
            Schema::list(Schema::float(0.0)).doc("temperature = convection: conductivity [W/m/K] of each thin wall layer (same length as wall_thickness)"),
        )
        .field(
            "wall_emissivity",
            Schema::float(0.0).doc("temperature = convection: emissivity in [0, 1] of the external surface for radiation to radiation_temperature (0 = none). Distinct from `emissivity`, the P1 radiation wall emissivity"),
        )
        .field(
            "radiation_temperature",
            Schema::float(-1.0).doc("temperature = convection: ambient temperature [K] the external surface radiates to, required when wall_emissivity > 0; -1 = unset"),
        )
        .field(
            "emissivity",
            Schema::float(1.0).doc("Wall emissivity in (0, 1] for the P1 radiation model (Marshak condition); patches with velocity = symmetry are reflecting"),
        )
        .field(
            "profile",
            Schema::string("").doc("Name of a table (tables.<name>) that supplies this patch's values from a profile file: its output columns ux, uy, uz (m/s, fixed velocity), T (K, fixed temperature) and heat_flux (W/m^2, heat_flux condition) replace the matching constant; its inputs must be among x, y, z, t; an explicit *_expression setting takes precedence"),
        )
        .field(
            "turbulence",
            Schema::enumeration("auto", &["auto", "wall", "inlet", "zero_gradient"]).doc(
                "Turbulence role of the patch (k-epsilon, k-omega SST): wall = wall functions / SST wall treatment, inlet = fixed k and epsilon (omega) from the turbulence_* settings, zero_gradient = outlet / symmetry. auto: a fixed zero velocity is a wall, any other fixed velocity an inlet, everything else zero_gradient",
            ),
        )
        .field(
            "turbulence_spec",
            Schema::enumeration(
                "intensity_length",
                &["intensity_length", "intensity_viscosity_ratio", "values"],
            )
            .doc("How an inlet specifies its turbulence: k = 1.5 (I U)^2 with epsilon = Cmu^0.75 k^1.5 / l (intensity_length) or epsilon = Cmu k^2 / (nu * nut/nu) (intensity_viscosity_ratio); or explicit k_value and epsilon_value"),
        )
        .field(
            "turbulence_intensity",
            Schema::float(0.05)
                .range(1e-6, 1.0)
                .doc("Turbulence intensity I = sqrt(2k/3) / |U| of an inlet"),
        )
        .field(
            "turbulence_length",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::LENGTH)
                .doc("Turbulence length scale l of an inlet (intensity_length); 0 = unset. For internal flow about 0.07 times the hydraulic diameter"),
        )
        .field(
            "turbulence_viscosity_ratio",
            Schema::float(10.0)
                .range(1e-3, 1e6)
                .doc("Turbulent-to-laminar viscosity ratio nut/nu of an inlet (intensity_viscosity_ratio)"),
        )
        .field(
            "k_value",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::VELOCITY * Dimension::VELOCITY)
                .doc("Turbulent kinetic energy of an inlet (turbulence_spec = values)"),
        )
        .field(
            "epsilon_value",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::VELOCITY * Dimension::VELOCITY * Dimension::ANGULAR_VELOCITY)
                .doc("Dissipation rate of an inlet (turbulence_spec = values)"),
        )
        .field(
            "omega_value",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::ANGULAR_VELOCITY)
                .doc("Specific dissipation rate of an inlet (k_omega_sst with turbulence_spec = values)"),
        )
        .field(
            "nutilde_value",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::KINEMATIC_VISCOSITY)
                .doc("Spalart-Allmaras working variable nuTilda of an inlet [m^2/s]; 0 = turbulence.nutilde_ratio times physics.nu"),
        )
        .field(
            "compressible_bc",
            Schema::enumeration(
                "auto",
                &[
                    "auto",
                    "slip",
                    "extrapolate",
                    "fixed",
                    "total_inlet",
                    "pressure_outlet",
                    "farfield",
                    "nonreflecting",
                ],
            )
            .doc(
                "Compressible solver condition: slip = wall / symmetry, extrapolate = supersonic outflow, fixed = free-stream or inflow state from velocity_value, pressure_value (static, Pa) and density_value (or temperature_value through the ideal gas law). Characteristic conditions (ADR 0006): total_inlet = subsonic inflow from total_pressure_value, temperature_value (total) and flow_direction; pressure_outlet = static pressure_value, switches to extrapolation when the outflow is supersonic; farfield = Riemann-invariant free stream from mach_value, flow_angle, pressure_value and temperature_value; nonreflecting = pressure outlet relaxed with reflection_value. auto: extrapolate on outlet patches, slip elsewhere",
            ),
        )
        .field(
            "flow_direction",
            vec3(
                [0.0; 3],
                "Flow direction of a total_inlet (any length > 0, normalised; [0, 0, 0] = against the patch normal)",
            ),
        )
        .field(
            "mach_value",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Free-stream Mach number of a farfield condition"),
        )
        .field(
            "flow_angle",
            Schema::float(0.0).doc("Free-stream flow angle of a farfield condition, degrees from +x towards +y"),
        )
        .field(
            "reflection_value",
            Schema::float(0.02).at_least(0.0).doc(
                "Relaxation weight sigma in [0, 1] of a nonreflecting outlet: 1 = fixed static pressure (fully reflecting), 0 = no pressure imposed (waves leave freely). Waves slower than the time scale dx/(sigma a) are reflected, so low values pass short pulses and hold the pressure only slowly",
            ),
        )
        .field(
            "inlet",
            Schema::enumeration("none", &["none", "stagnation"]).doc(
                "Variable-density solver (fluid.density = ideal_gas) inlet type: stagnation = total pressure total_pressure_value and total temperature temperature_value, flow direction from the interior (the velocity, pressure and temperature settings of the patch are replaced)",
            ),
        )
        .field(
            "total_pressure_value",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::PRESSURE)
                .doc("Total (stagnation) pressure of a stagnation inlet [Pa]"),
        )
        .field(
            "density_value",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::DENSITY)
                .doc("Density of a fixed compressible state; 0 = derive from pressure_value and temperature_value"),
        )
        .field(
            "bc_kind",
            Schema::enumeration(
                "none",
                &[
                    "none",
                    "mass_flow",
                    "outflow",
                    "pressure_outlet",
                    "opening",
                    "vent",
                    "total_pressure_inlet",
                    "radial_equilibrium_outlet",
                    "target_mass_flow_outlet",
                ],
            )
            .doc("Boundary-condition catalogue kind (ADR 0022, constant-density simple/simplec/piso/pimple only). none: the patch is read from velocity / pressure as before. mass_flow: prescribed mass flow with direction, profile shape, swirl or a recirculation source patch; outflow: outflow patch sharing the net inflow by flow_rate_weight; pressure_outlet: static pressure with backflow and averaging options; opening: pressure boundary with entrainment at the external total pressure; vent: pressure loss law with loss_coefficient; total_pressure_inlet: total pressure with direction and swirl; radial_equilibrium_outlet: swirl-consistent pressure profile; target_mass_flow_outlet: back pressure adjusted to a target mass flow. With a kind the velocity, pressure and velocity_value keys must keep their defaults"),
        )
        .field(
            "mass_flow_value",
            Schema::float(0.0).doc("mass_flow: mass flow rate [kg/s], positive INTO the domain (negative = a prescribed outflow)"),
        )
        .field(
            "mass_flow_shape_expression",
            Schema::string("").doc("mass_flow: profile shape as an expression of x, y, z with the dimension of a velocity (only the shape matters, it is scaled to the mass flow; tables can be called); empty = uniform"),
        )
        .field(
            "mass_flow_source_patch",
            Schema::string("").doc("mass_flow: take the mass flow from the outflow of this patch times mass_flow_fraction (outlet-to-inlet recirculation); not together with mass_flow_value"),
        )
        .field(
            "mass_flow_fraction",
            Schema::float(1.0).doc("mass_flow: fraction in (0, 1] of the source patch outflow that re-enters"),
        )
        .field(
            "swirl_rate",
            Schema::float(0.0).doc("mass_flow, total_pressure_inlet: swirl angular velocity [rad/s] about swirl_axis; the tangential velocity Omega x (x - swirl_origin) is added to the inflow"),
        )
        .field(
            "swirl_axis",
            vec3([0.0, 0.0, 1.0], "Axis direction of the swirl (mass_flow, total_pressure_inlet) or of the radial_equilibrium_outlet"),
        )
        .field(
            "swirl_origin",
            vec3([0.0; 3], "A point on the swirl / radial-equilibrium axis [m]"),
        )
        .field(
            "bc_pressure",
            Schema::float(0.0).dim(Dimension::PRESSURE).doc("Pressure [Pa] of the catalogue pressure kinds (static for pressure_outlet, vent and target_mass_flow_outlet (its start value), total for opening and total_pressure_inlet, the radial_reference value for radial_equilibrium_outlet); converted to the kinematic pressure of the solver with physics.rho"),
        )
        .field(
            "backflow_direction",
            Schema::enumeration("normal", &["normal", "zero_gradient"]).doc("pressure_outlet: velocity direction on faces where fluid enters: normal to the boundary or zero gradient"),
        )
        .field(
            "backflow_pressure",
            Schema::enumeration("static", &["static", "total"]).doc("pressure_outlet: pressure on faces where fluid enters: the set static value, or the value reduced by the dynamic head (total pressure specified)"),
        )
        .field(
            "prevent_reverse_flow",
            Schema::boolean(false).doc("pressure_outlet: close faces where fluid would enter (velocity zero); outflow: clip the outflow shape to non-negative normal velocity"),
        )
        .field(
            "pressure_average",
            Schema::enumeration("none", &["none", "area"]).doc("pressure_outlet: area: the specified pressure is the area-average of the face pressures (the profile shape follows the interior)"),
        )
        .field(
            "loss_coefficient",
            Schema::float(0.0).at_least(0.0).doc("vent: loss coefficient K of the pressure drop law p = p_ext + K rho u_n |u_n| / 2 (u_n outward)"),
        )
        .field(
            "flow_rate_weight",
            Schema::float(1.0).doc("outflow: weight of the patch in the split of the net inflow over all outflow patches (must be positive)"),
        )
        .field(
            "radial_reference",
            Schema::enumeration("hub", &["hub", "area_average"]).doc("radial_equilibrium_outlet: bc_pressure is the pressure of the smallest-radius face (hub) or the area-average pressure"),
        )
        .field(
            "radial_frame_rate",
            Schema::float(0.0).doc("radial_equilibrium_outlet: angular velocity [rad/s] about swirl_axis of the frame the velocity is solved in (the rotating_frame rate); the balance uses the absolute swirl w_theta + rate r (ADR 0078). Default 0: the solved velocity"),
        )
        .field(
            "target_mass_flow",
            Schema::float(0.0).doc("target_mass_flow_outlet: mass flow rate [kg/s] leaving the domain through the patch"),
        )
        .field(
            "pressure_update",
            Schema::enumeration("bernoulli", &["bernoulli", "secant", "guarded_secant"]).doc("target_mass_flow_outlet: pressure update from the mass-flow error: dynamic-head sensitivity, safeguarded secant slope, or the secant slope accepted only when the last mass-flow change is not spin-up dominated (|dm| <= 0.5 |e|, slope within 0.1 to 10 times the dynamic-head slope; ADR 0055 D5)"),
        )
        .field(
            "pressure_relax",
            Schema::float(0.5).range(0.01, 1.0).doc("target_mass_flow_outlet: relaxation of the pressure update"),
        );
    let boundary = crate::electric::boundary_fields(boundary);

    let solver = Schema::object()
        .doc("Numerical method and run control")
        .field(
            "algorithm",
            Schema::enumeration("simplec", &["simple", "simplec", "coupled", "piso", "pimple", "compressible"])
                .doc("Pressure-velocity coupling: simple/simplec are steady segregated, coupled is the steady block-coupled solver (u, v, w and p in one system, Krylov solver with an AMG pressure block; laminar constant-property flow; open boundaries are zero-gradient velocity with fixed pressure; uniform body force, Boussinesq buoyancy and the energy equation are available, and solid zones (solids) are excluded from the flow system; no turbulence, rheology, sources or non-orthogonal correctors), piso/pimple transient; compressible selects the density-based explicit Euler solver (see the compressible section)"),
        )
        .field(
            "convection",
            Schema::enumeration("linear_upwind", CONVECTION_SCHEMES).doc("Momentum convection scheme"),
        )
        .field(
            "alpha_u",
            Schema::float(0.9)
                .range(0.05, 1.0)
                .doc("Velocity under-relaxation factor"),
        )
        .field(
            "alpha_p",
            Schema::float(1.0)
                .range(0.05, 1.0)
                .doc("Pressure under-relaxation factor (SIMPLE: ~0.3, SIMPLEC: 1.0)"),
        )
        .field(
            "non_orthogonal_correctors",
            Schema::int(0)
                .range(0.0, 10.0)
                .doc("Extra pressure solves for non-orthogonal meshes"),
        )
        .field(
            "simplec_non_orthogonal_compensation",
            Schema::boolean(false).doc("SIMPLEC with non_orthogonal_correctors > 0: add the non-orthogonal secondary-gradient part to the SIMPLEC flux compensation (ADR 0050). Without it the corrected flux weights the deferred non-orthogonal pressure part with rAtU instead of rAU and, for alpha_u near 1 on strongly non-orthogonal meshes (about 35 degrees tested), an axial mode of the outer iteration grows until the run diverges. Default false keeps every earlier result bit-identical; no effect for SIMPLE or without correctors"),
        )
        .field(
            "max_iterations",
            Schema::int(2000).range(1.0, 1e7).doc("Iteration cap for steady runs"),
        )
        .field(
            "tolerance_u",
            Schema::float(1e-6)
                .range(1e-14, 1.0)
                .doc("Normalised momentum residual target"),
        )
        .field(
            "tolerance_p",
            Schema::float(1e-6)
                .range(1e-14, 1.0)
                .doc("Normalised pressure residual target"),
        )
        .field(
            "dt",
            Schema::float(0.01)
                .at_least(1e-12)
                .dim(Dimension::TIME)
                .doc("Time step (transient)"),
        )
        .field(
            "end_time",
            Schema::float(1.0)
                .at_least(0.0)
                .dim(Dimension::TIME)
                .doc("End time (transient)"),
        )
        .field(
            "divergence_courant_limit",
            Schema::float(0.0).at_least(0.0).doc("PISO/PIMPLE divergence guard (ADR 0067): the run stops with a located error (state diverged, exit 6) when a step's maximum Courant number exceeds this value; 0 = unset, the built-in 1e6 applies (use 1e300 to disable the Courant criterion). Non-finite velocity, pressure or flux always stops the run"),
        )
        .field(
            "time_scheme",
            Schema::enumeration("euler", &["euler", "bdf2", "bdf2_bounded"]).doc("Time integration scheme (transient): euler, bdf2, or bdf2_bounded (second order, falls back toward euler at temporal extrema; ADR 0016)"),
        )
        .field(
            "blend_factor",
            Schema::float(0.75)
                .range(0.0, 1.0)
                .doc("Blend factor of solver.convection = blended: (1 - f) upwind + f central (0 is upwind, 1 is central)"),
        )
        .field(
            "high_resolution_beta",
            Schema::float(0.2)
                .range(0.1, 0.5)
                .doc("Parameter beta_m of solver.convection = high_resolution (bounded blend of central and upwind on the normalised variable diagram; smaller is more central)"),
        )
        .field(
            "compressive_k",
            Schema::float(1.5)
                .range(0.0, 2.0)
                .doc("Slope parameter k of solver.convection = compressive (limiter max(0, min(k r, 1), min(r, k)): 0 first order, 1 minmod, 2 superbee)"),
        )
        .field(
            "component_diagonal",
            Schema::enumeration("averaged", &["averaged", "corrected"]).doc("Momentum-diagonal treatment of the segregated kernels (steady SIMPLE/SIMPLEC constant-density and variable-density, transient piso/pimple constant-density (ADR 0074) and variable-density): averaged uses the mean of the component diagonals in the H operator and the pressure equation (historical default); corrected adds (a_P - a_k) u_k to the H operator of component k where a symmetry plane or another component-specific condition makes the component diagonals differ (ADR 0010). MRF zones always use corrected"),
        )
        .field(
            "n_outer",
            Schema::int(1)
                .range(1.0, 50.0)
                .doc("PIMPLE outer iterations per time step"),
        )
        .field(
            "n_correctors",
            Schema::int(2)
                .range(1.0, 10.0)
                .doc("Pressure correctors per outer iteration"),
        )
        .field(
            "pseudo_mode",
            Schema::enumeration("off", &["off", "auto", "local", "physical"]).doc(
                "Pseudo-transient steady solve (SIMPLE/SIMPLEC only): off = plain under-relaxation, auto = one global step factor*L/U_max, local = per-cell step from a pseudo-Courant number, physical = fixed global pseudo time step",
            ),
        )
        .field(
            "pseudo_cfl",
            Schema::float(10.0)
                .at_least(1e-6)
                .doc("Pseudo-Courant number of the local mode"),
        )
        .field(
            "pseudo_dt",
            Schema::float(0.1)
                .at_least(1e-12)
                .dim(Dimension::TIME)
                .doc("Pseudo time step of the physical mode"),
        )
        .field(
            "pseudo_factor",
            Schema::float(1.0)
                .at_least(1e-6)
                .doc("Timescale factor multiplying the selected pseudo time step"),
        )
        .field(
            "stop_when",
            Schema::enumeration("any", &["any", "all"])
                .doc("Stop when any or all of the convergence conditions are met (the residual targets always stop a steady run)"),
        )
        .field(
            "min_iterations",
            Schema::int(0)
                .range(0.0, 1e9)
                .doc("Convergence conditions are only evaluated from this iteration / step on"),
        )
        .field(
            "max_courant",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Adaptive time-step Courant target (MAX criterion); 0 keeps dt fixed. With none of the further step-control keys set, solver.dt is the cap and the step never grows by more than 1.2 per step; with any of them set the controller of ADR 0050 is used (solver.dt is then the first step and, unless solver.max_dt is set, the cap)"),
        )
        .field(
            "max_courant_rms",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Adaptive step: target volume-weighted RMS Courant number sqrt(sum V Co^2 / sum V); 0 = off. Combined with max_courant the smaller step wins (ADR 0050, transient PISO/PIMPLE)"),
        )
        .field(
            "dt_safety",
            Schema::float(0.0)
                .range(0.0, 1.0)
                .doc("Adaptive step: the step aims at dt_safety x the Courant target; 0 = default 0.9"),
        )
        .field(
            "dt_growth",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Adaptive step: largest growth factor per step (at least 1); 0 = default 1.2"),
        )
        .field(
            "dt_shrink",
            Schema::float(0.0)
                .range(0.0, 1.0)
                .doc("Adaptive step: smallest factor per step (the step shrinks at most by this factor); 0 = default 0.5"),
        )
        .field(
            "min_dt",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Adaptive step: smallest step [s]; 0 = 1e-3 x solver.dt"),
        )
        .field(
            "max_dt",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Adaptive step: largest step [s]; 0 = solver.dt"),
        )
        .field(
            "error_tolerance",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Local-error step control by step doubling (implicit Euler, laminar constant density, no further physics): a step is accepted when max|u_2 - u_1| / max|u_2| per unit time is at most this tolerance, so the global relative error grows like tolerance x time (ADR 0050 D2); 0 = off"),
        )
        .field(
            "period",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Period-based steps for periodic forcing: dt = period / steps_per_period, times k x dt landed exactly; 0 = off. Not combined with Courant or error control"),
        )
        .field(
            "steps_per_period",
            Schema::int(0)
                .range(0.0, 1e9)
                .doc("Steps per period (needs period); 0 = unset"),
        )
        .field(
            "extrapolate_initial",
            Schema::boolean(false).doc("Start every transient step from the linear extrapolation of the two previous time levels of velocity and face flux (ADR 0050 D5); changes the PISO result at the order of the time error"),
        )
        .field(
            "h_operator",
            Schema::enumeration("default", &["default", "from_solution", "previous_iterate"]).doc(
                "H operator of the SIMPLE/SIMPLEC pressure equation: from the solved momentum predictor (classic SIMPLE) or from the previous iterate. default keeps the library default (always from_solution with k-epsilon)",
            ),
        )
        .field(
            "pseudo_control",
            Schema::enumeration("off", &["off", "ser", "ser2"]).doc(
                "Automatic pseudo-time-step controller (needs pseudo_mode other than off): ser2 = switched evolution relaxation, the step scale follows the inverse residual ratio in both directions (at most x2 up, x0.5 down per iteration), scale within [1/64, pseudo_scale_max]; ser = (kept for the record, ratchets to the cap for oscillating residuals) the step grows with the inverse residual ratio (at most x2 per iteration), is halved when the residual rises by more than x2 or is not finite, scale kept within [1/64, pseudo_scale_max] (ADR 0023)",
            ),
        )
        .field(
            "pseudo_scale_max",
            Schema::float(1000.0)
                .at_least(1.0)
                .doc("Largest scale the pseudo_control = ser controller may apply to the pseudo time step"),
        )
        .field(
            "residual_norm",
            Schema::enumeration("reference", &["reference", "backward", "scaled"]).doc(
                "Normalisation of the momentum residual of SIMPLE/SIMPLEC: reference = sum|b-Ax| over the variation of the iterate (default), backward = sum|b-Ax| over sum(|A||x|+|b|), which can reach round-off level for uniform exact solutions; scaled = common iterate-independent scales for the velocity components and for the pressure equation (needed where an exact component or the pressure is zero) (ADR 0023)",
            ),
        )
        .field(
            "p_ref_cell",
            Schema::int(0)
                .range(0.0, 1e9)
                .doc("Cell whose pressure is pinned when no patch fixes the pressure level"),
        )
        .field(
            "p_ref_value",
            Schema::float(0.0).doc("Pinned pressure value of p_ref_cell"),
        )
        .field(
            "bulk_velocity",
            Schema::float(0.0).doc(
                "Target volume-mean velocity along bulk_direction for periodic flows (steady algorithms): a body acceleration is adapted every iteration. 0 = off",
            ),
        )
        .field(
            "bulk_direction",
            vec3([1.0, 0.0, 0.0], "Flow direction of bulk_velocity (empty list = [1, 0, 0])"),
        )
        .field(
            "momentum_rel_tol",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Relative tolerance of the linear momentum solves; 0 = algorithm default"),
        )
        .field(
            "momentum_max_iter",
            Schema::int(0)
                .range(0.0, 1e7)
                .doc("Iteration cap of the linear momentum solves; 0 = algorithm default"),
        )
        .field(
            "pressure_abs_tol",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Absolute residual floor of the linear pressure solves (SIMPLE family, coupled); 0 = algorithm default. Lower it to drive the mass imbalance of the flux field to round-off"),
        )
        .field(
            "pressure_rel_tol",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Relative tolerance of the linear pressure solves; 0 = algorithm default"),
        )
        .field(
            "pressure_max_iter",
            Schema::int(0)
                .range(0.0, 1e7)
                .doc("Iteration cap of the linear pressure solves; 0 = algorithm default"),
        )
        .field(
            "ddt_correction",
            Schema::boolean(true).doc("Transient Rhie-Chow correction (piso / pimple)"),
        )
        .field(
            "viscous_stress",
            Schema::enumeration("laplacian", &["laplacian", "full"]).doc("Viscous term of the transient incompressible momentum equation (piso / pimple): laplacian = div(nu_eff grad u) only (default); full = adds the explicit transposed part div(nu_eff (grad u)^T), the full stress divergence needed for a variable (subgrid or eddy) viscosity (ADR 0032). Boundary faces other than periodic ones do not contribute to the transposed part"),
        )
        .field(
            "time_advance",
            Schema::enumeration("iterative", &["iterative", "fractional_step"]).doc("Transient incompressible time advance (solver.algorithm = piso only): iterative = the PISO corrector loop (default); fractional_step = non-iterative projection, one momentum predictor and one pressure Poisson solve per step (ADR 0037). Not combined with pimple, steady algorithms, energy, turbulence, rheology, body force, dynamic_mesh, solver.viscous_stress = full or time_scheme = bdf2_bounded (located errors)"),
        )
        .field(
            "fractional_form",
            Schema::enumeration("incremental", &["incremental", "chorin"]).doc("time_advance = fractional_step: incremental = predictor with the old pressure gradient and a pressure-increment Poisson problem (default); chorin = no pressure in the predictor and the full pressure from the projection (first-order steady state)"),
        )
        .field(
            "fractional_boundary",
            Schema::enumeration("kim_moin", &["kim_moin", "plain"]).doc("time_advance = fractional_step: kim_moin = intermediate velocity condition u_b + dt grad(q)_tangential with q the lagged pressure (increment) (Kim and Moin 1985); plain = the physical velocity"),
        )
        .field(
            "frozen_flow",
            Schema::boolean(false).doc("Prescribe the flow: the face flux comes from initial.velocity and initial.velocity_expression_x/y/z at the face centres and only the energy equation is solved (required by solids; conjugate heat transfer, ADR 0024)"),
        );

    let report = Schema::object()
        .doc("A scalar result computed from the current solution")
        .field(
            "type",
            Schema::enumeration(
                "volume_average",
                &[
                    "force",
                    "moment",
                    "force_coefficient",
                    "volume_average",
                    "volume_integral",
                    "area_average",
                    "area_integral",
                    "mass_flow",
                    "heat_flow",
                    "probe",
                    "minimum",
                    "maximum",
                    "expression",
                    "y_plus",
                    "scalar_flux",
                    "scalar_average",
                    "volume_std",
                    "area_std",
                    "mass_weighted_average",
                    "area_min",
                    "area_max",
                    "vector_flux",
                ],
            )
            .doc("What to compute (y_plus: wall y+ minimum / maximum / average over a wall patch, k-epsilon runs)"),
        )
        .field(
            "scalar",
            Schema::string("").doc("Name of a passive scalar or species (scalar_flux: net outflow through the patch, or the conservation imbalance of all boundaries when no patch is given; scalar_average: volume average)"),
        )
        .field(
            "statistic",
            Schema::enumeration("average", &["average", "min", "max"])
                .doc("Statistic over the wall faces of the patch (type = y_plus)"),
        )
        .field(
            "expression",
            Schema::string("").doc("Expression of other reports, constants and named expressions (type = expression)"),
        )
        .field(
            "patch",
            Schema::string("").doc("Boundary patch (force, moment, area_*, mass_flow, heat_flow, y_plus)"),
        )
        .field(
            "field",
            Schema::enumeration("p", FIELD_NAMES).doc("Field for averages, integrals, probes, minima and maxima"),
        )
        .field(
            "component",
            Schema::enumeration("total", &["total", "pressure", "viscous"]).doc("Force part to report"),
        )
        .field(
            "direction",
            vec3(
                [1.0, 0.0, 0.0],
                "Direction of force/coefficient or axis of the moment [x, y, z]",
            ),
        )
        .field("origin", vec3([0.0; 3], "Moment reference point"))
        .field("point", vec3([0.0; 3], "Probe location"))
        .field(
            "a_ref",
            Schema::float(1.0)
                .at_least(1e-300)
                .doc("Reference area for coefficients"),
        )
        .field(
            "u_ref",
            Schema::float(1.0)
                .at_least(1e-300)
                .doc("Reference velocity for coefficients"),
        )
        .field(
            "l_ref",
            Schema::float(1.0)
                .at_least(1e-300)
                .doc("Reference length for moment coefficients"),
        )
        .field(
            "conductivity",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Thermal conductivity for heat_flow (constant-density energy equation); 0 (unset) uses physics.conductivity, a positive value overrides it (ignored when properties.conductivity is temperature-dependent: the law is used at the wall temperature). The variable-density solver (fluid.density = ideal_gas or real_gas) takes its conductivity from fluid.* and does not read this key: a positive value there is a validation error (ADR 0021)"),
        );

    let convergence = Schema::object()
        .doc("A convergence or interrupt condition on a monitored quantity (report or residual)")
        .field(
            "monitor",
            Schema::string("").doc("Report name, or res_u / res_p / continuity / courant / dt"),
        )
        .field(
            "type",
            Schema::enumeration("window_range", &["window_range", "change", "below", "above"]).doc(
                "window_range: max-min over the last `window` samples <= tolerance; change: |last - previous| <= tolerance; below: last value <= tolerance; above: last value >= tolerance (interrupt, e.g. divergence guard)",
            ),
        )
        .field(
            "tolerance",
            Schema::float(1e-5).doc("Tolerance or threshold (same unit as the monitored quantity)"),
        )
        .field(
            "window",
            Schema::int(10)
                .range(2.0, 1e6)
                .doc("Number of trailing samples for window_range"),
        )
        .field(
            "relative",
            Schema::boolean(false).doc("Make the range / change relative to the magnitude of the monitored value"),
        );

    let output = Schema::object()
        .doc("Result files")
        .field(
            "directory",
            Schema::string("results").doc("Output directory (relative to the case)"),
        )
        .field(
            "write_interval",
            Schema::int(0)
                .range(0.0, 1e9)
                .doc("Write a numbered VTK snapshot (snapshots/snapshot_<step>.vtu in the output directory) every N iterations/steps during rcfd run and run.headless with an output directory (0 = only the final solution.vtu)"),
        )
        .field(
            "write_times",
            Schema::list(Schema::float(0.0)).doc("Times [s] on which the transient step lands exactly (relative error 1e-12) and a numbered snapshot is written (needs the step control of ADR 0050; run path with an output directory); empty = none"),
        )
        .field(
            "write_time_interval",
            Schema::float(0.0)
                .at_least(0.0)
                .doc("Land on every multiple of this interval [s] (as write_times); 0 = off"),
        )
        .field(
            "fields",
            Schema::list(Schema::string("")).doc("Result variables written to the VTK files (empty = all): U, p, speed, T, k, epsilon, omega, nut, wall_distance, y_plus, rho, mach, statistics (all run-time statistics) or single statistics fields such as ux_mean"),
        )
        .field(
            "retain_snapshots",
            Schema::int(0)
                .range(0.0, 1e9)
                .doc("Keep only the newest N numbered snapshots (0 = keep all)"),
        )
        .field(
            "retain_checkpoints",
            Schema::int(1)
                .range(1.0, 1e6)
                .doc("Periodic restart files kept (backup retention): 1 overwrites one file; N > 1 writes <checkpoint>.<step> files and keeps the newest N, the end-of-run checkpoint always goes to <checkpoint>"),
        );

    let statistics = Schema::object()
        .doc("Run-time statistics of transient incompressible runs: time-weighted mean, rms, std, min and max fields")
        .field(
            "enabled",
            Schema::boolean(false).doc("Accumulate statistics every time step (piso / pimple)"),
        )
        .field(
            "start_time",
            Schema::float(0.0).dim(Dimension::TIME).doc("Time [s] from which steps contribute (the part of a step after this time counts)"),
        )
        .field(
            "fields",
            Schema::list(Schema::string("")).doc("Cell fields to accumulate: ux, uy, uz, speed, p (empty = ux, uy, uz, p). Results are the fields <name>_mean, _rms, _std, _min, _max in solution.vtu, probes and reports"),
        );

    let coef_obj = |table: Vec<rcfd_turb::CoefficientInfo>, doc: &str, zero_default: bool| {
        let mut o = Schema::object().doc(doc);
        for c in table {
            let d = if zero_default { 0.0 } else { c.default };
            let lo = if zero_default { 0.0 } else { c.min };
            o = o.field(c.name, Schema::float(d).range(lo, c.max).doc(c.doc));
        }
        o
    };
    let coefficients = Schema::object()
        .doc("Model coefficients, visible for every model; only the group of the active model is used (a non-default value in another group is a validation error)")
        .field(
            "k_epsilon",
            coef_obj(
                rcfd_turb::ke_coefficient_table(),
                "k-epsilon coefficients; 0 keeps the value of the selected variant",
                true,
            ),
        )
        .field(
            "k_omega_sst",
            coef_obj(
                rcfd_turb::sst_coefficient_table(),
                "k-omega SST coefficients (Menter 2003, NASA TMR values). Shown for reference: this version rejects a non-default value",
                false,
            ),
        )
        .field(
            "sa",
            coef_obj(
                rcfd_turb::sa_infos(rcfd_turb::SaCoefficients::default()),
                "Spalart-Allmaras coefficients (TMR values); cw1 follows from cb1, cb2, sigma and kappa",
                false,
            ),
        )
        .field(
            "smagorinsky",
            coef_obj(
                rcfd_turb::smagorinsky_coefficient_table(),
                "Smagorinsky subgrid-scale model coefficients (turbulence.model = smagorinsky, ADR 0026)",
                false,
            ),
        )
        .field(
            "wale",
            coef_obj(
                rcfd_turb::wale_coefficient_table(),
                "WALE subgrid-scale model coefficient (turbulence.model = wale, ADR 0026)",
                false,
            ),
        )
        .field(
            "mixing_length",
            coef_obj(
                rcfd_turb::mixing_length_coefficient_table(),
                "Prandtl mixing-length model coefficients (turbulence.model = mixing_length, ADR 0026)",
                false,
            ),
        )
        .field(
            "rsm",
            coef_obj(
                rcfd_turb::rsm_coefficient_table(),
                "Reynolds stress model coefficients (turbulence.model = rsm, ADR 0032): SSG (c1, c1s, c2, c3, c3s, c4, c5, ce2_ssg) and LRR-IP (c1_lrr, c2_lrr, ce2_lrr) pressure-strain constants, epsilon-equation constants ce1, c_eps, stress diffusion c_s, c_mu of the eddy viscosity returned to the momentum solver",
                false,
            ),
        );
    let sa_group = Schema::object()
        .doc("Spalart-Allmaras options")
        .field(
            "wall_mode",
            Schema::enumeration("auto", &["auto", "resolved"]).doc("auto: Spalding law of the wall for the wall shear and, for wall-cell y+ > 5, a wall-cell nuTilda matching the law, so that y+ from about 1 into the log layer works; resolved: nuTilda = 0 on the wall, no wall function (needs y+ of order 1)"),
        )
        .field(
            "limiter",
            Schema::enumeration("allmaras", &["allmaras", "min_0p3", "positive"]).doc("Limiter of S~: Allmaras, Johnson and Spalart (2012), S~ >= 0.3 Omega, or S~ > 0 only (NASA TMR options c, b, a)"),
        )
        .field(
            "negative_nutilde",
            Schema::boolean(true).doc("SA-neg treatment of a negative nuTilda (Allmaras, Johnson and Spalart 2012); off: nuTilda is clipped at zero"),
        )
        .field(
            "des",
            Schema::enumeration("off", &["off", "des", "ddes"]).doc("Hybrid RANS-LES length scale (ADR 0032): off = plain SA (steady algorithms); des = detached-eddy simulation d~ = min(d, c_des Delta) (Spalart et al. 1997); ddes = delayed DES d~ = d - f_d max(0, d - c_des Delta) with f_d = 1 - tanh((8 r_d)^3) (Spalart et al. 2006). des and ddes run with the transient algorithms (piso, pimple) only, with a time derivative on nuTilda"),
        )
        .field(
            "des_delta",
            Schema::enumeration("max_extent", &["max_extent", "cbrt_volume"]).doc("Grid scale Delta of des / ddes: max_extent = largest distance between two face centres of the cell (empty directions excluded; max(dx, dy, dz) on boxes), cbrt_volume = cell volume^(1/3)"),
        )
        .field(
            "c_des",
            Schema::float(0.65).range(0.1, 2.0).doc("C_DES of the des / ddes length scale (0.65; the value is taken from secondary literature, see docs/reference-data.md)"),
        );

    let rsm_group = Schema::object().doc("Reynolds stress model options (turbulence.model = rsm)").field(
        "pressure_strain",
        Schema::enumeration("ssg", &["ssg", "lrr_ip"]).doc("Pressure-strain model: ssg (Speziale, Sarkar and Gatski 1991, quadratic) or lrr_ip (Launder, Reece and Rodi 1975, isotropisation of production, linear). Its epsilon-equation constant Ce2 differs (1.83 / 1.92)"),
    );

    let turbulence = Schema::object()
        .doc("Turbulence modelling (RANS models: steady SIMPLE / SIMPLEC runs; smagorinsky and wale: transient runs; mixing_length: both)")
        .field(
            "model",
            Schema::enumeration("laminar", &["laminar", "k_epsilon", "k_omega_sst", "spalart_allmaras", "smagorinsky", "wale", "mixing_length", "rsm", "plugin"]).doc("rsm is the Reynolds stress transport model (SSG or LRR-IP pressure-strain, generalised gradient diffusion; ADR 0032): transient algorithms (piso, pimple) only, no wall treatment, so no wall patches. smagorinsky and wale are algebraic subgrid-scale models for large-eddy simulation (transient algorithms piso/pimple only; resolved walls, nu_sgs = 0 on wall faces, filter width = cell volume^(1/3); ADR 0026), mixing_length is the Prandtl algebraic closure (steady or transient, needs a wall and turbulence.length_scale; ADR 0026). Otherwise: laminar, the k-epsilon eddy-viscosity family with wall functions, k-omega SST (Menter 2003) with an automatic wall treatment that works from y+ about 1 (wall-resolved) to the log layer, Spalart-Allmaras (SA-noft2 with the negative-nuTilda treatment, wall-resolved or with the automatic wall treatment), or a registered plugin model (turbulence.plugin). The model can be changed in a live session with settings.set (the flow field is kept, see docs/case-schema.md)"),
        )
        .field(
            "variant",
            Schema::enumeration("standard", &["standard", "rng", "realizable"]).doc(
                "k-epsilon variant: standard (Launder and Spalding 1974), rng (Yakhot et al. 1992) or realizable (Shih et al. 1995)",
            ),
        )
        .field(
            "wall_function",
            Schema::enumeration("standard", &["standard", "scalable", "enhanced"]).doc(
                "k-epsilon wall treatment: standard equilibrium log-law wall functions (Launder and Spalding 1974), scalable (y* limited below the viscous/log intersection, Grotjans and Menter 1998) or enhanced (two-layer near-wall length scales blended with the transported values for Re_y < 200 and Kader's blended law, wall-resolved meshes). k_epsilon only",
            ),
        )
        .field(
            "roughness_height",
            Schema::float(0.0).range(0.0, 1.0e3).doc(
                "Sand-grain roughness height k_s of the walls in metres (0: smooth). Cebeci and Bradshaw shift of the log law; k_epsilon with the standard or scalable wall function and k_omega_sst with wall_omega automatic",
            ),
        )
        .field(
            "roughness_constant",
            Schema::float(0.0).range(0.0, 10.0).doc(
                "Roughness constant C_s of the fully rough shift Delta B = ln(1 + C_s k_s+)/kappa (0: the Nikuradse-consistent value derived from kappa and e_wall, about 0.3)",
            ),
        )
        .field(
            "wall_omega",
            Schema::enumeration("automatic", &["automatic", "low_re"]).doc(
                "k_omega_sst wall treatment: automatic (Spalding law blend, y+ about 1 to the log layer) or low_re (k = 0 and the Menter omega on the wall, nu_t = 0 at the wall; needs y+ of order 1)",
            ),
        )
        .field(
            "sst_variant",
            Schema::enumeration("sst", &["sst", "bsl"]).doc(
                "k_omega_sst variant: sst (Menter 2003) or bsl (baseline, no shear-stress limiter; Menter 1994)",
            ),
        )
        .field(
            "wall_treatment",
            Schema::enumeration("smooth", &["smooth", "hysteresis", "switch"]).doc(
                "Wall-cell treatment around the viscous/log intersection y* = 11.5: smooth (blended epsilon and production, no chatter, default), hysteresis (sharp switch with a 10 percent dead band) or switch (sharp Launder-Spalding switch; the eddy viscosity of cells near y* = 11.5 may oscillate and res_nut then never converges)",
            ),
        )
        .field(
            "realizable_a0",
            Schema::float(0.0).doc(
                "Realizable k-epsilon constant A0 in Cmu = 1/(A0 + A_s U* k/eps), allowed 3 to 5; 0 keeps the default 4.04 (a commonly cited commercial-code variant). The original report (Shih et al., NASA TM-106721) prints 4.0. realizable variant only (ADR 0017)",
            ),
        )
        .field(
            "kappa",
            Schema::float(0.41).range(0.2, 0.6).doc("von Karman constant of the log law"),
        )
        .field(
            "e_wall",
            Schema::float(9.8).range(1.0, 100.0).doc("Wall roughness parameter E of the log law (smooth wall 9.8)"),
        )
        .field(
            "convection",
            Schema::enumeration("upwind", CONVECTION_SCHEMES).doc("Convection scheme of k and epsilon"),
        )
        .field(
            "relax",
            Schema::float(0.7)
                .range(0.05, 1.0)
                .doc("Implicit under-relaxation of the k and epsilon equations"),
        )
        .field(
            "relax_nut",
            Schema::float(0.5)
                .range(0.05, 1.0)
                .doc("Under-relaxation of the eddy viscosity"),
        )
        .field(
            "k_initial",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::VELOCITY * Dimension::VELOCITY)
                .doc("Initial k; 0 = derive from initial_intensity and the reference speed (largest initial / boundary velocity)"),
        )
        .field(
            "epsilon_initial",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::VELOCITY * Dimension::VELOCITY * Dimension::ANGULAR_VELOCITY)
                .doc("Initial epsilon; 0 = derive from initial_viscosity_ratio"),
        )
        .field(
            "omega_initial",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::ANGULAR_VELOCITY)
                .doc("Initial omega (k_omega_sst); 0 = derive from initial_viscosity_ratio. Set together with k_initial"),
        )
        .field(
            "initial_intensity",
            Schema::float(0.05)
                .range(1e-6, 1.0)
                .doc("Turbulence intensity used to derive the initial k"),
        )
        .field(
            "initial_viscosity_ratio",
            Schema::float(10.0)
                .range(1e-3, 1e6)
                .doc("Viscosity ratio nut/nu used to derive the initial epsilon"),
        )
        .field(
            "plugin",
            Schema::string("").doc("Key of the registered turbulence model used when model = plugin (see rcfd_turb::register_model); an unregistered key is a validation error"),
        )
        .field(
            "wall_damping",
            Schema::enumeration("none", &["none", "van_driest"]).doc("smagorinsky and mixing_length: van Driest damping 1 - exp(-y+/A+) of the length scale (A+ from the model coefficient a_plus), with y+ from the wall shear velocity of the nearest wall face; needs a wall patch. WALE has no damping (located error)"),
        )
        .field(
            "length_scale",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::LENGTH)
                .doc("mixing_length: length scale R [m] (half height of a channel, radius of a pipe): the linear profile is capped at l_max_ratio R and the Nikuradse profile is a function of d/R. Required > 0 for mixing_length"),
        )
        .field(
            "mixing_profile",
            Schema::enumeration("linear", &["linear", "nikuradse"]).doc("mixing_length profile: linear (l = min(kappa d, l_max_ratio R)) or nikuradse (Nikuradse's pipe formula l/R = 0.14 - 0.08 (1 - d/R)^2 - 0.06 (1 - d/R)^4)"),
        )
        .field(
            "nutilde_ratio",
            Schema::float(3.0)
                .range(0.0, 1e4)
                .doc("Spalart-Allmaras: nuTilda/nu of the initial field and of inlets without nutilde_value (TMR: 3 to 5 for far-field values)"),
        )
        .field("sa", sa_group)
        .field("rsm", rsm_group)
        .field("coefficients", coefficients)
        .field(
            "plugin_coefficients",
            Schema::named(
                Schema::object()
                    .field("value", Schema::float(0.0).doc("Value of the plugin coefficient with this name"))
                    .doc("Coefficient of the plugin model"),
            )
            .doc("Coefficients of the plugin model by name; the plugin lists the names it accepts, others are rejected with a located diagnostic"),
        );

    let state = |name: &str, rho: f64, p: f64| {
        Schema::object()
            .doc(&format!("{name} state of the initial condition"))
            .field(
                "density",
                Schema::float(rho).at_least(0.0).dim(Dimension::DENSITY).doc("Density"),
            )
            .field(
                "velocity",
                vec3([0.0; 3], "Velocity [ux, uy, uz] (empty list for zero)"),
            )
            .field(
                "pressure",
                Schema::float(p)
                    .at_least(0.0)
                    .dim(Dimension::PRESSURE)
                    .doc("Static pressure"),
            )
    };
    let compressible = Schema::object()
        .doc("Density-based explicit compressible Euler solver (solver.algorithm = compressible)")
        .field(
            "flux",
            Schema::enumeration("hllc", COMPRESSIBLE_FLUXES).doc("Numerical flux: Rusanov, Roe (Harten entropy fix), HLLC or AUSM+"),
        )
        .field(
            "reconstruction",
            Schema::enumeration("tvd_van_leer", RECONSTRUCTIONS)
                .doc("MUSCL reconstruction of the primitive variables with this limiter, or first_order"),
        )
        .field(
            "rk_order",
            Schema::int(3)
                .range(1.0, 3.0)
                .doc("Runge-Kutta order: 1 = forward Euler (first-order reconstruction only), 2 = SSP-RK2, 3 = SSP-RK3"),
        )
        .field(
            "cfl",
            Schema::float(0.4).range(0.01, 1.0).doc("CFL number of the explicit time step"),
        )
        .field(
            "gamma",
            Schema::float(1.4).range(1.001, 5.0).doc("Ratio of specific heats of the perfect gas"),
        )
        .field(
            "gas_constant",
            Schema::float(287.058)
                .at_least(1e-12)
                .dim(Dimension::SPECIFIC_HEAT)
                .doc("Specific gas constant (temperature T = p / (rho R))"),
        )
        .field(
            "mode",
            Schema::enumeration("transient", &["transient", "steady"]).doc(
                "transient: run to solver.end_time; steady: march with CFL-limited steps until the largest relative density change per step drops below steady_tolerance (or solver.max_iterations)",
            ),
        )
        .field(
            "steady_tolerance",
            Schema::float(1e-8)
                .range(1e-14, 1.0)
                .doc("Steady-mode stop criterion: largest relative density change per step"),
        )
        .field(
            "initial",
            Schema::object()
                .doc("Initial condition: a uniform state, or a Riemann problem (Sod-style) with different states left and right of a plane")
                .field(
                    "mode",
                    Schema::enumeration("uniform", &["uniform", "riemann"])
                        .doc("uniform: the left state everywhere; riemann: left state where the coordinate is below split_position, right state elsewhere"),
                )
                .field(
                    "split_axis",
                    Schema::enumeration("x", &["x", "y", "z"]).doc("Coordinate that splits the left and right state"),
                )
                .field(
                    "split_position",
                    Schema::float(0.5)
                        .dim(Dimension::LENGTH)
                        .doc("Position of the diaphragm along split_axis"),
                )
                .field("left", state("Left (or uniform)", 1.0, 1.0))
                .field("right", state("Right", 0.125, 0.1)),
        );

    let fluid = Schema::object()
        .doc("Fluid model. density = constant keeps the incompressible solvers; density = ideal_gas selects the steady pressure-based variable-density SIMPLE/SIMPLEC solver (ADR 0005): ideal-gas density rho = p/(R T), total-enthalpy energy equation, laminar, solver.algorithm simple or simplec; density = real_gas uses the same solver with a cubic equation of state (ADR 0007: Peng-Robinson or Redlich-Kwong density, enthalpy departure, real sound speed, isentropic stagnation inlet), compressible mode only")
        .field(
            "density",
            Schema::enumeration("constant", &["constant", "ideal_gas", "real_gas"])
                .doc("constant: physics.rho; ideal_gas: rho = p / (R T); real_gas: rho(p, T) from the cubic equation of state fluid.eos (needs fluid.species or the critical constants)"),
        )
        .field(
            "eos",
            Schema::enumeration("peng_robinson", &["peng_robinson", "redlich_kwong"])
                .doc("real_gas only: cubic equation of state (Peng-Robinson 1976, Redlich-Kwong 1949)"),
        )
        .field(
            "species",
            Schema::string("").doc("real_gas only: built-in fluid whose critical data are used (N2, O2, AR, CO2, CH4, H2, H2O; case-insensitive); empty = give critical_temperature, critical_pressure and molar_mass"),
        )
        .field(
            "critical_temperature",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::TEMPERATURE)
                .doc("real_gas only: critical temperature [K]; 0 = from fluid.species"),
        )
        .field(
            "critical_pressure",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::PRESSURE)
                .doc("real_gas only: critical pressure [Pa]; 0 = from fluid.species"),
        )
        .field(
            "acentric_factor",
            Schema::float(0.0).doc("real_gas only (Peng-Robinson): acentric factor; 0 = from fluid.species (0 is the value of a simple fluid when no species is given)"),
        )
        .field(
            "molar_mass",
            Schema::float(0.0).at_least(0.0).doc("real_gas only: molar mass [kg/mol]; 0 = from fluid.species. The gas constant is R_u / molar_mass"),
        )
        .field(
            "specific_heat",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::SPECIFIC_HEAT)
                .doc("real_gas only: ideal-gas (zero-pressure) specific heat cp0 [J/kg/K], constant; 0 = gamma R/(gamma - 1) from fluid.gamma"),
        )
        .field(
            "mode",
            Schema::enumeration("compressible", &["compressible", "low_mach"]).doc(
                "ideal_gas only. compressible: absolute pressure in p, full equation of state (subsonic to transonic nozzles, stagnation inlets); low_mach: p is the dynamic pressure and the density follows the thermodynamic pressure, which is uniform in space and fixed by the mass of a closed domain (large temperature differences, natural convection)",
            ),
        )
        .field(
            "gas_constant",
            Schema::float(287.058)
                .at_least(1e-12)
                .dim(Dimension::SPECIFIC_HEAT)
                .doc("ideal_gas only: specific gas constant R [J/kg/K]"),
        )
        .field(
            "gamma",
            Schema::float(1.4).range(1.001, 5.0).doc("ideal_gas only: ratio of specific heats (cp = gamma R / (gamma - 1))"),
        )
        .field(
            "viscosity",
            Schema::float(1.8e-5)
                .at_least(1e-12)
                .dim(Dimension::DYNAMIC_VISCOSITY)
                .doc("ideal_gas only: dynamic viscosity [Pa s]; the reference value at sutherland_t_ref for the Sutherland law"),
        )
        .field(
            "viscosity_model",
            Schema::enumeration("constant", &["constant", "sutherland"]).doc("ideal_gas only: constant, or Sutherland's law viscosity mu_ref (T/T_ref)^1.5 (T_ref + S)/(T + S)"),
        )
        .field(
            "sutherland_t_ref",
            Schema::float(273.0).at_least(1e-12).dim(Dimension::TEMPERATURE).doc("ideal_gas only: reference temperature of the Sutherland law [K]"),
        )
        .field(
            "sutherland_s",
            Schema::float(110.5).at_least(0.0).dim(Dimension::TEMPERATURE).doc("ideal_gas only: Sutherland constant [K]"),
        )
        .field(
            "prandtl",
            Schema::float(0.71).at_least(1e-6).doc("ideal_gas only: Prandtl number; the conductivity is mu cp / Pr unless conductivity is set"),
        )
        .field(
            "conductivity",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::CONDUCTIVITY)
                .doc("ideal_gas only: constant thermal conductivity [W/m/K]; 0 = mu cp / prandtl"),
        )
        .field(
            "reference_pressure",
            Schema::float(101325.0)
                .at_least(1e-12)
                .dim(Dimension::PRESSURE)
                .doc("ideal_gas only: thermodynamic pressure at the start (low_mach; fixes the mass of a closed domain); in compressible mode the initial pressure is initial.pressure when positive, else this value"),
        )
        .field(
            "density_scheme",
            Schema::enumeration("linear_upwind", &["upwind", "central", "linear_upwind"]).doc("ideal_gas only: face interpolation of the density in the mass flux"),
        )
        .field(
            "relax_density",
            Schema::float(1.0).range(0.05, 1.0).doc("ideal_gas only: density under-relaxation"),
        )
        .field(
            "hydrostatic_init",
            Schema::boolean(false).doc("ideal_gas only: start from the hydrostatic pressure and density of physics.gravity (closed columns)"),
        )
        .field(
            "inner_iterations",
            Schema::int(30)
                .range(1.0, 1000.0)
                .doc("ideal_gas, solver.algorithm = pimple only: maximum SIMPLE-inner iterations per time step (ADR 0009)"),
        )
        .field(
            "inner_tolerance",
            Schema::float(1e-8)
                .range(1e-14, 1.0)
                .doc("ideal_gas, solver.algorithm = pimple only: inner stopping tolerance, the largest of |dP|/max P, |dT|/max T and |du|/max(sound speed) between two inner iterations"),
        );

    let band = |what: &str| {
        Schema::object()
            .doc(&format!("Solution limits of {what}: after each step cells outside [min, max] are clipped to the band (a robustness measure; it breaks conservation locally, the clipped count is in the run status). Off by default"))
            .field(
                "enabled",
                Schema::boolean(false).doc("Clip after every iteration or time step"),
            )
            .field("min", Schema::float(0.0).doc("Lower bound"))
            .field("max", Schema::float(1.0e30).doc("Upper bound"))
    };
    let limits = Schema::object()
        .doc("Solution limits (ADR 0016)")
        .field("temperature", band("the temperature (needs energy.enabled)"))
        .field("scalar", band("every transported scalar and species"));

    Schema::object()
        .doc("rcfd case definition")
        .field("title", Schema::string("untitled").doc("Case title"))
        .field("mesh", mesh)
        .field("physics", physics)
        .field("energy", energy)
        .field("limits", limits)
        .field("rheology", rheology)
        .field("initial", initial)
        .field("registers", crate::registers::schema())
        .field("chemistry", crate::chemistry::schema())
        .field("zone_sources", crate::zone_sources::schema())
        .field("regions", crate::regions::schema())
        .field("electric", crate::electric::schema())
        .field("phase_change", crate::phase_change::schema())
        .field("multiphase", crate::vof::schema())
        .field("solids", crate::cht::solids_schema())
        .field("thermal_interfaces", crate::cht::interfaces_schema())
        .field("particles", crate::particles::schema())
        .field("mixing_plane", crate::mixing_run::schema())
        .field("dynamic_mesh", crate::dynmesh::schema())
        .field(
            "boundaries",
            Schema::named(boundary).doc("Boundary conditions by patch name"),
        )
        .field("solver", solver)
        .field("turbulence", turbulence)
        .field("compressible", compressible)
        .field("fluid", fluid)
        .field(
            "expressions",
            Schema::named(
                Schema::object().doc("A named expression").field(
                    "definition",
                    Schema::string("").doc(
                        "Expression text with units, e.g. \"0.1 [m/s] * (1 - (y/0.05 [m])^2)\"; may refer to other named expressions",
                    ),
                ),
            )
            .doc("Named expressions that boundary conditions and reports can use"),
        )
        .field(
            "tables",
            Schema::named(
                Schema::object()
                    .doc("A lookup table or profile read from CSV data")
                    .field(
                        "file",
                        Schema::string("").doc("CSV file (relative to the case file); first line names the columns, optionally with units in brackets, e.g. \"T [K], nu [m^2/s]\""),
                    )
                    .field(
                        "data",
                        Schema::string("").doc("The same CSV text given inline (used when file is empty)"),
                    )
                    .field(
                        "inputs",
                        Schema::list(Schema::string("")).doc("Names of the input columns, in argument order (1 to 3), for example [\"T\"] or [\"y\"]"),
                    )
                    .field(
                        "outputs",
                        Schema::list(Schema::string("")).doc("Output columns to keep (empty = all the other columns)"),
                    )
                    .field(
                        "units",
                        Schema::list(Schema::string("")).doc("Units for columns without a unit in the header, as \"column = unit\" entries. Columns x, y, z, t, ux, uy, uz, T and heat_flux default to m, m, m, s, m/s, m/s, m/s, K and W/m^2"),
                    )
                    .field(
                        "extrapolation",
                        Schema::enumeration("error", &["error", "clamp"]).doc("Behaviour outside the data range: error (default) or the nearest edge value"),
                    ),
            )
            .doc("Lookup tables and profiles; callable from expressions as <name>(args) (first output) or <name>_<column>(args)"),
        )
        .field(
            "udf",
            Schema::named(
                Schema::object()
                    .doc("A sandboxed user-function module (WebAssembly, ABI version 1, see docs/udf-abi.md)")
                    .field(
                        "file",
                        Schema::string("").doc("WebAssembly binary (.wasm, relative to the case file); it may import nothing"),
                    )
                    .field(
                        "fuel_per_call",
                        Schema::int(1_000_000).doc("Instruction budget of one call; a call that exceeds it stops the run with an error"),
                    )
                    .field(
                        "momentum_source",
                        Schema::boolean(false).doc("Use the module's momentum_source export as a body acceleration (steady SIMPLE/SIMPLEC runs; at most one module)"),
                    )
                    .field(
                        "viscosity",
                        Schema::boolean(false).doc("Use the module's kinematic_viscosity export as the laminar viscosity (steady runs without a turbulence model; at most one module)"),
                    )
                    .field(
                        "monitor",
                        Schema::boolean(false).doc("Record the module's monitor export as the monitor udf_<name> (sum of value times cell volume)"),
                    ),
            )
            .doc("Sandboxed user functions (WebAssembly)"),
        )
        .field(
            "sources",
            Schema::named(
                Schema::object()
                    .doc("A momentum source restricted to a cell zone (SIMPLE/SIMPLEC, PISO and PIMPLE runs; rotating_frame and mrf steady only)")
                    .field(
                        "type",
                        Schema::enumeration("porous", &["porous", "rotating_frame", "mrf", "acceleration", "actuator_disk", "lorentz"])
                            .doc("lorentz: Lorentz body force J x B / rho of a liquid conductor in an imposed magnetic field, J = sigma (E_ext - grad phi + u x B), low magnetic Reynolds number (ADR 0039; SIMPLE/SIMPLEC, PISO, PIMPLE per ADR 0045); actuator_disk: thrust (and optional torque) distributed over the zone as a body force, uniform or proportional to the distance from the disk axis (ADR 0036); porous: Darcy-Forchheimer resistance (superficial velocity); rotating_frame: relative-velocity Coriolis and centrifugal accelerations; mrf: multiple-reference-frame (frozen rotor) zone in the absolute-velocity formulation, boundary conditions stay in the absolute frame (ADR 0008); acceleration: constant acceleration"),
                    )
                    .field(
                        "zone",
                        Schema::enumeration("all", &["all", "box", "cell_zone", "register"])
                            .doc("Where the source acts: the whole mesh, the cells whose centres lie in the axis-aligned box [box_min, box_max], a named mesh cell zone (imported meshes) or a cell register (registers.<name>)"),
                    )
                    .field("register", Schema::string("").doc("Name of the register for zone = \"register\""))
                    .field("box_min", vec3([0.0; 3], "Lower corner [m] of the box zone"))
                    .field("box_max", vec3([0.0; 3], "Upper corner [m] of the box zone"))
                    .field("cell_zone", Schema::string("").doc("Name of the mesh cell zone"))
                    .field(
                        "model",
                        Schema::enumeration("ergun", &["ergun", "darcy_forchheimer"])
                            .doc("porous: ergun derives the resistances from porosity and particle_diameter (150 and 1.75 constants of Ergun 1952); darcy_forchheimer takes permeability and inertial_resistance directly"),
                    )
                    .field(
                        "porosity",
                        Schema::float(0.4).doc("Bed void fraction for the Ergun model (0 < e < 1); the velocity stays the superficial one"),
                    )
                    .field(
                        "particle_diameter",
                        Schema::float(0.0).dim(Dimension::LENGTH).doc("Particle diameter [m] for the Ergun model"),
                    )
                    .field(
                        "permeability",
                        Schema::float(0.0).doc("Permeability K [m^2] of the Darcy term nu/K u (0 = no viscous resistance)"),
                    )
                    .field(
                        "inertial_resistance",
                        Schema::float(0.0).doc("Inertial resistance C2 [1/m] of the Forchheimer term C2 |u| u / 2"),
                    )
                    .field("rotation_origin", vec3([0.0; 3], "A point on the rotation axis [m] (default the origin)"))
                    .field("rotation_axis", vec3([0.0, 0.0, 1.0], "Rotation axis direction (default +z); right-handed"))
                    .field(
                        "angular_velocity",
                        Schema::float(0.0).dim(Dimension::ANGULAR_VELOCITY).doc("Angular velocity Omega [rad/s] of the frame; the solved velocity is relative to it, so wall velocities are given in the rotating frame"),
                    )
                    .field(
                        "centrifugal",
                        Schema::boolean(true).doc("Include the centrifugal acceleration Omega^2 r_perp of the rotating frame (false keeps only the Coriolis force, which is needed for periodic domains)"),
                    )
                    .field("acceleration", vec3([0.0; 3], "Constant acceleration [m/s^2] for type = acceleration"))
                    .field(
                        "thrust",
                        Schema::float(0.0).doc("actuator_disk: total force [N] exerted on the fluid along direction (signed); the force is spread over the zone cells so that the integral is exactly this value"),
                    )
                    .field("direction", vec3([0.0; 3], "actuator_disk: direction of the thrust on the fluid (required, non-zero)"))
                    .field(
                        "profile",
                        Schema::enumeration("uniform", &["uniform", "linear_r"]).doc("actuator_disk: force density uniform, or proportional to the distance from the disk axis (needs disk_centre)"),
                    )
                    .field("disk_centre", vec3([0.0; 3], "actuator_disk: a point on the disk axis [m] (radial coordinate of linear_r and the swirl)"))
                    .field("disk_axis", vec3([0.0; 3], "actuator_disk: disk axis direction; empty = direction"))
                    .field(
                        "torque",
                        Schema::float(0.0).doc("actuator_disk: torque [N m] exerted on the fluid about disk_axis (right-handed); 0 = no swirl"),
                    )
                    .field(
                        "distribution",
                        Schema::enumeration("smooth", &["smooth", "cell"]).doc("actuator_disk: smooth (default) spreads the force over one extra cell layer so that the discrete pressure gradient balances it in the zone edge cells; cell keeps the sharp top-hat of cell forces (exact zone, but the edge cells see an unbalanced half force)"),
                    )
                    .field(
                        "density",
                        Schema::float(0.0).doc("actuator_disk: density [kg/m^3] converting force to the solver's kinematic acceleration; 0 = physics.rho"),
                    )
                    .field("magnetic_field", vec3([0.0; 3], "lorentz: applied uniform magnetic flux density [T] (required, non-zero); empty list = unset"))
                    .field(
                        "conductivity",
                        Schema::float(-1.0).doc("lorentz: electrical conductivity sigma [S/m] of the zone (required > 0; -1 = unset)"),
                    )
                    .field("applied_electric_field", vec3([0.0; 3], "lorentz: imposed uniform electric field E_ext [V/m] added to -grad phi (default 0)"))
                    .field(
                        "potential",
                        Schema::enumeration("none", &["none", "solve"]).doc("lorentz: none = grad phi = 0 (J = sigma (E_ext + u x B), the fully developed / short-circuited approximation); solve = solve div(sigma grad phi) = div(sigma (E_ext + u x B)) with the boundaries.<patch>.electric conditions (insulated by default; a cell is pinned when no patch fixes the potential)"),
                    ),
            )
            .doc("Momentum sources (porous media, rotating reference frame, constant acceleration) acting in cell zones; SIMPLE/SIMPLEC, PISO and PIMPLE runs (ADR 0045; rotating_frame and mrf steady only)"),
        )
        .field(
            "components",
            Schema::named(
                Schema::object()
                    .doc("A lumped zone component (ADR 0012; SIMPLE/SIMPLEC, PISO and PIMPLE runs per ADR 0045, constant density)")
                    .field(
                        "type",
                        Schema::enumeration("fan", &["fan", "porous_jump", "heat_exchanger", "network_outlet", "fan_boundary"])
                            .doc("fan_boundary: fixed-pressure patch whose static pressure follows reference_pressure + the fan curve at the volume flow into the domain (ADR 0039); fan: pressure rise from a fan curve applied as a pressure jump on the downstream boundary faces of the zone; porous_jump: thin Darcy-Forchheimer pressure jump on the downstream boundary faces of the zone; heat_exchanger: radiator / heat-exchanger heat source in a zone (fixed duty or effectiveness; needs energy.enabled); network_outlet: fixed-pressure patch whose pressure follows a lumped downstream network"),
                    )
                    .field(
                        "zone",
                        Schema::enumeration("all", &["all", "box", "cell_zone"])
                            .doc("Where the component acts (fan, porous_jump, heat_exchanger): the whole mesh, the cells whose centres lie in [box_min, box_max], or a named mesh cell zone"),
                    )
                    .field("box_min", vec3([0.0; 3], "Lower corner [m] of the box zone"))
                    .field("box_max", vec3([0.0; 3], "Upper corner [m] of the box zone"))
                    .field("cell_zone", Schema::string("").doc("Name of the mesh cell zone"))
                    .field("direction", vec3([1.0, 0.0, 0.0], "Flow direction through the zone (fan, porous_jump, heat_exchanger); the zone needs faces on its downstream side for this direction"))
                    .field(
                        "curve_flow",
                        Schema::list(Schema::float(0.0)).doc("fan: volume flow points [m^3/s] of the fan curve, strictly increasing (end values are held outside the range)"),
                    )
                    .field(
                        "curve_pressure",
                        Schema::list(Schema::float(0.0)).doc("fan: total pressure rise [Pa] at the points of curve_flow"),
                    )
                    .field(
                        "thickness",
                        Schema::float(0.0).dim(Dimension::LENGTH).doc("porous_jump: thickness t [m] of the jump: dp = (mu/permeability) t v + C2 t rho v^2/2"),
                    )
                    .field(
                        "permeability",
                        Schema::float(0.0).doc("porous_jump: face permeability alpha [m^2] (0 = no viscous term)"),
                    )
                    .field(
                        "inertial_resistance",
                        Schema::float(0.0).doc("porous_jump: pressure-jump coefficient C2 [1/m] (0 = no inertial term)"),
                    )
                    .field(
                        "mode",
                        Schema::enumeration("fixed_duty", &["fixed_duty", "effectiveness", "ntu"])
                            .doc("heat_exchanger: fixed_duty uses duty, effectiveness uses Q = eps C_min (T_secondary - T_in), ntu uses the effectiveness-NTU relation of the arrangement with NTU = ua / C_min evaluated at the actual flow (ADR 0039); secondary_temperature and secondary_capacity_rate describe a prescribed secondary stream, or dual_cell takes a second zone of the mesh"),
                    )
                    .field(
                        "duty",
                        Schema::float(0.0).doc("heat_exchanger, fixed_duty: heat added to the fluid [W] (negative for a radiator)"),
                    )
                    .field(
                        "effectiveness",
                        Schema::float(0.5).doc("heat_exchanger, effectiveness mode: effectiveness in (0, 1]"),
                    )
                    .field(
                        "secondary_temperature",
                        Schema::float(293.15).dim(Dimension::TEMPERATURE).doc("heat_exchanger, effectiveness mode: inlet temperature [K] of the secondary stream"),
                    )
                    .field(
                        "secondary_capacity_rate",
                        Schema::float(0.0).doc("heat_exchanger, effectiveness mode: capacity rate m cp [W/K] of the secondary stream (0 = infinite)"),
                    )
                    .field(
                        "arrangement",
                        Schema::enumeration("counterflow", &["counterflow", "parallel", "crossflow_unmixed", "crossflow_cmax_mixed", "crossflow_cmin_mixed", "shell_and_tube"])
                            .doc("heat_exchanger, ntu mode: flow arrangement of the effectiveness-NTU relation (crossflow_unmixed is the exact series solution (ADR 0044); shell_and_tube is one shell pass)"),
                    )
                    .field(
                        "ua",
                        Schema::float(-1.0).doc("heat_exchanger, ntu mode: overall conductance UA [W/K] (required > 0; -1 = unset)"),
                    )
                    .field(
                        "dual_cell",
                        Schema::boolean(false).doc("heat_exchanger, ntu mode: the secondary stream is the flow through a second zone of the mesh (the secondary object) instead of a prescribed stream; the heat leaving one zone enters the other exactly"),
                    )
                    .field(
                        "loss_coefficient",
                        Schema::float(0.0).doc("heat_exchanger: pressure-loss coefficient K of the core, dp = K rho v|v|/2 as a pressure jump on the downstream interior plane of the zone (0 = none)"),
                    )
                    .field(
                        "loss_linear",
                        Schema::float(0.0).doc("heat_exchanger: linear pressure-loss coefficient r1 [Pa s/m] of the core, dp = r1 v on the downstream interior plane (0 = none)"),
                    )
                    .field(
                        "secondary",
                        Schema::object()
                            .doc("heat_exchanger, dual_cell: the secondary zone (same keys as the primary zone)")
                            .field(
                                "zone",
                                Schema::enumeration("all", &["all", "box", "cell_zone"]).doc("Cells of the secondary zone: whole mesh, box or named cell zone"),
                            )
                            .field("box_min", vec3([0.0; 3], "Lower corner [m] of the box zone"))
                            .field("box_max", vec3([0.0; 3], "Upper corner [m] of the box zone"))
                            .field("cell_zone", Schema::string("").doc("Name of the mesh cell zone"))
                            .field("direction", vec3([1.0, 0.0, 0.0], "Flow direction of the secondary stream through its zone"))
                            .field(
                                "loss_coefficient",
                                Schema::float(0.0).doc("Pressure-loss coefficient K of the secondary core (0 = none)"),
                            )
                            .field(
                                "loss_linear",
                                Schema::float(0.0).doc("Linear pressure-loss coefficient r1 [Pa s/m] of the secondary core (0 = none)"),
                            ),
                    )
                    .field("patch", Schema::string("").doc("network_outlet, fan_boundary: name of the boundary patch (its pressure must be fixed)"))
                    .field(
                        "reference_pressure",
                        Schema::float(0.0).doc("network_outlet: pressure [Pa] at the far end of the network; fan_boundary: ambient static pressure [Pa] the fan pressure rise is added to"),
                    )
                    .field(
                        "resistance_linear",
                        Schema::float(0.0).doc("network_outlet: linear resistance R1 [Pa s/m^3]"),
                    )
                    .field(
                        "resistance_quadratic",
                        Schema::float(0.0).doc("network_outlet: quadratic resistance R2 [Pa s^2/m^6]"),
                    )
                    .field(
                        "relax",
                        Schema::float(0.5).range(0.01, 1.0).doc("fan, network_outlet, fan_boundary: under-relaxation of the lagged pressure update"),
                    ),
            )
            .doc("Lumped zone components: fans, porous jumps, radiator / heat-exchanger heat sources and lumped network outlets (ADR 0012)"),
        )
        .field(
            "radiation",
            Schema::object()
                .doc("Grey radiation in a participating medium coupled to the energy equation (steady SIMPLE/SIMPLEC, needs energy.enabled; ADR 0012)")
                .field(
                    "model",
                    Schema::enumeration("none", &["none", "rosseland", "p1"]).doc("none, rosseland (diffusion approximation: effective radiative conductivity 16 sigma n^2 T^3 / (3 beta)) or p1 (incident radiation G with Marshak boundary conditions)"),
                )
                .field(
                    "absorption_coefficient",
                    Schema::float(0.0).doc("Grey absorption coefficient a [1/m]"),
                )
                .field(
                    "scattering_coefficient",
                    Schema::float(0.0).doc("Isotropic scattering coefficient [1/m]"),
                )
                .field(
                    "refractive_index",
                    Schema::float(1.0).doc("Refractive index n of the medium"),
                ),
        )
        .field(
            "properties",
            Schema::object()
                .doc("Temperature-dependent fluid properties (steady runs with the energy equation); constant density")
                .field("viscosity", property_law("Dynamic viscosity mu(T) [Pa s]; the kinematic viscosity is mu(T) / physics.rho on the faces"))
                .field("conductivity", property_law("Thermal conductivity k(T) [W/m/K]"))
                .field("specific_heat", property_law("Specific heat cp(T) [J/kg/K]")),
        )
        .field(
            "scalars",
            Schema::named(scalar_schema(false))
                .doc("Passive scalars and user variables (steady iterations or transient steps): div(phi s) - div(D grad s) = S, constant density; equation selects convection-diffusion, pure diffusion, Poisson or an algebraic expression"),
        )
        .field(
            "species",
            Schema::named(scalar_schema(true)).doc(
                "Multicomponent mass fractions (steady or transient incompressible runs): all but the balance species are transported, the balance species is 1 - sum of the others; every transported species is conserved separately",
            ),
        )
        .field("reports", Schema::named(report).doc("Scalar reports by name"))
        .field(
            "convergence",
            Schema::named(convergence).doc("Convergence and interrupt conditions that stop the run automatically"),
        )
        .field("statistics", statistics)
        .field("output", output)
        .field("decomposition", crate::decomp::schema())
        .field("linear_solver", crate::linsolver::schema())
        .field("postprocessing", crate::postproc::schema())
        .field("restart", crate::restart::schema())
        .field("chain", crate::chain::schema())
        .field("study", crate::study::schema())
}

/// Settings of one passive scalar or species.
fn scalar_schema(species: bool) -> Schema {
    let mut s = Schema::object()
        .doc(if species {
            "A species mass fraction"
        } else {
            "A passive scalar"
        })
        .field(
            "diffusivity",
            Schema::float(0.0).doc("Molecular diffusivity [m^2/s]; give this or schmidt"),
        )
        .field(
            "schmidt",
            Schema::float(0.0).doc("Schmidt number nu / D; the diffusivity then follows the (temperature-dependent) viscosity of the flow solver"),
        )
        .field("initial", Schema::float(0.0).doc("Initial uniform value"))
        .field(
            "source",
            Schema::float(0.0).doc("Uniform volumetric source (value per second)"),
        )
        .field(
            "relax",
            Schema::float(1.0).doc("Implicit under-relaxation of the sweep (1 for the linear frozen-flow problem)"),
        );
    if !species {
        s = s
            .field(
                "equation",
                Schema::enumeration("transport", &["transport", "diffusive", "poisson", "algebraic"]).doc("transport: convection-diffusion-source (steady iterations or transient steps); diffusive: d s/dt = div(D grad s) + S without convection; poisson: div(D grad s) + S = 0 solved at every step without a time derivative; algebraic: s = expression, no transport (ADR 0019)"),
            )
            .field(
                "diffusivity_expression",
                Schema::string("").doc("Diffusivity [m^2/s] as an expression of x, y, z and t, evaluated at the face centres; replaces diffusivity (not with schmidt)"),
            )
            .field(
                "source_expression",
                Schema::string("").doc("Source as an expression of x, y, z and t at the cell centres, dimensionless or 1/s (value per second); added to source"),
            )
            .field(
                "initial_expression",
                Schema::string("").doc("Dimensionless initial field as an expression of x, y, z; replaces initial"),
            )
            .field(
                "expression",
                Schema::string("").doc("equation = algebraic: the value as an expression of x, y, z, t, T (with energy.enabled) and the names of the other scalars and species (dimensionless result)"),
            );
    }
    if species {
        s = s.field(
            "remainder",
            Schema::boolean(false)
                .doc("This species is the balance species, closed by difference (default: the last one by name)"),
        );
    }
    s.field(
        "boundaries",
        Schema::named(
            Schema::object()
                .doc("Condition on a patch (default zero_gradient)")
                .field(
                    "type",
                    Schema::enumeration("zero_gradient", &["zero_gradient", "fixed", "gradient"])
                        .doc("fixed value, or prescribed outward normal gradient"),
                )
                .field("value", Schema::float(0.0).doc("Value or outward normal gradient"))
                .field(
                    "value_expression",
                    Schema::string("")
                        .doc("Dimensionless expression of x, y, z (F-104) for a fixed value; overrides value"),
                ),
        )
        .doc("Boundary conditions by patch name"),
    )
}

/// One temperature-dependent property law.
fn property_law(doc: &str) -> Schema {
    Schema::object()
        .doc(doc)
        .field(
            "law",
            Schema::enumeration("constant", &["constant", "polynomial", "sutherland", "piecewise_linear", "piecewise_polynomial", "vogel"]).doc(
                "constant: the physics value; polynomial: c0 + c1 T + c2 T^2 + ... (T in kelvin); sutherland: v_ref (T/T_ref)^1.5 (T_ref + S) / (T + S); piecewise_linear: table (temperatures, values); piecewise_polynomial: one polynomial per temperature range (breakpoints, piece_coefficients, piece_order); vogel: vogel_a exp(vogel_b / (T - vogel_c)) (ADR 0036)",
            ),
        )
        .field(
            "temperatures",
            Schema::list(Schema::float(0.0)).doc("piecewise_linear: table temperatures [K], strictly increasing; empty = unset"),
        )
        .field(
            "values",
            Schema::list(Schema::float(0.0)).doc("piecewise_linear: property value at each temperature; empty = unset"),
        )
        .field(
            "breakpoints",
            Schema::list(Schema::float(0.0)).doc("piecewise_polynomial: n+1 strictly increasing range limits [K] of n pieces; empty = unset"),
        )
        .field(
            "piece_coefficients",
            Schema::list(Schema::float(0.0)).doc("piecewise_polynomial: n x piece_order coefficients, piece by piece, ascending powers of T (absolute T in kelvin)"),
        )
        .field(
            "piece_order",
            Schema::int(0).doc("piecewise_polynomial: coefficients per piece (degree + 1); 0 = unset (an error for this law)"),
        )
        .field(
            "extrapolation",
            Schema::enumeration("clamp", &["clamp", "linear", "error"]).doc("piecewise laws outside the table range: clamp holds the end value, linear continues the end segment, error returns NaN outside the range (no run-time stop; ADR 0036 addendum)"),
        )
        .field(
            "vogel_a",
            Schema::float(0.0).doc("vogel: prefactor (> 0)"),
        )
        .field(
            "vogel_b",
            Schema::float(0.0).dim(Dimension::TEMPERATURE).doc("vogel: exponent constant [K]"),
        )
        .field(
            "vogel_c",
            Schema::float(0.0).dim(Dimension::TEMPERATURE).doc("vogel: Vogel temperature [K] (the law needs T > vogel_c)"),
        )
        .field(
            "coefficients",
            Schema::list(Schema::float(0.0)).doc("Polynomial coefficients in ascending powers of T [K]"),
        )
        .field(
            "reference_value",
            Schema::float(0.0).doc("Sutherland: value at the reference temperature"),
        )
        .field(
            "reference_temperature",
            Schema::float(0.0).dim(Dimension::TEMPERATURE).doc("Sutherland: reference temperature [K]"),
        )
        .field(
            "sutherland_constant",
            Schema::float(0.0).dim(Dimension::TEMPERATURE).doc("Sutherland constant S [K]"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_core::settings::{validate_json_schema, SettingsTree};

    #[test]
    fn defaults_validate_against_the_exported_json_schema() {
        let t = SettingsTree::new(case_schema());
        let js = t.schema().to_json_schema();
        validate_json_schema(&js, &t.to_json()).unwrap();
        assert_eq!(
            js["properties"]["solver"]["properties"]["algorithm"]["enum"][0],
            "simple"
        );
        assert_eq!(t.get("physics.nu").unwrap(), serde_json::json!(0.01));
    }

    #[test]
    fn units_are_accepted_for_dimensioned_settings() {
        let mut t = SettingsTree::new(case_schema());
        t.set("physics.nu", serde_json::json!("1.5e-5 m^2/s")).unwrap();
        assert!((t.get("physics.nu").unwrap().as_f64().unwrap() - 1.5e-5).abs() < 1e-18);
        assert!(t.set("physics.nu", serde_json::json!("3 m")).is_err());
        assert!(t.set("solver.algorithm", serde_json::json!("nonsense")).is_err());
    }
}
