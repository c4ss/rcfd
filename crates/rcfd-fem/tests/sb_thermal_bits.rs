//! F-380 (ADR 0281): check B1, a model without a temperature load gives the bits it gave before the thermal strain was
//! added. Uses only API that existed at d4bde1e0; the expected digests were printed by this file run on a copy of
//! d4bde1e0 (ADR 0281 D4) and written into `EXPECTED` before any library code of F-380.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::{body_force_loads, nodal_load_vector, NodalForceAt};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use std::collections::BTreeSet;

const SEED: u64 = 0xF380_0281;
const KINDS: [ElementKind; 3] = [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10];

/// Digests of d4bde1e0, (model, kind) in the order of the run, from the base run (`f380-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 9]> = Some([
    ("uniaxial Tet4", 0x5e24_47d3_c205_f2fe),
    ("uniaxial Hex8", 0xddc7_5712_a254_3ae1),
    ("uniaxial Tet10", 0xab8a_d8e1_76f9_9ac7),
    ("all_loads Tet4", 0xc7bd_db21_5029_5356),
    ("all_loads Hex8", 0x4b7d_0920_8694_a7b0),
    ("all_loads Tet10", 0xb126_5346_c31c_cdc3),
    ("bar Tet4", 0x14dd_5221_4524_0e4e),
    ("bar Hex8", 0x118f_07a0_9bf5_7b82),
    ("bar Tet10", 0xc7c7_50ba_3e78_476d),
]);

struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

fn kind_index(kind: ElementKind) -> u64 {
    match kind {
        ElementKind::Tet4 => 0,
        ElementKind::Hex8 => 1,
        _ => 2,
    }
}

fn rebuild(mesh: &SolidMesh, nodes: Vec<Vec3>) -> SolidMesh {
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    out
}

fn box_of(kind: ElementKind, origin: Vec3, lengths: Vec3, div: [usize; 3], perturb: Option<u64>) -> SolidMesh {
    let base = match kind {
        ElementKind::Hex8 => hex_box(origin, lengths, div).unwrap(),
        _ => tet_box(origin, lengths, div).unwrap(),
    };
    let base = match perturb {
        None => base,
        Some(tag) => {
            let mut rng = Rng::new(tag);
            let mut boundary = BTreeSet::new();
            for f in BOX_FACES {
                boundary.extend(base.node_set(f).unwrap().iter().copied());
            }
            let h = [
                lengths[0] / div[0] as f64,
                lengths[1] / div[1] as f64,
                lengths[2] / div[2] as f64,
            ];
            let mut nodes = base.nodes().to_vec();
            for (i, p) in nodes.iter_mut().enumerate() {
                if !boundary.contains(&i) {
                    for k in 0..3 {
                        p[k] += 0.1 * h[k] * (2.0 * rng.uniform() - 1.0);
                    }
                }
            }
            rebuild(&base, nodes)
        }
    };
    match kind {
        ElementKind::Tet10 => upgrade_to_tet10(&base).unwrap().0,
        _ => base,
    }
}

fn node_near(mesh: &SolidMesh, x: Vec3) -> usize {
    let mut best = (f64::INFINITY, 0);
    for (i, p) in mesh.nodes().iter().enumerate() {
        let d = (0..3).map(|k| (p[k] - x[k]).powi(2)).sum::<f64>();
        if d < best.0 {
            best = (d, i);
        }
    }
    assert!(best.0 < 1e-24, "no node at {x:?}");
    best.1
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap())
        .unwrap();
    m
}

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
struct Digest(u64);

impl Digest {
    fn new() -> Digest {
        Digest(0xCBF2_9CE4_8422_2325)
    }
    fn f(&mut self, v: f64) {
        for b in v.to_bits().to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    fn all(&mut self, v: &[f64]) {
        for &x in v {
            self.f(x);
        }
    }
}

fn digest_of(model: &SolidModel, prescribed: &[Prescribed], extra: &[NodalForceAt]) -> u64 {
    let mut d = Digest::new();
    d.all(&nodal_load_vector(model, extra).unwrap());
    let r = solve_static_with(model, prescribed, extra, &StaticOptions::default()).unwrap();
    d.all(r.displacement_values());
    d.all(r.reactions().unwrap());
    d.f(r.stats().unwrap().relative_residual());
    let field = IntegrationPointField::from_result(model, &r).unwrap();
    for p in field.positions() {
        d.all(p);
    }
    d.all(field.weights());
    for s in field.strain() {
        d.all(s);
    }
    for s in field.stress() {
        d.all(s);
    }
    for s in field.element_means(field.stress()).unwrap() {
        d.all(&s);
    }
    for s in field.element_means(field.strain()).unwrap() {
        d.all(&s);
    }
    d.f(field.strain_energy());
    for s in nodal_average(model, &field, field.stress(), None).unwrap() {
        match s {
            Some(v) => d.all(&v),
            None => d.f(f64::NAN),
        }
    }
    d.0
}

fn uniaxial(kind: ElementKind) -> u64 {
    let mesh = box_of(kind, [0.0; 3], [1.2, 1.0, 0.8], [3, 3, 2], Some(10 + kind_index(kind)));
    let mut m = model_of(mesh);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [1.0e4, -2.0e3, 3.0e3])).unwrap();
    digest_of(&m, &[], &[])
}

fn all_loads(kind: ElementKind) -> u64 {
    let mesh = box_of(kind, [0.0; 3], [1.2, 1.0, 0.8], [3, 3, 2], Some(10 + kind_index(kind)));
    let n = mesh.nodes().len();
    let mut m = model_of(mesh);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("ymax", [1e5, -2e5, 3e5])).unwrap();
    m.add_load(Load::pressure("xmax", 2.5e5)).unwrap();
    m.add_load(Load::gravity([0.3, -9.81, 1.2])).unwrap();
    m.add_load(Load::centrifugal([1.0, 2.0, 2.0], [-0.5, 0.3, 0.1], 300.0))
        .unwrap();
    let values: Vec<Vec3> = m
        .mesh()
        .nodes()
        .iter()
        .map(|x| [1e3 + 1e3 * x[1], -2e3 + 5e2 * x[2], 5e2 - 3e2 * x[0]])
        .collect();
    assert_eq!(values.len(), n);
    m.add_load(Load::body_force_nodal(values)).unwrap();
    let extra = body_force_loads(&m, |x| [2e3 * x[0], -1e3, 4e2 * x[1] - 1e2 * x[2]]).unwrap();
    digest_of(&m, &[], &extra)
}

fn bar(kind: ElementKind) -> u64 {
    let mesh = box_of(
        kind,
        [0.0, -0.05, -0.05],
        [1.0, 0.1, 0.1],
        [8, 2, 2],
        Some(20 + kind_index(kind)),
    );
    let a = node_near(&mesh, [0.0, 0.0, 0.0]);
    let c = node_near(&mesh, [0.0, 0.05, 0.0]);
    let mut m = model_of(mesh);
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_load(Load::pressure("xmax", 1.0e6)).unwrap();
    let pres = [
        Prescribed::new(a, 1, 0.0),
        Prescribed::new(a, 2, 0.0),
        Prescribed::new(c, 2, 0.0),
    ];
    digest_of(&m, &pres, &[])
}

#[test]
fn b1_models_without_temperature_keep_their_bits() {
    let mut got: Vec<(String, u64)> = Vec::new();
    for (name, f) in [
        ("uniaxial", uniaxial as fn(ElementKind) -> u64),
        ("all_loads", all_loads),
        ("bar", bar),
    ] {
        for kind in KINDS {
            let label = format!("{name} {}", kind.name());
            let d = f(kind);
            println!("B1 digest {label}: {d:#018x}");
            got.push((label, d));
        }
    }
    match EXPECTED {
        None => panic!("B1: no expected digests yet (the base run prints them)"),
        Some(expected) => {
            let mut differ = 0;
            for ((label, d), (el, ed)) in got.iter().zip(expected.iter()) {
                assert_eq!(label, el, "model order");
                let same = d == ed;
                println!(
                    "B1 {label}: {d:#018x} against {ed:#018x}: {}",
                    if same { "equal" } else { "DIFFERENT" }
                );
                differ += usize::from(!same);
            }
            assert_eq!(differ, 0, "B1: {differ} digests differ from d4bde1e0");
        }
    }
}
