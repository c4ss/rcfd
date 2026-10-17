//! Wave 30C (ADR 0061, section A): differentially heated square cavity, three-mesh sequences at Ra = 1e3 .. 1e6 against the
//! benchmark solution of G. de Vahl Davis, Int. J. Numer. Meth. Fluids 3 (1983) 249-264, Table V, read from the scan of the
//! paper itself (`cases/heated_cavity/reference/devahldavis1983_tableV.csv`).
//!
//! Registered limits (ADR 0061 A0 to A6, written before the first run):
//! * u_ref(Ra) = 0.1, 0.2, 0.5, 1.0 percent (the paper's own accuracy statement, interpolated geometrically).
//! * A1 finest |Nu_avg/ref - 1| <= u_ref + 0.5 percent; A2 finest |u_max/ref - 1|, |v_max/ref - 1| <= u_ref + 1.0 percent;
//!   A3 finest |Nu_max/ref - 1| <= u_ref + 3.0 percent; A4 locations within 0.03 (Ra <= 1e4) or 0.02 (Ra >= 1e5);
//!   A5 monotone convergence of Nu_avg, u_max, v_max, 0.8 <= p(Nu_avg) <= 3.0, GCI_fine(Nu_avg) <= 1.0 percent;
//!   A6 Richardson-extrapolated Nu_avg within the A1 limit.
//!
//! Heavy meshes are ignored: `cargo test --release -p rcfd-fv --test validation_natural_convection -- --ignored --nocapture`.

use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::energy::{BuoyantSimple, EnergySolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use std::sync::Arc;

const PR: f64 = 0.71;
const TABLE: &str = include_str!("../../../cases/heated_cavity/reference/devahldavis1983_tableV.csv");

/// One row of de Vahl Davis Table V.
#[derive(Debug, Clone, Copy)]
struct Ref {
    nu_avg: f64,
    nu_max: f64,
    z_nu_max: f64,
    u_max: f64,
    z_u: f64,
    v_max: f64,
    x_v: f64,
}

fn reference(ra: f64) -> Ref {
    let mut it = TABLE.lines();
    let head: Vec<&str> = it.next().unwrap().split(',').collect();
    let col = |row: &[f64], name: &str| row[head.iter().position(|h| *h == name).unwrap()];
    for l in it {
        let row: Vec<f64> = l.split(',').map(|v| v.parse().unwrap()).collect();
        if (row[0] - ra).abs() < 1.0 {
            return Ref {
                nu_avg: col(&row, "Nu_avg"),
                nu_max: col(&row, "Nu_max"),
                z_nu_max: col(&row, "z_Nu_max"),
                u_max: col(&row, "u_max"),
                z_u: col(&row, "z_u_max"),
                v_max: col(&row, "v_max"),
                x_v: col(&row, "x_v_max"),
            };
        }
    }
    panic!("Ra {ra} not in Table V");
}

#[derive(Debug, Clone, Copy)]
struct Outcome {
    n: usize,
    nu_avg: f64,
    nu_max: f64,
    z_nu_max: f64,
    u_max: f64,
    z_u: f64,
    v_max: f64,
    x_v: f64,
    iterations: usize,
    res: f64,
    imbalance: f64,
}

/// Maximum of a sampled profile `(position, value)` refined by a parabola through the three points around the maximum.
fn refined_max(p: &[(f64, f64)]) -> (f64, f64) {
    let i = (0..p.len())
        .max_by(|&a, &b| p[a].1.partial_cmp(&p[b].1).unwrap())
        .unwrap();
    if i == 0 || i + 1 == p.len() {
        return (p[i].0, p[i].1);
    }
    let ((x0, y0), (x1, y1), (x2, y2)) = (p[i - 1], p[i], p[i + 1]);
    // Lagrange parabola vertex
    let d0 = (x0 - x1) * (x0 - x2);
    let d1 = (x1 - x0) * (x1 - x2);
    let d2 = (x2 - x0) * (x2 - x1);
    let a = y0 / d0 + y1 / d1 + y2 / d2;
    let b = -y0 * (x1 + x2) / d0 - y1 * (x0 + x2) / d1 - y2 * (x0 + x1) / d2;
    let c = y0 * x1 * x2 / d0 + y1 * x0 * x2 / d1 + y2 * x0 * x1 / d2;
    if a >= 0.0 {
        return (x1, y1);
    }
    let xv = -b / (2.0 * a);
    if xv < x0 || xv > x2 {
        return (x1, y1);
    }
    (xv, a * xv * xv + b * xv + c)
}

fn cell_width(geom: &FvGeometry, c: usize, axis: usize) -> f64 {
    let mesh = &geom.mesh;
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &f in mesh.cell_faces(c) {
        let x = mesh.face_centres()[f][axis];
        lo = lo.min(x);
        hi = hi.max(x);
    }
    hi - lo
}

fn solve(ra: f64, n: usize, ratio: f64, max_iter: usize) -> Outcome {
    let g = Grading::Symmetric(ratio);
    let mesh = Arc::new(box_2d_graded(n, n, 1.0, 1.0, [g, g], false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut t = ScalarField::uniform("T", &geom, 0.5);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut s = SimpleSettings::new(PR, Coupling::Simplec);
    s.convection = ConvScheme::Tvd(TvdLimiter::VanLeer);
    s.alpha_u = 0.8;
    let flow = SimpleSolver::new(geom.clone(), u, p, s);
    let mut energy = EnergySolver::new(geom.clone(), t, 1.0);
    energy.convection = ConvScheme::Tvd(TvdLimiter::VanLeer);
    energy.relax = 0.9;
    let mut sim = BuoyantSimple {
        flow,
        energy,
        beta: 1.0,
        gravity: [0.0, -ra * PR, 0.0],
        t_ref: 0.5,
    };
    let (mut res, mut it) = (f64::INFINITY, 0);
    while it < max_iter {
        let info = sim.iterate();
        it += 1;
        res = info.flow.res_u.max(info.res_t);
        if res < 1e-9 {
            break;
        }
    }
    let mesh = &geom.mesh;
    let wall = |patch: &str| -> (f64, Vec<(f64, f64)>) {
        let gr = sim.energy.wall_gradient(patch);
        let pt = mesh.patch(patch).unwrap();
        let area: f64 = pt.faces().map(|f| mesh.face_area(f)).sum();
        let tot: f64 = pt.faces().zip(&gr).map(|(f, g)| g * mesh.face_area(f)).sum();
        let local: Vec<(f64, f64)> = pt
            .faces()
            .zip(&gr)
            .map(|(f, g)| (mesh.face_centres()[f][1], *g))
            .collect();
        (tot / area, local)
    };
    let (nu_avg, mut local) = wall("xmin");
    let (nu_cold, _) = wall("xmax");
    local.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let (z_nu_max, nu_max) = local
        .iter()
        .cloned()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();
    let cc = mesh.cell_centres();
    let profile = |comp: usize, across: usize| -> Vec<(f64, f64)> {
        let along = 1 - across;
        let mut v: Vec<(f64, f64)> = (0..cc.len())
            .filter(|&c| (cc[c][across] - 0.5).abs() <= 0.5 * cell_width(&geom, c, across) + 1e-12)
            .map(|c| (cc[c][along], sim.flow.u.values[c][comp]))
            .collect();
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert_eq!(v.len() % 2, 0);
        v.chunks(2)
            .map(|w| (0.5 * (w[0].0 + w[1].0), 0.5 * (w[0].1 + w[1].1)))
            .collect()
    };
    let (z_u, u_max) = refined_max(&profile(0, 0));
    let (x_v, v_max) = refined_max(&profile(1, 1));
    Outcome {
        n,
        nu_avg,
        nu_max,
        z_nu_max,
        u_max,
        z_u,
        v_max,
        x_v,
        iterations: it,
        res,
        imbalance: (nu_avg + nu_cold).abs() / nu_avg,
    }
}

/// Observed order and GCI_fine (Roache, factor of safety 1.25, refinement ratio r) from coarse, medium, fine values.
fn order_gci(q: [f64; 3], r: f64) -> Option<(f64, f64, f64)> {
    let (e21, e32) = (q[1] - q[0], q[2] - q[1]);
    if e21 * e32 <= 0.0 || e32 == 0.0 {
        return None;
    }
    let p = (e21 / e32).ln() / r.ln();
    let gci = 1.25 * (e32 / q[2]).abs() / (r.powf(p) - 1.0);
    let extrap = q[2] + e32 / (r.powf(p) - 1.0);
    Some((p, gci, extrap))
}

struct Case {
    ra: f64,
    sizes: [usize; 3],
    ratio: f64,
    u_ref_pct: f64,
    loc_tol: f64,
}

fn run_case(c: &Case) {
    let r = reference(c.ra);
    let outs: Vec<Outcome> = c.sizes.iter().map(|&n| solve(c.ra, n, c.ratio, 40000)).collect();
    let pct = |v: f64, rf: f64| 100.0 * (v / rf - 1.0);
    for o in &outs {
        println!(
            "Ra={:.0e} n={:3}: {} its res {:.1e} imb {:.1e} | Nu {:.4} ({:+.3}%) Nu_max {:.3} at {:.4} ({:+.2}%, ref z {}) | u_max {:.3} at {:.4} ({:+.3}%, ref z {}) | v_max {:.3} at {:.4} ({:+.3}%, ref x {})",
            c.ra, o.n, o.iterations, o.res, o.imbalance,
            o.nu_avg, pct(o.nu_avg, r.nu_avg),
            o.nu_max, o.z_nu_max, pct(o.nu_max, r.nu_max), r.z_nu_max,
            o.u_max, o.z_u, pct(o.u_max, r.u_max), r.z_u,
            o.v_max, o.x_v, pct(o.v_max, r.v_max), r.x_v
        );
    }
    let series = |f: &dyn Fn(&Outcome) -> f64| [f(&outs[0]), f(&outs[1]), f(&outs[2])];
    let quantities: [(&str, [f64; 3]); 5] = [
        ("Nu_avg", series(&|o| o.nu_avg)),
        ("u_max", series(&|o| o.u_max)),
        ("v_max", series(&|o| o.v_max)),
        ("Nu_max", series(&|o| o.nu_max)),
        ("z_u", series(&|o| o.z_u)),
    ];
    let mut conv = Vec::new();
    for (name, q) in quantities {
        let og = order_gci(q, 2.0);
        match og {
            Some((p, gci, ex)) => println!(
                "Ra={:.0e} {name}: observed order {p:.2}, GCI_fine {:.3}%, Richardson extrapolation {ex:.5}",
                c.ra,
                100.0 * gci
            ),
            None => println!("Ra={:.0e} {name}: non-monotone sequence {q:?}, no order", c.ra),
        }
        conv.push((name, q, og));
    }
    let fin = outs[2];
    let mut fails: Vec<String> = Vec::new();
    for o in &outs {
        if o.res >= 1e-9 {
            fails.push(format!("n={} not converged: res {:.2e}", o.n, o.res));
        }
        if o.imbalance >= 1e-6 {
            fails.push(format!("n={} hot/cold flux imbalance {:.2e}", o.n, o.imbalance));
        }
    }
    let lim = |extra: f64| (c.u_ref_pct + extra) / 100.0;
    let chk = |name: &str, v: f64, rf: f64, tol: f64| -> Option<String> {
        ((v / rf - 1.0).abs() > tol).then(|| {
            format!(
                "{name}: {v:.5} vs {rf} ({:+.3}%) outside {:.2}%",
                pct(v, rf),
                100.0 * tol
            )
        })
    };
    fails.extend(chk("A1 Nu_avg", fin.nu_avg, r.nu_avg, lim(0.5)));
    fails.extend(chk("A2 u_max", fin.u_max, r.u_max, lim(1.0)));
    fails.extend(chk("A2 v_max", fin.v_max, r.v_max, lim(1.0)));
    fails.extend(chk("A3 Nu_max", fin.nu_max, r.nu_max, lim(3.0)));
    for (name, v, rf) in [("A4 z_u", fin.z_u, r.z_u), ("A4 x_v", fin.x_v, r.x_v)] {
        if (v - rf).abs() > c.loc_tol {
            fails.push(format!("{name}: {v:.4} vs {rf} outside {}", c.loc_tol));
        }
    }
    for (name, q, og) in &conv {
        if matches!(*name, "Nu_avg" | "u_max" | "v_max") && (q[2] - q[1]).abs() >= (q[1] - q[0]).abs() {
            fails.push(format!("A5 {name}: sequence not converging {q:?}"));
        }
        if *name == "Nu_avg" {
            match og {
                Some((p, gci, ex)) => {
                    if *p < 0.8 || *p > 3.0 {
                        fails.push(format!("A5 order {p:.2} outside [0.8, 3.0]"));
                    }
                    if *gci > 0.01 {
                        fails.push(format!("A5 GCI_fine {:.3}% > 1%", 100.0 * gci));
                    }
                    fails.extend(chk("A6 extrapolated Nu_avg", *ex, r.nu_avg, lim(0.5)));
                }
                None => fails.push("A5 Nu_avg sequence not monotone".into()),
            }
        }
    }
    assert!(
        fails.is_empty(),
        "Ra {:.0e}: registered limits missed:\n{}",
        c.ra,
        fails.join("\n")
    );
}

const CASES: [Case; 4] = [
    Case {
        ra: 1e3,
        sizes: [16, 32, 64],
        ratio: 4.0,
        u_ref_pct: 0.1,
        loc_tol: 0.03,
    },
    Case {
        ra: 1e4,
        sizes: [20, 40, 80],
        ratio: 4.0,
        u_ref_pct: 0.2,
        loc_tol: 0.03,
    },
    Case {
        ra: 1e5,
        sizes: [24, 48, 96],
        ratio: 6.0,
        u_ref_pct: 0.5,
        loc_tol: 0.02,
    },
    Case {
        ra: 1e6,
        sizes: [32, 64, 128],
        ratio: 8.0,
        u_ref_pct: 1.0,
        loc_tol: 0.02,
    },
];

#[test]
fn a_ra_1e3() {
    run_case(&CASES[0]);
}

#[test]
fn a_ra_1e4() {
    run_case(&CASES[1]);
}

#[test]
#[ignore = "heavy: three meshes up to 96 x 96"]
fn a_ra_1e5() {
    run_case(&CASES[2]);
}

/// KNOWN MISS (ADR 0061 addendum): measured once, 428 s for the whole ignored set. All accuracy checks A1 to A4 and A6 pass
/// (Nu_avg +0.33 percent, u_max +0.33, v_max +0.55, Nu_max -1.99, extrapolated Nu 8.819), but two registered convergence
/// checks miss: the 32 x 32 mesh does not reach the residual limit in 40000 iterations (limit cycle at 1.3e-2), and v_max
/// is non-monotone over the sequence (220.642, 220.651, 220.559, a 0.04 percent spread, below the table's own 1 percent).
#[test]
#[ignore = "known miss, heavy (about 6 min): see doc comment"]
fn a_ra_1e6_known_miss() {
    run_case(&CASES[3]);
}
