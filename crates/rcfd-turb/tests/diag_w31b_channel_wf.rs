//! Wave 31B (ADR 0063, part B): why the standard k-epsilon wall-function result depends on the wall mesh in the Re_tau 178 channel
//! (set-up of `crates/rcfd-api/tests/validation_channel_re180.rs`: body force 1 on half height 1, nu = 1/178.12, uniform mesh, so
//! u_tau = 1). Ignored diagnostics, no limits, prints only.
//!
//! `W31B_NY=6,8,12` and `W31B_CFG=std_smooth,std_switch,std_hyst,scalable` select meshes and wall treatments.
//! Per run: wall-cell y+ (y Re_tau), y* = Cmu^1/4 sqrt(k) y / nu of the wall cell, the wall-face eddy viscosity, the wall-cell
//! production and dissipation imposed by `wall_cell_sources_ext` relative to the log-law values (G_log = u_tau^3/(kappa y),
//! eps_log = u_tau^3/(kappa y)), and the bulk velocity against the DNS value 15.6787 (MKM Re_tau 178).

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_turb::kepsilon::{wall_cell_sources_ext, WallFunction, WallTreatment};
use rcfd_turb::wall::Roughness;
use rcfd_turb::{KEpsilon, KEpsilonVariant};
use std::sync::Arc;

const RE_TAU: f64 = 178.12;
const UB_DNS: f64 = 15.6787;

fn run(ny: usize, wf: WallFunction, tr: WallTreatment) -> (f64, f64, f64, f64, f64, f64, f64, usize, f64) {
    let nu = 1.0 / RE_TAU;
    let mesh = box_2d_graded(4, ny, 1.0, 2.0, [Grading::Uniform, Grading::Uniform], true, false).unwrap();
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
    let mut m = KEpsilon::new(
        KEpsilonVariant::Standard,
        geom.clone(),
        nu,
        &["ymin", "ymax"],
        1.0,
        0.36,
    )
    .unwrap();
    m.wall_function = wf;
    m.wall_treatment = tr;
    let (mut iterations, mut change) = (0, f64::INFINITY);
    for it in 0..30000 {
        change = m.update(&flow.u, &flow.phi);
        flow.nu_face = Some(m.nu_face());
        let info = flow.iterate();
        iterations = it + 1;
        if it > 10 && info.max_du < 1e-9 && change < 1e-8 {
            break;
        }
    }
    let _ = change;
    let cc = geom.mesh.cell_centres();
    let vol = geom.mesh.cell_volumes();
    let bulk = (0..vol.len()).map(|c| vol[c] * flow.u.values[c][0]).sum::<f64>() / vol.iter().sum::<f64>();
    // wall-adjacent cell on the lower wall
    let c0 = (0..cc.len())
        .filter(|&c| cc[c][0] < 0.3 && cc[c][1] < 1.0)
        .min_by(|&a, &b| cc[a][1].partial_cmp(&cc[b][1]).unwrap())
        .unwrap();
    let y = cc[c0][1];
    let k = m.k.values[c0];
    let ystar = m.wc.cmu.powf(0.25) * k.sqrt() * y / nu;
    let nutw = m
        .wall_nut
        .iter()
        .find(|&&(f, _)| geom.mesh.owner()[f] == c0)
        .map(|&(_, v)| v)
        .unwrap_or(f64::NAN);
    let (g, eps) = wall_cell_sources_ext(
        tr,
        wf,
        &Roughness::SMOOTH,
        &mut true,
        &m.wc,
        nu,
        k,
        y,
        flow.u.values[c0][0],
    );
    let g_log = 1.0 / (m.wc.kappa * y);
    (
        y * RE_TAU,
        ystar,
        nutw / nu,
        g / g_log,
        eps / g_log,
        bulk,
        k,
        iterations,
        change,
    )
}

fn cfgs() -> Vec<(&'static str, WallFunction, WallTreatment)> {
    vec![
        ("std_smooth", WallFunction::Standard, WallTreatment::Smooth),
        ("std_switch", WallFunction::Standard, WallTreatment::Switch),
        (
            "std_hyst",
            WallFunction::Standard,
            WallTreatment::Hysteresis { band: 0.1 },
        ),
        ("scalable", WallFunction::Scalable, WallTreatment::Smooth),
    ]
}

#[test]
#[ignore = "diagnostic: W31B_NY=6,8,12 W31B_CFG=std_smooth,scalable cargo test --release -p rcfd-turb --test diag_w31b_channel_wf -- --ignored --nocapture"]
fn channel_wall_function_sequences() {
    let nys: Vec<usize> = std::env::var("W31B_NY")
        .unwrap_or_else(|_| "4,5,6,8,10,12,14,16,20,24,32".into())
        .split(',')
        .map(|s| s.parse().unwrap())
        .collect();
    let names = std::env::var("W31B_CFG").unwrap_or_else(|_| "std_smooth,scalable".into());
    for name in names.split(',') {
        let (n, wf, tr) = *cfgs().iter().find(|c| c.0 == name).unwrap();
        for &ny in &nys {
            let (yp, ys, nutw, g, eps, ub, k, its, ch) = run(ny, wf, tr);
            println!(
                "CH {n} ny {ny:3}: y+ {yp:6.2} y* {ys:6.2} nut_w/nu {nutw:7.3} G/G_log {g:6.3} eps/eps_log {eps:6.3} k_c {k:.3} U_b+ {ub:7.3} ({:+6.1} %) its {its} change {ch:.1e}",
                100.0 * (ub / UB_DNS - 1.0)
            );
        }
    }
}
