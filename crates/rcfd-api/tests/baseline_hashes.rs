//! Constant-density baseline regression gate (replaces the git-excluded `baseline_hash` example).
//!
//! The notes of F-031, F-050 and ADR 0005 check d1 claim that the paths that existed before the variable-density
//! kernel give bit-identical results. This test makes the claim repeatable: each case runs a fixed number of
//! iterations and the exact f64 bit patterns of `ux, uy, p, T` and the face fluxes are hashed with FNV-1a (64 bit,
//! over the little-endian bytes of `f64::to_bits`). The expected values below were produced at commit 8b68e46
//! (F-114, the last commit before ADR 0005) in a separate worktree, run twice with identical output, and were
//! reproduced bit for bit at HEAD for every case except `rotating_channel` (see the ignored test).
//!
//! A mismatch means the numerics of a constant-density path changed. If that is intended, say so in the commit and
//! update the value; do not update it silently.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn cavity(alg: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 20\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"{alg}\"\nconvection = \"linear_upwind\"\nmax_iterations = 60\nend_time = 0.3\ndt = 0.01\n"
    )
}

const HEATED: &str = r#"
[mesh]
generator = "box_2d"
nx = 24
ny = 24
lx = 1.0
ly = 1.0
grading = 4.0
[physics]
nu = 0.71
rho = 1.0
cp = 1.0
conductivity = 1.0
reference_temperature = 0.5
thermal_expansion = 1.0
gravity = [0.0, -710.0, 0.0]
[energy]
enabled = true
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "ALG"
convection = "tvd_van_leer"
alpha_u = 0.8
max_iterations = 60
end_time = 0.2
dt = 0.005
"#;

/// FNV-1a over the little-endian bytes of the bit pattern of every value.
fn fnv1a(values: &[f64], acc: &mut u64) {
    for x in values {
        for b in x.to_bits().to_le_bytes() {
            *acc ^= u64::from(b);
            *acc = acc.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// Runs 60 iterations/steps of a case and returns (iterations reported, hash).
fn run(text: &str) -> (usize, u64) {
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

fn run_file(rel: &str) -> (usize, u64) {
    let path = format!("{}/../../cases/{rel}", env!("CARGO_MANIFEST_DIR"));
    run(&std::fs::read_to_string(path).expect("case file readable"))
}

fn check(name: &str, got: (usize, u64), steps: usize, hash: u64) {
    assert_eq!(
        got,
        (steps, hash),
        "{name}: got (steps, hash) = ({}, {:016x}), registered ({steps}, {hash:016x})",
        got.0,
        got.1
    );
}

#[test]
fn cavity_paths_match_registered_hashes() {
    check("cavity-simple", run(&cavity("simple")), 60, 0x07e7461c36893384);
    check("cavity-simplec", run(&cavity("simplec")), 60, 0x58d5c4fd29432b05);
    check("cavity-coupled", run(&cavity("coupled")), 59, 0x123b5330dae3f36a);
    check("cavity-piso", run(&cavity("piso")), 30, 0x2a042a42594c418b);
    check("cavity-pimple", run(&cavity("pimple")), 30, 0x2a042a42594c418b);
}

#[test]
fn heated_cavity_paths_match_registered_hashes() {
    check(
        "heated-simple",
        run(&HEATED.replace("ALG", "simple")),
        60,
        0x30a23d946786fd8a,
    );
    check(
        "heated-simplec",
        run(&HEATED.replace("ALG", "simplec")),
        60,
        0x7d35b1f2f7fcc4b7,
    );
    check(
        "heated-piso",
        run(&HEATED.replace("ALG", "piso")),
        40,
        0xfe7b5aa0a252ec0f,
    );
}

#[test]
fn shipped_cases_match_registered_hashes() {
    check(
        "packed_bed",
        run_file("packed_bed/packed_bed.toml"),
        60,
        0x19e53df373637262,
    );
    check("sod", run_file("sod/sod.toml"), 60, 0xe1612365139bf898);
    check("wedge", run_file("wedge/wedge.toml"), 60, 0xe198a93b898bd79b);
    check(
        "kepsilon_channel",
        run_file("channel/kepsilon_channel.toml"),
        60,
        0xda0ee795fd177d8e,
    );
}

/// KNOWN DIFFERS. The registered value is the one at 8b68e46 (before ADR 0005). It was unchanged through the
/// variable-density commits (still equal at c65ffb4) and changed at d1b43f0 "rest state in a rotating frame solved
/// to round-off (F-031)", an intentional change of the wall treatment in the rotating-frame path. Fields differ by
/// at most about 6e-5 (p), 1e-5 (uy), 1e-6 (ux) and 1.4e-8 (flux) after 60 iterations. Remove the `ignore` and
/// re-register once the change is accepted as the new baseline.
#[test]
#[ignore = "known differs since d1b43f0 (F-031 rotating-frame rest-state fix); registered hash is the pre-ADR-0005 value"]
fn rotating_channel_matches_pre_adr0005_hash() {
    check(
        "rotating_channel",
        run_file("rotating_channel/rotating_channel.toml"),
        60,
        0x54d9bee17b5895e8,
    );
}
