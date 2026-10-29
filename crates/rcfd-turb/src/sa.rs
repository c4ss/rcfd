//! Spalart-Allmaras one-equation eddy-viscosity model, SA-noft2 (F-041).
//!
//! Sources: P. R. Spalart and S. R. Allmaras, "A one-equation turbulence model for aerodynamic flows",
//! La Recherche Aerospatiale 1 (1994) 5-21; the NASA Turbulence Modeling Resource page
//! (https://tmbwg.github.io/turbmodels/spalart.html, fetched, see docs/reference-data.md and ADR 0014) for the
//! SA-noft2 form (no `f_t2`), the `S~` limiters and the negative-`nuT` treatment of Allmaras, Johnson and Spalart
//! (2012). Incompressible, kinematic form:
//!
//! ```text
//! D nuT/Dt = cb1 S~ nuT - cw1 fw (nuT/d)^2 + 1/sigma [ div((nu + nuT) grad nuT) + cb2 |grad nuT|^2 ]
//! nut = nuT fv1,  chi = nuT/nu,  fv1 = chi^3/(chi^3 + cv1^3),  fv2 = 1 - chi/(1 + chi fv1)
//! S~ = Omega + nuT fv2/(kappa^2 d^2),  Omega = sqrt(2 W_ij W_ij)
//! fw = g [(1 + cw3^6)/(g^6 + cw3^6)]^(1/6),  g = r + cw2 (r^6 - r),  r = min(nuT/(S~ kappa^2 d^2), 10)
//! cw1 = cb1/kappa^2 + (1 + cb2)/sigma
//! ```
//!
//! Negative `nuT` (SA-neg): production `cb1 Omega nuT`, destruction `+cw1 (nuT/d)^2`, diffusion `(nu + fn nuT)`,
//! `fn = (cn1 + chi^3)/(cn1 - chi^3)`, `nut = 0`.
//!
//! Numerics: the production and the cb2 term are explicit, the destruction is linearised on the diagonal, convection and
//! diffusion go through the shared finite-volume operators. Wall treatment: see [`SaWallMode`].

use crate::eddy::{effective_viscosity, patch_faces};
use crate::kepsilon::velocity_gradient;
use crate::wall::{wall_distance, SpaldingLaw};
use rcfd_core::vec3;
use rcfd_core::Result;
use rcfd_fv::convection::{convection, ConvScheme};
use rcfd_fv::diffusion::laplacian;
use rcfd_fv::gradient::{green_gauss, least_squares};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// Model coefficients (defaults: the TMR values).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaCoefficients {
    /// `sigma`.
    pub sigma: f64,
    /// `cb1`.
    pub cb1: f64,
    /// `cb2`.
    pub cb2: f64,
    /// von Karman constant used inside the model.
    pub kappa: f64,
    /// `cw2`.
    pub cw2: f64,
    /// `cw3`.
    pub cw3: f64,
    /// `cv1`.
    pub cv1: f64,
    /// `cn1` of the negative-`nuT` branch.
    pub cn1: f64,
    /// `c2` of the Allmaras-Johnson-Spalart `S~` limiter.
    pub c2: f64,
    /// `c3` of the Allmaras-Johnson-Spalart `S~` limiter.
    pub c3: f64,
}

impl Default for SaCoefficients {
    fn default() -> Self {
        SaCoefficients {
            sigma: 2.0 / 3.0,
            cb1: 0.1355,
            cb2: 0.622,
            kappa: 0.41,
            cw2: 0.3,
            cw3: 2.0,
            cv1: 7.1,
            cn1: 16.0,
            c2: 0.7,
            c3: 0.9,
        }
    }
}

impl SaCoefficients {
    /// `cw1 = cb1/kappa^2 + (1 + cb2)/sigma`.
    pub fn cw1(&self) -> f64 {
        self.cb1 / (self.kappa * self.kappa) + (1.0 + self.cb2) / self.sigma
    }
}

/// Limiter of `S~` (TMR options a, b, c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SLimiter {
    /// Allmaras, Johnson and Spalart (2012), `c2 = 0.7`, `c3 = 0.9` (default).
    Allmaras,
    /// `S~ >= 0.3 Omega` (Spalart's recommendation).
    Min03,
    /// `S~ > 0` only.
    Positive,
}

/// Wall treatment of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaWallMode {
    /// `nuT = 0` on the wall face and no wall function: needs a wall-resolved mesh (y+ of order 1).
    Resolved,
    /// Spalding law of the wall for the wall shear and strain; for a wall cell with y+ > 5 the cell `nuT` is fixed to the
    /// value whose `nut` equals the Spalding eddy viscosity `nu (dy+/du+ - 1)`. Works from y+ about 1 into the log layer.
    Auto,
    /// Opt-in (ADR 0168): the model's own inner-layer solution instead of Spalding's law. Wall shear, y+ and wall-face
    /// `nu_t` from [`SaInnerLaw`]; for a wall cell with y+ > `fix_y_plus` the cell `nuT` is fixed to `kappa u_tau y` and
    /// the face between the wall cell and its wall-normal neighbour gets the law's series-resistance viscosity
    /// `nu (y2+ - y1+)/(u+(y2+) - u+(y1+))`; every other internal face gets `nut` of the interpolated `nuT`.
    SaLaw,
}

/// Inner-layer law of the wall of the SA model itself (ADR 0168): in the constant-stress layer `nuT = kappa u_tau y`
/// (Spalart and Allmaras 1994, through the TMR page), so `du+/dy+ = 1/(1 + nut+)`, `nut+ = kappa y+ fv1(kappa y+)`.
/// `u+(y+)` is tabulated once by Simpson quadrature in `ln y+` and read by cubic Hermite interpolation with the exact
/// derivative; below the table `u+ = y+`, above it the log asymptote `u+ = u+_end + ln((1 + kappa y+)/(1 + kappa y+_end))/kappa`.
#[derive(Debug, Clone)]
pub struct SaInnerLaw {
    kappa: f64,
    cv1: f64,
    s0: f64,
    ds: f64,
    u: Vec<f64>,
}

impl SaInnerLaw {
    const Y_MIN: f64 = 1e-3;
    const Y_MAX: f64 = 1e8;
    const PER_DECADE: usize = 64;
    const SUB: usize = 8;

    /// Table for the model constants `kappa` and `cv1`.
    pub fn new(kappa: f64, cv1: f64) -> Self {
        let s0 = Self::Y_MIN.ln();
        let n = 11 * Self::PER_DECADE;
        let ds = (Self::Y_MAX.ln() - s0) / n as f64;
        let mut law = SaInnerLaw {
            kappa,
            cv1,
            s0,
            ds,
            u: Vec::with_capacity(n + 1),
        };
        let mut u = Self::Y_MIN;
        law.u.push(u);
        let h = ds / Self::SUB as f64;
        for i in 0..n {
            let a = s0 + i as f64 * ds;
            let mut sum = 0.0;
            for k in 0..=Self::SUB {
                let w = if k == 0 || k == Self::SUB {
                    1.0
                } else if k % 2 == 1 {
                    4.0
                } else {
                    2.0
                };
                sum += w * law.du_ds((a + k as f64 * h).exp());
            }
            u += sum * h / 3.0;
            law.u.push(u);
        }
        law
    }

    /// `nut/nu` of the inner layer at `y+`.
    pub fn nut_plus(&self, y_plus: f64) -> f64 {
        let chi = self.kappa * y_plus;
        if chi <= 0.0 {
            0.0
        } else {
            chi * fv1(chi, self.cv1)
        }
    }

    fn du_ds(&self, y_plus: f64) -> f64 {
        y_plus / (1.0 + self.nut_plus(y_plus))
    }

    /// `u+(y+)`.
    pub fn u_plus(&self, y_plus: f64) -> f64 {
        if y_plus <= Self::Y_MIN {
            return y_plus.max(0.0);
        }
        let n = self.u.len() - 1;
        if y_plus >= Self::Y_MAX {
            let k = self.kappa;
            return self.u[n] + ((1.0 + k * y_plus) / (1.0 + k * Self::Y_MAX)).ln() / k;
        }
        let s = y_plus.ln();
        let i = (((s - self.s0) / self.ds) as usize).min(n - 1);
        let sa = self.s0 + i as f64 * self.ds;
        let t = ((s - sa) / self.ds).clamp(0.0, 1.0);
        let (ua, ub) = (self.u[i], self.u[i + 1]);
        let (da, db) = (
            self.du_ds(sa.exp()) * self.ds,
            self.du_ds((sa + self.ds).exp()) * self.ds,
        );
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * ua + (t3 - 2.0 * t2 + t) * da + (-2.0 * t3 + 3.0 * t2) * ub + (t3 - t2) * db
    }

    /// Friction velocity from the tangential speed `u` at distance `y` (bisection; `u_tau u+(y u_tau/nu)` increases
    /// with `u_tau`). Zero speed gives zero.
    pub fn u_tau(&self, nu: f64, u: f64, y: f64) -> f64 {
        let u = u.abs();
        if u == 0.0 {
            return 0.0;
        }
        let g = |ut: f64| ut * self.u_plus(y * ut / nu) - u;
        let mut hi = 2.0 * u.max((nu * u / y).sqrt());
        let mut lo = hi * 1e-8;
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if g(mid) > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
            if hi - lo < 1e-14 * hi {
                break;
            }
        }
        0.5 * (lo + hi)
    }
}

/// `fv1`.
pub fn fv1(chi: f64, cv1: f64) -> f64 {
    let c3 = chi * chi * chi;
    c3 / (c3 + cv1 * cv1 * cv1)
}

/// `fv2`.
pub fn fv2(chi: f64, cv1: f64) -> f64 {
    1.0 - chi / (1.0 + chi * fv1(chi, cv1))
}

/// `fw` from `r`.
pub fn fw(r: f64, cw2: f64, cw3: f64) -> f64 {
    let g = r + cw2 * (r.powi(6) - r);
    let c6 = cw3.powi(6);
    g * ((1.0 + c6) / (g.powi(6) + c6)).powf(1.0 / 6.0)
}

/// Eddy viscosity `nut = nuT fv1(nuT/nu)` (zero for `nuT <= 0`).
pub fn nut_from_nutilde(nutilde: f64, nu: f64, cv1: f64) -> f64 {
    if nutilde <= 0.0 {
        0.0
    } else {
        nutilde * fv1(nutilde / nu, cv1)
    }
}

/// Inverse of [`nut_from_nutilde`]: the `nuT >= 0` with `nuT fv1 = nut`.
pub fn nutilde_from_nut(nut: f64, nu: f64, cv1: f64) -> f64 {
    if nut <= 0.0 {
        return 0.0;
    }
    // f(chi) = chi fv1(chi) increases from 0 and f(chi) <= chi, so chi >= nut/nu
    let target = nut / nu;
    let f = |chi: f64| chi * fv1(chi, cv1);
    let (mut lo, mut hi) = (target, target + 2.0 * cv1 + 1.0);
    while f(hi) < target {
        hi *= 2.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi) * nu
}

/// `S~` for the vorticity `omega`, the term `sbar = nuT fv2/(kappa^2 d^2)` and a limiter.
pub fn s_tilde(omega: f64, sbar: f64, limiter: SLimiter, c2: f64, c3: f64) -> f64 {
    match limiter {
        SLimiter::Allmaras => {
            if sbar >= -c2 * omega {
                omega + sbar
            } else {
                omega + omega * (c2 * c2 * omega + c3 * sbar) / ((c3 - 2.0 * c2) * omega - sbar)
            }
        }
        SLimiter::Min03 => (omega + sbar).max(0.3 * omega),
        SLimiter::Positive => (omega + sbar).max(0.0),
    }
}

/// `r = min(nuT/(S~ kappa^2 d^2), 10)`, 10 when `S~ = 0`.
pub fn r_function(nutilde: f64, s_t: f64, kappa: f64, d: f64) -> f64 {
    if s_t <= 0.0 {
        10.0
    } else {
        (nutilde / (s_t * kappa * kappa * d * d)).min(10.0)
    }
}

/// Hybrid RANS-LES mode of the SA model (ADR 0032).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesMode {
    /// Plain RANS: the length scale is the wall distance.
    Off,
    /// Detached-eddy simulation (Spalart et al. 1997): `d~ = min(d, C_DES Delta)`.
    Des,
    /// Delayed DES (Spalart et al. 2006): `d~ = d - f_d max(0, d - C_DES Delta)`.
    Ddes,
}

/// Definition of the grid scale `Delta` of the hybrid length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesDelta {
    /// Largest distance between two face centres of the cell (empty-direction faces excluded): `max(dx, dy, dz)` on boxes.
    MaxExtent,
    /// Cube root of the cell volume.
    CbrtVolume,
}

/// DES length scale `min(d, c_des Delta)`.
pub fn des_length(d: f64, delta: f64, c_des: f64) -> f64 {
    d.min(c_des * delta)
}

/// DDES shielding function `f_d = 1 - tanh[(8 r_d)^3]`.
pub fn ddes_fd(rd: f64) -> f64 {
    1.0 - (8.0 * rd).powi(3).tanh()
}

/// DDES argument `r_d = (nu + nu_t)/(max(sqrt(U_ij U_ij), 1e-10) kappa^2 d^2)`.
pub fn ddes_rd(nu: f64, nut: f64, grad_mag: f64, kappa: f64, d: f64) -> f64 {
    (nu + nut) / (grad_mag.max(1e-10) * kappa * kappa * d * d)
}

/// DDES length scale `d - f_d max(0, d - c_des Delta)`.
pub fn ddes_length(d: f64, delta: f64, c_des: f64, fd: f64) -> f64 {
    d - fd * (d - c_des * delta).max(0.0)
}

/// Production factor `S~` and destruction function `fw` of a cell with `nuT = x >= 0`, vorticity `omega` and length `d`.
pub fn sa_s_fw(x: f64, omega: f64, d: f64, nu: f64, cf: &SaCoefficients, limiter: SLimiter) -> (f64, f64) {
    let chi = x / nu;
    let sb = x * fv2(chi, cf.cv1) / (cf.kappa * cf.kappa * d * d);
    let st = s_tilde(omega, sb, limiter, cf.c2, cf.c3);
    let r = r_function(x, st, cf.kappa, d);
    (st, fw(r, cf.cw2, cf.cw3))
}

/// Local source `cb1 S~ nuT - cw1 fw (nuT/d)^2` of the SA equation (`nuT >= 0`).
pub fn sa_local_source(x: f64, omega: f64, d: f64, nu: f64, cf: &SaCoefficients, limiter: SLimiter) -> f64 {
    let (st, fwv) = sa_s_fw(x, omega, d, nu, cf, limiter);
    cf.cb1 * st.max(0.0) * x - cf.cw1() * fwv * x * x / (d * d)
}

/// Grid scale of every cell.
pub fn cell_delta(geom: &FvGeometry, kind: DesDelta) -> Vec<f64> {
    let mesh = &geom.mesh;
    match kind {
        DesDelta::CbrtVolume => mesh.cell_volumes().iter().map(|v| v.cbrt()).collect(),
        DesDelta::MaxExtent => {
            let fc = mesh.face_centres();
            (0..geom.n_cells())
                .map(|c| {
                    let faces: Vec<usize> = mesh.cell_faces(c).iter().copied().filter(|&f| !geom.empty[f]).collect();
                    let mut m: f64 = 0.0;
                    for (i, &a) in faces.iter().enumerate() {
                        for &b in &faces[i + 1..] {
                            m = m.max(vec3::norm(vec3::sub(fc[a], fc[b])));
                        }
                    }
                    m
                })
                .collect()
        }
    }
}

/// Steady Spalart-Allmaras model state.
pub struct SpalartAllmaras {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// The working variable `nuT` (m^2/s).
    pub nutilde: ScalarField,
    /// Laminar kinematic viscosity.
    pub nu: f64,
    /// Eddy viscosity of every cell (under-relaxed).
    pub nut: Vec<f64>,
    /// Wall eddy viscosity of the wall faces `(global face, nu_t,w)`.
    pub wall_nut: Vec<(usize, f64)>,
    /// Coefficients.
    pub coef: SaCoefficients,
    /// `S~` limiter.
    pub limiter: SLimiter,
    /// Negative-`nuT` treatment (SA-neg); off: `nuT` is clipped at zero.
    pub negative_nutilde: bool,
    /// Wall treatment.
    pub wall_mode: SaWallMode,
    /// Law of the wall (wall mode `Auto`, and the reported y+).
    pub law: SpaldingLaw,
    /// Wall-cell y+ above which the cell `nuT` is fixed in wall mode `Auto`.
    pub fix_y_plus: f64,
    /// Convection scheme.
    pub convection: ConvScheme,
    /// Implicit under-relaxation of the `nuT` equation.
    pub relax_turb: f64,
    /// Explicit under-relaxation of the eddy viscosity.
    pub relax_nut: f64,
    /// Linear solver controls.
    pub solver: SolveControl,
    /// Hybrid RANS-LES mode (default off).
    pub des: DesMode,
    /// `C_DES` (0.65).
    pub c_des: f64,
    /// Definition of the grid scale.
    pub des_delta: DesDelta,
    wall_dist: Vec<f64>,
    wall_faces: Vec<usize>,
    cell_wall_face: Vec<usize>,
    y_plus: Vec<f64>,
    started: bool,
    /// Time step of the current transient step (`None`: steady pseudo-time sweep).
    step_dt: Option<f64>,
    nutilde_old: Vec<f64>,
    pub(crate) delta_cache: Option<Vec<f64>>,
    // wall mode SaLaw only (ADR 0168): the inner law, the wall-normal first faces (wall face, internal face, neighbour,
    // wall-normal distance between the cell centres) and the relaxed internal-face eddy viscosity
    inner: Option<SaInnerLaw>,
    law_faces: Option<Vec<(usize, usize, f64)>>,
    face_nut: Vec<f64>,
}

impl SpalartAllmaras {
    /// New model with uniform `nuT = nutilde0`; `walls` are the wall patch names.
    pub fn new(geom: Arc<FvGeometry>, nu: f64, walls: &[&str], nutilde0: f64) -> Result<Self> {
        let wall_dist = wall_distance(&geom.mesh, walls)?;
        let wall_faces = patch_faces(&geom, walls);
        let own = geom.mesh.owner();
        let mut cell_wall_face = vec![usize::MAX; geom.n_cells()];
        for &f in &wall_faces {
            let c = own[f];
            if cell_wall_face[c] == usize::MAX
                || geom.dn[f - geom.n_internal] < geom.dn[cell_wall_face[c] - geom.n_internal]
            {
                cell_wall_face[c] = f;
            }
        }
        let n = geom.n_cells();
        let mut nutilde = ScalarField::uniform("nuTilda", &geom, nutilde0);
        for name in walls {
            if geom.mesh.patch(name).is_some() {
                nutilde.set_bc(&geom, name, Box::new(rcfd_fv::bc::FixedValue::scalar(0.0)))?;
            }
        }
        let coef = SaCoefficients::default();
        let nut0 = nut_from_nutilde(nutilde0, nu, coef.cv1);
        Ok(SpalartAllmaras {
            nutilde,
            nu,
            nut: vec![nut0; n],
            wall_nut: Vec::new(),
            coef,
            limiter: SLimiter::Allmaras,
            negative_nutilde: true,
            wall_mode: SaWallMode::Auto,
            law: SpaldingLaw::default(),
            fix_y_plus: 5.0,
            convection: ConvScheme::Upwind,
            relax_turb: 0.7,
            relax_nut: 0.5,
            solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-6,
                max_iter: 500,
            },
            wall_dist,
            des: DesMode::Off,
            c_des: 0.65,
            des_delta: DesDelta::MaxExtent,
            y_plus: vec![0.0; wall_faces.len()],
            wall_faces,
            cell_wall_face,
            geom,
            started: false,
            step_dt: None,
            nutilde_old: Vec::new(),
            delta_cache: None,
            inner: None,
            law_faces: None,
            face_nut: Vec::new(),
        })
    }

    /// Wall mode SaLaw (ADR 0168): for every wall cell, the internal face whose cell-centre step is closest to the wall
    /// normal (cosine above 0.5) and leads to a cell without a wall face, with the wall-normal distance of that step.
    fn build_law_faces(&self) -> Vec<(usize, usize, f64)> {
        let g = &self.geom;
        let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
        let cc = g.mesh.cell_centres();
        let mut out = Vec::new();
        for c1 in 0..g.n_cells() {
            let fw = self.cell_wall_face[c1];
            if fw == usize::MAX {
                continue;
            }
            let nw = g.normal(fw);
            let mut best: Option<(usize, usize, f64)> = None;
            let mut best_cos = 0.5;
            for &f in g.mesh.cell_faces(c1) {
                if f >= g.n_internal {
                    continue;
                }
                let c2 = if own[f] == c1 { nei[f] } else { own[f] };
                let d = vec3::sub(cc[c2], cc[c1]);
                let len = vec3::norm(d);
                let dy = -vec3::dot(d, nw);
                if len > 0.0 && dy / len > best_cos {
                    best_cos = dy / len;
                    best = Some((f, c2, dy));
                }
            }
            if let Some((f, c2, dy)) = best {
                if self.cell_wall_face[c2] == usize::MAX {
                    out.push((fw, f, dy));
                }
            }
        }
        out
    }

    /// Wall mode SaLaw: number of wall cells with a wall-normal first face (zero before the first sweep).
    pub fn law_face_count(&self) -> usize {
        self.law_faces.as_ref().map_or(0, Vec::len)
    }

    /// Wall distance of every cell centre.
    pub fn wall_distance(&self) -> &[f64] {
        &self.wall_dist
    }

    /// Wall faces (global indices) of the wall patches.
    pub fn wall_faces(&self) -> &[usize] {
        &self.wall_faces
    }

    /// `y+ = u_tau y/nu` (Spalding `u_tau`) of every wall face from the last sweep.
    pub fn y_plus(&self) -> &[f64] {
        &self.y_plus
    }

    /// Effective face viscosity `nu + nu_t` for the momentum equation.
    pub fn nu_face(&self) -> Vec<f64> {
        let mut out = effective_viscosity(&self.geom, self.nu, &self.nut, &self.wall_nut);
        if self.wall_mode == SaWallMode::SaLaw {
            for (o, &v) in out.iter_mut().zip(&self.face_nut) {
                *o = self.nu + v;
            }
        }
        out
    }

    /// Start from an eddy-viscosity field (e.g. the one of another model): `nuT` by inverting `nut = nuT fv1`.
    pub fn initialize_from_nut(&mut self, nut: &[f64]) {
        for (c, &v) in nut.iter().enumerate() {
            self.nutilde.values[c] = nutilde_from_nut(v, self.nu, self.coef.cv1);
        }
        self.nut = nut.to_vec();
        self.started = false;
    }

    fn tangential_speed(&self, u: &VectorField, f: usize) -> f64 {
        let c = self.geom.mesh.owner()[f];
        let n = self.geom.normal(f);
        let v = u.values[c];
        vec3::norm(vec3::sub(v, vec3::scale(n, vec3::dot(v, n))))
    }

    /// Start a transient step of size `dt`: the next [`SpalartAllmaras::update`] adds the implicit-Euler time derivative
    /// (no under-relaxation) with the present `nuT` as the old level.
    pub fn begin_step(&mut self, dt: f64) {
        self.step_dt = Some(dt);
        self.nutilde_old = self.nutilde.values.clone();
    }

    /// Length scale of every cell: the wall distance, or the DES / DDES length.
    fn length_scale(&mut self, grad_mag: &[f64]) -> Vec<f64> {
        let n = self.geom.n_cells();
        if self.des == DesMode::Off {
            return (0..n).map(|c| self.wall_dist[c].max(1e-30)).collect();
        }
        if self.delta_cache.is_none() {
            self.delta_cache = Some(cell_delta(&self.geom, self.des_delta));
        }
        let delta = self.delta_cache.as_ref().unwrap();
        (0..n)
            .map(|c| {
                let d = self.wall_dist[c].max(1e-30);
                let l = match self.des {
                    DesMode::Des => des_length(d, delta[c], self.c_des),
                    _ => {
                        let rd = ddes_rd(self.nu, self.nut[c], grad_mag[c], self.coef.kappa, d);
                        ddes_length(d, delta[c], self.c_des, ddes_fd(rd))
                    }
                };
                l.max(1e-30)
            })
            .collect()
    }

    /// One sweep: solve the `nuT` equation on the flux `phi` for the current velocity, then update `nut` and `wall_nut`.
    /// Returns the largest relative change of `nut`.
    pub fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
        let g = self.geom.clone();
        let n = g.n_cells();
        let (nu, cf) = (self.nu, self.coef);
        let cw1 = cf.cw1();
        let vol = g.mesh.cell_volumes().to_vec();
        let gr = velocity_gradient(&g, u);
        let mut vort: Vec<f64> = (0..n)
            .map(|c| {
                let mut w2 = 0.0;
                for i in 0..3 {
                    for j in 0..3 {
                        let wij = 0.5 * (gr[i][c][j] - gr[j][c][i]);
                        w2 += wij * wij;
                    }
                }
                (2.0 * w2).sqrt()
            })
            .collect();
        let nt = self.nutilde.values.clone();
        let grad_mag: Vec<f64> = if self.des == DesMode::Ddes {
            (0..n)
                .map(|c| {
                    let mut s2 = 0.0;
                    for gi in gr.iter() {
                        s2 += vec3::dot(gi[c], gi[c]);
                    }
                    s2.sqrt()
                })
                .collect()
        } else {
            Vec::new()
        };
        let len = self.length_scale(&grad_mag);

        // ---- wall cells (wall mode Auto): Spalding law, strain and fixed nuT
        let mut fixed: Vec<Option<f64>> = vec![None; n];
        let wall_cells: Vec<usize> = (0..n).filter(|&c| self.cell_wall_face[c] != usize::MAX).collect();
        if self.wall_mode == SaWallMode::Auto {
            for &c in &wall_cells {
                let f = self.cell_wall_face[c];
                let y = g.dn[f - g.n_internal];
                let uu = self.tangential_speed(u, f);
                let ut = self.law.u_tau(nu, uu, y);
                if ut <= 0.0 {
                    continue;
                }
                let dy_du = self.law.dy_du(uu / ut);
                if ut * y / nu > self.fix_y_plus {
                    vort[c] = ut * ut / nu / dy_du;
                    fixed[c] = Some(nutilde_from_nut(nu * (dy_du - 1.0).max(0.0), nu, cf.cv1));
                }
            }
        }
        if self.wall_mode == SaWallMode::SaLaw {
            if self.inner.is_none() {
                self.inner = Some(SaInnerLaw::new(cf.kappa, cf.cv1));
            }
            if self.law_faces.is_none() {
                self.law_faces = Some(self.build_law_faces());
            }
            let il = self.inner.as_ref().unwrap();
            for &c in &wall_cells {
                let f = self.cell_wall_face[c];
                let y = g.dn[f - g.n_internal];
                let ut = il.u_tau(nu, self.tangential_speed(u, f), y);
                if ut > 0.0 && ut * y / nu > self.fix_y_plus {
                    // the inner-layer solution nuT = kappa u_tau y and its strain u_tau^2/(nu + nut)
                    vort[c] = ut * ut / nu / (1.0 + il.nut_plus(ut * y / nu));
                    fixed[c] = Some(cf.kappa * ut * y);
                }
            }
        }

        // ---- gradient of nuT, diffusivity
        let nb = self.nutilde.boundary_values(&g);
        let gnt = green_gauss(&g, &nt, &nb, 0);
        let fnv = |x: f64| -> f64 {
            if x < 0.0 && self.negative_nutilde {
                let chi3 = (x / nu).powi(3);
                (cf.cn1 + chi3) / (cf.cn1 - chi3)
            } else {
                1.0
            }
        };
        let eff: Vec<f64> = nt
            .iter()
            .map(|&x| if self.negative_nutilde { fnv(x) * x } else { x.max(0.0) })
            .collect();
        let mut gam = effective_viscosity(&g, nu, &eff, &[]);
        for (i, &b) in nb.iter().enumerate() {
            let bb = if self.negative_nutilde { b } else { b.max(0.0) };
            gam[g.n_internal + i] = nu + fnv(b) * bb;
        }
        gam.iter_mut().for_each(|x| *x /= cf.sigma);

        // ---- matrix
        let cells = self.nutilde.values.clone();
        let bc = self.nutilde.boundary_coeffs(&g);
        let grad = if self.convection.needs_gradient() {
            Some(least_squares(&g, &cells, &nb))
        } else {
            None
        };
        let mut m = convection(&g, phi, &cells, &bc, self.convection, grad.as_deref());
        m.add(&laplacian(&g, &gam, &bc, grad.as_deref()));
        for c in 0..n {
            let d = len[c];
            let x = nt[c];
            let g2 = vec3::dot(gnt[c], gnt[c]);
            m.b[c] += cf.cb2 / cf.sigma * g2 * vol[c];
            if x >= 0.0 || !self.negative_nutilde {
                let x = x.max(0.0);
                let chi = x / nu;
                let sb = x * fv2(chi, cf.cv1) / (cf.kappa * cf.kappa * d * d);
                let st = s_tilde(vort[c], sb, self.limiter, cf.c2, cf.c3);
                let r = r_function(x, st, cf.kappa, d);
                let fwv = fw(r, cf.cw2, cf.cw3);
                m.b[c] += cf.cb1 * st.max(0.0) * x * vol[c];
                m.a.diag[c] += cw1 * fwv * x / (d * d) * vol[c];
            } else {
                m.b[c] += (cf.cb1 * vort[c] * x + cw1 * x * x / (d * d)) * vol[c];
            }
        }
        if let Some(dt) = self.step_dt {
            // implicit-Euler time derivative, no under-relaxation
            for c in 0..n {
                m.a.diag[c] += vol[c] / dt;
                m.b[c] += vol[c] * self.nutilde_old[c] / dt;
            }
        } else {
            m.relax(self.relax_turb, &nt);
        }
        for &c in &wall_cells {
            if let Some(v) = fixed[c] {
                let big = 1e8 * m.a.diag[c].abs().max(1e-30);
                m.a.diag[c] = big;
                m.b[c] = big * v;
            }
        }
        let mut new = nt.clone();
        m.solve(&mut new, &self.solver);
        if !self.negative_nutilde {
            new.iter_mut().for_each(|x| *x = x.max(0.0));
        }
        self.nutilde.values = new;

        // ---- eddy viscosity and wall values
        let cap = 1e5 * nu;
        let mut change: f64 = 0.0;
        let alpha = if self.started && self.step_dt.is_none() {
            self.relax_nut
        } else {
            1.0
        };
        for c in 0..n {
            let target = nut_from_nutilde(self.nutilde.values[c], nu, cf.cv1).min(cap);
            let upd = alpha * target + (1.0 - alpha) * self.nut[c];
            change = change.max((upd - self.nut[c]).abs() / (self.nut[c] + nu));
            self.nut[c] = upd;
        }
        self.started = true;
        let old = std::mem::take(&mut self.wall_nut);
        let law = self.law;
        let mode = self.wall_mode;
        let inner = self.inner.as_ref().filter(|_| mode == SaWallMode::SaLaw);
        let mut yp = vec![0.0; self.wall_faces.len()];
        self.wall_nut = self
            .wall_faces
            .iter()
            .enumerate()
            .map(|(i, &f)| {
                let y = g.dn[f - g.n_internal];
                let uu = self.tangential_speed(u, f);
                let ut = if uu > 0.0 {
                    match inner {
                        Some(il) => il.u_tau(nu, uu, y),
                        None => law.u_tau(nu, uu, y),
                    }
                } else {
                    0.0
                };
                yp[i] = ut * y / nu;
                let new = match mode {
                    SaWallMode::Resolved => 0.0,
                    SaWallMode::Auto | SaWallMode::SaLaw if uu > 0.0 => (ut * ut * y / uu - nu).max(0.0),
                    SaWallMode::Auto | SaWallMode::SaLaw => 0.0,
                };
                let v = match old.get(i) {
                    Some(&(_, o)) => alpha * new + (1.0 - alpha) * o,
                    None => new,
                };
                (f, v)
            })
            .collect();
        if let (Some(il), Some(lf)) = (inner, self.law_faces.as_ref()) {
            // internal faces: nut of the interpolated nuT (linear near the wall); wall-normal first faces of wall cells
            // above fix_y_plus: the series resistance of the inner law between the two cell centres
            let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
            let ntv = &self.nutilde.values;
            let mut target: Vec<f64> = (0..g.n_internal)
                .map(|f| {
                    let w = g.weights[f];
                    nut_from_nutilde(w * ntv[own[f]] + (1.0 - w) * ntv[nei[f]], nu, cf.cv1).min(cap)
                })
                .collect();
            for &(fw, fi, dy) in lf {
                let y1 = g.dn[fw - g.n_internal];
                let ut = il.u_tau(nu, self.tangential_speed(u, fw), y1);
                let (y1p, y2p) = (ut * y1 / nu, ut * (y1 + dy) / nu);
                if ut > 0.0 && y1p > self.fix_y_plus {
                    let du = il.u_plus(y2p) - il.u_plus(y1p);
                    if du > 0.0 {
                        target[fi] = (nu * (y2p - y1p) / du - nu).clamp(0.0, cap);
                    }
                }
            }
            if self.face_nut.len() == target.len() {
                for (o, t) in self.face_nut.iter_mut().zip(target) {
                    *o = alpha * t + (1.0 - alpha) * *o;
                }
            } else {
                self.face_nut = target;
            }
        }
        self.y_plus = yp;
        self.step_dt = None;
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_and_functions() {
        let c = SaCoefficients::default();
        assert!((c.cw1() - 3.2390678).abs() < 1e-6, "{}", c.cw1());
        assert!((fv1(c.cv1, c.cv1) - 0.5).abs() < 1e-14);
        assert!((fv2(0.0, c.cv1) - 1.0).abs() < 1e-15);
        assert!((fw(1.0, c.cw2, c.cw3) - 1.0).abs() < 1e-12);
        // r is capped at 10, and 10 when S~ vanishes
        assert_eq!(r_function(1.0, 0.0, 0.41, 1.0), 10.0);
        assert_eq!(r_function(1e9, 1.0, 0.41, 1.0), 10.0);
    }

    #[test]
    fn allmaras_limiter_is_continuous_and_positive() {
        let (c2, c3) = (0.7, 0.9);
        let om = 3.0;
        let at = |sb: f64| s_tilde(om, sb, SLimiter::Allmaras, c2, c3);
        let b = -c2 * om;
        assert!((at(b) - at(b - 1e-13)).abs() < 1e-12);
        assert!((at(b) - 0.3 * om).abs() < 1e-12);
        for e in [0.0, -0.5, -1.0, -10.0, -1e3, -1e6] {
            assert!(at(e * om) > 0.0, "S~ {} at {e}", at(e * om));
        }
        assert!((at(-1e12 * om) / om - 0.1).abs() < 1e-6);
        assert!(s_tilde(om, -100.0, SLimiter::Min03, c2, c3) >= 0.3 * om - 1e-15);
    }

    /// ADR 0168 T1: the inner law against an independent quadrature, its limits and its inversion.
    #[test]
    fn w49e_inner_law_quadrature_limits_and_inversion() {
        let c = SaCoefficients::default();
        let il = SaInnerLaw::new(c.kappa, c.cv1);
        // independent check: composite Simpson in y+ (not ln y+) with many points
        let direct = |yp: f64| {
            let n = 200_000;
            let h = yp / n as f64;
            let f = |y: f64| 1.0 / (1.0 + il.nut_plus(y));
            let mut s = f(0.0) + f(yp);
            for k in 1..n {
                s += if k % 2 == 1 { 4.0 } else { 2.0 } * f(k as f64 * h);
            }
            s * h / 3.0
        };
        for yp in [0.5, 1.0, 3.0, 5.0, 8.0, 12.0, 15.0, 30.0, 60.0, 200.0] {
            let (a, b) = (il.u_plus(yp), direct(yp));
            assert!((a - b).abs() < 1e-7 * b.max(1.0), "y+ {yp}: {a} vs {b}");
        }
        // viscous limit, continuity at both table ends, log-layer intercept of the SA model (about 5.0)
        assert!((il.u_plus(1e-4) - 1e-4).abs() < 1e-18);
        assert!((il.u_plus(1e-3 * (1.0 + 1e-12)) - 1e-3).abs() < 1e-12);
        assert!((il.u_plus(1e8 * (1.0 - 1e-12)) - il.u_plus(1e8 * (1.0 + 1e-12))).abs() < 1e-9);
        let b = il.u_plus(1e4) - (1e4f64).ln() / c.kappa;
        assert!((4.9..5.2).contains(&b), "B = {b}");
        // monotone, and u_tau inverts u = u_tau u+(y u_tau / nu)
        let mut prev = 0.0;
        for k in 0..2000 {
            let v = il.u_plus(1e-4 * 1.02f64.powi(k));
            assert!(v > prev);
            prev = v;
        }
        let nu = 2e-7;
        for &(ut, y) in &[(0.04, 1e-6), (0.04, 2e-5), (0.04, 1e-4), (0.04, 3e-4), (1.0, 1e-2)] {
            let u = ut * il.u_plus(ut * y / nu);
            let got = il.u_tau(nu, u, y);
            assert!((got / ut - 1.0).abs() < 1e-10, "{got} vs {ut}");
        }
        assert_eq!(il.u_tau(nu, 0.0, 1e-4), 0.0);
    }

    #[test]
    fn negative_branch_and_inversion() {
        assert!(nut_from_nutilde(-1e-6, 1e-5, 7.1) == 0.0);
        for ratio in [1e-3, 0.1, 1.0, 10.0, 100.0, 1e4] {
            let nu = 2e-7;
            let nt = nutilde_from_nut(ratio * nu, nu, 7.1);
            let back = nut_from_nutilde(nt, nu, 7.1);
            assert!((back / (ratio * nu) - 1.0).abs() < 1e-10, "ratio {ratio}: {back}");
        }
        // fn(chi = 0) = 1
        let c = SaCoefficients::default();
        assert!(((c.cn1 + 0.0) / (c.cn1 - 0.0) - 1.0).abs() < 1e-15);
    }
}
