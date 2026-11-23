//! F-881 (ADR 0771): why the mean drag of the cylinder at Re = 100 converges at an observed order near 1.4.
//!
//! The baseline B0 is the set-up and the mesh sequence of ADR 0763 (90 x 70, 128 x 100, 181 x 141; BDF2 at dt 0.025; far
//! field at 30 D; `Expansion(150)`; linear upwind; PISO with two outer iterations). Three candidates, each changing one
//! thing against B0, are run and classified by the order they give:
//!
//! * C1, the wall force integral: second-order wall terms (pressure extrapolated linearly to the wall, the wall-normal
//!   derivative of the velocity from the quadratic through the wall and the first two cells) on the fields of B0;
//! * C2, the grading of the O-grid: `Expansion(50)` in place of `Expansion(150)`;
//! * C3, the far-field condition (as corrected by amendment 1): the velocity fixed at the free stream on every far-field
//!   face in place of `Freestream`, so that the pressure is zero gradient there and its level is set by the reference cell.
//!
//! A candidate moves the order if its order differs from B0's by more than 0.1, the same on the whole window and on each
//! half (ADR 0771, "What decides").
//!
//! * The gate runs `g1_...` (the wall terms on a manufactured field), `g2_...` (the Richardson procedure), `n1_...` (the
//!   classification and the decision) and `g4_...` (the three set-ups on 90 x 70, 40 steps through the heavy path).
//! * `h1_...` to `h4_...` (ignored) are the heavy records, nine solves:
//!   `cargo test --release -p rcfd-api --test f881_cylinder_drag_order -- --ignored --nocapture --test-threads=1`.
//!
//! Every limit is registered in ADR 0771 and is not changed after a run. The tests of ADR 0761 and ADR 0763 are untouched.

mod validation_common;

use rcfd_core::vec3::{self, Vec3};
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
const R_OUT: f64 = 30.0;
const DT: f64 = 0.025;
/// The meshes of ADR 0763 (n_theta, n_r), coarse to fine.
const MESHES: [(usize, usize); 3] = [(90, 70), (128, 100), (181, 141)];
/// The range of row "Observed order of accuracy" of the tolerance table (within 0.5 of the formal order 2).
const ORDER_RANGE: (f64, f64) = (1.5, 2.5);
/// The amount by which a candidate must move the order (row "Observed order of accuracy", its 0.1).
const AMOUNT: f64 = 0.1;
/// The record of ADR 0763 (addendum 1), mean drag on the three meshes.
const RECORD_0763: [f64; 3] = [1.32259, 1.33285, 1.33900];

/// The force variants recorded per step: the standard integral, C1, C1 with the pressure term only, C1 with the
/// velocity term only, and the pressure and viscous parts of the standard integral.
const NV: usize = 6;
const VARIANT: [&str; NV] = ["standard", "C1", "C1p", "C1v", "pressure part", "viscous part"];
const STD: usize = 0;
const C1: usize = 1;

fn within(x: f64, w: (f64, f64)) -> bool {
    x.is_finite() && x >= w.0 && x <= w.1
}

/// The three set-ups: B0 and the two candidates that need their own solves.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SetUp {
    B0,
    C2Grading,
    C3FarField,
}

fn setup(set_up: SetUp, n_theta: usize, n_r: usize) -> PimpleSolver {
    let ratio = if set_up == SetUp::C2Grading { 50.0 } else { 150.0 };
    let mesh = Arc::new(cylinder_ogrid(n_theta, n_r, 0.5, R_OUT, Grading::Expansion(ratio), DEPTH).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let uinf = [1.0, 0.0, 0.0];
    let mut u = VectorField::from_fn("U", &geom, |c| {
        let blob = 0.1 * (-((c[0] - 2.0).powi(2) + (c[1] - 0.3).powi(2))).exp();
        [1.0, blob, 0.0]
    });
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    if set_up == SetUp::C3FarField {
        // amendment 1: the velocity fixed on every far-field face; the pressure is then zero gradient there (`pressure_bc`)
        u.set_bc(&geom, "farfield", Box::new(FixedValue::vector(uinf))).unwrap();
    } else {
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
    }
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

/// A face of the patch `cylinder` with its wall cell P (ring 0) and the cell Q of ring 1 on the same ray.
struct WallFace {
    face: usize,
    cell_p: usize,
    cell_q: usize,
    /// Unit outward normal (out of the fluid, into the cylinder).
    n: Vec3,
    area: f64,
    /// Distances of the centres of P and Q from the face, along the normal, into the fluid.
    s_p: f64,
    s_q: f64,
}

/// The wall faces of the O-grid (cell index `it + n_theta ir`), with the assertion that P and Q lie on the face normal.
fn wall_faces(geom: &FvGeometry, n_theta: usize) -> Vec<WallFace> {
    let mesh = &geom.mesh;
    let (sf, cf, cc, own) = (
        mesh.face_area_vectors(),
        mesh.face_centres(),
        mesh.cell_centres(),
        mesh.owner(),
    );
    mesh.patch("cylinder")
        .unwrap()
        .faces()
        .map(|f| {
            let cell_p = own[f];
            assert!(cell_p < n_theta, "the owner of a wall face is in ring 0");
            let cell_q = cell_p + n_theta;
            let n = vec3::unit(sf[f]);
            let (dp, dq) = (vec3::sub(cf[f], cc[cell_p]), vec3::sub(cf[f], cc[cell_q]));
            let (s_p, s_q) = (vec3::dot(dp, n), vec3::dot(dq, n));
            assert!(s_p > 0.0 && s_q > s_p, "face {f}: s_P {s_p}, s_Q {s_q}");
            assert!(
                vec3::norm(vec3::cross(dp, n)) <= 1e-9 * s_p,
                "face {f}: P off the normal"
            );
            assert!(
                vec3::norm(vec3::cross(dq, n)) <= 1e-9 * s_q,
                "face {f}: Q off the normal"
            );
            WallFace {
                face: f,
                cell_p,
                cell_q,
                n,
                area: vec3::norm(sf[f]),
                s_p,
                s_q,
            }
        })
        .collect()
}

/// The wall terms of one face: the code's outward-normal derivative of each velocity component and its face pressure,
/// and the second-order ones of C1.
#[derive(Clone, Copy, Debug)]
struct WallTerms {
    dn_code: Vec3,
    dn_second: Vec3,
    p_code: f64,
    p_second: f64,
}

/// du/ds at s = 0 of the quadratic through (0, 0), (s_p, u_p), (s_q, u_q).
fn wall_derivative(u_p: f64, u_q: f64, s_p: f64, s_q: f64) -> f64 {
    (u_p * s_q * s_q - u_q * s_p * s_p) / (s_p * s_q * (s_q - s_p))
}

/// p at s = 0 of the line through (s_p, p_p), (s_q, p_q).
fn wall_pressure(p_p: f64, p_q: f64, s_p: f64, s_q: f64) -> f64 {
    p_p - (p_q - p_p) * s_p / (s_q - s_p)
}

fn wall_terms(geom: &FvGeometry, faces: &[WallFace], u: &VectorField, p_face: &[f64], p: &[f64]) -> Vec<WallTerms> {
    let coeffs: Vec<_> = (0..3).map(|k| u.boundary_coeffs(geom, k)).collect();
    faces
        .iter()
        .map(|w| {
            let i = w.face - geom.n_internal;
            let (up, uq) = (u.values[w.cell_p], u.values[w.cell_q]);
            WallTerms {
                dn_code: [0, 1, 2].map(|k| coeffs[k][i].gradient(up[k])),
                // the outward normal points into the wall, against s
                dn_second: [0, 1, 2].map(|k| -wall_derivative(up[k], uq[k], w.s_p, w.s_q)),
                p_code: p_face[i],
                p_second: wall_pressure(p[w.cell_p], p[w.cell_q], w.s_p, w.s_q),
            }
        })
        .collect()
}

/// The x-force of every variant and the standard y-force of the fluid on the cylinder.
fn forces(s: &PimpleSolver, faces: &[WallFace]) -> ([f64; NV], f64) {
    let pbc = pressure_bc(&s.geom, &s.u, &s.p);
    let pf = boundary_values(&s.geom, &pbc.coeffs, &s.p.values);
    let (f, _) = wall_force_and_moment(&s.geom, "cylinder", &s.u, &pf, NU, 1.0, vec3::ZERO).unwrap();
    let terms = wall_terms(&s.geom, faces, &s.u, &pf, &s.p.values);
    let (mut dp, mut dv) = (vec3::ZERO, vec3::ZERO);
    for (w, t) in faces.iter().zip(&terms) {
        dp = vec3::add(dp, vec3::scale(w.n, w.area * (t.p_second - t.p_code)));
        // the traction of `force_and_moment` changes by delta + (n . delta) n for a change delta of the normal derivative
        let delta = vec3::sub(t.dn_second, t.dn_code);
        let dtr = vec3::add(delta, vec3::scale(w.n, vec3::dot(w.n, delta)));
        dv = vec3::add(dv, vec3::scale(dtr, -NU * w.area));
    }
    let fx = f.total()[0];
    (
        [
            fx,
            fx + dp[0] + dv[0],
            fx + dp[0],
            fx + dv[0],
            f.pressure[0],
            f.viscous[0],
        ],
        f.total()[1],
    )
}

struct Series {
    label: String,
    t: Vec<f64>,
    v: Vec<f64>,
    tc: Vec<f64>,
    fx: [Vec<f64>; NV],
    fy: Vec<f64>,
}

fn advance(set_up: SetUp, n_theta: usize, n_r: usize, t_end: f64, t_probe: f64, t_force: f64) -> Series {
    let label = format!("{set_up:?} {n_theta}x{n_r}");
    let mut s = setup(set_up, n_theta, n_r);
    let faces = wall_faces(&s.geom, n_theta);
    let probe = {
        let c = s.geom.mesh.cell_centres();
        (0..c.len())
            .min_by(|&a, &b| {
                let d = |i: usize| (c[i][0] - 3.0).powi(2) + c[i][1].powi(2);
                d(a).total_cmp(&d(b))
            })
            .unwrap()
    };
    let mut out = Series {
        label: label.clone(),
        t: Vec::new(),
        v: Vec::new(),
        tc: Vec::new(),
        fx: Default::default(),
        fy: Vec::new(),
    };
    let t0 = std::time::Instant::now();
    let mut next_report = 20.0;
    while s.time < t_end {
        let info = s.step(DT);
        // the blow-up guard of ADR 0763: a diverged run fails here and is not repeated
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
        if s.time > t_probe {
            out.t.push(s.time);
            out.v.push(s.u.values[probe][1]);
        }
        if s.time > t_force {
            let (fx, fy) = forces(&s, &faces);
            out.tc.push(s.time);
            for (series, x) in out.fx.iter_mut().zip(fx) {
                series.push(x);
            }
            out.fy.push(fy);
        }
    }
    println!("RAN {label} to t {:.3} in {:.0} s", s.time, t0.elapsed().as_secs_f64());
    out
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

fn mean_coefficient(t: &[f64], f: &[f64], ta: f64, tb: f64) -> f64 {
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

/// What one run gives over the window of its last 8 whole periods (ADR 0763) and its two halves.
struct Shed {
    label: String,
    crossings: usize,
    st: f64,
    /// Mean of each variant over W, and over W1 and W2.
    cd: [f64; NV],
    cd_half: [[f64; NV]; 2],
    cl_amplitude: f64,
    cl_rms: f64,
    window: (f64, f64),
}

fn statistics(s: &Series) -> Shed {
    let cross = crossings(&s.t, &s.v);
    let n_use = 8;
    let k = cross.len();
    assert!(k > n_use, "{}: only {k} zero crossings, no sustained shedding", s.label);
    let (ta, tm, tb) = (cross[k - 1 - n_use], cross[k - 1 - n_use / 2], cross[k - 1]);
    let mean = |a: f64, b: f64| -> [f64; NV] { std::array::from_fn(|v| mean_coefficient(&s.tc, &s.fx[v], a, b)) };
    let cl_mean = mean_coefficient(&s.tc, &s.fy, ta, tb);
    let cl: Vec<f64> =
        s.tc.iter()
            .zip(&s.fy)
            .filter(|(&time, _)| time >= ta && time <= tb)
            .map(|(_, &y)| y / (0.5 * DEPTH))
            .collect();
    let cl_max = cl.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let cl_min = cl.iter().cloned().fold(f64::INFINITY, f64::min);
    let cl_rms = (cl.iter().map(|c| (c - cl_mean).powi(2)).sum::<f64>() / cl.len() as f64).sqrt();
    Shed {
        label: s.label.clone(),
        crossings: k,
        st: n_use as f64 / (tb - ta),
        cd: mean(ta, tb),
        cd_half: [mean(ta, tm), mean(tm, tb)],
        cl_amplitude: 0.5 * (cl_max - cl_min),
        cl_rms,
        window: (ta, tb),
    }
}

fn run(set_up: SetUp, n_theta: usize, n_r: usize) -> Shed {
    let r = statistics(&advance(set_up, n_theta, n_r, 160.0, 60.0, 85.0));
    let cds: Vec<String> = (0..NV)
        .map(|v| {
            format!(
                "{} {:.5} ({:.5}, {:.5})",
                VARIANT[v], r.cd[v], r.cd_half[0][v], r.cd_half[1][v]
            )
        })
        .collect();
    println!(
        "RESULT {}: {} crossings, St {:.5}, C_L amplitude {:.4}, rms {:.4}, window {:.1}..{:.1}; C_D on W (W1, W2): {}",
        r.label,
        r.crossings,
        r.st,
        r.cl_amplitude,
        r.cl_rms,
        r.window.0,
        r.window.1,
        cds.join("; ")
    );
    r
}

/// The ratios (r21, r32) from the cell counts, h = N^(-1/2) (ADR 0763).
fn ratios() -> (f64, f64) {
    let n = |m: (usize, usize)| (m.0 * m.1) as f64;
    (
        (n(MESHES[1]) / n(MESHES[0])).sqrt(),
        (n(MESHES[2]) / n(MESHES[1])).sqrt(),
    )
}

/// The observed order by the bisection of ADR 0763 for unequal ratios, and the Richardson value; NaN where the
/// sequence is not monotone or has no root. Copied from `f869_cylinder_mesh_converged_drag.rs`.
fn richardson(f: [f64; 3]) -> (f64, f64) {
    let (r21, r32) = ratios();
    let g = |p: f64| r32.powf(p) * (r21.powf(p) - 1.0) / (r32.powf(p) - 1.0);
    let (d21, d32) = (f[0] - f[1], f[1] - f[2]);
    if d21 * d32 <= 0.0 {
        return (f64::NAN, f64::NAN);
    }
    let target = d21 / d32;
    let (mut lo, mut hi) = (0.01, 20.0);
    if !(g(lo) < target && target < g(hi)) {
        return (f64::NAN, f64::NAN);
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if g(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let p = 0.5 * (lo + hi);
    (p, f[2] + (f[2] - f[1]) / (r32.powf(p) - 1.0))
}

/// The orders of a variant on W, W1 and W2.
fn orders(m: &[Shed; 3], v: usize) -> [f64; 3] {
    [
        richardson([m[0].cd[v], m[1].cd[v], m[2].cd[v]]).0,
        richardson([m[0].cd_half[0][v], m[1].cd_half[0][v], m[2].cd_half[0][v]]).0,
        richardson([m[0].cd_half[1][v], m[1].cd_half[1][v], m[2].cd_half[1][v]]).0,
    ]
}

/// The classes of ADR 0771, "What decides".
#[derive(Clone, Copy, Debug, PartialEq)]
enum Class {
    RaisesIntoRange,
    Raises,
    Lowers,
    DoesNotMove,
    DoesNotDecide,
}

fn class_of(p0: f64, pc: f64) -> Class {
    if !p0.is_finite() || !pc.is_finite() || p0 >= ORDER_RANGE.0 {
        return Class::DoesNotDecide;
    }
    let d = pc - p0;
    if d > AMOUNT {
        if within(pc, ORDER_RANGE) {
            Class::RaisesIntoRange
        } else {
            Class::Raises
        }
    } else if -d > AMOUNT {
        Class::Lowers
    } else {
        Class::DoesNotMove
    }
}

/// The class on W, decided only if W1 and W2 give the same class.
fn combine(classes: [Class; 3]) -> Class {
    if classes.iter().all(|c| *c == classes[0]) {
        classes[0]
    } else {
        Class::DoesNotDecide
    }
}

fn classify(p0: [f64; 3], pc: [f64; 3]) -> Class {
    combine([0, 1, 2].map(|i| class_of(p0[i], pc[i])))
}

#[derive(Debug, PartialEq)]
enum Decision {
    /// The candidates (0 = C1, 1 = C2, 2 = C3) that raise the order.
    Named(Vec<usize>),
    /// Every candidate decided, none raises the order.
    NoneRaises,
    NotDecided,
}

fn decide(classes: [Class; 3]) -> Decision {
    let named: Vec<usize> = (0..3)
        .filter(|&i| matches!(classes[i], Class::RaisesIntoRange | Class::Raises))
        .collect();
    if !named.is_empty() {
        Decision::Named(named)
    } else if classes.iter().all(|c| *c != Class::DoesNotDecide) {
        Decision::NoneRaises
    } else {
        Decision::NotDecided
    }
}

/// G1 (ADR 0771): on a manufactured field the second-order wall terms are exact and the code's carry the first-order term.
#[test]
fn g1_the_second_order_wall_terms_are_exact_for_a_quadratic_profile_and_the_standard_ones_carry_the_first_order_term() {
    let (n_theta, n_r) = MESHES[0];
    let mesh = Arc::new(cylinder_ogrid(n_theta, n_r, 0.5, R_OUT, Grading::Expansion(150.0), DEPTH).unwrap());
    let geom = FvGeometry::new(mesh).unwrap();
    let r_f = 0.5 * (std::f64::consts::PI / n_theta as f64).cos();
    let sigma = |c: Vec3| (c[0] * c[0] + c[1] * c[1]).sqrt() - r_f;
    let (a, b) = ([0.8, -0.3], [-5.0, 7.0]);
    let (c0, e) = (0.2, -1.3);
    let mut u = VectorField::from_fn("U", &geom, |c| {
        let s = sigma(c);
        [a[0] * s + b[0] * s * s, a[1] * s + b[1] * s * s, 0.0]
    });
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::from_fn("p", &geom, |c| c0 + e * sigma(c));
    let faces = wall_faces(&geom, n_theta);
    assert_eq!(faces.len(), n_theta);
    let pbc = pressure_bc(&geom, &u, &p);
    let pf = boundary_values(&geom, &pbc.coeffs, &p.values);
    let terms = wall_terms(&geom, &faces, &u, &pf, &p.values);
    let rel = |x: f64, exact: f64| (x / exact - 1.0).abs();
    let mut worst = [0.0f64; 4];
    for (w, t) in faces.iter().zip(&terms) {
        for k in 0..2 {
            worst[0] = worst[0].max(rel(t.dn_second[k], -a[k]));
            worst[1] = worst[1].max(rel(t.dn_code[k], -(a[k] + b[k] * w.s_p)));
        }
        worst[2] = worst[2].max(rel(t.p_second, c0));
        worst[3] = worst[3].max(rel(t.p_code, c0 + e * w.s_p));
    }
    let mut c = Checks::default();
    c.at_most("G1 second-order derivative = -a", worst[0], 1e-9);
    c.at_most("G1 code's derivative = -(a + b s_P)", worst[1], 1e-9);
    c.at_most("G1 extrapolated pressure = c", worst[2], 1e-9);
    c.at_most("G1 code's face pressure = c + e s_P", worst[3], 1e-9);
    c.finish();
}

/// G2 (ADR 0771): the procedure recovers a manufactured order and gives the order of ADR 0763's printed record.
#[test]
fn g2_the_richardson_procedure_recovers_a_manufactured_order_and_the_record_of_adr_0763() {
    let (r21, r32) = ratios();
    let mut c = Checks::default();
    for p in [1.4, 2.0, 2.5] {
        let f = [
            1.345 - 0.008 * (r32 * r21).powf(p),
            1.345 - 0.008 * r32.powf(p),
            1.345 - 0.008,
        ];
        let (po, value) = richardson(f);
        c.at_most(&format!("G2 order, p {p}"), (po / p - 1.0).abs(), 1e-9);
        c.at_most(&format!("G2 limit, p {p}"), (value / 1.345 - 1.0).abs(), 1e-9);
    }
    let (p, value) = richardson(RECORD_0763);
    println!("ADR 0763 record: order {p:.4}, Richardson value {value:.5}");
    c.at_most(
        "G2 order of the record of ADR 0763 against 1.385",
        (p - 1.385).abs(),
        0.005,
    );
    c.finish();
}

/// N1 (ADR 0771): the negative controls of the classification and of the decision.
#[test]
fn n1_the_classification_names_a_cause_only_beyond_the_registered_amount() {
    let p0 = 1.387;
    assert_eq!(class_of(p0, 1.45), Class::DoesNotMove);
    assert_eq!(class_of(p0, 1.60), Class::RaisesIntoRange);
    assert_eq!(class_of(p0, 1.495), Class::Raises);
    assert_eq!(class_of(p0, 1.20), Class::Lowers);
    assert_eq!(class_of(p0, 2.60), Class::Raises);
    assert_eq!(class_of(p0, f64::NAN), Class::DoesNotDecide);
    assert_eq!(class_of(1.55, 1.9), Class::DoesNotDecide);
    assert_eq!(classify([p0; 3], [1.60, 1.60, 1.45]), Class::DoesNotDecide);
    assert_eq!(classify([p0; 3], [1.60, 1.61, 1.59]), Class::RaisesIntoRange);
    assert_eq!(decide([Class::DoesNotMove; 3]), Decision::NoneRaises);
    assert_eq!(
        decide([Class::RaisesIntoRange, Class::DoesNotMove, Class::DoesNotMove]),
        Decision::Named(vec![0])
    );
    assert_eq!(
        decide([Class::DoesNotDecide, Class::DoesNotMove, Class::DoesNotMove]),
        Decision::NotDecided
    );
    assert_eq!(
        decide([Class::Lowers, Class::DoesNotMove, Class::DoesNotMove]),
        Decision::NoneRaises
    );
}

/// G4 (ADR 0771): each set-up on the coarsest mesh, 40 steps through the path of the heavy records, both integrals.
#[test]
fn g4_each_set_up_advances_through_the_path_of_the_heavy_record() {
    let (n_theta, n_r) = MESHES[0];
    let mut c = Checks::default();
    let mut b0_drag = f64::NAN;
    for set_up in [SetUp::B0, SetUp::C2Grading, SetUp::C3FarField] {
        let s = advance(set_up, n_theta, n_r, 0.9999, 0.0, 0.0);
        let drag = s.fx[STD][39];
        if set_up == SetUp::B0 {
            b0_drag = drag;
        } else {
            // G4b (amendment 1): the candidate changes the problem
            c.at_least(
                &format!("G4b {set_up:?} drag after 40 steps differs from B0's (relative)"),
                (drag / b0_drag - 1.0).abs(),
                1e-8,
            );
        }
        assert_eq!((s.t.len(), s.tc.len()), (40, 40), "{set_up:?}: 40 steps of dt 0.025");
        assert!(s.fx.iter().all(|f| f.len() == 40));
        assert!(
            s.v.iter()
                .chain(s.fx.iter().flatten())
                .chain(&s.fy)
                .all(|x| x.is_finite()),
            "{set_up:?}: a value is not finite"
        );
        let last: Vec<String> = (0..NV)
            .map(|v| format!("{} {:.5}", VARIANT[v], s.fx[v][39] / (0.5 * DEPTH)))
            .collect();
        println!("{set_up:?} after 40 steps: {}", last.join(", "));
    }
    c.finish();
}

/// The three runs of a set-up, in three threads, made once per process.
fn record(set_up: SetUp) -> &'static [Shed; 3] {
    static B0: OnceLock<[Shed; 3]> = OnceLock::new();
    static C2: OnceLock<[Shed; 3]> = OnceLock::new();
    static C3: OnceLock<[Shed; 3]> = OnceLock::new();
    let lock = match set_up {
        SetUp::B0 => &B0,
        SetUp::C2Grading => &C2,
        SetUp::C3FarField => &C3,
    };
    lock.get_or_init(|| {
        let runs: Vec<Shed> = std::thread::scope(|scope| {
            let handles: Vec<_> = MESHES
                .iter()
                .map(|&(n_theta, n_r)| scope.spawn(move || run(set_up, n_theta, n_r)))
                .collect();
            handles.into_iter().map(|h| h.join().expect("a run failed")).collect()
        });
        runs.try_into().ok().unwrap()
    })
}

/// R2: the order and Richardson value of every variant on W, W1, W2.
fn report(name: &str, m: &[Shed; 3]) {
    for (v, label) in VARIANT.iter().enumerate() {
        let w = richardson([m[0].cd[v], m[1].cd[v], m[2].cd[v]]);
        let o = orders(m, v);
        println!(
            "R2 {name} {label}: C_D {:.5}, {:.5}, {:.5}; order {:.3} (W1 {:.3}, W2 {:.3}); Richardson value {:.5}",
            m[0].cd[v], m[1].cd[v], m[2].cd[v], w.0, o[1], o[2], w.1
        );
    }
}

fn shedding(c: &mut Checks, m: &[Shed; 3]) {
    for r in m {
        c.is_true(
            &format!("K4w {}", r.label),
            r.crossings > 9,
            &format!("{} crossings", r.crossings),
        );
    }
}

/// The classes of the three candidates.
fn classes() -> [Class; 3] {
    let b0 = record(SetUp::B0);
    let p0 = orders(b0, STD);
    [
        classify(p0, orders(b0, C1)),
        classify(p0, orders(record(SetUp::C2Grading), STD)),
        classify(p0, orders(record(SetUp::C3FarField), STD)),
    ]
}

/// Heavy record of ADR 0771: B0 (R0a, R0b, K4w) and the candidate C1 on its fields.
#[test]
#[ignore = "heavy record of ADR 0771 (three solves): cargo test --release -p rcfd-api --test f881_cylinder_drag_order -- --ignored --nocapture --test-threads=1"]
fn h1_b0_reproduces_the_order_and_c1_the_wall_force_integral_heavy_record() {
    let m = record(SetUp::B0);
    report("B0", m);
    let mut c = Checks::default();
    shedding(&mut c, m);
    for (r, rec) in m.iter().zip(RECORD_0763) {
        c.at_most(&format!("R0a {} against {rec}", r.label), (r.cd[STD] - rec).abs(), 1e-5);
    }
    let p0 = orders(m, STD);
    c.is_true(
        "R0b the order of B0 under 1.5 (W, W1, W2)",
        p0.iter().all(|p| p.is_finite() && *p < ORDER_RANGE.0),
        &format!("{p0:?}"),
    );
    let pc = orders(m, C1);
    let class = classify(p0, pc);
    println!("CLASS C1 (wall force integral): {class:?}; orders B0 {p0:?}, C1 {pc:?}");
    c.is_true("C1 decided", class != Class::DoesNotDecide, &format!("{class:?}"));
    c.finish();
}

/// Heavy record of ADR 0771: the candidate C2, the grading `Expansion(50)`.
#[test]
#[ignore = "heavy record of ADR 0771 (three solves): -- --ignored --nocapture --test-threads=1"]
fn h2_c2_the_grading_of_the_o_grid_heavy_record() {
    let m = record(SetUp::C2Grading);
    report("C2", m);
    let mut c = Checks::default();
    shedding(&mut c, m);
    let (p0, pc) = (orders(record(SetUp::B0), STD), orders(m, STD));
    let class = classify(p0, pc);
    println!("CLASS C2 (grading): {class:?}; orders B0 {p0:?}, C2 {pc:?}");
    c.is_true("C2 decided", class != Class::DoesNotDecide, &format!("{class:?}"));
    c.finish();
}

/// Heavy record of ADR 0771: the candidate C3, the pressure fixed on the whole far field.
#[test]
#[ignore = "heavy record of ADR 0771 (three solves): -- --ignored --nocapture --test-threads=1"]
fn h3_c3_the_far_field_condition_heavy_record() {
    let m = record(SetUp::C3FarField);
    report("C3", m);
    let mut c = Checks::default();
    shedding(&mut c, m);
    let (p0, pc) = (orders(record(SetUp::B0), STD), orders(m, STD));
    let class = classify(p0, pc);
    println!("CLASS C3 (far field): {class:?}; orders B0 {p0:?}, C3 {pc:?}");
    c.is_true("C3 decided", class != Class::DoesNotDecide, &format!("{class:?}"));
    c.finish();
}

/// Heavy record of ADR 0771: the decision D over the three candidates.
#[test]
#[ignore = "heavy record of ADR 0771, shares the runs of h1 to h3: -- --ignored --nocapture --test-threads=1"]
fn h4_the_decision_over_the_three_candidates_heavy_record() {
    let cl = classes();
    let d = decide(cl);
    println!(
        "DECISION D: classes C1 {:?}, C2 {:?}, C3 {:?}: {d:?}",
        cl[0], cl[1], cl[2]
    );
    let mut c = Checks::default();
    c.is_true("D decided", d != Decision::NotDecided, &format!("{d:?}"));
    c.finish();
}
