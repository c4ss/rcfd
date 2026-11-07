//! Shared case texts of the ADR 0212 (F-296) case-tree tests: CHX (the CH channel of ADR 0205 with an initial velocity that
//! varies in x, so that the seam pairs differ), CHXR (CHX laminar with a power-law viscosity), TGS (the doubly periodic
//! Taylor-Green box of `f286_default_bits.rs` with the vortex shifted by 0.5 in x), LAMX (CHX k-epsilon with the laminar region of
//! ADR 0205 LAM) and NPK (CHX k-epsilon without its seam), with the FNV-1a field record of `f295_default_bits.rs` and the V0b'
//! records.

#![allow(dead_code)]

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

/// The cell fields a record hashes (as `f295_default_bits.rs`).
pub const FIELDS: [&str; 9] = ["ux", "uy", "uz", "p", "k", "omega", "nuTilda", "epsilon", "nut"];

/// The kinematic viscosity of CHX, CHXR, LAMX and NPK (`1/395`; the bits of the text's `0.0025316455696202532`).
pub const NU_CH: f64 = 0.002_531_645_569_620_253;

/// FNV-1a (64 bit) over the little-endian bytes of `f64::to_bits`, as `baseline_hashes.rs`.
pub fn fnv1a(values: &[f64], acc: &mut u64) {
    for x in values {
        for b in x.to_bits().to_le_bytes() {
            *acc ^= u64::from(b);
            *acc = acc.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// (steps taken, fields hashed, hash).
pub type Record = (usize, usize, u64);

/// Build a simulation, with the diagnostics in the panic message.
pub fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let errs = case.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}\n{text}");
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}\n{text}"))
}

/// `n` steps or iterations of the text, then the record of its fields and face flux.
pub fn record(text: &str, n: usize) -> Record {
    let mut sim = sim_of(text);
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

/// `[turbulence.periodic_seam]` with the given diffusivity (ADR 0205 D3).
pub fn seam(diffusivity: &str) -> String {
    format!("[turbulence.periodic_seam]\ndiffusivity = \"{diffusivity}\"\n")
}

/// The turbulence lines of the models.
pub const KE: &str = "model = \"k_epsilon\"\n";
pub const SA: &str = "model = \"spalart_allmaras\"\n";
pub const ML: &str = "model = \"mixing_length\"\nlength_scale = 1.0\n";
pub const LAMINAR: &str = "model = \"laminar\"\n";
pub const SMAG: &str = "model = \"smagorinsky\"\n";
pub const WALE: &str = "model = \"wale\"\n";
pub const RSM: &str = "model = \"rsm\"\nk_initial = 1.0\nepsilon_initial = 1.0\n";

/// The viscosity law of CHXR.
pub const LAW: &str = "[rheology]\nmodel = \"power_law\"\nconsistency_index = 0.0025\npower_index = 0.6\n";
/// The viscosity law of the TGS record with the law (V0b').
pub const LAW_TGS: &str = "[rheology]\nmodel = \"power_law\"\nconsistency_index = 0.05\npower_index = 0.6\n";

/// SIMPLEC lines of CHX.
pub const SIMPLEC: &str = "algorithm = \"simplec\"\nmax_iterations = 1000\n";
/// The piso lines of TGS.
pub const PISO: &str = "algorithm = \"piso\"\nconvection = \"central\"\n";
/// The pimple lines of TGS.
pub const PIMPLE: &str = "algorithm = \"pimple\"\nn_outer = 3\ntime_scheme = \"euler\"\nconvection = \"central\"\n";

/// CHX: the CH channel of ADR 0205 (box_2d 4 x 20, periodic in x, drive (1, 0, 0), upwind, SIMPLEC) with
/// `velocity_expression_x = 15 + 3 sin(2 pi x)`; `turb` the model lines, `extra` text after them (before `[solver]`).
pub fn chx(turb: &str, extra: &str) -> String {
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
velocity_expression_x = "15 [m/s] + 3 [m/s] * sin(6.283185307179586 * x / 1 [m])"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[sources.drive]
type = "acceleration"
acceleration = [1.0, 0.0, 0.0]
zone = "all"
[turbulence]
{turb}{extra}[solver]
convection = "upwind"
{SIMPLEC}"#
    )
}

/// CHXR: CHX laminar with the power-law viscosity [`LAW`]; `extra` text after the law.
pub fn chxr(extra: &str) -> String {
    chx(LAMINAR, &format!("{LAW}{extra}"))
}

/// TGS: the doubly periodic Taylor-Green box of `f286_default_bits.rs` (16 x 16, 2 pi, nu = 0.05, dt = 0.01) with the vortex
/// shifted by 0.5 in x; `solver` the algorithm lines, `turb` the model lines, `extra` text after them.
pub fn tgs(turb: &str, extra: &str, solver: &str) -> String {
    format!(
        r#"
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
velocity_expression_x = "1 [m/s] * sin(x / 1 [m] + 0.5) * cos(y / 1 [m])"
velocity_expression_y = "-1 [m/s] * cos(x / 1 [m] + 0.5) * sin(y / 1 [m])"
[solver]
{solver}dt = 0.01
end_time = 1.0
[turbulence]
{turb}{extra}"#
    )
}

/// LAMX: CHX k-epsilon with the laminar region of ADR 0205 LAM (the first column of cells, the owners of the `xmin` seam faces);
/// `extra` text before the region.
pub fn lamx(extra: &str) -> String {
    chx(
        KE,
        &format!(
            "{extra}[regions.lam]\nzone = \"box\"\nbox_min = [-1.0, -1.0, -1.0]\nbox_max = [0.25, 3.0, 1.0]\nturbulence = \"laminar\"\n"
        ),
    )
}

/// NPK: CHX k-epsilon without its seam and drive: a fixed inlet `(15, 0, 0)` with `turbulence_length = 0.14` at `xmin`, a
/// fixed-pressure outlet at `xmax`; `extra` text after the model lines.
pub fn npk(extra: &str) -> String {
    let t = chx(KE, extra);
    let drive = "[sources.drive]\ntype = \"acceleration\"\nacceleration = [1.0, 0.0, 0.0]\nzone = \"all\"\n";
    let ymin = "[boundaries.ymin]\n";
    assert!(
        t.contains(drive) && t.contains("periodic_x = true\n") && t.contains(ymin),
        "the CHX text changed"
    );
    let ends = "[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [15.0, 0.0, 0.0]\nturbulence_length = 0.14\n[boundaries.xmax]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n";
    t.replace(drive, "")
        .replace("periodic_x = true\n", "periodic_x = false\n")
        .replace(ymin, &format!("{ends}{ymin}"))
}

/// The V0b' runs (ADR 0212): (name, case text with `extra` after the model lines, steps, registered record).
pub fn v0_cases(extra: &str) -> Vec<(&'static str, String, usize, Record)> {
    vec![
        ("CHX k_epsilon", chx(KE, extra), 50, V0B[0]),
        ("CHX spalart_allmaras", chx(SA, extra), 50, V0B[1]),
        ("CHX mixing_length", chx(ML, extra), 50, V0B[2]),
        ("CHXR power law", chxr(extra), 50, V0B[3]),
        ("TGS smagorinsky piso", tgs(SMAG, extra, PISO), 20, V0B[4]),
        ("TGS wale pimple", tgs(WALE, extra, PIMPLE), 10, V0B[5]),
        ("TGS rsm piso", tgs(RSM, extra, PISO), 20, V0B[6]),
        (
            "TGS laminar power law piso",
            tgs(LAMINAR, &format!("{LAW_TGS}{extra}"), PISO),
            10,
            V0B[7],
        ),
        ("LAMX k_epsilon", lamx(extra), 50, V0B[8]),
        ("NPK k_epsilon", npk(extra), 50, V0B[9]),
    ]
}

/// The V0b' records, in the order of [`v0_cases`], recorded on main 54167b0 with the ADR 0212 registration (c9c099f) and the
/// test files only, before any code of F-296.
pub const V0B: [Record; 10] = [
    // CHX k_epsilon
    (50, 7, 0xe527_a1d3_5ffe_4343),
    // CHX spalart_allmaras
    (50, 6, 0x78ef_cb6e_2d32_2296),
    // CHX mixing_length
    (50, 5, 0x90c0_c54c_9ddd_a72c),
    // CHXR power law
    (50, 4, 0xb923_4b3a_11e7_5d52),
    // TGS smagorinsky piso
    (20, 5, 0xd096_c2c5_6282_5b75),
    // TGS wale pimple
    (10, 5, 0x4ba5_616d_29ee_4494),
    // TGS rsm piso
    (20, 7, 0x67da_49ac_2a52_dfaa),
    // TGS laminar power law piso
    (10, 4, 0xeb0f_c754_2857_75f2),
    // LAMX k_epsilon
    (50, 7, 0xfe1d_4be4_f9ef_cf72),
    // NPK k_epsilon
    (50, 7, 0xaacd_2067_2fc8_6114),
];
