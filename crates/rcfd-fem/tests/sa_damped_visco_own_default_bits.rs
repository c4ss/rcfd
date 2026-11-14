//! F-586 (ADR 0520): check N1 (record B1, D3). The F-582 single-reference path of the damped modes (ADR 0495) gives the bits
//! it gave before F-586 when the own-frequency iteration is not asked for: rod A at its first resonance, rod B with Rayleigh
//! damping, a dashpot and the residual vectors, the 3-D cantilever with equal weights, a two-material cantilever with a
//! constant loss factor on the elastic set, and the refusal message of a reference frequency on a model without the material.
//! Uses only API that existed at 2f8c1d57; the expected digests were printed by this file run on the unchanged base 2f8c1d57
//! (`f586-scratch/b1-base.log`) and written into `EXPECTED` before any library code of F-586.

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions};
use rcfd_fem::harmonic_direct::{Dashpot, LossFactor};
use rcfd_fem::material::PronyTerm;
use rcfd_fem::{Constraint, Material, SolidModel};
use std::f64::consts::PI;

/// Digests of 2f8c1d57 in the order of the run, from the base run.
const EXPECTED: Option<[(&str, u64); 5]> = Some([
    ("rod A", 0x0353_81d5_671b_fdbe),
    ("rod B residual vectors", 0xb66b_2631_0b4f_62f8),
    ("cantilever equal weights", 0xeb4e_587c_42b1_12d0),
    ("two-material cantilever", 0x268e_ef36_99e4_34ca),
    ("refusal reference without the material", 0xc2ce_c4ef_a356_5fb6),
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

fn steel() -> Material {
    Material::isotropic_elastic(2e11, 0.3, 7850.0, 0.0).unwrap()
}

fn cantilever(material: Material) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap());
    m.add_material("all", material).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
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
    }
    v.extend_from_slice(&d.omegas);
    v
}

fn message<T>(r: rcfd_fem::Result<T>) -> String {
    match r {
        Ok(_) => "accepted".to_string(),
        Err(e) => format!("{e}"),
    }
}

#[test]
fn n1_f582_path_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();

    let (a, wa) = rod(0.0, 2.0 / 21.0, 2.0 / 21.0);
    let o = DampedOptions::new(6).viscoelastic_reference(wa, None);
    got.push(("rod A", digest(&flat(&damped_modes(&a, &o).unwrap()))));

    let (b, wb) = rod(0.3, 0.15, 0.05);
    let tip = b.mesh().node_set("xmax").unwrap()[0];
    let o = DampedOptions::new(4)
        .viscoelastic_reference(wb, None)
        .rayleigh(5.0, 2e-6)
        .dashpot(Dashpot::ground(tip, 0, 1.0e3))
        .residual_vectors(true);
    got.push(("rod B residual vectors", digest(&flat(&damped_modes(&b, &o).unwrap()))));

    let plain = cantilever(steel());
    let w1 = damped_modes(&plain, &DampedOptions::new(6)).unwrap().omegas[0];
    let wref = 0.8 * w1;
    let visco = Material::viscoelastic(steel(), vec![PronyTerm::new(0.2, 0.2, 1.0 / wref).unwrap()], None).unwrap();
    let ctip = plain.mesh().node_set("xmax").unwrap().to_vec();
    let o = DampedOptions::new(6)
        .viscoelastic_reference(wref, None)
        .rayleigh(5.0, 2e-6)
        .dashpot(Dashpot::ground(ctip[0], 1, 30.0));
    got.push((
        "cantilever equal weights",
        digest(&flat(&damped_modes(&cantilever(visco), &o).unwrap())),
    ));

    let mut mesh = hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap();
    mesh.add_element_set("left", (0..12).collect()).unwrap();
    mesh.add_element_set("right", (12..24).collect()).unwrap();
    let mut two = SolidModel::new(mesh);
    let vleft = Material::viscoelastic(steel(), vec![PronyTerm::new(0.15, 0.05, 1.0 / w1).unwrap()], None).unwrap();
    two.add_material("left", vleft).unwrap();
    two.add_material("right", Material::isotropic_elastic(7e10, 0.33, 2700.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        two.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let o = DampedOptions::new(6)
        .viscoelastic_reference(w1, None)
        .rayleigh(5.0, 2e-6)
        .loss_factor("right", LossFactor::Constant(0.01));
    got.push((
        "two-material cantilever",
        digest(&flat(&damped_modes(&two, &o).unwrap())),
    ));

    let msg = message(damped_modes(
        &plain,
        &DampedOptions::new(4).viscoelastic_reference(100.0, None),
    ));
    println!("B1 refusal: {msg}");
    got.push(("refusal reference without the material", digest_bytes(msg.into_bytes())));

    for (name, d) in &got {
        println!("B1 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B1 {name}: {d:#018x} against {e:#018x}");
    }
}
