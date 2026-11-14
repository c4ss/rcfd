//! F-567 (ADR 0510) D9, check B6: the bits of the F-417 default paths (the frequency grid, `spectral_response` with and
//! without a real correlation, groups of 1 and 3 outputs, and `RandomSolver::response` on the cantilever with force patterns
//! uncorrelated and correlated, a base acceleration and the stress), recorded on main 6c4ba2d9 before any library line of
//! F-567 was written. Uses only the API of that commit.

use rcfd_fem::builders::hex_box;
use rcfd_fem::harmonic::{HarmonicOptions, ModalBasis, ModalDamping};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::random::{
    frequency_grid, spectral_response, Excitation, ModalChannels, RandomRequest, RandomSolver, SpectralOptions,
    Spectrum,
};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

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

fn spectra() -> Vec<Spectrum> {
    vec![
        Spectrum::flat(10.0, 900.0, 2.0).unwrap(),
        Spectrum::new(vec![20.0, 80.0, 400.0, 700.0], vec![1.0, 3.0, 3.0, 0.5]).unwrap(),
        Spectrum::log_log(vec![30.0, 120.0, 600.0], vec![0.5, 4.0, 1.0]).unwrap(),
    ]
}

fn channels() -> ModalChannels {
    let freqs = [60.0_f64, 140.0, 310.0];
    let eig: Vec<f64> = freqs.iter().map(|f| (2.0 * std::f64::consts::PI * f).powi(2)).collect();
    let damp: Vec<f64> = freqs
        .iter()
        .map(|f| 2.0 * 0.02 * 2.0 * std::f64::consts::PI * f)
        .collect();
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

fn digests() -> Vec<u64> {
    let mut out = Vec::new();
    let sp = spectra();
    let ch = channels();
    let eig: Vec<f64> = (0..3)
        .map(|r| (2.0 * std::f64::consts::PI * [60.0_f64, 140.0, 310.0][r]).powi(2))
        .collect();
    let damp: Vec<f64> = (0..3)
        .map(|r| 2.0 * 0.02 * 2.0 * std::f64::consts::PI * [60.0_f64, 140.0, 310.0][r])
        .collect();
    for refinement in [1, 2] {
        let (n, w) = frequency_grid(&sp, &eig, &damp, refinement).unwrap();
        out.push(fnv(bits(&n).chain(bits(&w))));
    }
    let rho = [vec![1.0, 0.6, -0.3], vec![0.6, 1.0, 0.2], vec![-0.3, 0.2, 1.0]];
    for corr in [None, Some(&rho[..])] {
        for group in [1, 3] {
            let r = spectral_response(&ch, &sp, corr, &SpectralOptions::default().group(group)).unwrap();
            let mut words: Vec<u64> = Vec::new();
            for p in &r.psd {
                words.extend(bits(p));
            }
            for m in &r.moments {
                words.extend([m.m0.to_bits(), m.m1.to_bits(), m.m2.to_bits(), m.m4.to_bits()]);
            }
            for c in r.covariance.iter().chain(&r.rate_covariance) {
                words.extend(bits(c));
            }
            out.push(fnv(words));
        }
    }
    // The solid-model path.
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    let nodes = mesh.nodes().to_vec();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let dofs = m.dof_map();
    let find = |p: [f64; 3]| nodes.iter().position(|q| *q == p).unwrap();
    let (a, b) = (find([1.0, 0.1, 0.08]), find([1.0, 0.0, 0.0]));
    let n = dofs.n_equations();
    let mut p1 = vec![0.0; n];
    let mut p2 = vec![0.0; n];
    p1[dofs.equation(a, 1).unwrap()] = 1.0;
    p2[dofs.equation(b, 1).unwrap()] = 1.0;
    let eqs: Vec<usize> = [a, b]
        .iter()
        .flat_map(|&v| (0..3).map(move |c| (v, c)))
        .map(|(v, c)| dofs.equation(v, c).unwrap())
        .collect();
    let res = modal_analysis(
        &m,
        &ModalOptions::new(8)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let solver = RandomSolver::new(
        &m,
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
    ];
    for (k, ex) in excitations.iter().enumerate() {
        let req = RandomRequest::new(eqs.clone()).stress(k != 2);
        let r = solver.response(&m, ex, &req).unwrap();
        let mut words: Vec<u64> = Vec::new();
        for p in &r.psd {
            words.extend(bits(p));
        }
        for mo in &r.moments {
            words.extend([mo.m0.to_bits(), mo.m1.to_bits(), mo.m2.to_bits(), mo.m4.to_bits()]);
        }
        if let Some(s) = &r.stress {
            for c in s.covariance.iter().chain(&s.rate_covariance) {
                for row in c {
                    words.extend(bits(row));
                }
            }
        }
        out.push(fnv(words));
    }
    out
}

/// Prints the digests (run at main 6c4ba2d9 to record BASE).
#[test]
#[ignore = "prints the digests of D9 (ADR 0510); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// B6: the digests printed at main 6c4ba2d9 (ADR 0510 D9).
const BASE: [u64; 9] = [
    0x112c_bf5d_912b_c28a,
    0xa6d4_007d_26ba_4e8a,
    0x5a31_b526_d148_7190,
    0x2763_c494_1faa_2244,
    0xe3e2_1273_2c28_6ef8,
    0x59d1_5435_64d8_8d7c,
    0x9d62_66ba_da0a_8b2b,
    0xc0fb_a2c9_99db_744b,
    0xcb8c_2b4b_5b29_018e,
];

#[test]
fn b6_default_bits_of_the_f417_paths() {
    let now = digests();
    assert_eq!(now.len(), BASE.len());
    for (k, (a, b)) in now.iter().zip(BASE).enumerate() {
        assert_eq!(*a, b, "digest {k}: 0x{a:016x} against the recorded 0x{b:016x}");
    }
}
