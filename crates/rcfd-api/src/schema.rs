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
];

/// Names of fields that can be requested from a simulation.
pub const FIELD_NAMES: &[&str] = &[
    "ux",
    "uy",
    "uz",
    "speed",
    "p",
    "T",
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
        .field(
            "source",
            Schema::enumeration("generator", &["generator", "file"]).doc("Where the mesh comes from"),
        )
        .field(
            "generator",
            Schema::enumeration(
                "cavity_2d",
                &["cavity_2d", "box_2d", "channel_2d", "cylinder_ogrid", "wedge_2d"],
            )
            .doc("Built-in generator (used when source = generator); wedge_2d is a box_2d whose lower wall turns by ramp_angle at ramp_start (patches xmin, xmax, ymin, ymax)"),
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
            "ramp_start",
            Schema::float(0.2)
                .at_least(0.0)
                .dim(Dimension::LENGTH)
                .doc("x position where the lower wall starts to rise (wedge_2d)"),
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
        .field("pressure", Schema::float(0.0).doc("Initial kinematic pressure"))
        .field(
            "temperature",
            Schema::float(293.15)
                .at_least(0.0)
                .dim(Dimension::TEMPERATURE)
                .doc("Initial temperature (energy equation)"),
        );

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
        );

    let boundary = Schema::object()
        .doc("Boundary conditions for one mesh patch")
        .field(
            "velocity",
            Schema::enumeration("fixed", &["fixed", "zero_gradient", "symmetry", "freestream"])
                .doc("Velocity condition (fixed with value 0 is a no-slip wall)"),
        )
        .field(
            "velocity_value",
            vec3([0.0; 3], "Velocity vector for fixed / freestream conditions"),
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
            Schema::enumeration("zero_gradient", &["zero_gradient", "fixed", "heat_flux"])
                .doc("Temperature condition (energy equation): adiabatic, fixed value or prescribed heat flux"),
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
            "compressible_bc",
            Schema::enumeration("auto", &["auto", "slip", "extrapolate", "fixed"]).doc(
                "Compressible solver condition: slip = wall / symmetry, extrapolate = supersonic outflow, fixed = free-stream or inflow state from velocity_value, pressure_value (static, Pa) and density_value (or temperature_value through the ideal gas law). auto: extrapolate on outlet patches, slip elsewhere",
            ),
        )
        .field(
            "density_value",
            Schema::float(0.0)
                .at_least(0.0)
                .dim(Dimension::DENSITY)
                .doc("Density of a fixed compressible state; 0 = derive from pressure_value and temperature_value"),
        );

    let solver = Schema::object()
        .doc("Numerical method and run control")
        .field(
            "algorithm",
            Schema::enumeration("simplec", &["simple", "simplec", "piso", "pimple", "compressible"])
                .doc("Pressure-velocity coupling: simple/simplec are steady, piso/pimple transient; compressible selects the density-based explicit Euler solver (see the compressible section)"),
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
            "time_scheme",
            Schema::enumeration("euler", &["euler", "bdf2"]).doc("Time integration scheme (transient)"),
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
                .doc("Adaptive time-step Courant target; 0 keeps dt fixed"),
        )
        .field(
            "h_operator",
            Schema::enumeration("default", &["default", "from_solution", "previous_iterate"]).doc(
                "H operator of the SIMPLE/SIMPLEC pressure equation: from the solved momentum predictor (classic SIMPLE) or from the previous iterate. default keeps the library default (always from_solution with k-epsilon)",
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
                ],
            )
            .doc("What to compute (y_plus: wall y+ minimum / maximum / average over a wall patch, k-epsilon runs)"),
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
            Schema::float(1.0)
                .at_least(0.0)
                .doc("Thermal conductivity for heat_flow"),
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
                .doc("Write VTK every N iterations/steps (0 = only at the end)"),
        );

    let turbulence = Schema::object()
        .doc("Turbulence modelling (steady SIMPLE / SIMPLEC runs)")
        .field(
            "model",
            Schema::enumeration("laminar", &["laminar", "k_epsilon", "k_omega_sst"]).doc("laminar, the k-epsilon eddy-viscosity family with wall functions, or k-omega SST (Menter 2003) with an automatic wall treatment that works from y+ about 1 (wall-resolved) to the log layer"),
        )
        .field(
            "variant",
            Schema::enumeration("standard", &["standard", "rng", "realizable"]).doc(
                "k-epsilon variant: standard (Launder and Spalding 1974), rng (Yakhot et al. 1992) or realizable (Shih et al. 1995)",
            ),
        )
        .field(
            "wall_function",
            Schema::enumeration("standard", &["standard"]).doc(
                "Wall treatment: standard equilibrium log-law wall functions (Launder and Spalding 1974). Scalable wall functions and rough walls are not available",
            ),
        )
        .field(
            "wall_treatment",
            Schema::enumeration("smooth", &["smooth", "hysteresis", "switch"]).doc(
                "Wall-cell treatment around the viscous/log intersection y* = 11.5: smooth (blended epsilon and production, no chatter, default), hysteresis (sharp switch with a 10 percent dead band) or switch (sharp Launder-Spalding switch; the eddy viscosity of cells near y* = 11.5 may oscillate and res_nut then never converges)",
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

    Schema::object()
        .doc("rcfd case definition")
        .field("title", Schema::string("untitled").doc("Case title"))
        .field("mesh", mesh)
        .field("physics", physics)
        .field("energy", energy)
        .field("initial", initial)
        .field(
            "boundaries",
            Schema::named(boundary).doc("Boundary conditions by patch name"),
        )
        .field("solver", solver)
        .field("turbulence", turbulence)
        .field("compressible", compressible)
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
        .field("reports", Schema::named(report).doc("Scalar reports by name"))
        .field(
            "convergence",
            Schema::named(convergence).doc("Convergence and interrupt conditions that stop the run automatically"),
        )
        .field("output", output)
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
