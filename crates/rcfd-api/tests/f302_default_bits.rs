//! ADR 0227 V0b'' (F-302): the runs whose code paths the seam-diffusivity change of the property and user-function viscosities
//! passes through, and the k-epsilon and Spalart-Allmaras pimple texts of its checks, hashed bit for bit. FNV-1a (as
//! `f296_common::record`) of every cell field the run has among `ux, uy, uz, p, k, omega, nuTilda, epsilon, nut, T`, then of the
//! face flux. The records (`f302_common::V0B`) were taken on main 9f95fb12 with the ADR 0227 registration and the test files
//! only, before any code of F-302, and must stay bit-identical after it.

mod f296_common;
mod f302_common;

use f302_common::{record_in, v0_cases};

/// V0b'': every run bit-identical to its record.
#[test]
fn v0b_default_paths_are_bit_identical_to_main() {
    let mut bad = Vec::new();
    for (name, text, n, want) in v0_cases("") {
        let got = record_in(&text, n);
        println!(
            "V0b'' {name}: (steps, fields, hash) = ({}, {}, 0x{:016x})",
            got.0, got.1, got.2
        );
        if got != want {
            bad.push(format!(
                "{name}: got ({}, {}, 0x{:016x}), recorded ({}, {}, 0x{:016x})",
                got.0, got.1, got.2, want.0, want.1, want.2
            ));
        }
    }
    assert!(bad.is_empty(), "V0b'': {bad:#?}");
}
