//! ADR 0205 V0b (F-295): the runs whose code paths the seam-diffusivity change passes through, hashed bit for bit. FNV-1a (as
//! `f286_default_bits.rs`) of every cell field the run has among `ux, uy, uz, p, k, omega, nuTilda, epsilon, nut`, then of the
//! face flux. The values below were recorded on main 9631754 with the ADR 0205 registration and this file only, before any code
//! of F-295, and must stay bit-identical after it.

mod f295_common;

use f295_common::{b2s, ch, lam, record, s4, Record, CH_SST, PIMPLE_CH, SIMPLEC};

/// (name, case text, steps, registered record).
fn cases() -> Vec<(&'static str, String, usize, Record)> {
    vec![
        (
            "CH pimple SST (translational seam)",
            ch("", "", PIMPLE_CH),
            20,
            (20, 7, 0xdada_2826_3c54_03a0),
        ),
        (
            "S4 sector pimple SST (rotational seam)",
            s4(""),
            5,
            (5, 7, 0xabd8_3aa0_fd75_0ae7),
        ),
        (
            "LAM SIMPLEC SST, laminar region at the seam",
            lam(""),
            100,
            (100, 7, 0xd8e3_6629_4f57_0e5e),
        ),
        (
            "CH SIMPLEC k-epsilon",
            ch("", "", SIMPLEC).replace(CH_SST, "model = \"k_epsilon\"\n"),
            100,
            (100, 7, 0x051b_deb2_85ab_7c4a),
        ),
        (
            "B2S SIMPLEC SST (FAN-01 passage)",
            b2s(""),
            5,
            (5, 7, 0x85db_6064_d777_7a63),
        ),
    ]
}

/// V0b: every run bit-identical to its record on 9631754.
#[test]
fn v0b_default_paths_are_bit_identical_to_main() {
    let mut bad = Vec::new();
    for (name, text, n, want) in cases() {
        let got = record(&text, n);
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
