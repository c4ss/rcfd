//! Laminar vortex shedding behind a circular cylinder at Re = 100: Strouhal number and mean drag against the
//! published range read in arXiv:2303.09262 Table 1 and arXiv:1401.6513 Table 3 (St 0.164 to 0.167, C_D 1.314
//! to 1.370; experiment St 0.165), with a three-mesh and three-time-step sequence (ADR 0060, checks K1 to K4).
//!
//! Heavy (about 7 minutes in release): `cargo test --release -p rcfd-api --test validation_cylinder_re100 -- --ignored`.

mod validation_common;

use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{boundary_values, pressure_bc};
use rcfd_fv::{FixedValue, Freestream, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{cylinder_ogrid, Grading};
use rcfd_post::integrals::wall_force_and_moment;
use std::sync::{Arc, OnceLock};
use validation_common::*;

const DEPTH: f64 = 0.1;
const NU: f64 = 0.01;

struct Shed {
    n_theta: usize,
    n_r: usize,
    dt: f64,
    crossings: usize,
    st: f64,
    cd: f64,
}

fn setup(n_theta: usize, n_r: usize, r_out: f64) -> PimpleSolver {
    let mesh = Arc::new(cylinder_ogrid(n_theta, n_r, 0.5, r_out, Grading::Expansion(150.0), DEPTH).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let uinf = [1.0, 0.0, 0.0];
    let mut u = VectorField::from_fn("U", &geom, |c| {
        let blob = 0.1 * (-((c[0] - 2.0).powi(2) + (c[1] - 0.3).powi(2))).exp();
        [1.0, blob, 0.0]
    });
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: uinf,
            fixed_on_inflow: true,
        }),
    )
    .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: [0.0; 3],
            fixed_on_inflow: false,
        }),
    )
    .unwrap();
    let mut st = PimpleSettings::piso(NU);
    st.convection = ConvScheme::LinearUpwind;
    st.n_outer = 2;
    st.amg_pressure = true;
    st.alpha_u = 0.9;
    st.alpha_p = 0.9;
    PimpleSolver::new(geom, u, p, st)
}

fn nearest(s: &PimpleSolver, x: [f64; 3]) -> usize {
    let c = s.geom.mesh.cell_centres();
    (0..c.len())
        .min_by(|&a, &b| {
            let d = |i: usize| (c[i][0] - x[0]).powi(2) + (c[i][1] - x[1]).powi(2);
            d(a).total_cmp(&d(b))
        })
        .unwrap()
}

/// Drag coefficient F_x / (0.5 U^2 D depth) with D = 1, U = 1.
fn drag(s: &PimpleSolver) -> f64 {
    let pbc = pressure_bc(&s.geom, &s.u, &s.p);
    let pf = boundary_values(&s.geom, &pbc.coeffs, &s.p.values);
    let (f, _) = wall_force_and_moment(&s.geom, "cylinder", &s.u, &pf, NU, 1.0, vec3::ZERO).unwrap();
    f.total()[0] / (0.5 * DEPTH)
}

/// Upward zero crossings (linear interpolation) of `v - mean`.
fn crossings(t: &[f64], v: &[f64]) -> Vec<f64> {
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let mut cross = Vec::new();
    for i in 1..v.len() {
        if v[i - 1] - mean < 0.0 && v[i] - mean >= 0.0 {
            let w = (mean - v[i - 1]) / (v[i] - v[i - 1]);
            cross.push(t[i - 1] + w * (t[i] - t[i - 1]));
        }
    }
    cross
}

fn run(n_theta: usize, n_r: usize, dt: f64, r_out: f64) -> Shed {
    let mut s = setup(n_theta, n_r, r_out);
    let probe = nearest(&s, [3.0, 0.0, DEPTH / 2.0]);
    let (mut t, mut v) = (Vec::new(), Vec::new());
    let (mut tc, mut cd) = (Vec::new(), Vec::new());
    let t0 = std::time::Instant::now();
    while s.time < 160.0 {
        s.step(dt);
        if s.time > 60.0 {
            t.push(s.time);
            v.push(s.u.values[probe][1]);
        }
        if s.time > 85.0 {
            tc.push(s.time);
            cd.push(drag(&s));
        }
    }
    let cross = crossings(&t, &v);
    let n_use = 8;
    let k = cross.len();
    assert!(k > n_use, "K4: only {k} zero crossings, no sustained shedding");
    let (ta, tb) = (cross[k - 1 - n_use], cross[k - 1]);
    let st = n_use as f64 / (tb - ta);
    // trapezoid mean of C_D over the whole periods [ta, tb]
    let (mut integral, mut span) = (0.0, 0.0);
    for i in 1..tc.len() {
        let (a, b) = (tc[i - 1].max(ta), tc[i].min(tb));
        if b > a {
            integral += 0.5 * (cd[i - 1] + cd[i]) * (b - a);
            span += b - a;
        }
    }
    let cd_mean = integral / span;
    println!(
        "cylinder {n_theta}x{n_r} dt {dt} r_out {r_out}: {k} crossings, St {st:.5}, C_D {cd_mean:.5} (window {ta:.1}..{tb:.1}), {:.0}s",
        t0.elapsed().as_secs_f64()
    );
    Shed {
        n_theta,
        n_r,
        dt,
        crossings: k,
        st,
        cd: cd_mean,
    }
}

fn sequence() -> &'static Vec<Shed> {
    static SEQ: OnceLock<Vec<Shed>> = OnceLock::new();
    SEQ.get_or_init(|| {
        [(64, 50, 0.1), (90, 70, 0.07), (128, 100, 0.05)]
            .iter()
            .map(|&(a, b, dt)| run(a, b, dt, 20.0))
            .collect()
    })
}

fn literature() -> (Vec<f64>, Vec<f64>) {
    let rows = read_rows(&reference_dir().join("cylinder_re100_literature.csv"));
    let cds = rows.iter().filter_map(|r| r[1].parse().ok()).collect();
    let sts = rows.iter().filter_map(|r| r[2].parse().ok()).collect();
    (cds, sts)
}

#[test]
#[ignore = "heavy (about 7 minutes): cargo test --release -p rcfd-api --test validation_cylinder_re100 -- --ignored"]
fn k1_k4_cylinder_re100_strouhal_and_drag_with_grid_and_time_step_sequence() {
    let (cds, sts) = literature();
    let (cd_lo, cd_hi) = (
        cds.iter().cloned().fold(f64::INFINITY, f64::min),
        cds.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let (st_lo, st_hi) = (
        sts.iter().cloned().fold(f64::INFINITY, f64::min),
        sts.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    println!("literature C_D {cd_lo} .. {cd_hi}, St {st_lo} .. {st_hi}");
    let m = sequence();
    let mut c = Checks::default();
    for r in m {
        c.is_true(
            &format!("K4 {}x{} dt {}", r.n_theta, r.n_r, r.dt),
            r.crossings > 9,
            &format!("{} crossings", r.crossings),
        );
        println!(
            "   {}x{} dt {}: St {:.5} ({:+.2} % of 0.165), C_D {:.5} ({:+.2} % of 1.342)",
            r.n_theta,
            r.n_r,
            r.dt,
            r.st,
            pct(r.st, 0.165),
            r.cd,
            pct(r.cd, 1.342)
        );
    }
    let f = &m[2];
    c.at_most("K2 C_D relative deviation from 1.342", (f.cd / 1.342 - 1.0).abs(), 0.04);
    c.finish();
}

/// KNOWN MISS (K1, K3; limits unchanged): St = 0.14665, 0.15450, 0.15878 on the three meshes (fine: -3.77 % of 0.165,
/// limit 3 %), observed order 1.75, GCI_fine(St) 4.1 % and GCI_fine(C_D) 3.7 % (limit 3 %). The sequence is monotone and
/// approaches the published window from below; mesh and time step are refined together, so the two errors are not
/// separated by this sequence (see the `diagnose_*` test).
#[test]
#[ignore = "known miss K1/K3 (St -3.8 %, GCI 4.1 %); cargo test --release -p rcfd-api --test validation_cylinder_re100 -- --ignored k1_k3"]
fn k1_k3_strouhal_and_gci_known_miss() {
    let m = sequence();
    let f = &m[2];
    let mut c = Checks::default();
    c.at_most("K1 St relative deviation from 0.165", (f.st / 0.165 - 1.0).abs(), 0.03);
    let r = 2f64.sqrt();
    for (name, g) in [
        ("St", (m[0].st, m[1].st, m[2].st)),
        ("C_D", (m[0].cd, m[1].cd, m[2].cd)),
    ] {
        let p = observed_order(g.0, g.1, g.2, r);
        let gci = gci_fine(g.1, g.2, p, r);
        println!("{name}: observed order {p:.2}, GCI_fine {:.3} %", 100.0 * gci);
        c.at_most(&format!("K3 GCI_fine({name})"), gci, 0.03);
    }
    c.finish();
}

/// Bisection aid (prints only, about 14 minutes): the fine mesh with half the time step, then with r_out = 30.
#[test]
#[ignore = "diagnosis, prints only (about 14 minutes)"]
fn diagnose_cylinder_dt_and_domain() {
    run(128, 100, 0.025, 20.0);
    run(128, 100, 0.05, 30.0);
}
