//! F-577 (ADR 0491): check N1 (record B0, D6). The direct harmonic solve of ADR 0448 and the damped modes of ADR 0450 give
//! the bits they gave before F-577 for models without the viscoelastic material: a 40-element bar with a constant loss
//! factor, the same bar with a modulus table, a two-material bar, a cantilever with Rayleigh damping and dashpots (with the
//! harmonic stress of one response), a base-motion response, the damped modes of the cantilever, and the refusal messages
//! of the J2 material (alone and with a table) and of a set without a material. Uses only API that existed at e90c2350;
//! the expected digests were printed by this file run on the unchanged base e90c2350 (`f577-scratch/b0-base.log`) and
//! written into `EXPECTED` before any library code of F-577.

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse};
use rcfd_fem::harmonic_direct::{BaseMotion, Dashpot, DirectHarmonic, DirectHarmonicOptions, LossFactor, ModulusTable};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::complex::C64;

/// Digests of e90c2350 in the order of the run, from the base run (`f577-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 11]> = Some([
    ("bar constant loss", 0x8033_f47a_7405_6aca),
    ("bar table", 0x78c8_9e45_b069_bd06),
    ("two-material bar", 0x0947_8b34_9dec_027a),
    ("cantilever Rayleigh dashpots", 0xc1ed_ab01_5bc8_0c9b),
    ("cantilever stress", 0xc08a_1e38_5ee3_61be),
    ("bar base motion", 0x820a_e875_31a7_b4d7),
    ("damped modes cantilever", 0x716f_40c0_0926_e0eb),
    ("refusal J2 direct", 0x3986_85a1_bb32_2d6c),
    ("refusal table on J2", 0x3986_85a1_bb32_2d6c),
    ("refusal set without material", 0x4cf8_4ce2_d9be_3864),
    ("refusal J2 damped", 0x3986_85a1_bb32_2d6c),
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

const E_MOD: f64 = 2e11;
const RHO: f64 = 7850.0;

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap()
}

fn j2() -> Material {
    Material::j2(steel(0.3), 2.0e8, 2.0e9, 1.0e9).unwrap()
}

/// The bar of ADR 0448: `[0, 1] x [0, 0.04]^2`, Hex8 40 x 1 x 1, `nu = 0`, the lateral motion held, `xmin` held.
fn bar_mesh() -> rcfd_fem::SolidMesh {
    hex_box([0.0; 3], [1.0, 0.04, 0.04], [40, 1, 1]).unwrap()
}

fn hold_bar(m: &mut SolidModel) {
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
}

fn bar(material: Material) -> SolidModel {
    let mut m = SolidModel::new(bar_mesh());
    m.add_material("all", material).unwrap();
    hold_bar(&mut m);
    m
}

fn two_material_bar() -> SolidModel {
    let mut mesh = bar_mesh();
    mesh.add_element_set("left", (0..20).collect()).unwrap();
    mesh.add_element_set("right", (20..40).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("left", steel(0.0)).unwrap();
    m.add_material(
        "right",
        Material::isotropic_elastic(0.7 * E_MOD, 0.0, RHO, 0.0).unwrap(),
    )
    .unwrap();
    hold_bar(&mut m);
    m
}

fn cantilever(material: Material) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap());
    m.add_material("all", material).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn tip_load(model: &SolidModel, dir: [f64; 3]) -> HarmonicLoad {
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let (mut re, mut im) = (vec![0.0; n], vec![0.0; n]);
    let nodes = model.mesh().node_set("xmax").unwrap();
    let share = 1.0 / nodes.len() as f64;
    for &a in nodes {
        for (c, d) in dir.iter().enumerate() {
            if let Some(eq) = dofs.equation(a, c) {
                re[eq] = share * d;
                im[eq] = 0.1 * share * d * (c as f64 + 1.0);
            }
        }
    }
    HarmonicLoad::new(re, im).unwrap()
}

fn flat(rs: &[HarmonicResponse]) -> Vec<f64> {
    let mut v = Vec::new();
    for r in rs {
        v.push(r.omega);
        v.extend_from_slice(&r.re);
        v.extend_from_slice(&r.im);
    }
    v
}

/// The first longitudinal frequency of the steel bar, rad/s.
fn w1() -> f64 {
    std::f64::consts::PI / 2.0 * (E_MOD / RHO).sqrt()
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
    let ws = [0.5 * w1(), 1.0 * w1(), 1.5 * w1()];

    // constant loss factor
    let m = bar(steel(0.0));
    let s = DirectHarmonic::new(
        &m,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(0.02)),
    )
    .unwrap();
    let load = tip_load(&m, [1.0, 0.0, 0.0]);
    got.push(("bar constant loss", digest(&flat(&s.sweep(&load, &ws).unwrap()))));

    // modulus table
    let f1 = w1() / (2.0 * std::f64::consts::PI);
    let table = ModulusTable::new(
        vec![0.0, f1, 3.0 * f1],
        vec![E_MOD, 0.9 * E_MOD, 1.3 * E_MOD],
        vec![0.01, 0.05, 0.03],
    )
    .unwrap();
    let s = DirectHarmonic::new(
        &m,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Table(table.clone())),
    )
    .unwrap();
    got.push(("bar table", digest(&flat(&s.sweep(&load, &ws).unwrap()))));

    // two materials
    let m2 = two_material_bar();
    let o = DirectHarmonicOptions::new()
        .loss_factor("left", LossFactor::Constant(0.03))
        .loss_factor("right", LossFactor::Table(table));
    let s = DirectHarmonic::new(&m2, &o).unwrap();
    let load2 = tip_load(&m2, [1.0, 0.0, 0.0]);
    got.push(("two-material bar", digest(&flat(&s.sweep(&load2, &ws).unwrap()))));

    // cantilever: Rayleigh, dashpots, stress
    let c = cantilever(steel(0.3));
    let tip = c.mesh().node_set("xmax").unwrap().to_vec();
    let o = DirectHarmonicOptions::new()
        .rayleigh(5.0, 2e-6)
        .dashpot(Dashpot::ground(tip[0], 1, 30.0))
        .dashpot(Dashpot::between(tip[1], tip[2], 2, 12.0))
        .threads(2);
    let s = DirectHarmonic::new(&c, &o).unwrap();
    let cl = tip_load(&c, [0.3, 1.0, -0.5]);
    let rs = s.sweep(&cl, &[800.0, 2500.0]).unwrap();
    got.push(("cantilever Rayleigh dashpots", digest(&flat(&rs))));
    let st = s.stress(&c, &rs[1]).unwrap();
    let mut v = Vec::new();
    for (a, b) in st.re.iter().zip(&st.im) {
        v.extend_from_slice(a);
        v.extend_from_slice(b);
    }
    got.push(("cantilever stress", digest(&v)));

    // base motion
    let s = DirectHarmonic::new(
        &m,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(0.02)),
    )
    .unwrap();
    let mut base = BaseMotion::new();
    base.add_set(&m, "xmin", 0, C64::new(1e-3, 2e-4)).unwrap();
    let zero = HarmonicLoad::real(vec![0.0; m.dof_map().n_equations()]).unwrap();
    let r = s.response_with_base(&zero, &base, 0.7 * w1()).unwrap();
    got.push(("bar base motion", digest(&flat(&[r]))));

    // damped modes
    let o = DampedOptions::new(6)
        .rayleigh(5.0, 2e-6)
        .dashpot(Dashpot::ground(tip[0], 1, 30.0))
        .loss_factor("all", LossFactor::Constant(0.02));
    let d = damped_modes(&c, &o).unwrap();
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
    }
    got.push(("damped modes cantilever", digest(&v)));

    // refusals
    let msg = message(DirectHarmonic::new(&cantilever(j2()), &DirectHarmonicOptions::new()));
    println!("B0 refusal J2 direct: {msg}");
    got.push(("refusal J2 direct", digest_bytes(msg.into_bytes())));
    let t2 = ModulusTable::new(vec![0.0, 1e5], vec![E_MOD, E_MOD], vec![0.02, 0.02]).unwrap();
    let msg = message(DirectHarmonic::new(
        &cantilever(j2()),
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Table(t2)),
    ));
    println!("B0 refusal table on J2: {msg}");
    got.push(("refusal table on J2", digest_bytes(msg.into_bytes())));
    let msg = message(DirectHarmonic::new(
        &m,
        &DirectHarmonicOptions::new().loss_factor("nowhere", LossFactor::Constant(0.02)),
    ));
    println!("B0 refusal set: {msg}");
    got.push(("refusal set without material", digest_bytes(msg.into_bytes())));
    let msg = message(damped_modes(&cantilever(j2()), &DampedOptions::new(4)));
    println!("B0 refusal J2 damped: {msg}");
    got.push(("refusal J2 damped", digest_bytes(msg.into_bytes())));

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
