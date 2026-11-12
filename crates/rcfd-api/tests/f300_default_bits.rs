//! ADR 0225 V0b (F-300): the runs whose code paths the smooth wall eddy viscosity of k-epsilon passes through, hashed bit for
//! bit. FNV-1a (as `f294_default_bits.rs`, 64 bit, over the little-endian bytes of `f64::to_bits`) of every cell field the run
//! has among `ux, uy, uz, p, k, omega, nuTilda, epsilon, nut`, then of the face flux. The values below were recorded on main
//! c201a770 with the ADR 0225 registration and this file only, before any code of F-300, and must stay bit-identical after it.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const FIELDS: [&str; 9] = ["ux", "uy", "uz", "p", "k", "omega", "nuTilda", "epsilon", "nut"];

/// (steps, fields hashed, hash).
type Record = (usize, usize, u64);

fn fnv1a(values: &[f64], acc: &mut u64) {
    for x in values {
        for b in x.to_bits().to_le_bytes() {
            *acc ^= u64::from(b);
            *acc = acc.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// `n` steps or iterations; (steps taken, fields hashed, hash).
fn run(text: &str, n: usize) -> Record {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let errs = case.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}\n{text}");
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let steps = sim.run(n);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let mut acc = 0xcbf2_9ce4_8422_2325_u64;
    let mut fields = 0;
    for f in FIELDS {
        if let Ok(v) = sim.field(f) {
            fnv1a(&v, &mut acc);
            fields += 1;
        }
    }
    fnv1a(sim.flux(), &mut acc);
    if let Ok(yp) = sim.field("y_plus") {
        // report only: the wall-cell y* at the end of the run (k-epsilon), against y*_lam = 11.53
        let w: Vec<f64> = yp.into_iter().filter(|&v| v > 0.0).collect();
        let (lo, hi) = w
            .iter()
            .fold((f64::INFINITY, 0.0_f64), |(a, b), &v| (a.min(v), b.max(v)));
        println!("  wall-cell y* at the end in [{lo:.4}, {hi:.4}]");
    }
    (steps, fields, acc)
}

/// The CH channel of ADR 0199 with the given `[turbulence]` lines and solver block.
fn channel(turbulence: &str, solver: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 20
lx = 1.0
ly = 2.0
periodic_x = true
[physics]
nu = 0.0025316455696202532
rho = 1.0
[initial]
velocity = [15.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[sources.drive]
type = "acceleration"
acceleration = [1.0, 0.0, 0.0]
zone = "all"
[turbulence]
{turbulence}[solver]
convection = "upwind"
{solver}"#
    )
}

const KE: &str = "model = \"k_epsilon\"\nk_initial = 1.0\nepsilon_initial = 3.6\n";
/// ADR 0225 amendment 1: the start of the ADR 0210 S3 diagnosis (k 2, epsilon 14.4).
const KE_LOG: &str = "model = \"k_epsilon\"\nk_initial = 2.0\nepsilon_initial = 14.4\n";
const SA: &str = "model = \"spalart_allmaras\"\nnutilde_ratio = 40.0\n";
const STEADY: &str = "algorithm = \"simplec\"\nmax_iterations = 1000\n";
const PIMPLE_BDF2: &str = "algorithm = \"pimple\"\ntime_scheme = \"bdf2\"\nn_outer = 3\ndt = 0.1\nend_time = 100.0\n";
const PIMPLE_EULER: &str = "algorithm = \"pimple\"\ntime_scheme = \"euler\"\nn_outer = 3\ndt = 0.1\nend_time = 100.0\n";

/// (name, case text, steps, registered record).
fn cases() -> Vec<(&'static str, String, usize, Record)> {
    vec![
        (
            "transient k-epsilon channel on pimple, bdf2 (the F-294 path)",
            channel(KE, PIMPLE_BDF2),
            20,
            (20, 7, 0x89ac_d6f4_02fb_0b3c),
        ),
        (
            "transient k-epsilon channel on pimple, euler, switch treatment",
            channel(&format!("{KE}wall_treatment = \"switch\"\n"), PIMPLE_EULER),
            20,
            (20, 7, 0xdc8f_6503_7faa_4a96),
        ),
        (
            "transient k-epsilon channel on pimple, bdf2, scalable wall function",
            channel(&format!("{KE}wall_function = \"scalable\"\n"), PIMPLE_BDF2),
            20,
            (20, 7, 0x3f14_0c90_0c0a_7c17),
        ),
        (
            // k_s+ below 2.25 (hydraulically smooth): the rough branch runs with Delta B = 0 and gives the bits of the first run
            "transient k-epsilon channel on pimple, bdf2, rough walls",
            channel(&format!("{KE}roughness_height = 0.002\n"), PIMPLE_BDF2),
            20,
            (20, 7, 0x89ac_d6f4_02fb_0b3c),
        ),
        (
            // ADR 0225 amendment 1: k 2 and epsilon 14.4 (the same initial nut) put the wall cells at y* about 15, above
            // y*_lam, so the log branch of the wall function runs on the transient path from the first step
            "transient k-epsilon channel on pimple, bdf2, wall cells in the log layer",
            channel(KE_LOG, PIMPLE_BDF2),
            20,
            (20, 7, 0x02fd_4893_3dab_aa94),
        ),
        (
            // ADR 0225 amendment 1: the same with k_s+ about 6 (transitional), where the rough law differs from the smooth one
            "transient k-epsilon channel on pimple, bdf2, wall cells in the log layer, transitionally rough walls",
            channel(&format!("{KE_LOG}roughness_height = 0.02\n"), PIMPLE_BDF2),
            20,
            (20, 7, 0x70a0_889f_7f89_bccd),
        ),
        (
            "steady SIMPLEC k-epsilon channel, hysteresis treatment",
            channel(&format!("{KE}wall_treatment = \"hysteresis\"\n"), STEADY),
            300,
            (300, 7, 0xc954_0fae_514f_f3f8),
        ),
        (
            "transient SA channel on pimple, bdf2 (the F-294 path)",
            channel(SA, PIMPLE_BDF2),
            20,
            (20, 6, 0x0ce3_3c79_d483_d023),
        ),
    ]
}

/// V0b: every run bit-identical to its record on c201a770.
#[test]
fn v0b_default_paths_are_bit_identical_to_main() {
    let mut bad = Vec::new();
    for (name, text, n, want) in cases() {
        let got = run(&text, n);
        println!(
            "V0b {name}: (steps, fields, hash) = ({}, {}, 0x{:016x})",
            got.0, got.1, got.2
        );
        if got != want {
            bad.push(format!(
                "{name}: got ({}, {}, 0x{:016x}), recorded ({}, {}, 0x{:016x})",
                got.0, got.1, got.2, want.0, want.1, want.2
            ));
        }
    }
    assert!(bad.is_empty(), "V0b: {bad:#?}");
}
