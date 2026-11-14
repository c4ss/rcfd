//! F-591 (ADR 0502 amendment 1): check B0b, the rotating parts of the warped Shell4 plate and of its Shell3 twin (a `Shell`
//! block) give the bits they gave before F-591: the loss of stiffness at 100 rad/s (its speed and negative count) and the
//! frequencies and modes at 10 rad/s. Uses only API that existed at 70348e9f; the expected digests were printed by this file
//! run once on a `git archive` copy of 6ee966ca (no library line of F-591; `f591-scratch/b0b-base.log`) and written into
//! `EXPECTED`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Material, SolidMesh, SolidModel};

/// Digests of 6ee966ca in the order of the run, from the base run (`f591-scratch/b0b-base.log`).
const EXPECTED: Option<[(&str, u64); 5]> = Some([
    ("Shell4 loss at 100 rad/s", 0x7108_b6dd_3d72_e419),
    ("Shell4 modes at 10 rad/s", 0x6065_20c2_3bd3_b97b),
    ("Shell3 loss at 100 rad/s", 0x7108_b6dd_3d72_e419),
    ("Shell3 modes at 10 rad/s", 0x6f8f_d9ad_c1c5_74af),
    ("count", 0x0000_0000_0000_0004),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

const AXIS: Vec3 = [0.2, 0.3, 1.0];
const ORIGIN: Vec3 = [0.1, 0.05, -0.1];

/// The warped 3 x 3 plate of `sa_laminate_default_bits.rs`, edge `x = 0` clamped.
fn plate(kind: ElementKind) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..=3 {
        for i in 0..=3 {
            nodes.push([i as f64 / 3.0, j as f64 / 3.0, 0.02 * (i * j) as f64]);
        }
    }
    let idx = |i: usize, j: usize| j * 4 + i;
    let mut conn = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if kind == ElementKind::Shell3 {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    let ne = if kind == ElementKind::Shell3 { 18 } else { 9 };
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", (0..=3).map(|j| idx(0, j)).collect()).unwrap();
    mesh.add_node_set("x1", (0..=3).map(|j| idx(3, j)).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    m
}

/// The loss of stiffness at 100 rad/s (speed, negative count or -1) and the frequencies and modes at 10 rad/s (NaN markers
/// if either is not what its name says).
fn rotating_values(m: &SolidModel) -> (Vec<f64>, Vec<f64>) {
    let part = RotatingPart::new(m, AXIS, ORIGIN, &RotatingOptions::new(4).modal(ModalOptions::new(4))).unwrap();
    let loss = match part.modes(100.0).unwrap() {
        SpeedModes::LossOfStiffness(l) => vec![l.speed, l.negative.map(|n| n as f64).unwrap_or(-1.0)],
        SpeedModes::Modes(_) => vec![f64::NAN],
    };
    let modes = match part.modes(10.0).unwrap() {
        SpeedModes::Modes(r) => {
            let mut v = r.frequencies.clone();
            for phi in &r.modes {
                v.extend_from_slice(phi);
            }
            v
        }
        SpeedModes::LossOfStiffness(_) => vec![f64::NAN],
    };
    (loss, modes)
}

#[test]
fn b0b_default_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let (loss, modes) = rotating_values(&plate(ElementKind::Shell4));
    println!(
        "Shell4 loss {loss:?}, frequencies at 10 rad/s {:?}",
        &modes[..4.min(modes.len())]
    );
    got.push(("Shell4 loss at 100 rad/s", digest(&loss)));
    got.push(("Shell4 modes at 10 rad/s", digest(&modes)));
    let (loss, modes) = rotating_values(&plate(ElementKind::Shell3));
    println!(
        "Shell3 loss {loss:?}, frequencies at 10 rad/s {:?}",
        &modes[..4.min(modes.len())]
    );
    got.push(("Shell3 loss at 100 rad/s", digest(&loss)));
    got.push(("Shell3 modes at 10 rad/s", digest(&modes)));
    got.push(("count", got.len() as u64));
    for (name, d) in &got {
        println!(
            "    (\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    match EXPECTED {
        None => panic!("no expected digests yet: print them on the base"),
        Some(expected) => {
            let mut bad = Vec::new();
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                if name != en || d != ed {
                    bad.push(format!("{name}: 0x{d:016x} != 0x{ed:016x}"));
                }
            }
            assert_eq!(got.len(), expected.len(), "count");
            assert!(bad.is_empty(), "changed: {bad:?}");
        }
    }
}
