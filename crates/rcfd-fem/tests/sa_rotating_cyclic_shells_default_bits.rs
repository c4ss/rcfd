//! F-647 (ADR 0561) B4: the default-options results of the Hex8 twin sector on the rotating paths and the full rotor, hashed;
//! the hash was recorded by running this file on the unchanged library (ADR 0561 commit ffef73e8 plus only this file).

use rcfd_fem::cyclic::{full_rotor, modal_analysis_harmonic};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, RotatingSector};
use rcfd_fem::{Constraint, ElementKind, FaceRef, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const N: usize = 6;
const NC: usize = 2;
const NA: usize = 3;
const T: f64 = 0.05;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
const SPEEDS: [f64; 3] = [0.0, 200.0, 400.0];
/// Marker of a loss of stiffness in the hashed values.
const LOSS: f64 = -1.0;

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn point(i: usize, s: f64, dr: f64) -> Vec3 {
    let n = axis();
    let l = 5.0f64.sqrt();
    let ea = [2.0 / l, -1.0 / l, 0.0];
    let eb = cross(n, ea);
    let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
    let r = 1.0 - 0.4 * s + dr;
    let z = 0.8 * s;
    let (sp, cp) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (cp * ea[k] + sp * eb[k]) + z * n[k])
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

fn fnv(values: impl IntoIterator<Item = f64>) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

/// The Hex8 twin sector of ADR 0556 B3: surface sets `low` / `high`, node sets `low_nodes` / `high_nodes`, `clamp`, `all`.
fn solid_sector() -> SolidMesh {
    let id = |i: usize, a: usize, w: usize| w * (NA + 1) * (NC + 1) + a * (NC + 1) + i;
    let mut nodes = Vec::new();
    for w in 0..2 {
        for a in 0..=NA {
            for i in 0..=NC {
                nodes.push(point(i, a as f64 / NA as f64, (w as f64 - 0.5) * 2.0 * T));
            }
        }
    }
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..NC {
            for w in 0..2 {
                conn.extend([id(i, a, w), id(i + 1, a, w), id(i + 1, a + 1, w), id(i, a + 1, w)]);
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = NA * NC;
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let col = |i: usize| -> Vec<usize> {
        let mut v: Vec<usize> = (0..2).flat_map(|w| (0..=NA).map(move |a| id(i, a, w))).collect();
        v.sort_unstable();
        v
    };
    for (name, i) in [("low", 0), ("high", NC)] {
        let nodes = col(i);
        let mut faces = Vec::new();
        for e in 0..ne {
            for f in 0..6 {
                let fr = FaceRef { element: e, face: f };
                if mesh.face_nodes(fr).iter().all(|p| nodes.binary_search(p).is_ok()) {
                    faces.push(fr);
                }
            }
        }
        mesh.add_surface_set(name, faces).unwrap();
        mesh.add_node_set(&format!("{name}_nodes"), nodes).unwrap();
    }
    let mut clamp: Vec<usize> = (0..2).flat_map(|w| (0..=NC).map(move |i| id(i, 0, w))).collect();
    clamp.sort_unstable();
    mesh.add_node_set("clamp", clamp).unwrap();
    mesh
}

fn mat() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn with_clamp(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("clamp", comp, 0.0)).unwrap();
    }
    m
}

/// The values of B3 / B4 with the default options.
fn default_values() -> Vec<f64> {
    let mut m = with_clamp(solid_sector());
    m.add_constraint(Constraint::cyclic("low", "high", axis(), ORIGIN, N))
        .unwrap();
    let mut out = Vec::new();
    let opts = RotatingOptions::new(3).modal(modal_options(3));
    let s = RotatingSector::new(&m, &opts).unwrap();
    for k in 0..=N / 2 {
        for w in SPEEDS {
            match s.modes(k, w).unwrap().modes() {
                Some(h) => out.extend(&h.eigenvalues),
                None => out.push(LOSS),
            }
        }
    }
    let sw = RotatingSector::new(&m, &opts.clone().coriolis(true)).unwrap();
    for k in 0..=N / 2 {
        for w in SPEEDS {
            match sw.whirl_modes(k, w).unwrap().modes() {
                Some(r) => out.extend(r.modes.iter().map(|x| x.omega)),
                None => out.push(LOSS),
            }
        }
    }
    let rotor = full_rotor(&m).unwrap();
    out.extend(rotor.mesh().nodes().iter().flatten());
    let ring = with_clamp(rotor.mesh().clone());
    let (sd, rd) = (m.dof_map(), ring.dof_map());
    let h = modal_analysis_harmonic(&m, &[], 1, &modal_options(3)).unwrap();
    out.extend(rotor.expand(&sd, &rd, 1, &h.modes[0]));
    let n = sd.n_equations();
    let fa: Vec<f64> = (0..n).map(|i| (0.37 * i as f64 + 0.1).sin()).collect();
    let fb: Vec<f64> = (0..n).map(|i| (0.53 * i as f64 + 0.2).cos()).collect();
    out.extend(rotor.expand_field(&sd, &rd, 1, &fa, &fb));
    out.extend(rotor.expand_load(&sd, &rd, 1, &fa, &fb));
    let part = RotatingPart::new(
        &ring,
        axis(),
        ORIGIN,
        &RotatingOptions::new(6).modal(modal_options(6)).coriolis(true),
    )
    .unwrap();
    match part.whirl_modes(400.0).unwrap().modes() {
        Some(r) => out.extend(r.modes.iter().map(|x| x.omega)),
        None => out.push(LOSS),
    }
    out
}

/// B4: recorded on the unchanged library.
const B4_HASH: u64 = 0x8963b28998b2cdc6;

#[test]
fn b4_solid_twin_rotating_and_full_rotor_default_bits_recorded() {
    let v = default_values();
    let h = fnv(v.iter().copied());
    println!("B4 hash {h:#018x} over {} values", v.len());
    assert_eq!(h, B4_HASH, "B4: {h:#018x}");
}
