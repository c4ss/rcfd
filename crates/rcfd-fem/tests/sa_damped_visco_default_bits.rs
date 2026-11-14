//! F-582 (ADR 0495): check N1 (record B0, D5). The damped modes of ADR 0450, 0464 and 0467 give the bits they gave before
//! F-582 for models without a reference frequency: the cantilever with Rayleigh damping, a dashpot and a constant loss
//! factor, the same with the residual vectors, a two-material cantilever with constant loss factors, a 40-element bar, and
//! the refusal message of a viscoelastic rod without the option. Uses only API that existed at aa0e4d5b; the expected digests
//! were printed by this file run on the unchanged base aa0e4d5b (`f582-scratch/b0-base.log`) and written into `EXPECTED`
//! before any library code of F-582.

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions};
use rcfd_fem::harmonic_direct::{Dashpot, LossFactor};
use rcfd_fem::material::PronyTerm;
use rcfd_fem::{Constraint, Material, SolidModel};

/// Digests of aa0e4d5b in the order of the run, from the base run.
const EXPECTED: Option<[(&str, u64); 5]> = Some([
    ("cantilever constant loss", 0xd071_d11a_ef34_6d4c),
    ("cantilever residual vectors", 0x4424_e4ce_fce5_dd90),
    ("two-material cantilever", 0x97eb_302c_1fe4_3396),
    ("bar", 0xa487_b7bb_9d5c_b769),
    ("refusal visco damped", 0x075f_6394_d355_e482),
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

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(2e11, nu, 7850.0, 0.0).unwrap()
}

fn cantilever(material: Material) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap());
    m.add_material("all", material).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn two_cantilever() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap();
    mesh.add_element_set("left", (0..12).collect()).unwrap();
    mesh.add_element_set("right", (12..24).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("left", steel(0.3)).unwrap();
    m.add_material("right", Material::isotropic_elastic(7e10, 0.33, 2700.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn bar(material: Material) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.05, 0.05], [40, 1, 1]).unwrap());
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn flat(d: &DampedModes) -> Vec<f64> {
    let mut v = Vec::new();
    for z in &d.roots {
        v.push(z.re);
        v.push(z.im);
    }
    for md in &d.modes {
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
fn n1_default_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let c = cantilever(steel(0.3));
    let tip = c.mesh().node_set("xmax").unwrap().to_vec();
    let o = DampedOptions::new(6)
        .rayleigh(5.0, 2e-6)
        .dashpot(Dashpot::ground(tip[0], 1, 30.0))
        .loss_factor("all", LossFactor::Constant(0.02));
    got.push(("cantilever constant loss", digest(&flat(&damped_modes(&c, &o).unwrap()))));

    let o = DampedOptions::new(5)
        .rayleigh(5.0, 2e-6)
        .dashpot(Dashpot::ground(tip[0], 1, 3.0e4))
        .dashpot(Dashpot::between(tip[1], tip[2], 2, 1.0e4))
        .residual_vectors(true)
        .threads(2);
    got.push(("cantilever residual vectors", digest(&flat(&damped_modes(&c, &o).unwrap()))));

    let o = DampedOptions::new(6)
        .loss_factor("left", LossFactor::Constant(0.03))
        .loss_factor("right", LossFactor::Constant(0.01));
    got.push(("two-material cantilever", digest(&flat(&damped_modes(&two_cantilever(), &o).unwrap()))));

    let b = bar(steel(0.0));
    got.push(("bar", digest(&flat(&damped_modes(&b, &DampedOptions::new(6)).unwrap()))));

    let visco = Material::viscoelastic(
        Material::isotropic_elastic(2e9, 0.0, 1200.0, 0.0).unwrap(),
        vec![PronyTerm::new(0.1, 0.1, 1e-3).unwrap()],
        None,
    )
    .unwrap();
    let msg = message(damped_modes(&bar(visco), &DampedOptions::new(4)));
    println!("B0 refusal visco damped: {msg}");
    got.push(("refusal visco damped", digest_bytes(msg.into_bytes())));

    for (name, d) in &got {
        println!("B0 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B0 {name}: {d:#018x} against {e:#018x}");
    }
}
