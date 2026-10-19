//! Wave 34C (ADR 0074 W1): `solver.component_diagonal` reaches the transient PISO/PIMPLE kernel.
//!
//! Before ADR 0074 the key was applied to the steady and variable-density kernels only, although `docs/case-schema.md` documented it for the
//! transient constant-density kernel too. Checks on the packed bed (`cases/packed_bed`, symmetry walls, exact u_y = 0) run with PISO,
//! dt = 0.01, 100 steps: (W1a) without the key and with `averaged` the fields are bit-identical; (W1b) `corrected` changes the fields;
//! (W1c) max |u_y| with `corrected` is not larger than with `averaged`.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn packed_bed_piso(option: &str) -> Simulation {
    let path = format!("{}/../../cases/packed_bed/packed_bed.toml", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path)
        .expect("case file readable")
        .replace("\r\n", "\n");
    let text = text
        .replace("algorithm = \"simplec\"", "algorithm = \"piso\"")
        .replacen(
            "[solver]\n",
            &format!("[solver]\n{option}dt = 0.01\nend_time = 1.0\n"),
            1,
        );
    Simulation::new(&Case::parse_toml(&text).expect("case parses")).expect("builds")
}

fn fields(sim: &Simulation) -> Vec<Vec<f64>> {
    ["ux", "uy", "p"].iter().map(|f| sim.field(f).unwrap()).collect()
}

#[test]
fn w1_component_diagonal_key_reaches_the_transient_kernel() {
    let mut none = packed_bed_piso("");
    let mut avg = packed_bed_piso("component_diagonal = \"averaged\"\n");
    let mut cor = packed_bed_piso("component_diagonal = \"corrected\"\n");
    let steps = [none.run(100), avg.run(100), cor.run(100)];
    assert_eq!(steps, [100, 100, 100], "steps {steps:?}");
    let (fn_, fa, fc) = (fields(&none), fields(&avg), fields(&cor));
    // W1a: the default is the averaged split, bit for bit
    for (a, b) in fn_.iter().zip(&fa) {
        assert!(
            a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()),
            "W1a: default differs from averaged"
        );
    }
    // W1b: the corrected split changes the solution
    let diff = fa[0].iter().zip(&fc[0]).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    let uy = |f: &Vec<Vec<f64>>| f[1].iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    println!(
        "W1: max |ux_corrected - ux_averaged| {diff:e}; max |uy| averaged {:e}, corrected {:e}",
        uy(&fa),
        uy(&fc)
    );
    assert!(diff > 0.0, "W1b: the key has no effect on the transient kernel");
    // W1c: the spurious wall-normal velocity does not grow
    assert!(uy(&fc) <= uy(&fa), "W1c");
}
