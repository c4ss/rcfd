//! F-041: Spalart-Allmaras (SA-noft2) on the NASA TMR zero-pressure-gradient flat plate. Checks S1-S6 of ADR 0014
//! (written before this file was run; limits are not changed after a run).
//!
//! Set-up as `flat_plate_sst.rs`: Re = 5e6/m (U = 1, nu = 2e-7), plate x = 0..2 turbulent from the leading edge (no
//! upstream symmetry segment, unlike the TMR grid), height 1, symmetry top, fixed-pressure outlet, nuT_inf = 3 nu,
//! 140 x 110 cells with `Grading::WallLayers` target y+ 0.5 (defined at x = 2), growth 1.1, 70 layers.
//!
//! Reference values (docs/reference-data.md, ADR 0014): TMR CFL3D SA file `FlatPlate/SA/cf_cfl3d.dat`, Cf(0.97)
//! interpolated 2.70565e-3; TMR theory curves of `cf_as_function_of_x.dat` at x = 0.97; power law and White.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_mesh::layers::{first_cell_height_from_y_plus, flat_plate_u_tau};
use rcfd_turb::{SaWallMode, SpalartAllmaras};
use std::sync::{Arc, OnceLock};

fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

struct Plate {
    /// (x, Cf, first-cell y+)
    cf: Vec<(f64, f64, f64)>,
    iterations: usize,
    du: f64,
    change: f64,
}

fn run(target_y_plus: f64, growth: f64, layers: usize, ny: usize, nx: usize, nit: usize, mode: SaWallMode) -> Plate {
    let (re_per_l, lx, ly) = (5.0e6, 2.0, 1.0);
    let nu = 1.0 / re_per_l;
    // friction velocity at the downstream end (smallest u_tau * 1 ... largest y+): target y+ there
    let ut_end = flat_plate_u_tau(1.0, nu, lx);
    let h1 = first_cell_height_from_y_plus(target_y_plus, ut_end, nu);
    let g_y = Grading::WallLayers {
        first: h1,
        growth,
        layers,
        both: false,
    };
    let mesh = box_2d_graded(nx, ny, lx, ly, [Grading::Expansion(8.0), g_y], false, false).unwrap();
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
    let nt_in = 3.0 * nu;
    let mut model = SpalartAllmaras::new(geom.clone(), nu, &["ymin"], nt_in).unwrap();
    model.relax_turb = env("AT", 0.6);
    model.wall_mode = mode;
    model
        .nutilde
        .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(nt_in)))
        .unwrap();
    let (mut iterations, mut change, mut du) = (0, f64::INFINITY, f64::INFINITY);
    for it in 0..nit {
        change = model.update(&flow.u, &flow.phi);
        flow.nu_face = Some(model.nu_face());
        let info = flow.iterate();
        iterations = it + 1;
        du = info.max_du;
        if it % env("EVERY", 1000) == 0 {
            println!("it {it} du {du:.2e} dnut {change:.2e}");
        }
        if !info.res_u.is_finite() {
            break;
        }
        if it > 200 && du < 1e-9 && change < 1e-6 {
            break;
        }
    }
    // wall shear from the wall-face viscosity and the first-cell velocity
    let wall: Vec<usize> = geom.mesh.patch("ymin").unwrap().faces().collect();
    let mut cf = Vec::new();
    for (&(f, nutw), &face) in model.wall_nut.iter().zip(&wall) {
        assert_eq!(f, face);
        let c = geom.mesh.owner()[f];
        let y = geom.dn[f - geom.n_internal];
        let uu = flow.u.values[c][0];
        let tau = (nu + nutw) * uu / y;
        let ut = tau.sqrt();
        cf.push((geom.mesh.cell_centres()[c][0], 2.0 * tau, ut * y / nu));
    }
    cf.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    Plate {
        cf,
        iterations,
        du,
        change,
    }
}

fn at(p: &Plate, x: f64) -> (f64, f64) {
    let i = p.cf.partition_point(|c| c.0 < x).clamp(1, p.cf.len() - 1);
    let (a, b) = (p.cf[i - 1], p.cf[i]);
    let t = (x - a.0) / (b.0 - a.0);
    (a.1 * (1.0 - t) + b.1 * t, a.2 * (1.0 - t) + b.2 * t)
}

fn white(re_x: f64) -> f64 {
    0.455 / (0.06 * re_x).ln().powi(2)
}

fn power(re_x: f64) -> f64 {
    0.0576 * re_x.powf(-0.2)
}

const TMR_SA_CF_097: f64 = 2.70565e-3;
const TMR_THEORY_097: [f64; 3] = [2.72379e-3, 2.88302e-3, 2.87459e-3];

fn plate_params(kind: &str) -> (f64, usize, usize, SaWallMode) {
    // (target y+, layers, ny, mode)
    match kind {
        "wf" => (30.0, 40, 60, SaWallMode::Auto),
        "res" => (0.5, 70, 110, SaWallMode::Resolved),
        _ => (0.5, 70, 110, SaWallMode::Auto),
    }
}

fn build(kind: &str) -> Plate {
    let (yp, layers, ny, mode) = plate_params(kind);
    let o = run(yp, 1.1, layers, ny, env("NX", 140), env("NIT", 8000), mode);
    println!("[{kind}] {} its, du {:.2e}, dnut {:.2e}", o.iterations, o.du, o.change);
    o
}

fn plate() -> &'static Plate {
    static P: OnceLock<Plate> = OnceLock::new();
    P.get_or_init(|| build("auto"))
}
fn plate_wf() -> &'static Plate {
    static P: OnceLock<Plate> = OnceLock::new();
    P.get_or_init(|| build("wf"))
}
fn plate_res() -> &'static Plate {
    static P: OnceLock<Plate> = OnceLock::new();
    P.get_or_init(|| build("res"))
}

fn converged(o: &Plate) {
    assert!(
        o.change < 1e-6 && o.du < 1e-9,
        "not converged: du {:.2e} dnut {:.2e}",
        o.du,
        o.change
    );
}

fn report(o: &Plate, label: &str) {
    for &x in &[0.4, 0.6, 0.97, 1.0, 1.4, 1.99] {
        let re_x = x * 5.0e6;
        let (cf, yp) = at(o, x);
        println!(
            "[{label}] x {x:.2} Cf {cf:.6} White {:+.1}% power {:+.1}% first-cell y+ {yp:.2}",
            100.0 * (cf / white(re_x) - 1.0),
            100.0 * (cf / power(re_x) - 1.0)
        );
    }
    let (cf97, yp97) = at(o, 0.97);
    println!(
        "[{label}] Cf(0.97) = {cf97:.6}: TMR SA {:+.2}%, theory 1/2/3 {:+.2}% {:+.2}% {:+.2}%, y+ {yp97:.2}",
        100.0 * (cf97 / TMR_SA_CF_097 - 1.0),
        100.0 * (cf97 / TMR_THEORY_097[0] - 1.0),
        100.0 * (cf97 / TMR_THEORY_097[1] - 1.0),
        100.0 * (cf97 / TMR_THEORY_097[2] - 1.0)
    );
}

#[test]
#[ignore = "heavy: wall-resolved flat plate, run in release"]
fn sa_flat_plate_report() {
    let o = plate();
    report(o, "auto y+0.5");
    converged(o);
}

/// S1
#[test]
#[ignore = "heavy: wall-resolved flat plate, run in release"]
fn sa_flat_plate_cf_vs_tmr() {
    let o = plate();
    converged(o);
    let (cf97, _) = at(o, 0.97);
    assert!((cf97 / TMR_SA_CF_097 - 1.0).abs() < 0.05, "Cf(0.97) {cf97}");
}

/// S2. KNOWN MISS: measured +0.8 / +2.7 / +5.3 % at x = 0.4 / 0.6 / 1.0, the registered 5 % band is not widened.
#[test]
#[ignore = "KNOWN MISS: SA Cf is +5.3 % above the power law at x = 1.0 (Re_x = 5e6), registered band 5 %"]
fn sa_flat_plate_cf_vs_power_law_known_miss() {
    let o = plate();
    converged(o);
    for &x in &[0.4, 0.6, 1.0] {
        let (cf, _) = at(o, x);
        let dev = cf / power(x * 5.0e6) - 1.0;
        assert!(
            dev.abs() < 0.05,
            "x {x}: Cf {cf} deviates {:+.2}% from the power law",
            100.0 * dev
        );
    }
}

/// S3
#[test]
#[ignore = "heavy: wall-resolved flat plate, run in release"]
fn sa_flat_plate_cf_vs_white() {
    let o = plate();
    converged(o);
    for &x in &[0.4, 0.6, 0.97, 1.0] {
        let (cf, _) = at(o, x);
        let dev = cf / white(x * 5.0e6) - 1.0;
        assert!(
            dev.abs() < 0.05,
            "x {x}: Cf {cf} deviates {:+.2}% from White",
            100.0 * dev
        );
    }
}

/// S4
#[test]
#[ignore = "heavy: wall-resolved flat plate, run in release"]
fn sa_flat_plate_cf_vs_tmr_theory_curves() {
    let o = plate();
    converged(o);
    let (cf97, _) = at(o, 0.97);
    for (i, t) in TMR_THEORY_097.iter().enumerate() {
        let dev = cf97 / t - 1.0;
        assert!(
            dev.abs() < 0.05,
            "theory {}: Cf {cf97} deviates {:+.2}%",
            i + 1,
            100.0 * dev
        );
    }
}

/// S5
#[test]
#[ignore = "heavy: wall-function mesh, run in release"]
fn sa_flat_plate_wall_function_mesh_cf_vs_tmr() {
    let o = plate_wf();
    report(o, "auto y+30");
    converged(o);
    let (cf97, yp) = at(o, 0.97);
    assert!(
        (20.0..=60.0).contains(&yp),
        "first-cell y+ {yp} outside the valid band: not applicable"
    );
    assert!((cf97 / TMR_SA_CF_097 - 1.0).abs() < 0.10, "Cf(0.97) {cf97}");
}

/// S6
#[test]
#[ignore = "heavy: wall-resolved flat plate, run in release"]
fn sa_flat_plate_resolved_mode_cf_vs_tmr() {
    let o = plate_res();
    report(o, "resolved y+0.5");
    converged(o);
    let (cf97, _) = at(o, 0.97);
    assert!((cf97 / TMR_SA_CF_097 - 1.0).abs() < 0.05, "Cf(0.97) {cf97}");
}
