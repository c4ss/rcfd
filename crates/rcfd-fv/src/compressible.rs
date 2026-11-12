//! Density-based explicit compressible Euler solver on polyhedral meshes:
//! cell-centred finite volumes, MUSCL reconstruction of primitive variables
//! with TVD limiters (Darwish-Moukalled r-ratio form), approximate Riemann
//! fluxes ([`RiemannSolver`]) and Shu-Osher SSP Runge-Kutta time stepping.
//!
//! References: Toro (2009); Shu and Osher, J. Comput. Phys. 77 (1988) for the
//! SSP-RK schemes; Darwish and Moukalled, Int. J. Heat Mass Transfer 46 (2003) 599-611 for
//! the unstructured TVD reconstruction.

use crate::compressible_bc::{self, MsdWall, TotalInlet};
use crate::convection::TvdLimiter;
use crate::geometry::FvGeometry;
pub use crate::riemann::{to_prim, Cons, Gas, Prim, RiemannSolver};
use crate::sponge::{self, MatchedOptions, Sponge, SpongeForm, SpongeFrame, SpongeMemories};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::motion::{cell_swept_sum, swept_volumes};
use rcfd_mesh::PatchKind;

/// Boundary treatment of a patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompBc {
    /// Slip wall / symmetry plane (mirror ghost state).
    Slip,
    /// Zero-gradient outflow (ghost state equals the interior state; valid for
    /// supersonic outflow).
    Extrapolate,
    /// Fixed free-stream / inflow state (the Riemann problem against it is solved).
    Fixed(Prim),
    /// Subsonic total pressure / total temperature inlet (ADR 0006).
    TotalInlet(TotalInlet),
    /// Static-pressure outlet with supersonic switching (ADR 0006).
    PressureOutlet {
        /// Static pressure.
        p: f64,
    },
    /// Partially non-reflecting pressure outlet: `sigma = 1` is [`CompBc::PressureOutlet`],
    /// `sigma = 0` imposes no pressure (ADR 0006).
    NonReflecting {
        /// Target static pressure.
        p: f64,
        /// Relaxation weight in `[0, 1]`.
        sigma: f64,
    },
    /// Riemann-invariant far field for the given free-stream state (ADR 0006).
    FarField(Prim),
    /// Locally reacting wall with a real specific acoustic impedance (F-243, ADR 0233): `p_face - p = z u_n`, with `u_n`
    /// relative to the face. `z = 0` is [`CompBc::PressureOutlet`]; `z = rho c` of the ambient state is a quiet boundary
    /// that holds `p`; a rigid wall is [`CompBc::Slip`]. Normal incidence reflects `(z - rho c)/(z + rho c)`.
    Impedance {
        /// Ambient (reference) static pressure.
        p: f64,
        /// Specific acoustic impedance [Pa s/m], finite and `>= 0`.
        z: f64,
    },
}

/// Time integration scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RkScheme {
    /// Forward Euler (first order; for tests).
    Euler,
    /// Two-stage SSP-RK2 (Heun).
    Rk2,
    /// Three-stage SSP-RK3.
    Rk3,
}

/// Solver settings.
#[derive(Debug, Clone, Copy)]
pub struct CompSettings {
    /// Numerical flux.
    pub solver: RiemannSolver,
    /// Slope limiter; `None` gives first-order upwind-type fluxes.
    pub limiter: Option<TvdLimiter>,
    /// Time integrator.
    pub rk: RkScheme,
    /// CFL number.
    pub cfl: f64,
}

impl Default for CompSettings {
    fn default() -> Self {
        Self {
            solver: RiemannSolver::Hllc,
            limiter: Some(TvdLimiter::VanLeer),
            rk: RkScheme::Rk3,
            cfl: 0.4,
        }
    }
}

/// Explicit compressible solver state.
#[derive(Debug, Clone)]
pub struct CompressibleSolver {
    /// Mesh geometry.
    pub geom: FvGeometry,
    /// Gas model.
    pub gas: Gas,
    /// Settings.
    pub settings: CompSettings,
    /// Conserved variables per cell.
    pub u: Vec<Cons>,
    /// Boundary treatment per mesh patch (indexed like `mesh.patches()`).
    pub bcs: Vec<CompBc>,
    /// Inverse of the weighted least-squares moment matrix per cell.
    ls_inv: Vec<[[f64; 3]; 3]>,
    /// Physical time.
    pub time: f64,
    /// Number of steps taken.
    pub steps: usize,
    /// The step count after the last [`Self::step_ale`] and the normal mesh speed of every face in it (F-258, ADR 0207 M2).
    ale_speeds: Option<(usize, Vec<f64>)>,
    /// The opt-in sponge layer (F-331, ADR 0237); `None` by default.
    sponge: Option<Sponge>,
    /// The memory `Q` of the matched sponge form, one value per cell while a sponge is set.
    sponge_q: Vec<f64>,
    /// The further memories of the shifted matched form (F-340, ADR 0248): `r`, the momentum memory and the rates of
    /// the second layer; empty unless that form is set.
    sponge_r: Vec<f64>,
    sponge_m: Vec<Vec3>,
    sponge_rate2: Vec<f64>,
    /// The frame of the sponge on a moving mesh (F-341, ADR 0265 E1); `StaticMesh` by default.
    sponge_frame: SpongeFrame,
    /// The opt-in mass-spring-damper walls and their state (F-332, ADR 0243); `None` by default.
    msd: Option<MsdState>,
}

/// The mass-spring-damper walls of a solver (F-332, ADR 0243 D1, D2).
#[derive(Debug, Clone)]
struct MsdState {
    /// The wall of every mesh patch, if it has one.
    walls: Vec<Option<MsdWall>>,
    /// Displacement of the wall along the outward normal, per boundary face (index `face - n_internal`).
    xi: Vec<f64>,
    /// Velocity of the wall, per boundary face; not used by a wall without mass.
    v: Vec<f64>,
}

impl CompressibleSolver {
    /// New solver with initial primitive state `init(x)`. Patches default to
    /// slip walls (empty patches are ignored).
    pub fn new(geom: FvGeometry, gas: Gas, settings: CompSettings, init: impl Fn(Vec3) -> Prim) -> Result<Self> {
        let cc = geom.mesh.cell_centres();
        let u: Vec<Cons> = cc.iter().map(|&x| init(x).to_cons(gas)).collect();
        for c in &u {
            let p = to_prim(gas, c);
            if !(p.rho > 0.0 && p.p > 0.0) {
                return Err(Error::Numerical(
                    "initial state has non-positive density or pressure".into(),
                ));
            }
        }
        let bcs = geom
            .mesh
            .patches()
            .iter()
            .map(|p| match p.kind {
                PatchKind::Outlet => CompBc::Extrapolate,
                _ => CompBc::Slip,
            })
            .collect();
        let ls_inv = ls_inverse(&geom);
        Ok(Self {
            geom,
            gas,
            settings,
            u,
            bcs,
            ls_inv,
            time: 0.0,
            steps: 0,
            ale_speeds: None,
            sponge: None,
            sponge_q: Vec::new(),
            sponge_r: Vec::new(),
            sponge_m: Vec::new(),
            sponge_rate2: Vec::new(),
            sponge_frame: SpongeFrame::StaticMesh,
            msd: None,
        })
    }

    /// Set the boundary treatment of a named patch.
    pub fn set_bc(&mut self, patch: &str, bc: CompBc) -> Result<()> {
        let i = self
            .geom
            .mesh
            .patches()
            .iter()
            .position(|p| p.name == patch)
            .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))?;
        if let CompBc::Impedance { p, z } = bc {
            if !(z.is_finite() && z >= 0.0 && p.is_finite() && p > 0.0) {
                return Err(Error::InvalidValue {
                    path: format!("patch {patch}"),
                    reason: format!(
                        "an impedance boundary needs a finite impedance >= 0 and a positive pressure, got z = {z}, p = {p}"
                    ),
                });
            }
        }
        self.bcs[i] = bc;
        Ok(())
    }

    /// Set or clear the sponge layer (F-331, ADR 0237; [`crate::sponge`]). The sponge is checked against the mesh
    /// ([`Sponge::validate`]) and its memory starts at zero. Without a sponge [`Self::step`] is unchanged. While a sponge
    /// is set, [`Self::replace_mesh`] returns an error, [`Self::step_ale`] returns one unless the sponge is attached to the
    /// mesh ([`Self::set_sponge_frame`], F-341, ADR 0265), and [`Self::max_dt`] does not know the
    /// sponge: `rate dt` must stay well below 2.5.
    pub fn set_sponge(&mut self, sponge: Option<Sponge>) -> Result<()> {
        if sponge.is_some() && self.msd.is_some() {
            return Err(Error::Numerical(
                "set_sponge: a sponge together with a mass-spring-damper wall is not built (ADR 0243 D6); clear the wall first"
                    .into(),
            ));
        }
        if let Some(s) = &sponge {
            s.validate(self.u.len())?;
        }
        // F-340 (ADR 0248): the shifted matched form carries two more memories and the rates of its second layer
        let shifted = match &sponge {
            Some(s) if matches!(s.form, SpongeForm::MatchedShifted(_)) => Some(s.second_rates(&self.geom)?),
            _ => None,
        };
        self.sponge_q = match &sponge {
            Some(_) => vec![0.0; self.u.len()],
            None => Vec::new(),
        };
        match shifted {
            Some(rate2) => {
                self.sponge_r = vec![0.0; self.u.len()];
                self.sponge_m = vec![[0.0; 3]; self.u.len()];
                self.sponge_rate2 = rate2;
            }
            None => {
                self.sponge_r = Vec::new();
                self.sponge_m = Vec::new();
                self.sponge_rate2 = Vec::new();
            }
        }
        self.sponge = sponge;
        Ok(())
    }

    /// The memories of the shifted matched form (F-340, ADR 0248): `q`, `r` and the momentum memory per cell. For the
    /// other forms `r` and `m` are empty and `q` is [`Self::sponge_memory`].
    pub fn sponge_memories(&self) -> SpongeMemories {
        SpongeMemories {
            q: self.sponge_q.clone(),
            r: self.sponge_r.clone(),
            m: self.sponge_m.clone(),
        }
    }

    /// Set the memories of the shifted matched form (for a restart, or for a start with a wave already inside the
    /// layer): one finite value per cell in each; an error unless that form is set.
    pub fn set_sponge_memories(&mut self, mem: &SpongeMemories) -> Result<()> {
        let n = self.u.len();
        let shifted = matches!(
            self.sponge.as_ref().map(|s| s.form),
            Some(SpongeForm::MatchedShifted(_))
        );
        let ok = shifted
            && mem.q.len() == n
            && mem.r.len() == n
            && mem.m.len() == n
            && mem.q.iter().chain(&mem.r).all(|x| x.is_finite())
            && mem.m.iter().flatten().all(|x| x.is_finite());
        if !ok {
            return Err(Error::InvalidValue {
                path: "sponge".into(),
                reason: "the sponge memories need the shifted matched form and one finite value per cell in each"
                    .into(),
            });
        }
        self.sponge_q = mem.q.clone();
        self.sponge_r = mem.r.clone();
        self.sponge_m = mem.m.clone();
        Ok(())
    }

    /// The sponge layer, if one is set.
    pub fn sponge(&self) -> Option<&Sponge> {
        self.sponge.as_ref()
    }

    /// Set the frame of the sponge on a moving mesh (F-341, ADR 0265 E1). With [`SpongeFrame::StaticMesh`] (the default)
    /// [`Self::step_ale`] refuses a sponge; with [`SpongeFrame::AttachedToMesh`] it takes the sponged ALE step, the rates
    /// and the memories moving with the cells; with [`SpongeFrame::AttachedWithLabMemory`] (F-407, ADR 0344) the rates move
    /// with the cells and the memories are those of the lab frame. The setting is kept across [`Self::set_sponge`].
    /// [`Self::replace_mesh`] refuses a sponge in every frame.
    pub fn set_sponge_frame(&mut self, frame: SpongeFrame) {
        self.sponge_frame = frame;
    }

    /// The frame of the sponge on a moving mesh.
    pub fn sponge_frame(&self) -> SpongeFrame {
        self.sponge_frame
    }

    /// Set the memory `Q` of the matched sponge form (ADR 0237 amendment 1), for a restart or for a start from a state in
    /// which a wave is already inside the layer: one finite value per cell; an error without a sponge.
    pub fn set_sponge_memory(&mut self, q: &[f64]) -> Result<()> {
        if self.sponge.is_none() || q.len() != self.u.len() || q.iter().any(|x| !x.is_finite()) {
            return Err(Error::InvalidValue {
                path: "sponge".into(),
                reason: "the sponge memory needs a sponge and one finite value per cell".into(),
            });
        }
        self.sponge_q = q.to_vec();
        Ok(())
    }

    /// The memory `Q` (time integral of `p - p_ref`) of the matched sponge form per cell; empty without a sponge.
    pub fn sponge_memory(&self) -> &[f64] {
        &self.sponge_q
    }

    /// Index of the patch `patch`.
    fn patch_index(&self, patch: &str) -> Result<usize> {
        self.geom
            .mesh
            .patches()
            .iter()
            .position(|p| p.name == patch)
            .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))
    }

    /// Set or clear the mass-spring-damper wall of a patch (F-332, ADR 0243; [`MsdWall`]). While a wall is set it
    /// replaces the patch's [`CompBc`]; its state (displacement and velocity per face) starts at zero and is advanced with
    /// the Runge-Kutta stages of [`Self::step`]. Without any wall [`Self::step`] and [`Self::max_dt`] are unchanged. With
    /// a wall, [`Self::max_dt`] is also limited by [`Self::msd_dt_limit`], [`Self::step`] refuses a larger step, and
    /// [`Self::step_ale`], [`Self::replace_mesh`] and [`Self::set_sponge`] return an error. An error for an unknown patch,
    /// an invalid wall ([`MsdWall::validate`]) or while a sponge is set.
    pub fn set_msd_wall(&mut self, patch: &str, wall: Option<MsdWall>) -> Result<()> {
        let i = self.patch_index(patch)?;
        if let Some(w) = &wall {
            w.validate().map_err(|e| match e {
                Error::InvalidValue { reason, .. } => Error::InvalidValue {
                    path: format!("patch {patch}"),
                    reason,
                },
                other => other,
            })?;
            if self.sponge.is_some() {
                return Err(Error::Numerical(
                    "set_msd_wall: a mass-spring-damper wall together with a sponge is not built (ADR 0243 D6); clear the sponge first"
                        .into(),
                ));
            }
        }
        if wall.is_none() && self.msd.is_none() {
            return Ok(());
        }
        let mesh = &self.geom.mesh;
        let (n_patches, n_bnd, n_int) = (
            mesh.patches().len(),
            mesh.n_faces() - self.geom.n_internal,
            self.geom.n_internal,
        );
        let faces = mesh.patches()[i].faces();
        let st = self.msd.get_or_insert_with(|| MsdState {
            walls: vec![None; n_patches],
            xi: vec![0.0; n_bnd],
            v: vec![0.0; n_bnd],
        });
        st.walls[i] = wall;
        for f in faces {
            st.xi[f - n_int] = 0.0;
            st.v[f - n_int] = 0.0;
        }
        if st.walls.iter().all(|w| w.is_none()) {
            self.msd = None;
        }
        Ok(())
    }

    /// The mass-spring-damper wall of a patch, if it has one.
    pub fn msd_wall(&self, patch: &str) -> Option<MsdWall> {
        let i = self.patch_index(patch).ok()?;
        self.msd.as_ref().and_then(|st| st.walls[i])
    }

    /// The state `[xi, v]` (displacement along the outward normal, velocity) of the wall of a patch, one pair per face
    /// in the order of the patch's faces; an error when the patch has no wall. A wall without mass does not use `v`.
    pub fn msd_wall_state(&self, patch: &str) -> Result<Vec<[f64; 2]>> {
        let i = self.patch_index(patch)?;
        match &self.msd {
            Some(st) if st.walls[i].is_some() => Ok(self.geom.mesh.patches()[i]
                .faces()
                .map(|f| [st.xi[f - self.geom.n_internal], st.v[f - self.geom.n_internal]])
                .collect()),
            _ => Err(Error::InvalidValue {
                path: format!("patch {patch}"),
                reason: "the patch has no mass-spring-damper wall".into(),
            }),
        }
    }

    /// Set the state `[xi, v]` of the wall of a patch (a restart, or a wall released from a displacement): one finite
    /// pair per face of the patch; an error when the patch has no wall.
    pub fn set_msd_wall_state(&mut self, patch: &str, state: &[[f64; 2]]) -> Result<()> {
        let i = self.patch_index(patch)?;
        let faces = self.geom.mesh.patches()[i].faces();
        let n_int = self.geom.n_internal;
        match &mut self.msd {
            Some(st)
                if st.walls[i].is_some()
                    && state.len() == faces.len()
                    && state.iter().all(|q| q[0].is_finite() && q[1].is_finite()) =>
            {
                for (f, q) in faces.zip(state) {
                    st.xi[f - n_int] = q[0];
                    st.v[f - n_int] = q[1];
                }
                Ok(())
            }
            _ => Err(Error::InvalidValue {
                path: format!("patch {patch}"),
                reason: "the wall state needs a mass-spring-damper wall on the patch and one finite pair per face"
                    .into(),
            }),
        }
    }

    /// The largest step for which the explicit integration of every mass-spring-damper wall is stable
    /// ([`MsdWall::dt_limit`] with `rho a` of the cell at each wall face, ADR 0243 DR5); infinite without a wall.
    pub fn msd_dt_limit(&self) -> f64 {
        let Some(st) = &self.msd else {
            return f64::INFINITY;
        };
        let mesh = &self.geom.mesh;
        let own = mesh.owner();
        let mut lim = f64::INFINITY;
        for (pi, patch) in mesh.patches().iter().enumerate() {
            let Some(wall) = &st.walls[pi] else { continue };
            for f in patch.faces() {
                if self.geom.empty[f] {
                    continue;
                }
                let q = to_prim(self.gas, &self.u[own[f]]);
                lim = lim.min(wall.dt_limit(q.rho * q.sound_speed(self.gas)));
            }
        }
        lim
    }

    /// The face states of the mass-spring-damper walls for the state `u` and the wall state `(xi, v)`, per boundary face
    /// (`None` on a face without a wall), and the rates of `xi` and `v` (ADR 0243 DR2, DR3).
    #[allow(clippy::type_complexity)]
    fn msd_faces(&self, u: &[Cons], xi: &[f64], v: &[f64]) -> Result<(Vec<Option<Prim>>, Vec<f64>, Vec<f64>)> {
        let mesh = &self.geom.mesh;
        let n_int = self.geom.n_internal;
        let n_bnd = mesh.n_faces() - n_int;
        let mut faces = vec![None; n_bnd];
        let (mut xd, mut vd) = (vec![0.0; n_bnd], vec![0.0; n_bnd]);
        let Some(st) = &self.msd else {
            return Ok((faces, xd, vd));
        };
        let own = mesh.owner();
        let sf = mesh.face_area_vectors();
        for (pi, patch) in mesh.patches().iter().enumerate() {
            let Some(wall) = &st.walls[pi] else { continue };
            for f in patch.faces() {
                if self.geom.empty[f] {
                    continue;
                }
                let q = to_prim(self.gas, &u[own[f]]);
                if !(q.rho > 0.0 && q.p > 0.0) {
                    return Err(Error::Numerical(
                        "non-positive density or pressure in compressible solver".into(),
                    ));
                }
                let b = f - n_int;
                let r = wall
                    .rates(self.gas, &q, vec3::unit(sf[f]), xi[b], v[b])
                    .ok_or_else(|| {
                        Error::Numerical(format!(
                            "mass-spring-damper wall of patch {}: the displacement {} takes the reference pressure to zero or below",
                            patch.name, xi[b]
                        ))
                    })?;
                faces[b] = Some(r.face);
                xd[b] = r.xi_dot;
                vd[b] = r.v_dot;
            }
        }
        Ok((faces, xd, vd))
    }

    /// Primitive state of every cell.
    pub fn primitives(&self) -> Vec<Prim> {
        self.u.iter().map(|c| to_prim(self.gas, c)).collect()
    }

    /// Largest stable explicit step for the current state (`cfl` times the
    /// cell-wise limit `V / (sum_f (|u.n| + a)|S| / 2)`); while a mass-spring-damper wall is set, not above
    /// [`Self::msd_dt_limit`].
    pub fn max_dt(&self) -> f64 {
        let mesh = &self.geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let prim = self.primitives();
        let mut sum = vec![0.0; mesh.n_cells()];
        for f in 0..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let a = vec3::norm(sf[f]);
            let n = vec3::scale(sf[f], 1.0 / a);
            let lam = |p: &Prim| (vec3::dot(p.u, n).abs() + p.sound_speed(self.gas)) * a;
            sum[own[f]] += lam(&prim[own[f]]);
            if f < self.geom.n_internal {
                sum[nei[f]] += lam(&prim[nei[f]]);
            }
        }
        let vol = mesh.cell_volumes();
        let dt = (0..mesh.n_cells())
            .map(|c| 2.0 * vol[c] / sum[c])
            .fold(f64::INFINITY, f64::min)
            * self.settings.cfl;
        // F-332 (ADR 0243 D5): a mass-spring-damper wall limits the step as well
        if self.msd.is_some() {
            dt.min(self.msd_dt_limit())
        } else {
            dt
        }
    }

    /// Largest stable explicit step on a moving mesh (F-258, ADR 0207 M1): [`Self::max_dt`] with the signal speed of a face
    /// taken relative to the face, `cfl` times `V / (sum_f (|u.n - w_n| + a)|S| / 2)` on the current geometry. `wn` is the
    /// normal mesh speed of every face, along the owner's outward normal as [`Self::step_ale`] takes it; with every value
    /// zero this is `max_dt` operation for operation. An error when `wn` has not one finite value per face.
    pub fn max_dt_moving(&self, wn: &[f64]) -> Result<f64> {
        let mesh = &self.geom.mesh;
        if wn.len() != mesh.n_faces() || wn.iter().any(|w| !w.is_finite()) {
            return Err(Error::Numerical(format!(
                "max_dt_moving: one finite normal mesh speed per face is required ({} values for {} faces)",
                wn.len(),
                mesh.n_faces()
            )));
        }
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let prim = self.primitives();
        let mut sum = vec![0.0; mesh.n_cells()];
        for f in 0..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let a = vec3::norm(sf[f]);
            let n = vec3::scale(sf[f], 1.0 / a);
            let lam = |p: &Prim| ((vec3::dot(p.u, n) - wn[f]).abs() + p.sound_speed(self.gas)) * a;
            sum[own[f]] += lam(&prim[own[f]]);
            if f < self.geom.n_internal {
                sum[nei[f]] += lam(&prim[nei[f]]);
            }
        }
        let vol = mesh.cell_volumes();
        Ok((0..mesh.n_cells())
            .map(|c| 2.0 * vol[c] / sum[c])
            .fold(f64::INFINITY, f64::min)
            * self.settings.cfl)
    }

    /// The normal mesh speed of every face in the last [`Self::step_ale`] (`W_f / (dt |S_f|)`), while no [`Self::step`] and
    /// no [`Self::replace_mesh`] came after it (F-258, ADR 0207 M2).
    pub fn mesh_speeds(&self) -> Option<&[f64]> {
        match &self.ale_speeds {
            Some((step, w)) if *step == self.steps => Some(w),
            _ => None,
        }
    }

    /// The step of a moving mesh with a lagged mesh speed (F-258, ADR 0207 M2): [`Self::max_dt_moving`] with the speeds of
    /// the last ALE step ([`Self::mesh_speeds`]), which stand in for those of the next one; [`Self::max_dt`] when there are
    /// none.
    pub fn max_dt_ale(&self) -> f64 {
        self.mesh_speeds()
            .and_then(|w| self.max_dt_moving(w).ok())
            .unwrap_or_else(|| self.max_dt())
    }

    /// Ghost state across boundary face `f` of patch `pi`.
    fn ghost(&self, pi: usize, interior: &Prim, n: Vec3) -> Prim {
        match self.bcs[pi] {
            CompBc::Slip => interior.mirrored(n),
            CompBc::Extrapolate => *interior,
            CompBc::Fixed(p) => p,
            _ => *interior,
        }
    }

    /// Boundary face state of the characteristic conditions (None for the ghost-state ones).
    fn face_state(&self, pi: usize, interior: &Prim, n: Vec3) -> Option<Prim> {
        static_face_state(self.gas, &self.bcs[pi], interior, n)
    }

    /// Weighted least-squares gradients of the five primitive variables.
    fn gradients(&self, prim: &[Prim], bface: &[Prim], var: &dyn Fn(&Prim, usize) -> f64) -> Vec<[Vec3; 5]> {
        let mesh = &self.geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let mut rhs = vec![[[0.0; 3]; 5]; mesh.n_cells()];
        let mut add = |c: usize, dr: Vec3, dphi: [f64; 5]| {
            let w = 1.0 / vec3::dot(dr, dr);
            for k in 0..5 {
                for i in 0..3 {
                    rhs[c][k][i] += w * dr[i] * dphi[k];
                }
            }
        };
        for f in 0..self.geom.n_internal {
            let (o, nb) = (own[f], nei[f]);
            let d = self.geom.delta[f];
            let dphi: [f64; 5] = std::array::from_fn(|k| var(&prim[nb], k) - var(&prim[o], k));
            add(o, d, dphi);
            add(nb, vec3::scale(d, -1.0), dphi.map(|x| -x));
        }
        for f in self.geom.n_internal..mesh.n_faces() {
            if !self.geom.empty[f] {
                let o = own[f];
                let b = &bface[f - self.geom.n_internal];
                add(
                    o,
                    self.geom.delta[f],
                    std::array::from_fn(|k| var(b, k) - var(&prim[o], k)),
                );
            }
        }
        rhs.iter()
            .zip(&self.ls_inv)
            .map(|(r, inv)| {
                std::array::from_fn(|k| [0, 1, 2].map(|i| (0..3).map(|j| inv[i][j] * r[k][j]).sum::<f64>()))
            })
            .collect()
    }

    /// Residual `-(1/V) sum_f F_f S_f` for the state `u`.
    fn residual(&self, u: &[Cons]) -> Result<Vec<Cons>> {
        self.residual_walled(u, None)
    }

    /// [`Self::residual`] with the face states of the mass-spring-damper walls (`None`: no wall).
    fn residual_walled(&self, u: &[Cons], wall: Option<&[Option<Prim>]>) -> Result<Vec<Cons>> {
        let mut res = self.flux_sum(u, None, wall)?;
        let vol = self.geom.mesh.cell_volumes();
        for (r, v) in res.iter_mut().zip(vol) {
            for x in r.iter_mut() {
                *x /= v;
            }
        }
        Ok(res)
    }

    /// Numerical flux through face normal `n`; with a face normal mesh speed `vn != 0` the ALE flux `F.n - vn U`, computed
    /// in the frame moving with the face (ADR 0090 D2).
    fn face_flux(&self, l: &Prim, r: &Prim, n: Vec3, vn: f64) -> Cons {
        if vn == 0.0 {
            return self.settings.solver.flux(self.gas, l, r, n);
        }
        let v = vec3::scale(n, vn);
        let rel = |p: &Prim| Prim {
            u: vec3::sub(p.u, v),
            ..*p
        };
        lab_flux(self.settings.solver.flux(self.gas, &rel(l), &rel(r), n), v)
    }

    /// Undivided residual `-sum_f F_f |S_f|`; `wn` (per face, ALE only) is the normal mesh speed; `wall` (per boundary
    /// face, F-332 only) is the face state of a mass-spring-damper wall, which replaces the patch's condition.
    fn flux_sum(&self, u: &[Cons], wn: Option<&[f64]>, wall: Option<&[Option<Prim>]>) -> Result<Vec<Cons>> {
        let mesh = &self.geom.mesh;
        let n_cells = mesh.n_cells();
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let cf = mesh.face_centres();
        let cc = mesh.cell_centres();
        let n_int = self.geom.n_internal;
        let gas = self.gas;
        let prim: Vec<Prim> = u.iter().map(|c| to_prim(gas, c)).collect();
        if prim.iter().any(|p| !(p.rho > 0.0 && p.p > 0.0)) {
            return Err(Error::Numerical(
                "non-positive density or pressure in compressible solver".into(),
            ));
        }
        // boundary ghost states and face values
        let patch_of: Vec<usize> = {
            let mut v = vec![usize::MAX; mesh.n_faces()];
            for (pi, p) in mesh.patches().iter().enumerate() {
                for f in p.faces() {
                    v[f] = pi;
                }
            }
            v
        };
        let mut ghost = vec![prim[0]; mesh.n_faces() - n_int];
        let mut bface = vec![prim[0]; mesh.n_faces() - n_int];
        let mut direct = vec![false; mesh.n_faces() - n_int];
        for f in n_int..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let n = vec3::unit(sf[f]);
            let pi = patch_of[f];
            let vn = wn.map_or(0.0, |w| w[f]);
            let g = if vn != 0.0 && self.bcs[pi] == CompBc::Slip {
                // moving slip wall: mirror relative to the wall velocity
                let v = vec3::scale(n, vn);
                let mut g = Prim {
                    u: vec3::sub(prim[own[f]].u, v),
                    ..prim[own[f]]
                }
                .mirrored(n);
                g.u = vec3::add(g.u, v);
                g
            } else {
                self.ghost(pi, &prim[own[f]], n)
            };
            ghost[f - n_int] = g;
            let pr = &prim[own[f]];
            // F-258 (ADR 0207 M3): a characteristic condition on a moving face is evaluated in the frame of the face
            let fs = if let Some(q) = wall.and_then(|w| w[f - n_int]) {
                // F-332 (ADR 0243 D3): the face of a mass-spring-damper wall
                Some(q)
            } else if vn != 0.0 {
                characteristic_face_state(gas, &self.bcs[pi], pr, n, vn)
            } else {
                self.face_state(pi, pr, n)
            };
            if let Some(fs) = fs {
                ghost[f - n_int] = fs;
                bface[f - n_int] = fs;
                direct[f - n_int] = true;
                continue;
            }
            bface[f - n_int] = match self.bcs[pi] {
                CompBc::Slip => Prim {
                    rho: pr.rho,
                    u: vec3::sub(pr.u, vec3::scale(n, vec3::dot(pr.u, n))),
                    p: pr.p,
                },
                CompBc::Extrapolate => *pr,
                CompBc::Fixed(p) => p,
                _ => *pr,
            };
            if vn != 0.0 && self.bcs[pi] == CompBc::Slip {
                bface[f - n_int].u = vec3::add(bface[f - n_int].u, vec3::scale(n, vn));
            }
        }
        // reconstruction gradients of the 5 primitive variables
        let var = |p: &Prim, k: usize| -> f64 {
            match k {
                0 => p.rho,
                1 => p.u[0],
                2 => p.u[1],
                3 => p.u[2],
                _ => p.p,
            }
        };
        let grads: Option<Vec<[Vec3; 5]>> = self.settings.limiter.map(|_| self.gradients(&prim, &bface, &var));
        let mut res = vec![[0.0; 5]; n_cells];
        for f in 0..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let a = vec3::norm(sf[f]);
            let n = vec3::scale(sf[f], 1.0 / a);
            let o = own[f];
            let vn = wn.map_or(0.0, |w| w[f]);
            let flux = if f < n_int {
                let nb = nei[f];
                let (mut pl, mut pr) = (prim[o], prim[nb]);
                if let (Some(lim), Some(gr)) = (self.settings.limiter, grads.as_ref()) {
                    let d = self.geom.delta[f];
                    let _ = (cf, cc);
                    let mut left = [0.0; 5];
                    let mut right = [0.0; 5];
                    for k in 0..5 {
                        let (vp, vn) = (var(&prim[o], k), var(&prim[nb], k));
                        let dv = vn - vp;
                        let rp = 2.0 * vec3::dot(gr[o][k], d) / safe(dv) - 1.0;
                        let rn = 2.0 * vec3::dot(gr[nb][k], vec3::scale(d, -1.0)) / safe(-dv) - 1.0;
                        left[k] = vp + 0.5 * lim.psi(rp) * dv;
                        right[k] = vn - 0.5 * lim.psi(rn) * dv;
                    }
                    let l = Prim {
                        rho: left[0],
                        u: [left[1], left[2], left[3]],
                        p: left[4],
                    };
                    let r = Prim {
                        rho: right[0],
                        u: [right[1], right[2], right[3]],
                        p: right[4],
                    };
                    if l.rho > 0.0 && l.p > 0.0 && r.rho > 0.0 && r.p > 0.0 {
                        pl = l;
                        pr = r;
                    }
                }
                self.face_flux(&pl, &pr, n, vn)
            } else if direct[f - n_int] {
                if vn == 0.0 {
                    ghost[f - n_int].flux(gas, n)
                } else {
                    // the ALE flux F(q).n - w_n U(q): the physical flux of the state seen from the face, transformed back
                    // (ADR 0207 M3)
                    let v = vec3::scale(n, vn);
                    let q = &ghost[f - n_int];
                    lab_flux(
                        Prim {
                            u: vec3::sub(q.u, v),
                            ..*q
                        }
                        .flux(gas, n),
                        v,
                    )
                }
            } else {
                self.face_flux(&prim[o], &ghost[f - n_int], n, vn)
            };
            for k in 0..5 {
                res[o][k] -= flux[k] * a;
                if f < n_int {
                    res[nei[f]][k] += flux[k] * a;
                }
            }
        }
        Ok(res)
    }

    /// One step of size `dt` while the mesh moves from the current geometry to `new_geom` (same topology), ALE form of
    /// ADR 0090 D2: `d(V U)/dt = -sum_f (F.n - w_n U)|S_f|` with `w_n = W_f / (dt |S_f|)` from the swept volumes, so a uniform
    /// state stays uniform to round-off (discrete GCL) and the moving slip walls carry no mass. A characteristic condition on
    /// a moving face is evaluated in the frame of the face ([`characteristic_face_state`], F-258, ADR 0207 M3). The step
    /// keeps its `w_n` for [`Self::mesh_speeds`] and [`Self::max_dt_ale`].
    pub fn step_ale(&mut self, new_geom: FvGeometry, dt: f64) -> Result<()> {
        if self.msd.is_some() {
            return Err(Error::Numerical(
                "step_ale: the mass-spring-damper wall is not built for moving meshes (ADR 0243 D6); clear it first"
                    .into(),
            ));
        }
        if self.sponge.is_some() {
            if self.sponge_frame != SpongeFrame::StaticMesh {
                return self.step_ale_sponged(new_geom, dt);
            }
            return Err(Error::Numerical(
                "step_ale: the sponge layer is not built for moving meshes in its default frame (ADR 0237 D5); clear it, \
                 or attach it to the mesh with set_sponge_frame (ADR 0265)"
                    .into(),
            ));
        }
        let old = &self.geom.mesh;
        if new_geom.mesh.n_points() != old.n_points() || new_geom.mesh.n_cells() != old.n_cells() {
            return Err(Error::Numerical(
                "step_ale: the moved mesh must keep the topology".into(),
            ));
        }
        let w = swept_volumes(old, new_geom.mesh.points());
        let v0 = old.cell_volumes().to_vec();
        let dv = cell_swept_sum(old, &w);
        self.geom = new_geom;
        self.ls_inv = ls_inverse(&self.geom);
        let sf = self.geom.mesh.face_area_vectors();
        let wn: Vec<f64> = (0..w.len())
            .map(|f| w[f] / (dt * vec3::norm(sf[f]).max(1e-300)))
            .collect();
        let n = self.u.len();
        let vu0: Vec<Cons> = (0..n).map(|i| self.u[i].map(|x| x * v0[i])).collect();
        // SSP-RK stage (V U)_s = a (V U)^n + b ((V U)_prev + dt R(U_prev)); the same combination applied to dV/dt = dv/dt
        // gives the stage volume V^n + c dv
        let stage = |vu_prev: &[Cons], u_prev: &[Cons], a: f64, b: f64, c: f64| -> Result<(Vec<Cons>, Vec<Cons>)> {
            let r = self.flux_sum(u_prev, Some(&wn), None)?;
            let vu: Vec<Cons> = (0..n)
                .map(|i| std::array::from_fn(|k| a * vu0[i][k] + b * (vu_prev[i][k] + dt * r[i][k])))
                .collect();
            let u = (0..n).map(|i| vu[i].map(|x| x / (v0[i] + c * dv[i]))).collect();
            Ok((vu, u))
        };
        let u0 = self.u.clone();
        self.u = match self.settings.rk {
            RkScheme::Euler => stage(&vu0, &u0, 0.0, 1.0, 1.0)?.1,
            RkScheme::Rk2 => {
                let (vu1, u1) = stage(&vu0, &u0, 0.0, 1.0, 1.0)?;
                stage(&vu1, &u1, 0.5, 0.5, 1.0)?.1
            }
            RkScheme::Rk3 => {
                let (vu1, u1) = stage(&vu0, &u0, 0.0, 1.0, 1.0)?;
                let (vu2, u2) = stage(&vu1, &u1, 0.75, 0.25, 0.5)?;
                stage(&vu2, &u2, 1.0 / 3.0, 2.0 / 3.0, 1.0)?.1
            }
        };
        self.time += dt;
        self.steps += 1;
        // F-258 (ADR 0207 M2): the speeds of this step stand in for those of the next one in `max_dt_ale`
        self.ale_speeds = Some((self.steps, wn));
        Ok(())
    }

    /// [`Self::step_ale`] with a sponge attached to the mesh (F-341, ADR 0265 E2): the geometric statements of `step_ale`;
    /// every stage adds `V_p S(U_p, Q_p)` to the undivided residual of the cells in the layer, with `V_p = V^n + c_p dv` the
    /// volume of the stage state `U_p` (DR4), and the memories advance per cell with the stage weights of the state (DR3).
    /// Cells outside the layer are not touched, so a sponge whose rates are all zero gives the bits of `step_ale`.
    fn step_ale_sponged(&mut self, new_geom: FvGeometry, dt: f64) -> Result<()> {
        let Some(sp) = self.sponge.clone() else {
            return Err(Error::Numerical("step_ale_sponged: no sponge is set".into()));
        };
        let old = &self.geom.mesh;
        if new_geom.mesh.n_points() != old.n_points() || new_geom.mesh.n_cells() != old.n_cells() {
            return Err(Error::Numerical(
                "step_ale: the moved mesh must keep the topology".into(),
            ));
        }
        let w = swept_volumes(old, new_geom.mesh.points());
        let v0 = old.cell_volumes().to_vec();
        let dv = cell_swept_sum(old, &w);
        // F-407 (ADR 0344 DR4): the velocity of each cell centre over the step, for the memories of the lab frame
        let wc: Option<Vec<Vec3>> =
            if self.sponge_frame == SpongeFrame::AttachedWithLabMemory && sp.form != SpongeForm::Plain {
                let (c_old, c_new) = (old.cell_centres(), new_geom.mesh.cell_centres());
                Some(
                    (0..c_old.len())
                        .map(|c| vec3::scale(vec3::sub(c_new[c], c_old[c]), 1.0 / dt))
                        .collect(),
                )
            } else {
                None
            };
        self.geom = new_geom;
        self.ls_inv = ls_inverse(&self.geom);
        let sf = self.geom.mesh.face_area_vectors();
        let wn: Vec<f64> = (0..w.len())
            .map(|f| w[f] / (dt * vec3::norm(sf[f]).max(1e-300)))
            .collect();
        let n = self.u.len();
        let shifted = match sp.form {
            SpongeForm::MatchedShifted(o) => Some(o),
            _ => None,
        };
        let rate2 = &self.sponge_rate2;
        let active: Vec<bool> = (0..n)
            .map(|c| sp.rate[c] > 0.0 || (shifted.is_some() && rate2[c] > 0.0))
            .collect();
        let vu0: Vec<Cons> = (0..n).map(|i| self.u[i].map(|x| x * v0[i])).collect();
        let m0 = SpongeMemories {
            q: self.sponge_q.clone(),
            r: self.sponge_r.clone(),
            m: self.sponge_m.clone(),
        };
        // the stage of `step_ale` with the source at the volume of the stage state (`c_prev`: the `c` of the stage that
        // produced `u_prev`, 0 for the state of the start of the step) and the memories advanced with the same weights
        type StageOut = (Vec<Cons>, Vec<Cons>, SpongeMemories);
        let stage = |vu_prev: &[Cons],
                     u_prev: &[Cons],
                     m_prev: &SpongeMemories,
                     c_prev: f64,
                     a: f64,
                     b: f64,
                     c: f64|
         -> Result<StageOut> {
            let mut r = self.flux_sum(u_prev, Some(&wn), None)?;
            let mut s = vec![[0.0; 5]; n];
            let mut d = match &shifted {
                Some(o) => sponge::add_source_shifted(&self.geom, self.gas, &sp, o, rate2, u_prev, m_prev, &mut s),
                None => SpongeMemories {
                    q: sponge::add_source(&self.geom, self.gas, &sp, u_prev, &m_prev.q, &mut s),
                    r: Vec::new(),
                    m: Vec::new(),
                },
            };
            if let Some(wc) = &wc {
                sponge::add_lab_transport(&self.geom, &sp, rate2, wc, m_prev, &mut d);
            }
            for i in 0..n {
                if active[i] {
                    let vp = v0[i] + c_prev * dv[i];
                    for k in 0..5 {
                        r[i][k] += vp * s[i][k];
                    }
                }
            }
            let vu: Vec<Cons> = (0..n)
                .map(|i| std::array::from_fn(|k| a * vu0[i][k] + b * (vu_prev[i][k] + dt * r[i][k])))
                .collect();
            let u = (0..n).map(|i| vu[i].map(|x| x / (v0[i] + c * dv[i]))).collect();
            let adv = |x0: &[f64], xp: &[f64], xd: &[f64]| -> Vec<f64> {
                (0..x0.len()).map(|i| a * x0[i] + b * (xp[i] + dt * xd[i])).collect()
            };
            let mem = SpongeMemories {
                q: adv(&m0.q, &m_prev.q, &d.q),
                r: adv(&m0.r, &m_prev.r, &d.r),
                m: (0..m0.m.len())
                    .map(|i| std::array::from_fn(|k| a * m0.m[i][k] + b * (m_prev.m[i][k] + dt * d.m[i][k])))
                    .collect(),
            };
            Ok((vu, u, mem))
        };
        let u0 = self.u.clone();
        let (u_new, m_new) = match self.settings.rk {
            RkScheme::Euler => {
                let (_, u1, m1) = stage(&vu0, &u0, &m0, 0.0, 0.0, 1.0, 1.0)?;
                (u1, m1)
            }
            RkScheme::Rk2 => {
                let (vu1, u1, m1) = stage(&vu0, &u0, &m0, 0.0, 0.0, 1.0, 1.0)?;
                let (_, u2, m2) = stage(&vu1, &u1, &m1, 1.0, 0.5, 0.5, 1.0)?;
                (u2, m2)
            }
            RkScheme::Rk3 => {
                let (vu1, u1, m1) = stage(&vu0, &u0, &m0, 0.0, 0.0, 1.0, 1.0)?;
                let (vu2, u2, m2) = stage(&vu1, &u1, &m1, 1.0, 0.75, 0.25, 0.5)?;
                let (_, u3, m3) = stage(&vu2, &u2, &m2, 0.5, 1.0 / 3.0, 2.0 / 3.0, 1.0)?;
                (u3, m3)
            }
        };
        self.u = u_new;
        self.sponge_q = m_new.q;
        self.sponge_r = m_new.r;
        self.sponge_m = m_new.m;
        self.time += dt;
        self.steps += 1;
        self.ale_speeds = Some((self.steps, wn));
        Ok(())
    }

    /// Replace the mesh (any topology) and the conserved state, e.g. after a layer change (ADR 0090 D3). The mesh speeds of
    /// the last ALE step are dropped ([`Self::mesh_speeds`]).
    pub fn replace_mesh(&mut self, geom: FvGeometry, u: Vec<Cons>) -> Result<()> {
        if self.msd.is_some() {
            return Err(Error::Numerical(
                "replace_mesh: the mass-spring-damper wall holds its state per face of the old mesh (ADR 0243 D6); clear it first"
                    .into(),
            ));
        }
        if self.sponge.is_some() {
            return Err(Error::Numerical(
                "replace_mesh: the sponge layer holds one rate and its memories per cell of the old mesh (ADR 0237 D5, \
                 ADR 0265 DR7); clear it, replace the mesh, set a sponge built on the new mesh and its mapped memories"
                    .into(),
            ));
        }
        if u.len() != geom.n_cells() || geom.mesh.patches().len() != self.bcs.len() {
            return Err(Error::Numerical(
                "replace_mesh: one state per cell and the same patches are required".into(),
            ));
        }
        self.geom = geom;
        self.ls_inv = ls_inverse(&self.geom);
        self.u = u;
        self.ale_speeds = None;
        Ok(())
    }

    /// Advance one step of size `dt`.
    pub fn step(&mut self, dt: f64) -> Result<()> {
        if self.sponge.is_some() {
            return self.step_sponged(dt);
        }
        if self.msd.is_some() {
            return self.step_walled(dt);
        }
        let n = self.u.len();
        let axpy = |a: f64, x: &[Cons], b: f64, y: &[Cons], c: f64, r: &[Cons]| -> Vec<Cons> {
            (0..n)
                .map(|i| std::array::from_fn(|k| a * x[i][k] + b * y[i][k] + c * r[i][k]))
                .collect()
        };
        let u0 = self.u.clone();
        match self.settings.rk {
            RkScheme::Euler => {
                let r = self.residual(&u0)?;
                self.u = axpy(1.0, &u0, 0.0, &u0, dt, &r);
            }
            RkScheme::Rk2 => {
                let r0 = self.residual(&u0)?;
                let u1 = axpy(1.0, &u0, 0.0, &u0, dt, &r0);
                let r1 = self.residual(&u1)?;
                self.u = axpy(0.5, &u0, 0.5, &u1, 0.5 * dt, &r1);
            }
            RkScheme::Rk3 => {
                let r0 = self.residual(&u0)?;
                let u1 = axpy(1.0, &u0, 0.0, &u0, dt, &r0);
                let r1 = self.residual(&u1)?;
                let u2 = axpy(0.75, &u0, 0.25, &u1, 0.25 * dt, &r1);
                let r2 = self.residual(&u2)?;
                self.u = axpy(1.0 / 3.0, &u0, 2.0 / 3.0, &u2, 2.0 / 3.0 * dt, &r2);
            }
        }
        self.time += dt;
        self.steps += 1;
        Ok(())
    }

    /// [`Self::step`] with a sponge layer (F-331, ADR 0237 D2): the sponge source joins the residual of every stage and
    /// the memory `Q` of the matched form is advanced with the same stage weights as the state.
    fn step_sponged(&mut self, dt: f64) -> Result<()> {
        let Some(sp) = self.sponge.as_ref() else {
            return Err(Error::Numerical("step_sponged: no sponge is set".into()));
        };
        if let SpongeForm::MatchedShifted(opts) = sp.form {
            return self.step_sponged_shifted(dt, opts);
        }
        let n = self.u.len();
        let rhs = |u: &[Cons], q: &[f64]| -> Result<(Vec<Cons>, Vec<f64>)> {
            let mut r = self.residual(u)?;
            let qd = sponge::add_source(&self.geom, self.gas, sp, u, q, &mut r);
            Ok((r, qd))
        };
        let axpy = |a: f64, x: &[Cons], b: f64, y: &[Cons], c: f64, r: &[Cons]| -> Vec<Cons> {
            (0..n)
                .map(|i| std::array::from_fn(|k| a * x[i][k] + b * y[i][k] + c * r[i][k]))
                .collect()
        };
        let axpy_q = |a: f64, x: &[f64], b: f64, y: &[f64], c: f64, r: &[f64]| -> Vec<f64> {
            (0..n).map(|i| a * x[i] + b * y[i] + c * r[i]).collect()
        };
        let (u0, q0) = (&self.u, &self.sponge_q);
        let (u_new, q_new) = match self.settings.rk {
            RkScheme::Euler => {
                let (r, qd) = rhs(u0, q0)?;
                (axpy(1.0, u0, 0.0, u0, dt, &r), axpy_q(1.0, q0, 0.0, q0, dt, &qd))
            }
            RkScheme::Rk2 => {
                let (r0, qd0) = rhs(u0, q0)?;
                let (u1, q1) = (axpy(1.0, u0, 0.0, u0, dt, &r0), axpy_q(1.0, q0, 0.0, q0, dt, &qd0));
                let (r1, qd1) = rhs(&u1, &q1)?;
                (
                    axpy(0.5, u0, 0.5, &u1, 0.5 * dt, &r1),
                    axpy_q(0.5, q0, 0.5, &q1, 0.5 * dt, &qd1),
                )
            }
            RkScheme::Rk3 => {
                let (r0, qd0) = rhs(u0, q0)?;
                let (u1, q1) = (axpy(1.0, u0, 0.0, u0, dt, &r0), axpy_q(1.0, q0, 0.0, q0, dt, &qd0));
                let (r1, qd1) = rhs(&u1, &q1)?;
                let (u2, q2) = (
                    axpy(0.75, u0, 0.25, &u1, 0.25 * dt, &r1),
                    axpy_q(0.75, q0, 0.25, &q1, 0.25 * dt, &qd1),
                );
                let (r2, qd2) = rhs(&u2, &q2)?;
                (
                    axpy(1.0 / 3.0, u0, 2.0 / 3.0, &u2, 2.0 / 3.0 * dt, &r2),
                    axpy_q(1.0 / 3.0, q0, 2.0 / 3.0, &q2, 2.0 / 3.0 * dt, &qd2),
                )
            }
        };
        self.u = u_new;
        self.sponge_q = q_new;
        self.time += dt;
        self.steps += 1;
        Ok(())
    }

    /// [`Self::step`] with the shifted matched sponge form (F-340, ADR 0248 E2): the source of DR14 joins the residual of
    /// every stage and the memories `q`, `r` and the momentum memory advance with the stage weights of the state. With
    /// one layer and without shift and damping along the layer this is `step_sponged` of the matched form, operation
    /// for operation, on the state and on `q`.
    fn step_sponged_shifted(&mut self, dt: f64, opts: MatchedOptions) -> Result<()> {
        let Some(sp) = self.sponge.as_ref() else {
            return Err(Error::Numerical("step_sponged_shifted: no sponge is set".into()));
        };
        let n = self.u.len();
        let rhs = |u: &[Cons], mem: &SpongeMemories| -> Result<(Vec<Cons>, SpongeMemories)> {
            let mut r = self.residual(u)?;
            let d = sponge::add_source_shifted(&self.geom, self.gas, sp, &opts, &self.sponge_rate2, u, mem, &mut r);
            Ok((r, d))
        };
        let axpy = |a: f64, x: &[Cons], b: f64, y: &[Cons], c: f64, r: &[Cons]| -> Vec<Cons> {
            (0..n)
                .map(|i| std::array::from_fn(|k| a * x[i][k] + b * y[i][k] + c * r[i][k]))
                .collect()
        };
        let axpy_q = |a: f64, x: &[f64], b: f64, y: &[f64], c: f64, r: &[f64]| -> Vec<f64> {
            (0..n).map(|i| a * x[i] + b * y[i] + c * r[i]).collect()
        };
        let axpy_m =
            |a: f64, x: &SpongeMemories, b: f64, y: &SpongeMemories, c: f64, r: &SpongeMemories| SpongeMemories {
                q: axpy_q(a, &x.q, b, &y.q, c, &r.q),
                r: axpy_q(a, &x.r, b, &y.r, c, &r.r),
                m: (0..n)
                    .map(|i| std::array::from_fn(|k| a * x.m[i][k] + b * y.m[i][k] + c * r.m[i][k]))
                    .collect(),
            };
        let u0 = &self.u;
        let m0 = SpongeMemories {
            q: self.sponge_q.clone(),
            r: self.sponge_r.clone(),
            m: self.sponge_m.clone(),
        };
        let (u_new, m_new) = match self.settings.rk {
            RkScheme::Euler => {
                let (r, d) = rhs(u0, &m0)?;
                (axpy(1.0, u0, 0.0, u0, dt, &r), axpy_m(1.0, &m0, 0.0, &m0, dt, &d))
            }
            RkScheme::Rk2 => {
                let (r0, d0) = rhs(u0, &m0)?;
                let (u1, m1) = (axpy(1.0, u0, 0.0, u0, dt, &r0), axpy_m(1.0, &m0, 0.0, &m0, dt, &d0));
                let (r1, d1) = rhs(&u1, &m1)?;
                (
                    axpy(0.5, u0, 0.5, &u1, 0.5 * dt, &r1),
                    axpy_m(0.5, &m0, 0.5, &m1, 0.5 * dt, &d1),
                )
            }
            RkScheme::Rk3 => {
                let (r0, d0) = rhs(u0, &m0)?;
                let (u1, m1) = (axpy(1.0, u0, 0.0, u0, dt, &r0), axpy_m(1.0, &m0, 0.0, &m0, dt, &d0));
                let (r1, d1) = rhs(&u1, &m1)?;
                let (u2, m2) = (
                    axpy(0.75, u0, 0.25, &u1, 0.25 * dt, &r1),
                    axpy_m(0.75, &m0, 0.25, &m1, 0.25 * dt, &d1),
                );
                let (r2, d2) = rhs(&u2, &m2)?;
                (
                    axpy(1.0 / 3.0, u0, 2.0 / 3.0, &u2, 2.0 / 3.0 * dt, &r2),
                    axpy_m(1.0 / 3.0, &m0, 2.0 / 3.0, &m2, 2.0 / 3.0 * dt, &d2),
                )
            }
        };
        self.u = u_new;
        self.sponge_q = m_new.q;
        self.sponge_r = m_new.r;
        self.sponge_m = m_new.m;
        self.time += dt;
        self.steps += 1;
        Ok(())
    }

    /// [`Self::step`] with mass-spring-damper walls (F-332, ADR 0243 DR4): the wall faces take the face state of the
    /// stage's wall state, and the wall state `(xi, v)` is advanced with the same stage weights as the conserved state.
    /// A step above [`Self::msd_dt_limit`] is refused.
    fn step_walled(&mut self, dt: f64) -> Result<()> {
        let Some(st) = self.msd.as_ref() else {
            return Err(Error::Numerical("step_walled: no wall is set".into()));
        };
        let lim = self.msd_dt_limit();
        if dt > lim * (1.0 + 1e-12) {
            return Err(Error::Numerical(format!(
                "step: the step {dt} is above the stability limit {lim} of the mass-spring-damper wall (ADR 0243 D5); use max_dt()"
            )));
        }
        let n = self.u.len();
        let nb = st.xi.len();
        let rhs = |u: &[Cons], xi: &[f64], v: &[f64]| -> Result<(Vec<Cons>, Vec<f64>, Vec<f64>)> {
            let (faces, xd, vd) = self.msd_faces(u, xi, v)?;
            Ok((self.residual_walled(u, Some(&faces))?, xd, vd))
        };
        let axpy = |a: f64, x: &[Cons], b: f64, y: &[Cons], c: f64, r: &[Cons]| -> Vec<Cons> {
            (0..n)
                .map(|i| std::array::from_fn(|k| a * x[i][k] + b * y[i][k] + c * r[i][k]))
                .collect()
        };
        let axpy_w = |a: f64, x: &[f64], b: f64, y: &[f64], c: f64, r: &[f64]| -> Vec<f64> {
            (0..nb).map(|i| a * x[i] + b * y[i] + c * r[i]).collect()
        };
        let (u0, x0, v0) = (&self.u, &st.xi, &st.v);
        let (u_new, x_new, v_new) = match self.settings.rk {
            RkScheme::Euler => {
                let (r, xd, vd) = rhs(u0, x0, v0)?;
                (
                    axpy(1.0, u0, 0.0, u0, dt, &r),
                    axpy_w(1.0, x0, 0.0, x0, dt, &xd),
                    axpy_w(1.0, v0, 0.0, v0, dt, &vd),
                )
            }
            RkScheme::Rk2 => {
                let (r0, xd0, vd0) = rhs(u0, x0, v0)?;
                let (u1, x1, v1) = (
                    axpy(1.0, u0, 0.0, u0, dt, &r0),
                    axpy_w(1.0, x0, 0.0, x0, dt, &xd0),
                    axpy_w(1.0, v0, 0.0, v0, dt, &vd0),
                );
                let (r1, xd1, vd1) = rhs(&u1, &x1, &v1)?;
                (
                    axpy(0.5, u0, 0.5, &u1, 0.5 * dt, &r1),
                    axpy_w(0.5, x0, 0.5, &x1, 0.5 * dt, &xd1),
                    axpy_w(0.5, v0, 0.5, &v1, 0.5 * dt, &vd1),
                )
            }
            RkScheme::Rk3 => {
                let (r0, xd0, vd0) = rhs(u0, x0, v0)?;
                let (u1, x1, v1) = (
                    axpy(1.0, u0, 0.0, u0, dt, &r0),
                    axpy_w(1.0, x0, 0.0, x0, dt, &xd0),
                    axpy_w(1.0, v0, 0.0, v0, dt, &vd0),
                );
                let (r1, xd1, vd1) = rhs(&u1, &x1, &v1)?;
                let (u2, x2, v2) = (
                    axpy(0.75, u0, 0.25, &u1, 0.25 * dt, &r1),
                    axpy_w(0.75, x0, 0.25, &x1, 0.25 * dt, &xd1),
                    axpy_w(0.75, v0, 0.25, &v1, 0.25 * dt, &vd1),
                );
                let (r2, xd2, vd2) = rhs(&u2, &x2, &v2)?;
                (
                    axpy(1.0 / 3.0, u0, 2.0 / 3.0, &u2, 2.0 / 3.0 * dt, &r2),
                    axpy_w(1.0 / 3.0, x0, 2.0 / 3.0, &x2, 2.0 / 3.0 * dt, &xd2),
                    axpy_w(1.0 / 3.0, v0, 2.0 / 3.0, &v2, 2.0 / 3.0 * dt, &vd2),
                )
            }
        };
        self.u = u_new;
        if let Some(st) = self.msd.as_mut() {
            st.xi = x_new;
            st.v = v_new;
        }
        self.time += dt;
        self.steps += 1;
        Ok(())
    }

    /// Advance to physical time `t_end` with CFL-limited steps (last step clipped).
    pub fn run_to(&mut self, t_end: f64) -> Result<()> {
        while self.time < t_end - 1e-14 * t_end.abs().max(1.0) {
            let dt = self.max_dt().min(t_end - self.time);
            self.step(dt)?;
        }
        Ok(())
    }

    /// Advance with CFL-limited steps until the largest relative density
    /// change in a step falls below `tol` (steady state) or `max_steps` is
    /// reached. Returns `(steps, final_change)`.
    pub fn run_steady(&mut self, tol: f64, max_steps: usize) -> Result<(usize, f64)> {
        let mut change = f64::INFINITY;
        for i in 0..max_steps {
            let before: Vec<f64> = self.u.iter().map(|c| c[0]).collect();
            let dt = self.max_dt();
            self.step(dt)?;
            change = self
                .u
                .iter()
                .zip(&before)
                .map(|(c, b)| (c[0] - b).abs() / b)
                .fold(0.0, f64::max);
            if change < tol {
                return Ok((i + 1, change));
            }
        }
        Ok((max_steps, change))
    }

    /// Total conserved quantities `sum_c U_c V_c`.
    pub fn totals(&self) -> Cons {
        let vol = self.geom.mesh.cell_volumes();
        let mut t = [0.0; 5];
        for (c, v) in self.u.iter().zip(vol) {
            for k in 0..5 {
                t[k] += c[k] * v;
            }
        }
        t
    }
}

/// The lab-frame flux of a flux `f` computed in the frame moving with the velocity `v` (ADR 0090 D2): `F_rho = F'_rho`,
/// `F_m = F'_m + v F'_rho`, `F_E = F'_E + v.F'_m + |v|^2/2 F'_rho`.
fn lab_flux(f: Cons, v: Vec3) -> Cons {
    [
        f[0],
        f[1] + v[0] * f[0],
        f[2] + v[1] * f[0],
        f[3] + v[2] * f[0],
        f[4] + v[0] * f[1] + v[1] * f[2] + v[2] * f[3] + 0.5 * vec3::dot(v, v) * f[0],
    ]
}

/// Face state of a characteristic condition on a static face (ADR 0006); `None` for the ghost-state conditions.
fn static_face_state(gas: Gas, bc: &CompBc, interior: &Prim, n: Vec3) -> Option<Prim> {
    match *bc {
        CompBc::TotalInlet(t) => Some(compressible_bc::total_inlet(gas, &t, interior, n)),
        CompBc::PressureOutlet { p } => Some(compressible_bc::pressure_outlet(gas, p, 1.0, interior, n)),
        CompBc::NonReflecting { p, sigma } => Some(compressible_bc::pressure_outlet(gas, p, sigma, interior, n)),
        CompBc::FarField(fs) => Some(compressible_bc::far_field(gas, &fs, interior, n)),
        CompBc::Impedance { p, z } => Some(compressible_bc::impedance_wall(gas, p, z, interior, n)),
        _ => None,
    }
}

/// The lab-frame face state of a characteristic condition (ADR 0006) on a face moving with the normal mesh speed `wn`
/// along the outward unit normal `n` (F-258, ADR 0207 M3). The condition is evaluated in the frame of the face,
/// `v = wn n`: the pressure outlet, the non-reflecting outlet and the far field see the interior state and the free stream
/// (a lab-frame state) with `v` removed, and their result is shifted back by `v`, so their switches read the normal velocity
/// relative to the face. The total inlet's total conditions and direction are lab-frame data; `wn` cancels in its outgoing
/// invariant, so its face state is the static one. `None` for slip, extrapolated and fixed faces. With `wn == 0.0` this is
/// the static face state of [`compressible_bc`], bit for bit.
pub fn characteristic_face_state(gas: Gas, bc: &CompBc, interior: &Prim, n: Vec3, wn: f64) -> Option<Prim> {
    if wn == 0.0 {
        return static_face_state(gas, bc, interior, n);
    }
    let v = vec3::scale(n, wn);
    let rel = |p: &Prim| Prim {
        u: vec3::sub(p.u, v),
        ..*p
    };
    let back = |p: Prim| Prim {
        u: vec3::add(p.u, v),
        ..p
    };
    Some(match *bc {
        CompBc::TotalInlet(t) => compressible_bc::total_inlet(gas, &t, interior, n),
        CompBc::PressureOutlet { p } => back(compressible_bc::pressure_outlet(gas, p, 1.0, &rel(interior), n)),
        CompBc::NonReflecting { p, sigma } => back(compressible_bc::pressure_outlet(gas, p, sigma, &rel(interior), n)),
        CompBc::FarField(fs) => back(compressible_bc::far_field(gas, &rel(&fs), &rel(interior), n)),
        CompBc::Impedance { p, z } => back(compressible_bc::impedance_wall(gas, p, z, &rel(interior), n)),
        _ => return None,
    })
}

/// Inverse of the (masked) inverse-distance weighted least-squares matrix of each cell.
pub(crate) fn ls_inverse(geom: &FvGeometry) -> Vec<[[f64; 3]; 3]> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut mat = vec![[[0.0; 3]; 3]; mesh.n_cells()];
    let mut add = |c: usize, dr: Vec3| {
        let w = 1.0 / vec3::dot(dr, dr);
        for i in 0..3 {
            for j in 0..3 {
                mat[c][i][j] += w * dr[i] * dr[j];
            }
        }
    };
    for f in 0..geom.n_internal {
        add(own[f], geom.delta[f]);
        add(nei[f], geom.delta[f]);
    }
    for f in geom.n_internal..mesh.n_faces() {
        if !geom.empty[f] {
            add(own[f], geom.delta[f]);
        }
    }
    mat.into_iter()
        .map(|mut m| {
            for d in 0..3 {
                if !geom.active[d] {
                    for k in 0..3 {
                        m[d][k] = 0.0;
                        m[k][d] = 0.0;
                    }
                    m[d][d] = 1.0;
                }
            }
            let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
            if det.abs() < 1e-300 {
                return [[0.0; 3]; 3];
            }
            let mut inv = [[0.0; 3]; 3];
            for r in 0..3 {
                for c in 0..3 {
                    let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
                    let (c1, c2) = ((c + 1) % 3, (c + 2) % 3);
                    inv[r][c] = (m[c1][r1] * m[c2][r2] - m[c1][r2] * m[c2][r1]) / det;
                }
            }
            for d in 0..3 {
                if !geom.active[d] {
                    inv[d] = [0.0; 3];
                }
            }
            inv
        })
        .collect()
}

pub(crate) fn safe(x: f64) -> f64 {
    if x.abs() < 1e-300 {
        if x < 0.0 {
            -1e-300
        } else {
            1e-300
        }
    } else {
        x
    }
}

/// L1 norm `sum |a_c - b_c| V_c / sum V_c` of the difference between two cell fields.
pub fn l1_error(geom: &FvGeometry, a: &[f64], b: &[f64]) -> f64 {
    let vol = geom.mesh.cell_volumes();
    let num: f64 = a.iter().zip(b).zip(vol).map(|((x, y), v)| (x - y).abs() * v).sum();
    num / vol.iter().sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riemann::{oblique_shock_beta, ExactRiemann};
    use rcfd_mesh::gen::box_2d;
    use std::sync::Arc;

    /// 1D shock tube on `[0,1]` with `n` cells (2D mesh, one cell tall; the
    /// y-walls act as slip walls and the front/back are empty).
    fn sod(n: usize, settings: CompSettings, t_end: f64) -> (CompressibleSolver, Vec<[f64; 3]>) {
        let mesh = Arc::new(box_2d(n, 1, 1.0, 1.0 / n as f64, false, false).unwrap());
        let geom = FvGeometry::new(mesh).unwrap();
        let gas = Gas::AIR;
        let left = Prim {
            rho: 1.0,
            u: [0.0; 3],
            p: 1.0,
        };
        let right = Prim {
            rho: 0.125,
            u: [0.0; 3],
            p: 0.1,
        };
        let mut s = CompressibleSolver::new(geom, gas, settings, |x| if x[0] < 0.5 { left } else { right }).unwrap();
        s.set_bc("xmin", CompBc::Fixed(left)).unwrap();
        s.set_bc("xmax", CompBc::Fixed(right)).unwrap();
        s.run_to(t_end).unwrap();
        let ex = ExactRiemann::new(gas, [1.0, 0.0, 1.0], [0.125, 0.0, 0.1]).unwrap();
        let exact: Vec<[f64; 3]> = s
            .geom
            .mesh
            .cell_centres()
            .iter()
            .map(|x| ex.sample((x[0] - 0.5) / t_end))
            .collect();
        (s, exact)
    }

    fn sod_errors(n: usize, settings: CompSettings) -> [f64; 3] {
        let (s, exact) = sod(n, settings, 0.2);
        let prim = s.primitives();
        let rho: Vec<f64> = prim.iter().map(|p| p.rho).collect();
        let u: Vec<f64> = prim.iter().map(|p| p.u[0]).collect();
        let p: Vec<f64> = prim.iter().map(|p| p.p).collect();
        let er: Vec<f64> = exact.iter().map(|e| e[0]).collect();
        let eu: Vec<f64> = exact.iter().map(|e| e[1]).collect();
        let ep: Vec<f64> = exact.iter().map(|e| e[2]).collect();
        [
            l1_error(&s.geom, &rho, &er),
            l1_error(&s.geom, &u, &eu),
            l1_error(&s.geom, &p, &ep),
        ]
    }

    #[test]
    fn sod_l1_convergence_hllc_muscl() {
        let st = CompSettings::default();
        let e: Vec<[f64; 3]> = [100, 200, 400].iter().map(|&n| sod_errors(n, st)).collect();
        for (n, e) in [100, 200, 400].iter().zip(&e) {
            println!(
                "Sod HLLC+vanLeer RK3 n={n}: L1(rho,u,p) = {:.4e} {:.4e} {:.4e}",
                e[0], e[1], e[2]
            );
        }
        let o1 = (e[0][0] / e[1][0]).log2();
        let o2 = (e[1][0] / e[2][0]).log2();
        println!("observed density L1 orders: {o1:.2} {o2:.2}");
        assert!(e[2][0] < 3.0e-3, "fine-grid density L1 {}", e[2][0]);
        assert!(o1 > 0.7 && o2 > 0.7, "orders {o1} {o2}");
    }

    #[test]
    fn sod_all_solvers_and_first_order() {
        let mut first = None;
        for (name, solver) in [
            ("rusanov", RiemannSolver::Rusanov),
            ("roe", RiemannSolver::Roe),
            ("hllc", RiemannSolver::Hllc),
            ("ausm+", RiemannSolver::AusmPlus),
        ] {
            let st = CompSettings {
                solver,
                ..CompSettings::default()
            };
            let e = sod_errors(200, st);
            println!("Sod n=200 {name} MUSCL: L1 rho = {:.4e}", e[0]);
            assert!(e[0] < 5.0e-3, "{name}: {}", e[0]);
            if solver == RiemannSolver::Hllc {
                first = Some(e[0]);
            }
        }
        let st1 = CompSettings {
            limiter: None,
            rk: RkScheme::Rk2,
            ..CompSettings::default()
        };
        let e1 = sod_errors(200, st1);
        println!("Sod n=200 hllc first order: L1 rho = {:.4e}", e1[0]);
        assert!(e1[0] > first.unwrap(), "MUSCL must beat first order");
    }

    /// Supersonic flow past a compression ramp: M = 2, theta = 10 degrees. The
    /// measured wave angle is compared with the theta-beta-M relation.
    fn wedge_run(nx: usize, ny: usize, settings: CompSettings, t_end: f64) -> (f64, f64, f64, f64) {
        use rcfd_mesh::gen::warp;
        let (lx, ly, x0, th) = (1.5, 1.0, 0.2, 10f64.to_radians());
        let base = box_2d(nx, ny, lx, ly, false, false).unwrap();
        let mesh = warp(&base, |p| {
            let rise = (p[0] - x0).max(0.0) * th.tan();
            [p[0], p[1] + rise * (1.0 - p[1] / ly), p[2]]
        })
        .unwrap();
        let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
        let gas = Gas::AIR;
        let fs = Prim {
            rho: 1.0,
            u: [2.0, 0.0, 0.0], // a = sqrt(1.4 * (1/1.4) / 1) = 1
            p: 1.0 / 1.4,
        };
        let mut s = CompressibleSolver::new(geom, gas, settings, |_| fs).unwrap();
        s.set_bc("xmin", CompBc::Fixed(fs)).unwrap();
        s.set_bc("xmax", CompBc::Extrapolate).unwrap();
        s.set_bc("ymax", CompBc::Extrapolate).unwrap();
        s.set_bc("ymin", CompBc::Slip).unwrap();
        s.run_to(t_end).unwrap();
        // steadiness: one more unit of time changes the density by very little
        let before = s.u.iter().map(|c| c[0]).collect::<Vec<_>>();
        s.run_to(t_end + 0.5).unwrap();
        let drift =
            s.u.iter()
                .zip(&before)
                .map(|(c, b)| (c[0] - b).abs())
                .fold(0.0, f64::max);
        let prim = s.primitives();
        let cc = s.geom.mesh.cell_centres();
        let beta = oblique_shock_beta(1.4, 2.0, th).unwrap();
        let p2 = fs.p * (1.0 + 2.0 * 1.4 / 2.4 * (4.0 * beta.sin().powi(2) - 1.0));
        let mid = 0.5 * (fs.p + p2);
        // shock position per column: first crossing of the mid pressure scanning down from the top
        let mut cols: std::collections::BTreeMap<i64, Vec<(f64, f64)>> = Default::default();
        for (c, x) in cc.iter().enumerate() {
            cols.entry((x[0] * 1e9).round() as i64)
                .or_default()
                .push((x[1], prim[c].p));
        }
        let (mut sx, mut sy, mut sxx, mut sxy, mut n) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (k, mut col) in cols {
            let x = k as f64 * 1e-9;
            if !(0.5..=1.2).contains(&x) {
                continue;
            }
            col.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            for w in col.windows(2) {
                if w[0].1 < mid && w[1].1 >= mid {
                    let t = (mid - w[0].1) / (w[1].1 - w[0].1);
                    let y = w[0].0 + t * (w[1].0 - w[0].0);
                    sx += x;
                    sy += y;
                    sxx += x * x;
                    sxy += x * y;
                    n += 1.0;
                    break;
                }
            }
        }
        let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
        let beta_meas = slope.atan();
        // post-shock plateau pressure sampled at x = 1.0 just above the wall
        let p_post = cc
            .iter()
            .enumerate()
            .filter(|(_, x)| (x[0] - 1.0).abs() < 0.5 * lx / nx as f64)
            .min_by(|a, b| a.1[1].partial_cmp(&b.1[1]).unwrap())
            .map(|(c, _)| prim[c].p)
            .unwrap();
        (beta_meas, beta, p_post / p2, drift)
    }

    #[test]
    #[ignore = "heavy: 200x70 cells, about 1 min in release"]
    fn oblique_shock_fine_grid() {
        let (bm, be, pr, drift) = wedge_run(200, 70, CompSettings::default(), 3.5);
        println!(
            "fine 200x70: beta {:.3} deg vs {:.3} deg, p2 ratio {pr:.4}, drift {drift:.1e}",
            bm.to_degrees(),
            be.to_degrees()
        );
        assert!((bm - be).abs() / be < 0.01 && (pr - 1.0).abs() < 0.02 && drift < 1e-4);
    }

    #[test]
    fn oblique_shock_angle_within_one_percent() {
        let (bm, be, pr, drift) = wedge_run(100, 35, CompSettings::default(), 3.0);
        println!(
            "wedge M=2 theta=10: beta measured {:.3} deg, theory {:.3} deg (err {:.3} %), p2/p2_theory {:.4}, steady drift {:.2e}",
            bm.to_degrees(),
            be.to_degrees(),
            100.0 * (bm - be).abs() / be,
            pr,
            drift
        );
        assert!(
            (bm - be).abs() / be < 0.01,
            "beta {} vs {}",
            bm.to_degrees(),
            be.to_degrees()
        );
        assert!((pr - 1.0).abs() < 0.02, "post-shock pressure ratio {pr}");
        assert!(drift < 1e-4, "not steady: {drift}");
    }

    #[test]
    fn sod_solution_is_monotone_and_conservative() {
        let (s, _) = sod(200, CompSettings::default(), 0.2);
        let prim = s.primitives();
        // total mass/energy: waves have not reached the ends, so totals are exactly the initial ones
        let vol = s.geom.mesh.cell_volumes()[0];
        let n = prim.len() as f64;
        let mass0 = (0.5 * 1.0 + 0.5 * 0.125) * n * vol;
        let e0 = (0.5 * 1.0 / 0.4 + 0.5 * 0.1 / 0.4) * n * vol;
        let t = s.totals();
        assert!((t[0] - mass0).abs() < 1e-12 * mass0, "mass {} vs {}", t[0], mass0);
        assert!((t[4] - e0).abs() < 1e-12 * e0, "energy");
        // overshoot of density/pressure beyond the initial extrema must be tiny
        let rmax = prim.iter().map(|p| p.rho).fold(0.0, f64::max);
        let rmin = prim.iter().map(|p| p.rho).fold(1e9, f64::min);
        assert!(rmax < 1.0 + 1e-3 && rmin > 0.125 - 1e-3, "rho range {rmin}..{rmax}");
        // transverse momentum stays zero
        assert!(s.u.iter().all(|c| c[2].abs() < 1e-13 && c[3].abs() < 1e-13));
    }
}
