//! Differential viscoelastic constitutive models in conformation form with an optional log-conformation evolution
//! (F-135, ADR 0051).
//!
//! Models ([`Model`]): Oldroyd-B, Giesekus, simplified linear PTT and FENE-P. Every model has the form
//! `C^ = dC/dt + u.grad C - L C - C L^T = -P(C)/lambda` with `L_ij = d u_i / d x_j` and `P` a function of `C` alone.
//! The polymer stress is `tau_p = (eta_p/lambda) (g(C))` (see [`Model::stress`]).
//!
//! * [`advance_local`] integrates the local (homogeneous) ODE by RK4 sub-steps, either for `C` itself
//!   ([`Formulation::Conformation`]) or for `Theta = log C` ([`Formulation::LogConformation`], Fattal and Kupferman 2004, in
//!   the regular divided-difference form of the chain rule, so the start from `C = I` is not singular).
//! * [`ViscoelasticSolver`] couples the stress carrier to [`PimpleSolver`](crate::pimple::PimpleSolver) on the cell-centred
//!   mesh with an explicit-implicit split of the polymer viscosity (both-sides diffusion type, ADR 0051 D5).
//!
//! Closed-form references of the homogeneous flows are in [`steady_shear`] and [`oldroyd_b_startup`].

use crate::field::{ScalarField, VectorField};
use crate::geometry::FvGeometry;
use crate::gradient::green_gauss;
use crate::pimple::{PimpleSettings, PimpleSolver};
use crate::simple::BodyForce;
use rcfd_core::vec3::Vec3;
use std::sync::Arc;

/// Symmetric 3x3 tensor stored as a full matrix.
pub type M3 = [[f64; 3]; 3];

/// Identity.
pub const EYE: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn mm(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0; 3]; 3];
    for (i, ri) in r.iter_mut().enumerate() {
        for (j, rij) in ri.iter_mut().enumerate() {
            *rij = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}
fn tr_(a: &M3) -> M3 {
    let mut r = [[0.0; 3]; 3];
    for (i, ri) in r.iter_mut().enumerate() {
        for (j, rij) in ri.iter_mut().enumerate() {
            *rij = a[j][i];
        }
    }
    r
}
fn add(a: &M3, b: &M3) -> M3 {
    let mut r = *a;
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] += b[i][j];
        }
    }
    r
}
fn scale(a: &M3, s: f64) -> M3 {
    let mut r = *a;
    for row in r.iter_mut() {
        for v in row.iter_mut() {
            *v *= s;
        }
    }
    r
}
fn trace(a: &M3) -> f64 {
    a[0][0] + a[1][1] + a[2][2]
}
fn fro(a: &M3) -> f64 {
    a.iter().flatten().map(|v| v * v).sum::<f64>().sqrt()
}
fn sym(a: &M3) -> M3 {
    let mut r = *a;
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = 0.5 * (a[i][j] + a[j][i]);
        }
    }
    r
}

/// Cyclic Jacobi eigen-decomposition of a symmetric 3x3 matrix: eigenvalues `w` and the matrix `r` whose COLUMNS are the
/// orthonormal eigenvectors (`A = r diag(w) r^T`).
pub fn eig_sym(a: &M3) -> ([f64; 3], M3) {
    let mut a = sym(a);
    let mut v = EYE;
    for _ in 0..60 {
        let off = (a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2]).sqrt();
        let diag = fro(&a).max(1e-300);
        if off <= 1e-16 * diag {
            break;
        }
        for (p, q) in [(0usize, 1usize), (0, 2), (1, 2)] {
            if a[p][q].abs() < 1e-300 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let t = if theta == 0.0 { 1.0 } else { t };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            let mut g = EYE;
            g[p][p] = c;
            g[q][q] = c;
            g[p][q] = s;
            g[q][p] = -s;
            a = sym(&mm(&tr_(&g), &mm(&a, &g)));
            v = mm(&v, &g);
        }
    }
    ([a[0][0], a[1][1], a[2][2]], v)
}

/// `R diag(f(w)) R^T` of a symmetric matrix.
pub fn sym_fn(a: &M3, f: impl Fn(f64) -> f64) -> M3 {
    let (w, r) = eig_sym(a);
    rebuild(&r, [f(w[0]), f(w[1]), f(w[2])])
}

fn rebuild(r: &M3, d: [f64; 3]) -> M3 {
    let mut out = [[0.0; 3]; 3];
    for (i, oi) in out.iter_mut().enumerate() {
        for (j, oij) in oi.iter_mut().enumerate() {
            *oij = (0..3).map(|k| r[i][k] * d[k] * r[j][k]).sum();
        }
    }
    out
}

/// Constitutive model with its parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Model {
    /// Oldroyd-B.
    OldroydB {
        /// Relaxation time.
        lambda: f64,
        /// Polymer viscosity.
        eta_p: f64,
    },
    /// Giesekus with mobility `alpha` in `[0, 1]`.
    Giesekus {
        /// Relaxation time.
        lambda: f64,
        /// Polymer viscosity.
        eta_p: f64,
        /// Mobility factor.
        alpha: f64,
    },
    /// Simplified linear PTT with extensibility `epsilon`.
    Ptt {
        /// Relaxation time.
        lambda: f64,
        /// Polymer viscosity.
        eta_p: f64,
        /// Extensibility parameter.
        epsilon: f64,
    },
    /// FENE-P with maximum extensibility `l2 > 3`.
    FeneP {
        /// Relaxation time.
        lambda: f64,
        /// Polymer viscosity.
        eta_p: f64,
        /// Squared maximum extensibility.
        l2: f64,
    },
}

/// Evolved quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formulation {
    /// The conformation tensor `C`.
    Conformation,
    /// `Theta = log C`.
    LogConformation,
}

impl Model {
    /// Relaxation time.
    pub fn lambda(&self) -> f64 {
        match *self {
            Model::OldroydB { lambda, .. }
            | Model::Giesekus { lambda, .. }
            | Model::Ptt { lambda, .. }
            | Model::FeneP { lambda, .. } => lambda,
        }
    }

    /// Polymer viscosity.
    pub fn eta_p(&self) -> f64 {
        match *self {
            Model::OldroydB { eta_p, .. }
            | Model::Giesekus { eta_p, .. }
            | Model::Ptt { eta_p, .. }
            | Model::FeneP { eta_p, .. } => eta_p,
        }
    }

    /// The same model with another polymer viscosity.
    pub fn with_eta_p(mut self, v: f64) -> Self {
        match &mut self {
            Model::OldroydB { eta_p, .. }
            | Model::Giesekus { eta_p, .. }
            | Model::Ptt { eta_p, .. }
            | Model::FeneP { eta_p, .. } => *eta_p = v,
        }
        self
    }

    /// Located parameter check; the message names the offending parameter.
    pub fn validate(&self) -> Result<(), String> {
        let lam = self.lambda();
        if !(lam.is_finite() && lam > 0.0) {
            return Err(format!("viscoelastic lambda must be finite and > 0, got {lam}"));
        }
        let ep = self.eta_p();
        if !(ep.is_finite() && ep >= 0.0) {
            return Err(format!("viscoelastic eta_p must be finite and >= 0, got {ep}"));
        }
        match *self {
            Model::Giesekus { alpha, .. } if !(0.0..=1.0).contains(&alpha) => {
                Err(format!("viscoelastic alpha must be in [0, 1], got {alpha}"))
            }
            Model::Ptt { epsilon, .. } if !(epsilon.is_finite() && epsilon >= 0.0) => {
                Err(format!("viscoelastic epsilon must be finite and >= 0, got {epsilon}"))
            }
            Model::FeneP { l2, .. } if !(l2.is_finite() && l2 > 3.0) => {
                Err(format!("viscoelastic l2 must be finite and > 3, got {l2}"))
            }
            _ => Ok(()),
        }
    }

    /// FENE-P factor `f(tr C)`; 1 for the other models.
    fn f_fene(&self, trc: f64) -> f64 {
        match *self {
            Model::FeneP { l2, .. } => (l2 - 3.0) / (l2 - trc).max(1e-12),
            _ => 1.0,
        }
    }

    /// Scalar relaxation function of an eigenvalue `c` given the trace of `C`: `P(C)` has eigenvalues `p(c_i)`.
    fn p_eig(&self, c: f64, trc: f64) -> f64 {
        match *self {
            Model::OldroydB { .. } => c - 1.0,
            Model::Giesekus { alpha, .. } => (c - 1.0) + alpha * (c - 1.0) * (c - 1.0),
            Model::Ptt { epsilon, .. } => (1.0 + epsilon * (trc - 3.0)) * (c - 1.0),
            Model::FeneP { .. } => self.f_fene(trc) * c - 1.0,
        }
    }

    /// `P(C)`.
    fn p_of_c(&self, c: &M3) -> M3 {
        let t = trace(c);
        match *self {
            Model::OldroydB { .. } => add(c, &scale(&EYE, -1.0)),
            Model::Giesekus { alpha, .. } => {
                let a = add(c, &scale(&EYE, -1.0));
                add(&a, &scale(&mm(&a, &a), alpha))
            }
            Model::Ptt { epsilon, .. } => scale(&add(c, &scale(&EYE, -1.0)), 1.0 + epsilon * (t - 3.0)),
            Model::FeneP { .. } => add(&scale(c, self.f_fene(t)), &scale(&EYE, -1.0)),
        }
    }

    /// Polymer extra stress from the conformation tensor (divided by the density: kinematic units are used by the solver).
    pub fn stress(&self, c: &M3) -> M3 {
        let g = self.eta_p() / self.lambda();
        match *self {
            Model::FeneP { .. } => scale(&add(&scale(c, self.f_fene(trace(c))), &scale(&EYE, -1.0)), g),
            _ => scale(&add(c, &scale(&EYE, -1.0)), g),
        }
    }

    /// Stiffness estimate of the relaxation term (1/s) for the sub-step count.
    fn stiffness(&self, c: &M3) -> f64 {
        let lam = self.lambda();
        let n = fro(c);
        let extra = match *self {
            Model::OldroydB { .. } => 1.0,
            Model::Giesekus { alpha, .. } => 1.0 + 2.0 * alpha * n,
            Model::Ptt { epsilon, .. } => 1.0 + epsilon * (trace(c) - 3.0).abs() + epsilon * n,
            Model::FeneP { .. } => self.f_fene(trace(c)) * (1.0 + n / 3.0),
        };
        extra / lam
    }
}

/// Right-hand side `dC/dt` of the local conformation equation.
fn rhs_c(model: &Model, c: &M3, l: &M3) -> M3 {
    let lc = mm(l, c);
    let mut r = add(&lc, &tr_(&lc));
    r = add(&r, &scale(&model.p_of_c(c), -1.0 / model.lambda()));
    sym(&r)
}

/// Right-hand side `dTheta/dt` of the log-conformation equation (ADR 0051 D3).
fn rhs_log(model: &Model, theta: &M3, l: &M3) -> M3 {
    let (th, r) = eig_sym(theta);
    let c = [th[0].exp(), th[1].exp(), th[2].exp()];
    let trc = c[0] + c[1] + c[2];
    let rt = tr_(&r);
    let m = mm(&rt, &mm(l, &r));
    let lam = model.lambda();
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let cd = if i == j {
                2.0 * m[i][i] * c[i] - model.p_eig(c[i], trc) / lam
            } else {
                m[i][j] * c[j] + c[i] * m[j][i]
            };
            let dd = if i == j || (th[i] - th[j]).abs() < 1e-9 {
                let mean = 0.5 * (th[i] + th[j]);
                (-mean).exp()
            } else {
                (th[i] - th[j]) / (c[i] - c[j])
            };
            out[i][j] = dd * cd;
        }
    }
    sym(&mm(&r, &mm(&out, &rt)))
}

fn rhs(model: &Model, form: Formulation, s: &M3, l: &M3) -> M3 {
    match form {
        Formulation::Conformation => rhs_c(model, s, l),
        Formulation::LogConformation => rhs_log(model, s, l),
    }
}

/// Convert the evolved quantity to the conformation tensor.
pub fn to_conformation(form: Formulation, s: &M3) -> M3 {
    match form {
        Formulation::Conformation => *s,
        Formulation::LogConformation => sym_fn(s, f64::exp),
    }
}

/// Initial evolved quantity for the equilibrium state `C = I`.
pub fn equilibrium(form: Formulation) -> M3 {
    match form {
        Formulation::Conformation => EYE,
        Formulation::LogConformation => [[0.0; 3]; 3],
    }
}

/// Convert a conformation tensor to the evolved quantity. The tensor must be symmetric positive definite for the log form.
pub fn from_conformation(form: Formulation, c: &M3) -> Result<M3, String> {
    match form {
        Formulation::Conformation => Ok(*c),
        Formulation::LogConformation => {
            let (w, _) = eig_sym(c);
            if !w.iter().all(|x| x.is_finite() && *x > 0.0) {
                return Err(format!(
                    "initial conformation tensor is not positive definite (eigenvalues {w:?})"
                ));
            }
            Ok(sym_fn(c, f64::ln))
        }
    }
}

/// Advance the local state over `dt` in a homogeneous velocity gradient `l` (RK4 sub-steps, ADR 0051 D4).
pub fn advance_local(model: &Model, form: Formulation, s0: &M3, l: &M3, dt: f64) -> M3 {
    let c = to_conformation(form, s0);
    let rate = 2.0 * fro(l) + model.stiffness(&c);
    let n = ((dt * rate / 0.5).ceil() as usize).clamp(1, 200_000);
    let h = dt / n as f64;
    let mut s = *s0;
    for _ in 0..n {
        let k1 = rhs(model, form, &s, l);
        let k2 = rhs(model, form, &add(&s, &scale(&k1, 0.5 * h)), l);
        let k3 = rhs(model, form, &add(&s, &scale(&k2, 0.5 * h)), l);
        let k4 = rhs(model, form, &add(&s, &scale(&k3, h)), l);
        let sum = add(&add(&k1, &k4), &scale(&add(&k2, &k3), 2.0));
        s = add(&s, &scale(&sum, h / 6.0));
    }
    s
}

/// Simple-shear velocity gradient `u = (gdot y, 0, 0)`.
pub fn simple_shear(gdot: f64) -> M3 {
    [[0.0, gdot, 0.0], [0.0; 3], [0.0; 3]]
}

/// Uniaxial extension `L = diag(-e/2, -e/2, e)` (axis z).
pub fn uniaxial_extension(e: f64) -> M3 {
    [[-0.5 * e, 0.0, 0.0], [0.0, -0.5 * e, 0.0], [0.0, 0.0, e]]
}

/// Planar extension `L = diag(e, -e, 0)`.
pub fn planar_extension(e: f64) -> M3 {
    [[e, 0.0, 0.0], [0.0, -e, 0.0], [0.0; 3]]
}

/// Oldroyd-B stress growth after the sudden start of simple shear from rest: `(tau_xy, N1)` at time `t`.
pub fn oldroyd_b_startup(lambda: f64, eta_p: f64, gdot: f64, t: f64) -> (f64, f64) {
    let s = t / lambda;
    let e = (-s).exp();
    (
        eta_p * gdot * (1.0 - e),
        2.0 * eta_p * lambda * gdot * gdot * (1.0 - e * (1.0 + s)),
    )
}

/// Steady simple-shear viscometric functions `(tau_xy/gdot, N1)` of a model at shear rate `gdot` (closed forms of ADR 0051 V3;
/// the Giesekus value is derived in ADR 0051 and checked against the ODE and an independent Newton solve).
pub fn steady_shear(model: &Model, gdot: f64) -> (f64, f64) {
    let (lam, ep) = (model.lambda(), model.eta_p());
    let wi = lam * gdot;
    let n1_of = |f: f64| 2.0 * ep * lam * gdot * gdot / (f * f);
    match *model {
        Model::OldroydB { .. } => (ep, 2.0 * ep * lam * gdot * gdot),
        Model::Ptt { epsilon, .. } => {
            let y = solve_cubic_factor(|y| y * y * (y - 1.0) - 2.0 * epsilon * wi * wi);
            (ep / y, n1_of(y))
        }
        Model::FeneP { l2, .. } => {
            let f = solve_cubic_factor(|f| l2 * f * f * (f - 1.0) - 2.0 * wi * wi);
            (ep / f, n1_of(f))
        }
        Model::Giesekus { alpha: a, .. } => {
            if wi == 0.0 {
                return (ep, 0.0);
            }
            // derived (ADR 0051 addendum): with f = -A_yy, Wi^2 = f (1 - a f) (1 + (1-2a) f)^2 / (a (1-f)^4), f in [0, 1)
            let w2 = wi * wi;
            let h = |f: f64| f * (1.0 - a * f) * (1.0 + (1.0 - 2.0 * a) * f).powi(2) / (a * (1.0 - f).powi(4));
            let (mut lo, mut hi) = (0.0f64, 1.0 - 1e-15);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if h(mid) < w2 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let f = 0.5 * (lo + hi);
            let q = 1.0 + (1.0 - 2.0 * a) * f;
            let eta = ep * (1.0 - f) * (1.0 - f) / q;
            let n1 = ep / lam * 2.0 * w2 * (1.0 - f).powi(3) / (q * q);
            (eta, n1)
        }
    }
}

/// Root `>= 1` of an increasing function (zero at 1 is negative) by bisection.
fn solve_cubic_factor(g: impl Fn(f64) -> f64) -> f64 {
    let (mut lo, mut hi) = (1.0f64, 2.0f64);
    while g(hi) < 0.0 {
        hi *= 2.0;
        if hi > 1e300 {
            break;
        }
    }
    for _ in 0..300 {
        let mid = 0.5 * (lo + hi);
        if g(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Velocity-gradient tensor `L_ij = d u_i / d x_j` at every cell from Green-Gauss gradients of the components.
pub fn velocity_gradient(geom: &FvGeometry, u: &VectorField) -> Vec<M3> {
    let bv = u.boundary_values(geom);
    let n = geom.n_cells();
    let mut l = vec![[[0.0; 3]; 3]; n];
    for i in 0..3 {
        let cells: Vec<f64> = u.values.iter().map(|v| v[i]).collect();
        let bi: Vec<f64> = bv.iter().map(|v| v[i]).collect();
        let g = green_gauss(geom, &cells, &bi, 0);
        for c in 0..n {
            l[c][i] = g[c];
        }
    }
    l
}

/// Viscoelastic flow solver: [`PimpleSolver`] plus a cell-centred stress carrier (ADR 0051 D5).
pub struct ViscoelasticSolver {
    /// The flow solver (its `settings.nu` is the TOTAL kinematic viscosity `nu_s + nu_p`).
    pub flow: PimpleSolver,
    /// Constitutive model (density 1: `eta_p` is kinematic).
    pub model: Model,
    /// Evolved quantity.
    pub form: Formulation,
    /// Evolved tensor per cell (`C` or `Theta`).
    pub state: Vec<M3>,
    /// Uniform extra acceleration (the driving pressure gradient of the channel checks).
    pub body: Vec3,
    nu_s: f64,
}

impl ViscoelasticSolver {
    /// Build the solver; `settings.nu` is the solvent viscosity and is replaced by `nu_s + eta_p` internally.
    pub fn new(
        geom: Arc<FvGeometry>,
        u: VectorField,
        p: ScalarField,
        mut settings: PimpleSettings,
        model: Model,
        form: Formulation,
        body: Vec3,
    ) -> Result<Self, String> {
        model.validate()?;
        let nu_s = settings.nu;
        settings.nu = nu_s + model.eta_p();
        let n = geom.n_cells();
        let flow = PimpleSolver::new(geom, u, p, settings);
        Ok(ViscoelasticSolver {
            flow,
            model,
            form,
            state: vec![equilibrium(form); n],
            body,
            nu_s,
        })
    }

    /// Polymer stress per cell.
    pub fn tau(&self) -> Vec<M3> {
        self.state
            .iter()
            .map(|s| self.model.stress(&to_conformation(self.form, s)))
            .collect()
    }

    /// Smallest eigenvalue of `C` over all cells.
    pub fn min_eig_c(&self) -> f64 {
        self.state
            .iter()
            .map(|s| {
                eig_sym(&to_conformation(self.form, s))
                    .0
                    .iter()
                    .cloned()
                    .fold(f64::INFINITY, f64::min)
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// Solvent viscosity.
    pub fn nu_s(&self) -> f64 {
        self.nu_s
    }

    /// Momentum source `div(tau_p) - nu_p lap(u) + body` per cell (kinematic), from the lagged state.
    fn momentum_source(&self) -> Vec<Vec3> {
        let g = self.flow.geom.clone();
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let vol = mesh.cell_volumes();
        let n = g.n_cells();
        let nu_p = self.model.eta_p();
        let mut out = vec![self.body; n];
        if nu_p == 0.0 {
            return out;
        }
        let tau = self.tau();
        let u = &self.flow.u;
        let bu = u.boundary_values(&g);
        let mut acc = vec![[0.0; 3]; n];
        let mut push = |c: usize, sign: f64, t: &M3, s: Vec3, du: Vec3, coeff: f64| {
            for i in 0..3 {
                let div: f64 = (0..3).map(|j| t[i][j] * s[j]).sum();
                acc[c][i] += sign * (div - nu_p * coeff * du[i]);
            }
        };
        for f in 0..g.n_internal {
            let (o, nb) = (own[f], nei[f]);
            let w = g.weights[f];
            let mut tf = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    tf[i][j] = w * tau[o][i][j] + (1.0 - w) * tau[nb][i][j];
                }
            }
            let du = [0, 1, 2].map(|k| u.values[nb][k] - u.values[o][k]);
            push(o, 1.0, &tf, sf[f], du, g.lap_coeff[f]);
            push(nb, -1.0, &tf, sf[f], du, g.lap_coeff[f]);
        }
        // partner cells of periodic faces
        let mut partner = vec![usize::MAX; mesh.n_faces()];
        for (pi, p) in mesh.patches().iter().enumerate() {
            if let Some(map) = &g.periodic_partner[pi] {
                for (k, f) in p.faces().enumerate() {
                    partner[f] = own[map[k]];
                }
            }
        }
        for f in g.n_internal..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            let o = own[f];
            if partner[f] != usize::MAX {
                let q = partner[f];
                let mut tf = [[0.0; 3]; 3];
                for i in 0..3 {
                    for j in 0..3 {
                        tf[i][j] = 0.5 * (tau[o][i][j] + tau[q][i][j]);
                    }
                }
                let du = [0, 1, 2].map(|k| u.values[q][k] - u.values[o][k]);
                push(o, 1.0, &tf, sf[f], du, 0.5 * g.lap_coeff[f]);
            } else {
                let du = [0, 1, 2].map(|k| bu[f - g.n_internal][k] - u.values[o][k]);
                push(o, 1.0, &tau[o], sf[f], du, g.lap_coeff[f]);
            }
        }
        for c in 0..n {
            for i in 0..3 {
                out[c][i] += acc[c][i] / vol[c];
            }
        }
        out
    }

    /// Upwind transport of the stress carrier over `dt` on the flux `phi` (sub-cycled to a cell Courant number of 0.5).
    fn transport(&mut self, dt: f64) {
        let g = self.flow.geom.clone();
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let vol = mesh.cell_volumes();
        let phi = &self.flow.phi;
        let n = g.n_cells();
        let mut out_flux = vec![0.0f64; n];
        for f in 0..g.n_internal {
            if phi[f] > 0.0 {
                out_flux[own[f]] += phi[f];
            } else {
                out_flux[nei[f]] -= phi[f];
            }
        }
        let cmax = (0..n).map(|c| out_flux[c] / vol[c]).fold(0.0f64, f64::max) * dt;
        if cmax < 1e-14 {
            return;
        }
        let nsub = (cmax / 0.5).ceil().max(1.0) as usize;
        let h = dt / nsub as f64;
        let mut partner = vec![usize::MAX; mesh.n_faces()];
        for (pi, p) in mesh.patches().iter().enumerate() {
            if let Some(map) = &g.periodic_partner[pi] {
                for (k, f) in p.faces().enumerate() {
                    partner[f] = own[map[k]];
                }
            }
        }
        for _ in 0..nsub {
            let s0 = self.state.clone();
            let mut d = vec![[[0.0; 3]; 3]; n];
            let mut acc = |c: usize, sign: f64, phi_f: f64, up: &M3, here: &M3| {
                for i in 0..3 {
                    for j in 0..3 {
                        d[c][i][j] += sign * phi_f * (up[i][j] - here[i][j]);
                    }
                }
            };
            for f in 0..g.n_internal {
                let (o, nb) = (own[f], nei[f]);
                let up = if phi[f] > 0.0 { &s0[o] } else { &s0[nb] };
                acc(o, 1.0, phi[f], up, &s0[o]);
                acc(nb, -1.0, phi[f], up, &s0[nb]);
            }
            for f in g.n_internal..mesh.n_faces() {
                if partner[f] != usize::MAX && phi[f] < 0.0 {
                    let o = own[f];
                    acc(o, 1.0, phi[f], &s0[partner[f]], &s0[o]);
                }
            }
            for c in 0..n {
                for i in 0..3 {
                    for j in 0..3 {
                        self.state[c][i][j] = s0[c][i][j] - h * d[c][i][j] / vol[c];
                    }
                }
            }
        }
    }

    /// Advance one time step (flow with the lagged stress source, then stress transport and local update on the new velocity).
    pub fn step(&mut self, dt: f64) {
        if self.model.eta_p() > 0.0 || self.body != [0.0; 3] {
            let src = self.momentum_source();
            self.flow.source_force = Some(BodyForce::from_cells(&self.flow.geom, src));
        }
        self.flow.step(dt);
        if self.model.eta_p() > 0.0 {
            let l = velocity_gradient(&self.flow.geom, &self.flow.u);
            self.transport(dt);
            for (s, lc) in self.state.iter_mut().zip(&l) {
                *s = advance_local(&self.model, self.form, s, lc, dt);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jacobi_and_log_roundtrip() {
        let a = [[2.0, 0.3, -0.1], [0.3, 1.5, 0.2], [-0.1, 0.2, 0.7]];
        let (w, r) = eig_sym(&a);
        let b = rebuild(&r, w);
        for i in 0..3 {
            for j in 0..3 {
                assert!((a[i][j] - b[i][j]).abs() < 1e-13);
            }
        }
        let l = sym_fn(&a, f64::ln);
        let e = sym_fn(&l, f64::exp);
        for i in 0..3 {
            for j in 0..3 {
                assert!((a[i][j] - e[i][j]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn validate_names_parameter() {
        let e = Model::Giesekus {
            lambda: 1.0,
            eta_p: 1.0,
            alpha: 2.0,
        }
        .validate()
        .unwrap_err();
        assert!(e.contains("alpha"));
    }
}
