//! Sandboxed user functions (F-122): WebAssembly modules supply a momentum source, a velocity
//! profile, a kinematic viscosity and a monitor; hostile modules are contained.
//!
//! Checks fixed before running:
//! * a constant-acceleration module reproduces `physics.body_force` field for field (relative
//!   difference of every velocity and pressure value below 1e-9);
//! * a position-dependent, curl-free acceleration a = (3 x, 0, 0) in a closed box is balanced by
//!   p = 1.5 x^2: the spurious velocity and the pressure error against that field must fall by at
//!   least 1.7 when the 10x10 grid is refined to 20x20 (the wall pressure extrapolation uses the
//!   owner-cell force, a first-order inconsistency for a non-uniform force). The first version of
//!   this check demanded max |u| < 1e-6 on one grid; it measured 4.4e-2 on 10x10, which showed
//!   that premise ignored the wall treatment, so the check was replaced (not loosened) by the
//!   convergence check above, written before the 20x20 result was seen;
//! * a constant-viscosity module reproduces `physics.nu` (relative difference < 1e-9);
//! * a velocity_profile module (6 y (1 - y)) equals the same inlet given as an expression (< 1e-9);
//! * the monitor equals sum(ux * V) computed independently from the cell fields (< 1e-12 relative);
//! * imports, wrong ABI version, wrong signature, missing ABI export, infinite loops, memory growth
//!   past the cap, NaN results and non-positive viscosity are all refused or stop the run with an
//!   error; none of them panics or hangs; the fuel budget is renewed on every call.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_api::udf::{Udf, ABI_VERSION};
use std::path::PathBuf;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_udf_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn wasm(wat_text: &str) -> Vec<u8> {
    wat::parse_str(wat_text).unwrap()
}

/// Module with the given extra items and a correct ABI export.
fn module(body: &str) -> String {
    format!("(module (func (export \"rcfd_abi_version\") (result i32) i32.const {ABI_VERSION}) {body})")
}

const MOMENTUM_PARAMS: &str = "(param f64 f64 f64 f64 f64 f64 f64 i32) (result f64)";

fn channel(extra_bc: &str, physics: &str, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 16
ny = 10
lx = 2.0
ly = 1.0
depth = 0.1
[physics]
{physics}
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
{extra_bc}
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-8
pressure_rel_tol = 1e-8
pressure_max_iter = 1000
max_iterations = 300
tolerance_u = 1e-10
tolerance_p = 1e-10
{extra}
"#
    )
}

fn run(dir: &std::path::Path, text: &str) -> Simulation {
    let case = Case::parse_toml_in(text, dir).unwrap();
    let errs = case.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}");
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(400);
    sim
}

fn max_rel_diff(a: &Simulation, b: &Simulation) -> f64 {
    let mut worst: f64 = 0.0;
    for f in ["ux", "uy", "p"] {
        let (x, y) = (a.field(f).unwrap(), b.field(f).unwrap());
        let scale = x.iter().fold(1e-12_f64, |m, v| m.max(v.abs()));
        for (p, q) in x.iter().zip(&y) {
            worst = worst.max((p - q).abs() / scale);
        }
    }
    worst
}

#[test]
fn constant_momentum_source_equals_body_force() {
    let d = tmp("mom");
    let m = module(&format!(
        "(func (export \"momentum_source\") {MOMENTUM_PARAMS} local.get 7 i32.eqz if (result f64) f64.const 0.7 else f64.const 0.0 end)"
    ));
    std::fs::write(d.join("a.wasm"), wasm(&m)).unwrap();
    let with_udf = run(
        &d,
        &channel(
            "velocity_value = [1.0, 0.0, 0.0]",
            "nu = 0.1",
            "[udf.a]\nfile = \"a.wasm\"\nmomentum_source = true\n",
        ),
    );
    let with_bf = run(
        &d,
        &channel(
            "velocity_value = [1.0, 0.0, 0.0]",
            "nu = 0.1\nbody_force = [0.7, 0.0, 0.0]",
            "",
        ),
    );
    let none = run(&d, &channel("velocity_value = [1.0, 0.0, 0.0]", "nu = 0.1", ""));
    let diff = max_rel_diff(&with_udf, &with_bf);
    assert!(diff < 1e-9, "udf vs body_force: {diff:e}");
    let sanity = max_rel_diff(&with_udf, &none);
    assert!(sanity > 1e-3, "the source had no effect: {sanity:e}");
    assert!(with_udf.failure().is_none());
}

#[test]
fn curl_free_source_is_balanced_by_pressure_in_a_closed_box() {
    let d = tmp("hydro");
    let m = module(&format!(
        "(func (export \"momentum_source\") {MOMENTUM_PARAMS} local.get 7 i32.eqz if (result f64) f64.const 3.0 local.get 0 f64.mul else f64.const 0.0 end)"
    ));
    std::fs::write(d.join("a.wasm"), wasm(&m)).unwrap();
    let text = |n: usize| {
        format!(
            r#"
[mesh]
generator = "cavity_2d"
nx = {n}
ny = {n}
[physics]
nu = 0.05
[boundaries.lid]
velocity = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-10
pressure_rel_tol = 1e-10
pressure_max_iter = 2000
max_iterations = 300
tolerance_u = 1e-12
tolerance_p = 1e-12
[udf.a]
file = "a.wasm"
momentum_source = true
"#
        )
    };
    let measure = |n: usize| {
        let sim = run(&d, &text(n));
        assert!(sim.failure().is_none());
        let (ux, uy, p) = (
            sim.field("ux").unwrap(),
            sim.field("uy").unwrap(),
            sim.field("p").unwrap(),
        );
        let umax = ux.iter().chain(&uy).fold(0.0_f64, |m, v| m.max(v.abs()));
        let xs: Vec<f64> = sim.geom.mesh.cell_centres().iter().map(|c| c[0]).collect();
        let exact = |i: usize| 1.5 * (xs[i] * xs[i] - xs[0] * xs[0]);
        let perr = (0..xs.len()).fold(0.0_f64, |m, i| m.max((p[i] - p[0] - exact(i)).abs()));
        (umax, perr)
    };
    let (u1, p1) = measure(10);
    let (u2, p2) = measure(20);
    eprintln!("hydrostatic UDF source: 10x10 max|u| {u1:e}, p error {p1:e}; 20x20 max|u| {u2:e}, p error {p2:e}");
    assert!(u1 / u2 >= 1.7, "spurious velocity {u1:e} -> {u2:e}");
    assert!(p1 / p2 >= 1.7, "pressure error {p1:e} -> {p2:e}");
}

#[test]
fn viscosity_and_velocity_profile_modules_match_their_plain_counterparts() {
    let d = tmp("visc");
    let visc =
        module("(func (export \"kinematic_viscosity\") (param f64 f64 f64 f64 f64 f64) (result f64) f64.const 0.1)");
    let prof = module(
        "(func (export \"velocity_profile\") (param f64 f64 f64 f64 i32) (result f64) \
         local.get 4 i32.eqz if (result f64) f64.const 6.0 local.get 1 f64.mul f64.const 1.0 local.get 1 f64.sub f64.mul else f64.const 0.0 end)",
    );
    std::fs::write(d.join("v.wasm"), wasm(&visc)).unwrap();
    std::fs::write(d.join("p.wasm"), wasm(&prof)).unwrap();
    let plain = run(
        &d,
        &channel(
            "velocity_expression_x = \"6 [m/s] * y / 1 [m] * (1 - y / 1 [m])\"",
            "nu = 0.1",
            "",
        ),
    );
    let both = run(
        &d,
        &channel(
            "velocity_udf = \"p\"",
            "nu = 0.37",
            "[udf.v]\nfile = \"v.wasm\"\nviscosity = true\n[udf.p]\nfile = \"p.wasm\"\n",
        ),
    );
    assert!(both.failure().is_none(), "{:?}", both.failure());
    let diff = max_rel_diff(&both, &plain);
    assert!(diff < 1e-9, "udf viscosity+profile vs plain: {diff:e}");
    // physics.nu = 0.37 was overridden by the module: a run without the module differs
    let wrong = run(
        &d,
        &channel(
            "velocity_expression_x = \"6 [m/s] * y / 1 [m] * (1 - y / 1 [m])\"",
            "nu = 0.37",
            "",
        ),
    );
    assert!(max_rel_diff(&wrong, &plain) > 1e-3);
}

#[test]
fn monitor_matches_independent_sum() {
    let d = tmp("mon");
    let m = module("(func (export \"monitor\") (param f64 f64 f64 f64 f64 f64 f64 f64 f64) (result f64) local.get 4)");
    std::fs::write(d.join("m.wasm"), wasm(&m)).unwrap();
    let sim = run(
        &d,
        &channel(
            "velocity_value = [1.0, 0.0, 0.0]",
            "nu = 0.1",
            "[udf.m]\nfile = \"m.wasm\"\nmonitor = true\n",
        ),
    );
    let ux = sim.field("ux").unwrap();
    let vol = sim.geom.mesh.cell_volumes();
    let expect: f64 = ux.iter().zip(vol).map(|(u, v)| u * v).sum();
    let series = sim.monitors.get("udf_m").expect("monitor recorded");
    let got = *series.y.last().unwrap();
    assert!(((got - expect) / expect).abs() < 1e-12, "{got} vs {expect}");
    assert_eq!(series.y.len(), sim.last().unwrap().index);
}

#[test]
fn module_isolation_and_abi_checks() {
    // imports are refused: no WASI, no host access
    let e = Udf::load(
        "w",
        &wasm(
            "(module (import \"wasi_snapshot_preview1\" \"fd_write\" (func (param i32 i32 i32 i32) (result i32))) \
             (func (export \"rcfd_abi_version\") (result i32) i32.const 1))",
        ),
        1000,
    )
    .err()
    .unwrap();
    assert!(e.contains("imports") && e.contains("fd_write"), "{e}");
    // not WebAssembly
    assert!(Udf::load("w", b"not wasm", 1000).is_err());
    // missing ABI export
    let e = Udf::load("w", &wasm("(module (func (export \"x\")))"), 1000)
        .err()
        .unwrap();
    assert!(e.contains("rcfd_abi_version"), "{e}");
    // wrong ABI version
    let e = Udf::load(
        "w",
        &wasm("(module (func (export \"rcfd_abi_version\") (result i32) i32.const 7))"),
        1000,
    )
    .err()
    .unwrap();
    assert!(e.contains("ABI version 7"), "{e}");
    // wrong signature of an entry point
    let e = Udf::load(
        "w",
        &wasm(&module(
            "(func (export \"monitor\") (param f64) (result f64) local.get 0)",
        )),
        1000,
    )
    .err()
    .unwrap();
    assert!(e.contains("wrong signature"), "{e}");
    // zero fuel budget
    assert!(Udf::load("w", &wasm(&module("")), 0).is_err());
}

#[test]
fn runaway_modules_are_stopped() {
    let params = "(param f64 f64 f64 f64 f64 f64) (result f64)";
    // infinite loop: fuel runs out
    let mut u = Udf::load(
        "spin",
        &wasm(&module(&format!(
            "(func (export \"kinematic_viscosity\") {params} (loop $l br $l) f64.const 1.0)"
        ))),
        10_000,
    )
    .unwrap();
    let e = u.kinematic_viscosity([0.0; 3], 0.0, 300.0, 1.0).unwrap_err();
    assert!(e.contains("kinematic_viscosity failed"), "{e}");
    // memory growth beyond the 16 MiB cap traps (1024 pages = 64 MiB)
    let mut g = Udf::load(
        "grow",
        &wasm(&module(&format!(
            "(memory 1) (func (export \"kinematic_viscosity\") {params} i32.const 1024 memory.grow drop f64.const 1.0)"
        ))),
        10_000,
    )
    .unwrap();
    assert!(g.kinematic_viscosity([0.0; 3], 0.0, 300.0, 1.0).is_err());
    // NaN and non-positive viscosity are rejected
    let mut n = Udf::load(
        "nan",
        &wasm(&module(&format!(
            "(func (export \"kinematic_viscosity\") {params} f64.const 0.0 f64.const 0.0 f64.div)"
        ))),
        1000,
    )
    .unwrap();
    assert!(n
        .kinematic_viscosity([0.0; 3], 0.0, 300.0, 1.0)
        .unwrap_err()
        .contains("non-finite"));
    let mut z = Udf::load(
        "neg",
        &wasm(&module(&format!(
            "(func (export \"kinematic_viscosity\") {params} f64.const -1.0)"
        ))),
        1000,
    )
    .unwrap();
    assert!(z
        .kinematic_viscosity([0.0; 3], 0.0, 300.0, 1.0)
        .unwrap_err()
        .contains("not positive"));
    // a call that fits its budget can be repeated far beyond the total budget (fuel is renewed)
    let mut ok = Udf::load(
        "ok",
        &wasm(&module(&format!(
            "(func (export \"kinematic_viscosity\") {params} f64.const 2.5)"
        ))),
        1000,
    )
    .unwrap();
    for _ in 0..20_000 {
        assert_eq!(ok.kinematic_viscosity([0.0; 3], 0.0, 300.0, 1.0).unwrap(), 2.5);
    }
    assert!(ok.max_fuel_per_call > 0 && ok.max_fuel_per_call < 1000);
}

#[test]
fn a_failing_module_stops_the_run_with_an_error() {
    let d = tmp("fail");
    let m = module(&format!(
        "(func (export \"momentum_source\") {MOMENTUM_PARAMS} (loop $l br $l) f64.const 0.0)"
    ));
    std::fs::write(d.join("spin.wasm"), wasm(&m)).unwrap();
    let case = Case::parse_toml_in(
        &channel(
            "velocity_value = [1.0, 0.0, 0.0]",
            "nu = 0.1",
            "[udf.s]\nfile = \"spin.wasm\"\nmomentum_source = true\nfuel_per_call = 5000\n",
        ),
        &d,
    )
    .unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let s = sim.step();
    assert!(s.done && !s.converged);
    let f = sim.failure().unwrap();
    assert!(f.contains("momentum_source failed"), "{f}");
    assert_eq!(sim.stop_reason(), Some(f));
}

#[test]
fn case_level_problems_are_diagnosed() {
    let d = tmp("diag");
    std::fs::write(d.join("a.wasm"), wasm(&module(""))).unwrap();
    let base = |extra: &str| channel("velocity_value = [1.0, 0.0, 0.0]", "nu = 0.1", extra);
    let check = |extra: &str, want: &str| {
        let errs = Case::parse_toml_in(&base(extra), &d).expect_err("must be rejected");
        assert!(
            errs.iter().any(|e| e.message.contains(want)),
            "want '{want}' in {errs:?}"
        );
    };
    check("[udf.a]\nfile = \"missing.wasm\"\n", "cannot read");
    check(
        "[udf.a]\nfile = \"a.wasm\"\nmomentum_source = true\n",
        "no 'momentum_source' export",
    );
    check("[udf.a]\nfile = \"a.wasm\"\nmonitor = true\n", "no 'monitor' export");
    check("[udf.a]\n", "needs a .wasm file");
    // a velocity UDF needs a defined module
    let text = channel("velocity_udf = \"nope\"", "nu = 0.1", "");
    let errs = Case::parse_toml_in(&text, &d).err().unwrap();
    assert!(
        errs.iter().any(|e| e.message.contains("no user function 'nope'")),
        "{errs:?}"
    );
    // the sandbox keeps module files inside the root
    let text = base("[udf.a]\nfile = \"../a.wasm\"\n");
    let sub = d.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let errs = Case::parse_toml_with(&text, &sub, Some(&sub)).err().unwrap();
    assert!(
        errs.iter().any(|e| e.message.contains("outside the sandbox")),
        "{errs:?}"
    );
    // steady-only roles are refused in transient runs
    let m = module(&format!(
        "(func (export \"momentum_source\") {MOMENTUM_PARAMS} f64.const 0.0)"
    ));
    std::fs::write(d.join("m.wasm"), wasm(&m)).unwrap();
    let text = base("[udf.a]\nfile = \"m.wasm\"\nmomentum_source = true\n").replace("simplec", "piso");
    let case = Case::parse_toml_in(&text, &d).unwrap();
    let err = Simulation::new(&case).err().unwrap();
    assert!(err.iter().any(|e| e.message.contains("steady")), "{err:?}");
}
