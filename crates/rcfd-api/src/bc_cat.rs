//! Boundary-condition catalogue completion (F-200, ADR 0022): `boundaries.<patch>.bc_kind` = `mass_flow` | `outflow` |
//! `pressure_outlet` | `opening` | `vent` | `total_pressure_inlet` | `radial_equilibrium_outlet` |
//! `target_mass_flow_outlet`, for the constant-density segregated kernels.
//!
//! [`check`] holds the settings checks that need no mesh (called from `Case::semantic_errors`);
//! [`Catalogue::from_case`] adds the checks that need the patch geometry and builds the per-patch state;
//! [`Catalogue::refresh`] rebuilds the flow-dependent conditions from the lagged fields before every iteration or step.

use crate::case::{Case, Diagnostic};
use crate::exprs;
use crate::sim::{get_f, get_s, get_v};
use crate::turb::Role;
use rcfd_core::units::Dimension;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{Bc, Directed, FixedValue, Profile, Switched, ZeroGradient};
use rcfd_fv::outlets::{
    area_mean_shift, outflow_speeds, radial_equilibrium, PressureUpdate, PressureUpdater, RadialReference,
};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::PatchKind;

/// The kinds of `bc_kind` other than the sentinel `none`.
pub const KINDS: [&str; 8] = [
    "mass_flow",
    "outflow",
    "pressure_outlet",
    "opening",
    "vent",
    "total_pressure_inlet",
    "radial_equilibrium_outlet",
    "target_mass_flow_outlet",
];

/// Weight of the new value in the relaxed lagged pressure updates (vent, total-pressure inlet, radial equilibrium).
const LAG_WEIGHT: f64 = 0.5;

/// Keys that only some kinds use: a non-default value on another kind is a located error.
const KEY_KINDS: [(&str, &[&str]); 20] = [
    ("mass_flow_value", &["mass_flow"]),
    ("mass_flow_shape_expression", &["mass_flow"]),
    ("mass_flow_source_patch", &["mass_flow"]),
    ("mass_flow_fraction", &["mass_flow"]),
    ("flow_direction", &["mass_flow", "total_pressure_inlet"]),
    ("swirl_rate", &["mass_flow", "total_pressure_inlet"]),
    (
        "swirl_axis",
        &["mass_flow", "total_pressure_inlet", "radial_equilibrium_outlet"],
    ),
    (
        "swirl_origin",
        &["mass_flow", "total_pressure_inlet", "radial_equilibrium_outlet"],
    ),
    (
        "bc_pressure",
        &[
            "pressure_outlet",
            "opening",
            "vent",
            "total_pressure_inlet",
            "radial_equilibrium_outlet",
            "target_mass_flow_outlet",
        ],
    ),
    ("backflow_direction", &["pressure_outlet"]),
    ("backflow_pressure", &["pressure_outlet"]),
    ("prevent_reverse_flow", &["pressure_outlet", "outflow"]),
    ("pressure_average", &["pressure_outlet"]),
    ("loss_coefficient", &["vent"]),
    ("flow_rate_weight", &["outflow"]),
    ("radial_reference", &["radial_equilibrium_outlet"]),
    ("radial_frame_rate", &["radial_equilibrium_outlet"]),
    ("target_mass_flow", &["target_mass_flow_outlet"]),
    ("pressure_update", &["target_mass_flow_outlet"]),
    ("pressure_relax", &["target_mass_flow_outlet"]),
];

fn d(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// `bc_kind` of a boundary entry (`none` when unset).
pub fn kind(case: &Case, patch: &str) -> String {
    let k = get_s(case, &format!("boundaries.{patch}.bc_kind"));
    if k.is_empty() {
        "none".into()
    } else {
        k
    }
}

/// True when any boundary entry has a `bc_kind` other than `none`.
pub fn any_active(case: &Case) -> bool {
    case.tree
        .names("boundaries")
        .unwrap_or_default()
        .iter()
        .any(|n| kind(case, n) != "none")
}

/// Turbulence role of a catalogue patch (`turbulence = auto`), `None` for `bc_kind = none` (ADR 0022 D6b).
pub fn role(case: &Case, patch: &str) -> Option<Role> {
    let b = format!("boundaries.{patch}");
    match kind(case, patch).as_str() {
        "none" => None,
        "mass_flow" => {
            let inflow = get_f(case, &format!("{b}.mass_flow_value")) > 0.0
                || !get_s(case, &format!("{b}.mass_flow_source_patch")).is_empty();
            Some(if inflow { Role::Inlet } else { Role::ZeroGradient })
        }
        "total_pressure_inlet" => Some(Role::Inlet),
        _ => Some(Role::ZeroGradient),
    }
}

/// Inlet speed of a catalogue patch for the turbulence set-up: `|m| / (rho A)` of a `mass_flow` patch with a given
/// flow rate (with `geom` the patch area is exact; without it only "positive" is known and 1 is returned), 0 where the
/// speed is a result (a sourced mass flow, a total-pressure inlet). `None` for a patch with `bc_kind = none`.
pub fn nominal_speed(case: &Case, geom: Option<&FvGeometry>, patch: &str) -> Option<f64> {
    let k = kind(case, patch);
    if k == "none" {
        return None;
    }
    if k != "mass_flow" {
        return Some(0.0);
    }
    let m = get_f(case, &format!("boundaries.{patch}.mass_flow_value")).abs();
    if m == 0.0 {
        return Some(0.0);
    }
    Some(match geom.and_then(|g| g.mesh.patch(patch).map(|p| (g, p))) {
        Some((g, p)) => {
            let a: f64 = p.faces().map(|f| vec3::norm(g.mesh.face_area_vectors()[f])).sum();
            m / (get_f(case, "physics.rho") * a)
        }
        None => 1.0,
    })
}

fn is_default(case: &Case, path: &str) -> bool {
    match (case.tree.get(path), case.tree.schema_at(path)) {
        (Ok(v), Ok(s)) => v == s.default_value(),
        _ => true,
    }
}

/// Settings checks that need no mesh (ADR 0022 V1, V2, V4, V5, V8, V9, V10, V11, V14).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let names = case.tree.names("boundaries").unwrap_or_default();
    let alg = get_s(case, "solver.algorithm");
    let potential = get_s(case, "initial.method") == "potential";
    for n in &names {
        let k = kind(case, n);
        if k == "none" {
            continue;
        }
        let b = format!("boundaries.{n}");
        let p = |key: &str| format!("{b}.{key}");
        if alg == "compressible" || alg == "coupled" || crate::vd::enabled(case) {
            out.push(d(
                &p("bc_kind"),
                format!(
                    "bc_kind = \"{k}\" is available with the constant-density segregated algorithms (simple, simplec, piso, pimple), not with {}",
                    if crate::vd::enabled(case) {
                        "the variable-density solver".to_string()
                    } else {
                        format!("solver.algorithm = \"{alg}\"")
                    }
                ),
            ));
        }
        // the kind owns the velocity and pressure conditions
        if get_s(case, &p("velocity")) != "fixed" {
            out.push(d(
                &p("velocity"),
                format!(
                    "bc_kind = \"{k}\" sets the velocity condition itself; leave velocity = \"fixed\" (the default)"
                ),
            ));
        }
        if get_s(case, &p("pressure")) != "zero_gradient" {
            out.push(d(
                &p("pressure"),
                format!("bc_kind = \"{k}\" sets the pressure condition itself; leave pressure = \"zero_gradient\" (the default)"),
            ));
        }
        if vec3::norm(get_v(case, &p("velocity_value"), [0.0; 3])) != 0.0 {
            out.push(d(
                &p("velocity_value"),
                format!("bc_kind = \"{k}\" does not use velocity_value (use the kind's own keys)"),
            ));
        }
        for key in [
            "velocity_expression_x",
            "velocity_expression_y",
            "velocity_expression_z",
            "velocity_udf",
            "profile",
        ] {
            if !get_s(case, &p(key)).trim().is_empty() {
                out.push(d(&p(key), format!("bc_kind = \"{k}\" does not use {key}")));
            }
        }
        // keys of other kinds
        for (key, kinds) in KEY_KINDS {
            if !kinds.contains(&k.as_str()) && !is_default(case, &p(key)) {
                out.push(d(
                    &p(key),
                    format!(
                        "{key} is used by bc_kind = {} (this patch is \"{k}\")",
                        kinds.join(" | ")
                    ),
                ));
            }
        }
        if potential && k != "mass_flow" {
            out.push(d(
                &p("bc_kind"),
                format!("initial.method = \"potential\" supports bc_kind = \"mass_flow\" patches, not \"{k}\""),
            ));
        }
        match k.as_str() {
            "mass_flow" => {
                let m = get_f(case, &p("mass_flow_value"));
                let src = get_s(case, &p("mass_flow_source_patch"));
                if !src.is_empty() {
                    if m != 0.0 {
                        out.push(d(
                            &p("mass_flow_source_patch"),
                            "give mass_flow_value or mass_flow_source_patch, not both",
                        ));
                    }
                    let f = get_f(case, &p("mass_flow_fraction"));
                    if !(f > 0.0 && f <= 1.0) {
                        out.push(d(&p("mass_flow_fraction"), "the fraction must lie in (0, 1]"));
                    }
                    if &src == n {
                        out.push(d(
                            &p("mass_flow_source_patch"),
                            "a patch cannot be its own mass-flow source",
                        ));
                    } else if !names.contains(&src) {
                        out.push(d(
                            &p("mass_flow_source_patch"),
                            format!("no boundary entry '{src}' (entries: {})", names.join(", ")),
                        ));
                    } else {
                        // follow the chain of sources: it must not come back to this patch
                        let mut cur = src.clone();
                        for _ in 0..names.len() {
                            if kind(case, &cur) != "mass_flow" {
                                break;
                            }
                            let next = get_s(case, &format!("boundaries.{cur}.mass_flow_source_patch"));
                            if next.is_empty() {
                                break;
                            }
                            if &next == n {
                                out.push(d(
                                    &p("mass_flow_source_patch"),
                                    format!("the mass-flow sources form a cycle through '{cur}'"),
                                ));
                                break;
                            }
                            cur = next;
                        }
                    }
                    if crate::turb::enabled(case)
                        && role(case, n) == Some(Role::Inlet)
                        && get_s(case, &p("turbulence")) == "auto"
                        && get_s(case, &p("turbulence_spec")) != "values"
                    {
                        out.push(d(
                            &p("turbulence_spec"),
                            "a mass-flow inlet sourced from another patch has no speed at build time; use turbulence_spec = \"values\"",
                        ));
                    }
                }
                match exprs::compile_bc(
                    case,
                    &p("mass_flow_shape_expression"),
                    Dimension::VELOCITY,
                    &exprs::spatial_context(case),
                ) {
                    Ok(_) => {}
                    Err(e) => out.extend(e),
                }
            }
            "outflow" => {
                if get_f(case, &p("flow_rate_weight")) <= 0.0 {
                    out.push(d(&p("flow_rate_weight"), "the flow-rate weight must be positive"));
                }
            }
            "total_pressure_inlet" => {
                if crate::turb::enabled(case)
                    && get_s(case, &p("turbulence")) == "auto"
                    && get_s(case, &p("turbulence_spec")) != "values"
                {
                    out.push(d(
                        &p("turbulence_spec"),
                        "the inlet speed of a total_pressure_inlet is a result, so a turbulence model needs turbulence_spec = \"values\" here",
                    ));
                }
            }
            "target_mass_flow_outlet" if get_f(case, &p("target_mass_flow")) < 0.0 => {
                out.push(d(
                    &p("target_mass_flow"),
                    "the target mass flow (out of the domain) must not be negative",
                ));
            }
            _ => {}
        }
        if matches!(k.as_str(), "mass_flow" | "total_pressure_inlet")
            && get_f(case, &p("swirl_rate")) != 0.0
            && vec3::norm(get_v(case, &p("swirl_axis"), [0.0, 0.0, 1.0])) == 0.0
        {
            out.push(d(&p("swirl_axis"), "the swirl axis must have non-zero length"));
        }
        if k == "radial_equilibrium_outlet" && vec3::norm(get_v(case, &p("swirl_axis"), [0.0, 0.0, 1.0])) == 0.0 {
            out.push(d(&p("swirl_axis"), "the axis must have non-zero length"));
        }
    }
    out
}

/// Per-patch geometry shared by all kinds.
#[derive(Debug, Clone)]
struct PatchData {
    name: String,
    faces: Vec<usize>,
    owner: Vec<usize>,
    normal: Vec<Vec3>,
    area: Vec<f64>,
    centre: Vec<Vec3>,
}

#[derive(Debug, Clone)]
enum Spec {
    MassFlow {
        /// Signed input [kg/s] into the domain (ignored when sourced).
        m: f64,
        /// Source patch faces and fraction.
        source: Option<(Vec<usize>, f64)>,
        /// `shape_f * e_f` per face.
        dir_shape: Vec<Vec3>,
        /// `sum shape |e.n| A`.
        w_sum: f64,
        /// Swirl velocity per face.
        swirl: Vec<Vec3>,
    },
    Outflow {
        weight: f64,
        prevent: bool,
    },
    Pressure {
        p_set: f64,
        normal: bool,
        total: bool,
        prevent: bool,
        average: bool,
        mask: Vec<bool>,
    },
    Vent {
        p_ext: f64,
        k: f64,
        p_old: Option<Vec<f64>>,
    },
    TotalIn {
        p0: f64,
        dir: Option<Vec3>,
        swirl: Vec<Vec3>,
        p_old: Option<Vec<f64>>,
    },
    Radial {
        axis: Vec3,
        origin: Vec3,
        hub: bool,
        p_ref: f64,
        frame_rate: f64,
        p_old: Option<Vec<f64>>,
    },
    Target {
        target: f64,
        p_t: f64,
        updater: PressureUpdater,
    },
}

#[derive(Debug, Clone)]
struct Entry {
    data: PatchData,
    spec: Spec,
}

/// The catalogue patches of a simulation.
#[derive(Debug, Clone)]
pub struct Catalogue {
    rho: f64,
    entries: Vec<Entry>,
    /// Boundary faces that do not belong to an `outflow` patch.
    other_faces: Vec<usize>,
    weight_sum: f64,
    first: bool,
}

fn unit(v: Vec3) -> Vec3 {
    vec3::unit(v)
}

fn tangent_part(v: Vec3, n: Vec3) -> Vec3 {
    let vn = vec3::dot(v, n);
    [v[0] - n[0] * vn, v[1] - n[1] * vn, v[2] - n[2] * vn]
}

impl Catalogue {
    /// Build the state of every catalogue patch of the case; `None` when no patch uses a kind. Mesh-dependent
    /// validation errors are located.
    pub fn from_case(case: &Case, geom: &FvGeometry) -> Result<Option<Catalogue>, Vec<Diagnostic>> {
        let rho = get_f(case, "physics.rho");
        let mesh = &geom.mesh;
        let mut errs = Vec::new();
        let mut entries = Vec::new();
        let ctx = exprs::spatial_context(case);
        for patch in mesh.patches() {
            if matches!(patch.kind, PatchKind::Empty | PatchKind::Cyclic) {
                continue;
            }
            let k = kind(case, &patch.name);
            if k == "none" {
                continue;
            }
            let b = format!("boundaries.{}", patch.name);
            let p = |key: &str| format!("{b}.{key}");
            let faces: Vec<usize> = patch.faces().collect();
            let data = PatchData {
                name: patch.name.clone(),
                owner: faces.iter().map(|&f| mesh.owner()[f]).collect(),
                normal: faces.iter().map(|&f| geom.normal(f)).collect(),
                area: faces.iter().map(|&f| vec3::norm(mesh.face_area_vectors()[f])).collect(),
                centre: faces.iter().map(|&f| mesh.face_centres()[f]).collect(),
                faces,
            };
            let kin = |pa: f64| pa / rho;
            let spec = match k.as_str() {
                "mass_flow" => {
                    let m = get_f(case, &p("mass_flow_value"));
                    let src_name = get_s(case, &p("mass_flow_source_patch"));
                    let inflow = m >= 0.0 || !src_name.is_empty();
                    let source = if src_name.is_empty() {
                        None
                    } else {
                        mesh.patch(&src_name).map(|sp| {
                            (
                                sp.faces().collect::<Vec<usize>>(),
                                get_f(case, &p("mass_flow_fraction")),
                            )
                        })
                    };
                    if !src_name.is_empty() && source.is_none() {
                        errs.push(d(
                            &p("mass_flow_source_patch"),
                            format!("mesh has no patch '{src_name}'"),
                        ));
                    }
                    let dir_in = get_v(case, &p("flow_direction"), [0.0; 3]);
                    let explicit = vec3::norm(dir_in) > 0.0;
                    let sign = if inflow { -1.0 } else { 1.0 };
                    let shape_expr =
                        exprs::compile_bc(case, &p("mass_flow_shape_expression"), Dimension::VELOCITY, &ctx);
                    let shape: Vec<f64> = match shape_expr {
                        Ok(Some(c)) => data.centre.iter().map(|x| c.eval(&[x[0], x[1], x[2], 0.0])).collect(),
                        Ok(None) => vec![1.0; data.faces.len()],
                        Err(e) => {
                            errs.extend(e);
                            vec![1.0; data.faces.len()]
                        }
                    };
                    if shape.iter().any(|s| !s.is_finite() || *s < 0.0) {
                        errs.push(d(
                            &p("mass_flow_shape_expression"),
                            "the profile shape must be finite and non-negative on every face",
                        ));
                    }
                    let omega_rate = get_f(case, &p("swirl_rate"));
                    let axis = get_v(case, &p("swirl_axis"), [0.0, 0.0, 1.0]);
                    let origin = get_v(case, &p("swirl_origin"), [0.0; 3]);
                    let omega = if omega_rate != 0.0 && vec3::norm(axis) > 0.0 {
                        vec3::scale(unit(axis), omega_rate)
                    } else {
                        [0.0; 3]
                    };
                    let mut dir_shape = Vec::new();
                    let mut swirl = Vec::new();
                    let mut w_sum = 0.0;
                    let mut bad_dir = false;
                    for (i, n) in data.normal.iter().enumerate() {
                        let e = if explicit { unit(dir_in) } else { vec3::scale(*n, sign) };
                        let en = vec3::dot(e, *n) * sign;
                        if en < 0.05 {
                            bad_dir = true;
                        }
                        w_sum += shape[i] * vec3::dot(e, *n).abs() * data.area[i];
                        dir_shape.push(vec3::scale(e, shape[i]));
                        swirl.push(tangent_part(vec3::cross(omega, vec3::sub(data.centre[i], origin)), *n));
                    }
                    if bad_dir {
                        errs.push(d(
                            &p("flow_direction"),
                            "the flow direction must enter (leave, for a negative mass flow) through every face of the patch at an angle of at most about 87 degrees to the normal",
                        ));
                    }
                    if w_sum <= 0.0 && (m != 0.0 || source.is_some()) {
                        errs.push(d(
                            &p("mass_flow_shape_expression"),
                            "the profile shape integrates to zero over the patch",
                        ));
                    }
                    Spec::MassFlow {
                        m,
                        source,
                        dir_shape,
                        w_sum,
                        swirl,
                    }
                }
                "outflow" => Spec::Outflow {
                    weight: get_f(case, &p("flow_rate_weight")),
                    prevent: crate::sim::get_b(case, &p("prevent_reverse_flow")),
                },
                "pressure_outlet" | "opening" => {
                    let open = k == "opening";
                    Spec::Pressure {
                        p_set: kin(get_f(case, &p("bc_pressure"))),
                        normal: open || get_s(case, &p("backflow_direction")) == "normal",
                        total: open || get_s(case, &p("backflow_pressure")) == "total",
                        prevent: !open && crate::sim::get_b(case, &p("prevent_reverse_flow")),
                        average: !open && get_s(case, &p("pressure_average")) == "area",
                        mask: vec![false; data.faces.len()],
                    }
                }
                "vent" => Spec::Vent {
                    p_ext: kin(get_f(case, &p("bc_pressure"))),
                    k: get_f(case, &p("loss_coefficient")),
                    p_old: None,
                },
                "total_pressure_inlet" => {
                    let dir = get_v(case, &p("flow_direction"), [0.0; 3]);
                    if vec3::norm(dir) > 0.0 {
                        let e = unit(dir);
                        let dominant_bad = data.normal.iter().any(|n| {
                            let kk = (0..3).max_by(|&a, &b| n[a].abs().total_cmp(&n[b].abs())).unwrap_or(0);
                            -vec3::dot(e, *n) < 0.05 || e[kk].abs() < 0.05
                        });
                        if dominant_bad {
                            errs.push(d(
                                &p("flow_direction"),
                                "the flow direction must enter through every face of the patch (-e.n >= 0.05) and must have a component along the dominant normal axis of every face (at least 5 percent of its length)",
                            ));
                        }
                    }
                    let omega_rate = get_f(case, &p("swirl_rate"));
                    let axis = get_v(case, &p("swirl_axis"), [0.0, 0.0, 1.0]);
                    let origin = get_v(case, &p("swirl_origin"), [0.0; 3]);
                    let omega = if omega_rate != 0.0 && vec3::norm(axis) > 0.0 {
                        vec3::scale(unit(axis), omega_rate)
                    } else {
                        [0.0; 3]
                    };
                    let swirl = (0..data.faces.len())
                        .map(|i| tangent_part(vec3::cross(omega, vec3::sub(data.centre[i], origin)), data.normal[i]))
                        .collect();
                    Spec::TotalIn {
                        p0: kin(get_f(case, &p("bc_pressure"))),
                        dir: (vec3::norm(dir) > 0.0).then(|| unit(dir)),
                        swirl,
                        p_old: None,
                    }
                }
                "radial_equilibrium_outlet" => {
                    let axis = unit(get_v(case, &p("swirl_axis"), [0.0, 0.0, 1.0]));
                    let origin = get_v(case, &p("swirl_origin"), [0.0; 3]);
                    let rmax = data
                        .centre
                        .iter()
                        .map(|x| radius(*x, origin, axis))
                        .fold(0.0_f64, f64::max);
                    if data
                        .centre
                        .iter()
                        .any(|x| radius(*x, origin, axis) < 1e-12 * rmax.max(1e-30))
                    {
                        errs.push(d(
                            &p("swirl_axis"),
                            "a face of the patch lies on the axis (radius zero); the radial pressure balance is singular there",
                        ));
                    }
                    Spec::Radial {
                        axis,
                        origin,
                        hub: get_s(case, &p("radial_reference")) == "hub",
                        p_ref: kin(get_f(case, &p("bc_pressure"))),
                        frame_rate: get_f(case, &p("radial_frame_rate")),
                        p_old: None,
                    }
                }
                "target_mass_flow_outlet" => Spec::Target {
                    target: get_f(case, &p("target_mass_flow")),
                    p_t: kin(get_f(case, &p("bc_pressure"))),
                    updater: PressureUpdater::new(
                        match get_s(case, &p("pressure_update")).as_str() {
                            "secant" => PressureUpdate::Secant,
                            "guarded_secant" => PressureUpdate::GuardedSecant,
                            _ => PressureUpdate::Bernoulli,
                        },
                        get_f(case, &p("pressure_relax")),
                    ),
                },
                _ => continue,
            };
            entries.push(Entry { data, spec });
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        if entries.is_empty() {
            return Ok(None);
        }
        let mut outflow_face = vec![false; mesh.n_faces()];
        let mut weight_sum = 0.0;
        for e in &entries {
            if let Spec::Outflow { weight, .. } = &e.spec {
                weight_sum += weight;
                for &f in &e.data.faces {
                    outflow_face[f] = true;
                }
            }
        }
        let other_faces = (geom.n_internal..mesh.n_faces())
            .filter(|&f| !outflow_face[f])
            .collect();
        Ok(Some(Catalogue {
            rho,
            entries,
            other_faces,
            weight_sum,
            first: true,
        }))
    }

    /// Names of the catalogue patches.
    pub fn patches(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.data.name.clone()).collect()
    }

    /// Rebuild the flow-dependent conditions from the lagged face flux `phi`, the velocity and the pressure; sets the
    /// boundary conditions of every catalogue patch on `u` and `p`.
    pub fn refresh(&mut self, geom: &FvGeometry, u: &mut VectorField, p: &mut ScalarField, phi: &[f64]) {
        let rho = self.rho;
        let first = std::mem::replace(&mut self.first, false);
        let uf = u.boundary_values(geom);
        let n_int = geom.n_internal;
        let net_other: f64 = self.other_faces.iter().map(|&f| phi[f]).sum();
        let inflow_total = (-rho * net_other).max(0.0);
        let weight_sum = self.weight_sum;
        for e in &mut self.entries {
            let data = &e.data;
            let nf = data.faces.len();
            let flux: f64 = data.faces.iter().map(|&f| phi[f]).sum();
            let abs_flux: f64 = data.faces.iter().map(|&f| phi[f].abs()).sum();
            let eps = 1e-12 * abs_flux.max(1e-30);
            let speed2 = |i: usize| {
                let v = uf[data.faces[i] - n_int];
                vec3::dot(v, v)
            };
            let un = |i: usize| vec3::dot(uf[data.faces[i] - n_int], data.normal[i]);
            let per_face = |v: Vec<f64>| -> Box<dyn Bc> {
                Box::new(FixedValue(Profile::PerFace(
                    v.into_iter().map(|x| [x, 0.0, 0.0]).collect(),
                )))
            };
            match &mut e.spec {
                Spec::MassFlow {
                    m,
                    source,
                    dir_shape,
                    w_sum,
                    swirl,
                } => {
                    let m_now = match source {
                        Some((faces, f)) => *f * (rho * faces.iter().map(|&q| phi[q]).sum::<f64>()).max(0.0),
                        None => *m,
                    };
                    let c = if *w_sum > 0.0 {
                        m_now.abs() / (rho * *w_sum)
                    } else {
                        0.0
                    };
                    let vel: Vec<Vec3> = (0..nf)
                        .map(|i| [0, 1, 2].map(|k| c * dir_shape[i][k] + swirl[i][k]))
                        .collect();
                    let _ = u.set_bc(geom, &data.name, Box::new(FixedValue(Profile::PerFace(vel))));
                    let _ = p.set_bc(geom, &data.name, Box::new(ZeroGradient));
                }
                Spec::Outflow { weight, prevent } => {
                    let target_flux = inflow_total * *weight / weight_sum / rho;
                    let u_n: Vec<f64> = (0..nf)
                        .map(|i| vec3::dot(u.values[data.owner[i]], data.normal[i]))
                        .collect();
                    let q = outflow_speeds(&u_n, &data.area, target_flux, *prevent);
                    let vel: Vec<Vec3> = (0..nf)
                        .map(|i| {
                            let ut = tangent_part(u.values[data.owner[i]], data.normal[i]);
                            [0, 1, 2].map(|k| q[i] * data.normal[i][k] + ut[k])
                        })
                        .collect();
                    let _ = u.set_bc(geom, &data.name, Box::new(FixedValue(Profile::PerFace(vel))));
                    let _ = p.set_bc(geom, &data.name, Box::new(ZeroGradient));
                }
                Spec::Pressure {
                    p_set,
                    normal,
                    total,
                    prevent,
                    average,
                    mask,
                } => {
                    for i in 0..nf {
                        mask[i] = if phi[data.faces[i]] < -eps {
                            true
                        } else if phi[data.faces[i]] > eps {
                            false
                        } else {
                            p.values[data.owner[i]] < *p_set
                        };
                    }
                    let base: Vec<f64> = if *average {
                        let pc: Vec<f64> = data.owner.iter().map(|&c| p.values[c]).collect();
                        area_mean_shift(&pc, &data.area, *p_set)
                    } else {
                        vec![*p_set; nf]
                    };
                    let vals: Vec<f64> = (0..nf)
                        .map(|i| {
                            if mask[i] && *total {
                                base[i] - 0.5 * speed2(i)
                            } else {
                                base[i]
                            }
                        })
                        .collect();
                    let on: Box<dyn Bc> = if *prevent {
                        Box::new(FixedValue::vector([0.0; 3]))
                    } else if *normal {
                        Box::new(Directed {
                            dir: None,
                            tangential: Profile::Uniform([0.0; 3]),
                        })
                    } else {
                        Box::new(ZeroGradient)
                    };
                    let _ = u.set_bc(
                        geom,
                        &data.name,
                        Box::new(Switched {
                            mask: mask.clone(),
                            on,
                            off: Box::new(ZeroGradient),
                        }),
                    );
                    let _ = p.set_bc(geom, &data.name, per_face(vals));
                }
                Spec::Vent { p_ext, k, p_old } => {
                    let new: Vec<f64> = (0..nf)
                        .map(|i| {
                            let v = un(i);
                            *p_ext + 0.5 * *k * v * v.abs()
                        })
                        .collect();
                    let vals = relax_lag(p_old, new);
                    let _ = u.set_bc(geom, &data.name, Box::new(ZeroGradient));
                    let _ = p.set_bc(geom, &data.name, per_face(vals));
                }
                Spec::TotalIn { p0, dir, swirl, p_old } => {
                    let new: Vec<f64> = (0..nf)
                        .map(|i| {
                            if phi[data.faces[i]] < eps {
                                *p0 - 0.5 * speed2(i)
                            } else {
                                *p0
                            }
                        })
                        .collect();
                    let vals = relax_lag(p_old, new);
                    let _ = p.set_bc(geom, &data.name, per_face(vals));
                    // velocity: set at the first refresh (the swirl and direction do not change)
                    if first {
                        let _ = u.set_bc(
                            geom,
                            &data.name,
                            Box::new(Directed {
                                dir: *dir,
                                tangential: Profile::PerFace(swirl.clone()),
                            }),
                        );
                    }
                }
                Spec::Radial {
                    axis,
                    origin,
                    hub,
                    p_ref,
                    frame_rate,
                    p_old,
                } => {
                    let r: Vec<f64> = data.centre.iter().map(|x| radius(*x, *origin, *axis)).collect();
                    // absolute swirl w_theta + Omega r when the velocity is solved in a rotating frame (ADR 0078); 0 leaves w unchanged
                    let w: Vec<f64> = (0..nf)
                        .map(|i| {
                            let x = vec3::sub(data.centre[i], *origin);
                            let ax = vec3::dot(x, *axis);
                            let rad = vec3::sub(x, vec3::scale(*axis, ax));
                            let th = vec3::cross(*axis, unit(rad));
                            vec3::dot(uf[data.faces[i] - n_int], th) + *frame_rate * r[i]
                        })
                        .collect();
                    let reference = if *hub {
                        RadialReference::Hub(*p_ref)
                    } else {
                        RadialReference::AreaMean(*p_ref)
                    };
                    let new = radial_equilibrium(&r, &w, &data.area, reference);
                    let vals = relax_lag(p_old, new);
                    let _ = u.set_bc(geom, &data.name, Box::new(ZeroGradient));
                    let _ = p.set_bc(geom, &data.name, per_face(vals));
                }
                Spec::Target { target, p_t, updater } => {
                    if !first {
                        let area: f64 = data.area.iter().sum();
                        *p_t = updater.step(*p_t, rho * flux, *target, rho, area);
                    }
                    let _ = u.set_bc(geom, &data.name, Box::new(ZeroGradient));
                    let _ = p.set_bc(geom, &data.name, Box::new(FixedValue::scalar(*p_t)));
                }
            }
        }
    }
}

/// Relax a lagged per-face pressure with the previous values (first call: the new values).
fn relax_lag(old: &mut Option<Vec<f64>>, new: Vec<f64>) -> Vec<f64> {
    let out = match old {
        Some(o) => o
            .iter()
            .zip(&new)
            .map(|(a, b)| (1.0 - LAG_WEIGHT) * a + LAG_WEIGHT * b)
            .collect(),
        None => new,
    };
    *old = Some(out.clone());
    out
}

/// Distance of `x` from the axis line through `origin` with unit direction `axis`.
fn radius(x: Vec3, origin: Vec3, axis: Vec3) -> f64 {
    let v = vec3::sub(x, origin);
    let a = vec3::dot(v, axis);
    vec3::norm(vec3::sub(v, vec3::scale(axis, a)))
}
