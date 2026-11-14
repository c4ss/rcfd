//! F-588 (ADR 0525): check B1, the paths of the item give the bits they gave before F-588 when no block asks for a Hex8
//! formulation: the damped modes and the direct harmonic solve with two element sets (the loss-factor closures), the viscoelastic
//! damped modes (reference frequency, own frequency, equal and unequal weights) and direct solve (the split closure), a
//! tabulated material with a non-uniform temperature and an orthotropic material with cylindrical axes on one distorted element,
//! an orthotropic material with a temperature on a box, and the buckling factors of the one-layer strip. Uses only API that
//! existed at f168d994; the expected digests were printed by this file run on the unchanged base (`f588-scratch/b1-base.log`)
//! and written into `EXPECTED` before any library code of F-588.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions, OwnFrequency};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::material::{MaterialAxes, PronyTerm};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// Digests of f168d994 in the order of the run, from the base run (`f588-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 18]> = Some([
    ("damped two sets", 0xf7c3_47a8_913b_6bb3),
    ("damped visco equal", 0xa6b8_9667_92ce_0b6c),
    ("damped visco unequal", 0xfc88_3bf3_600d_5874),
    ("damped visco own frequency", 0x6fc3_004a_927d_cd13),
    ("direct two sets response", 0xee88_24de_39b2_454d),
    ("direct two sets stress", 0x4147_aba8_3b81_52f5),
    ("direct visco unequal response", 0x233c_fd68_5292_a81d),
    ("direct visco unequal stress", 0x9ed3_6ce5_1a46_4ff9),
    ("direct visco equal response", 0x32d4_c55f_41ae_27d1),
    ("direct visco equal stress", 0xace2_259a_705b_fe9d),
    ("tabulated element solution", 0x12f0_f99b_4b95_5988),
    ("tabulated element strain", 0xc775_3a26_a50b_8444),
    ("tabulated element stress", 0x8db8_fd5f_f7d1_4d28),
    ("cylindrical element solution", 0x6dc3_9e6a_22e2_4187),
    ("cylindrical element stress", 0xd6ff_0dea_540e_4e61),
    ("orthotropic thermal solution", 0x1208_b51a_30a0_d5fb),
    ("orthotropic thermal stress", 0xd68e_4fde_e985_a223),
    ("strip buckling factors", 0x4404_d1a7_2c53_fb63),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h
}

fn flat6(v: &[[f64; 6]]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn visco(shear: f64, bulk: f64, tau: f64) -> Material {
    Material::viscoelastic(steel(), vec![PronyTerm::new(shear, bulk, tau).unwrap()], None).unwrap()
}

/// The strip `2 x 0.1 x 0.1` of ten elements, root face fixed, a tip traction; two element sets `a`, `b` with the same material
/// when `two`, else the one set `all`.
fn strip(two: bool, material: Material) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [2.0, 0.1, 0.1], [10, 1, 1]).unwrap();
    let mut m;
    if two {
        mesh.add_element_set("a", (0..5).collect()).unwrap();
        mesh.add_element_set("b", (5..10).collect()).unwrap();
        m = SolidModel::new(mesh);
        m.add_material("a", material.clone()).unwrap();
        m.add_material("b", material).unwrap();
    } else {
        m = SolidModel::new(mesh);
        m.add_material("all", material).unwrap();
    }
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e5])).unwrap();
    m
}

fn damped_digest(d: &DampedModes) -> u64 {
    let mut v = Vec::new();
    for m in &d.modes {
        v.push(m.frequency);
        v.push(m.decay_rate);
        v.push(m.eigenvalue.re);
        v.push(m.eigenvalue.im);
    }
    digest(&v)
}

fn modal() -> ModalOptions {
    ModalOptions::new(4)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

fn response_digest(d: &DirectHarmonic, model: &SolidModel, omegas: &[f64]) -> (u64, u64) {
    let load = HarmonicLoad::from_model(model).unwrap();
    let mut u = Vec::new();
    let mut s = Vec::new();
    for &w in omegas {
        let r = d.response(&load, w).unwrap();
        u.extend_from_slice(&r.re);
        u.extend_from_slice(&r.im);
        let st = d.stress(model, &r).unwrap();
        s.extend(flat6(&st.re));
        s.extend(flat6(&st.im));
    }
    (digest(&u), digest(&s))
}

fn element_mesh(coords: [Vec3; 8]) -> SolidMesh {
    let mut mesh = SolidMesh::new(coords.to_vec()).unwrap();
    mesh.add_block(ElementKind::Hex8, (0..8).collect()).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    mesh
}

/// The distorted single element of ADR 0493 (`distorted_element`).
fn distorted() -> [Vec3; 8] {
    let offs = [
        [0.0, 0.0, 0.0],
        [0.1, 0.05, -0.05],
        [0.15, -0.1, 0.05],
        [-0.1, 0.1, 0.0],
        [0.05, -0.05, 0.1],
        [0.0, 0.1, 0.05],
        [0.1, 0.1, 0.1],
        [-0.05, 0.0, -0.1],
    ];
    let mut out = [[0.0; 3]; 8];
    for (i, r) in ElementKind::Hex8.reference_nodes().iter().enumerate() {
        for k in 0..3 {
            out[i][k] = 0.5 * r[k] + 0.5 + offs[i][k];
        }
    }
    out
}

fn single_supports() -> Vec<Prescribed> {
    vec![
        Prescribed::new(0, 0, 0.0),
        Prescribed::new(0, 1, 0.0),
        Prescribed::new(0, 2, 0.0),
        Prescribed::new(1, 1, 0.0),
        Prescribed::new(1, 2, 0.0),
        Prescribed::new(3, 2, 0.0),
    ]
}

fn ortho(expansion: [f64; 3], axes: MaterialAxes) -> Material {
    Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.4],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        expansion,
        axes,
    )
    .unwrap()
}

#[test]
fn b1_the_default_paths_keep_their_bits() {
    let mut got: Vec<(String, u64)> = Vec::new();

    // Damped modes, two sets with a loss factor each (the multi-set closure).
    let two = strip(true, steel());
    let opts = DampedOptions::with_modal(modal())
        .loss_factor("a", LossFactor::Constant(0.02))
        .loss_factor("b", LossFactor::Constant(0.03));
    got.push((
        "damped two sets".into(),
        damped_digest(&damped_modes(&two, &opts).unwrap()),
    ));

    // Viscoelastic damped modes: the reference frequency (equal and unequal weights) and the own frequency.
    let eq = strip(false, visco(0.2, 0.2, 1.0e-2));
    let un = strip(false, visco(0.03, 0.01, 1.0e-2));
    let vopts = |o: DampedOptions| o.viscoelastic_reference(100.0, None);
    got.push((
        "damped visco equal".into(),
        damped_digest(&damped_modes(&eq, &vopts(DampedOptions::with_modal(modal()))).unwrap()),
    ));
    got.push((
        "damped visco unequal".into(),
        damped_digest(&damped_modes(&un, &vopts(DampedOptions::with_modal(modal()))).unwrap()),
    ));
    got.push((
        "damped visco own frequency".into(),
        damped_digest(
            &damped_modes(
                &un,
                &vopts(DampedOptions::with_modal(modal())).viscoelastic_own_frequency(OwnFrequency::new(4)),
            )
            .unwrap(),
        ),
    ));

    // Direct harmonic, two sets with a loss factor each.
    let dopts = DirectHarmonicOptions::new()
        .loss_factor("a", LossFactor::Constant(0.02))
        .loss_factor("b", LossFactor::Constant(0.03));
    let d = DirectHarmonic::new(&two, &dopts).unwrap();
    let (u, s) = response_digest(&d, &two, &[50.0, 400.0]);
    got.push(("direct two sets response".into(), u));
    got.push(("direct two sets stress".into(), s));

    // Direct harmonic, viscoelastic, unequal and equal weights.
    for (name, model) in [("unequal", &un), ("equal", &eq)] {
        let d = DirectHarmonic::new(model, &DirectHarmonicOptions::new()).unwrap();
        let (u, s) = response_digest(&d, model, &[30.0, 300.0]);
        got.push((format!("direct visco {name} response"), u));
        got.push((format!("direct visco {name} stress"), s));
    }

    // A tabulated material with a non-uniform temperature, one distorted element.
    let table = Material::tabulated(
        steel(),
        vec![200.0, 600.0],
        vec![
            ("e".to_string(), vec![2.0e11, 1.8e11]),
            ("nu".to_string(), vec![0.3, 0.3]),
            ("expansion".to_string(), vec![1.2e-5, 1.5e-5]),
        ],
    )
    .unwrap();
    let mut t = SolidModel::new(element_mesh(distorted()));
    t.add_material("all", table).unwrap();
    t.add_load(Load::temperature(
        vec![300.0, 320.0, 350.0, 310.0, 340.0, 380.0, 360.0, 330.0],
        293.0,
    ))
    .unwrap();
    let forces = [
        NodalForceAt::new(6, [1.0e3, -2.0e3, 3.0e3]),
        NodalForceAt::new(7, [-1.0e3, 5.0e2, 8.0e2]),
    ];
    let r = solve_static_with(&t, &single_supports(), &forces, &StaticOptions::default()).unwrap();
    got.push(("tabulated element solution".into(), digest(r.displacement_values())));
    let f = IntegrationPointField::compute(&t, r.displacement_values()).unwrap();
    got.push(("tabulated element strain".into(), digest(&flat6(f.strain()))));
    got.push(("tabulated element stress".into(), digest(&flat6(f.stress()))));

    // An orthotropic material with cylindrical axes, one distorted element.
    let cyl = ortho(
        [0.0; 3],
        MaterialAxes::cylindrical([-0.4, -0.3, 0.0], [0.0, 0.0, 1.0]).unwrap(),
    );
    let mut o = SolidModel::new(element_mesh(distorted()));
    o.add_material("all", cyl).unwrap();
    let r = solve_static_with(&o, &single_supports(), &forces, &StaticOptions::default()).unwrap();
    got.push(("cylindrical element solution".into(), digest(r.displacement_values())));
    let f = IntegrationPointField::compute(&o, r.displacement_values()).unwrap();
    got.push(("cylindrical element stress".into(), digest(&flat6(f.stress()))));

    // An orthotropic material with a uniform temperature on a box.
    let fixed = MaterialAxes::Fixed([[0.8, 0.6, 0.0], [-0.6, 0.8, 0.0], [0.0, 0.0, 1.0]]);
    let mut b = SolidModel::new(hex_box([0.0; 3], [1.0, 0.5, 0.5], [4, 2, 2]).unwrap());
    b.add_material("all", ortho([2.0e-6, 3.0e-5, 4.0e-5], fixed)).unwrap();
    for c in 0..3 {
        b.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let n = b.mesh().nodes().len();
    b.add_load(Load::temperature(vec![353.0; n], 293.0)).unwrap();
    let r = solve_static(&b, &StaticOptions::default()).unwrap();
    got.push(("orthotropic thermal solution".into(), digest(r.displacement_values())));
    let f = IntegrationPointField::compute(&b, r.displacement_values()).unwrap();
    got.push(("orthotropic thermal stress".into(), digest(&flat6(f.stress()))));

    // The buckling factors of the one-layer strip (the geometric stiffness of the Hex8 element).
    let mut s = {
        let mesh = hex_box([0.0; 3], [2.0, 0.1, 0.1], [20, 1, 1]).unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", Material::isotropic_elastic(2.0e11, 0.0, 7850.0, 0.0).unwrap())
            .unwrap();
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        m
    };
    s.add_load(Load::traction("xmax", [-1.0e6, 0.0, 0.0])).unwrap();
    let r = solve_static(&s, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&s, &r).unwrap();
    let bf = factors(&s, &pre, &BucklingOptions::new(2)).unwrap();
    got.push(("strip buckling factors".into(), digest(&bf.factors)));

    for (name, d) in &got {
        println!(
            "(\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    let expected = EXPECTED.expect("the digests of the base run");
    let got_ref: Vec<(&str, u64)> = got.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    assert_eq!(got_ref, expected.to_vec());
}
