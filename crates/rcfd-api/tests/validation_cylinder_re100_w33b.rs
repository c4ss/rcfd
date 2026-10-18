//! Wave 33B (ADR 0069): the Re = 100 cylinder of `validation_cylinder_re100_w31.rs` ONE time, with `time_scheme = Bdf2` instead of the Euler default.
//! Set-up identical to the w31 runner (128 x 100, dt 0.05, PISO/PIMPLE n_outer 2, alpha 0.9, linear upwind, r_out 20, Expansion(150), t = 160, statistics
//! from t > 60, the last 8 whole periods). The registered decision rule (ADR 0069, fixed before the run):
//!
//! * K1b: St in [0.16005, 0.16995]; K2b: C_D in [1.2883, 1.3957]; K4: at least 9 zero crossings (the unchanged w31 limits).
//! * T1 (tight): St in [0.16339, 0.16733] and |C_D - 1.3437| <= 0.0101 (the dt-Richardson limits of ADR 0062, St 0.16536, C_D 1.3437).
//! * A guard trip (blow-up panic) means "BDF2 unstable at this set-up"; there is no second run.
//! * Euler baseline of the same set-up: St 0.15878, C_D 1.30996.
//!
//! Ignored (heavy). Command: `cargo test --release -p rcfd-api --test validation_cylinder_re100_w33b -- --ignored --nocapture`.

use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{boundary_values, pressure_bc};
use rcfd_fv::{FixedValue, Freestream, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{cylinder_ogrid, Grading};
use rcfd_post::integrals::wall_force_and_moment;
use std::sync::Arc;

const DEPTH: f64 = 0.1;
const NU: f64 = 0.01;

struct Shed {
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
    st.time_scheme = TimeScheme::Bdf2;
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
        crossings: k,
        st,
        cd: cd_mean,
    }
}

#[test]
#[ignore = "heavy (about 5 to 15 minutes), ADR 0069 registered single run"]
fn bdf2_128x100_dt0050() {
    let r = run(128, 100, 0.05);
    let t1 = (0.16339..=0.16733).contains(&r.st) && (r.cd - 1.3437).abs() <= 0.0101;
    println!(
        "RESULT33B K1b {} K2b {} K4 {} T1 {} (Euler baseline St 0.15878, C_D 1.30996)",
        (0.16005..=0.16995).contains(&r.st),
        (1.2883..=1.3957).contains(&r.cd),
        r.crossings > 9,
        t1
    );
    assert!(r.crossings > 9, "K4: {} crossings", r.crossings);
    assert!((0.16005..=0.16995).contains(&r.st), "K1b: St {}", r.st);
    assert!((1.2883..=1.3957).contains(&r.cd), "K2b: C_D {}", r.cd);
}
