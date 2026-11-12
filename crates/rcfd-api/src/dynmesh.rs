//! The `dynamic_mesh` section of the case tree (F-063, F-229, ADR 0033 D8): a prescribed rigid motion or a six-degree-of-
//! freedom body drives the moving patches of an incompressible PISO/PIMPLE run; the interior mesh follows rigidly or by
//! Laplacian / inverse-distance / inverse-volume (F-290, ADR 0203) smoothing. F-259 (ADR 0191): a prescribed motion also
//! moves the mesh of a transient
//! compressible run (the ALE step of ADR 0090), and `deformation = "layering"` runs ADR 0090's planar layer stack of a
//! piston. ADR 0257 (the rest of F-259): characteristic conditions on moving faces (F-258, ADR 0207), the opt-in mesh-speed
//! step `[dynamic_mesh.compressible] time_step`, and the extruded layer stack of `[mesh.box_3d] section` (F-257, ADR 0213)
//! for prism layers and for layering on the piso / pimple path.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_i, get_s, get_v};
use rcfd_core::expr::{Compiled, Expr};
use rcfd_core::settings::Schema;
use rcfd_core::units::Dimension;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions, set_rest_conditions};
use rcfd_fv::compressible::{CompressibleSolver, Cons};
use rcfd_fv::pimple::{PimpleSolver, StepInfo};
use rcfd_fv::sixdof::{patch_force_moment, quat_from_axis_angle, Body, BodySpec, BodyState, Integrator, Rotation};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::FvGeometry;
use rcfd_linalg::SolveControl;
use rcfd_mesh::layering::{remap, remap_face_flux, ExtrudedStack, LayerChange, LayerStack, Section};
use rcfd_mesh::motion::{mat_vec, moved_mesh, patch_points, rotation_matrix};
use rcfd_mesh::Mesh;
use serde_json::Value;
use std::sync::Arc;

/// Removal threshold of the layer stack, ADR 0090 D3's registered value (ADR 0191 W2, Q1: no key).
pub const LAYER_REMOVAL_RATIO: f64 = 0.4;
/// Addition threshold of the layer stack, ADR 0090 D3's registered value (ADR 0191 W2, Q1: no key).
pub const LAYER_ADDITION_RATIO: f64 = 1.6;

/// Path of the cross-section of `box_3d` (ADR 0257 D2).
pub const SECTION: &str = "mesh.box_3d.section";
/// Path of the step choice of a moving compressible run (ADR 0257 D4).
pub const TIME_STEP: &str = "dynamic_mesh.compressible.time_step";

/// The extruded layer stack of a `box_3d` case whose `mesh.box_3d.section` is `rectangles` or `triangles` (ADR 0257 D2);
/// `None` for `box`, the uniform box of ADR 0191.
pub(crate) fn extruded_stack(case: &Case) -> Result<Option<ExtrudedStack>, Vec<Diagnostic>> {
    let section = get_s(case, SECTION);
    if section != "rectangles" && section != "triangles" {
        return Ok(None);
    }
    let n = get_i(case, "mesh.nx");
    if n < 2 {
        return Err(vec![d(
            "mesh.nx",
            format!("mesh.box_3d.section = \"{section}\" needs at least 2 layers (mesh.nx >= 2)"),
        )]);
    }
    let cross = [
        get_i(case, "mesh.ny").max(1) as usize,
        get_i(case, "mesh.nz").max(1) as usize,
    ];
    let size = [get_f(case, "mesh.ly"), get_f(case, "mesh.lz")];
    let sec = if section == "triangles" {
        Section::rect_triangles(cross, size)
    } else {
        Section::rect(cross, size)
    }
    .map_err(|e| vec![d(SECTION, e.to_string())])?;
    ExtrudedStack::uniform(
        sec,
        n as usize,
        get_f(case, "mesh.lx"),
        LAYER_REMOVAL_RATIO,
        LAYER_ADDITION_RATIO,
    )
    .map(Some)
    .map_err(|e| vec![d("mesh.nx", e.to_string())])
}

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
            Schema::enumeration("laplacian", &["laplacian", "inverse_distance", "inverse_volume"])
                .doc("Diffusivity of the point-displacement equation: 1 (laplacian), 1/l^exponent with l the distance to the nearest moving face (inverse_distance), or 1/V^exponent with V the mean initial volume of the two cells of a face, so small cells deform less (inverse_volume, ADR 0090 D4, F-290)"),
        )
        .field("exponent", Schema::float(1.0).range(0.0, 10.0).doc("Exponent of the inverse-distance or inverse-volume diffusivity"))
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
        .doc("Dynamic mesh for incompressible piso / pimple runs (ADR 0033): prescribed rigid motion or a six-degree-of-freedom body; implicit Euler only, no turbulence / energy / scalars / particles / sources / restart. Transient compressible runs (ADR 0191): prescribed motion with rigid, smoothing or layering deformation")
        .field("enabled", Schema::boolean(false).doc("Move the mesh during the run"))
        .field("motion", Schema::enumeration("prescribed", &["prescribed", "six_dof"]).doc("Where the motion of the moving patches comes from"))
        .field(
            "deformation",
            Schema::enumeration("rigid", &["rigid", "smoothing", "layering"])
                .doc("rigid: every point follows the motion; smoothing: the moving patches follow it and the interior points are smoothed (fixed_patches stay put, all other patches slide in their plane); layering: the piston xmax of a box_3d mesh moves by displacement_expression_x, only the layer next to it deforms, and a layer is removed below 0.4 or added above 1.6 times lx / nx (ADR 0090 D3; transient compressible runs only, F-259, ADR 0191)"),
        )
        .field("moving_patches", lst("Patches that carry the motion (required, except for a rigid prescribed motion)"))
        .field("follow_patches", lst("Patches whose velocity condition is the mesh velocity (empty = the moving patches); must have the default velocity condition"))
        .field("fixed_patches", lst("Smoothing only: patches whose points do not move"))
        .field(
            "rest_patches",
            lst("Rigid deformation only: walls at rest on the moving mesh (ADR 0183 P-B), face velocity (m_f/|S_f|) n_f with no tangential part, so no relative flux crosses them; not in moving_patches or follow_patches; velocity \"fixed\" with value 0")
                .hash_omit_default(),
        )
        .field("displacement_expression_x", expr("Prescribed displacement of the x direction, expression of t with unit m (empty = 0)"))
        .field("displacement_expression_y", expr("As displacement_expression_x, y direction"))
        .field("displacement_expression_z", expr("As displacement_expression_x, z direction"))
        .field("rotation_angle_expression", expr("Prescribed rotation angle (dimensionless, radian) as an expression of t (empty = 0)"))
        .field("rotation_origin", nums("Point on the rotation axis (empty = the origin)"))
        .field("rotation_axis", nums("Rotation axis (empty = z)"))
        .field("smoothing", smoothing)
        .field("six_dof", six)
        // F-259, ADR 0257 D4: an optional table, absent from every case that does not write it
        .field(
            "compressible",
            Schema::object()
                .optional()
                .doc("Options of a moving mesh with solver.algorithm = \"compressible\" (ADR 0257). Absent = the step of ADR 0191")
                .field(
                    "time_step",
                    Schema::enumeration("absolute", &["absolute", "mesh_speed"])
                        .doc("Stable step of a moving compressible run: absolute = from |u.n| + c (default, ADR 0191 W4); mesh_speed = from |u.n - w_n| + c with the normal mesh speed w_n of the last step (ADR 0207 M1, M2; the absolute step on the first step and on the step after a layer event)"),
                ),
        )
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
    // ADR 0257 D5 N2: the cross-section belongs to the generator box_3d (checked with or without a moving mesh)
    let section = get_s(case, SECTION);
    if !section.is_empty()
        && section != "box"
        && (get_s(case, "mesh.source") == "file" || get_s(case, "mesh.generator") != "box_3d")
    {
        out.push(d(
            SECTION,
            format!("section = \"{section}\" belongs to mesh.generator = \"box_3d\"; remove the table [mesh.box_3d] or use that generator"),
        ));
    }
    if !enabled(case) {
        return out;
    }
    let motion = get_s(case, "dynamic_mesh.motion");
    let rigid = get_s(case, "dynamic_mesh.deformation") == "rigid";
    // F-259, ADR 0191 W5: the compressible solver and the layer stack branch off the incompressible checks
    let compressible = crate::comp::enabled(case);
    let layering = get_s(case, "dynamic_mesh.deformation") == "layering";
    // ADR 0183 P-B: a rigid prescribed motion moves every point and may have no moving (following) patch
    if strings(case, "dynamic_mesh.moving_patches").is_empty() && !(rigid && motion == "prescribed") {
        out.push(d("dynamic_mesh.moving_patches", "name at least one moving patch"));
    }
    let rest = !strings(case, "dynamic_mesh.rest_patches").is_empty();
    if rest && compressible {
        // ADR 0191 E4
        out.push(d(
            "dynamic_mesh.rest_patches",
            "walls at rest belong to piso / pimple; with the compressible solver a slip wall that moves with the mesh carries the mesh speed itself (ADR 0090 D2, ADR 0191)",
        ));
    } else if rest && !rigid {
        out.push(d(
            "dynamic_mesh.rest_patches",
            "walls at rest are available with deformation = \"rigid\" only (ADR 0183 P-B)",
        ));
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
    if compressible {
        // ADR 0191 W5: the compressible checks (turbulence and energy are refused at their own keys by comp::check)
        check_compressible(case, &mut out);
    } else {
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
        // ADR 0257 D5 N5
        if get_s(case, TIME_STEP) == "mesh_speed" {
            out.push(d(
                TIME_STEP,
                "the mesh-speed step is the explicit step of the compressible solver (solver.algorithm = \"compressible\"); piso / pimple take solver.dt",
            ));
        }
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
    if layering {
        check_layering(case, compressible, &mut out);
    }
    out
}

/// ADR 0191 W5 E1 to E3: a moving mesh with the compressible solver (E4 is checked with `rest_patches` above).
fn check_compressible(case: &Case, out: &mut Vec<Diagnostic>) {
    if get_s(case, "compressible.mode") == "steady" {
        out.push(d(
            "dynamic_mesh.enabled",
            "a moving mesh needs compressible.mode = \"transient\" (ADR 0191)",
        ));
    }
    if get_s(case, "dynamic_mesh.motion") == "six_dof" {
        out.push(d(
            "dynamic_mesh.motion",
            "six-degree-of-freedom motion is not available with the compressible solver (F-244); use motion = \"prescribed\"",
        ));
    }
    if !strings(case, "dynamic_mesh.follow_patches").is_empty() {
        out.push(d(
            "dynamic_mesh.follow_patches",
            "following patches belong to piso / pimple; with the compressible solver a slip wall that moves with the mesh carries the mesh speed itself (ADR 0090 D2)",
        ));
    }
    // E5 of ADR 0191 (a characteristic condition on a patch that can move) is lifted by ADR 0257 D1: `step_ale` evaluates
    // those conditions in the frame of the moving face since F-258 (ADR 0207 M3)
}

/// ADR 0191 W5 E6 to E12 and ADR 0257 D5 N4: `deformation = "layering"`.
fn check_layering(case: &Case, compressible: bool, out: &mut Vec<Diagnostic>) {
    if !compressible {
        if !matches!(get_s(case, "solver.algorithm").as_str(), "piso" | "pimple") {
            out.push(d(
                "dynamic_mesh.deformation",
                "layering is available with solver.algorithm = \"compressible\", \"piso\" or \"pimple\"",
            ));
        } else if !matches!(get_s(case, SECTION).as_str(), "rectangles" | "triangles") {
            out.push(d(
                "dynamic_mesh.deformation",
                "layering with piso / pimple needs the extruded layer stack (F-257, ADR 0213): set [mesh.box_3d] section = \"rectangles\" or \"triangles\"",
            ));
        }
        if get_s(case, "dynamic_mesh.motion") == "six_dof" {
            out.push(d(
                "dynamic_mesh.motion",
                "layering moves the piston by displacement_expression_x: use motion = \"prescribed\"",
            ));
        }
    }
    if get_s(case, "mesh.source") == "file" {
        out.push(d(
            "mesh.source",
            "layering needs the generated mesh mesh.generator = \"box_3d\" (ADR 0191 W3)",
        ));
    } else if get_s(case, "mesh.generator") != "box_3d" {
        out.push(d(
            "mesh.generator",
            "layering needs mesh.generator = \"box_3d\": a uniform stack of nx layers along x whose last plane, the patch xmax, is the piston (ADR 0191 W3)",
        ));
    }
    let moving = strings(case, "dynamic_mesh.moving_patches");
    if !moving.is_empty() && moving != ["xmax"] {
        out.push(d(
            "dynamic_mesh.moving_patches",
            "layering moves one piston, the patch xmax: moving_patches = [\"xmax\"]",
        ));
    }
    for k in [
        "displacement_expression_y",
        "displacement_expression_z",
        "rotation_angle_expression",
    ] {
        let key = format!("dynamic_mesh.{k}");
        if !get_s(case, &key).trim().is_empty() {
            out.push(d(
                &key,
                "layering moves the piston along x only (displacement_expression_x)",
            ));
        }
    }
    if get_i(case, "mesh.nx") < 2 {
        out.push(d("mesh.nx", "layering needs at least 2 layers (mesh.nx >= 2)"));
    }
    if !strings(case, "dynamic_mesh.fixed_patches").is_empty() {
        out.push(d(
            "dynamic_mesh.fixed_patches",
            "fixed_patches belongs to deformation = \"smoothing\"; under layering only the layer next to the piston deforms",
        ));
    }
    if crate::mesh_transform::rotation(case).is_some() {
        out.push(d(
            crate::mesh_transform::ANGLE,
            "layering stacks the layers along x; a rotated mesh (mesh.transform) is not available with it",
        ));
    }
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

/// The two layer stacks of the library behind one piston: ADR 0090's box stack and ADR 0213's extruded stack.
enum Stack {
    Box(LayerStack),
    Extruded(ExtrudedStack),
}

impl Stack {
    fn set_piston(&mut self, x: f64) -> Result<(), String> {
        match self {
            Stack::Box(s) => s.set_piston(x),
            Stack::Extruded(s) => s.set_piston(x),
        }
        .map_err(|e| e.to_string())
    }
    fn mesh(&self) -> Result<Mesh, String> {
        match self {
            Stack::Box(s) => s.mesh(),
            Stack::Extruded(s) => s.mesh(),
        }
        .map_err(|e| e.to_string())
    }
    fn adjust(&mut self) -> Option<LayerChange> {
        match self {
            Stack::Box(s) => s.adjust(),
            Stack::Extruded(s) => s.adjust(),
        }
    }
}

/// The layer stack of `deformation = "layering"` (ADR 0191 W2, ADR 0257 D2): a planar stack along x with the piston `xmax`.
struct Layers {
    stack: Stack,
    /// Initial piston position (`mesh.lx`); the piston is at `x0 + d_x(t)`.
    x0: f64,
}

impl Layers {
    /// The stack of a `box_3d` case, whose mesh must be the case's mesh bit for bit (ADR 0191 W3).
    fn from_case(case: &Case, mesh: &Mesh) -> Result<Self, Vec<Diagnostic>> {
        let n = get_i(case, "mesh.nx");
        if n < 2 {
            return Err(vec![d("mesh.nx", "layering needs at least 2 layers (mesh.nx >= 2)")]);
        }
        let x0 = get_f(case, "mesh.lx");
        let stack = match extruded_stack(case)? {
            Some(st) => Stack::Extruded(st),
            None => Stack::Box(Self::box_stack(case, n as usize, x0)?),
        };
        let m = stack.mesh().map_err(|e| vec![d("mesh.generator", e)])?;
        Self::same_mesh(&m, mesh)?;
        Ok(Layers { stack, x0 })
    }

    /// ADR 0090's box stack with ADR 0191's thresholds.
    fn box_stack(case: &Case, n: usize, x0: f64) -> Result<LayerStack, Vec<Diagnostic>> {
        LayerStack::uniform(
            n,
            x0,
            [
                get_i(case, "mesh.ny").max(1) as usize,
                get_i(case, "mesh.nz").max(1) as usize,
            ],
            [get_f(case, "mesh.ly"), get_f(case, "mesh.lz")],
            LAYER_REMOVAL_RATIO,
            LAYER_ADDITION_RATIO,
        )
        .map_err(|e| vec![d("mesh.nx", e.to_string())])
    }

    /// The stack's mesh `m` is the case's mesh, bit for bit (ADR 0191 W3).
    fn same_mesh(m: &Mesh, mesh: &Mesh) -> Result<(), Vec<Diagnostic>> {
        let names = |m: &Mesh| m.patches().iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        let same = m.content_hash() == mesh.content_hash()
            && names(m) == names(mesh)
            && m.points().len() == mesh.points().len()
            && m.points()
                .iter()
                .zip(mesh.points())
                .all(|(a, b)| (0..3).all(|k| a[k].to_bits() == b[k].to_bits()));
        if !same {
            return Err(vec![d(
                "mesh.generator",
                "layering needs the mesh of mesh.generator = \"box_3d\" unchanged (no mesh.transform): the case's mesh is not the layer stack's (ADR 0191 W3)",
            )]);
        }
        Ok(())
    }
}

/// Run-time state of the dynamic mesh.
pub struct DynMesh {
    mesh0: Mesh,
    geom0: FvGeometry,
    drive: Drive,
    rigid: bool,
    moving: Vec<String>,
    follow: Vec<String>,
    rest: Vec<String>,
    classes: Vec<PatchClass>,
    method: SmoothingMethod,
    control: SolveControl,
    integrator: Integrator,
    rho: f64,
    nu: f64,
    /// The layer stack of `deformation = "layering"` (ADR 0191; with piso / pimple the extruded stack, ADR 0257 D3).
    layers: Option<Layers>,
    /// ADR 0257 D4: the moving compressible step takes the mesh-speed step `max_dt_ale()`.
    mesh_speed_step: bool,
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
        let rest = list("rest_patches");
        // F-259, ADR 0191 W5: the compressible solver has no body, no following and no resting walls (also static checks)
        let compressible = crate::comp::enabled(case);
        let layering = get_s(case, "dynamic_mesh.deformation") == "layering";
        if compressible {
            if get_s(case, "dynamic_mesh.motion") == "six_dof" {
                errs.push(d(
                    "dynamic_mesh.motion",
                    "six-degree-of-freedom motion is not available with the compressible solver (F-244)",
                ));
            }
            if !follow.is_empty() {
                errs.push(d(
                    "dynamic_mesh.follow_patches",
                    "following patches belong to piso / pimple",
                ));
            }
            if !rest.is_empty() {
                errs.push(d("dynamic_mesh.rest_patches", "walls at rest belong to piso / pimple"));
            }
        } else if layering && !matches!(get_s(case, SECTION).as_str(), "rectangles" | "triangles") {
            errs.push(d(
                "dynamic_mesh.deformation",
                "layering with piso / pimple needs [mesh.box_3d] section = \"rectangles\" or \"triangles\" (F-257, ADR 0213)",
            ));
        } else if layering && get_s(case, "dynamic_mesh.motion") == "six_dof" {
            errs.push(d(
                "dynamic_mesh.motion",
                "layering moves the piston by displacement_expression_x: use motion = \"prescribed\"",
            ));
        }
        if follow.is_empty() && !compressible {
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
        // ADR 0183 P-B: walls at rest on the moving mesh
        for n in &rest {
            if moving.contains(n) || follow.contains(n) {
                errs.push(d(
                    "dynamic_mesh.rest_patches",
                    format!("patch '{n}' is also a moving or following patch; a wall at rest does not follow the mesh"),
                ));
            }
            if !known.contains(n) {
                continue;
            }
            let b = format!("boundaries.{n}");
            if get_s(case, &format!("{b}.velocity")) != "fixed" {
                errs.push(d(
                    &format!("{b}.velocity"),
                    "a wall at rest takes its velocity from the mesh flux; leave its velocity condition at \"fixed\" with velocity_value 0",
                ));
            }
            if vec3::norm(get_v(case, &format!("{b}.velocity_value"), vec3::ZERO)) != 0.0 {
                errs.push(d(
                    &format!("{b}.velocity_value"),
                    "a wall at rest has velocity 0 (its face velocity on the moving mesh is the normal part that carries the swept volume)",
                ));
            }
            for k in ["x", "y", "z"] {
                let key = format!("{b}.velocity_expression_{k}");
                if case.tree.get(&key).is_ok() && !get_s(case, &key).trim().is_empty() {
                    errs.push(d(&key, "a wall at rest has no velocity expression"));
                }
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
        // ADR 0203 W2: the stiffness of the initial mesh, as the displacement is solved from it every step
        let method = match get_s(case, "dynamic_mesh.smoothing.method").as_str() {
            "inverse_distance" => SmoothingMethod::InverseDistance(exponent),
            "inverse_volume" => SmoothingMethod::InverseVolume(exponent),
            _ => SmoothingMethod::Laplacian,
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
        // ADR 0191 W2, W3: the layer stack, whose mesh is the case's
        let layers = if layering {
            if moving != ["xmax"] {
                return Err(vec![d(
                    "dynamic_mesh.moving_patches",
                    "layering moves one piston, the patch xmax: moving_patches = [\"xmax\"]",
                )]);
            }
            Some(Layers::from_case(case, mesh)?)
        } else {
            None
        };
        Ok(Some(DynMesh {
            mesh0: (*geom.mesh).clone(),
            geom0: FvGeometry::new(geom.mesh.clone()).map_err(|e| vec![d("dynamic_mesh", e.to_string())])?,
            drive,
            rigid,
            moving,
            follow,
            rest,
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
            layers,
            mesh_speed_step: compressible && get_s(case, TIME_STEP) == "mesh_speed",
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

    /// The point positions at `t_new` of a rigid or smoothed motion: the pose of the initial points, or the initial points
    /// plus the smoothed displacement with the moving patches prescribed (ADR 0033 D4, D5; one function since ADR 0191 W4,
    /// its operations unchanged).
    fn new_points(&self, t_new: f64) -> Result<Vec<Vec3>, String> {
        let p0 = self.mesh0.points();
        Ok(if self.rigid {
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
        })
    }

    /// The prescribed x displacement at `t` (the piston of a layer stack sits at `lx` plus it).
    fn piston_displacement(&self, t: f64) -> f64 {
        match &self.drive {
            Drive::Prescribed { disp, .. } => disp[0].as_ref().map_or(0.0, |c| c.eval(&[0.0, 0.0, 0.0, t])),
            Drive::Body(..) => 0.0,
        }
    }

    /// ADR 0257 D4: true when `[dynamic_mesh.compressible] time_step = "mesh_speed"` asks for the mesh-speed step.
    pub(crate) fn mesh_speed_step(&self) -> bool {
        self.mesh_speed_step
    }

    /// The geometry at `t_new` of a compressible run's mesh (ADR 0191 W4): the moved points of a rigid or smoothed motion
    /// on the initial topology, or the layer stack with its piston at `lx + d_x(t_new)`.
    pub(crate) fn compressible_geometry(&mut self, t_new: f64) -> Result<FvGeometry, String> {
        let dx = self.piston_displacement(t_new);
        let mesh = if let Some(l) = self.layers.as_mut() {
            l.stack.set_piston(l.x0 + dx)?;
            l.stack.mesh()?
        } else {
            moved_mesh(&self.mesh0, self.new_points(t_new)?).map_err(|e| e.to_string())?
        };
        FvGeometry::new(Arc::new(mesh)).map_err(|e| e.to_string())
    }

    /// The layer change after a compressible step (ADR 0191 W4 step 3, ADR 0090 D3): `None` without layering or when the
    /// piston layer is within the thresholds; else the geometry of the new stack and the state remapped onto it.
    pub(crate) fn layer_event(&mut self, s: &CompressibleSolver) -> Result<Option<(FvGeometry, Vec<Cons>)>, String> {
        let Some(l) = self.layers.as_mut() else {
            return Ok(None);
        };
        let Some(change) = l.stack.adjust() else {
            return Ok(None);
        };
        let mesh = l.stack.mesh()?;
        let g = FvGeometry::new(Arc::new(mesh)).map_err(|e| e.to_string())?;
        let u = remap(&change, &s.u, s.geom.mesh.cell_volumes(), g.mesh.cell_volumes());
        Ok(Some((g, u)))
    }

    /// One step of a layered piston on the piso / pimple path (ADR 0257 D3, the loop of ADR 0213): the piston at its
    /// position at the end of the step, the ALE step onto the stack's points, then the layer check; on an event the new
    /// mesh, the volume-weighted remap of `u` and `p`, the face-flux map of ADR 0213 M3 and `replace_mesh`.
    fn step_layered(&mut self, s: &mut PimpleSolver, dt: f64) -> Result<StepInfo, String> {
        let dx = self.piston_displacement(s.time + dt);
        let Some(Layers {
            stack: Stack::Extruded(stack),
            x0,
        }) = self.layers.as_mut()
        else {
            return Err("layering with piso / pimple needs the extruded layer stack (mesh.box_3d.section)".into());
        };
        stack.set_piston(*x0 + dx).map_err(|e| e.to_string())?;
        let st = move_mesh(&s.geom, stack.points(), dt)?;
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &st, &old, &self.follow, dt)?;
        let info = s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls)?;
        if let Some(change) = stack.adjust() {
            let old = s.geom.clone();
            let mesh = stack.mesh().map_err(|e| e.to_string())?;
            let sources = stack
                .face_sources(&change, &old.mesh, &mesh)
                .map_err(|e| e.to_string())?;
            let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).map_err(|e| e.to_string())?);
            let (vo, vn) = (old.mesh.cell_volumes(), geom.mesh.cell_volumes());
            let u = remap(&change, &s.u.values, vo, vn);
            let p1: Vec<[f64; 1]> = s.p.values.iter().map(|x| [*x]).collect();
            let p: Vec<f64> = remap(&change, &p1, vo, vn).into_iter().map(|x| x[0]).collect();
            let phi = remap_face_flux(&sources, &s.phi, &geom.mesh);
            s.replace_mesh(geom, u, p, phi)?;
        }
        Ok(info)
    }

    /// Advance the flow one step with the mesh moved to its position at the end of the step.
    pub fn step(&mut self, s: &mut PimpleSolver, dt: f64) -> Result<StepInfo, String> {
        if self.layers.is_some() {
            return self.step_layered(s, dt);
        }
        let t_new = s.time + dt;
        if let Drive::Body(b, patches) = &mut self.drive {
            let fm = patch_force_moment(&s.geom, &s.u, &s.p, self.rho, self.nu, patches, b.state.x)?;
            b.advance(dt, self.integrator, fm)?;
        }
        let new_points = self.new_points(t_new)?;
        let st = move_mesh(&s.geom, new_points, dt)?;
        let old = s.geom.mesh.clone();
        let mut walls = set_follow_conditions(&mut s.u, &st, &old, &self.follow, dt)?;
        if !self.rest.is_empty() {
            // ADR 0183 P-B: walls at rest (disjoint from the following patches)
            let rest = set_rest_conditions(&mut s.u, &st, &self.rest, dt)?;
            for (w, r) in walls.iter_mut().zip(rest) {
                if r.is_finite() {
                    *w = r;
                }
            }
        }
        s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls)
    }

    /// True for `deformation = "layering"`.
    pub(crate) fn layered(&self) -> bool {
        self.layers.is_some()
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
