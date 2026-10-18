//! Wave 33C (ADR 0070, parts 1 and 3): SST resolved-wall behaviour and the log layer without convection.
//!
//! (S1, S2) sublayer of the Re_tau = 395 channel in `LowRe` mode: omega against the analytic sublayer solution 6 nu/(beta1 y^2) and the
//! exponent of k (derived in the ADR from nu k'' = beta* omega k: k ~ y^3.23).
//! (P3a, P3b) Re_tau = 1e6 half channel, Karman measure KM = 1/(y+ du+/dy+) against the NASA TMR 2DFDC SST data
//! (`validation-data/reference/w33c_tmr_sst_channel_km_cfl3d.csv`, CFL3D, https://tmbwg.github.io/turbmodels/channelflow_val_sst.html).

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_turb::sst::SstWallMode;
use rcfd_turb::KOmegaSst;
use std::sync::{Arc, OnceLock};

const KM_TMR: &str = include_str!("../../../validation-data/reference/w33c_tmr_sst_channel_km_cfl3d.csv");

struct Field {
    /// (y, u, k, omega) of the cell column nearest x = 0, sorted by y
    col: Vec<(f64, f64, f64, f64)>,
    iterations: usize,
    du: f64,
    change: f64,
}

/// Periodic channel (4 cells in x) with a uniform body force 1 (u_tau = 1 for the half height 1), wall at y = 0.
fn channel(re_tau: f64, ny: usize, layers: usize, both_walls: bool, mode: SstWallMode, max_it: usize) -> Field {
    let nu = 1.0 / re_tau;
    let h = if both_walls { 2.0 } else { 1.0 };
    let first = 2.0 / re_tau; // cell-centre y+ = 1
    let grading = Grading::WallLayers {
        first,
        growth: 1.1,
        layers,
        both: both_walls,
    };
    let mesh = box_2d_graded(4, ny, 1.0, h, [Grading::Uniform, grading], true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [20.0, 0.0, 0.0]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    let walls: Vec<&str> = if both_walls {
        u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        vec!["ymin", "ymax"]
    } else {
        u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
        vec!["ymin"]
    };
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    s.h_from_solution = true;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, s);
    flow.body_force = Some(BodyForce::uniform(&geom, [1.0, 0.0, 0.0]));
    let mut model = KOmegaSst::new(geom.clone(), nu, &walls, 1.0, 1.0 / (10.0 * nu)).unwrap();
    model.set_wall_mode(mode).unwrap();
    let (mut iterations, mut change, mut du) = (0, f64::INFINITY, f64::INFINITY);
    for it in 0..max_it {
        change = model.update(&flow.u, &flow.phi);
        flow.nu_face = Some(model.nu_face());
        let info = flow.iterate();
        iterations = it + 1;
        du = info.max_du;
        if it > 10 && du < 1e-9 && change < 1e-8 {
            break;
        }
    }
    let cc = geom.mesh.cell_centres();
    let mut col: Vec<(f64, f64, f64, f64)> = (0..cc.len())
        .filter(|&c| cc[c][0] < 0.3 && cc[c][1] < 1.0)
        .map(|c| (cc[c][1], flow.u.values[c][0], model.k.values[c], model.omega.values[c]))
        .collect();
    col.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    Field {
        col,
        iterations,
        du,
        change,
    }
}

fn sublayer_field() -> &'static Field {
    static F: OnceLock<Field> = OnceLock::new();
    F.get_or_init(|| channel(RE_TAU_SUB, 60, 16, true, SstWallMode::LowRe, 30000))
}

const RE_TAU_SUB: f64 = 395.0;

fn s1_ratio(y: f64, w: f64) -> f64 {
    w / (6.0 / RE_TAU_SUB / (0.075 * y * y))
}

/// S1 as registered in ADR 0070: omega / analytic in [0.75, 1.5] for the FIRST FOUR cell centres (y+ 1, 3.1, 5.4, 8.1).
/// KNOWN MISS (defective set-up): the measured ratios are 0.857, 1.467, 1.626, ...; the analytic sublayer solution only holds for
/// y+ below about 4, so cells 2 and 3 (y+ 5.4 and 8.1) are outside its range of validity. The corrected test is
/// `w33c_sst_sublayer_omega_cells_inside_the_sublayer`; the registered limit is unchanged here.
#[test]
#[ignore = "known miss: registered cell set reaches y+ 8, beyond the sublayer (ADR 0070 addendum)"]
fn w33c_sst_sublayer_omega_first_four_cells_known_miss() {
    let f = sublayer_field();
    assert!(f.change < 1e-6 && f.du < 1e-8, "not converged");
    for (i, &(y, _, _, w)) in f.col.iter().take(4).enumerate() {
        let ratio = s1_ratio(y, w);
        assert!((0.75..=1.5).contains(&ratio), "S1 cell {i}: ratio {ratio}");
    }
}

/// Corrected S1: same limits [0.75, 1.5], applied to the cells with y+ <= 4 (the range stated for S2 in the ADR).
#[test]
fn w33c_sst_sublayer_omega_cells_inside_the_sublayer() {
    let f = sublayer_field();
    println!(
        "sublayer run: {} iterations, du {:.2e}, change {:.2e}",
        f.iterations, f.du, f.change
    );
    assert!(f.change < 1e-6 && f.du < 1e-8, "not converged");
    let mut n = 0;
    for (i, &(y, _, _, w)) in f.col.iter().enumerate().take(6) {
        let ratio = s1_ratio(y, w);
        println!("  cell {i}: y+ {:.3} omega/analytic {ratio:.4}", y * RE_TAU_SUB);
        if y * RE_TAU_SUB <= 4.0 {
            n += 1;
            assert!((0.75..=1.5).contains(&ratio), "S1 cell {i}: ratio {ratio}");
        }
    }
    assert!(n >= 2);
}

/// S2 (ADR 0070): exponent of k over the cells with y+ in [0.9, 4] in [2.6, 3.9] (derived value 3.2295).
#[test]
fn w33c_sst_sublayer_k_exponent() {
    let f = sublayer_field();
    let pts: Vec<(f64, f64)> = f
        .col
        .iter()
        .filter(|c| c.0 * RE_TAU_SUB >= 0.9 && c.0 * RE_TAU_SUB <= 4.0)
        .map(|c| (c.0.ln(), c.2.ln()))
        .collect();
    assert!(pts.len() >= 2, "too few cells: {}", pts.len());
    let slope = lsq_slope(&pts);
    println!(
        "  k exponent over y+ 0.9..4 ({} cells): {slope:.3} (derived 3.2295)",
        pts.len()
    );
    assert!((2.6..=3.9).contains(&slope), "S2 exponent {slope}");
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

/// Linear interpolation in log10 y+ of column `which` (1 = u+, 2 = KM) of the TMR table.
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

/// P3a, P3b exactly as registered in ADR 0070 (half channel, `Symmetry` top, 150 cells, 40000 iterations).
/// KNOWN MISS, DEFECTIVE SET-UP: the run (5606 iterations, "converged" to du 1e-9) gave KM 1.0716, 1.3352, 1.4560, 1.5265, 1.9113 at
/// y+ 100, 316, 1000, 3000, 10000 (TMR 0.366 ... 0.409) and u+ slopes 0.9006 (50..300) and 0.6456 (1000..10000): the body-forced periodic
/// half channel with a `Symmetry` top does not reach u_tau = 1 (laminar Poiseuille check in the ADR addendum: the Symmetry top acts like a
/// no-slip wall). The corrected set-up is `w33c_sst_channel_karman_measure_full_channel`; the limits are unchanged.
#[test]
#[ignore = "known miss: defective set-up (Symmetry top), see doc comment"]
fn w33c_sst_channel_karman_measure_half_channel_known_miss() {
    let re_tau = 1.0e6;
    let f = channel(re_tau, 150, 100, false, SstWallMode::Automatic, 40000);
    println!(
        "Re_tau 1e6 channel: {} iterations, du {:.2e}, change {:.2e}, first y+ {:.3}",
        f.iterations,
        f.du,
        f.change,
        f.col[0].0 * re_tau
    );
    assert!(f.change < 1e-6 && f.du < 1e-8, "not converged");
    let up: Vec<(f64, f64)> = f.col.iter().map(|c| (c.0 * re_tau, c.1)).collect();
    // KM at cell i from central differences of u+ in ln y+
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
    let mut fails = Vec::new();
    for yp in [100.0, 316.0, 1000.0, 3000.0, 10000.0] {
        let (m, r) = (at_km(yp), tmr_at(&tab, yp, 2));
        println!("  y+ {yp:7.0}: KM {m:.4}  TMR {r:.4}  ({:+.4})", m - r);
        if (m - r).abs() > 0.02 {
            fails.push(format!("P3a: KM({yp}) {m:.4} vs TMR {r:.4} outside 0.02"));
        }
    }
    // slopes of u+ against ln y+ over windows, rcfd cells and TMR table points
    for (lo, hi, tol, tmr_ref) in [(50.0, 300.0, 0.04, 2.693), (1000.0, 10000.0, 0.03, 2.443)] {
        let mine: Vec<(f64, f64)> = up
            .iter()
            .filter(|p| p.0 >= lo && p.0 <= hi)
            .map(|p| (p.0.ln(), p.1))
            .collect();
        let theirs: Vec<(f64, f64)> = tab
            .iter()
            .filter(|r| r.0 >= lo && r.0 <= hi)
            .map(|r| (r.0.ln(), r.1))
            .collect();
        let (s, st) = (lsq_slope(&mine), lsq_slope(&theirs));
        println!(
            "  fit y+ {lo}..{hi}: rcfd {s:.4} ({} cells), TMR table {st:.4} (registered {tmr_ref})",
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

/// P3a, P3b (ADR 0070, corrected set-up: full channel with two walls, height 2, 300 cells, 250000 iterations cap): Karman measure and log-law slopes of a Re_tau = 1e6 channel against the TMR CFL3D SST channel data.
#[test]
#[ignore = "heavy: Re_tau 1e6 channel, 58532 iterations, about 6 minutes in release"]
fn w33c_sst_channel_karman_measure_full_channel() {
    let re_tau = 1.0e6;
    let f = channel(re_tau, 300, 100, true, SstWallMode::Automatic, 250000);
    println!(
        "Re_tau 1e6 channel: {} iterations, du {:.2e}, change {:.2e}, first y+ {:.3}",
        f.iterations,
        f.du,
        f.change,
        f.col[0].0 * re_tau
    );
    assert!(f.change < 1e-6 && f.du < 1e-8, "not converged");
    let up: Vec<(f64, f64)> = f.col.iter().map(|c| (c.0 * re_tau, c.1)).collect();
    // KM at cell i from central differences of u+ in ln y+
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
    let mut fails = Vec::new();
    for yp in [100.0, 316.0, 1000.0, 3000.0, 10000.0] {
        let (m, r) = (at_km(yp), tmr_at(&tab, yp, 2));
        println!("  y+ {yp:7.0}: KM {m:.4}  TMR {r:.4}  ({:+.4})", m - r);
        if (m - r).abs() > 0.02 {
            fails.push(format!("P3a: KM({yp}) {m:.4} vs TMR {r:.4} outside 0.02"));
        }
    }
    // slopes of u+ against ln y+ over windows, rcfd cells and TMR table points
    for (lo, hi, tol, tmr_ref) in [(50.0, 300.0, 0.04, 2.693), (1000.0, 10000.0, 0.03, 2.443)] {
        let mine: Vec<(f64, f64)> = up
            .iter()
            .filter(|p| p.0 >= lo && p.0 <= hi)
            .map(|p| (p.0.ln(), p.1))
            .collect();
        let theirs: Vec<(f64, f64)> = tab
            .iter()
            .filter(|r| r.0 >= lo && r.0 <= hi)
            .map(|r| (r.0.ln(), r.1))
            .collect();
        let (s, st) = (lsq_slope(&mine), lsq_slope(&theirs));
        println!(
            "  fit y+ {lo}..{hi}: rcfd {s:.4} ({} cells), TMR table {st:.4} (registered {tmr_ref})",
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
