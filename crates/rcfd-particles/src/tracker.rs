//! Particle state, the tracker and wall interaction (ADR 0028 D3, D5, D6).

use crate::coupling::{nusselt, tau_thermal, Coupling, Thermal};
use crate::dispersion::{Dispersion, Eddy};
use crate::drag::{drag_factor, reflect, step_particle, tau_p, DragLaw, Fluid, Scheme};
use crate::inject::{InjKind, InjectorExt, InjectorSpec, VelMode};
use crate::locate::{Locator, Walk};
use crate::rng::SplitMix64;
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
}

impl Default for Aux {
    fn default() -> Self {
        Aux {
            eddy: Eddy::default(),
            tp: f64::NAN,
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

    /// Switch on convective heat transfer with the given carrier data (the temperature field may be refreshed with
    /// [`Tracker::thermal_mut`] before each step).
    pub fn set_thermal(&mut self, th: Thermal) -> Result<()> {
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
    pub fn enable_coupling(&mut self) {
        let n = self.loc.mesh().n_cells();
        self.coupling = Some(Coupling::new(n));
    }

    /// The recorded exchange.
    pub fn coupling(&self) -> Option<&Coupling> {
        self.coupling.as_ref()
    }

    /// The recorded exchange, mutable (for `reset`).
    pub fn coupling_mut(&mut self) -> Option<&mut Coupling> {
        self.coupling.as_mut()
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
            eddy: Eddy::default(),
            tp: tinj.unwrap_or(f64::NAN),
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
                };
                advance_particle(&self.loc, &opts, &self.actions, flow, p, a, &mut ctx, left)?;
            }
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
    let mass = p.parcel * p.rho * std::f64::consts::PI * p.d * p.d * p.d / 6.0;
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
            if !(aux.eddy.t_left > 0.0) {
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
                if let Some(th) = ctx.thermal {
                    let (t1, q) = heat_step(th, fl, p, aux.tp, cell0, norm(sub(u_start, v0)), h);
                    aux.tp = t1;
                    dq = q;
                }
                if let Some(cpl) = ctx.cpl.as_deref_mut() {
                    book(cpl, cell0, mass, geff, x0, v0, x1, v1, h, dq);
                }
                p.x = x1;
                p.v = v1;
                p.cell = c;
                p.age += h;
                rem -= h;
                if upr.is_some() {
                    aux.eddy.t_left -= h;
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
                if let Some(th) = ctx.thermal {
                    let (t1, q) = heat_step(th, fl, p, aux.tp, cell0, norm(sub(u_start, v0)), h_hit);
                    aux.tp = t1;
                    dq = q;
                }
                if let Some(cpl) = ctx.cpl.as_deref_mut() {
                    book(cpl, cell0, mass, geff, x0, v0, x_hit, v_hit, h_hit, dq);
                }
                p.x = x_hit;
                p.v = v_hit;
                p.cell = cell;
                p.age += h_hit;
                rem -= h_hit;
                if upr.is_some() {
                    aux.eddy.t_left -= h_hit;
                }
                match actions[face - nint] {
                    WallAction::Reflect(e) => {
                        p.v = reflect(v_hit, n, e);
                        if let Some(cpl) = ctx.cpl.as_deref_mut() {
                            let dj = scale(sub(p.v, v_hit), mass);
                            for k in 0..3 {
                                cpl.wall_impulse[k] += dj[k];
                            }
                        }
                        bounces += 1;
                        if bounces > opts.max_bounces {
                            p.state = State::Stuck;
                        }
                    }
                    WallAction::Stick => p.state = State::Deposited,
                    WallAction::Escape => p.state = State::Escaped,
                }
            }
        }
    }
    Ok(())
}
