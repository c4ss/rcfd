//! F-869 (ADR 0761): the mean drag coefficient of the laminar flow past a circular cylinder at Re = 100 against the band of
//! the published values, [1.314, 1.370] (arXiv:2303.09262 Table 1 and arXiv:1401.6513 Table 3; `docs/reference-data.md`,
//! "Cylinder wake: the mean drag coefficient at Re 100").
//!
//! Set-up of `validation_cylinder_re100_w33b.rs` (ADR 0069): O-grid, far field at 20 D, PISO with two outer iterations,
//! `time_scheme = Bdf2`, linear upwind, to t = 160, statistics over the last 8 whole shedding periods.
//!
//! * The gate runs `g1_...` (64 x 50, dt 0.05: G1 the mean drag in the window of K2 of ADR 0060, G2 sustained shedding, N1 the
//!   pressure part of the drag alone must fail) and the pure `g3_...` (the band is the recorded spread).
//! * `h1_..._heavy_record` (ignored, about 32 minutes) carries the comparison with the band itself: V1, V2a, S1, S2, K4h and
//!   the reported rows R1 and R2; `v2b_..._known_miss` (ignored) is the missed V2b, limit intact (amendment 1). Command:
//!   `cargo test --release -p rcfd-api --test f869_cylinder_re100_mean_drag -- --ignored --nocapture`.
//!
//! Every limit is registered in ADR 0761 and is not changed after a run. The existing ignored tests of
//! `validation_cylinder_re100.rs` (K1 to K4 and the known miss of K1 and K3) are untouched.

mod validation_common;

use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{boundary_values, pressure_bc};
use rcfd_fv::{FixedValue, Freestream, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{cylinder_ogrid, Grading};
use rcfd_post::integrals::wall_force_and_moment;
use std::sync::{Arc, OnceLock};
use validation_common::*;

const DEPTH: f64 = 0.1;
const NU: f64 = 0.01;

/// The band of the published mean drag coefficients at Re 100 (ADR 0761, "The band, registered").
const BAND: (f64, f64) = (1.314, 1.370);
/// The window of K2 of ADR 0060: within 4 % of 1.342, the band and 2 % for the discretisation.
const K2_WINDOW: (f64, f64) = (1.342 * 0.96, 1.342 * 1.04);
/// The window of K1 of ADR 0060 for the Strouhal number (reported, not a limit of this file).
const K1_WINDOW: (f64, f64) = (0.165 * 0.97, 0.165 * 1.03);

fn within(x: f64, window: (f64, f64)) -> bool {
    x.is_finite() && x >= window.0 && x <= window.1
}

/// What one run gives over its window of 8 whole periods.
struct Shed {
    label: String,
    crossings: usize,
    st: f64,
    /// Mean drag coefficient.
    cd: f64,
    /// Mean of the pressure part of the drag coefficient alone.
    cd_pressure: f64,
    /// Half of the largest less the smallest lift coefficient.
    cl_amplitude: f64,
    /// Root mean square of the lift coefficient about its mean.
    cl_rms: f64,
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

/// Force of the fluid on the cylinder: (F_x, the pressure part of F_x, F_y).
fn force(s: &PimpleSolver) -> (f64, f64, f64) {
    let pbc = pressure_bc(&s.geom, &s.u, &s.p);
    let pf = boundary_values(&s.geom, &pbc.coeffs, &s.p.values);
    let (f, _) = wall_force_and_moment(&s.geom, "cylinder", &s.u, &pf, NU, 1.0, vec3::ZERO).unwrap();
    (f.total()[0], f.pressure[0], f.total()[1])
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

/// Mean over `[ta, tb]` (trapezoid rule) of the force coefficient `f / (0.5 U^2 D depth)` with U = 1, D = 1.
fn mean_drag(t: &[f64], f: &[f64], ta: f64, tb: f64) -> f64 {
    let (mut integral, mut span) = (0.0, 0.0);
    for i in 1..t.len() {
        let (a, b) = (t[i - 1].max(ta), t[i].min(tb));
        if b > a {
            integral += 0.5 * (f[i - 1] + f[i]) * (b - a);
            span += b - a;
        }
    }
    integral / span / (0.5 * DEPTH)
}

fn run(n_theta: usize, n_r: usize, dt: f64, r_out: f64) -> Shed {
    let label = format!("{n_theta}x{n_r} dt {dt} r_out {r_out}");
    let mut s = setup(n_theta, n_r, r_out);
    let probe = nearest(&s, [3.0, 0.0, DEPTH / 2.0]);
    let (mut t, mut v) = (Vec::new(), Vec::new());
    let (mut tc, mut fx, mut fxp, mut fy) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let t0 = std::time::Instant::now();
    let mut next_report = 20.0;
    while s.time < 160.0 {
        let info = s.step(dt);
        // the blow-up guard of the w31 runner (ADR 0062): a diverged run fails here and is not repeated
        assert!(
            info.courant.is_finite() && info.courant < 1e4,
            "blow-up of {label} at t = {:.2}: Courant number {}",
            s.time,
            info.courant
        );
        if s.time >= next_report {
            println!("PROGRESS {label}: t {:.1}, {:.0} s", s.time, t0.elapsed().as_secs_f64());
            next_report += 20.0;
        }
        if s.time > 60.0 {
            t.push(s.time);
            v.push(s.u.values[probe][1]);
        }
        if s.time > 85.0 {
            let (x, xp, y) = force(&s);
            tc.push(s.time);
            fx.push(x);
            fxp.push(xp);
            fy.push(y);
        }
    }
    let cross = crossings(&t, &v);
    let n_use = 8;
    let k = cross.len();
    assert!(k > n_use, "{label}: only {k} zero crossings, no sustained shedding");
    let (ta, tb) = (cross[k - 1 - n_use], cross[k - 1]);
    let st = n_use as f64 / (tb - ta);
    let cd = mean_drag(&tc, &fx, ta, tb);
    let cd_pressure = mean_drag(&tc, &fxp, ta, tb);
    let cl_mean = mean_drag(&tc, &fy, ta, tb);
    let cl: Vec<f64> = tc
        .iter()
        .zip(&fy)
        .filter(|(&time, _)| time >= ta && time <= tb)
        .map(|(_, &y)| y / (0.5 * DEPTH))
        .collect();
    let cl_max = cl.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let cl_min = cl.iter().cloned().fold(f64::INFINITY, f64::min);
    let cl_rms = (cl.iter().map(|c| (c - cl_mean).powi(2)).sum::<f64>() / cl.len() as f64).sqrt();
    let r = Shed {
        label,
        crossings: k,
        st,
        cd,
        cd_pressure,
        cl_amplitude: 0.5 * (cl_max - cl_min),
        cl_rms,
    };
    println!(
        "RESULT {}: {} crossings, St {:.5} ({:+.2} % of 0.165, in K1's window: {}), C_D {:.5} (pressure part {:.5}), \
         C_L amplitude {:.4}, C_L rms {:.4}, window {ta:.1}..{tb:.1}, {:.0} s",
        r.label,
        r.crossings,
        r.st,
        pct(r.st, 0.165),
        within(r.st, K1_WINDOW),
        r.cd,
        r.cd_pressure,
        r.cl_amplitude,
        r.cl_rms,
        t0.elapsed().as_secs_f64()
    );
    r
}

/// G3 (ADR 0761): the band of this file is the smallest and the largest drag coefficient of the recorded table, and the
/// check of the band rejects a drag that is outside it by more than its width.
#[test]
fn g3_the_band_is_the_recorded_spread_and_rejects_a_drag_outside_it() {
    let rows = read_rows(&reference_dir().join("cylinder_re100_literature.csv"));
    let cds: Vec<f64> = rows.iter().filter_map(|r| r[1].parse().ok()).collect();
    assert_eq!(cds.len(), 9, "nine published drag coefficients in the recorded table");
    let lo = cds.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = cds.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    println!("recorded C_D {lo} .. {hi}; band {} .. {}", BAND.0, BAND.1);
    assert_eq!((lo, hi), BAND);
    let width = BAND.1 - BAND.0;
    assert!((width - 0.056).abs() < 1e-12, "width {width}");
    // the two primary values of ADR 0761 (the papers' own computations) lie in the band
    assert!(within(1.359, BAND) && within(1.328, BAND));
    assert!(within(BAND.0, BAND) && within(BAND.1, BAND));
    assert!(!within(BAND.0 - 0.0561, BAND) && !within(BAND.1 + 0.0561, BAND));
    assert!(!within(f64::NAN, BAND));
    // the window of K2 holds the band and is wider by 2 % of its middle on each side
    assert!(K2_WINDOW.0 < BAND.0 && K2_WINDOW.1 > BAND.1);
    assert!((K2_WINDOW.0 - 1.28832).abs() < 1e-9 && (K2_WINDOW.1 - 1.39568).abs() < 1e-9);
}

/// G1, G2 and N1 (ADR 0761), run by the gate: the coarse mesh through the same path as the heavy record.
#[test]
fn g1_coarse_mesh_mean_drag_within_the_k2_window_and_the_pressure_part_alone_fails() {
    let r = run(64, 50, 0.05, 20.0);
    let mut c = Checks::default();
    c.is_true(
        "G1 mean C_D on 64x50 in [1.2883, 1.3957]",
        within(r.cd, K2_WINDOW),
        &format!("C_D {:.5} ({:+.2} % of 1.342)", r.cd, pct(r.cd, 1.342)),
    );
    c.is_true(
        "G2 more than 9 zero crossings",
        r.crossings > 9,
        &format!("{} crossings", r.crossings),
    );
    c.is_true(
        "N1a the pressure part alone fails the window of G1",
        !within(r.cd_pressure, K2_WINDOW),
        &format!("pressure part {:.5}", r.cd_pressure),
    );
    c.at_least(
        "N1b the pressure part is under the full drag by at least the width of the band",
        r.cd - r.cd_pressure,
        0.056,
    );
    c.finish();
}

/// The five runs of the heavy record, one after the other, made once for the ignored tests of this file.
struct Record {
    coarse: Shed,
    medium: Shed,
    fine: Shed,
    fine_half_dt: Shed,
    far: Shed,
}

fn record() -> &'static Record {
    static RECORD: OnceLock<Record> = OnceLock::new();
    RECORD.get_or_init(|| Record {
        coarse: run(64, 50, 0.05, 20.0),
        medium: run(90, 70, 0.05, 20.0),
        fine: run(128, 100, 0.05, 20.0),
        fine_half_dt: run(128, 100, 0.025, 20.0),
        far: run(128, 100, 0.05, 30.0),
    })
}

/// Of the three meshes at dt 0.05: (monotone, observed order, the order used, the Richardson value).
fn richardson(m: &Record) -> (bool, f64, f64, f64) {
    let ratio = 2f64.sqrt();
    let (f1, f2, f3) = (m.coarse.cd, m.medium.cd, m.fine.cd);
    let monotone = (f1 - f2) * (f2 - f3) > 0.0;
    let p_obs = observed_order(f1, f2, f3, ratio);
    let p = if p_obs.is_finite() { p_obs.clamp(1.0, 2.0) } else { 1.0 };
    (monotone, p_obs, p, f3 + (f3 - f2) / (ratio.powf(p) - 1.0))
}

/// The heavy record of ADR 0761 (V1, V2a, S1, S2, K4h; R1 and R2 printed). V2b is the known miss below (amendment 1).
/// Recorded on 2026-11-22 (1897 s): C_D 1.31674, 1.33879, 1.35247 on the three meshes at dt 0.05, 1.34511 at dt 0.025,
/// 1.33884 with the far field at 30 D; GCI_fine 2.06 %, GCI_t 0.23 %.
#[test]
#[ignore = "heavy record of ADR 0761 (about 32 minutes): cargo test --release -p rcfd-api --test f869_cylinder_re100_mean_drag -- --ignored --nocapture"]
fn h1_fine_mesh_mean_drag_in_the_published_band_heavy_record() {
    let m = record();
    let mut c = Checks::default();
    for r in [&m.coarse, &m.medium, &m.fine, &m.fine_half_dt, &m.far] {
        c.is_true(
            &format!("K4h {}", r.label),
            r.crossings > 9,
            &format!("{} crossings", r.crossings),
        );
    }
    c.is_true(
        "V1 mean C_D on 128x100 dt 0.05 in [1.314, 1.370]",
        within(m.fine.cd, BAND),
        &format!("C_D {:.5} ({:+.2} % of 1.342)", m.fine.cd, pct(m.fine.cd, 1.342)),
    );
    c.is_true(
        "V2a mean C_D on 128x100 dt 0.025 in [1.314, 1.370]",
        within(m.fine_half_dt.cd, BAND),
        &format!("C_D {:.5}", m.fine_half_dt.cd),
    );
    let (f1, f2, f3) = (m.coarse.cd, m.medium.cd, m.fine.cd);
    let (monotone, p_obs, p, value) = richardson(m);
    println!(
        "mesh sequence at dt 0.05: C_D {f1:.5}, {f2:.5}, {f3:.5}; monotone {monotone}; observed order {p_obs:.2} (used {p:.2}); Richardson value {value:.5}"
    );
    c.at_most(
        "S1 GCI_fine(C_D), three meshes at dt 0.05",
        gci_fine(f2, f3, p_obs, 2f64.sqrt()),
        0.03,
    );
    let g = m.fine_half_dt.cd;
    c.at_most(
        "S2 GCI_t(C_D), dt 0.05 and 0.025 on 128x100, order 2",
        1.25 * (g - f3).abs() / (g.abs() * 3.0),
        0.03,
    );
    println!(
        "R2 far field at 30 D: C_D {:.5} ({:+.2} % of the 20 D value), in the band: {}",
        m.far.cd,
        pct(m.far.cd, m.fine.cd),
        within(m.far.cd, BAND)
    );
    c.finish();
}

/// KNOWN MISS (V2b of ADR 0761; the band unchanged): the Richardson value of the mean drag on 64 x 50, 90 x 70, 128 x 100
/// at dt 0.05 is 1.37479 (observed order 1.38), 0.0048 above the upper end of the band [1.314, 1.370]. The fine-mesh
/// value itself, 1.35247, is in the band (V1). Diagnosis and what is missing: ADR 0761, addendum 1.
#[test]
#[ignore = "known miss V2b (Richardson value 1.37479 against [1.314, 1.370]); heavy, shares the runs of h1: -- --ignored v2b"]
fn v2b_richardson_value_of_the_mesh_sequence_in_the_band_known_miss() {
    let (monotone, _, _, value) = richardson(record());
    let mut c = Checks::default();
    c.is_true(
        "V2b the Richardson value of the three meshes in [1.314, 1.370]",
        monotone && within(value, BAND),
        &format!("monotone {monotone}, Richardson value {value:.5}"),
    );
    c.finish();
}
