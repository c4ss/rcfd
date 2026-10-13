//! F-043/F-108: k-omega SST on a zero-pressure-gradient flat plate, wall-resolved mesh built from a
//! target y+ with `Grading::WallLayers`.
//!
//! Set-up after the NASA TMR 2D zero pressure gradient flat plate case
//! (tmbwg.github.io/turbmodels/flatplate_sst.html): Re = 5e6 per unit length (U = 1,
//! nu = 2e-7), plate from x = 0 to 2 (Re_x up to 1e7), domain height 1, TMR freestream
//! k = 9e-9 a^2 = 2.25e-7 U^2 (M = 0.2) and omega = 125 U / L (nu_t/nu = 0.009), symmetry top,
//! fixed-pressure outlet. The plate is turbulent from the leading edge.
//!
//! References: the TMR page quotes Cf(x = 0.97) = 0.00268299 for CFL3D with the SST model on its
//! 273 x 193 grid; the two correlations of docs/reference-data.md are White's
//! `Cf = 0.455 / ln^2(0.06 Re_x)` and the power law `Cf = 0.0576 Re_x^-0.2`, which themselves differ
//! by 8.7 percent at Re_x = 5e6 (SST lies between them).

use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_mesh::layers::{first_cell_height_from_y_plus, flat_plate_u_tau};
use rcfd_turb::KOmegaSst;
use std::sync::Arc;

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

fn run(target_y_plus: f64, growth: f64, layers: usize, ny: usize, nx: usize, nit: usize) -> Plate {
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
    let mut model = KOmegaSst::new(geom.clone(), nu, &["ymin"], env("K0", 2.25e-7), 125.0).unwrap();
    model.relax_turb = env("AT", 0.6);
    if env::<String>("TCONV", "up".into()) == "lu" {
        model.convection = ConvScheme::LinearUpwind;
    }
    model
        .k
        .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(env("K0", 2.25e-7))))
        .unwrap();
    model
        .omega
        .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(125.0)))
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

#[test]
#[ignore = "heavy: wall-resolved flat plate, run in release"]
fn sst_flat_plate_cf() {
    let (ny, layers) = (env("NY", 110), env("LAYERS", 70));
    let o = run(
        env("YP", 0.5),
        env("GROWTH", 1.1),
        layers,
        ny,
        env("NX", 140),
        env("NIT", 8000),
    );
    println!("{} its, du {:.2e}, dnut {:.2e}", o.iterations, o.du, o.change);
    let (cf97, _) = at(&o, 0.97);
    println!(
        "Cf(0.97) = {cf97:.6} (TMR CFL3D SST 0.002683: {:+.1}%)",
        100.0 * (cf97 / 0.00268299 - 1.0)
    );
    let mut worst: f64 = 0.0;
    for &x in &[0.4, 0.6, 1.0, 1.4, 2.0 - 0.01] {
        let re_x = x * 5.0e6;
        let (cf, yp) = at(&o, x);
        println!(
            "x {x:.2} Re_x {re_x:.2e} Cf {cf:.5} White {:.5} ({:+.1}%) power-law {:.5} ({:+.1}%) first-cell y+ {yp:.2}",
            white(re_x),
            100.0 * (cf / white(re_x) - 1.0),
            power(re_x),
            100.0 * (cf / power(re_x) - 1.0)
        );
        if x <= 1.0 {
            worst = worst.max((cf / power(re_x) - 1.0).abs());
        }
    }
    assert!(
        o.change < 1e-6 && o.du < 1e-9,
        "not converged: du {:.2e} dnut {:.2e}",
        o.du,
        o.change
    );
    assert!((cf97 / 0.00268299 - 1.0).abs() < 0.05, "Cf(0.97) {cf97}");
    assert!(
        worst < 0.05,
        "Cf deviates {worst} from the power-law correlation for Re_x <= 5e6"
    );
}
