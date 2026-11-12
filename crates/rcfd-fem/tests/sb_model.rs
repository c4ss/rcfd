//! F-370, ADR 0271: the solid data model and the structured builders. Checks K1 to K5, Q1 to Q4, R1 to R6 and D1 of the
//! registration. Every model is built through the public constructors (the request of ADR 0290 section 2.4).

use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::{
    Constraint, DofMap, ElementKind, FaceRef, FemError, Load, Material, SetKind, SolidMesh, SolidModel, SolidResult,
    SolverStats,
};
use std::collections::BTreeSet;

const ORIGIN: [f64; 3] = [0.25, -0.5, 1.0];
const LENGTHS: [f64; 3] = [1.3, 0.7, 2.1];
const NS: [usize; 3] = [1, 2, 5];

fn edges_closed_form(n: usize) -> usize {
    3 * n * (n + 1) * (n + 1) + 3 * n * n * (n + 1) + n * n * n
}

fn face_area_box(f: usize) -> f64 {
    let [lx, ly, lz] = LENGTHS;
    match f / 2 {
        0 => ly * lz,
        1 => lx * lz,
        _ => lx * ly,
    }
}

fn box_normal(f: usize) -> [f64; 3] {
    let mut v = [0.0; 3];
    v[f / 2] = if f % 2 == 0 { -1.0 } else { 1.0 };
    v
}

/// Whether `b` lists the cycle `a` in the opposite cyclic order.
fn opposite_cycle(a: &[usize], b: &[usize]) -> bool {
    let k = a.len();
    let rev: Vec<usize> = b.iter().rev().copied().collect();
    (0..k).any(|s| (0..k).all(|i| a[i] == rev[(i + s) % k]))
}

/// K1 to K5 (and Q3 for Tet10) on one mesh; returns (min det J, volume error, max area error, max normal error).
fn geometry(mesh: &SolidMesh, n: usize, label: &str, elements: usize, boundary: usize, interior: usize) -> [f64; 4] {
    // K5
    assert_eq!(mesh.n_elements(), elements, "{label} n={n}: element count");
    // K1
    let mut min_j = f64::INFINITY;
    for e in 0..mesh.n_elements() {
        for j in mesh.jacobians_at_nodes(e) {
            min_j = min_j.min(j);
        }
    }
    assert!(min_j > 0.0, "{label} n={n}: min det J = {min_j:e}");
    // K2
    let vol: f64 = (0..mesh.n_elements()).map(|e| mesh.element_volume(e)).sum();
    let v_box = LENGTHS[0] * LENGTHS[1] * LENGTHS[2];
    let vol_err = (vol - v_box).abs() / v_box;
    assert!(vol_err <= 1e-12, "{label} n={n}: volume error {vol_err:e}");
    // K3, K3b
    let per_face = if label == "hex" { n * n } else { 2 * n * n };
    let mut area_err: f64 = 0.0;
    let mut normal_err: f64 = 0.0;
    let mut boundary_keys = BTreeSet::new();
    for (f, name) in BOX_FACES.iter().enumerate() {
        let faces = mesh.surface_set(name).expect("box face set");
        assert_eq!(faces.len(), per_face, "{label} n={n}: faces of {name}");
        let a: f64 = faces.iter().map(|&fr| mesh.face_area(fr)).sum();
        area_err = area_err.max((a - face_area_box(f)).abs() / face_area_box(f));
        let nb = box_normal(f);
        for &fr in faces {
            let av = mesh.face_area_vector(fr);
            let m = (av[0] * av[0] + av[1] * av[1] + av[2] * av[2]).sqrt();
            for i in 0..3 {
                normal_err = normal_err.max((av[i] / m - nb[i]).abs());
            }
            let mut key = mesh.face_nodes(fr);
            key.truncate(mesh.element_kind(fr.element).face_corners());
            key.sort_unstable();
            assert!(boundary_keys.insert(key), "{label} n={n}: a face in two box sets");
        }
    }
    assert!(area_err <= 1e-12, "{label} n={n}: face area error {area_err:e}");
    assert!(normal_err <= 1e-14, "{label} n={n}: normal error {normal_err:e}");
    // K4
    let inc = mesh.face_incidence();
    let mut ones = BTreeSet::new();
    let mut twos = 0;
    for (key, members) in &inc {
        match members.len() {
            1 => {
                ones.insert(key.clone());
            }
            2 => {
                twos += 1;
                let corners = mesh.element_kind(members[0].element).face_corners();
                let a = &mesh.face_nodes(members[0])[..corners];
                let b = &mesh.face_nodes(members[1])[..corners];
                assert!(opposite_cycle(a, b), "{label} n={n}: shared face {key:?} not oppositely oriented");
                assert_ne!(members[0].element, members[1].element);
            }
            k => panic!("{label} n={n}: face {key:?} has {k} members"),
        }
    }
    assert_eq!(ones.len(), boundary, "{label} n={n}: boundary faces");
    assert_eq!(ones, boundary_keys, "{label} n={n}: faces with one element are the six face sets");
    assert_eq!(twos, interior, "{label} n={n}: interior faces");
    println!(
        "{label} n={n}: elements {elements}, nodes {}, min det J {min_j:.6e}, volume error {vol_err:.3e}, \
         max face-set area error {area_err:.3e}, max normal error {normal_err:.3e}, boundary {boundary}, interior {twos}",
        mesh.nodes().len()
    );
    [min_j, vol_err, area_err, normal_err]
}

#[test]
fn k_hex_box_geometry() {
    for n in NS {
        let m = hex_box(ORIGIN, LENGTHS, [n, n, n]).expect("hex box");
        assert_eq!(m.nodes().len(), (n + 1).pow(3));
        geometry(&m, n, "hex", n.pow(3), 6 * n * n, 3 * n * n * (n - 1));
    }
}

#[test]
fn k_tet_box_geometry() {
    for n in NS {
        let m = tet_box(ORIGIN, LENGTHS, [n, n, n]).expect("tet box");
        assert_eq!(m.nodes().len(), (n + 1).pow(3));
        geometry(&m, n, "tet", 6 * n.pow(3), 12 * n * n, 12 * n.pow(3) - 6 * n * n);
    }
}

#[test]
fn q_quadratic_upgrade() {
    // The registered local edge order of Tet10 (ADR 0271 D2), typed here independently of the library's table.
    let edges = [[0usize, 1usize], [1, 2], [0, 2], [0, 3], [1, 3], [2, 3]];
    for n in NS {
        let lin = tet_box(ORIGIN, LENGTHS, [n, n, n]).expect("tet box");
        // Q1: distinct edges counted from the Tet4 connectivity (all six pairs of each element).
        let mut distinct = BTreeSet::new();
        for e in 0..lin.n_elements() {
            let v = lin.element_nodes(e);
            for a in 0..4 {
                for b in a + 1..4 {
                    distinct.insert((v[a].min(v[b]), v[a].max(v[b])));
                }
            }
        }
        let (quad, mids) = upgrade_to_tet10(&lin).expect("upgrade");
        let n_lin = lin.nodes().len();
        assert_eq!(distinct.len(), edges_closed_form(n), "n={n}: edges counted vs closed form");
        assert_eq!(quad.nodes().len(), n_lin + edges_closed_form(n), "n={n}: Tet10 nodes");
        assert_eq!(quad.nodes().len(), n_lin + distinct.len());
        assert_eq!(mids.len(), edges_closed_form(n));
        // Q2
        let x = quad.nodes();
        let mut err: f64 = 0.0;
        for e in 0..quad.n_elements() {
            assert_eq!(quad.element_kind(e), ElementKind::Tet10);
            let v = quad.element_nodes(e);
            assert_eq!(&v[..4], lin.element_nodes(e), "corners kept");
            for (k, [a, b]) in edges.iter().enumerate() {
                let m = v[4 + k];
                for i in 0..3 {
                    err = err.max((x[m][i] - 0.5 * (x[v[*a]][i] + x[v[*b]][i])).abs());
                }
            }
        }
        for md in &mids {
            for i in 0..3 {
                err = err.max((x[md.node][i] - 0.5 * (x[md.ends[0]][i] + x[md.ends[1]][i])).abs());
            }
        }
        assert!(err <= 1e-15, "n={n}: mid-edge error {err:e}");
        println!(
            "tet10 n={n}: nodes {} = {} + {} edges (closed form {}), max mid-edge error {err:.3e}",
            quad.nodes().len(),
            n_lin,
            distinct.len(),
            edges_closed_form(n)
        );
        // Q3: the Tet10 geometry.
        geometry(&quad, n, "tet10", 6 * n.pow(3), 12 * n * n, 12 * n.pow(3) - 6 * n * n);
        // The node sets of the box faces gain the mid-edge nodes of their edges: equal to the surface-set nodes.
        for name in BOX_FACES {
            let mut s = quad.node_set(name).expect("set").to_vec();
            s.sort_unstable();
            assert_eq!(s, quad.surface_set_nodes(name).expect("set"), "n={n}: node set {name}");
        }
    }
}

#[test]
fn q4_upgrade_refuses_hexahedra() {
    let m = hex_box(ORIGIN, LENGTHS, [1, 1, 1]).expect("hex box");
    let e = upgrade_to_tet10(&m).unwrap_err();
    assert_eq!(e, FemError::UnsupportedKind { block: 0, kind: "Hex8", operation: "upgrade_to_tet10" });
    assert!(e.to_string().contains("block 0") && e.to_string().contains("Hex8"), "{e}");
}

fn connectivity(m: &SolidMesh) -> Vec<usize> {
    m.blocks()[0].connectivity().to_vec()
}

#[test]
fn r1_inverted_element() {
    let t = tet_box(ORIGIN, LENGTHS, [2, 2, 2]).expect("tet box");
    let mut c = connectivity(&t);
    c.swap(5 * 4 + 1, 5 * 4 + 2);
    let mut m = SolidMesh::new(t.nodes().to_vec()).expect("nodes");
    let e = m.add_block(ElementKind::Tet4, c).unwrap_err();
    assert!(matches!(e, FemError::InvertedElement { element: 5, .. }), "{e:?}");
    assert!(e.to_string().contains("element 5"), "{e}");
    println!("R1 tet: {e}");

    let h = hex_box(ORIGIN, LENGTHS, [1, 1, 1]).expect("hex box");
    let c0 = connectivity(&h);
    let c: Vec<usize> = c0[4..8].iter().chain(c0[0..4].iter()).copied().collect();
    let mut m = SolidMesh::new(h.nodes().to_vec()).expect("nodes");
    let e = m.add_block(ElementKind::Hex8, c).unwrap_err();
    assert!(matches!(e, FemError::InvertedElement { element: 0, .. }), "{e:?}");
    assert!(e.to_string().contains("element 0"), "{e}");
    println!("R1 hex: {e}");

    let (q, _) = upgrade_to_tet10(&tet_box(ORIGIN, LENGTHS, [1, 1, 1]).expect("tet box")).expect("upgrade");
    let v = q.element_nodes(0).to_vec();
    let mut nodes = q.nodes().to_vec();
    let (x0, x1) = (nodes[v[0]], nodes[v[1]]);
    nodes[v[4]] = [0, 1, 2].map(|i| x0[i] + 1.5 * (x1[i] - x0[i]));
    let mut m = SolidMesh::new(nodes).expect("nodes");
    let e = m.add_block(ElementKind::Tet10, connectivity(&q)).unwrap_err();
    assert!(matches!(e, FemError::InvertedElement { element: 0, .. }), "{e:?}");
    assert!(e.to_string().contains("element 0"), "{e}");
    println!("R1 tet10: {e}");
}

#[test]
fn r2_repeated_node() {
    let t = tet_box(ORIGIN, LENGTHS, [2, 2, 2]).expect("tet box");
    let mut c = connectivity(&t);
    c[3 * 4 + 3] = c[3 * 4];
    let mut m = SolidMesh::new(t.nodes().to_vec()).expect("nodes");
    let e = m.add_block(ElementKind::Tet4, c.clone()).unwrap_err();
    assert_eq!(e, FemError::RepeatedNode { element: 3, node: c[12] });
    assert!(e.to_string().contains("element 3"), "{e}");
    println!("R2: {e}");
}

#[test]
fn r3_empty_sets() {
    let mut m = hex_box(ORIGIN, LENGTHS, [1, 1, 1]).expect("hex box");
    let cases = [
        (m.add_node_set("empty_set", vec![]).unwrap_err(), SetKind::Node),
        (m.add_element_set("empty_set", vec![]).unwrap_err(), SetKind::Element),
        (m.add_surface_set("empty_set", vec![]).unwrap_err(), SetKind::Surface),
    ];
    for (e, kind) in cases {
        assert_eq!(e, FemError::EmptySet { kind, name: "empty_set".into() });
        assert!(e.to_string().contains("'empty_set'"), "{e}");
        println!("R3: {e}");
    }
}

#[test]
fn r4_youngs_modulus() {
    for e in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let err = Material::isotropic_elastic(e, 0.3, 7800.0, 1.2e-5).unwrap_err();
        assert!(matches!(err, FemError::InvalidMaterial { key: "e", .. }), "{err:?}");
        assert!(err.to_string().contains("'e'"), "{err}");
        println!("R4: {err}");
    }
    let m = Material::isotropic_elastic(1e-300, 0.3, 0.0, 0.0).expect("control");
    assert_eq!(m.youngs_modulus(), 1e-300);
}

#[test]
fn r5_poisson_ratio() {
    for nu in [-1.0, 0.5, -1.5, 0.7, f64::NAN] {
        let err = Material::isotropic_elastic(2.1e11, nu, 7800.0, 1.2e-5).unwrap_err();
        assert!(matches!(err, FemError::InvalidMaterial { key: "nu", .. }), "{err:?}");
        assert!(err.to_string().contains("'nu'"), "{err}");
        println!("R5: {err}");
    }
    for nu in [-0.999, 0.0, 0.499] {
        let m = Material::isotropic_elastic(2.1e11, nu, 7800.0, 1.2e-5).expect("control");
        assert_eq!(m.poisson_ratio(), nu);
    }
}

fn steel() -> Material {
    Material::isotropic_elastic(2.1e11, 0.3, 7800.0, 1.2e-5).expect("steel")
}

#[test]
fn r6_other_refusals_and_a_valid_model() {
    let h = hex_box(ORIGIN, LENGTHS, [1, 1, 1]).expect("hex box");
    let nodes = h.nodes().to_vec();
    // Node out of range.
    let mut c = connectivity(&h);
    c[7] = 99;
    let e = SolidMesh::new(nodes.clone()).unwrap().add_block(ElementKind::Hex8, c).unwrap_err();
    assert_eq!(e, FemError::NodeOutOfRange { element: 0, node: 99, n_nodes: 8 });
    assert!(e.to_string().contains("element 0"));
    // Connectivity of wrong length.
    let e = SolidMesh::new(nodes.clone()).unwrap().add_block(ElementKind::Hex8, vec![0, 1, 2]).unwrap_err();
    assert_eq!(e, FemError::ConnectivityLength { block: 0, len: 3, nodes_per_element: 8 });
    assert!(e.to_string().contains("block 0"));
    // Non-finite node.
    let mut bad = nodes.clone();
    bad[4][1] = f64::NAN;
    assert_eq!(SolidMesh::new(bad).unwrap_err(), FemError::NonFiniteNode { node: 4 });
    // Duplicate set name; face index out of range.
    let mut m = h.clone();
    let e = m.add_node_set("xmin", vec![0]).unwrap_err();
    assert_eq!(e, FemError::DuplicateSet { kind: SetKind::Node, name: "xmin".into() });
    let e = m.add_surface_set("f", vec![FaceRef { element: 0, face: 6 }]).unwrap_err();
    assert_eq!(e, FemError::FaceOutOfRange { set: "f".into(), element: 0, face: 6 });
    assert!(e.to_string().contains("element 0") && e.to_string().contains("'f'"));
    // Negative density.
    let e = Material::isotropic_elastic(1.0, 0.3, -1.0, 0.0).unwrap_err();
    assert!(matches!(e, FemError::InvalidMaterial { key: "density", .. }));
    assert!(e.to_string().contains("'density'"));
    // Material on an unknown element set.
    let mut model = SolidModel::new(hex_box(ORIGIN, LENGTHS, [2, 2, 2]).unwrap());
    let e = model.add_material("nowhere", steel()).unwrap_err();
    assert_eq!(e, FemError::UnknownSet { kind: SetKind::Element, name: "nowhere".into() });
    assert!(e.to_string().contains("'nowhere'"));
    // Element without material, element with two.
    assert_eq!(model.validate().unwrap_err(), FemError::MaterialUnassigned { element: 0 });
    let mut m2 = hex_box(ORIGIN, LENGTHS, [2, 2, 2]).unwrap();
    m2.add_element_set("half", vec![3, 4]).unwrap();
    let mut model2 = SolidModel::new(m2);
    model2.add_material("all", steel()).unwrap();
    model2.add_material("half", steel()).unwrap();
    let e = model2.validate().unwrap_err();
    assert_eq!(e, FemError::MaterialOverlap { element: 3, first: "all".into(), second: "half".into() });
    assert!(e.to_string().contains("element 3"));
    assert!(model2.material_of(3).is_none() && model2.material_of(0).is_some());
    // Constraint component 3.
    let e = model.add_constraint(Constraint::displacement("xmin", 3, 0.0)).unwrap_err();
    assert!(matches!(e, FemError::ComponentOutOfRange { component: 3, available: 3, .. }), "{e:?}");
    assert!(e.to_string().contains("'xmin'") && e.to_string().contains("node"));
    // Nodal force on an unknown set; zero axis; temperature of wrong length.
    let e = model.add_load(Load::nodal_force("tip", [1.0, 0.0, 0.0])).unwrap_err();
    assert_eq!(e, FemError::UnknownSet { kind: SetKind::Node, name: "tip".into() });
    let e = model.add_load(Load::centrifugal([0.0; 3], [0.0; 3], 100.0)).unwrap_err();
    assert!(matches!(e, FemError::InvalidValue { key: "axis", .. }));
    assert!(e.to_string().contains("'axis'"));
    let e = model.add_load(Load::temperature(vec![300.0; 5], 293.0)).unwrap_err();
    assert_eq!(e, FemError::FieldLength { key: "nodal", expected: 27, found: 5 });
    // A traction on a node set name that is not a surface set.
    let e = model.add_load(Load::traction("all", [0.0, 0.0, 1.0])).unwrap_err();
    assert_eq!(e, FemError::UnknownSet { kind: SetKind::Surface, name: "all".into() });
    // SolidResult of wrong length.
    let e = SolidResult::new(DofMap::uniform(27, 3), vec![0.0; 80]).unwrap_err();
    assert_eq!(e, FemError::FieldLength { key: "displacement", expected: 81, found: 80 });

    // Control: a valid model with every load kind.
    model.add_material("all", steel()).unwrap();
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    model.add_constraint(Constraint::displacement("xmax", 0, 1e-4)).unwrap();
    model.add_load(Load::nodal_force("xmax", [1e3, 0.0, 0.0])).unwrap();
    model.add_load(Load::traction("zmax", [0.0, 0.0, -1e5])).unwrap();
    model.add_load(Load::pressure("ymin", 2e5)).unwrap();
    model.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    model.add_load(Load::centrifugal([0.0, 0.0, 1.0], [0.0; 3], 300.0)).unwrap();
    model.add_load(Load::body_force_nodal(vec![[0.0, 1.0, 0.0]; 27])).unwrap();
    model.add_load(Load::temperature(vec![350.0; 27], 293.15)).unwrap();
    model.validate().expect("valid model");
    assert_eq!(model.loads().len(), 7);
    assert_eq!(model.constraints().len(), 4);
    assert_eq!(model.material_indices().unwrap(), vec![0; 8]);
    assert_eq!(model.dof_map(), DofMap::uniform(27, 3));

    // A result built through its constructors.
    let r = SolidResult::new(model.dof_map(), (0..81).map(|i| i as f64).collect())
        .unwrap()
        .with_reactions(vec![0.0; 81])
        .unwrap()
        .with_integration_points((0..=8).map(|e| 8 * e).collect(), vec![[0.0; 6]; 64], vec![[1.0; 6]; 64])
        .unwrap()
        .with_stats(SolverStats::new("skyline", 81, 0, 1e-15));
    assert_eq!(r.displacement(2), &[6.0, 7.0, 8.0]);
    assert_eq!(r.stress().len(), 64);
    assert_eq!(r.stats().unwrap().solver(), "skyline");
    let e = SolidResult::new(model.dof_map(), vec![0.0; 81])
        .unwrap()
        .with_integration_points(vec![0, 8, 4], vec![], vec![])
        .unwrap_err();
    assert!(matches!(e, FemError::InvalidValue { key: "offsets", .. }));
    // Conversion to the workspace error.
    let ce: rcfd_core::Error = FemError::RepeatedNode { element: 3, node: 1 }.into();
    assert!(matches!(ce, rcfd_core::Error::Mesh(ref s) if s.contains("element 3")));
    let ce: rcfd_core::Error = Material::isotropic_elastic(0.0, 0.3, 0.0, 0.0).unwrap_err().into();
    assert!(matches!(ce, rcfd_core::Error::InvalidValue { ref path, .. } if path == "e"));
}

#[test]
fn d1_dof_map() {
    let d = DofMap::uniform(4, 3);
    for n in 0..4 {
        for c in 0..3 {
            assert_eq!(d.equation(n, c), Some(3 * n + c));
            assert_eq!(d.node_component(3 * n + c), Some((n, c)));
        }
    }
    assert_eq!(d.equation(3, 3), None);
    assert_eq!(d.equation(4, 0), None);
    assert_eq!(d.n_equations(), 12);
    let m = DofMap::from_counts(&[3, 6, 3]);
    assert_eq!([m.equation(0, 0), m.equation(1, 0), m.equation(2, 0)], [Some(0), Some(3), Some(9)]);
    assert_eq!(m.n_equations(), 12);
    assert_eq!(m.node_component(5), Some((1, 2)));
    assert_eq!(m.node_component(12), None);
    assert_eq!(m.components(1), 6);
    assert_eq!(DofMap::for_mesh(&hex_box(ORIGIN, LENGTHS, [1, 1, 1]).unwrap()), DofMap::uniform(8, 3));
}
