//! Shared case texts of the ADR 0227 (F-302) case-tree tests, built on the CHX channel of ADR 0212 (`f296_common`): CHXP (CHX
//! laminar with the energy equation and a polynomial property viscosity), GCHXP (CHXP with `mesh.grading = 3`), CHXU (CHX laminar
//! with the user-function viscosity of the module UT), CHXUT (CHXU with the energy equation), CHXK and CHXA (CHX k-epsilon and
//! Spalart-Allmaras on `pimple`), LAMXP (LAMX of ADR 0212 on `pimple`), the modules UT and UX in WAT, the FNV-1a record of a text
//! with its case directory and the V0b'' records. Every test file that uses this module also declares `mod f296_common;`.

#![allow(dead_code)]

use crate::f296_common::{chx, fnv1a, lamx, Record, FIELDS, KE, LAMINAR, SA, SIMPLEC};
use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Module UT (ADR 0227 item 8): `nu = 0.0025 + 1e-4 speed + 1e-7 temperature` (the parameters are `x, y, z, t, temperature,
/// speed`).
pub const UT: &str = "(module (func (export \"rcfd_abi_version\") (result i32) i32.const 1) (func (export \"kinematic_viscosity\") (param f64 f64 f64 f64 f64 f64) (result f64) f64.const 0.0025 local.get 5 f64.const 1e-4 f64.mul f64.add local.get 4 f64.const 1e-7 f64.mul f64.add))";

/// Module UX (ADR 0227 item 8): `nu = 0.0025 + 1e-4 speed + 1e-4 x`.
pub const UX: &str = "(module (func (export \"rcfd_abi_version\") (result i32) i32.const 1) (func (export \"kinematic_viscosity\") (param f64 f64 f64 f64 f64 f64) (result f64) f64.const 0.0025 local.get 5 f64.const 1e-4 f64.mul f64.add local.get 0 f64.const 1e-4 f64.mul f64.add))";

/// The coefficients of the property viscosity of CHXP (`mu = -0.005 + 2.5e-5 T` Pa s).
pub const P_COEFFS: [f64; 2] = [-0.005, 2.5e-5];

/// The compiled bytes of a WAT module.
pub fn wasm(text: &str) -> Vec<u8> {
    wat::parse_str(text).unwrap()
}

/// The case directory of the texts: `v.wasm` (module UT), written once per test process.
pub fn case_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("rcfd_f302_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("v.wasm"), wasm(UT)).unwrap();
        d
    })
}

const YMIN: &str = "[boundaries.ymin]\nvelocity = \"fixed\"\n";
const YMAX: &str = "[boundaries.ymax]\nvelocity = \"fixed\"\n";

/// The energy lines of CHXP and CHXUT (placed after the model lines).
pub const ENERGY: &str = "[energy]\nenabled = true\nbuoyancy = false\n";
/// The property viscosity of CHXP.
pub const LAW_P: &str = "[properties.viscosity]\nlaw = \"polynomial\"\ncoefficients = [-0.005, 2.5e-5]\n";
/// The user-function viscosity of CHXU and CHXUT.
pub const UDF_V: &str = "[udf.v]\nfile = \"v.wasm\"\nviscosity = true\n";

/// The energy set-up of CHXP and CHXUT on a CHX text: `physics.conductivity = 0.05`, `physics.cp = 1`, `initial.temperature =
/// 300`, `ymin` a fixed temperature `300 + 20 sin(2 pi x)` K, `ymax` a fixed 300 K.
fn with_energy(t: String) -> String {
    assert!(
        t.contains(YMIN) && t.contains(YMAX) && t.contains("[initial]\n") && t.contains("rho = 1.0\n"),
        "the CHX text changed"
    );
    t.replace("[initial]\n", "[initial]\ntemperature = 300.0\n")
        .replace("rho = 1.0\n", "rho = 1.0\nconductivity = 0.05\ncp = 1.0\n")
        .replace(
            YMIN,
            &format!(
                "{YMIN}temperature = \"fixed\"\ntemperature_expression = \"300 [K] + 20 [K] * sin(6.283185307179586 * x / 1 [m])\"\n"
            ),
        )
        .replace(
            YMAX,
            &format!("{YMAX}temperature = \"fixed\"\ntemperature_value = 300.0\n"),
        )
}

/// CHXP; `extra` text after the property law.
pub fn chxp(extra: &str) -> String {
    with_energy(chx(LAMINAR, &format!("{ENERGY}{LAW_P}{extra}")))
}

/// GCHXP: CHXP with `mesh.grading = 3` (symmetric grading; the pair weights differ from 0.5 by a few ulps).
pub fn gchxp(extra: &str) -> String {
    let t = chxp(extra);
    assert!(t.contains("periodic_x = true\n"), "the CHX text changed");
    t.replace("periodic_x = true\n", "periodic_x = true\ngrading = 3.0\n")
}

/// CHXU; `extra` text after the module lines.
pub fn chxu(extra: &str) -> String {
    chx(LAMINAR, &format!("{UDF_V}{extra}"))
}

/// CHXUT: CHXU with the energy set-up of CHXP (no property law).
pub fn chxut(extra: &str) -> String {
    with_energy(chx(LAMINAR, &format!("{UDF_V}{ENERGY}{extra}")))
}

/// The pimple lines of CHXK, CHXA and LAMXP.
pub fn pimple_lines(scheme: &str) -> String {
    format!("algorithm = \"pimple\"\ntime_scheme = \"{scheme}\"\nn_outer = 3\ndt = 0.1\nend_time = 100.0\n")
}

/// A CHX-based text with its SIMPLEC lines replaced by the pimple lines.
pub fn to_pimple(text: &str, scheme: &str) -> String {
    assert!(text.contains(SIMPLEC), "the CHX text changed");
    text.replace(SIMPLEC, &pimple_lines(scheme))
}

/// CHXK: CHX k-epsilon on pimple; `extra` text after the model lines.
pub fn chxk(scheme: &str, extra: &str) -> String {
    to_pimple(&chx(KE, extra), scheme)
}

/// CHXA: CHX Spalart-Allmaras on pimple.
pub fn chxa(scheme: &str, extra: &str) -> String {
    to_pimple(&chx(SA, extra), scheme)
}

/// LAMXP: LAMX of ADR 0212 on pimple with Euler.
pub fn lamxp(extra: &str) -> String {
    to_pimple(&lamx(extra), "euler")
}

/// Build a simulation in the case directory, with the diagnostics in the panic message.
pub fn sim_in(text: &str) -> Simulation {
    let case = Case::parse_toml_in(text, case_dir()).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let errs = case.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}\n{text}");
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}\n{text}"))
}

/// `n` steps or iterations of the text in the case directory, then the record of its fields and face flux (as
/// `f296_common::record`).
pub fn record_in(text: &str, n: usize) -> Record {
    let mut sim = sim_in(text);
    let steps = sim.run(n);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let mut acc = 0xcbf2_9ce4_8422_2325_u64;
    let mut fields = 0;
    for f in FIELDS.iter().copied().chain(["T"]) {
        if let Ok(v) = sim.field(f) {
            fnv1a(&v, &mut acc);
            fields += 1;
        }
    }
    fnv1a(sim.flux(), &mut acc);
    (steps, fields, acc)
}

/// The V0b'' runs (ADR 0227): (name, case text with `extra` after the model lines, steps, registered record).
pub fn v0_cases(extra: &str) -> Vec<(&'static str, String, usize, Record)> {
    vec![
        ("CHXP", chxp(extra), 50, V0B[0]),
        ("GCHXP", gchxp(extra), 50, V0B[1]),
        ("CHXU", chxu(extra), 50, V0B[2]),
        ("CHXUT", chxut(extra), 50, V0B[3]),
        ("CHXK euler", chxk("euler", extra), 10, V0B[4]),
        ("CHXK bdf2", chxk("bdf2", extra), 10, V0B[5]),
        ("CHXA euler", chxa("euler", extra), 10, V0B[6]),
        ("CHXA bdf2", chxa("bdf2", extra), 10, V0B[7]),
        ("LAMXP", lamxp(extra), 10, V0B[8]),
    ]
}

/// The V0b'' records, in the order of [`v0_cases`], recorded on main 9f95fb12 with the ADR 0227 registration and the test files
/// only, before any code of F-302.
pub const V0B: [Record; 9] = [
    // CHXP
    (50, 5, 0x4fed_a236_8847_708c),
    // GCHXP
    (50, 5, 0x3dd1_7d17_fbb0_4550),
    // CHXU
    (50, 4, 0xdb69_c17d_9140_3384),
    // CHXUT
    (50, 5, 0xf255_d266_d551_acaf),
    // CHXK euler
    (10, 7, 0xdcb4_96a6_f63c_8273),
    // CHXK bdf2
    (10, 7, 0x7dd8_9fbd_cd8c_979d),
    // CHXA euler
    (10, 6, 0x9525_367e_c22c_ff7f),
    // CHXA bdf2
    (10, 6, 0xf475_8e19_b0d6_a7cd),
    // LAMXP
    (10, 7, 0xacc0_5f0e_5534_153d),
];
