//! F-346 (ADR 0249): the force and moment of the fluid on an immersed solid in unsteady flow.
//!
//! The registered checks of ADR 0249: the closed forms of the uniformly accelerated annulus (X1) and plate (X2) with the
//! reaction alone and the literal reading as negative controls, the steady cases (A3a, A3b), DFG 2D-2 on M2 and T (A2, A2T, C1,
//! D1p) and the unit checks U1 to U5. The set-up helpers are copies of those of `immersed_solid_f219.rs`,
//! `immersed_second_order_f309.rs` and `immersed_force_order_f337.rs` (those files are not edited). The heavy tests run in
//! release only.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{velocity_rate, ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, Symmetry, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d, box_2d_coords, expansion_for_end_cell, Grading};
use std::sync::Arc;
use std::time::Instant;

/// Which forcing a run uses.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `ImmersedForcing::build` with `modified = true` (ADR 0218).
    First,
    /// `ImmersedForcing::build_second_order` (ADR 0234).
    Second,
}

fn build(geom: &FvGeometry, bodies: &[ImmersedBody], k: f64, nu: f64, forcing: Forcing) -> ImmersedForcing {
    let gamma = uniform_gamma(geom, nu);
    let st = ImmersedSettings::new(k);
    match forcing {
        Forcing::First => ImmersedForcing::build(geom, bodies, st, &gamma).unwrap(),
        Forcing::Second => ImmersedForcing::build_second_order(geom, bodies, st, &gamma).unwrap(),
    }
}

/// The cells of the modification terms of a body (its first fluid cells), each once.
fn first_fluid_cells(ib: &ImmersedForcing, body: usize) -> Vec<usize> {
    let mut cells: Vec<usize> = ib.bodies[body]
        .terms
        .iter()
        .filter(|t| t.face.is_some())
        .map(|t| t.cell)
        .collect();
    cells.sort_unstable();
    cells.dedup();
    cells
}

/// `rho sum V dudt` and `rho sum (x_c - x_ref) x V dudt` over `cells`, from the mesh (not a library call).
fn storage(geom: &FvGeometry, cells: &[usize], dudt: &[Vec3], rho: f64, x_ref: Vec3) -> (Vec3, Vec3) {
    let (cc, vol) = (geom.mesh.cell_centres(), geom.mesh.cell_volumes());
    let (mut f, mut m) = (vec3::ZERO, vec3::ZERO);
    for &c in cells {
        let dp = vec3::scale(dudt[c], rho * vol[c]);
        f = vec3::add(f, dp);
        m = vec3::add(m, vec3::cross(vec3::sub(cc[c], x_ref), dp));
    }
    (f, m)
}

/// The three evaluations of ADR 0249 for one body: new (`force_moment_unsteady`), the reaction (`force_moment_second_order`)
/// and the literal reading (new plus the storage of the first fluid cells, formed here).
struct Evals {
    new: (Vec3, Vec3),
    reaction: (Vec3, Vec3),
    literal: (Vec3, Vec3),
}

fn evaluate(geom: &FvGeometry, ib: &ImmersedForcing, body: usize, u: &[Vec3], dudt: &[Vec3], x_ref: Vec3) -> Evals {
    let new = ib.force_moment_unsteady(body, u, dudt, 1.0, x_ref);
    let reaction = ib.force_moment_second_order(body, u, 1.0, x_ref);
    let (lf, lm) = storage(geom, &first_fluid_cells(ib, body), dudt, 1.0, x_ref);
    Evals {
        new,
        reaction,
        literal: (vec3::add(new.0, lf), vec3::add(new.1, lm)),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Circular Couette geometry (the set-up CC of ADR 0218)
// ---------------------------------------------------------------------------------------------------------------------------

const CC_C: Vec3 = [1.2, 1.2, 0.0];
const R1: f64 = 0.5;
const R2: f64 = 1.0;

/// The inner cylinder rotating with `om_in`, the outer (inverted) one with `om_out`, about the axis.
fn cc_bodies(om_in: f64, om_out: f64) -> Vec<ImmersedBody> {
    let mut inner = ImmersedBody::fixed(
        "inner",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: R1,
        },
    );
    inner.omega = [0.0, 0.0, om_in];
    inner.origin = CC_C;
    let mut outer = ImmersedBody::fixed(
        "outer",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: R2,
        },
    );
    outer.inverted = true;
    outer.omega = [0.0, 0.0, om_out];
    outer.origin = CC_C;
    vec![inner, outer]
}

fn cc_mesh(n: usize) -> (Arc<FvGeometry>, VectorField, ScalarField) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(n, n, 2.4, 2.4, false, false).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    (geom, u, p)
}

// ---------------------------------------------------------------------------------------------------------------------------
// X1: the uniformly accelerated annulus
// ---------------------------------------------------------------------------------------------------------------------------

struct Annulus {
    /// z moment per unit depth at step 200: new, reaction, literal.
    m: [f64; 3],
    /// new at step 150.
    m150: f64,
    /// Norm of the new force per unit depth at step 200.
    force: f64,
}

fn annulus(n: usize, forcing: Forcing, outer_rotates: bool) -> Annulus {
    let (alpha, dt, steps) = (1.0, 0.0025, 200usize);
    let (geom, u, p) = cc_mesh(n);
    let dz = 2.4 / n as f64;
    let mut st = PimpleSettings::piso(1.0);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    let mut u_old = s.u.values.clone();
    let mut older: Option<Vec<Vec3>> = None;
    let mut out = Annulus {
        m: [f64::NAN; 3],
        m150: f64::NAN,
        force: f64::NAN,
    };
    let t0 = Instant::now();
    for k in 1..=steps {
        let om = alpha * k as f64 * dt;
        let ib = build(
            &geom,
            &cc_bodies(om, if outer_rotates { om } else { 0.0 }),
            1e8,
            1.0,
            forcing,
        );
        s.immersed = Some(ib.clone());
        s.step(dt);
        assert!(s.diverged.is_none(), "X1 N {n}: stopped at step {k}: {:?}", s.diverged);
        let rate = velocity_rate(&s.u.values, &u_old, older.as_ref().map(|o| (o.as_slice(), dt)), dt);
        if k % 25 == 0 {
            let e = evaluate(&geom, &ib, 0, &s.u.values, &rate, CC_C);
            println!(
                "X1 N {n} {forcing:?} step {k} t {:.4}: M_z/dz new {:.6} reaction {:.6} literal {:.6}; |F|/dz {:.3e}",
                k as f64 * dt,
                e.new.1[2] / dz,
                e.reaction.1[2] / dz,
                e.literal.1[2] / dz,
                vec3::norm(e.new.0) / dz
            );
            if k == 150 {
                out.m150 = e.new.1[2] / dz;
            }
            if k == steps {
                out.m = [e.new.1[2] / dz, e.reaction.1[2] / dz, e.literal.1[2] / dz];
                out.force = vec3::norm(e.new.0) / dz;
            }
        }
        older = Some(std::mem::replace(&mut u_old, s.u.values.clone()));
    }
    if !outer_rotates {
        // printed: the velocity error in the annulus against alpha t f(r) + w(r) of amendment 1
        let (t, cc) = (steps as f64 * dt, geom.mesh.cell_centres());
        let (ca, cb, c, d) = inner_cylinder_constants();
        let mut e = 0.0f64;
        for (x, v) in cc.iter().zip(&s.u.values) {
            let (dx, dy) = (x[0] - CC_C[0], x[1] - CC_C[1]);
            let r = (dx * dx + dy * dy).sqrt();
            if r > R1 && r < R2 {
                let ut = alpha * t * (ca * r + cb / r)
                    + alpha * (ca * r * r * r / 8.0 + 0.5 * cb * r * r.ln())
                    + c * r
                    + d / r;
                e = e.max(vec3::norm(vec3::sub(*v, [-ut * dy / r, ut * dx / r, 0.0])));
            }
        }
        println!("X1b N {n} {forcing:?}: largest velocity error in the annulus {e:.3e}");
    }
    println!("X1 N {n} {forcing:?}: {:.1} s", t0.elapsed().as_secs_f64());
    out
}

/// Amendment 1, X1b: `A`, `B` of the Couette profile of unit angular velocity and `C`, `D` of `w` (`alpha = nu = 1`).
fn inner_cylinder_constants() -> (f64, f64, f64, f64) {
    let a = -R1 * R1 / (R2 * R2 - R1 * R1);
    let b = R1 * R1 * R2 * R2 / (R2 * R2 - R1 * R1);
    let wp = |r: f64| a * r * r * r / 8.0 + 0.5 * b * r * r.ln();
    let det = R1 / R2 - R2 / R1;
    let c = (-wp(R1) / R2 + wp(R2) / R1) / det;
    let d = (-R1 * wp(R2) + R2 * wp(R1)) / det;
    (a, b, c, d)
}

/// Amendment 1, X1b: the moment per unit depth of the fluid on the inner cylinder accelerated with `alpha = 1` in the fixed
/// outer one, after the transients: `-4 pi B t + 2 pi R1^2 ((A R1^2/4 + B/2) - 2 D/R1^2)`.
fn inner_cylinder_moment(t: f64) -> f64 {
    let (a, b, _, d) = inner_cylinder_constants();
    let pi = std::f64::consts::PI;
    -4.0 * pi * b * t + 2.0 * pi * R1 * R1 * ((a * R1 * R1 / 4.0 + b / 2.0) - 2.0 * d / (R1 * R1))
}

/// ADR 0249 amendment 1: X1b, X1cb, X1lb, S-X1b (the corrected X1: the inner cylinder accelerated in a fixed outer one).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn x1b_accelerated_inner_cylinder_moment() {
    let (exact, exact150) = (inner_cylinder_moment(0.5), inner_cylinder_moment(0.375));
    println!("X1b: closed form {exact:.6} at t 0.5, {exact150:.6} at t 0.375");
    assert!((exact + 2.218_329).abs() <= 1e-6 && (exact150 + 1.694_730).abs() <= 1e-6);
    let mut fails = Vec::new();
    for (n, lit_limit) in [(48usize, 0.01), (96, 0.005)] {
        let a = annulus(n, Forcing::Second, false);
        let err = |m: f64| (m - exact).abs() / exact.abs();
        let (e_new, e_rea, e_lit) = (err(a.m[0]), err(a.m[1]), err(a.m[2]));
        let e150 = (a.m150 - exact150).abs() / exact150.abs();
        println!(
            "X1b N {n}: exact {exact:.6}; new {:.6} (error {:.4} %), reaction {:.6} ({:.3} %), literal {:.6} ({:.3} %); S-X1b step \
             150 new {:.6} (error {:.4} %); force {:.3e}",
            a.m[0],
            100.0 * e_new,
            a.m[1],
            100.0 * e_rea,
            a.m[2],
            100.0 * e_lit,
            a.m150,
            100.0 * e150,
            a.force
        );
        if e150.is_nan() || e150 > 0.005 {
            fails.push(format!("S-X1b N {n}: error at step 150 {e150:.5}"));
        }
        if e_new.is_nan() || e_new > 0.005 {
            fails.push(format!("X1b N {n}: error {e_new:.5}"));
        }
        if e_rea.is_nan() || e_rea < 0.03 {
            fails.push(format!("X1cb N {n} (negative control) did not fire: {e_rea:.5}"));
        }
        if e_lit.is_nan() || e_lit < lit_limit {
            fails.push(format!("X1lb N {n} (negative control) did not fire: {e_lit:.5}"));
        }
    }
    let b = annulus(48, Forcing::First, false);
    println!(
        "printed: X1b N 48 with build: new {:.6} ({:+.4} %), reaction {:.6}, literal {:.6}",
        b.m[0],
        100.0 * (b.m[0] - exact) / exact.abs(),
        b.m[1],
        b.m[2]
    );
    assert!(fails.is_empty(), "{fails:#?}");
}

/// ADR 0249 X1, X1c, X1l, S-X1 as registered: a defective set-up (the rotating outer solid reaches the box walls at rest;
/// amendment 1). Kept as a record.
#[test]
#[ignore = "defective set-up (ADR 0249 amendment 1), kept as a record; replaced by the test of the same name with b"]
fn x1_accelerated_annulus_moment() {
    let exact = -0.5 * std::f64::consts::PI * R1 * R1 * (R2 * R2 - R1 * R1);
    let mut fails = Vec::new();
    for (n, lit_limit) in [(48usize, 0.08), (96, 0.04)] {
        let a = annulus(n, Forcing::Second, true);
        let err = |m: f64| (m - exact).abs() / exact.abs();
        let (e_new, e_rea, e_lit) = (err(a.m[0]), err(a.m[1]), err(a.m[2]));
        let drift = (a.m[0] - a.m150).abs() / a.m[0].abs();
        println!(
            "X1 N {n}: exact {exact:.6}; new {:.6} (error {:.4} %), reaction {:.6} ({:.3} %), literal {:.6} ({:.3} %); S-X1 \
             drift 150 to 200 {drift:.3e}; force {:.3e}",
            a.m[0],
            100.0 * e_new,
            a.m[1],
            100.0 * e_rea,
            a.m[2],
            100.0 * e_lit,
            a.force
        );
        if drift.is_nan() || drift > 1e-3 {
            fails.push(format!("S-X1 N {n}: drift {drift:.3e}"));
        }
        if e_new.is_nan() || e_new > 0.015 {
            fails.push(format!("X1 N {n}: error {e_new:.5}"));
        }
        if e_rea.is_nan() || e_rea < 0.25 {
            fails.push(format!("X1c N {n} (negative control) did not fire: {e_rea:.5}"));
        }
        if e_lit.is_nan() || e_lit < lit_limit {
            fails.push(format!("X1l N {n} (negative control) did not fire: {e_lit:.5}"));
        }
    }
    let b = annulus(48, Forcing::First, true);
    println!(
        "printed: X1 N 48 with build: new {:.6} ({:+.4} %), reaction {:.6}, literal {:.6}",
        b.m[0],
        100.0 * (b.m[0] - exact) / exact.abs(),
        b.m[1],
        b.m[2]
    );
    assert!(fails.is_empty(), "{fails:#?}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// X2: the uniformly accelerated plate
// ---------------------------------------------------------------------------------------------------------------------------

struct Plate {
    /// `F_x/(L dz)` at `t = 0.5`: new, reaction, literal.
    fx: [f64; 3],
    fy: f64,
    /// S-X2: the largest x variation of `u_x` and the largest `u_y`.
    dux: f64,
    uy: f64,
}

fn plate(n: usize, forcing: Forcing, corrected: bool) -> Plate {
    let (a, dt, steps, nx) = (1.0, 0.0025, 200usize, 8usize);
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(nx, n, 1.0, 1.0, false, false).unwrap())).unwrap());
    let dz = geom.mesh.cell_volumes()[0] * (nx * n) as f64;
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    if corrected {
        u.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    } else {
        u.set_bc(&geom, "ymin", Box::new(Symmetry)).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    for w in ["xmin", "xmax"] {
        u.set_bc(&geom, w, Box::new(ZeroGradient)).unwrap();
        p.set_bc(&geom, w, Box::new(FixedValue::scalar(0.0))).unwrap();
    }
    let body = |v: f64| {
        let mut b = ImmersedBody::fixed(
            "plate",
            SolidShape::Cuboid {
                lo: [-10.0, -10.0, -10.0],
                hi: [10.0, 0.5, 10.0],
            },
        );
        b.velocity = [v, 0.0, 0.0];
        b
    };
    let mut st = PimpleSettings::piso(1.0);
    st.time_scheme = TimeScheme::ImplicitEuler;
    st.convection = ConvScheme::Central;
    if corrected {
        st.momentum_solver.tol_abs = 1e-20;
        st.momentum_solver.tol_rel = 1e-14;
        st.pressure_solver.tol_abs = 1e-20;
        st.pressure_solver.tol_rel = 1e-14;
    }
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    let mut u_old = s.u.values.clone();
    let mut out = Plate {
        fx: [f64::NAN; 3],
        fy: f64::NAN,
        dux: f64::NAN,
        uy: f64::NAN,
    };
    for k in 1..=steps {
        let ib = build(&geom, &[body(a * k as f64 * dt)], 1e8, 1.0, forcing);
        s.immersed = Some(ib.clone());
        s.step(dt);
        assert!(s.diverged.is_none(), "X2 N {n}: stopped at step {k}: {:?}", s.diverged);
        let rate = velocity_rate(&s.u.values, &u_old, None, dt);
        if k % 25 == 0 {
            let e = evaluate(&geom, &ib, 0, &s.u.values, &rate, [0.0; 3]);
            println!(
                "X2 N {n} {forcing:?} step {k}: F_x/(L dz) new {:.6} reaction {:.6} literal {:.6}; exact {:.6}",
                e.new.0[0] / dz,
                e.reaction.0[0] / dz,
                e.literal.0[0] / dz,
                -(a * k as f64 * dt / 0.5 + a * 0.5 / 3.0)
            );
            if k == steps {
                out.fx = [e.new.0[0] / dz, e.reaction.0[0] / dz, e.literal.0[0] / dz];
                out.fy = e.new.0[1] / dz;
            }
        }
        u_old.clone_from(&s.u.values);
    }
    let v = &s.u.values;
    out.dux = (0..n)
        .flat_map(|j| (0..nx).map(move |i| (i, j)))
        .map(|(i, j)| (v[i + nx * j][0] - v[nx * j][0]).abs())
        .fold(0.0, f64::max);
    out.uy = v.iter().map(|w| w[1].abs()).fold(0.0, f64::max);
    out
}

/// ADR 0249 X2, X2c, X2l, S-X2 as registered: a defective set-up (the symmetry plane under the plate acts as a wall in the
/// PISO kernel; the solver tolerances break the x uniformity; amendment 1). Kept as a record.
#[test]
#[ignore = "defective set-up (ADR 0249 amendment 1), kept as a record; replaced by the test of the same name with b"]
fn x2_accelerated_plate_force() {
    x2_checks(false);
}

/// ADR 0249 amendment 1: X2b, X2cb, X2lb, S-X2b (the corrected X2: zero gradient under the plate, tight solver tolerances).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn x2b_accelerated_plate_force() {
    x2_checks(true);
}

fn x2_checks(corrected: bool) {
    let exact = -(1.0 + 0.5 / 3.0);
    let mut fails = Vec::new();
    for (n, lit_limit) in [(16usize, 0.025), (32, 0.012)] {
        let r = plate(n, Forcing::Second, corrected);
        let err = |f: f64| (f - exact).abs() / exact.abs();
        let (e_new, e_rea, e_lit) = (err(r.fx[0]), err(r.fx[1]), err(r.fx[2]));
        println!(
            "X2 N {n}: exact {exact:.6}; new {:.6} (error {:.4} %), reaction {:.6} ({:.3} %), literal {:.6} ({:.3} %); S-X2 \
             x variation {:.3e}, max |u_y| {:.3e}; F_y/(L dz) {:.3e}",
            r.fx[0],
            100.0 * e_new,
            r.fx[1],
            100.0 * e_rea,
            r.fx[2],
            100.0 * e_lit,
            r.dux,
            r.uy,
            r.fy
        );
        if r.dux.is_nan() || r.dux > 1e-9 || r.uy.is_nan() || r.uy > 1e-9 {
            fails.push(format!("S-X2 N {n}: x variation {:.3e}, u_y {:.3e}", r.dux, r.uy));
        }
        if e_new.is_nan() || e_new > 0.01 {
            fails.push(format!("X2 N {n}: error {e_new:.5}"));
        }
        if e_rea.is_nan() || e_rea < 0.30 {
            fails.push(format!("X2c N {n} (negative control) did not fire: {e_rea:.5}"));
        }
        if e_lit.is_nan() || e_lit < lit_limit {
            fails.push(format!("X2l N {n} (negative control) did not fire: {e_lit:.5}"));
        }
    }
    let b = plate(16, Forcing::First, corrected);
    println!(
        "printed: X2 N 16 with build: new {:.6}, reaction {:.6}, literal {:.6}",
        b.fx[0], b.fx[1], b.fx[2]
    );
    assert!(fails.is_empty(), "{fails:#?}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// DFG set-up (ADR 0218)
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;
/// FeatFlow level 6, `k = 1/200` (docs/reference-data.md, the row "DFG 2D-2 set-up" of the F-219 section).
const CD_MAX_REF: f64 = 3.2200;
const CD_MIN_REF: f64 = 3.1569;
const CD_MEAN_REF: f64 = 3.1884;
const CL_MAX_REF: f64 = 0.9859;
const CL_MIN_REF: f64 = -1.0206;
const ST_REF: f64 = 0.301_88;

fn geo_end(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(r)).unwrap()
}

fn geo_start(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(1.0 / r)).unwrap()
}

fn uniform(a: f64, b: f64, n: usize, centre: f64) -> Vec<f64> {
    let h = (b - a) / n as f64;
    let i0 = ((centre - a) / h).round() as i64;
    (0..=n).map(|i| centre + (i as i64 - i0) as f64 * h).collect()
}

fn join(parts: &[Vec<f64>], offsets: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for (p, &o) in parts.iter().zip(offsets) {
        let start = usize::from(!out.is_empty());
        out.extend(p[start..].iter().map(|x| x + o));
    }
    out
}

fn bisect(xs: &[f64]) -> Vec<f64> {
    let mut out = vec![xs[0]];
    for w in xs.windows(2) {
        out.push(0.5 * (w[0] + w[1]));
        out.push(w[1]);
    }
    out
}

/// ADR 0218 set-up: M1 coordinates, bisected `level - 1` times.
fn dfg_family(level: usize) -> (Vec<f64>, Vec<f64>) {
    let h = 0.005;
    let mut xs = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.32, 40, 0.2),
            geo_start(38, 1.88, h),
        ],
        &[0.0, 0.0, 0.32],
    );
    let mut ys = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.28, 32, 0.2),
            geo_start(10, 0.13, h),
        ],
        &[0.0, 0.0, 0.28],
    );
    for _ in 1..level {
        xs = bisect(&xs);
        ys = bisect(&ys);
    }
    (xs, ys)
}

/// ADR 0218 T mesh of 2D-2.
fn t_mesh() -> (Vec<f64>, Vec<f64>) {
    let xs = join(&[uniform(0.0, 0.6, 240, 0.2), geo_start(84, 1.6, 0.0025)], &[0.0, 0.6]);
    let ys = uniform(0.0, H, 164, 0.2);
    (xs, ys)
}

fn inflow_mean(um: f64, y1: f64, y2: f64) -> f64 {
    let prim = |y: f64| H * y * y / 2.0 - y * y * y / 3.0;
    4.0 * um / (H * H) * (prim(y2) - prim(y1)) / (y2 - y1)
}

fn inflow_point(um: f64, y: f64) -> f64 {
    4.0 * um * y * (H - y) / (H * H)
}

struct Dfg {
    geom: Arc<FvGeometry>,
    u: VectorField,
    p: ScalarField,
    ib: ImmersedForcing,
}

/// The DFG channel on the given coordinates with the cylinder as an immersed solid (ADR 0218).
fn dfg(xs: &[f64], ys: &[f64], um: f64, forcing: Forcing) -> Dfg {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d_coords(xs, ys, DZ).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [inflow_point(um, c[1]), 0.0, 0.0]);
    let cf = geom.mesh.face_centres();
    let prof: Vec<Vec3> = geom
        .mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| {
            let y = cf[f][1];
            let j = ys.partition_point(|&v| v < y) - 1;
            [inflow_mean(um, ys[j], ys[j + 1]), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "xmin", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let body = ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: CENTRE,
            axis: [0.0, 0.0, 1.0],
            radius: 0.5 * DIAM,
        },
    );
    let ib = build(&geom, &[body], K_DFG, NU, forcing);
    Dfg { geom, u, p, ib }
}

// ---------------------------------------------------------------------------------------------------------------------------
// 2D-2
// ---------------------------------------------------------------------------------------------------------------------------

/// Upward crossings of `v - mean` (linear interpolation).
fn crossings(t: &[f64], v: &[f64], mean: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for i in 1..v.len() {
        if v[i - 1] - mean < 0.0 && v[i] - mean >= 0.0 {
            let w = (mean - v[i - 1]) / (v[i] - v[i - 1]);
            out.push(t[i - 1] + w * (t[i] - t[i - 1]));
        }
    }
    out
}

/// The window statistics of ADR 0218 (its A3 and S2) of one pair of coefficient series.
#[derive(Debug, Clone, Copy)]
struct Window {
    crossings: usize,
    st: f64,
    period_spread: f64,
    max_spread: f64,
    amplitude: f64,
    cd_max: f64,
    cd_min: f64,
    cd_mean: f64,
    cl_max: f64,
    cl_min: f64,
}

fn window(ts: &[f64], cds: &[f64], cls: &[f64]) -> Window {
    let w0 = ts.iter().position(|&t| t >= 5.5 - 1e-12).unwrap();
    let (tw, dw, lw) = (&ts[w0..], &cds[w0..], &cls[w0..]);
    let mean = lw.iter().sum::<f64>() / lw.len() as f64;
    let cr = crossings(tw, lw, mean);
    let n = cr.len();
    let mut out = Window {
        crossings: n,
        st: f64::NAN,
        period_spread: f64::NAN,
        max_spread: f64::NAN,
        amplitude: lw.iter().copied().fold(f64::MIN, f64::max) - lw.iter().copied().fold(f64::MAX, f64::min),
        cd_max: f64::MIN,
        cd_min: f64::MAX,
        cd_mean: dw.iter().sum::<f64>() / dw.len() as f64,
        cl_max: f64::MIN,
        cl_min: f64::MAX,
    };
    if n < 2 {
        return out;
    }
    let f = (n - 1) as f64 / (cr[n - 1] - cr[0]);
    out.st = f * DIAM / 1.0;
    let periods: Vec<f64> = cr.windows(2).map(|w| w[1] - w[0]).collect();
    let pm = periods.iter().sum::<f64>() / periods.len() as f64;
    out.period_spread = periods.iter().map(|p| (p - pm).abs() / pm).fold(0.0, f64::max);
    let mut maxima = Vec::new();
    for w in cr.windows(2) {
        let idx: Vec<usize> = (0..tw.len()).filter(|&i| tw[i] >= w[0] && tw[i] < w[1]).collect();
        maxima.push(idx.iter().map(|&i| lw[i]).fold(f64::MIN, f64::max));
        for &i in &idx {
            out.cd_max = out.cd_max.max(dw[i]);
            out.cd_min = out.cd_min.min(dw[i]);
            out.cl_max = out.cl_max.max(lw[i]);
            out.cl_min = out.cl_min.min(lw[i]);
        }
    }
    let mm = maxima.iter().sum::<f64>() / maxima.len() as f64;
    out.max_spread = maxima.iter().map(|m| (m - mm).abs() / mm.abs()).fold(0.0, f64::max);
    out
}

fn pct(a: f64, r: f64) -> f64 {
    100.0 * (a - r) / r.abs()
}

fn print_window(name: &str, w: &Window) {
    println!(
        "2D-2 {name}: {} crossings, St {:.5} ({:+.3} %), period spread {:.3e}, C_L max spread {:.3e}, amplitude {:.4}; C_D max \
         {:.5} ({:+.3} %) min {:.5} ({:+.3} %) mean {:.5} ({:+.3} %); C_L max {:.5} ({:+.3} %) min {:.5} ({:+.3} %)",
        w.crossings,
        w.st,
        pct(w.st, ST_REF),
        w.period_spread,
        w.max_spread,
        w.amplitude,
        w.cd_max,
        pct(w.cd_max, CD_MAX_REF),
        w.cd_min,
        pct(w.cd_min, CD_MIN_REF),
        w.cd_mean,
        pct(w.cd_mean, CD_MEAN_REF),
        w.cl_max,
        pct(w.cl_max, CL_MAX_REF),
        w.cl_min,
        -pct(w.cl_min, CL_MIN_REF)
    );
}

struct TwoD2 {
    /// Old (`force_moment`), new (`force_moment_unsteady`), literal with the storage of the first fluid cells added, and with
    /// it subtracted.
    windows: [Window; 4],
    /// D1p: the largest `abs(C_new - C_old)` over the window, drag and lift.
    diff: (f64, f64),
}

/// The 2D-2 run of ADR 0218 on the given coordinates with the evaluations of ADR 0249 after every step. Returns `None` when
/// the run stops (the divergence guard).
fn run_2d2(name: &str, xs: &[f64], ys: &[f64], forcing: Forcing) -> Option<TwoD2> {
    let (um, ubar) = (1.5, 1.0);
    let d = dfg(xs, ys, um, forcing);
    let n_cells = d.geom.n_cells();
    let mut u = d.u;
    let cc = d.geom.mesh.cell_centres().to_vec();
    for (c, x) in cc.iter().enumerate() {
        let blob = 0.2 * ubar * (-((x[0] - 0.35).powi(2) + (x[1] - 0.2).powi(2)) / (0.05 * 0.05)).exp();
        u.values[c] = [inflow_point(um, x[1]), blob, 0.0];
    }
    let mut st = PimpleSettings::piso(NU);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.n_correctors = 2;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let mut s = PimpleSolver::new(d.geom.clone(), u, d.p, st);
    s.immersed = Some(d.ib.clone());
    let (dt, steps) = (0.001, 8000usize);
    let q = 0.5 * ubar * ubar * DIAM * DZ;
    let layer = first_fluid_cells(&d.ib, 0);
    println!(
        "2D-2 {name} {forcing:?}: {n_cells} cells, {} solid cells, {} first fluid cells",
        d.ib.bodies[0].solid_cells.len(),
        layer.len()
    );
    let t0 = Instant::now();
    let mut ts = Vec::new();
    let mut series: [(Vec<f64>, Vec<f64>); 4] = Default::default();
    let mut mz = Vec::new();
    let mut u_old = s.u.values.clone();
    let mut older: Option<Vec<Vec3>> = None;
    for n in 1..=steps {
        let info = s.step(dt);
        if s.diverged.is_some() {
            println!("2D-2 {name} {forcing:?}: stopped at step {n}: {:?}", s.diverged);
            return None;
        }
        let rate = velocity_rate(&s.u.values, &u_old, older.as_ref().map(|o| (o.as_slice(), dt)), dt);
        let (f_old, _) = d.ib.force_moment(0, &s.u.values, 1.0, CENTRE);
        let (f_new, m_new) = d.ib.force_moment_unsteady(0, &s.u.values, &rate, 1.0, CENTRE);
        let (lf, _) = storage(&d.geom, &layer, &rate, 1.0, CENTRE);
        let all = [f_old, f_new, vec3::add(f_new, lf), vec3::sub(f_new, lf)];
        for (sr, f) in series.iter_mut().zip(&all) {
            sr.0.push(f[0] / q);
            sr.1.push(f[1] / q);
        }
        mz.push(m_new[2] / (q * DIAM));
        let t = n as f64 * dt;
        ts.push(t);
        if n % 500 == 0 {
            println!(
                "{name} t {t:.3}: old C_D {:.5} C_L {:+.5}; new {:.5} {:+.5}; literal {:.5} {:+.5}; Co {:.3} ({:.0} s)",
                all[0][0] / q,
                all[0][1] / q,
                all[1][0] / q,
                all[1][1] / q,
                all[2][0] / q,
                all[2][1] / q,
                info.courant,
                t0.elapsed().as_secs_f64()
            );
        }
        older = Some(std::mem::replace(&mut u_old, s.u.values.clone()));
    }
    let windows = [0, 1, 2, 3].map(|k| window(&ts, &series[k].0, &series[k].1));
    for (k, label) in [
        "old",
        "new",
        "literal (+ first fluid cells)",
        "literal (- first fluid cells)",
    ]
    .iter()
    .enumerate()
    {
        print_window(&format!("{name} {forcing:?} {label}"), &windows[k]);
    }
    let w0 = ts.iter().position(|&t| t >= 5.5 - 1e-12).unwrap();
    let dmax = |k: usize| {
        let (a, b) = if k == 0 {
            (&series[0].0, &series[1].0)
        } else {
            (&series[0].1, &series[1].1)
        };
        a[w0..]
            .iter()
            .zip(&b[w0..])
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
    };
    let diff = (dmax(0), dmax(1));
    let (mz_max, mz_min) = (
        mz[w0..].iter().copied().fold(f64::MIN, f64::max),
        mz[w0..].iter().copied().fold(f64::MAX, f64::min),
    );
    let vs = d.ib.bodies[0]
        .solid_cells
        .iter()
        .map(|&c| vec3::norm(s.u.values[c]))
        .fold(0.0, f64::max);
    println!(
        "2D-2 {name} {forcing:?}: D1p max |C_D new - old| {:.3e}, max |C_L new - old| {:.3e}; M_z/(q D) max {mz_max:.5e} min \
         {mz_min:.5e}; max |u| solid {vs:.3e}; {:.0} s",
        diff.0,
        diff.1,
        t0.elapsed().as_secs_f64()
    );
    Some(TwoD2 { windows, diff })
}

/// S2 of ADR 0218 on the old evaluation; D1p; the acceptance limits on the new evaluation. Returns the failures.
fn judge_2d2(name: &str, r: &TwoD2) -> Vec<String> {
    let mut fails = Vec::new();
    let o = &r.windows[0];
    if o.crossings < 6 {
        fails.push(format!("S2 {name}: {} upward crossings", o.crossings));
    }
    if o.period_spread.is_nan() || o.period_spread > 0.01 {
        fails.push(format!("S2 {name}: period spread {:.3e}", o.period_spread));
    }
    if o.max_spread.is_nan() || o.max_spread > 0.02 {
        fails.push(format!("S2 {name}: C_L maximum spread {:.3e}", o.max_spread));
    }
    if o.amplitude.is_nan() || o.amplitude < 1.0 {
        fails.push(format!("S2 {name}: C_L amplitude {:.4}", o.amplitude));
    }
    if r.diff.0.is_nan() || r.diff.0 > 1e-3 || r.diff.1.is_nan() || r.diff.1 > 1e-3 {
        fails.push(format!("D1p {name}: {:.3e}, {:.3e}", r.diff.0, r.diff.1));
    }
    let w = &r.windows[1];
    let (ed, el) = (
        (w.cd_max - CD_MAX_REF).abs() / CD_MAX_REF,
        (w.cl_max - CL_MAX_REF).abs() / CL_MAX_REF,
    );
    println!(
        "acceptance {name}: new max C_D {:.5} error {:.4} %, max C_L {:.5} error {:.4} % (limit 1.5 %)",
        w.cd_max,
        100.0 * ed,
        w.cl_max,
        100.0 * el
    );
    if ed.is_nan() || ed > 0.015 {
        fails.push(format!("acceptance {name}: max C_D {} error {ed:.5}", w.cd_max));
    }
    if el.is_nan() || el > 0.015 {
        fails.push(format!("acceptance {name}: max C_L {} error {el:.5}", w.cl_max));
    }
    fails
}

/// ADR 0249 A2T, S2, C1, C1r, D1p on the T mesh of ADR 0218.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a2t_c1_two_d_two_on_t() {
    let (xs, ys) = t_mesh();
    let r = run_2d2("T", &xs, &ys, Forcing::First).expect("S2: the T run stopped");
    let mut fails = judge_2d2("A2T", &r);
    let o = &r.windows[0];
    // C1: the present evaluation misses the 1.5 percent on the lift
    let e_old = (o.cl_max - CL_MAX_REF).abs() / CL_MAX_REF;
    if e_old.is_nan() || e_old <= 0.015 {
        fails.push(format!("C1 (control) did not fire: old max C_L error {e_old:.5}"));
    }
    // C1r: the run is that of ADR 0218 (3.2462, 1.0113, 0.30073 to the digits given)
    let same = |a: f64, b: f64, half: f64| (a - b).abs() <= half;
    if !(same(o.cd_max, 3.2462, 5e-5) && same(o.cl_max, 1.0113, 5e-5) && same(o.st, 0.30073, 5e-6)) {
        fails.push(format!(
            "C1r: old max C_D {:.5}, max C_L {:.5}, St {:.6} against 3.2462, 1.0113, 0.30073",
            o.cd_max, o.cl_max, o.st
        ));
    }
    assert!(fails.is_empty(), "{fails:#?}");
}

/// ADR 0249 A2 (the acceptance: DFG 2D-2 on M2), S2, D1p.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a2_two_d_two_on_m2() {
    let (xs, ys) = dfg_family(2);
    assert_eq!((xs.len() - 1) * (ys.len() - 1), 18304, "cell count of M2");
    let r = run_2d2("M2", &xs, &ys, Forcing::First).expect("S2: the M2 run stopped");
    let fails = judge_2d2("A2", &r);
    assert!(fails.is_empty(), "{fails:#?}");
}

/// Printed only (ADR 0249): 2D-2 on M2 with the second-order forcing.
#[test]
#[ignore = "printed only (ADR 0249), no limit; run in release with --ignored"]
fn printed_two_d_two_on_m2_second_order() {
    let (xs, ys) = dfg_family(2);
    match run_2d2("M2", &xs, &ys, Forcing::Second) {
        Some(r) => println!(
            "printed: M2 second: failures of the M2 judgement would be {:#?}",
            judge_2d2("M2 second", &r)
        ),
        None => println!("printed: 2D-2 on M2 with build_second_order stopped"),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// A3: steady cases
// ---------------------------------------------------------------------------------------------------------------------------

struct Steady {
    solver: SimpleSolver,
    ib: ImmersedForcing,
    xs: Vec<f64>,
    converged: bool,
    iterations: usize,
}

/// One 2D-1 run of ADR 0239 A2 (SIMPLEC, linear upwind, the convergence rule of ADR 0218 on the coefficients of
/// `force_moment_second_order`, the second-order forcing).
fn run_2d1(level: usize, init: Option<&Steady>, cap: usize) -> Steady {
    let (xs, ys) = dfg_family(level);
    let d = dfg(&xs, &ys, 0.3, Forcing::Second);
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);
    assert_eq!(nx * ny, [4576, 18304][level - 1], "cell count of M{level}");
    let (mut u, mut p) = (d.u, d.p);
    if let Some(c) = init {
        let nxc = c.xs.len() - 1;
        for j in 0..ny {
            for i in 0..nx {
                let pc = i / 2 + nxc * (j / 2);
                u.values[i + nx * j] = c.solver.u.values[pc];
                p.values[i + nx * j] = c.solver.p.values[pc];
            }
        }
    }
    let mut st = SimpleSettings::new(NU, Coupling::Simplec);
    st.convection = ConvScheme::LinearUpwind;
    st.momentum_solver.tol_abs = 1e-20;
    st.pressure_solver.tol_abs = 1e-20;
    let mut s = SimpleSolver::new(d.geom, u, p, st);
    s.immersed = Some(d.ib.clone());
    let q = 0.5 * 0.2 * 0.2 * DIAM * DZ;
    let coefficients = |u: &[Vec3]| {
        let (f, _) = d.ib.force_moment_second_order(0, u, 1.0, CENTRE);
        (f[0] / q, f[1] / q)
    };
    let mut hist: Vec<(f64, f64)> = vec![coefficients(&s.u.values)];
    let (mut iterations, mut converged) = (0, false);
    for it in 1..=cap {
        s.iterate();
        iterations = it;
        if it % 10 == 0 {
            let (cd, cl) = coefficients(&s.u.values);
            hist.push((cd, cl));
            if !cd.is_finite() {
                break;
            }
            if it >= 200 {
                let (cd0, cl0) = hist[hist.len() - 21];
                if (cd - cd0).abs() <= 1e-7 * cd.abs() && (cl - cl0).abs() <= 1e-8 {
                    converged = true;
                    break;
                }
            }
        }
    }
    Steady {
        solver: s,
        ib: d.ib,
        xs,
        converged,
        iterations,
    }
}

/// ADR 0249 A3a: a steady case by iteration returns the values of F-337.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a3a_steady_two_d_one_on_m2() {
    let m1 = run_2d1(1, None, 4000);
    assert!(
        m1.converged,
        "A3a set-up: M1 not converged ({} iterations)",
        m1.iterations
    );
    let m2 = run_2d1(2, Some(&m1), 4000);
    assert!(
        m2.converged,
        "A3a set-up: M2 not converged ({} iterations)",
        m2.iterations
    );
    let u = &m2.solver.u.values;
    let zero = vec![vec3::ZERO; u.len()];
    let (f, m) = m2.ib.force_moment_unsteady(0, u, &zero, 1.0, CENTRE);
    let (f0, m0) = m2.ib.force_moment_second_order(0, u, 1.0, CENTRE);
    let q = 0.5 * 0.2 * 0.2 * DIAM * DZ;
    let (cd, cl) = (f[0] / q, f[1] / q);
    println!(
        "A3a: M1 {} and M2 {} iterations; new C_D {cd:.8}, C_L {cl:.10}; F-337 C_D {:.8}, C_L {:.10}; moments {:?} {:?}",
        m1.iterations,
        m2.iterations,
        f0[0] / q,
        f0[1] / q,
        m,
        m0
    );
    assert!(
        f.iter()
            .chain(m.iter())
            .zip(f0.iter().chain(m0.iter()))
            .all(|(a, b)| a == b),
        "A3a: {f:?} {m:?} against {f0:?} {m0:?}"
    );
    assert!(
        (cd - 5.562_464_54).abs() <= 5e-9 && (cl - 0.010_888_049_2).abs() <= 5e-11,
        "A3a: C_D {cd:.9}, C_L {cl:.11} against 5.56246454, 0.0108880492"
    );
}

/// ADR 0249 A3b: a steady case reached by time stepping returns the values of F-337.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a3b_steady_couette_by_time_stepping() {
    let n = 48;
    let (geom, u, p) = cc_mesh(n);
    let ib = build(&geom, &cc_bodies(1.0, 0.0), 1e8, 1.0, Forcing::Second);
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib.clone());
    let dt = 0.0025;
    let mut before = s.u.values.clone();
    let mut change = f64::NAN;
    for _ in 0..400 {
        before.clone_from(&s.u.values);
        s.step(dt);
        assert!(s.diverged.is_none(), "{:?}", s.diverged);
        change =
            s.u.values
                .iter()
                .zip(&before)
                .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
                .fold(0.0, f64::max);
    }
    let rate = velocity_rate(&s.u.values, &before, None, dt);
    let dz = 2.4 / n as f64;
    let (f, m) = ib.force_moment_unsteady(0, &s.u.values, &rate, 1.0, CC_C);
    let (f0, m0) = ib.force_moment_second_order(0, &s.u.values, 1.0, CC_C);
    let t = 4.0 * std::f64::consts::PI * R1 * R1 * R2 * R2 / (R2 * R2 - R1 * R1);
    let dm = (m[2] - m0[2]).abs() / dz / t;
    let df = vec3::norm(vec3::sub(f, f0)) / dz;
    println!(
        "A3b: last step change {change:.3e}; M_z/dz new {:.9} F-337 {:.9} (exact {:.5}); |dM|/T {dm:.3e}; |dF|/dz {df:.3e}",
        m[2] / dz,
        m0[2] / dz,
        -t
    );
    assert!(change <= 1e-9, "A3b set-up: not steady ({change:.3e})");
    assert!(dm <= 1e-9, "A3b: moment {dm:.3e}");
    assert!(df <= 1e-9, "A3b: force {df:.3e}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// U1 to U5
// ---------------------------------------------------------------------------------------------------------------------------

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

fn random_field(n_cells: usize, mut seed: u64) -> Vec<Vec3> {
    (0..n_cells)
        .map(|_| [2.0 * lcg(&mut seed) - 1.0, 2.0 * lcg(&mut seed) - 1.0, 0.0])
        .collect()
}

const X1: Vec3 = [0.3, -0.2, 0.1];
const X2: Vec3 = [-1.1, 0.7, 0.4];

/// The forcings of the unit checks with their meshes: CC N 24 with `build` and `build_second_order`, DFG M1.
fn unit_forcings() -> Vec<(&'static str, Arc<FvGeometry>, ImmersedForcing)> {
    let (g1, _, _) = cc_mesh(24);
    let first = build(&g1, &cc_bodies(1.0, 0.0), 1e8, 1.0, Forcing::First);
    let second = build(&g1, &cc_bodies(1.0, 0.0), 1e8, 1.0, Forcing::Second);
    let (xs, ys) = dfg_family(1);
    let d = dfg(&xs, &ys, 0.3, Forcing::Second);
    vec![
        ("CC 24 build", g1.clone(), first),
        ("CC 24 build_second_order", g1, second),
        ("DFG M1", d.geom, d.ib),
    ]
}

/// ADR 0249 U1 to U3: the new evaluation is the reaction plus the storage of the solid cells; with a zero rate it is the
/// reaction; the reference point transfers.
#[test]
#[ignore = "defective set-up (ADR 0249 amendment 1), kept as a record; replaced by the test of the same name with b"]
fn u1_u2_u3_storage_of_the_solid_cells() {
    u1_checks(1.0, 1e-6);
}

/// ADR 0249 amendment 1: U1b to U3b (the rate field times 1e6, so that the storage is of the size of the reaction).
#[test]
fn u1b_u2b_u3b_storage_of_the_solid_cells() {
    u1_checks(1e6, 1e-4);
}

fn u1_checks(rate_scale: f64, guard: f64) {
    let rho = 1.3;
    let mut bodies = 0;
    for (name, geom, ib) in unit_forcings() {
        let u = random_field(geom.n_cells(), 0x0346);
        let dudt: Vec<Vec3> = random_field(geom.n_cells(), 0x0347)
            .iter()
            .map(|v| vec3::scale(*v, rate_scale))
            .collect();
        let zero = vec![vec3::ZERO; geom.n_cells()];
        for b in 0..ib.bodies.len() {
            bodies += 1;
            let (f0, m0) = ib.force_moment_second_order(b, &u, rho, X1);
            let (f, m) = ib.force_moment_unsteady(b, &u, &dudt, rho, X1);
            // U1
            let (sf, sm) = storage(&geom, &ib.bodies[b].solid_cells, &dudt, rho, X1);
            let d1f = vec3::norm(vec3::sub(f, vec3::add(f0, sf))) / vec3::norm(f);
            let d1m = vec3::norm(vec3::sub(m, vec3::add(m0, sm))) / vec3::norm(m);
            // U2
            let (fz, mz) = ib.force_moment_unsteady(b, &u, &zero, rho, X1);
            let same = fz
                .iter()
                .chain(mz.iter())
                .zip(f0.iter().chain(m0.iter()))
                .all(|(a, b)| a == b);
            // U3
            let (f2, m2) = ib.force_moment_unsteady(b, &u, &dudt, rho, X2);
            let shift = vec3::cross(vec3::sub(X2, X1), f2);
            let d3 = vec3::norm(vec3::sub(vec3::sub(m, m2), shift)) / vec3::norm(m);
            println!(
                "U1 to U3 {name} body {b}: {} solid cells; storage/total force {:.3e}, moment {:.3e}; U1 {d1f:.3e} {d1m:.3e}; \
                 U2 {same}; U3 {d3:.3e}",
                ib.bodies[b].solid_cells.len(),
                vec3::norm(sf) / vec3::norm(f),
                vec3::norm(sm) / vec3::norm(m)
            );
            assert!(
                ib.bodies[b].solid_cells.len() > 20,
                "{name} body {b}: too few solid cells"
            );
            assert!(
                vec3::norm(sf) > guard * vec3::norm(f) && vec3::norm(sm) > guard * vec3::norm(m),
                "{name} body {b}: the storage is not visible in the sum"
            );
            assert!(d1f <= 1e-12 && d1m <= 1e-12, "U1 {name} body {b}: {d1f:.3e} {d1m:.3e}");
            assert!(same, "U2 {name} body {b}: {fz:?} {mz:?} against {f0:?} {m0:?}");
            assert!(d3 <= 1e-10, "U3 {name} body {b}: {d3:.3e}");
        }
    }
    assert_eq!(bodies, 5);
}

/// ADR 0249 U4: the storage of a cuboid on grid lines with a rigid rate, against the closed form.
#[test]
fn u4_cuboid_with_a_rigid_rate() {
    let (geom, _, _) = cc_mesh(24);
    let body = ImmersedBody::fixed(
        "box",
        SolidShape::Cuboid {
            lo: [0.6, 0.9, -10.0],
            hi: [1.8, 1.5, 10.0],
        },
    );
    let (rho, alpha) = (1.3, 1.1);
    let (a, o): (Vec3, Vec3) = ([0.7, -0.4, 0.0], [0.2, 0.3, 0.0]);
    let cc = geom.mesh.cell_centres();
    let dudt: Vec<Vec3> = cc
        .iter()
        .map(|x| vec3::add(a, vec3::cross([0.0, 0.0, alpha], vec3::sub(*x, o))))
        .collect();
    let u = vec![vec3::ZERO; geom.n_cells()];
    let (dz, zc) = (0.1, cc[0][2]);
    let xcm: Vec3 = [1.2, 1.2, zc];
    let vb = 1.2 * 0.6 * dz;
    let want_f = vec3::scale(
        vec3::add(a, vec3::cross([0.0, 0.0, alpha], vec3::sub(xcm, o))),
        rho * vb,
    );
    let want_mz = rho * alpha * vb * (1.2 * 1.2 * (1.0 - 1.0 / 144.0) + 0.6 * 0.6 * (1.0 - 1.0 / 36.0)) / 12.0;
    for forcing in [Forcing::First, Forcing::Second] {
        let ib = build(&geom, std::slice::from_ref(&body), 1e8, 1.0, forcing);
        assert_eq!(ib.bodies[0].solid_cells.len(), 72, "U4: 12 x 6 solid cells");
        let (f0, m0) = ib.force_moment_second_order(0, &u, rho, xcm);
        let (f, m) = ib.force_moment_unsteady(0, &u, &dudt, rho, xcm);
        let df = vec3::norm(vec3::sub(vec3::sub(f, f0), want_f)) / vec3::norm(want_f);
        let dm = vec3::sub(m, m0);
        let dmz = (dm[2] - want_mz).abs() / want_mz;
        let dmxy = (dm[0].abs() + dm[1].abs()) / want_mz;
        println!(
            "U4 {forcing:?}: reaction {f0:?}; storage force {:?} (closed form {want_f:?}), {df:.3e}; moment z {:.12e} (closed \
             form {want_mz:.12e}), {dmz:.3e}; x, y {dmxy:.3e}",
            vec3::sub(f, f0),
            dm[2]
        );
        assert!(df <= 1e-12, "U4 force: {df:.3e}");
        assert!(dmz <= 1e-12 && dmxy <= 1e-12, "U4 moment: {dmz:.3e} {dmxy:.3e}");
    }
}

/// ADR 0249 U5: `velocity_rate` is exact for a quadratic in time with a variable step (BDF2) and is the backward difference
/// without the older level.
#[test]
fn u5_velocity_rate() {
    let n = 200;
    let (u0, u1, u2) = (
        random_field(n, 0x1346),
        random_field(n, 0x2346),
        random_field(n, 0x3346),
    );
    let at = |t: f64| -> Vec<Vec3> {
        (0..n)
            .map(|i| [0, 1, 2].map(|k| u0[i][k] + u1[i][k] * t + u2[i][k] * t * t))
            .collect()
    };
    let (dt_old, dt) = (0.2, 0.3);
    let (older, old, new) = (at(0.0), at(dt_old), at(dt_old + dt));
    let rate = velocity_rate(&new, &old, Some((&older, dt_old)), dt);
    let t = dt_old + dt;
    let (mut err, mut size) = (0.0f64, 0.0f64);
    for i in 0..n {
        for k in 0..2 {
            let want = u1[i][k] + 2.0 * u2[i][k] * t;
            err = err.max((rate[i][k] - want).abs());
            size = size.max(want.abs());
        }
    }
    let euler = velocity_rate(&new, &old, None, dt);
    let same = (0..n).all(|i| (0..3).all(|k| euler[i][k] == (new[i][k] - old[i][k]) / dt));
    println!(
        "U5: BDF2 error {:.3e} of the largest rate {size:.3}; Euler equal {same}",
        err / size
    );
    assert!(err <= 1e-12 * size, "U5 BDF2: {:.3e}", err / size);
    assert!(same, "U5 Euler");
}
