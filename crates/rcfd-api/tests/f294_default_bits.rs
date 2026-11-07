//! ADR 0210 V0b (F-294): the runs whose code paths the transient k-epsilon and Spalart-Allmaras change passes through, hashed
//! bit for bit. FNV-1a (as `f286_default_bits.rs`, 64 bit, over the little-endian bytes of `f64::to_bits`) of every cell field
//! the run has among `ux, uy, uz, p, k, omega, nuTilda, epsilon, nut`, then of the face flux. The values below were recorded on
//! main 765ac64 with the ADR 0210 registration and this file only, before any code of F-294, and must stay bit-identical after
//! it.

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
const SA: &str = "model = \"spalart_allmaras\"\nnutilde_ratio = 40.0\n";
const SST: &str = "model = \"k_omega_sst\"\nk_initial = 1.0\nomega_initial = 40.0\n";
const STEADY: &str = "algorithm = \"simplec\"\nmax_iterations = 1000\n";

/// The ADR 0183 PC-3 sector with k-epsilon (the S4 text of ADR 0210 amendment 1: `outer` a wall at rest in the rotating frame)
/// and the given solver block.
fn sector_kepsilon(solver: &str) -> String {
    format!(
        r#"
[mesh]
generator = "annulus_3d_sector"
nx = 8
ny = 6
nz = 8
r_in = 0.5
r_out = 1.0
lz = 0.5
sector_angle = 40.0
[physics]
nu = 0.01
rho = 1.0
[initial]
velocity = [0.0, 0.0, 1.0]
[boundaries.zmin]
velocity = "fixed"
velocity_expression_x = "2 [1/s] * y + 0.2 [1/s] * x"
velocity_expression_y = "-2 [1/s] * x + 0.2 [1/s] * y"
velocity_expression_z = "1 [m/s] + 1 [1/s] * y"
turbulence_spec = "values"
k_value = 0.01
epsilon_value = 0.009
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "fixed"
turbulence = "wall"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = true
[turbulence]
model = "k_epsilon"
[solver]
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
{solver}"#
    )
}

/// The SA channel of `des_tree.rs` with the given DES mode and solver lines.
fn des_channel(des: &str, solver: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 8
ny = 20
lx = 2.0
ly = 1.0
periodic_x = true
[physics]
nu = 0.001
[initial]
velocity = [0.0, 0.0, 0.0]
velocity_expression_x = "4 [m/s] * (y / 1 [m]) * (1 - y / 1 [m])"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
convection = "upwind"
dt = 0.02
end_time = 1.0
{solver}
[turbulence]
model = "spalart_allmaras"
[turbulence.sa]
wall_mode = "resolved"
des = "{des}"
"#
    )
}

/// (name, case text, steps, registered record).
fn cases() -> Vec<(&'static str, String, usize, Record)> {
    vec![
        (
            "steady SIMPLEC k-epsilon channel",
            channel(KE, STEADY),
            300,
            (300, 7, 0xdb8f_ab77_6283_84d0),
        ),
        (
            "steady SIMPLEC realizable k-epsilon channel, enhanced wall function",
            channel(
                &format!("{KE}variant = \"realizable\"\nwall_function = \"enhanced\"\n"),
                STEADY,
            ),
            300,
            (300, 7, 0x629f_5830_7f83_bf41),
        ),
        (
            "steady SIMPLEC RNG k-epsilon channel, scalable wall function, hysteresis",
            channel(
                &format!("{KE}variant = \"rng\"\nwall_function = \"scalable\"\nwall_treatment = \"hysteresis\"\n"),
                STEADY,
            ),
            300,
            (300, 7, 0x8d6a_2b47_a356_8ecb),
        ),
        (
            "steady SIMPLEC k-epsilon sector with the frame",
            sector_kepsilon(STEADY),
            100,
            (100, 7, 0xc9cd_944d_e516_406b),
        ),
        (
            "steady SIMPLEC SA channel",
            channel(SA, STEADY),
            300,
            (300, 6, 0xfe68_bf9b_4023_52b6),
        ),
        (
            "steady SIMPLEC SA channel, auto_sa_law",
            channel(&format!("{SA}[turbulence.sa]\nwall_mode = \"auto_sa_law\"\n"), STEADY),
            300,
            (300, 6, 0x0e47_cb23_7b67_6340),
        ),
        (
            "SA-DES piso",
            des_channel("des", "algorithm = \"piso\""),
            20,
            (20, 6, 0xe59a_424a_6556_4884),
        ),
        (
            "SA-DDES pimple without a source (no hook)",
            des_channel("ddes", "algorithm = \"pimple\"\nn_outer = 2"),
            20,
            (20, 6, 0xf53c_f8ac_d9b6_ae13),
        ),
        (
            "transient SST channel on pimple (the F-286 path)",
            channel(
                SST,
                "algorithm = \"pimple\"\ntime_scheme = \"bdf2\"\nn_outer = 3\ndt = 0.1\nend_time = 100.0\n",
            ),
            20,
            (20, 7, 0xdada_2826_3c54_03a0),
        ),
    ]
}

/// V0b: every run bit-identical to its record on 765ac64.
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
