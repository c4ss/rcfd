//! ADR 0468 (F-418) DB1: digests of the `rcfd_post::acoustics` paths that `solid_acoustic` calls, printed at main 9aab52ae
//! before any library code of F-418 and kept as constants (the default paths stay bit-identical).

use rcfd_post::acoustics::{
    band_index, level_from_ms, nominal_frequency, sphere_quadrature, third_octave_bands, Spectrum,
};

fn fnv(acc: &mut u64, bits: u64) {
    for b in bits.to_le_bytes() {
        *acc ^= b as u64;
        *acc = acc.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn digest(values: &[f64]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut h, values.len() as u64);
    for v in values {
        fnv(&mut h, v.to_bits());
    }
    h
}

fn digests() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    // band_index and nominal_frequency over bands -30..20 and frequencies between the centres
    let mut v = Vec::new();
    for n in -30..=20 {
        let f = 1000.0 * 10f64.powf(0.1 * n as f64);
        v.push(nominal_frequency(n));
        for s in [0.97, 1.0, 1.03, 1.12] {
            v.push(band_index(f * s).unwrap() as f64);
        }
    }
    out.push(("band_index_nominal", digest(&v)));
    // third_octave_bands of a synthetic spectrum
    let df = 3.7;
    let freq: Vec<f64> = (0..=2048).map(|k| k as f64 * df).collect();
    let ms: Vec<f64> = (0..=2048)
        .map(|k| 1e-6 * (1.0 + (k as f64 * 0.013).sin().powi(2)) / (1.0 + k as f64 * 0.01))
        .collect();
    let spec = Spectrum {
        df,
        freq,
        ms,
        n_used: 4096,
    };
    let bands = third_octave_bands(&spec, 20.0, 7000.0).unwrap();
    let mut v = Vec::new();
    for b in &bands {
        v.extend([b.n as f64, b.nominal, b.centre, b.lo, b.hi, b.ms]);
    }
    out.push(("third_octave_bands", digest(&v)));
    // sphere_quadrature(48, 96)
    let mut v = Vec::new();
    for (d, w) in sphere_quadrature(48, 96).unwrap() {
        v.extend([d[0], d[1], d[2], w]);
    }
    out.push(("sphere_quadrature_48_96", digest(&v)));
    // level_from_ms
    let v: Vec<f64> = [4e-10, 1e-6, 0.37, 1.0, 123.0]
        .iter()
        .map(|&m| level_from_ms(m).unwrap().unwrap())
        .collect();
    out.push(("level_from_ms", digest(&v)));
    out
}

/// Printed at main 9aab52ae by this file before any library code of F-418.
const BASE: [(&str, u64); 4] = [
    ("band_index_nominal", 0x5d96824c9a2569f4),
    ("third_octave_bands", 0x30fe1d704c072e11),
    ("sphere_quadrature_48_96", 0xa2fc9f445a6a2e39),
    ("level_from_ms", 0xea7b2362bbb6cc43),
];

#[test]
fn db1_acoustics_paths_unchanged() {
    let got = digests();
    let mut equal = 0;
    for ((name, d), (bname, bd)) in got.iter().zip(BASE.iter()) {
        println!("DB1 {name} {d:#018x}");
        assert_eq!(name, bname);
        if d == bd {
            equal += 1;
        }
    }
    println!("DB1 {equal} of {} digests equal to the base", BASE.len());
    assert_eq!(equal, BASE.len());
}
