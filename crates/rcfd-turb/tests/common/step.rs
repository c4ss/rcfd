//! Shared backward-facing-step runner for the F-042 / F-043 tests (Driver and Seegmiller geometry,
//! expansion ratio 1.125). ADR 0015 describes the two inlet set-ups.
#![allow(dead_code)]

use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{backward_step_graded, Grading, StepGrading, StepSpec};
use rcfd_turb::sst::{SstVariant, SstWallMode};
use rcfd_turb::{KEpsilon, KEpsilonVariant, KOmegaSst, WallTreatment};
use std::sync::Arc;

pub fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// Registered reattachment window: 6.26 +- 0.10 (Driver and Seegmiller 1985, NASA TMR).
pub const XR_REF: f64 = 6.26;
pub const XR_TOL: f64 = 0.10;

/// Measured profile at x/H = -4 (validation-data/data/backward-step-tmr/profiles.exp.dat, zone
/// "exp, x/H=-4"): (distance above the upstream floor in H, u/Uref, 1000 u'u', 1000 v'v').
pub const EXP_PROFILE: [(f64, f64, f64, f64); 15] = [
    (0.10, 0.657, 5.16, 2.70),
    (0.15, 0.696, 5.26, 2.19),
    (0.20, 0.719, 4.67, 2.23),
    (0.30, 0.760, 3.86, 2.32),
    (0.40, 0.790, 3.49, 2.10),
    (0.50, 0.818, 3.06, 2.05),
    (0.70, 0.870, 2.58, 1.64),
    (1.00, 0.926, 1.80, 1.18),
    (1.40, 0.982, 0.86, 0.63),
    (1.80, 1.003, 0.42, 0.32),
    (2.20, 1.003, 0.39, 0.30),
    (2.60, 1.000, 0.37, 0.28),
    (3.00, 1.001, 0.37, 0.28),
    (4.00, 1.001, 0.37, 0.27),
    (5.00, 1.000, 0.36, 0.26),
];

/// Inlet description.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Inlet {
    /// The original approximation: 1/7 power law, delta 1.5, u_tau 0.038 k profile, 1 percent free-stream
    /// intensity (k = 1.5e-4).
    Old,
    /// BFS-C: measured u, k = (u'u' + 2 v'v')/2 at x/H = -4, epsilon from a mixing length (ADR 0015).
    Measured,
}

#[derive(Clone, Copy, Debug)]
pub enum Model {
    Ke(KEpsilonVariant),
    Sst,
}

#[derive(Clone, Debug)]
pub struct StepCfg {
    pub model: Model,
    pub re_h: f64,
    pub l_in: f64,
    pub l_out: f64,
    pub n: [usize; 4],
    pub grading: StepGrading,
    pub inlet: Inlet,
    /// Free-stream k override (replaces the tabulated / default free-stream value).
    pub k_free: Option<f64>,
    pub turb_conv: ConvScheme,
    pub comp_diag: bool,
    pub nit: usize,
    pub wall_treatment: WallTreatment,
    pub alpha_u: f64,
    pub relax_turb: f64,
    pub verbose: bool,
    /// SST only: model variant and wall mode.
    pub sst_variant: SstVariant,
    pub sst_mode: SstWallMode,
    /// Wall-resolved mesh: first-cell height on the walls (`None`: the wall-function grading of `grading`).
    pub wall_first: Option<f64>,
    /// Realizable `A0` override (`None`: the library default 4.04; ADR 0017).
    pub a0: Option<f64>,
}

pub const MESH_COARSE: [usize; 4] = [16, 107, 27, 13];
pub const MESH_MEDIUM: [usize; 4] = [24, 160, 40, 20];
pub const MESH_FINE: [usize; 4] = [36, 240, 60, 30];

pub fn wf_grading() -> StepGrading {
    StepGrading {
        x_up: Grading::Expansion(1.0 / 3.0),
        x_down: Grading::Expansion(8.0),
        y_step: Grading::Symmetric(2.0),
        y_in: Grading::Symmetric(8.0),
    }
}

impl StepCfg {
    pub fn base(model: Model, inlet: Inlet, n: [usize; 4]) -> Self {
        let measured = inlet == Inlet::Measured;
        StepCfg {
            model,
            re_h: if measured { 36000.0 } else { 37500.0 },
            l_in: if measured { 4.0 } else { 5.0 },
            l_out: 40.0,
            n,
            grading: wf_grading(),
            inlet,
            k_free: None,
            turb_conv: ConvScheme::Upwind,
            comp_diag: false,
            nit: 6000,
            wall_treatment: WallTreatment::Smooth,
            alpha_u: 0.8,
            relax_turb: 0.6,
            verbose: false,
            sst_variant: SstVariant::Sst,
            sst_mode: SstWallMode::Automatic,
            wall_first: None,
            a0: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StepOut {
    pub xr: Option<f64>,
    pub converged_at: Option<usize>,
    pub iterations: usize,
    pub du: f64,
    pub dnut: f64,
    pub u: Vec<[f64; 3]>,
    pub n_cells: usize,
    /// min / max first-cell y+ on the floor downstream of the step (SST only, NaN otherwise).
    pub y_plus_floor: (f64, f64),
}

fn lerp_table(d: f64, col: impl Fn(&(f64, f64, f64, f64)) -> f64) -> f64 {
    let t = &EXP_PROFILE;
    if d <= t[0].0 {
        return col(&t[0]);
    }
    for w in t.windows(2) {
        if d <= w[1].0 {
            let s = (d - w[0].0) / (w[1].0 - w[0].0);
            return col(&w[0]) * (1.0 - s) + col(&w[1]) * s;
        }
    }
    col(&t[t.len() - 1])
}

/// (u, k, epsilon) at distance d from the nearer inlet-channel wall.
pub fn inlet_state(inlet: Inlet, d: f64, k_free: Option<f64>) -> (f64, f64, f64) {
    let (cmu, kappa) = (0.09f64, 0.41f64);
    match inlet {
        Inlet::Old => {
            let d = d.max(1e-6);
            let delta = 1.5;
            let u_tau = 0.038f64;
            let u = if d < delta { (d / delta).powf(1.0 / 7.0) } else { 1.0 };
            let k_floor = k_free.unwrap_or(1.5 * 0.01f64.powi(2));
            let k = if d < delta {
                (u_tau * u_tau / cmu.sqrt() * (1.0 - d / delta).powi(2)).max(k_floor)
            } else {
                k_floor
            };
            let l = (kappa * d).min(0.09 * delta);
            (u, k, cmu.powf(0.75) * k.powf(1.5) / l)
        }
        Inlet::Measured => {
            let d = d.max(1e-6);
            let u = if d < EXP_PROFILE[0].0 {
                EXP_PROFILE[0].1 * (d / EXP_PROFILE[0].0).powf(1.0 / 7.0)
            } else {
                lerp_table(d, |r| r.1).min(1.0)
            };
            let mut k = 0.5 * (lerp_table(d, |r| r.2) + 2.0 * lerp_table(d, |r| r.3)) * 1e-3;
            if let Some(kf) = k_free {
                if d > 2.2 {
                    k = kf;
                }
            }
            let l = (kappa * d).min(0.09 * 1.5);
            (u, k, cmu.powf(0.75) * k.powf(1.5) / l)
        }
    }
}

/// Last negative-to-positive sign change of u in the first cell row above the floor, in step heights.
pub fn reattachment(s: &SimpleSolver, l_in: f64) -> Option<f64> {
    let cc = s.geom.mesh.cell_centres();
    let ymin = cc.iter().map(|c| c[1]).fold(f64::INFINITY, f64::min);
    let mut row: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| (cc[c][1] - ymin).abs() < 1e-9 && cc[c][0] > l_in)
        .map(|c| (cc[c][0], s.u.values[c][0]))
        .collect();
    row.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out = None;
    for w in row.windows(2) {
        if w[0].1 < 0.0 && w[1].1 >= 0.0 {
            let t = -w[0].1 / (w[1].1 - w[0].1);
            out = Some(w[0].0 + t * (w[1].0 - w[0].0) - l_in);
        }
    }
    out
}

enum M {
    K(Box<KEpsilon>),
    S(Box<KOmegaSst>),
}

pub fn run_step(cfg: &StepCfg) -> StepOut {
    let nu = 1.0 / cfg.re_h;
    let spec = StepSpec::from_expansion_ratio(1.125, 1.0, cfg.l_in, cfg.l_out, 0.1, cfg.n).unwrap();
    let grading = match cfg.wall_first {
        None => cfg.grading,
        Some(first) => {
            // up to 20 layers of growth 1.25, fewer when the stack would exceed half the step segment (length 1)
            let mut layers = 20usize;
            while layers > 1 && first * (1.25f64.powi(layers as i32) - 1.0) / 0.25 > 0.5 {
                layers -= 1;
            }
            let wl = |both| Grading::WallLayers {
                first,
                growth: 1.25,
                layers,
                both,
            };
            StepGrading {
                x_up: cfg.grading.x_up,
                x_down: cfg.grading.x_down,
                y_step: wl(false),
                y_in: wl(true),
            }
        }
    };
    let geom = Arc::new(FvGeometry::new(Arc::new(backward_step_graded(spec, grading).unwrap())).unwrap());
    let inlet: Vec<usize> = geom.mesh.patch("inlet").unwrap().faces().collect();
    let st_at: Vec<(f64, f64, f64)> = inlet
        .iter()
        .map(|&f| {
            let y = geom.mesh.face_centres()[f][1] - spec.step;
            let d = y.min(spec.h_in - y);
            inlet_state(cfg.inlet, d, cfg.k_free)
        })
        .collect();
    let prof =
        |f: &dyn Fn(&(f64, f64, f64)) -> f64| -> Vec<[f64; 3]> { st_at.iter().map(|s| [f(s), 0.0, 0.0]).collect() };
    let mut u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    u.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof(&|s| s.0)))))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(nu, Coupling::Simplec);
    st.alpha_u = cfg.alpha_u;
    st.convection = ConvScheme::LinearUpwind;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, st);
    flow.component_diag_correction = cfg.comp_diag;
    let mut model = match cfg.model {
        Model::Ke(v) => {
            let mut m = KEpsilon::new(v, geom.clone(), nu, &["walls"], 1.5e-4, 1e-5).unwrap();
            m.relax_turb = cfg.relax_turb;
            m.wall_treatment = cfg.wall_treatment;
            if let Some(a0) = cfg.a0 {
                m.a0 = a0;
            }
            m.convection = cfg.turb_conv;
            m.k.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof(&|s| s.1)))))
                .unwrap();
            m.eps
                .set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof(&|s| s.2)))))
                .unwrap();
            M::K(Box::new(m))
        }
        Model::Sst => {
            let mut m = KOmegaSst::new(geom.clone(), nu, &["walls"], 1.5e-4, 100.0).unwrap();
            m.relax_turb = cfg.relax_turb;
            m.convection = cfg.turb_conv;
            m.variant = cfg.sst_variant;
            m.set_wall_mode(cfg.sst_mode).unwrap();
            m.k.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof(&|s| s.1)))))
                .unwrap();
            // omega = epsilon / (beta* k)
            m.omega
                .set_bc(
                    &geom,
                    "inlet",
                    Box::new(FixedValue(Profile::PerFace(prof(&|s| s.2 / (0.09 * s.1))))),
                )
                .unwrap();
            M::S(Box::new(m))
        }
    };
    let mut change = f64::INFINITY;
    let mut last = None;
    let mut converged_at = None;
    let mut iterations = 0;
    for it in 1..=cfg.nit {
        change = match &mut model {
            M::K(m) => m.update(&flow.u, &flow.phi),
            M::S(m) => m.update(&flow.u, &flow.phi),
        };
        flow.nu_face = Some(match &model {
            M::K(m) => m.nu_face(),
            M::S(m) => m.nu_face(),
        });
        let info = flow.iterate();
        iterations = it;
        let xr = reattachment(&flow, cfg.l_in);
        if cfg.verbose && (it % env::<usize>("VEVERY", 250) == 0 || it == 1) {
            println!(
                "it {it:5} res_u {:.2e} res_p {:.2e} du {:.2e} dnut {change:.2e} xr/h {xr:?}",
                info.res_u, info.res_p, info.max_du
            );
        }
        last = Some((info, xr));
        if !info.res_u.is_finite() {
            break;
        }
        if it > 100 && info.max_du < 1e-7 && change < 1e-6 {
            converged_at = Some(it);
            break;
        }
    }
    let (info, xr) = last.unwrap();
    let yp = match &model {
        M::S(m) => {
            let v = m.y_plus();
            let wall: Vec<usize> = geom.mesh.patch("walls").unwrap().faces().collect();
            let cc = geom.mesh.face_centres();
            let sel: Vec<f64> = wall
                .iter()
                .zip(v)
                .filter(|(&f, _)| cc[f][1].abs() < 1e-9 && cc[f][0] > cfg.l_in + 1.0)
                .map(|(_, &y)| y)
                .collect();
            (
                sel.iter().cloned().fold(f64::INFINITY, f64::min),
                sel.iter().cloned().fold(0.0, f64::max),
            )
        }
        _ => (f64::NAN, f64::NAN),
    };
    StepOut {
        xr,
        converged_at,
        iterations,
        du: info.max_du,
        dnut: change,
        u: flow.u.values.clone(),
        n_cells: geom.n_cells(),
        y_plus_floor: yp,
    }
}
