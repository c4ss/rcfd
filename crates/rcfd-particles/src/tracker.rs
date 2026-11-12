//! Particle state, the tracker and wall interaction (ADR 0028 D3, D5, D6; erosion and rough walls ADR 0194; dispersion
//! fields and the surface reaction ADR 0208; droplet evaporation and TAB breakup ADR 0224; their exchange ADR 0230).

use crate::breakup::{normal_speed, oscillator, sauter_radius, Breakup, BreakupSpec, Oscillator};
use crate::coupling::{nusselt, tau_thermal, Coupling, Thermal};
use crate::dispersion::{Dispersion, Eddy};
use crate::drag::{drag_factor, reflect, step_particle, tau_p, DragLaw, Fluid, Scheme};
use crate::erosion::{Erosion, ErosionSpec};
use crate::evaporation::{phi1, EvapRates, Evaporation, EvaporationSpec};
use crate::film::{Film, FilmGas, FilmSpec};
use crate::inject::{InjKind, InjectorExt, InjectorSpec, VelMode};
use crate::locate::{Locator, Walk};
use crate::reaction::{ReactionSpec, SurfaceReaction};
use crate::rng::SplitMix64;
use crate::rough::{tangent_pair, RoughSpec, RoughWall};
use crate::{err, Result};
use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, Vec3};
use rcfd_mesh::{Mesh, PatchKind};
use std::sync::Arc;

/// Fluid-velocity interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interp {
    /// The value of the containing cell.
    CellCentre,
    /// Cell value plus `(x - x_c) . grad u` (needs the gradient in [`Flow`]).
    Gradient,
}

/// What a boundary does to a particle that reaches it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WallAction {
    /// Specular reflection with restitution `e` in `[0, 1]`.
    Reflect(f64),
    /// The particle is deposited.
    Stick,
    /// The particle leaves the domain.
    Escape,
    /// The particle joins the wall film of the face (ADR 0192; set by [`Tracker::set_film`]).
    Film,
    /// Reflection with restitution `e` in `(0, 1]` about a randomly tilted virtual wall (ADR 0194; set by
    /// [`Tracker::set_rough_wall`]).
    Rough(f64),
}

/// Life-cycle state of a particle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Being tracked.
    Active = 0,
    /// Deposited on a wall.
    Deposited = 1,
    /// Left through an escape boundary.
    Escaped = 2,
    /// Stopped after too many reflections in one step.
    Stuck = 3,
    /// Absorbed into the wall film (ADR 0192); its mass is in the film.
    Film = 4,
    /// Evaporated to the minimum diameter (ADR 0224 E6); its mass is in the vapour booking.
    Evaporated = 5,
}

/// One parcel.
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    /// Identifier (injection order).
    pub id: u64,
    /// Position.
    pub x: Vec3,
    /// Velocity.
    pub v: Vec3,
    /// Diameter [m].
    pub d: f64,
    /// Density [kg/m^3].
    pub rho: f64,
    /// Real particles represented.
    pub parcel: f64,
    /// Containing cell.
    pub cell: usize,
    /// Time since injection [s].
    pub age: f64,
    /// State.
    pub state: State,
}

/// Fluid data for a tracking step.
#[derive(Debug, Clone, Copy)]
pub struct Flow<'a> {
    /// Cell-centre velocity.
    pub u: &'a [Vec3],
    /// Per cell, the gradient of each velocity component: `grad[c][i] = grad(u_i)`.
    pub grad: Option<&'a [[Vec3; 3]]>,
    /// Fluid density [kg/m^3].
    pub rho: f64,
    /// Dynamic viscosity [Pa s].
    pub mu: f64,
}

/// Tracker options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Drag law.
    pub drag: DragLaw,
    /// Integration scheme.
    pub scheme: Scheme,
    /// Interpolation.
    pub interp: Interp,
    /// Gravity [m/s^2].
    pub gravity: Vec3,
    /// Maximum sub-step as a fraction of the time to cross one cell length.
    pub courant: f64,
    /// Maximum sub-step as a fraction of the relaxation time (Euler, RK2, RK4).
    pub relax_fraction: f64,
    /// Reflections in one `advance` before a particle is declared stuck.
    pub max_bounces: usize,
    /// Action of `Wall` patches that have no override.
    pub default_wall: WallAction,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            drag: DragLaw::SchillerNaumann,
            scheme: Scheme::Analytic,
            interp: Interp::Gradient,
            gravity: [0.0; 3],
            courant: 0.5,
            relax_fraction: 0.1,
            max_bounces: 1000,
            default_wall: WallAction::Reflect(1.0),
        }
    }
}

/// Per-parcel state of the extensions (kept beside `Particle` so that its literal stays valid).
#[derive(Debug, Clone, Copy)]
struct Aux {
    eddy: Eddy,
    /// Particle temperature [K]; NaN = not yet set.
    tp: f64,
    /// Reacting mass per real particle [kg] (ADR 0208 R2); NaN = not yet set.
    m_r: f64,
    /// Inert mass per real particle [kg] (ADR 0208 R2).
    m_i: f64,
    /// TAB distortion `y` and its rate (ADR 0224 B2).
    y: f64,
    yd: f64,
    /// Breakups of the parcel and its age at the last one (NaN = none).
    n_bu: u32,
    t_bu: f64,
}

impl Default for Aux {
    fn default() -> Self {
        Aux {
            eddy: Eddy::default(),
            tp: f64::NAN,
            m_r: f64::NAN,
            m_i: f64::NAN,
            y: 0.0,
            yd: 0.0,
            n_bu: 0,
            t_bu: f64::NAN,
        }
    }
}

/// Dispersion context: model, per-cell turbulence fields and the generator.
struct DispCtx {
    model: Dispersion,
    k: Vec<f64>,
    eps: Vec<f64>,
    rng: SplitMix64,
}

/// Extension contexts handed to the per-parcel advance.
struct Ctx<'a> {
    disp: Option<&'a mut DispCtx>,
    thermal: Option<&'a Thermal>,
    cpl: Option<&'a mut Coupling>,
    /// Latent heat of the evaporation [J/kg] (for the heat and the vapour enthalpy of the record, ADR 0230 D3).
    latent: f64,
    film: Option<&'a mut Film>,
    erosion: Option<&'a mut Erosion>,
    rough: Option<&'a mut RoughWall>,
    reaction: Option<&'a mut SurfaceReaction>,
    evap: Option<&'a mut Evaporation>,
    breakup: Option<&'a mut Breakup>,
}

struct Injector {
    spec: InjectorSpec,
    ext: InjectorExt,
    rng: SplitMix64,
    emitted: u64,
    /// Surface injection: faces and cumulative areas.
    faces: Vec<usize>,
    cum_area: Vec<f64>,
}

/// The particle tracker.
pub struct Tracker {
    loc: Locator,
    /// Options.
    pub opts: Options,
    actions: Vec<WallAction>,
    injectors: Vec<Injector>,
    aux: Vec<Aux>,
    disp: Option<DispCtx>,
    thermal: Option<Thermal>,
    coupling: Option<Coupling>,
    /// The record was started with the droplet terms (ADR 0230 D1).
    droplet_exchange: bool,
    film: Option<Film>,
    erosion: Option<Erosion>,
    rough: Option<RoughWall>,
    reaction: Option<SurfaceReaction>,
    evaporation: Option<Evaporation>,
    breakup: Option<Breakup>,
    /// All parcels, including finished ones.
    pub particles: Vec<Particle>,
    /// Current time [s].
    pub time: f64,
}

/// Fluid velocity at `x` inside cell `c`.
pub fn fluid_at(mesh: &Mesh, flow: &Flow, interp: Interp, c: usize, x: Vec3) -> Vec3 {
    let uc = flow.u[c];
    match (interp, flow.grad) {
        (Interp::Gradient, Some(g)) => {
            let dx = sub(x, mesh.cell_centres()[c]);
            [
                uc[0] + dot(dx, g[c][0]),
                uc[1] + dot(dx, g[c][1]),
                uc[2] + dot(dx, g[c][2]),
            ]
        }
        _ => uc,
    }
}

impl Tracker {
    /// Build a tracker. `overrides` assign an action to named boundary patches; the others follow the patch kind.
    pub fn new(mesh: Arc<Mesh>, opts: Options, overrides: &[(String, WallAction)]) -> Result<Tracker> {
        let nint = mesh.n_internal_faces();
        let mut actions = vec![WallAction::Escape; mesh.n_boundary_faces()];
        for p in mesh.patches() {
            let a = match p.kind {
                PatchKind::Wall => opts.default_wall,
                PatchKind::Symmetry | PatchKind::Empty => WallAction::Reflect(1.0),
                PatchKind::Outlet | PatchKind::Inlet | PatchKind::Generic => WallAction::Escape,
                PatchKind::Cyclic => {
                    return err(format!(
                        "patch '{}' is cyclic: particle tracking across periodic patches is not available (ADR 0028)",
                        p.name
                    ))
                }
            };
            for f in p.faces() {
                actions[f - nint] = a;
            }
        }
        for (name, a) in overrides {
            let Some(p) = mesh.patch(name) else {
                return err(format!("unknown boundary patch '{name}'"));
            };
            for f in p.faces() {
                actions[f - nint] = *a;
            }
        }
        Ok(Tracker {
            loc: Locator::new(mesh),
            opts,
            actions,
            injectors: Vec::new(),
            aux: Vec::new(),
            disp: None,
            thermal: None,
            coupling: None,
            droplet_exchange: false,
            film: None,
            erosion: None,
            rough: None,
            reaction: None,
            evaporation: None,
            breakup: None,
            particles: Vec::new(),
            time: 0.0,
        })
    }

    /// Switch on turbulent dispersion by the discrete random walk with per-cell `k` and `eps` (ADR 0057 D2).
    pub fn set_dispersion(&mut self, model: Dispersion, k: Vec<f64>, eps: Vec<f64>, seed: u64) -> Result<()> {
        let n = self.loc.mesh().n_cells();
        if k.len() != n || eps.len() != n {
            return err("dispersion: k and epsilon must have one value per cell");
        }
        if !(model.c_l.is_finite() && model.c_l > 0.0) {
            return err("dispersion: C_L must be positive");
        }
        self.disp = Some(DispCtx {
            model,
            k,
            eps,
            rng: SplitMix64::new(seed),
        });
        Ok(())
    }

    /// Replace the per-cell `k` and `eps` of the random walk (ADR 0208 S1), for example with the turbulence model's fields of
    /// the current step. The generator, the model and the eddies the parcels hold are untouched. Refused without
    /// [`Tracker::set_dispersion`], for a length other than the cell count and for a value that is not finite.
    pub fn set_dispersion_fields(&mut self, k: &[f64], eps: &[f64]) -> Result<()> {
        let n = self.loc.mesh().n_cells();
        let Some(dc) = self.disp.as_mut() else {
            return err("dispersion: the fields need the random walk (Tracker::set_dispersion first)");
        };
        if k.len() != n || eps.len() != n {
            return err("dispersion: k and epsilon must have one value per cell");
        }
        if k.iter().chain(eps).any(|v| !v.is_finite()) {
            return err("dispersion: k and epsilon must be finite");
        }
        dc.k.copy_from_slice(k);
        dc.eps.copy_from_slice(eps);
        Ok(())
    }

    /// The per-cell `k` and `eps` of the random walk, if on.
    pub fn dispersion_fields(&self) -> Option<(&[f64], &[f64])> {
        self.disp.as_ref().map(|d| (d.k.as_slice(), d.eps.as_slice()))
    }

    /// Switch on the surface reaction of ADR 0208 D4 with the oxidiser mass fraction `y_o` per cell. Refused when a
    /// reaction is already set, with heat transfer or the exchange record on, and for an invalid spec or field.
    pub fn set_reaction(&mut self, spec: ReactionSpec, y_o: Vec<f64>) -> Result<()> {
        if self.reaction.is_some() {
            return err("reaction: a reaction is already set");
        }
        if self.evaporation.is_some() || self.breakup.is_some() {
            return err("reaction: not available with evaporation or breakup (ADR 0224 E8, B5)");
        }
        if self.thermal.is_some() || self.coupling.is_some() {
            return err("reaction: not available with heat transfer or the exchange record (ADR 0208 S1)");
        }
        self.reaction = Some(SurfaceReaction::new(spec, y_o, self.loc.mesh().n_cells())?);
        Ok(())
    }

    /// Replace the oxidiser mass fraction of the reaction (refused without one).
    pub fn set_reaction_oxidiser(&mut self, y_o: &[f64]) -> Result<()> {
        match self.reaction.as_mut() {
            Some(r) => r.set_oxidiser_fraction(y_o),
            None => err("reaction: no reaction is set (Tracker::set_reaction first)"),
        }
    }

    /// The surface reaction, if set.
    pub fn reaction(&self) -> Option<&SurfaceReaction> {
        self.reaction.as_ref()
    }

    /// Reacting mass per real particle of parcel `i` [kg] (NaN before its first sub-step with the reaction on).
    pub fn reacting_mass(&self, i: usize) -> f64 {
        self.aux.get(i).map_or(f64::NAN, |a| a.m_r)
    }

    /// Switch on droplet evaporation (ADR 0224 E1 to E8) with the seen vapour mass fraction `y_s` per cell. Needs heat
    /// transfer ([`Tracker::set_thermal`] first); refused when evaporation is already set, with the surface reaction, and for an
    /// invalid spec or field. `advance` refuses it with the exchange record.
    pub fn set_evaporation(&mut self, spec: EvaporationSpec, y_s: Vec<f64>) -> Result<()> {
        if self.evaporation.is_some() {
            return err("evaporation: evaporation is already set");
        }
        if self.thermal.is_none() {
            return err("evaporation: needs heat transfer (Tracker::set_thermal first, ADR 0224 E8)");
        }
        if self.reaction.is_some() {
            return err("evaporation: not available with the surface reaction (ADR 0224 E8)");
        }
        self.evaporation = Some(Evaporation::new(spec, y_s, self.loc.mesh().n_cells())?);
        Ok(())
    }

    /// Replace the seen vapour mass fraction of the evaporation (refused without it).
    pub fn set_evaporation_vapour(&mut self, y_s: &[f64]) -> Result<()> {
        match self.evaporation.as_mut() {
            Some(e) => e.set_vapour_fraction(y_s),
            None => err("evaporation: evaporation is not set (Tracker::set_evaporation first)"),
        }
    }

    /// The evaporation and its vapour booking, if set.
    pub fn evaporation(&self) -> Option<&Evaporation> {
        self.evaporation.as_ref()
    }

    /// Switch on TAB breakup (ADR 0224 B1 to B6). Refused when breakup is already set, with the surface reaction and for an
    /// invalid spec. `advance` refuses it with the exchange record.
    pub fn set_breakup(&mut self, spec: BreakupSpec) -> Result<()> {
        if self.breakup.is_some() {
            return err("breakup: breakup is already set");
        }
        if self.reaction.is_some() {
            return err("breakup: not available with the surface reaction (ADR 0224 B5)");
        }
        self.breakup = Some(Breakup::new(spec)?);
        Ok(())
    }

    /// The breakup model, if set.
    pub fn breakup(&self) -> Option<&Breakup> {
        self.breakup.as_ref()
    }

    /// TAB distortion `(y, dy/dt)` of parcel `i` (NaN for an unknown parcel).
    pub fn distortion(&self, i: usize) -> (f64, f64) {
        self.aux.get(i).map_or((f64::NAN, f64::NAN), |a| (a.y, a.yd))
    }

    /// Breakups of parcel `i` and its age at the last one (NaN when none).
    pub fn breakups(&self, i: usize) -> (u32, f64) {
        self.aux.get(i).map_or((0, f64::NAN), |a| (a.n_bu, a.t_bu))
    }

    /// Switch on convective heat transfer with the given carrier data (the temperature field may be refreshed with
    /// [`Tracker::thermal_mut`] before each step).
    pub fn set_thermal(&mut self, th: Thermal) -> Result<()> {
        if self.reaction.is_some() {
            return err("thermal: not available with the surface reaction (ADR 0208 S1)");
        }
        if th.t_fluid.len() != self.loc.mesh().n_cells() {
            return err("thermal: the carrier temperature needs one value per cell");
        }
        if !(th.cp_p > 0.0 && th.k_f > 0.0 && th.cp_f > 0.0) {
            return err("thermal: specific heats and conductivity must be positive");
        }
        self.thermal = Some(th);
        Ok(())
    }

    /// The heat-transfer data, if on.
    pub fn thermal_mut(&mut self) -> Option<&mut Thermal> {
        self.thermal.as_mut()
    }

    /// Start recording the exchange with the carrier (ADR 0057 D1). Observation only: the particles do not change.
    /// `advance` refuses evaporation and breakup with this record (use [`Tracker::enable_droplet_exchange`]).
    pub fn enable_coupling(&mut self) {
        let n = self.loc.mesh().n_cells();
        self.coupling = Some(Coupling::new(n));
        self.droplet_exchange = false;
    }

    /// Start recording the exchange with the carrier with the droplet terms (ADR 0230 D1): the record of
    /// [`Tracker::enable_coupling`], and evaporation and breakup are accepted with it; the caller must then also give the
    /// carrier the vapour fields of the record ([`Coupling::droplet_sources`]). Observation only.
    pub fn enable_droplet_exchange(&mut self) {
        let n = self.loc.mesh().n_cells();
        self.coupling = Some(Coupling::new(n));
        self.droplet_exchange = true;
    }

    /// The recorded exchange.
    pub fn coupling(&self) -> Option<&Coupling> {
        self.coupling.as_ref()
    }

    /// The recorded exchange, mutable (for `reset`).
    pub fn coupling_mut(&mut self) -> Option<&mut Coupling> {
        self.coupling.as_mut()
    }

    /// Switch on the wall film on the patches of `spec` (ADR 0192): parcels that reach one of their faces are absorbed into
    /// it, and the film is advanced at the end of every [`Tracker::advance`]. Refused when a film is already set.
    pub fn set_film(&mut self, spec: FilmSpec) -> Result<()> {
        if self.film.is_some() {
            return err("wall film: a film is already set");
        }
        let film = Film::new(self.loc.mesh(), &spec)?;
        let nint = self.loc.mesh().n_internal_faces();
        for (k, &f) in film.faces().iter().enumerate() {
            if self.erosion.as_ref().is_some_and(|e| e.local(f).is_some()) {
                return err(format!(
                    "wall film: patch '{}' also books erosion (ADR 0194)",
                    film.patch_name(k)
                ));
            }
            if matches!(self.actions[f - nint], WallAction::Rough(_)) {
                return err(format!(
                    "wall film: patch '{}' is a rough wall (ADR 0194)",
                    film.patch_name(k)
                ));
            }
        }
        for &f in film.faces() {
            self.actions[f - nint] = WallAction::Film;
        }
        self.film = Some(film);
        Ok(())
    }

    /// Switch the gas drives of the wall film (ADR 0206). With a drive on, every [`Tracker::advance`] first gives the film
    /// the gas density `flow.rho` and, with the shear on, the carrier's wall shear from `flow.u` and `flow.mu`
    /// ([`Film::carrier_shear`]). Refused when no film is set.
    pub fn set_film_gas(&mut self, gas: FilmGas) -> Result<()> {
        match self.film.as_mut() {
            Some(f) => {
                f.set_gas(gas);
                Ok(())
            }
            None => err("wall film: the gas drives need a film (Tracker::set_film first)"),
        }
    }

    /// The wall film, if set.
    pub fn film(&self) -> Option<&Film> {
        self.film.as_ref()
    }

    /// The wall film, mutable.
    pub fn film_mut(&mut self) -> Option<&mut Film> {
        self.film.as_mut()
    }

    /// Book the erosion of every parcel hit on the patches of `spec` (ADR 0194 E1 to E4). An observation: the parcels do not
    /// change. Refused when erosion is already set and on a face of the wall film.
    pub fn set_erosion(&mut self, spec: ErosionSpec) -> Result<()> {
        if self.erosion.is_some() {
            return err("erosion: erosion is already set");
        }
        let er = Erosion::new(self.loc.mesh(), &spec)?;
        let nint = self.loc.mesh().n_internal_faces();
        for (k, &f) in er.faces().iter().enumerate() {
            if self.actions[f - nint] == WallAction::Film {
                return err(format!("erosion: patch '{}' carries the wall film", er.patch_name(k)));
            }
        }
        self.erosion = Some(er);
        Ok(())
    }

    /// The erosion booking, if set.
    pub fn erosion(&self) -> Option<&Erosion> {
        self.erosion.as_ref()
    }

    /// Make the reflecting faces of the patches of `spec` rough (ADR 0194 W1 to W7): each hit reflects about a virtual wall
    /// tilted by a random roughness angle. Refused when a rough wall is already set and on a face that does not reflect
    /// (stick, escape, film) or reflects with restitution 0.
    pub fn set_rough_wall(&mut self, spec: RoughSpec) -> Result<()> {
        if self.rough.is_some() {
            return err("rough wall: a rough wall is already set");
        }
        let rw = RoughWall::new(self.loc.mesh(), &spec)?;
        let nint = self.loc.mesh().n_internal_faces();
        for (k, &f) in rw.faces().iter().enumerate() {
            match self.actions[f - nint] {
                WallAction::Reflect(e) if e > 0.0 && e <= 1.0 => {}
                WallAction::Reflect(e) => {
                    return err(format!(
                        "rough wall: patch '{}' reflects with restitution {e}; a rough wall needs one in (0, 1]",
                        rw.patch_name(k)
                    ))
                }
                _ => {
                    return err(format!(
                        "rough wall: patch '{}' does not reflect (stick, escape or film)",
                        rw.patch_name(k)
                    ))
                }
            }
        }
        for &f in rw.faces() {
            if let WallAction::Reflect(e) = self.actions[f - nint] {
                self.actions[f - nint] = WallAction::Rough(e);
            }
        }
        self.rough = Some(rw);
        Ok(())
    }

    /// The rough wall, if set.
    pub fn rough_wall(&self) -> Option<&RoughWall> {
        self.rough.as_ref()
    }

    /// Temperature of parcel `i` [K] (NaN when heat transfer is off).
    pub fn temperature(&self, i: usize) -> f64 {
        self.aux.get(i).map_or(f64::NAN, |a| a.tp)
    }

    /// Attach extensions (cone, mass flow, temperature) to the injector `name`.
    pub fn set_injector_ext(&mut self, name: &str, ext: InjectorExt) -> Result<()> {
        if let Err((field, msg)) = ext.validate() {
            return err(format!("injector '{name}': {field} {msg}"));
        }
        match self.injectors.iter_mut().find(|i| i.spec.name == name) {
            Some(i) => {
                i.ext = ext;
                Ok(())
            }
            None => err(format!("unknown injector '{name}'")),
        }
    }

    /// The locator.
    pub fn locator(&self) -> &Locator {
        &self.loc
    }

    /// Add an injector (validates the location).
    pub fn add_injector(&mut self, spec: InjectorSpec) -> Result<()> {
        if !spec.rate.is_finite() || spec.rate <= 0.0 {
            return err(format!("injector '{}': rate must be positive", spec.name));
        }
        let mut faces = Vec::new();
        let mut cum = Vec::new();
        match &spec.kind {
            InjKind::Point(p) => {
                if self.loc.locate(*p, None).is_none() {
                    return err(format!(
                        "injector '{}': the point {:?} is outside the mesh",
                        spec.name, p
                    ));
                }
            }
            InjKind::Surface(name) => {
                let mesh = self.loc.mesh();
                let Some(patch) = mesh.patch(name) else {
                    return err(format!("injector '{}': unknown patch '{name}'", spec.name));
                };
                let mut total = 0.0;
                for f in patch.faces() {
                    total += mesh.face_area(f);
                    faces.push(f);
                    cum.push(total);
                }
                if faces.is_empty() {
                    return err(format!("injector '{}': patch '{name}' has no faces", spec.name));
                }
            }
        }
        let rng = SplitMix64::new(spec.seed);
        self.injectors.push(Injector {
            ext: InjectorExt::default(),
            spec,
            rng,
            emitted: 0,
            faces,
            cum_area: cum,
        });
        Ok(())
    }

    /// Number of parcels injected so far.
    pub fn injected(&self) -> usize {
        self.particles.len()
    }

    /// Number of parcels in a state.
    pub fn count(&self, s: State) -> usize {
        self.particles.iter().filter(|p| p.state == s).count()
    }

    fn spawn(&mut self, inj: usize, flow: &Flow) -> Result<usize> {
        let loc = &self.loc;
        let mesh = loc.mesh();
        let id = self.particles.len() as u64;
        let ij = &mut self.injectors[inj];
        let (x, cell) = match &ij.spec.kind {
            InjKind::Point(p) => match loc.locate(*p, None) {
                Some(c) => (*p, c),
                None => return err("injection point is outside the mesh"),
            },
            InjKind::Surface(_) => {
                let total = *ij.cum_area.last().unwrap_or(&0.0);
                let r = ij.rng.uniform() * total;
                let k = ij.cum_area.partition_point(|&a| a <= r).min(ij.faces.len() - 1);
                let f = ij.faces[k];
                let pts = mesh.points();
                let ids = mesh.face_points(f);
                let fa = loc.face_apex(f);
                let n = ids.len();
                // Triangle of the fan, weighted by area.
                let mut tri_cum = Vec::with_capacity(n);
                let mut s = 0.0;
                for i in 0..n {
                    let e1 = sub(pts[ids[i]], fa);
                    let e2 = sub(pts[ids[(i + 1) % n]], fa);
                    s += 0.5 * norm(cross(e1, e2));
                    tri_cum.push(s);
                }
                let rt = ij.rng.uniform() * s;
                let ti = tri_cum.partition_point(|&a| a <= rt).min(n - 1);
                let mut r1 = ij.rng.uniform();
                let mut r2 = ij.rng.uniform();
                if r1 + r2 > 1.0 {
                    r1 = 1.0 - r1;
                    r2 = 1.0 - r2;
                }
                let e1 = sub(pts[ids[ti]], fa);
                let e2 = sub(pts[ids[(ti + 1) % n]], fa);
                let q = add(fa, add(scale(e1, r1), scale(e2, r2)));
                let c = mesh.owner()[f];
                let nrm = loc.face_normal(f);
                (sub(q, scale(nrm, 1e-9 * loc.length_scale(c))), c)
            }
        };
        let v = match ij.spec.velocity {
            VelMode::Given(v) => v,
            VelMode::Fluid => fluid_at(mesh, flow, self.opts.interp, cell, x),
        };
        let d = ij.spec.dist.draw(&mut ij.rng);
        let mut v = v;
        if let Some(cone) = &ij.ext.cone {
            let (u1, u2) = (ij.rng.uniform(), ij.rng.uniform());
            v = scale(cone.direction(u1, u2), cone.speed);
        }
        let parcel = match ij.ext.mass_flow {
            Some(mdot) => (mdot / ij.spec.rate) / (ij.spec.density * std::f64::consts::PI * d * d * d / 6.0),
            None => ij.spec.parcel_count,
        };
        let tinj = ij.ext.temperature;
        self.aux.resize(self.particles.len(), Aux::default());
        self.aux.push(Aux {
            tp: tinj.unwrap_or(f64::NAN),
            ..Aux::default()
        });
        self.particles.push(Particle {
            id,
            x,
            v,
            d,
            rho: ij.spec.density,
            parcel,
            cell,
            age: 0.0,
            state: State::Active,
        });
        Ok(self.particles.len() - 1)
    }

    /// Advance all particles by `dt` on the (frozen) flow, injecting the parcels that fall due in the step.
    pub fn advance(&mut self, flow: &Flow, dt: f64) -> Result<()> {
        if flow.u.len() != self.loc.mesh().n_cells() {
            return err("velocity field size does not match the mesh");
        }
        if self.opts.interp == Interp::Gradient && flow.grad.is_none() {
            return err("gradient interpolation needs the velocity gradient");
        }
        if self.reaction.is_some() && self.coupling.is_some() {
            return err("reaction: not available with the exchange record (ADR 0208 S1)");
        }
        if (self.evaporation.is_some() || self.breakup.is_some()) && self.coupling.is_some() && !self.droplet_exchange {
            return err("evaporation and breakup: not available with the exchange record (ADR 0224 E8, B5)");
        }
        if let Some(film) = self.film.as_mut() {
            let gas = film.gas();
            if gas != FilmGas::default() {
                // ADR 0206 S1: the gas state of the step, before any parcel moves
                let tau = if gas.shear {
                    film.carrier_shear(flow.u, flow.mu)
                } else {
                    Vec::new()
                };
                film.set_gas_state(flow.rho, &tau)?;
            }
        }
        let latent = self.evaporation.as_ref().map_or(0.0, |e| e.spec().latent_heat);
        let t0 = self.time;
        let t1 = t0 + dt;
        let n_old = self.particles.len();
        self.aux.resize(n_old, Aux::default());
        {
            let opts = self.opts.clone();
            let loc = &self.loc;
            let actions = &self.actions;
            let mut ctx = Ctx {
                disp: self.disp.as_mut(),
                thermal: self.thermal.as_ref(),
                cpl: self.coupling.as_mut(),
                latent,
                film: self.film.as_mut(),
                erosion: self.erosion.as_mut(),
                rough: self.rough.as_mut(),
                reaction: self.reaction.as_mut(),
                evap: self.evaporation.as_mut(),
                breakup: self.breakup.as_mut(),
            };
            for (p, a) in self.particles[..n_old].iter_mut().zip(self.aux.iter_mut()) {
                if p.state == State::Active {
                    advance_particle(loc, &opts, actions, flow, p, a, &mut ctx, dt)?;
                }
            }
        }
        for k in 0..self.injectors.len() {
            let before = self.injectors[k].emitted;
            let after = self.injectors[k].spec.emitted_by(t1);
            for m in (before + 1)..=after {
                let tm = self.injectors[k].spec.emission_time(m);
                let idx = self.spawn(k, flow)?;
                self.injectors[k].emitted = m;
                let left = (t1 - tm).max(0.0);
                let opts = self.opts.clone();
                let p = &mut self.particles[idx];
                let a = &mut self.aux[idx];
                let mut ctx = Ctx {
                    disp: self.disp.as_mut(),
                    thermal: self.thermal.as_ref(),
                    cpl: self.coupling.as_mut(),
                    latent,
                    film: self.film.as_mut(),
                    erosion: self.erosion.as_mut(),
                    rough: self.rough.as_mut(),
                    reaction: self.reaction.as_mut(),
                    evap: self.evaporation.as_mut(),
                    breakup: self.breakup.as_mut(),
                };
                advance_particle(&self.loc, &opts, &self.actions, flow, p, a, &mut ctx, left)?;
            }
        }
        if let Some(film) = self.film.as_mut() {
            film.advance(dt);
        }
        self.time = t1;
        Ok(())
    }

    /// All state of the tracker as bits, for bit-for-bit comparisons.
    pub fn fingerprint(&self) -> Vec<u64> {
        let mut out = vec![self.time.to_bits(), self.particles.len() as u64];
        for p in &self.particles {
            out.push(p.id);
            for k in 0..3 {
                out.push(p.x[k].to_bits());
            }
            for k in 0..3 {
                out.push(p.v[k].to_bits());
            }
            out.push(p.d.to_bits());
            out.push(p.rho.to_bits());
            out.push(p.parcel.to_bits());
            out.push(p.cell as u64);
            out.push(p.age.to_bits());
            out.push(p.state as u64);
        }
        if let Some(film) = &self.film {
            out.extend(film.masses().iter().map(|m| m.to_bits()));
            out.push(film.drained().to_bits());
        }
        if let Some(er) = &self.erosion {
            for k in 0..er.len() {
                out.push(er.eroded()[k].to_bits());
                out.push(er.impact_mass()[k].to_bits());
                out.push(er.hits()[k]);
            }
        }
        if let Some(rw) = &self.rough {
            let c = rw.counts();
            out.extend([c.collisions, c.redraws, c.fallbacks, c.normal_incidence, c.grazing]);
        }
        if let Some(rx) = &self.reaction {
            out.extend(rx.bits());
            out.extend((0..self.particles.len()).map(|i| self.reacting_mass(i).to_bits()));
        }
        if let Some(ev) = &self.evaporation {
            out.extend(ev.bits());
            out.extend((0..self.particles.len()).map(|i| self.temperature(i).to_bits()));
        }
        if let Some(bu) = &self.breakup {
            out.push(bu.events());
            for i in 0..self.particles.len() {
                let (y, yd) = self.distortion(i);
                let (n, t) = self.breakups(i);
                out.extend([y.to_bits(), yd.to_bits(), n as u64, t.to_bits()]);
            }
        }
        out
    }
}

/// Lumped-sphere heat exchange over a sub-step: new particle temperature and the heat taken from the carrier [J].
fn heat_step(th: &Thermal, fl: Fluid, p: &Particle, t0: f64, cell: usize, rel_speed: f64, h: f64) -> (f64, f64) {
    let pr = fl.mu * th.cp_f / th.k_f;
    let re = fl.rho * p.d * rel_speed / fl.mu;
    let tau_t = tau_thermal(p.rho, th.cp_p, p.d, th.k_f, nusselt(re, pr));
    let tf = th.t_fluid[cell];
    // expm1 keeps the update accurate for h << tau_t.
    let t1 = t0 + (tf - t0) * -(-h / tau_t).exp_m1();
    let m = p.parcel * p.rho * std::f64::consts::PI * p.d * p.d * p.d / 6.0;
    (t1, m * th.cp_p * (t1 - t0))
}

/// The surface reaction over a sub-step of length `h` that started in `cell` (ADR 0208 R2 to R5): sets the composition at
/// the parcel's first reacting sub-step, shrinks the parcel and updates `mass` (the parcel mass) when mass reacted.
fn react(rx: &mut SurfaceReaction, p: &mut Particle, aux: &mut Aux, cell: usize, rho_g: f64, h: f64, mass: &mut f64) {
    if aux.m_r.is_nan() {
        let m0 = p.rho * std::f64::consts::PI * p.d * p.d * p.d / 6.0;
        let f = rx.spec().reacting_fraction;
        aux.m_r = f * m0;
        aux.m_i = (1.0 - f) * m0;
    }
    if let Some(d) = rx.react(cell, rho_g, p.rho, p.d, p.parcel, &mut aux.m_r, aux.m_i, h) {
        p.d = d;
        *mass = p.parcel * p.rho * std::f64::consts::PI * p.d * p.d * p.d / 6.0;
    }
}

/// The evaporation over a sub-step of length `h` that started in `cell`, with the rates of its start (ADR 0224 E6): the
/// temperature by one exponential Euler step, `d^2` linearly, the vapour booked in `cell`; with `complete` the parcel's whole
/// liquid mass is booked and it becomes `Evaporated`. Updates `mass` (the parcel mass).
#[allow(clippy::too_many_arguments)]
fn evaporate(
    ev: &mut Evaporation,
    p: &mut Particle,
    aux: &mut Aux,
    cell: usize,
    r: &EvapRates,
    h: f64,
    complete: bool,
    mass: &mut f64,
) -> f64 {
    aux.tp += h * phi1(r.jac * h) * r.f;
    let c = std::f64::consts::PI * p.rho / 6.0;
    let d0 = p.d;
    let dm;
    if complete {
        dm = p.parcel * c * d0 * d0 * d0;
        ev.book(cell, dm);
        ev.complete();
        p.d = 0.0;
        p.state = State::Evaporated;
    } else {
        let d1 = (d0 * d0 - r.k * h).sqrt();
        dm = p.parcel * c * (d0 * d0 * d0 - d1 * d1 * d1);
        ev.book(cell, dm);
        p.d = d1;
    }
    *mass = p.parcel * p.rho * std::f64::consts::PI * p.d * p.d * p.d / 6.0;
    dm
}

/// Book the vapour `dm` [kg] of a sub-step in `cell`: it leaves with the velocity `v1` and the temperature `t1` of the
/// sub-step's end, the liquid enthalpy being `c_l T` (ADR 0230 D3, D4).
fn book_vapour(cpl: &mut Coupling, cell: usize, dm: f64, v1: Vec3, t1: f64, c_l: f64, latent: f64) {
    cpl.vapour_mass[cell] += dm;
    for k in 0..3 {
        cpl.vapour_impulse[cell][k] += dm * v1[k];
    }
    cpl.vapour_kinetic[cell] += 0.5 * dm * dot(v1, v1);
    cpl.vapour_enthalpy[cell] += dm * (c_l * t1 + latent);
}

/// Book a wall reflection of a parcel of mass `m` from `v_hit` to `v_new` (ADR 0057 D1, ADR 0230 D6).
fn book_wall(cpl: &mut Coupling, m: f64, v_hit: Vec3, v_new: Vec3) {
    let dj = scale(sub(v_new, v_hit), m);
    for k in 0..3 {
        cpl.wall_impulse[k] += dj[k];
    }
    cpl.wall_energy += 0.5 * m * (dot(v_new, v_new) - dot(v_hit, v_hit));
}

/// The TAB breakup at the end of a sub-step of length `s` (the breakup time) with the oscillator of its start and its
/// relative velocity `w_rel` (ADR 0224 B4). Updates `mass`.
fn break_up(bu: &mut Breakup, p: &mut Particle, aux: &mut Aux, o: &Oscillator, s: f64, w_rel: Vec3, mass: &mut f64) {
    let (_, yd_b) = o.advance(aux.y, aux.yd, s);
    let r = 0.5 * p.d;
    let r32 = sauter_radius(bu.spec(), p.rho, r, yd_b);
    let q = r / r32;
    p.parcel *= q * q * q;
    p.d = 2.0 * r32;
    let vn = normal_speed(bu.spec(), r, yd_b);
    let psi = 2.0 * std::f64::consts::PI * bu.rng.uniform();
    let wn = norm(w_rel);
    if vn != 0.0 && wn > 0.0 {
        let (e1, e2) = tangent_pair(scale(w_rel, 1.0 / wn));
        p.v = add(p.v, scale(add(scale(e1, psi.cos()), scale(e2, psi.sin())), vn));
    }
    aux.y = 0.0;
    aux.yd = 0.0;
    aux.n_bu += 1;
    aux.t_bu = p.age;
    bu.count();
    *mass = p.parcel * p.rho * std::f64::consts::PI * p.d * p.d * p.d / 6.0;
}

/// Book the exchange of one sub-step (before any wall reflection) in the cell `cell` where it started.
#[allow(clippy::too_many_arguments)]
fn book(cpl: &mut Coupling, cell: usize, m: f64, geff: Vec3, x0: Vec3, v0: Vec3, x1: Vec3, v1: Vec3, h: f64, dq: f64) {
    let j = scale(sub(sub(v1, v0), scale(geff, h)), m);
    for k in 0..3 {
        cpl.impulse[cell][k] -= j[k];
    }
    let dke = 0.5 * m * (dot(v1, v1) - dot(v0, v0));
    let wg = m * dot(geff, sub(x1, x0));
    cpl.energy_mech[cell] -= dke - wg;
    cpl.energy_heat[cell] -= dq;
    // ADR 0230 D6
    for k in 0..3 {
        cpl.gravity_impulse[k] += m * geff[k] * h;
    }
    cpl.gravity_work += wg;
}

#[allow(clippy::too_many_arguments)]
fn advance_particle(
    loc: &Locator,
    opts: &Options,
    actions: &[WallAction],
    flow: &Flow,
    p: &mut Particle,
    aux: &mut Aux,
    ctx: &mut Ctx,
    dt: f64,
) -> Result<()> {
    let mesh = loc.mesh();
    let nint = mesh.n_internal_faces();
    let fl = Fluid {
        rho: flow.rho,
        mu: flow.mu,
    };
    let geff = scale(opts.gravity, 1.0 - fl.rho / p.rho);
    let mut mass = p.parcel * p.rho * std::f64::consts::PI * p.d * p.d * p.d / 6.0;
    if let Some(th) = ctx.thermal {
        if aux.tp.is_nan() {
            aux.tp = th.t_fluid[p.cell];
        }
    }
    let mut rem = dt;
    let mut bounces = 0usize;
    let mut guard = 0usize;
    while rem > 0.0 && p.state == State::Active {
        guard += 1;
        if guard > 5_000_000 {
            return err(format!("particle {}: too many sub-steps in one flow step", p.id));
        }
        let cell0 = p.cell;
        let x0 = p.x;
        let v0 = p.v;
        let u_mean = fluid_at(mesh, flow, opts.interp, cell0, x0);
        let mut u_start = u_mean;
        if let Some(dc) = ctx.disp.as_deref_mut() {
            if aux.eddy.t_left.is_nan() || aux.eddy.t_left <= 0.0 {
                let re = fl.rho * p.d * norm(sub(u_mean, v0)) / fl.mu;
                let f = drag_factor(opts.drag, re);
                let tau_eff = if f > 0.0 {
                    tau_p(p.d, p.rho, fl.mu) / f
                } else {
                    f64::INFINITY
                };
                aux.eddy
                    .draw(&mut dc.rng, &dc.model, dc.k[cell0], dc.eps[cell0], tau_eff, u_mean, v0);
            }
            u_start = add(u_mean, aux.eddy.uprime);
        }
        let upr = if ctx.disp.is_some() {
            Some(aux.eddy.uprime)
        } else {
            None
        };
        let mut h = rem;
        let sp = norm(v0);
        if sp > 0.0 {
            h = h.min(opts.courant * loc.length_scale(cell0) / sp);
        }
        if opts.scheme != Scheme::Analytic && opts.drag != DragLaw::None {
            let re = fl.rho * p.d * norm(sub(u_start, v0)) / fl.mu;
            let f = drag_factor(opts.drag, re).max(1e-300);
            h = h.min(opts.relax_fraction * tau_p(p.d, p.rho, fl.mu) / f);
        }
        if rem - h <= 1e-12 * rem {
            h = rem;
        }
        if upr.is_some() && aux.eddy.t_left < h {
            h = aux.eddy.t_left;
        }
        // ADR 0224 E6: the evaporation rates of the sub-step's start, the thermal limit and the completion
        let mut evap_rates = None;
        let mut completes = false;
        if let (Some(ev), Some(th)) = (ctx.evap.as_deref(), ctx.thermal) {
            let r = ev.rates(th, fl.rho, fl.mu, cell0, p.d, p.rho, aux.tp, norm(sub(u_start, v0)))?;
            h = h.min(ev.spec().max_step_fraction * r.tau_t);
            if r.k > 0.0 {
                let dmin = ev.spec().min_diameter;
                let hc = ((p.d * p.d - dmin * dmin) / r.k).max(0.0);
                if hc <= h {
                    h = hc;
                    completes = true;
                }
            }
            evap_rates = Some(r);
        }
        // ADR 0224 B3: the oscillator of the sub-step's start and the time to breakup
        let mut osc = None;
        let mut bu_due = false;
        if let Some(bu) = ctx.breakup.as_deref() {
            let w_rel = sub(u_start, v0);
            let o = oscillator(bu.spec(), fl.rho, p.rho, 0.5 * p.d, norm(w_rel));
            let s = o.time_to_breakup(aux.y, aux.yd);
            if s < h {
                h = s;
                bu_due = true;
                completes = false;
            } else if s == h {
                bu_due = true;
            }
            osc = Some((o, w_rel));
        }
        let mut u_at = |x: Vec3| -> Vec3 {
            if x == x0 {
                return u_start;
            }
            let u = match loc.locate(x, Some(cell0)) {
                Some(c) => fluid_at(mesh, flow, opts.interp, c, x),
                None => fluid_at(mesh, flow, opts.interp, cell0, x),
            };
            match upr {
                Some(up) => add(u, up),
                None => u,
            }
        };
        let (x1, v1) = step_particle(
            opts.scheme,
            opts.drag,
            fl,
            p.d,
            p.rho,
            opts.gravity,
            x0,
            v0,
            h,
            &mut u_at,
        );
        match loc.walk(cell0, x0, x1) {
            Walk::Inside(c) => {
                let mut dq = 0.0;
                // ADR 0230 D2: the motion and the temperature step act on the mass of the sub-step's start
                let m0 = mass;
                let mut vapour = None;
                if let (Some(ev), Some(r)) = (ctx.evap.as_deref_mut(), evap_rates.as_ref()) {
                    let t0 = aux.tp;
                    let dm = evaporate(ev, p, aux, cell0, r, h, completes, &mut mass);
                    if let Some(th) = ctx.thermal {
                        dq = m0 * th.cp_p * (aux.tp - t0) + dm * ctx.latent;
                        vapour = Some((dm, th.cp_p));
                    }
                } else if let Some(th) = ctx.thermal {
                    let (t1, q) = heat_step(th, fl, p, aux.tp, cell0, norm(sub(u_start, v0)), h);
                    aux.tp = t1;
                    dq = q;
                }
                if let Some(cpl) = ctx.cpl.as_deref_mut() {
                    book(cpl, cell0, m0, geff, x0, v0, x1, v1, h, dq);
                    if let Some((dm, c_l)) = vapour {
                        book_vapour(cpl, cell0, dm, v1, aux.tp, c_l, ctx.latent);
                    }
                }
                p.x = x1;
                p.v = v1;
                p.cell = c;
                p.age += h;
                rem -= h;
                if upr.is_some() {
                    aux.eddy.t_left -= h;
                }
                if let Some(rx) = ctx.reaction.as_deref_mut() {
                    react(rx, p, aux, cell0, fl.rho, h, &mut mass);
                }
                if let (Some(bu), Some((o, w_rel))) = (ctx.breakup.as_deref_mut(), osc) {
                    if bu_due && p.state == State::Active {
                        let v_b = p.v;
                        break_up(bu, p, aux, &o, h, w_rel, &mut mass);
                        if let Some(cpl) = ctx.cpl.as_deref_mut() {
                            // ADR 0230 D5: no carrier counterpart
                            let dj = scale(sub(p.v, v_b), mass);
                            for k in 0..3 {
                                cpl.breakup_impulse[k] += dj[k];
                            }
                            cpl.breakup_energy += 0.5 * mass * (dot(p.v, p.v) - dot(v_b, v_b));
                        }
                    } else {
                        (aux.y, aux.yd) = o.advance(aux.y, aux.yd, h);
                    }
                }
            }
            Walk::Lost => return err(format!("particle {}: the cell walk did not terminate", p.id)),
            Walk::Boundary { cell, face, point } => {
                // Refine the hit time on the integrated path. The predicate is the one the tracker itself uses: the chord from the
                // start of the sub-step to the end of the shortened step leaves the domain. (A bisection on the distance to the
                // plane of one face is not enough: near an edge or corner the curved path can leave through another face first.)
                let (mut lo, mut hi) = (0.0, h);
                let (mut hit_face, mut hit_pt, mut hit_cell, mut v_hit) = (face, point, cell, v1);
                for _ in 0..60 {
                    let mid = 0.5 * (lo + hi);
                    let (xm, vm) = step_particle(
                        opts.scheme,
                        opts.drag,
                        fl,
                        p.d,
                        p.rho,
                        opts.gravity,
                        x0,
                        v0,
                        mid,
                        &mut u_at,
                    );
                    match loc.walk(cell0, x0, xm) {
                        Walk::Boundary { cell, face, point } => {
                            hi = mid;
                            hit_face = face;
                            hit_pt = point;
                            hit_cell = cell;
                            v_hit = vm;
                        }
                        _ => lo = mid,
                    }
                }
                let (face, cell) = (hit_face, hit_cell);
                let n = loc.face_normal(face);
                let (h_hit, x_hit) = (hi, hit_pt);
                let mut dq = 0.0;
                let m0 = mass;
                let mut vapour = None;
                if let (Some(ev), Some(r)) = (ctx.evap.as_deref_mut(), evap_rates.as_ref()) {
                    let t0 = aux.tp;
                    let dm = evaporate(ev, p, aux, cell0, r, h_hit, false, &mut mass);
                    if let Some(th) = ctx.thermal {
                        dq = m0 * th.cp_p * (aux.tp - t0) + dm * ctx.latent;
                        vapour = Some((dm, th.cp_p));
                    }
                } else if let Some(th) = ctx.thermal {
                    let (t1, q) = heat_step(th, fl, p, aux.tp, cell0, norm(sub(u_start, v0)), h_hit);
                    aux.tp = t1;
                    dq = q;
                }
                if let Some(cpl) = ctx.cpl.as_deref_mut() {
                    book(cpl, cell0, m0, geff, x0, v0, x_hit, v_hit, h_hit, dq);
                    if let Some((dm, c_l)) = vapour {
                        book_vapour(cpl, cell0, dm, v_hit, aux.tp, c_l, ctx.latent);
                    }
                }
                p.x = x_hit;
                p.v = v_hit;
                p.cell = cell;
                p.age += h_hit;
                rem -= h_hit;
                if upr.is_some() {
                    aux.eddy.t_left -= h_hit;
                }
                if let Some(rx) = ctx.reaction.as_deref_mut() {
                    react(rx, p, aux, cell0, fl.rho, h_hit, &mut mass);
                }
                if let Some((o, _)) = osc {
                    (aux.y, aux.yd) = o.advance(aux.y, aux.yd, h_hit);
                }
                if let Some(er) = ctx.erosion.as_deref_mut() {
                    er.book(face, mass, v_hit, n);
                }
                match actions[face - nint] {
                    WallAction::Reflect(e) => {
                        p.v = reflect(v_hit, n, e);
                        if let Some(cpl) = ctx.cpl.as_deref_mut() {
                            book_wall(cpl, mass, v_hit, p.v);
                        }
                        bounces += 1;
                        if bounces > opts.max_bounces {
                            p.state = State::Stuck;
                        }
                    }
                    WallAction::Rough(e) => {
                        p.v = match ctx.rough.as_deref_mut() {
                            Some(rw) => rw.rebound(v_hit, n, e),
                            None => reflect(v_hit, n, e),
                        };
                        if let Some(cpl) = ctx.cpl.as_deref_mut() {
                            book_wall(cpl, mass, v_hit, p.v);
                        }
                        bounces += 1;
                        if bounces > opts.max_bounces {
                            p.state = State::Stuck;
                        }
                    }
                    WallAction::Stick => p.state = State::Deposited,
                    WallAction::Escape => p.state = State::Escaped,
                    WallAction::Film => {
                        if let Some(film) = ctx.film.as_deref_mut() {
                            film.absorb(face, mass);
                        }
                        p.state = State::Film;
                    }
                }
            }
        }
    }
    Ok(())
}
