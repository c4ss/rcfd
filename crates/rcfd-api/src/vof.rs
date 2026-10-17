//! Volume-of-fluid free-surface flow in the settings tree (F-055, ADR 0049 D6): `multiphase.enabled` selects
//! [`rcfd_fv::vof`] (two incompressible immiscible fluids, algebraic interface compression, CSF surface tension, well-balanced
//! `p_rgh` gravity) instead of the single-phase transient kernel. Nothing changes for a case without `multiphase.enabled`.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_i, get_s, get_v};
use rcfd_core::settings::Schema;
use rcfd_core::vec3;
use rcfd_fv::vof::{
    shape_fraction, MassTransfer, MassTransferModel, RelaxTarget, RelaxZone, Shape, VofSettings, VofSolver, WaveInlet,
    WaveSettings,
};
use rcfd_fv::waves::{Blend, Wave};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use std::sync::Arc;

/// Sentinel of the unset positive numbers.
const UNSET: f64 = -1.0;

fn d(path: impl Into<String>, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// Is the free-surface solver selected?
pub fn enabled(case: &Case) -> bool {
    get_b(case, "multiphase.enabled")
}

/// Schema of the `multiphase` section (all keys inert while `enabled = false`; setting one then is an error).
pub fn schema() -> Schema {
    let nums = |doc: &'static str| Schema::list(Schema::float(0.0)).doc(doc);
    let initial = Schema::object()
        .doc("Initial phase fraction: a shape; the cells are sub-sampled so a face-aligned box is exact. alpha = 1 is phase 1")
        .field(
            "shape",
            Schema::enumeration("unset", &["unset", "box", "sphere", "half_space"])
                .doc("Shape of the region: box (box_min, box_max), sphere (centre, radius; a disk in 2-D) or half_space (normal, point: the side n . (x - point) <= 0); must be set"),
        )
        .field(
            "inside",
            Schema::enumeration("phase_1", &["phase_1", "phase_2"])
                .doc("Which phase fills the shape; the other phase fills the rest of the domain"),
        )
        .field("box_min", nums("box: lower corner [x, y, z] (in 2-D give a z range that covers the mesh thickness); empty = unset"))
        .field("box_max", nums("box: upper corner [x, y, z]; empty = unset"))
        .field("centre", nums("sphere: centre [x, y, z]; empty = unset"))
        .field("radius", Schema::float(UNSET).doc("sphere: radius [m] (> 0); -1 = unset"))
        .field("normal", nums("half_space: normal [nx, ny, nz] (non-zero); empty = unset"))
        .field("point", nums("half_space: a point on the plane [x, y, z]; empty = unset"));
    Schema::object()
        .doc("Volume-of-fluid free-surface flow of two incompressible immiscible fluids (F-055, ADR 0049): phase fraction with interface compression and a bounded (flux-corrected) explicit limiter, CSF surface tension, p_rgh gravity, transient piso / pimple only. Gravity is physics.gravity; wall conditions are the boundaries.<patch> conditions (velocity fixed or zero_gradient, pressure fixed (a value of p_rgh) or zero_gradient); pressures are in Pa")
        .field("enabled", Schema::boolean(false).doc("Solve the two-phase free-surface flow"))
        .field("model", Schema::enumeration("vof", &["vof"]).doc("Multiphase model (only the algebraic volume of fluid)"))
        .field("density_1", Schema::float(UNSET).doc("Density of phase 1 (alpha = 1) [kg/m^3]; must be set (> 0), -1 = unset"))
        .field("density_2", Schema::float(UNSET).doc("Density of phase 2 (alpha = 0) [kg/m^3]; must be set (> 0), -1 = unset"))
        .field("viscosity_1", Schema::float(UNSET).doc("Dynamic viscosity of phase 1 [Pa s]; must be set (> 0), -1 = unset"))
        .field("viscosity_2", Schema::float(UNSET).doc("Dynamic viscosity of phase 2 [Pa s]; must be set (> 0), -1 = unset"))
        .field("surface_tension", Schema::float(0.0).at_least(0.0).doc("Surface tension coefficient [N/m] (continuum surface force, 90 degree contact angle at walls); 0 = none"))
        .field("compression", Schema::float(1.0).range(0.0, 4.0).doc("Interface compression coefficient c_alpha (0 = none, 1 = standard)"))
        .field("alpha_courant", Schema::float(0.25).range(1e-3, 0.5).doc("Largest Courant number of a phase-fraction sub-step (the phase fraction is sub-cycled inside each flow step)"))
        .field("curvature_smoothing", Schema::int(2).range(0.0, 10.0).doc("Smoothing passes of the phase fraction before the curvature is taken"))
        .field("initial", initial)
        .field("reference_point", Schema::list(Schema::float(0.0)).doc("Point [x, y, z] where p_rgh equals p (origin of the gravity potential g.(x - x_ref)); empty = the origin. For a tank with an open top set a point on the open boundary so that still water is in equilibrium with p_rgh = 0 there"))
        .field("mass_transfer", mass_transfer_schema())
        .field("waves", waves_schema())
}

fn mass_transfer_schema() -> Schema {
    let nums = |doc: &'static str| Schema::list(Schema::float(0.0)).doc(doc);
    Schema::object()
        .doc("Evaporation-condensation between the two phases (F-226, ADR 0052 B): sources in the phase-fraction equation (S_alpha = -mdot / rho_l for the liquid) and the continuity equation (volume source mdot (1/rho_v - 1/rho_l)). There is NO energy equation in the multiphase path: the temperature is a PRESCRIBED field (uniform plus an optional constant gradient) and the latent heat is only reported, never applied. A pressure patch (boundaries.<patch>.pressure = fixed) is required for the volume to leave through")
        .field("model", Schema::enumeration("none", &["none", "lee", "hertz_knudsen"]).doc("none, lee (temperature-driven relaxation, rate c alpha rho theta with theta = (T - T_sat)/T_sat) or hertz_knudsen (kinetic flux times |grad alpha|, Clausius-Clapeyron saturation pressure)"))
        .field("liquid_phase", Schema::enumeration("phase_1", &["phase_1", "phase_2"]).doc("Which phase is the liquid (the other is its vapour)"))
        .field("saturation_temperature", Schema::float(UNSET).doc("T_sat [K] (> 0); -1 = unset"))
        .field("latent_heat", Schema::float(UNSET).doc("h_fg [J/kg] (> 0); -1 = unset"))
        .field("evaporation_frequency", Schema::float(0.0).at_least(0.0).doc("lee: c_evap [1/s]"))
        .field("condensation_frequency", Schema::float(0.0).at_least(0.0).doc("lee: c_cond [1/s]"))
        .field("temperature", Schema::float(UNSET).doc("Prescribed temperature at temperature_point [K] (> 0); -1 = unset"))
        .field("temperature_gradient", nums("Constant temperature gradient [K/m] as [gx, gy, gz]; empty = uniform"))
        .field("temperature_point", nums("Point [x, y, z] where the temperature is `temperature`; empty = the origin"))
        .field("accommodation", Schema::float(1.0).range(0.0, 1.0).doc("hertz_knudsen: accommodation coefficient in (0, 1]"))
        .field("schrage", Schema::boolean(false).doc("hertz_knudsen: Schrage factor 2 sigma / (2 - sigma)"))
        .field("molar_mass", Schema::float(UNSET).doc("hertz_knudsen: molar mass [kg/mol] (> 0); -1 = unset"))
        .field("vapour_pressure", Schema::float(UNSET).doc("hertz_knudsen: vapour-side pressure [Pa] (>= 0); -1 = unset"))
        .field("saturation_pressure", Schema::float(UNSET).doc("hertz_knudsen: saturation pressure at saturation_temperature [Pa] (> 0); -1 = unset"))
}

fn waves_schema() -> Schema {
    Schema::object()
        .doc("Wave generation and absorption in a 2-D tank (F-227, ADR 0052 C): a wave-inlet boundary (velocity and phase fraction from linear or second-order Stokes kinematics on `inlet_patch`, the patch at the smallest x, propagation along +x, gravity along -y, phase 1 = water) and a relaxation-zone numerical beach that blends alpha and u toward still water with the weights of the wave library. Sub-keys are inert unless inlet_patch is set or beach = still_water")
        .field("inlet_patch", Schema::string("").doc("Boundary patch that generates the wave (its velocity condition must be `fixed`; it is overridden every step); empty = no inlet"))
        .field("height", Schema::float(UNSET).doc("Wave height H = 2 a [m] (> 0); -1 = unset"))
        .field("period", Schema::float(UNSET).doc("Wave period [s] (> 0); -1 = unset"))
        .field("depth", Schema::float(UNSET).doc("Still-water depth [m] above the bed (> 0); -1 = unset"))
        .field("level", Schema::float(UNSET).doc("y of the still-water surface [m]; -1 = depth (bed at y = 0)"))
        .field("order", Schema::int(1).range(1.0, 2.0).doc("1 = linear (Airy), 2 = second-order Stokes"))
        .field("phase", Schema::float(0.0).doc("Phase offset [rad]"))
        .field("ramp_periods", Schema::float(2.0).at_least(0.0).doc("Start-up ramp length in wave periods (0 = none)"))
        .field("beach", Schema::enumeration("none", &["none", "still_water"]).doc("Relaxation zone: none or still_water (damps toward still water)"))
        .field("beach_x_inner", Schema::float(0.0).doc("Beach: x where the computed solution has weight 1 [m]"))
        .field("beach_x_outer", Schema::float(0.0).doc("Beach: x where the target has weight 1 [m] (!= x_inner)"))
        .field("beach_blend", Schema::enumeration("exponential", &["exponential", "polynomial"]).doc("Weight shape: w = 1 - (exp(chi^p) - 1)/(e - 1) or w = (1 - chi)^n"))
        .field("beach_exponent", Schema::float(3.5).at_least(0.0).doc("p (exponential) or n (polynomial)"))
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let lst = |p: &str| -> usize {
        case.tree
            .get(p)
            .ok()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0)
    };
    if !enabled(case) {
        for k in ["density_1", "density_2", "viscosity_1", "viscosity_2", "radius"] {
            let p = if k == "radius" {
                "multiphase.initial.radius".to_string()
            } else {
                format!("multiphase.{k}")
            };
            if get_f(case, &p) != UNSET {
                out.push(d(p, "only used with multiphase.enabled = true"));
            }
        }
        for (p, def) in [
            ("multiphase.surface_tension", 0.0),
            ("multiphase.compression", 1.0),
            ("multiphase.alpha_courant", 0.25),
        ] {
            if (get_f(case, p) - def).abs() > 1e-300 {
                out.push(d(p, "only used with multiphase.enabled = true"));
            }
        }
        if get_i(case, "multiphase.curvature_smoothing") != 2 {
            out.push(d(
                "multiphase.curvature_smoothing",
                "only used with multiphase.enabled = true",
            ));
        }
        extension_inert(case, &mut out);
        for (p, def) in [
            ("multiphase.initial.shape", "unset"),
            ("multiphase.initial.inside", "phase_1"),
        ] {
            if get_s(case, p) != def {
                out.push(d(p, "only used with multiphase.enabled = true"));
            }
        }
        for k in ["box_min", "box_max", "centre", "normal", "point"] {
            let p = format!("multiphase.initial.{k}");
            if lst(&p) != 0 {
                out.push(d(p, "only used with multiphase.enabled = true"));
            }
        }
        return out;
    }
    // required phase properties
    for k in ["density_1", "density_2", "viscosity_1", "viscosity_2"] {
        let p = format!("multiphase.{k}");
        if get_f(case, &p) <= 0.0 {
            out.push(d(
                p,
                format!("multiphase.enabled = true needs a positive {k} (-1 = unset)"),
            ));
        }
    }
    // initial shape
    let shape = get_s(case, "multiphase.initial.shape");
    let need = |out: &mut Vec<Diagnostic>, k: &str| {
        let p = format!("multiphase.initial.{k}");
        if lst(&p) != 3 {
            out.push(d(p, format!("initial.shape = \"{shape}\" needs {k} = [x, y, z]")));
        }
    };
    match shape.as_str() {
        "unset" => out.push(d(
            "multiphase.initial.shape",
            "set the initial phase distribution: box, sphere or half_space",
        )),
        "box" => {
            need(&mut out, "box_min");
            need(&mut out, "box_max");
            if lst("multiphase.initial.box_min") == 3 && lst("multiphase.initial.box_max") == 3 {
                let (lo, hi) = (
                    get_v(case, "multiphase.initial.box_min", [0.0; 3]),
                    get_v(case, "multiphase.initial.box_max", [0.0; 3]),
                );
                if (0..3).any(|k| hi[k] <= lo[k]) {
                    out.push(d(
                        "multiphase.initial.box_max",
                        "every component of box_max must exceed box_min",
                    ));
                }
            }
        }
        "sphere" => {
            need(&mut out, "centre");
            if get_f(case, "multiphase.initial.radius") <= 0.0 {
                out.push(d(
                    "multiphase.initial.radius",
                    "initial.shape = \"sphere\" needs a positive radius (-1 = unset)",
                ));
            }
        }
        _ => {
            need(&mut out, "normal");
            need(&mut out, "point");
            if lst("multiphase.initial.normal") == 3
                && vec3::norm(get_v(case, "multiphase.initial.normal", [0.0; 3])) <= 0.0
            {
                out.push(d("multiphase.initial.normal", "the normal must be non-zero"));
            }
        }
    }
    for (k, shapes) in [
        ("box_min", "box"),
        ("box_max", "box"),
        ("centre", "sphere"),
        ("normal", "half_space"),
        ("point", "half_space"),
    ] {
        let p = format!("multiphase.initial.{k}");
        if lst(&p) != 0 && shape != shapes {
            out.push(d(p, format!("only used with initial.shape = \"{shapes}\"")));
        }
    }
    if shape != "sphere" && get_f(case, "multiphase.initial.radius") != UNSET {
        out.push(d(
            "multiphase.initial.radius",
            "only used with initial.shape = \"sphere\"",
        ));
    }
    check_extensions(case, &mut out);
    // solver
    let alg = get_s(case, "solver.algorithm");
    if !matches!(alg.as_str(), "piso" | "pimple") {
        out.push(d(
            "solver.algorithm",
            format!(
                "the free-surface solver is transient: set solver.algorithm = \"piso\" or \"pimple\" (coupled, compressible and the steady algorithms are not supported), got \"{alg}\""
            ),
        ));
    }
    if get_s(case, "fluid.density") != "constant" {
        out.push(d(
            "fluid.density",
            "multiphase.enabled uses the phase densities multiphase.density_1/2; set fluid.density = \"constant\"",
        ));
    }
    let mut unsupported = |present: bool, path: &str, what: &str| {
        if present {
            out.push(d(
                path,
                format!(
                    "{what} is not supported with multiphase.enabled = true (laminar isothermal two-phase flow only)"
                ),
            ));
        }
    };
    unsupported(crate::turb::enabled(case), "turbulence.model", "a turbulence model");
    unsupported(get_b(case, "energy.enabled"), "energy.enabled", "the energy equation");
    unsupported(
        crate::rheology::enabled(case),
        "rheology.model",
        "non-Newtonian rheology",
    );
    unsupported(
        crate::properties::enabled(case),
        "properties",
        "temperature-dependent properties",
    );
    unsupported(crate::scalars::enabled(case), "scalars", "passive scalars and species");
    unsupported(crate::sources::enabled(case), "sources", "momentum sources");
    unsupported(crate::lumped::enabled(case), "components", "lumped components");
    unsupported(crate::radiation::enabled(case), "radiation.model", "radiation");
    unsupported(crate::stats::enabled(case), "statistics.enabled", "run-time statistics");
    unsupported(crate::dynmesh::enabled(case), "dynamic_mesh.enabled", "a moving mesh");
    unsupported(
        crate::particles::enabled(case),
        "particles.enabled",
        "particle tracking",
    );
    unsupported(crate::chemistry::enabled(case), "chemistry", "chemistry");
    unsupported(crate::electric::enabled(case), "electric.enabled", "the electric field");
    unsupported(
        crate::phase_change::enabled(case),
        "phase_change.enabled",
        "melting and solidification",
    );
    unsupported(crate::cht::enabled(case), "solids", "conjugate heat transfer");
    unsupported(crate::zone_sources::enabled(case), "zone_sources", "zone sources");
    unsupported(
        get_s(case, "solver.time_advance") == "fractional_step",
        "solver.time_advance",
        "the fractional-step projection",
    );
    unsupported(
        get_f(case, "solver.max_courant") > 0.0,
        "solver.max_courant",
        "adaptive time stepping (use a fixed solver.dt; set solver.max_courant = 0)",
    );
    unsupported(
        get_s(case, "solver.time_scheme") != "euler",
        "solver.time_scheme",
        "a time scheme other than euler",
    );
    unsupported(
        get_s(case, "solver.viscous_stress") == "full",
        "solver.viscous_stress",
        "the full viscous stress",
    );
    unsupported(
        get_f(case, "solver.bulk_velocity") != 0.0,
        "solver.bulk_velocity",
        "a bulk-velocity driver",
    );
    unsupported(
        get_b(case, "solver.frozen_flow"),
        "solver.frozen_flow",
        "a frozen flow field",
    );
    unsupported(
        vec3::norm(get_v(case, "physics.body_force", [0.0; 3])) > 0.0,
        "physics.body_force",
        "physics.body_force (use physics.gravity)",
    );
    for f in ["mesh.periodic_x", "mesh.periodic_y"] {
        unsupported(get_b(case, f), f, "periodic meshes");
    }
    if (get_f(case, "physics.rho") - 1.0).abs() > 1e-300 {
        out.push(d(
            "physics.rho",
            "ignored with multiphase.enabled (the phase densities are multiphase.density_1/2); remove it",
        ));
    }
    if (get_f(case, "physics.nu") - 0.01).abs() > 1e-300 {
        out.push(d(
            "physics.nu",
            "ignored with multiphase.enabled (the phase viscosities are multiphase.viscosity_1/2); remove it",
        ));
    }
    for n in case.tree.names("boundaries").unwrap_or_default() {
        let b = format!("boundaries.{n}");
        let v = get_s(case, &format!("{b}.velocity"));
        if !matches!(v.as_str(), "fixed" | "zero_gradient") {
            out.push(d(
                format!("{b}.velocity"),
                format!("velocity \"{v}\" (symmetry, partial_slip, marangoni, freestream, moving_wall, shear) is not supported with multiphase.enabled; use fixed (no-slip walls: value 0) or zero_gradient"),
            ));
        }
        let p = get_s(case, &format!("{b}.pressure"));
        if !matches!(p.as_str(), "fixed" | "zero_gradient") {
            out.push(d(
                format!("{b}.pressure"),
                format!("pressure \"{p}\" is not supported with multiphase.enabled; use fixed (a value of p_rgh [Pa]) or zero_gradient"),
            ));
        }
        if crate::bc_cat::kind(case, &n) != "none" {
            out.push(d(
                b.clone(),
                "catalogue boundary conditions are not supported with multiphase.enabled",
            ));
        }
    }
    out
}

/// Keys of `mass_transfer` and `waves` that differ from their defaults while the model is off.
fn extension_inert(case: &Case, out: &mut Vec<Diagnostic>) {
    let nonempty = |p: &str| {
        case.tree
            .get(p)
            .ok()
            .and_then(|v| v.as_array().map(|a| !a.is_empty()))
            .unwrap_or(false)
    };
    if !enabled(case) && nonempty("multiphase.reference_point") {
        out.push(d(
            "multiphase.reference_point",
            "only used with multiphase.enabled = true",
        ));
    }
    let model = get_s(case, "multiphase.mass_transfer.model");
    let why = if model == "none" || !enabled(case) {
        Some("only used with multiphase.mass_transfer.model != \"none\" (and multiphase.enabled = true)")
    } else {
        None
    };
    if let Some(why) = why {
        for k in [
            "saturation_temperature",
            "latent_heat",
            "temperature",
            "molar_mass",
            "vapour_pressure",
            "saturation_pressure",
        ] {
            let p = format!("multiphase.mass_transfer.{k}");
            if get_f(case, &p) != UNSET {
                out.push(d(p, why));
            }
        }
        for k in ["evaporation_frequency", "condensation_frequency"] {
            let p = format!("multiphase.mass_transfer.{k}");
            if get_f(case, &p) != 0.0 {
                out.push(d(p, why));
            }
        }
        for k in ["temperature_gradient", "temperature_point"] {
            let p = format!("multiphase.mass_transfer.{k}");
            if nonempty(&p) {
                out.push(d(p, why));
            }
        }
        if get_f(case, "multiphase.mass_transfer.accommodation") != 1.0 {
            out.push(d("multiphase.mass_transfer.accommodation", why));
        }
        if get_b(case, "multiphase.mass_transfer.schrage") {
            out.push(d("multiphase.mass_transfer.schrage", why));
        }
        if get_s(case, "multiphase.mass_transfer.liquid_phase") != "phase_1" {
            out.push(d("multiphase.mass_transfer.liquid_phase", why));
        }
    }
    let inlet = !get_s(case, "multiphase.waves.inlet_patch").is_empty();
    let beach = get_s(case, "multiphase.waves.beach") != "none";
    let on = enabled(case);
    let why_w =
        "only used with multiphase.enabled = true and multiphase.waves.inlet_patch set (or beach = \"still_water\")";
    if !(on && (inlet || beach)) {
        for k in ["height", "period", "depth", "level"] {
            let p = format!("multiphase.waves.{k}");
            if get_f(case, &p) != UNSET {
                out.push(d(p, why_w));
            }
        }
        for (k, def) in [
            ("phase", 0.0),
            ("ramp_periods", 2.0),
            ("beach_x_inner", 0.0),
            ("beach_x_outer", 0.0),
        ] {
            let p = format!("multiphase.waves.{k}");
            if get_f(case, &p) != def {
                out.push(d(p, why_w));
            }
        }
        if get_i(case, "multiphase.waves.order") != 1 {
            out.push(d("multiphase.waves.order", why_w));
        }
        if get_s(case, "multiphase.waves.beach_blend") != "exponential" {
            out.push(d("multiphase.waves.beach_blend", why_w));
        }
        if get_f(case, "multiphase.waves.beach_exponent") != 3.5 {
            out.push(d("multiphase.waves.beach_exponent", why_w));
        }
        if !on && inlet {
            out.push(d("multiphase.waves.inlet_patch", why_w));
        }
        if !on && beach {
            out.push(d("multiphase.waves.beach", why_w));
        }
    }
}

/// Settings checks of the mass-transfer and wave sections (multiphase enabled).
fn check_extensions(case: &Case, out: &mut Vec<Diagnostic>) {
    let nonempty_len = |p: &str| {
        case.tree
            .get(p)
            .ok()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0)
    };
    let rp = nonempty_len("multiphase.reference_point");
    if rp != 0 && rp != 3 {
        out.push(d("multiphase.reference_point", "must be [x, y, z] or empty"));
    }
    let model = get_s(case, "multiphase.mass_transfer.model");
    extension_inert(case, out);
    if model != "none" {
        for k in ["saturation_temperature", "latent_heat", "temperature"] {
            let p = format!("multiphase.mass_transfer.{k}");
            if get_f(case, &p) <= 0.0 {
                out.push(d(
                    p,
                    format!("mass_transfer.model = \"{model}\" needs a positive {k} (-1 = unset)"),
                ));
            }
        }
        for k in ["temperature_gradient", "temperature_point"] {
            let p = format!("multiphase.mass_transfer.{k}");
            let n = nonempty_len(&p);
            if n != 0 && n != 3 {
                out.push(d(p, format!("{k} must be [x, y, z] or empty")));
            }
        }
        if model == "lee" {
            for k in ["molar_mass", "vapour_pressure", "saturation_pressure"] {
                let p = format!("multiphase.mass_transfer.{k}");
                if get_f(case, &p) != UNSET {
                    out.push(d(p, "only used with mass_transfer.model = \"hertz_knudsen\""));
                }
            }
            if get_b(case, "multiphase.mass_transfer.schrage") {
                out.push(d(
                    "multiphase.mass_transfer.schrage",
                    "only used with mass_transfer.model = \"hertz_knudsen\"",
                ));
            }
            if get_f(case, "multiphase.mass_transfer.accommodation") != 1.0 {
                out.push(d(
                    "multiphase.mass_transfer.accommodation",
                    "only used with mass_transfer.model = \"hertz_knudsen\"",
                ));
            }
        } else {
            for k in ["evaporation_frequency", "condensation_frequency"] {
                let p = format!("multiphase.mass_transfer.{k}");
                if get_f(case, &p) != 0.0 {
                    out.push(d(p, "only used with mass_transfer.model = \"lee\""));
                }
            }
            if get_f(case, "multiphase.mass_transfer.molar_mass") <= 0.0 {
                out.push(d(
                    "multiphase.mass_transfer.molar_mass",
                    "hertz_knudsen needs a positive molar_mass [kg/mol]",
                ));
            }
            if get_f(case, "multiphase.mass_transfer.vapour_pressure") < 0.0 {
                out.push(d(
                    "multiphase.mass_transfer.vapour_pressure",
                    "hertz_knudsen needs vapour_pressure >= 0 (-1 = unset)",
                ));
            }
            if get_f(case, "multiphase.mass_transfer.saturation_pressure") <= 0.0 {
                out.push(d(
                    "multiphase.mass_transfer.saturation_pressure",
                    "hertz_knudsen needs a positive saturation_pressure [Pa] at the saturation temperature",
                ));
            }
            if get_f(case, "multiphase.mass_transfer.accommodation") <= 0.0 {
                out.push(d("multiphase.mass_transfer.accommodation", "must be in (0, 1]"));
            }
        }
        let has_pressure_patch = case
            .tree
            .names("boundaries")
            .unwrap_or_default()
            .iter()
            .any(|n| get_s(case, &format!("boundaries.{n}.pressure")) == "fixed");
        if !has_pressure_patch {
            out.push(d(
                "multiphase.mass_transfer.model",
                "mass transfer creates volume: at least one boundary needs pressure = \"fixed\" for it to leave through",
            ));
        }
    }
    // waves
    let inlet = get_s(case, "multiphase.waves.inlet_patch");
    let beach = get_s(case, "multiphase.waves.beach") != "none";
    if inlet.is_empty() && !beach {
        return;
    }
    let wp = |k: &str| format!("multiphase.waves.{k}");
    if !inlet.is_empty() {
        for k in ["height", "period", "depth"] {
            if get_f(case, &wp(k)) <= 0.0 {
                out.push(d(wp(k), format!("waves.inlet_patch needs a positive {k} (-1 = unset)")));
            }
        }
        let b = format!("boundaries.{inlet}");
        if case
            .tree
            .names("boundaries")
            .unwrap_or_default()
            .iter()
            .all(|n| *n != inlet)
        {
            out.push(d(wp("inlet_patch"), format!("no boundary named \"{inlet}\"")));
        } else if get_s(case, &format!("{b}.velocity")) != "fixed" {
            out.push(d(
                format!("{b}.velocity"),
                "the wave-inlet patch needs velocity = \"fixed\" (its value is replaced by the wave every step)",
            ));
        }
    } else {
        for k in ["height", "period", "depth"] {
            if get_f(case, &wp(k)) != UNSET && k != "depth" {
                out.push(d(wp(k), "only used with waves.inlet_patch set"));
            }
        }
    }
    if beach {
        let (xi, xo) = (get_f(case, &wp("beach_x_inner")), get_f(case, &wp("beach_x_outer")));
        if xi == xo {
            out.push(d(wp("beach_x_outer"), "the beach needs beach_x_inner != beach_x_outer"));
        }
        if get_f(case, &wp("depth")) <= 0.0 && get_f(case, &wp("level")) <= 0.0 {
            out.push(d(
                wp("level"),
                "the beach needs the still-water level: set waves.level (or waves.depth with the bed at y = 0)",
            ));
        }
    }
    let g = get_v(case, "physics.gravity", [0.0; 3]);
    if !(g[1] < 0.0 && g[0].abs() <= 1e-12 * g[1].abs() && g[2].abs() <= 1e-12 * g[1].abs()) {
        out.push(d(
            "physics.gravity",
            "waves need gravity along -y (propagation along +x, 2-D)".to_string(),
        ));
    }
}

/// Runtime state of a free-surface simulation.
pub(crate) struct VofState {
    /// Solver.
    pub solver: VofSolver,
    /// Time steps taken.
    pub steps: usize,
}

impl VofState {
    /// Build the solver on `geom` from the velocity and `p_rgh` fields that carry the boundary conditions of the generic setup.
    pub fn new(
        case: &Case,
        geom: &Arc<FvGeometry>,
        u: VectorField,
        p_rgh: ScalarField,
    ) -> Result<Self, Vec<Diagnostic>> {
        let v3 = |k: &str| get_v(case, &format!("multiphase.initial.{k}"), [0.0; 3]);
        let shape = match get_s(case, "multiphase.initial.shape").as_str() {
            "box" => Shape::Box {
                min: v3("box_min"),
                max: v3("box_max"),
            },
            "sphere" => Shape::Sphere {
                centre: v3("centre"),
                radius: get_f(case, "multiphase.initial.radius"),
            },
            _ => {
                let n = v3("normal");
                let nn = vec3::norm(n);
                let nh = vec3::scale(n, 1.0 / nn);
                Shape::HalfSpace {
                    normal: nh,
                    offset: vec3::dot(nh, v3("point")),
                }
            }
        };
        let invert = get_s(case, "multiphase.initial.inside") == "phase_2";
        let mut alpha = ScalarField::uniform("alpha", geom, 0.0);
        alpha.values = shape_fraction(geom, shape, invert, 10);
        let mut s = VofSettings::new(
            [get_f(case, "multiphase.density_1"), get_f(case, "multiphase.density_2")],
            [
                get_f(case, "multiphase.viscosity_1"),
                get_f(case, "multiphase.viscosity_2"),
            ],
        );
        s.sigma = get_f(case, "multiphase.surface_tension");
        s.x_ref = get_v(case, "multiphase.reference_point", [0.0; 3]);
        s.gravity = get_v(case, "physics.gravity", [0.0; 3]);
        s.c_alpha = get_f(case, "multiphase.compression");
        s.alpha_courant = get_f(case, "multiphase.alpha_courant");
        s.n_smooth = get_i(case, "multiphase.curvature_smoothing").max(0) as usize;
        s.convection = crate::sim::solver_conv_scheme(case);
        s.n_outer = if get_s(case, "solver.algorithm") == "pimple" {
            get_i(case, "solver.n_outer").max(1) as usize
        } else {
            1
        };
        s.n_correctors = get_i(case, "solver.n_correctors").max(1) as usize;
        s.non_orth_correctors = get_i(case, "solver.non_orthogonal_correctors").max(0) as usize;
        s.alpha_u = get_f(case, "solver.alpha_u");
        s.alpha_p = get_f(case, "solver.alpha_p");
        s.ddt_correction = get_b(case, "solver.ddt_correction");
        s.p_ref = (
            get_i(case, "solver.p_ref_cell").max(0) as usize,
            get_f(case, "solver.p_ref_value"),
        );
        // AMG as in the single-phase transient path (ADR 0052 A: the NaN hierarchy of the 1000:1 density-jump pressure matrix was a
        // zero filtered diagonal in the AMG setup, fixed in rcfd-linalg; the 120 x 40 dam break passes with AMG)
        s.amg_pressure = geom.n_cells() > 2000;
        let (mt, mi) = (
            get_f(case, "solver.momentum_rel_tol"),
            get_i(case, "solver.momentum_max_iter"),
        );
        let (pt, pi) = (
            get_f(case, "solver.pressure_rel_tol"),
            get_i(case, "solver.pressure_max_iter"),
        );
        if mt > 0.0 {
            s.momentum_solver.tol_rel = mt;
        }
        if mi > 0 {
            s.momentum_solver.max_iter = mi as usize;
        }
        if pt > 0.0 {
            s.pressure_solver.tol_rel = pt;
        }
        let pa = get_f(case, "solver.pressure_abs_tol");
        if pa > 0.0 {
            s.pressure_solver.tol_abs = pa;
        }
        if pi > 0 {
            s.pressure_solver.max_iter = pi as usize;
        }
        s.linear = crate::linsolver::options(case);
        let wp = |k: &str| format!("multiphase.waves.{k}");
        let mp = |k: &str| format!("multiphase.mass_transfer.{k}");
        let model = get_s(case, &mp("model"));
        if model != "none" {
            let pt = get_v(case, &mp("temperature_point"), [0.0; 3]);
            let gr = get_v(case, &mp("temperature_gradient"), [0.0; 3]);
            let t0 = get_f(case, &mp("temperature"));
            let temperature: Vec<f64> = geom
                .mesh
                .cell_centres()
                .iter()
                .map(|x| t0 + vec3::dot(gr, vec3::sub(*x, pt)))
                .collect();
            s.mass_transfer = Some(MassTransfer {
                liquid_phase: if get_s(case, &mp("liquid_phase")) == "phase_2" {
                    2
                } else {
                    1
                },
                model: if model == "lee" {
                    MassTransferModel::Lee {
                        c_evap: get_f(case, &mp("evaporation_frequency")),
                        c_cond: get_f(case, &mp("condensation_frequency")),
                    }
                } else {
                    MassTransferModel::HertzKnudsen {
                        accommodation: get_f(case, &mp("accommodation")),
                        molar_mass: get_f(case, &mp("molar_mass")),
                        schrage: get_b(case, &mp("schrage")),
                        p_vapour: get_f(case, &mp("vapour_pressure")),
                        p_sat_ref: get_f(case, &mp("saturation_pressure")),
                    }
                },
                t_sat: get_f(case, &mp("saturation_temperature")),
                h_fg: get_f(case, &mp("latent_heat")),
                temperature,
            });
        }
        let inlet_patch = get_s(case, &wp("inlet_patch"));
        let beach = get_s(case, &wp("beach")) != "none";
        if !inlet_patch.is_empty() || beach {
            let depth = get_f(case, &wp("depth"));
            let level = if get_f(case, &wp("level")) != UNSET {
                get_f(case, &wp("level"))
            } else {
                depth
            };
            let gmag = vec3::norm(s.gravity);
            let mk_wave = || -> Result<Wave, Vec<Diagnostic>> {
                let period = get_f(case, &wp("period"));
                Wave::new(
                    0.5 * get_f(case, &wp("height")),
                    2.0 * std::f64::consts::PI / period,
                    depth,
                    gmag,
                    get_f(case, &wp("phase")),
                    get_i(case, &wp("order")).clamp(1, 2) as u8,
                )
                .map_err(|e| vec![d("multiphase.waves", e)])
            };
            let inlet = if inlet_patch.is_empty() {
                None
            } else {
                Some(WaveInlet {
                    patch: inlet_patch,
                    wave: mk_wave()?,
                    t_ramp: get_f(case, &wp("ramp_periods")) * get_f(case, &wp("period")),
                })
            };
            let exponent = get_f(case, &wp("beach_exponent"));
            let zones = if beach {
                vec![RelaxZone {
                    x_inner: get_f(case, &wp("beach_x_inner")),
                    x_outer: get_f(case, &wp("beach_x_outer")),
                    blend: if get_s(case, &wp("beach_blend")) == "polynomial" {
                        Blend::Polynomial { n: exponent }
                    } else {
                        Blend::Exponential { p: exponent }
                    },
                    target: RelaxTarget::StillWater,
                }]
            } else {
                vec![]
            };
            s.waves = Some(WaveSettings { level, inlet, zones });
        }
        let solver =
            VofSolver::new(geom.clone(), u, p_rgh, alpha, s).map_err(|e| vec![d("multiphase", e.to_string())])?;
        Ok(VofState { solver, steps: 0 })
    }

    /// Named cell field (`alpha`, `p_rgh`, `p`, `rho`) of the free-surface solver.
    pub fn field(&self, name: &str) -> Option<Vec<f64>> {
        Some(match name {
            "alpha" => self.solver.transport.alpha.values.clone(),
            "p_rgh" => self.solver.p_rgh.values.clone(),
            "p" => self.solver.p.clone(),
            "rho" => self.solver.density(),
            _ => return None,
        })
    }
}
