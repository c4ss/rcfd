//! The `dynamic_mesh` section of the case tree (F-063, F-229, ADR 0033 D8): a prescribed rigid motion or a six-degree-of-
//! freedom body drives the moving patches of an incompressible PISO/PIMPLE run; the interior mesh follows rigidly or by
//! Laplacian / inverse-distance smoothing.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_i, get_s, get_v};
use rcfd_core::expr::{Compiled, Expr};
use rcfd_core::settings::Schema;
use rcfd_core::units::Dimension;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::pimple::{PimpleSolver, StepInfo};
use rcfd_fv::sixdof::{patch_force_moment, quat_from_axis_angle, Body, BodySpec, BodyState, Integrator, Rotation};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::FvGeometry;
use rcfd_linalg::SolveControl;
use rcfd_mesh::motion::{mat_vec, patch_points, rotation_matrix};
use rcfd_mesh::Mesh;
use serde_json::Value;

fn d(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// Schema of the `dynamic_mesh` section (all keys inert while `enabled = false`).
pub fn schema() -> Schema {
    let lst = |doc: &'static str| Schema::list(Schema::string("")).doc(doc);
    let nums = |doc: &'static str| Schema::list(Schema::float(0.0)).doc(doc);
    let expr = |doc: &'static str| Schema::string("").doc(doc);
    let smoothing = Schema::object()
        .doc("Interior mesh smoothing (deformation = smoothing)")
        .field(
            "method",
            Schema::enumeration("laplacian", &["laplacian", "inverse_distance"])
                .doc("Diffusivity of the point-displacement equation: 1 (laplacian) or 1/l^exponent with l the distance to the nearest moving face"),
        )
        .field("exponent", Schema::float(1.0).range(0.0, 10.0).doc("Exponent of the inverse-distance diffusivity"))
        .field("substeps", Schema::int(1).range(1.0, 1000.0).doc("Reserved; only 1 is implemented (the displacement from the initial mesh is solved once per step)"))
        .field("tolerance", Schema::float(1e-12).range(0.0, 1.0).doc("Relative tolerance of the displacement solve"));
    let six = Schema::object()
        .doc("Rigid body of motion = six_dof; the flow force on force_patches drives it (loose explicit coupling)")
        .field("mass", Schema::float(0.0).doc("Mass [kg] (> 0 required; 0 = unset)"))
        .field("inertia", nums("Principal moments [Ixx, Iyy, Izz] or the six components [xx, yy, zz, xy, xz, yz] about the centre of mass, kg m^2"))
        .field("position", nums("Initial centre of mass [x, y, z], m (empty = the origin)"))
        .field("velocity", nums("Initial velocity [m/s] (empty = at rest)"))
        .field("angular_velocity", nums("Initial angular velocity [rad/s] (empty = zero)"))
        .field("orientation_axis", nums("Axis of the initial orientation (empty = identity)"))
        .field("orientation_angle", Schema::float(0.0).doc("Angle of the initial orientation, rad"))
        .field("translation_locked", Schema::list(Schema::boolean(false)).doc("Locked world axes [x, y, z] (empty = all free)"))
        .field(
            "rotation",
            Schema::enumeration("locked", &["locked", "free", "about_axis", "hinge"])
                .doc("Rotational freedom: locked, free (Euler equations, RK4 only), about_axis (through the centre of mass, principal axis) or hinge (about axis through pivot)"),
        )
        .field("axis", nums("Rotation axis for about_axis / hinge"))
        .field("pivot", nums("A point on the hinge axis"))
        .field("stiffness", nums("Translational spring stiffness per world axis [N/m] (empty = none)"))
        .field("damping", nums("Translational damping per world axis [N s/m] (empty = none)"))
        .field("rest_position", nums("Rest position of the springs (empty = the origin)"))
        .field("torsion_stiffness", Schema::float(0.0).doc("Torsional spring stiffness, N m/rad (single-axis rotation)"))
        .field("torsion_damping", Schema::float(0.0).doc("Torsional damping, N m s/rad"))
        .field("torsion_rest_angle", Schema::float(0.0).doc("Rest angle of the torsional spring, rad"))
        .field("gravity", Schema::boolean(false).doc("Apply physics.gravity as the weight of the body"))
        .field("force_patches", lst("Patches the flow force is integrated on (empty = moving_patches)"))
        .field(
            "integrator",
            Schema::enumeration("rk4", &["rk4", "newmark"]).doc("Body integrator: rk4 or newmark (average acceleration; not for free rotation)"),
        );
    Schema::object()
        .doc("Dynamic mesh for incompressible piso / pimple runs (ADR 0033): prescribed rigid motion or a six-degree-of-freedom body; implicit Euler only, no turbulence / energy / scalars / particles / sources / restart")
        .field("enabled", Schema::boolean(false).doc("Move the mesh during the run"))
        .field("motion", Schema::enumeration("prescribed", &["prescribed", "six_dof"]).doc("Where the motion of the moving patches comes from"))
        .field(
            "deformation",
            Schema::enumeration("rigid", &["rigid", "smoothing"])
                .doc("rigid: every point follows the motion; smoothing: the moving patches follow it and the interior points are smoothed (fixed_patches stay put, all other patches slide in their plane)"),
        )
        .field("moving_patches", lst("Patches that carry the motion (required)"))
        .field("follow_patches", lst("Patches whose velocity condition is the mesh velocity (empty = the moving patches); must have the default velocity condition"))
        .field("fixed_patches", lst("Smoothing only: patches whose points do not move"))
        .field("displacement_expression_x", expr("Prescribed displacement of the x direction, expression of t with unit m (empty = 0)"))
        .field("displacement_expression_y", expr("As displacement_expression_x, y direction"))
        .field("displacement_expression_z", expr("As displacement_expression_x, z direction"))
        .field("rotation_angle_expression", expr("Prescribed rotation angle (dimensionless, radian) as an expression of t (empty = 0)"))
        .field("rotation_origin", nums("Point on the rotation axis (empty = the origin)"))
        .field("rotation_axis", nums("Rotation axis (empty = z)"))
        .field("smoothing", smoothing)
        .field("six_dof", six)
}

/// True when the section is enabled.
pub fn enabled(case: &Case) -> bool {
    get_b(case, "dynamic_mesh.enabled")
}

fn strings(case: &Case, path: &str) -> Vec<String> {
    case.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

fn floats(case: &Case, path: &str) -> Vec<f64> {
    case.tree
        .get(path)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_f64)
        .collect()
}

fn compile_t(case: &Case, path: &str, dim: Dimension) -> Result<Option<Compiled>, Diagnostic> {
    let src = get_s(case, path);
    if src.trim().is_empty() {
        return Ok(None);
    }
    let ctx = crate::exprs::spatial_context(case);
    let e = Expr::parse(&src).map_err(|e| d(path, format!("{e}")))?;
    let c = e.compile(&ctx).map_err(|e| d(path, format!("{e}")))?;
    if c.dimension() != dim {
        return Err(d(
            path,
            format!("expression has dimension '{}' but '{dim}' is required", c.dimension()),
        ));
    }
    Ok(Some(c))
}

/// Static checks of the `dynamic_mesh` section.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !enabled(case) {
        return out;
    }
    let motion = get_s(case, "dynamic_mesh.motion");
    if strings(case, "dynamic_mesh.moving_patches").is_empty() {
        out.push(d("dynamic_mesh.moving_patches", "name at least one moving patch"));
    }
    for k in ["x", "y", "z"] {
        if let Err(e) = compile_t(
            case,
            &format!("dynamic_mesh.displacement_expression_{k}"),
            Dimension::LENGTH,
        ) {
            out.push(e);
        }
    }
    let rot = compile_t(case, "dynamic_mesh.rotation_angle_expression", Dimension::NONE);
    let has_rot = match &rot {
        Ok(c) => c.is_some(),
        Err(e) => {
            out.push(e.clone());
            false
        }
    };
    let axis = floats(case, "dynamic_mesh.rotation_axis");
    if !axis.is_empty() && axis.len() != 3 {
        out.push(d("dynamic_mesh.rotation_axis", "needs three components"));
    }
    if has_rot && axis.len() == 3 && vec3::norm([axis[0], axis[1], axis[2]]) == 0.0 {
        out.push(d(
            "dynamic_mesh.rotation_axis",
            "the rotation axis must have non-zero length",
        ));
    }
    if !floats(case, "dynamic_mesh.rotation_origin").is_empty()
        && floats(case, "dynamic_mesh.rotation_origin").len() != 3
    {
        out.push(d("dynamic_mesh.rotation_origin", "needs three components"));
    }
    if get_f(case, "dynamic_mesh.smoothing.exponent") < 0.0 {
        out.push(d("dynamic_mesh.smoothing.exponent", "must not be negative"));
    }
    if get_i(case, "dynamic_mesh.smoothing.substeps") != 1 {
        out.push(d("dynamic_mesh.smoothing.substeps", "only 1 is implemented"));
    }
    if motion == "six_dof" {
        if get_f(case, "dynamic_mesh.six_dof.mass") <= 0.0 {
            out.push(d("dynamic_mesh.six_dof.mass", "the mass must be positive"));
        }
        let inertia = floats(case, "dynamic_mesh.six_dof.inertia");
        if inertia.len() != 3 && inertia.len() != 6 {
            out.push(d(
                "dynamic_mesh.six_dof.inertia",
                "needs 3 principal moments or 6 components (xx, yy, zz, xy, xz, yz)",
            ));
        } else if inertia.iter().take(3).any(|&i| i <= 0.0) {
            out.push(d(
                "dynamic_mesh.six_dof.inertia",
                "the moments of inertia must be positive",
            ));
        }
        for k in [
            "position",
            "velocity",
            "angular_velocity",
            "orientation_axis",
            "stiffness",
            "damping",
            "rest_position",
            "axis",
            "pivot",
        ] {
            let n = floats(case, &format!("dynamic_mesh.six_dof.{k}")).len();
            if n != 0 && n != 3 {
                out.push(d(&format!("dynamic_mesh.six_dof.{k}"), "needs three components"));
            }
        }
        let rotation = get_s(case, "dynamic_mesh.six_dof.rotation");
        let axis = floats(case, "dynamic_mesh.six_dof.axis");
        if matches!(rotation.as_str(), "about_axis" | "hinge")
            && (axis.len() != 3 || vec3::norm([axis[0], axis[1], axis[2]]) == 0.0)
        {
            out.push(d(
                "dynamic_mesh.six_dof.axis",
                "a non-zero axis is required for this rotation",
            ));
        }
        if rotation == "hinge" && floats(case, "dynamic_mesh.six_dof.pivot").len() != 3 {
            out.push(d("dynamic_mesh.six_dof.pivot", "a hinge needs a pivot point"));
        }
        if rotation == "free" && get_s(case, "dynamic_mesh.six_dof.integrator") == "newmark" {
            out.push(d(
                "dynamic_mesh.six_dof.integrator",
                "newmark is not available for free rotation; use rk4",
            ));
        }
    }
    let conflict = |what: &str| {
        d(
            "dynamic_mesh.enabled",
            format!("not supported together with {what} (ADR 0033 D2)"),
        )
    };
    if !matches!(get_s(case, "solver.algorithm").as_str(), "piso" | "pimple") {
        out.push(conflict("solver.algorithm other than piso / pimple"));
    }
    if get_s(case, "solver.time_scheme") != "euler" {
        out.push(conflict("solver.time_scheme other than euler"));
    }
    let tm = get_s(case, "turbulence.model");
    if !tm.is_empty() && tm != "laminar" {
        out.push(conflict("a turbulence model"));
    }
    if get_b(case, "energy.enabled") {
        out.push(conflict("the energy equation"));
    }
    if get_b(case, "statistics.enabled") {
        out.push(conflict("statistics"));
    }
    for (on, what) in [
        (crate::scalars::enabled(case), "passive scalars / species"),
        (crate::particles::enabled(case), "particles"),
        (crate::sources::enabled(case), "momentum sources"),
        (crate::rheology::enabled(case), "a rheology model"),
        (crate::restart::requested(case), "restart"),
    ] {
        if on {
            out.push(conflict(what));
        }
    }
    out
}

#[allow(clippy::large_enum_variant)]
enum Drive {
    Prescribed {
        disp: [Option<Compiled>; 3],
        angle: Option<Compiled>,
        origin: Vec3,
        axis: Vec3,
    },
    Body(Box<Body>, Vec<String>),
}

/// Run-time state of the dynamic mesh.
pub struct DynMesh {
    mesh0: Mesh,
    geom0: FvGeometry,
    drive: Drive,
    rigid: bool,
    moving: Vec<String>,
    follow: Vec<String>,
    classes: Vec<PatchClass>,
    method: SmoothingMethod,
    control: SolveControl,
    integrator: Integrator,
    rho: f64,
    nu: f64,
}

impl DynMesh {
    /// Build from the case when the section is enabled; located diagnostics otherwise.
    pub fn from_case(case: &Case, geom: &FvGeometry) -> Result<Option<DynMesh>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let mesh = &geom.mesh;
        let known: Vec<String> = mesh.patches().iter().map(|p| p.name.clone()).collect();
        let mut errs = Vec::new();
        let mut list = |key: &str| {
            let v = strings(case, &format!("dynamic_mesh.{key}"));
            for n in &v {
                if !known.contains(n) {
                    errs.push(d(
                        &format!("dynamic_mesh.{key}"),
                        format!("unknown patch '{n}' (patches: {})", known.join(", ")),
                    ));
                }
            }
            v
        };
        let moving = list("moving_patches");
        let mut follow = list("follow_patches");
        let fixed = list("fixed_patches");
        let force_patches = list("six_dof.force_patches");
        if follow.is_empty() {
            follow = moving.clone();
        }
        for n in &follow {
            let b = format!("boundaries.{n}");
            if known.contains(n) && get_s(case, &format!("{b}.velocity")) != "fixed" {
                errs.push(d(
                    &format!("{b}.velocity"),
                    "a patch that follows the mesh takes its velocity from the mesh motion; leave its velocity condition at \"fixed\" (its velocity_value only sets the flux of the initial state)",
                ));
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let rigid = get_s(case, "dynamic_mesh.deformation") == "rigid";
        let classes = mesh
            .patches()
            .iter()
            .map(|p| {
                if moving.contains(&p.name) {
                    PatchClass::Prescribed
                } else if fixed.contains(&p.name) {
                    PatchClass::Fixed
                } else {
                    PatchClass::Slip
                }
            })
            .collect();
        let exponent = get_f(case, "dynamic_mesh.smoothing.exponent");
        let method = if get_s(case, "dynamic_mesh.smoothing.method") == "inverse_distance" {
            SmoothingMethod::InverseDistance(exponent)
        } else {
            SmoothingMethod::Laplacian
        };
        let tol = get_f(case, "dynamic_mesh.smoothing.tolerance");
        let v3 = |k: &str, def: Vec3| {
            let v = floats(case, &format!("dynamic_mesh.{k}"));
            if v.len() == 3 {
                [v[0], v[1], v[2]]
            } else {
                def
            }
        };
        let drive = if get_s(case, "dynamic_mesh.motion") == "six_dof" {
            let p = |k: &str| format!("dynamic_mesh.six_dof.{k}");
            let inertia = floats(case, &p("inertia"));
            let mut spec = BodySpec::new(get_f(case, &p("mass")), [inertia[0], inertia[1], inertia[2]]);
            if inertia.len() == 6 {
                spec.inertia[0][1] = inertia[3];
                spec.inertia[1][0] = inertia[3];
                spec.inertia[0][2] = inertia[4];
                spec.inertia[2][0] = inertia[4];
                spec.inertia[1][2] = inertia[5];
                spec.inertia[2][1] = inertia[5];
            }
            let locked = case
                .tree
                .get(&p("translation_locked"))
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            for (i, l) in spec.translation_locked.iter_mut().enumerate() {
                *l = locked.get(i).and_then(Value::as_bool).unwrap_or(false);
            }
            let axis = v3("six_dof.axis", [0.0, 0.0, 1.0]);
            spec.rotation = match get_s(case, &p("rotation")).as_str() {
                "free" => Rotation::Free,
                "about_axis" => Rotation::AboutAxis { axis },
                "hinge" => Rotation::Hinge {
                    point: v3("six_dof.pivot", vec3::ZERO),
                    axis,
                },
                _ => Rotation::Locked,
            };
            if get_b(case, &p("gravity")) {
                spec.gravity = get_v(case, "physics.gravity", vec3::ZERO);
            }
            spec.stiffness = v3("six_dof.stiffness", vec3::ZERO);
            spec.damping = v3("six_dof.damping", vec3::ZERO);
            spec.rest_position = v3("six_dof.rest_position", vec3::ZERO);
            spec.torsion_stiffness = get_f(case, &p("torsion_stiffness"));
            spec.torsion_damping = get_f(case, &p("torsion_damping"));
            spec.rest_angle = get_f(case, &p("torsion_rest_angle"));
            let mut st = BodyState::at(v3("six_dof.position", vec3::ZERO));
            st.v = v3("six_dof.velocity", vec3::ZERO);
            st.w = v3("six_dof.angular_velocity", vec3::ZERO);
            let oa = v3("six_dof.orientation_axis", vec3::ZERO);
            if vec3::norm(oa) > 0.0 {
                st.q = quat_from_axis_angle(oa, get_f(case, &p("orientation_angle")));
            }
            let body = Body::new(spec, st).map_err(|e| vec![d("dynamic_mesh.six_dof", e)])?;
            let fp = if force_patches.is_empty() {
                moving.clone()
            } else {
                force_patches
            };
            Drive::Body(Box::new(body), fp)
        } else {
            let c = |k: &str| {
                compile_t(
                    case,
                    &format!("dynamic_mesh.{k}"),
                    if k.starts_with("rot") {
                        Dimension::NONE
                    } else {
                        Dimension::LENGTH
                    },
                )
            };
            let (dx, dy, dz, an) = (
                c("displacement_expression_x").map_err(|e| vec![e])?,
                c("displacement_expression_y").map_err(|e| vec![e])?,
                c("displacement_expression_z").map_err(|e| vec![e])?,
                c("rotation_angle_expression").map_err(|e| vec![e])?,
            );
            Drive::Prescribed {
                disp: [dx, dy, dz],
                angle: an,
                origin: v3("rotation_origin", vec3::ZERO),
                axis: v3("rotation_axis", [0.0, 0.0, 1.0]),
            }
        };
        // the moving patches must exist as point sets
        for n in &moving {
            if patch_points(mesh, n).is_none() {
                return Err(vec![d(
                    "dynamic_mesh.moving_patches",
                    format!("patch '{n}' has no points"),
                )]);
            }
        }
        Ok(Some(DynMesh {
            mesh0: (*geom.mesh).clone(),
            geom0: FvGeometry::new(geom.mesh.clone()).map_err(|e| vec![d("dynamic_mesh", e.to_string())])?,
            drive,
            rigid,
            moving,
            follow,
            classes,
            method,
            control: SolveControl {
                tol_abs: 1e-16,
                tol_rel: tol.max(1e-14),
                max_iter: 5000,
            },
            integrator: if get_s(case, "dynamic_mesh.six_dof.integrator") == "newmark" {
                Integrator::Newmark
            } else {
                Integrator::Rk4
            },
            rho: get_f(case, "physics.rho"),
            nu: get_f(case, "physics.nu"),
        }))
    }

    /// Position at time `t` of the point that sat at `p0` initially.
    fn map(&self, p0: Vec3, t: f64) -> Vec3 {
        match &self.drive {
            Drive::Body(b, _) => b.map_point(p0),
            Drive::Prescribed {
                disp,
                angle,
                origin,
                axis,
            } => {
                let slots = [0.0, 0.0, 0.0, t];
                let dsp = [0, 1, 2].map(|i| disp[i].as_ref().map_or(0.0, |c| c.eval(&slots)));
                let a = angle.as_ref().map_or(0.0, |c| c.eval(&slots));
                let r = rotation_matrix(*axis, a);
                vec3::add(vec3::add(*origin, mat_vec(&r, vec3::sub(p0, *origin))), dsp)
            }
        }
    }

    /// Advance the flow one step with the mesh moved to its position at the end of the step.
    pub fn step(&mut self, s: &mut PimpleSolver, dt: f64) -> Result<StepInfo, String> {
        let t_new = s.time + dt;
        if let Drive::Body(b, patches) = &mut self.drive {
            let fm = patch_force_moment(&s.geom, &s.u, &s.p, self.rho, self.nu, patches, b.state.x)?;
            b.advance(dt, self.integrator, fm)?;
        }
        let p0 = self.mesh0.points();
        let new_points: Vec<Vec3> = if self.rigid {
            p0.iter().map(|&p| self.map(p, t_new)).collect()
        } else {
            let mut presc = vec![vec3::ZERO; p0.len()];
            for n in &self.moving {
                for q in patch_points(&self.mesh0, n).unwrap_or_default() {
                    presc[q] = vec3::sub(self.map(p0[q], t_new), p0[q]);
                }
            }
            let sm = smooth_displacement(&self.geom0, &self.classes, &presc, self.method, &self.control)?;
            p0.iter()
                .zip(&sm.displacement)
                .map(|(p, dd)| vec3::add(*p, *dd))
                .collect()
        };
        let st = move_mesh(&s.geom, new_points, dt)?;
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &st, &old, &self.follow, dt)?;
        s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls)
    }

    /// The body of a six-degree-of-freedom run.
    pub fn body(&self) -> Option<&Body> {
        match &self.drive {
            Drive::Body(b, _) => Some(b),
            Drive::Prescribed { .. } => None,
        }
    }
}

/// Wrap a run-time failure into the message the run reports.
pub fn failure_message(step: usize, time: f64, e: &str) -> String {
    format!("dynamic_mesh: step {step}, t = {time:.6e}: {e}")
}
