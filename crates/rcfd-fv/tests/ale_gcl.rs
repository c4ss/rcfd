//! Moving-mesh (ALE) checks G1-G3 of ADR 0033: swept volumes and the discrete geometric conservation law, a uniform
//! flow on translating, rotating, oscillating and expanding meshes, and Galilean invariance.
//! The limits are those registered in the ADR before the code ran.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::move_mesh;
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, cavity_2d_rect, Grading};
use rcfd_mesh::motion::{cell_swept_sum, moved_mesh, swept_volumes, swept_volumes_about, Pose};
use rcfd_mesh::Mesh;
use std::sync::Arc;

const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

fn mesh_a() -> Mesh {
    box_mesh(
        [5, 4, 3],
        [1.0, 0.8, 0.6],
        [Grading::Expansion(2.0), Grading::Symmetric(2.0), Grading::Uniform],
    )
    .unwrap()
}

/// Mesh (a) with every interior point displaced so that the faces are warped.
fn mesh_b() -> Mesh {
    let a = mesh_a();
    let l = [1.0, 0.8, 0.6];
    let hmin = a.cell_volumes().iter().map(|v| v.cbrt()).fold(f64::INFINITY, f64::min);
    let mut pts: Vec<Vec3> = a.points().to_vec();
    for (i, p) in pts.iter_mut().enumerate() {
        if (0..3).all(|k| p[k] > 1e-9 && p[k] < l[k] - 1e-9) {
            let w = 0.15 * hmin;
            let fi = i as f64;
            *p = vec3::add(
                *p,
                [
                    w * (7.0 * fi + 1.0).sin(),
                    w * (5.0 * fi + 2.0).cos(),
                    w * (3.0 * fi + 4.0).sin(),
                ],
            );
        }
    }
    moved_mesh(&a, pts).unwrap()
}

type Motion = Box<dyn Fn(&Mesh, f64) -> Vec<Vec3>>;

/// Point positions at time `t` for the four motions of the registered checks (functions of the initial points);
/// each entry carries its step size and the number of steps.
fn motions() -> Vec<(&'static str, Motion, f64, usize)> {
    let centre = [0.5, 0.4, 0.3];
    let tau = std::f64::consts::TAU;
    vec![
        (
            "translation",
            Box::new(|m: &Mesh, t: f64| {
                m.points()
                    .iter()
                    .map(|&p| vec3::add(p, vec3::scale([0.3, -0.2, 0.1], t / 0.05)))
                    .collect()
            }),
            0.05,
            1,
        ),
        (
            "rotation",
            Box::new(|m: &Mesh, t: f64| {
                let pose = Pose {
                    origin: [0.2, 0.1, 0.0],
                    axis: [1.0, 2.0, 3.0],
                    angle: 0.4 * t / 0.05,
                    translation: [0.0; 3],
                };
                m.points().iter().map(|&p| pose.apply(p)).collect()
            }),
            0.05,
            1,
        ),
        (
            "oscillation",
            Box::new(move |m: &Mesh, t: f64| {
                let dx = 0.1 * (tau * t / 1.0).sin();
                m.points().iter().map(|&p| vec3::add(p, [dx, 0.0, 0.0])).collect()
            }),
            0.125,
            8,
        ),
        (
            "expansion",
            Box::new(move |m: &Mesh, t: f64| {
                let s = [1.0 + 0.25 * t / 0.05, 1.0 + 0.1 * t / 0.05, 1.0 - 0.1 * t / 0.05];
                m.points()
                    .iter()
                    .map(|&p| {
                        let q = vec3::sub(p, centre);
                        vec3::add(centre, [s[0] * q[0], s[1] * q[1], s[2] * q[2]])
                    })
                    .collect()
            }),
            0.05,
            1,
        ),
    ]
}

#[test]
fn g1_swept_volumes_satisfy_the_gcl() {
    for (mname, mesh) in [("a", mesh_a()), ("b", mesh_b())] {
        let vmean = mesh.total_volume() / mesh.n_cells() as f64;
        let smax = mesh
            .face_area_vectors()
            .iter()
            .map(|s| vec3::norm(*s))
            .fold(0.0, f64::max);
        for (name, motion, dt, nsteps) in motions() {
            let mut cur = mesh.clone();
            let mut worst_gcl: f64 = 0.0;
            let mut worst_origin: f64 = 0.0;
            for s in 1..=nsteps {
                let np = motion(&mesh, s as f64 * dt);
                let w = swept_volumes(&cur, &np);
                let w_alt = swept_volumes_about(&cur, &np, Some([3.0, -2.0, 1.5]));
                let next = moved_mesh(&cur, np).unwrap();
                let sums = cell_swept_sum(&cur, &w);
                for c in 0..cur.n_cells() {
                    let dv = next.cell_volumes()[c] - cur.cell_volumes()[c];
                    worst_gcl = worst_gcl.max((sums[c] - dv).abs() / vmean);
                }
                for f in 0..cur.n_faces() {
                    worst_origin = worst_origin.max((w[f] - w_alt[f]).abs() / vmean);
                }
                cur = next;
            }
            println!("G1 mesh {mname} {name}: GCL {worst_gcl:.2e}, origin {worst_origin:.2e}");
            assert!(worst_gcl <= 1e-12, "{mname} {name}: GCL {worst_gcl}");
            assert!(
                worst_origin <= 1e-12,
                "{mname} {name}: origin dependence {worst_origin}"
            );
            if name == "translation" {
                let d = [0.3, -0.2, 0.1];
                let np = motion(&mesh, dt);
                let w = swept_volumes(&mesh, &np);
                let worst = (0..mesh.n_faces())
                    .map(|f| (w[f] - vec3::dot(d, mesh.face_area_vectors()[f])).abs())
                    .fold(0.0, f64::max);
                let rel = worst / (vec3::norm(d) * smax);
                println!("G1 mesh {mname} translation W - d.S: {rel:.2e}");
                assert!(rel <= 1e-12, "{rel}");
            }
        }
    }
}

fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

/// One moving-mesh step to the point positions `pts`.
fn advance(s: &mut PimpleSolver, pts: Vec<Vec3>, dt: f64) -> rcfd_fv::pimple::StepInfo {
    let step = move_mesh(&s.geom, pts, dt).expect("valid mesh");
    s.step_ale(dt, step.geom, step.mesh_flux).expect("ale step")
}

#[test]
fn g2_uniform_flow_is_preserved_on_moving_meshes() {
    let u0 = [1.0, 0.5, -0.25];
    let tau = std::f64::consts::TAU;
    let centre = [0.5, 0.4, 0.3];
    let dt = 0.05;
    let cases: Vec<(&str, Motion)> = vec![
        (
            "translation",
            Box::new(|m: &Mesh, t: f64| {
                m.points()
                    .iter()
                    .map(|&p| vec3::add(p, vec3::scale([0.4, -0.3, 0.2], t)))
                    .collect()
            }),
        ),
        (
            "rotation",
            Box::new(move |m: &Mesh, t: f64| {
                let pose = Pose {
                    origin: centre,
                    axis: [0.0, 0.0, 1.0],
                    angle: t, // 0.5 rad over the 10 steps
                    translation: [0.0; 3],
                };
                m.points().iter().map(|&p| pose.apply(p)).collect()
            }),
        ),
        (
            "oscillation",
            Box::new(move |m: &Mesh, t: f64| {
                let dx = 0.05 * (tau * t / 0.5).sin();
                m.points().iter().map(|&p| vec3::add(p, [dx, 0.0, 0.0])).collect()
            }),
        ),
        (
            "expansion",
            Box::new(move |m: &Mesh, t: f64| {
                let s = 1.0 + 0.2 * (tau * t / 0.5).sin();
                m.points()
                    .iter()
                    .map(|&p| vec3::add(centre, vec3::scale(vec3::sub(p, centre), s)))
                    .collect()
            }),
        ),
    ];
    for (name, motion) in cases {
        let mesh = mesh_a();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
        let mut u = VectorField::uniform("U", &geom, u0);
        for p in mesh.patches() {
            u.set_bc(&geom, &p.name, Box::new(FixedValue::vector(u0))).unwrap();
        }
        let p = ScalarField::uniform("p", &geom, 0.0);
        let mut s = PimpleSolver::new(geom, u, p, settings(0.01));
        for n in 1..=10 {
            advance(&mut s, motion(&mesh, n as f64 * dt), dt);
        }
        let du =
            s.u.values
                .iter()
                .map(|v| vec3::norm(vec3::sub(*v, u0)))
                .fold(0.0, f64::max);
        let dp = s.p.values.iter().fold(0.0f64, |a, b| a.max(b.abs()));
        println!("G2 {name}: max|u-u0| {du:.2e}, max|p| {dp:.2e}");
        assert!(du <= 1e-12, "{name}: |u-u0| {du}");
        assert!(dp <= 1e-12, "{name}: |p| {dp}");
    }
}

/// Lid-driven cavity started from rest, in a fixed frame or in a frame translating with `v`.
fn cavity_run(v: Vec3, steps: usize, dt: f64) -> (PimpleSolver, Vec<Vec3>) {
    let mesh = cavity_2d_rect(8, 8, 1.0, 1.0).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, v);
    u.set_bc(&geom, "walls", Box::new(FixedValue(Profile::Uniform(v))))
        .unwrap();
    u.set_bc(
        &geom,
        "lid",
        Box::new(FixedValue(Profile::Uniform(vec3::add(v, [1.0, 0.0, 0.0])))),
    )
    .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSolver::new(geom, u, p, settings(0.05));
    for n in 1..=steps {
        if v == vec3::ZERO {
            s.step(dt);
        } else {
            let t = n as f64 * dt;
            let pts: Vec<Vec3> = mesh.points().iter().map(|&p| vec3::add(p, vec3::scale(v, t))).collect();
            advance(&mut s, pts, dt);
        }
    }
    let u_rel = s.u.values.iter().map(|x| vec3::sub(*x, v)).collect();
    (s, u_rel)
}

#[test]
fn g3_galilean_invariance_of_a_cavity_start_up() {
    let (fixed, u_fixed) = cavity_run(vec3::ZERO, 20, 0.02);
    let (moving, u_mov) = cavity_run([0.6, 0.2, 0.0], 20, 0.02);
    let diff = u_fixed
        .iter()
        .zip(&u_mov)
        .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
        .fold(0.0, f64::max);
    let dp = fixed
        .p
        .values
        .iter()
        .zip(&moving.p.values)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    let peak = u_fixed.iter().map(|v| vec3::norm(*v)).fold(0.0, f64::max);
    println!("G3 max |u_moving - V - u_fixed| {diff:.2e} (peak {peak:.3}), pressure diff {dp:.2e}");
    assert!(peak > 0.05, "the start-up flow must be non-trivial");
    assert!(diff <= 1e-9, "{diff}");
}

#[test]
fn g3_steady_couette_profile_is_preserved_on_a_translating_mesh() {
    // u = V + (y', 0, 0), y' the height above the bottom wall: lid V + (1, 0, 0), bottom V, side walls the profile
    let v = [0.6, 0.2, 0.0];
    let mesh = cavity_2d_rect(6, 8, 1.0, 1.0).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let exact = |y_rel: f64| vec3::add(v, [y_rel, 0.0, 0.0]);
    let mut u = VectorField::from_fn("U", &geom, |c| exact(c[1]));
    let profile = |g: &FvGeometry, y0: f64| -> Vec<Vec3> {
        g.mesh
            .patch("walls")
            .unwrap()
            .faces()
            .map(|f| exact(g.mesh.face_centres()[f][1] - y0))
            .collect()
    };
    u.set_bc(
        &geom,
        "walls",
        Box::new(FixedValue(Profile::PerFace(profile(&geom, 0.0)))),
    )
    .unwrap();
    u.set_bc(
        &geom,
        "lid",
        Box::new(FixedValue(Profile::Uniform(vec3::add(v, [1.0, 0.0, 0.0])))),
    )
    .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSolver::new(geom, u, p, settings(0.05));
    let dt = 0.02;
    let mut y0 = 0.0;
    for n in 1..=10 {
        let t = n as f64 * dt;
        let pts: Vec<Vec3> = mesh.points().iter().map(|&q| vec3::add(q, vec3::scale(v, t))).collect();
        let st = move_mesh(&s.geom, pts, dt).unwrap();
        y0 = v[1] * t;
        s.u.set_bc(
            &st.geom,
            "walls",
            Box::new(FixedValue(Profile::PerFace(profile(&st.geom, y0)))),
        )
        .unwrap();
        s.step_ale(dt, st.geom, st.mesh_flux).unwrap();
    }
    let err =
        s.u.values
            .iter()
            .zip(s.geom.mesh.cell_centres())
            .map(|(u, c)| vec3::norm(vec3::sub(*u, exact(c[1] - y0))))
            .fold(0.0, f64::max);
    println!("G3 steady Couette on a translating mesh: max error {err:.2e}");
    assert!(err <= 1e-10, "{err}");
}
