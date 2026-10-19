//! Wave 34C (ADR 0074 part C): measurements for the analysis of `solver.component_diagonal = "corrected"` as the default
//! (`docs/component-diagonal-default.md`). Diagnostics only (ignored, no limits, nothing asserted): the default is NOT changed.
//!
//! (C1) every case of `baseline_hashes.rs` hashed exactly as there, once with the key `corrected` injected into `[solver]`;
//! (C2) every shipped `cases/*/*.toml` run with `averaged` and with `corrected` (same iteration budget), field differences reported.
//! `cargo test --release -p rcfd-api --test w34c_component_diagonal_default -- --ignored --nocapture --test-threads=1`

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn cavity(alg: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 20\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"{alg}\"\nconvection = \"linear_upwind\"\nmax_iterations = 60\nend_time = 0.3\ndt = 0.01\n"
    )
}

const HEATED: &str = "[mesh]\ngenerator = \"box_2d\"\nnx = 24\nny = 24\nlx = 1.0\nly = 1.0\ngrading = 4.0\n[physics]\nnu = 0.71\nrho = 1.0\ncp = 1.0\nconductivity = 1.0\nreference_temperature = 0.5\nthermal_expansion = 1.0\ngravity = [0.0, -710.0, 0.0]\n[energy]\nenabled = true\n[initial]\ntemperature = 0.5\n[boundaries.xmin]\nvelocity = \"fixed\"\ntemperature = \"fixed\"\ntemperature_value = 1.0\n[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"fixed\"\ntemperature_value = 0.0\n[boundaries.ymin]\nvelocity = \"fixed\"\n[boundaries.ymax]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"ALG\"\nconvection = \"tvd_van_leer\"\nalpha_u = 0.8\nmax_iterations = 60\nend_time = 0.2\ndt = 0.005\n";

fn fnv1a(values: &[f64], acc: &mut u64) {
    for x in values {
        for b in x.to_bits().to_le_bytes() {
            *acc ^= u64::from(b);
            *acc = acc.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// The hash of `baseline_hashes.rs` (60 steps, fields ux, uy, p, T and the face flux).
fn hash(text: &str) -> (usize, u64) {
    let case = Case::parse_toml(text).expect("case parses");
    let mut sim = Simulation::new(&case).expect("simulation builds");
    let n = sim.run(60);
    let mut acc = 0xcbf2_9ce4_8422_2325_u64;
    for f in ["ux", "uy", "p", "T"] {
        if let Ok(v) = sim.field(f) {
            fnv1a(&v, &mut acc);
        }
    }
    fnv1a(sim.flux(), &mut acc);
    (n, acc)
}

fn with_key(text: &str, value: &str) -> String {
    let t = text.replace("\r\n", "\n");
    assert!(t.contains("[solver]\n"), "case without [solver]");
    t.replacen(
        "[solver]\n",
        &format!("[solver]\ncomponent_diagonal = \"{value}\"\n"),
        1,
    )
}

fn file(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/../../cases/{rel}", env!("CARGO_MANIFEST_DIR"))).expect("case file readable")
}

/// C1: the baseline-hash cases with the corrected diagonal.
#[test]
#[ignore = "diagnostic (ADR 0074 part C), no limit"]
fn c1_baseline_hash_cases_with_the_corrected_diagonal() {
    let mut cases: Vec<(String, String, (usize, u64))> = Vec::new();
    for (alg, reg) in [
        ("simple", (60, 0x07e7461c36893384)),
        ("simplec", (60, 0x58d5c4fd29432b05)),
        ("coupled", (59, 0x123b5330dae3f36a)),
        ("piso", (30, 0x2a042a42594c418b)),
        ("pimple", (30, 0x2a042a42594c418b)),
    ] {
        cases.push((format!("cavity-{alg}"), cavity(alg), reg));
    }
    for (alg, reg) in [
        ("simple", (60, 0x30a23d946786fd8a)),
        ("simplec", (60, 0x7d35b1f2f7fcc4b7)),
        ("piso", (40, 0xfe7b5aa0a252ec0f)),
    ] {
        cases.push((format!("heated-{alg}"), HEATED.replace("ALG", alg), reg));
    }
    for (name, rel, reg) in [
        ("packed_bed", "packed_bed/packed_bed.toml", (60, 0x19e53df373637262)),
        ("sod", "sod/sod.toml", (60, 0xe1612365139bf898)),
        ("wedge", "wedge/wedge.toml", (60, 0xe198a93b898bd79b)),
        (
            "kepsilon_channel",
            "channel/kepsilon_channel.toml",
            (60, 0x6ddc55969af2a835),
        ),
        (
            "rotating_channel (ignored test)",
            "rotating_channel/rotating_channel.toml",
            (60, 0x54d9bee17b5895e8),
        ),
    ] {
        cases.push((name.to_string(), file(rel), reg));
    }
    for (name, text, reg) in &cases {
        let avg = hash(text);
        let cor = hash(&with_key(text, "corrected"));
        println!(
            "C1 {name}: registered ({}, {:016x}); averaged now ({}, {:016x}){}; corrected ({}, {:016x}){}",
            reg.0,
            reg.1,
            avg.0,
            avg.1,
            if avg == *reg {
                " = registered"
            } else {
                " DIFFERS from registered"
            },
            cor.0,
            cor.1,
            if cor == avg { " = averaged" } else { " MOVES" }
        );
    }
}

/// Field names reported by C2.
const FIELDS: [&str; 8] = ["ux", "uy", "uz", "p", "k", "omega", "epsilon", "nut"];

fn run_case(text: &str, dir: &std::path::Path, n: usize) -> (usize, Vec<(String, Vec<f64>)>, Simulation) {
    let case = Case::parse_toml_in(text, dir).expect("case parses");
    let mut sim = Simulation::new(&case).expect("simulation builds");
    let done = sim.run(n);
    let f = FIELDS
        .iter()
        .filter_map(|n| sim.field(n).ok().map(|v| (n.to_string(), v)))
        .collect();
    (done, f, sim)
}

/// C2: every shipped case with both settings at the same iteration budget (`min(max_iterations, 2000)` or the case's step count).
#[test]
#[ignore = "diagnostic (ADR 0074 part C), no limit"]
fn c2_shipped_cases_averaged_against_corrected() {
    let dir = format!("{}/../../cases", env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<String> = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let d = e.unwrap().path();
        if d.is_dir() {
            for f in std::fs::read_dir(&d).unwrap() {
                let p = f.unwrap().path();
                if p.extension().is_some_and(|x| x == "toml") {
                    files.push(p.to_string_lossy().to_string());
                }
            }
        }
    }
    files.sort();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
        let short = path.rsplit(['/', '\\']).take(2).collect::<Vec<_>>().join(" <- ");
        if !text.contains("[solver]\n") {
            println!("C2 {short}: no [solver] table, skipped");
            continue;
        }
        let budget = 2000;
        let t0 = std::time::Instant::now();
        let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dir = std::path::Path::new(&path).parent().unwrap();
            let (na, fa, sa) = run_case(&with_key(&text, "averaged"), dir, budget);
            let (nc, fc, sc) = run_case(&with_key(&text, "corrected"), dir, budget);
            (na, fa, sa.is_converged(), nc, fc, sc.is_converged())
        }));
        let Ok((na, fa, sta, nc, fc, stc)) = built else {
            println!("C2 {short}: run failed (panic)");
            continue;
        };
        let mut line = format!("C2 {short}: iterations averaged {na}, corrected {nc};");
        let mut identical = true;
        for ((name, a), (_, c)) in fa.iter().zip(&fc) {
            let range = a.iter().fold(0.0_f64, |m, v| m.max(v.abs())).max(1e-300);
            let d = a.iter().zip(c).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
            if a.iter().zip(c).any(|(x, y)| x.to_bits() != y.to_bits()) {
                identical = false;
            }
            line += &format!(" {name} {:.3e} ({:.3e} rel);", d, d / range);
        }
        line += if identical { " BIT-IDENTICAL" } else { " differs" };
        println!("{line} [{:.0} s]", t0.elapsed().as_secs_f64());
        println!("C2 {short}: converged averaged {sta}, corrected {stc}");
    }
}
