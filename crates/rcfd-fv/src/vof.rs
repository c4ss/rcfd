//! Volume-of-fluid free-surface flow of two incompressible immiscible fluids (ADR 0049).
//!
//! * [`AlphaTransport`] advances the phase fraction `alpha` with the conservative flux
//!   `alpha phi + alpha (1 - alpha) phi_r` (interface-compression flux, `phi_r = c |phi| n_f . S / |S|`) limited by a
//!   Zalesak (1979) flux-corrected-transport limiter between the first-order upwind flux and the central-plus-compression
//!   flux, sub-cycled under a Courant limit. The scheme is bounded in `[0, 1]` for a solenoidal flux and conserves the
//!   phase volume to round-off.
//! * [`VofSolver`] couples it with a transient variable-density PISO/PIMPLE kernel in the `p_rgh = p - rho g.x`
//!   formulation. Gravity and the continuum-surface-force surface tension (Brackbill, Kothe and Zemach 1992) are FACE
//!   forces that enter the pressure equation with the same face gradient and geometry coefficient as the pressure term,
//!   and the cell velocity is reconstructed from the very same face quantities, so a still stratified pool has a velocity
//!   of the order of the linear-solver tolerance.
//!
//! Defaults (ADR 0114, user decision 2026-10-24): [`VofSettings::new`] turns on the component-diagonal `HbyA`
//! (`component_diagonal`, ADR 0103: a symmetry plane is free slip), the transposed viscous stress at the viscosity jump
//! (`full_viscous_stress`, ADR 0103) and the height-function curvature (`height_function`, ADR 0106) with the CSF fallback
//! (`height_function_csf_fallback`, ADR 0108): the height function is used on a uniform axis-aligned Cartesian grid with
//! `sigma > 0`; on any other mesh and without surface tension the curvature is the CSF estimator of Brackbill et al. after
//! `n_smooth` smoothing passes, as before. Set the four fields to `false` for the solver before ADR 0114. With these defaults
//! the Hysing et al. (2009) case-1 bubble on 160 x 320 gives y_c(3) 1.0795 and V_c,max 0.2416 (ADR 0108).

use crate::bc::{FixedValue, Profile};
use crate::cavitation::Cavitation;
use crate::convection::{convection, face_flux, ConvScheme};
use crate::diffusion::laplacian;
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::{green_gauss, least_squares};
use crate::masstransfer::{HertzKnudsen, R_UNIVERSAL};
use crate::mixture::{MixtureSettings, SlipTransport};
use crate::pressure::{
    boundary_hby_a, boundary_values_pbc, corrected_flux, divergence, face_interpolate, pressure_bc,
    solve_pressure_parts_src, solve_pressure_parts_src_k, PressureBc, PressureControls,
};
use crate::waves::{liquid_fraction, ramp, relax, zone_chi, Blend, Wave};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_linalg::{LinearOptions, SolveControl};
use std::sync::Arc;

/// Initial shape of phase 1 (`alpha = 1` inside).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// Axis-aligned box `[min, max]`.
    Box {
        /// Lower corner.
        min: Vec3,
        /// Upper corner.
        max: Vec3,
    },
    /// Sphere (disk in 2-D) of the given radius.
    Sphere {
        /// Centre.
        centre: Vec3,
        /// Radius.
        radius: f64,
    },
    /// Half space `n . x <= offset`.
    HalfSpace {
        /// Normal.
        normal: Vec3,
        /// Offset.
        offset: f64,
    },
}

impl Shape {
    /// Is the point inside the shape?
    pub fn contains(&self, x: Vec3) -> bool {
        match *self {
            Shape::Box { min, max } => (0..3).all(|k| x[k] >= min[k] && x[k] <= max[k]),
            Shape::Sphere { centre, radius } => vec3::norm(vec3::sub(x, centre)) <= radius,
            Shape::HalfSpace { normal, offset } => vec3::dot(normal, x) <= offset,
        }
    }
}

/// Phase fraction of every cell of an axis-aligned mesh from sub-sampling the cell bounding box (`n_sub` cell-centred
/// sample points per active direction) with the indicator `inside`.
pub fn indicator_fraction(geom: &FvGeometry, n_sub: usize, inside: impl Fn(Vec3) -> bool) -> Vec<f64> {
    let mesh = &geom.mesh;
    let n_sub = n_sub.max(1);
    (0..geom.n_cells())
        .map(|c| {
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for &f in mesh.cell_faces(c) {
                for &pi in mesh.face_points(f) {
                    let p = mesh.points()[pi];
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
            }
            let ns: [usize; 3] = [0, 1, 2].map(|k| if geom.active[k] { n_sub } else { 1 });
            let mut count = 0usize;
            for i in 0..ns[0] {
                for j in 0..ns[1] {
                    for l in 0..ns[2] {
                        let idx = [i, j, l];
                        let x = [0, 1, 2].map(|k| lo[k] + (hi[k] - lo[k]) * (idx[k] as f64 + 0.5) / ns[k] as f64);
                        if inside(x) {
                            count += 1;
                        }
                    }
                }
            }
            count as f64 / (ns[0] * ns[1] * ns[2]) as f64
        })
        .collect()
}

/// [`indicator_fraction`] of a [`Shape`]; `invert` gives the complement (phase 1 outside the shape).
pub fn shape_fraction(geom: &FvGeometry, shape: Shape, invert: bool, n_sub: usize) -> Vec<f64> {
    indicator_fraction(geom, n_sub, |x| shape.contains(x) != invert)
}

/// Diagnostics of one phase-fraction advance.
#[derive(Debug, Clone)]
pub struct AlphaInfo {
    /// Number of sub-steps.
    pub n_sub: usize,
    /// Courant number of the whole step, `dt max_c (sum of outflow |phi|) / V`.
    pub courant: f64,
    /// Time-averaged total face flux of `alpha` (convective plus compression) over the step.
    pub alpha_phi: Vec<f64>,
}

/// Explicit conservative, bounded transport of the phase fraction (compression flux plus Zalesak limiter).
pub struct AlphaTransport {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Phase fraction (conditions on boundary patches: zero gradient by default).
    pub alpha: ScalarField,
    /// Compression coefficient `c_alpha` (0 = no compression flux).
    pub c_alpha: f64,
    /// Largest Courant number of a sub-step.
    pub courant_max: f64,
    /// Upper bound on the number of sub-steps (a larger need is an error).
    pub max_sub: usize,
    /// Algebraic-slip (drift-flux) transport of the mixture model (`None` = none, bit-identical to the plain path).
    pub slip: Option<SlipTransport>,
    /// Advective (non-conservative) form for a flux with a volume source, `alpha_t + div(alpha phi) - alpha div(phi)`: set by the
    /// cavitation coupling, whose lagged `div phi = S_v` would otherwise compress the liquid fraction above 1 when a cavity collapses
    /// within one step (ADR 0056 addendum). `false` = the conservative form, bit-identical to the plain path.
    pub advective: bool,
    dilation: Option<Vec<f64>>,
    delta: f64,
}

impl AlphaTransport {
    /// Create the transport of `alpha`.
    pub fn new(geom: Arc<FvGeometry>, alpha: ScalarField, c_alpha: f64, courant_max: f64) -> Self {
        let vmean = geom.mesh.total_volume() / geom.n_cells() as f64;
        AlphaTransport {
            geom,
            alpha,
            c_alpha,
            courant_max,
            max_sub: 2000,
            slip: None,
            advective: false,
            dilation: None,
            delta: 1e-8 / vmean.cbrt(),
        }
    }

    /// Courant number `dt max_c (sum over outflow faces of |phi|) / V` of the flux `phi`.
    pub fn courant(&self, phi: &[f64], dt: f64) -> f64 {
        let g = &self.geom;
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let mut out = vec![0.0; g.n_cells()];
        for f in 0..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            if f < g.n_internal {
                if phi[f] >= 0.0 {
                    out[own[f]] += phi[f];
                } else {
                    out[nei[f]] -= phi[f];
                }
            } else if phi[f] > 0.0 {
                out[own[f]] += phi[f];
            }
        }
        let vol = mesh.cell_volumes();
        dt * out.iter().zip(vol).map(|(o, v)| o / v).fold(0.0, f64::max)
    }

    /// Advance `alpha` by `dt` with the frozen face flux `phi`.
    pub fn advance(&mut self, phi: &[f64], dt: f64) -> Result<AlphaInfo> {
        let co = self.courant(phi, dt) + self.slip_courant(dt);
        let n_sub = ((co / self.courant_max).ceil() as usize).max(1);
        if n_sub > self.max_sub {
            return Err(Error::InvalidValue {
                path: "multiphase.alpha_courant".into(),
                reason: format!(
                    "the phase-fraction Courant number {co:.3} needs {n_sub} sub-steps (limit {}); reduce the time step",
                    self.max_sub
                ),
            });
        }
        let dts = dt / n_sub as f64;
        let mut acc = vec![0.0; self.geom.mesh.n_faces()];
        if self.advective {
            // advective form for a flux with a volume source (`div phi = S_v`): `alpha_t + div(alpha phi) - alpha div(phi)`
            let g = self.geom.clone();
            let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
            let mut div = vec![0.0; g.n_cells()];
            for f in 0..g.mesh.n_faces() {
                if g.empty[f] {
                    continue;
                }
                div[own[f]] += phi[f];
                if f < g.n_internal {
                    div[nei[f]] -= phi[f];
                }
            }
            self.dilation = Some(div);
        }
        for _ in 0..n_sub {
            self.substep(phi, dts, dts / dt, &mut acc);
        }
        self.dilation = None;
        Ok(AlphaInfo {
            n_sub,
            courant: co,
            alpha_phi: acc,
        })
    }

    /// Courant number of the slip signal speed (`dt max_c sum_f bound |gs| / V`); zero without slip.
    fn slip_courant(&self, dt: f64) -> f64 {
        let Some(sl) = &self.slip else { return 0.0 };
        let g = &self.geom;
        let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
        let mut out = vec![0.0; g.n_cells()];
        for f in 0..g.n_internal {
            if g.empty[f] {
                continue;
            }
            let w = sl.bound * sl.gs[f].abs();
            out[own[f]] += w;
            out[nei[f]] += w;
        }
        let vol = g.mesh.cell_volumes();
        dt * out.iter().zip(vol).map(|(o, v)| o / v).fold(0.0, f64::max)
    }

    fn substep(&mut self, phi: &[f64], dts: f64, weight: f64, acc: &mut [f64]) {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let n_int = g.n_internal;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let vol = mesh.cell_volumes();
        let a = self.alpha.values.clone();
        let bv = self.alpha.boundary_values(&g);
        let grad = green_gauss(&g, &a, &bv, 0);

        let mut f_low = vec![0.0; n_int];
        let mut anti = vec![0.0; n_int];
        let mut net = vec![0.0; n];
        for f in 0..n_int {
            let (o, nb) = (own[f], nei[f]);
            let w = g.weights[f];
            let up = if phi[f] >= 0.0 { a[o] } else { a[nb] };
            let lin = w * a[o] + (1.0 - w) * a[nb];
            let gf = [0, 1, 2].map(|k| w * grad[o][k] + (1.0 - w) * grad[nb][k]);
            let nd = vec3::dot(gf, sf[f]) / (vec3::norm(gf) + self.delta);
            let phi_r = self.c_alpha * phi[f].abs() / vec3::norm(sf[f]) * nd;
            let mut high = phi[f] * lin + phi_r * lin * (1.0 - lin);
            f_low[f] = phi[f] * up;
            if let Some(sl) = &self.slip {
                let gs = sl.gs[f];
                if gs != 0.0 && !g.empty[f] {
                    let low_s = sl.godunov(gs, a[o], a[nb]);
                    let high_s = sl.central(gs, lin);
                    if low_s != 0.0 || high_s != 0.0 {
                        f_low[f] += low_s;
                        high += high_s;
                    }
                }
            }
            anti[f] = high - f_low[f];
            net[o] += f_low[f];
            net[nb] -= f_low[f];
        }
        let mut f_bnd = vec![0.0; mesh.n_faces() - n_int];
        for f in n_int..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            f_bnd[f - n_int] = phi[f] * bv[f - n_int];
            net[own[f]] += f_bnd[f - n_int];
        }
        let dil = self.dilation.clone();
        let low: Vec<f64> = match &dil {
            None => (0..n).map(|c| a[c] - dts / vol[c] * net[c]).collect(),
            Some(d) => (0..n).map(|c| a[c] - dts / vol[c] * (net[c] - a[c] * d[c])).collect(),
        };

        // Zalesak limiter
        let mut p_plus = vec![0.0; n];
        let mut p_minus = vec![0.0; n];
        let mut a_max: Vec<f64> = (0..n).map(|c| a[c].max(low[c])).collect();
        let mut a_min: Vec<f64> = (0..n).map(|c| a[c].min(low[c])).collect();
        for f in 0..n_int {
            let (o, nb) = (own[f], nei[f]);
            let x = anti[f];
            p_plus[o] += (-x).max(0.0);
            p_minus[o] += x.max(0.0);
            p_plus[nb] += x.max(0.0);
            p_minus[nb] += (-x).max(0.0);
            let (hi_n, lo_n) = (a[nb].max(low[nb]), a[nb].min(low[nb]));
            let (hi_o, lo_o) = (a[o].max(low[o]), a[o].min(low[o]));
            a_max[o] = a_max[o].max(hi_n);
            a_min[o] = a_min[o].min(lo_n);
            a_max[nb] = a_max[nb].max(hi_o);
            a_min[nb] = a_min[nb].min(lo_o);
        }
        let mut r_plus = vec![1.0; n];
        let mut r_minus = vec![1.0; n];
        for c in 0..n {
            let q_plus = vol[c] / dts * (a_max[c].min(1.0) - low[c]);
            let q_minus = vol[c] / dts * (low[c] - a_min[c].max(0.0));
            if p_plus[c] > 1e-300 {
                r_plus[c] = (q_plus / p_plus[c]).clamp(0.0, 1.0);
            }
            if p_minus[c] > 1e-300 {
                r_minus[c] = (q_minus / p_minus[c]).clamp(0.0, 1.0);
            }
        }
        let mut net2 = vec![0.0; n];
        for f in 0..n_int {
            let (o, nb) = (own[f], nei[f]);
            let c = if anti[f] >= 0.0 {
                r_plus[nb].min(r_minus[o])
            } else {
                r_plus[o].min(r_minus[nb])
            };
            let flux = f_low[f] + c * anti[f];
            net2[o] += flux;
            net2[nb] -= flux;
            acc[f] += weight * flux;
        }
        for f in n_int..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            net2[own[f]] += f_bnd[f - n_int];
            acc[f] += weight * f_bnd[f - n_int];
        }
        match &dil {
            None => {
                for c in 0..n {
                    self.alpha.values[c] = a[c] - dts / vol[c] * net2[c];
                }
            }
            Some(d) => {
                for c in 0..n {
                    self.alpha.values[c] = a[c] - dts / vol[c] * (net2[c] - a[c] * d[c]);
                }
            }
        }
    }

    /// Volume of phase 1, `sum alpha V`.
    pub fn phase_volume(&self) -> f64 {
        self.alpha
            .values
            .iter()
            .zip(self.geom.mesh.cell_volumes())
            .map(|(a, v)| a * v)
            .sum()
    }
}

/// Interfacial mass-transfer model (ADR 0052 B; closures of [`crate::masstransfer`]).
#[derive(Debug, Clone, Copy)]
pub enum MassTransferModel {
    /// Temperature-driven Lee form: evaporation `c_evap alpha_l rho_l theta`, condensation `c_cond alpha_v rho_v theta`
    /// with `theta = (T - T_sat)/T_sat`.
    Lee {
        /// Evaporation frequency, 1/s.
        c_evap: f64,
        /// Condensation frequency, 1/s.
        c_cond: f64,
    },
    /// Hertz-Knudsen (Schrage) flux times the interfacial area density `|grad alpha|`; the saturation pressure is
    /// Clausius-Clapeyron with the constant latent heat, `p_sat(T) = p_sat_ref exp(h_fg M/R (1/T_sat - 1/T))`.
    HertzKnudsen {
        /// Accommodation coefficient in (0, 1].
        accommodation: f64,
        /// Molar mass, kg/mol.
        molar_mass: f64,
        /// Use the Schrage factor.
        schrage: bool,
        /// Vapour-side pressure, Pa.
        p_vapour: f64,
        /// Saturation pressure at `t_sat`, Pa.
        p_sat_ref: f64,
    },
}

/// Evaporation-condensation sources of the phase-fraction equation and the continuity equation (ADR 0052 B). The temperature
/// is a PRESCRIBED per-cell field: the multiphase path has no energy equation, so no latent-heat feedback exists.
#[derive(Debug, Clone)]
pub struct MassTransfer {
    /// Which phase is the liquid: 1 (`alpha = 1`) or 2.
    pub liquid_phase: usize,
    /// Closure.
    pub model: MassTransferModel,
    /// Saturation temperature, K.
    pub t_sat: f64,
    /// Latent heat, J/kg (diagnostic energy source only).
    pub h_fg: f64,
    /// Cell temperatures, K (one per cell).
    pub temperature: Vec<f64>,
}

/// Wave-inlet boundary (ADR 0052 C1): per-face velocity and phase fraction from the wave kinematics.
#[derive(Debug, Clone)]
pub struct WaveInlet {
    /// Boundary patch name (the patch is at the smallest `x`, propagation along +x).
    pub patch: String,
    /// The generated wave.
    pub wave: Wave,
    /// Start-up ramp duration, s.
    pub t_ramp: f64,
}

/// Target state of a relaxation zone.
#[derive(Debug, Clone, Copy)]
pub enum RelaxTarget {
    /// Still water: `alpha` from the still level, zero velocity.
    StillWater,
    /// The wave of the settings (with the given start-up ramp).
    Wave(Wave, f64),
}

/// Relaxation zone / numerical beach (ADR 0052 C2): blend of `alpha` and `u` toward the target with the weights of
/// [`crate::waves::Blend`] (weight 1 = computed solution at `x_inner`, 0 = target at `x_outer`).
#[derive(Debug, Clone, Copy)]
pub struct RelaxZone {
    /// Inner edge (computed solution), m.
    pub x_inner: f64,
    /// Outer edge (target), m.
    pub x_outer: f64,
    /// Weight shape.
    pub blend: Blend,
    /// Target state.
    pub target: RelaxTarget,
}

/// Wave generation and absorption (2-D, vertical coordinate `y`, phase 1 is the water).
#[derive(Debug, Clone)]
pub struct WaveSettings {
    /// Still-water level `y`, m.
    pub level: f64,
    /// Optional wave inlet.
    pub inlet: Option<WaveInlet>,
    /// Relaxation zones.
    pub zones: Vec<RelaxZone>,
}

/// Settings of [`VofSolver`].
#[derive(Debug, Clone)]
pub struct VofSettings {
    /// Density of phase 1 (`alpha = 1`) and phase 2.
    pub rho: [f64; 2],
    /// Dynamic viscosity of phase 1 and phase 2.
    pub mu: [f64; 2],
    /// Surface tension coefficient [N/m] (0 = none).
    pub sigma: f64,
    /// Gravity vector.
    pub gravity: Vec3,
    /// Origin of the gravity potential `g . (x - x_ref)`.
    pub x_ref: Vec3,
    /// Interface compression coefficient.
    pub c_alpha: f64,
    /// Largest Courant number of a phase-fraction sub-step.
    pub alpha_courant: f64,
    /// Smoothing passes of the phase fraction before the curvature.
    pub n_smooth: usize,
    /// Convection scheme of momentum.
    pub convection: ConvScheme,
    /// Outer iterations per step (1 = PISO).
    pub n_outer: usize,
    /// Pressure correctors per outer iteration.
    pub n_correctors: usize,
    /// Non-orthogonal pressure correctors.
    pub non_orth_correctors: usize,
    /// Velocity under-relaxation on non-final outer iterations.
    pub alpha_u: f64,
    /// Pressure under-relaxation on non-final outer iterations.
    pub alpha_p: f64,
    /// Transient Rhie-Chow correction.
    pub ddt_correction: bool,
    /// Reference cell and value of `p_rgh` when no boundary fixes the level.
    pub p_ref: (usize, f64),
    /// AMG-preconditioned CG for the pressure equation.
    pub amg_pressure: bool,
    /// Momentum linear solver.
    pub momentum_solver: SolveControl,
    /// Pressure linear solver.
    pub pressure_solver: SolveControl,
    /// Linear solver device.
    pub linear: LinearOptions,
    /// Interfacial mass transfer (`None` = none; the path is then bit-identical to the one without it).
    pub mass_transfer: Option<MassTransfer>,
    /// Mixture model with algebraic slip (drift flux, ADR 0056; `None` = none).
    pub mixture: Option<MixtureSettings>,
    /// Schnerr-Sauer cavitation (ADR 0056; `None` = none; exclusive with `mass_transfer`).
    pub cavitation: Option<Cavitation>,
    /// Wave inlet and relaxation zones (`None` = none).
    pub waves: Option<WaveSettings>,
    /// Add the transposed part `div(mu (grad u)^T)` of the viscous stress as an explicit source from the previous iterate
    /// (ADR 0103); it is non-zero at a viscosity jump. Default `true` since ADR 0114; `false` = Laplacian form only (the
    /// behaviour before ADR 0114).
    pub full_viscous_stress: bool,
    /// Cell mixture viscosity harmonic in alpha, `1/mu = alpha/mu_1 + (1 - alpha)/mu_2` (ADR 0103 diagnostic). Default `false`:
    /// linear in alpha.
    pub harmonic_viscosity: bool,
    /// Make `HbyA` consistent with the component-averaged diagonal, `HbyA_k = (H_k + (a_p - a_kk) u_k) / a_p` (ADR 0010 in the
    /// single-phase kernels, ADR 0103 here): with the averaged diagonal a symmetry plane acts partly like a no-slip wall. Default
    /// `true` since ADR 0114; `false` = averaged (the behaviour before ADR 0114).
    pub component_diagonal: bool,
    /// Height-function curvature on uniform Cartesian grids instead of the CSF estimator (ADR 0106); must be set before
    /// [`VofSolver::new`]. Default `true` since ADR 0114 (with `height_function_csf_fallback`); `false` = CSF everywhere (the
    /// behaviour before ADR 0114).
    pub height_function: bool,
    /// With `height_function`: keep the CSF estimator instead of refusing when the mesh is not a uniform Cartesian grid, and
    /// skip the lattice when `sigma <= 0` (ADR 0108); must be set before [`VofSolver::new`]. Default `true` since ADR 0114;
    /// `false` with `height_function` = strict mode, other meshes are refused.
    pub height_function_csf_fallback: bool,
}

impl VofSettings {
    /// Defaults for the given phase properties.
    pub fn new(rho: [f64; 2], mu: [f64; 2]) -> Self {
        VofSettings {
            rho,
            mu,
            sigma: 0.0,
            gravity: [0.0; 3],
            x_ref: [0.0; 3],
            c_alpha: 1.0,
            alpha_courant: 0.25,
            n_smooth: 2,
            convection: ConvScheme::Upwind,
            n_outer: 1,
            n_correctors: 3,
            non_orth_correctors: 0,
            alpha_u: 0.8,
            alpha_p: 0.5,
            ddt_correction: true,
            p_ref: (0, 0.0),
            amg_pressure: false,
            momentum_solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-8,
                max_iter: 500,
            },
            pressure_solver: SolveControl {
                tol_abs: 1e-16,
                tol_rel: 1e-12,
                max_iter: 5000,
            },
            linear: LinearOptions::default(),
            mass_transfer: None,
            mixture: None,
            cavitation: None,
            waves: None,
            full_viscous_stress: true,
            harmonic_viscosity: false,
            component_diagonal: true,
            height_function: true,
            height_function_csf_fallback: true,
        }
    }
}

/// Uniform axis-aligned lattice of the cells (height-function curvature, ADR 0106).
#[derive(Debug, Clone)]
struct HfGrid {
    n: [usize; 3],
    h: [f64; 3],
    /// Lattice index of every cell.
    ijk: Vec<[usize; 3]>,
    /// Cell at lattice index `i + n0 (j + n1 k)`.
    cell: Vec<usize>,
}

impl HfGrid {
    /// The lattice of `geom`, or `None` unless the cell centres form a complete grid of constant spacing per axis with equal
    /// cell volumes.
    fn new(geom: &FvGeometry) -> Option<Self> {
        let cc = geom.mesh.cell_centres();
        let vol = geom.mesh.cell_volumes();
        let nc = cc.len();
        if nc == 0 || vol.iter().any(|v| (v - vol[0]).abs() > 1e-9 * vol[0]) {
            return None;
        }
        let (mut n, mut h, mut lo) = ([1usize; 3], [1.0f64; 3], [0.0f64; 3]);
        for d in 0..3 {
            let mut x: Vec<f64> = cc.iter().map(|p| p[d]).collect();
            x.sort_by(f64::total_cmp);
            let tol = 1e-9 * x[0].abs().max(x[nc - 1].abs()).max(x[nc - 1] - x[0]).max(1e-300);
            x.dedup_by(|b, a| (*b - *a).abs() <= tol);
            n[d] = x.len();
            lo[d] = x[0];
            if n[d] > 1 {
                h[d] = (x[n[d] - 1] - x[0]) / (n[d] - 1) as f64;
                if x.windows(2).any(|w| (w[1] - w[0] - h[d]).abs() > 1e-6 * h[d]) {
                    return None;
                }
            }
        }
        if n[0] * n[1] * n[2] != nc {
            return None;
        }
        let mut cell = vec![usize::MAX; nc];
        let mut ijk = Vec::with_capacity(nc);
        for (c, p) in cc.iter().enumerate() {
            let q = [0, 1, 2].map(|d| {
                if n[d] > 1 {
                    ((p[d] - lo[d]) / h[d]).round() as usize
                } else {
                    0
                }
            });
            if (0..3).any(|d| n[d] > 1 && (p[d] - lo[d] - q[d] as f64 * h[d]).abs() > 1e-6 * h[d]) {
                return None;
            }
            let i = q[0] + n[0] * (q[1] + n[1] * q[2]);
            if i >= nc || cell[i] != usize::MAX {
                return None;
            }
            cell[i] = c;
            ijk.push(q);
        }
        Some(HfGrid { n, h, ijk, cell })
    }

    fn at(&self, q: [isize; 3]) -> Option<usize> {
        if (0..3).any(|d| q[d] < 0 || q[d] >= self.n[d] as isize) {
            return None;
        }
        let q = q.map(|x| x as usize);
        Some(self.cell[q[0] + self.n[0] * (q[1] + self.n[1] * q[2])])
    }

    /// Height-function curvature `-div(grad alpha / |grad alpha|)` of cell `c` from the first direction (in order of decreasing
    /// `|grad alpha|` component) and half-length K = 3, 4, 5 whose 3 (2-D) or 3 x 3 (3-D) columns all bracket the interface;
    /// `None` if none does.
    fn column_curvature(&self, a: &[f64], c: usize, grad: Vec3, axes: &[usize]) -> Option<f64> {
        const PURE: f64 = 1e-3;
        let mut dirs = axes.to_vec();
        dirs.sort_by(|&p, &q| grad[q].abs().total_cmp(&grad[p].abs()));
        for &d in &dirs {
            let tr: Vec<usize> = axes.iter().copied().filter(|&t| t != d).collect();
            if grad[d] == 0.0 || tr.is_empty() {
                continue;
            }
            // s < 0: phase 1 at the -d end of the column
            let s = grad[d].signum();
            'k: for k in 3..=5isize {
                let mut ht = [[0.0f64; 3]; 3];
                for (o0, row) in ht.iter_mut().enumerate() {
                    for (o1, hv) in row.iter_mut().enumerate() {
                        if tr.len() == 1 && o1 != 1 {
                            continue;
                        }
                        let mut q = self.ijk[c].map(|x| x as isize);
                        q[tr[0]] += o0 as isize - 1;
                        if tr.len() == 2 {
                            q[tr[1]] += o1 as isize - 1;
                        }
                        let mut sum = 0.0;
                        let (mut first, mut last) = (0.0, 0.0);
                        for m in -k..=k {
                            let mut p = q;
                            p[d] += m;
                            let Some(cell) = self.at(p) else { continue 'k };
                            if m == -k {
                                first = a[cell];
                            }
                            last = a[cell];
                            sum += a[cell];
                        }
                        let (one, zero) = if s < 0.0 { (first, last) } else { (last, first) };
                        if one < 1.0 - PURE || zero > PURE {
                            continue 'k;
                        }
                        *hv = self.h[d] * if s < 0.0 { sum } else { (2 * k + 1) as f64 - sum };
                    }
                }
                let hx = self.h[tr[0]];
                let h_x = (ht[2][1] - ht[0][1]) / (2.0 * hx);
                let h_xx = (ht[2][1] - 2.0 * ht[1][1] + ht[0][1]) / (hx * hx);
                if tr.len() == 1 {
                    return Some(s * h_xx / (1.0 + h_x * h_x).powf(1.5));
                }
                let hy = self.h[tr[1]];
                let h_y = (ht[1][2] - ht[1][0]) / (2.0 * hy);
                let h_yy = (ht[1][2] - 2.0 * ht[1][1] + ht[1][0]) / (hy * hy);
                let h_xy = (ht[2][2] - ht[2][0] - ht[0][2] + ht[0][0]) / (4.0 * hx * hy);
                let num = h_xx * (1.0 + h_y * h_y) + h_yy * (1.0 + h_x * h_x) - 2.0 * h_xy * h_x * h_y;
                return Some(s * num / (1.0 + h_x * h_x + h_y * h_y).powf(1.5));
            }
        }
        None
    }
}

/// Diagnostics of one VOF time step.
#[derive(Debug, Clone, Copy)]
pub struct VofStepInfo {
    /// Step number (1-based).
    pub step: usize,
    /// Time after the step.
    pub time: f64,
    /// Step size.
    pub dt: f64,
    /// Flow Courant number of the new flux.
    pub courant: f64,
    /// Phase-fraction Courant number (whole step).
    pub alpha_courant: f64,
    /// Phase-fraction sub-steps.
    pub n_sub: usize,
    /// Normalised continuity error of the new flux.
    pub continuity: f64,
    /// Initial momentum residual, first outer iteration.
    pub res_u: f64,
    /// Initial pressure residual of the first corrector of the last outer iteration.
    pub res_p: f64,
    /// Pressure linear iterations.
    pub p_iters: usize,
    /// `min alpha`.
    pub alpha_min: f64,
    /// `max alpha`.
    pub alpha_max: f64,
    /// True when `dt` exceeded the capillary time-step limit.
    pub capillary_exceeded: bool,
    /// Phase-1 volume leaving through the boundary, m^3/s (step average of the alpha flux).
    pub alpha_outflow: f64,
    /// Phase-1 volume added by the relaxation zones in this step, m^3.
    pub relax_volume: f64,
    /// Phase-1 volume added by phase change in this step, m^3.
    pub transfer_volume: f64,
    /// Delivered evaporation rate `sum mdot V`, kg/s (positive = liquid to vapour).
    pub transfer_rate: f64,
    /// Net volume source `sum S_v V` of the continuity equation, m^3/s.
    pub volume_source: f64,
    /// Latent-heat power `sum mdot h_fg V` that an energy equation would remove, W (diagnostic, never applied).
    pub latent_power: f64,
}

/// Phase-change quantities of one step.
#[derive(Debug, Clone)]
pub struct PhaseChange {
    /// Delivered evaporation rate of every cell, kg/(m^3 s).
    pub mdot: Vec<f64>,
    /// Net outflow each cell must produce, `S_v V`, m^3/s.
    pub vol_src: Vec<f64>,
    /// Phase-1 volume added, m^3.
    pub volume: f64,
    /// `sum mdot V`, kg/s.
    pub rate: f64,
    /// Latent heat, J/kg.
    pub h_fg: f64,
    /// Rate requested by the closure before the liquid inventory limit, `sum m V`, kg/s (Hertz-Knudsen; equal to
    /// `rate` for Lee, whose exponential update never over-draws).
    pub requested: f64,
}

/// Largest number of pressure re-linearisations per corrector of the implicit cavitation coupling.
const CAV_MAX_ITERS: usize = 60;
/// Convergence of the implicit coupling: the largest difference between the linearised and the exact source, as a vapour-fraction
/// change per step (the registered tolerance of the fraction bounds is 1e-9).
const CAV_TOL: f64 = 1e-9;
/// Half-width (Pa) of the pressure band around saturation in which the chord slope is regularised.
const CAV_D_REG: f64 = 1e-3;

/// Implicit (pressure-coupled) Schnerr-Sauer source of one step (ADR 0056 D). The exact source `S(p)` of a cell is the exact
/// radius update of [`Cavitation::advance`] at the pressure `p`, converted to the volume source `S_v V` of the host. It is
/// linearised by the chord to the saturation point, `S_lin(p) = S_k (p - p_sat)/(p_k - p_sat)` (slope `K <= 0`, `S(p_sat) = 0`
/// exactly), anchored at the latest pressure estimate `p_k`; the slope goes on the diagonal of the pressure equation. Re-anchoring
/// at the new pressure converges to the exact nonlinear source (fixed-point ratio 1/2 for the square-root law), and the sign of
/// the source is that of `p_sat - p` at the solved pressure by construction.
struct CavIter {
    cv: Cavitation,
    rl: f64,
    rv: f64,
    dt: f64,
    /// Vapour fraction after the transport, before the source.
    av: Vec<f64>,
    /// Mixture density at the start of the source.
    rho_m: Vec<f64>,
    /// `p - p_rgh` of every cell.
    off: Vec<f64>,
    vol: Vec<f64>,
    /// Source delivered by the last pressure solve (m^3/s), the linearised value clipped to the bounds.
    s_used: Vec<f64>,
    /// Largest `|S_lin - S| dt/V` of the last re-linearisation of the last solve.
    disc: f64,
    /// Total re-linearisations of the step.
    iters: usize,
}

impl CavIter {
    /// Exact source (m^3/s) of cell `c` at the physical pressure `p`.
    fn s_true(&self, c: usize, p: f64) -> f64 {
        let av = self.av[c];
        let g_dt = self.cv.advance(av, self.rl, p, self.dt) - av;
        (self.rl - self.rv) / self.rho_m[c] * g_dt / self.dt * self.vol[c]
    }

    /// Bounds of the source of cell `c`: vapour fraction not above 1, not below the nuclei (or the start value if lower).
    fn bounds(&self, c: usize) -> (f64, f64) {
        let av = self.av[c];
        let scale = (self.rl - self.rv) / self.rho_m[c] * self.vol[c] / self.dt;
        (scale * (self.cv.alpha_floor().min(av) - av), scale * (1.0 - av))
    }

    /// Chord linearisation at the `p_rgh` estimate `p_k`: returns `(S_0, slope, p_a)` per cell, `S_lin(p) = S_0 + slope (p - p_a)`.
    /// Normally `S_0 = S(p_k)`, `p_a = p_k` and the slope is the chord to the saturation point. Within `CAV_D_REG` of saturation, where
    /// the square-root law has an unbounded slope, the chord is taken from the source at `p_sat -+ CAV_D_REG` (the model passes through
    /// `(p_sat, 0)`), which keeps the slope finite and the iteration from alternating between an unconstrained and a pinned cell.
    fn linearise(&self, p_k: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let n = p_k.len();
        let mut s0 = vec![0.0; n];
        let mut k = vec![0.0; n];
        let mut pa = p_k.to_vec();
        for c in 0..n {
            let p = p_k[c] + self.off[c];
            let s = self.s_true(c, p);
            s0[c] = s;
            let d = p - self.cv.p_sat;
            // a source held at its bound (all vapour condensed to the nuclei, or no liquid left) is flat in the pressure: a chord
            // through the saturation point would extend it beyond the bound, so it is held constant (slope 0) until the pressure
            // leaves the range where the bound is active
            let (lo, hi) = self.bounds(c);
            let held = s <= lo + 1e-12 * lo.abs() || s >= hi - 1e-12 * hi.abs();
            if held {
                continue;
            }
            if d.abs() >= CAV_D_REG {
                if s != 0.0 {
                    k[c] = s / d;
                }
            } else {
                let d_eff = if d > 0.0 { CAV_D_REG } else { -CAV_D_REG };
                let s_a = self.s_true(c, self.cv.p_sat + d_eff);
                k[c] = s_a / d_eff;
                s0[c] = 0.0;
                pa[c] = self.cv.p_sat - self.off[c];
            }
        }
        (s0, k, pa)
    }

    /// Solve the pressure equation with the implicit source; `p_new` starts from the estimate `p_old` and ends at the converged
    /// pressure. Returns the initial normalised residual of the first solve and the linear iterations.
    #[allow(clippy::too_many_arguments)]
    fn solve(
        &mut self,
        g: &FvGeometry,
        gamma_p: &[f64],
        pbc: &PressureBc,
        phi_hby_a: &[f64],
        p_old: &[f64],
        p_new: &mut [f64],
        ctl: &PressureControls,
    ) -> (f64, usize) {
        let n = p_old.len();
        let mut est = p_old.to_vec();
        let (mut res0, mut lin_iters) = (0.0, 0);
        for it in 0..CAV_MAX_ITERS {
            let (sk, k, pa) = self.linearise(&est);
            // S_lin(p) = S_0 + K (p - p_a) = (S_0 - K p_a) + K p
            let src: Vec<f64> = (0..n).map(|c| sk[c] - k[c] * pa[c]).collect();
            p_new.copy_from_slice(&est);
            let (r0, li, _) =
                solve_pressure_parts_src_k(g, gamma_p, pbc, phi_hby_a, p_new, ctl, false, Some(&src), Some(&k));
            if it == 0 {
                res0 = r0;
            }
            lin_iters += li;
            self.iters += 1;
            let mut disc = 0.0f64;
            for c in 0..n {
                let (lo, hi) = self.bounds(c);
                let lin = (sk[c] + k[c] * (p_new[c] - pa[c])).clamp(lo, hi);
                self.s_used[c] = lin;
                let exact = self.s_true(c, p_new[c] + self.off[c]);
                disc = disc.max((lin - exact).abs() * self.dt / self.vol[c]);
            }
            self.disc = disc;
            est.copy_from_slice(p_new);
            if disc <= CAV_TOL {
                break;
            }
        }
        (res0, lin_iters)
    }
}

/// Transient VOF solver state.
pub struct VofSolver {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Settings.
    pub settings: VofSettings,
    /// Velocity.
    pub u: VectorField,
    /// Modified pressure `p - rho g.(x - x_ref)` [Pa].
    pub p_rgh: ScalarField,
    /// Pressure [Pa] `p_rgh + rho g.(x - x_ref)`.
    pub p: Vec<f64>,
    /// Phase fraction transport (field `alpha`).
    pub transport: AlphaTransport,
    /// Volumetric face flux.
    pub phi: Vec<f64>,
    /// Current time.
    pub time: f64,
    /// Diagnostics of every step.
    pub history: Vec<VofStepInfo>,
    step_no: usize,
    /// Delivered evaporation rate of every cell in the last step, kg/(m^3 s).
    pub mdot: Vec<f64>,
    /// Implicit cavitation coupling, last step: largest difference between the delivered and the exact Schnerr-Sauer source as a
    /// vapour-fraction change per step (0 without cavitation).
    pub cav_disc: f64,
    /// Implicit cavitation coupling, last step: total pressure re-linearisations (0 without cavitation).
    pub cav_iters: usize,
    /// Velocity before the last relaxation blend (reference of the Rhie-Chow defect of the next step).
    u_rc: Option<Vec<Vec3>>,
    /// Vertical extent `[y_lo, y_hi]` of every cell (wave zones).
    cell_y: Vec<[f64; 2]>,
    /// Inverse of `sum_f S S / |S|` per cell on the active dimensions (row-major).
    recon: Vec<[[f64; 3]; 3]>,
    /// Cell lattice of the height-function curvature (`None` unless `settings.height_function`).
    hf: Option<HfGrid>,
}

impl VofSolver {
    /// Create a solver at `t = 0`; the initial flux is interpolated from `u`.
    pub fn new(
        geom: Arc<FvGeometry>,
        u: VectorField,
        p_rgh: ScalarField,
        alpha: ScalarField,
        settings: VofSettings,
    ) -> Result<Self> {
        if geom.periodic_partner.iter().any(|p| p.is_some()) {
            return Err(Error::InvalidValue {
                path: "mesh".into(),
                reason: "the VOF solver does not support periodic patches".into(),
            });
        }
        let bu = u.boundary_values(&geom);
        let phi = face_flux(&geom, &u.values, &bu);
        let transport = AlphaTransport::new(geom.clone(), alpha, settings.c_alpha, settings.alpha_courant);
        let recon = reconstruction_tensors(&geom);
        let hf = if !settings.height_function || (settings.height_function_csf_fallback && settings.sigma <= 0.0) {
            None
        } else {
            match HfGrid::new(&geom) {
                Some(grid) => Some(grid),
                None if settings.height_function_csf_fallback => None, // `curvature()` uses CSF without a lattice
                None => {
                    return Err(Error::InvalidValue {
                        path: "settings.height_function".into(),
                        reason: "height-function curvature needs a uniform axis-aligned Cartesian grid".into(),
                    })
                }
            }
        };
        let mut s = VofSolver {
            geom,
            settings,
            u,
            p_rgh,
            p: Vec::new(),
            transport,
            phi,
            time: 0.0,
            history: Vec::new(),
            step_no: 0,
            mdot: Vec::new(),
            cav_disc: 0.0,
            cav_iters: 0,
            u_rc: None,
            cell_y: Vec::new(),
            recon,
            hf,
        };
        s.validate_extensions()?;
        s.update_pressure();
        Ok(s)
    }

    /// Settings checks of the mass-transfer and wave extensions; fills the cell extents used by the wave zones.
    fn validate_extensions(&mut self) -> Result<()> {
        let bad = |path: &str, reason: String| Error::InvalidValue {
            path: path.into(),
            reason,
        };
        let g = self.geom.clone();
        let n = g.n_cells();
        if let Some(mx) = &self.settings.mixture {
            if !(mx.dispersed_phase == 1 || mx.dispersed_phase == 2) {
                return Err(bad("multiphase.mixture.dispersed_phase", "must be 1 or 2".into()));
            }
            if !(mx.diameter.is_finite() && mx.diameter > 0.0) {
                return Err(bad(
                    "multiphase.mixture.particle_diameter",
                    "must be finite and > 0".into(),
                ));
            }
        }
        if let Some(cv) = &self.settings.cavitation {
            if self.settings.mass_transfer.is_some() {
                return Err(bad(
                    "multiphase.cavitation",
                    "cannot be combined with the thermal mass transfer".into(),
                ));
            }
            if !(cv.liquid_phase == 1 || cv.liquid_phase == 2) {
                return Err(bad("multiphase.cavitation.liquid_phase", "must be 1 or 2".into()));
            }
            for (path, v) in [
                ("multiphase.cavitation.saturation_pressure", cv.p_sat),
                ("multiphase.cavitation.bubble_number_density", cv.n0),
                ("multiphase.cavitation.nucleus_radius", cv.r_nucleus),
            ] {
                if !(v.is_finite() && v > 0.0) {
                    return Err(bad(path, "must be finite and > 0".into()));
                }
            }
            if !(cv.f_vap.is_finite() && cv.f_vap >= 0.0 && cv.f_cond.is_finite() && cv.f_cond >= 0.0) {
                return Err(bad(
                    "multiphase.cavitation",
                    "vaporisation_factor and condensation_factor must be finite and >= 0".into(),
                ));
            }
            if !pressure_bc(&g, &self.u, &self.p_rgh).has_dirichlet() {
                return Err(bad(
                    "multiphase.cavitation",
                    "a net volume source needs a pressure (fixed p_rgh) patch to leave through".into(),
                ));
            }
            // vapour nuclei exist everywhere in the liquid: the seed of inception
            let floor = cv.alpha_floor();
            for a in self.transport.alpha.values.iter_mut() {
                if cv.liquid_phase == 1 {
                    *a = a.min(1.0 - floor);
                } else {
                    *a = a.max(floor);
                }
            }
            self.transport.advective = true;
        }
        if let Some(mt) = &self.settings.mass_transfer {
            if !(mt.liquid_phase == 1 || mt.liquid_phase == 2) {
                return Err(bad("multiphase.mass_transfer.liquid_phase", "must be 1 or 2".into()));
            }
            if mt.temperature.len() != n || mt.temperature.iter().any(|t| !t.is_finite() || *t <= 0.0) {
                return Err(bad(
                    "multiphase.mass_transfer.temperature",
                    format!("needs {n} finite positive cell temperatures"),
                ));
            }
            if !(mt.t_sat.is_finite() && mt.t_sat > 0.0 && mt.h_fg.is_finite() && mt.h_fg > 0.0) {
                return Err(bad(
                    "multiphase.mass_transfer",
                    "t_sat and h_fg must be finite and > 0".into(),
                ));
            }
            match mt.model {
                MassTransferModel::Lee { c_evap, c_cond } => {
                    if !(c_evap.is_finite() && c_evap >= 0.0 && c_cond.is_finite() && c_cond >= 0.0) {
                        return Err(bad(
                            "multiphase.mass_transfer",
                            "c_evap and c_cond must be finite and >= 0".into(),
                        ));
                    }
                }
                MassTransferModel::HertzKnudsen {
                    accommodation,
                    molar_mass,
                    p_vapour,
                    p_sat_ref,
                    ..
                } => {
                    if !(accommodation > 0.0
                        && accommodation <= 1.0
                        && molar_mass > 0.0
                        && p_vapour >= 0.0
                        && p_sat_ref > 0.0)
                    {
                        return Err(bad(
                            "multiphase.mass_transfer",
                            "Hertz-Knudsen needs accommodation in (0, 1], molar_mass > 0, p_vapour >= 0, p_sat_ref > 0"
                                .into(),
                        ));
                    }
                }
            }
            if !pressure_bc(&g, &self.u, &self.p_rgh).has_dirichlet() {
                return Err(bad(
                    "multiphase.mass_transfer",
                    "a net volume source needs a pressure (fixed p_rgh) patch to leave through".into(),
                ));
            }
        }
        if let Some(w) = &self.settings.waves {
            if g.active[2] {
                return Err(bad(
                    "multiphase.waves",
                    "wave inlets and zones are 2-D (x, y) only".into(),
                ));
            }
            if let Some(inl) = &w.inlet {
                if g.mesh.patch(&inl.patch).is_none() {
                    return Err(bad("multiphase.waves.inlet", format!("no patch \"{}\"", inl.patch)));
                }
            }
            for z in &w.zones {
                if !(z.x_inner.is_finite() && z.x_outer.is_finite()) || z.x_inner == z.x_outer {
                    return Err(bad(
                        "multiphase.waves.zone",
                        "x_inner and x_outer must be finite and different".into(),
                    ));
                }
            }
            let mesh = &g.mesh;
            self.cell_y = (0..n)
                .map(|c| {
                    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                    for &f in mesh.cell_faces(c) {
                        for &pi in mesh.face_points(f) {
                            let y = mesh.points()[pi][1];
                            lo = lo.min(y);
                            hi = hi.max(y);
                        }
                    }
                    [lo, hi]
                })
                .collect();
        }
        if let Some(mx) = self.settings.mixture {
            let gv = self.settings.gravity;
            let g_mag = vec3::norm(gv);
            if g_mag > 0.0 {
                let ghat = vec3::scale(gv, 1.0 / g_mag);
                let sf = g.mesh.face_area_vectors();
                let gs: Vec<f64> = (0..g.n_internal).map(|f| vec3::dot(ghat, sf[f])).collect();
                let bound = mx.speed_bound(self.settings.rho, self.settings.mu, g_mag);
                self.transport.slip = Some(SlipTransport {
                    mixture: mx,
                    rho: self.settings.rho,
                    mu: self.settings.mu,
                    g_mag,
                    gs,
                    bound,
                });
            }
        }
        Ok(())
    }

    /// Cell force density `-div(j (x) J + T)` (N/m^3) of the drift mass flux and drift stress of the mixture model (ADR 0056 A),
    /// from the current phase fraction and flux; `None` when the mixture model is off or its slip is zero everywhere.
    pub fn drift_force(&self) -> Option<Vec<Vec3>> {
        let sl = self.transport.slip.as_ref()?;
        let g = &self.geom;
        let n = g.n_cells();
        let [r1, r2] = self.settings.rho;
        let ghat = vec3::scale(self.settings.gravity, 1.0 / sl.g_mag);
        let a = &self.transport.alpha.values;
        // cell drift mass flux direction coefficient jc = J . ghat and stress coefficient tc (T = tc ghat (x) ghat)
        let mut jc = vec![0.0; n];
        let mut tc = vec![0.0; n];
        let mut any = false;
        for c in 0..n {
            let al = a[c].clamp(0.0, 1.0);
            let v = sl.c(al);
            if v != 0.0 {
                any = true;
                jc[c] = al * (1.0 - al) * (r1 - r2) * v;
                tc[c] = al * (1.0 - al) * ((1.0 - al) * r1 + al * r2) * v * v;
            }
        }
        if !any {
            return None;
        }
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let vol = mesh.cell_volumes();
        let mut div = vec![[0.0; 3]; n];
        for f in 0..g.n_internal {
            if g.empty[f] {
                continue;
            }
            let (o, nb) = (own[f], nei[f]);
            let w = g.weights[f];
            let jf = w * jc[o] + (1.0 - w) * jc[nb];
            let tf = w * tc[o] + (1.0 - w) * tc[nb];
            // flux vector through the face: phi J_f + T_f . S = (phi jf + tf gs) ghat
            // (j (x) J: the face carries the flux of the vector J by phi; J = jc ghat)
            let fl = self.phi[f] * jf + tf * sl.gs[f];
            for k in 0..3 {
                div[o][k] += fl * ghat[k];
                div[nb][k] -= fl * ghat[k];
            }
        }
        Some((0..n).map(|c| [0, 1, 2].map(|k| -div[c][k] / vol[c])).collect())
    }

    /// Face force flux `lap_coeff (b_f . delta)` of a cell force density `b` (the analogue of `-grad p`) on internal faces.
    fn body_force_flux(&self, b: &[Vec3]) -> Vec<f64> {
        let g = &self.geom;
        let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
        (0..g.n_internal)
            .map(|f| {
                let w = g.weights[f];
                let bf = [0, 1, 2].map(|k| w * b[own[f]][k] + (1.0 - w) * b[nei[f]][k]);
                g.lap_coeff[f] * vec3::dot(bf, g.delta[f])
            })
            .collect()
    }

    /// Finish the implicit Schnerr-Sauer coupling after the pressure iterations (ADR 0056 D): the source the last pressure solve
    /// enforced, `S` (m^3/s), is converted to the delivered mass source `mdot = S/((1/rho_v - 1/rho_l) V)` and the vapour fraction
    /// grows by the material rate `D alpha_v/Dt = mdot rho_m/(rho_v rho_l)` (the Schnerr-Sauer identity).
    fn finish_cavitation(&mut self, ci: &CavIter, dt: f64) -> PhaseChange {
        let n = self.geom.n_cells();
        let vol = self.geom.mesh.cell_volumes();
        let mut mdot = vec![0.0; n];
        let (mut volume, mut rate) = (0.0, 0.0);
        let liquid_is_1 = ci.cv.liquid_phase == 1;
        for c in 0..n {
            let m_eff = ci.s_used[c] / ((1.0 / ci.rv - 1.0 / ci.rl) * vol[c]);
            mdot[c] = m_eff;
            let a = self.transport.alpha.values[c];
            let al = if liquid_is_1 { a } else { 1.0 - a };
            // material growth of the vapour fraction over the step (the transport is advective, the dilation is not applied twice):
            // `g dt = D alpha_v/Dt dt = mdot dt rho_m/(rho_v rho_l)`, inside the bounds of the source by construction
            let new_l = al - m_eff * dt * ci.rho_m[c] / (ci.rv * ci.rl);
            let a_new = if liquid_is_1 { new_l } else { 1.0 - new_l };
            self.transport.alpha.values[c] = a_new;
            volume += (a_new - a) * vol[c];
            rate += m_eff * vol[c];
        }
        PhaseChange {
            mdot,
            vol_src: ci.s_used.clone(),
            volume,
            rate,
            h_fg: 0.0,
            requested: rate,
        }
    }

    /// Replace the velocity and phase-fraction conditions of the wave-inlet patch by the wave values at time `t`.
    fn update_wave_inlet(&mut self, t: f64) {
        let Some(w) = self.settings.waves.clone() else { return };
        let Some(inl) = w.inlet else { return };
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let faces = mesh.patch(&inl.patch).expect("patch checked at construction").faces();
        let r = ramp(t, inl.t_ramp);
        let (mut vel, mut alp) = (Vec::new(), Vec::new());
        for f in faces {
            let xc = mesh.face_centres()[f];
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for &pi in mesh.face_points(f) {
                lo = lo.min(mesh.points()[pi][1]);
                hi = hi.max(mesh.points()[pi][1]);
            }
            let z = xc[1] - w.level;
            let eta = r * inl.wave.eta(xc[0], t);
            let a = liquid_fraction(z, hi - lo, eta);
            let (u, v) = inl.wave.velocity(xc[0], z.min(eta), t);
            vel.push([a * r * u, a * r * v, 0.0]);
            alp.push([a, 0.0, 0.0]);
        }
        self.u
            .set_bc(&g, &inl.patch, Box::new(FixedValue(Profile::PerFace(vel))))
            .expect("patch checked at construction");
        self.transport
            .alpha
            .set_bc(&g, &inl.patch, Box::new(FixedValue(Profile::PerFace(alp))))
            .expect("patch checked at construction");
    }

    /// Blend `alpha` and `u` toward the targets of the relaxation zones; returns the phase-1 volume added.
    fn apply_relaxation(&mut self, t: f64) -> f64 {
        let Some(w) = self.settings.waves.clone() else {
            return 0.0;
        };
        if w.zones.is_empty() {
            return 0.0;
        }
        let vol = self.geom.mesh.cell_volumes().to_vec();
        let cc = self.geom.mesh.cell_centres().to_vec();
        self.u_rc = Some(self.u.values.clone());
        let mut added = 0.0;
        for z in &w.zones {
            for c in 0..self.geom.n_cells() {
                let wgt = z.blend.weight(zone_chi(cc[c][0], z.x_inner, z.x_outer));
                if wgt >= 1.0 {
                    continue;
                }
                let [y_lo, y_hi] = self.cell_y[c];
                let zc = 0.5 * (y_lo + y_hi) - w.level;
                let (a_t, u_t) = match z.target {
                    RelaxTarget::StillWater => (liquid_fraction(zc, y_hi - y_lo, 0.0), [0.0; 3]),
                    RelaxTarget::Wave(wave, t_ramp) => {
                        let r = ramp(t, t_ramp);
                        let eta = r * wave.eta(cc[c][0], t);
                        let a = liquid_fraction(zc, y_hi - y_lo, eta);
                        let (u, v) = wave.velocity(cc[c][0], zc.min(eta), t);
                        (a, [a * r * u, a * r * v, 0.0])
                    }
                };
                let a_old = self.transport.alpha.values[c];
                let a_new = relax(wgt, a_old, a_t);
                self.transport.alpha.values[c] = a_new;
                added += (a_new - a_old) * vol[c];
                for k in 0..2 {
                    self.u.values[c][k] = relax(wgt, self.u.values[c][k], u_t[k]);
                }
            }
        }
        added
    }

    /// Apply the phase-change source to the transported `alpha` (ADR 0052 B2); `alpha_start` is the phase fraction before the
    /// transport (area density of the Hertz-Knudsen form).
    pub fn apply_mass_transfer(&mut self, dt: f64, alpha_start: &[f64]) -> PhaseChange {
        let mt = self.settings.mass_transfer.clone().expect("checked by the caller");
        let g = self.geom.clone();
        let n = g.n_cells();
        let vol = g.mesh.cell_volumes();
        let [r1, r2] = self.settings.rho;
        let liquid_is_1 = mt.liquid_phase == 1;
        let (rl, rv) = if liquid_is_1 { (r1, r2) } else { (r2, r1) };
        let grad = match mt.model {
            MassTransferModel::HertzKnudsen { .. } => {
                let own = g.mesh.owner();
                let bv: Vec<f64> = (0..g.mesh.n_boundary_faces())
                    .map(|i| alpha_start[own[g.n_internal + i]])
                    .collect();
                Some(green_gauss(&g, alpha_start, &bv, 0))
            }
            _ => None,
        };
        let mut mdot = vec![0.0; n];
        let (mut volume, mut rate, mut requested) = (0.0, 0.0, 0.0);
        let mut vol_src = vec![0.0; n];
        for c in 0..n {
            let a = self.transport.alpha.values[c];
            let al = if liquid_is_1 { a } else { 1.0 - a };
            let t = mt.temperature[c];
            let theta = (t - mt.t_sat) / mt.t_sat;
            let mut req = None;
            let new_l = match mt.model {
                MassTransferModel::Lee { c_evap, c_cond } => {
                    if t > mt.t_sat {
                        al * (-c_evap * theta * dt).exp()
                    } else if t < mt.t_sat {
                        let lambda = c_cond * rv * theta.abs() / rl;
                        1.0 - (1.0 - al) * (-lambda * dt).exp()
                    } else {
                        al
                    }
                }
                MassTransferModel::HertzKnudsen {
                    accommodation,
                    molar_mass,
                    schrage,
                    p_vapour,
                    p_sat_ref,
                } => {
                    let ai = vec3::norm(grad.as_ref().expect("gradient computed")[c]);
                    let psat = p_sat_ref * (mt.h_fg * molar_mass / R_UNIVERSAL * (1.0 / mt.t_sat - 1.0 / t)).exp();
                    let hk = HertzKnudsen {
                        sigma: accommodation,
                        molar_mass,
                        schrage,
                    };
                    let m = hk.rate(psat, t, p_vapour, t, ai).unwrap_or(0.0);
                    req = Some(m);
                    (al - m * dt / rl).clamp(0.0, 1.0)
                }
            };
            let m_eff = rl * (al - new_l) / dt;
            mdot[c] = m_eff;
            let a_new = if liquid_is_1 { new_l } else { 1.0 - new_l };
            self.transport.alpha.values[c] = a_new;
            volume += (a_new - a) * vol[c];
            rate += m_eff * vol[c];
            requested += req.unwrap_or(m_eff) * vol[c];
            vol_src[c] = m_eff * (1.0 / rv - 1.0 / rl) * vol[c];
        }
        PhaseChange {
            mdot,
            vol_src,
            volume,
            rate,
            h_fg: mt.h_fg,
            requested,
        }
    }

    /// Mixture density of every cell.
    pub fn density(&self) -> Vec<f64> {
        let [r1, r2] = self.settings.rho;
        self.transport.alpha.values.iter().map(|a| r2 + (r1 - r2) * a).collect()
    }

    fn gh_cells(&self) -> Vec<f64> {
        let s = &self.settings;
        self.geom
            .mesh
            .cell_centres()
            .iter()
            .map(|x| vec3::dot(s.gravity, vec3::sub(*x, s.x_ref)))
            .collect()
    }

    fn update_pressure(&mut self) {
        let rho = self.density();
        let gh = self.gh_cells();
        self.p = (0..rho.len()).map(|c| self.p_rgh.values[c] + rho[c] * gh[c]).collect();
    }

    /// Capillary time-step limit `sqrt((rho_1 + rho_2) dx^3 / (8 pi sigma))` (infinite without surface tension).
    pub fn capillary_dt(&self) -> f64 {
        let s = &self.settings;
        if s.sigma <= 0.0 {
            return f64::INFINITY;
        }
        let g = &self.geom;
        let dx = (0..g.n_internal)
            .filter(|&f| !g.empty[f])
            .map(|f| vec3::norm(g.delta[f]))
            .fold(f64::INFINITY, f64::min);
        ((s.rho[0] + s.rho[1]) * dx.powi(3) / (8.0 * std::f64::consts::PI * s.sigma)).sqrt()
    }

    /// Cell curvature `kappa = -div(n)` of the interface: the height-function estimate when `settings.height_function` is on
    /// (ADR 0106), the CSF estimate otherwise.
    pub fn curvature(&self) -> Vec<f64> {
        match (&self.hf, self.settings.height_function) {
            (Some(grid), true) => self.hf_curvature(grid).0,
            _ => self.csf_curvature(),
        }
    }

    /// Active cells (owning a face with `|d alpha| > 1e-6`) without a bracketing height-function column:
    /// `[filled from the neighbours, given the CSF value]`; `[0, 0]` without the height-function option.
    pub fn curvature_fallbacks(&self) -> [usize; 2] {
        match (&self.hf, self.settings.height_function) {
            (Some(grid), true) => self.hf_curvature(grid).1,
            _ => [0, 0],
        }
    }

    /// Height-function curvature (ADR 0106) of the active cells; inactive cells get 0 (their faces carry `|d alpha| <= 1e-6`).
    /// Fallback near under-resolved interfaces: an active cell without a bracketing column takes the mean of its face
    /// neighbours that have a value (up to 3 passes), then the CSF value.
    fn hf_curvature(&self, grid: &HfGrid) -> (Vec<f64>, [usize; 2]) {
        let g = &self.geom;
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let a = &self.transport.alpha.values;
        let n = a.len();
        let mut active = vec![false; n];
        for f in 0..g.n_internal {
            if (a[nei[f]] - a[own[f]]).abs() > 1e-6 {
                active[own[f]] = true;
                active[nei[f]] = true;
            }
        }
        let bv: Vec<f64> = (0..mesh.n_boundary_faces()).map(|i| a[own[g.n_internal + i]]).collect();
        let grad = green_gauss(g, a, &bv, 0);
        let axes: Vec<usize> = (0..3).filter(|&d| grid.n[d] > 1).collect();
        let mut val: Vec<Option<f64>> = (0..n)
            .map(|c| {
                if active[c] {
                    grid.column_curvature(a, c, grad[c], &axes)
                } else {
                    None
                }
            })
            .collect();
        let mut fb = [0usize; 2];
        for _ in 0..3 {
            let (mut sum, mut cnt) = (vec![0.0; n], vec![0usize; n]);
            for f in 0..g.n_internal {
                for (c, o) in [(own[f], nei[f]), (nei[f], own[f])] {
                    if let (true, None, Some(v)) = (active[c], val[c], val[o]) {
                        sum[c] += v;
                        cnt[c] += 1;
                    }
                }
            }
            for c in 0..n {
                if cnt[c] > 0 {
                    val[c] = Some(sum[c] / cnt[c] as f64);
                    fb[0] += 1;
                }
            }
        }
        let csf = if (0..n).any(|c| active[c] && val[c].is_none()) {
            Some(self.csf_curvature())
        } else {
            None
        };
        let kappa = (0..n)
            .map(|c| match (active[c], val[c], &csf) {
                (false, _, _) => 0.0,
                (true, Some(v), _) => v,
                (true, None, Some(k)) => {
                    fb[1] += 1;
                    k[c]
                }
                (true, None, None) => unreachable!(),
            })
            .collect();
        (kappa, fb)
    }

    /// CSF curvature `kappa = -div(n)` of the normalised gradient of the smoothed phase fraction (internal faces only,
    /// i.e. a 90 degree contact angle at walls).
    fn csf_curvature(&self) -> Vec<f64> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let n = g.n_cells();
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let vol = mesh.cell_volumes();
        let mut a = self.transport.alpha.values.clone();
        for _ in 0..self.settings.n_smooth {
            let mut sum = a.clone();
            let mut cnt = vec![1.0; n];
            for f in 0..g.n_internal {
                sum[own[f]] += a[nei[f]];
                sum[nei[f]] += a[own[f]];
                cnt[own[f]] += 1.0;
                cnt[nei[f]] += 1.0;
            }
            // zero-gradient mirror at the domain boundary: a missing neighbour counts as the cell itself, so a flat
            // interface stays flat up to the wall
            for f in g.n_internal..mesh.n_faces() {
                if !g.empty[f] {
                    sum[own[f]] += a[own[f]];
                    cnt[own[f]] += 1.0;
                }
            }
            a = (0..n).map(|c| sum[c] / cnt[c]).collect();
        }
        let bv: Vec<f64> = (0..mesh.n_boundary_faces()).map(|i| a[own[g.n_internal + i]]).collect();
        let grad = green_gauss(g, &a, &bv, 0);
        let delta = 1e-8 / (mesh.total_volume() / n as f64).cbrt();
        let mut kappa = vec![0.0; n];
        for f in 0..g.n_internal {
            let w = g.weights[f];
            let gf = [0, 1, 2].map(|k| w * grad[own[f]][k] + (1.0 - w) * grad[nei[f]][k]);
            let flux = vec3::dot(gf, sf[f]) / (vec3::norm(gf) + delta);
            kappa[own[f]] -= flux / vol[own[f]];
            kappa[nei[f]] += flux / vol[nei[f]];
        }
        kappa
    }

    /// Face force flux `lap_coeff [ sigma kappa_f (alpha_N - alpha_P) - (g.x_f)(rho_N - rho_P) ]` of the internal faces.
    fn force_flux(&self, rho: &[f64]) -> Vec<f64> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let s = &self.settings;
        let a = &self.transport.alpha.values;
        let kappa = if s.sigma > 0.0 { Some(self.curvature()) } else { None };
        let cf = mesh.face_centres();
        (0..g.n_internal)
            .map(|f| {
                let (o, nb) = (own[f], nei[f]);
                let mut t = 0.0;
                if let Some(k) = &kappa {
                    let w = g.weights[f];
                    t += s.sigma * (w * k[o] + (1.0 - w) * k[nb]) * (a[nb] - a[o]);
                }
                let ghf = vec3::dot(s.gravity, vec3::sub(cf[f], s.x_ref));
                t -= ghf * (rho[nb] - rho[o]);
                g.lap_coeff[f] * t
            })
            .collect()
    }

    /// Cell vector from a face flux `q` (`sum_f (S/|S|) q_f` times the inverse tensor): the least-squares-exact
    /// reconstruction of a gradient-like quantity whose face flux is `q`.
    fn reconstruct(&self, q: &[f64]) -> Vec<Vec3> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let mut sum = vec![[0.0; 3]; g.n_cells()];
        for (f, &qf) in q.iter().enumerate() {
            if g.empty[f] || qf == 0.0 {
                continue;
            }
            let nrm = vec3::scale(sf[f], 1.0 / vec3::norm(sf[f]));
            // outward normal and outward flux both change sign for the neighbour: same contribution
            vec3::axpy(&mut sum[own[f]], qf, nrm);
            if f < g.n_internal {
                vec3::axpy(&mut sum[nei[f]], qf, nrm);
            }
        }
        sum.iter()
            .zip(&self.recon)
            .map(|(s, t)| [0, 1, 2].map(|i| t[i][0] * s[0] + t[i][1] * s[1] + t[i][2] * s[2]))
            .collect()
    }

    /// Advance one time step of size `dt`.
    pub fn step(&mut self, dt: f64) -> Result<VofStepInfo> {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let n_int = g.n_internal;
        let nf = mesh.n_faces();
        let vol = mesh.cell_volumes().to_vec();
        let sf = mesh.face_area_vectors();
        let s = self.settings.clone();
        self.step_no += 1;
        let [r1, r2] = s.rho;

        let rho_old = self.density();
        let u_old: Vec<Vec3> = self.u.values.clone();
        let phi_old = self.phi.clone();
        let u_rc = self.u_rc.take();

        // ---- phase fraction ---------------------------------------------------------
        let t_new = self.time + dt;
        self.update_wave_inlet(t_new);
        let alpha_start = self.transport.alpha.values.clone();
        let ainfo = self.transport.advance(&self.phi, dt)?;
        let mut transfer = if self.settings.mass_transfer.is_some() {
            Some(self.apply_mass_transfer(dt, &alpha_start))
        } else {
            None
        };
        let rho = self.density();
        let mut cav: Option<CavIter> = self.settings.cavitation.map(|cv| {
            let (rl, rv) = if cv.liquid_phase == 1 { (r1, r2) } else { (r2, r1) };
            let gh = self.gh_cells();
            let av: Vec<f64> = self
                .transport
                .alpha
                .values
                .iter()
                .map(|&a| if cv.liquid_phase == 1 { 1.0 - a } else { a })
                .collect();
            CavIter {
                cv,
                rl,
                rv,
                dt,
                rho_m: av.iter().map(|&x| x * rv + (1.0 - x) * rl).collect(),
                av,
                off: (0..n).map(|c| rho[c] * gh[c]).collect(),
                vol: vol.clone(),
                s_used: vec![0.0; n],
                disc: 0.0,
                iters: 0,
            }
        });
        let a = self.transport.alpha.values.clone();
        let mu_c: Vec<f64> = if s.harmonic_viscosity {
            a.iter().map(|x| 1.0 / (x / s.mu[0] + (1.0 - x) / s.mu[1])).collect()
        } else {
            a.iter().map(|x| s.mu[1] + (s.mu[0] - s.mu[1]) * x).collect()
        };
        let mu_f = face_interpolate(&g, &mu_c);
        let rho_phi: Vec<f64> = (0..nf)
            .map(|f| {
                if g.empty[f] {
                    0.0
                } else {
                    r2 * self.phi[f] + (r1 - r2) * ainfo.alpha_phi[f]
                }
            })
            .collect();
        let mut force = self.force_flux(&rho);
        if let Some(b) = self.drift_force() {
            let df = self.body_force_flux(&b);
            for (fo, d) in force.iter_mut().zip(df) {
                if d != 0.0 {
                    *fo += d;
                }
            }
        }

        // old-level flux defect for the transient Rhie-Chow term
        let ddt_defect: Vec<f64> = if s.ddt_correction {
            let zero_b = vec![[0.0; 3]; mesh.n_boundary_faces()];
            let fo = face_flux(&g, u_rc.as_ref().unwrap_or(&u_old), &zero_b);
            (0..n_int).map(|f| phi_old[f] - fo[f]).collect()
        } else {
            vec![0.0; n_int]
        };

        let n_active = g.active.iter().filter(|&&x| x).count() as f64;
        let mut res_u: f64 = 0.0;
        let mut res_p = 0.0;
        let mut p_iters = 0;
        let own = mesh.owner();

        for outer in 0..s.n_outer {
            let last = outer + 1 == s.n_outer;
            let (alpha_u, alpha_p) = if last { (1.0, 1.0) } else { (s.alpha_u, s.alpha_p) };
            let pbc = pressure_bc(&g, &self.u, &self.p_rgh);
            let ub = self.u.boundary_values(&g);

            // net face acceleration flux of the previous iterate (force minus p_rgh gradient) -> predictor source
            let mut q_pred = vec![0.0; nf];
            for f in 0..n_int {
                if !g.empty[f] {
                    q_pred[f] = force[f]
                        - g.lap_coeff[f] * (self.p_rgh.values[mesh.neighbour()[f]] - self.p_rgh.values[own[f]]);
                }
            }
            for f in n_int..nf {
                let i = f - n_int;
                if !g.empty[f] && !pbc.fixed_u[i] {
                    q_pred[f] = -vec3::norm(sf[f]) * pbc.coeffs[i].gradient(self.p_rgh.values[own[f]]);
                }
            }
            let src = self.reconstruct(&q_pred);
            let transposed = s
                .full_viscous_stress
                .then(|| crate::pimple::transposed_stress_source(&g, &self.u, &mu_f));

            let mut mats: Vec<Option<FvMatrix>> = vec![None, None, None];
            let mut a_p = vec![0.0; n];
            let u_iter: Vec<Vec3> = self.u.values.clone();
            for k in 0..3 {
                if !g.active[k] {
                    continue;
                }
                let cells: Vec<f64> = u_iter.iter().map(|v| v[k]).collect();
                let bc = self.u.boundary_coeffs(&g, k);
                let bvk: Vec<f64> = ub.iter().map(|v| v[k]).collect();
                let grad = if s.convection.needs_gradient() || s.non_orth_correctors > 0 {
                    Some(least_squares(&g, &cells, &bvk))
                } else {
                    None
                };
                let mut m = convection(&g, &rho_phi, &cells, &bc, s.convection, grad.as_deref());
                m.add(&laplacian(&g, &mu_f, &bc, grad.as_deref()));
                for c in 0..n {
                    m.a.diag[c] += rho[c] * vol[c] / dt;
                    m.b[c] += rho_old[c] * vol[c] / dt * u_old[c][k];
                }
                if let Some(ts) = &transposed {
                    for c in 0..n {
                        m.b[c] += vol[c] * ts[c][k];
                    }
                }
                if alpha_u < 1.0 {
                    m.relax(alpha_u, &cells);
                }
                let mut sys = m.clone();
                for c in 0..n {
                    sys.b[c] += vol[c] * src[c][k];
                }
                if outer == 0 {
                    res_u = res_u.max(crate::simple::normalised_residual(&sys, &cells));
                }
                let mut x = cells;
                sys.solve(&mut x, &s.momentum_solver);
                for c in 0..n {
                    self.u.values[c][k] = x[c];
                    a_p[c] += m.a.diag[c];
                }
                mats[k] = Some(m);
            }
            for x in a_p.iter_mut() {
                *x /= n_active;
            }
            let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
            let gamma_p = face_interpolate(&g, &r_au);
            // transient Rhie-Chow coefficient interpolate(rho rAU) / dt <= 1 (rho and rAU are interpolated together: their
            // separate interpolations are unbounded at a density jump and made the hydrostatic state unstable)
            let rho_r_au: Vec<f64> = (0..n).map(|c| rho[c] * r_au[c]).collect();
            let ddt_coeff = face_interpolate(&g, &rho_r_au);

            for corr in 0..s.n_correctors {
                let mut hby_a = vec![[0.0; 3]; n];
                for k in 0..3 {
                    if let Some(m) = &mats[k] {
                        let cells: Vec<f64> = self.u.values.iter().map(|v| v[k]).collect();
                        let h = m.a.h_operator(&cells, &m.b);
                        for c in 0..n {
                            hby_a[c][k] = if s.component_diagonal {
                                (h[c] + (a_p[c] - m.a.diag[c]) * cells[c]) / a_p[c]
                            } else {
                                h[c] / a_p[c]
                            };
                        }
                    }
                }
                let hb = boundary_hby_a(&g, &pbc, &ub, &hby_a);
                let mut phi_hby_a = face_flux(&g, &hby_a, &hb);
                if s.ddt_correction {
                    for f in 0..n_int {
                        phi_hby_a[f] += ddt_coeff[f] / dt * ddt_defect[f];
                    }
                }
                let phi_hby_a0 = phi_hby_a.clone();
                for f in 0..n_int {
                    if !g.empty[f] {
                        phi_hby_a[f] += gamma_p[f] * force[f];
                    }
                }
                let controls = PressureControls {
                    p_ref: s.p_ref,
                    non_orth_correctors: s.non_orth_correctors,
                    solver: s.pressure_solver,
                    amg: s.amg_pressure,
                    linear: s.linear,
                };
                let p_old = self.p_rgh.values.clone();
                let mut p_new = p_old.clone();
                let (r0, it) = if let Some(ci) = cav.as_mut() {
                    ci.solve(&g, &gamma_p, &pbc, &phi_hby_a, &p_old, &mut p_new, &controls)
                } else {
                    let vol_src = transfer.as_ref().map(|t| t.vol_src.as_slice());
                    let (r0, it, _) =
                        solve_pressure_parts_src(&g, &gamma_p, &pbc, &phi_hby_a, &mut p_new, &controls, false, vol_src);
                    (r0, it)
                };
                p_iters += it;
                if corr == 0 {
                    res_p = r0;
                }
                self.phi = corrected_flux(&g, &gamma_p, &pbc, &phi_hby_a, &p_new, &ub, s.non_orth_correctors > 0);
                let mut q = vec![0.0; nf];
                for f in 0..nf {
                    if g.empty[f] || (f >= n_int && pbc.fixed_u[f - n_int]) {
                        continue;
                    }
                    q[f] = (self.phi[f] - phi_hby_a0[f]) / gamma_p[f];
                }
                let rec = self.reconstruct(&q);
                for c in 0..n {
                    for k in 0..3 {
                        if g.active[k] {
                            self.u.values[c][k] = hby_a[c][k] + r_au[c] * rec[c][k];
                        }
                    }
                }
                let last_corr = corr + 1 == s.n_correctors;
                let ap = if last_corr { alpha_p } else { 1.0 };
                for c in 0..n {
                    self.p_rgh.values[c] = p_old[c] + ap * (p_new[c] - p_old[c]);
                }
            }
        }

        if let Some(ci) = &cav {
            transfer = Some(self.finish_cavitation(ci, dt));
            self.cav_disc = ci.disc;
            self.cav_iters = ci.iters;
        }
        let relax_volume = self.apply_relaxation(t_new);
        self.time += dt;
        self.update_pressure();
        let cont = divergence(&g, &self.phi).iter().fold(0.0f64, |m, &b| m.max(b.abs()));
        let area_scale = sf.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
        let (amin, amax) = self
            .transport
            .alpha
            .values
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
                (lo.min(x), hi.max(x))
            });
        let info = VofStepInfo {
            step: self.step_no,
            time: self.time,
            dt,
            courant: crate::ddt::max_courant(&g, &self.phi, dt),
            alpha_courant: ainfo.courant,
            n_sub: ainfo.n_sub,
            continuity: cont / area_scale.max(1e-300),
            res_u,
            res_p,
            p_iters,
            alpha_min: amin,
            alpha_max: amax,
            capillary_exceeded: dt > self.capillary_dt(),
            alpha_outflow: (n_int..nf).map(|f| ainfo.alpha_phi[f]).sum(),
            relax_volume,
            transfer_volume: transfer.as_ref().map_or(0.0, |t| t.volume),
            transfer_rate: transfer.as_ref().map_or(0.0, |t| t.rate),
            volume_source: transfer.as_ref().map_or(0.0, |t| t.vol_src.iter().sum()),
            latent_power: transfer.as_ref().map_or(0.0, |t| t.rate * t.h_fg),
        };
        if let Some(t) = transfer {
            self.mdot = t.mdot;
        }
        self.history.push(info);
        Ok(info)
    }

    /// Advance to `t_end` with a fixed step.
    pub fn run_fixed(&mut self, t_end: f64, dt: f64) -> Result<Vec<VofStepInfo>> {
        let mut out = Vec::new();
        while self.time < t_end - 1e-12 * dt.max(1e-300) {
            let d = dt.min(t_end - self.time);
            out.push(self.step(d)?);
        }
        Ok(out)
    }

    /// Volume of phase 1.
    pub fn phase_volume(&self) -> f64 {
        self.transport.phase_volume()
    }

    /// Boundary-face values of `p_rgh` under the pressure conditions (diagnostic).
    pub fn boundary_p_rgh(&self) -> Vec<f64> {
        let pbc = pressure_bc(&self.geom, &self.u, &self.p_rgh);
        boundary_values_pbc(&self.geom, &pbc, &self.p_rgh.values)
    }
}

/// Inverse of `T = sum_f S S / |S|` per cell over the non-empty faces; inactive directions get the identity entry.
fn reconstruction_tensors(geom: &FvGeometry) -> Vec<[[f64; 3]; 3]> {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut t = vec![[[0.0; 3]; 3]; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let a = vec3::norm(sf[f]);
        let mut rows = vec![own[f]];
        if f < geom.n_internal {
            rows.push(nei[f]);
        }
        for row in rows {
            for p in 0..3 {
                for q in 0..3 {
                    t[row][p][q] += sf[f][p] * sf[f][q] / a;
                }
            }
        }
    }
    t.iter_mut()
        .map(|m| {
            for k in 0..3 {
                if !geom.active[k] {
                    m[k] = [0.0; 3];
                    for row in m.iter_mut() {
                        row[k] = 0.0;
                    }
                    m[k][k] = 1.0;
                }
            }
            invert3(*m)
        })
        .collect()
}

fn invert3(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let c = |r: usize, s: usize| {
        let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
        let (s1, s2) = ((s + 1) % 3, (s + 2) % 3);
        m[r1][s1] * m[r2][s2] - m[r1][s2] * m[r2][s1]
    };
    let det = m[0][0] * c(0, 0) + m[0][1] * c(0, 1) + m[0][2] * c(0, 2);
    let mut inv = [[0.0; 3]; 3];
    for (i, row) in inv.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = c(j, i) / det;
        }
    }
    inv
}
