//! Laminar flat plate against the Blasius solution with a three-mesh sequence (ADR 0060, checks B1 to B4).
//!
//! Reference: arXiv:2204.02215 (Lal and Martin 2022) 32-digit solution of f''' + f f''/2 = 0: f''(0) = 0.332057336,
//! f'(1..5) = 0.329780, 0.629766, 0.846044, 0.955518, 0.991542, displacement thickness 1.720788 sqrt(nu x / U);
//! Cf = 2 f''(0)/sqrt(Re_x) = 0.664115/sqrt(Re_x). Set-up: U = 1, L = 1, nu = 1e-5, plate from the inlet plane,
//! open top at y = 0.3 (zero-gradient u, p = 0 through `Freestream`; no `Symmetry`).

mod validation_common;

use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{Freestream, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use std::sync::Arc;
use validation_common::*;

const NU: f64 = 1.0e-5;
const H: f64 = 0.3;

struct Plate {
    n: usize,
    /// (x, Cf) along the wall, sorted
    cf: Vec<(f64, f64)>,
    /// (y, u) in the column pair around x = 0.5, sorted, with cell edges for the integral
    column: Vec<(f64, f64, f64)>,
    iterations: usize,
    converged: bool,
}

#[derive(Clone, Copy, Debug)]
enum Top {
    Open,
    Slip,
    Fixed,
}

#[derive(Clone, Copy, Debug)]
enum Outlet {
    Pressure,
}

fn run_cfg(n: usize, h: f64, top: Top, _outlet: Outlet) -> Plate {
    run_full(n, h, top, Variant::default())
}

#[derive(Clone, Copy, Debug)]
struct Variant {
    scheme: ConvScheme,
    coupling: Coupling,
    h_from_solution: bool,
    ratio: f64,
    nu: f64,
    /// Length of an upstream section whose wall moves with the free stream (no shear), 0 for the plate at the inlet.
    belt: f64,
}

impl Default for Variant {
    fn default() -> Self {
        Variant {
            scheme: ConvScheme::LinearUpwind,
            coupling: Coupling::Simplec,
            h_from_solution: true,
            ratio: 60.0,
            nu: NU,
            belt: 0.0,
        }
    }
}

fn run_full(n: usize, h: f64, top: Top, v: Variant) -> Plate {
    let nu_ = v.nu;
    let nx = n + (v.belt * n as f64).round() as usize;
    let mesh = box_2d_graded(
        nx,
        n,
        1.0 + v.belt,
        h,
        [Grading::Uniform, Grading::Expansion(v.ratio)],
        false,
        false,
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    let wall_faces: Vec<usize> = geom.mesh.patch("ymin").unwrap().faces().collect();
    let wall_prof: Vec<[f64; 3]> = wall_faces
        .iter()
        .map(|&f| {
            if geom.mesh.face_centres()[f][0] < v.belt {
                [1.0, 0.0, 0.0]
            } else {
                [0.0; 3]
            }
        })
        .collect();
    u.set_bc(
        &geom,
        "ymin",
        Box::new(FixedValue(rcfd_fv::Profile::PerFace(wall_prof))),
    )
    .unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let uinf = [1.0, 0.0, 0.0];
    match top {
        Top::Open => u
            .set_bc(
                &geom,
                "ymax",
                Box::new(Freestream {
                    u_ref: uinf,
                    value: uinf,
                    fixed_on_inflow: true,
                }),
            )
            .unwrap(),
        Top::Slip => u.set_bc(&geom, "ymax", Box::new(rcfd_fv::Symmetry)).unwrap(),
        Top::Fixed => u
            .set_bc(&geom, "ymax", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
            .unwrap(),
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    if let Top::Open = top {
        p.set_bc(
            &geom,
            "ymax",
            Box::new(Freestream {
                u_ref: uinf,
                value: [0.0; 3],
                fixed_on_inflow: false,
            }),
        )
        .unwrap();
    }
    let mut st = SimpleSettings::new(nu_, v.coupling);
    st.convection = v.scheme;
    st.h_from_solution = v.h_from_solution;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, st);
    let t0 = std::time::Instant::now();
    let sum = flow.run(6000, 1e-7);
    println!(
        "plate n={n}: {} iterations, converged {}, max_du {:.2e}, {:.0}s",
        sum.iterations,
        sum.converged,
        sum.last.max_du,
        t0.elapsed().as_secs_f64()
    );
    let wall: Vec<usize> = geom.mesh.patch("ymin").unwrap().faces().collect();
    let cc = geom.mesh.cell_centres();
    let mut cf: Vec<(f64, f64)> = wall
        .iter()
        .map(|&f| {
            let c = geom.mesh.owner()[f];
            let y = geom.dn[f - geom.n_internal];
            (cc[c][0] - v.belt, 2.0 * nu_ * flow.u.values[c][0] / y)
        })
        .collect();
    cf.retain(|c| c.0 > 0.0);
    cf.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    // columns bracketing x = 0.5 (n is even): average the two
    let dx = 1.0 / n as f64;
    let mut cols: Vec<Vec<(f64, f64)>> = vec![vec![], vec![]];
    for c in 0..geom.n_cells() {
        let i = ((cc[c][0] / dx).floor()) as usize;
        let hi = ((v.belt + 0.5) * n as f64).round() as usize;
        if i + 1 == hi {
            cols[0].push((cc[c][1], flow.u.values[c][0]));
        } else if i == hi {
            cols[1].push((cc[c][1], flow.u.values[c][0]));
        }
    }
    for col in cols.iter_mut() {
        col.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    }
    let mut column = Vec::new();
    let mut lo = 0.0;
    for k in 0..cols[0].len() {
        let (y, _) = cols[0][k];
        let hi = 2.0 * y - lo;
        column.push((y, 0.5 * (cols[0][k].1 + cols[1][k].1), hi - lo));
        lo = hi;
    }
    Plate {
        n,
        cf,
        column,
        iterations: sum.iterations,
        converged: sum.converged,
    }
}

fn cf_at(p: &Plate, x: f64) -> f64 {
    let i = p.cf.partition_point(|c| c.0 < x).clamp(1, p.cf.len() - 1);
    let (a, b) = (p.cf[i - 1], p.cf[i]);
    a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0)
}

/// u/U at height `y` of the column (linear between cell centres, 0 at the wall).
fn u_at(p: &Plate, y: f64) -> f64 {
    let mut prev = (0.0, 0.0);
    for &(yc, u, _) in &p.column {
        if y <= yc {
            return prev.1 + (u - prev.1) * (y - prev.0) / (yc - prev.0);
        }
        prev = (yc, u);
    }
    prev.1
}

/// The registered meshes with the plate at the inlet (`belt` 0, the set-up of the first ADR 0060 run) or with
/// an upstream shear-free moving-wall section of 0.5 (corrected set-up, ADR 0060 addendum).
fn sequence(belt: f64) -> &'static Vec<Plate> {
    static AT_INLET: std::sync::OnceLock<Vec<Plate>> = std::sync::OnceLock::new();
    static WITH_BELT: std::sync::OnceLock<Vec<Plate>> = std::sync::OnceLock::new();
    let cell = if belt == 0.0 { &AT_INLET } else { &WITH_BELT };
    cell.get_or_init(|| {
        [60, 120, 240]
            .iter()
            .map(|&n| {
                run_full(
                    n,
                    H,
                    Top::Open,
                    Variant {
                        belt,
                        ..Variant::default()
                    },
                )
            })
            .collect()
    })
}

fn reference() -> (f64, Vec<(f64, f64)>, f64) {
    let rows = read_rows(&reference_dir().join("blasius_lal_martin_2022.csv"));
    let mut fpp0 = 0.0;
    let mut fp = vec![];
    let mut dstar = 0.0;
    for r in rows {
        let v: f64 = r[2].parse().unwrap();
        match r[0].as_str() {
            "fpp0" => fpp0 = v,
            "fp" => fp.push((r[1].parse().unwrap(), v)),
            "delta_star_coeff" => dstar = v,
            q => panic!("unknown quantity {q}"),
        }
    }
    (fpp0, fp, dstar)
}

/// `edge_integral`: integrate the displacement thickness only up to eta = 10 with the local edge velocity (the corrected
/// measurement); otherwise over the whole column height with U = 1.
fn blasius_checks(belt: f64, edge_integral: bool) {
    let (fpp0, fp, dstar) = reference();
    let cf_coeff = 2.0 * fpp0;
    let m = sequence(belt);
    let mut c = Checks::default();
    for p in m {
        c.is_true(
            &format!("B0 n={}", p.n),
            p.converged,
            &format!("{} iterations", p.iterations),
        );
    }
    println!("Cf sqrt(Re_x) / 0.664115 - 1 [%] per mesh at x = 0.25, 0.5, 0.75, 0.9:");
    for p in m {
        let row: Vec<String> = [0.25, 0.5, 0.75, 0.9]
            .iter()
            .map(|&x| format!("{:+.2}", pct(cf_at(p, x) * (1.0e5 * x).sqrt(), cf_coeff)))
            .collect();
        println!("   n = {:4}: {}", p.n, row.join("  "));
    }
    let fine = &m[2];
    for x in [0.25, 0.5, 0.75, 0.9] {
        let v = cf_at(fine, x) * (1.0e5 * x).sqrt();
        c.at_most(
            &format!("B1 Cf sqrt(Re_x) at x={x} relative deviation"),
            (v / cf_coeff - 1.0).abs(),
            0.03,
        );
    }
    let scale = (1.0e5f64 / 0.5).sqrt();
    println!("profile at x = 0.5 (n=240): eta, u, Blasius f'");
    let mut worst = 0.0f64;
    for (eta, f1) in &fp {
        let u = u_at(fine, eta / scale);
        println!("   eta {eta}: {u:.5} vs {f1:.5}");
        worst = worst.max((u - f1).abs());
    }
    c.at_most("B2 profile max |u/U - f'|", worst, 0.02);
    let dstar_sim: f64 = if edge_integral {
        let y_e = 10.0 / scale;
        let u_e = u_at(fine, y_e);
        println!("edge velocity at eta = 10: {u_e:.5}");
        fine.column
            .iter()
            .map(|&(y, u, dy)| {
                let (lo, hi) = (y - 0.5 * dy, (y + 0.5 * dy).min(y_e));
                (1.0 - u / u_e) * (hi - lo).max(0.0)
            })
            .sum()
    } else {
        fine.column.iter().map(|&(_, u, dy)| (1.0 - u) * dy).sum()
    };
    let coeff = dstar_sim * scale;
    println!(
        "delta* sqrt(U/(nu x)) = {coeff:.5} vs {dstar:.5} ({:+.2} %)",
        pct(coeff, dstar)
    );
    c.at_most("B3 delta* relative deviation", (coeff / dstar - 1.0).abs(), 0.03);
    let f = |p: &Plate| cf_at(p, 0.5) * (1.0e5f64 * 0.5).sqrt();
    let order = observed_order(f(&m[0]), f(&m[1]), f(&m[2]), 2.0);
    let gci = gci_fine(f(&m[1]), f(&m[2]), order, 2.0);
    println!(
        "Cf sqrt(Re_x)(0.5): {:.5}, {:.5}, {:.5}; observed order {order:.2}, GCI_fine {:.3} %",
        f(&m[0]),
        f(&m[1]),
        f(&m[2]),
        100.0 * gci
    );
    c.at_least("B4 observed order", order, 1.0);
    c.at_most("B4 GCI_fine", gci, 0.01);
    c.finish();
}

/// Corrected set-up (shear-free moving-wall section of 0.5 upstream of the plate, free stream at the inlet), with the
/// limits B1 to B4 of ADR 0060 unchanged. Run time about 4 minutes in release.
#[test]
fn b1_b4_blasius_skin_friction_profile_and_displacement_thickness_with_grid_sequence() {
    blasius_checks(0.5, true);
}

/// RECORD OF THE SET-UP DEFECT (kept, ignored): the plate starting exactly at the fixed-velocity inlet. All of B1 to B4
/// miss (Cf sqrt(Re_x) +12 % at x = 0.5 on the finest mesh, free-stream overshoot of 4 % at the boundary-layer edge).
/// Bisection (diagnose_* tests): independent of the top boundary, domain height, scheme, coupling and wall grading;
/// removed by an upstream shear-free section (-0.7 % at n = 60), so the cause is the interaction of the leading edge
/// with the Dirichlet inlet, not the solver. Limits unchanged.
#[test]
#[ignore = "known set-up defect, superseded by the corrected test: cargo test --release -p rcfd-api --test validation_blasius_plate -- --ignored b1_b4_plate_at_inlet"]
fn b1_b4_plate_at_inlet_setup_defect_known_miss() {
    blasius_checks(0.0, false);
}

/// Diagnosis of the registered misses (ADR 0060 addendum): top boundary and domain height variations on the
/// fast meshes; prints Cf sqrt(Re_x) at x = 0.5 and the edge velocity. Not a validation test.
#[test]
#[ignore = "diagnosis, prints only"]
fn diagnose_blasius_setup() {
    for (n, h, top) in [
        (60, 0.3, Top::Open),
        (60, 0.3, Top::Slip),
        (60, 0.3, Top::Fixed),
        (60, 0.6, Top::Open),
        (60, 0.6, Top::Slip),
        (120, 0.3, Top::Slip),
    ] {
        let p = run_cfg(n, h, top, Outlet::Pressure);
        let u_top = p.column.last().unwrap().1;
        println!(
            "DIAG n={n} H={h} top={top:?}: Cf sqrt(Re_x)(0.5) = {:.5} ({:+.2} %), u at the last cell = {:.4}",
            cf_at(&p, 0.5) * (0.5e5f64).sqrt(),
            pct(cf_at(&p, 0.5) * (0.5e5f64).sqrt(), 0.664115),
            u_top
        );
    }
}

/// Further diagnosis: scheme, coupling, H-operator, wall grading and viscosity (n = 60, Cf sqrt(Re_x) at x = 0.5 is
/// compared with 0.664115 sqrt(Re_x) scaled to the actual viscosity). Not a validation test.
#[test]
#[ignore = "diagnosis, prints only"]
fn diagnose_blasius_numerics() {
    let base = Variant::default();
    let cases: Vec<(&str, Variant)> = vec![
        ("base", base),
        (
            "upwind",
            Variant {
                scheme: ConvScheme::Upwind,
                ..base
            },
        ),
        (
            "simple",
            Variant {
                coupling: Coupling::Simple,
                ..base
            },
        ),
        (
            "lagged H",
            Variant {
                h_from_solution: false,
                ..base
            },
        ),
        ("ratio 10", Variant { ratio: 10.0, ..base }),
        ("ratio 200", Variant { ratio: 200.0, ..base }),
        ("nu 1e-4", Variant { nu: 1e-4, ..base }),
        ("nu 1e-3", Variant { nu: 1e-3, ..base }),
    ];
    for (name, v) in cases {
        let p = run_full(60, 0.3, Top::Open, v);
        let re = 0.5 / v.nu;
        let cf = cf_at(&p, 0.5) * re.sqrt();
        let umax = p.column.iter().map(|c| c.1).fold(0.0, f64::max);
        println!(
            "DIAG2 {name}: Cf sqrt(Re_x) {cf:.5} ({:+.2} %), max u in the column {umax:.4}",
            pct(cf, 0.664115)
        );
    }
}

/// Prints the column u(y) at x = 0.5 (n = 60) to see where the overshoot sits. Not a validation test.
#[test]
#[ignore = "diagnosis, prints only"]
fn diagnose_blasius_column() {
    let p = run_full(60, 0.3, Top::Open, Variant::default());
    for (y, u, _) in p.column.iter().step_by(3) {
        println!("COL y {y:.5} eta {:.2} u {u:.4}", y * (1.0e5f64 / 0.5).sqrt());
    }
}

/// The plate at the inlet against an upstream shear-free moving-wall section of 0.5 (the corrected set-up under test).
#[test]
#[ignore = "diagnosis, prints only"]
fn diagnose_blasius_belt() {
    for belt in [0.0, 0.25, 0.5] {
        let p = run_full(
            60,
            0.3,
            Top::Open,
            Variant {
                belt,
                ..Variant::default()
            },
        );
        let umax = p.column.iter().map(|c| c.1).fold(0.0, f64::max);
        let cf = cf_at(&p, 0.5) * (0.5e5f64).sqrt();
        println!(
            "BELT {belt}: Cf sqrt(Re_x)(0.5) {cf:.5} ({:+.2} %), max u {umax:.4}",
            pct(cf, 0.664115)
        );
    }
}

/// KNOWN MISS (B3 as first measured, limit unchanged): delta* from the whole column height with U = 1 is -9.5 % on the
/// finest mesh of the corrected set-up. Diagnosis: a free-stream velocity offset of +0.2 % over the 0.3 high column
/// contributes -4e-4 against delta* = 3.8e-3; the integral is dominated by the outer flow, so the measurement is
/// ill-conditioned, not the boundary layer wrong (B1, B2 pass). The corrected measurement (to eta = 10 with the local edge
/// velocity) is in the main test with the same 3 % limit; this was decided after the miss was seen.
#[test]
#[ignore = "known miss of the first B3 measurement (limit 3 % unchanged), superseded: cargo test --release -p rcfd-api --test validation_blasius_plate -- --ignored b3_whole_column"]
fn b3_whole_column_displacement_thickness_known_miss() {
    let (_, _, dstar) = reference();
    let fine = &sequence(0.5)[2];
    let scale = (1.0e5f64 / 0.5).sqrt();
    let sim: f64 = fine.column.iter().map(|&(_, u, dy)| (1.0 - u) * dy).sum();
    let mut c = Checks::default();
    c.at_most(
        "B3 whole column delta* relative deviation",
        (sim * scale / dstar - 1.0).abs(),
        0.03,
    );
    c.finish();
}
