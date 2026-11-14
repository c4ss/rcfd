//! F-426 (ADR 0456) B1: the bits of the default paths that F-426 touches (the cyclic boundary match, the cyclic rows of the
//! transformation and of the harmonic transformation, the full rotor, the linear-constraint static solve), recorded on main
//! b154dbe6 before any library line of F-426 was written. Uses only the API of that commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::{boundary, full_rotor, modal_analysis_harmonic};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::LinearTerm;
use rcfd_fem::mpc::{self, harmonic_transformation, transformation};
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel};

const AXIS: Vec3 = [0.0, 0.0, 1.0];
const N: usize = 12;

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

fn polar_sector(r0: f64, dr: f64, z0: f64, dz: f64, alpha: f64, div: [usize; 3]) -> SolidMesh {
    let b = hex_box([r0, 0.0, z0], [dr, alpha, dz], div).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn ring() -> SolidModel {
    let a = 2.0 * std::f64::consts::PI / N as f64;
    let mut mesh = polar_sector(0.975, 0.05, -0.05, 0.1, a, [1, 4, 1]);
    let spoke: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&p| {
            let x = mesh.nodes()[p];
            ((x[0] * x[0] + x[1] * x[1]).sqrt() - 0.975).abs() < 1e-12 && (x[1].atan2(x[0]) - a / 2.0).abs() < 1e-12
        })
        .collect();
    mesh.add_node_set("spoke", spoke).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("spoke", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, [0.0; 3], N)).unwrap();
    m
}

fn disc() -> SolidModel {
    let a = 2.0 * std::f64::consts::PI / N as f64;
    let mut mesh = polar_sector(0.25, 0.75, -0.01, 0.02, a, [4, 2, 1]);
    let mid: Vec<usize> = (0..mesh.nodes().len()).filter(|&p| mesh.nodes()[p][2] == 0.0).collect();
    let pin: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&p| {
            let x = mesh.nodes()[p];
            (x[0] - 0.25).abs() < 1e-12 && x[1].abs() < 1e-12 && x[2] == 0.0
        })
        .collect();
    mesh.add_node_set("mid", mid).unwrap();
    mesh.add_node_set("pin", pin).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("mid", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin", 1, 0.0)).unwrap();
    m.add_load(Load::centrifugal(AXIS, [0.0; 3], 100.0)).unwrap();
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, [0.0; 3], N)).unwrap();
    m
}

fn linear_model() -> SolidModel {
    let mesh = hex_box([0.0; 3], [2.0, 0.5, 0.5], [4, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let tip = m.mesh().node_set("xmax").unwrap().to_vec();
    // u_z of the tip nodes tied to the first tip node, and a rigid set on the tip.
    for &p in &tip[1..] {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(p, 2, 1.0), LinearTerm::new(tip[0], 2, -1.0)],
            0.0,
        ))
        .unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -1e3])).unwrap();
    m
}

#[test]
fn b1_default_bits() {
    let mut words: Vec<(String, u64)> = Vec::new();
    // The ring: boundary pairs, transformation at k = 0, harmonic transformations at k = 1 and k = 6, modes at k = 0, 1, 6.
    let r = ring();
    let bd = boundary(&r, 3).unwrap();
    words.push((
        "ring pairs".into(),
        fnv(bd.pairs().iter().flat_map(|&(l, h)| [l as u64, h as u64])),
    ));
    let t0 = transformation(&r, &[]).unwrap();
    words.push(("ring T k0".into(), fnv(bits(&t0.t().val).chain(t0.t().col.iter().map(|&c| c as u64)))));
    for k in [1usize, 6] {
        let t = harmonic_transformation(&r, &[], k).unwrap();
        words.push((
            format!("ring T k{k}"),
            fnv(bits(&t.t().val).chain(t.t().col.iter().map(|&c| c as u64))),
        ));
    }
    let mut o = ModalOptions::new(3).tolerance(1e-9);
    o.max_iterations = 400;
    for k in [0usize, 1, 6] {
        let h = modal_analysis_harmonic(&r, &[], k, &o).unwrap();
        let mut w: Vec<u64> = bits(&h.eigenvalues).collect();
        for m in &h.modes {
            w.extend(bits(&m.cos));
            w.extend(bits(&m.sin));
        }
        words.push((format!("ring modes k{k}"), fnv(w)));
    }
    let rotor = full_rotor(&r).unwrap();
    words.push((
        "ring full rotor".into(),
        fnv(rotor
            .mesh()
            .nodes()
            .iter()
            .flat_map(|x| x.map(f64::to_bits))
            .chain(rotor.mesh().blocks()[0].connectivity().iter().map(|&c| c as u64))),
    ));
    // The node-on-both-boundaries refusal (R4 of ADR 0386).
    let mut both = SolidModel::new(r.mesh().clone());
    both.add_material("all", Material::isotropic_elastic(2e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    both.add_constraint(Constraint::cyclic("ymin", "ymin", AXIS, [0.0; 3], N)).unwrap();
    let text = match boundary(&both, 0) {
        Err(e @ FemError::InvalidValue { .. }) => e.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    };
    words.push(("refusal R4".into(), fnv(text.bytes().map(|b| b as u64))));
    // The spinning disc at k = 0.
    let d = disc();
    let s = mpc::solve_static_mpc(&d, &[], &[], &[], &StaticOptions::default()).unwrap();
    words.push(("disc u".into(), fnv(bits(s.result.displacement_values()))));
    words.push(("disc rho".into(), fnv(bits(&s.constraint_forces))));
    // Linear constraints.
    let l = linear_model();
    let s = mpc::solve_static_mpc(&l, &[], &[], &[], &StaticOptions::default()).unwrap();
    words.push(("linear u".into(), fnv(bits(s.result.displacement_values()))));
    words.push(("linear rho".into(), fnv(bits(&s.constraint_forces))));
    let mut ok = true;
    for (name, h) in &words {
        let want = BASE.iter().find(|b| b.0 == name).map(|b| b.1);
        println!("B1 {name}: {h:#018x} (base {want:#018x?})");
        ok &= want == Some(*h);
    }
    assert!(ok, "B1: a default path changed its bits");
}

/// The words recorded on main b154dbe6 (ADR 0456 B1).
const BASE: &[(&str, u64)] = &[
    ("ring pairs", 0xc8cbad4ffd0742ec),
    ("ring T k0", 0x4b7482bf9e734622),
    ("ring T k1", 0x83f2361c262b66c4),
    ("ring T k6", 0x8b2215da6ded3722),
    ("ring modes k0", 0x71cddcce757f6ce0),
    ("ring modes k1", 0xab3a472ecc2dff48),
    ("ring modes k6", 0x5a2dfd0207f3a9ec),
    ("ring full rotor", 0xa62435442adf9425),
    ("refusal R4", 0xbcddbc8808cd8b22),
    ("disc u", 0x84e9fc40a1b2d473),
    ("disc rho", 0x96faafcbfb43bdf9),
    ("linear u", 0x540b9e2b3d758676),
    ("linear rho", 0x4e6f010b91d7ac66),
];
