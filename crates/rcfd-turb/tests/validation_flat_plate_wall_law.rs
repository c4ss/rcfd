//! Wave 30C (ADR 0061 section D and addendum 4): zero-pressure-gradient flat plate against the NASA TMR 2DZP empirical references,
//! (a) Karman-Schoenherr skin friction versus momentum-thickness Reynolds number, (b) the Coles velocity law u+(y+) at Re_theta = 10000
//! (`validation-data/reference/w30c_tmr_coles_uplus_yplus_retheta10000.csv`, sampled from the TMR `u+y+.dat`), with mesh sequences.
//!
//! Set-up: TMR 2DZP as `flat_plate_sst.rs` (U = 1, nu = 2e-7, plate x in [0, 2], height 1, symmetry top). Own runner: a copy of the
//! `common/plate.rs` run that also returns the velocity columns; no shared test code is edited.
//!
//! Heavy (minutes to tens of minutes each): `cargo test --release -p rcfd-turb --test validation_flat_plate_wall_law -- --ignored --nocapture`.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_mesh::layers::{first_cell_height_from_y_plus, flat_plate_u_tau};
use rcfd_turb::kepsilon::WallFunction;
use rcfd_turb::{KEpsilon, KEpsilonVariant, KOmegaSst};
use std::sync::{Arc, OnceLock};

const NU: f64 = 2.0e-7;
const COLES: &str = include_str!("../../../validation-data/reference/w30c_tmr_coles_uplus_yplus_retheta10000.csv");

#[derive(Clone, Copy, Debug)]
enum Model {
    Sst,
    Ke(WallFunction),
}

#[derive(Clone, Copy, Debug)]
struct Mesh {
    nx: usize,
    ny: usize,
    layers: usize,
    growth: f64,
    y_plus: f64,
}

/// One streamwise column of the converged field.
#[derive(Clone, Debug)]
struct Column {
    x: f64,
    cf: f64,
    u_tau: f64,
    y_plus_first: f64,
    re_theta: f64,
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

    /// Cf and column index interpolation at a target Re_theta (Re_theta increases with x). Returns (Cf, x, column nearest).
    fn at_re_theta(&self, target: f64) -> (f64, f64, usize) {
        let i = self
            .cols
            .partition_point(|c| c.re_theta < target)
            .clamp(1, self.cols.len() - 1);
        let (a, b) = (&self.cols[i - 1], &self.cols[i]);
        let t = (target - a.re_theta) / (b.re_theta - a.re_theta);
        let near = if t < 0.5 { i - 1 } else { i };
        (a.cf * (1.0 - t) + b.cf * t, a.x * (1.0 - t) + b.x * t, near)
    }
}

fn run(model: Model, m: Mesh, kin: f64, second: f64, linear_upwind_turbulence: bool) -> Run {
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
    enum M {
        S(Box<KOmegaSst>),
        K(Box<KEpsilon>),
    }
    let mut turb = match model {
        Model::Sst => {
            let mut t = KOmegaSst::new(geom.clone(), NU, &["ymin"], kin, second).unwrap();
            if linear_upwind_turbulence {
                t.convection = ConvScheme::LinearUpwind;
            }
            t.relax_turb = 0.6;
            t.k.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(kin))).unwrap();
            t.omega
                .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(second)))
                .unwrap();
            M::S(Box::new(t))
        }
        Model::Ke(wf) => {
            let mut t = KEpsilon::new(KEpsilonVariant::Standard, geom.clone(), NU, &["ymin"], kin, second).unwrap();
            t.relax_turb = 0.6;
            t.wall_function = wf;
            t.k.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(kin))).unwrap();
            t.eps
                .set_bc(&geom, "xmin", Box::new(FixedValue::scalar(second)))
                .unwrap();
            M::K(Box::new(t))
        }
    };
    let (mut iterations, mut change, mut du) = (0, f64::INFINITY, f64::INFINITY);
    for it in 0..14000 {
        change = match &mut turb {
            M::S(t) => t.update(&flow.u, &flow.phi),
            M::K(t) => t.update(&flow.u, &flow.phi),
        };
        flow.nu_face = Some(match &turb {
            M::S(t) => t.nu_face(),
            M::K(t) => t.nu_face(),
        });
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
    let wall_nut: &[(usize, f64)] = match &turb {
        M::S(t) => &t.wall_nut,
        M::K(t) => &t.wall_nut,
    };
    let mesh = &geom.mesh;
    let cc = mesh.cell_centres();
    let mut cols = Vec::new();
    for (&(f, nutw), &face) in wall_nut.iter().zip(&wall_faces) {
        assert_eq!(f, face);
        let c0 = mesh.owner()[f];
        let x = cc[c0][0];
        let yw = geom.dn[f - geom.n_internal];
        let tau = (NU + nutw) * flow.u.values[c0][0] / yw;
        // the column: all cells with the same centre x
        let mut col: Vec<(f64, f64)> = (0..cc.len())
            .filter(|&c| (cc[c][0] - x).abs() < 1e-9)
            .map(|c| (cc[c][1], flow.u.values[c][0]))
            .collect();
        col.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        // edge velocity at the cell nearest y = 0.25
        let ue = col
            .iter()
            .min_by(|a, b| (a.0 - 0.25).abs().partial_cmp(&(b.0 - 0.25).abs()).unwrap())
            .unwrap()
            .1;
        // momentum thickness, trapezoid from the wall (u = 0) to y = 0.15
        let mut theta = 0.0;
        let (mut y0, mut g0) = (0.0, 0.0);
        for &(y, uu) in &col {
            if y > 0.15 {
                break;
            }
            let g = (uu / ue) * (1.0 - uu / ue);
            theta += 0.5 * (g0 + g) * (y - y0);
            y0 = y;
            g0 = g;
        }
        let u_tau = tau.abs().sqrt();
        cols.push(Column {
            x,
            cf: 2.0 * tau / (ue * ue),
            u_tau,
            y_plus_first: u_tau * yw / NU,
            re_theta: theta * ue / NU,
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

/// Karman-Schoenherr (TMR flatplate_val page): Cf = 1 / (17.08 (log10 Re_theta)^2 + 25.11 log10 Re_theta + 6.012).
fn karman_schoenherr(re_theta: f64) -> f64 {
    let l = re_theta.log10();
    1.0 / (17.08 * l * l + 25.11 * l + 6.012)
}

fn coles() -> Vec<(f64, f64)> {
    COLES
        .lines()
        .skip(1)
        .map(|l| {
            let c: Vec<f64> = l.split(',').map(|v| v.parse().unwrap()).collect();
            (c[0], c[1])
        })
        .collect()
}

/// u+ of the Coles curve at y+ (linear in ln y+); `None` outside the sampled range.
fn coles_at(tab: &[(f64, f64)], yp: f64) -> Option<f64> {
    if yp < tab[0].0 || yp > tab.last().unwrap().0 {
        return None;
    }
    let i = tab.partition_point(|r| r.0 < yp).max(1);
    let (a, b) = (tab[i - 1], tab[i]);
    let t = (yp.ln() - a.0.ln()) / (b.0.ln() - a.0.ln());
    Some(a.1 * (1.0 - t) + b.1 * t)
}

/// Least-squares u+ = s ln y+ + c over the points with y+ in [lo, hi]; returns (1/kappa, C).
fn log_fit(pts: &[(f64, f64)], lo: f64, hi: f64) -> (f64, f64) {
    let p: Vec<(f64, f64)> = pts
        .iter()
        .filter(|q| q.0 >= lo && q.0 <= hi)
        .map(|q| (q.0.ln(), q.1))
        .collect();
    assert!(p.len() >= 3, "too few points for the log-law fit: {}", p.len());
    let n = p.len() as f64;
    let (sx, sy) = (p.iter().map(|q| q.0).sum::<f64>(), p.iter().map(|q| q.1).sum::<f64>());
    let sxx: f64 = p.iter().map(|q| q.0 * q.0).sum();
    let sxy: f64 = p.iter().map(|q| q.0 * q.1).sum();
    let s = (n * sxy - sx * sy) / (n * sxx - sx * sx);
    (s, (sy - s * sx) / n)
}

/// Observed order and GCI (safety factor 1.25, refinement ratio r) from coarse, medium, fine.
fn order_gci(q: [f64; 3], r: f64) -> Option<(f64, f64)> {
    let (e21, e32) = (q[1] - q[0], q[2] - q[1]);
    if e21 * e32 <= 0.0 || e32 == 0.0 {
        return None;
    }
    let p = (e21 / e32).ln() / r.ln();
    Some((p, 1.25 * (e32 / q[2]).abs() / (r.powf(p) - 1.0)))
}

const SST_MESHES: [Mesh; 3] = [
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

const KE_MESHES: [Mesh; 3] = [
    Mesh {
        nx: 70,
        ny: 30,
        layers: 20,
        growth: 1.21,
        y_plus: 30.0,
    },
    Mesh {
        nx: 140,
        ny: 60,
        layers: 40,
        growth: 1.10,
        y_plus: 30.0,
    },
    Mesh {
        nx: 280,
        ny: 120,
        layers: 80,
        growth: 1.0488,
        y_plus: 30.0,
    },
];

fn sequence(
    cell: &'static OnceLock<Vec<Run>>,
    tag: &str,
    model: Model,
    meshes: &[Mesh; 3],
    kin: f64,
    second: f64,
) -> &'static [Run] {
    cell.get_or_init(|| {
        meshes
            .iter()
            .map(|m| {
                let t0 = std::time::Instant::now();
                let r = run(model, *m, kin, second, false);
                println!(
                    "[{tag}] mesh {m:?}: {} iterations, du {:.2e}, dnut {:.2e}, converged {}, {:.0} s",
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

fn sst_runs() -> &'static [Run] {
    static C: OnceLock<Vec<Run>> = OnceLock::new();
    sequence(&C, "SST", Model::Sst, &SST_MESHES, 2.25e-7, 125.0)
}

fn ke_runs(wf: WallFunction) -> &'static [Run] {
    static SCAL: OnceLock<Vec<Run>> = OnceLock::new();
    static STD: OnceLock<Vec<Run>> = OnceLock::new();
    match wf {
        WallFunction::Scalable => sequence(&SCAL, "k-eps Scalable", Model::Ke(wf), &KE_MESHES, 1.5e-4, 1e-3),
        _ => sequence(&STD, "k-eps Standard", Model::Ke(wf), &KE_MESHES, 1.5e-4, 1e-3),
    }
}

fn cf_report(tag: &str, r: &Run, stations: &[f64]) -> Vec<String> {
    let mut fails = Vec::new();
    for &re in stations {
        let (cf, x, _) = r.at_re_theta(re);
        let ks = karman_schoenherr(re);
        println!(
            "  [{tag}] Re_theta {re}: x {x:.4} Cf {cf:.6} K-S {ks:.6} ({:+.2}%)",
            100.0 * (cf / ks - 1.0)
        );
        if (cf / ks - 1.0).abs() > 0.05 {
            fails.push(format!(
                "{tag}: Cf at Re_theta {re} {cf:.6} vs K-S {ks:.6} ({:+.2}%) outside 5%",
                100.0 * (cf / ks - 1.0)
            ));
        }
    }
    fails
}

fn check_converged(tag: &str, runs: &[Run], fails: &mut Vec<String>) {
    for (i, r) in runs.iter().enumerate() {
        if !r.converged() {
            fails.push(format!(
                "{tag} mesh {i} not converged (du {:.2e}, dnut {:.2e})",
                r.du, r.change
            ));
        }
    }
}

/// u+ comparison at the column nearest Re_theta = 10000. `bands` are (y+ lo, y+ hi, tolerance); only cells above index `skip` count.
fn uplus_check(tag: &str, r: &Run, bands: &[(f64, f64, f64)], skip: usize, fit: bool) -> Vec<String> {
    let tab = coles();
    let (_, x, ci) = r.at_re_theta(10000.0);
    let col = &r.cols[ci];
    println!(
        "  [{tag}] u+ column x {:.4} (target x {x:.4}): Re_theta {:.0}, u_tau {:.5}, first-cell y+ {:.2}",
        col.x, col.re_theta, col.u_tau, col.y_plus_first
    );
    let mut fails = Vec::new();
    let mut sol = Vec::new();
    for (k, &(y, u)) in col.u.iter().enumerate() {
        let (yp, up) = (y * col.u_tau / NU, u / col.u_tau);
        sol.push((yp, up));
        if k < skip {
            continue;
        }
        if let Some(c) = coles_at(&tab, yp) {
            if let Some(&(_, _, tol)) = bands.iter().find(|b| yp >= b.0 && yp <= b.1) {
                let d = up / c - 1.0;
                println!("    y+ {yp:9.2} u+ {up:7.3} Coles {c:7.3} ({:+.2}%)", 100.0 * d);
                if d.abs() > tol {
                    fails.push(format!(
                        "{tag}: u+({yp:.1}) {up:.3} vs Coles {c:.3} ({:+.2}%) outside {:.0}%",
                        100.0 * d,
                        100.0 * tol
                    ));
                }
            }
        }
    }
    if fit {
        let (s, c) = log_fit(&sol, 50.0, 300.0);
        let (st, ct) = log_fit(&tab, 50.0, 300.0);
        println!("  [{tag}] log-law fit 50..300: 1/kappa {s:.4} C {c:.3}; TMR data {st:.4} C {ct:.3}");
        if (s / st - 1.0).abs() > 0.05 {
            fails.push(format!("{tag}: log-law slope {s:.4} vs {st:.4} outside 5%"));
        }
        if (c - ct).abs() > 0.4 {
            fails.push(format!("{tag}: log-law intercept {c:.3} vs {ct:.3} outside 0.4"));
        }
    }
    fails
}

const BANDS_SST: [(f64, f64, f64); 4] = [
    (1.0, 5.0, 0.05),
    (5.0, 30.0, 0.06),
    (30.0, 300.0, 0.03),
    (300.0, 1000.0, 0.05),
];

/// KNOWN MISS (ADR 0061 addendum 5): measured once, 30 min for the sequence. Finest mesh Cf versus Karman-Schoenherr:
/// -8.65 percent at Re_theta 5000, -6.44 at 7500, -5.18 at 10000 (limit 5), -4.36 at 12500 (inside). The sequence converges toward K-S
/// from below (Re_theta 10000: -9.72, -7.02, -5.18 percent), see `d3_sst_cf_mesh_convergence_known_miss` and the diagnostic test.
/// NOTE (wave 31B, ADR 0063 addenda): the Re_theta of this runner (profile trapezoid to y = 0.15, edge velocity at y = 0.25) is biased low
/// (negative near the leading edge, 25 percent below the momentum integral at x ~ 1), which inflates this deficit; limits and test unchanged.
#[test]
#[ignore = "known miss, heavy (30 min): see doc comment"]
fn d1_sst_cf_vs_karman_schoenherr_known_miss() {
    let runs = sst_runs();
    let mut fails = Vec::new();
    check_converged("SST", runs, &mut fails);
    for (i, r) in runs.iter().enumerate() {
        fails.extend(
            cf_report(&format!("SST mesh {i}"), r, &[5000.0, 7500.0, 10000.0, 12500.0])
                .into_iter()
                .filter(|_| i == 2),
        );
    }
    assert!(fails.is_empty(), "registered limits missed:\n{}", fails.join("\n"));
}

#[test]
#[ignore = "heavy: SST wall-resolved mesh sequence, release"]
fn d2_sst_uplus_vs_coles() {
    let runs = sst_runs();
    let mut fails = Vec::new();
    check_converged("SST", runs, &mut fails);
    fails.extend(uplus_check("SST fine", &runs[2], &BANDS_SST, 0, true));
    assert!(fails.is_empty(), "registered limits missed:\n{}", fails.join("\n"));
}

/// KNOWN MISS (ADR 0061 addendum 5): |Cf_f - Cf_m| / Cf_f = 1.94 percent (limit 1.5) at Re_theta 10000; the sequence converges
/// monotonically (0.0023776, 0.0024485, 0.0024969) with observed order 0.55 and GCI_fine 5.3 percent.
#[test]
#[ignore = "known miss, heavy (30 min): see doc comment"]
fn d3_sst_cf_mesh_convergence_known_miss() {
    let runs = sst_runs();
    let cf: Vec<f64> = runs.iter().map(|r| r.at_re_theta(10000.0).0).collect();
    println!("  Cf(Re_theta = 10000) sequence {cf:?}");
    let mut fails = Vec::new();
    match order_gci([cf[0], cf[1], cf[2]], 2.0) {
        Some((p, g)) => println!("  observed order {p:.2}, GCI_fine {:.3}%", 100.0 * g),
        None => println!("  sequence not monotone: no observed order"),
    }
    if (cf[2] - cf[1]).abs() >= (cf[1] - cf[0]).abs() {
        fails.push("D3 sequence not converging".to_string());
    }
    if (cf[2] - cf[1]).abs() / cf[2] > 0.015 {
        fails.push(format!(
            "D3 |Cf_f - Cf_m| / Cf_f = {:.3}% > 1.5%",
            100.0 * (cf[2] - cf[1]).abs() / cf[2]
        ));
    }
    check_converged("SST", runs, &mut fails);
    assert!(fails.is_empty(), "registered limits missed:\n{}", fails.join("\n"));
}

fn d4(wf: WallFunction, tag: &str) {
    let runs = ke_runs(wf);
    let mut fails = Vec::new();
    check_converged(tag, runs, &mut fails);
    for (i, r) in runs.iter().enumerate() {
        let (_, _, ci) = r.at_re_theta(10000.0);
        println!(
            "  [{tag}] mesh {i}: first-cell y+ at Re_theta 10000: {:.1}",
            r.cols[ci].y_plus_first
        );
        let f = cf_report(&format!("{tag} mesh {i}"), r, &[7500.0, 10000.0, 12500.0]);
        if i == 2 {
            fails.extend(f);
            let yp = r.cols[ci].y_plus_first;
            if !(20.0..=60.0).contains(&yp) {
                fails.push(format!("{tag}: NOT APPLICABLE, first-cell y+ {yp:.1} outside [20, 60]"));
            }
            fails.extend(uplus_check(tag, r, &[(30.0, 300.0, 0.05)], 1, false));
        }
    }
    assert!(fails.is_empty(), "registered limits missed:\n{}", fails.join("\n"));
}

#[test]
fn d4_kepsilon_scalable_wall_function() {
    d4(WallFunction::Scalable, "k-eps Scalable");
}

#[test]
fn d4_kepsilon_standard_wall_function() {
    d4(WallFunction::Standard, "k-eps Standard");
}

/// Diagnostic only (no registered limit): the medium SST mesh with (a) linear-upwind instead of first-order upwind convection of k and
/// omega, (b) twice the streamwise cells. Prints Cf at Re_theta = 10000 for comparison with the registered sequence.
#[test]
#[ignore = "diagnostic, heavy"]
fn diag_sst_medium_mesh_variants() {
    let base = SST_MESHES[1];
    let variants = [
        ("linear-upwind k and omega", base, true),
        ("nx doubled (280)", Mesh { nx: 280, ..base }, false),
    ];
    for (name, m, lu) in variants {
        let r = run(Model::Sst, m, 2.25e-7, 125.0, lu);
        let (cf, x, _) = r.at_re_theta(10000.0);
        println!(
            "[diag] {name}: {} iterations, converged {}, Cf(Re_theta 10000) {cf:.6} at x {x:.4}, K-S {:.6} ({:+.2}%)",
            r.iterations,
            r.converged(),
            karman_schoenherr(10000.0),
            100.0 * (cf / karman_schoenherr(10000.0) - 1.0)
        );
    }
}
