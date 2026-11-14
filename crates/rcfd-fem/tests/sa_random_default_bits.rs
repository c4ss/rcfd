//! F-417 (ADR 0477) D9, check B1b: the bits of the default paths next to the random-vibration module (the F-399 mode
//! superposition with and without the static correction, inertia relief of a free body, participation, the response stress,
//! and the narrow-band helpers of F-413), recorded on main 57808e24 before any library line of F-417 was written. Uses only
//! the API of that commit.

use rcfd_fem::builders::hex_box;
use rcfd_fem::fatigue::{narrow_band_damage_rate, spectral_rate};
use rcfd_fem::harmonic::{participation, HarmonicLoad, HarmonicOptions, HarmonicSolver, ModalBasis, ModalDamping};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// The FNV-1a hash of a sequence of 64-bit words.
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

fn steel() -> Material {
    Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap()
}

/// The cantilever C of ADR 0477 and the load vector of its two tip forces.
fn cantilever() -> (SolidModel, Vec<f64>) {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    let nodes = mesh.nodes().to_vec();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let dofs = m.dof_map();
    let find = |p: [f64; 3]| nodes.iter().position(|q| *q == p).unwrap();
    let mut f = vec![0.0; dofs.n_equations()];
    f[dofs.equation(find([1.0, 0.1, 0.08]), 1).unwrap()] = 1.0;
    f[dofs.equation(find([1.0, 0.0, 0.0]), 1).unwrap()] = 0.5;
    f[dofs.equation(find([1.0, 0.0, 0.0]), 2).unwrap()] = -0.25;
    (m, f)
}

fn digests() -> Vec<u64> {
    let (m, f) = cantilever();
    let res = modal_analysis(
        &m,
        &ModalOptions::new(8)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let load = HarmonicLoad::real(f.clone()).unwrap();
    let mut out = vec![fnv(bits(&res.eigenvalues))];
    for corr in [true, false] {
        let s = HarmonicSolver::new(
            &m,
            ModalBasis::from_result(&res).unwrap(),
            &HarmonicOptions::new(ModalDamping::Ratio(0.02))
                .static_correction(corr)
                .threads(1),
        )
        .unwrap();
        for w in [0.0, 500.0, 2.0 * std::f64::consts::PI * 91.0] {
            let r = s.response(&load, w).unwrap();
            out.push(fnv(bits(&r.re)));
            out.push(fnv(bits(&r.im)));
        }
        let sweep = s.sweep(&load, &[100.0, 700.0, 3000.0]).unwrap();
        out.push(fnv(sweep
            .iter()
            .flat_map(|r| bits(&r.re).chain(bits(&r.im)).collect::<Vec<_>>())));
        let q = s.modal_amplitudes(&load, 600.0).unwrap();
        out.push(fnv(q.iter().flat_map(|&(a, b)| [a.to_bits(), b.to_bits()])));
        let st = s.response(&load, 600.0).unwrap().stress(&m).unwrap();
        out.push(fnv(st
            .re
            .iter()
            .chain(&st.im)
            .flat_map(|x| bits(x).collect::<Vec<_>>())));
    }
    let p = participation(&m, &ModalBasis::from_result(&res).unwrap(), [0.0, 1.0, 0.0], Some(1)).unwrap();
    out.push(fnv(bits(&p.factors).chain([p.total_mass.to_bits()])));
    // A free body: inertia relief (ADR 0400).
    let mesh = hex_box([0.0; 3], [0.4, 0.1, 0.08], [4, 2, 2]).unwrap();
    let mut free = SolidModel::new(mesh);
    free.add_material("all", steel()).unwrap();
    let n = free.dof_map().n_equations();
    let mut g = vec![0.0; n];
    g[n - 1] = 1.0;
    g[n - 5] = -0.3;
    let s = HarmonicSolver::new(
        &free,
        ModalBasis::all_modes(&free, Some(1)).unwrap(),
        &HarmonicOptions::new(ModalDamping::Ratio(0.01)).threads(1),
    )
    .unwrap();
    let r = s.response(&HarmonicLoad::real(g).unwrap(), 9000.0).unwrap();
    out.push(fnv(bits(&r.re).chain(bits(&r.im))));
    let (rms, nu0) = spectral_rate(3.5e10, 2.2e14).unwrap();
    let d = narrow_band_damage_rate(nu0, rms, 3.0, 1e12).unwrap();
    out.push(fnv([rms.to_bits(), nu0.to_bits(), d.to_bits()]));
    out
}

/// Prints the digests (run at main 57808e24 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of D9 (ADR 0477); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 57808e24 (ADR 0477 D9).
const BASE: [u64; 22] = [
    0x4ee4_06bf_f3f6_6622,
    0x2397_e8e6_0295_3b4c,
    0xbe0c_df51_5f46_0005,
    0x916c_721c_026c_6ac4,
    0x785b_3a81_c2e8_fb94,
    0xb5c8_2c1c_6058_2630,
    0xd733_feb3_f1ea_0578,
    0x008c_9f79_7171_e879,
    0x1aba_e52a_0eb3_b85e,
    0xd1b4_1118_0576_25f2,
    0xcd56_c915_3aa0_489a,
    0xbe0c_df51_5f46_0005,
    0x7c93_08aa_514a_1b25,
    0x785b_3a81_c2e8_fb94,
    0xe94d_f0b6_2f1d_c6f8,
    0xd733_feb3_f1ea_0578,
    0xe476_a96b_8346_a6cd,
    0x1aba_e52a_0eb3_b85e,
    0x4b40_94b7_4aa6_390f,
    0x1a5b_c060_9b03_bb09,
    0x0d15_1d3b_04be_12f3,
    0xe299_dec1_0318_4e18,
];

#[test]
fn b1b_default_paths_unchanged() {
    let got = digests();
    assert_eq!(got.len(), BASE.len(), "digest count");
    for (k, (g, b)) in got.iter().zip(BASE.iter()).enumerate() {
        println!("B1b digest {k}: 0x{g:016x} (base 0x{b:016x})");
    }
    assert_eq!(got, BASE.to_vec(), "B1b: a default path changed its bits");
}
