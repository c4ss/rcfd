//! Lagrangian particle tracking in the settings tree (F-056, ADR 0028 D9).
//!
//! The numerics live in `rcfd-particles`; this module reads and checks the `particles` section, builds the tracker on the
//! simulation mesh and runs it on a frozen (converged or prescribed) velocity field or alongside a transient flow. The flow
//! is never modified (one-way coupling). Every numeric key has an explicit unset sentinel (`-1`) with a located error when it
//! is needed and unset.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::case::{Case, Diagnostic};
use crate::sim::Simulation;
use rcfd_core::settings::Schema;
use rcfd_core::vec3::Vec3;
use rcfd_particles::vtp::write_vtp;
use rcfd_particles::{
    ConeSpec, Dispersion, DragLaw, ErosionSpec, FilmGas, Flow, InjKind, InjectorExt, InjectorSpec, Interp, Options,
    RoughSpec, Scheme, SizeDist, Tracker, VelMode, WallAction,
};

/// Sentinel of the numeric keys that are not set.
pub const UNSET: f64 = -1.0;

fn d(path: impl Into<String>, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn gf(c: &Case, p: &str) -> f64 {
    c.tree.get(p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn gs(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn gb(c: &Case, p: &str) -> bool {
    c.tree.get(p).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

fn gi(c: &Case, p: &str) -> i64 {
    c.tree.get(p).ok().and_then(|v| v.as_i64()).unwrap_or(0)
}

/// A list setting: `None` when the list is empty, `Err(len)` for a wrong length.
fn vec3_of(c: &Case, p: &str) -> std::result::Result<Option<Vec3>, usize> {
    let v = c.tree.get(p).ok();
    let a = v.as_ref().and_then(|v| v.as_array()).cloned().unwrap_or_default();
    match a.len() {
        0 => Ok(None),
        3 => Ok(Some([
            a[0].as_f64().unwrap_or(f64::NAN),
            a[1].as_f64().unwrap_or(f64::NAN),
            a[2].as_f64().unwrap_or(f64::NAN),
        ])),
        n => Err(n),
    }
}

/// Is the particle section enabled?
pub fn enabled(case: &Case) -> bool {
    gb(case, "particles.enabled")
}

/// Names of the configured injections.
pub fn injection_names(case: &Case) -> Vec<String> {
    case.tree.names("particles.injection").unwrap_or_default()
}

/// The value of `particles.erosion.material` that names no preset.
pub const NO_PRESET: &str = "none";

/// Schema of `particles.erosion.material` (ADR 0197 P1): `"none"` or the name of a preset of
/// [`rcfd_particles::EROSION_PRESETS`]; left out of the settings hash at its default.
fn erosion_material_schema() -> Schema {
    let mut names = vec![NO_PRESET];
    names.extend(rcfd_particles::EROSION_PRESETS.iter().map(|p| p.name));
    let rows: Vec<String> = rcfd_particles::EROSION_PRESETS
        .iter()
        .map(|p| {
            format!(
                "{} = the {} of {} (K {:?}, n {:?}, density {:?})",
                p.name, p.row, p.source, p.k, p.n, p.density
            )
        })
        .collect();
    let doc = format!(
        "Preset of k, n and density (ADR 0197): none = no preset (k, n and density must be set); {}. With a preset, k, n and density written with a value other than -1 override it, and k and n are overridden together or not at all; the report and particle_erosion.csv record the preset and the values used. {}",
        rows.join("; "),
        rcfd_particles::PRESET_PROVENANCE
    );
    Schema::enumeration(NO_PRESET, &names).doc(&doc).hash_omit_default()
}

/// Schema of the `particles` section.
pub fn schema() -> Schema {
    let injection = Schema::named(
        Schema::object()
            .doc("One particle injection (ADR 0028 D7)")
            .field(
                "kind",
                Schema::enumeration("unset", &["unset", "point", "surface"])
                    .doc("point: all parcels at `position`; surface: uniformly over the faces of the boundary patch `patch`. \"unset\" is an error"),
            )
            .field("position", Schema::list(Schema::float(0.0)).doc("kind = point: position [x, y, z] in metres (3 components)"))
            .field("patch", Schema::string("").doc("kind = surface: name of the boundary patch"))
            .field(
                "rate",
                Schema::float(UNSET).doc("Parcels per second; must be set (> 0), -1 = unset. The number injected up to time t is floor(rate (t - start_time))"),
            )
            .field("start_time", Schema::float(0.0).at_least(0.0).doc("Start of injection [s]"))
            .field(
                "end_time",
                Schema::float(UNSET).doc("End of injection [s], inclusive; -1 = particles.end_time"),
            )
            .field(
                "velocity_mode",
                Schema::enumeration("given", &["given", "fluid"])
                    .doc("given: initial velocity `velocity`; fluid: the local fluid velocity"),
            )
            .field(
                "velocity",
                Schema::list(Schema::float(0.0)).doc("velocity_mode = given: initial velocity [m/s] (3 components; empty list = at rest)"),
            )
            .field("parcel_count", Schema::float(1.0).doc("Real particles represented by one parcel (> 0)"))
            .field(
                "distribution",
                Schema::enumeration("constant", &["constant", "rosin_rammler", "uniform"])
                    .doc("Diameter distribution: constant (`diameter`), Rosin-Rammler (oversize mass fraction exp(-(d/rr_d_bar)^rr_n)) or uniform in [d_min, d_max] (both required)"),
            )
            .field("diameter", Schema::float(UNSET).doc("distribution = constant: diameter [m] (> 0), -1 = unset"))
            .field("rr_d_bar", Schema::float(UNSET).doc("Rosin-Rammler size constant [m] (> 0), -1 = unset"))
            .field("rr_n", Schema::float(UNSET).doc("Rosin-Rammler spread parameter (> 0), -1 = unset"))
            .field("d_min", Schema::float(UNSET).doc("Rosin-Rammler lower truncation [m]; -1 = none"))
            .field("d_max", Schema::float(UNSET).doc("Rosin-Rammler upper truncation [m]; -1 = none"))
            .field(
                "mass_flow",
                Schema::float(UNSET).doc("Mass flow [kg/s] of the injection (ADR 0057): every parcel carries mass_flow/rate and the parcel weight (real particles per parcel) follows from its diameter, overriding parcel_count; -1 = off (parcel_count)"),
            )
            .field(
                "cone_half_angle",
                Schema::float(UNSET).doc("Cone injection: half angle [deg] in (0, 180]; directions uniform in solid angle about cone_axis with speed cone_speed (overrides velocity_mode); -1 = no cone"),
            )
            .field("cone_inner_half_angle", Schema::float(0.0).doc("Hollow cone: inner half angle [deg] in [0, cone_half_angle)"))
            .field("cone_axis", Schema::list(Schema::float(0.0)).doc("Cone axis [x, y, z] (3 components, non-zero)"))
            .field("cone_speed", Schema::float(UNSET).doc("Cone injection speed [m/s] (> 0); -1 = unset"))
            .field(
                "temperature",
                Schema::float(UNSET).doc("Parcel temperature at injection [K] (ADR 0059); required (> 0) when particles.heat_transfer, -1 = unset"),
            ),
    )
    .doc("Particle injections by name");
    let walls = Schema::named(
        Schema::object()
            .doc("Particle action on one boundary patch")
            .field(
                "interaction",
                Schema::enumeration("default", &["default", "reflect", "stick", "escape"])
                    .doc("default: reflect on walls (particles.wall_interaction), escape at inlets, outlets and generic patches, specular reflection on symmetry planes"),
            )
            .field(
                "restitution",
                Schema::float(UNSET).doc("Restitution of a reflecting patch in [0, 1]; -1 = particles.restitution"),
            ),
    )
    .doc("Per-patch particle boundary actions");
    let film = Schema::object()
        .optional()
        .doc("Wall film fed by the parcels (ADR 0192): a parcel that reaches a face of a film patch is absorbed into a thin laminar film on that face; the film runs down the wall under gravity with the depth-averaged velocity rho h^2 g_t/(3 mu) of a steady laminar film of its own thickness (gas buoyancy and gas shear only with gas_buoyancy and gas_shear, ADR 0206; no splash, no evaporation) and leaves the film where no other film face continues it. The film liquid density is particles.density; the film never acts on the flow. Absent = no film")
        .field("enabled", Schema::boolean(false).doc("Absorb the parcels that reach the film patches into a wall film"))
        .field(
            "patches",
            Schema::list(Schema::string("")).doc("Wall patches that carry the film (required, each once, without a particles.walls entry)"),
        )
        .field(
            "viscosity",
            Schema::float(UNSET).doc("Dynamic viscosity of the film liquid [Pa s] (> 0); -1 = unset (an error when the film is enabled)"),
        )
        .field(
            "gas_buoyancy",
            Schema::boolean(false)
                .doc("Gas buoyancy (ADR 0206 G1): drive the film by (rho_l - rho_g) g_t in place of rho_l g_t, with rho_g = physics.rho (the buoyancy the parcels have); needs physics.rho below particles.density")
                .hash_omit_default(),
        )
        .field(
            "gas_shear",
            Schema::boolean(false)
                .doc("Gas shear (ADR 0206 G2, G4): add h tau/(2 mu_l) to the film velocity, with tau = mu_g (u_P)_t / d the one-sided laminar wall stress of the carrier at each film face (u_P the velocity of the cell next to the face, d the normal distance of its centre). Laminar carriers only, the film patches no-slip walls at rest (velocity = \"fixed\" with velocity_value [0, 0, 0], no expression or udf), no moving mesh, no rotating frame. A film whose gravity and shear parts oppose (counter-current) is not delivered and is counted in the report")
                .hash_omit_default(),
        )
        .hash_omit_default();
    let erosion = Schema::object()
        .optional()
        .doc("Erosion of wall patches by the parcels (ADR 0194): every parcel hit on an erosion patch erodes the mass m K U^n F(alpha) of DNV RP O501 eq. 7.1 to 7.3 (Revision 4.2-2007), with m the parcel mass, U the impact speed, alpha the angle between the path and the wall and F the angle function DNV gives for steel grades; booked before the wall action, whatever it is, and never changing a parcel. Reported per face over the tracked time (particle_erosion.csv). Absent = no erosion")
        .field("enabled", Schema::boolean(false).doc("Book the erosion of the parcel hits on the erosion patches"))
        .field(
            "patches",
            Schema::list(Schema::string("")).doc("Wall patches where the hits are booked (required, each once, not a film patch)"),
        )
        .field(
            "k",
            Schema::float(UNSET).doc("K of DNV eq. 7.1 [(m/s)^-n] (> 0; DNV Table 7-2 recommends 2.0e-9 for steel grades); -1 = unset (an error when erosion is enabled, unless material names a preset)"),
        )
        .field(
            "n",
            Schema::float(UNSET).doc("Velocity exponent n of DNV eq. 7.1 (> 0; DNV Table 7-2 recommends 2.6 for steel grades); -1 = unset (the preset value when material names one)"),
        )
        .field(
            "density",
            Schema::float(UNSET).doc("Density of the wall material [kg/m^3] for the depth rate (> 0); -1 = unset (the preset value when material names one)"),
        )
        .field("material", erosion_material_schema())
        .hash_omit_default();
    let dispersion_from_model = Schema::object()
        .optional()
        .doc("dispersion = drw on the turbulence model's fields (ADR 0208): k and epsilon per cell of turbulence.model = k_epsilon (its epsilon) or k_omega_sst (epsilon = 0.09 k omega), in place of dispersion_k and dispersion_epsilon. Read when the tracker is built for steady and frozen tracking, and before every tracking step of a transient run (the fields at the end of the flow step; the old time level with two_way). Absent = the constants")
        .field(
            "enabled",
            Schema::boolean(false)
                .doc("Take k and epsilon from the turbulence model (needs dispersion = drw, turbulence.model = k_epsilon or k_omega_sst, and dispersion_k, dispersion_epsilon left at -1)")
                .hash_omit_default(),
        )
        .hash_omit_default();
    let rough_wall = Schema::object()
        .optional()
        .doc("Rough reflecting walls (ADR 0194): a parcel that hits a rough patch reflects, with the patch restitution, about a virtual wall whose normal is tilted in the plane of incidence by a Gaussian roughness angle of standard deviation angle_std (the virtual-wall model of Sommerfeld 1992 as restated in the literature); a draw that would let the parcel reach the virtual wall from behind or rebound into the wall is drawn again. No friction, no particle rotation. Absent = smooth walls")
        .field("enabled", Schema::boolean(false).doc("Make the listed reflecting patches rough"))
        .field(
            "patches",
            Schema::list(Schema::string("")).doc("Reflecting wall patches that are rough (required, each once, restitution above 0, not a film patch)"),
        )
        .field(
            "angle_std",
            Schema::float(UNSET).doc("Standard deviation of the roughness angle [degrees], in (0, 30]; -1 = unset (an error when the rough wall is enabled)"),
        )
        .field("seed", Schema::int(1).at_least(0.0).doc("Seed of the roughness random numbers; a run is reproducible bit for bit"))
        .hash_omit_default();
    Schema::object()
        .doc("One-way coupled Lagrangian particle tracking (ADR 0028). Point particles with drag, gravity and buoyancy on a frozen or transient flow field; no effect on the flow")
        .field("enabled", Schema::boolean(false).doc("Enable the particle section"))
        .field("density", Schema::float(UNSET).doc("Particle density [kg/m^3] (> 0); -1 = unset (an error when enabled)"))
        .field(
            "drag",
            Schema::enumeration("schiller_naumann", &["schiller_naumann", "stokes", "none"])
                .doc("Drag law (Schiller-Naumann: 24/Re (1 + 0.15 Re^0.687) up to Re = 1000, 0.44 above; the switch at 1000 is unverified)"),
        )
        .field(
            "integration",
            Schema::enumeration("analytic", &["analytic", "euler", "rk2", "rk4"])
                .doc("analytic: exponential relaxation with frozen fluid velocity (exact for Stokes drag in a uniform field); euler, rk2 (midpoint), rk4"),
        )
        .field(
            "interpolation",
            Schema::enumeration("gradient", &["gradient", "cell_centre"])
                .doc("Fluid velocity at the particle: cell value plus (x - x_c) . grad u (least squares), or the cell value"),
        )
        .field("gravity", Schema::boolean(true).doc("Apply physics.gravity (with buoyancy (1 - rho_f/rho_p))"))
        .field(
            "dt",
            Schema::float(UNSET).doc("Tracking step [s] for track_frozen (-1 = solver.dt, which must then be positive); transient tracking uses the flow step"),
        )
        .field("end_time", Schema::float(UNSET).doc("End of tracking [s]; must be set (> 0), -1 = unset"))
        .field("seed", Schema::int(1).at_least(0.0).doc("Seed of the random numbers (injection positions and diameters); a run is reproducible bit for bit"))
        .field(
            "wall_interaction",
            Schema::enumeration("reflect", &["reflect", "stick", "escape"]).doc("Action at wall patches without an entry in `walls`"),
        )
        .field("restitution", Schema::float(1.0).range(0.0, 1.0).doc("Restitution coefficient of reflecting walls"))
        .field(
            "dispersion",
            Schema::enumeration("none", &["none", "drw"]).doc("Turbulent dispersion (ADR 0057): none, or the discrete random walk with eddy lifetime T_L = dispersion_cl k/eps and eddy length C_mu^(3/4) k^(3/2)/eps in homogeneous isotropic turbulence of the given k and epsilon"),
        )
        .field("dispersion_k", Schema::float(UNSET).doc("dispersion = drw: turbulent kinetic energy [m^2/s^2] (> 0), -1 = unset"))
        .field("dispersion_epsilon", Schema::float(UNSET).doc("dispersion = drw: dissipation rate [m^2/s^3] (> 0), -1 = unset"))
        .field("dispersion_cl", Schema::float(0.15).doc("dispersion = drw: constant C_L of the Lagrangian integral time T_L = C_L k/epsilon (> 0)"))
        .field("dispersion_crossing", Schema::boolean(true).doc("dispersion = drw: limit the interaction time by the eddy crossing time"))
        .field("dispersion_seed", Schema::int(1).at_least(0.0).doc("dispersion = drw: seed of the dispersion random numbers"))
        .field("two_way", Schema::boolean(false).doc("Two-way coupling (ADR 0059): the momentum the parcels receive from the carrier is given back to the carrier momentum equation as a body acceleration impulse/(rho V T) (opposite sign to the parcel gain); transient: particles first on the field of the old time level, source constant over the step; steady: fixed-point iteration with a fresh tracker, source normalised by the injection window; constant-density segregated algorithms only"))
        .field("two_way_relax", Schema::float(0.5).doc("Steady two-way: under-relaxation beta of the source, S <- S + beta (S_new - S), 0 < beta <= 1"))
        .field("two_way_every", Schema::int(1).at_least(1.0).doc("Steady two-way: re-track every this many iterations (>= 1)"))
        .field("two_way_start", Schema::int(0).at_least(0.0).doc("Steady two-way: first iteration (from 0) at which the coupling is updated"))
        .field("two_way_tolerance", Schema::float(1e-6).doc("Steady two-way: the run counts as converged only if the relative change of the coupling source is below this (> 0)"))
        .field("heat_transfer", Schema::boolean(false).doc("Ranz-Marshall lumped heat transfer between the parcels and the carrier (ADR 0059; needs energy.enabled, physics.cp, physics.conductivity, particle_cp and an injection temperature); with two_way the heat given by the parcels enters the temperature equation (the mechanical energy exchange and the drag dissipation are not fed in)"))
        .field("particle_cp", Schema::float(UNSET).doc("Particle specific heat [J/(kg K)] (> 0 when heat_transfer), -1 = unset"))
        .field("walls", walls)
        .field("injection", injection)
        .field("output_file", Schema::string("").doc("VTK PolyData (.vtp) file of the particles written at the end by the library hooks track_frozen / track_transient (relative to the working directory); the run path (rcfd run, service) writes particles.vtp into the output directory instead; empty = none"))
        .field(
            "output_every",
            Schema::int(0).at_least(0.0).doc("Also write <output_file stem>_<n>.vtp (run path: particles_<n>.vtp in the output directory) every n tracking steps (0 = only the final file)"),
        )
        .field(
            "tracks",
            Schema::boolean(false).doc("Run path: also write particle_tracks.csv (id,time,x,y,z,state of every parcel at injection, every track_every-th tracking step while active, and when it leaves)"),
        )
        .field(
            "track_every",
            Schema::int(1).at_least(1.0).doc("tracks = true: log a position every this many tracking steps"),
        )
        .field("film", film)
        .field("erosion", erosion)
        .field("rough_wall", rough_wall)
        .field("dispersion_from_model", dispersion_from_model)
}

/// Does the random walk take `k` and `epsilon` from the turbulence model (ADR 0208 D2, amendment 2)?
pub fn dispersion_from_model(case: &Case) -> bool {
    enabled(case) && gs(case, "particles.dispersion") == "drw" && gb(case, "particles.dispersion_from_model.enabled")
}

/// `k` and `epsilon` per cell of the turbulence state (ADR 0208 D2): the transported `epsilon` of k-epsilon, or
/// `epsilon = beta* k omega` of k-omega SST with the SST's `beta*`, as `chemistry::mixing_vectors` does.
pub(crate) fn model_k_epsilon(
    turb: Option<&crate::turb::TurbState>,
) -> std::result::Result<(Vec<f64>, Vec<f64>), String> {
    let t = turb.ok_or("dispersion_from_model needs a turbulence model (k_epsilon or k_omega_sst)")?;
    let k = t
        .scalar("k")
        .ok_or("dispersion_from_model needs a model that transports k (k_epsilon or k_omega_sst)")?
        .values
        .clone();
    if let Some(e) = t.scalar("epsilon") {
        return Ok((k, e.values.clone()));
    }
    let w = t
        .scalar("omega")
        .ok_or("dispersion_from_model needs epsilon or omega (k_epsilon or k_omega_sst)")?;
    let eps = k
        .iter()
        .zip(&w.values)
        .map(|(k, w)| rcfd_turb::sst::BETA_STAR * k * w)
        .collect();
    Ok((k, eps))
}

/// Is the wall film requested (ADR 0192)?
pub fn film(case: &Case) -> bool {
    enabled(case) && gb(case, "particles.film.enabled")
}

/// A patch list as written (non-strings read as empty names).
fn patch_list(case: &Case, key: &str) -> Vec<String> {
    case.tree
        .get(key)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect()
}

/// The film patch names as written (non-strings read as empty names).
fn film_patches(case: &Case) -> Vec<String> {
    patch_list(case, "particles.film.patches")
}

/// Is erosion requested (ADR 0194)?
pub fn erosion(case: &Case) -> bool {
    enabled(case) && gb(case, "particles.erosion.enabled")
}

/// Is the rough wall requested (ADR 0194)?
pub fn rough_wall(case: &Case) -> bool {
    enabled(case) && gb(case, "particles.rough_wall.enabled")
}

/// The names of a patch list with its located errors (empty list, empty or repeated name); the first occurrence of each
/// non-empty name is returned.
fn checked_patches(case: &Case, key: &str, what: &str, out: &mut Vec<Diagnostic>) -> Vec<String> {
    let names = patch_list(case, key);
    if names.is_empty() {
        out.push(d(key, format!("{what} needs at least one wall patch in {key}")));
    }
    let mut kept = Vec::new();
    for (i, n) in names.iter().enumerate() {
        if n.is_empty() {
            out.push(d(key, "a patch name is empty"));
        } else if names[..i].contains(n) {
            out.push(d(key, format!("patch '{n}' is listed twice")));
        } else {
            kept.push(n.clone());
        }
    }
    kept
}

/// Located errors for the patches of `key` that also carry an enabled wall film (ADR 0194).
fn check_not_film(case: &Case, key: &str, names: &[String], out: &mut Vec<Diagnostic>) {
    if !gb(case, "particles.film.enabled") {
        return;
    }
    let film = film_patches(case);
    for n in names {
        if film.contains(n) {
            out.push(d(
                key,
                format!("patch '{n}' carries the wall film; {key} is not available on a film patch (ADR 0194)"),
            ));
        }
    }
}

/// The constants of the erosion model as a case gives them (ADR 0194 S2, ADR 0197 P3).
#[derive(Debug, Clone, PartialEq)]
pub struct ErosionConstants {
    /// `K` [(m/s)^-n].
    pub k: f64,
    /// Velocity exponent.
    pub n: f64,
    /// Density of the wall material [kg/m^3].
    pub density: f64,
    /// The preset named by `particles.erosion.material`, if any.
    pub preset: Option<rcfd_particles::ErosionPreset>,
    /// For `k`, `n` and `density`: true when the value is the preset's (the case leaves the key at -1).
    pub from_preset: [bool; 3],
}

/// The keys of the erosion constants, in the order of [`ErosionConstants::from_preset`].
const EROSION_KEYS: [&str; 3] = ["k", "n", "density"];

/// The constants of the erosion model (ADR 0197 P3): with a preset, each key the case leaves at -1 takes the preset value
/// and a written value overrides it; without one, the values as written.
pub fn erosion_constants(case: &Case) -> ErosionConstants {
    let preset = rcfd_particles::erosion_preset(&gs(case, "particles.erosion.material")).copied();
    let written = EROSION_KEYS.map(|k| gf(case, &format!("particles.erosion.{k}")));
    let Some(p) = preset else {
        let [k, n, density] = written;
        return ErosionConstants {
            k,
            n,
            density,
            preset: None,
            from_preset: [false; 3],
        };
    };
    let from_preset = written.map(|x| x == UNSET);
    let pick = |i: usize, v: f64| if from_preset[i] { v } else { written[i] };
    ErosionConstants {
        k: pick(0, p.k),
        n: pick(1, p.n),
        density: pick(2, p.density),
        preset: Some(p),
        from_preset,
    }
}

/// Checks of the erosion keys (ADR 0194 S2, ADR 0197 P5), located; mesh-dependent ones are made by [`build_tracker`].
fn check_erosion(case: &Case, out: &mut Vec<Diagnostic>) {
    if !gb(case, "particles.erosion.enabled") {
        return;
    }
    let names = checked_patches(case, "particles.erosion.patches", "erosion", out);
    check_not_film(case, "particles.erosion.patches", &names, out);
    let what = [
        "K of DNV eq. 7.1 [(m/s)^-n]",
        "the velocity exponent n",
        "the density of the wall material [kg/m^3]",
    ];
    let c = erosion_constants(case);
    let Some(p) = c.preset else {
        for (k, what) in EROSION_KEYS.iter().zip(what) {
            if !(gf(case, &format!("particles.erosion.{k}")) > 0.0) {
                out.push(d(
                    format!("particles.erosion.{k}"),
                    format!("{what} must be set (> 0); -1 means unset"),
                ));
            }
        }
        return;
    };
    for (i, (k, what)) in EROSION_KEYS.iter().zip(what).enumerate() {
        let x = gf(case, &format!("particles.erosion.{k}"));
        if !c.from_preset[i] && !(x > 0.0) {
            out.push(d(
                format!("particles.erosion.{k}"),
                format!(
                    "{what} must be above 0, or -1 to take the value of the preset {} (ADR 0197)",
                    p.name
                ),
            ));
        }
    }
    if c.from_preset[0] != c.from_preset[1] {
        let (set, unset) = if c.from_preset[0] { ("n", "k") } else { ("k", "n") };
        out.push(d(
            format!("particles.erosion.{unset}"),
            format!(
                "particles.erosion.{set} is set and particles.erosion.{unset} is not: K [(m/s)^-n] and n are a fitted pair, so with the preset {} set both or neither (ADR 0197)",
                p.name
            ),
        ));
    }
}

/// The action a wall patch gets from `particles.walls`, `wall_interaction` and `restitution` as [`build_tracker`] sets it:
/// `Some(e)` for a reflection with restitution `e`, `None` for stick or escape.
fn wall_reflection(case: &Case, n: &str) -> Option<f64> {
    let walls = case.tree.names("particles.walls").unwrap_or_default();
    let global = || match gs(case, "particles.wall_interaction").as_str() {
        "stick" | "escape" => None,
        _ => Some(gf(case, "particles.restitution")),
    };
    if !walls.iter().any(|w| w == n) {
        return global();
    }
    let r = gf(case, &format!("particles.walls.{n}.restitution"));
    let e = if r == UNSET {
        gf(case, "particles.restitution")
    } else {
        r
    };
    match gs(case, &format!("particles.walls.{n}.interaction")).as_str() {
        "reflect" => Some(e),
        "stick" | "escape" => None,
        _ if r != UNSET => Some(r),
        _ => global(),
    }
}

/// Checks of the rough-wall keys (ADR 0194 S2), located; mesh-dependent ones are made by [`build_tracker`].
fn check_rough_wall(case: &Case, out: &mut Vec<Diagnostic>) {
    if !gb(case, "particles.rough_wall.enabled") {
        return;
    }
    let key = "particles.rough_wall.patches";
    let names = checked_patches(case, key, "the rough wall", out);
    check_not_film(case, key, &names, out);
    for n in &names {
        match wall_reflection(case, n) {
            None => out.push(d(
                key,
                format!("patch '{n}' does not reflect (particles.walls.{n}.interaction or particles.wall_interaction is stick or escape); a rough wall must reflect"),
            )),
            Some(e) if !(e > 0.0) => out.push(d(
                key,
                format!("patch '{n}' reflects with restitution {e}; a rough wall needs a restitution above 0"),
            )),
            Some(_) => {}
        }
    }
    let s = gf(case, "particles.rough_wall.angle_std");
    if !(s > 0.0 && s <= rcfd_particles::rough::MAX_ANGLE_STD_DEG) {
        out.push(d(
            "particles.rough_wall.angle_std",
            "the standard deviation of the roughness angle [degrees] must be set in (0, 30]; -1 means unset",
        ));
    }
}

/// Checks of the wall-film keys (ADR 0192 S2), located; mesh-dependent ones are made by [`build_tracker`].
fn check_film(case: &Case, out: &mut Vec<Diagnostic>) {
    if !gb(case, "particles.film.enabled") {
        return;
    }
    for k in ["two_way", "heat_transfer"] {
        if gb(case, &format!("particles.{k}")) {
            out.push(d(
                "particles.film.enabled",
                format!("the wall film is not available with particles.{k} (ADR 0192)"),
            ));
        }
    }
    let names = film_patches(case);
    let walls = case.tree.names("particles.walls").unwrap_or_default();
    if names.is_empty() {
        out.push(d(
            "particles.film.patches",
            "the wall film needs at least one wall patch in particles.film.patches",
        ));
    }
    for (i, n) in names.iter().enumerate() {
        if n.is_empty() {
            out.push(d("particles.film.patches", "a film patch name is empty"));
        } else if names[..i].contains(n) {
            out.push(d("particles.film.patches", format!("patch '{n}' is listed twice")));
        } else if walls.contains(n) {
            out.push(d(
                "particles.film.patches",
                format!("patch '{n}' carries the film and also has an entry particles.walls.{n}; remove one"),
            ));
        }
    }
    if !(gf(case, "particles.film.viscosity") > 0.0) {
        out.push(d(
            "particles.film.viscosity",
            "the film liquid viscosity [Pa s] must be set (> 0); -1 means unset",
        ));
    }
    if !gb(case, "particles.gravity") {
        out.push(d(
            "particles.gravity",
            "the wall film is driven by gravity alone: particles.gravity must be true",
        ));
    } else {
        let g = vec3_of(case, "physics.gravity").ok().flatten().unwrap_or([0.0; 3]);
        if !(g.iter().all(|x| x.is_finite()) && g.iter().any(|x| *x != 0.0)) {
            out.push(d(
                "physics.gravity",
                "the wall film is driven by gravity alone: physics.gravity must be a non-zero vector",
            ));
        }
    }
    check_film_gas(case, &names, out);
}

/// Checks of the gas drives of the wall film (ADR 0206 S2, E1 to E5), with the film enabled.
fn check_film_gas(case: &Case, names: &[String], out: &mut Vec<Diagnostic>) {
    let (rho_g, rho_l) = (gf(case, "physics.rho"), gf(case, "particles.density"));
    if gb(case, "particles.film.gas_buoyancy") && rho_l > 0.0 && !(rho_g < rho_l) {
        out.push(d(
            "particles.film.gas_buoyancy",
            format!("gas buoyancy needs a carrier density physics.rho ({rho_g}) below the liquid density particles.density ({rho_l}) (ADR 0206)"),
        ));
    }
    if !gb(case, "particles.film.gas_shear") {
        return;
    }
    let key = "particles.film.gas_shear";
    let model = gs(case, "turbulence.model");
    if !matches!(model.as_str(), "" | "laminar") {
        out.push(d(
            key,
            format!("the gas shear of the film is the laminar wall stress of the carrier; it is not available with turbulence.model = \"{model}\" (ADR 0206)"),
        ));
    }
    if gb(case, "dynamic_mesh.enabled") {
        out.push(d(
            key,
            "the gas shear of the film needs walls at rest; it is not available with dynamic_mesh.enabled (ADR 0206)",
        ));
    }
    for n in case.tree.names("sources").unwrap_or_default() {
        let t = gs(case, &format!("sources.{n}.type"));
        if matches!(t.as_str(), "rotating_frame" | "mrf") {
            out.push(d(
                key,
                format!("the gas shear of the film is not available with a rotating frame (sources.{n}.type = \"{t}\") (ADR 0206)"),
            ));
        }
    }
    for n in names.iter().filter(|n| !n.is_empty()) {
        let b = |f: &str| format!("boundaries.{n}.{f}");
        let kind = gs(case, &b("velocity"));
        if kind.is_empty() {
            // no boundaries entry: Simulation::new reports the missing condition
            continue;
        }
        let moving = vec3_of(case, &b("velocity_value"))
            .ok()
            .flatten()
            .is_some_and(|v| v.iter().any(|x| *x != 0.0));
        let expr = ["x", "y", "z"]
            .iter()
            .any(|c| !gs(case, &b(&format!("velocity_expression_{c}"))).is_empty());
        if kind != "fixed" || moving || expr || !gs(case, &b("velocity_udf")).is_empty() {
            out.push(d(
                key,
                format!("the gas shear of the film needs the film patch '{n}' to be a no-slip wall at rest (boundaries.{n}: velocity = \"fixed\" with velocity_value [0, 0, 0], no velocity_expression or velocity_udf) (ADR 0206)"),
            ));
        }
    }
}

/// Is two-way coupling requested (ADR 0059)?
pub fn two_way(case: &Case) -> bool {
    enabled(case) && gb(case, "particles.two_way")
}

/// Is the lumped heat transfer requested (ADR 0059)?
pub fn heat_transfer(case: &Case) -> bool {
    enabled(case) && gb(case, "particles.heat_transfer")
}

/// Checks of the two-way coupling and heat-transfer keys (ADR 0059 D1-D9), located.
fn check_coupling(case: &Case, out: &mut Vec<Diagnostic>) {
    let two = gb(case, "particles.two_way");
    let heat = gb(case, "particles.heat_transfer");
    let names = injection_names(case);
    if two {
        let alg = gs(case, "solver.algorithm");
        let steady = matches!(alg.as_str(), "simple" | "simplec");
        if !matches!(alg.as_str(), "simple" | "simplec" | "piso" | "pimple") {
            out.push(d(
                "particles.two_way",
                format!("two-way coupling is available with the simple, simplec, piso and pimple algorithms, not \"{alg}\" (ADR 0059)"),
            ));
        }
        if gb(case, "solver.frozen_flow") {
            out.push(d(
                "particles.two_way",
                "two-way coupling needs a solved flow; it is not available with solver.frozen_flow",
            ));
        }
        if gb(case, "dynamic_mesh.enabled") {
            out.push(d(
                "particles.two_way",
                "two-way coupling is not available with dynamic_mesh.enabled",
            ));
        }
        if gf(case, "solver.error_tolerance") > 0.0 {
            out.push(d(
                "particles.two_way",
                "two-way coupling is not available with solver.error_tolerance > 0 (the step is not known before the flow step)",
            ));
        }
        if gs(case, "solver.time_advance") == "fractional_step" {
            out.push(d(
                "particles.two_way",
                "two-way coupling is not available with solver.time_advance = fractional_step",
            ));
        }
        let r = gf(case, "particles.two_way_relax");
        if !(r > 0.0 && r <= 1.0) {
            out.push(d(
                "particles.two_way_relax",
                "the coupling relaxation must be in (0, 1]",
            ));
        }
        if !(gf(case, "particles.two_way_tolerance") > 0.0) {
            out.push(d(
                "particles.two_way_tolerance",
                "the coupling tolerance must be positive",
            ));
        }
        if steady && names.len() > 1 {
            // one normalisation window for all injections
            let end = gf(case, "particles.end_time");
            let key = |n: &String| {
                let p = |f: &str| format!("particles.injection.{n}.{f}");
                let e = match gf(case, &p("end_time")) {
                    t if t == UNSET => end,
                    t => t,
                };
                (gf(case, &p("rate")), gf(case, &p("start_time")), e)
            };
            let k0 = key(&names[0]);
            for n in &names[1..] {
                if key(n) != k0 {
                    out.push(d(
                        format!("particles.injection.{n}.rate"),
                        format!(
                            "steady two-way coupling needs the same rate, start_time and end_time for all injections (differs from '{}')",
                            names[0]
                        ),
                    ));
                }
            }
        }
    }
    if heat {
        if !two {
            out.push(d(
                "particles.heat_transfer",
                "heat transfer needs particles.two_way = true (the temperature of the parcels is tracked with the carrier temperature fed back)",
            ));
        }
        if !gb(case, "energy.enabled") {
            out.push(d(
                "particles.heat_transfer",
                "heat transfer needs energy.enabled = true",
            ));
        }
        if !(gf(case, "physics.cp") > 0.0) {
            out.push(d("physics.cp", "heat transfer needs physics.cp > 0"));
        }
        if !(gf(case, "physics.conductivity") > 0.0) {
            out.push(d(
                "physics.conductivity",
                "heat transfer needs physics.conductivity > 0",
            ));
        }
        if !(gf(case, "particles.particle_cp") > 0.0) {
            out.push(d(
                "particles.particle_cp",
                "heat transfer needs the particle specific heat (> 0); -1 means unset",
            ));
        }
        for n in &names {
            let k = format!("particles.injection.{n}.temperature");
            if !(gf(case, &k) > 0.0) {
                out.push(d(
                    k,
                    "heat transfer needs the injection temperature [K] (> 0); -1 means unset",
                ));
            }
        }
    }
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !enabled(case) {
        for k in ["two_way", "heat_transfer"] {
            if gb(case, &format!("particles.{k}")) {
                out.push(d(format!("particles.{k}"), "needs particles.enabled = true (ADR 0059)"));
            }
        }
        if gb(case, "particles.film.enabled") {
            out.push(d("particles.film.enabled", "needs particles.enabled = true (ADR 0192)"));
        }
        for k in ["erosion", "rough_wall"] {
            if gb(case, &format!("particles.{k}.enabled")) {
                out.push(d(
                    format!("particles.{k}.enabled"),
                    "needs particles.enabled = true (ADR 0194)",
                ));
            }
        }
        return out;
    }
    check_coupling(case, &mut out);
    check_film(case, &mut out);
    check_erosion(case, &mut out);
    check_rough_wall(case, &mut out);
    if gf(case, "particles.density") <= 0.0 {
        out.push(d(
            "particles.density",
            "particle density must be set (> 0); -1 means unset",
        ));
    }
    if gf(case, "particles.end_time") <= 0.0 {
        out.push(d(
            "particles.end_time",
            "end of tracking must be set (> 0); -1 means unset",
        ));
    }
    let dt = gf(case, "particles.dt");
    if dt != UNSET && dt <= 0.0 {
        out.push(d(
            "particles.dt",
            "the tracking step must be positive (or -1 for solver.dt)",
        ));
    }
    if gf(case, "physics.rho") <= 0.0 || gf(case, "physics.nu") <= 0.0 {
        out.push(d(
            "physics.nu",
            "particles need positive physics.rho and physics.nu (constant fluid properties)",
        ));
    }
    if crate::vd::enabled(case) || crate::comp::enabled(case) {
        out.push(d(
            "particles.enabled",
            "particles are available with the incompressible constant-property solvers only (ADR 0028)",
        ));
    }
    let names = injection_names(case);
    if names.is_empty() {
        out.push(d(
            "particles.injection",
            "at least one injection is needed (particles.injection.<name>)",
        ));
    }
    if let Ok(w) = case.tree.names("particles.walls") {
        for n in w {
            let r = gf(case, &format!("particles.walls.{n}.restitution"));
            if r != UNSET && !(0.0..=1.0).contains(&r) {
                out.push(d(
                    format!("particles.walls.{n}.restitution"),
                    "restitution must be in [0, 1] (or -1 for particles.restitution)",
                ));
            }
        }
    }
    for n in &names {
        let p = |f: &str| format!("particles.injection.{n}.{f}");
        let kind = gs(case, &p("kind"));
        match kind.as_str() {
            "point" => match vec3_of(case, &p("position")) {
                Ok(Some(v)) if v.iter().all(|x| x.is_finite()) => {}
                Ok(_) => out.push(d(p("position"), "a point injection needs position = [x, y, z]")),
                Err(k) => out.push(d(p("position"), format!("expected 3 components [x, y, z], got {k}"))),
            },
            "surface" => {
                if gs(case, &p("patch")).is_empty() {
                    out.push(d(p("patch"), "a surface injection needs the name of a boundary patch"));
                }
            }
            _ => out.push(d(p("kind"), "injection kind must be set: \"point\" or \"surface\"")),
        }
        let rate = gf(case, &p("rate"));
        if !(rate > 0.0) {
            out.push(d(
                p("rate"),
                "injection rate [parcels/s] must be set (> 0); -1 means unset",
            ));
        }
        let t0 = gf(case, &p("start_time"));
        let t1 = gf(case, &p("end_time"));
        if t1 != UNSET && t1 < t0 {
            out.push(d(p("end_time"), format!("end_time {t1} is before start_time {t0}")));
        }
        if gf(case, &p("parcel_count")) <= 0.0 {
            out.push(d(p("parcel_count"), "parcel_count must be positive"));
        }
        if gs(case, &p("velocity_mode")) == "given" {
            if let Err(k) = vec3_of(case, &p("velocity")) {
                out.push(d(p("velocity"), format!("expected 3 components [x, y, z], got {k}")));
            }
        }
        match gs(case, &p("distribution")).as_str() {
            "rosin_rammler" => {
                if gf(case, &p("rr_d_bar")) <= 0.0 {
                    out.push(d(p("rr_d_bar"), "Rosin-Rammler size constant must be set (> 0)"));
                }
                if gf(case, &p("rr_n")) <= 0.0 {
                    out.push(d(p("rr_n"), "Rosin-Rammler spread parameter must be set (> 0)"));
                }
                let (lo, hi) = (gf(case, &p("d_min")), gf(case, &p("d_max")));
                if lo != UNSET && lo <= 0.0 {
                    out.push(d(p("d_min"), "d_min must be positive (or -1 for none)"));
                }
                if hi != UNSET && hi <= 0.0 {
                    out.push(d(p("d_max"), "d_max must be positive (or -1 for none)"));
                }
                if lo > 0.0 && hi > 0.0 && hi <= lo {
                    out.push(d(p("d_max"), "d_max must exceed d_min"));
                }
            }
            "uniform" => {
                let (lo, hi) = (gf(case, &p("d_min")), gf(case, &p("d_max")));
                if !(lo > 0.0) {
                    out.push(d(p("d_min"), "a uniform distribution needs d_min > 0"));
                }
                if !(hi > 0.0) {
                    out.push(d(p("d_max"), "a uniform distribution needs d_max > 0"));
                } else if lo > 0.0 && hi <= lo {
                    out.push(d(p("d_max"), "d_max must exceed d_min"));
                }
            }
            _ => {
                if gf(case, &p("diameter")) <= 0.0 {
                    out.push(d(p("diameter"), "particle diameter must be set (> 0); -1 means unset"));
                }
            }
        }
        let mf = gf(case, &p("mass_flow"));
        if mf != UNSET && !(mf > 0.0) {
            out.push(d(
                p("mass_flow"),
                "mass_flow must be positive (or -1 for parcel_count weighting)",
            ));
        }
        let ha = gf(case, &p("cone_half_angle"));
        if ha != UNSET {
            if !(ha > 0.0 && ha <= 180.0) {
                out.push(d(
                    p("cone_half_angle"),
                    "cone_half_angle must be in (0, 180] degrees (or -1 for no cone)",
                ));
            } else {
                let hi = gf(case, &p("cone_inner_half_angle"));
                if !(hi >= 0.0 && hi < ha) {
                    out.push(d(
                        p("cone_inner_half_angle"),
                        "cone_inner_half_angle must be in [0, cone_half_angle) degrees",
                    ));
                }
            }
            match vec3_of(case, &p("cone_axis")) {
                Ok(Some(v)) if v.iter().all(|x| x.is_finite()) && v.iter().any(|x| *x != 0.0) => {}
                Ok(_) => out.push(d(
                    p("cone_axis"),
                    "a cone injection needs a non-zero cone_axis = [x, y, z]",
                )),
                Err(k) => out.push(d(p("cone_axis"), format!("expected 3 components [x, y, z], got {k}"))),
            }
            if !(gf(case, &p("cone_speed")) > 0.0) {
                out.push(d(
                    p("cone_speed"),
                    "a cone injection needs cone_speed > 0; -1 means unset",
                ));
            }
        }
    }
    let model_fields = gb(case, "particles.dispersion_from_model.enabled");
    if model_fields {
        check_dispersion_from_model(case, &mut out);
    }
    if gs(case, "particles.dispersion") == "drw" {
        if !model_fields {
            if !(gf(case, "particles.dispersion_k") > 0.0) {
                out.push(d(
                    "particles.dispersion_k",
                    "dispersion = drw needs dispersion_k > 0; -1 means unset",
                ));
            }
            if !(gf(case, "particles.dispersion_epsilon") > 0.0) {
                out.push(d(
                    "particles.dispersion_epsilon",
                    "dispersion = drw needs dispersion_epsilon > 0; -1 means unset",
                ));
            }
        }
        if !(gf(case, "particles.dispersion_cl") > 0.0) {
            out.push(d("particles.dispersion_cl", "dispersion_cl must be positive"));
        }
    }
    out
}

/// The located errors of `particles.dispersion_from_model.enabled` (ADR 0208 S2 as amended, DE1 to DE3).
fn check_dispersion_from_model(case: &Case, out: &mut Vec<Diagnostic>) {
    if gs(case, "particles.dispersion") != "drw" {
        out.push(d(
            "particles.dispersion_from_model.enabled",
            "dispersion_from_model needs particles.dispersion = \"drw\" (ADR 0208)",
        ));
    }
    let tm = gs(case, "turbulence.model");
    if !matches!(tm.as_str(), "k_epsilon" | "k_omega_sst") {
        out.push(d(
            "particles.dispersion_from_model.enabled",
            format!("dispersion_from_model needs the turbulence fields k and epsilon (turbulence.model = \"k_epsilon\" or \"k_omega_sst\"), not \"{tm}\" (ADR 0208)"),
        ));
    }
    for k in ["dispersion_k", "dispersion_epsilon"] {
        if gf(case, &format!("particles.{k}")) != UNSET {
            out.push(d(
                format!("particles.{k}"),
                format!("{k} is not used with dispersion_from_model (the turbulence model gives k and epsilon); leave it -1 (ADR 0208)"),
            ));
        }
    }
}

fn options(case: &Case) -> Options {
    let g = if gb(case, "particles.gravity") {
        vec3_of(case, "physics.gravity").ok().flatten().unwrap_or([0.0; 3])
    } else {
        [0.0; 3]
    };
    let wall = match gs(case, "particles.wall_interaction").as_str() {
        "stick" => WallAction::Stick,
        "escape" => WallAction::Escape,
        _ => WallAction::Reflect(gf(case, "particles.restitution")),
    };
    Options {
        drag: match gs(case, "particles.drag").as_str() {
            "stokes" => DragLaw::Stokes,
            "none" => DragLaw::None,
            _ => DragLaw::SchillerNaumann,
        },
        scheme: match gs(case, "particles.integration").as_str() {
            "euler" => Scheme::Euler,
            "rk2" => Scheme::Rk2,
            "rk4" => Scheme::Rk4,
            _ => Scheme::Analytic,
        },
        interp: match gs(case, "particles.interpolation").as_str() {
            "cell_centre" => Interp::CellCentre,
            _ => Interp::Gradient,
        },
        gravity: g,
        default_wall: wall,
        ..Options::default()
    }
}

/// Build the tracker on the mesh of a simulation, with all injectors. Mesh-dependent problems (unknown patches, points
/// outside the mesh, periodic patches) are reported as located diagnostics.
pub fn build_tracker(case: &Case, sim: &Simulation) -> std::result::Result<Tracker, Vec<Diagnostic>> {
    let mut errs = check(case);
    if !errs.is_empty() {
        return Err(errs);
    }
    let mesh = sim.geom.mesh.clone();
    let opts = options(case);
    let mut overrides = Vec::new();
    for n in case.tree.names("particles.walls").unwrap_or_default() {
        let p = |f: &str| format!("particles.walls.{n}.{f}");
        let e = match gf(case, &p("restitution")) {
            r if r == UNSET => gf(case, "particles.restitution"),
            r => r,
        };
        let a = match gs(case, &p("interaction")).as_str() {
            "reflect" => Some(WallAction::Reflect(e)),
            "stick" => Some(WallAction::Stick),
            "escape" => Some(WallAction::Escape),
            _ => None,
        };
        if mesh.patch(&n).is_none() {
            errs.push(d(
                format!("particles.walls.{n}"),
                format!("unknown boundary patch '{n}'"),
            ));
        } else if let Some(a) = a {
            overrides.push((n.clone(), a));
        } else if gf(case, &p("restitution")) != UNSET {
            // default interaction with a restitution: reflect with that value
            overrides.push((n.clone(), WallAction::Reflect(e)));
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let mut tr = Tracker::new(mesh, opts, &overrides).map_err(|e| vec![d("particles", e.to_string())])?;
    let density = gf(case, "particles.density");
    let seed = gi(case, "particles.seed").max(0) as u64;
    let end = gf(case, "particles.end_time");
    for (k, n) in injection_names(case).iter().enumerate() {
        let p = |f: &str| format!("particles.injection.{n}.{f}");
        let kind = if gs(case, &p("kind")) == "point" {
            InjKind::Point(vec3_of(case, &p("position")).ok().flatten().unwrap_or([0.0; 3]))
        } else {
            InjKind::Surface(gs(case, &p("patch")))
        };
        let velocity = if gs(case, &p("velocity_mode")) == "fluid" {
            VelMode::Fluid
        } else {
            VelMode::Given(vec3_of(case, &p("velocity")).ok().flatten().unwrap_or([0.0; 3]))
        };
        let opt = |v: f64| if v == UNSET { None } else { Some(v) };
        let dist = if gs(case, &p("distribution")) == "rosin_rammler" {
            SizeDist::RosinRammler {
                d_bar: gf(case, &p("rr_d_bar")),
                n: gf(case, &p("rr_n")),
                d_min: opt(gf(case, &p("d_min"))),
                d_max: opt(gf(case, &p("d_max"))),
            }
        } else if gs(case, &p("distribution")) == "uniform" {
            SizeDist::Uniform {
                d_min: gf(case, &p("d_min")),
                d_max: gf(case, &p("d_max")),
            }
        } else {
            SizeDist::Constant(gf(case, &p("diameter")))
        };
        let t_end = match gf(case, &p("end_time")) {
            t if t == UNSET => end,
            t => t,
        };
        let spec = InjectorSpec {
            name: n.clone(),
            kind,
            rate: gf(case, &p("rate")),
            t_start: gf(case, &p("start_time")),
            t_end,
            velocity,
            dist,
            density,
            parcel_count: gf(case, &p("parcel_count")),
            seed: seed.wrapping_mul(1_000_003).wrapping_add(k as u64),
        };
        tr.add_injector(spec).map_err(|e| {
            let field = match gs(case, &p("kind")).as_str() {
                "point" => "position",
                _ => "patch",
            };
            vec![d(p(field), e.to_string())]
        })?;
        let mf = gf(case, &p("mass_flow"));
        let ha = gf(case, &p("cone_half_angle"));
        let ext = InjectorExt {
            mass_flow: (mf != UNSET).then_some(mf),
            cone: (ha != UNSET).then(|| ConeSpec {
                axis: vec3_of(case, &p("cone_axis")).ok().flatten().unwrap_or([0.0, 0.0, 1.0]),
                half_angle: ha.to_radians(),
                inner_half_angle: gf(case, &p("cone_inner_half_angle")).to_radians(),
                speed: gf(case, &p("cone_speed")),
            }),
            temperature: (heat_transfer(case) && gf(case, &p("temperature")) != UNSET)
                .then(|| gf(case, &p("temperature"))),
        };
        if ext != InjectorExt::default() {
            tr.set_injector_ext(n, ext)
                .map_err(|e| vec![d(format!("particles.injection.{n}"), e.to_string())])?;
        }
    }
    if gs(case, "particles.dispersion") == "drw" {
        let nc = sim.geom.mesh.n_cells();
        let model = Dispersion {
            c_l: gf(case, "particles.dispersion_cl"),
            crossing: gb(case, "particles.dispersion_crossing"),
        };
        let dseed = (gi(case, "particles.dispersion_seed").max(0) as u64).wrapping_add(0x5EED_D15B);
        let (k, eps) = if dispersion_from_model(case) {
            // ADR 0208 D2: the turbulence model's fields of the current state
            model_k_epsilon(sim.turb_state()).map_err(|e| vec![d("particles.dispersion_from_model.enabled", e)])?
        } else {
            (
                vec![gf(case, "particles.dispersion_k"); nc],
                vec![gf(case, "particles.dispersion_epsilon"); nc],
            )
        };
        tr.set_dispersion(model, k, eps, dseed)
            .map_err(|e| vec![d("particles.dispersion", e.to_string())])?;
    }
    if heat_transfer(case) {
        // carrier temperature of the cells: the energy solution
        let t_fluid = sim
            .field("T")
            .map_err(|e| vec![d("particles.heat_transfer", format!("carrier temperature: {e}"))])?;
        tr.set_thermal(rcfd_particles::Thermal {
            cp_p: gf(case, "particles.particle_cp"),
            k_f: gf(case, "physics.conductivity"),
            cp_f: gf(case, "physics.cp"),
            t_fluid,
        })
        .map_err(|e| vec![d("particles.heat_transfer", e.to_string())])?;
    }
    if film(case) {
        let spec = rcfd_particles::FilmSpec {
            patches: film_patches(case),
            density,
            viscosity: gf(case, "particles.film.viscosity"),
            gravity: tr.opts.gravity,
        };
        tr.set_film(spec)
            .map_err(|e| vec![d("particles.film.patches", e.to_string())])?;
        let gas = FilmGas {
            buoyancy: gb(case, "particles.film.gas_buoyancy"),
            shear: gb(case, "particles.film.gas_shear"),
        };
        if gas != FilmGas::default() {
            // ADR 0206 S2: only with a gas drive
            tr.set_film_gas(gas)
                .map_err(|e| vec![d("particles.film.enabled", e.to_string())])?;
        }
    }
    if erosion(case) {
        // ADR 0197 P9: without a preset these are the three values as written
        let c = erosion_constants(case);
        let spec = ErosionSpec {
            patches: patch_list(case, "particles.erosion.patches"),
            k: c.k,
            n: c.n,
            density: c.density,
        };
        tr.set_erosion(spec)
            .map_err(|e| vec![d("particles.erosion.patches", e.to_string())])?;
    }
    if rough_wall(case) {
        let spec = RoughSpec {
            patches: patch_list(case, "particles.rough_wall.patches"),
            angle_std: gf(case, "particles.rough_wall.angle_std").to_radians(),
            seed: gi(case, "particles.rough_wall.seed").max(0) as u64,
        };
        tr.set_rough_wall(spec)
            .map_err(|e| vec![d("particles.rough_wall.patches", e.to_string())])?;
    }
    Ok(tr)
}

/// Fluid data of a simulation for the tracker (cell velocity, least-squares gradient, constant properties).
pub(crate) struct FlowData {
    u: Vec<Vec3>,
    grad: Vec<[Vec3; 3]>,
    rho: f64,
    mu: f64,
}

pub(crate) fn flow_data(case: &Case, sim: &Simulation) -> FlowData {
    flow_data_from(case, &sim.geom, sim.velocity())
}

/// [`flow_data`] for an explicit geometry and velocity field (the two-way coupling tracks on the field of the old time
/// level, ADR 0059).
pub(crate) fn flow_data_from(case: &Case, geom: &rcfd_fv::FvGeometry, u: &rcfd_fv::VectorField) -> FlowData {
    let bv = u.boundary_values(geom);
    let n = u.values.len();
    let mut grad = vec![[[0.0; 3]; 3]; n];
    for k in 0..3 {
        let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
        let b: Vec<f64> = bv.iter().map(|v| v[k]).collect();
        let g = rcfd_fv::gradient::least_squares(geom, &cells, &b);
        for c in 0..n {
            grad[c][k] = g[c];
        }
    }
    let rho = gf(case, "physics.rho");
    FlowData {
        u: u.values.clone(),
        grad,
        rho,
        mu: rho * gf(case, "physics.nu"),
    }
}

impl FlowData {
    pub(crate) fn flow(&self) -> Flow<'_> {
        Flow {
            u: &self.u,
            grad: Some(&self.grad),
            rho: self.rho,
            mu: self.mu,
        }
    }
}

fn io_diag(e: std::io::Error, path: &str) -> Vec<Diagnostic> {
    vec![d("particles.output_file", format!("cannot write '{path}': {e}"))]
}

fn write_series(case: &Case, tr: &Tracker, index: Option<usize>) -> std::result::Result<(), Vec<Diagnostic>> {
    let base = gs(case, "particles.output_file");
    if base.is_empty() {
        return Ok(());
    }
    let path = match index {
        None => base.clone(),
        Some(i) => {
            let stem = base.strip_suffix(".vtp").unwrap_or(&base);
            format!("{stem}_{i}.vtp")
        }
    };
    write_vtp(&path, &tr.particles).map_err(|e| io_diag(e, &path))
}

/// Track the particles of `case` on the current velocity field of `sim` (a prescribed `solver.frozen_flow` field or a
/// converged steady solution), from time 0 to `particles.end_time` in steps of `particles.dt`.
pub fn track_frozen(case: &Case, sim: &Simulation) -> std::result::Result<Tracker, Vec<Diagnostic>> {
    let mut tr = build_tracker(case, sim)?;
    let mut dt = gf(case, "particles.dt");
    if dt == UNSET {
        dt = gf(case, "solver.dt");
    }
    if !(dt > 0.0) {
        return Err(vec![d(
            "particles.dt",
            "set the tracking step (solver.dt is not positive either)",
        )]);
    }
    let end = gf(case, "particles.end_time");
    let every = gi(case, "particles.output_every").max(0) as usize;
    let fd = flow_data(case, sim);
    let flow = fd.flow();
    let mut step = 0usize;
    while tr.time < end * (1.0 - 1e-12) {
        let h = dt.min(end - tr.time);
        tr.advance(&flow, h).map_err(|e| vec![d("particles", e.to_string())])?;
        step += 1;
        if every > 0 && step % every == 0 {
            write_series(case, &tr, Some(step))?;
        }
    }
    write_series(case, &tr, None)?;
    Ok(tr)
}

/// Advance `sim` step by step (a transient algorithm) and track the particles over each flow step on the current field,
/// until `particles.end_time` or the end of the simulation.
pub fn track_transient(case: &Case, sim: &mut Simulation) -> std::result::Result<Tracker, Vec<Diagnostic>> {
    if two_way(case) {
        return Err(vec![d(
            "particles.two_way",
            "track_transient is one-way; two-way coupling runs inside Simulation::step",
        )]);
    }
    sim.detach_particles();
    let mut tr = build_tracker(case, sim)?;
    if sim.is_steady() {
        return Err(vec![d(
            "solver.algorithm",
            "transient tracking needs a transient algorithm (piso or pimple); use track_frozen for steady flows",
        )]);
    }
    let end = gf(case, "particles.end_time");
    let every = gi(case, "particles.output_every").max(0) as usize;
    let mut step = 0usize;
    // the solver's own time: the flow's steps exactly, not the end time a fixed-step run reports (ADR 0160 D2)
    let mut t_prev = sim.solver_time();
    while tr.time < end * (1.0 - 1e-12) && !sim.is_done() {
        sim.step();
        let dt = sim.solver_time() - t_prev;
        t_prev = sim.solver_time();
        if dt <= 0.0 {
            break;
        }
        let h = dt.min(end - tr.time);
        if dispersion_from_model(case) {
            // ADR 0208 D2: the fields at the end of the flow step, the level of the velocity tracked on
            let (k, eps) =
                model_k_epsilon(sim.turb_state()).map_err(|e| vec![d("particles.dispersion_from_model.enabled", e)])?;
            tr.set_dispersion_fields(&k, &eps)
                .map_err(|e| vec![d("particles.dispersion_from_model.enabled", e.to_string())])?;
        }
        let fd = flow_data(case, sim);
        tr.advance(&fd.flow(), h)
            .map_err(|e| vec![d("particles", e.to_string())])?;
        step += 1;
        if every > 0 && step % every == 0 {
            write_series(case, &tr, Some(step))?;
        }
    }
    write_series(case, &tr, None)?;
    Ok(tr)
}
