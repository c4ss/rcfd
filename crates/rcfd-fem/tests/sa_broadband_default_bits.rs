//! F-566 (ADR 0497) B1: the bits of the existing narrow-band and counting paths of `fatigue` and of `Moments::zero_crossing_rate`,
//! recorded at the base 9386de34 (plus the ADR commit) before any library line of F-566 was written. Uses only the API of the base.

use rcfd_fem::fatigue::{
    miner, narrow_band_damage_rate, narrow_band_damage_rate_curve, rainflow, rainflow_periodic, spectral_rate,
    MeanStressCorrection, SnCurve,
};
use rcfd_fem::random::Moments;

fn fnv(words: impl IntoIterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn history() -> Vec<f64> {
    (0..4000)
        .map(|i| {
            let t = i as f64 * 0.01;
            3.0 * (2.0 * std::f64::consts::PI * 1.3 * t).sin()
                + 1.1 * (2.0 * std::f64::consts::PI * 4.1 * t + 0.7).sin()
        })
        .collect()
}

fn digests() -> Vec<u64> {
    let mut out = Vec::new();
    let curve = SnCurve::power(3.0, 1e12).unwrap();
    for (m0, m2) in [(4.0e-7, 5.0e-3), (2.5, 9.0e3), (1.0, 1.0)] {
        let (rms, nu0) = spectral_rate(m0, m2).unwrap();
        out.push(fnv([rms.to_bits(), nu0.to_bits()]));
        let d = narrow_band_damage_rate(nu0, rms, 3.0, 1e12).unwrap();
        let dc = narrow_band_damage_rate_curve(nu0, rms, &curve).unwrap();
        out.push(fnv([d.to_bits(), dc.to_bits()]));
        let mut mo = Moments::default();
        mo.m0 = m0;
        mo.m1 = 0.5 * m2;
        mo.m2 = m2;
        mo.m4 = 40.0 * m2 * m2 / m0;
        out.push(fnv([mo.zero_crossing_rate().unwrap().to_bits(), mo.rms().to_bits()]));
    }
    let h = history();
    let c = rainflow(&h).unwrap();
    let cp = rainflow_periodic(&h).unwrap();
    let corr = MeanStressCorrection::none();
    let d = miner(&c.with_half_cycles(), &curve, &corr).unwrap();
    let dp = miner(&cp.cycles, &curve, &corr).unwrap();
    out.push(fnv(c
        .cycles
        .iter()
        .flat_map(|x| [x.range.to_bits(), x.mean.to_bits(), x.count.to_bits()])));
    out.push(fnv(cp
        .cycles
        .iter()
        .flat_map(|x| [x.range.to_bits(), x.mean.to_bits(), x.count.to_bits()])));
    out.push(fnv([d.to_bits(), dp.to_bits()]));
    out
}

#[test]
#[ignore]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

const BASE: [u64; 12] = [
    0xb3d1a3522cf94a7f,
    0xedac4a2d8c830621,
    0xaee7533ba89dfb87,
    0xbd0d44fd5e812cd9,
    0x818aaa0733e3d27d,
    0x4c6e781c06afd909,
    0x2be2cbea19a827c5,
    0xae470858316c164d,
    0x2be2cbea19a827c5,
    0xb0ec4f3c434446e3,
    0x08a3300c8f274351,
    0xebbee53268c7c087,
];

#[test]
fn b1_default_paths_unchanged() {
    let got = digests();
    assert_eq!(got.len(), BASE.len(), "digest count");
    assert_eq!(got, BASE.to_vec(), "B1: a default path changed its bits");
}
