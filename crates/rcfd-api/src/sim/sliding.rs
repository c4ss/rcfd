//! The case key `[mesh.sliding]` (F-303, ADR 0637): the rotor block of `mesh.generator = "annulus_3d_cylindrical_interface"`
//! turns against the stator block at `angular_velocity` about `+z`, and the interface is rebuilt every step by the sliding
//! interface of F-250 (`rcfd_mesh::sliding`, `rcfd_fv::sliding`, `PimpleSolver::step_sliding`, ADR 0216).
//!
//! With `mesh.generator = "annulus_3d_sector_cylindrical_interface"` (F-711, ADR 0646) the rotor sector turns against the
//! stator sector of the same pitch through the periodic sliding interface of F-304 and F-704 (`rcfd_mesh::sliding_periodic`,
//! `PeriodicSlidingInterface::mesh_turned`, `rcfd_fv::sliding_periodic`, `PimpleSolver::step_sliding_periodic`), any number
//! of pitches, the rotor re-imaged by one pitch whenever its turned angle passes one.
//!
//! A case without the table never reaches this module beyond [`present`]: `Simulation::new` builds the frozen join as
//! before.

use super::{diag, get_f, get_s, get_v, Engine, Simulation};
use crate::case::{Case, Diagnostic};
use rcfd_core::settings::Schema;
use rcfd_core::units::Dimension;
use rcfd_fv::pimple::{PimpleSolver, StepInfo};
use rcfd_fv::sliding::{rotor_wall_conditions, sliding_step};
use rcfd_fv::sliding_periodic::{periodic_rotor_wall_conditions, periodic_sliding_step, periodic_sliding_step_coupled};
use rcfd_fv::FvGeometry;
use rcfd_mesh::non_conformal::{interface_non_orthogonality, CYLINDRICAL_INTERFACE_MAX_NON_ORTHOGONALITY_DEG};
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use rcfd_mesh::sliding_periodic::{PeriodicSlidingInterface, PeriodicSlidingMesh};
use rcfd_mesh::PatchKind;
use std::sync::Arc;

/// Path of the table.
pub(crate) const TABLE: &str = "mesh.sliding";

/// The generator whose rotor and stator blocks slide (D1).
const GENERATOR: &str = "annulus_3d_cylindrical_interface";

/// The sector rotor-stator generator whose rotor sector slides against the stator sector (F-711, ADR 0646 G1).
const SECTOR_GENERATOR: &str = "annulus_3d_sector_cylindrical_interface";

/// Schema of `[mesh.sliding]` (D1): an optional table, absent from every case that does not write it.
pub fn schema() -> Schema {
    Schema::object()
        .optional()
        .doc("Sliding rotor-stator interface (F-303, ADR 0637): the rotor block of mesh.generator = \"annulus_3d_cylindrical_interface\" (r_in <= r <= r_interface) turns about +z through the origin against the stator block, and the interface is rebuilt every step (the sliding interface of ADR 0216: the rotor faces' swept volumes, the interface mesh flux that closes the GCL, the old flux projected onto the new faces). With mesh.generator = \"annulus_3d_sector_cylindrical_interface\" (F-711, ADR 0646) the rotor sector turns against the stator sector of the same pitch through the periodic sliding interface (wrap pairs across the seams), any number of pitches (angular_velocity >= 0; the rotor is re-imaged by one pitch whenever its angle passes one; a non-zero fixed value on a patch with rotor faces must be axial). Transient laminar piso / pimple with time_scheme = euler, constant properties, no sources, models or expression boundary values. Patches with rotor faces: velocity fixed 0 is a wall of the rotor (rotor_walls), a non-zero uniform fixed value, symmetry and zero_gradient are kept as written (laboratory frame). Absent = the frozen join")
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
        .field(
            "coincidences",
            Schema::object()
                .optional()
                .doc("Node coincidences of the sector path (F-730, ADR 0654), annulus_3d_sector_cylindrical_interface only: on a step where rotor and stator nodes coincide the interface graph falls into parts; coupled = true couples them through the internal faces and seam pairs between them (periodic_sliding_step_coupled, ADR 0641) so that the remapped old flux stays divergence-free for flows without the node sets' symmetry, and writes the monitor sliding_flux_residual. Absent = the base periodic step. The full annulus is coupled always (F-710) and refuses this table")
                .field(
                    "coupled",
                    Schema::boolean(false)
                        .doc("Couple the parts of the sector's interface graph at node coincidences (ADR 0654 S1); off coincidences the step is the base step bit for bit"),
                ),
        )
}

/// Path of the coincidence table (F-730, ADR 0654 S1).
const COINCIDENCES: &str = "mesh.sliding.coincidences";

/// Whether the case writes `[mesh.sliding.coincidences]` (an optional table, ADR 0654 S1).
fn coincidences_present(case: &Case) -> bool {
    case.tree
        .get("mesh")
        .ok()
        .is_some_and(|m| m.get("sliding").and_then(|s| s.get("coincidences")).is_some())
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
    } else if generator != GENERATOR && generator != SECTOR_GENERATOR {
        out.push(refused(
            "mesh.generator",
            &format!("mesh.generator = \"{generator}\" ({GENERATOR} and {SECTOR_GENERATOR} are)"),
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
    } else if generator == SECTOR_GENERATOR && get_f(case, "mesh.sliding.angular_velocity") < 0.0 {
        out.push(at(
            "mesh.sliding.angular_velocity",
            format!("the rotor sector of {SECTOR_GENERATOR} turns forwards only: angular_velocity >= 0 (the periodic sliding re-images the rotor one pitch forwards, ADR 0634, ADR 0646)"),
        ));
    }
    // ADR 0654 S2: the full annulus is coupled at coincidences always (F-710)
    if generator != SECTOR_GENERATOR && coincidences_present(case) {
        out.push(at(
            COINCIDENCES,
            format!("[mesh.sliding.coincidences] is for {SECTOR_GENERATOR} only: the full annulus of {GENERATOR} is coupled at node coincidences always (F-710, ADR 0638); remove the table"),
        ));
    }
    out
}

/// The pair of a sliding run and its current joined mesh: the full annulus (F-303) or the sector (F-711, ADR 0646 P1).
enum Pair {
    Annulus {
        iface: SlidingInterface,
        cur: SlidingMesh,
    },
    Sector {
        iface: PeriodicSlidingInterface,
        cur: PeriodicSlidingMesh,
    },
}

/// The sliding loop of a case (D3): the pair, the current joined mesh and the walls of the rotor.
pub(crate) struct SlidingRun {
    pair: Pair,
    omega: f64,
    wall_omega: f64,
    walls: Vec<String>,
    /// The sector step is coupled at node coincidences (F-730, ADR 0654 S1).
    coupled: bool,
}

/// `m` with the cell zone `rotor` of the generator (D2).
fn with_zone(mut m: SlidingMesh) -> Result<SlidingMesh, String> {
    let n = m.n_rotor_cells();
    m.mesh
        .add_cell_zone("rotor", (0..n).collect())
        .map_err(|e| e.to_string())?;
    Ok(m)
}

/// The periodic mesh `m` with the cell zone `rotor` (ADR 0646 P1).
fn with_zone_sector(mut m: PeriodicSlidingMesh) -> Result<PeriodicSlidingMesh, String> {
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
    let me = |e: rcfd_core::Error| diag(TABLE, e.to_string());
    let sector = get_s(case, "mesh.generator") == SECTOR_GENERATOR;
    let pair = if sector {
        // F-711, ADR 0646 P1: the sector pair at turned angle 0 (the frozen sector join of the generator)
        let iface = super::sliding_sector_pair(case)?;
        let mut cur = iface.mesh_turned(0.0).map_err(me)?;
        let faces = cur.report.join.interface_faces.clone();
        super::sector_interface_finish(&mut cur.mesh, iface.n_rotor_cells(), &faces)?;
        Pair::Sector { iface, cur }
    } else {
        let (a, b) = super::sliding_blocks(case)?;
        let iface =
            SlidingInterface::cylindrical(a, "outer", b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).map_err(me)?;
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
        Pair::Annulus { iface, cur }
    };
    let mesh0 = match &pair {
        Pair::Annulus { cur, .. } => &cur.mesh,
        Pair::Sector { cur, .. } => &cur.mesh,
    };
    let is_rotor_face = |f: usize| match &pair {
        Pair::Annulus { cur, .. } => cur.is_rotor_face(f),
        Pair::Sector { cur, .. } => cur.is_rotor_face(f),
    };
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).map_err(|e| diag("mesh", e.to_string()))?);
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
    // D4: the patches with rotor faces (ADR 0646 D4s: a cyclic seam is no wall; on a sector a non-zero fixed value must be
    // axial, the only uniform vector periodic under the pitch rotation)
    let mut walls = Vec::new();
    let mut errs = Vec::new();
    for patch in mesh0.patches() {
        if patch.kind == PatchKind::Cyclic || !patch.faces().any(is_rotor_face) {
            continue;
        }
        let b = format!("boundaries.{}", patch.name);
        let kind = get_s(case, &format!("{b}.velocity"));
        let value = get_v(case, &format!("{b}.velocity_value"), [0.0; 3]);
        match kind.as_str() {
            "fixed" if value == [0.0; 3] => walls.push(patch.name.clone()),
            "fixed" if sector && (value[0] != 0.0 || value[1] != 0.0) => errs.push(at(
                &format!("{b}.velocity_value"),
                format!("velocity_value = {value:?} on patch '{}', which holds rotor faces of a sector, is not axial: a uniform value is periodic under the pitch rotation only along the axis (ADR 0646 D4s); use [0, 0, w] or a wall", patch.name),
            )),
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
    let coupled = sector && super::get_b(case, &format!("{COINCIDENCES}.coupled"));
    sim.sliding = Some(SlidingRun {
        pair,
        omega,
        wall_omega,
        walls,
        coupled,
    });
    Ok(sim)
}

/// What one step of the loop reports for the monitors (D3).
pub(crate) struct SlidingRecord {
    pub angle: f64,
    pub interface_flux: f64,
    pub gcl: f64,
    pub non_orthogonality: f64,
    /// The image of a sector rotor (ADR 0646 P1); `None` for the full annulus.
    pub image: Option<i64>,
    /// The remapped old flux's residual of a coupled sector step (ADR 0654 S3); `None` otherwise.
    pub flux_residual: Option<f64>,
}

impl SlidingRun {
    /// One step of ADR 0216's loop SL on the solver `s` (D3, D4): the joined mesh at the rotor angle of the end of the step,
    /// the step data, the wall conditions of the rotor, `step_sliding`.
    pub(crate) fn step(&mut self, s: &mut PimpleSolver, dt: f64) -> Result<(StepInfo, SlidingRecord), String> {
        let (iface, cur) = match &mut self.pair {
            Pair::Annulus { iface, cur } => (&*iface, cur),
            Pair::Sector { iface, cur } => {
                return Self::step_sector(
                    iface,
                    cur,
                    (self.omega, self.wall_omega),
                    &self.walls,
                    self.coupled,
                    s,
                    dt,
                )
            }
        };
        let theta = self.omega * (s.time + dt);
        let new = iface.mesh_at(theta).map_err(|e| e.to_string())?;
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
        let st = sliding_step(&s.geom, cur, &new, &s.phi, &s.u.values, dt)?;
        let gcl = st.gcl_residual;
        let g = s.geom.clone();
        let dpdn = if self.walls.is_empty() {
            Vec::new()
        } else {
            rotor_wall_conditions(&mut s.u, &st, &g, &new, &self.walls, self.wall_omega, dt)?
        };
        let info = s.step_sliding(dt, st, dpdn)?;
        *cur = new;
        let interface_flux = cur.report.interface_faces.iter().map(|&f| s.phi[f]).sum();
        Ok((
            info,
            SlidingRecord {
                angle: theta,
                interface_flux,
                gcl,
                non_orthogonality: q.worst_deg,
                image: None,
                flux_residual: None,
            },
        ))
    }

    /// One step of ADR 0634's turned loop on the sector pair (ADR 0646 P1, P2): the joined mesh at the turned angle of the
    /// end of the step (any number of pitches, `mesh_turned`), the step data (re-imaging the rotor when the image changes),
    /// the wall conditions of the rotor in its image, `step_sliding_periodic`. With `coupled` the step data are those of
    /// `periodic_sliding_step_coupled` (ADR 0654 S1).
    fn step_sector(
        iface: &PeriodicSlidingInterface,
        cur: &mut PeriodicSlidingMesh,
        (omega, wall_omega): (f64, f64),
        walls: &[String],
        coupled: bool,
        s: &mut PimpleSolver,
        dt: f64,
    ) -> Result<(StepInfo, SlidingRecord), String> {
        let turned = omega * (s.time + dt);
        let new = iface.mesh_turned(turned).map_err(|e| e.to_string())?;
        let new = with_zone_sector(new)?;
        let q = interface_non_orthogonality(
            &new.mesh,
            &new.report.join.interface_faces,
            CYLINDRICAL_INTERFACE_MAX_NON_ORTHOGONALITY_DEG,
        )
        .map_err(|e| e.to_string())?;
        if q.exceeds {
            return Err(format!(
                "[mesh.sliding] at the turned angle {turned} rad: {}",
                q.message()
            ));
        }
        let st = if coupled {
            periodic_sliding_step_coupled(&s.geom, cur, &new, &s.phi, &s.u.values, dt)?
        } else {
            periodic_sliding_step(&s.geom, cur, &new, &s.phi, &s.u.values, dt)?
        };
        let gcl = st.gcl_residual;
        let flux_residual = coupled.then_some(st.flux_residual);
        let g = s.geom.clone();
        let dpdn = if walls.is_empty() {
            Vec::new()
        } else {
            periodic_rotor_wall_conditions(&mut s.u, &st, &g, &new, walls, wall_omega, dt)?
        };
        let info = s.step_sliding_periodic(dt, st, dpdn)?;
        *cur = new;
        let direct: f64 = cur.report.join.interface_faces.iter().map(|&f| s.phi[f]).sum();
        let wrap: f64 = cur.report.wrap_pairs.iter().map(|&(a, _)| s.phi[a]).sum();
        Ok((
            info,
            SlidingRecord {
                angle: turned,
                interface_flux: direct + wrap,
                gcl,
                non_orthogonality: q.worst_deg,
                image: Some(cur.image),
                flux_residual,
            },
        ))
    }

    /// Interface faces of the current joined mesh (the direct faces of a sector).
    pub(crate) fn interface_faces(&self) -> &[usize] {
        match &self.pair {
            Pair::Annulus { cur, .. } => &cur.report.interface_faces,
            Pair::Sector { cur, .. } => &cur.report.join.interface_faces,
        }
    }

    /// The wrap pairs of the current joined mesh of a sector (ADR 0646 P1); `None` for the full annulus.
    pub(crate) fn wrap_pairs(&self) -> Option<&[(usize, usize)]> {
        match &self.pair {
            Pair::Annulus { .. } => None,
            Pair::Sector { cur, .. } => Some(&cur.report.wrap_pairs),
        }
    }
}
