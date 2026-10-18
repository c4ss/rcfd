//! Wave 31B (ADR 0063): diagnostic experiments for the wall-resolved SST skin-friction deficit. Ignored, no limits, prints only.
//! `mod probe` is a test-side copy of `crates/rcfd-turb/src/sst.rs` with single-factor switches ([`probe::Sw`]); with all switches
//! off it must reproduce `KOmegaSst` (checked by `probe_matches_library`). Select experiments with the environment variables
//! `W31B_EXP` (comma list of experiment names, see `experiments()`) and `W31B_MESH` (c, m or f).

use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_mesh::layers::{first_cell_height_from_y_plus, flat_plate_u_tau};
use rcfd_turb::sst::SstWallMode;
use rcfd_turb::KOmegaSst;
use std::sync::Arc;

use probe::Sw;
type Probe = probe::KOmegaSst;

#[allow(dead_code, clippy::all)]
mod probe {
    use rcfd_core::vec3;
    use rcfd_core::Result;
    use rcfd_fv::convection::{convection, ConvScheme};
    use rcfd_fv::diffusion::laplacian;
    use rcfd_fv::gradient::{green_gauss, least_squares};
    use rcfd_fv::{FvGeometry, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_turb::eddy::{effective_viscosity, patch_faces};
    use rcfd_turb::sst::{blend_f1, blend_f2, SstVariant, SstWallMode, BETA_STAR};
    use rcfd_turb::wall::{wall_distance, SpaldingLaw};
    use std::sync::Arc;

    const A1: f64 = 0.31;
    const SIGMA_K: [f64; 2] = [0.85, 1.0];
    const SIGMA_W: [f64; 2] = [0.5, 0.856];
    const BETA: [f64; 2] = [0.075, 0.0828];
    const GAMMA: [f64; 2] = [5.0 / 9.0, 0.44];

    fn velocity_gradient(geom: &FvGeometry, u: &VectorField) -> Vec<Vec<[f64; 3]>> {
        let ub = u.boundary_values(geom);
        (0..3)
            .map(|k| {
                let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
                let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
                green_gauss(geom, &cells, &bv, 0)
            })
            .collect()
    }

    fn bsl_gamma(i: usize) -> f64 {
        BETA[i] / BETA_STAR - SIGMA_W[i] * 0.41f64.powi(2) / BETA_STAR.sqrt()
    }

    /// Experiment switches (all off / default = the unchanged `KOmegaSst` of `crates/rcfd-turb/src/sst.rs`).
    #[derive(Clone, Copy, Debug)]
    pub struct Sw {
        /// factor of the wall-face omega value: `face_factor * 6 nu / (beta1 y^2)` (default 10).
        pub face_factor: f64,
        /// fix the first-cell omega to `cell1_factor * 6 nu / (beta1 y^2)` in every wall cell.
        pub cell1_analytic: bool,
        pub cell1_factor: f64,
        /// no Spalding strain/production override in the wall cell (plain Green-Gauss strain, plain production).
        pub plain_wall_strain: bool,
        /// k = 0 on the wall face while the mode stays Automatic.
        pub k_wall_zero: bool,
        /// cross-diffusion source of the omega equation off (F1 still uses CD).
        pub cd_off: bool,
        /// production limiter off.
        pub limiter_off: bool,
        /// least-squares velocity gradient for the strain.
        pub ls_gradient: bool,
    }

    impl Sw {
        pub const BASE: Sw = Sw {
            face_factor: 10.0,
            cell1_analytic: false,
            cell1_factor: 1.0,
            plain_wall_strain: false,
            k_wall_zero: false,
            cd_off: false,
            limiter_off: false,
            ls_gradient: false,
        };
    }

    /// Steady k-omega SST model state.
    pub struct KOmegaSst {
        /// Geometry.
        pub geom: Arc<FvGeometry>,
        /// Turbulent kinetic energy (m^2/s^2).
        pub k: ScalarField,
        /// Specific dissipation rate (1/s).
        pub omega: ScalarField,
        /// Laminar kinematic viscosity.
        pub nu: f64,
        /// Eddy viscosity of every cell (under-relaxed).
        pub nut: Vec<f64>,
        /// Wall eddy viscosity of the wall faces `(global face, nu_t,w)`.
        pub wall_nut: Vec<(usize, f64)>,
        /// Law of the wall used for the wall shear.
        pub law: SpaldingLaw,
        /// Wall-cell `y+` above which omega of the wall cell is fixed to the blended viscous/log value
        /// instead of being solved with the Menter wall value on the wall face.
        pub omega_fix_y_plus: f64,
        /// Convection scheme of k and omega.
        pub convection: ConvScheme,
        /// Implicit under-relaxation of k and omega.
        pub relax_turb: f64,
        /// Explicit under-relaxation of the eddy viscosity.
        pub relax_nut: f64,
        /// Linear solver controls.
        pub solver: SolveControl,
        /// Model variant (default [`SstVariant::Sst`]).
        pub variant: SstVariant,
        /// Wall treatment mode (default [`SstWallMode::Automatic`]); change it with [`KOmegaSst::set_wall_mode`].
        pub wall_mode: SstWallMode,
        /// Experiment switches.
        pub sw: Sw,
        /// Largest unlimited P / (10 beta* k omega) of the last sweep.
        pub max_lim_ratio: f64,
        wall_names: Vec<String>,
        wall_dist: Vec<f64>,
        wall_faces: Vec<usize>,
        cell_wall_face: Vec<usize>,
        f1: Vec<f64>,
        y_plus: Vec<f64>,
        started: bool,
    }

    impl KOmegaSst {
        /// New model with uniform initial `k` and `omega`; `walls` are the wall patch names.
        pub fn new(geom: Arc<FvGeometry>, nu: f64, walls: &[&str], k0: f64, omega0: f64, sw: Sw) -> Result<Self> {
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
            let wall_faces_len = wall_faces.len();
            // Menter's wall value omega_w = 10 * 6 nu / (beta_1 y^2) on the wall faces, y being the
            // distance of the first cell centre
            let mut omega = ScalarField::uniform("omega", &geom, omega0);
            for name in walls {
                if let Some(patch) = geom.mesh.patch(name) {
                    let vals: Vec<[f64; 3]> = patch
                        .faces()
                        .map(|f| {
                            let y = geom.dn[f - geom.n_internal];
                            [sw.face_factor * 6.0 * nu / (BETA[0] * y * y), 0.0, 0.0]
                        })
                        .collect();
                    omega.set_bc(
                        &geom,
                        name,
                        Box::new(rcfd_fv::bc::FixedValue(rcfd_fv::bc::Profile::PerFace(vals))),
                    )?;
                }
            }
            Ok(KOmegaSst {
                k: ScalarField::uniform("k", &geom, k0),
                omega,
                nu,
                nut: vec![k0 / omega0; n],
                wall_nut: Vec::new(),
                law: SpaldingLaw::default(),
                omega_fix_y_plus: 5.0,
                convection: ConvScheme::Upwind,
                relax_turb: 0.7,
                relax_nut: 0.5,
                solver: SolveControl {
                    tol_abs: 1e-14,
                    tol_rel: 1e-6,
                    max_iter: 500,
                },
                variant: SstVariant::Sst,
                wall_mode: SstWallMode::Automatic,
                sw,
                max_lim_ratio: 0.0,
                wall_names: walls.iter().map(|w| w.to_string()).collect(),
                wall_dist,
                wall_faces,
                cell_wall_face,
                f1: vec![1.0; n],
                y_plus: vec![0.0; wall_faces_len],
                geom,
                started: false,
            })
        }

        /// Select the wall treatment. `LowRe` sets `k = 0` on the wall faces; `Automatic` restores zero gradient.
        pub fn set_wall_mode(&mut self, mode: SstWallMode) -> Result<()> {
            self.wall_mode = mode;
            for name in self.wall_names.clone() {
                let bc: Box<dyn rcfd_fv::bc::Bc> = match (mode, self.sw.k_wall_zero) {
                    (SstWallMode::LowRe, _) | (_, true) => Box::new(rcfd_fv::bc::FixedValue::scalar(0.0)),
                    _ => Box::new(rcfd_fv::ZeroGradient),
                };
                self.k.set_bc(&self.geom, &name, bc)?;
            }
            Ok(())
        }

        /// Model coefficients `(sigma_k, sigma_w, beta, gamma)` (sets 1 and 2) of the active variant.
        fn coeffs(&self) -> ([f64; 2], [f64; 2], [f64; 2], [f64; 2]) {
            match self.variant {
                SstVariant::Sst => (SIGMA_K, SIGMA_W, BETA, GAMMA),
                SstVariant::Bsl => ([0.5, 1.0], SIGMA_W, BETA, [bsl_gamma(0), bsl_gamma(1)]),
            }
        }

        /// Eddy viscosity from the local state (`s` strain magnitude, `f2` blending function).
        fn nut_of(&self, k: f64, w: f64, s: f64, f2: f64) -> f64 {
            match self.variant {
                SstVariant::Sst => A1 * k / (A1 * w).max(s * f2).max(1e-30),
                SstVariant::Bsl => k / w.max(1e-30),
            }
        }

        /// Wall distance of every cell centre.
        pub fn wall_distance(&self) -> &[f64] {
            &self.wall_dist
        }

        /// `y+ = u_tau y / nu` (Spalding `u_tau`) of every wall face, in the order of the wall patches'
        /// faces, from the last sweep.
        pub fn y_plus(&self) -> &[f64] {
            &self.y_plus
        }

        /// Blending function `F1` of the last sweep.
        pub fn f1(&self) -> &[f64] {
            &self.f1
        }

        /// Effective face viscosity `nu + nu_t` (wall faces from the wall law) for the momentum equation.
        pub fn nu_face(&self) -> Vec<f64> {
            effective_viscosity(&self.geom, self.nu, &self.nut, &self.wall_nut)
        }

        fn tangential_speed(&self, u: &VectorField, f: usize) -> f64 {
            let c = self.geom.mesh.owner()[f];
            let n = self.geom.normal(f);
            let v = u.values[c];
            vec3::norm(vec3::sub(v, vec3::scale(n, vec3::dot(v, n))))
        }

        fn transport(&self, fld: &ScalarField, phi: &[f64], gamma_f: &[f64]) -> rcfd_fv::FvMatrix {
            let g = &self.geom;
            let cells = fld.values.clone();
            let bc = fld.boundary_coeffs(g);
            let bvals = fld.boundary_values(g);
            let grad = if self.convection.needs_gradient() {
                Some(least_squares(g, &cells, &bvals))
            } else {
                None
            };
            let mut m = convection(g, phi, &cells, &bc, self.convection, grad.as_deref());
            m.add(&laplacian(g, gamma_f, &bc, grad.as_deref()));
            m
        }

        /// One sweep: solve k and omega on the flux `phi` for the current velocity, then update `nut`
        /// and `wall_nut`. Returns the largest relative change of `nut`.
        pub fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
            let g = self.geom.clone();
            let n = g.n_cells();
            let nu = self.nu;
            let vol = g.mesh.cell_volumes().to_vec();
            let gr = if self.sw.ls_gradient {
                let ub = u.boundary_values(&g);
                (0..3)
                    .map(|k| {
                        let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
                        let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
                        least_squares(&g, &cells, &bv)
                    })
                    .collect::<Vec<_>>()
            } else {
                velocity_gradient(&g, u)
            };
            let mut s_mag: Vec<f64> = (0..n)
                .map(|c| {
                    let mut s2 = 0.0;
                    for i in 0..3 {
                        for j in 0..3 {
                            let sij = 0.5 * (gr[i][c][j] + gr[j][c][i]);
                            s2 += sij * sij;
                        }
                    }
                    (2.0 * s2).sqrt()
                })
                .collect();
            let kv = self.k.values.clone();
            let wv = self.omega.values.clone();

            // ---- wall cells: law of the wall, strain, production and omega
            let mut wall_g: Vec<Option<f64>> = vec![None; n];
            let mut wall_w: Vec<Option<f64>> = vec![None; n];
            let wall_cells: Vec<usize> = (0..n).filter(|&c| self.cell_wall_face[c] != usize::MAX).collect();
            let low_re = self.wall_mode == SstWallMode::LowRe;
            for &c in &wall_cells {
                if low_re {
                    break;
                }
                let f = self.cell_wall_face[c];
                let y = g.dn[f - g.n_internal];
                let uu = self.tangential_speed(u, f);
                let ut = self.law.u_tau(nu, uu, y);
                if ut <= 0.0 {
                    continue;
                }
                // u+ at the cell centre from the law (invert y+ = y+(u+) by u+ = U/u_tau)
                let up = uu / ut;
                let dy_du = self.law.dy_du(up);
                let dudy = ut * ut / nu / dy_du;
                let f_t = 1.0 - 1.0 / dy_du;
                if !self.sw.plain_wall_strain {
                    s_mag[c] = dudy;
                    wall_g[c] = Some(ut * ut * f_t * dudy);
                }
                if ut * y / nu > self.omega_fix_y_plus {
                    let w_vis = 6.0 * nu / (BETA[0] * y * y);
                    let w_log = ut / (BETA_STAR.sqrt() * self.law.kappa * y);
                    wall_w[c] = Some((w_vis * w_vis + w_log * w_log).sqrt());
                }
                if self.sw.cell1_analytic {
                    wall_w[c] = Some(self.sw.cell1_factor * 6.0 * nu / (BETA[0] * y * y));
                }
            }

            // ---- cross diffusion, F1 and F2
            let kb = self.k.boundary_values(&g);
            let wb = self.omega.boundary_values(&g);
            let gk = green_gauss(&g, &kv, &kb, 0);
            let gw = green_gauss(&g, &wv, &wb, 0);
            let mut cd = vec![0.0; n]; // 2 sigma_w2 / omega grad k . grad omega
            let mut f1 = vec![0.0; n];
            let mut f2 = vec![0.0; n];
            for c in 0..n {
                let (kc, wc, d) = (kv[c].max(1e-30), wv[c].max(1e-30), self.wall_dist[c]);
                cd[c] = 2.0 * SIGMA_W[1] / wc * vec3::dot(gk[c], gw[c]);
                f1[c] = blend_f1(kc, wc, d, nu, cd[c]);
                f2[c] = blend_f2(kc, wc, d, nu);
            }
            self.f1 = f1.clone();
            let mix = |f: f64, a: [f64; 2]| f * a[0] + (1.0 - f) * a[1];
            let (c_sigma_k, c_sigma_w, c_beta, c_gamma) = self.coeffs();

            // eddy viscosity used in the transport equations (consistent with the strain used here)
            let nut_now: Vec<f64> = (0..n).map(|c| self.nut_of(kv[c], wv[c], s_mag[c], f2[c])).collect();
            let gam_k: Vec<f64> = (0..n).map(|c| mix(f1[c], c_sigma_k) * nut_now[c]).collect();
            let gam_w: Vec<f64> = (0..n).map(|c| mix(f1[c], c_sigma_w) * nut_now[c]).collect();
            let gk_f = effective_viscosity(&g, nu, &gam_k, &[]);
            let gw_f = effective_viscosity(&g, nu, &gam_w, &[]);

            // production (limited)
            let mut ptil = vec![0.0; n];
            for c in 0..n {
                let p = wall_g[c].unwrap_or(nut_now[c] * s_mag[c] * s_mag[c]);
                let lim = 10.0 * BETA_STAR * kv[c] * wv[c];
                self.max_lim_ratio = if c == 0 {
                    p / lim
                } else {
                    self.max_lim_ratio.max(p / lim)
                };
                ptil[c] = if self.sw.limiter_off { p } else { p.min(lim) };
            }

            // ---- k equation
            let mut mk = self.transport(&self.k, phi, &gk_f);
            for c in 0..n {
                mk.b[c] += ptil[c] * vol[c];
                mk.a.diag[c] += BETA_STAR * wv[c] * vol[c];
            }
            mk.relax(self.relax_turb, &kv);
            let mut knew = kv.clone();
            mk.solve(&mut knew, &self.solver);
            knew.iter_mut().for_each(|x| *x = x.max(1e-12));
            self.k.values = knew;

            // ---- omega equation
            let mut mw = self.transport(&self.omega, phi, &gw_f);
            for c in 0..n {
                let beta = mix(f1[c], c_beta);
                let gamma = mix(f1[c], c_gamma);
                let nut_c = nut_now[c].max(1e-30);
                mw.b[c] += gamma * ptil[c] / nut_c * vol[c];
                mw.a.diag[c] += beta * wv[c] * vol[c];
                let cdc = if self.sw.cd_off { 0.0 } else { (1.0 - f1[c]) * cd[c] };
                if cdc >= 0.0 {
                    mw.b[c] += cdc * vol[c];
                } else {
                    mw.a.diag[c] += -cdc / wv[c].max(1e-30) * vol[c];
                }
            }
            mw.relax(self.relax_turb, &wv);
            for &c in &wall_cells {
                if let Some(w) = wall_w[c] {
                    let big = 1e8 * mw.a.diag[c].abs().max(1e-30);
                    mw.a.diag[c] = big;
                    mw.b[c] = big * w;
                }
            }
            let mut wnew = wv.clone();
            mw.solve(&mut wnew, &self.solver);
            wnew.iter_mut().for_each(|x| *x = x.max(1e-10));
            self.omega.values = wnew;

            // ---- eddy viscosity
            let (kc, wcv) = (self.k.values.clone(), self.omega.values.clone());
            let cap = 1e5 * nu;
            let mut change: f64 = 0.0;
            let alpha = if self.started { self.relax_nut } else { 1.0 };
            for c in 0..n {
                let new = self.nut_of(kc[c], wcv[c], s_mag[c], f2[c]).min(cap);
                let upd = alpha * new + (1.0 - alpha) * self.nut[c];
                change = change.max((upd - self.nut[c]).abs() / (self.nut[c] + nu));
                self.nut[c] = upd;
            }
            self.started = true;
            let old = std::mem::take(&mut self.wall_nut);
            let law = self.law;
            let mut yp = vec![0.0; self.wall_faces.len()];
            self.wall_nut = self
                .wall_faces
                .iter()
                .enumerate()
                .map(|(i, &f)| {
                    let y = g.dn[f - g.n_internal];
                    let uu = self.tangential_speed(u, f);
                    let ut = if uu <= 0.0 {
                        0.0
                    } else if low_re {
                        (nu * uu / y).sqrt()
                    } else {
                        law.u_tau(nu, uu, y)
                    };
                    yp[i] = ut * y / nu;
                    let new = if uu > 0.0 && !low_re {
                        (ut * ut * y / uu - nu).max(0.0)
                    } else {
                        0.0
                    };
                    let v = match old.get(i) {
                        Some(&(_, o)) => alpha * new + (1.0 - alpha) * o,
                        None => new,
                    };
                    (f, v)
                })
                .collect();
            self.y_plus = yp;
            change
        }
    }
}

const NU: f64 = 2.0e-7;

#[derive(Clone, Copy, Debug)]
struct Mesh {
    nx: usize,
    ny: usize,
    layers: usize,
    growth: f64,
    y_plus: f64,
}

const MESHES: [Mesh; 3] = [
    Mesh {
        nx: 70,
        ny: 55,
        layers: 35,
        growth: 1.21,
        y_plus: 1.0,
    },
    Mesh {
        nx: 140,
        ny: 110,
        layers: 70,
        growth: 1.10,
        y_plus: 0.5,
    },
    Mesh {
        nx: 280,
        ny: 220,
        layers: 140,
        growth: 1.0488,
        y_plus: 0.25,
    },
];

#[derive(Clone, Copy, Debug)]
enum Kind {
    Lib(SstWallMode),
    Probe(Sw, SstWallMode),
}

enum T {
    L(Box<KOmegaSst>),
    P(Box<Probe>),
}

#[derive(Clone, Debug, Default)]
struct Col {
    x: f64,
    cf: f64,
    u_tau: f64,
    re_theta: f64,
    re_theta_a: f64,
    re_theta_b: f64,
    /// Re_theta from the profile with the edge where nut/nu falls below 1 (edge velocity there)
    re_theta_edge: f64,
    /// Re_theta from the integral of Cf/2 dx (filled after the sweep)
    re_theta_mi: f64,
    /// per cell of the column from the wall: (y, wall distance, u, k, omega, nut, f1)
    cells: Vec<[f64; 7]>,
}

struct Res {
    cols: Vec<Col>,
    iterations: usize,
    du: f64,
    change: f64,
    max_lim_ratio: f64,
    first: f64,
}

fn run(kind: Kind, m: Mesh, kin: f64, omega_in: f64) -> Res {
    let ut_end = flat_plate_u_tau(1.0, NU, 2.0);
    let first = first_cell_height_from_y_plus(m.y_plus, ut_end, NU);
    let g_y = Grading::WallLayers {
        first,
        growth: m.growth,
        layers: m.layers,
        both: false,
    };
    let mesh = box_2d_graded(m.nx, m.ny, 2.0, 1.0, [Grading::Expansion(8.0), g_y], false, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(NU, Coupling::Simplec);
    st.alpha_u = 0.8;
    st.convection = ConvScheme::LinearUpwind;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, st);
    let mut turb = match kind {
        Kind::Lib(mode) => {
            let mut t = KOmegaSst::new(geom.clone(), NU, &["ymin"], kin, omega_in).unwrap();
            t.relax_turb = 0.6;
            t.k.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(kin))).unwrap();
            t.omega
                .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(omega_in)))
                .unwrap();
            if mode != SstWallMode::Automatic {
                t.set_wall_mode(mode).unwrap();
            }
            T::L(Box::new(t))
        }
        Kind::Probe(sw, mode) => {
            let mut t = Probe::new(geom.clone(), NU, &["ymin"], kin, omega_in, sw).unwrap();
            t.relax_turb = 0.6;
            t.k.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(kin))).unwrap();
            t.omega
                .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(omega_in)))
                .unwrap();
            if mode != SstWallMode::Automatic || sw.k_wall_zero {
                t.set_wall_mode(mode).unwrap();
            }
            T::P(Box::new(t))
        }
    };
    let (mut iterations, mut change, mut du) = (0, f64::INFINITY, f64::INFINITY);
    for it in 0..14000 {
        change = match &mut turb {
            T::L(t) => t.update(&flow.u, &flow.phi),
            T::P(t) => t.update(&flow.u, &flow.phi),
        };
        flow.nu_face = Some(match &turb {
            T::L(t) => t.nu_face(),
            T::P(t) => t.nu_face(),
        });
        let info = flow.iterate();
        iterations = it + 1;
        du = info.max_du;
        if it % 2000 == 0 {
            println!("  it {it} du {du:.2e} dnut {change:.2e}");
        }
        if !info.res_u.is_finite() {
            break;
        }
        if it > 200 && du < 1e-9 && change < 1e-6 {
            break;
        }
    }
    let (kf, wf, nutf, f1f, wd, mlr): (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, f64) = match &turb {
        T::L(t) => (
            t.k.values.clone(),
            t.omega.values.clone(),
            t.nut.clone(),
            t.f1().to_vec(),
            t.wall_distance().to_vec(),
            f64::NAN,
        ),
        T::P(t) => (
            t.k.values.clone(),
            t.omega.values.clone(),
            t.nut.clone(),
            t.f1().to_vec(),
            t.wall_distance().to_vec(),
            t.max_lim_ratio,
        ),
    };
    let wall_nut: Vec<(usize, f64)> = match &turb {
        T::L(t) => t.wall_nut.clone(),
        T::P(t) => t.wall_nut.clone(),
    };
    let wall_faces: Vec<usize> = geom.mesh.patch("ymin").unwrap().faces().collect();
    let mesh = &geom.mesh;
    let cc = mesh.cell_centres();
    let mut cols = Vec::new();
    for (&(f, nutw), &face) in wall_nut.iter().zip(&wall_faces) {
        assert_eq!(f, face);
        let c0 = mesh.owner()[f];
        let x = cc[c0][0];
        let yw = geom.dn[f - geom.n_internal];
        let tau = (NU + nutw) * flow.u.values[c0][0] / yw;
        let mut idx: Vec<usize> = (0..cc.len()).filter(|&c| (cc[c][0] - x).abs() < 1e-9).collect();
        idx.sort_by(|&a, &b| cc[a][1].partial_cmp(&cc[b][1]).unwrap());
        let col: Vec<(f64, f64)> = idx.iter().map(|&c| (cc[c][1], flow.u.values[c][0])).collect();
        let ue = col
            .iter()
            .min_by(|a, b| (a.0 - 0.25).abs().partial_cmp(&(b.0 - 0.25).abs()).unwrap())
            .unwrap()
            .1;
        let theta_to = |ymax: f64| {
            let mut theta = 0.0;
            let (mut y0, mut g0) = (0.0, 0.0);
            for &(y, uu) in &col {
                if y > ymax {
                    break;
                }
                let g = (uu / ue) * (1.0 - uu / ue);
                theta += 0.5 * (g0 + g) * (y - y0);
                y0 = y;
                g0 = g;
            }
            theta
        };
        let u_tau = tau.abs().sqrt();
        cols.push(Col {
            x,
            cf: 2.0 * tau / (ue * ue),
            u_tau,
            re_theta: theta_to(0.15) * ue / NU,
            re_theta_a: theta_to(0.10) * ue / NU,
            re_theta_b: theta_to(0.20) * ue / NU,
            re_theta_edge: {
                // edge: first cell above the nut/nu maximum with nut/nu < 1
                let nuts: Vec<f64> = idx.iter().map(|&c| nutf[c] / NU).collect();
                let imax = (0..nuts.len())
                    .max_by(|&a, &b| nuts[a].partial_cmp(&nuts[b]).unwrap())
                    .unwrap();
                let ie = (imax..nuts.len()).find(|&i| nuts[i] < 1.0).unwrap_or(nuts.len() - 1);
                let (ye, ued) = col[ie];
                let mut theta = 0.0;
                let (mut y0, mut g0) = (0.0, 0.0);
                for &(y, uu) in &col {
                    if y > ye {
                        break;
                    }
                    let g = (uu / ued) * (1.0 - uu / ued);
                    theta += 0.5 * (g0 + g) * (y - y0);
                    y0 = y;
                    g0 = g;
                }
                theta * ued / NU
            },
            re_theta_mi: 0.0,
            cells: idx
                .iter()
                .map(|&c| [cc[c][1], wd[c], flow.u.values[c][0], kf[c], wf[c], nutf[c], f1f[c]])
                .collect(),
        });
    }
    cols.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    {
        // momentum integral of the zero-pressure-gradient plate, theta(x) = int_0^x Cf/2 dx'
        let (mut integ, mut xp, mut cp) = (0.0, 0.0, cols[0].cf);
        for c in cols.iter_mut() {
            integ += 0.5 * (cp + c.cf) * 0.5 * (c.x - xp);
            xp = c.x;
            cp = c.cf;
            c.re_theta_mi = integ / NU;
        }
    }
    Res {
        cols,
        iterations,
        du,
        change,
        max_lim_ratio: mlr,
        first,
    }
}

fn ks(re: f64) -> f64 {
    let l = re.log10();
    1.0 / (17.08 * l * l + 25.11 * l + 6.012)
}

/// interpolate a column quantity at a target Re_theta (or x)
fn at_re(r: &Res, target: f64, sel: impl Fn(&Col) -> f64, q: impl Fn(&Col) -> f64) -> (f64, usize) {
    let i = r.cols.partition_point(|c| sel(c) < target).clamp(1, r.cols.len() - 1);
    let (a, b) = (&r.cols[i - 1], &r.cols[i]);
    let t = (target - sel(a)) / (sel(b) - sel(a));
    (q(a) * (1.0 - t) + q(b) * t, if t < 0.5 { i - 1 } else { i })
}

const CFL3D_CF_097: f64 = 0.00268299;

fn report(tag: &str, r: &Res) {
    let mut s = format!(
        "RES {tag}: its {} conv {} du {:.1e} dnut {:.1e} first {:.3e} |",
        r.iterations,
        r.change < 1e-6 && r.du < 1e-9,
        r.du,
        r.change,
        r.first
    );
    for rt in [5000.0, 7500.0, 10000.0, 12500.0] {
        let (cf, _) = at_re(r, rt, |c| c.re_theta, |c| c.cf);
        s += &format!(" {:+.2}%", 100.0 * (cf / ks(rt) - 1.0));
    }
    for (name, sel) in [
        ("edge-profile", (|c: &Col| c.re_theta_edge) as fn(&Col) -> f64),
        ("momentum-integral", |c: &Col| c.re_theta_mi),
    ] {
        let mut t = format!("    K-S deviation, Re_theta from {name}:");
        for rt in [5000.0, 7500.0, 10000.0, 12500.0] {
            let (cf, _) = at_re(r, rt, sel, |c| c.cf);
            t += &format!(" {:+.2}%", 100.0 * (cf / ks(rt) - 1.0));
        }
        let (xs, _) = at_re(r, 10000.0, sel, |c| c.x);
        let (re_edge_chk, _) = at_re(r, 0.97, |c| c.x, sel);
        t += &format!(" (x at 10000: {xs:.3}; Re_theta(0.97) {re_edge_chk:.0})");
        println!("{t}");
    }
    let (cf10, i10) = at_re(r, 10000.0, |c| c.re_theta, |c| c.cf);
    let (cf097, _) = at_re(r, 0.97, |c| c.x, |c| c.cf);
    let (rt097, _) = at_re(r, 0.97, |c| c.x, |c| c.re_theta);
    s += &format!(
        " | Cf(10000) {cf10:.6e} Cf(0.97) {cf097:.6e} ({:+.2}% vs CFL3D) Re_theta(0.97) {rt097:.0} K-S {:.6e} ({:+.2}%) maxP/lim {:.3}",
        100.0 * (cf097 / CFL3D_CF_097 - 1.0),
        ks(rt097),
        100.0 * (cf097 / ks(rt097) - 1.0),
        r.max_lim_ratio
    );
    println!("{s}");
    // momentum-integral consistency (HA9): theta(x) from the profile against the integral of Cf/2 dx (zero pressure gradient)
    {
        let mut integ = 0.0;
        let (mut xp, mut cp) = (0.0, r.cols[0].cf);
        let mut next = 0;
        let marks = [0.05, 0.1, 0.25, 0.5, 0.97, 1.5, 1.9];
        for c in &r.cols {
            integ += 0.5 * (cp + c.cf) * 0.5 * (c.x - xp);
            xp = c.x;
            cp = c.cf;
            if next < marks.len() && c.x >= marks[next] {
                println!(
                    "    HA9 x {:.3}: Re_theta (profile) {:.0}, Re_theta from int Cf/2 dx {:.0}, Re_x {:.3e}, Cf {:.5e}",
                    c.x,
                    c.re_theta,
                    integ / NU,
                    c.x / NU,
                    c.cf
                );
                next += 1;
            }
        }
    }
    // Re_theta definition variants at x = 0.97 (HA9)
    let (ra, _) = at_re(r, 0.97, |c| c.x, |c| c.re_theta_a);
    let (rb, _) = at_re(r, 0.97, |c| c.x, |c| c.re_theta_b);
    println!(
        "    HA9 Re_theta(0.97) y<=0.10: {ra:.0}, y<=0.15: {rt097:.0}, y<=0.20: {rb:.0}; K-S deviation {:+.2}%, {:+.2}%, {:+.2}%",
        100.0 * (cf097 / ks(ra) - 1.0),
        100.0 * (cf097 / ks(rt097) - 1.0),
        100.0 * (cf097 / ks(rb) - 1.0)
    );
    // near-wall structure at the column of Re_theta 10000 (HA1, HA4, HA6)
    let c = &r.cols[i10];
    println!(
        "    column x {:.4} Re_theta {:.0} u_tau {:.5}",
        c.x, c.re_theta, c.u_tau
    );
    let mut maxrel: f64 = 0.0;
    for cc in &c.cells {
        maxrel = maxrel.max((cc[1] - cc[0]).abs() / cc[0]);
    }
    println!("    HA6 max relative |wall_distance - y| over the column: {maxrel:.2e}");
    for (j, cc) in c.cells.iter().enumerate().take(8) {
        let ana = 6.0 * NU / (0.075 * cc[1] * cc[1]);
        let yp = cc[0] * c.u_tau / NU;
        println!(
            "    cell {j}: y+ {yp:8.3} u+ {:8.3} k+ {:.3e} omega/analytic {:.4} nut/nu {:.3e} F1 {:.4}",
            cc[2] / c.u_tau,
            cc[3] / (c.u_tau * c.u_tau),
            cc[4] / ana,
            cc[5] / NU,
            cc[6]
        );
    }
    let mut line = String::from("    F1 vs y+:");
    for target in [10.0, 30.0, 100.0, 300.0, 1000.0, 3000.0, 10000.0] {
        if let Some(cc) = c.cells.iter().find(|cc| cc[0] * c.u_tau / NU >= target) {
            line += &format!(" [{target:.0}: {:.3}]", cc[6]);
        }
    }
    println!("{line}");
    // log-law slope (y+ 50..300)
    let pts: Vec<(f64, f64)> = c
        .cells
        .iter()
        .map(|cc| (cc[0] * c.u_tau / NU, cc[2] / c.u_tau))
        .filter(|p| p.0 >= 50.0 && p.0 <= 300.0)
        .collect();
    if pts.len() >= 3 {
        let n = pts.len() as f64;
        let (sx, sy): (f64, f64) = pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0.ln(), a.1 + p.1));
        let (sxx, sxy): (f64, f64) = pts
            .iter()
            .fold((0.0, 0.0), |a, p| (a.0 + p.0.ln().powi(2), a.1 + p.0.ln() * p.1));
        let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
        println!(
            "    log fit y+ 50..300: slope {slope:.4} intercept {:.4} ({} pts)",
            (sy - slope * sx) / n,
            pts.len()
        );
    }
}

fn experiments() -> Vec<(&'static str, Kind)> {
    let b = Sw::BASE;
    let a = SstWallMode::Automatic;
    vec![
        ("lib_auto", Kind::Lib(a)),
        ("lib_lowre", Kind::Lib(SstWallMode::LowRe)),
        ("probe_base", Kind::Probe(b, a)),
        ("face1", Kind::Probe(Sw { face_factor: 1.0, ..b }, a)),
        (
            "face100",
            Kind::Probe(
                Sw {
                    face_factor: 100.0,
                    ..b
                },
                a,
            ),
        ),
        (
            "cell1_analytic",
            Kind::Probe(
                Sw {
                    cell1_analytic: true,
                    ..b
                },
                a,
            ),
        ),
        (
            "plain_strain",
            Kind::Probe(
                Sw {
                    plain_wall_strain: true,
                    ..b
                },
                a,
            ),
        ),
        ("k_wall_zero", Kind::Probe(Sw { k_wall_zero: true, ..b }, a)),
        ("cd_off", Kind::Probe(Sw { cd_off: true, ..b }, a)),
        ("limiter_off", Kind::Probe(Sw { limiter_off: true, ..b }, a)),
        ("ls_gradient", Kind::Probe(Sw { ls_gradient: true, ..b }, a)),
        (
            "lowre_ls",
            Kind::Probe(Sw { ls_gradient: true, ..b }, SstWallMode::LowRe),
        ),
    ]
}

/// Runs the experiments named in `W31B_EXP` on the mesh `W31B_MESH` (default m) and prints the results.
#[test]
#[ignore = "diagnostic, minutes: W31B_EXP=lib_auto,face1 W31B_MESH=m cargo test --release -p rcfd-turb --test diag_w31b_sst_probe -- --ignored --nocapture probe_experiments"]
fn probe_experiments() {
    let names = std::env::var("W31B_EXP").unwrap_or_else(|_| "lib_auto".into());
    let mesh = match std::env::var("W31B_MESH").unwrap_or_else(|_| "m".into()).as_str() {
        "c" => MESHES[0],
        "f" => MESHES[2],
        _ => MESHES[1],
    };
    let all = experiments();
    for name in names.split(',') {
        let (n, k) = all
            .iter()
            .find(|e| e.0 == name)
            .unwrap_or_else(|| panic!("unknown experiment {name}"));
        let t0 = std::time::Instant::now();
        let r = run(*k, mesh, 2.25e-7, 125.0);
        report(&format!("{n} {mesh:?} ({:.0} s)", t0.elapsed().as_secs_f64()), &r);
    }
}

/// With every switch off the probe equals the library model (coarse mesh): Cf agrees to 1e-9 relative.
#[test]
#[ignore = "diagnostic, about one minute"]
fn probe_matches_library() {
    let a = run(Kind::Lib(SstWallMode::Automatic), MESHES[0], 2.25e-7, 125.0);
    let b = run(Kind::Probe(Sw::BASE, SstWallMode::Automatic), MESHES[0], 2.25e-7, 125.0);
    let worst = a
        .cols
        .iter()
        .zip(&b.cols)
        .map(|(p, q)| ((p.cf - q.cf) / p.cf).abs())
        .fold(0.0, f64::max);
    println!(
        "probe vs library: iterations {} / {}, worst relative Cf difference {worst:.3e}",
        a.iterations, b.iterations
    );
    assert!(worst < 1e-9);
}

/// HA8 helper: Green-Gauss versus least-squares gradient of u = y^2 + sin(3y) on the coarse plate mesh (max relative difference of du/dy
/// over the first 20 cells of a column and the derivative error of each).
#[test]
#[ignore = "diagnostic, seconds"]
fn gradient_operators_on_stretched_column() {
    use rcfd_fv::gradient::{green_gauss, least_squares};
    let m = MESHES[0];
    let ut_end = flat_plate_u_tau(1.0, NU, 2.0);
    let first = first_cell_height_from_y_plus(m.y_plus, ut_end, NU);
    let g_y = Grading::WallLayers {
        first,
        growth: m.growth,
        layers: m.layers,
        both: false,
    };
    let mesh = box_2d_graded(m.nx, m.ny, 2.0, 1.0, [Grading::Expansion(8.0), g_y], false, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let cc = geom.mesh.cell_centres().to_vec();
    let f = |y: f64| y * y + (3.0 * y).sin();
    let df = |y: f64| 2.0 * y + 3.0 * (3.0 * y).cos();
    let cells: Vec<f64> = cc.iter().map(|c| f(c[1])).collect();
    let ub = VectorField::uniform("U", &geom, [0.0; 3]);
    let _ = ub;
    let bv: Vec<f64> = (geom.n_internal..geom.mesh.n_faces())
        .map(|fc| f(geom.mesh.face_centres()[fc][1]))
        .collect();
    let gg = green_gauss(&geom, &cells, &bv, 0);
    let ls = least_squares(&geom, &cells, &bv);
    let x0 = cc[0][0];
    let mut idx: Vec<usize> = (0..cc.len()).filter(|&c| (cc[c][0] - x0).abs() < 1e-9).collect();
    idx.sort_by(|&a, &b| cc[a][1].partial_cmp(&cc[b][1]).unwrap());
    for &c in idx.iter().take(12) {
        let ex = df(cc[c][1]);
        println!(
            "y {:.3e}: exact {ex:.5} GG {:.5} ({:+.2e}) LS {:.5} ({:+.2e})",
            cc[c][1],
            gg[c][1],
            gg[c][1] / ex - 1.0,
            ls[c][1],
            ls[c][1] / ex - 1.0
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Channel Re_tau 178.12 (set-up of `crates/rcfd-api/tests/validation_channel_re180.rs`, SST, `Symmetric(40)` grading).

fn channel(kind: Kind, ny: usize) -> (f64, f64, f64, usize) {
    use rcfd_fv::simple::BodyForce;
    let re_tau = 178.12;
    let nu = 1.0 / re_tau;
    let mesh = box_2d_graded(
        4,
        ny,
        1.0,
        2.0,
        [Grading::Uniform, Grading::Symmetric(40.0)],
        true,
        false,
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [15.0, 0.0, 0.0]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    s.h_from_solution = true;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, s);
    flow.body_force = Some(BodyForce::uniform(&geom, [1.0, 0.0, 0.0]));
    let mut turb = match kind {
        Kind::Lib(mode) => {
            let mut t = KOmegaSst::new(geom.clone(), nu, &["ymin", "ymax"], 1.0, 1.0 / (10.0 * nu)).unwrap();
            if mode != SstWallMode::Automatic {
                t.set_wall_mode(mode).unwrap();
            }
            T::L(Box::new(t))
        }
        Kind::Probe(sw, mode) => {
            let mut t = Probe::new(geom.clone(), nu, &["ymin", "ymax"], 1.0, 1.0 / (10.0 * nu), sw).unwrap();
            if mode != SstWallMode::Automatic || sw.k_wall_zero {
                t.set_wall_mode(mode).unwrap();
            }
            T::P(Box::new(t))
        }
    };
    let (mut its, mut change) = (0, f64::INFINITY);
    for it in 0..30000 {
        let (ch, nuf) = match &mut turb {
            T::L(s) => (s.update(&flow.u, &flow.phi), s.nu_face()),
            T::P(s) => (s.update(&flow.u, &flow.phi), s.nu_face()),
        };
        change = ch;
        flow.nu_face = Some(nuf);
        let info = flow.iterate();
        its = it + 1;
        if it > 10 && info.max_du < 1e-9 && change < 1e-8 {
            break;
        }
    }
    let cc = geom.mesh.cell_centres();
    let mut pts: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| cc[c][0] < 0.3 && cc[c][1] < 1.0)
        .map(|c| (cc[c][1], flow.u.values[c][0]))
        .collect();
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let n = pts.len();
    let (y1, u1, y0, u0) = (pts[n - 1].0, pts[n - 1].1, pts[n - 2].0, pts[n - 2].1);
    let centre = u1 + (u1 - u0) / (y1 - y0) * (1.0 - y1);
    let vol = geom.mesh.cell_volumes();
    let bulk = (0..vol.len()).map(|c| vol[c] * flow.u.values[c][0]).sum::<f64>() / vol.iter().sum::<f64>();
    let _ = change;
    (pts[0].0 * re_tau, bulk, centre, its)
}

/// Channel SST drift of U_b+ with ny under the wall-omega switches (`W31B_NY`, `W31B_EXP` as above).
#[test]
#[ignore = "diagnostic, seconds to minutes: W31B_NY=40,80,160 W31B_EXP=lib_auto,face1 ..."]
fn channel_experiments() {
    let nys: Vec<usize> = std::env::var("W31B_NY")
        .unwrap_or_else(|_| "40,80".into())
        .split(',')
        .map(|s| s.parse().unwrap())
        .collect();
    let names = std::env::var("W31B_EXP").unwrap_or_else(|_| "lib_auto".into());
    let all = experiments();
    for name in names.split(',') {
        let (n, k) = all
            .iter()
            .find(|e| e.0 == name)
            .unwrap_or_else(|| panic!("unknown experiment {name}"));
        for &ny in &nys {
            let (yp, ub, uc, its) = channel(*k, ny);
            println!(
                "CHSST {n} ny {ny}: first-cell y+ {yp:.3} U_b+ {ub:.4} ({:+.2} % of 15.6787) U_c+ {uc:.4} its {its}",
                100.0 * (ub / 15.6787 - 1.0)
            );
        }
    }
}
