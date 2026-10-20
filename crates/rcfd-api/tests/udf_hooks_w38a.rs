//! User-function hook catalogue v2 (F-239, ADR 0088 checks C1 to C8). The limits are those of the ADR, registered
//! before this file was written; none was changed afterwards. Modules are written in WAT.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_api::udf::ABI_VERSION;
use std::path::{Path, PathBuf};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_udf38_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Write a module with a correct ABI export and the given items to `dir/name.wasm`.
fn put(dir: &Path, name: &str, body: &str) {
    let text = format!("(module (func (export \"rcfd_abi_version\") (result i32) i32.const {ABI_VERSION}) {body})");
    std::fs::write(dir.join(format!("{name}.wasm")), wat::parse_str(text).unwrap()).unwrap();
}

const P4: &str = "(param f64 f64 f64 f64) (result f64)";
const P5: &str = "(param f64 f64 f64 f64 f64) (result f64)";
const P9: &str = "(param f64 f64 f64 f64 f64 f64 f64 f64 f64) (result f64)";

const L: f64 = 0.1;
const K: f64 = 2.0;

/// The slab of tests/zone_sources.rs: nx x 2 cells on 0.1 x 0.02 m, no flow, T = 300 K on xmin and xmax.
fn slab(n: usize, xmin: &str, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = {n}
ny = 2
lx = 0.1
ly = 0.02
[physics]
nu = 0.01
rho = 1.0
cp = 1000.0
conductivity = 2.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 300.0
[boundaries.xmin]
velocity = "fixed"
{xmin}
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
tolerance_u = 1e-12
max_iterations = 400
{extra}
"#
    )
}

const T300: &str = "temperature = \"fixed\"\ntemperature_value = 300.0";
const ZONE_Q: &str = "[zone_sources.q]\ntarget = \"temperature\"\nzone = \"all\"\nvalue = 1e4\n";

fn run(dir: &Path, text: &str) -> Simulation {
    let case = Case::parse_toml_in(text, dir).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    sim.run(400);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    sim
}

/// max |a - b| over the cells of field `f`, divided by the range of `b` (temperature) or max |b| (other fields).
fn rel(a: &Simulation, b: &Simulation, f: &str) -> f64 {
    let (x, y) = (a.field(f).unwrap(), b.field(f).unwrap());
    let (lo, hi) = y.iter().fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    let scale = if f == "T" {
        hi - lo
    } else {
        y.iter().fold(0.0_f64, |m, v| m.max(v.abs()))
    };
    x.iter().zip(&y).map(|(p, q)| (p - q).abs()).fold(0.0, f64::max) / scale.max(1e-300)
}

fn xs(s: &Simulation) -> Vec<f64> {
    s.geom.mesh.cell_centres().iter().map(|c| c[0]).collect()
}

/// Set-up defect, kept as the record: the sanity part divides by the range of the uniform-300 K run, which is zero, so
/// its value (about 1e301) is meaningless. The equivalence part passed (0). Superseded by `c1_corrected_sanity_scale`.
#[test]
#[ignore = "set-up defect (sanity metric scaled by the zero range of the uniform run); superseded by c1_corrected_sanity_scale"]
fn c1_temperature_profile_equals_the_expression() {
    c1_body(false);
}

#[test]
fn c1_corrected_sanity_scale() {
    c1_body(true);
}

fn c1_body(corrected: bool) {
    let d = tmp(if corrected { "c1c" } else { "c1" });
    put(
        &d,
        "tp",
        &format!(
            "(func (export \"temperature_profile\") {P4} f64.const 300.0 f64.const 1000.0 local.get 1 f64.mul f64.add)"
        ),
    );
    let hook = run(
        &d,
        &slab(
            32,
            "temperature = \"fixed\"\ntemperature_udf = \"tp\"",
            "[udf.tp]\nfile = \"tp.wasm\"\n",
        ),
    );
    let plain = run(
        &d,
        &slab(
            32,
            "temperature = \"fixed\"\ntemperature_expression = \"300 [K] + 1000 [K/m] * y\"",
            "",
        ),
    );
    let uniform = run(&d, &slab(32, T300, ""));
    let sanity = if corrected {
        rel(&uniform, &hook, "T")
    } else {
        rel(&hook, &uniform, "T")
    };
    let diff = rel(&hook, &plain, "T");
    println!("C1 rel diff {diff:e} (limit 1e-9), against uniform 300 K {sanity:e} (> 1e-3)");
    assert!(diff < 1e-9, "C1 {diff:e}");
    assert!(sanity > 1e-3, "C1 sanity {sanity:e}");
}

#[test]
fn c2_wall_heat_flux_equals_the_built_in_and_the_linear_profile() {
    let d = tmp("c2");
    put(
        &d,
        "hf",
        &format!("(func (export \"wall_heat_flux\") {P4} f64.const 5000.0)"),
    );
    let hook = run(
        &d,
        &slab(
            32,
            "temperature = \"heat_flux\"\nheat_flux_udf = \"hf\"",
            "[udf.hf]\nfile = \"hf.wasm\"\n",
        ),
    );
    let plain = run(
        &d,
        &slab(32, "temperature = \"heat_flux\"\nheat_flux_value = 5000.0", ""),
    );
    let diff = rel(&hook, &plain, "T");
    let q = 5000.0;
    let (t, x) = (hook.field("T").unwrap(), xs(&hook));
    let exact = (0..t.len())
        .map(|i| (t[i] - (300.0 + q * (L - x[i]) / K)).abs())
        .fold(0.0, f64::max)
        / (q * L / K);
    println!("C2 rel diff {diff:e} (limit 1e-9), closed form {exact:e} (limit 1e-9)");
    assert!(diff < 1e-9, "C2 {diff:e}");
    assert!(exact < 1e-9, "C2 closed form {exact:e}");
}

#[test]
fn c3_conductivity_hook_equals_the_constant_property_run() {
    let d = tmp("c3");
    put(
        &d,
        "k",
        &format!("(func (export \"thermal_conductivity\") {P5} f64.const 2.0)"),
    );
    let hook = run(
        &d,
        &slab(
            32,
            T300,
            &format!("{ZONE_Q}[udf.k]\nfile = \"k.wasm\"\nconductivity = true\n"),
        )
        .replace("conductivity = 2.0", "conductivity = 5.0"),
    );
    let plain = run(&d, &slab(32, T300, ZONE_Q));
    let wrong = run(
        &d,
        &slab(32, T300, ZONE_Q).replace("conductivity = 2.0", "conductivity = 5.0"),
    );
    let (diff, sanity) = (rel(&hook, &plain, "T"), rel(&wrong, &plain, "T"));
    println!("C3 rel diff {diff:e} (limit 1e-9), k = 5 without the hook {sanity:e} (> 1e-3)");
    assert!(diff < 1e-9, "C3 {diff:e}");
    assert!(sanity > 1e-3, "C3 sanity {sanity:e}");
}

#[test]
fn c4_energy_source_equals_the_zone_source_and_the_slab_profile() {
    let d = tmp("c4");
    put(
        &d,
        "q",
        &format!("(func (export \"energy_source\") {P5} f64.const 1e4)"),
    );
    put(
        &d,
        "ql",
        &format!("(func (export \"energy_source\") {P5} f64.const 2e4 local.get 0 f64.mul f64.const 0.1 f64.div)"),
    );
    let hook = run(
        &d,
        &slab(64, T300, "[udf.q]\nfile = \"q.wasm\"\nenergy_source = true\n"),
    );
    let plain = run(&d, &slab(64, T300, ZONE_Q));
    // C4a
    let diff = rel(&hook, &plain, "T");
    // C4b: discrete identity of the uniform source
    let q = 1e4;
    let h = L / 64.0;
    let (t, x) = (hook.field("T").unwrap(), xs(&hook));
    let ident = (0..t.len())
        .map(|i| (t[i] - (300.0 + q * x[i] * (L - x[i]) / (2.0 * K)) - q * h * h / (8.0 * K)).abs())
        .fold(0.0, f64::max)
        / (q * L * L / (8.0 * K));
    // C4c: linear source against the closed form
    let lin = run(
        &d,
        &slab(64, T300, "[udf.ql]\nfile = \"ql.wasm\"\nenergy_source = true\n"),
    );
    let q0 = 2e4;
    let exact = |x: f64| q0 * x * (L * L - x * x) / (6.0 * K * L);
    let rise = (0..2000).map(|i| exact(L * i as f64 / 2000.0)).fold(0.0, f64::max);
    let t = lin.field("T").unwrap();
    let err = (0..t.len())
        .map(|i| (t[i] - 300.0 - exact(x[i])).abs())
        .fold(0.0, f64::max)
        / rise;
    println!("C4a rel diff {diff:e} (limit 1e-9); C4b identity {ident:e} (limit 1e-6); C4c error {err:e} (limit 6e-4)");
    assert!(diff < 1e-9, "C4a {diff:e}");
    assert!(ident <= 1e-6, "C4b {ident:e}");
    assert!(err <= 6e-4, "C4c {err:e}");
}

#[test]
fn c5_scalar_source_equals_the_built_in_source_and_the_identity() {
    let d = tmp("c5");
    put(
        &d,
        "s",
        &format!("(func (export \"scalar_source\") {P5} f64.const 5.0)"),
    );
    let scalar = |src: &str| {
        format!("[scalars.c]\ndiffusivity = 0.01\n{src}\n[scalars.c.boundaries.xmin]\ntype = \"fixed\"\nvalue = 0.0\n[scalars.c.boundaries.xmax]\ntype = \"fixed\"\nvalue = 0.0\n")
    };
    let hook = run(
        &d,
        &slab(
            64,
            T300,
            &format!("{}[udf.s]\nfile = \"s.wasm\"\n", scalar("source_udf = \"s\"")),
        ),
    );
    let plain = run(&d, &slab(64, T300, &scalar("source = 5.0")));
    let diff = rel(&hook, &plain, "c");
    let (sc, dc, h) = (5.0, 0.01, L / 64.0);
    let (c, x) = (hook.field("c").unwrap(), xs(&hook));
    let ident = (0..c.len())
        .map(|i| (c[i] - sc * x[i] * (L - x[i]) / (2.0 * dc) - sc * h * h / (8.0 * dc)).abs())
        .fold(0.0, f64::max)
        / (sc * L * L / (8.0 * dc));
    println!("C5 rel diff {diff:e} (limit 1e-9), identity {ident:e} (limit 1e-6)");
    assert!(diff < 1e-9, "C5 {diff:e}");
    assert!(ident <= 1e-6, "C5 identity {ident:e}");
}

fn channel(initial: &str, extra: &str) -> String {
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
nu = 0.1
rho = 1.0
cp = 1000.0
conductivity = 0.5
[energy]
enabled = true
[initial]
velocity = [1.0, 0.0, 0.0]
{initial}
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
temperature = "fixed"
temperature_value = 320.0
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
max_iterations = 50
{extra}
"#
    )
}

#[test]
fn c6_initialise_hook_equals_the_initial_expressions() {
    let d = tmp("c6");
    put(
        &d,
        "i",
        "(func (export \"initial_value\") (param f64 f64 f64 i32) (result f64)
           (if (result f64) (i32.eqz (local.get 3))
             (then (f64.mul (f64.mul (f64.const 6.0) (local.get 1)) (f64.sub (f64.const 1.0) (local.get 1))))
             (else (if (result f64) (i32.eq (local.get 3) (i32.const 3))
               (then (f64.const 0.25))
               (else (if (result f64) (i32.eq (local.get 3) (i32.const 4))
                 (then (f64.const 310.0))
                 (else (f64.const 0.0))))))))",
    );
    let build = |text: String| {
        let case = Case::parse_toml_in(&text, &d).unwrap_or_else(|e| panic!("{e:?}"));
        Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
    };
    let mut hook = build(channel("", "[udf.i]\nfile = \"i.wasm\"\ninitialise = true\n"));
    let mut plain = build(channel(
        "velocity_expression_x = \"6 [m/s] * y / 1 [m] * (1 - y / 1 [m])\"\npressure = 0.25\ntemperature = 310.0",
        "",
    ));
    let fields = ["ux", "uy", "p", "T"];
    let at_build = fields.iter().map(|f| rel(&hook, &plain, f)).fold(0.0, f64::max);
    // the plain build must itself differ from a uniform start, or the comparison is empty
    assert!(plain.field("ux").unwrap().iter().any(|v| (v - 1.0).abs() > 0.1));
    hook.run(50);
    plain.run(50);
    assert!(hook.failure().is_none() && plain.failure().is_none());
    let after = fields.iter().map(|f| rel(&hook, &plain, f)).fold(0.0, f64::max);
    println!("C6 rel diff at build {at_build:e} (limit 1e-12), after 50 iterations {after:e} (limit 1e-9)");
    assert!(at_build < 1e-12, "C6 build {at_build:e}");
    assert!(after < 1e-9, "C6 after {after:e}");
}

#[test]
fn c7_execute_at_end_runs_once_on_the_final_fields() {
    let d = tmp("c7");
    put(&d, "e", &format!("(func (export \"execute_at_end\") {P9} local.get 4)"));
    let sim = run(
        &d,
        &channel("", "[udf.e]\nfile = \"e.wasm\"\nat_end = true\n")
            .replace("max_iterations = 50", "max_iterations = 300"),
    );
    let ux = sim.field("ux").unwrap();
    let expect: f64 = ux.iter().zip(sim.geom.mesh.cell_volumes()).map(|(u, v)| u * v).sum();
    let series = sim.monitors.get("udf_end_e").expect("end-of-run value recorded");
    let last = sim.last().unwrap().index as f64;
    let r = ((series.y[0] - expect) / expect).abs();
    println!(
        "C7 values {} at x {:?} (last step {last}), relative difference {r:e} (limit 1e-12)",
        series.y.len(),
        series.x
    );
    assert_eq!(series.y.len(), 1);
    assert_eq!(series.x[0], last);
    assert!(r < 1e-12, "C7 {r:e}");
}

/// The case is refused (at parse or at build) with a message containing `want`.
fn refused(dir: &Path, text: &str, want: &str) {
    let errs = match Case::parse_toml_in(text, dir) {
        Err(e) => e,
        Ok(c) => match Simulation::new(&c) {
            Err(e) => e,
            Ok(_) => panic!("accepted, wanted '{want}'"),
        },
    };
    assert!(
        errs.iter().any(|e| e.message.contains(want)),
        "want '{want}' in {errs:?}"
    );
}

#[test]
fn c8_diagnostics_and_refusals() {
    let d = tmp("c8");
    put(&d, "none", "");
    put(
        &d,
        "q",
        &format!("(func (export \"energy_source\") {P5} f64.const 1e4)"),
    );
    put(
        &d,
        "q2",
        &format!("(func (export \"energy_source\") {P5} f64.const 1e4)"),
    );
    put(
        &d,
        "k",
        &format!("(func (export \"thermal_conductivity\") {P5} f64.const 2.0)"),
    );
    put(
        &d,
        "s",
        &format!("(func (export \"scalar_source\") {P5} f64.const 5.0)"),
    );
    put(
        &d,
        "tp",
        &format!("(func (export \"temperature_profile\") {P4} f64.const 300.0)"),
    );
    put(
        &d,
        "hf",
        &format!("(func (export \"wall_heat_flux\") {P4} f64.const 10.0)"),
    );
    put(
        &d,
        "i",
        "(func (export \"initial_value\") (param f64 f64 f64 i32) (result f64) f64.const 0.0)",
    );
    let flag = |module: &str, f: &str| slab(16, T300, &format!("[udf.m]\nfile = \"{module}.wasm\"\n{f} = true\n"));
    // role flags without their export
    for (f, export) in [
        ("conductivity", "thermal_conductivity"),
        ("energy_source", "energy_source"),
        ("initialise", "initial_value"),
        ("at_end", "execute_at_end"),
    ] {
        refused(&d, &flag("none", f), &format!("no '{export}' export"));
    }
    // references without the export, undefined, or on the wrong condition
    let m = "[udf.m]\nfile = \"none.wasm\"\n";
    refused(
        &d,
        &slab(16, "temperature = \"fixed\"\ntemperature_udf = \"m\"", m),
        "no 'temperature_profile' export",
    );
    refused(
        &d,
        &slab(16, "temperature = \"heat_flux\"\nheat_flux_udf = \"m\"", m),
        "no 'wall_heat_flux' export",
    );
    let sc = "[scalars.c]\ndiffusivity = 0.01\nsource_udf = \"m\"\n";
    refused(&d, &slab(16, T300, &format!("{m}{sc}")), "no 'scalar_source' export");
    refused(
        &d,
        &slab(16, "temperature = \"fixed\"\ntemperature_udf = \"nope\"", ""),
        "no user function 'nope'",
    );
    refused(
        &d,
        &slab(
            16,
            "temperature = \"heat_flux\"\ntemperature_udf = \"tp\"",
            "[udf.tp]\nfile = \"tp.wasm\"\n",
        ),
        "needs boundaries.xmin.temperature = \"fixed\"",
    );
    refused(
        &d,
        &slab(
            16,
            "temperature = \"fixed\"\nheat_flux_udf = \"hf\"",
            "[udf.hf]\nfile = \"hf.wasm\"\n",
        ),
        "needs boundaries.xmin.temperature = \"heat_flux\"",
    );
    // two providers
    refused(
        &d,
        &slab(
            16,
            T300,
            "[udf.a]\nfile = \"q.wasm\"\nenergy_source = true\n[udf.b]\nfile = \"q2.wasm\"\nenergy_source = true\n",
        ),
        "only one user function can provide energy_source",
    );
    // steady SIMPLE/SIMPLEC only
    let piso = |t: String| {
        t.replace(
            "algorithm = \"simplec\"",
            "algorithm = \"piso\"\ndt = 0.01\nend_time = 0.1",
        )
    };
    refused(&d, &piso(flag("q", "energy_source")), "only in steady");
    refused(&d, &piso(flag("k", "conductivity")), "only in steady");
    refused(
        &d,
        &piso(slab(
            16,
            T300,
            "[udf.s]\nfile = \"s.wasm\"\n[scalars.c]\ndiffusivity = 0.01\nsource_udf = \"s\"\n",
        )),
        "only in steady",
    );
    // conductivity with a property law or a heat_flux boundary
    refused(
        &d,
        &(flag("k", "conductivity") + "[properties.conductivity]\nlaw = \"polynomial\"\ncoefficients = [2.0]\n"),
        "cannot be combined with properties.conductivity",
    );
    refused(
        &d,
        &slab(
            16,
            "temperature = \"heat_flux\"\nheat_flux_value = 10.0",
            "[udf.m]\nfile = \"k.wasm\"\nconductivity = true\n",
        ),
        "heat_flux boundary",
    );
    // initialise needs the uniform method
    refused(
        &d,
        &channel(
            "method = \"potential\"",
            "[udf.m]\nfile = \"i.wasm\"\ninitialise = true\n",
        ),
        "initial.method = \"uniform\"",
    );
    // a non-positive conductivity and a non-finite energy source stop the run
    for (body, want) in [
        (
            format!("(func (export \"thermal_conductivity\") {P5} f64.const -1.0)"),
            "not positive",
        ),
        (
            format!("(func (export \"energy_source\") {P5} f64.const 0.0 f64.const 0.0 f64.div)"),
            "non-finite",
        ),
    ] {
        put(&d, "bad", &body);
        let f = if body.contains("thermal") {
            "conductivity"
        } else {
            "energy_source"
        };
        let case = Case::parse_toml_in(&flag("bad", f), &d).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        let s = sim.step();
        assert!(s.done && !s.converged);
        let e = sim.failure().unwrap();
        assert!(e.contains(want), "{e}");
    }
}
