//! The case key `[mesh.sliding]` (F-303, ADR 0637): the rotor block of `mesh.generator = "annulus_3d_cylindrical_interface"`
//! turns against the stator block at `angular_velocity` about `+z`, and the interface is rebuilt every step by the sliding
//! interface of F-250 (`rcfd_mesh::sliding`, `rcfd_fv::sliding`, `PimpleSolver::step_sliding`, ADR 0216).
//!
//! A case without the table never reaches this module beyond [`present`]: `Simulation::new` builds the frozen join as
//! before.

use super::{diag, get_f, get_s, get_v, Engine, Simulation};
use crate::case::{Case, Diagnostic};
use rcfd_core::settings::Schema;
use rcfd_core::units::Dimension;
use rcfd_fv::pimple::{PimpleSolver, StepInfo};
use rcfd_fv::sliding::{rotor_wall_conditions, sliding_step};
use rcfd_fv::FvGeometry;
use rcfd_mesh::non_conformal::{interface_non_orthogonality, CYLINDRICAL_INTERFACE_MAX_NON_ORTHOGONALITY_DEG};
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use std::sync::Arc;

/// Path of the table.
pub(crate) const TABLE: &str = "mesh.sliding";

/// The generator whose rotor and stator blocks slide (D1).
const GENERATOR: &str = "annulus_3d_cylindrical_interface";

/// Schema of `[mesh.sliding]` (D1): an optional table, absent from every case that does not write it.
pub fn schema() -> Schema {
    Schema::object()
        .optional()
        .doc("Sliding rotor-stator interface (F-303, ADR 0637): the rotor block of mesh.generator = \"annulus_3d_cylindrical_interface\" (r_in <= r <= r_interface) turns about +z through the origin against the stator block, and the interface is rebuilt every step (the sliding interface of ADR 0216: the rotor faces' swept volumes, the interface mesh flux that closes the GCL, the old flux projected onto the new faces). Transient laminar piso / pimple with time_scheme = euler, constant properties, no sources, models or expression boundary values. Patches with rotor faces: velocity fixed 0 is a wall of the rotor (rotor_walls), a non-zero uniform fixed value, symmetry and zero_gradient are kept as written (laboratory frame). Absent = the frozen join")
        .field(
            "angular_velocity",
            Schema::float(0.0)
                .dim(Dimension::ANGULAR_VELOCITY)
                .doc("Angular velocity Omega [rad/s] of the rotor block about +z (right-handed); the rotor angle after the time t is Omega t"),
        )
        .field(
            "rotor_walls",
            Schema::enumeration("rotating", &["rotating", "rest"])
                .doc("No-slip walls of the rotor block (patches with rotor faces and velocity = fixed 0): rotating = they turn with the rotor (a hub); rest = at rest in the laboratory frame. Stator faces of the same patch are at rest"),
        )
}

/// Whether the case writes the table (an absent optional table reads its defaults, so the data is looked at).
pub fn present(case: &Case) -> bool {
    case.tree.get("mesh").ok().is_some_and(|m| m.get("sliding").is_some())
}

fn at(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn refused(path: &str, what: &str) -> Diagnostic {
    at(
        path,
        format!("{what} is not available with [mesh.sliding] (the sliding interface runs transient laminar constant-property piso / pimple with time_scheme = euler, ADR 0637); remove the table [mesh.sliding] or this setting"),
    )
}

/// Static checks of the table (D5, `rcfd validate`); empty without it.
pub(crate) fn check(case: &Case) -> Vec<Diagnostic> {
    if !present(case) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let generator = get_s(case, "mesh.generator");
    if get_s(case, "mesh.source") == "file" {
        out.push(refused("mesh.source", "a mesh file"));
    } else if generator != GENERATOR {
        out.push(refused(
            "mesh.generator",
            &format!("mesh.generator = \"{generator}\" ({GENERATOR} is)"),
        ));
    }
    let alg = get_s(case, "solver.algorithm");
    if alg != "piso" && alg != "pimple" {
        out.push(refused("solver.algorithm", &format!("solver.algorithm = \"{alg}\"")));
    }
    let ts = get_s(case, "solver.time_scheme");
    if ts != "euler" {
        out.push(refused("solver.time_scheme", &format!("solver.time_scheme = \"{ts}\"")));
    }
    if super::get_b(case, "dynamic_mesh.enabled") {
        out.push(refused("dynamic_mesh.enabled", "a dynamic mesh"));
    }
    if get_s(case, "solver.time_advance") == "fractional_step" {
        out.push(refused("solver.time_advance", "the fractional step"));
    }
    if !get_f(case, "mesh.sliding.angular_velocity").is_finite() {
        out.push(at(
            "mesh.sliding.angular_velocity",
            "the angular velocity must be finite",
        ));
    }
    out
}

/// The sliding loop of a case (D3): the pair, the current joined mesh and the walls of the rotor.
pub(crate) struct SlidingRun {
    iface: SlidingInterface,
    cur: SlidingMesh,
    omega: f64,
    wall_omega: f64,
    walls: Vec<String>,
}

/// `m` with the cell zone `rotor` of the generator (D2).
fn with_zone(mut m: SlidingMesh) -> Result<SlidingMesh, String> {
    let n = m.n_rotor_cells();
    m.mesh
        .add_cell_zone("rotor", (0..n).collect())
        .map_err(|e| e.to_string())?;
    Ok(m)
}

/// The simulation of a case with `[mesh.sliding]` (D2, D4, D5).
pub(crate) fn new_sliding(case: &Case) -> Result<Simulation, Vec<Diagnostic>> {
    let errs = check(case);
    if !errs.is_empty() {
        return Err(errs);
    }
    if crate::mesh_transform::rotation(case).is_some() {
        return Err(vec![refused("mesh.transform", "a rotation of the mesh")]);
    }
    let (a, b) = super::sliding_blocks(case)?;
    let me = |e: rcfd_core::Error| diag(TABLE, e.to_string());
    let iface = SlidingInterface::cylindrical(a, "outer", b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).map_err(me)?;
    let cur = iface.mesh_at(0.0).map_err(me)?;
    let cur = with_zone(cur).map_err(|e| diag(TABLE, e))?;
    let q = interface_non_orthogonality(
        &cur.mesh,
        &cur.report.interface_faces,
        CYLINDRICAL_INTERFACE_MAX_NON_ORTHOGONALITY_DEG,
    )
    .map_err(me)?;
    if q.exceeds {
        return Err(diag(
            "mesh.r_interface",
            format!("{}; mesh.isotropic_interface = true sizes them", q.message()),
        ));
    }
    let geom = Arc::new(FvGeometry::new(Arc::new(cur.mesh.clone())).map_err(|e| diag("mesh", e.to_string()))?);
    let mut sim = Simulation::from_geometry(case, geom)?;
    let others: [(bool, &str, &str); 22] = [
        (
            !matches!(sim.engine, Engine::Transient(_)),
            "solver.algorithm",
            "this solver (constant-density piso / pimple is needed)",
        ),
        (sim.energy.is_some(), "energy.enabled", "the energy equation"),
        (sim.turb.is_some(), "turbulence.model", "a turbulence model"),
        (sim.scalars.is_some(), "scalars", "transported scalars or species"),
        (sim.chem.is_some(), "chemistry", "chemistry"),
        (sim.rheology.is_some(), "rheology", "a rheology law"),
        (sim.sources.is_some(), "sources", "sources"),
        (sim.zone_sources.is_some(), "zone_sources", "zone sources"),
        (sim.components.is_some(), "components", "lumped components"),
        (sim.radiation.is_some(), "radiation", "radiation"),
        (sim.cht.is_some(), "solids", "conjugate heat transfer"),
        (sim.electric.is_some(), "electric", "the electric potential"),
        (sim.phase.is_some(), "phase_change", "phase change"),
        (
            sim.particles.is_some() || sim.two_way.is_some(),
            "particles",
            "particles",
        ),
        (sim.mixing.is_some(), "mixing_plane", "a mixing plane"),
        (
            !sim.expr_bcs.is_empty(),
            "boundaries",
            "expression or user-function boundary values",
        ),
        (!sim.marangoni.is_empty(), "boundaries", "Marangoni walls"),
        (!sim.udfs.is_empty(), "udf", "user functions"),
        (sim.base_body.is_some(), "physics.body_force", "a body force"),
        (sim.catalogue.is_some(), "boundaries", "boundary-catalogue conditions"),
        (
            sim.timectl.as_ref().is_some_and(|c| c.ctl.settings.error_tol > 0.0),
            "solver.error_tolerance",
            "adaptive time-step control",
        ),
        (
            get_s(case, "solver.corrector_control") == "adaptive",
            "solver.corrector_control",
            "the adaptive corrector count",
        ),
    ];
    let bad: Vec<Diagnostic> = others
        .iter()
        .filter(|(on, _, _)| *on)
        .map(|(_, path, what)| refused(path, what))
        .collect();
    if !bad.is_empty() {
        return Err(bad);
    }
    // D4: the patches with rotor faces
    let mut walls = Vec::new();
    let mut errs = Vec::new();
    for patch in cur.mesh.patches() {
        if !patch.faces().any(|f| cur.is_rotor_face(f)) {
            continue;
        }
        let b = format!("boundaries.{}", patch.name);
        let kind = get_s(case, &format!("{b}.velocity"));
        let value = get_v(case, &format!("{b}.velocity_value"), [0.0; 3]);
        match kind.as_str() {
            "fixed" if value == [0.0; 3] => walls.push(patch.name.clone()),
            "fixed" | "symmetry" | "zero_gradient" => {}
            other => errs.push(at(
                &format!("{b}.velocity"),
                format!("velocity = \"{other}\" on patch '{}', which holds rotor faces, is not available with [mesh.sliding] (fixed, symmetry and zero_gradient are; ADR 0637 D4)", patch.name),
            )),
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let omega = get_f(case, "mesh.sliding.angular_velocity");
    let wall_omega = if get_s(case, "mesh.sliding.rotor_walls") == "rest" {
        0.0
    } else {
        omega
    };
    sim.sliding = Some(SlidingRun {
        iface,
        cur,
        omega,
        wall_omega,
        walls,
    });
    Ok(sim)
}

/// What one step of the loop reports for the monitors (D3).
pub(crate) struct SlidingRecord {
    pub angle: f64,
    pub interface_flux: f64,
    pub gcl: f64,
    pub non_orthogonality: f64,
}

impl SlidingRun {
    /// One step of ADR 0216's loop SL on the solver `s` (D3, D4): the joined mesh at the rotor angle of the end of the step,
    /// the step data, the wall conditions of the rotor, `step_sliding`.
    pub(crate) fn step(&mut self, s: &mut PimpleSolver, dt: f64) -> Result<(StepInfo, SlidingRecord), String> {
        let theta = self.omega * (s.time + dt);
        let new = self.iface.mesh_at(theta).map_err(|e| e.to_string())?;
        let new = with_zone(new)?;
        let q = interface_non_orthogonality(
            &new.mesh,
            &new.report.interface_faces,
            CYLINDRICAL_INTERFACE_MAX_NON_ORTHOGONALITY_DEG,
        )
        .map_err(|e| e.to_string())?;
        if q.exceeds {
            return Err(format!(
                "[mesh.sliding] at the rotor angle {theta} rad: {}",
                q.message()
            ));
        }
        let st = sliding_step(&s.geom, &self.cur, &new, &s.phi, &s.u.values, dt)?;
        let gcl = st.gcl_residual;
        let g = s.geom.clone();
        let dpdn = if self.walls.is_empty() {
            Vec::new()
        } else {
            rotor_wall_conditions(&mut s.u, &st, &g, &new, &self.walls, self.wall_omega, dt)?
        };
        let info = s.step_sliding(dt, st, dpdn)?;
        self.cur = new;
        let interface_flux = self.cur.report.interface_faces.iter().map(|&f| s.phi[f]).sum();
        Ok((
            info,
            SlidingRecord {
                angle: theta,
                interface_flux,
                gcl,
                non_orthogonality: q.worst_deg,
            },
        ))
    }

    /// Interface faces of the current joined mesh.
    pub(crate) fn interface_faces(&self) -> &[usize] {
        &self.cur.report.interface_faces
    }
}
