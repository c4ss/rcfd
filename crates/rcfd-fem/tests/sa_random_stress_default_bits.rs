//! F-589 (ADR 0516) D6, check B1: the bits of the existing random-stress and channel-response fields (the covariance `C_0` and
//! `C_2` of `RandomStress` and of `ChannelResponse`, the moments, the densities, the grid), and of the F-566 and F-413 rates,
//! recorded on main d41012db before any library line of F-589 was written. Uses only the API of that commit.

use rcfd_fem::builders::hex_box;
use rcfd_fem::fatigue::{broadband_damage_rate, narrow_band_damage_rate};
use rcfd_fem::harmonic::{HarmonicOptions, ModalBasis, ModalDamping};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::random::{
    spectral_response, spectral_response_cross, ChannelResponse, Coherence, CrossSpectrum, Excitation, ModalChannels,
    RandomRequest, RandomResponse, RandomSolver, SpectralOptions, Spectrum,
};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

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

fn bits(v: &[f64]) -> impl Iterator<Item = u64> + '_ {
    v.iter().map(|x| x.to_bits())
}

fn channel_words(r: &ChannelResponse) -> Vec<u64> {
    let mut words: Vec<u64> = Vec::new();
    words.extend(bits(&r.frequencies));
    words.extend(bits(&r.weights));
    for p in &r.psd {
        words.extend(bits(p));
    }
    for m in &r.moments {
        words.extend([m.m0.to_bits(), m.m1.to_bits(), m.m2.to_bits(), m.m4.to_bits()]);
    }
    for c in r.covariance.iter().chain(&r.rate_covariance) {
        words.extend(bits(c));
    }
    words
}

fn response_words(r: &RandomResponse) -> Vec<u64> {
    let mut words: Vec<u64> = Vec::new();
    words.extend(bits(&r.frequencies));
    words.extend(bits(&r.weights));
    for p in &r.psd {
        words.extend(bits(p));
    }
    for m in &r.moments {
        words.extend([m.m0.to_bits(), m.m1.to_bits(), m.m2.to_bits(), m.m4.to_bits()]);
    }
    if let Some(s) = &r.stress {
        for c in s.covariance.iter().chain(&s.rate_covariance) {
            for row in c {
                words.extend(bits(row));
            }
        }
        for p in &s.positions {
            words.extend(bits(p));
        }
    }
    words
}

fn spectra() -> Vec<Spectrum> {
    vec![
        Spectrum::flat(10.0, 900.0, 2.0).unwrap(),
        Spectrum::new(vec![20.0, 80.0, 400.0, 700.0], vec![1.0, 3.0, 3.0, 0.5]).unwrap(),
        Spectrum::log_log(vec![30.0, 120.0, 600.0], vec![0.5, 4.0, 1.0]).unwrap(),
    ]
}

fn channels() -> ModalChannels {
    let freqs = [60.0_f64, 140.0, 310.0];
    let eig: Vec<f64> = freqs.iter().map(|f| (2.0 * PI * f).powi(2)).collect();
    let damp: Vec<f64> = freqs.iter().map(|f| 2.0 * 0.02 * 2.0 * PI * f).collect();
    let gains: Vec<Vec<Vec<f64>>> = (0..3)
        .map(|a| {
            (0..6)
                .map(|o| {
                    (0..3)
                        .map(|r| 1e3 * (0.3 + 0.1 * a as f64 - 0.2 * o as f64 + 0.35 * r as f64 * (a as f64 - 1.0)))
                        .collect()
                })
                .collect()
        })
        .collect();
    let res: Vec<Vec<f64>> = (0..3)
        .map(|a| (0..6).map(|o| 1e-3 * (a as f64 + 0.5 * o as f64)).collect())
        .collect();
    ModalChannels::new(eig, damp, gains, res).unwrap()
}

fn steel() -> Material {
    Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap()
}

fn bar(nu: f64) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [40, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, nu, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn digests() -> Vec<u64> {
    let mut out = Vec::new();
    let sp = spectra();
    let ch = channels();
    let rho = [vec![1.0, 0.6, -0.3], vec![0.6, 1.0, 0.2], vec![-0.3, 0.2, 1.0]];
    // spectral_response, groups 1, 3 and 6, uncorrelated and correlated.
    for corr in [None, Some(&rho[..])] {
        for group in [1, 3, 6] {
            let r = spectral_response(&ch, &sp, corr, &SpectralOptions::default().group(group)).unwrap();
            out.push(fnv(channel_words(&r)));
        }
    }
    // spectral_response_cross: the general loop on a constant real coherence and on a complex pure delay.
    let flat3 = vec![Spectrum::flat(10.0, 900.0, 2.0).unwrap(); 3];
    let real = CrossSpectrum::coherent(flat3.clone(), Coherence::constant(rho.to_vec()).unwrap()).unwrap();
    let delay = CrossSpectrum::coherent(
        flat3,
        Coherence::function(3, |f, a, b| {
            let phase = -2.0 * PI * f * 1.5e-3 * (b - a) as f64;
            (phase.cos(), phase.sin())
        }),
    )
    .unwrap();
    for cs in [&real, &delay] {
        for group in [1, 3] {
            let r =
                spectral_response_cross(&ch, cs, &SpectralOptions::default().group(group).real_route(false)).unwrap();
            out.push(fnv(channel_words(&r)));
        }
    }
    // The one-mode bar, nu = 0.3, force and base, with stress.
    let m = bar(0.3);
    let res = modal_analysis(
        &m,
        &ModalOptions::new(1)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let basis = ModalBasis::from_result(&res).unwrap();
    let f1 = basis.eigenvalues()[0].sqrt() / (2.0 * PI);
    let solver = RandomSolver::new(
        &m,
        basis,
        &HarmonicOptions::new(ModalDamping::Ratio(0.02))
            .static_correction(false)
            .threads(1),
    )
    .unwrap();
    let dofs = m.dof_map();
    let eqs: Vec<usize> = m
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&p| dofs.equation(p, 0).unwrap())
        .collect();
    let mut f = vec![0.0; dofs.n_equations()];
    for &e in &eqs {
        f[e] = 0.25;
    }
    let spec = Spectrum::flat(0.0, 10.0 * f1, 1.0).unwrap();
    let req = RandomRequest::new(vec![eqs[0]]).stress(true);
    for ex in [
        Excitation::force(f, spec.clone()),
        Excitation::base([1.0, 0.0, 0.0], spec).unwrap(),
    ] {
        out.push(fnv(response_words(&solver.response(&m, &ex, &req).unwrap())));
    }
    // The cantilever of ADR 0510, 8 modes, static correction on, force patterns uncorrelated and correlated, base.
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    let nodes = mesh.nodes().to_vec();
    let mut c = SolidModel::new(mesh);
    c.add_material("all", steel()).unwrap();
    for k in 0..3 {
        c.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    let dofs = c.dof_map();
    let find = |p: [f64; 3]| nodes.iter().position(|q| *q == p).unwrap();
    let (a, b) = (find([1.0, 0.1, 0.08]), find([1.0, 0.0, 0.0]));
    let n = dofs.n_equations();
    let mut p1 = vec![0.0; n];
    let mut p2 = vec![0.0; n];
    p1[dofs.equation(a, 1).unwrap()] = 1.0;
    p2[dofs.equation(b, 1).unwrap()] = 1.0;
    let oeq: Vec<usize> = [a, b]
        .iter()
        .flat_map(|&v| (0..3).map(move |k| (v, k)))
        .map(|(v, k)| dofs.equation(v, k).unwrap())
        .collect();
    let res = modal_analysis(
        &c,
        &ModalOptions::new(8)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let csolver = RandomSolver::new(
        &c,
        ModalBasis::from_result(&res).unwrap(),
        &HarmonicOptions::new(ModalDamping::Ratio(0.02)).threads(1),
    )
    .unwrap();
    let g = Spectrum::flat(5.0, 2000.0, 1.0).unwrap();
    let g2 = Spectrum::new(vec![5.0, 300.0, 2000.0], vec![1.0, 4.0, 0.5]).unwrap();
    let one = vec![vec![1.0, 0.7], vec![0.7, 1.0]];
    let excitations = [
        Excitation::forces(vec![p1.clone(), p2.clone()], vec![g.clone(), g2.clone()], None).unwrap(),
        Excitation::forces(vec![p1.clone(), p2.clone()], vec![g.clone(), g2.clone()], Some(one)).unwrap(),
        Excitation::base([0.0, 1.0, 0.0], g.clone()).unwrap(),
        Excitation::cross(
            vec![p1, p2],
            CrossSpectrum::coherent(
                vec![g.clone(), g2],
                Coherence::function(2, |f, _, _| {
                    let phase = -2.0 * PI * f * 1.0e-3;
                    (0.8 * phase.cos(), 0.8 * phase.sin())
                }),
            )
            .unwrap(),
        )
        .unwrap(),
    ];
    for ex in &excitations {
        let r = csolver
            .response(&c, ex, &RandomRequest::new(oeq.clone()).stress(true))
            .unwrap();
        out.push(fnv(response_words(&r)));
    }
    // The rates on fixed moments.
    let (m0, m1, m2, m4) = (
        39.269_570_796_526_09,
        50.0 * 38.772_580_668_507_95,
        2500.0 * 39.169_235_991_861_99,
        6.25e6 * 49.006_230_409_610_886,
    );
    let bb = broadband_damage_rate(m0, m1, m2, m4, 3.0, 1.0).unwrap();
    let nb = narrow_band_damage_rate((m2 / m0).sqrt(), m0.sqrt(), 3.0, 1.0).unwrap();
    out.push(fnv([bb.to_bits(), nb.to_bits()]));
    out
}

/// Prints the digests (run at main d41012db to record BASE).
#[test]
#[ignore = "prints the digests of D6 (ADR 0516); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

#[test]
fn b1_default_bits_of_the_existing_stress_and_channel_fields() {
    let now = digests();
    assert_eq!(now.len(), BASE.len());
    for (k, (a, b)) in now.iter().zip(BASE).enumerate() {
        assert_eq!(*a, b, "digest {k}: 0x{a:016x} against the recorded 0x{b:016x}");
    }
}

/// B1: the digests printed at main d41012db (ADR 0516 D6).
const BASE: [u64; 17] = [
    0x8c5a_d9d9_e0fc_0d4f,
    0xcb30_6846_9720_c853,
    0xd645_6cd1_554c_b063,
    0x1710_ffaf_c065_f34f,
    0xb628_ecda_b9d7_55a3,
    0x9f8d_4a3c_3f64_e50f,
    0xa694_594f_f759_f798,
    0xde09_fe4d_9cc2_635c,
    0xa345_6704_258a_12dc,
    0xaa39_cf86_6790_d344,
    0xc6de_f9da_744e_e3c6,
    0x9498_f29e_d23e_4a6f,
    0x0562_5b94_0920_c218,
    0xaa51_72dc_5f2c_3608,
    0xe541_df03_a58f_7548,
    0x9840_689c_24c5_3ba8,
    0xd564_3306_0caf_538e,
];
