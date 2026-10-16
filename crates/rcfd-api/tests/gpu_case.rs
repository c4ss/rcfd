//! ADR 0038 case-level checks: C1 (device = cuda vs cpu on a full case), C2 (explicit cpu is bit-identical), the case
//! halves of D2 (no device) and E1 (located errors). Runs in the default build too (the device-free parts); the
//! C1 test needs `--features cuda` and a device and prints `GPU TEST SKIPPED` without one.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const DISABLE: &str = "RCFD_GPU_DISABLE";

fn cavity(nx: usize, alg: &str, solver_extra: &str, section: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = {nx}\nny = {nx}\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"{alg}\"\nconvection = \"linear_upwind\"\nmax_iterations = 150\nend_time = 0.2\ndt = 0.01\n{solver_extra}\n{section}\n"
    )
}

struct Fields {
    names: Vec<(&'static str, Vec<f64>)>,
    flux: Vec<f64>,
}

fn run(text: &str, n: usize) -> Fields {
    let case = Case::parse_toml(text).expect("case parses");
    let mut sim = Simulation::new(&case).expect("simulation builds");
    sim.run(n);
    Fields {
        names: ["ux", "uy", "p"]
            .into_iter()
            .map(|f| (f, sim.field(f).expect("field")))
            .collect(),
        flux: sim.flux().to_vec(),
    }
}

fn differing_bits(a: &Fields, b: &Fields) -> usize {
    let mut n = 0;
    for ((_, x), (_, y)) in a.names.iter().zip(&b.names) {
        n += x.iter().zip(y).filter(|(p, q)| p.to_bits() != q.to_bits()).count();
    }
    n + a
        .flux
        .iter()
        .zip(&b.flux)
        .filter(|(p, q)| p.to_bits() != q.to_bits())
        .count()
}

#[test]
fn c2_explicit_cpu_device_is_bit_identical() {
    for (alg, n) in [("simple", 60), ("piso", 30)] {
        let plain = run(&cavity(20, alg, "", ""), n);
        for section in [
            "[linear_solver]\ndevice = \"cpu\"\n",
            "[linear_solver]\ndevice = \"cpu\"\npreconditioner = \"chebyshev\"\nchebyshev_degree = 6\n",
        ] {
            let with = run(&cavity(20, alg, "", section), n);
            assert_eq!(differing_bits(&plain, &with), 0, "{alg} {section}");
        }
    }
    println!("C2: explicit device = cpu (also with device-only keys set): 0 differing bits for simple and piso");
}

/// Diagnostics of a case text (parse and semantic errors alike).
fn diags(text: &str) -> Vec<rcfd_api::case::Diagnostic> {
    match Case::parse_toml(text) {
        Ok(c) => c.semantic_errors(),
        Err(e) => e,
    }
}

fn parse_err(text: &str) -> String {
    match Case::parse_toml(text) {
        Ok(c) => format!("OK; semantic: {:?}", c.semantic_errors()),
        Err(e) => format!("{e:?}"),
    }
}

#[test]
fn e1_schema_errors_are_located() {
    let bad_device = parse_err(&cavity(8, "simple", "", "[linear_solver]\ndevice = \"gpu\"\n"));
    println!("E1 device = gpu: {bad_device}");
    assert!(bad_device.contains("linear_solver.device"), "{bad_device}");
    let bad_degree = parse_err(&cavity(8, "simple", "", "[linear_solver]\nchebyshev_degree = 99\n"));
    println!("E1 chebyshev_degree = 99: {bad_degree}");
    assert!(bad_degree.contains("linear_solver.chebyshev_degree"), "{bad_degree}");
}

#[test]
fn e1_unsupported_algorithm_is_a_located_semantic_error() {
    for device in ["cuda", "auto"] {
        let text = cavity(8, "coupled", "", &format!("[linear_solver]\ndevice = \"{device}\"\n"));
        let diags = diags(&text);
        println!("E1 coupled + {device}: {diags:?}");
        assert!(
            diags
                .iter()
                .any(|d| d.path == "linear_solver.device" && d.message.contains("\"coupled\"")),
            "{diags:?}"
        );
    }
}

/// Body of the no-device check; it only acts when the parent test set the override (the probe is cached per process).
#[test]
fn d2_child_body() {
    if std::env::var(DISABLE).as_deref() != Ok("1") {
        return;
    }
    // auto falls back to the CPU path: identical bits to cpu, for the steady and the transient solver
    for (alg, n) in [("simple", 60), ("piso", 30)] {
        let cpu = run(&cavity(20, alg, "", "[linear_solver]\ndevice = \"cpu\"\n"), n);
        let auto = run(&cavity(20, alg, "", "[linear_solver]\ndevice = \"auto\"\n"), n);
        assert_eq!(
            differing_bits(&cpu, &auto),
            0,
            "auto without a device must equal cpu ({alg})"
        );
    }
    // cuda is a located diagnostic naming the reason
    let diags = diags(&cavity(
        8,
        "simple",
        "",
        "[linear_solver]
device = \"cuda\"
",
    ));
    assert!(
        diags
            .iter()
            .any(|d| d.path == "linear_solver.device" && d.message.contains(DISABLE)),
        "{diags:?}"
    );
    println!("D2 CASE CHILD OK: {diags:?}");
}

#[test]
fn d2_no_device_fallback_and_located_error_in_child_process() {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["d2_child_body", "--exact", "--nocapture", "--test-threads=1"])
        .env(DISABLE, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    println!("{stdout}");
    assert!(
        out.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("D2 CASE CHILD OK"),
        "the child did not run its assertions"
    );
}

#[cfg(not(feature = "cuda"))]
#[test]
fn d2_default_build_cuda_is_a_located_error() {
    let diags = diags(&cavity(
        8,
        "simple",
        "",
        "[linear_solver]
device = \"cuda\"
",
    ));
    println!("default build, device = cuda: {diags:?}");
    assert!(
        diags
            .iter()
            .any(|d| d.path == "linear_solver.device" && d.message.contains("feature cuda")),
        "{diags:?}"
    );
}

#[cfg(feature = "cuda")]
fn max_rel(a: &Fields, b: &Fields) -> Vec<(&'static str, f64)> {
    a.names
        .iter()
        .zip(&b.names)
        .map(|((name, x), (_, y))| {
            let scale = y.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            let diff = x.iter().zip(y).fold(0.0f64, |m, (p, q)| m.max((p - q).abs()));
            (*name, diff / scale)
        })
        .collect()
}

/// One C1 comparison set; `steady_extra` goes into the `[solver]` section of the steady case only.
#[cfg(feature = "cuda")]
fn c1_body(steady_extra: &str) {
    let tight = "pressure_rel_tol = 1e-10
pressure_max_iter = 5000
";
    let mut worst = 0.0f64;
    for (label, nx, alg, n, extra) in [
        (
            "steady simple 32x32, 150 iterations",
            32,
            "simple",
            150,
            format!("{tight}{steady_extra}"),
        ),
        ("piso 20x20, 20 steps", 20, "piso", 20, tight.to_string()),
    ] {
        let cpu = run(
            &cavity(
                nx,
                alg,
                &extra,
                "[linear_solver]
device = \"cpu\"
",
            ),
            n,
        );
        let size = |f: &Fields| {
            f.names
                .iter()
                .map(|(_, v)| v.iter().fold(0.0f64, |m, x| m.max(x.abs())))
                .fold(0.0f64, f64::max)
        };
        println!("C1 {label}: largest |field| of the cpu run {:.3e}", size(&cpu));
        for (pname, section) in [
            (
                "jacobi",
                "[linear_solver]
device = \"cuda\"
preconditioner = \"jacobi\"
",
            ),
            (
                "chebyshev4",
                "[linear_solver]
device = \"cuda\"
preconditioner = \"chebyshev\"
chebyshev_degree = 4
",
            ),
        ] {
            let gpu = run(&cavity(nx, alg, &extra, section), n);
            let rel = max_rel(&gpu, &cpu);
            println!("C1 {label} {pname}: max|a-b|/max|b| = {rel:?} (limit 1e-6)");
            for (name, r) in rel {
                worst = worst.max(r);
                assert!(r <= 1e-6, "{label} {pname} {name}: {r:e}");
            }
        }
    }
    println!("C1 worst over all: {worst:.3e}");
}

/// FIRST C1 test, kept as a record. Its steady case used the default relaxation of `simple`, with which the case
/// DIVERGES on the CPU and on the device alike (fields of order 1e60 after 150 iterations), so the comparison is
/// meaningless. Set-up defect, found on the first run; the corrected test is `c1_full_case_cuda_matches_cpu`.
#[cfg(feature = "cuda")]
#[test]
#[ignore = "set-up defect: the steady case diverges with default relaxation; superseded by c1_full_case_cuda_matches_cpu"]
fn c1_full_case_cuda_matches_cpu_diverged_setup() {
    if rcfd_gpu::skip_without_device("c1_full_case_diverged_setup") {
        return;
    }
    c1_body("");
}

/// C1 (corrected set-up): the steady case uses `alpha_u = 0.7`, `alpha_p = 0.3`; limits unchanged.
#[cfg(feature = "cuda")]
#[test]
fn c1_full_case_cuda_matches_cpu() {
    if rcfd_gpu::skip_without_device("c1_full_case") {
        return;
    }
    c1_body(
        "alpha_u = 0.7
alpha_p = 0.3
",
    );
}
