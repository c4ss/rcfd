//! F-246 (ADR 0193): pressure-interpolation variants of the steady SIMPLE kernel, library checks. Limits are those registered in
//! docs/adr/0193-f246-registration-pressure-interpolation-variants.md before this file was written.
//!
//! P: mesh preconditions. M: an incompressible manufactured solution on the F-208 meshes (M1 orders of the pair (32, 64), M2 error
//! ratios on the near-orthogonal meshes at n = 32, M3 the levels 16 and 32). B: the quiescent stably stratified Boussinesq box on the
//! skewed mesh K. L0: the body-force-weighted variant without a body force is the default bit for bit.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::energy::boussinesq_force;
use rcfd_fv::pressure_interp::PressureInterpolation;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::Mesh;
use std::sync::Arc;
use std::time::Instant;

const TP: f64 = 2.0 * std::f64::consts::PI;
/// Kinematic viscosity of the manufactured solution.
const NU: f64 = 0.1;
/// M1 limits: order of the pair (32, 64) for the velocity and the pressure.
const ORDER_U: f64 = 1.7;
const ORDER_P: f64 = 1.2;
/// M2 limit: velocity error of a variant over the default on G and G3 at n = 32.
const NEAR_ORTHOGONAL_RATIO: f64 = 1.10;
/// Stop rule: estimated remaining iteration error over the largest exact speed; iteration cap.
const ITER_ERROR: f64 = 1e-10;
const CAP: usize = 30000;

#[derive(Clone, Copy, Debug, PartialEq)]
enum MeshKind {
    G,
    G3,
    K,
    W,
    S,
}

/// The meshes of F-208 (ADR 0016, `mms_poor_meshes.rs`).
fn make_mesh(kind: MeshKind, n: usize) -> Mesh {
    let un = [Grading::Uniform; 3];
    match kind {
        MeshKind::G => box_mesh([n; 3], [1.0; 3], un).unwrap(),
        MeshKind::G3 => warp(&box_mesh([n; 3], [1.0; 3], un).unwrap(), |p| {
            [p[0] + 0.015 * (TP * p[1]).sin(), p[1] + 0.015 * (TP * p[0]).sin(), p[2]]
        })
        .unwrap(),
        MeshKind::K => warp(&box_mesh([n; 3], [1.0; 3], un).unwrap(), |p| {
            [p[0] + 0.7 * p[1] + 0.3 * p[2], p[1] + 0.3 * p[2], p[2]]
        })
        .unwrap(),
        MeshKind::W => warp(&box_mesh([n; 3], [1.0; 3], un).unwrap(), |p| {
            [
                p[0] + 0.1 * (TP * p[1]).sin() * (TP * p[2]).sin(),
                p[1] + 0.1 * (TP * p[0]).sin() * (TP * p[2]).sin(),
                p[2] + 0.1 * (TP * p[0]).sin() * (TP * p[1]).sin(),
            ]
        })
        .unwrap(),
        MeshKind::S => box_mesh(
            [n; 3],
            [1.0; 3],
            [Grading::Uniform, Grading::Expansion(50.0), Grading::Uniform],
        )
        .unwrap(),
    }
}

fn geometry(kind: MeshKind, n: usize) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(make_mesh(kind, n))).unwrap())
}

/// Largest `1 - |S_f| / sum of the fan-triangle areas` over the faces.
fn non_planarity(m: &Mesh) -> f64 {
    let pts = m.points();
    let mut worst = 0.0_f64;
    for f in 0..m.n_faces() {
        let ids = m.face_points(f);
        let c = m.face_centres()[f];
        let mut sum = 0.0;
        for j in 0..ids.len() {
            let (a, b) = (pts[ids[j]], pts[ids[(j + 1) % ids.len()]]);
            sum += 0.5 * vec3::norm(vec3::cross(vec3::sub(a, c), vec3::sub(b, c)));
        }
        worst = worst.max(1.0 - m.face_area(f) / sum);
    }
    worst
}

/// Largest `|x_f - x_f'| / |d|` over the internal faces, `x_f' = w x_P + (1 - w) x_N`.
fn skewness(geom: &FvGeometry) -> f64 {
    let mesh = &geom.mesh;
    let (own, nei, cc, cf) = (mesh.owner(), mesh.neighbour(), mesh.cell_centres(), mesh.face_centres());
    (0..geom.n_internal)
        .map(|f| {
            let w = geom.weights[f];
            let xp = vec3::add(vec3::scale(cc[own[f]], w), vec3::scale(cc[nei[f]], 1.0 - w));
            vec3::norm(vec3::sub(cf[f], xp)) / vec3::norm(geom.delta[f])
        })
        .fold(0.0, f64::max)
}

// ---- manufactured solution (ADR 0193 M) ------------------------------------------------------------------------------------------

/// Velocity; every component is independent of its own coordinate, so the divergence is zero exactly.
fn exact_u(x: Vec3) -> Vec3 {
    [
        (1.1 * x[1] + 0.3).sin() * (0.9 * x[2] + 0.2).cos(),
        (1.2 * x[0] + 0.1).cos() * (0.8 * x[2] + 0.4).sin(),
        (0.7 * x[0] + 0.5).sin() * (1.3 * x[1] + 0.2).cos(),
    ]
}

/// Kinematic pressure.
fn exact_p(x: Vec3) -> f64 {
    (0.9 * x[0] + 0.3).cos() * (1.2 * x[1] + 0.1).sin() * (0.8 * x[2] + 0.5).cos()
}

fn grad_p(x: Vec3) -> Vec3 {
    let (s1, c1) = (0.9 * x[0] + 0.3).sin_cos();
    let (s2, c2) = (1.2 * x[1] + 0.1).sin_cos();
    let (s3, c3) = (0.8 * x[2] + 0.5).sin_cos();
    [-0.9 * s1 * s2 * c3, 1.2 * c1 * c2 * c3, -0.8 * c1 * s2 * s3]
}

/// Source acceleration `f = (u . grad) u - nu lap u + grad p`.
fn source(x: Vec3) -> Vec3 {
    let u = exact_u(x);
    let (sa1, ca1) = (1.1 * x[1] + 0.3).sin_cos();
    let (sb1, cb1) = (0.9 * x[2] + 0.2).sin_cos();
    let (sa2, ca2) = (1.2 * x[0] + 0.1).sin_cos();
    let (sb2, cb2) = (0.8 * x[2] + 0.4).sin_cos();
    let (sa3, ca3) = (0.7 * x[0] + 0.5).sin_cos();
    let (sb3, cb3) = (1.3 * x[1] + 0.2).sin_cos();
    // derivatives that are not zero
    let (u_y, u_z) = (1.1 * ca1 * cb1, -0.9 * sa1 * sb1);
    let (v_x, v_z) = (-1.2 * sa2 * sb2, 0.8 * ca2 * cb2);
    let (w_x, w_y) = (0.7 * ca3 * cb3, -1.3 * sa3 * sb3);
    let conv = [
        u[1] * u_y + u[2] * u_z,
        u[0] * v_x + u[2] * v_z,
        u[0] * w_x + u[1] * w_y,
    ];
    let k2 = [1.1 * 1.1 + 0.9 * 0.9, 1.2 * 1.2 + 0.8 * 0.8, 0.7 * 0.7 + 1.3 * 1.3];
    let gp = grad_p(x);
    [0, 1, 2].map(|i| conv[i] + NU * k2[i] * u[i] + gp[i])
}

const VARIANTS: [(&str, PressureInterpolation); 3] = [
    ("V0", PressureInterpolation::Linear),
    ("V1", PressureInterpolation::SkewCorrected),
    ("V2", PressureInterpolation::BodyForceWeighted),
];

#[derive(Debug, Clone, Copy)]
struct Outcome {
    e_u: f64,
    e_p: f64,
    iterations: usize,
    converged: bool,
}

/// The manufactured solution on `kind` at `n` with the variant `v` (ADR 0193 M: data, kernel set-up, stop rule, errors).
fn mms(kind: MeshKind, n: usize, v: PressureInterpolation) -> Outcome {
    let start = Instant::now();
    let geom = geometry(kind, n);
    let mesh = &geom.mesh;
    let (cc, cf, sf, vol) = (
        mesh.cell_centres(),
        mesh.face_centres(),
        mesh.face_area_vectors(),
        mesh.cell_volumes(),
    );
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let n_int = geom.n_internal;
    // velocity on the boundary: the exact value with a uniform normal shift that makes the net flux zero
    let bfaces: Vec<usize> = (n_int..mesh.n_faces()).filter(|&f| !geom.empty[f]).collect();
    let e_net: f64 = bfaces.iter().map(|&f| vec3::dot(exact_u(cf[f]), sf[f])).sum();
    let a_b: f64 = bfaces.iter().map(|&f| vec3::norm(sf[f])).sum();
    let ub = |f: usize| vec3::sub(exact_u(cf[f]), vec3::scale(vec3::unit(sf[f]), e_net / a_b));
    let (net, gross) = bfaces.iter().fold((0.0, 0.0), |(a, b), &f| {
        let q = vec3::dot(ub(f), sf[f]);
        (a + q, b + q.abs())
    });
    assert!(
        net.abs() <= 1e-13 * gross,
        "M set-up {kind:?} n={n}: net boundary flux {net:e} of {gross:e}"
    );
    let mut u = VectorField::from_fn("U", &geom, exact_u);
    for p in mesh.patches() {
        if p.kind == rcfd_mesh::PatchKind::Empty {
            continue;
        }
        let vals: Vec<Vec3> = p.faces().map(ub).collect();
        u.set_bc(&geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    let p = ScalarField::from_fn("p", &geom, exact_p);
    // source: cell values, internal faces from the cell values (as BodyForce::from_cells), boundary faces the true wall
    // pressure gradient (ADR 0069)
    let cell: Vec<Vec3> = cc.iter().map(|&x| source(x)).collect();
    let face: Vec<f64> = (0..mesh.n_faces())
        .map(|f| {
            if geom.empty[f] {
                0.0
            } else if f < n_int {
                let w = geom.weights[f];
                let a = vec3::add(vec3::scale(cell[own[f]], w), vec3::scale(cell[nei[f]], 1.0 - w));
                vec3::dot(a, sf[f])
            } else {
                vec3::dot(grad_p(cf[f]), sf[f])
            }
        })
        .collect();
    let mut s = SimpleSettings::new(NU, Coupling::Simple);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = 1;
    s.momentum_solver.tol_abs = 1e-300;
    s.pressure_solver.tol_abs = 1e-300;
    let mut sol = SimpleSolver::new(geom.clone(), u, p, s);
    sol.body_force = Some(BodyForce { cell, face });
    sol.pressure_interpolation = v;
    let umax = cc.iter().map(|&x| vec3::norm(exact_u(x))).fold(0.0, f64::max);
    let mut deltas: Vec<f64> = Vec::new();
    let mut converged = false;
    for k in 1..=CAP {
        let info = sol.iterate();
        deltas.push(info.max_du);
        if !info.max_du.is_finite() {
            break;
        }
        if k >= 100 {
            let rho = (deltas[k - 1] / deltas[k - 51]).powf(1.0 / 50.0);
            if rho < 1.0 && deltas[k - 1] * rho / (1.0 - rho) <= ITER_ERROR * umax {
                converged = true;
                break;
            }
        }
    }
    let tot: f64 = vol.iter().sum();
    let mean = |a: &dyn Fn(usize) -> f64| (0..cc.len()).map(|c| a(c) * vol[c]).sum::<f64>() / tot;
    let (pm, pe) = (mean(&|c| sol.p.values[c]), mean(&|c| exact_p(cc[c])));
    let (mut eu, mut ep) = (0.0, 0.0);
    for c in 0..cc.len() {
        let du = vec3::sub(sol.u.values[c], exact_u(cc[c]));
        eu += vec3::dot(du, du) * vol[c];
        ep += ((sol.p.values[c] - pm) - (exact_p(cc[c]) - pe)).powi(2) * vol[c];
    }
    let o = Outcome {
        e_u: (eu / tot).sqrt(),
        e_p: (ep / tot).sqrt(),
        iterations: deltas.len(),
        converged,
    };
    println!(
        "  {kind:?} n={n} {v:?}: e_u {:.4e} e_p {:.4e}, {} iterations, converged {}, net flux {:.1e} of {:.1e}, {:.1} s",
        o.e_u,
        o.e_p,
        o.iterations,
        o.converged,
        net,
        gross,
        start.elapsed().as_secs_f64()
    );
    o
}

/// P: the registered preconditions of the meshes (ADR 0193 P).
#[test]
fn p_preconditions() {
    let q = |k| {
        let m = make_mesh(k, 16);
        (check_mesh(&m), non_planarity(&m))
    };
    let (g3, _) = q(MeshKind::G3);
    let (k, _) = q(MeshKind::K);
    let (w, np) = q(MeshKind::W);
    let (s, _) = q(MeshKind::S);
    println!(
        "P: G3 nonorth {:.2} deg; K nonorth {:.2} deg; W nonorth {:.2} deg, nonplanarity {np:.3e}; S aspect {:.2}",
        g3.max_non_orthogonality, k.max_non_orthogonality, w.max_non_orthogonality, s.max_aspect_ratio
    );
    for n in [32, 64] {
        println!(
            "P: W nonorth at n = {n}: {:.2} deg",
            check_mesh(&make_mesh(MeshKind::W, n)).max_non_orthogonality
        );
    }
    let (sk, sg3, sw) = (
        skewness(&geometry(MeshKind::K, 16)),
        skewness(&geometry(MeshKind::G3, 16)),
        skewness(&geometry(MeshKind::W, 16)),
    );
    println!(
        "P: largest |x_f - x_f'| / |d| at n = 16: K {sk:.3e} (at most 1e-12), G3 {sg3:.3e}, W {sw:.3e} (at least 1e-6)"
    );
    assert!(g3.max_non_orthogonality <= 12.0);
    assert!(k.max_non_orthogonality >= 25.0);
    assert!(w.max_non_orthogonality >= 20.0 && np >= 1e-4);
    assert!(s.max_aspect_ratio >= 10.0);
    assert!(sk <= 1e-12, "P K skewness {sk}");
    assert!(sg3 >= 1e-6 && sw >= 1e-6, "P G3 {sg3}, W {sw}");
}

/// M3 (reported): the levels n = 16 and 32 of every variant on one mesh; V0 must converge (the set-up-defect rule).
fn levels_16_32(kind: MeshKind) {
    for (name, v) in VARIANTS {
        let (a, b) = (mms(kind, 16, v), mms(kind, 32, v));
        println!(
            "M3 {kind:?} {name}: order (16, 32) velocity {:.3}, pressure {:.3}",
            (a.e_u / b.e_u).log2(),
            (a.e_p / b.e_p).log2()
        );
        if name == "V0" {
            assert!(a.converged && b.converged, "set-up: V0 on {kind:?} not converged");
        }
    }
}

#[test]
fn m3_k_levels_16_32() {
    levels_16_32(MeshKind::K);
}

#[test]
fn m3_w_levels_16_32() {
    levels_16_32(MeshKind::W);
}

#[test]
fn m3_s_levels_16_32() {
    levels_16_32(MeshKind::S);
}

/// M1: the sequence n = 16, 32, 64 of one variant on one mesh; the order of the pair (32, 64) is bound for V1 and V2 (V0 reported).
fn sequence(kind: MeshKind, name: &str) {
    let v = VARIANTS.iter().find(|x| x.0 == name).unwrap().1;
    let r: Vec<Outcome> = [16, 32, 64].iter().map(|&n| mms(kind, n, v)).collect();
    let (ou, op) = ((r[1].e_u / r[2].e_u).log2(), (r[1].e_p / r[2].e_p).log2());
    println!(
        "M1 {kind:?} {name}: orders velocity {:.3} / {ou:.3}, pressure {:.3} / {op:.3} (pairs (16, 32) / (32, 64)); converged {:?}",
        (r[0].e_u / r[1].e_u).log2(),
        (r[0].e_p / r[1].e_p).log2(),
        r.iter().map(|o| o.converged).collect::<Vec<_>>()
    );
    if name != "V0" {
        assert!(
            r.iter().all(|o| o.converged),
            "M1 {kind:?} {name}: a run did not converge"
        );
        assert!(ou >= ORDER_U, "M1 {kind:?} {name}: velocity order {ou:.3} < {ORDER_U}");
        assert!(op >= ORDER_P, "M1 {kind:?} {name}: pressure order {op:.3} < {ORDER_P}");
    }
}

#[test]
fn m1_k_v0() {
    sequence(MeshKind::K, "V0");
}

#[test]
fn m1_k_v1() {
    sequence(MeshKind::K, "V1");
}

#[test]
fn m1_k_v2() {
    sequence(MeshKind::K, "V2");
}

#[test]
fn m1_w_v0() {
    sequence(MeshKind::W, "V0");
}

#[test]
fn m1_w_v1() {
    sequence(MeshKind::W, "V1");
}

#[test]
fn m1_w_v2() {
    sequence(MeshKind::W, "V2");
}

#[test]
fn m1_s_v0() {
    sequence(MeshKind::S, "V0");
}

#[test]
fn m1_s_v1() {
    sequence(MeshKind::S, "V1");
}

#[test]
fn m1_s_v2() {
    sequence(MeshKind::S, "V2");
}

/// M2: on the near-orthogonal meshes G and G3 at n = 32 no variant raises the velocity error over V0 by more than 1.10x.
#[test]
fn m2_near_orthogonal_meshes() {
    let mut misses = Vec::new();
    for kind in [MeshKind::G, MeshKind::G3] {
        let r: Vec<Outcome> = VARIANTS.iter().map(|&(_, v)| mms(kind, 32, v)).collect();
        assert!(r[0].converged, "set-up: V0 on {kind:?} not converged");
        for (i, (name, _)) in VARIANTS.iter().enumerate().skip(1) {
            let (ru, rp) = (r[i].e_u / r[0].e_u, r[i].e_p / r[0].e_p);
            println!(
                "M2 {kind:?} {name}: e_u / e_u(V0) = {ru:.4} (limit {NEAR_ORTHOGONAL_RATIO}), e_p / e_p(V0) = {rp:.4} (reported)"
            );
            if !r[i].converged {
                misses.push(format!("{kind:?} {name}: not converged"));
            }
            if ru > NEAR_ORTHOGONAL_RATIO {
                misses.push(format!("{kind:?} {name}: velocity ratio {ru:.4}"));
            }
        }
    }
    assert!(misses.is_empty(), "M2: {misses:#?}");
}

// ---- B: the quiescent stably stratified Boussinesq box on K ----------------------------------------------------------------------

/// Velocity scale `|g| beta dT H^2 / nu` of the box.
const U_B: f64 = 100.0;
const B_ITERS: usize = 4000;

struct BoxRun {
    speed: f64,
    face_speed: f64,
    last_change: f64,
}

/// The box on K at n = 16 with the frozen profile T = z, `beta = 1`, `g = (0, 0, -10)`, `t_ref = 0.5`, `nu = 0.1`.
fn stratified_box(coupling: Coupling, correctors: usize, v: PressureInterpolation) -> BoxRun {
    let geom = geometry(MeshKind::K, 16);
    let mesh = &geom.mesh;
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let mut t = ScalarField::from_fn("T", &geom, |x| x[2]);
    for p in mesh.patches() {
        u.set_bc(&geom, &p.name, Box::new(FixedValue::vector([0.0; 3])))
            .unwrap();
        let tb: Vec<Vec3> = p.faces().map(|f| [mesh.face_centres()[f][2], 0.0, 0.0]).collect();
        t.set_bc(&geom, &p.name, Box::new(FixedValue(Profile::PerFace(tb))))
            .unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(NU, coupling);
    s.non_orth_correctors = correctors;
    let mut sol = SimpleSolver::new(geom.clone(), u, p, s);
    sol.body_force = Some(boussinesq_force(&geom, &t, 1.0, [0.0, 0.0, -10.0], 0.5));
    sol.pressure_interpolation = v;
    let mut last = f64::NAN;
    for _ in 0..B_ITERS {
        last = sol.iterate().max_du;
    }
    let speed = sol.u.values.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
    let sf = mesh.face_area_vectors();
    let face_speed = (0..mesh.n_faces())
        .filter(|&f| !geom.empty[f])
        .map(|f| sol.phi[f].abs() / vec3::norm(sf[f]))
        .fold(0.0, f64::max);
    BoxRun {
        speed,
        face_speed,
        last_change: last,
    }
}

/// B0 to B3 for one kernel set-up.
fn stratified(label: &str, coupling: Coupling, correctors: usize) {
    let r0 = stratified_box(coupling, correctors, PressureInterpolation::Linear);
    let r2 = stratified_box(coupling, correctors, PressureInterpolation::BodyForceWeighted);
    println!(
        "B {label}: largest cell speed V0 {:.3e} ({:.3e} U_b), V2 {:.3e} ({:.3e} U_b); ratio {:.3e}; face speed V0 {:.3e}, V2 {:.3e}; last change V0 {:.3e}, V2 {:.3e}",
        r0.speed,
        r0.speed / U_B,
        r2.speed,
        r2.speed / U_B,
        r0.speed / r2.speed,
        r0.face_speed,
        r2.face_speed,
        r0.last_change,
        r2.last_change
    );
    assert!(
        r0.last_change <= 1e-12 * U_B && r2.last_change <= 1e-12 * U_B,
        "B3 {label}: not converged"
    );
    assert!(
        r0.speed >= 1e-7 * U_B,
        "B0 {label}: V0 speed {} below the teeth floor",
        r0.speed
    );
    assert!(
        r2.speed <= r0.speed / 10.0,
        "B1 {label}: V2 {} vs V0 {}",
        r2.speed,
        r0.speed
    );
    assert!(r2.speed <= 1e-8 * U_B, "B2 {label}: V2 {}", r2.speed);
}

#[test]
fn b_stratified_box_simplec_without_correctors() {
    stratified("B-a", Coupling::Simplec, 0);
}

#[test]
fn b_stratified_box_simple_with_a_corrector() {
    stratified("B-b", Coupling::Simple, 1);
}

/// L0: without a body force the body-force-weighted variant is the default bit for bit (lid cavity on K, n = 8).
#[test]
fn l0_body_force_weighted_without_a_body_force_is_linear() {
    let run = |v: PressureInterpolation| {
        let geom = geometry(MeshKind::K, 8);
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        for p in geom.mesh.patches() {
            let lid = if p.name == "zmax" { [1.0, 0.0, 0.0] } else { [0.0; 3] };
            u.set_bc(&geom, &p.name, Box::new(FixedValue::vector(lid))).unwrap();
        }
        let p = ScalarField::uniform("p", &geom, 0.0);
        let mut s = SimpleSettings::new(NU, Coupling::Simplec);
        s.non_orth_correctors = 1;
        let mut sol = SimpleSolver::new(geom.clone(), u, p, s);
        sol.pressure_interpolation = v;
        for _ in 0..50 {
            sol.iterate();
        }
        (sol.u.values.clone(), sol.p.values.clone())
    };
    let (u0, p0) = run(PressureInterpolation::Linear);
    let (u2, p2) = run(PressureInterpolation::BodyForceWeighted);
    let du = u0
        .iter()
        .zip(&u2)
        .filter(|(a, b)| (0..3).any(|k| a[k].to_bits() != b[k].to_bits()))
        .count();
    let dp = p0.iter().zip(&p2).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    let moved = u0.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
    println!(
        "L0: {du} of {} cells differ in a bit of u, {dp} in p (limit 0); largest speed {moved:.3e}",
        u0.len()
    );
    assert!(moved > 1e-3, "L0 set-up: the lid drives no flow");
    assert_eq!((du, dp), (0, 0), "L0");
}
