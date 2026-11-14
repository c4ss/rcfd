//! ADR 0431 (F-433): elasticity solves on the CUDA device. Checks D1 (heavy record, `#[ignore]`), D1f, D2 and the
//! negative control N3 of the registration; run with `--features cuda`. Without a usable device each prints "NOT RUN"
//! with the reason (D5) and returns.

use rcfd_api::solid_parallel::{solve_static_device, DeviceOptions, DevicePreconditioner, DeviceReport};
use rcfd_fem::builders::hex_box;
use rcfd_fem::{Constraint, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::LinearDevice;

const TOL: f64 = 1e-10;
const DIFF_LIMIT: f64 = 1e-9;

fn cube(n: usize) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0; 3], [n; 3]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 0.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [2.0e6, 1.0e6, -1.5e6])).unwrap();
    m
}

fn run(model: &SolidModel, d: LinearDevice, p: DevicePreconditioner, tol: f64) -> (SolidResult, DeviceReport) {
    let mut o = DeviceOptions::new(d);
    o.preconditioner = p;
    o.tolerance = tol;
    solve_static_device(model, &[], &[], &o).unwrap()
}

fn rel_diff(a: &SolidResult, b: &SolidResult) -> f64 {
    let (x, y) = (a.displacement_values(), b.displacement_values());
    let m = y.iter().fold(0.0f64, |s, v| s.max(v.abs()));
    x.iter().zip(y).fold(0.0f64, |s, (p, q)| s.max((p - q).abs())) / m
}

fn no_device(tag: &str) -> bool {
    match rcfd_gpu::probe() {
        rcfd_gpu::DeviceStatus::Available(_) => false,
        s => {
            println!("{tag} NOT RUN: {}", s.describe());
            true
        }
    }
}

fn show(tag: &str, r: &SolidResult, rep: &DeviceReport) {
    println!(
        "{tag}: '{}', {} unknowns, backend {} ({}), {} iterations, {} restarts, residual {:.3e}, solve {:.3} s, assembly \
         {:.3} s",
        rep.solver,
        r.stats().unwrap().unknowns(),
        rep.backend,
        rep.reason,
        rep.iterations,
        rep.restarts,
        rep.relative_residual,
        rep.solve_seconds,
        rep.assembly_seconds
    );
}

#[test]
#[ignore = "heavy record (ADR 0431 D1): the 46^3 cube, about a minute or more"]
fn d1_heavy_device_cg_equals_cpu_cg_on_a_cube_of_3e5_unknowns() {
    if no_device("D1") {
        return;
    }
    let model = cube(46);
    let (dev, drep) = run(&model, LinearDevice::Auto, DevicePreconditioner::Jacobi, TOL);
    show("D1 auto", &dev, &drep);
    let (cpu, crep) = run(&model, LinearDevice::Cpu, DevicePreconditioner::Jacobi, TOL);
    show("D1 cpu", &cpu, &crep);
    let d = rel_diff(&dev, &cpu);
    println!(
        "D1: {} unknowns, backend {} (expected cuda), difference {d:.3e} (limit {DIFF_LIMIT:e}), residuals {:.3e} / {:.3e} \
         (limit {TOL:e}), times device {:.3} s cpu {:.3} s",
        dev.stats().unwrap().unknowns(),
        drep.backend,
        drep.relative_residual,
        crep.relative_residual,
        drep.solve_seconds,
        crep.solve_seconds
    );
    assert!(dev.stats().unwrap().unknowns() >= 300_000);
    assert_eq!(drep.backend, "cuda");
    assert!(d <= DIFF_LIMIT && drep.relative_residual <= TOL && crep.relative_residual <= TOL);
}

#[test]
fn d1f_device_cg_equals_cpu_cg_on_the_fast_cube() {
    if no_device("D1f") {
        return;
    }
    let model = cube(12);
    let mut ok = true;
    for p in [
        DevicePreconditioner::Jacobi,
        DevicePreconditioner::Chebyshev { degree: 0 },
    ] {
        let (dev, drep) = run(&model, LinearDevice::Cuda, p, TOL);
        show("D1f cuda", &dev, &drep);
        let (cpu, crep) = run(&model, LinearDevice::Cpu, p, TOL);
        show("D1f cpu", &cpu, &crep);
        let d = rel_diff(&dev, &cpu);
        println!("D1f {p:?}: difference {d:.3e} (limit {DIFF_LIMIT:e})");
        ok &=
            drep.backend == "cuda" && d <= DIFF_LIMIT && drep.relative_residual <= TOL && crep.relative_residual <= TOL;
    }
    assert!(ok);
}

#[test]
fn d2_auto_below_the_threshold_runs_on_the_cpu() {
    if no_device("D2") {
        return;
    }
    let model = cube(12);
    let (auto, arep) = run(&model, LinearDevice::Auto, DevicePreconditioner::Jacobi, TOL);
    let (cpu, _) = run(&model, LinearDevice::Cpu, DevicePreconditioner::Jacobi, TOL);
    let same = auto
        .displacement_values()
        .iter()
        .map(|v| v.to_bits())
        .collect::<Vec<_>>()
        == cpu
            .displacement_values()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>();
    println!(
        "D2: {} unknowns, auto: backend {}, reason '{}', bit-equal to cpu {same}",
        auto.stats().unwrap().unknowns(),
        arep.backend,
        arep.reason
    );
    assert!(arep.backend == "cpu" && arep.reason.contains("below the threshold") && same);
}

#[test]
fn n3_a_loose_cpu_solve_is_detected_by_the_device_comparison() {
    let model = cube(12);
    let reference = if no_device("N3 (device reference; cpu reference used)") {
        LinearDevice::Cpu
    } else {
        LinearDevice::Cuda
    };
    let (r, _) = run(&model, reference, DevicePreconditioner::Jacobi, TOL);
    let (loose, _) = run(&model, LinearDevice::Cpu, DevicePreconditioner::Jacobi, 1e-4);
    let d = rel_diff(&loose, &r);
    println!(
        "N3: cpu at 1e-4 against {} at 1e-10: {d:.3e} (expected > 1e-9)",
        reference.as_str()
    );
    assert!(d > DIFF_LIMIT);
}
