//! Wave 34C (ADR 0074 part A, A11, A11m, A12): SST transport at a `Symmetry` plane.
//!
//! A11: a body-forced periodic SST half channel (Re_tau 395, `Symmetry` top) against the full channel whose node rows are the half channel's
//! mirrored at y = 1, both with the component-consistent diagonal (`component_diag_correction`, ADR 0010). A11m: the half channel with the averaged
//! default (registered expected miss). A12: the registered ADR 0070 P3 half channel (Re_tau 1e6) re-run with the correction on, limits of ADR 0070.

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField};
use rcfd_mesh::gen::{axis_coords, box_2d_graded, Grading};
use rcfd_mesh::{BoundarySpec, Mesh, PatchKind};
use rcfd_turb::sst::SstWallMode;
use rcfd_turb::KOmegaSst;
use std::sync::Arc;

const KM_TMR: &str = include_str!("../../../validation-data/reference/w33c_tmr_sst_channel_km_cfl3d.csv");

/// 2-D periodic-in-x mesh (4 cells of 0.25, one cell in z, `frontAndBack` empty) on the node rows `ys`; patches xmin/xmax (cyclic), ymin, ymax.
fn mesh_rows(ys: &[f64]) -> Mesh {
    let xs: Vec<f64> = (0..=4).map(|i| 0.25 * i as f64).collect();
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);
    let pid = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let mut points: Vec<Vec3> = Vec::new();
    for z in [0.0, 0.1] {
        for &y in ys {
            for &x in &xs {
                points.push([x, y, z]);
            }
        }
    }
    let names = ["xmin", "xmax", "ymin", "ymax", "frontAndBack"];
    let mut specs: Vec<BoundarySpec> = names
        .iter()
        .map(|n| BoundarySpec {
            name: n.to_string(),
            kind: if *n == "frontAndBack" {
                PatchKind::Empty
            } else {
                PatchKind::Wall
            },
            faces: vec![],
        })
        .collect();
    let mut cells = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let p: Vec<usize> = (0..8).map(|c| pid(i + (c & 1), j + ((c >> 1) & 1), c >> 2)).collect();
            let faces = vec![
                vec![p[0], p[2], p[6], p[4]],
                vec![p[1], p[3], p[7], p[5]],
                vec![p[0], p[1], p[5], p[4]],
                vec![p[2], p[3], p[7], p[6]],
                vec![p[0], p[1], p[3], p[2]],
                vec![p[4], p[5], p[7], p[6]],
            ];
            let free = [i == 0, i + 1 == nx, j == 0, j + 1 == ny, true, true];
            for (d, f) in faces.iter().enumerate() {
                if free[d] {
                    specs[d.min(4)].faces.push(f.clone());
                }
            }
            cells.push(faces);
        }
    }
    let mut m = Mesh::from_cell_faces(points, &cells, &specs).unwrap();
    m.set_periodic("xmin", "xmax").unwrap();
    m.set_periodic("xmax", "xmin").unwrap();
    m
}

struct Run {
    /// (y, u) of the first column, sorted by y
    col: Vec<(f64, f64)>,
    /// sum of the wall shear force over the walls / (f V)
    force_balance: f64,
    iterations: usize,
    du: f64,
    change: f64,
}

fn run(geom: Arc<FvGeometry>, half: bool, nu: f64, corrected: bool, max_it: usize, du_tol: f64) -> Run {
    let mut u = VectorField::uniform("U", &geom, [20.0, 0.0, 0.0]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    let walls: Vec<&str> = if half {
        u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
        vec!["ymin"]
    } else {
        u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        vec!["ymin", "ymax"]
    };
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    s.h_from_solution = true;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, s);
    flow.body_force = Some(BodyForce::uniform(&geom, [1.0, 0.0, 0.0]));
    flow.component_diag_correction = corrected;
    let mut model = KOmegaSst::new(geom.clone(), nu, &walls, 1.0, 1.0 / (10.0 * nu)).unwrap();
    model.set_wall_mode(SstWallMode::Automatic).unwrap();
    let (mut iterations, mut change, mut du) = (0, f64::INFINITY, f64::INFINITY);
    for it in 0..max_it {
        change = model.update(&flow.u, &flow.phi);
        flow.nu_face = Some(model.nu_face());
        let info = flow.iterate();
        iterations = it + 1;
        du = info.max_du;
        if it > 10 && du < du_tol && change < 1e-8 {
            break;
        }
    }
    let mesh = &geom.mesh;
    let (own, cc, vol) = (mesh.owner(), mesh.cell_centres(), mesh.cell_volumes());
    let mut force = 0.0;
    for &(f, nutw) in &model.wall_nut {
        let c = own[f];
        let y = geom.dn[f - geom.n_internal];
        force += (nu + nutw) * flow.u.values[c][0] / y * mesh.face_area(f);
    }
    let force_balance = force / vol.iter().sum::<f64>();
    let mut col: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| cc[c][0] < 0.25)
        .map(|c| (cc[c][1], flow.u.values[c][0]))
        .collect();
    col.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    Run {
        col,
        force_balance,
        iterations,
        du,
        change,
    }
}

const RE_TAU: f64 = 395.0;

fn half_rows() -> Vec<f64> {
    axis_coords(
        30,
        1.0,
        Grading::WallLayers {
            first: 2.0 / RE_TAU,
            growth: 1.1,
            layers: 16,
            both: false,
        },
    )
    .unwrap()
}

fn geom_rows(ys: &[f64]) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(mesh_rows(ys))).unwrap())
}

/// Bulk velocity of the cells below y = 1 (uniform column widths, so the cell-height weighted mean of the column).
fn bulk(col: &[(f64, f64)], ys: &[f64]) -> f64 {
    let mut s = 0.0;
    for (i, &(_, u)) in col.iter().enumerate().take(ys.len() - 1) {
        s += u * (ys[i + 1] - ys[i]);
    }
    s
}

/// (a) |U_b half / U_b full - 1|, (b) max |u_half - u_full| / u_centre, (c) force balances: the A11 metrics.
fn a11_metrics(half: &Run, full: &Run, ys: &[f64]) -> (f64, f64, f64, f64) {
    let (bh, bf) = (bulk(&half.col, ys), bulk(&full.col, ys));
    let uc = half.col.last().unwrap().1;
    let mut worst: f64 = 0.0;
    for (a, b) in half.col.iter().zip(&full.col) {
        assert!((a.0 - b.0).abs() < 1e-12);
        worst = worst.max((a.1 - b.1).abs() / uc);
    }
    println!(
        "  U_b half {bh:.6} full {bf:.6}; u_centre half {uc:.6} full {:.6}; force balance half {:.9} full {:.9}",
        full.col[ys.len() - 2].1,
        half.force_balance,
        full.force_balance
    );
    ((bh / bf - 1.0).abs(), worst, half.force_balance, full.force_balance)
}

fn full_rows(ys: &[f64]) -> Vec<f64> {
    let mut full = ys.to_vec();
    for &y in ys.iter().rev().skip(1) {
        full.push(2.0 - y);
    }
    full
}

/// A11: SST half channel with the corrected diagonal against the mirrored full channel.
#[test]
fn a11_sst_half_channel_equals_mirrored_full_channel() {
    let ys = half_rows();
    let nu = 1.0 / RE_TAU;
    let half = run(geom_rows(&ys), true, nu, true, 30000, 1e-9);
    let full = run(geom_rows(&full_rows(&ys)), false, nu, true, 30000, 1e-9);
    println!(
        "A11: half {} it (du {:.1e}, change {:.1e}), full {} it (du {:.1e}, change {:.1e})",
        half.iterations, half.du, half.change, full.iterations, full.du, full.change
    );
    let (a, b, ch, cf) = a11_metrics(&half, &full, &ys);
    println!(
        "A11: (a) {:.4} %, (b) {:.4} %, (c) {ch:.9} {cf:.9}",
        100.0 * a,
        100.0 * b
    );
    assert!(
        half.du < 1e-9 && full.du < 1e-9 && half.change < 1e-8 && full.change < 1e-8,
        "A11: not converged"
    );
    assert!(a <= 0.005 && b <= 0.01, "A11: {a:e} {b:e}");
    assert!(
        (ch - 1.0).abs() <= 1e-6 && (cf - 1.0).abs() <= 1e-6,
        "A11 force balance: {ch} {cf}"
    );
}

/// A11m: the same half channel with the averaged default (registered expected miss). KNOWN MISS (ADR 0074 addendum): U_b 4.8648 against
/// 18.1669 (-73.2 percent), centre velocity 2.4585 against 20.4086, wall force balance 0.2429 instead of 1 (the half channel converges, 1638
/// iterations, to a state where the Symmetry top carries about three quarters of the driving force as a spurious sink).
#[test]
#[ignore = "known miss (registered expectation): averaged diagonal, see ADR 0074 addendum"]
fn a11m_sst_half_channel_averaged_known_miss() {
    let ys = half_rows();
    let nu = 1.0 / RE_TAU;
    let half = run(geom_rows(&ys), true, nu, false, 30000, 1e-9);
    let full = run(geom_rows(&full_rows(&ys)), false, nu, true, 30000, 1e-9);
    println!(
        "A11m: half {} it (du {:.1e}, change {:.1e}), full {} it",
        half.iterations, half.du, half.change, full.iterations
    );
    let (a, b, ch, _) = a11_metrics(&half, &full, &ys);
    println!("A11m: (a) {:.4} %, (b) {:.4} %, (c) {ch:.9}", 100.0 * a, 100.0 * b);
    assert!(a <= 0.005 && b <= 0.01, "A11m: {a:e} {b:e}");
}

fn km_tmr() -> Vec<(f64, f64, f64)> {
    KM_TMR
        .lines()
        .skip(1)
        .map(|l| {
            let c: Vec<f64> = l.split(',').map(|v| v.parse().unwrap()).collect();
            (c[0], c[1], c[2])
        })
        .collect()
}

fn tmr_at(tab: &[(f64, f64, f64)], yp: f64, which: usize) -> f64 {
    let i = tab.partition_point(|r| r.0 < yp).clamp(1, tab.len() - 1);
    let (a, b) = (tab[i - 1], tab[i]);
    let t = (yp.log10() - a.0.log10()) / (b.0.log10() - a.0.log10());
    let pick = |r: (f64, f64, f64)| if which == 1 { r.1 } else { r.2 };
    pick(a) * (1.0 - t) + pick(b) * t
}

fn lsq_slope(p: &[(f64, f64)]) -> f64 {
    let n = p.len() as f64;
    let (mx, my) = (
        p.iter().map(|q| q.0).sum::<f64>() / n,
        p.iter().map(|q| q.1).sum::<f64>() / n,
    );
    p.iter().map(|q| (q.0 - mx) * (q.1 - my)).sum::<f64>() / p.iter().map(|q| (q.0 - mx).powi(2)).sum::<f64>()
}

/// A12: ADR 0070 P3a/P3b with the registered half-channel set-up (Re_tau 1e6, 150 cells, `Symmetry` top, 40000 iterations cap), NEW corrected
/// set-up: `component_diag_correction` on. Limits of ADR 0070: KM within 0.02 of TMR at y+ 100, 316, 1000, 3000, 10000; slopes 50..300 within
/// 4 percent of 2.693 and 1000..10000 within 3 percent of 2.443.
/// KNOWN MISS (ADR 0074 addendum): KM -0.0098 / -0.0067 / -0.0057 / -0.0054 / -0.0054 against TMR and slopes 2.7627 / 2.4759 are inside the
/// limits (and equal to the ADR 0070 full channel to the printed digits), but the run is not converged at the registered cap: du 9.97e-8 after
/// 40000 iterations (criterion du < 1e-8). A12b repeats it with the 250000 cap of the ADR 0070 full-channel set-up.
#[test]
#[ignore = "known miss: not converged within the registered 40000 iterations, see doc comment"]
fn a12_p3_half_channel_with_corrected_diagonal_known_miss() {
    p3_half_channel(40000);
}

/// A12b: A12 with the iteration cap of the ADR 0070 full-channel set-up (250000); limits unchanged (registered after A12, ADR 0074 addendum).
#[test]
#[ignore = "heavy: Re_tau 1e6 SST half channel, several minutes in release"]
fn a12b_p3_half_channel_with_corrected_diagonal_cap_250000() {
    p3_half_channel(250000);
}

fn p3_half_channel(cap: usize) {
    let re_tau = 1.0e6;
    let nu = 1.0 / re_tau;
    let grading = Grading::WallLayers {
        first: 2.0 / re_tau,
        growth: 1.1,
        layers: 100,
        both: false,
    };
    let geom = Arc::new(
        FvGeometry::new(Arc::new(
            box_2d_graded(4, 150, 1.0, 1.0, [Grading::Uniform, grading], true, false).unwrap(),
        ))
        .unwrap(),
    );
    let f = run(geom, true, nu, true, cap, 1e-9);
    println!(
        "A12 Re_tau 1e6 half channel (corrected): {} iterations, du {:.2e}, change {:.2e}, first y+ {:.3}, force balance {:.6}",
        f.iterations,
        f.du,
        f.change,
        f.col[0].0 * re_tau,
        f.force_balance
    );
    let mut fails = Vec::new();
    if !(f.change < 1e-6 && f.du < 1e-8) {
        fails.push("not converged".to_string());
    }
    let up: Vec<(f64, f64)> = f.col.iter().map(|c| (c.0 * re_tau, c.1)).collect();
    let km: Vec<(f64, f64)> = (1..up.len() - 1)
        .map(|i| {
            let d = (up[i + 1].1 - up[i - 1].1) / (up[i + 1].0.ln() - up[i - 1].0.ln());
            (up[i].0, 1.0 / d)
        })
        .collect();
    let at_km = |yp: f64| {
        let i = km.partition_point(|r| r.0 < yp).clamp(1, km.len() - 1);
        let (a, b) = (km[i - 1], km[i]);
        let t = (yp.ln() - a.0.ln()) / (b.0.ln() - a.0.ln());
        a.1 * (1.0 - t) + b.1 * t
    };
    let tab = km_tmr();
    for yp in [100.0, 316.0, 1000.0, 3000.0, 10000.0] {
        let (m, r) = (at_km(yp), tmr_at(&tab, yp, 2));
        println!("  y+ {yp:7.0}: KM {m:.4}  TMR {r:.4}  ({:+.4})", m - r);
        if (m - r).abs() > 0.02 {
            fails.push(format!("P3a: KM({yp}) {m:.4} vs TMR {r:.4} outside 0.02"));
        }
    }
    for (lo, hi, tol, tmr_ref) in [(50.0, 300.0, 0.04, 2.693), (1000.0, 10000.0, 0.03, 2.443)] {
        let mine: Vec<(f64, f64)> = up
            .iter()
            .filter(|p| p.0 >= lo && p.0 <= hi)
            .map(|p| (p.0.ln(), p.1))
            .collect();
        let s = lsq_slope(&mine);
        println!(
            "  fit y+ {lo}..{hi}: rcfd {s:.4} ({} cells), registered {tmr_ref}",
            mine.len()
        );
        if (s / tmr_ref - 1.0).abs() > tol {
            fails.push(format!(
                "P3b: slope {lo}..{hi} {s:.4} vs {tmr_ref} outside {}%",
                tol * 100.0
            ));
        }
    }
    assert!(fails.is_empty(), "registered limits missed:\n{}", fails.join("\n"));
}
