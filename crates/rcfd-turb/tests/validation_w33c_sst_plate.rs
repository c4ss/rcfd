//! Wave 33C (ADR 0070, part 2): SST zero-pressure-gradient flat plate against the NASA TMR 2DZP SST data.
//!
//! Set-up: the runner of `validation_flat_plate_wall_law.rs` (U = 1, nu = 2e-7, plate x in [0, 2], height 1, symmetry top, Dirichlet
//! inlet k = 2.25e-7, omega = 125), three meshes 70x55, 140x110, 280x220 (first-cell y+ 1, 0.5, 0.25), each run once for the whole file.
//! References (vendored in `validation-data/reference/w33c_tmr_sst_*.csv`, read from https://tmbwg.github.io/turbmodels/flatplate_sst.html
//! and its data files, 2026-10-18): CFL3D SST-Vm u+(y+) at x = 0.97008, CFL3D Cf(x) on the 545 x 385 grid (compressible, M = 0.2),
//! the incompressible FUN3D / SC-Tetra / GHOST Cf(x = 0.97008) sequences.
//!
//! Heavy (tens of minutes): `cargo test --release -p rcfd-turb --test validation_w33c_sst_plate -- --ignored --nocapture --test-threads=1`.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_mesh::layers::{first_cell_height_from_y_plus, flat_plate_u_tau};
use rcfd_turb::KOmegaSst;
use std::sync::{Arc, OnceLock};

const NU: f64 = 2.0e-7;
const X_REF: f64 = 0.970084;
const TMR_UPLUS: &str = include_str!("../../../validation-data/reference/w33c_tmr_sst_uplus_yplus_x0p97008.csv");
const TMR_CF_X: &str = include_str!("../../../validation-data/reference/w33c_tmr_sst_cf_x_cfl3d_545x385.csv");
const TMR_CF_CONV: &str = include_str!("../../../validation-data/reference/w33c_tmr_sst_cf_convergence_x0p97008.csv");

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

#[derive(Clone, Debug)]
struct Column {
    x: f64,
    cf: f64,
    u_tau: f64,
    /// (y of the cell centre, u) from the wall cell upwards
    u: Vec<(f64, f64)>,
}

struct Run {
    cols: Vec<Column>,
    iterations: usize,
    du: f64,
    change: f64,
}

impl Run {
    fn converged(&self) -> bool {
        self.change < 1e-6 && self.du < 1e-9
    }

    /// Cf at x, linear between the bracketing column centres.
    fn cf_at(&self, x: f64) -> f64 {
        let i = self.cols.partition_point(|c| c.x < x).clamp(1, self.cols.len() - 1);
        let (a, b) = (&self.cols[i - 1], &self.cols[i]);
        let t = (x - a.x) / (b.x - a.x);
        a.cf * (1.0 - t) + b.cf * t
    }

    fn nearest(&self, x: f64) -> &Column {
        self.cols
            .iter()
            .min_by(|a, b| (a.x - x).abs().partial_cmp(&(b.x - x).abs()).unwrap())
            .unwrap()
    }

    /// Momentum-integral Reynolds number U int_0^x Cf/2 dx / nu: rectangle up to the first column centre, then trapezoids.
    fn re_theta_m(&self) -> Vec<(f64, f64)> {
        let c0 = &self.cols[0];
        let mut th = 0.5 * c0.cf * c0.x;
        let mut out = vec![(c0.x, th / NU)];
        for w in self.cols.windows(2) {
            th += 0.25 * (w[0].cf + w[1].cf) * (w[1].x - w[0].x);
            out.push((w[1].x, th / NU));
        }
        out
    }
}

fn run(m: Mesh) -> Run {
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
    let (kin, omega_in) = (2.25e-7, 125.0);
    let mut turb = KOmegaSst::new(geom.clone(), NU, &["ymin"], kin, omega_in).unwrap();
    turb.relax_turb = 0.6;
    turb.k.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(kin))).unwrap();
    turb.omega
        .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(omega_in)))
        .unwrap();
    let (mut iterations, mut change, mut du) = (0, f64::INFINITY, f64::INFINITY);
    for it in 0..14000 {
        change = turb.update(&flow.u, &flow.phi);
        flow.nu_face = Some(turb.nu_face());
        let info = flow.iterate();
        iterations = it + 1;
        du = info.max_du;
        if it % 1000 == 0 {
            println!("  it {it} du {du:.2e} dnut {change:.2e}");
        }
        if !info.res_u.is_finite() {
            break;
        }
        if it > 200 && du < 1e-9 && change < 1e-6 {
            break;
        }
    }
    let wall_faces: Vec<usize> = geom.mesh.patch("ymin").unwrap().faces().collect();
    let mesh = &geom.mesh;
    let cc = mesh.cell_centres();
    let mut cols = Vec::new();
    for (&(f, nutw), &face) in turb.wall_nut.iter().zip(&wall_faces) {
        assert_eq!(f, face);
        let c0 = mesh.owner()[f];
        let x = cc[c0][0];
        let yw = geom.dn[f - geom.n_internal];
        let tau = (NU + nutw) * flow.u.values[c0][0] / yw;
        let mut col: Vec<(f64, f64)> = (0..cc.len())
            .filter(|&c| (cc[c][0] - x).abs() < 1e-9)
            .map(|c| (cc[c][1], flow.u.values[c][0]))
            .collect();
        col.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let ue = col
            .iter()
            .min_by(|a, b| (a.0 - 0.25).abs().partial_cmp(&(b.0 - 0.25).abs()).unwrap())
            .unwrap()
            .1;
        cols.push(Column {
            x,
            cf: 2.0 * tau / (ue * ue),
            u_tau: tau.abs().sqrt(),
            u: col,
        });
    }
    cols.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    Run {
        cols,
        iterations,
        du,
        change,
    }
}

fn runs() -> &'static [Run] {
    static C: OnceLock<Vec<Run>> = OnceLock::new();
    C.get_or_init(|| {
        MESHES
            .iter()
            .map(|m| {
                let t0 = std::time::Instant::now();
                let r = run(*m);
                println!(
                    "[SST plate] mesh {m:?}: {} iterations, du {:.2e}, dnut {:.2e}, converged {}, {:.0} s",
                    r.iterations,
                    r.du,
                    r.change,
                    r.converged(),
                    t0.elapsed().as_secs_f64()
                );
                r
            })
            .collect()
    })
}

fn table2(src: &str) -> Vec<(f64, f64)> {
    src.lines()
        .skip(1)
        .map(|l| {
            let c: Vec<f64> = l.split(',').map(|v| v.parse().unwrap()).collect();
            (c[0], c[1])
        })
        .collect()
}

/// Karman-Schoenherr (TMR flatplate_val page): Cf = 1 / (17.08 (log10 Re_theta)^2 + 25.11 log10 Re_theta + 6.012).
fn karman_schoenherr(re_theta: f64) -> f64 {
    let l = re_theta.log10();
    1.0 / (17.08 * l * l + 25.11 * l + 6.012)
}

/// Least-squares slope and intercept of y against ln x over the points with x in [lo, hi].
fn log_fit(pts: &[(f64, f64)], lo: f64, hi: f64) -> (f64, f64, usize) {
    let p: Vec<(f64, f64)> = pts
        .iter()
        .filter(|q| q.0 >= lo && q.0 <= hi)
        .map(|q| (q.0.ln(), q.1))
        .collect();
    assert!(p.len() >= 3, "too few points for the log-law fit: {}", p.len());
    let n = p.len() as f64;
    let (mx, my) = (
        p.iter().map(|q| q.0).sum::<f64>() / n,
        p.iter().map(|q| q.1).sum::<f64>() / n,
    );
    let s = p.iter().map(|q| (q.0 - mx) * (q.1 - my)).sum::<f64>() / p.iter().map(|q| (q.0 - mx).powi(2)).sum::<f64>();
    (s, my - s * mx, p.len())
}

/// Linear interpolation in ln x of a table (x ascending).
fn interp_ln(tab: &[(f64, f64)], x: f64) -> f64 {
    let i = tab.partition_point(|r| r.0 < x).clamp(1, tab.len() - 1);
    let (a, b) = (tab[i - 1], tab[i]);
    let t = (x.ln() - a.0.ln()) / (b.0.ln() - a.0.ln());
    a.1 * (1.0 - t) + b.1 * t
}

/// (y+, u+) of the column nearest x.
fn uplus_profile(r: &Run, x: f64) -> (f64, Vec<(f64, f64)>) {
    let c = r.nearest(x);
    (c.x, c.u.iter().map(|&(y, u)| (y * c.u_tau / NU, u / c.u_tau)).collect())
}

fn check_converged(fails: &mut Vec<String>) {
    for (i, r) in runs().iter().enumerate() {
        if !r.converged() {
            fails.push(format!(
                "mesh {i} not converged (du {:.2e}, dnut {:.2e})",
                r.du, r.change
            ));
        }
    }
}

fn finish(fails: Vec<String>) {
    assert!(fails.is_empty(), "registered limits missed:\n{}", fails.join("\n"));
}

/// P2a: Cf(x = 0.970084) of the finest mesh within 2.0 percent of the mean of the three incompressible TMR codes (545 x 385 grid).
#[test]
#[ignore = "heavy: SST plate mesh sequence, release"]
fn w33c_p2a_cf_at_x_ref_vs_incompressible_tmr() {
    let mut fails = Vec::new();
    check_converged(&mut fails);
    let inc: Vec<f64> = TMR_CF_CONV
        .lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            (c[0].contains("incompressible") && c[1] == "208896").then(|| c[2].parse::<f64>().unwrap())
        })
        .collect();
    assert_eq!(inc.len(), 3);
    let mean = inc.iter().sum::<f64>() / 3.0;
    let comp = TMR_CF_CONV
        .lines()
        .skip(1)
        .find(|l| l.starts_with("CFL3D") && l.contains(",208896,"))
        .map(|l| l.rsplit(',').next().unwrap().parse::<f64>().unwrap())
        .unwrap();
    for (i, r) in runs().iter().enumerate() {
        let cf = r.cf_at(X_REF);
        println!(
            "  mesh {i}: Cf(x_ref) {cf:.6e}  vs incompressible mean {mean:.6e} ({:+.2}%), vs CFL3D compressible {comp:.6e} ({:+.2}%)",
            100.0 * (cf / mean - 1.0),
            100.0 * (cf / comp - 1.0)
        );
    }
    let cf = runs()[2].cf_at(X_REF);
    if (cf / mean - 1.0).abs() > 0.02 {
        fails.push(format!(
            "P2a: Cf(x_ref) {cf:.6e} vs {mean:.6e} ({:+.2}%) outside 2%",
            100.0 * (cf / mean - 1.0)
        ));
    }
    finish(fails);
}

/// P2b: Cf(x) of the finest mesh within 3.0 percent of the TMR CFL3D curve (compressible) at x = 0.5, 1.0, 1.5, 1.8.
#[test]
#[ignore = "heavy: SST plate mesh sequence, release"]
fn w33c_p2b_cf_curve_vs_cfl3d() {
    let mut fails = Vec::new();
    check_converged(&mut fails);
    let tab = table2(TMR_CF_X);
    for x in [0.5, 1.0, 1.5, 1.8] {
        let i = tab.partition_point(|r| r.0 < x).clamp(1, tab.len() - 1);
        let t = (x - tab[i - 1].0) / (tab[i].0 - tab[i - 1].0);
        let reference = tab[i - 1].1 * (1.0 - t) + tab[i].1 * t;
        for (m, r) in runs().iter().enumerate() {
            let cf = r.cf_at(x);
            println!(
                "  x {x}: mesh {m} Cf {cf:.6e} CFL3D {reference:.6e} ({:+.2}%)",
                100.0 * (cf / reference - 1.0)
            );
            if m == 2 && (cf / reference - 1.0).abs() > 0.03 {
                fails.push(format!(
                    "P2b: Cf({x}) {cf:.6e} vs {reference:.6e} ({:+.2}%) outside 3%",
                    100.0 * (cf / reference - 1.0)
                ));
            }
        }
    }
    finish(fails);
}

/// P2c: log-law slope (fit y+ 50 to 300) of the column nearest x_ref within 3.0 percent of the TMR SST fit, 2.7784.
#[test]
#[ignore = "heavy: SST plate mesh sequence, release"]
fn w33c_p2c_loglaw_slope_vs_tmr() {
    let mut fails = Vec::new();
    check_converged(&mut fails);
    let tmr = table2(TMR_UPLUS);
    let (st, ct, nt) = log_fit(&tmr, 50.0, 300.0);
    println!("  TMR CFL3D table fit 50..300: slope {st:.4} intercept {ct:.3} ({nt} points; registered 2.7784)");
    for (i, r) in runs().iter().enumerate() {
        let (x, prof) = uplus_profile(r, X_REF);
        let (s, c, n) = log_fit(&prof, 50.0, 300.0);
        println!(
            "  mesh {i}: column x {x:.4}: slope {s:.4} intercept {c:.3} ({n} cells), 1/slope {:.4}",
            1.0 / s
        );
        if i == 2 && (s / 2.7784 - 1.0).abs() > 0.03 {
            fails.push(format!(
                "P2c: slope {s:.4} vs 2.7784 ({:+.2}%) outside 3%",
                100.0 * (s / 2.7784 - 1.0)
            ));
        }
    }
    finish(fails);
}

/// P2d: u+ of the finest mesh at the column nearest x_ref against the TMR SST data: 3.0 percent at y+ 30 .. 600, 5.0 percent at y+ 5, 10, 15.
#[test]
#[ignore = "heavy: SST plate mesh sequence, release"]
fn w33c_p2d_uplus_profile_vs_tmr() {
    let mut fails = Vec::new();
    check_converged(&mut fails);
    let tmr = table2(TMR_UPLUS);
    for (m, r) in runs().iter().enumerate() {
        let (x, prof) = uplus_profile(r, X_REF);
        println!("  mesh {m}: column x {x:.4}");
        for (yp, tol) in [
            (5.0, 0.05),
            (10.0, 0.05),
            (15.0, 0.05),
            (30.0, 0.03),
            (50.0, 0.03),
            (100.0, 0.03),
            (200.0, 0.03),
            (300.0, 0.03),
            (600.0, 0.03),
        ] {
            let (mine, theirs) = (interp_ln(&prof, yp), interp_ln(&tmr, yp));
            println!(
                "    y+ {yp:6.1}: u+ {mine:7.3}  TMR {theirs:7.3}  ({:+.2}%)",
                100.0 * (mine / theirs - 1.0)
            );
            if m == 2 && (mine / theirs - 1.0).abs() > tol {
                fails.push(format!(
                    "P2d: u+({yp}) {mine:.3} vs {theirs:.3} ({:+.2}%) outside {}%",
                    100.0 * (mine / theirs - 1.0),
                    tol * 100.0
                ));
            }
        }
    }
    finish(fails);
}

/// P2e: Cf versus Karman-Schoenherr at the momentum-integral Re_theta 5000, 7500, 10000, 12500: deviation within 1.5 percentage points of
/// the deviation of the TMR CFL3D curve (-3.61, -2.76, -2.16, -1.69 percent, recomputed here from the vendored curve).
#[test]
#[ignore = "heavy: SST plate mesh sequence, release"]
fn w33c_p2e_cf_vs_karman_schoenherr_momentum_integral() {
    let mut fails = Vec::new();
    check_converged(&mut fails);
    // the TMR curve with the same formula
    let tab = table2(TMR_CF_X);
    let mut th = 0.5 * tab[0].1 * tab[0].0;
    let mut tmr_re = vec![(tab[0].0, th / NU, tab[0].1)];
    for w in tab.windows(2) {
        th += 0.25 * (w[0].1 + w[1].1) * (w[1].0 - w[0].0);
        tmr_re.push((w[1].0, th / NU, w[1].1));
    }
    let dev_at = |series: &[(f64, f64, f64)], target: f64| -> f64 {
        let i = series.partition_point(|r| r.1 < target).clamp(1, series.len() - 1);
        let t = (target - series[i - 1].1) / (series[i].1 - series[i - 1].1);
        let cf = series[i - 1].2 * (1.0 - t) + series[i].2 * t;
        100.0 * (cf / karman_schoenherr(target) - 1.0)
    };
    for re in [5000.0, 7500.0, 10000.0, 12500.0] {
        let dt = dev_at(&tmr_re, re);
        for (m, r) in runs().iter().enumerate() {
            let series: Vec<(f64, f64, f64)> = r
                .re_theta_m()
                .iter()
                .zip(&r.cols)
                .map(|(&(x, re_t), c)| (x, re_t, c.cf))
                .collect();
            let d = dev_at(&series, re);
            println!(
                "  Re_theta {re}: mesh {m} {d:+.2}%  TMR CFL3D {dt:+.2}%  (difference {:+.2} points)",
                d - dt
            );
            if m == 2 && (d - dt).abs() > 1.5 {
                fails.push(format!(
                    "P2e: Re_theta {re}: {d:+.2}% vs TMR {dt:+.2}% differ by more than 1.5 points"
                ));
            }
        }
    }
    finish(fails);
}

/// P2f: grid convergence of Cf(x_ref): monotone, observed order in [0.8, 2.5], GCI_fine <= 3 percent, |Cf_f - Cf_m|/Cf_f <= 1.5 percent.
/// KNOWN MISS (ADR 0070 addendum, expected when registered): Cf(x_ref) 2.57698e-3, 2.65138e-3, 2.69840e-3; monotone, observed order 0.662
/// (limit 0.8), GCI_fine 3.74 percent (limit 3), |Cf_f - Cf_m|/Cf_f 1.742 percent (limit 1.5); Richardson value 2.7791e-3.
#[test]
#[ignore = "known miss, heavy (30 min): see doc comment"]
fn w33c_p2f_grid_convergence_of_cf_known_miss() {
    let mut fails = Vec::new();
    check_converged(&mut fails);
    let cf: Vec<f64> = runs().iter().map(|r| r.cf_at(X_REF)).collect();
    println!("  Cf(x_ref) sequence {cf:?}");
    let (e21, e32) = (cf[1] - cf[0], cf[2] - cf[1]);
    println!(
        "  differences {e21:.4e} {e32:.4e}, finest-medium {:.3}% of finest",
        100.0 * e32.abs() / cf[2]
    );
    if e21 * e32 <= 0.0 || e32 == 0.0 || e32.abs() >= e21.abs() {
        fails.push("P2f: sequence not monotonically converging".to_string());
    } else {
        let p = (e21 / e32).ln() / 2.0f64.ln();
        let gci = 1.25 * (e32 / cf[2]).abs() / (2.0f64.powf(p) - 1.0);
        let rich = cf[2] + e32 / (2.0f64.powf(p) - 1.0);
        println!(
            "  observed order {p:.3}, GCI_fine {:.3}%, Richardson value {rich:.6e}",
            100.0 * gci
        );
        if !(0.8..=2.5).contains(&p) {
            fails.push(format!("P2f: observed order {p:.3} outside [0.8, 2.5]"));
        }
        if gci > 0.03 {
            fails.push(format!("P2f: GCI_fine {:.3}% above 3%", 100.0 * gci));
        }
    }
    if e32.abs() / cf[2] > 0.015 {
        fails.push(format!(
            "P2f: |Cf_f - Cf_m|/Cf_f {:.3}% above 1.5%",
            100.0 * e32.abs() / cf[2]
        ));
    }
    // the TMR CFL3D sequence (compressible) for the record
    let tmr: Vec<f64> = TMR_CF_CONV
        .lines()
        .skip(1)
        .filter(|l| l.starts_with("CFL3D"))
        .map(|l| l.rsplit(',').next().unwrap().parse().unwrap())
        .collect();
    println!("  TMR CFL3D sequence (finest first) {tmr:?}");
    finish(fails);
}
