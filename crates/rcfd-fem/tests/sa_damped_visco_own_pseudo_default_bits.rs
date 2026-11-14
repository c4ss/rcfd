//! F-609 (ADR 0529): check N1 (record of the default bits). The own-frequency iteration of F-586 (ADR 0520) gives the bits it
//! gave before F-609 when the pseudo skip is not asked for: rod A, rod B plain, rod B with residual vectors and 500
//! iterations (the pseudo-mode takes 170), its notes, and the refusal message of the default 50 iterations. Uses only API that
//! existed at 1f142196; the expected digests were printed by this file run on the unchanged base 1f142196
//! (`f609-scratch/n1-base.log`) and written into `EXPECTED` before any library code of F-609.

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions, OwnFrequency};
use rcfd_fem::harmonic_direct::Dashpot;
use rcfd_fem::material::PronyTerm;
use rcfd_fem::{Constraint, Material, SolidModel};
use std::f64::consts::PI;

/// Digests of 1f142196 in the order of the run, from the base run.
const EXPECTED: Option<[(&str, u64); 6]> = Some([
    ("rod A own", 0x7481_56ed_1580_74e9),
    ("rod A notes", 0x5619_c8d6_e71f_df35),
    ("rod B own", 0x6e7b_d256_c618_e67c),
    ("rod B residual vectors 500", 0x0b69_22d8_8f21_daa7),
    ("rod B residual vectors 500 notes", 0xcc73_bf32_113a_44e6),
    ("refusal at 50 iterations", 0x807b_c396_1a72_e260),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    digest_bytes(v.iter().flat_map(|x| x.to_bits().to_le_bytes()))
}

fn digest_bytes<I: IntoIterator<Item = u8>>(bytes: I) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

const L: f64 = 1.0;
const B: f64 = 0.05;
const RHO: f64 = 1200.0;
const E0: f64 = 2.0e9;

fn g0(nu: f64) -> f64 {
    E0 / (2.0 * (1.0 + nu))
}

fn k0(nu: f64) -> f64 {
    E0 / (3.0 * (1.0 - 2.0 * nu))
}

/// The axial rod of ADR 0495 with one term of weights `g`, `k` at `tau = 1/omega_c`; returns the model and `omega_c`.
fn rod(nu: f64, g: f64, k: f64) -> (SolidModel, f64) {
    let stored = k0(nu) * (1.0 - k / 2.0) + 4.0 / 3.0 * g0(nu) * (1.0 - g / 2.0);
    let wc = PI / (2.0 * L) * (stored / RHO).sqrt();
    let base = Material::isotropic_elastic(E0, nu, RHO, 0.0).unwrap();
    let visco = Material::viscoelastic(base, vec![PronyTerm::new(g, k, 1.0 / wc).unwrap()], None).unwrap();
    let mut m = SolidModel::new(hex_box([0.0; 3], [L, B, B], [40, 1, 1]).unwrap());
    m.add_material("all", visco).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    (m, wc)
}

fn flat(d: &DampedModes) -> Vec<f64> {
    let mut v = Vec::new();
    for z in &d.roots {
        v.push(z.re);
        v.push(z.im);
    }
    for md in &d.modes {
        v.push(md.eigenvalue.re);
        v.push(md.eigenvalue.im);
        for z in &md.shape {
            v.push(z.re);
            v.push(z.im);
        }
        v.push(md.residual);
        v.push(md.residual_share);
        v.push(f64::from(u8::from(md.pseudo)));
    }
    v.extend_from_slice(&d.omegas);
    for r in &d.own_frequency {
        v.push(r.frozen.re);
        v.push(r.frozen.im);
        v.push(r.omega);
        v.push(r.iterations as f64);
        v.push(r.change);
    }
    v
}

fn notes(d: &DampedModes) -> u64 {
    digest_bytes(
        d.notes
            .join(
                "
",
            )
            .into_bytes(),
    )
}

fn message<T>(r: rcfd_fem::Result<T>) -> String {
    match r {
        Ok(_) => "accepted".to_string(),
        Err(e) => format!("{e}"),
    }
}

#[test]
fn n1_f586_own_frequency_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();

    let (a, wa) = rod(0.0, 2.0 / 21.0, 2.0 / 21.0);
    let o = DampedOptions::new(6)
        .viscoelastic_reference(wa, None)
        .viscoelastic_own_frequency(OwnFrequency::new(4));
    let d = damped_modes(&a, &o).unwrap();
    got.push(("rod A own", digest(&flat(&d))));
    got.push(("rod A notes", notes(&d)));

    let (b, wb) = rod(0.3, 0.15, 0.05);
    let o = DampedOptions::new(6)
        .viscoelastic_reference(wb, None)
        .rayleigh(5.0, 2e-6)
        .viscoelastic_own_frequency(OwnFrequency::new(4));
    got.push(("rod B own", digest(&flat(&damped_modes(&b, &o).unwrap()))));

    let tip = b.mesh().node_set("xmax").unwrap()[0];
    let rv = |own: OwnFrequency| {
        DampedOptions::new(4)
            .viscoelastic_reference(wb, None)
            .viscoelastic_own_frequency(own)
            .dashpot(Dashpot::ground(tip, 0, 1.0e3))
            .residual_vectors(true)
    };
    let d = damped_modes(&b, &rv(OwnFrequency::new(3).max_iterations(500))).unwrap();
    println!(
        "RV iterations {:?}",
        d.own_frequency.iter().map(|r| r.iterations).collect::<Vec<_>>()
    );
    got.push(("rod B residual vectors 500", digest(&flat(&d))));
    got.push(("rod B residual vectors 500 notes", notes(&d)));

    let msg = message(damped_modes(&b, &rv(OwnFrequency::new(3))));
    println!("N1 refusal: {msg}");
    got.push(("refusal at 50 iterations", digest_bytes(msg.into_bytes())));

    for (name, d) in &got {
        println!("N1 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "N1 {name}: {d:#018x} against {e:#018x}");
    }
}
