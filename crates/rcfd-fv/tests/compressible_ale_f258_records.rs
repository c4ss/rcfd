//! F-258 (ADR 0207) records R1 to R4: the bits of four compressible runs that F-258 must not change, recorded on main
//! d953cd6 before any code of F-258 (Windows; printed elsewhere, ADR 0137). R1 to R3 move the mesh with `step_ale`
//! (layering, smoothing with a static pressure outlet, a rigid motion with fixed states); R4 is the fixed-mesh `step`
//! with the four characteristic conditions. Every step is `max_dt()`, the time step these runs had before F-258.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::compressible::{CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::compressible_bc::TotalInlet;
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::FvGeometry;
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use rcfd_mesh::layering::{remap, LayerStack};
use rcfd_mesh::motion::{mat_vec, moved_mesh, patch_points, rotation_matrix};
use std::f64::consts::PI;
use std::sync::Arc;

const AIR: Gas = Gas::AIR;

/// FNV-1a 64 of the bits of every conserved value, the time, the step count and the cell count.
fn state_hash(s: &CompressibleSolver) -> String {
    let mut bytes = Vec::new();
    for c in &s.u {
        for x in c {
            bytes.extend_from_slice(&x.to_bits().to_le_bytes());
        }
    }
    bytes.extend_from_slice(&s.time.to_bits().to_le_bytes());
    bytes.extend_from_slice(&(s.steps as u64).to_le_bytes());
    bytes.extend_from_slice(&(s.u.len() as u64).to_le_bytes());
    format!("{:016x}", rcfd_core::hash::fnv64(&bytes))
}

fn check(name: &str, s: &CompressibleSolver, want: &str) {
    let got = state_hash(s);
    println!("{name}: {got} ({} steps, t = {}, {} cells)", s.steps, s.time, s.u.len());
    if cfg!(windows) {
        assert_eq!(got, want, "{name}");
    }
}

fn rest() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: 1.0,
    }
}

fn geom(m: rcfd_mesh::Mesh) -> FvGeometry {
    FvGeometry::new(Arc::new(m)).unwrap()
}

/// The end test of ADR 0191's library loops.
fn ended(time: f64, end: f64) -> bool {
    time >= end - 1e-12 * end.max(1.0)
}

/// The step of ADR 0191's library loops: `max_dt()` clipped to the end time.
fn step_to(s: &CompressibleSolver, end: f64) -> f64 {
    s.max_dt().min((end - s.time).max(1e-300))
}

/// R1: ADR 0090's piston P (40 x 2 x 2 layers, slip walls, period 16) to t = 4 with its layer events, on the schedule of
/// ADR 0090's loop (the step landed on the end time).
#[test]
fn r1_layered_piston_record() {
    let x_p = |t: f64| 1.0 - 0.3 * (1.0 - (PI * t / 16.0).cos());
    let mut stack = LayerStack::uniform(40, 1.0, [2, 2], [0.1, 0.1], 0.4, 1.6).unwrap();
    let mut s = CompressibleSolver::new(geom(stack.mesh().unwrap()), AIR, CompSettings::default(), |_| rest()).unwrap();
    let (mut t, mut events) = (0.0, 0);
    while t < 4.0 {
        let mut dt = s.max_dt();
        let last = t + dt >= 4.0 - 1e-9;
        if last {
            dt = 4.0 - t;
        }
        let t_new = if last { 4.0 } else { t + dt };
        stack.set_piston(x_p(t_new)).unwrap();
        s.step_ale(geom(stack.mesh().unwrap()), dt).unwrap();
        t = t_new;
        if let Some(ch) = stack.adjust() {
            let g = geom(stack.mesh().unwrap());
            let u = remap(&ch, &s.u, s.geom.mesh.cell_volumes(), g.mesh.cell_volumes());
            s.replace_mesh(g, u).unwrap();
            events += 1;
        }
    }
    println!("R1 layer events {events}");
    check("R1", &s, "6c63454d45c1e84b");
}

/// R2: ADR 0191's P-B as a library loop: smoothing (xmax prescribed, xmin fixed) with a static pressure outlet at xmin.
#[test]
fn r2_smoothing_with_a_static_outlet_record() {
    let mesh0 = box_2d(20, 2, 1.0, 0.1, false, false).unwrap();
    let geom0 = geom(mesh0.clone());
    let classes: Vec<PatchClass> = mesh0
        .patches()
        .iter()
        .map(|p| match p.name.as_str() {
            "xmax" => PatchClass::Prescribed,
            "xmin" => PatchClass::Fixed,
            _ => PatchClass::Slip,
        })
        .collect();
    let control = SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-12,
        max_iter: 5000,
    };
    let mut s = CompressibleSolver::new(geom0.clone(), AIR, CompSettings::default(), |_| rest()).unwrap();
    s.set_bc("xmin", CompBc::PressureOutlet { p: 1.0 }).unwrap();
    let p0 = mesh0.points();
    let piston = patch_points(&mesh0, "xmax").unwrap();
    while !ended(s.time, 1.0) {
        let dt = step_to(&s, 1.0);
        let t_new = s.time + dt;
        let mut presc = vec![vec3::ZERO; p0.len()];
        for &q in &piston {
            presc[q] = [-0.2 * (1.0 - (PI * t_new / 2.0).cos()), 0.0, 0.0];
        }
        let sm = smooth_displacement(&geom0, &classes, &presc, SmoothingMethod::Laplacian, &control).unwrap();
        let points: Vec<Vec3> = p0.iter().zip(&sm.displacement).map(|(p, d)| vec3::add(*p, *d)).collect();
        s.step_ale(geom(moved_mesh(&mesh0, points).unwrap()), dt).unwrap();
    }
    check("R2", &s, "56d082d8b41c8da9");
}

/// R3: a rigid rotation and translation of a smooth isentropic density pulse in a uniform stream, fixed states on every
/// patch, to t = 0.5.
#[test]
fn r3_rigid_motion_with_a_pulse_record() {
    let mesh0 = box_2d(20, 4, 1.0, 0.2, false, false).unwrap();
    let u0 = [0.3, 0.1, 0.0];
    let free = Prim { rho: 1.0, u: u0, p: 1.0 };
    let init = |x: Vec3| {
        let r2 = (x[0] - 0.5).powi(2) + (x[1] - 0.1).powi(2);
        let rho = 1.0 + 0.2 * (-r2 / 0.01).exp();
        Prim {
            rho,
            u: u0,
            p: rho.powf(1.4),
        }
    };
    let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), init).unwrap();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        s.set_bc(p, CompBc::Fixed(free)).unwrap();
    }
    let origin = [0.5, 0.1, 0.0];
    let pose = |p: Vec3, t: f64| {
        let r = rotation_matrix([0.0, 0.0, 1.0], 0.1 * (2.0 * PI * t).sin());
        let d = [0.05 * (2.0 * PI * t).sin(), 0.02 * (4.0 * PI * t).sin(), 0.0];
        vec3::add(vec3::add(origin, mat_vec(&r, vec3::sub(p, origin))), d)
    };
    while !ended(s.time, 0.5) {
        let dt = step_to(&s, 0.5);
        let t_new = s.time + dt;
        let points: Vec<Vec3> = mesh0.points().iter().map(|&p| pose(p, t_new)).collect();
        s.step_ale(geom(moved_mesh(&mesh0, points).unwrap()), dt).unwrap();
    }
    check("R3", &s, "6e414e610c2290bd");
}

/// R4: the fixed-mesh `step` with the four characteristic conditions (total inlet, pressure outlet, far field,
/// non-reflecting outlet) on a stream with a pressure pulse, 150 steps.
#[test]
fn r4_fixed_mesh_characteristic_conditions_record() {
    let mesh = box_2d(30, 10, 1.5, 0.5, false, false).unwrap();
    let u0 = [0.4, 0.05, 0.0];
    let init = |x: Vec3| {
        let r2 = (x[0] - 0.5).powi(2) + (x[1] - 0.25).powi(2);
        Prim {
            rho: 1.0,
            u: u0,
            p: 1.0 + 0.1 * (-r2 / 0.02).exp(),
        }
    };
    let mut s = CompressibleSolver::new(geom(mesh), AIR, CompSettings::default(), init).unwrap();
    // the stagnation state of rho = 1, p = 1, |u| = |u0| with r_gas = 1 (T = 1)
    let m2 = vec3::dot(u0, u0) / 1.4;
    let t0 = 1.0 + 0.2 * m2;
    s.set_bc(
        "xmin",
        CompBc::TotalInlet(TotalInlet {
            p0: t0.powf(3.5),
            t0,
            r_gas: 1.0,
            dir: vec3::unit(u0),
        }),
    )
    .unwrap();
    s.set_bc("xmax", CompBc::PressureOutlet { p: 1.0 }).unwrap();
    s.set_bc("ymin", CompBc::FarField(Prim { rho: 1.0, u: u0, p: 1.0 })).unwrap();
    s.set_bc("ymax", CompBc::NonReflecting { p: 1.0, sigma: 0.3 }).unwrap();
    for _ in 0..150 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
    }
    check("R4", &s, "3912775885144ca1");
}
