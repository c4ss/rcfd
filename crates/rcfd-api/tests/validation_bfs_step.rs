//! Laminar backward-facing step, expansion ratio 2, Re_D = 800 (Re_h = 400): reattachment length against the ten
//! published values 11.48 to 12.20 read in Table 2 of arXiv:2507.16509, with a three-mesh sequence (ADR 0060, checks
//! S1 to S3). Armaly et al. (1983) and the Re_D < 800 benchmark values were not obtained: no check.
//!
//! Run (about 10 minutes in release): `cargo test --release -p rcfd-api --test validation_bfs_step -- --ignored`.

mod validation_common;

use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{backward_step_graded, StepGrading, StepSpec};
use std::sync::Arc;
use validation_common::*;

struct Run {
    cells: usize,
    iterations: usize,
    converged: bool,
    x_r: f64,
}

/// Reattachment length (x - l_in, in step heights): last negative-to-positive crossing of the streamwise velocity in
/// the first cell row above the floor, linear interpolation (the definition of `step_laminar.rs`).
fn reattachment(s: &SimpleSolver, l_in: f64, step: f64) -> Option<f64> {
    let cc = s.geom.mesh.cell_centres();
    let ymin = cc.iter().map(|c| c[1]).fold(f64::INFINITY, f64::min);
    let mut row: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| (cc[c][1] - ymin).abs() < 1e-9 && cc[c][0] > l_in)
        .map(|c| (cc[c][0], s.u.values[c][0]))
        .collect();
    row.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out = None;
    for w in row.windows(2) {
        if w[0].1 < 0.0 && w[1].1 >= 0.0 {
            let t = -w[0].1 / (w[1].1 - w[0].1);
            out = Some(w[0].0 + t * (w[1].0 - w[0].0) - l_in);
        }
    }
    out.map(|x| x / step)
}

fn run(re_h: f64, n: [usize; 4]) -> Run {
    let spec = StepSpec {
        l_in: 2.0,
        l_out: 40.0,
        h_in: 1.0,
        step: 1.0,
        depth: 0.1,
        n,
    };
    let geom =
        Arc::new(FvGeometry::new(Arc::new(backward_step_graded(spec, StepGrading::uniform()).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let patch = geom.mesh.patch("inlet").unwrap().clone();
    let cf = geom.mesh.face_centres();
    let prof: Vec<[f64; 3]> = patch
        .faces()
        .map(|f| {
            let s = (cf[f][1] - spec.step) / spec.h_in;
            [6.0 * s * (1.0 - s), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(spec.step / re_h, Coupling::Simplec);
    st.convection = ConvScheme::Tvd(TvdLimiter::VanLeer);
    st.h_from_solution = true;
    let cells = geom.n_cells();
    let mut solver = SimpleSolver::new(geom, u, p, st);
    let t0 = std::time::Instant::now();
    let (mut iterations, mut converged) = (8000, false);
    for i in 1..=8000 {
        let info = solver.iterate();
        let res = info.res_u.max(info.res_p);
        if !res.is_finite() || res > 1e8 {
            iterations = i;
            break;
        }
        if res < 1e-7 {
            iterations = i;
            converged = true;
            break;
        }
    }
    let x_r = reattachment(&solver, spec.l_in, spec.step).unwrap_or(f64::NAN);
    println!(
        "step Re_h {re_h} mesh {n:?} ({cells} cells): {iterations} iterations, converged {converged}, x_r/h = {x_r:.4}, {:.0}s",
        t0.elapsed().as_secs_f64()
    );
    Run {
        cells,
        iterations,
        converged,
        x_r,
    }
}

fn sequence() -> &'static Vec<Run> {
    static SEQ: std::sync::OnceLock<Vec<Run>> = std::sync::OnceLock::new();
    SEQ.get_or_init(|| {
        [[8, 160, 16, 16], [16, 320, 32, 32], [32, 640, 64, 64]]
            .iter()
            .map(|&n| run(400.0, n))
            .collect()
    })
}

#[test]
#[ignore = "heavy (minutes): cargo test --release -p rcfd-api --test validation_bfs_step -- --ignored"]
fn s1_s3_step_re_d_800_reattachment_against_published_range_with_grid_sequence() {
    let rows = read_rows(&reference_dir().join("bfs_er2_re800_xr_literature.csv"));
    let vals: Vec<f64> = rows.iter().map(|r| r[1].parse().unwrap()).collect();
    let (lo, hi) = (
        vals.iter().cloned().fold(f64::INFINITY, f64::min),
        vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let mid = 0.5 * (lo + hi);
    println!(
        "published x_r/h: {lo} .. {hi} (mid {mid:.3}, half-width {:.2} %)",
        100.0 * (hi - lo) / (hi + lo)
    );
    let m = sequence();
    let mut c = Checks::default();
    for r in m {
        c.is_true(
            &format!("S3 {} cells", r.cells),
            r.converged,
            &format!("{} iterations, x_r/h {:.4}", r.iterations, r.x_r),
        );
    }
    let fine = m[2].x_r;
    println!(
        "x_r/h on the three meshes: {:.4}, {:.4}, {:.4}; deviation of the fine mesh from the mid-range {:+.2} %",
        m[0].x_r,
        m[1].x_r,
        fine,
        pct(fine, mid)
    );
    c.is_true(
        "S1 x_r/h in [11.248, 12.432]",
        (11.248..=12.432).contains(&fine),
        &format!("{fine:.4}"),
    );
    let p = observed_order(m[0].x_r, m[1].x_r, m[2].x_r, 2.0);
    let gci = gci_fine(m[1].x_r, m[2].x_r, p, 2.0);
    println!("observed order {p:.2}, GCI_fine {:.3} %", 100.0 * gci);
    c.at_least("S2 observed order", p, 0.9);
    c.at_most("S2 GCI_fine(x_r)", gci, 0.03);
    c.finish();
}
