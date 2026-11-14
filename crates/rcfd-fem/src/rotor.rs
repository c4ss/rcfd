//! Flexible rotor on bearings in the stationary frame: shaft, discs, bearings; whirl, critical speeds, stability, unbalance
//! response. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0476 (F-416, design stub ADR 0319).
//!
//! A [`RotorModel`] is a list of stations along the spin axis `z` with four unknowns each in stationary axes, `(u_x, u_y,
//! theta_x, theta_y)`, spinning at `Omega >= 0` about `+z` (ADR 0476 D1). Shaft elements are the F-391 Timoshenko element of one
//! bending plane in each of the planes `X = (u_x, theta_y)` and `Y = (u_y, -theta_x)` (`beam::plane_stiffness`,
//! `beam::plane_mass`), with the gyroscopic matrix `G = [[0, P], [-P, 0]]`, `P = int rho I_p N_psi^T N_psi` from the same
//! section-rotation functions (D2); rigid discs (mass, transverse and polar inertia); bearings with 2 x 2 stiffness and damping
//! matrices (cross-coupling allowed); pins and clamps. The equations are `M q_tt + (C + Omega G) q_t + K q = f`.
//!
//! Massless undamped unknowns are condensed statically (D3). [`RotorModel::whirl`] solves the quadratic problem by the solver
//! of ADR 0450 in the scaled time of ADR 0472 D3 (D4) and labels each mode forward or backward by the sense of its orbit, the
//! mass-weighted whirl number of D5 (`Whirl` of ADR 0411); degenerate roots are resolved into circular orbits. The speed is
//! flagged unstable by the rule of ADR 0472 D4 and no unstable mode is dropped. [`RotorModel::critical_speeds`] solves the
//! undamped synchronous problem `K z = Omega^2 (M - i G) z` by a Hermitian Jacobi method (D6, D7);
//! [`RotorModel::harmonic_response`] and [`RotorModel::unbalance_response`] the complex dynamic stiffness (D8);
//! [`RotorModel::campbell`] tracks the modes over the speeds by the complex-line tracker of ADR 0411 D4 into the split Campbell
//! table of ADR 0472 (D9).

use crate::beam::{plane_mass, plane_stiffness};
use crate::damped::REAL_TOLERANCE;
use crate::error::{FemError, Result};
use crate::gyro::{SplitCampbell, SplitSpeed};
use crate::mesh::BeamSection;
use crate::model::Material;
use crate::rotating::{check_speeds, track_whirl, Entry, Stability, Whirl, WhirlMode, WhirlModes, STABILITY_TOLERANCE};
pub use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen_general::{quadratic_eigen_with, GeneralEigenError, GeneralEigenOptions};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Roots (or critical-speed eigenvalues) within this relative distance form a degenerate cluster (ADR 0476 D5, D6).
pub const CLUSTER_TOLERANCE: f64 = 1e-8;

/// The off-diagonal Frobenius norm at which the Hermitian Jacobi method stops, relative to the whole (ADR 0476 D7).
const JACOBI_TOLERANCE: f64 = 1e-15;

/// Sweeps of the Hermitian Jacobi method before the solve is refused.
const JACOBI_SWEEPS: usize = 100;

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

fn check_spin(omega: f64) -> Result<()> {
    if omega.is_finite() && omega >= 0.0 {
        Ok(())
    } else {
        Err(invalid("speed", format!("{omega} rad/s is not a finite speed >= 0")))
    }
}

/// A rigid disc at a station (ADR 0476 D1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Disc {
    /// Mass, kg.
    pub mass: f64,
    /// Transverse moment of inertia about the station, kg m^2.
    pub transverse: f64,
    /// Polar moment of inertia, kg m^2.
    pub polar: f64,
}

/// A bearing at a station: the force on the rotor is `f = -k u - c u_t` on `(u_x, u_y)` (ADR 0476 D1). `k[0][1] != k[1][0]` is
/// a cross-coupled stiffness; `k = 0` is an external damper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bearing {
    /// Stiffness `[[kxx, kxy], [kyx, kyy]]`, N/m.
    pub k: [[f64; 2]; 2],
    /// Damping `[[cxx, cxy], [cyx, cyy]]`, N s/m.
    pub c: [[f64; 2]; 2],
}

impl Bearing {
    /// A bearing with the given matrices.
    pub fn new(k: [[f64; 2]; 2], c: [[f64; 2]; 2]) -> Bearing {
        Bearing { k, c }
    }
    /// An isotropic bearing `k I`, `c I`.
    pub fn isotropic(k: f64, c: f64) -> Bearing {
        Bearing {
            k: [[k, 0.0], [0.0, k]],
            c: [[c, 0.0], [0.0, c]],
        }
    }
}

/// A point unbalance: `amount = m e` (kg m) at the angle `angle` (rad) from the `x` axis at `t = 0`, at a station.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unbalance {
    /// The station.
    pub node: usize,
    /// `m e`, kg m, `>= 0`.
    pub amount: f64,
    /// Angle, rad.
    pub angle: f64,
}

/// One shaft element of a [`RotorModel`].
#[derive(Debug, Clone, PartialEq)]
pub struct ShaftElement {
    /// The two stations.
    pub nodes: [usize; 2],
    /// The (axisymmetric) section.
    pub section: BeamSection,
    /// The isotropic elastic material.
    pub material: Material,
}

/// A flexible rotor on bearings (ADR 0476 D1).
#[derive(Debug, Clone, PartialEq)]
pub struct RotorModel {
    z: Vec<f64>,
    shafts: Vec<ShaftElement>,
    discs: Vec<(usize, Disc)>,
    bearings: Vec<(usize, Bearing)>,
    fixed: Vec<bool>,
}

/// The dense matrices of a [`RotorModel`] on all `4 N` station unknowns (row-major), with the fixed unknowns marked.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RotorMatrices {
    /// `4 N`.
    pub n: usize,
    /// Mass.
    pub m: Vec<f64>,
    /// Stiffness (not symmetric with cross-coupled bearings).
    pub k: Vec<f64>,
    /// Bearing damping.
    pub c: Vec<f64>,
    /// The gyroscopic matrix per unit spin, skew bit for bit.
    pub g: Vec<f64>,
    /// Whether each unknown is fixed (pins, clamps).
    pub fixed: Vec<bool>,
}

/// One whirl mode `u = Re(x e^{lambda t})`, `lambda = growth + i omega` (ADR 0476 D4, D5).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RotorWhirlMode {
    /// `Im lambda`, rad/s, positive.
    pub omega: f64,
    /// `Re lambda`, 1/s (positive: growing).
    pub growth: f64,
    /// `-Re lambda / |lambda|`.
    pub damping_ratio: f64,
    /// The shape per station `[u_x, u_y, theta_x, theta_y]`, `x^H M x = 1`.
    pub shape: Vec<[C64; 4]>,
    /// The whirl number `kappa` (D5), `+1` a circular forward orbit, `-1` backward.
    pub whirl: f64,
    /// Its direction.
    pub direction: Whirl,
    /// The solver's relative residual of the root (on the scaled problem).
    pub residual: f64,
}

/// The whirl at one speed (ADR 0476 D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RotorWhirl {
    /// The spin, rad/s.
    pub speed: f64,
    /// The whirl modes by `omega` ascending (unstable ones included).
    pub modes: Vec<RotorWhirlMode>,
    /// All `2 n` roots `lambda`, 1/s.
    pub roots: Vec<C64>,
    /// The real parts of the non-oscillating roots.
    pub real_roots: Vec<f64>,
    /// The stability flag (ADR 0472 D4).
    pub stability: Stability,
    /// The number of unknowns after the condensation.
    pub unknowns: usize,
    /// The shapes on the free unknowns (internal, for the tracker).
    free_vectors: Vec<Vec<C64>>,
}

/// An undamped critical speed (ADR 0476 D6).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CriticalSpeed {
    /// The spin at which a whirl frequency equals the spin, rad/s.
    pub speed: f64,
    /// The whirl number of the synchronous mode.
    pub whirl: f64,
    /// Forward (reached by unbalance), backward or neutral (a straight-line orbit of anisotropic bearings).
    pub direction: Whirl,
    /// The shape per station.
    pub shape: Vec<[C64; 4]>,
}

/// The orbit of a station (ADR 0476 D8).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Orbit {
    /// Forward radius `|X + i Y|/2`, m.
    pub forward: f64,
    /// Backward radius `|conj X + i conj Y|/2`, m.
    pub backward: f64,
    /// Semi-major axis, m.
    pub major: f64,
    /// Semi-minor axis, m.
    pub minor: f64,
    /// The sense of the orbit.
    pub direction: Whirl,
}

/// A steady harmonic response (ADR 0476 D8).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RotorResponse {
    /// The spin, rad/s.
    pub speed: f64,
    /// The frequency, rad/s.
    pub omega: f64,
    /// Per station `[u_x, u_y, theta_x, theta_y]` phasors, `u(t) = Re(U e^{i omega t})`.
    pub disp: Vec<[C64; 4]>,
    /// Per station the orbit of `(u_x, u_y)`.
    pub orbits: Vec<Orbit>,
}

fn finite_nonneg(v: f64) -> bool {
    v.is_finite() && v >= 0.0
}

impl RotorModel {
    /// A model with stations at the axial positions `z` (finite, strictly increasing; key `"nodes"`).
    pub fn new(z: Vec<f64>) -> Result<RotorModel> {
        if z.is_empty() {
            return Err(invalid("nodes", "the rotor has no station".to_string()));
        }
        for (i, v) in z.iter().enumerate() {
            if !v.is_finite() {
                return Err(invalid("nodes", format!("station {i} at {v} is not finite")));
            }
            if i > 0 && *v <= z[i - 1] {
                return Err(invalid(
                    "nodes",
                    format!(
                        "station {i} at {v} m does not follow station {} at {} m",
                        i - 1,
                        z[i - 1]
                    ),
                ));
            }
        }
        let n = z.len();
        Ok(RotorModel {
            z,
            shafts: Vec::new(),
            discs: Vec::new(),
            bearings: Vec::new(),
            fixed: vec![false; 4 * n],
        })
    }

    fn check_node(&self, node: usize) -> Result<()> {
        if node < self.z.len() {
            Ok(())
        } else {
            Err(invalid(
                "node",
                format!("station {node} does not exist ({} stations)", self.z.len()),
            ))
        }
    }

    /// The axial positions of the stations, m.
    pub fn stations(&self) -> &[f64] {
        &self.z
    }
    /// The shaft elements.
    pub fn shafts(&self) -> &[ShaftElement] {
        &self.shafts
    }
    /// The discs with their stations.
    pub fn discs(&self) -> &[(usize, Disc)] {
        &self.discs
    }
    /// The bearings with their stations.
    pub fn bearings(&self) -> &[(usize, Bearing)] {
        &self.bearings
    }

    /// A shaft element between stations `a` and `b` (ADR 0476 D1, D2): the section must be axisymmetric (`I_y == I_z`,
    /// `shear_y == shear_z`; key `"section"`), the material isotropic elastic (key `"material"`).
    pub fn add_shaft(&mut self, a: usize, b: usize, section: BeamSection, material: &Material) -> Result<()> {
        self.check_node(a)?;
        self.check_node(b)?;
        if a == b {
            return Err(invalid("node", format!("a shaft element joins station {a} to itself")));
        }
        if section.i_y() != section.i_z() || section.shear_y() != section.shear_z() {
            return Err(invalid(
                "section",
                format!(
                    "the section is not axisymmetric (I_y {:e}, I_z {:e}, shear factors {}, {}): a non-axisymmetric shaft has \
                     time-varying matrices in the stationary frame",
                    section.i_y(),
                    section.i_z(),
                    section.shear_y(),
                    section.shear_z()
                ),
            ));
        }
        if !matches!(material, Material::IsotropicElastic { .. }) {
            return Err(invalid(
                "material",
                format!("the shaft takes an isotropic elastic material, not {}", material.name()),
            ));
        }
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        self.shafts.push(ShaftElement {
            nodes: [lo, hi],
            section,
            material: material.clone(),
        });
        Ok(())
    }

    /// A rigid disc at a station (each value finite and `>= 0`; key `"disc"`).
    pub fn add_disc(&mut self, node: usize, disc: Disc) -> Result<()> {
        self.check_node(node)?;
        if !(finite_nonneg(disc.mass) && finite_nonneg(disc.transverse) && finite_nonneg(disc.polar)) {
            return Err(invalid(
                "disc",
                format!("the disc at station {node} needs a finite mass and inertias >= 0: {disc:?}"),
            ));
        }
        self.discs.push((node, disc));
        Ok(())
    }

    /// A bearing at a station (finite entries; key `"bearing"`).
    pub fn add_bearing(&mut self, node: usize, bearing: Bearing) -> Result<()> {
        self.check_node(node)?;
        if bearing
            .k
            .iter()
            .chain(bearing.c.iter())
            .flatten()
            .any(|v| !v.is_finite())
        {
            return Err(invalid(
                "bearing",
                format!("the bearing at station {node} has a non-finite entry: {bearing:?}"),
            ));
        }
        self.bearings.push((node, bearing));
        Ok(())
    }

    /// A pin: `u_x = u_y = 0` at the station.
    pub fn pin(&mut self, node: usize) -> Result<()> {
        self.check_node(node)?;
        self.fixed[4 * node] = true;
        self.fixed[4 * node + 1] = true;
        Ok(())
    }

    /// A clamp: all four unknowns of the station zero.
    pub fn clamp(&mut self, node: usize) -> Result<()> {
        self.check_node(node)?;
        for c in 0..4 {
            self.fixed[4 * node + c] = true;
        }
        Ok(())
    }

    /// The dense matrices on all station unknowns (ADR 0476 D1, D2).
    pub fn matrices(&self) -> RotorMatrices {
        let n = 4 * self.z.len();
        let mut m = vec![0.0; n * n];
        let mut k = vec![0.0; n * n];
        let mut c = vec![0.0; n * n];
        let mut g = vec![0.0; n * n];
        for s in &self.shafts {
            let [a, b] = s.nodes;
            let l = self.z[b] - self.z[a];
            let e = s.material.youngs_modulus();
            let gm = e / (2.0 * (1.0 + s.material.poisson_ratio()));
            let rho = s.material.density();
            let sec = &s.section;
            let ei = e * sec.i_y();
            let ks = sec.shear_y() * gm * sec.area();
            let kp = plane_stiffness(ei, ks, l);
            let mp = plane_mass(rho * sec.area(), rho * sec.i_y(), ei, ks, l);
            let pp = plane_mass(0.0, rho * (sec.i_y() + sec.i_z()), ei, ks, l);
            let px = [4 * a, 4 * a + 3, 4 * b, 4 * b + 3];
            let py = [4 * a + 1, 4 * a + 2, 4 * b + 1, 4 * b + 2];
            let sy = [1.0, -1.0, 1.0, -1.0];
            for i in 0..4 {
                for j in 0..4 {
                    k[px[i] * n + px[j]] += kp[i][j];
                    m[px[i] * n + px[j]] += mp[i][j];
                    k[py[i] * n + py[j]] += sy[i] * sy[j] * kp[i][j];
                    m[py[i] * n + py[j]] += sy[i] * sy[j] * mp[i][j];
                    let v = pp[i][j] * sy[j];
                    g[px[i] * n + py[j]] += v;
                    g[py[j] * n + px[i]] -= v;
                }
            }
        }
        for (node, d) in &self.discs {
            let o = 4 * node;
            m[o * n + o] += d.mass;
            m[(o + 1) * n + o + 1] += d.mass;
            m[(o + 2) * n + o + 2] += d.transverse;
            m[(o + 3) * n + o + 3] += d.transverse;
            g[(o + 2) * n + o + 3] += d.polar;
            g[(o + 3) * n + o + 2] -= d.polar;
        }
        for (node, br) in &self.bearings {
            let o = 4 * node;
            for a in 0..2 {
                for b in 0..2 {
                    k[(o + a) * n + o + b] += br.k[a][b];
                    c[(o + a) * n + o + b] += br.c[a][b];
                }
            }
        }
        RotorMatrices {
            n,
            m,
            k,
            c,
            g,
            fixed: self.fixed.clone(),
        }
    }

    fn free(&self) -> Vec<usize> {
        (0..self.fixed.len()).filter(|&i| !self.fixed[i]).collect()
    }

    /// The whirl modes at the spin `omega`, rad/s (ADR 0476 D3 to D5). Refuses a negative or non-finite speed (`"speed"`), a
    /// singular block of massless unknowns (`"stiffness"`) and a mass that is not positive definite after the condensation
    /// (`"mass"`).
    pub fn whirl(&self, omega: f64) -> Result<RotorWhirl> {
        check_spin(omega)?;
        let mats = self.matrices();
        let free = self.free();
        let sense = sense_matrix(&mats);
        self.whirl_with(omega, &mats, &free, &sense)
    }

    fn whirl_with(&self, omega: f64, mats: &RotorMatrices, free: &[usize], sense: &[f64]) -> Result<RotorWhirl> {
        let nf = free.len();
        if nf == 0 {
            return Err(invalid("stiffness", "every unknown is fixed".to_string()));
        }
        let nt = mats.n;
        let sub = |a: &[f64]| -> Vec<f64> {
            let mut o = vec![0.0; nf * nf];
            for (i, &p) in free.iter().enumerate() {
                for (j, &q) in free.iter().enumerate() {
                    o[i * nf + j] = a[p * nt + q];
                }
            }
            o
        };
        let (mf, kf, cf, gf) = (sub(&mats.m), sub(&mats.k), sub(&mats.c), sub(&mats.g));
        let zero = |i: usize| -> bool {
            (0..nf).all(|j| {
                mf[i * nf + j] == 0.0
                    && mf[j * nf + i] == 0.0
                    && cf[i * nf + j] == 0.0
                    && cf[j * nf + i] == 0.0
                    && gf[i * nf + j] == 0.0
                    && gf[j * nf + i] == 0.0
            })
        };
        let cond = Condensation::new(&kf, nf, &(0..nf).map(zero).collect::<Vec<_>>())?;
        let na = cond.kept.len();
        let pick = |a: &[f64]| -> Vec<f64> {
            let mut o = vec![0.0; na * na];
            for (i, &p) in cond.kept.iter().enumerate() {
                for (j, &q) in cond.kept.iter().enumerate() {
                    o[i * na + j] = a[p * nf + q];
                }
            }
            o
        };
        let ma = pick(&mf);
        let da: Vec<f64> = pick(&cf).iter().zip(pick(&gf)).map(|(c, g)| c + omega * g).collect();
        let ka = &cond.k;
        let mut kmax: f64 = 0.0;
        let mut dmax: f64 = 0.0;
        for i in 0..na {
            let d = ma[i * na + i];
            if d.is_nan() || d <= 0.0 {
                return Err(invalid(
                    "mass",
                    format!(
                        "unknown {} has no mass after the condensation of the massless undamped unknowns (a massless unknown \
                         with a damper or a gyroscopic term is not taken)",
                        free[cond.kept[i]]
                    ),
                ));
            }
            kmax = kmax.max(ka[i * na + i].abs() / d);
            dmax = dmax.max((0..na).map(|j| da[i * na + j].abs()).sum::<f64>() / d);
        }
        let mut tau = kmax.sqrt() + dmax;
        if !(tau > 0.0 && tau.is_finite()) {
            tau = 1.0;
        }
        let ds: Vec<f64> = da.iter().map(|v| v / tau).collect();
        let ks: Vec<f64> = ka.iter().map(|v| v / (tau * tau)).collect();
        let q =
            quadratic_eigen_with(&ma, &ds, &ks, None, na, &GeneralEigenOptions::default()).map_err(|e| match e {
                GeneralEigenError::MassNotPositiveDefinite { .. } => invalid("mass", e.to_string()),
                other => FemError::Solver {
                    solver: "quadratic_eigen",
                    reason: other.to_string(),
                },
            })?;
        let roots: Vec<C64> = q.values.iter().map(|mu| mu.scale(tau)).collect();
        let mut picked: Vec<(C64, Vec<C64>, f64)> = Vec::new();
        let mut real_roots = Vec::new();
        for (idx, lam) in roots.iter().enumerate() {
            if lam.im.abs() <= REAL_TOLERANCE * lam.abs() {
                real_roots.push(lam.re);
                continue;
            }
            if lam.im <= 0.0 {
                continue;
            }
            let xf = cond.expand(&q.vectors[idx]);
            picked.push((*lam, to_full(&xf, free, nt), q.residuals[idx]));
        }
        picked.sort_by(|a, b| a.0.im.total_cmp(&b.0.im).then(a.0.re.total_cmp(&b.0.re)));
        // D5: resolve degenerate clusters into circular orbits.
        let mut vecs: Vec<Vec<C64>> = picked.iter().map(|p| p.1.clone()).collect();
        let lams: Vec<C64> = picked.iter().map(|p| p.0).collect();
        let mut i = 0;
        while i < lams.len() {
            let mut j = i + 1;
            while j < lams.len() && (lams[j] - lams[i]).abs() <= CLUSTER_TOLERANCE * lams[i].abs() {
                j += 1;
            }
            if j - i > 1 {
                resolve_cluster(&mut vecs[i..j], &mats.m, sense, nt);
            }
            i = j;
        }
        let mut modes = Vec::with_capacity(picked.len());
        let mut free_vectors = Vec::with_capacity(picked.len());
        for ((lam, _, res), x) in picked.iter().zip(vecs) {
            let kappa = whirl_number(&x, &mats.m, sense, nt);
            free_vectors.push(free.iter().map(|&p| x[p]).collect());
            modes.push(RotorWhirlMode {
                omega: lam.im,
                growth: lam.re,
                damping_ratio: -lam.re / lam.abs(),
                shape: stations(&x),
                whirl: kappa,
                direction: Whirl::of(kappa),
                residual: *res,
            });
        }
        let max_re = roots.iter().map(|l| l.re).fold(f64::NEG_INFINITY, f64::max);
        let max_abs = roots.iter().map(|l| l.abs()).fold(0.0, f64::max);
        let stability = if max_abs > 0.0 && max_re > STABILITY_TOLERANCE * max_abs {
            Stability::Unstable { growth: max_re }
        } else {
            Stability::Stable
        };
        Ok(RotorWhirl {
            speed: omega,
            modes,
            roots,
            real_roots,
            stability,
            unknowns: na,
            free_vectors,
        })
    }

    /// The undamped critical speeds, ascending, each with its direction (ADR 0476 D6). Refuses a stiffness that is not
    /// symmetric (`"cross_coupling"`) or not positive definite (`"stiffness"`).
    pub fn critical_speeds(&self) -> Result<Vec<CriticalSpeed>> {
        let mats = self.matrices();
        let free = self.free();
        let nf = free.len();
        if nf == 0 {
            return Err(invalid("stiffness", "every unknown is fixed".to_string()));
        }
        let nt = mats.n;
        let mut mf = vec![0.0; nf * nf];
        let mut kf = vec![0.0; nf * nf];
        let mut gf = vec![0.0; nf * nf];
        for (i, &p) in free.iter().enumerate() {
            for (j, &q) in free.iter().enumerate() {
                mf[i * nf + j] = mats.m[p * nt + q];
                kf[i * nf + j] = mats.k[p * nt + q];
                gf[i * nf + j] = mats.g[p * nt + q];
            }
        }
        for i in 0..nf {
            for j in 0..i {
                if kf[i * nf + j].to_bits() != kf[j * nf + i].to_bits() {
                    return Err(invalid(
                        "cross_coupling",
                        format!(
                            "the stiffness is not symmetric at unknowns {}, {} ({:e} against {:e}): a cross-coupled rotor has \
                             no real critical-speed problem; use whirl and its stability",
                            free[i],
                            free[j],
                            kf[i * nf + j],
                            kf[j * nf + i]
                        ),
                    ));
                }
            }
        }
        let zero = |i: usize| -> bool {
            (0..nf).all(|j| {
                mf[i * nf + j] == 0.0 && mf[j * nf + i] == 0.0 && gf[i * nf + j] == 0.0 && gf[j * nf + i] == 0.0
            })
        };
        let cond = Condensation::new(&kf, nf, &(0..nf).map(zero).collect::<Vec<_>>())?;
        let na = cond.kept.len();
        let mut ka = cond.k.clone();
        for i in 0..na {
            for j in 0..i {
                let s = 0.5 * (ka[i * na + j] + ka[j * na + i]);
                ka[i * na + j] = s;
                ka[j * na + i] = s;
            }
        }
        let l = cholesky_real(&ka, na).ok_or_else(|| {
            invalid(
                "stiffness",
                "the stiffness is not positive definite (a free rotor or a mechanism): no critical speed".to_string(),
            )
        })?;
        let pick = |a: &[f64]| -> Vec<f64> {
            let mut o = vec![0.0; na * na];
            for (i, &p) in cond.kept.iter().enumerate() {
                for (j, &q) in cond.kept.iter().enumerate() {
                    o[i * na + j] = a[p * nf + q];
                }
            }
            o
        };
        let am = congruence(&l, &pick(&mf), na);
        let ag = congruence(&l, &pick(&gf), na);
        // A = L^-1 (M - i G) L^-T, Hermitian.
        let mut a = vec![C64::ZERO; na * na];
        for i in 0..na {
            for j in 0..na {
                let re = 0.5 * (am[i * na + j] + am[j * na + i]);
                let im = -0.5 * (ag[i * na + j] - ag[j * na + i]);
                a[i * na + j] = C64::new(re, im);
            }
        }
        let (mu, y) = hermitian_jacobi(&a, na)?;
        let sense = sense_matrix(&mats);
        let mut found: Vec<(f64, Vec<C64>)> = Vec::new();
        for (k, &m) in mu.iter().enumerate() {
            if m.is_nan() || m <= 0.0 {
                continue;
            }
            // z = L^-T y
            let mut z: Vec<C64> = (0..na).map(|i| y[i * na + k]).collect();
            for i in (0..na).rev() {
                let mut s = z[i];
                for p in i + 1..na {
                    s -= z[p].scale(l[p * na + i]);
                }
                z[i] = s.scale(1.0 / l[i * na + i]);
            }
            found.push((m, to_full(&cond.expand(&z), &free, nt)));
        }
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut vecs: Vec<Vec<C64>> = found.iter().map(|f| f.1.clone()).collect();
        let mut i = 0;
        while i < found.len() {
            let mut j = i + 1;
            while j < found.len() && (found[j].0 - found[i].0).abs() <= CLUSTER_TOLERANCE * found[i].0 {
                j += 1;
            }
            if j - i > 1 {
                resolve_cluster(&mut vecs[i..j], &mats.m, &sense, nt);
            }
            i = j;
        }
        let mut out: Vec<CriticalSpeed> = found
            .iter()
            .zip(vecs)
            .map(|((m, _), x)| {
                let kappa = whirl_number(&x, &mats.m, &sense, nt);
                CriticalSpeed {
                    speed: 1.0 / m.sqrt(),
                    whirl: kappa,
                    direction: Whirl::of(kappa),
                    shape: stations(&x),
                }
            })
            .collect();
        out.sort_by(|a, b| a.speed.total_cmp(&b.speed).then(b.whirl.total_cmp(&a.whirl)));
        Ok(out)
    }

    /// The steady response at the spin `spin` to station loads `[F_x, F_y, M_x, M_y]` at the frequency `omega > 0` (ADR 0476
    /// D8). A load on a fixed unknown is taken by the support. Refuses a negative speed (`"speed"`), a frequency that is not
    /// positive and finite (`"frequency"`), a non-finite load (`"load"`) and a singular dynamic stiffness (`"singular"`).
    pub fn harmonic_response(&self, spin: f64, omega: f64, loads: &[(usize, [C64; 4])]) -> Result<RotorResponse> {
        check_spin(spin)?;
        if !(omega.is_finite() && omega > 0.0) {
            return Err(invalid(
                "frequency",
                format!("{omega} rad/s is not a positive finite frequency"),
            ));
        }
        let mats = self.matrices();
        let free = self.free();
        let nf = free.len();
        let nt = mats.n;
        let mut pos = vec![usize::MAX; nt];
        for (i, &p) in free.iter().enumerate() {
            pos[p] = i;
        }
        let mut f = vec![C64::ZERO; nf];
        for (node, l) in loads {
            self.check_node(*node)?;
            for (c, v) in l.iter().enumerate() {
                if !v.is_finite() {
                    return Err(invalid("load", format!("the load at station {node} is not finite")));
                }
                let p = pos[4 * node + c];
                if p != usize::MAX {
                    f[p] += *v;
                }
            }
        }
        let mut d = vec![C64::ZERO; nf * nf];
        for (i, &p) in free.iter().enumerate() {
            for (j, &q) in free.iter().enumerate() {
                let e = p * nt + q;
                d[i * nf + j] = C64::new(
                    mats.k[e] - omega * omega * mats.m[e],
                    omega * (mats.c[e] + spin * mats.g[e]),
                );
            }
        }
        let x = complex_solve(d, nf, f).ok_or_else(|| {
            invalid(
                "singular",
                format!("the dynamic stiffness is singular at {omega} rad/s (an undamped resonance)"),
            )
        })?;
        let full = to_full(&x, &free, nt);
        let disp = stations(&full);
        let orbits = disp.iter().map(|u| orbit(u[0], u[1])).collect();
        Ok(RotorResponse {
            speed: spin,
            omega,
            disp,
            orbits,
        })
    }

    /// The steady response to point unbalances at the spin `spin > 0` (ADR 0476 D8): the force `F_x = amount spin^2 e^{i
    /// angle}`, `F_y = -i F_x` rotating with the rotor. Refuses a negative or non-finite amount or a non-finite angle
    /// (`"unbalance"`).
    pub fn unbalance_response(&self, spin: f64, unbalances: &[Unbalance]) -> Result<RotorResponse> {
        let mut loads = Vec::with_capacity(unbalances.len());
        for u in unbalances {
            if !(finite_nonneg(u.amount) && u.angle.is_finite()) {
                return Err(invalid(
                    "unbalance",
                    format!("the unbalance {u:?} needs a finite amount >= 0 and a finite angle"),
                ));
            }
            let fx = C64::exp_i(u.angle).scale(u.amount * spin * spin);
            loads.push((u.node, [fx, C64::new(fx.im, -fx.re), C64::ZERO, C64::ZERO]));
        }
        self.harmonic_response(spin, spin, &loads)
    }

    /// The Campbell table over a list of speeds (finite, `>= 0`, strictly increasing; key `"speeds"`) (ADR 0476 D9): the whirl
    /// modes of every speed tracked by the complex-line tracker of ADR 0411 D4, the growth per track and speed, the stability per
    /// speed.
    pub fn campbell(&self, speeds: &[f64]) -> Result<SplitCampbell> {
        check_speeds(speeds)?;
        let mats = self.matrices();
        let free = self.free();
        let sense = sense_matrix(&mats);
        let nt = mats.n;
        let nf = free.len();
        let mut trip = Vec::new();
        for (i, &p) in free.iter().enumerate() {
            for (j, &q) in free.iter().enumerate() {
                let v = mats.m[p * nt + q];
                if v != 0.0 {
                    trip.push((i, j, v));
                }
            }
        }
        let mass = CsrMatrix::from_triplets(nf, trip);
        let ns = speeds.len();
        let mut results = Vec::with_capacity(ns);
        let mut entries = Vec::with_capacity(ns);
        for &w in speeds {
            let r = self.whirl_with(w, &mats, &free, &sense)?;
            let modes: Vec<WhirlMode> = r
                .modes
                .iter()
                .zip(&r.free_vectors)
                .map(|(m, x)| WhirlMode {
                    omega: m.omega,
                    eigenvalue: m.omega * m.omega,
                    frequency: m.omega / (2.0 * PI),
                    re: x.iter().map(|v| v.re).collect(),
                    im: x.iter().map(|v| v.im).collect(),
                    whirl: m.whirl,
                    direction: m.direction,
                    wave: None,
                    residual: m.residual,
                })
                .collect();
            entries.push(Entry::Modes(WhirlModes {
                frequencies_all: modes.iter().map(|m| m.omega).collect(),
                modes,
                unknowns: r.unknowns,
            }));
            results.push(r);
        }
        let campbell = track_whirl(speeds, &entries, &mass);
        let growth = campbell
            .table
            .tracks
            .iter()
            .map(|tr| {
                (0..ns)
                    .map(|j| {
                        let line = tr.modes[j].as_ref()?;
                        results[j]
                            .modes
                            .iter()
                            .zip(&results[j].free_vectors)
                            .find(|(_, x)| {
                                x.iter().zip(&line[..nf]).all(|(v, l)| v.re.to_bits() == l.to_bits())
                                    && x.iter().zip(&line[nf..]).all(|(v, l)| v.im.to_bits() == l.to_bits())
                            })
                            .map(|(m, _)| m.growth)
                    })
                    .collect()
            })
            .collect();
        let per = results
            .iter()
            .map(|r| SplitSpeed {
                speed: r.speed,
                stability: Some(r.stability),
                basis: None,
                loss: None,
            })
            .collect();
        Ok(SplitCampbell {
            coriolis: true,
            campbell,
            growth,
            speeds: per,
        })
    }
}

/// The orbit of the phasors `(X, Y)` (ADR 0476 D8).
pub fn orbit(x: C64, y: C64) -> Orbit {
    let i = C64::I;
    let rf = 0.5 * (x + i * y).abs();
    let rb = 0.5 * (x.conj() + i * y.conj()).abs();
    let den = rf * rf + rb * rb;
    let s = if den > 0.0 { (rf * rf - rb * rb) / den } else { 0.0 };
    Orbit {
        forward: rf,
        backward: rb,
        major: rf + rb,
        minor: (rf - rb).abs(),
        direction: Whirl::of(s),
    }
}

fn stations(x: &[C64]) -> Vec<[C64; 4]> {
    x.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()
}

fn to_full(x: &[C64], free: &[usize], nt: usize) -> Vec<C64> {
    let mut o = vec![C64::ZERO; nt];
    for (v, &p) in x.iter().zip(free) {
        o[p] = *v;
    }
    o
}

/// The skew sense matrix `S` of ADR 0476 D5 on all station unknowns (dense, row-major): `S[(i, cx), (j, dy)] = -t P_m`,
/// `S[(i, cy), (j, dx)] = s P_m`, `P_m = (M_X + M_Y)/2` between the plane-`X` unknowns, `(cx, cy, s)` in `{(u_x, u_y, 1),
/// (theta_y, theta_x, -1)}`.
fn sense_matrix(mats: &RotorMatrices) -> Vec<f64> {
    let n = mats.n;
    let ns = n / 4;
    let pairs = [(0usize, 1usize, 1.0), (3, 2, -1.0)];
    let mut s = vec![0.0; n * n];
    for i in 0..ns {
        for j in 0..ns {
            for &(cx, cy, si) in &pairs {
                for &(dx, dy, tj) in &pairs {
                    let mx = mats.m[(4 * i + cx) * n + 4 * j + dx];
                    let my = si * tj * mats.m[(4 * i + cy) * n + 4 * j + dy];
                    let pm = 0.5 * (mx + my);
                    s[(4 * i + cx) * n + 4 * j + dy] += -tj * pm;
                    s[(4 * i + cy) * n + 4 * j + dx] += si * pm;
                }
            }
        }
    }
    s
}

/// `x^H A y` for a real dense `A`.
fn form(x: &[C64], a: &[f64], y: &[C64], n: usize) -> C64 {
    let mut s = C64::ZERO;
    for i in 0..n {
        if x[i] == C64::ZERO {
            continue;
        }
        let mut r = C64::ZERO;
        for j in 0..n {
            let v = a[i * n + j];
            if v != 0.0 {
                r += y[j].scale(v);
            }
        }
        s += x[i].conj() * r;
    }
    s
}

/// The whirl number `2 Re(x)^T S Im(x) / x^H M x` (ADR 0476 D5).
fn whirl_number(x: &[C64], m: &[f64], s: &[f64], n: usize) -> f64 {
    let den = form(x, m, x, n).re;
    // x^H (-i S) x = 2 Re^T S Im for a real skew S.
    let num = (C64::new(0.0, -1.0) * form(x, s, x, n)).re;
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

/// Resolves a degenerate cluster: the eigenvectors of the Hermitian pencil `(Z^H (-i S) Z, Z^H M Z)` (ADR 0476 D5).
fn resolve_cluster(z: &mut [Vec<C64>], m: &[f64], s: &[f64], n: usize) {
    let p = z.len();
    let mut hm = vec![C64::ZERO; p * p];
    let mut hs = vec![C64::ZERO; p * p];
    let mi = C64::new(0.0, -1.0);
    for a in 0..p {
        for b in 0..p {
            hm[a * p + b] = form(&z[a], m, &z[b], n);
            hs[a * p + b] = mi * form(&z[a], s, &z[b], n);
        }
    }
    // Hermitian parts (rounding).
    for a in 0..p {
        for b in 0..=a {
            let v = (hm[a * p + b] + hm[b * p + a].conj()).scale(0.5);
            hm[a * p + b] = v;
            hm[b * p + a] = v.conj();
            let w = (hs[a * p + b] + hs[b * p + a].conj()).scale(0.5);
            hs[a * p + b] = w;
            hs[b * p + a] = w.conj();
        }
    }
    let Some(l) = cholesky_complex(&hm, p) else {
        return;
    };
    // B = L^-1 Hs L^-H
    let linv = lower_inverse(&l, p);
    let mut t = vec![C64::ZERO; p * p];
    for a in 0..p {
        for b in 0..p {
            let mut v = C64::ZERO;
            for c in 0..p {
                v += linv[a * p + c] * hs[c * p + b];
            }
            t[a * p + b] = v;
        }
    }
    let mut bm = vec![C64::ZERO; p * p];
    for a in 0..p {
        for b in 0..p {
            let mut v = C64::ZERO;
            for c in 0..p {
                v += t[a * p + c] * linv[b * p + c].conj();
            }
            bm[a * p + b] = v;
        }
    }
    let Ok((_, w)) = hermitian_jacobi(&bm, p) else {
        return;
    };
    // Z' = Z L^-H W
    let mut coef = vec![C64::ZERO; p * p];
    for a in 0..p {
        for k in 0..p {
            let mut v = C64::ZERO;
            for c in 0..p {
                v += linv[c * p + a].conj() * w[c * p + k];
            }
            coef[a * p + k] = v;
        }
    }
    let old: Vec<Vec<C64>> = z.to_vec();
    for (k, zk) in z.iter_mut().enumerate() {
        for (e, v) in zk.iter_mut().enumerate() {
            let mut s = C64::ZERO;
            for a in 0..p {
                s += old[a][e] * coef[a * p + k];
            }
            *v = s;
        }
    }
}

/// The static condensation of the flagged unknowns (ADR 0476 D3).
struct Condensation {
    n: usize,
    kept: Vec<usize>,
    dropped: Vec<usize>,
    /// The condensed stiffness on `kept`.
    k: Vec<f64>,
    /// `R = -K_bb^-1 K_ba`, `|dropped| x |kept|` row-major.
    r: Vec<f64>,
}

impl Condensation {
    fn new(k: &[f64], n: usize, drop: &[bool]) -> Result<Condensation> {
        let kept: Vec<usize> = (0..n).filter(|&i| !drop[i]).collect();
        let dropped: Vec<usize> = (0..n).filter(|&i| drop[i]).collect();
        let (na, nb) = (kept.len(), dropped.len());
        let mut kaa = vec![0.0; na * na];
        for (i, &p) in kept.iter().enumerate() {
            for (j, &q) in kept.iter().enumerate() {
                kaa[i * na + j] = k[p * n + q];
            }
        }
        if nb == 0 {
            return Ok(Condensation {
                n,
                kept,
                dropped,
                k: kaa,
                r: Vec::new(),
            });
        }
        let mut kbb = vec![0.0; nb * nb];
        for (i, &p) in dropped.iter().enumerate() {
            for (j, &q) in dropped.iter().enumerate() {
                kbb[i * nb + j] = k[p * n + q];
            }
        }
        // X = K_bb^-1 K_ba (nb x na), column by column.
        let lu = RealLu::new(kbb, nb).ok_or_else(|| {
            invalid(
                "stiffness",
                "the massless unknowns form a singular block (an unsupported massless part)".to_string(),
            )
        })?;
        let mut r = vec![0.0; nb * na];
        for (j, &q) in kept.iter().enumerate() {
            let col: Vec<f64> = dropped.iter().map(|&p| k[p * n + q]).collect();
            let x = lu.solve(&col);
            for i in 0..nb {
                r[i * na + j] = -x[i];
            }
        }
        // K* = K_aa + K_ab R
        for (i, &p) in kept.iter().enumerate() {
            for j in 0..na {
                let mut s = 0.0;
                for (b, &q) in dropped.iter().enumerate() {
                    s += k[p * n + q] * r[b * na + j];
                }
                kaa[i * na + j] += s;
            }
        }
        Ok(Condensation {
            n,
            kept,
            dropped,
            k: kaa,
            r,
        })
    }

    /// The vector on all `n` unknowns from its kept part.
    fn expand(&self, xa: &[C64]) -> Vec<C64> {
        let na = self.kept.len();
        let mut x = vec![C64::ZERO; self.n];
        for (i, &p) in self.kept.iter().enumerate() {
            x[p] = xa[i];
        }
        for (b, &p) in self.dropped.iter().enumerate() {
            let mut s = C64::ZERO;
            for j in 0..na {
                s += xa[j].scale(self.r[b * na + j]);
            }
            x[p] = s;
        }
        x
    }
}

/// Dense real LU with partial pivoting.
struct RealLu {
    n: usize,
    a: Vec<f64>,
    piv: Vec<usize>,
}

impl RealLu {
    fn new(mut a: Vec<f64>, n: usize) -> Option<RealLu> {
        let scale = a.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        let mut piv = (0..n).collect::<Vec<_>>();
        for k in 0..n {
            let p = (k..n).max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))?;
            let pv = a[p * n + k].abs();
            if pv.is_nan() || pv <= f64::EPSILON * scale * n as f64 {
                return None;
            }
            if p != k {
                for c in 0..n {
                    a.swap(k * n + c, p * n + c);
                }
                piv.swap(k, p);
            }
            let d = a[k * n + k];
            for i in k + 1..n {
                let f = a[i * n + k] / d;
                a[i * n + k] = f;
                if f != 0.0 {
                    for c in k + 1..n {
                        a[i * n + c] -= f * a[k * n + c];
                    }
                }
            }
        }
        Some(RealLu { n, a, piv })
    }

    fn solve(&self, b: &[f64]) -> Vec<f64> {
        let n = self.n;
        let mut x: Vec<f64> = self.piv.iter().map(|&p| b[p]).collect();
        for i in 0..n {
            for c in 0..i {
                x[i] -= self.a[i * n + c] * x[c];
            }
        }
        for i in (0..n).rev() {
            for c in i + 1..n {
                x[i] -= self.a[i * n + c] * x[c];
            }
            x[i] /= self.a[i * n + i];
        }
        x
    }
}

/// Dense complex solve by LU with partial pivoting (by modulus); `None` at a zero pivot.
fn complex_solve(mut a: Vec<C64>, n: usize, mut b: Vec<C64>) -> Option<Vec<C64>> {
    for k in 0..n {
        let p = (k..n).max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))?;
        let piv = a[p * n + k].abs();
        if piv.is_nan() || piv <= 0.0 {
            return None;
        }
        if p != k {
            for c in 0..n {
                a.swap(k * n + c, p * n + c);
            }
            b.swap(k, p);
        }
        let d = a[k * n + k];
        for i in k + 1..n {
            let f = a[i * n + k] / d;
            if f != C64::ZERO {
                for c in k + 1..n {
                    let v = a[k * n + c];
                    a[i * n + c] -= f * v;
                }
                let v = b[k];
                b[i] -= f * v;
            }
        }
    }
    for i in (0..n).rev() {
        let mut s = b[i];
        for c in i + 1..n {
            s -= a[i * n + c] * b[c];
        }
        b[i] = s / a[i * n + i];
    }
    if b.iter().all(|v| v.is_finite()) {
        Some(b)
    } else {
        None
    }
}

/// Real Cholesky `A = L L^T` (L row-major lower); `None` if not positive definite.
fn cholesky_real(a: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let mut s = a[j * n + j];
        for p in 0..j {
            s -= l[j * n + p] * l[j * n + p];
        }
        // A pivot below 1e-12 of its diagonal is a (numerically) singular stiffness: a free rotor or a mechanism.
        if s.is_nan() || s <= 1e-12 * a[j * n + j].abs() {
            return None;
        }
        let d = s.sqrt();
        l[j * n + j] = d;
        for i in j + 1..n {
            let mut v = a[i * n + j];
            for p in 0..j {
                v -= l[i * n + p] * l[j * n + p];
            }
            l[i * n + j] = v / d;
        }
    }
    Some(l)
}

/// `L^-1 X L^-T` for a real `X`.
fn congruence(l: &[f64], x: &[f64], n: usize) -> Vec<f64> {
    let fwd = |b: &[f64]| -> Vec<f64> {
        let mut w = b.to_vec();
        for col in 0..n {
            for i in 0..n {
                let mut s = w[i * n + col];
                for p in 0..i {
                    s -= l[i * n + p] * w[p * n + col];
                }
                w[i * n + col] = s / l[i * n + i];
            }
        }
        w
    };
    let w = fwd(x);
    let mut wt = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            wt[j * n + i] = w[i * n + j];
        }
    }
    let u = fwd(&wt);
    let mut out = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            out[j * n + i] = u[i * n + j];
        }
    }
    out
}

/// Complex Cholesky `A = L L^H` of a Hermitian positive definite `A`; `None` otherwise.
fn cholesky_complex(a: &[C64], n: usize) -> Option<Vec<C64>> {
    let mut l = vec![C64::ZERO; n * n];
    let scale = (0..n).map(|i| a[i * n + i].re.abs()).fold(0.0, f64::max);
    for j in 0..n {
        let mut s = a[j * n + j].re;
        for p in 0..j {
            s -= l[j * n + p].norm_sqr();
        }
        if s.is_nan() || s <= 1e-12 * scale {
            return None;
        }
        let d = s.sqrt();
        l[j * n + j] = C64::from_real(d);
        for i in j + 1..n {
            let mut v = a[i * n + j];
            for p in 0..j {
                v -= l[i * n + p] * l[j * n + p].conj();
            }
            l[i * n + j] = v.scale(1.0 / d);
        }
    }
    Some(l)
}

/// The inverse of a lower triangular complex matrix.
fn lower_inverse(l: &[C64], n: usize) -> Vec<C64> {
    let mut inv = vec![C64::ZERO; n * n];
    for col in 0..n {
        for i in col..n {
            let mut s = if i == col { C64::ONE } else { C64::ZERO };
            for p in col..i {
                s -= l[i * n + p] * inv[p * n + col];
            }
            inv[i * n + col] = s / l[i * n + i];
        }
    }
    inv
}

/// The cyclic Jacobi method for a Hermitian matrix (ADR 0476 D7): the eigenvalues (unsorted) and the unitary matrix of the
/// eigenvectors (columns, row-major).
fn hermitian_jacobi(a: &[C64], n: usize) -> Result<(Vec<f64>, Vec<C64>)> {
    let mut a = a.to_vec();
    let mut v = vec![C64::ZERO; n * n];
    for i in 0..n {
        v[i * n + i] = C64::ONE;
    }
    let total: f64 = a.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();
    let off = |a: &[C64]| -> f64 {
        let mut s = 0.0;
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    s += a[i * n + j].norm_sqr();
                }
            }
        }
        s.sqrt()
    };
    let mut sweeps = 0;
    while total > 0.0 && off(&a) > JACOBI_TOLERANCE * total {
        if sweeps == JACOBI_SWEEPS {
            return Err(FemError::Solver {
                solver: "hermitian_jacobi",
                reason: format!("not converged after {JACOBI_SWEEPS} sweeps"),
            });
        }
        sweeps += 1;
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                let r = apq.abs();
                if r == 0.0 {
                    continue;
                }
                // D = diag(1, e^{-i phi}) makes the pivot real: e^{-i phi} = conj(apq)/r.
                let ph = apq.conj().scale(1.0 / r);
                let (app, aqq) = (a[p * n + p].re, a[q * n + q].re);
                let tau = (aqq - app) / (2.0 * r);
                let t = if tau >= 0.0 {
                    1.0 / (tau + (1.0 + tau * tau).sqrt())
                } else {
                    -1.0 / (-tau + (1.0 + tau * tau).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = t * c;
                // U = D R, R = [[c, s], [-s, c]]: U_pp = c, U_pq = s, U_qp = -s ph, U_qq = c ph.
                let (upp, upq, uqp, uqq) = (C64::from_real(c), C64::from_real(s), ph.scale(-s), ph.scale(c));
                // A <- A U (columns p, q)
                for i in 0..n {
                    let (aip, aiq) = (a[i * n + p], a[i * n + q]);
                    a[i * n + p] = aip * upp + aiq * uqp;
                    a[i * n + q] = aip * upq + aiq * uqq;
                    let (vip, viq) = (v[i * n + p], v[i * n + q]);
                    v[i * n + p] = vip * upp + viq * uqp;
                    v[i * n + q] = vip * upq + viq * uqq;
                }
                // A <- U^H A (rows p, q)
                for j in 0..n {
                    let (apj, aqj) = (a[p * n + j], a[q * n + j]);
                    a[p * n + j] = upp.conj() * apj + uqp.conj() * aqj;
                    a[q * n + j] = upq.conj() * apj + uqq.conj() * aqj;
                }
                a[p * n + q] = C64::ZERO;
                a[q * n + p] = C64::ZERO;
                a[p * n + p] = C64::from_real(a[p * n + p].re);
                a[q * n + q] = C64::from_real(a[q * n + q].re);
            }
        }
    }
    Ok(((0..n).map(|i| a[i * n + i].re).collect(), v))
}
