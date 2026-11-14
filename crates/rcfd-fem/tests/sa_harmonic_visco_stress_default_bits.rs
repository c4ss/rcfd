//! F-583 (ADR 0494): check N1, record B1 (D5). `DirectHarmonic::stress` of models without the viscoelastic material gives
//! the bits it gave before F-583: the 40-element bar with a constant loss factor, with a modulus table, the two-material
//! bar, the cantilever with Rayleigh damping and dashpots, and the refusal of a response of another size. Uses only API that
//! existed at aa0e4d5b; the expected digests were printed by this file run on the unchanged base aa0e4d5b
//! (`f583-scratch/b1-base.log`) and written into `EXPECTED` before any library code of F-583.

use rcfd_fem::builders::hex_box;
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse, HarmonicStress};
use rcfd_fem::harmonic_direct::{Dashpot, DirectHarmonic, DirectHarmonicOptions, LossFactor, ModulusTable};
use rcfd_fem::{Constraint, Material, SolidModel};

/// Digests of aa0e4d5b in the order of the run, from the base run (`f583-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 5]> = Some([
    ("stress bar constant loss", 0x6dc0_bc0d_69b1_75a9),
    ("stress bar table", 0x6149_c92e_f66c_02ca),
    ("stress two-material bar", 0xd26f_1ee7_ab5a_cc71),
    ("stress cantilever Rayleigh dashpots", 0x0e37_b3c5_c2b0_c000),
    ("refusal response of another size", 0xbc94_973b_499b_e1b2),
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

fn bar_mesh() -> rcfd_fem::SolidMesh {
    hex_box([0.0; 3], [1.0, 0.04, 0.04], [40, 1, 1]).unwrap()
}

fn hold_bar(m: &mut SolidModel) {
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
}

fn bar() -> SolidModel {
    let mut m = SolidModel::new(bar_mesh());
    m.add_material("all", steel(0.0)).unwrap();
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

fn cantilever() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap());
    m.add_material("all", steel(0.3)).unwrap();
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

fn w1() -> f64 {
    std::f64::consts::PI / 2.0 * (E_MOD / RHO).sqrt()
}

fn flat(model: &SolidModel, solver: &DirectHarmonic, rs: &[HarmonicResponse]) -> Vec<f64> {
    let mut v = Vec::new();
    for r in rs {
        let st: HarmonicStress = solver.stress(model, r).unwrap();
        v.push(r.omega);
        for (a, b) in st.re.iter().zip(&st.im) {
            v.extend_from_slice(a);
            v.extend_from_slice(b);
        }
    }
    v
}

#[test]
fn n1_default_bits_of_the_stress_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let ws = [0.5 * w1(), 1.0 * w1(), 1.5 * w1()];
    let m = bar();
    let load = tip_load(&m, [1.0, 0.0, 0.0]);

    let s = DirectHarmonic::new(
        &m,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(0.02)),
    )
    .unwrap();
    got.push((
        "stress bar constant loss",
        digest(&flat(&m, &s, &s.sweep(&load, &ws).unwrap())),
    ));

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
    got.push((
        "stress bar table",
        digest(&flat(&m, &s, &s.sweep(&load, &ws).unwrap())),
    ));

    let m2 = two_material_bar();
    let o = DirectHarmonicOptions::new()
        .loss_factor("left", LossFactor::Constant(0.03))
        .loss_factor("right", LossFactor::Table(table));
    let s = DirectHarmonic::new(&m2, &o).unwrap();
    let load2 = tip_load(&m2, [1.0, 0.0, 0.0]);
    got.push((
        "stress two-material bar",
        digest(&flat(&m2, &s, &s.sweep(&load2, &ws[..2]).unwrap())),
    ));

    let c = cantilever();
    let tip = c.mesh().node_set("xmax").unwrap().to_vec();
    let o = DirectHarmonicOptions::new()
        .rayleigh(5.0, 2e-6)
        .dashpot(Dashpot::ground(tip[0], 1, 30.0))
        .dashpot(Dashpot::between(tip[1], tip[2], 2, 12.0))
        .threads(2);
    let s = DirectHarmonic::new(&c, &o).unwrap();
    let cl = tip_load(&c, [0.3, 1.0, -0.5]);
    got.push((
        "stress cantilever Rayleigh dashpots",
        digest(&flat(&c, &s, &s.sweep(&cl, &[1200.0, 3100.0]).unwrap())),
    ));

    // a response of another size is refused with the message of the recovery
    let small = {
        let mut sm = SolidModel::new(hex_box([0.0; 3], [1.0, 0.04, 0.04], [4, 1, 1]).unwrap());
        sm.add_material("all", steel(0.0)).unwrap();
        hold_bar(&mut sm);
        sm
    };
    let sm_solver = DirectHarmonic::new(&small, &DirectHarmonicOptions::new()).unwrap();
    let r = sm_solver
        .response(&tip_load(&small, [1.0, 0.0, 0.0]), 0.5 * w1())
        .unwrap();
    let s = DirectHarmonic::new(&m, &DirectHarmonicOptions::new()).unwrap();
    let msg = match s.stress(&m, &r) {
        Ok(_) => "accepted".to_string(),
        Err(e) => format!("{e}"),
    };
    println!("B1 refusal other size: {msg}");
    got.push(("refusal response of another size", digest_bytes(msg.into_bytes())));

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
