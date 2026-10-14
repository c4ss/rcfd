//! ADR 0010: `solver.component_diagonal = "corrected"` (opt-in; the default `averaged` keeps every shipped result
//! bit-identical, see `baseline_hashes`).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn packed_bed_text(option: &str) -> String {
    let path = format!("{}/../../cases/packed_bed/packed_bed.toml", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("case file readable");
    text.replace("\r\n", "\n")
        .replacen("[solver]\n", &format!("[solver]\n{option}"), 1)
}

fn packed_bed(option: &str) -> Case {
    Case::parse_toml(&packed_bed_text(option)).expect("case parses")
}

/// The packed bed (symmetry side walls, uniform Ergun flow) stalls at the iteration cap (1700) with the averaged
/// diagonal and converges in 16 iterations with the corrected one; uy (exact value 0) is halved.
#[test]
fn packed_bed_converges_with_the_corrected_diagonal() {
    let mut sim = Simulation::new(&packed_bed("component_diagonal = \"corrected\"\n")).expect("builds");
    let n = sim.run(200);
    assert!(n <= 50, "corrected diagonal: {n} iterations");
    let uy = sim.field("uy").unwrap().iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    assert!(uy < 6e-7, "max |uy| {uy}");
}

#[test]
fn averaged_is_the_default_and_accepted_explicitly() {
    let mut a = Simulation::new(&packed_bed("")).expect("builds");
    let mut b = Simulation::new(&packed_bed("component_diagonal = \"averaged\"\n")).expect("builds");
    assert_eq!(a.run(20), b.run(20));
    assert_eq!(a.field("p").unwrap(), b.field("p").unwrap());
}

#[test]
fn unknown_value_is_rejected() {
    let text = packed_bed_text("component_diagonal = \"best\"\n");
    assert!(Case::parse_toml(&text).is_err());
}
