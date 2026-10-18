//! Wave 31A (ADR 0062, item 3): remaining registered bisection steps (ii) finer mesh and (iv) probe position for the Re = 100 cylinder
//! of `validation_cylinder_re100.rs`, and an independent time-step / mesh study on the matrix {64x50, 90x70, 128x100} x dt {0.1, 0.05, 0.025}
//! plus 128x100 at dt 0.0125 and 181x141 at dt 0.05 and 0.025. Set-up exactly that of the wave 30B file (PISO, linear upwind, alpha 0.9, n_outer 2,
//! r_out 20, Expansion(150), t = 160, statistics from t > 60, 8 whole periods); this runner also records a second probe at (2, 0, 0.1).
//!
//! Every test is ignored (each run is 1 to 20 minutes). Cell tests print one `RESULT` line; the corrected sequence asserts the unchanged
//! limits K1 to K4. Command: `cargo test --release -p rcfd-api --test validation_cylinder_re100_w31 -- --ignored --nocapture <name>`.

mod validation_common;

use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{boundary_values, pressure_bc};
use rcfd_fv::{FixedValue, Freestream, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{cylinder_ogrid, Grading};
use rcfd_post::integrals::wall_force_and_moment;
use std::sync::Arc;
use validation_common::*;

const DEPTH: f64 = 0.1;
const NU: f64 = 0.01;

struct Shed {
    n_theta: usize,
    n_r: usize,
    dt: f64,
    crossings: usize,
    st: f64,
    st2: f64,
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

fn drag(s: &PimpleSolver) -> f64 {
    let pbc = pressure_bc(&s.geom, &s.u, &s.p);
    let pf = boundary_values(&s.geom, &pbc.coeffs, &s.p.values);
    let (f, _) = wall_force_and_moment(&s.geom, "cylinder", &s.u, &pf, NU, 1.0, vec3::ZERO).unwrap();
    f.total()[0] / (0.5 * DEPTH)
}

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

/// St from the last 8 whole periods of the zero crossings; returns (St, crossings, window start, window end).
fn strouhal(t: &[f64], v: &[f64]) -> (f64, usize, f64, f64) {
    let cross = crossings(t, v);
    let n_use = 8;
    let k = cross.len();
    assert!(k > n_use, "K4: only {k} zero crossings, no sustained shedding");
    let (ta, tb) = (cross[k - 1 - n_use], cross[k - 1]);
    (n_use as f64 / (tb - ta), k, ta, tb)
}

fn run(n_theta: usize, n_r: usize, dt: f64) -> Shed {
    let mut s = setup(n_theta, n_r, 20.0);
    let probe = nearest(&s, [3.0, 0.0, DEPTH / 2.0]);
    let probe2 = nearest(&s, [2.0, 0.0, DEPTH]);
    let (mut t, mut v, mut v2) = (Vec::new(), Vec::new(), Vec::new());
    let (mut tc, mut cd) = (Vec::new(), Vec::new());
    let t0 = std::time::Instant::now();
    let mut next_report = 10.0;
    while s.time < 160.0 {
        let info = s.step(dt);
        // blow-up guard (wave 31A): the 181x141 run at dt 0.05 diverges at t = 35 and the pressure solver then spends 12000 iterations per step on NaN
        assert!(
            info.courant.is_finite() && info.courant < 1e4,
            "blow-up at t = {:.2}: Courant number {}",
            s.time,
            info.courant
        );
        if s.time >= next_report {
            let umax = s.u.values.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f64::max);
            println!(
                "PROGRESS {n_theta}x{n_r} dt {dt}: t {:.1}, |u|max {umax:.3}, {:.0} s",
                s.time,
                t0.elapsed().as_secs_f64()
            );
            next_report += 10.0;
        }
        if s.time > 60.0 {
            t.push(s.time);
            v.push(s.u.values[probe][1]);
            v2.push(s.u.values[probe2][1]);
        }
        if s.time > 85.0 {
            tc.push(s.time);
            cd.push(drag(&s));
        }
    }
    let (st, k, ta, tb) = strouhal(&t, &v);
    let (st2, _, _, _) = strouhal(&t, &v2);
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
        "RESULT mesh {n_theta}x{n_r} dt {dt} : {k} crossings, St {st:.5}, St(probe 2) {st2:.5}, C_D {cd_mean:.5} (window {ta:.1}..{tb:.1}), {:.0}s",
        t0.elapsed().as_secs_f64()
    );
    Shed {
        n_theta,
        n_r,
        dt,
        crossings: k,
        st,
        st2,
        cd: cd_mean,
    }
}

macro_rules! cell {
    ($name:ident, $nt:expr, $nr:expr, $dt:expr) => {
        #[test]
        #[ignore = "heavy diagnosis cell, prints one RESULT line; see the file header"]
        fn $name() {
            let r = run($nt, $nr, $dt);
            assert!(r.crossings > 9, "K4: {} crossings", r.crossings);
        }
    };
}

cell!(cell_64x50_dt0100, 64, 50, 0.1);
cell!(cell_64x50_dt0050, 64, 50, 0.05);
cell!(cell_90x70_dt0100, 90, 70, 0.1);
cell!(cell_90x70_dt0050, 90, 70, 0.05);
cell!(cell_128x100_dt0100, 128, 100, 0.1);
cell!(cell_128x100_dt0125, 128, 100, 0.0125);
// bisection step (ii): next finer mesh (ratio sqrt 2 on the fine mesh), same dt as the fine mesh, and half of it
cell!(step_ii_181x141_dt0025, 181, 141, 0.025);

/// KNOWN MISS (registered step (ii), ADR 0062 addendum 3): 181 x 141 at the dt = 0.05 of the 128 x 100 mesh is numerically unstable (Courant number 10 to 30
/// from t = 28, diverging at t = 35, then NaN). Measured by `probe_181x141_dt0050_slow_step`; this test was not re-run after the blow-up guard was added
/// (both permitted runs of that cell were used), the guard turns the former endless NaN loop into a panic at t about 35.
#[test]
#[ignore = "known miss: dt 0.05 is unstable on 181x141 (ADR 0062 addendum 3); about 3 minutes until the guard panics"]
fn step_ii_181x141_dt0050_known_miss() {
    let r = run(181, 141, 0.05);
    assert!(r.crossings > 9, "K4: {} crossings", r.crossings);
}

/// Bisection step (iv): probe-position independence, 128x100 at dt 0.05 (the fine mesh of the wave 30B sequence) with the second
/// probe at (2, 0, 0.1). Registered criterion: St from the two probes agrees to 0.5 %.
#[test]
#[ignore = "heavy (about 4 minutes)"]
fn step_iv_probe_independence() {
    let r = run(128, 100, 0.05);
    let rel = (r.st2 / r.st - 1.0).abs();
    let mut c = Checks::default();
    c.at_most("step (iv) |St(probe 2)/St(probe 1) - 1|", rel, 0.005);
    c.finish();
}

/// Corrected sequence (ADR 0062 item 3, registered before any run): the three matrix meshes at the common dt = 0.025, so that the observed
/// order and GCI are spatial only. Limits as the wave 30B file: K1 St within 3 % of 0.165, K2 C_D within 4 % of 1.342, K3 GCI_fine of St and
/// C_D <= 3 % (refinement ratio sqrt 2), K4 at least 9 zero crossings. The old mixed sequence stays in `validation_cylinder_re100.rs`.
#[test]
#[ignore = "heavy (about 12 minutes): cargo test --release -p rcfd-api --test validation_cylinder_re100_w31 -- --ignored --nocapture k_corrected"]
fn k_corrected_sequence_common_dt_0025() {
    let m: Vec<Shed> = [(64, 50), (90, 70), (128, 100)]
        .iter()
        .map(|&(a, b)| run(a, b, 0.025))
        .collect();
    let mut c = Checks::default();
    for r in &m {
        c.is_true(
            &format!("K4 {}x{} dt {}", r.n_theta, r.n_r, r.dt),
            r.crossings > 9,
            &format!("{} crossings", r.crossings),
        );
    }
    let f = &m[2];
    c.at_most("K1 St relative deviation from 0.165", (f.st / 0.165 - 1.0).abs(), 0.03);
    c.at_most("K2 C_D relative deviation from 1.342", (f.cd / 1.342 - 1.0).abs(), 0.04);
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

/// Cost probe (prints only): wall time per step and the largest speed after 60 steps, for a mesh and dt.
#[test]
#[ignore = "cost probe, prints only"]
fn timing_probe_181x141() {
    for (nt, nr) in [(128usize, 100usize), (181, 141)] {
        let mut s = setup(nt, nr, 20.0);
        let t0 = std::time::Instant::now();
        for _ in 0..60 {
            s.step(0.05);
        }
        let umax = s.u.values.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f64::max);
        println!(
            "TIMING {nt}x{nr}: {:.2} s per step over 60 steps, |u|max {umax:.3}, cells {}",
            t0.elapsed().as_secs_f64() / 60.0,
            s.u.values.len()
        );
    }
}

/// Probe for ADR 0062 addendum 3 (prints only): 181x141 at dt 0.05, one line per 0.5 time units from t = 28; stops itself when a step takes more than
/// 30 s of wall time, or a value is not finite, or t reaches 60.
#[test]
#[ignore = "probe, prints only"]
fn probe_181x141_dt0050_slow_step() {
    let mut s = setup(181, 141, 20.0);
    let t0 = std::time::Instant::now();
    let mut next = 28.0;
    loop {
        let t_step = std::time::Instant::now();
        let info = s.step(0.05);
        let wall = t_step.elapsed().as_secs_f64();
        let umax = s.u.values.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f64::max);
        let bad = !umax.is_finite() || !info.courant.is_finite() || wall > 30.0;
        if s.time >= next || bad || wall > 3.0 {
            println!(
                "PROBE t {:.2}: |u|max {umax:.3}, Courant {:.2}, p_iters {}, continuity {:.2e}, res_u {:.2e}, res_p {:.2e}, step {wall:.2} s, total {:.0} s",
                s.time,
                info.courant,
                info.p_iters,
                info.continuity,
                info.res_u,
                info.res_p,
                t0.elapsed().as_secs_f64()
            );
            if s.time >= next {
                next += 0.5;
            }
        }
        if bad || s.time >= 60.0 {
            break;
        }
    }
}
