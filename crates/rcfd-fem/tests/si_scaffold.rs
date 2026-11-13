//! F-388 (ADR 0291): the scaffold of the intermediate tier. Checks X1 to X3, H1, B5, R1 to R11, V1 and the negative
//! controls N2, N3 of the registration (B1 to B4 and N1 are in `si_scaffold_bits.rs`).
//!
//! Maintainer decision 2026-11-13: when a package of the tier lands, the tests whose rows it retires stay as ignored
//! records of ADR 0291 and a copy without those rows is added (the `_after_` tests at the end of this file).

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::{assemble_with, stiffness, stiffness_with};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{element_for, ElementMatrix, ElementState};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::quadrature;
use rcfd_fem::solve::{solve_static, solve_static_report, solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElasticSupport, ElementKind, FaceRef, FemError, LinearTerm, Load, Material,
    SolidFormulation, SolidMesh, SolidModel,
};
use rcfd_linalg::CsrMatrix;

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn with_material(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m
}

fn clamp_xmin(m: &mut SolidModel) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
}

fn cantilever_mesh() -> SolidMesh {
    hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap()
}

/// The cantilever C of the registration on a given mesh.
fn cantilever_on(mesh: SolidMesh) -> SolidModel {
    let mut m = with_material(mesh);
    clamp_xmin(&mut m);
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    m
}

fn cantilever() -> SolidModel {
    cantilever_on(cantilever_mesh())
}

/// The expected refusal: item, variant, location (exact) and package.
fn expect_refusal(
    what: &str,
    r: Result<impl std::fmt::Debug, FemError>,
    item: &str,
    variant: &str,
    location: &str,
    package: &str,
) {
    let e = r.expect_err(what);
    println!("{what}: {e}");
    match &e {
        FemError::NotYetSupported {
            item: i,
            variant: v,
            location: l,
            package: p,
        } => {
            assert_eq!((*i, *v, l.as_str(), *p), (item, variant, location, package), "{what}");
        }
        other => panic!("{what}: not NotYetSupported but {other:?}"),
    }
    let msg = e.to_string();
    assert!(msg.contains(variant) && msg.contains(location), "{what}: message {msg}");
}

// ---------------------------------------------------------------------------------------------------------------- X1

#[test]
#[ignore = "record of ADR 0291 before ADR 0292 (Hex20) and ADR 0296 (mpc) landed; superseded by the _after_ copy below (maintainer, 2026-11-13)"]
fn x1_the_scaffold_exists() {
    assert_eq!(rcfd_fem::hex20::PENDING, Some("ADR 0292"));
    assert_eq!(rcfd_fem::hex8x::PENDING, Some("ADR 0293"));
    assert_eq!(rcfd_fem::beam::PENDING, Some("ADR 0294"));
    assert_eq!(rcfd_fem::shell::PENDING, Some("ADR 0295"));
    assert_eq!(rcfd_fem::mpc::PENDING, Some("ADR 0296"));
    assert_eq!(rcfd_fem::cyclic::PENDING, Some("ADR 0297"));
    // The modules without a pending variant exist by path (a `use` of the module is enough to compile against it).
    #[allow(unused_imports)]
    use rcfd_fem::{buckling, geometric, harmonic, nonlinear, rotating, transient};
    assert_eq!(ElementKind::Hex20.nodes_per_element(), 20);
    assert_eq!(ElementKind::Beam2.nodes_per_element(), 2);
    assert_eq!(ElementKind::Shell4.nodes_per_element(), 4);
    assert_eq!(ElementKind::Hex20.pending(), Some("ADR 0292"));
    for k in [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10] {
        assert_eq!(k.pending(), None);
        assert!(k.is_solid());
    }
    assert!(ElementKind::Hex20.is_solid() && !ElementKind::Beam2.is_solid() && !ElementKind::Shell4.is_solid());
    assert_eq!(SolidFormulation::default(), SolidFormulation::Full);
    assert_eq!(SolidFormulation::BBar.pending(), Some("ADR 0293"));
    let section = BeamSection::new(1e-4, 1e-9, 2e-9, 3e-9, 5.0 / 6.0, 5.0 / 6.0).unwrap();
    assert_eq!(section.area(), 1e-4);
    assert_eq!(BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap().name(), "Beam");
    assert_eq!(BlockProperty::shell(0.002).unwrap().name(), "Shell");
    assert_eq!(BlockProperty::solid(SolidFormulation::Incompatible).name(), "Solid");
    assert_eq!(
        Constraint::linear(vec![LinearTerm::new(0, 0, 1.0)], 0.0).name(),
        "Linear"
    );
    assert_eq!(Constraint::rigid_set("a", [0.0; 3]).name(), "RigidSet");
    assert_eq!(
        Constraint::cyclic("a", "b", [0.0, 0.0, 1.0], [0.0; 3], 12).name(),
        "Cyclic"
    );
    assert_eq!(ElasticSupport::nodal_spring("a", 0, 1.0).name(), "NodalSpring");
    assert_eq!(ElasticSupport::foundation("a", 1.0).name(), "Foundation");
    let m = cantilever();
    assert!(m.supports().is_empty());
    assert_eq!(m.mesh().blocks()[0].property(), None);
    assert_eq!(m.mesh().blocks()[0].formulation(), SolidFormulation::Full);
    // The three entry points are methods of the trait (checked by X2); the post modules are checked in rcfd-post
    // (si_scaffold_output.rs).
    println!("X1: every item of D1 to D5 exists");
}

// ---------------------------------------------------------------------------------------------------------------- X2

#[test]
#[ignore = "record of ADR 0291 before ADR 0390 (F-395, geometric stiffness) retired the geometric_stiffness row; superseded by the _after_geometric_landed copy below (maintainer, 2026-11-13)"]
fn x2_entry_points_refuse_by_default() {
    let mat = steel();
    let state = ElementState::empty();
    for kind in [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10] {
        let el = element_for(kind);
        let coords: Vec<Vec3> = kind.reference_nodes().to_vec();
        let n = el.stiffness_rule().len();
        let u = vec![0.0; 3 * kind.nodes_per_element()];
        let loc = format!("element kind {}", kind.name());
        expect_refusal(
            "geometric_stiffness",
            el.geometric_stiffness(&coords, &vec![[1.0; 6]; n]),
            "element entry point",
            "geometric_stiffness",
            &loc,
            "ADR 0298",
        );
        expect_refusal(
            "internal_force",
            el.internal_force(&coords, &mat, &u, &state),
            "element entry point",
            "internal_force",
            &loc,
            "ADR 0300",
        );
        expect_refusal(
            "tangent",
            el.tangent(&coords, &mat, &u, &state),
            "element entry point",
            "tangent",
            &loc,
            "ADR 0300",
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------- X3

#[test]
#[ignore = "record of ADR 0291 before ADR 0292 (Hex20) and ADR 0296 (mpc) landed; superseded by the _after_ copy below (maintainer, 2026-11-13)"]
fn x3_placeholder_elements_refuse() {
    let d = steel().elasticity_matrix().unwrap();
    for (kind, package) in [
        (ElementKind::Hex20, "ADR 0292"),
        (ElementKind::Beam2, "ADR 0294"),
        (ElementKind::Shell4, "ADR 0295"),
    ] {
        let el = element_for(kind);
        assert_eq!(el.kind(), kind);
        let n = kind.nodes_per_element();
        let coords: Vec<Vec3> = (0..n).map(|a| [a as f64, (a * a) as f64, 1.0]).collect();
        let u = vec![0.0; 3 * n];
        let name = kind.name();
        expect_refusal(
            "stiffness",
            el.stiffness(&coords, &d),
            "element kind",
            name,
            "element routines",
            package,
        );
        expect_refusal(
            "integration_points",
            el.integration_points(&coords, quadrature::gauss_2()),
            "element kind",
            name,
            "element routines",
            package,
        );
        expect_refusal(
            "strain_at",
            el.strain_at(&coords, &u, [0.0; 3]),
            "element kind",
            name,
            "element routines",
            package,
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------- H1

fn bits_equal(a: &CsrMatrix, b: &CsrMatrix) -> bool {
    a.n == b.n
        && a.row_ptr == b.row_ptr
        && a.col == b.col
        && a.val.len() == b.val.len()
        && a.val.iter().zip(&b.val).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn hook_models() -> Vec<(&'static str, SolidModel)> {
    let origin = [0.1, -0.2, 0.3];
    let lengths = [1.2, 1.0, 0.8];
    vec![
        ("P-Hex8", with_material(hex_box(origin, lengths, [3, 3, 3]).unwrap())),
        (
            "P-Tet10",
            with_material(
                upgrade_to_tet10(&tet_box(origin, lengths, [2, 2, 2]).unwrap())
                    .unwrap()
                    .0,
            ),
        ),
        ("Tet4", with_material(tet_box(origin, lengths, [2, 2, 2]).unwrap())),
        ("C", cantilever()),
    ]
}

fn hook(model: &SolidModel, threads: usize, scale: f64) -> CsrMatrix {
    assemble_with(model, Some(threads), |ctx| {
        let d = ctx.material.elasticity_matrix()?;
        let mut k: ElementMatrix = element_for(ctx.kind).stiffness(ctx.coords, &d)?;
        if scale != 1.0 {
            for v in k.values_mut() {
                *v *= scale;
            }
        }
        Ok(k)
    })
    .unwrap()
}

#[test]
fn h1_generic_hook_reproduces_stiffness() {
    for (name, model) in hook_models() {
        let k = stiffness(&model).unwrap();
        for t in [1, 8] {
            let h = hook(&model, t, 1.0);
            let s = stiffness_with(&model, Some(t)).unwrap();
            let ok = bits_equal(&h, &k) && bits_equal(&h, &s);
            println!(
                "H1 {name} threads {t}: {} ({} entries)",
                if ok { "bit-equal" } else { "DIFFERS" },
                h.val.len()
            );
            assert!(ok, "H1 {name} threads {t}");
        }
    }
}

#[test]
fn n2_hook_comparison_sees_one_ulp() {
    let (_, model) = hook_models().remove(0);
    let k = stiffness(&model).unwrap();
    let h = hook(&model, 1, 1.0 + f64::EPSILON);
    let ok = bits_equal(&h, &k);
    println!(
        "N2: scaled hook {}",
        if ok { "bit-equal (control FAILED)" } else { "unequal" }
    );
    assert!(!ok);
}

// ---------------------------------------------------------------------------------------------------------------- B5, N3

#[test]
fn b5_explicit_full_is_the_default() {
    let a = solve_static(&cantilever(), &StaticOptions::default().threads(1)).unwrap();
    let mut mesh = cantilever_mesh();
    mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::Full))
        .unwrap();
    let b = solve_static(&cantilever_on(mesh), &StaticOptions::default().threads(1)).unwrap();
    let ok = a.displacement_values().len() == b.displacement_values().len()
        && a.displacement_values()
            .iter()
            .zip(b.displacement_values())
            .all(|(x, y)| x.to_bits() == y.to_bits());
    println!("B5: explicit Full {}", if ok { "bit-equal" } else { "DIFFERS" });
    assert!(ok);
}

#[test]
fn n3_basic_model_is_not_refused() {
    let c = cantilever();
    assert!(solve_static(&c, &StaticOptions::default()).is_ok());
    assert!(modal_analysis(&c, &ModalOptions::new(2)).is_ok());
    assert!(stiffness(&c).is_ok());
    println!("N3: static, modal and stiffness of C accepted");
}

// ---------------------------------------------------------------------------------------------------------------- R1 to R11

/// The single Hex20 of R1: a 2 x 1 x 1 m box, corners then the mid-points of the hexahedron edges.
fn hex20_model() -> SolidModel {
    let corners: [Vec3; 8] = [
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [2.0, 0.0, 1.0],
        [2.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let mut nodes = corners.to_vec();
    for [a, b] in ElementKind::Hex20.edges() {
        let (p, q) = (corners[*a], corners[*b]);
        nodes.push([0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1]), 0.5 * (p[2] + q[2])]);
    }
    let xmin: Vec<usize> = (0..nodes.len()).filter(|&n| nodes[n][0] == 0.0).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex20, (0..20).collect()).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    mesh.add_node_set("xmin", xmin).unwrap();
    let mut m = with_material(mesh);
    clamp_xmin(&mut m);
    m
}

/// A Hex8 box [2, 1, 1] with a second block of a structural kind on the given nodes (by position), with its material.
fn mixed_model(kind: ElementKind, at: &[Vec3]) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let conn: Vec<usize> = at
        .iter()
        .map(|p| mesh.nodes().iter().position(|q| q == p).expect("a node of the box"))
        .collect();
    mesh.add_block(kind, conn).unwrap();
    mesh.add_element_set("structural", vec![2]).unwrap();
    let mut m = with_material(mesh);
    m.add_material("structural", steel()).unwrap();
    clamp_xmin(&mut m);
    m
}

fn cantilever_with_formulation(f: SolidFormulation) -> SolidModel {
    let mut mesh = cantilever_mesh();
    mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    cantilever_on(mesh)
}

fn faces_of(mesh: &SolidMesh, set: &str) -> Vec<FaceRef> {
    mesh.surface_set(set).unwrap().to_vec()
}

/// The registered refusal cases: name, model, item, variant, location, package.
#[allow(clippy::vec_init_then_push)] // one case per push, in the order of the registration
fn refusal_cases() -> Vec<(
    &'static str,
    SolidModel,
    &'static str,
    &'static str,
    String,
    &'static str,
)> {
    let mut v = Vec::new();
    v.push((
        "R1",
        hex20_model(),
        "element kind",
        "Hex20",
        "block 0".to_string(),
        "ADR 0292",
    ));
    v.push((
        "R2",
        mixed_model(ElementKind::Beam2, &[[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
        "element kind",
        "Beam2",
        "block 1".into(),
        "ADR 0294",
    ));
    v.push((
        "R3",
        mixed_model(
            ElementKind::Shell4,
            &[[2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [2.0, 1.0, 1.0], [2.0, 0.0, 1.0]],
        ),
        "element kind",
        "Shell4",
        "block 1".into(),
        "ADR 0295",
    ));
    v.push((
        "R4",
        cantilever_with_formulation(SolidFormulation::BBar),
        "formulation",
        "BBar",
        "block 0".into(),
        "ADR 0293",
    ));
    v.push((
        "R4b",
        cantilever_with_formulation(SolidFormulation::Incompatible),
        "formulation",
        "Incompatible",
        "block 0".into(),
        "ADR 0293",
    ));
    let mut m = cantilever();
    m.add_constraint(Constraint::linear(
        vec![LinearTerm::new(5, 0, 1.0), LinearTerm::new(7, 0, -1.0)],
        0.0,
    ))
    .unwrap();
    v.push((
        "R5",
        m,
        "constraint",
        "Linear",
        "constraint 3, first node 5".into(),
        "ADR 0296",
    ));
    let mut m = cantilever();
    m.add_constraint(Constraint::rigid_set("xmax", [1.0, 0.05, 0.05]))
        .unwrap();
    v.push((
        "R6",
        m,
        "constraint",
        "RigidSet",
        "constraint 3, node set 'xmax'".into(),
        "ADR 0296",
    ));
    let mut mesh = cantilever_mesh();
    let (low, high) = (faces_of(&mesh, "ymin"), faces_of(&mesh, "ymax"));
    mesh.add_surface_set("low", low).unwrap();
    mesh.add_surface_set("high", high).unwrap();
    let mut m = cantilever_on(mesh);
    m.add_constraint(Constraint::cyclic("low", "high", [1.0, 0.0, 0.0], [0.0; 3], 12))
        .unwrap();
    v.push((
        "R7",
        m,
        "constraint",
        "Cyclic",
        "constraint 3, surface sets 'low' and 'high'".into(),
        "ADR 0297",
    ));
    let mut m = cantilever();
    m.add_support(ElasticSupport::nodal_spring("xmax", 2, 1e6)).unwrap();
    v.push((
        "R8",
        m,
        "support",
        "NodalSpring",
        "support 0, node set 'xmax'".into(),
        "ADR 0296",
    ));
    let mut mesh = cantilever_mesh();
    let zmin = faces_of(&mesh, "zmin");
    mesh.add_surface_set("zmin_faces", zmin).unwrap();
    let mut m = cantilever_on(mesh);
    m.add_support(ElasticSupport::foundation("zmin_faces", 1e9)).unwrap();
    v.push((
        "R9",
        m,
        "support",
        "Foundation",
        "support 0, surface set 'zmin_faces'".into(),
        "ADR 0296",
    ));
    v
}

#[test]
#[ignore = "record of ADR 0291 before ADR 0292 (Hex20) and ADR 0296 (mpc) landed; superseded by the _after_ copy below (maintainer, 2026-11-13)"]
fn r1_to_r9_static_solve_refuses_each_new_variant() {
    for (name, model, item, variant, location, package) in refusal_cases() {
        expect_refusal(
            name,
            solve_static(&model, &StaticOptions::default()),
            item,
            variant,
            &location,
            package,
        );
    }
}

#[test]
#[ignore = "record of ADR 0291 before ADR 0292 (Hex20) and ADR 0296 (mpc) landed; superseded by the _after_ copy below (maintainer, 2026-11-13)"]
fn r10_refused_on_every_static_path() {
    let cg = StaticOptions::with_solver(StaticSolver::ConjugateGradient {
        tolerance: 1e-10,
        max_iterations: 10_000,
    });
    for (name, model, item, variant, location, package) in refusal_cases() {
        let what = format!("{name} cg");
        expect_refusal(
            &what,
            solve_static_with(&model, &[], &[], &cg),
            item,
            variant,
            &location,
            package,
        );
        let what = format!("{name} threads 8");
        let extra = [Prescribed::new(0, 0, 0.0)];
        expect_refusal(
            &what,
            solve_static_with(&model, &extra, &[], &StaticOptions::default().threads(8)),
            item,
            variant,
            &location,
            package,
        );
        let what = format!("{name} report");
        expect_refusal(
            &what,
            solve_static_report(&model, &[], &[], &StaticOptions::default()),
            item,
            variant,
            &location,
            package,
        );
    }
}

#[test]
#[ignore = "record of ADR 0291 before ADR 0292 (Hex20) and ADR 0296 (mpc) landed; superseded by the _after_ copy below (maintainer, 2026-11-13)"]
fn r11_modal_and_stiffness_refuse() {
    let cases = refusal_cases();
    let get = |n: &str| cases.iter().find(|c| c.0 == n).unwrap();
    for n in ["R4", "R8"] {
        let (name, model, item, variant, location, package) = get(n);
        expect_refusal(
            &format!("{name} modal"),
            modal_analysis(model, &ModalOptions::new(2)),
            item,
            variant,
            location,
            package,
        );
    }
    let (name, model, item, variant, location, package) = get("R4");
    expect_refusal(
        &format!("{name} stiffness"),
        stiffness(model),
        item,
        variant,
        location,
        package,
    );
}

// ---------------------------------------------------------------------------------------------------------------- V1

#[test]
fn v1_new_data_are_validated() {
    let mut refused = Vec::new();
    let mut note = |what: &str, r: Result<(), FemError>| {
        let e = r.expect_err(what);
        println!("V1 {what}: {e}");
        refused.push(what.to_string());
    };
    let mut hex = cantilever_mesh();
    note(
        "Shell on Hex8",
        hex.set_block_property(0, BlockProperty::shell(0.01).unwrap()),
    );
    let section = BeamSection::new(1e-4, 1e-9, 2e-9, 3e-9, 0.8, 0.8).unwrap();
    note(
        "Beam on Hex8",
        hex.set_block_property(0, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap()),
    );
    note(
        "block 9",
        hex.set_block_property(9, BlockProperty::solid(SolidFormulation::BBar)),
    );
    let mixed = mixed_model(ElementKind::Beam2, &[[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    let mut mixed_mesh = mixed.mesh().clone();
    note(
        "Solid on Beam2",
        mixed_mesh.set_block_property(1, BlockProperty::solid(SolidFormulation::Full)),
    );
    note("area 0", BeamSection::new(0.0, 1e-9, 2e-9, 3e-9, 0.8, 0.8).map(|_| ()));
    note("thickness -1", BlockProperty::shell(-1.0).map(|_| ()));
    let mut m = cantilever();
    let n = m.mesh().nodes().len();
    note("Linear with no term", m.add_constraint(Constraint::linear(vec![], 0.0)));
    let r = m.add_constraint(Constraint::linear(vec![LinearTerm::new(n + 7, 0, 1.0)], 0.0));
    assert!(
        matches!(r, Err(FemError::MissingNode { node, .. }) if node == n + 7),
        "{r:?}"
    );
    note("Linear on node n + 7", r);
    note(
        "Linear with zero coefficients",
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(1, 0, 0.0)], 0.0)),
    );
    note(
        "Cyclic with 1 sector",
        m.add_constraint(Constraint::cyclic("ymin", "ymax", [1.0, 0.0, 0.0], [0.0; 3], 1)),
    );
    note(
        "RigidSet on an unknown set",
        m.add_constraint(Constraint::rigid_set("nowhere", [0.0; 3])),
    );
    note(
        "NodalSpring stiffness 0",
        m.add_support(ElasticSupport::nodal_spring("xmax", 0, 0.0)),
    );
    note(
        "NodalSpring component 3",
        m.add_support(ElasticSupport::nodal_spring("xmax", 3, 1.0)),
    );
    assert_eq!(m.constraints().len(), 3);
    assert!(m.supports().is_empty());
    assert_eq!(refused.len(), 13);
}

// ---------------------------------------------------------------------------------------------------------------- copies
// Copies of X1, X3 and R1 to R11 without the rows of the packages that have landed: ADR 0292 (Hex20, F-389) and
// ADR 0296 (linear constraints, rigid sets and supports, F-393). Every other row is as registered in ADR 0291.

/// The packages that have landed; their refusal rows are retired.
const LANDED: [&str; 5] = ["R1", "R5", "R6", "R8", "R9"];

fn pending_refusal_cases() -> Vec<(
    &'static str,
    SolidModel,
    &'static str,
    &'static str,
    String,
    &'static str,
)> {
    refusal_cases().into_iter().filter(|c| !LANDED.contains(&c.0)).collect()
}

#[test]
#[ignore = "record of ADR 0291 after ADR 0292 and ADR 0296, before ADR 0386 (cyclic symmetry) retired the R7 row and cyclic::PENDING; superseded by the _after_cyclic_landed copy below (maintainer, 2026-11-13)"]
fn x1_the_scaffold_exists_after_landed() {
    assert_eq!(rcfd_fem::hex20::PENDING, None);
    assert_eq!(rcfd_fem::hex8x::PENDING, Some("ADR 0293"));
    assert_eq!(rcfd_fem::beam::PENDING, Some("ADR 0294"));
    assert_eq!(rcfd_fem::shell::PENDING, Some("ADR 0295"));
    assert_eq!(rcfd_fem::mpc::PENDING, None);
    assert_eq!(rcfd_fem::cyclic::PENDING, Some("ADR 0297"));
    assert_eq!(ElementKind::Hex20.nodes_per_element(), 20);
    assert_eq!(ElementKind::Hex20.pending(), None);
    assert_eq!(ElementKind::Beam2.pending(), Some("ADR 0294"));
    assert_eq!(ElementKind::Shell4.pending(), Some("ADR 0295"));
    assert!(ElementKind::Hex20.is_solid() && !ElementKind::Beam2.is_solid() && !ElementKind::Shell4.is_solid());
    assert_eq!(SolidFormulation::BBar.pending(), Some("ADR 0293"));
}

#[test]
#[ignore = "record of ADR 0291 before ADR 0417 (F-391, Beam2) retired the Beam2 row (refusals naming ADR 0294); superseded by the x3_placeholder_elements_refuse_after_beam_landed copy below (maintainer standing decision, 2026-11-13)"]
fn x3_placeholder_elements_refuse_after_landed() {
    let d = steel().elasticity_matrix().unwrap();
    for (kind, package) in [(ElementKind::Beam2, "ADR 0294"), (ElementKind::Shell4, "ADR 0295")] {
        let el = element_for(kind);
        let n = kind.nodes_per_element();
        let coords: Vec<Vec3> = (0..n).map(|a| [a as f64, (a * a) as f64, 1.0]).collect();
        let u = vec![0.0; 3 * n];
        let name = kind.name();
        expect_refusal(
            "stiffness",
            el.stiffness(&coords, &d),
            "element kind",
            name,
            "element routines",
            package,
        );
        expect_refusal(
            "integration_points",
            el.integration_points(&coords, quadrature::gauss_2()),
            "element kind",
            name,
            "element routines",
            package,
        );
        expect_refusal(
            "strain_at",
            el.strain_at(&coords, &u, [0.0; 3]),
            "element kind",
            name,
            "element routines",
            package,
        );
    }
}

#[test]
#[ignore = "record of ADR 0291 after ADR 0292 and ADR 0296, before ADR 0386 (cyclic symmetry) retired the R7 row and cyclic::PENDING; superseded by the _after_cyclic_landed copy below (maintainer, 2026-11-13)"]
fn r1_to_r9_static_solve_refuses_each_new_variant_after_landed() {
    let cases = pending_refusal_cases();
    assert_eq!(cases.len(), 5);
    for (name, model, item, variant, location, package) in cases {
        expect_refusal(
            name,
            solve_static(&model, &StaticOptions::default()),
            item,
            variant,
            &location,
            package,
        );
    }
}

#[test]
#[ignore = "record of ADR 0291 after ADR 0292 and ADR 0296, before ADR 0386 (cyclic symmetry) retired the R7 row and cyclic::PENDING; superseded by the _after_cyclic_landed copy below (maintainer, 2026-11-13)"]
fn r10_refused_on_every_static_path_after_landed() {
    let cg = StaticOptions::with_solver(StaticSolver::ConjugateGradient {
        tolerance: 1e-10,
        max_iterations: 10_000,
    });
    for (name, model, item, variant, location, package) in pending_refusal_cases() {
        expect_refusal(
            &format!("{name} cg"),
            solve_static_with(&model, &[], &[], &cg),
            item,
            variant,
            &location,
            package,
        );
        let extra = [Prescribed::new(0, 0, 0.0)];
        expect_refusal(
            &format!("{name} threads 8"),
            solve_static_with(&model, &extra, &[], &StaticOptions::default().threads(8)),
            item,
            variant,
            &location,
            package,
        );
        expect_refusal(
            &format!("{name} report"),
            solve_static_report(&model, &[], &[], &StaticOptions::default()),
            item,
            variant,
            &location,
            package,
        );
    }
}

#[test]
#[ignore = "record of ADR 0291 after ADR 0292 and ADR 0296, before ADR 0384 (modal analysis on the transformation) retired the R8 modal row; superseded by the _after_modal_landed copy below (maintainer, 2026-11-13)"]
fn r11_modal_and_stiffness_refuse_after_landed() {
    let cases = refusal_cases();
    let get = |n: &str| cases.iter().find(|c| c.0 == n).unwrap();
    let (name, model, item, variant, location, package) = get("R4");
    expect_refusal(
        &format!("{name} modal"),
        modal_analysis(model, &ModalOptions::new(2)),
        item,
        variant,
        location,
        package,
    );
    expect_refusal(
        &format!("{name} stiffness"),
        stiffness(model),
        item,
        variant,
        location,
        package,
    );
    // R8's modal refusal moved to the follow-up of ADR 0296 (modal analysis on the transformation).
    let (name, model, item, variant, location, _) = get("R8");
    expect_refusal(
        &format!("{name} modal"),
        modal_analysis(model, &ModalOptions::new(2)),
        item,
        variant,
        location,
        "a follow-up of ADR 0296 (modal analysis on the transformation)",
    );
}

/// R11 after ADR 0384: the modal analysis solves R8 (a spring) on the transformation; R4 is still refused.
#[test]
fn r11_modal_and_stiffness_refuse_after_modal_landed() {
    let cases = refusal_cases();
    let get = |n: &str| cases.iter().find(|c| c.0 == n).unwrap();
    let (name, model, item, variant, location, package) = get("R4");
    expect_refusal(
        &format!("{name} modal"),
        modal_analysis(model, &ModalOptions::new(2)),
        item,
        variant,
        location,
        package,
    );
    expect_refusal(
        &format!("{name} stiffness"),
        stiffness(model),
        item,
        variant,
        location,
        package,
    );
}

/// The packages that have landed after ADR 0386 (cyclic symmetry): R7 is retired as well.
const LANDED_AFTER_CYCLIC: [&str; 6] = ["R1", "R5", "R6", "R7", "R8", "R9"];

fn pending_refusal_cases_after_cyclic() -> Vec<(
    &'static str,
    SolidModel,
    &'static str,
    &'static str,
    String,
    &'static str,
)> {
    refusal_cases()
        .into_iter()
        .filter(|c| !LANDED_AFTER_CYCLIC.contains(&c.0))
        .collect()
}

#[test]
#[ignore = "record of ADR 0291 before ADR 0417 (F-391, Beam2) retired the beam::PENDING and Beam2.pending rows; superseded by the x1_the_scaffold_exists_after_beam_landed copy below (maintainer standing decision, 2026-11-13)"]
fn x1_the_scaffold_exists_after_cyclic_landed() {
    assert_eq!(rcfd_fem::hex20::PENDING, None);
    assert_eq!(rcfd_fem::hex8x::PENDING, Some("ADR 0293"));
    assert_eq!(rcfd_fem::beam::PENDING, Some("ADR 0294"));
    assert_eq!(rcfd_fem::shell::PENDING, Some("ADR 0295"));
    assert_eq!(rcfd_fem::mpc::PENDING, None);
    assert_eq!(rcfd_fem::cyclic::PENDING, None);
    assert_eq!(ElementKind::Hex20.nodes_per_element(), 20);
    assert_eq!(ElementKind::Hex20.pending(), None);
    assert_eq!(ElementKind::Beam2.pending(), Some("ADR 0294"));
    assert_eq!(ElementKind::Shell4.pending(), Some("ADR 0295"));
    assert!(ElementKind::Hex20.is_solid() && !ElementKind::Beam2.is_solid() && !ElementKind::Shell4.is_solid());
    assert_eq!(SolidFormulation::BBar.pending(), Some("ADR 0293"));
}

#[test]
#[ignore = "record of ADR 0291 before ADR 0417 (F-391, Beam2) retired the R2 row; superseded by the r1_to_r9_static_solve_refuses_each_new_variant_after_beam_landed copy below (maintainer standing decision, 2026-11-13)"]
fn r1_to_r9_static_solve_refuses_each_new_variant_after_cyclic_landed() {
    let cases = pending_refusal_cases_after_cyclic();
    assert_eq!(cases.len(), 4);
    for (name, model, item, variant, location, package) in cases {
        expect_refusal(
            name,
            solve_static(&model, &StaticOptions::default()),
            item,
            variant,
            &location,
            package,
        );
    }
}

#[test]
#[ignore = "record of ADR 0291 before ADR 0417 (F-391, Beam2) retired the R2 row; superseded by the r10_refused_on_every_static_path_after_beam_landed copy below (maintainer standing decision, 2026-11-13)"]
fn r10_refused_on_every_static_path_after_cyclic_landed() {
    let cg = StaticOptions::with_solver(StaticSolver::ConjugateGradient {
        tolerance: 1e-10,
        max_iterations: 10_000,
    });
    for (name, model, item, variant, location, package) in pending_refusal_cases_after_cyclic() {
        expect_refusal(
            &format!("{name} cg"),
            solve_static_with(&model, &[], &[], &cg),
            item,
            variant,
            &location,
            package,
        );
        let extra = [Prescribed::new(0, 0, 0.0)];
        expect_refusal(
            &format!("{name} threads 8"),
            solve_static_with(&model, &extra, &[], &StaticOptions::default().threads(8)),
            item,
            variant,
            &location,
            package,
        );
        expect_refusal(
            &format!("{name} report"),
            solve_static_report(&model, &[], &[], &StaticOptions::default()),
            item,
            variant,
            &location,
            package,
        );
    }
}

/// X2 after ADR 0390 (F-395) landed: `geometric_stiffness` of the solid elements returns the matrix, the two entry points of
/// ADR 0300 still refuse.
#[test]
#[ignore = "record of ADR 0390 before ADR 0404 (F-397, geometric nonlinearity) retired the internal_force and tangent rows; no row stays valid (maintainer, 2026-11-13)"]
fn x2_entry_points_refuse_by_default_after_geometric_landed() {
    let mat = steel();
    let state = ElementState::empty();
    for kind in [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10] {
        let el = element_for(kind);
        let coords: Vec<Vec3> = kind.reference_nodes().to_vec();
        let u = vec![0.0; 3 * kind.nodes_per_element()];
        let loc = format!("element kind {}", kind.name());
        expect_refusal(
            "internal_force",
            el.internal_force(&coords, &mat, &u, &state),
            "element entry point",
            "internal_force",
            &loc,
            "ADR 0300",
        );
        expect_refusal(
            "tangent",
            el.tangent(&coords, &mat, &u, &state),
            "element entry point",
            "tangent",
            &loc,
            "ADR 0300",
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------- copies
// Copies of X1, X3, R1 to R9 and R10 after ADR 0417 (F-391, Beam2): beam::PENDING and Beam2.pending are None, the
// solid-element routines of Beam2 refuse naming beam::SOLID_ROUTINES, and the R2 model (a beam on nodes of the Hex8 box) is
// refused as a beam node shared with a solid (si_beam.rs S1), so its row is retired. Every other row as registered.

/// The packages that have landed after ADR 0417 (Beam2): R2 is retired as well.
const LANDED_AFTER_BEAM: [&str; 7] = ["R1", "R2", "R5", "R6", "R7", "R8", "R9"];

fn pending_refusal_cases_after_beam() -> Vec<(
    &'static str,
    SolidModel,
    &'static str,
    &'static str,
    String,
    &'static str,
)> {
    refusal_cases()
        .into_iter()
        .filter(|c| !LANDED_AFTER_BEAM.contains(&c.0))
        .collect()
}

#[test]
fn x1_the_scaffold_exists_after_beam_landed() {
    assert_eq!(rcfd_fem::hex20::PENDING, None);
    assert_eq!(rcfd_fem::hex8x::PENDING, Some("ADR 0293"));
    assert_eq!(rcfd_fem::beam::PENDING, None);
    assert_eq!(rcfd_fem::shell::PENDING, Some("ADR 0295"));
    assert_eq!(rcfd_fem::mpc::PENDING, None);
    assert_eq!(rcfd_fem::cyclic::PENDING, None);
    assert_eq!(ElementKind::Hex20.nodes_per_element(), 20);
    assert_eq!(ElementKind::Hex20.pending(), None);
    assert_eq!(ElementKind::Beam2.pending(), None);
    assert_eq!(ElementKind::Shell4.pending(), Some("ADR 0295"));
    assert!(ElementKind::Hex20.is_solid() && !ElementKind::Beam2.is_solid() && !ElementKind::Shell4.is_solid());
    assert_eq!(SolidFormulation::BBar.pending(), Some("ADR 0293"));
}

#[test]
fn x3_placeholder_elements_refuse_after_beam_landed() {
    let d = steel().elasticity_matrix().unwrap();
    for (kind, package) in [
        (ElementKind::Beam2, rcfd_fem::beam::SOLID_ROUTINES),
        (ElementKind::Shell4, "ADR 0295"),
    ] {
        let el = element_for(kind);
        let n = kind.nodes_per_element();
        let coords: Vec<Vec3> = (0..n).map(|a| [a as f64, (a * a) as f64, 1.0]).collect();
        let u = vec![0.0; 3 * n];
        let name = kind.name();
        expect_refusal(
            "stiffness",
            el.stiffness(&coords, &d),
            "element kind",
            name,
            "element routines",
            package,
        );
        expect_refusal(
            "integration_points",
            el.integration_points(&coords, quadrature::gauss_2()),
            "element kind",
            name,
            "element routines",
            package,
        );
        expect_refusal(
            "strain_at",
            el.strain_at(&coords, &u, [0.0; 3]),
            "element kind",
            name,
            "element routines",
            package,
        );
    }
}

#[test]
fn r1_to_r9_static_solve_refuses_each_new_variant_after_beam_landed() {
    let cases = pending_refusal_cases_after_beam();
    assert_eq!(cases.len(), 3);
    for (name, model, item, variant, location, package) in cases {
        expect_refusal(
            name,
            solve_static(&model, &StaticOptions::default()),
            item,
            variant,
            &location,
            package,
        );
    }
}

#[test]
fn r10_refused_on_every_static_path_after_beam_landed() {
    let cg = StaticOptions::with_solver(StaticSolver::ConjugateGradient {
        tolerance: 1e-10,
        max_iterations: 10_000,
    });
    for (name, model, item, variant, location, package) in pending_refusal_cases_after_beam() {
        expect_refusal(
            &format!("{name} cg"),
            solve_static_with(&model, &[], &[], &cg),
            item,
            variant,
            &location,
            package,
        );
        let extra = [Prescribed::new(0, 0, 0.0)];
        expect_refusal(
            &format!("{name} threads 8"),
            solve_static_with(&model, &extra, &[], &StaticOptions::default().threads(8)),
            item,
            variant,
            &location,
            package,
        );
        expect_refusal(
            &format!("{name} report"),
            solve_static_report(&model, &[], &[], &StaticOptions::default()),
            item,
            variant,
            &location,
            package,
        );
    }
}
