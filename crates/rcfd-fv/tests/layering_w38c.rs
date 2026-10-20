//! F-230 checks of ADR 0090 (wave 38C): piston in a closed cylinder with layer removal and addition (A1, A2, B1-B5), the
//! discrete GCL of the compressible ALE step (C1, C2) and the inverse-volume smoothing stiffness (S1-S3). Limits are the
//! registered ones.

use rcfd_fv::compressible::{CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::layering::{remap, LayerEvent, LayerStack};
use std::f64::consts::PI;
use std::sync::Arc;

const T: f64 = 16.0;

fn geom(s: &LayerStack) -> FvGeometry {
    FvGeometry::new(Arc::new(s.mesh().unwrap())).unwrap()
}

fn rest() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: 1.0,
    }
}

/// `[sum V, sum rho V, sum rho E V]` of the solver state.
fn totals(s: &CompressibleSolver) -> [f64; 3] {
    let t = s.totals();
    [s.geom.mesh.cell_volumes().iter().sum(), t[0], t[4]]
}

/// Volume-averaged pressure and total volume.
fn mean_pressure(s: &CompressibleSolver) -> (f64, f64) {
    let v = s.geom.mesh.cell_volumes();
    let vol: f64 = v.iter().sum();
    let pv: f64 = s.primitives().iter().zip(v).map(|(p, v)| p.p * v).sum();
    (pv / vol, vol)
}

fn piston_x(t: f64) -> f64 {
    1.0 - 0.3 * (1.0 - (PI * t / T).cos())
}

#[test]
fn piston_layering_isentrope_and_conservation() {
    let clock = std::time::Instant::now();
    let gamma = Gas::AIR.gamma;
    let mut stack = LayerStack::uniform(40, 1.0, [2, 2], [0.1, 0.1], 0.4, 1.6).unwrap();
    let mut s = CompressibleSolver::new(geom(&stack), Gas::AIR, CompSettings::default(), |_| rest()).unwrap();
    let (p0, v0) = mean_pressure(&s);
    let inv0 = p0 * v0.powf(gamma);
    let m0 = s.totals()[0];
    let (mut t, mut steps) = (0.0, 0usize);
    let (mut removed, mut added) = (0usize, 0usize);
    let (mut b1, mut b2, mut b3, mut diag) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut b5_violations = 0usize;
    let mut prev_event: Option<LayerEvent> = None;
    let mut a = [f64::NAN; 2];
    for (stroke, target) in [T, 2.0 * T].into_iter().enumerate() {
        while t < target {
            let mut dt = s.max_dt();
            let last = t + dt >= target - 1e-9;
            if last {
                dt = target - t;
            }
            let t_new = if last { target } else { t + dt };
            stack.set_piston(piston_x(t_new)).unwrap();
            s.step_ale(geom(&stack), dt).unwrap();
            t = t_new;
            steps += 1;
            b2 = b2.max((s.totals()[0] / m0 - 1.0).abs());
            let vol: f64 = s.geom.mesh.cell_volumes().iter().sum();
            b3 = b3.max((vol / (0.01 * piston_x(t)) - 1.0).abs());
            let (pm, vm) = mean_pressure(&s);
            diag = diag.max((pm * vm.powf(gamma) / inv0 - 1.0).abs());
            let event = match stack.adjust() {
                Some(ch) => {
                    let before = totals(&s);
                    let g = geom(&stack);
                    let u = remap(&ch, &s.u, s.geom.mesh.cell_volumes(), g.mesh.cell_volumes());
                    s.replace_mesh(g, u).unwrap();
                    let after = totals(&s);
                    for k in 0..3 {
                        b1 = b1.max((after[k] / before[k] - 1.0).abs());
                    }
                    match ch.event {
                        LayerEvent::Removed => removed += 1,
                        LayerEvent::Added => added += 1,
                    }
                    Some(ch.event)
                }
                None => None,
            };
            if let (Some(e), Some(p)) = (event, prev_event) {
                if e != p {
                    b5_violations += 1;
                }
            }
            prev_event = event;
        }
        let (pm, vm) = mean_pressure(&s);
        a[stroke] = pm * vm.powf(gamma) / inv0 - 1.0;
        if stroke == 0 {
            let q: Vec<f64> = s.primitives().iter().map(|p| p.p / p.rho.powf(gamma)).collect();
            let (lo, hi) = q
                .iter()
                .fold((f64::INFINITY, 0.0f64), |(l, h), &x| (l.min(x), h.max(x)));
            println!(
                "t = T: V/V0 = {:.6}, layers {}, p_bar = {pm:.6} (isentrope {:.6}), p/rho^gamma spread [{lo:.6}, {hi:.6}]",
                vm / v0,
                stack.n_layers(),
                inv0 / vm.powf(gamma)
            );
        }
    }
    println!(
        "A1 p_bar V^gamma / (p0 V0^gamma) - 1 at t = T: {:.3e} (limit 2e-2)",
        a[0]
    );
    println!("A2 the same at t = 2T: {:.3e} (limit 2e-2)", a[1]);
    println!("diagnostic: max over all steps {diag:.3e}");
    println!("B1 max relative change of (V, mass, energy) over a layer event: {b1:.2e} (limit 1e-13)");
    println!("B2 max relative mass drift: {b2:.2e} (limit 1e-12)");
    println!("B3 max relative volume error vs A x_p: {b3:.2e} (limit 1e-13)");
    println!("B4 removals {removed}, additions {added} (each >= 5)");
    println!("B5 opposite events in consecutive steps: {b5_violations}");
    println!(
        "steps {steps}, final layers {}, wall time {:.1} s",
        stack.n_layers(),
        clock.elapsed().as_secs_f64()
    );
    assert!(a[0].abs() <= 0.02 && a[1].abs() <= 0.02, "A1/A2 {a:?}");
    assert!(b1 <= 1e-13, "B1 {b1}");
    assert!(b2 <= 1e-12, "B2 {b2}");
    assert!(b3 <= 1e-13, "B3 {b3}");
    assert!(removed >= 5 && added >= 5, "B4 {removed} {added}");
    assert_eq!(b5_violations, 0, "B5");
}

#[test]
fn c1_uniform_flow_on_a_moving_mesh_stays_uniform() {
    let base = box_mesh([20, 2, 2], [1.0, 0.1, 0.1], [Grading::Uniform; 3]).unwrap();
    let moved = |t: f64| {
        let m = warp(&base, |p| {
            [p[0] + 0.01 * (2.0 * PI * p[0]).sin() * (2.0 * PI * t).sin(), p[1], p[2]]
        })
        .unwrap();
        FvGeometry::new(Arc::new(m)).unwrap()
    };
    let st = Prim {
        rho: 1.0,
        u: [0.3, 0.0, 0.0],
        p: 1.0,
    };
    let mut s = CompressibleSolver::new(moved(0.0), Gas::AIR, CompSettings::default(), |_| st).unwrap();
    s.set_bc("xmin", CompBc::Fixed(st)).unwrap();
    s.set_bc("xmax", CompBc::Fixed(st)).unwrap();
    let u0 = st.to_cons(Gas::AIR);
    let mut t = 0.0;
    for _ in 0..200 {
        let dt = s.max_dt();
        t += dt;
        s.step_ale(moved(t), dt).unwrap();
    }
    let err =
        s.u.iter()
            .flat_map(|c| (0..5).map(move |k| (c[k] - u0[k]).abs() / u0.iter().fold(0.0f64, |a, b| a.max(b.abs()))))
            .fold(0.0, f64::max);
    println!("C1 max |U - U0| / max|U0| after 200 moving steps (t = {t:.3}): {err:.2e} (limit 1e-12)");
    assert!(err <= 1e-12, "C1 {err}");
}

#[test]
fn c2_zero_motion_ale_step_matches_the_fixed_mesh_step() {
    let stack = LayerStack::uniform(40, 1.0, [2, 2], [0.1, 0.1], 0.4, 1.6).unwrap();
    let sod = |x: [f64; 3]| {
        if x[0] < 0.5 {
            rest()
        } else {
            Prim {
                rho: 0.125,
                u: [0.0; 3],
                p: 0.1,
            }
        }
    };
    let mut a = CompressibleSolver::new(geom(&stack), Gas::AIR, CompSettings::default(), sod).unwrap();
    let mut b = a.clone();
    let dt = a.max_dt();
    a.step(dt).unwrap();
    b.step_ale(geom(&stack), dt).unwrap();
    let scale = a.u.iter().flat_map(|c| c.iter()).fold(0.0f64, |m, x| m.max(x.abs()));
    let err =
        a.u.iter()
            .zip(&b.u)
            .flat_map(|(x, y)| (0..5).map(move |k| (x[k] - y[k]).abs()))
            .fold(0.0, f64::max)
            / scale;
    println!("C2 zero-motion step_ale vs step: {err:.2e} (limit 1e-13)");
    assert!(err <= 1e-13, "C2 {err}");
}

mod stiffness {
    use rcfd_core::vec3::{self, Vec3};
    use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
    use rcfd_fv::FvGeometry;
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::{box_mesh, Grading};
    use rcfd_mesh::motion::{moved_mesh, patch_points};
    use rcfd_mesh::Mesh;
    use std::sync::Arc;

    const TIGHT: SolveControl = SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-14,
        max_iter: 20000,
    };

    fn prescribe(mesh: &Mesh, patches: &[&str], d: Vec3) -> Vec<Vec3> {
        let mut out = vec![vec3::ZERO; mesh.n_points()];
        for name in patches {
            for q in patch_points(mesh, name).unwrap() {
                out[q] = d;
            }
        }
        out
    }

    fn solve(mesh: &Mesh, cls: &[PatchClass], presc: &[Vec3], m: SmoothingMethod) -> Vec<Vec3> {
        let g = FvGeometry::new(Arc::new(mesh.clone())).unwrap();
        smooth_displacement(&g, cls, presc, m, &TIGHT).unwrap().displacement
    }

    #[test]
    fn s1_inverse_volume_translation_is_exact() {
        let mesh = box_mesh(
            [5, 4, 3],
            [1.0, 0.8, 0.6],
            [Grading::Expansion(2.0), Grading::Uniform, Grading::Uniform],
        )
        .unwrap();
        let d0 = [0.13, -0.07, 0.21];
        let all = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
        let cls = vec![PatchClass::Prescribed; mesh.patches().len()];
        let d = solve(
            &mesh,
            &cls,
            &prescribe(&mesh, &all, d0),
            SmoothingMethod::InverseVolume(1.0),
        );
        let err = d.iter().map(|x| vec3::norm(vec3::sub(*x, d0))).fold(0.0, f64::max);
        println!("S1 inverse volume k = 1: max |d - d0| = {err:.2e} (limit 1e-12)");
        assert!(err <= 1e-12, "S1 {err}");
    }

    #[test]
    fn s2_s3_inverse_volume_protects_the_small_cells() {
        let n = 20;
        let mesh = box_mesh(
            [n, 2, 1],
            [1.0, 0.1, 0.1],
            [Grading::Expansion(10.0), Grading::Uniform, Grading::Uniform],
        )
        .unwrap();
        let presc = prescribe(&mesh, &["xmin"], [0.1, 0.0, 0.0]);
        let cls: Vec<PatchClass> = mesh
            .patches()
            .iter()
            .map(|p| match p.name.as_str() {
                "xmin" => PatchClass::Prescribed,
                "xmax" => PatchClass::Fixed,
                _ => PatchClass::Slip,
            })
            .collect();
        // strain (relative volume change) of the first (smallest) and last (largest) layer
        let strains = |m: SmoothingMethod| -> [f64; 2] {
            let d = solve(&mesh, &cls, &presc, m);
            let pts: Vec<Vec3> = mesh.points().iter().zip(&d).map(|(p, d)| vec3::add(*p, *d)).collect();
            let moved = moved_mesh(&mesh, pts).unwrap();
            [0, n - 1].map(|c| (moved.cell_volumes()[c] / mesh.cell_volumes()[c] - 1.0).abs())
        };
        let lap = strains(SmoothingMethod::Laplacian);
        let inv = strains(SmoothingMethod::InverseVolume(1.0));
        // cell cross-section 0.05 x 0.1 (two cells across y)
        let h: Vec<f64> = (0..n).map(|c| mesh.cell_volumes()[c] / 0.005).collect();
        let continuous = h[0] * 1.0 / h.iter().map(|x| x * x).sum::<f64>();
        let ratio = inv[0] / lap[0];
        println!(
            "S2 smallest-cell strain: laplacian {:.4e}, inverse volume {:.4e}, ratio {ratio:.4} (limit < 1; continuous 1-D {continuous:.4})",
            lap[0], inv[0]
        );
        println!(
            "    largest-cell strain: laplacian {:.4e}, inverse volume {:.4e}",
            lap[1], inv[1]
        );
        let s3 = (lap[0] / lap[1] - 1.0).abs();
        println!("S3 laplacian first/last layer strain relative difference {s3:.2e} (limit 1e-9)");
        assert!(ratio < 1.0, "S2 {ratio}");
        assert!(s3 <= 1e-9, "S3 {s3}");
    }
}
