//! F-043: k-omega SST (Menter 2003) on the periodic channel at Re_tau = 395, wall-resolved
//! (cell-centre y+ about 1) and with a coarse wall-function-like mesh (y+ about 30).
//!
//! DNS reference: Moser, Kim and Mansour (1999), Phys. Fluids 11, 943, file
//! `turbulence.oden.utexas.edu/data/MKM/chan395/profiles/chan395.means` (actual Re_tau = 392.24;
//! the model is run at the nominal 395, a 0.7 percent difference that moves U+ by less than 0.05).
//! DNS mean velocity U+ interpolated in y+ from that file:
//! y+ 5: 4.82, 10: 8.44, 20: 12.007, 30: 13.493, 50: 14.856, 100: 16.534, 150: 17.646,
//! 200: 18.553, 300: 19.745; centre line U_c+ = 20.13; bulk U_b+ = 17.54 (at 392.24; 17.4 at the
//! nominal 395 for the same Re_b).

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_turb::KOmegaSst;
use std::sync::Arc;

const RE_TAU: f64 = 395.0;
const DNS: [(f64, f64); 9] = [
    (5.0, 4.82),
    (10.0, 8.44),
    (20.0, 12.007),
    (30.0, 13.493),
    (50.0, 14.856),
    (100.0, 16.534),
    (150.0, 17.646),
    (200.0, 18.553),
    (300.0, 19.745),
];

struct Out {
    yplus: Vec<f64>,
    uplus: Vec<f64>,
    centre: f64,
    bulk: f64,
    iterations: usize,
    change: f64,
    y_first: f64,
}

fn run(grading: Grading, ny: usize) -> Out {
    let nu = 1.0 / RE_TAU;
    let mesh = box_2d_graded(4, ny, 1.0, 2.0, [Grading::Uniform, grading], true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [17.0, 0.0, 0.0]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    s.h_from_solution = true;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, s);
    flow.body_force = Some(BodyForce::uniform(&geom, [1.0, 0.0, 0.0]));
    // k = 1, omega = k / (10 nu)
    let mut model = KOmegaSst::new(geom.clone(), nu, &["ymin", "ymax"], 1.0, 1.0 / (10.0 * nu)).unwrap();
    let (mut iterations, mut change) = (0, f64::INFINITY);
    for it in 0..30000 {
        change = model.update(&flow.u, &flow.phi);
        flow.nu_face = Some(model.nu_face());
        let info = flow.iterate();
        iterations = it + 1;
        if it > 10 && info.max_du < 1e-9 && change < 1e-8 {
            break;
        }
    }
    let cc = geom.mesh.cell_centres();
    let mut pts: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| cc[c][0] < 0.3 && cc[c][1] < 1.0)
        .map(|c| (cc[c][1] * RE_TAU, flow.u.values[c][0]))
        .collect();
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let nn = pts.len();
    let (y1, u1) = (pts[nn - 1].0 / RE_TAU, pts[nn - 1].1);
    let (y0, u0) = (pts[nn - 2].0 / RE_TAU, pts[nn - 2].1);
    let centre = u1 + (u1 - u0) / (y1 - y0) * (1.0 - y1);
    // volume-weighted bulk velocity (cells are graded)
    let vol = geom.mesh.cell_volumes();
    let bulk = (0..vol.len()).map(|c| vol[c] * flow.u.values[c][0]).sum::<f64>() / vol.iter().sum::<f64>();
    Out {
        y_first: pts[0].0,
        yplus: pts.iter().map(|p| p.0).collect(),
        uplus: pts.iter().map(|p| p.1).collect(),
        centre,
        bulk,
        iterations,
        change,
    }
}

/// Linear interpolation of U+ at a given y+ from the run.
fn at(o: &Out, yp: f64) -> f64 {
    for i in 0..o.yplus.len() - 1 {
        if o.yplus[i] <= yp && yp <= o.yplus[i + 1] {
            let t = (yp - o.yplus[i]) / (o.yplus[i + 1] - o.yplus[i]);
            return o.uplus[i] * (1.0 - t) + o.uplus[i + 1] * t;
        }
    }
    f64::NAN
}

fn report(name: &str, o: &Out) {
    println!(
        "{name}: first-cell y+ {:.2}, {} its (change {:.1e}), U_c+ = {:.3} (DNS 20.13, {:+.1}%), U_b+ = {:.3} (DNS 17.4, {:+.1}%)",
        o.y_first,
        o.iterations,
        o.change,
        o.centre,
        100.0 * (o.centre / 20.13 - 1.0),
        o.bulk,
        100.0 * (o.bulk / 17.4 - 1.0)
    );
    for (y, d) in DNS {
        let v = at(o, y);
        println!(
            "   y+ {y:6.1}  U+ {v:7.3}  DNS {d:7.3}  ({:+.1}%)",
            100.0 * (v / d - 1.0)
        );
    }
}

#[test]
fn sst_channel_wall_resolved() {
    // half channel 1.0 is 395 wall units; first cell height 2/395 gives a cell-centre y+ of 1
    let o = run(
        Grading::WallLayers {
            first: 2.0 / RE_TAU,
            growth: 1.1,
            layers: 16,
            both: true,
        },
        60,
    );
    report("SST y+~1", &o);
    assert!((o.y_first - 1.0).abs() < 0.01, "y+ {}", o.y_first);
    assert!(o.change < 1e-6, "not converged: {}", o.change);
    assert!((o.centre / 20.13 - 1.0).abs() < 0.10, "centre line {}", o.centre);
    assert!((o.bulk / 17.4 - 1.0).abs() < 0.10, "bulk {}", o.bulk);
    for (y, d) in DNS {
        assert!((at(&o, y) / d - 1.0).abs() < 0.10, "y+ {y}: {} vs {d}", at(&o, y));
    }
}

#[test]
fn sst_channel_coarse_mesh_keeps_the_wall_treatment() {
    // 14 uniform cells: first-cell y+ about 28, where a low-Re condition would fail
    let o = run(Grading::Uniform, 14);
    report("SST y+~28", &o);
    assert!(o.change < 1e-6, "not converged: {}", o.change);
    assert!((o.centre / 20.13 - 1.0).abs() < 0.10, "centre line {}", o.centre);
    assert!((o.bulk / 17.4 - 1.0).abs() < 0.10, "bulk {}", o.bulk);
}
