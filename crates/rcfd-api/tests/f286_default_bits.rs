//! ADR 0199 V0b (F-286): the runs whose code paths the transient SST change passes through, hashed bit for bit. FNV-1a (as
//! `baseline_hashes.rs`, 64 bit, over the little-endian bytes of `f64::to_bits`) of every cell field the run has among `ux, uy,
//! uz, p, k, omega, nuTilda, epsilon, nut`, then of the face flux. The values below were recorded on main 9edfbda with the ADR
//! 0199 registration and this file only, before any code of F-286, and must stay bit-identical after it.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const FIELDS: [&str; 9] = ["ux", "uy", "uz", "p", "k", "omega", "nuTilda", "epsilon", "nut"];

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

/// The R1 channel of ADR 0199 with the given solver block.
fn channel(solver: &str) -> String {
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
model = "k_omega_sst"
k_initial = 1.0
omega_initial = 40.0
[solver]
convection = "upwind"
{solver}"#
    )
}

/// The ADR 0183 PC-3 sector; `turbulence` adds k-omega SST (the S4 text of ADR 0199), `solver` is the solver block.
fn sector(turbulence: bool, solver: &str) -> String {
    let (inlet_t, outer_t, model) = if turbulence {
        (
            "turbulence_spec = \"values\"\nk_value = 0.01\nomega_value = 10.0\n",
            "turbulence = \"wall\"\n",
            "[turbulence]\nmodel = \"k_omega_sst\"\n",
        )
    } else {
        ("", "", "")
    };
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
{inlet_t}[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "moving_wall"
wall_rotation_rate = -2.0
wall_velocity_projection = "tangential"
{outer_t}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = true
{model}[solver]
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
{solver}"#
    )
}

/// The SA-DDES channel of `des_tree.rs`; `extra` is appended.
fn ddes(solver: &str, extra: &str) -> String {
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
des = "ddes"
{extra}"#
    )
}

const RSM_BOX: &str = r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 4
lx = 1.0
ly = 1.0
periodic_x = true
periodic_y = true
[physics]
nu = 0.001
[initial]
velocity = [0.0, 0.0, 0.0]
[solver]
algorithm = "piso"
convection = "upwind"
dt = 0.0001
end_time = 1.0
[turbulence]
model = "rsm"
k_initial = 1.0
epsilon_initial = 1.0
"#;

const TG: &str = r#"
[mesh]
generator = "box_2d"
nx = 16
ny = 16
lx = 6.283185307179586
ly = 6.283185307179586
periodic_x = true
periodic_y = true
[physics]
nu = 0.05
[initial]
velocity = [0.0, 0.0, 0.0]
velocity_expression_x = "1 [m/s] * sin(x / 1 [m]) * cos(y / 1 [m])"
velocity_expression_y = "-1 [m/s] * cos(x / 1 [m]) * sin(y / 1 [m])"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.01
end_time = 1.0
[turbulence]
model = "smagorinsky"
"#;

const PC2B: &str = r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 40
lx = 0.4
ly = 1.0
periodic_x = true
[physics]
nu = 0.1
rho = 1.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[sources.drive]
type = "acceleration"
acceleration = [1.0, 0.0, 0.0]
zone = "all"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = false
[solver]
algorithm = "pimple"
convection = "upwind"
time_scheme = "euler"
n_outer = 2
dt = 0.01
end_time = 1000.0
momentum_rel_tol = 1e-9
pressure_rel_tol = 1e-9
pressure_max_iter = 1000
"#;

const PIMPLE_SECTOR: &str =
    "algorithm = \"pimple\"\ntime_scheme = \"bdf2\"\nn_outer = 3\nn_correctors = 2\ndt = 0.05\nend_time = 100.0\n";

/// (steps, fields hashed, hash).
type Record = (usize, usize, u64);

/// (name, case text, steps, registered record).
fn cases() -> Vec<(&'static str, String, usize, Record)> {
    let drive = "[sources.drive]\ntype = \"acceleration\"\nacceleration = [1.0, 0.0, 0.0]\nzone = \"all\"\n";
    vec![
        (
            "steady SIMPLEC SST channel",
            channel("algorithm = \"simplec\"\nmax_iterations = 1000\n"),
            300,
            (300, 7, 0xe0c0_6d93_f89a_6026),
        ),
        (
            "steady SIMPLEC SST sector with the frame",
            sector(true, "algorithm = \"simplec\"\nmax_iterations = 1000\n"),
            100,
            (100, 7, 0xc2f1_6998_92d6_3041),
        ),
        (
            "SA-DDES piso",
            ddes("algorithm = \"piso\"", ""),
            20,
            (20, 6, 0x5815_ab1e_8181_474b),
        ),
        (
            "SA-DDES pimple with a drive (hook path)",
            ddes("algorithm = \"pimple\"\nn_outer = 2", drive),
            20,
            (20, 6, 0xb22c_d2c4_9440_a812),
        ),
        (
            "RSM decay piso",
            RSM_BOX.to_string(),
            20,
            (20, 7, 0xc39e_0dbb_60c1_81bc),
        ),
        (
            "Smagorinsky Taylor-Green piso",
            TG.to_string(),
            20,
            (20, 5, 0x50bf_d098_92f5_9843),
        ),
        (
            "laminar pimple sector with the frame",
            sector(false, PIMPLE_SECTOR),
            5,
            (5, 4, 0x0173_7054_b519_21c3),
        ),
        (
            "laminar pimple channel with drive and frame",
            PC2B.to_string(),
            50,
            (50, 4, 0x8346_7df6_f8e6_6875),
        ),
    ]
}

/// V0b: every run bit-identical to its record on 9edfbda.
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
