//! Shared flat-plate runner (TMR 2DZP set-up of `flat_plate_sst.rs`) for the F-043 / F-203 tests, ADR 0015.
#![allow(dead_code)]

use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_mesh::layers::{first_cell_height_from_y_plus, flat_plate_u_tau};
use rcfd_turb::kepsilon::WallFunction;
use rcfd_turb::sst::{SstVariant, SstWallMode};
use rcfd_turb::wall::Roughness;
use rcfd_turb::{KEpsilon, KEpsilonVariant, KOmegaSst};
use std::sync::Arc;

pub fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

#[derive(Clone, Copy, Debug)]
pub enum PlModel {
    Sst { variant: SstVariant, mode: SstWallMode },
    Ke { variant: KEpsilonVariant, wf: WallFunction },
}

#[derive(Clone, Debug)]
pub struct PlateCfg {
    pub model: PlModel,
    pub nu: f64,
    pub lx: f64,
    pub ly: f64,
    pub nx: usize,
    pub ny: usize,
    pub layers: usize,
    pub growth: f64,
    /// Target first-cell y+ at x = lx (friction velocity of White's law), or `None` with `first` set.
    pub y_plus: Option<f64>,
    /// First cell height when no target y+ is given.
    pub first: f64,
    /// Free-stream k and (omega for SST, epsilon for k-epsilon).
    pub k_in: f64,
    pub second_in: f64,
    pub ks: f64,
    pub nit: usize,
    pub comp_diag: bool,
    pub verbose: bool,
}

impl PlateCfg {
    /// TMR set-up: Re = 5e6 per unit length, k = 2.25e-7, omega = 125.
    pub fn tmr(model: PlModel, y_plus: f64, layers: usize, ny: usize) -> Self {
        let (k_in, second_in) = match model {
            PlModel::Sst { .. } => (2.25e-7, 125.0),
            // epsilon equivalent to omega = 125: beta* k omega
            PlModel::Ke { .. } => (2.25e-7, 0.09 * 2.25e-7 * 125.0),
        };
        PlateCfg {
            model,
            nu: 1.0 / 5.0e6,
            lx: 2.0,
            ly: 1.0,
            nx: env("NX", 140),
            ny,
            layers,
            growth: 1.1,
            y_plus: Some(y_plus),
            first: 0.0,
            k_in,
            second_in,
            ks: 0.0,
            nit: env("NIT", 8000),
            comp_diag: false,
            verbose: env("VERBOSE", 0) == 1,
        }
    }
}

pub struct Plate {
    /// (x of the cell centre, Cf, first-cell y+ from the wall shear)
    pub cf: Vec<(f64, f64, f64)>,
    /// (x_lo, x_hi, tau) of every wall face, sorted
    pub faces: Vec<(f64, f64, f64)>,
    pub iterations: usize,
    pub du: f64,
    pub change: f64,
}

impl Plate {
    pub fn at(&self, x: f64) -> (f64, f64) {
        let i = self.cf.partition_point(|c| c.0 < x).clamp(1, self.cf.len() - 1);
        let (a, b) = (self.cf[i - 1], self.cf[i]);
        let t = (x - a.0) / (b.0 - a.0);
        (a.1 * (1.0 - t) + b.1 * t, a.2 * (1.0 - t) + b.2 * t)
    }

    /// Plate-average friction coefficient over [0, l] (wall-face shear times width, partial cell clipped).
    pub fn cf_avg(&self, l: f64) -> f64 {
        let mut f = 0.0;
        for &(x0, x1, tau) in &self.faces {
            if x0 >= l {
                break;
            }
            f += tau * (x1.min(l) - x0);
        }
        2.0 * f / l
    }

    pub fn converged(&self) -> bool {
        self.change < 1e-6 && self.du < 1e-9
    }
}

pub fn run_plate(cfg: &PlateCfg) -> Plate {
    let nu = cfg.nu;
    let first = match cfg.y_plus {
        Some(yp) => {
            let ut_end = flat_plate_u_tau(1.0, nu, cfg.lx);
            first_cell_height_from_y_plus(yp, ut_end, nu)
        }
        None => cfg.first,
    };
    let g_y = Grading::WallLayers {
        first,
        growth: cfg.growth,
        layers: cfg.layers,
        both: false,
    };
    let mesh = box_2d_graded(
        cfg.nx,
        cfg.ny,
        cfg.lx,
        cfg.ly,
        [Grading::Expansion(8.0), g_y],
        false,
        false,
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(nu, Coupling::Simplec);
    st.alpha_u = env("AU", 0.8);
    st.convection = ConvScheme::LinearUpwind;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, st);
    flow.component_diag_correction = cfg.comp_diag;
    enum M {
        S(Box<KOmegaSst>),
        K(Box<KEpsilon>),
    }
    let rough = if cfg.ks > 0.0 {
        Roughness::new(cfg.ks, 0.41, 9.8)
    } else {
        Roughness::SMOOTH
    };
    let mut model = match cfg.model {
        PlModel::Sst { variant, mode } => {
            let mut m = KOmegaSst::new(geom.clone(), nu, &["ymin"], cfg.k_in, cfg.second_in).unwrap();
            m.relax_turb = env("AT", 0.6);
            m.variant = variant;
            m.roughness = rough;
            m.set_wall_mode(mode).unwrap();
            m.k.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(cfg.k_in)))
                .unwrap();
            m.omega
                .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(cfg.second_in)))
                .unwrap();
            M::S(Box::new(m))
        }
        PlModel::Ke { variant, wf } => {
            let mut m = KEpsilon::new(variant, geom.clone(), nu, &["ymin"], cfg.k_in, cfg.second_in).unwrap();
            m.relax_turb = env("AT", 0.6);
            m.wall_function = wf;
            m.roughness = rough;
            m.k.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(cfg.k_in)))
                .unwrap();
            m.eps
                .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(cfg.second_in)))
                .unwrap();
            M::K(Box::new(m))
        }
    };
    let (mut iterations, mut change, mut du) = (0, f64::INFINITY, f64::INFINITY);
    for it in 0..cfg.nit {
        change = match &mut model {
            M::S(m) => m.update(&flow.u, &flow.phi),
            M::K(m) => m.update(&flow.u, &flow.phi),
        };
        flow.nu_face = Some(match &model {
            M::S(m) => m.nu_face(),
            M::K(m) => m.nu_face(),
        });
        let info = flow.iterate();
        iterations = it + 1;
        du = info.max_du;
        if cfg.verbose && it % 1000 == 0 {
            println!("it {it} du {du:.2e} dnut {change:.2e}");
        }
        if !info.res_u.is_finite() {
            break;
        }
        if it > 200 && du < 1e-9 && change < 1e-6 {
            break;
        }
    }
    let wall: Vec<usize> = geom.mesh.patch("ymin").unwrap().faces().collect();
    let wall_nut: &[(usize, f64)] = match &model {
        M::S(m) => &m.wall_nut,
        M::K(m) => &m.wall_nut,
    };
    let pts = geom.mesh.points();
    let mut cf = Vec::new();
    let mut faces = Vec::new();
    for (&(f, nutw), &face) in wall_nut.iter().zip(&wall) {
        assert_eq!(f, face);
        let c = geom.mesh.owner()[f];
        let y = geom.dn[f - geom.n_internal];
        let uu = flow.u.values[c][0];
        let tau = (nu + nutw) * uu / y;
        let ut = tau.abs().sqrt();
        cf.push((geom.mesh.cell_centres()[c][0], 2.0 * tau, ut * y / nu));
        let xs: Vec<f64> = geom.mesh.face_points(f).iter().map(|&i| pts[i][0]).collect();
        let (x0, x1) = (
            xs.iter().cloned().fold(f64::INFINITY, f64::min),
            xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        faces.push((x0, x1, tau));
    }
    cf.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    faces.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    Plate {
        cf,
        faces,
        iterations,
        du,
        change,
    }
}

/// White's formula (TMR theory 3, "6-134").
pub fn white(re_x: f64) -> f64 {
    0.455 / (0.06 * re_x).ln().powi(2)
}

/// Power law 0.0576 Re_x^-0.2.
pub fn power(re_x: f64) -> f64 {
    0.0576 * re_x.powf(-0.2)
}

/// Prandtl-Schlichting drag law C_F = 0.455 / (log10 Re_L)^2.58.
pub fn prandtl_schlichting(re_l: f64) -> f64 {
    0.455 / re_l.log10().powf(2.58)
}

/// Schlichting fully rough local friction coefficient (2.87 + 1.58 log10(x/ks))^-2.5.
pub fn fully_rough(x_over_ks: f64) -> f64 {
    (2.87 + 1.58 * x_over_ks.log10()).powf(-2.5)
}

type Slot = Arc<std::sync::OnceLock<Arc<Plate>>>;

/// Run (once per process and key) a plate configuration; concurrent tests with the same key share the result.
pub fn cached_plate(key: &str, cfg: &PlateCfg) -> Arc<Plate> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
    let slot: Slot = {
        let mut m = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
        m.entry(key.to_string()).or_default().clone()
    };
    slot.get_or_init(|| {
        let p = run_plate(cfg);
        println!(
            "[{key}] {} iterations, du {:.2e}, dnut {:.2e}, converged {}",
            p.iterations,
            p.du,
            p.change,
            p.converged()
        );
        Arc::new(p)
    })
    .clone()
}
